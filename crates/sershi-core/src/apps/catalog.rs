//! The application catalog: discovered applications, de-duplicated, with
//! deterministic name resolution.
//!
//! Resolution tiers, first tier with any match wins:
//! 1. exact name or id
//! 2. alias (the application's own aliases, SERSHI's curated aliases, and a
//!    vendor-qualified name: "Microsoft Word" → "Word")
//! 3. word-boundary prefix ("visual studio" → "visual studio code")
//! 4. every query word appears as a whole word in the name
//!
//! One match launches; several matches are **ambiguous** and SERSHI asks
//! instead of guessing, listing the closest candidates first. There is no
//! edit-distance matching here: similarity and phonetics are scored,
//! thresholded signals in `crate::understanding`, which only ever select
//! one of these trusted entries.

use std::collections::HashSet;

use super::model::{ApplicationDescriptor, ApplicationSummary, slug};
use super::normalize::{normalize, words};

/// Most candidates reported for an ambiguous request.
pub const MAX_CANDIDATES: usize = 6;

/// Common short names for well-known applications. Values are normalized
/// display names as Windows usually presents them. Aliases are strings
/// only: they select a discovered application, never a path or command.
const CURATED_ALIASES: &[(&str, &[&str])] = &[
    ("chrome", &["google chrome"]),
    ("navegador chrome", &["google chrome"]),
    ("navegador de google", &["google chrome"]),
    ("navegador google", &["google chrome"]),
    ("ms word", &["word", "microsoft word"]),
    ("ms excel", &["excel", "microsoft excel"]),
    ("ms outlook", &["outlook", "microsoft outlook"]),
    ("ms powerpoint", &["powerpoint", "microsoft powerpoint"]),
    ("power point", &["powerpoint", "microsoft powerpoint"]),
    // A frequent speech-recognition spelling of "Bloc de notas".
    ("blog de notas", &["notepad", "bloc de notas"]),
    ("bloc de notas", &["notepad", "bloc de notas"]),
    ("powershell ise", &["windows powershell ise"]),
    ("vscode", &["visual studio code"]),
    ("vs code", &["visual studio code"]),
    ("code", &["visual studio code"]),
    ("edge", &["microsoft edge"]),
    ("firefox", &["mozilla firefox", "firefox"]),
    ("word", &["word", "microsoft word"]),
    ("excel", &["excel", "microsoft excel"]),
    ("powerpoint", &["powerpoint", "microsoft powerpoint"]),
    ("outlook", &["outlook", "microsoft outlook", "outlook new"]),
    ("teams", &["microsoft teams", "teams"]),
    ("terminal", &["terminal", "windows terminal"]),
];

/// Vendor names that may prefix an application's own name ("Microsoft
/// Word" for "Word", "Google Chrome" for "Chrome").
const VENDORS: &[&str] = &["microsoft", "ms", "google", "mozilla", "adobe", "windows"];

/// A trusted application as understanding may see it: identity and names,
/// never how it is started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogNames {
    pub application: ApplicationSummary,
    /// Normalized display name first, then aliases and curated aliases.
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum MatchKind {
    Exact,
    Alias,
    Prefix,
    Words,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Found {
        application: ApplicationDescriptor,
        matched: MatchKind,
    },
    Ambiguous(Vec<ApplicationSummary>),
    NotFound,
}

type Matcher<'a> = &'a dyn Fn(&Entry) -> bool;

#[derive(Debug, Clone)]
struct Entry {
    app: ApplicationDescriptor,
    name: String,
    /// The id, normalized like a query ("windows.calculator" → "windows calculator").
    id_key: String,
    aliases: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ApplicationCatalog {
    entries: Vec<Entry>,
}

impl ApplicationCatalog {
    /// Builds a catalog from raw discovery results. Duplicates (same
    /// normalized name, same launch target, or a name that is another
    /// application's alias) keep the higher-priority source.
    pub fn build(mut discovered: Vec<ApplicationDescriptor>) -> Self {
        discovered.sort_by(|a, b| {
            a.source
                .cmp(&b.source)
                .then_with(|| a.display_name.cmp(&b.display_name))
        });

        let mut entries: Vec<Entry> = Vec::new();
        let mut names: HashSet<String> = HashSet::new();
        let mut targets: HashSet<String> = HashSet::new();
        let mut ids: HashSet<String> = HashSet::new();

        for mut app in discovered {
            let name = normalize(&app.display_name);
            if name.is_empty() || names.contains(&name) || targets.contains(&app.target.identity())
            {
                continue;
            }
            let aliases: Vec<String> = app
                .aliases
                .iter()
                .map(|a| normalize(a))
                .filter(|a| !a.is_empty())
                .collect();
            // Discovered entries can't shadow a higher-priority app's alias
            // (e.g. a Start Menu "Calculadora" duplicating the built-in).
            if entries.iter().any(|e| e.aliases.contains(&name)) {
                continue;
            }
            let base = if app.id.is_empty() {
                slug(&app.display_name)
            } else {
                app.id.clone()
            };
            let mut id = base.clone();
            let mut n = 2;
            while ids.contains(&id) {
                id = format!("{base}-{n}");
                n += 1;
            }
            let id_key = normalize(&id);
            app.id = id.clone();
            ids.insert(id);
            names.insert(name.clone());
            targets.insert(app.target.identity());
            entries.push(Entry {
                app,
                name,
                id_key,
                aliases,
            });
        }
        Self { entries }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Summaries in display order.
    pub fn list(&self) -> Vec<ApplicationSummary> {
        let mut list: Vec<_> = self.entries.iter().map(|e| e.app.summary()).collect();
        list.sort_by_key(|a| a.display_name.to_lowercase());
        list
    }

    pub fn resolve(&self, query: &str) -> Resolution {
        let q = normalize(query);
        match self.resolve_normalized(&q) {
            // Speech recognition splits compound names ("Power Point",
            // "Power Shell"): try once without the spaces.
            Resolution::NotFound if q.contains(' ') => self.resolve_normalized(&q.replace(' ', "")),
            other => other,
        }
    }

    fn resolve_normalized(&self, q: &str) -> Resolution {
        let q = q.to_owned();
        if q.is_empty() {
            return Resolution::NotFound;
        }
        let curated: Vec<&str> = CURATED_ALIASES
            .iter()
            .filter(|(alias, _)| *alias == q)
            .flat_map(|(_, names)| names.iter().copied())
            .collect();
        let query_words: Vec<&str> = words(&q).collect();
        let prefix = format!("{q} ");
        // "microsoft word" → "word"; only for a remaining name of 3+ chars.
        let unqualified: Option<&str> = VENDORS.iter().find_map(|v| {
            q.strip_prefix(v)
                .and_then(|rest| rest.strip_prefix(' '))
                .filter(|rest| rest.len() >= 3)
        });

        let tiers: [(MatchKind, Matcher<'_>); 4] = [
            (MatchKind::Exact, &|e| e.name == q || e.id_key == q),
            (MatchKind::Alias, &|e| {
                e.aliases.contains(&q)
                    || curated.contains(&e.name.as_str())
                    || unqualified.is_some_and(|u| e.name == u)
            }),
            (MatchKind::Prefix, &|e| {
                q.len() >= 3 && e.name.starts_with(&prefix)
            }),
            (MatchKind::Words, &|e| {
                query_words.iter().any(|w| w.chars().count() >= 3)
                    && query_words.iter().all(|w| words(&e.name).any(|n| n == *w))
            }),
        ];

        for (kind, matches) in tiers {
            let found: Vec<&Entry> = self.entries.iter().filter(|e| matches(e)).collect();
            match found.as_slice() {
                [] => continue,
                [one] => {
                    return Resolution::Found {
                        application: one.app.clone(),
                        matched: kind,
                    };
                }
                many => {
                    let mut ranked: Vec<&Entry> = many.to_vec();
                    ranked.sort_by_key(|e| relevance(&query_words, &e.name));
                    return Resolution::Ambiguous(
                        ranked
                            .iter()
                            .take(MAX_CANDIDATES)
                            .map(|e| e.app.summary())
                            .collect(),
                    );
                }
            }
        }
        Resolution::NotFound
    }

    /// Every application's identity and names, for scored matching in
    /// `crate::understanding`. No launch targets leave the catalog.
    pub fn names(&self) -> Vec<CatalogNames> {
        self.entries
            .iter()
            .map(|e| {
                let mut names = vec![e.name.clone()];
                for alias in &e.aliases {
                    if !names.contains(alias) {
                        names.push(alias.clone());
                    }
                }
                for (alias, targets) in CURATED_ALIASES {
                    if targets.contains(&e.name.as_str()) && !names.iter().any(|n| n == alias) {
                        names.push((*alias).to_owned());
                    }
                }
                CatalogNames {
                    application: e.app.summary(),
                    names,
                }
            })
            .collect()
    }
}

/// Sort key for ambiguous matches: closest first. Fewer words beyond the
/// query, and a platform-variant suffix such as "(x86)" last, so "Windows
/// PowerShell" is offered before "Windows PowerShell ISE (x86)".
fn relevance(query_words: &[&str], name: &str) -> (bool, usize, usize) {
    let name_words: Vec<&str> = words(name).collect();
    let extra = name_words
        .iter()
        .filter(|w| !query_words.contains(w))
        .count();
    let variant = name_words
        .iter()
        .any(|w| matches!(*w, "x86" | "x64" | "32" | "arm64"));
    (variant, extra, name.len())
}

#[cfg(test)]
pub(crate) mod fixtures {
    use std::path::PathBuf;

    use super::super::model::{AppSource, CloseSupport, LaunchTarget};
    use super::*;

    pub fn exe(name: &str, source: AppSource, file: &str) -> ApplicationDescriptor {
        let path = PathBuf::from(format!("C:\\\\Apps\\\\{file}"));
        ApplicationDescriptor {
            id: String::new(),
            display_name: name.to_owned(),
            aliases: vec![],
            source,
            target: LaunchTarget::Executable {
                path: path.clone(),
                arguments: None,
                working_dir: None,
            },
            close: CloseSupport::ExecutablePath(path),
        }
    }

    pub fn packaged(name: &str, aumid: &str) -> ApplicationDescriptor {
        ApplicationDescriptor {
            id: String::new(),
            display_name: name.to_owned(),
            aliases: vec![],
            source: AppSource::PackagedApp,
            target: LaunchTarget::PackagedApp {
                aumid: aumid.to_owned(),
            },
            close: CloseSupport::PackagedApp(aumid.to_owned()),
        }
    }

    pub fn builtin(id: &str, name: &str, aliases: &[&str]) -> ApplicationDescriptor {
        ApplicationDescriptor {
            id: id.to_owned(),
            display_name: name.to_owned(),
            aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
            source: AppSource::BuiltIn,
            // Distinct targets so built-ins are not de-duplicated together.
            target: LaunchTarget::Shortcut {
                path: PathBuf::from(id),
            },
            close: CloseSupport::Unsupported,
        }
    }

    /// A small machine: Spotify from the Store, Chrome and VS Code from the
    /// Start Menu, Visual Studio 2022, and two built-ins.
    pub fn catalog() -> ApplicationCatalog {
        ApplicationCatalog::build(vec![
            packaged("Spotify", "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
            exe("Google Chrome", AppSource::StartMenu, "chrome.exe"),
            exe("Visual Studio Code", AppSource::StartMenu, "Code.exe"),
            exe("Visual Studio 2022", AppSource::StartMenu, "devenv.exe"),
            exe("chrome", AppSource::AppPaths, "chrome.exe"),
            builtin(
                "windows.calculator",
                "Calculator",
                &["calculator", "calc", "calculadora"],
            ),
            builtin(
                "windows.notepad",
                "Notepad",
                &["notepad", "bloc de notas", "bloco de notas"],
            ),
            // A Start Menu duplicate of a built-in, in another language.
            exe("Calculadora", AppSource::StartMenu, "calc2.exe"),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::AppSource;
    use super::fixtures::{catalog, exe};
    use super::*;

    fn found(query: &str) -> Option<(String, MatchKind)> {
        match catalog().resolve(query) {
            Resolution::Found {
                application,
                matched,
            } => Some((application.id, matched)),
            _ => None,
        }
    }

    #[test]
    fn de_duplicates_by_target_name_and_alias() {
        let names: Vec<String> = catalog()
            .list()
            .into_iter()
            .map(|a| a.display_name)
            .collect();
        assert_eq!(
            names,
            [
                "Calculator",
                "Google Chrome",
                "Notepad",
                "Spotify",
                "Visual Studio 2022",
                "Visual Studio Code"
            ]
        );
    }

    #[test]
    fn exact_names_and_ids_resolve() {
        assert_eq!(found("Spotify"), Some(("spotify".into(), MatchKind::Exact)));
        assert_eq!(
            found("  spotify! "),
            Some(("spotify".into(), MatchKind::Exact))
        );
        assert_eq!(
            found("windows.calculator"),
            Some(("windows.calculator".into(), MatchKind::Exact))
        );
    }

    #[test]
    fn aliases_resolve_in_every_language() {
        for q in ["calc", "Calculadora", "calculator"] {
            assert_eq!(
                found(q).map(|f| f.0),
                Some("windows.calculator".into()),
                "{q}"
            );
        }
        assert_eq!(
            found("bloc de notas").map(|f| f.0),
            Some("windows.notepad".into())
        );
        assert_eq!(
            found("chrome"),
            Some(("google-chrome".into(), MatchKind::Alias))
        );
        for q in ["vscode", "VS Code", "code"] {
            assert_eq!(
                found(q).map(|f| f.0),
                Some("visual-studio-code".into()),
                "{q}"
            );
        }
    }

    #[test]
    fn ambiguous_requests_are_never_guessed() {
        match catalog().resolve("Visual Studio") {
            Resolution::Ambiguous(candidates) => {
                let names: Vec<_> = candidates.into_iter().map(|c| c.display_name).collect();
                assert!(names.contains(&"Visual Studio Code".to_owned()));
                assert!(names.contains(&"Visual Studio 2022".to_owned()));
            }
            other => panic!("expected ambiguity, got {other:?}"),
        }
    }

    #[test]
    fn missing_or_nonsense_requests_are_not_found() {
        for q in [
            "Photoshop",
            "",
            "!!!",
            "vis",
            "C:\\\\Windows\\\\cmd.exe",
            "s",
        ] {
            assert_eq!(catalog().resolve(q), Resolution::NotFound, "{q:?}");
        }
    }

    #[test]
    fn vendor_qualified_names_and_split_compounds_resolve() {
        let catalog = ApplicationCatalog::build(vec![
            exe("Word", AppSource::StartMenu, "WINWORD.EXE"),
            exe("PowerPoint", AppSource::StartMenu, "POWERPNT.EXE"),
            exe("Google Chrome", AppSource::StartMenu, "chrome.exe"),
        ]);
        let id = |q: &str| match catalog.resolve(q) {
            Resolution::Found { application, .. } => Some(application.id),
            _ => None,
        };
        assert_eq!(id("Microsoft Word").as_deref(), Some("word"));
        assert_eq!(id("MS Word").as_deref(), Some("word"));
        assert_eq!(id("Power Point").as_deref(), Some("powerpoint"));
        assert_eq!(id("navegador de Google").as_deref(), Some("google-chrome"));
        // A vendor alone, or a vendor before an unknown name, is not a match.
        assert_eq!(id("Microsoft"), None);
        assert_eq!(id("Microsoft Photoshop"), None);
    }

    #[test]
    fn shortcuts_with_different_arguments_are_different_applications() {
        use super::super::model::LaunchTarget;
        let with_args = |name: &str, args: Option<&str>| ApplicationDescriptor {
            target: LaunchTarget::Executable {
                path: "C:\\Windows\\powershell.exe".into(),
                arguments: args.map(str::to_owned),
                working_dir: None,
            },
            ..exe(name, AppSource::StartMenu, "powershell.exe")
        };
        let catalog = ApplicationCatalog::build(vec![
            with_args("Anaconda PowerShell Prompt", Some("-NoExit -Command conda")),
            with_args("Windows PowerShell", None),
            with_args("Windows PowerShell copy", None),
        ]);
        let names: Vec<String> = catalog.list().into_iter().map(|a| a.display_name).collect();
        // The plain shortcut is no longer hidden by the one with arguments;
        // an exact duplicate target still is.
        assert_eq!(names, ["Anaconda PowerShell Prompt", "Windows PowerShell"]);
    }

    #[test]
    fn ambiguous_candidates_come_closest_first() {
        let catalog = ApplicationCatalog::build(vec![
            exe(
                "Windows PowerShell ISE (x86)",
                AppSource::StartMenu,
                "a.exe",
            ),
            exe(
                "Developer PowerShell for VS 2022",
                AppSource::StartMenu,
                "b.exe",
            ),
            exe("Windows PowerShell ISE", AppSource::StartMenu, "c.exe"),
            exe("Windows PowerShell", AppSource::StartMenu, "d.exe"),
        ]);
        let Resolution::Ambiguous(candidates) = catalog.resolve("PowerShell") else {
            panic!("expected ambiguity");
        };
        let names: Vec<String> = candidates.into_iter().map(|c| c.display_name).collect();
        assert_eq!(
            names,
            [
                "Windows PowerShell",
                "Windows PowerShell ISE",
                "Developer PowerShell for VS 2022",
                "Windows PowerShell ISE (x86)"
            ]
        );
    }

    #[test]
    fn names_expose_aliases_but_never_targets() {
        let names = catalog().names();
        let notepad = names
            .iter()
            .find(|n| n.application.id == "windows.notepad")
            .unwrap();
        assert!(notepad.names.contains(&"bloc de notas".to_owned()));
        assert!(notepad.names.contains(&"blog de notas".to_owned()));
        let debug = format!("{names:?}");
        assert!(!debug.contains("C:") && !debug.contains(".exe"));
    }

    #[test]
    fn prefix_and_word_matches_are_whole_word_only() {
        assert_eq!(
            found("google"),
            Some(("google-chrome".into(), MatchKind::Prefix))
        );
        // "studio code" is not a prefix but both words appear.
        assert_eq!(
            found("studio code"),
            Some(("visual-studio-code".into(), MatchKind::Words))
        );
        // Partial words never match.
        assert_eq!(catalog().resolve("spot"), Resolution::NotFound);
    }
}
