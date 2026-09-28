//! Built-in tools. Each is read-only and depends only on a port, so it runs
//! identically against a real platform adapter or a test fake.

use std::sync::Arc;

use serde_json::{Value, json};

use thiserror::Error;

use crate::ids::{InvalidId, PermissionId, ToolId};
use crate::platform::Platform;
use crate::ports::{PortError, SystemInfoProvider};
use crate::system::{SystemSnapshot, format_bytes};
use crate::tool::{
    NoInput, RegistryError, RiskLevel, Tool, ToolDefinition, ToolError, ToolOutput, ToolRegistry,
    parse_input,
};

pub const SYSTEM_INFO_READ: &str = "system.info.read";

#[derive(Debug, Error)]
pub enum BuiltinError {
    #[error(transparent)]
    Id(#[from] InvalidId),
    #[error(transparent)]
    Registry(#[from] RegistryError),
}

/// Registers every built-in tool.
pub fn register_all(
    registry: &mut ToolRegistry,
    system: Arc<dyn SystemInfoProvider>,
) -> Result<(), BuiltinError> {
    for kind in [SystemTool::Info, SystemTool::Memory, SystemTool::Cpu] {
        registry.register(Arc::new(SystemInfoTool::new(kind, system.clone())?))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SystemTool {
    Info,
    Memory,
    Cpu,
}

#[derive(Debug)]
struct SystemInfoTool {
    kind: SystemTool,
    definition: ToolDefinition,
    system: Arc<dyn SystemInfoProvider>,
}

impl SystemInfoTool {
    fn new(kind: SystemTool, system: Arc<dyn SystemInfoProvider>) -> Result<Self, InvalidId> {
        let (id, name, description) = match kind {
            SystemTool::Info => (
                "system.get_info",
                "System information",
                "Operating system, architecture, processor, memory and uptime.",
            ),
            SystemTool::Memory => (
                "system.get_memory",
                "Memory usage",
                "Total and used physical memory.",
            ),
            SystemTool::Cpu => (
                "system.get_cpu",
                "Processor usage",
                "Processor model, core count and current load.",
            ),
        };
        let definition = ToolDefinition {
            id: ToolId::new(id)?,
            name: name.to_owned(),
            description: description.to_owned(),
            input_schema: json!({"type": "object", "additionalProperties": false}),
            output_schema: json!({"type": "object"}),
            permissions: vec![PermissionId::new(SYSTEM_INFO_READ)?],
            risk: RiskLevel::Safe,
            timeout_ms: 2_000,
            platforms: vec![Platform::Windows, Platform::Linux, Platform::Macos],
        };
        Ok(Self {
            kind,
            definition,
            system,
        })
    }
}

impl Tool for SystemInfoTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    fn execute(&self, input: &Value) -> Result<ToolOutput, ToolError> {
        parse_input::<NoInput>(input)?;
        let snapshot = self.system.snapshot().map_err(|e| match e {
            PortError::Unsupported(what) => ToolError::Unavailable(what),
            PortError::Platform(msg) => ToolError::Failed(msg),
        })?;
        Ok(match self.kind {
            SystemTool::Info => info_output(&snapshot),
            SystemTool::Memory => memory_output(&snapshot),
            SystemTool::Cpu => cpu_output(&snapshot),
        })
    }
}

fn to_value<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn info_output(s: &SystemSnapshot) -> ToolOutput {
    let version =
        s.os.version
            .as_deref()
            .map(|v| format!(" {v}"))
            .unwrap_or_default();
    ToolOutput::new(
        to_value(s),
        format!(
            "{}{version} on {}, {} with {} logical cores, {} of memory.",
            s.os.name,
            s.os.arch,
            s.cpu.brand,
            s.cpu.logical_cores,
            format_bytes(s.memory.total_bytes),
        ),
    )
}

fn memory_output(s: &SystemSnapshot) -> ToolOutput {
    ToolOutput::new(
        to_value(&s.memory),
        format!(
            "You're using {} of {} memory ({:.0}%).",
            format_bytes(s.memory.used_bytes),
            format_bytes(s.memory.total_bytes),
            s.memory.used_percent(),
        ),
    )
}

fn cpu_output(s: &SystemSnapshot) -> ToolOutput {
    let load = match s.cpu.usage_percent {
        Some(p) => format!("running at {p:.0}%"),
        None => "still taking its first measurement — ask again in a moment".to_owned(),
    };
    ToolOutput::new(
        to_value(&s.cpu),
        format!(
            "Your processor is {load} across {} cores.",
            s.cpu.logical_cores
        ),
    )
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::system::{CpuInfo, MemoryInfo, OsInfo};

    #[derive(Debug)]
    pub struct FakeSystem(pub Result<SystemSnapshot, PortError>);

    impl FakeSystem {
        pub fn ok() -> Self {
            Self(Ok(SystemSnapshot {
                os: OsInfo {
                    platform: Platform::Windows,
                    name: "Windows 11 Pro".into(),
                    version: Some("24H2".into()),
                    arch: "x86_64".into(),
                },
                cpu: CpuInfo {
                    brand: "Test CPU".into(),
                    logical_cores: 16,
                    usage_percent: Some(18.4),
                },
                memory: MemoryInfo {
                    total_bytes: 32 * 1024 * 1024 * 1024,
                    used_bytes: 12 * 1024 * 1024 * 1024,
                },
                uptime_secs: 3_600,
            }))
        }
    }

    impl SystemInfoProvider for FakeSystem {
        fn snapshot(&self) -> Result<SystemSnapshot, PortError> {
            self.0.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::FakeSystem;
    use super::*;

    fn registry(system: FakeSystem) -> ToolRegistry {
        let mut r = ToolRegistry::default();
        register_all(&mut r, Arc::new(system)).unwrap();
        r
    }

    fn run(r: &ToolRegistry, id: &str) -> Result<ToolOutput, ToolError> {
        r.get(&ToolId::new(id).unwrap())
            .unwrap()
            .execute(&Value::Null)
    }

    #[test]
    fn all_builtins_are_safe_read_only_tools() {
        let r = registry(FakeSystem::ok());
        let defs: Vec<_> = r.definitions().collect();
        assert_eq!(defs.len(), 3);
        for d in defs {
            assert_eq!(d.risk, RiskLevel::Safe, "{}", d.id);
            assert_eq!(d.permissions[0].as_str(), SYSTEM_INFO_READ);
        }
    }

    #[test]
    fn memory_summary_is_human_readable() {
        let out = run(&registry(FakeSystem::ok()), "system.get_memory").unwrap();
        assert_eq!(out.summary, "You're using 12.0 GB of 32.0 GB memory (38%).");
        assert_eq!(out.data["totalBytes"], 32_u64 * 1024 * 1024 * 1024);
    }

    #[test]
    fn cpu_summary_is_honest_about_missing_first_sample() {
        let mut fake = FakeSystem::ok();
        if let Ok(s) = fake.0.as_mut() {
            s.cpu.usage_percent = None;
        }
        let out = run(&registry(fake), "system.get_cpu").unwrap();
        assert!(out.summary.contains("first measurement"));
    }

    #[test]
    fn port_errors_map_to_tool_errors() {
        let r = registry(FakeSystem(Err(PortError::Unsupported("memory".into()))));
        assert!(matches!(
            run(&r, "system.get_memory"),
            Err(ToolError::Unavailable(_))
        ));
    }

    #[test]
    fn rejects_unexpected_input() {
        let r = registry(FakeSystem::ok());
        let tool = r.get(&ToolId::new("system.get_info").unwrap()).unwrap();
        assert!(tool.execute(&json!({"path": "C:\\"})).is_err());
    }
}
