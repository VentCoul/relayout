use std::ffi::c_void;

use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
use core_foundation::base::{CFRelease, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use relayout_core::layouts::{EN_US, UK_MAC, UK_MAC_PC};
use relayout_core::{Lang, Layout};

type TISInputSourceRef = *mut c_void;

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    static kTISPropertyInputSourceID: CFStringRef;
    fn TISCopyCurrentKeyboardLayoutInputSource() -> TISInputSourceRef;
    fn TISCreateInputSourceList(properties: CFDictionaryRef, include_all: u8) -> CFArrayRef;
    fn TISGetInputSourceProperty(source: TISInputSourceRef, key: CFStringRef) -> *const c_void;
    fn TISSelectInputSource(source: TISInputSourceRef) -> i32;
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOServiceMatching(name: *const std::ffi::c_char) -> *mut c_void;
    fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> u32;
    fn IOServiceOpen(service: u32, owning_task: u32, kind: u32, connect: *mut u32) -> i32;
    fn IOServiceClose(connect: u32) -> i32;
    fn IOObjectRelease(object: u32) -> i32;
    fn IOHIDSetModifierLockState(connect: u32, selector: i32, state: bool) -> i32;
}

extern "C" {
    static mach_task_self_: u32;
}

const IOHID_PARAM_CONNECT_TYPE: u32 = 1;
const IOHID_CAPS_LOCK_STATE: i32 = 1;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    static kAXTrustedCheckOptionPrompt: CFStringRef;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
}

const KEY_C: CGKeyCode = 8;
const KEY_V: CGKeyCode = 9;
const KEY_LEFT: CGKeyCode = 123;

const ENGLISH_IDS: [&str; 8] = [
    "com.apple.keylayout.US",
    "com.apple.keylayout.ABC",
    "com.apple.keylayout.USExtended",
    "com.apple.keylayout.USInternational-PC",
    "com.apple.keylayout.British",
    "com.apple.keylayout.British-PC",
    "com.apple.keylayout.Australian",
    "com.apple.keylayout.Canadian",
];
const UK_PC_ID: &str = "com.apple.keylayout.Ukrainian-PC";
const UK_ID: &str = "com.apple.keylayout.Ukrainian";

/// Whether macOS lets this app send keystrokes (Accessibility permission).
/// With `prompt`, macOS shows its own dialog pointing to System Settings.
pub fn has_permission(prompt: bool) -> bool {
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let value = if prompt { CFBoolean::true_value() } else { CFBoolean::false_value() };
        let options = CFDictionary::from_CFType_pairs(&[(key, value)]);
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0
    }
}

unsafe fn source_id(source: TISInputSourceRef) -> Option<String> {
    let value = TISGetInputSourceProperty(source, kTISPropertyInputSourceID) as CFStringRef;
    if value.is_null() {
        return None;
    }
    Some(CFString::wrap_under_get_rule(value).to_string())
}

fn lang_of(id: &str) -> Option<Lang> {
    if id == UK_PC_ID || id == UK_ID {
        Some(Lang::Uk)
    } else if ENGLISH_IDS.contains(&id) {
        Some(Lang::En)
    } else {
        None
    }
}

/// Main thread. Id of the active keyboard layout.
fn current_id() -> Option<String> {
    unsafe {
        let source = TISCopyCurrentKeyboardLayoutInputSource();
        if source.is_null() {
            return None;
        }
        let id = source_id(source);
        CFRelease(source as *const c_void);
        id
    }
}

/// Main thread. Calls `f` for every layout enabled in System Settings.
fn for_each_enabled(mut f: impl FnMut(&str, TISInputSourceRef)) {
    unsafe {
        let list = TISCreateInputSourceList(std::ptr::null(), 0);
        if list.is_null() {
            return;
        }
        for i in 0..CFArrayGetCount(list) {
            let source = CFArrayGetValueAtIndex(list, i) as TISInputSourceRef;
            if let Some(id) = source_id(source) {
                f(&id, source);
            }
        }
        CFRelease(list as *const c_void);
    }
}

/// Main thread. Language of the active layout, if it is one we know.
pub fn current_lang() -> Option<Lang> {
    current_id().as_deref().and_then(lang_of)
}

/// Main thread. The English and Ukrainian tables matching the user's layouts.
pub fn tables() -> (&'static Layout, &'static Layout) {
    let current = current_id();
    let mut has_pc = false;
    let mut has_native = false;
    for_each_enabled(|id, _| {
        has_pc |= id == UK_PC_ID;
        has_native |= id == UK_ID;
    });
    let uk = match (has_pc, has_native) {
        (false, true) => &UK_MAC,
        (true, true) if current.as_deref() == Some(UK_ID) => &UK_MAC,
        _ => &UK_MAC_PC,
    };
    (&EN_US, uk)
}

/// Main thread. Activates the first enabled layout of the given language.
pub fn switch_to(lang: Lang) {
    let mut done = false;
    for_each_enabled(|id, source| {
        if !done && lang_of(id) == Some(lang) {
            unsafe { TISSelectInputSource(source) };
            done = true;
        }
    });
}

/// Sends one key press with exactly the given modifiers. A private event
/// source keeps the modifiers of the hotkey the user may still be holding
/// from leaking into the keystroke.
fn key(code: CGKeyCode, flags: CGEventFlags) {
    let Ok(source) = CGEventSource::new(CGEventSourceStateID::Private) else {
        return;
    };
    for down in [true, false] {
        if let Ok(event) = CGEvent::new_keyboard_event(source.clone(), code, down) {
            event.set_flags(flags);
            event.post(CGEventTapLocation::Session);
        }
    }
}

/// Not needed on macOS: `key` sends keystrokes with explicit modifiers.
pub fn wait_for_modifiers_release() {}

pub fn copy() {
    key(KEY_C, CGEventFlags::CGEventFlagCommand);
}

pub fn paste() {
    key(KEY_V, CGEventFlags::CGEventFlagCommand);
}

pub fn select_previous_word() {
    key(KEY_LEFT, CGEventFlags::CGEventFlagAlternate | CGEventFlags::CGEventFlagShift);
}

/// Runs `f` with a connection to the HID system, which owns the Caps Lock state.
fn with_hid_system<T>(f: impl FnOnce(u32) -> T) -> Option<T> {
    unsafe {
        let matching = IOServiceMatching(c"IOHIDSystem".as_ptr());
        // Takes ownership of `matching`. Port 0 is the default main port.
        let service = IOServiceGetMatchingService(0, matching);
        if service == 0 {
            return None;
        }
        let mut connect = 0;
        let opened = IOServiceOpen(service, mach_task_self_, IOHID_PARAM_CONNECT_TYPE, &mut connect);
        IOObjectRelease(service);
        if opened != 0 {
            return None;
        }
        let result = f(connect);
        IOServiceClose(connect);
        Some(result)
    }
}

/// Turns Caps Lock off. Needs no permission beyond the app's own.
pub fn caps_lock_off() {
    with_hid_system(|connect| unsafe {
        IOHIDSetModifierLockState(connect, IOHID_CAPS_LOCK_STATE, false);
    });
}
