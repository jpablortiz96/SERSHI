//! How risky it is to close an application (Gate 4.1.1).
//!
//! Trusted metadata, never model output: closing is a sensitive action
//! because an application may hold unsaved work. A very small curated set
//! of SERSHI's own built-in components is known to hold none and closes
//! without asking. Everything else — including applications that merely
//! *sound* harmless — stays conservative.
//!
//! Policy precedence for a user's close request (see [`crate::executor`]):
//!
//! ```text
//! high-risk / prohibited / denied (never overridable)
//!   > per-application setting the user stored (Settings › Security)
//!   > this close-risk metadata (SafeToClose closes without asking)
//!   > the category setting ("Close applications": Ask / Always allow)
//! ```
//!
//! A close proposed by a language model (`CallOrigin::Agent`) always asks.

use serde::Serialize;

use super::model::{AppSource, ApplicationSummary};

/// Close risk of an application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum CloseRisk {
    /// Holds no user work worth a confirmation (curated, built-in only).
    SafeToClose,
    /// May hold unsaved work (documents, mail, browsing, code).
    MayLoseUserWork,
    /// Not known: treated like [`CloseRisk::MayLoseUserWork`].
    Unknown,
}

/// Built-in components (stable ids SERSHI assigns itself; a discovered
/// application's id is a slug and can never contain a dot) that are safe to
/// close. Deliberately tiny: Notepad, Paint, Office, browsers, IDEs and
/// terminals hold user state and are not here.
const SAFE_TO_CLOSE: &[&str] = &["windows.calculator"];

/// Applications SERSHI knows hold user work.
const MAY_LOSE_WORK: &[&str] = &["windows.notepad", "windows.paint"];

pub fn close_risk(app: &ApplicationSummary) -> CloseRisk {
    if app.source != AppSource::BuiltIn {
        return CloseRisk::Unknown;
    }
    if SAFE_TO_CLOSE.contains(&app.id.as_str()) {
        CloseRisk::SafeToClose
    } else if MAY_LOSE_WORK.contains(&app.id.as_str()) {
        CloseRisk::MayLoseUserWork
    } else {
        CloseRisk::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, name: &str, source: AppSource) -> ApplicationSummary {
        ApplicationSummary {
            id: id.to_owned(),
            display_name: name.to_owned(),
            source,
        }
    }

    #[test]
    fn only_the_built_in_calculator_is_safe_to_close() {
        assert_eq!(
            close_risk(&app("windows.calculator", "Calculator", AppSource::BuiltIn)),
            CloseRisk::SafeToClose
        );
        // A discovered application that merely shares the name is not.
        for source in [AppSource::StartMenu, AppSource::PackagedApp] {
            assert_eq!(
                close_risk(&app("calculator", "Calculator", source)),
                CloseRisk::Unknown
            );
            assert_eq!(
                close_risk(&app("windows.calculator", "Calculator", source)),
                CloseRisk::Unknown
            );
        }
    }

    #[test]
    fn harmless_sounding_applications_stay_conservative() {
        for (id, name, source) in [
            ("windows.notepad", "Notepad", AppSource::BuiltIn),
            ("windows.paint", "Paint", AppSource::BuiltIn),
            ("windows.explorer", "File Explorer", AppSource::BuiltIn),
            ("excel", "Excel", AppSource::StartMenu),
            ("outlook", "Outlook", AppSource::StartMenu),
            ("google-chrome", "Google Chrome", AppSource::StartMenu),
            (
                "visual-studio-code",
                "Visual Studio Code",
                AppSource::StartMenu,
            ),
            ("windows-terminal", "Terminal", AppSource::PackagedApp),
            ("clock", "Clock", AppSource::PackagedApp),
        ] {
            assert_ne!(
                close_risk(&app(id, name, source)),
                CloseRisk::SafeToClose,
                "{name}"
            );
        }
    }
}
