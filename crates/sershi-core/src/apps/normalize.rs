//! Conservative text normalization for application names and utterances.
//!
//! Case, accents, punctuation and whitespace are folded so that "Visual
//! Studio Code", "visual studio code" and "Visual-Studio Code!" compare equal.
//! Nothing lexical is changed here (no stemming, no spelling correction):
//! similarity and phonetics are separate, scored signals in
//! `crate::understanding`, never a silent rewrite.

/// Folds a name for comparison: lowercase, Latin diacritics removed,
/// punctuation turned into spaces (except `+` and `#`, which appear in real
/// names like "Notepad++"), a trailing `.exe` dropped, whitespace collapsed.
pub fn normalize(input: &str) -> String {
    let lowered = input.trim().to_lowercase();
    let without_exe = lowered.strip_suffix(".exe").unwrap_or(&lowered);
    let mut out = String::with_capacity(without_exe.len());
    for c in without_exe.chars() {
        // Apostrophes join a word ("don't" → "dont").
        if matches!(c, '\'' | '’' | '`' | '´') {
            continue;
        }
        match fold_char(c) {
            Some(folded) => out.push_str(folded),
            None if c.is_alphanumeric() || c == '+' || c == '#' => out.push(c),
            None if !out.ends_with(' ') => out.push(' '),
            None => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whole words of a normalized name.
pub fn words(normalized: &str) -> impl Iterator<Item = &str> {
    normalized.split(' ').filter(|w| !w.is_empty())
}

/// Removes Latin diacritics from a lowercase character. Covers Spanish,
/// Portuguese, English and the Nordic letters speech recognition sometimes
/// produces for Spanish audio ("Afrið", "kláði").
fn fold_char(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'ď' | 'đ' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' | 'ŋ' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'œ' => "oe",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' => "s",
        'ß' => "ss",
        'ţ' | 'ť' | 'ŧ' => "t",
        'þ' => "th",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_case_accents_punctuation_and_spacing() {
        assert_eq!(normalize("  Visual   Studio  Code "), "visual studio code");
        assert_eq!(normalize("Configurações"), "configuracoes");
        assert_eq!(
            normalize("Explorador de Archivos!"),
            "explorador de archivos"
        );
        assert_eq!(normalize("Spotify®"), "spotify");
        assert_eq!(normalize("chrome.exe"), "chrome");
        assert_eq!(normalize("Notepad++"), "notepad++");
        assert_eq!(normalize("Visual-Studio: Code"), "visual studio code");
    }

    #[test]
    fn folds_letters_speech_recognition_produces() {
        assert_eq!(normalize("Afrið Google Chrome"), "afrid google chrome");
        assert_eq!(normalize("Af rýr kláði"), "af ryr kladi");
        assert_eq!(normalize("Ábreme"), "abreme");
        assert_eq!(normalize("¿Qué?"), "que");
        assert_eq!(normalize("Don't"), "dont");
        assert_eq!(normalize("Straße"), "strasse");
        assert_eq!(normalize("日本語"), "日本語");
    }

    #[test]
    fn does_not_over_normalize() {
        assert_ne!(
            normalize("Visual Studio 2022"),
            normalize("Visual Studio Code")
        );
        assert_eq!(normalize("¿?"), "");
    }
}
