//! Typed tools: the only way SERSHI acts on the computer.
//!
//! A tool is a narrow, typed capability (`system.get_memory`, later
//! `system.open_application`). It declares its risk and the permissions it
//! needs; the [`crate::policy::PolicyEngine`] decides whether a call may run.
//! There is no generic "run this command" tool and there never will be.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::confirmation::{ConfirmationAction, ConfirmationSubject};
use crate::ids::{PermissionId, ToolId};
use crate::platform::Platform;

/// How much damage a tool could do if misused. Declared by the tool
/// definition, never by the caller: a model cannot downgrade the risk of the
/// call it proposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum RiskLevel {
    /// Read-only or trivially reversible (read memory usage, open an app).
    Safe,
    /// Changes user data or sends something on the user's behalf.
    Sensitive,
    /// Destructive, financial, credential or security-relevant.
    HighRisk,
}

/// Static description of a tool, suitable for UIs, audit and (later) for
/// presenting to a language model as a function schema.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub id: ToolId,
    pub name: String,
    pub description: String,
    /// JSON Schema for the input object.
    pub input_schema: Value,
    /// JSON Schema for [`ToolOutput::data`].
    pub output_schema: Value,
    pub permissions: Vec<PermissionId>,
    pub risk: RiskLevel,
    /// Upper bound on execution time. Enforced by the asynchronous execution
    /// runtime introduced with the first long-running tool; every tool shipped
    /// today is a bounded, synchronous read.
    pub timeout_ms: u32,
    pub platforms: Vec<Platform>,
}

/// Who initiated a call. Recorded for audit; the policy engine may treat
/// model-proposed calls more strictly as capabilities grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum CallOrigin {
    /// Derived from the user's own command.
    User,
    /// Proposed by a language model.
    Agent,
    /// Started by a routine.
    Routine,
}

/// A structured request to run one tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub tool_id: ToolId,
    pub input: Value,
    pub origin: CallOrigin,
}

impl ToolCall {
    pub fn new(tool_id: ToolId, input: Value, origin: CallOrigin) -> Self {
        Self {
            tool_id,
            input,
            origin,
        }
    }
}

/// What a tool returns: structured data plus a short, human-readable
/// summary composed by the tool itself (not by a model), so results can be
/// shown faithfully even without a language model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ToolOutput {
    pub data: Value,
    /// Canonical English summary.
    pub summary: String,
    /// Trusted name of what the tool acted on (e.g. a resolved application's
    /// display name), safe to record in activity. Never raw user input.
    pub subject: Option<String>,
}

impl ToolOutput {
    pub fn new(data: Value, summary: impl Into<String>) -> Self {
        Self {
            data,
            summary: summary.into(),
            subject: None,
        }
    }

    pub fn with_subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }
}

/// How serious a declined outcome is for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Nothing went wrong, but nothing happened (not found, ambiguous…).
    Attention,
    /// The action was attempted and failed (e.g. Windows blocked it).
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ToolError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("capability unavailable: {0}")]
    Unavailable(String),
    #[error("tool failed: {0}")]
    Failed(String),
    /// An expected, structured negative result the user should see, such as
    /// "application not found". `output.data` carries the details.
    #[error("declined: {}", .output.summary)]
    Declined {
        output: ToolOutput,
        severity: Severity,
    },
}

/// The result of [`Tool::prepare`]: what a call would act on, resolved from
/// trusted data before anything runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    pub action: ConfirmationAction,
    pub subject: Option<ConfirmationSubject>,
}

impl Default for Prepared {
    fn default() -> Self {
        Self {
            action: ConfirmationAction::RunTool,
            subject: None,
        }
    }
}

pub trait Tool: Send + Sync + fmt::Debug {
    fn definition(&self) -> &ToolDefinition;

    /// Validates input and resolves the call's target without side effects.
    /// Runs after policy allows the call and before any confirmation, so the
    /// confirmation can name the resolved target rather than echo input.
    fn prepare(&self, _input: &Value) -> Result<Prepared, ToolError> {
        Ok(Prepared::default())
    }

    fn execute(&self, input: &Value) -> Result<ToolOutput, ToolError>;
}

/// Parses a tool's input into its typed form. Tools use
/// `#[serde(deny_unknown_fields)]` input structs so unexpected keys (a common
/// prompt-injection vector) are rejected rather than ignored.
pub fn parse_input<T: DeserializeOwned>(input: &Value) -> Result<T, ToolError> {
    let normalized = if input.is_null() {
        Value::Object(Default::default())
    } else {
        input.clone()
    };
    serde_json::from_value(normalized).map_err(|e| ToolError::InvalidInput(e.to_string()))
}

/// Input type for tools that take no arguments.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoInput {}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("tool `{0}` is already registered")]
    Duplicate(ToolId),
    #[error("tool `{0}` must declare at least one permission")]
    MissingPermissions(ToolId),
}

/// The set of tools SERSHI can use. Anything not registered does not exist
/// as far as the agent is concerned.
#[derive(Debug, Default, Clone)]
pub struct ToolRegistry {
    tools: BTreeMap<ToolId, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn register(&mut self, tool: Arc<dyn Tool>) -> Result<(), RegistryError> {
        let def = tool.definition();
        if def.permissions.is_empty() {
            return Err(RegistryError::MissingPermissions(def.id.clone()));
        }
        if self.tools.contains_key(&def.id) {
            return Err(RegistryError::Duplicate(def.id.clone()));
        }
        self.tools.insert(def.id.clone(), tool);
        Ok(())
    }

    pub fn get(&self, id: &ToolId) -> Option<&Arc<dyn Tool>> {
        self.tools.get(id)
    }

    pub fn definitions(&self) -> impl Iterator<Item = &ToolDefinition> {
        self.tools.values().map(|t| t.definition())
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use serde_json::json;

    /// A configurable fake tool for policy and executor tests.
    #[derive(Debug)]
    pub struct FakeTool {
        pub def: ToolDefinition,
        pub result: Result<ToolOutput, ToolError>,
    }

    impl FakeTool {
        pub fn new(id: &str, risk: RiskLevel, permissions: &[&str]) -> Self {
            Self {
                def: ToolDefinition {
                    id: ToolId::new(id).unwrap(),
                    name: id.to_owned(),
                    description: "test tool".to_owned(),
                    input_schema: json!({"type": "object"}),
                    output_schema: json!({"type": "object"}),
                    permissions: permissions
                        .iter()
                        .map(|p| PermissionId::new(*p).unwrap())
                        .collect(),
                    risk,
                    timeout_ms: 1_000,
                    platforms: vec![Platform::Windows, Platform::Linux, Platform::Macos],
                },
                result: Ok(ToolOutput::new(json!({"ok": true}), "done")),
            }
        }
    }

    impl Tool for FakeTool {
        fn definition(&self) -> &ToolDefinition {
            &self.def
        }
        fn execute(&self, _input: &Value) -> Result<ToolOutput, ToolError> {
            self.result.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::FakeTool;
    use super::*;
    use serde_json::json;

    #[test]
    fn registry_rejects_duplicates_and_permissionless_tools() {
        let mut registry = ToolRegistry::default();
        registry
            .register(Arc::new(FakeTool::new("a.b", RiskLevel::Safe, &["a.read"])))
            .unwrap();
        assert!(matches!(
            registry.register(Arc::new(FakeTool::new("a.b", RiskLevel::Safe, &["a.read"]))),
            Err(RegistryError::Duplicate(_))
        ));
        assert!(matches!(
            registry.register(Arc::new(FakeTool::new("a.c", RiskLevel::Safe, &[]))),
            Err(RegistryError::MissingPermissions(_))
        ));
    }

    #[test]
    fn no_input_accepts_null_and_empty_but_rejects_extra_fields() {
        assert!(parse_input::<NoInput>(&Value::Null).is_ok());
        assert!(parse_input::<NoInput>(&json!({})).is_ok());
        assert!(matches!(
            parse_input::<NoInput>(&json!({"command": "format c:"})),
            Err(ToolError::InvalidInput(_))
        ));
    }
}
