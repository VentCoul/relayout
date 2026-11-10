//! Fixes text typed in the wrong keyboard layout.
//!
//! Pure logic: no I/O, no dependencies, nothing is stored or logged.

mod bigrams;
pub mod layouts;

pub use layouts::{Lang, Layout};

#[derive(Debug, PartialEq, Eq)]
pub struct Conversion {
    pub text: String,
    /// The layout the text is in now, i.e. the one to switch the system to.
    /// `None` when only the letter case was fixed.
    pub target: Option<Lang>,
    /// The text was typed with Caps Lock on, so Caps Lock should be turned off.
    pub caps_fixed: bool,
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

/// Text typed with Caps Lock on, recognised by its inverted case: at least one
/// word like `пРИВІТ` (first letter small, the rest capital) and no small
/// letter anywhere except as the first letter of a word. Returns the text with the
/// case of every letter swapped. Text in all capitals is left alone: it may
/// well be intended.
fn fix_caps(text: &str) -> Option<String> {
    let mut inverted_word = false;
    // Words are split on spaces only: in the wrong layout a letter may arrive
    // as punctuation (`k.LBYF` is `лЮДИНА`), which must not split the word.
    for word in text.split(char::is_whitespace) {
        let mut cased = word.chars().filter(|c| c.is_lowercase() || c.is_uppercase());
        let Some(first) = cased.next() else { continue };
        let mut rest = 0;
        for c in cased {
            if c.is_lowercase() {
                return None;
            }
            rest += 1;
        }
        inverted_word |= first.is_lowercase() && rest > 0;
    }
    if !inverted_word {
        return None;
    }
    let swap = |c: char| -> String {
        if c.is_lowercase() {
            c.to_uppercase().collect()
        } else {
            c.to_lowercase().collect()
        }
    };
    Some(text.chars().map(swap).collect())
}

type LogP = fn(usize, usize) -> f32;

/// How much more natural the layout-converted text must look before Caps Lock
/// text is also converted. Close calls (mostly very short words) keep the
/// layout: correctly typed text is never damaged, and a missed layout fix
/// costs one more press.
const LAYOUT_MARGIN: f32 = 0.3;

/// Score of a punctuation mark inside a word, on the scale of `naturalness`:
/// as unlikely as a letter pair seen about once in 50 000.
const PUNCTUATION_PENALTY: f32 = -8.0;

fn en_log_p(a: usize, b: usize) -> f32 {
    bigrams::EN_LOG_P[a][b]
}

fn uk_log_p(a: usize, b: usize) -> f32 {
    bigrams::UK_LOG_P[a][b]
}

/// How natural the text looks for the language of its script, judged by how
/// often each pair of neighbouring letters occurs in that language. About 0
/// for real text, clearly negative for text typed in the wrong layout
/// (`ghbdsn`, `руддщ`), which is full of letter pairs rare in its script.
fn naturalness(text: &str) -> f32 {
    let models: [(fn(char) -> bool, &str, f32, LogP); 2] = [
        (is_latin, bigrams::EN_SYMBOLS, bigrams::EN_AVERAGE, en_log_p),
        (is_cyrillic, bigrams::UK_SYMBOLS, bigrams::UK_AVERAGE, uk_log_p),
    ];
    let (mut sum, mut count) = (0.0, 0u32);
    // Punctuation right before a letter is rare in real text but common in the
    // wrong layout, where letters land on punctuation keys (`};f` is `їжа`).
    // Apostrophes between Cyrillic letters (`пам'ять`) are normal.
    let chars: Vec<char> = text.chars().collect();
    for (i, window) in chars.windows(2).enumerate() {
        let (c, next) = (window[0], window[1]);
        let apostrophe = matches!(c, '\'' | 'ʼ' | '’');
        let between_cyrillic = i > 0 && is_cyrillic(chars[i - 1]) && is_cyrillic(next);
        let letter = |c: char| is_latin(c) || is_cyrillic(c);
        if !letter(c) && !c.is_whitespace() && letter(next) && !(apostrophe && between_cyrillic) {
            sum += PUNCTUATION_PENALTY;
            count += 1;
        }
    }
    for (in_script, symbols, average, log_p) in models {
        // Symbol 0 is the word boundary.
        let mut previous = 0;
        let letters = text.chars().flat_map(char::to_lowercase).chain([' ']);
        for c in letters {
            let symbol = if in_script(c) { symbols.chars().position(|s| s == c) } else { None };
            match symbol {
                Some(current) => {
                    sum += log_p(previous, current) - average;
                    count += 1;
                    previous = current;
                }
                None if previous != 0 => {
                    sum += log_p(previous, 0) - average;
                    count += 1;
                    previous = 0;
                }
                None => {}
            }
        }
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f32
    }
}

/// Fixes text typed with Caps Lock on, in the wrong layout, or both.
///
/// Caps Lock is recognised by its inverted case (`пРИВІТ`). When it is, the
/// case is fixed, and the layout too if that gives more natural-looking text
/// (judged by letter-pair statistics; a wrong guess is undone by pressing
/// again):
/// `gHBDSN` becomes `Привіт` in one go, while `пРИВІТ` only becomes `Привіт`.
///
/// The layout direction is detected from the text. Words that are already
/// entirely in the target script are left alone, so a selection that mixes
/// correct and mistyped words comes out right. Returns `None` when there is
/// nothing to do.
pub fn convert(text: &str, en: &Layout, uk: &Layout, hint: Option<Lang>) -> Option<Conversion> {
    let Some(case_fixed) = fix_caps(text) else {
        let (text, target) = convert_layout(text, en, uk, hint)?;
        return Some(Conversion { text, target: Some(target), caps_fixed: false });
    };
    match convert_layout(&case_fixed, en, uk, hint) {
        Some((both_fixed, target)) if naturalness(&both_fixed) > naturalness(&case_fixed) + LAYOUT_MARGIN => {
            Some(Conversion { text: both_fixed, target: Some(target), caps_fixed: true })
        }
        _ => Some(Conversion { text: case_fixed, target: None, caps_fixed: true }),
    }
}

/// Converts between the layouts. Returns the new text and its layout.
fn convert_layout(text: &str, en: &Layout, uk: &Layout, hint: Option<Lang>) -> Option<(String, Lang)> {
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
    Some((out, to.lang))
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
        assert_eq!(c.target, Some(Lang::Uk));
    }

    #[test]
    fn ukrainian_to_latin() {
        let c = pc("руддщ цщкдв");
        assert_eq!(c.text, "hello world");
        assert_eq!(c.target, Some(Lang::En));
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
        assert_eq!(wrong.target, Some(Lang::En));
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
    fn caps_lock_is_fixed_in_the_same_script() {
        let c = pc("пРИВІТ, ЯК СПРАВИ?");
        assert_eq!(c.text, "Привіт, як справи?");
        assert_eq!(c.target, None);
        assert!(c.caps_fixed);
        assert_eq!(pc("hELLO WORLD").text, "Hello world");
        assert_eq!(pc("пАМʼЯТЬ").text, "Памʼять");
        assert_eq!(pc("пАМ'ЯТЬ").text, "Пам'ять");
    }

    #[test]
    fn caps_lock_and_wrong_layout_are_fixed_together() {
        let c = pc("gHBDSN");
        assert_eq!(c.text, "Привіт");
        assert_eq!(c.target, Some(Lang::Uk));
        assert!(c.caps_fixed);
        assert_eq!(pc("gHBDSN? ZR CGHFDB&").text, "Привіт, як справи?");
        assert_eq!(pc("zR").text, "Як");
        assert_eq!(pc("рУДДЩ").text, "Hello");
        assert_eq!(pc("рУДДЩ ЦЩКДВ").text, "Hello world");
        // Letters on punctuation keys keep the case they were typed with.
        assert_eq!(pc("k.LBYF").text, "Людина");
        assert_eq!(pc("z,KERJ").text, "Яблуко");
        assert_eq!(pc("pFDNHF").text, "Завтра");
    }

    #[test]
    fn caps_lock_alone_keeps_the_layout() {
        for (typed, meant) in [("пРИВІТ", "Привіт"), ("яК", "Як"), ("тАК", "Так"), ("мАМА", "Мама"), ("hELLO", "Hello"), ("tHANKS", "Thanks"), ("yES", "Yes"), ("wHY", "Why")] {
            let c = pc(typed);
            assert_eq!(c.text, meant, "{typed}");
            assert_eq!(c.target, None, "{typed}");
        }
    }

    #[test]
    fn intended_capitals_are_not_caps_lock() {
        // All capitals, acronyms and normal text go to the layout fix.
        assert_eq!(pc("CGFCB<J").text, "СПАСИБО");
        assert_eq!(pc("ghbdsn NASA").text, "привіт ТФІФ");
        assert!(!pc("Ye yjdbq iOS").caps_fixed);
        assert!(!pc("Ghbdsn").caps_fixed);
        assert!(!pc("iPhone").caps_fixed);
        assert!(!pc("eBay").caps_fixed);
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
        assert_eq!(c.target, Some(Lang::Uk));
    }
}

