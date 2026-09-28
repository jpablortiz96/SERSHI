//! Conservative text normalization for application names.
//!
//! Case, accents, punctuation and whitespace are folded so that "Visual
//! Studio Code", "visual studio code" and "Visual-Studio Code!" compare equal.
//! Nothing more aggressive (stemming, edit distance) is done: a wrong match
//! would launch the wrong program.

/// Folds a name for comparison: lowercase, common Latin accents removed,
/// punctuation turned into spaces (except `+` and `#`, which appear in real
/// names like "Notepad++"), a trailing `.exe` dropped, whitespace collapsed.
pub fn normalize(input: &str) -> String {
    let lowered = input.trim().to_lowercase();
    let without_exe = lowered.strip_suffix(".exe").unwrap_or(&lowered);
    let mut out = String::with_capacity(without_exe.len());
    for c in without_exe.chars() {
        let c = fold_accent(c);
        if c.is_alphanumeric() || c == '+' || c == '#' {
            out.push(c);
        } else if !out.ends_with(' ') {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whole words of a normalized name.
pub fn words(normalized: &str) -> impl Iterator<Item = &str> {
    normalized.split(' ').filter(|w| !w.is_empty())
}

fn fold_accent(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        _ => c,
    }
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
    fn does_not_over_normalize() {
        assert_ne!(
            normalize("Visual Studio 2022"),
            normalize("Visual Studio Code")
        );
        assert_eq!(normalize("¿?"), "");
    }
}
