//! Linux layer, X11 only.
//!
//! Keystrokes are sent with the XTest extension and the layout is read and
//! switched through XKB groups. The layout tables are not hard-coded: they are
//! read from the X server's live keymap, so whatever variant of the Ukrainian
//! layout the user has (legacy, winkeys, unicode...) converts correctly.
//!
//! Under Wayland this only reaches applications running through XWayland.

use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use relayout_core::layouts::{EN_US, NO_CHAR, UK_WIN_ENHANCED};
use relayout_core::{Lang, Layout};
use x11rb::connection::Connection;
use x11rb::protocol::xkb::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{self, ConnectionExt as _, Keycode};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

/// evdev keycodes of the 47 keys, in the order the layout tables use.
const KEYCODES: [Keycode; 47] = [
    49, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, // ` 1 .. 0 - =
    24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 51, // q .. ] \
    38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, // a .. '
    52, 53, 54, 55, 56, 57, 58, 59, 60, 61, // z .. /
];

const XK_SHIFT_L: u32 = 0xffe1;
const XK_CONTROL_L: u32 = 0xffe3;
const XK_LEFT: u32 = 0xff51;
const XK_C: u32 = 0x63;
const XK_V: u32 = 0x76;
/// Control, Shift, Alt, Super and Meta, left and right.
const MODIFIER_KEYSYMS: [u32; 10] =
    [0xffe1, 0xffe2, 0xffe3, 0xffe4, 0xffe7, 0xffe8, 0xffe9, 0xffea, 0xffeb, 0xffec];

/// Characters of the legacy Cyrillic keysyms 0x6c0..=0x6df (lower case; the
/// upper-case keysyms 0x6e0..=0x6ff follow the same order).
const CYRILLIC_LOWER: &str = "юабцдефгхийклмнопярстужвьызшэщчъ";
/// Keysyms 0x6a1..=0x6bf.
const CYRILLIC_EXTRA: &str = "ђѓёєѕіїјљњћќґўџ№ЂЃЁЄЅІЇЈЉЊЋЌҐЎЏ";

struct Keymap {
    min_keycode: Keycode,
    per_keycode: usize,
    keysyms: Vec<u32>,
}

impl Keymap {
    fn read(conn: &RustConnection) -> Option<Keymap> {
        let setup = conn.setup();
        let (min, max) = (setup.min_keycode, setup.max_keycode);
        let reply = conn.get_keyboard_mapping(min, max - min + 1).ok()?.reply().ok()?;
        Some(Keymap {
            min_keycode: min,
            per_keycode: reply.keysyms_per_keycode as usize,
            keysyms: reply.keysyms,
        })
    }

    fn get(&self, keycode: Keycode, index: usize) -> u32 {
        if index >= self.per_keycode || keycode < self.min_keycode {
            return 0;
        }
        let at = (keycode - self.min_keycode) as usize * self.per_keycode + index;
        self.keysyms.get(at).copied().unwrap_or(0)
    }

    /// In the core keymap, group 1 is at indices 0-1 and group 2 at 2-3.
    /// Groups 3 and 4 are not reliably exposed there, so they are not used.
    fn group_char(&self, group: usize, keycode: Keycode, shift: bool) -> char {
        keysym_to_char(self.get(keycode, group * 2 + shift as usize)).unwrap_or(NO_CHAR)
    }

    fn group_count(&self) -> usize {
        (self.per_keycode / 2).min(2)
    }

    fn keycode_of(&self, keysym: u32) -> Option<Keycode> {
        (self.min_keycode..=u8::MAX)
            .find(|&code| (0..self.per_keycode).any(|i| self.get(code, i) == keysym))
    }

    fn lang_of_group(&self, group: usize) -> Option<Lang> {
        let at = |i: usize| self.group_char(group, KEYCODES[i], false);
        // Keys Q, Y, A and ]: "q y a ]" on US / UK, "й н ф ї" on Ukrainian.
        match (at(13), at(18), at(26), at(24)) {
            ('q', 'y', 'a', _) => Some(Lang::En),
            ('й', 'н', 'ф', 'ї') => Some(Lang::Uk),
            _ => None,
        }
    }

    fn group_of(&self, lang: Lang) -> Option<usize> {
        (0..self.group_count()).find(|&g| self.lang_of_group(g) == Some(lang))
    }

    fn table(&self, group: usize, lang: Lang) -> &'static Layout {
        let row = |shift| KEYCODES.iter().map(|&k| self.group_char(group, k, shift)).collect();
        cached_layout(lang, row(false), row(true))
    }
}

fn keysym_to_char(keysym: u32) -> Option<char> {
    match keysym {
        0x20..=0x7e | 0xa0..=0xff => char::from_u32(keysym),
        0x6a1..=0x6bf => CYRILLIC_EXTRA.chars().nth((keysym - 0x6a1) as usize),
        0x6c0..=0x6df => CYRILLIC_LOWER.chars().nth((keysym - 0x6c0) as usize),
        0x6e0..=0x6ff => CYRILLIC_LOWER
            .chars()
            .nth((keysym - 0x6e0) as usize)
            .and_then(|c| c.to_uppercase().next()),
        0x0100_0100..=0x0110_ffff => char::from_u32(keysym - 0x0100_0000),
        _ => None,
    }
}

/// Tables read from the keymap have to live as long as the static ones.
/// Each distinct keymap is allocated once and then reused.
fn cached_layout(lang: Lang, base: String, shift: String) -> &'static Layout {
    static CACHE: Mutex<Vec<&'static Layout>> = Mutex::new(Vec::new());
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(found) = cache.iter().find(|l| l.lang == lang && l.base == base && l.shift == shift) {
        return found;
    }
    let layout: &'static Layout = Box::leak(Box::new(Layout {
        id: "x11-keymap",
        lang,
        base: Box::leak(base.into_boxed_str()),
        shift: Box::leak(shift.into_boxed_str()),
    }));
    cache.push(layout);
    layout
}

fn connect() -> Option<RustConnection> {
    let (conn, _) = x11rb::connect(None).ok()?;
    conn.xkb_use_extension(1, 0).ok()?.reply().ok()?;
    Some(conn)
}

fn current_group(conn: &RustConnection) -> Option<usize> {
    let state = conn.xkb_get_state(xkb::ID::USE_CORE_KBD.into()).ok()?.reply().ok()?;
    Some(u8::from(state.group) as usize)
}

/// Works whenever an X server is reachable; X11 needs no permission.
pub fn has_permission(_prompt: bool) -> bool {
    connect().is_some()
}

pub fn current_lang() -> Option<Lang> {
    let conn = connect()?;
    let keymap = Keymap::read(&conn)?;
    let group = current_group(&conn)?;
    (group < keymap.group_count()).then(|| keymap.lang_of_group(group)).flatten()
}

pub fn tables() -> (&'static Layout, &'static Layout) {
    let Some(keymap) = connect().and_then(|conn| Keymap::read(&conn)) else {
        return (&EN_US, &UK_WIN_ENHANCED);
    };
    let en = keymap.group_of(Lang::En).map_or(&EN_US, |g| keymap.table(g, Lang::En));
    let uk = keymap.group_of(Lang::Uk).map_or(&UK_WIN_ENHANCED, |g| keymap.table(g, Lang::Uk));
    (en, uk)
}

pub fn switch_to(lang: Lang) {
    let Some(conn) = connect() else { return };
    let Some(group) = Keymap::read(&conn).and_then(|k| k.group_of(lang)) else { return };
    let none = xproto::ModMask::from(0u16);
    // Wait for the server's answer: closing the connection right after
    // sending the request can drop it before it is applied.
    let _ = conn
        .xkb_latch_lock_state(
            xkb::ID::USE_CORE_KBD.into(),
            none,
            none,
            true,
            xkb::Group::from(group as u8),
            none,
            false,
            0,
        )
        .map(|cookie| cookie.check());
}

/// Caps Lock is the XKB "Lock" modifier: unlock it.
pub fn caps_lock_off() {
    let Some(conn) = connect() else { return };
    let none = xproto::ModMask::from(0u16);
    let _ = conn
        .xkb_latch_lock_state(
            xkb::ID::USE_CORE_KBD.into(),
            xproto::ModMask::LOCK,
            none,
            false,
            xkb::Group::M1,
            none,
            false,
            0,
        )
        .map(|cookie| cookie.check());
}

/// X11 delivers synthetic keystrokes together with the modifiers the user is
/// physically holding, so wait until the hotkey is fully released.
pub fn wait_for_modifiers_release() {
    let Some(conn) = connect() else { return };
    let Some(keymap) = Keymap::read(&conn) else { return };
    let codes: Vec<Keycode> =
        MODIFIER_KEYSYMS.iter().filter_map(|&s| keymap.keycode_of(s)).collect();
    let deadline = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < deadline {
        let Some(pressed) = conn.query_keymap().ok().and_then(|c| c.reply().ok()) else {
            return;
        };
        let held = codes.iter().any(|&k| pressed.keys[k as usize / 8] & (1 << (k % 8)) != 0);
        if !held {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// Presses `key` while holding `modifiers`, then releases everything.
fn chord(modifiers: &[u32], key: u32) {
    let Some(conn) = connect() else { return };
    let Some(keymap) = Keymap::read(&conn) else { return };
    let Some(codes) = modifiers
        .iter()
        .chain(std::iter::once(&key))
        .map(|&s| keymap.keycode_of(s))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let root = conn.setup().roots[0].root;
    let send = |kind: u8, code: Keycode| {
        let _ = conn.xtest_fake_input(kind, code, 0, root, 0, 0, 0);
    };
    for &code in &codes {
        send(xproto::KEY_PRESS_EVENT, code);
    }
    for &code in codes.iter().rev() {
        send(xproto::KEY_RELEASE_EVENT, code);
    }
    // Round trip so the server has processed the events before we go on.
    let _ = conn.get_input_focus().map(|c| c.reply());
}

pub fn copy() {
    chord(&[XK_CONTROL_L], XK_C);
}

pub fn paste() {
    chord(&[XK_CONTROL_L], XK_V);
}

pub fn select_previous_word() {
    chord(&[XK_CONTROL_L, XK_SHIFT_L], XK_LEFT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cyrillic_keysyms() {
        assert_eq!(keysym_to_char(0x6c1), Some('а'));
        assert_eq!(keysym_to_char(0x6e1), Some('А'));
        assert_eq!(keysym_to_char(0x6ca), Some('й'));
        assert_eq!(keysym_to_char(0x6c0), Some('ю'));
        assert_eq!(keysym_to_char(0x6e0), Some('Ю'));
        assert_eq!(keysym_to_char(0x6a6), Some('і'));
        assert_eq!(keysym_to_char(0x6a7), Some('ї'));
        assert_eq!(keysym_to_char(0x6a4), Some('є'));
        assert_eq!(keysym_to_char(0x6ad), Some('ґ'));
        assert_eq!(keysym_to_char(0x6bd), Some('Ґ'));
        assert_eq!(keysym_to_char(0x6b0), Some('№'));
        assert_eq!(keysym_to_char(0x10002bc), Some('ʼ'));
        assert_eq!(keysym_to_char(0x27), Some('\''));
        assert_eq!(keysym_to_char(0xff51), None);
    }

    #[test]
    fn table_sizes() {
        assert_eq!(CYRILLIC_LOWER.chars().count(), 32);
        assert_eq!(CYRILLIC_EXTRA.chars().count(), 31);
    }
}
