//! Lexical and phonetic similarity between what was said and a trusted
//! application name. Both inputs are already normalized
//! (`crate::apps::normalize`).
//!
//! These are *signals*, never decisions: a score only ranks catalog
//! entries, and `super::policy` decides — with a minimum score and a margin
//! over the runner-up — whether to act, ask, or do nothing.
//!
//! The phonetic key is designed for speech-recognition errors on
//! application names in Spanish, English and Portuguese (and names that mix
//! them): it merges letters that recognition confuses across those
//! languages (b/v/p/f, d/t, c/k/q, s/z/c before e/i, e/i, o/u, w/u, y/i),
//! drops a silent h, and collapses repeats. "World" and "Word", or "Apreer"
//! and "Abrir", get close keys. It is deliberately coarse, which is why it
//! can only ever raise a score part of the way (see [`similarity`]).

/// Similarity of a spoken phrase to a name, in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Similarity {
    /// Edit-distance similarity of the spellings (with and without spaces).
    pub lexical: f32,
    /// Edit-distance similarity of the phonetic keys.
    pub phonetic: f32,
    /// The score used for ranking: lexical, raised by at most half the way
    /// toward the phonetic score.
    pub combined: f32,
}

pub fn similarity(query: &str, name: &str) -> Similarity {
    if query == name {
        return Similarity {
            lexical: 1.0,
            phonetic: 1.0,
            combined: 1.0,
        };
    }
    let q_compact: String = query.chars().filter(|c| *c != ' ').collect();
    let n_compact: String = name.chars().filter(|c| *c != ' ').collect();
    let lexical = ratio(query, name).max(ratio(&q_compact, &n_compact));
    let phonetic = ratio(&phonetic_key(&q_compact), &phonetic_key(&n_compact));
    let combined = if phonetic > lexical {
        (lexical + phonetic) / 2.0
    } else {
        lexical
    };
    Similarity {
        lexical,
        phonetic,
        combined,
    }
}

/// `1 - distance / longer length` (optimal string alignment distance).
pub fn ratio(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let r = 1.0 - distance(&a, &b) as f32 / longest as f32;
    r
}

/// Optimal string alignment distance (Levenshtein plus adjacent
/// transpositions).
pub fn distance(a: &[char], b: &[char]) -> usize {
    let (n, m) = (a.len(), b.len());
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev2 = vec![0usize; m + 1];
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];
    for i in 1..=n {
        cur[0] = i;
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(prev2[j - 2] + 1);
            }
            cur[j] = best;
        }
        std::mem::swap(&mut prev2, &mut prev);
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[m]
}

/// A coarse, multilingual sound key of a normalized word or phrase.
pub fn phonetic_key(text: &str) -> String {
    let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let soft = matches!(next, Some('e' | 'i' | 'y'));
        let (code, skip) = match c {
            'p' if next == Some('h') => ('b', 2),
            's' | 'c' if next == Some('h') => ('x', 2),
            'l' if next == Some('l') => ('i', 2),
            'q' if next == Some('u') => ('k', 2),
            'c' if next == Some('k') => ('k', 2),
            'g' if next == Some('u') && matches!(chars.get(i + 2), Some('e' | 'i')) => ('k', 2),
            'c' if soft => ('s', 1),
            'g' if soft => ('j', 1),
            'a' => ('a', 1),
            'e' | 'i' | 'y' => ('i', 1),
            'o' | 'u' | 'w' => ('u', 1),
            'b' | 'v' | 'p' | 'f' => ('b', 1),
            'd' | 't' => ('t', 1),
            'c' | 'k' | 'q' | 'g' => ('k', 1),
            's' | 'z' => ('s', 1),
            'x' => ('s', 1),
            'm' | 'n' => ('n', 1),
            'h' => ('\0', 1),
            other => (other, 1),
        };
        if code != '\0' && out.last() != Some(&code) {
            out.push(code);
        }
        i += skip;
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_counts_edits_and_transpositions() {
        let d = |a: &str, b: &str| {
            distance(
                &a.chars().collect::<Vec<_>>(),
                &b.chars().collect::<Vec<_>>(),
            )
        };
        assert_eq!(d("word", "word"), 0);
        assert_eq!(d("world", "word"), 1);
        assert_eq!(d("chrome", "chorme"), 1);
        assert_eq!(d("", "abc"), 3);
        assert!((ratio("world", "word") - 0.8).abs() < 1e-6);
    }

    #[test]
    fn phonetic_keys_merge_what_recognition_confuses() {
        assert_eq!(phonetic_key("apreer"), phonetic_key("abrir"));
        assert_eq!(phonetic_key("vloc"), phonetic_key("bloc"));
        assert_eq!(phonetic_key("blog"), phonetic_key("bloc"));
        assert_eq!(phonetic_key("exel"), phonetic_key("excel"));
        assert_eq!(phonetic_key("powershell"), phonetic_key("power shell"));
        assert_ne!(phonetic_key("chrome"), phonetic_key("word"));
    }

    #[test]
    fn world_is_close_to_word_but_not_identical() {
        let s = similarity("world", "word");
        assert!(s.combined >= 0.75 && s.combined < 0.9, "{s:?}");
        assert_eq!(similarity("word", "word").combined, 1.0);
        let far = similarity("world", "google chrome");
        assert!(far.combined < 0.4, "{far:?}");
    }

    #[test]
    fn spacing_differences_do_not_hurt() {
        assert_eq!(similarity("power point", "powerpoint").lexical, 1.0);
    }

    #[test]
    fn phonetics_raise_a_score_at_most_half_way() {
        let s = similarity("apreer", "abrir");
        assert_eq!(s.phonetic, 1.0);
        assert!(s.combined <= (s.lexical + 1.0) / 2.0 + 1e-6);
        assert!(s.combined > s.lexical);
    }
}
