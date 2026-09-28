//! `system.open_application` and `system.close_application`.
//!
//! Both accept only an application *name*. The name is resolved against the
//! catalog of discovered applications; only a resolved, trusted launch target
//! ever reaches the platform adapter. Paths, commands and arguments are not
//! accepted from input at all.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::catalog::{MatchKind, Resolution};
use super::manager::ApplicationManager;
use super::model::{ApplicationDescriptor, ApplicationSummary, CloseSupport};
use crate::builtin::BuiltinError;
use crate::confirmation::{ConfirmationAction, ConfirmationSubject};
use crate::ids::{PermissionId, ToolId};
use crate::platform::Platform;
use crate::ports::{ApplicationError, RunningState};
use crate::tool::{
    Prepared, RiskLevel, Severity, Tool, ToolDefinition, ToolError, ToolOutput, ToolRegistry,
    parse_input,
};

pub const OPEN_APPLICATION: &str = "system.open_application";
pub const CLOSE_APPLICATION: &str = "system.close_application";
pub const APPS_LAUNCH: &str = "system.apps.launch";
pub const APPS_CLOSE: &str = "system.apps.close";

/// Longest application name accepted.
pub const MAX_APPLICATION_NAME: usize = 80;

/// Why a launch failed, as the UI may explain it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum LaunchFailure {
    TargetMissing,
    AccessDenied,
    ElevationRequired,
    Unsupported,
    TimedOut,
    Failed,
}

/// Structured result of an application tool (the tool output's `data`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum ApplicationResult {
    Opened {
        application: ApplicationSummary,
        matched: MatchKind,
    },
    NotFound {
        /// The name as requested (sanitised, length-limited). Shown in the
        /// conversation only, never in trusted confirmation UI.
        query: String,
    },
    Ambiguous {
        query: String,
        candidates: Vec<ApplicationSummary>,
    },
    LaunchFailed {
        application: ApplicationSummary,
        reason: LaunchFailure,
    },
    CloseRequested {
        application: ApplicationSummary,
        windows: u32,
    },
    NotRunning {
        application: ApplicationSummary,
    },
    CloseUnsupported {
        application: ApplicationSummary,
    },
    CatalogUnavailable,
}

impl ApplicationResult {
    fn summary(&self) -> String {
        match self {
            Self::Opened { application, .. } => format!("Opened {}.", application.display_name),
            Self::NotFound { query } => format!("I couldn't find {query} on this computer."),
            Self::Ambiguous { query, .. } => {
                format!("I found more than one application matching \"{query}\". Which one?")
            }
            Self::LaunchFailed { application, .. } => format!(
                "I found {}, but Windows couldn't open it.",
                application.display_name
            ),
            Self::CloseRequested { application, .. } => {
                format!("Asked {} to close.", application.display_name)
            }
            Self::NotRunning { application } => {
                format!("{} isn't running.", application.display_name)
            }
            Self::CloseUnsupported { application } => format!(
                "{} can't be closed safely by SERSHI yet.",
                application.display_name
            ),
            Self::CatalogUnavailable => {
                "I couldn't read the list of installed applications.".to_owned()
            }
        }
    }

    fn output(self) -> ToolOutput {
        let subject = match &self {
            Self::Opened { application, .. }
            | Self::LaunchFailed { application, .. }
            | Self::CloseRequested { application, .. }
            | Self::NotRunning { application }
            | Self::CloseUnsupported { application } => Some(application.display_name.clone()),
            Self::NotFound { .. } | Self::Ambiguous { .. } | Self::CatalogUnavailable => None,
        };
        let summary = self.summary();
        let output = ToolOutput::new(serde_json::to_value(&self).unwrap_or(Value::Null), summary);
        match subject {
            Some(s) => output.with_subject(s),
            None => output,
        }
    }

    fn declined(self, severity: Severity) -> ToolError {
        ToolError::Declined {
            output: self.output(),
            severity,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicationInput {
    application: String,
}

/// Parses and sanitises the requested name. Anything that looks like a path
/// or command is rejected outright rather than "cleaned".
fn requested_name(input: &Value) -> Result<String, ToolError> {
    let ApplicationInput { application } = parse_input(input)?;
    let name = application.trim();
    if name.is_empty() || name.chars().count() > MAX_APPLICATION_NAME {
        return Err(ToolError::InvalidInput(
            "application name length".to_owned(),
        ));
    }
    if name.chars().any(|c| {
        c.is_control() || matches!(c, '\\' | '/' | ':' | '<' | '>' | '|' | '"' | '*' | '?')
    }) {
        return Err(ToolError::InvalidInput(
            "application must be a name, not a path or command".to_owned(),
        ));
    }
    Ok(name.to_owned())
}

fn resolve(
    apps: &ApplicationManager,
    query: &str,
) -> Result<(ApplicationDescriptor, MatchKind), ToolError> {
    match apps.resolve(query) {
        Ok(Resolution::Found {
            application,
            matched,
        }) => Ok((application, matched)),
        Ok(Resolution::NotFound) => Err(ApplicationResult::NotFound {
            query: query.to_owned(),
        }
        .declined(Severity::Attention)),
        Ok(Resolution::Ambiguous(candidates)) => Err(ApplicationResult::Ambiguous {
            query: query.to_owned(),
            candidates,
        }
        .declined(Severity::Attention)),
        Err(_) => Err(ApplicationResult::CatalogUnavailable.declined(Severity::Failure)),
    }
}

fn subject(app: &ApplicationDescriptor) -> Option<ConfirmationSubject> {
    Some(ConfirmationSubject::Application {
        application: app.summary(),
    })
}

fn definition(
    id: &str,
    name: &str,
    description: &str,
    permission: &str,
    risk: RiskLevel,
) -> Result<ToolDefinition, BuiltinError> {
    Ok(ToolDefinition {
        id: ToolId::new(id)?,
        name: name.to_owned(),
        description: description.to_owned(),
        input_schema: json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["application"],
            "properties": {
                "application": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": MAX_APPLICATION_NAME,
                    "description": "The application's name as the user said it, e.g. \"Spotify\". Never a path, command or arguments."
                }
            }
        }),
        output_schema: json!({"type": "object", "required": ["kind"]}),
        permissions: vec![PermissionId::new(permission)?],
        risk,
        timeout_ms: 15_000,
        platforms: vec![Platform::Windows],
    })
}

pub fn register(
    registry: &mut ToolRegistry,
    apps: Arc<ApplicationManager>,
) -> Result<(), BuiltinError> {
    registry.register(Arc::new(OpenApplicationTool {
        definition: definition(
            OPEN_APPLICATION,
            "Open application",
            "Open an installed application by name.",
            APPS_LAUNCH,
            RiskLevel::Safe,
        )?,
        apps: apps.clone(),
    }))?;
    registry.register(Arc::new(CloseApplicationTool {
        definition: definition(
            CLOSE_APPLICATION,
            "Close application",
            "Ask a running application to close its windows. Unsaved work may be lost.",
            APPS_CLOSE,
            RiskLevel::Sensitive,
        )?,
        apps,
    }))?;
    Ok(())
}

#[derive(Debug)]
struct OpenApplicationTool {
    definition: ToolDefinition,
    apps: Arc<ApplicationManager>,
}

impl Tool for OpenApplicationTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    fn prepare(&self, input: &Value) -> Result<Prepared, ToolError> {
        let (app, _) = resolve(&self.apps, &requested_name(input)?)?;
        Ok(Prepared {
            action: ConfirmationAction::RunTool,
            subject: subject(&app),
        })
    }

    fn execute(&self, input: &Value) -> Result<ToolOutput, ToolError> {
        let (app, matched) = resolve(&self.apps, &requested_name(input)?)?;
        match self.apps.platform().launch(&app.target) {
            Ok(()) => Ok(ApplicationResult::Opened {
                application: app.summary(),
                matched,
            }
            .output()),
            Err(error) => {
                let (reason, severity) = launch_failure(&error);
                Err(ApplicationResult::LaunchFailed {
                    application: app.summary(),
                    reason,
                }
                .declined(severity))
            }
        }
    }
}

fn launch_failure(error: &ApplicationError) -> (LaunchFailure, Severity) {
    match error {
        ApplicationError::TargetMissing => (LaunchFailure::TargetMissing, Severity::Failure),
        ApplicationError::AccessDenied => (LaunchFailure::AccessDenied, Severity::Failure),
        ApplicationError::ElevationRequired => {
            (LaunchFailure::ElevationRequired, Severity::Attention)
        }
        ApplicationError::Unsupported | ApplicationError::PlatformUnavailable => {
            (LaunchFailure::Unsupported, Severity::Attention)
        }
        ApplicationError::TimedOut => (LaunchFailure::TimedOut, Severity::Failure),
        ApplicationError::Failed(_) => (LaunchFailure::Failed, Severity::Failure),
    }
}

#[derive(Debug)]
struct CloseApplicationTool {
    definition: ToolDefinition,
    apps: Arc<ApplicationManager>,
}

impl CloseApplicationTool {
    /// Resolves the application and checks it can actually be closed, so the
    /// user is only ever asked to confirm something SERSHI will do.
    fn closable(&self, input: &Value) -> Result<ApplicationDescriptor, ToolError> {
        let (app, _) = resolve(&self.apps, &requested_name(input)?)?;
        if app.close == CloseSupport::Unsupported {
            return Err(unsupported(&app));
        }
        match self.apps.platform().running_state(&app.close) {
            Ok(RunningState::Running { .. }) => Ok(app),
            Ok(RunningState::NotRunning) => Err(ApplicationResult::NotRunning {
                application: app.summary(),
            }
            .declined(Severity::Attention)),
            Ok(RunningState::NoClosableWindow) | Err(_) => Err(unsupported(&app)),
        }
    }
}

fn unsupported(app: &ApplicationDescriptor) -> ToolError {
    ApplicationResult::CloseUnsupported {
        application: app.summary(),
    }
    .declined(Severity::Attention)
}

impl Tool for CloseApplicationTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    fn prepare(&self, input: &Value) -> Result<Prepared, ToolError> {
        let app = self.closable(input)?;
        Ok(Prepared {
            action: ConfirmationAction::CloseApplication,
            subject: subject(&app),
        })
    }

    fn execute(&self, input: &Value) -> Result<ToolOutput, ToolError> {
        let app = self.closable(input)?;
        match self.apps.platform().close(&app.close) {
            Ok(RunningState::Running { windows }) => Ok(ApplicationResult::CloseRequested {
                application: app.summary(),
                windows,
            }
            .output()),
            Ok(RunningState::NotRunning) => Err(ApplicationResult::NotRunning {
                application: app.summary(),
            }
            .declined(Severity::Attention)),
            Ok(RunningState::NoClosableWindow) | Err(_) => Err(unsupported(&app)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::manager::test_support::FakeApps;
    use super::*;

    fn tools() -> (Arc<FakeApps>, ToolRegistry) {
        let fake = FakeApps::new();
        let manager = Arc::new(ApplicationManager::new(fake.clone(), || 0));
        let mut registry = ToolRegistry::default();
        register(&mut registry, manager).unwrap();
        (fake, registry)
    }

    fn tool<'a>(registry: &'a ToolRegistry, id: &str) -> &'a Arc<dyn Tool> {
        registry.get(&ToolId::new(id).unwrap()).unwrap()
    }

    fn data(result: Result<ToolOutput, ToolError>) -> Value {
        match result {
            Ok(output) | Err(ToolError::Declined { output, .. }) => output.data,
            Err(other) => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn open_is_safe_and_permissioned_close_is_sensitive() {
        let (_, registry) = tools();
        let open = tool(&registry, OPEN_APPLICATION).definition();
        assert_eq!(open.risk, RiskLevel::Safe);
        assert_eq!(open.permissions[0].as_str(), APPS_LAUNCH);
        let close = tool(&registry, CLOSE_APPLICATION).definition();
        assert_eq!(close.risk, RiskLevel::Sensitive);
        assert_eq!(close.permissions[0].as_str(), APPS_CLOSE);
        assert_eq!(open.platforms, [Platform::Windows]);
    }

    #[test]
    fn opens_the_resolved_application() {
        let (fake, registry) = tools();
        let out =
            data(tool(&registry, OPEN_APPLICATION).execute(&json!({"application": "spotify"})));
        assert_eq!(out["kind"], "opened");
        assert_eq!(out["application"]["displayName"], "Spotify");
        assert_eq!(fake.launches(), 1);
    }

    #[test]
    fn never_accepts_paths_commands_or_extra_fields() {
        let (fake, registry) = tools();
        let open = tool(&registry, OPEN_APPLICATION);
        for input in [
            json!({"application": "C:\\Windows\\System32\\cmd.exe"}),
            json!({"application": "cmd /c del *"}),
            json!({"application": "powershell | calc"}),
            json!({"application": "../../evil"}),
            json!({"application": ""}),
            json!({"application": "x".repeat(MAX_APPLICATION_NAME + 1)}),
            json!({"application": "Spotify", "arguments": "--evil"}),
            json!({"application": "Spotify", "path": "C:\\evil.exe"}),
            json!({"command": "spotify.exe"}),
        ] {
            assert!(
                matches!(open.execute(&input), Err(ToolError::InvalidInput(_))),
                "{input}"
            );
        }
        assert_eq!(fake.launches(), 0);
    }

    #[test]
    fn not_found_and_ambiguous_never_launch() {
        let (fake, registry) = tools();
        let open = tool(&registry, OPEN_APPLICATION);
        let missing = data(open.execute(&json!({"application": "Photoshop"})));
        assert_eq!(missing, json!({"kind": "notFound", "query": "Photoshop"}));
        let ambiguous = data(open.execute(&json!({"application": "Visual Studio"})));
        assert_eq!(ambiguous["kind"], "ambiguous");
        assert_eq!(ambiguous["candidates"].as_array().map(Vec::len), Some(2));
        assert_eq!(fake.launches(), 0);
    }

    #[test]
    fn launch_errors_are_structured_without_internal_details() {
        let (fake, registry) = tools();
        *fake.launch_result.lock().unwrap() = Err(ApplicationError::Failed(
            "HRESULT 0x80070005 at C:\\Users\\alice\\AppData\\x.exe".to_owned(),
        ));
        let result = tool(&registry, OPEN_APPLICATION).execute(&json!({"application": "Chrome"}));
        let Err(ToolError::Declined { output, severity }) = result else {
            panic!("expected a declined result");
        };
        assert_eq!(severity, Severity::Failure);
        assert_eq!(output.data["reason"], "failed");
        let text = serde_json::to_string(&output).unwrap();
        assert!(!text.contains("alice") && !text.contains("0x8007"));
    }

    #[test]
    fn close_prepares_a_trusted_subject_only_for_running_closable_apps() {
        let (fake, registry) = tools();
        let close = tool(&registry, CLOSE_APPLICATION);
        let prepared = close.prepare(&json!({"application": "spotify"})).unwrap();
        assert_eq!(prepared.action, ConfirmationAction::CloseApplication);
        assert!(matches!(
            prepared.subject,
            Some(ConfirmationSubject::Application { ref application })
                if application.display_name == "Spotify"
        ));

        *fake.running.lock().unwrap() = RunningState::NotRunning;
        let not_running = data(
            close
                .prepare(&json!({"application": "spotify"}))
                .map(|_| ToolOutput::new(Value::Null, "")),
        );
        assert_eq!(not_running["kind"], "notRunning");

        *fake.running.lock().unwrap() = RunningState::NoClosableWindow;
        assert!(matches!(
            close.prepare(&json!({"application": "spotify"})),
            Err(ToolError::Declined { .. })
        ));
        // The shell host is never closable.
        assert!(matches!(
            close.prepare(&json!({"application": "File Explorer"})),
            Err(ToolError::Declined { .. })
        ));
        assert_eq!(fake.closes(), 0, "prepare never closes anything");
    }

    #[test]
    fn close_asks_windows_to_close_and_reports_it() {
        let (fake, registry) = tools();
        let out =
            data(tool(&registry, CLOSE_APPLICATION).execute(&json!({"application": "chrome"})));
        assert_eq!(out["kind"], "closeRequested");
        assert_eq!(out["application"]["displayName"], "Google Chrome");
        assert_eq!(fake.closes(), 1);
    }
}
