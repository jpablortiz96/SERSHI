//! The application domain model.
//!
//! An application is something SERSHI *discovered* on the computer and can
//! refer to by a stable id. Callers (the user, and later a language model)
//! only ever name an application; how it is started is decided here, from
//! trusted discovery data, never from caller input.

use std::path::PathBuf;

use serde::Serialize;

/// Where an application was discovered. Also the tie-break priority when two
/// sources describe the same application (earlier variants win).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum AppSource {
    /// A Windows component SERSHI knows how to start (Calculator, Settings…).
    BuiltIn,
    /// A packaged (Microsoft Store / MSIX) application, started by its
    /// Application User Model ID.
    PackagedApp,
    /// A Start Menu shortcut.
    StartMenu,
    /// An `App Paths` registry registration.
    AppPaths,
}

/// How an application is started. Internal to the core and the platform
/// adapter: it is never serialized to the UI and never accepted from input.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LaunchTarget {
    /// A Win32 executable, started directly without a shell. `arguments` and
    /// `working_dir` come only from the shortcut that registered it.
    Executable {
        path: PathBuf,
        arguments: Option<String>,
        working_dir: Option<PathBuf>,
    },
    /// A packaged application activated by its Application User Model ID.
    PackagedApp { aumid: String },
    /// A fixed URI defined by SERSHI itself (e.g. `ms-settings:`).
    ShellUri { uri: &'static str },
    /// A Start Menu shortcut that does not resolve to an executable (e.g. an
    /// MSI "advertised" shortcut); opened through the Windows shell.
    Shortcut { path: PathBuf },
}

impl LaunchTarget {
    /// Identity used for de-duplication. Windows paths are case-insensitive
    /// and sources disagree on case, so compare lowercased. Arguments are
    /// part of the identity: "Anaconda PowerShell Prompt" runs the same
    /// `powershell.exe` as "Windows PowerShell" but is a different
    /// application, and must not hide it (observed in Gate 3C).
    pub fn identity(&self) -> String {
        match self {
            Self::Executable {
                path, arguments, ..
            } => match arguments.as_deref().map(str::trim) {
                Some(args) if !args.is_empty() => {
                    format!("exe:{} {args}", path.display()).to_lowercase()
                }
                _ => format!("exe:{}", path.display()).to_lowercase(),
            },
            Self::PackagedApp { aumid } => format!("aumid:{aumid}").to_lowercase(),
            Self::ShellUri { uri } => format!("uri:{uri}"),
            Self::Shortcut { path } => format!("lnk:{}", path.display()).to_lowercase(),
        }
    }
}

/// How SERSHI may close an application, if at all.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CloseSupport {
    /// Ask top-level windows of processes running this image to close.
    ExecutablePath(PathBuf),
    /// Ask top-level windows of processes of this packaged app to close.
    PackagedApp(String),
    /// Any of these (e.g. Notepad runs as a classic exe on Windows 10 and as a
    /// packaged app on Windows 11).
    Any(Vec<CloseSupport>),
    /// Closing is not supported (e.g. File Explorer, which also hosts the
    /// Windows shell).
    Unsupported,
}

/// A discovered application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationDescriptor {
    /// Stable, lowercase identifier, e.g. `spotify` or `windows.calculator`.
    pub id: String,
    /// Name as Windows presents it. Trusted: comes from discovery, not input.
    pub display_name: String,
    /// Additional names the application answers to (normalized on use).
    pub aliases: Vec<String>,
    pub source: AppSource,
    pub target: LaunchTarget,
    pub close: CloseSupport,
}

impl ApplicationDescriptor {
    pub fn summary(&self) -> ApplicationSummary {
        ApplicationSummary {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            source: self.source,
        }
    }
}

/// What may leave the core about an application: no paths, no commands.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ApplicationSummary {
    pub id: String,
    pub display_name: String,
    pub source: AppSource,
}

/// Turns a display name into a stable id: lowercase ASCII words joined by `-`.
pub fn slug(display_name: &str) -> String {
    let normalized = super::normalize::normalize(display_name);
    let slug: String = normalized
        .split(' ')
        .filter(|w| !w.is_empty())
        .map(|w| {
            w.chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '+')
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "app".to_owned()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_stable_ascii_ids() {
        assert_eq!(slug("Google Chrome"), "google-chrome");
        assert_eq!(slug("Visual Studio Code"), "visual-studio-code");
        assert_eq!(slug("Notepad++"), "notepad++");
        assert_eq!(slug("Configurações"), "configuracoes");
        assert_eq!(slug("???"), "app");
    }
}
