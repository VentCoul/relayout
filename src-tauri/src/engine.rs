//! What happens when the hotkey is pressed.
//!
//! Privacy rules for this file: the user's text lives only in memory for the
//! duration of one conversion, is wiped afterwards, and is never logged,
//! written to disk or sent anywhere.

use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use arboard::{Clipboard, ImageData};
use relayout_core::Lang;
use tauri::AppHandle;
use zeroize::Zeroize;

use crate::platform;

static BUSY: AtomicBool = AtomicBool::new(false);

const COPY_TIMEOUT: Duration = Duration::from_millis(300);
const PASTE_SETTLE: Duration = Duration::from_millis(200);
const KEY_SETTLE: Duration = Duration::from_millis(60);

enum Saved {
    Text(String),
    Image(ImageData<'static>),
    Nothing,
}

pub fn trigger(app: AppHandle) {
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    thread::spawn(move || {
        run(&app);
        BUSY.store(false, Ordering::SeqCst);
    });
}

fn on_main<T: Send + 'static>(app: &AppHandle, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(f());
    })
    .ok()?;
    rx.recv_timeout(Duration::from_secs(2)).ok()
}

fn run(app: &AppHandle) {
    if !platform::has_permission(true) {
        return;
    }
    let Some((hint, (en, uk))) = on_main(app, || (platform::current_lang(), platform::tables()))
    else {
        return;
    };
    let Ok(mut clipboard) = Clipboard::new() else {
        return;
    };

    let saved = save(&mut clipboard);
    let _ = clipboard.clear();

    // Let the hotkey's own key-up reach the focused app before we type.
    thread::sleep(KEY_SETTLE);
    platform::wait_for_modifiers_release();
    platform::copy();
    let mut text = wait_for_text(&mut clipboard);
    if text.is_none() {
        // Nothing was selected: take the word before the cursor.
        platform::select_previous_word();
        thread::sleep(KEY_SETTLE);
        platform::copy();
        text = wait_for_text(&mut clipboard);
    }

    let mut target: Option<Lang> = None;
    if let Some(mut original) = text {
        if let Some(mut converted) = relayout_core::convert(&original, en, uk, hint) {
            if set_private_text(&mut clipboard, &converted.text) {
                platform::paste();
                thread::sleep(PASTE_SETTLE);
                target = Some(converted.target);
            }
            converted.text.zeroize();
        }
        original.zeroize();
    }

    restore(&mut clipboard, saved);

    if let Some(lang) = target {
        on_main(app, move || platform::switch_to(lang));
    }
}

fn wait_for_text(clipboard: &mut Clipboard) -> Option<String> {
    let deadline = Instant::now() + COPY_TIMEOUT;
    while Instant::now() < deadline {
        match clipboard.get_text() {
            Ok(text) if !text.is_empty() => return Some(text),
            _ => thread::sleep(Duration::from_millis(15)),
        }
    }
    None
}

fn save(clipboard: &mut Clipboard) -> Saved {
    if let Ok(text) = clipboard.get_text() {
        return Saved::Text(text);
    }
    if let Ok(image) = clipboard.get_image() {
        return Saved::Image(ImageData {
            width: image.width,
            height: image.height,
            bytes: Cow::Owned(image.bytes.into_owned()),
        });
    }
    Saved::Nothing
}

fn restore(clipboard: &mut Clipboard, saved: Saved) {
    match saved {
        Saved::Text(mut text) => {
            let _ = clipboard.set_text(text.as_str());
            text.zeroize();
        }
        Saved::Image(image) => {
            let _ = clipboard.set_image(image);
        }
        Saved::Nothing => {
            let _ = clipboard.clear();
        }
    }
}

/// Puts text on the clipboard flagged as concealed, so clipboard managers,
/// clipboard history and cloud clipboard sync skip it.
fn set_private_text(clipboard: &mut Clipboard, text: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        use arboard::SetExtApple;
        clipboard.set().exclude_from_history().text(text).is_ok()
    }
    #[cfg(target_os = "windows")]
    {
        use arboard::SetExtWindows;
        clipboard
            .set()
            .exclude_from_history()
            .exclude_from_cloud()
            .exclude_from_monitoring()
            .text(text)
            .is_ok()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        clipboard.set_text(text).is_ok()
    }
}
