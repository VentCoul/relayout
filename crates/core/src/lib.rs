//! Fixes text typed in the wrong keyboard layout.
//!
//! Pure logic: no I/O, no dependencies, nothing is stored or logged.

pub mod layouts;

pub use layouts::{Lang, Layout};

#[derive(Debug, PartialEq, Eq)]
pub struct Conversion {
    pub text: String,
    /// The layout the text is in now, i.e. the one to switch the system to.
    pub target: Lang,
}

fn is_latin(c: char) -> bool {
    c.is_ascii_alphabetic()
}

fn is_cyrillic(c: char) -> bool {
    matches!(c, '\u{0400}'..='\u{04FF}')
}

fn is_letter_of(lang: Lang, c: char) -> bool {
    match lang {
        Lang::En => is_latin(c),
        Lang::Uk => is_cyrillic(c),
    }
}

/// Which layout the text was (wrongly) typed in: the script most of its
/// letters belong to. `hint` (the active system layout) breaks ties.
fn detect_source(text: &str, hint: Option<Lang>) -> Option<Lang> {
    let latin = text.chars().filter(|&c| is_latin(c)).count();
    let cyrillic = text.chars().filter(|&c| is_cyrillic(c)).count();
    match latin.cmp(&cyrillic) {
        std::cmp::Ordering::Greater => Some(Lang::En),
        std::cmp::Ordering::Less => Some(Lang::Uk),
        std::cmp::Ordering::Equal => hint,
    }
}

/// The key a typographic quote was typed with. Editors with "smart quotes"
/// (on by default on macOS) replace `'` and `"` while the user types, so the
/// mistyped text contains `’` or `“` instead of the character on the key.
fn plain_quote(c: char) -> Option<char> {
    match c {
        '‘' | '’' | '‚' => Some('\''),
        '“' | '”' | '„' => Some('"'),
        _ => None,
    }
}

fn map_char(c: char, from: &Layout, to: &Layout) -> char {
    let lookup = |c: char| {
        for (src, dst) in [(from.base, to.base), (from.shift, to.shift)] {
            if let Some(pos) = src.chars().position(|s| s == c) {
                return match dst.chars().nth(pos) {
                    Some(mapped) if mapped != layouts::NO_CHAR => Some(mapped),
                    _ => None,
                };
            }
        }
        None
    };
    lookup(c).or_else(|| plain_quote(c).and_then(lookup)).unwrap_or(c)
}

/// Converts `text` between the `en` and `uk` layouts.
///
/// The direction is detected from the text. Words that are already entirely
/// in the target script are left alone, so a selection that mixes correct and
/// mistyped words comes out right. Returns `None` when there is nothing to do.
pub fn convert(text: &str, en: &Layout, uk: &Layout, hint: Option<Lang>) -> Option<Conversion> {
    let source = detect_source(text, hint)?;
    let (from, to) = match source {
        Lang::En => (en, uk),
        Lang::Uk => (uk, en),
    };

    let mut out = String::with_capacity(text.len() * 2);
    let mut token = String::new();
    let flush = |token: &mut String, out: &mut String| {
        let has_source = token.chars().any(|c| is_letter_of(from.lang, c));
        let has_target = token.chars().any(|c| is_letter_of(to.lang, c));
        if has_target && !has_source {
            out.push_str(token);
        } else {
            out.extend(token.chars().map(|c| map_char(c, from, to)));
        }
        token.clear();
    };
    for c in text.chars() {
        if c.is_whitespace() {
            flush(&mut token, &mut out);
            out.push(c);
        } else {
            token.push(c);
        }
    }
    flush(&mut token, &mut out);

    if out == text {
        return None;
    }
    Some(Conversion { text: out, target: to.lang })
}

#[cfg(test)]
mod tests {
    use super::layouts::*;
    use super::*;

    fn pc(text: &str) -> Conversion {
        convert(text, &EN_US, &UK_MAC_PC, None).expect("conversion")
    }

    #[test]
    fn tables_cover_the_same_keys() {
        for l in ALL {
            assert_eq!(l.base.chars().count(), 47, "{} base", l.id);
            assert_eq!(l.shift.chars().count(), 47, "{} shift", l.id);
        }
    }

    #[test]
    fn latin_to_ukrainian() {
        let c = pc("ghbdsn");
        assert_eq!(c.text, "привіт");
        assert_eq!(c.target, Lang::Uk);
    }

    #[test]
    fn ukrainian_to_latin() {
        let c = pc("руддщ цщкдв");
        assert_eq!(c.text, "hello world");
        assert_eq!(c.target, Lang::En);
    }

    #[test]
    fn case_and_punctuation_follow_the_keys() {
        assert_eq!(pc("Ghbdsn? zr cghfdb&").text, "Привіт, як справи?");
        assert_eq!(pc("Руддщ? ащщ").text, "Hello& foo");
    }

    #[test]
    fn ukrainian_specific_letters() {
        assert_eq!(pc("]; 'lyscnm `fyjr").text, "їж єдність ґанок");
        assert_eq!(pc("g\\znm").text, "пʼять");
    }

    #[test]
    fn correct_words_in_a_mixed_selection_are_kept() {
        assert_eq!(pc("Привіт, zr cghfdb&").text, "Привіт, як справи?");
        assert_eq!(pc("Hello, цщкдв ащщ").text, "Hello, world foo");
    }

    #[test]
    fn whitespace_is_preserved() {
        assert_eq!(pc("ghbdsn\n\tcdsn  ").text, "привіт\n\tсвіт  ");
    }

    #[test]
    fn round_trip() {
        let original = "Зʼїж ще цих мʼяких булок, та випий чаю. Ґанок №5?";
        let wrong = pc(original);
        assert_eq!(wrong.target, Lang::En);
        assert_eq!(pc(&wrong.text).text, original);
    }

    #[test]
    fn mac_native_ukrainian_layout_differs() {
        let c = convert("ghbdsn", &EN_US, &UK_MAC, None).unwrap();
        assert_eq!(c.text, "прівит");
        let c = convert("ghsdbn", &EN_US, &UK_MAC, None).unwrap();
        assert_eq!(c.text, "привіт");
    }

    #[test]
    fn nothing_to_do() {
        assert_eq!(convert("", &EN_US, &UK_MAC_PC, None), None);
        assert_eq!(convert("12345", &EN_US, &UK_MAC_PC, None), None);
        assert_eq!(convert("   ", &EN_US, &UK_MAC_PC, Some(Lang::En)), None);
    }

    #[test]
    fn smart_quotes_are_read_as_the_key_that_typed_them() {
        assert_eq!(pc("Ghjcnj ’lyscnm").text, "Просто єдність");
        assert_eq!(pc("‘lyscnm").text, "єдність");
        assert_eq!(pc("“lyscnm").text, "Єдність");
        assert_eq!(pc("ckjdj ”ghbdsn”").text, "слово ЄпривітЄ");
    }

    #[test]
    fn keys_without_a_character_are_left_alone() {
        static PARTIAL: Layout = Layout {
            id: "partial",
            lang: Lang::Uk,
            base: "\u{FFFF}1234567890-=йцукенгшщзхїʼфівапролджєячсмитьбю.",
            shift: "\u{FFFF}!\"№;%:?*()_+ЙЦУКЕНГШЩЗХЇ₴ФІВАПРОЛДЖЄЯЧСМИТЬБЮ,",
        };
        let c = convert("`ghbdsn", &EN_US, &PARTIAL, None).unwrap();
        assert_eq!(c.text, "`привіт");
    }

    #[test]
    fn hint_decides_when_there_are_no_letters() {
        let c = convert("?", &EN_US, &UK_MAC_PC, Some(Lang::En)).unwrap();
        assert_eq!(c.text, ",");
        assert_eq!(c.target, Lang::Uk);
    }
}
