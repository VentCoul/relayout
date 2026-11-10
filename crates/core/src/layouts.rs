//! Layout tables.
//!
//! Every table lists the characters produced by the same 47 physical keys, in
//! the same order (ANSI rows, top to bottom, left to right), once without Shift
//! and once with it. Converting a character means finding its key in one table
//! and reading that key in another. Adding a language is adding a table.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Uk,
}

/// Marks a key that produces no character in a table. Never appears in text.
pub const NO_CHAR: char = '\u{FFFF}';

#[derive(Debug)]
pub struct Layout {
    pub id: &'static str,
    pub lang: Lang,
    pub base: &'static str,
    pub shift: &'static str,
}

/// US QWERTY (macOS "U.S." / "ABC", Windows "US").
pub static EN_US: Layout = Layout {
    id: "en-us",
    lang: Lang::En,
    base: "`1234567890-=qwertyuiop[]\\asdfghjkl;'zxcvbnm,./",
    shift: "~!@#$%^&*()_+QWERTYUIOP{}|ASDFGHJKL:\"ZXCVBNM<>?",
};

/// macOS "Ukrainian - PC". Dumped from the system with UCKeyTranslate.
pub static UK_MAC_PC: Layout = Layout {
    id: "uk-mac-pc",
    lang: Lang::Uk,
    base: "ґ1234567890-=йцукенгшщзхїʼфівапролджєячсмитьбю.",
    shift: "Ґ!\"№;%:?*()_+ЙЦУКЕНГШЩЗХЇ₴ФІВАПРОЛДЖЄЯЧСМИТЬБЮ,",
};

/// macOS "Ukrainian". Dumped from the system with UCKeyTranslate.
pub static UK_MAC: Layout = Layout {
    id: "uk-mac",
    lang: Lang::Uk,
    base: "'1234567890-=йцукенгшщзхїґфивапролджєячсмітьбю/",
    shift: "~!\"№%:,.;()_+ЙЦУКЕНГШЩЗХЇҐФИВАПРОЛДЖЄЯЧСМІТЬБЮ?",
};

/// Windows "Ukrainian (Enhanced)" (KLID 00020422).
/// NOT YET VERIFIED against a real Windows machine.
pub static UK_WIN_ENHANCED: Layout = Layout {
    id: "uk-win-enhanced",
    lang: Lang::Uk,
    base: "'1234567890-=йцукенгшщзхїґфівапролджєячсмитьбю.",
    shift: "₴!\"№;%:?*()_+ЙЦУКЕНГШЩЗХЇҐФІВАПРОЛДЖЄЯЧСМИТЬБЮ,",
};

/// Windows legacy "Ukrainian" (KLID 00000422).
/// NOT YET VERIFIED against a real Windows machine.
pub static UK_WIN: Layout = Layout {
    id: "uk-win",
    lang: Lang::Uk,
    base: "'1234567890-=йцукенгшщзхї\\фівапролджєячсмитьбю.",
    shift: "₴!\"№;%:?*()_+ЙЦУКЕНГШЩЗХЇ/ФІВАПРОЛДЖЄЯЧСМИТЬБЮ,",
};

pub static ALL: [&Layout; 5] = [&EN_US, &UK_MAC_PC, &UK_MAC, &UK_WIN_ENHANCED, &UK_WIN];
