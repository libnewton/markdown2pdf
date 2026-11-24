//! Emoji sequences -> bundled Twemoji file names.
//!
//! Twemoji names a file after the sequence's codepoints, dropping U+FE0F
//! everywhere except inside most ZWJ sequences. No rule covers every file, so
//! a sequence is matched against the actual list of files, longest first. A
//! sequence with no file stays text: the hosts only ever load files that exist.

use std::collections::HashSet;
use std::sync::OnceLock;
use unic_emoji_char::{is_emoji, is_emoji_modifier, is_emoji_presentation};

const VS16: char = '\u{FE0F}';
const ZWJ: char = '\u{200D}';

/// Every file in `package/twemoji`, without the `.svg`.
fn files() -> &'static HashSet<&'static str> {
    static FILES: OnceLock<HashSet<&'static str>> = OnceLock::new();
    FILES.get_or_init(|| include_str!("twemoji.txt").lines().collect())
}

/// The Twemoji file name for exactly this sequence, if there is one.
fn file_for(seq: &[char]) -> Option<String> {
    let hex = |keep_vs16: bool| {
        seq.iter()
            .filter(|&&c| keep_vs16 || c != VS16)
            .map(|c| format!("{:x}", *c as u32))
            .collect::<Vec<_>>()
            .join("-")
    };
    [hex(true), hex(false)]
        .into_iter()
        .find(|name| files().contains(name.as_str()))
}

fn is_regional(c: char) -> bool {
    ('\u{1F1E6}'..='\u{1F1FF}').contains(&c)
}

fn is_tag(c: char) -> bool {
    ('\u{E0020}'..='\u{E007F}').contains(&c)
}

/// Match an emoji sequence at `chars[i]`: a flag, or an emoji with its
/// presentation selector, skin tone, tag characters and ZWJ-joined parts.
/// Returns (end index, Twemoji file name).
///
/// A character whose default presentation is text (©, ™, ↔, ✔ …) counts only
/// when U+FE0F or a skin tone asks for the emoji form. `is_emoji` also covers ASCII `0-9 # *`
/// (keycap bases), which stay text.
pub(crate) fn match_emoji(chars: &[char], i: usize) -> Option<(usize, String)> {
    let c = chars[i];
    let at = |j: usize| chars.get(j).copied();
    if is_regional(c) {
        let end = i + 2;
        return at(i + 1)
            .filter(|&n| is_regional(n))
            .and_then(|_| file_for(&chars[i..end]))
            .map(|name| (end, name));
    }
    if c.is_ascii() || !is_emoji(c) {
        return None;
    }
    let selected = at(i + 1) == Some(VS16);
    // A skin tone asks for the emoji form as plainly as the selector does.
    let toned = at(i + 1).is_some_and(is_emoji_modifier);
    if !selected && !toned && !is_emoji_presentation(c) {
        return None;
    }
    let base_end = i + 1 + usize::from(selected);
    let mut j = base_end;
    if at(j).is_some_and(is_emoji_modifier) {
        j += 1;
    }
    while at(j).is_some_and(is_tag) {
        j += 1;
    }
    let mut ends = vec![j];
    while at(j) == Some(ZWJ) && at(j + 1).is_some_and(|n| !n.is_ascii()) {
        j += 2;
        if at(j) == Some(VS16) {
            j += 1;
        }
        if at(j).is_some_and(is_emoji_modifier) {
            j += 1;
        }
        ends.push(j);
    }
    ends.into_iter()
        .rev()
        .chain([base_end])
        .find_map(|end| file_for(&chars[i..end]).map(|name| (end, name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> Option<String> {
        let chars: Vec<char> = s.chars().collect();
        match_emoji(&chars, 0)
            .filter(|(end, _)| *end == chars.len())
            .map(|(_, name)| name)
    }

    #[test]
    fn the_file_list_matches_the_package() {
        let mut on_disk: Vec<String> = std::fs::read_dir("../package/twemoji")
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter_map(|n| n.strip_suffix(".svg").map(str::to_string))
            .collect();
        on_disk.sort();
        let mut listed: Vec<&str> = files().iter().copied().collect();
        listed.sort();
        assert_eq!(listed, on_disk, "regenerate engine/src/twemoji.txt");
    }

    #[test]
    fn zwj_sequences_keep_the_selector_their_file_has() {
        for (emoji, file) in [
            ("🤷‍♀️", "1f937-200d-2640-fe0f"),
            ("🏴‍☠️", "1f3f4-200d-2620-fe0f"),
            ("🏳️‍🌈", "1f3f3-fe0f-200d-1f308"),
            ("👩🏽‍💻", "1f469-1f3fd-200d-1f4bb"),
            ("👁️‍🗨️", "1f441-200d-1f5e8"),
            ("👨‍👩‍👧", "1f468-200d-1f469-200d-1f467"),
            ("🇩🇪", "1f1e9-1f1ea"),
            ("🛡️", "1f6e1"),
        ] {
            assert_eq!(name(emoji).as_deref(), Some(file), "{emoji}");
        }
    }

    #[test]
    fn a_skin_tone_joins_its_base() {
        assert_eq!(name("👍🏽").as_deref(), Some("1f44d-1f3fd"));
    }

    #[test]
    fn text_presentation_characters_need_the_selector() {
        for c in ["©", "®", "™", "↔", "ℹ", "‼", "✔", "▶"] {
            assert_eq!(name(c), None, "{c}");
            assert!(name(&format!("{c}\u{FE0F}")).is_some(), "{c} + FE0F");
        }
    }

    #[test]
    fn a_sequence_without_a_file_stays_text() {
        assert_eq!(name("🇦🇦"), None);
    }

    /// Every emoji the shortcode table knows, in every skin tone, resolves to
    /// a file when Twemoji has one for it. Only bases `unic-emoji-char` knows
    /// as emoji (its data is Unicode 10) are detected at all.
    #[test]
    fn every_known_emoji_with_a_file_resolves_to_it() {
        let mut missed = Vec::new();
        for e in emojis::iter() {
            let variants = std::iter::once(e.as_str().to_string()).chain(
                e.skin_tones()
                    .into_iter()
                    .flatten()
                    .map(|t| t.as_str().to_string()),
            );
            for v in variants {
                let chars: Vec<char> = v.chars().collect();
                let has_file = file_for(&chars).is_some();
                if has_file && name(&v).is_none() && !chars[0].is_ascii() && is_emoji(chars[0]) {
                    missed.push(v);
                }
            }
        }
        assert!(missed.is_empty(), "{missed:?}");
    }
}
