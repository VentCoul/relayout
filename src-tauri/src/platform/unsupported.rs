//! Placeholder for platforms whose layer is not written yet (BSD and others).

use relayout_core::layouts::{EN_US, UK_WIN_ENHANCED};
use relayout_core::{Lang, Layout};

pub fn has_permission(_prompt: bool) -> bool {
    false
}

pub fn current_lang() -> Option<Lang> {
    None
}

pub fn tables() -> (&'static Layout, &'static Layout) {
    (&EN_US, &UK_WIN_ENHANCED)
}

pub fn switch_to(_lang: Lang) {}

pub fn wait_for_modifiers_release() {}

pub fn caps_lock_off() {}

pub fn copy() {}

pub fn paste() {}

pub fn select_previous_word() {}
