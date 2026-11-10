//! Windows layer.

use std::mem::size_of;
use std::thread;
use std::time::{Duration, Instant};

use relayout_core::layouts::{EN_US, UK_WIN, UK_WIN_ENHANCED};
use relayout_core::{Lang, Layout};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardLayout, GetKeyboardLayoutList, MapVirtualKeyExW, SendInput,
    ToUnicodeEx, GetKeyState, HKL, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CAPITAL, VK_CONTROL, VK_LEFT, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, PostMessageW, WM_INPUTLANGCHANGEREQUEST,
};

const VK_C: VIRTUAL_KEY = 0x43;
const VK_V: VIRTUAL_KEY = 0x56;
const PRIMARY_ENGLISH: usize = 0x09;
const PRIMARY_UKRAINIAN: usize = 0x22;
/// Scan code of the `\` key, the one key where the two Ukrainian layouts differ.
const SCAN_BACKSLASH: u32 = 0x2B;
const MAPVK_VSC_TO_VK_EX: u32 = 3;
/// ToUnicodeEx flag: do not touch the keyboard state.
const TO_UNICODE_NO_STATE_CHANGE: u32 = 4;

/// Windows needs no permission to send keystrokes.
pub fn has_permission(_prompt: bool) -> bool {
    true
}

fn lang_of(layout: HKL) -> Option<Lang> {
    match (layout as usize & 0xFFFF) & 0x3FF {
        PRIMARY_ENGLISH => Some(Lang::En),
        PRIMARY_UKRAINIAN => Some(Lang::Uk),
        _ => None,
    }
}

fn installed() -> Vec<HKL> {
    unsafe {
        let count = GetKeyboardLayoutList(0, std::ptr::null_mut());
        if count <= 0 {
            return Vec::new();
        }
        let mut list: Vec<HKL> = vec![std::ptr::null_mut(); count as usize];
        let written = GetKeyboardLayoutList(count, list.as_mut_ptr());
        list.truncate(written.max(0) as usize);
        list
    }
}

/// Layout of the window the user is typing in (each window has its own).
fn foreground_layout() -> HKL {
    unsafe {
        let window = GetForegroundWindow();
        let thread = GetWindowThreadProcessId(window, std::ptr::null_mut());
        GetKeyboardLayout(thread)
    }
}

pub fn current_lang() -> Option<Lang> {
    lang_of(foreground_layout())
}

/// "Ukrainian (Enhanced)" has ґ on the `\` key; the legacy layout does not.
fn is_enhanced(layout: HKL) -> bool {
    unsafe {
        let key = MapVirtualKeyExW(SCAN_BACKSLASH, MAPVK_VSC_TO_VK_EX, layout);
        let state = [0u8; 256];
        let mut out = [0u16; 8];
        let written = ToUnicodeEx(
            key,
            SCAN_BACKSLASH,
            state.as_ptr(),
            out.as_mut_ptr(),
            out.len() as i32,
            TO_UNICODE_NO_STATE_CHANGE,
            layout,
        );
        written == 1 && out[0] == 'ґ' as u16
    }
}

pub fn tables() -> (&'static Layout, &'static Layout) {
    let ukrainian = installed().into_iter().find(|&l| lang_of(l) == Some(Lang::Uk));
    let uk = match ukrainian {
        Some(layout) if !is_enhanced(layout) => &UK_WIN,
        _ => &UK_WIN_ENHANCED,
    };
    (&EN_US, uk)
}

/// Asks the focused window to switch to the first installed layout of `lang`.
pub fn switch_to(lang: Lang) {
    if let Some(layout) = installed().into_iter().find(|&l| lang_of(l) == Some(lang)) {
        unsafe {
            PostMessageW(GetForegroundWindow(), WM_INPUTLANGCHANGEREQUEST, 0, layout as isize);
        }
    }
}

/// Windows cannot hide physically held modifiers from synthetic keystrokes,
/// so Ctrl+C sent while the user still holds Shift would arrive as
/// Ctrl+Shift+C. Wait until the hotkey's modifiers are released.
pub fn wait_for_modifiers_release() {
    let deadline = Instant::now() + Duration::from_millis(1500);
    let held = || {
        [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
            .iter()
            .any(|&key| unsafe { GetAsyncKeyState(key as i32) } < 0)
    };
    while held() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
}

fn input(key: VIRTUAL_KEY, up: bool) -> INPUT {
    let mut flags = if up { KEYEVENTF_KEYUP } else { 0 };
    if key == VK_LEFT {
        // Without this, Shift+Left is treated as a numpad key and loses Shift.
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: key, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 },
        },
    }
}

/// Presses `key` while holding `modifiers`, then releases everything.
fn chord(modifiers: &[VIRTUAL_KEY], key: VIRTUAL_KEY) {
    let mut events: Vec<INPUT> = modifiers.iter().map(|&m| input(m, false)).collect();
    events.push(input(key, false));
    events.push(input(key, true));
    events.extend(modifiers.iter().rev().map(|&m| input(m, true)));
    unsafe {
        SendInput(events.len() as u32, events.as_ptr(), size_of::<INPUT>() as i32);
    }
}

pub fn copy() {
    chord(&[VK_CONTROL], VK_C);
}

pub fn paste() {
    chord(&[VK_CONTROL], VK_V);
}

pub fn select_previous_word() {
    chord(&[VK_CONTROL, VK_SHIFT], VK_LEFT);
}

/// Caps Lock is a toggle: press it once if it is on.
pub fn caps_lock_off() {
    if unsafe { GetKeyState(VK_CAPITAL as i32) } & 1 != 0 {
        chord(&[], VK_CAPITAL);
    }
}
