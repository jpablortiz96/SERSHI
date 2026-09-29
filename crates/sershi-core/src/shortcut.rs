//! The global shortcut that summons SERSHI: validation and the rules for
//! replacing it. Invocation only — a shortcut can bring SERSHI forward, it
//! never grants a capability, changes a permission or approves anything.
//!
//! Validation is deliberately conservative: at least two modifiers, one of
//! them Ctrl or Alt; one letter, digit, Space or F1–F12; no Windows-key
//! combinations (reserved by Windows). Single-modifier combinations (Ctrl+C,
//! Alt+F4, Alt+Tab…) are rejected, so everyday and system shortcuts cannot
//! be taken over.
//!
//! Replacement is safe: the new shortcut is registered first and the old one
//! released only if that succeeded, so a failed change never leaves SERSHI
//! without its working shortcut. There is no automatic fallback to a
//! different combination — the user chooses.

use serde::Serialize;
use thiserror::Error;

use crate::ipc::{FeatureStatus, ShortcutStatus};

/// SERSHI's default: Ctrl+Alt+Space (see docs/WINDOWS_PLATFORM.md for why
/// not Alt+Space or Ctrl+Shift+Space).
pub const DEFAULT_SHORTCUT: &str = "Ctrl+Alt+Space";

/// Longest accelerator text accepted over IPC.
const MAX_LEN: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Letter(char),
    Digit(char),
    Space,
    Function(u8),
}

/// A validated, canonical accelerator (e.g. `Ctrl+Alt+J`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Accelerator {
    ctrl: bool,
    alt: bool,
    shift: bool,
    key: Key,
}

/// Why an accelerator was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ShortcutProblem {
    /// Not a recognisable combination.
    #[error("not a valid shortcut")]
    Malformed,
    /// Needs at least two modifiers, including Ctrl or Alt.
    #[error("needs Ctrl or Alt plus another modifier")]
    NeedsModifiers,
    /// Only letters, digits, Space and F1–F12 can be the main key.
    #[error("unsupported key")]
    UnsupportedKey,
    /// Windows-key combinations belong to Windows.
    #[error("Windows-key combinations are reserved")]
    Reserved,
}

impl Accelerator {
    /// Parses `Ctrl+Alt+J`-style text (case-insensitive, any modifier order).
    pub fn parse(text: &str) -> Result<Self, ShortcutProblem> {
        if text.is_empty() || text.len() > MAX_LEN {
            return Err(ShortcutProblem::Malformed);
        }
        let (mut ctrl, mut alt, mut shift) = (false, false, false);
        let mut key = None;
        for token in text.split('+').map(str::trim) {
            let upper = token.to_ascii_uppercase();
            let flag = match upper.as_str() {
                "CTRL" | "CONTROL" => Some(&mut ctrl),
                "ALT" | "OPTION" => Some(&mut alt),
                "SHIFT" => Some(&mut shift),
                "WIN" | "SUPER" | "META" | "CMD" | "COMMAND" => {
                    return Err(ShortcutProblem::Reserved);
                }
                _ => None,
            };
            if let Some(flag) = flag {
                if *flag {
                    return Err(ShortcutProblem::Malformed);
                }
                *flag = true;
                continue;
            }
            if key.is_some() {
                return Err(ShortcutProblem::Malformed);
            }
            key = Some(parse_key(&upper)?);
        }
        let key = key.ok_or(ShortcutProblem::Malformed)?;
        let modifiers = [ctrl, alt, shift].iter().filter(|m| **m).count();
        if modifiers < 2 || !(ctrl || alt) {
            return Err(ShortcutProblem::NeedsModifiers);
        }
        Ok(Self {
            ctrl,
            alt,
            shift,
            key,
        })
    }

    /// Canonical text, in the fixed order Ctrl, Alt, Shift, key.
    pub fn canonical(&self) -> String {
        let mut parts: Vec<String> = Vec::with_capacity(4);
        if self.ctrl {
            parts.push("Ctrl".into());
        }
        if self.alt {
            parts.push("Alt".into());
        }
        if self.shift {
            parts.push("Shift".into());
        }
        parts.push(match self.key {
            Key::Letter(c) | Key::Digit(c) => c.to_string(),
            Key::Space => "Space".into(),
            Key::Function(n) => format!("F{n}"),
        });
        parts.join("+")
    }

    /// Ctrl+Alt+letter/digit is AltGr on many keyboard layouts (e.g. `@`,
    /// `€`): registering it globally can stop that character being typed.
    /// Allowed, but the UI warns.
    pub fn may_conflict_with_altgr(&self) -> bool {
        self.ctrl && self.alt && !self.shift && matches!(self.key, Key::Letter(_) | Key::Digit(_))
    }
}

fn parse_key(upper: &str) -> Result<Key, ShortcutProblem> {
    let mut chars = upper.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_uppercase() => return Ok(Key::Letter(c)),
        (Some(c), None) if c.is_ascii_digit() => return Ok(Key::Digit(c)),
        _ => {}
    }
    if upper == "SPACE" {
        return Ok(Key::Space);
    }
    if let Some(n) = upper.strip_prefix('F').and_then(|n| n.parse::<u8>().ok())
        && (1..=12).contains(&n)
    {
        return Ok(Key::Function(n));
    }
    Err(ShortcutProblem::UnsupportedKey)
}

/// The operating system refused a registration (usually: another
/// application already owns the combination).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("the shortcut could not be registered")]
pub struct Unavailable;

/// Registers accelerators with the operating system (the desktop shell's
/// global-shortcut plugin; fakes in tests).
pub trait ShortcutRegistrar {
    fn register(&mut self, accelerator: &Accelerator) -> Result<(), Unavailable>;
    fn unregister(&mut self, accelerator: &Accelerator);
}

/// What happened to a requested shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum ShortcutResult {
    /// Registered; the previous shortcut (if any) was released.
    Registered,
    /// Already the active shortcut.
    Unchanged,
    /// Another application owns it; the previous shortcut still works.
    Unavailable,
    /// Rejected by validation; nothing changed.
    Invalid,
}

/// Reply to a change request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ShortcutChange {
    pub result: ShortcutResult,
    pub problem: Option<ShortcutProblem>,
    /// Canonical form of the request, when it was valid.
    pub requested: Option<String>,
    /// Ctrl+Alt+letter/digit may be AltGr on some keyboard layouts.
    pub altgr_warning: bool,
    /// The shortcut state after the request.
    pub shortcut: ShortcutStatus,
}

/// The active global shortcut and the last one asked for.
#[derive(Debug, Default, Clone)]
pub struct ShortcutBinding {
    active: Option<Accelerator>,
    /// The last requested shortcut that could not be registered (shown as
    /// "Unavailable" when nothing is active).
    unavailable: Option<Accelerator>,
}

impl ShortcutBinding {
    /// Tries to make `requested` the active shortcut. Never releases the
    /// working shortcut unless the new one registered.
    pub fn apply(
        &mut self,
        requested: &str,
        registrar: &mut impl ShortcutRegistrar,
    ) -> ShortcutChange {
        let accelerator = match Accelerator::parse(requested) {
            Ok(a) => a,
            Err(problem) => {
                return ShortcutChange {
                    result: ShortcutResult::Invalid,
                    problem: Some(problem),
                    requested: None,
                    altgr_warning: false,
                    shortcut: self.status(),
                };
            }
        };
        let result = if self.active.as_ref() == Some(&accelerator) {
            ShortcutResult::Unchanged
        } else {
            match registrar.register(&accelerator) {
                Ok(()) => {
                    if let Some(previous) = self.active.take() {
                        registrar.unregister(&previous);
                    }
                    self.active = Some(accelerator.clone());
                    self.unavailable = None;
                    ShortcutResult::Registered
                }
                Err(Unavailable) => {
                    self.unavailable = Some(accelerator.clone());
                    ShortcutResult::Unavailable
                }
            }
        };
        ShortcutChange {
            result,
            problem: None,
            requested: Some(accelerator.canonical()),
            altgr_warning: accelerator.may_conflict_with_altgr(),
            shortcut: self.status(),
        }
    }

    /// Whether any shortcut has been requested yet.
    pub fn is_configured(&self) -> bool {
        self.active.is_some() || self.unavailable.is_some()
    }

    pub fn active(&self) -> Option<&Accelerator> {
        self.active.as_ref()
    }

    /// Active shortcut, or the one that could not be registered.
    pub fn status(&self) -> ShortcutStatus {
        match (&self.active, &self.unavailable) {
            (Some(active), _) => ShortcutStatus {
                accelerator: active.canonical(),
                status: FeatureStatus::Active,
            },
            (None, Some(failed)) => ShortcutStatus {
                accelerator: failed.canonical(),
                status: FeatureStatus::Unavailable,
            },
            (None, None) => ShortcutStatus {
                accelerator: DEFAULT_SHORTCUT.to_owned(),
                status: FeatureStatus::Unavailable,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// A fake OS: some combinations are owned by "another application".
    #[derive(Default)]
    struct FakeOs {
        taken: HashSet<String>,
        registered: HashSet<String>,
    }

    impl ShortcutRegistrar for FakeOs {
        fn register(&mut self, a: &Accelerator) -> Result<(), Unavailable> {
            let key = a.canonical();
            if self.taken.contains(&key) {
                return Err(Unavailable);
            }
            self.registered.insert(key);
            Ok(())
        }
        fn unregister(&mut self, a: &Accelerator) {
            self.registered.remove(&a.canonical());
        }
    }

    fn parse(text: &str) -> Result<String, ShortcutProblem> {
        Accelerator::parse(text).map(|a| a.canonical())
    }

    #[test]
    fn accepts_modifier_combinations_and_canonicalises_them() {
        assert_eq!(parse("Ctrl+Alt+J"), Ok("Ctrl+Alt+J".into()));
        assert_eq!(parse("shift+ctrl+k"), Ok("Ctrl+Shift+K".into()));
        assert_eq!(parse("Alt + Shift + Space"), Ok("Alt+Shift+Space".into()));
        assert_eq!(parse("Control+Alt+F5"), Ok("Ctrl+Alt+F5".into()));
        assert_eq!(parse("Ctrl+Shift+7"), Ok("Ctrl+Shift+7".into()));
        assert_eq!(parse(DEFAULT_SHORTCUT), Ok(DEFAULT_SHORTCUT.into()));
    }

    #[test]
    fn rejects_simple_or_unsafe_bindings() {
        for simple in [
            "A", "Enter", "Space", "Escape", "F1", "Ctrl+C", "Alt+F4", "Alt+Tab",
        ] {
            assert!(parse(simple).is_err(), "{simple}");
        }
        assert_eq!(parse("Shift+J"), Err(ShortcutProblem::NeedsModifiers));
        assert_eq!(parse("Ctrl+K"), Err(ShortcutProblem::NeedsModifiers));
        // Shift needs Ctrl or Alt alongside it.
        assert_eq!(parse("Shift+Shift+K"), Err(ShortcutProblem::Malformed));
        assert_eq!(
            parse("Ctrl+Alt+Enter"),
            Err(ShortcutProblem::UnsupportedKey)
        );
        assert_eq!(
            parse("Ctrl+Alt+Delete"),
            Err(ShortcutProblem::UnsupportedKey)
        );
        assert_eq!(parse("Ctrl+Alt+F13"), Err(ShortcutProblem::UnsupportedKey));
        assert_eq!(parse("Win+Alt+K"), Err(ShortcutProblem::Reserved));
        assert_eq!(parse("Ctrl+Alt+J+K"), Err(ShortcutProblem::Malformed));
        assert_eq!(parse("Ctrl+Alt"), Err(ShortcutProblem::Malformed));
        assert_eq!(parse(""), Err(ShortcutProblem::Malformed));
        assert_eq!(parse(&"Ctrl+".repeat(20)), Err(ShortcutProblem::Malformed));
    }

    #[test]
    fn warns_about_altgr_combinations() {
        let warn = |t: &str| Accelerator::parse(t).map(|a| a.may_conflict_with_altgr());
        assert_eq!(warn("Ctrl+Alt+Q"), Ok(true));
        assert_eq!(warn("Ctrl+Alt+2"), Ok(true));
        assert_eq!(warn("Ctrl+Alt+Space"), Ok(false));
        assert_eq!(warn("Ctrl+Alt+Shift+Q"), Ok(false));
        assert_eq!(warn("Ctrl+Shift+Q"), Ok(false));
    }

    #[test]
    fn a_conflicting_replacement_keeps_the_working_shortcut() {
        let mut os = FakeOs::default();
        os.taken.insert("Ctrl+Shift+K".into());
        let mut binding = ShortcutBinding::default();
        assert_eq!(
            binding.apply("Ctrl+Alt+J", &mut os).result,
            ShortcutResult::Registered
        );
        let change = binding.apply("Ctrl+Shift+K", &mut os);
        assert_eq!(change.result, ShortcutResult::Unavailable);
        assert_eq!(change.shortcut.accelerator, "Ctrl+Alt+J");
        assert_eq!(change.shortcut.status, FeatureStatus::Active);
        assert!(os.registered.contains("Ctrl+Alt+J"));
    }

    #[test]
    fn a_successful_replacement_releases_the_previous_shortcut() {
        let mut os = FakeOs::default();
        let mut binding = ShortcutBinding::default();
        binding.apply("Ctrl+Alt+J", &mut os);
        let change = binding.apply("alt+shift+space", &mut os);
        assert_eq!(change.result, ShortcutResult::Registered);
        assert_eq!(change.requested.as_deref(), Some("Alt+Shift+Space"));
        assert_eq!(os.registered, HashSet::from(["Alt+Shift+Space".to_owned()]));
        assert_eq!(
            binding.apply("Alt+Shift+Space", &mut os).result,
            ShortcutResult::Unchanged
        );
    }

    #[test]
    fn an_unavailable_shortcut_at_start_is_reported_without_a_fallback() {
        let mut os = FakeOs::default();
        os.taken.insert(DEFAULT_SHORTCUT.into());
        let mut binding = ShortcutBinding::default();
        let change = binding.apply(DEFAULT_SHORTCUT, &mut os);
        assert_eq!(change.result, ShortcutResult::Unavailable);
        assert_eq!(change.shortcut.status, FeatureStatus::Unavailable);
        assert_eq!(change.shortcut.accelerator, DEFAULT_SHORTCUT);
        // Nothing else was registered in its place.
        assert!(os.registered.is_empty());
        assert!(binding.active().is_none());
        assert!(binding.is_configured());
    }

    #[test]
    fn invalid_requests_change_nothing() {
        let mut os = FakeOs::default();
        let mut binding = ShortcutBinding::default();
        binding.apply("Ctrl+Alt+J", &mut os);
        let change = binding.apply("Ctrl+C", &mut os);
        assert_eq!(change.result, ShortcutResult::Invalid);
        assert_eq!(change.problem, Some(ShortcutProblem::NeedsModifiers));
        assert_eq!(change.shortcut.accelerator, "Ctrl+Alt+J");
        assert_eq!(os.registered, HashSet::from(["Ctrl+Alt+J".to_owned()]));
    }

    #[test]
    fn a_never_registered_binding_reports_unavailable_safely() {
        let binding = ShortcutBinding::default();
        assert!(!binding.is_configured());
        assert_eq!(binding.status().status, FeatureStatus::Unavailable);
    }
}
