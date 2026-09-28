//! Writes canonical JSON payloads for the TypeScript contract tests.
//!
//! Run with `cargo test -p sershi-core --features ts`. The same command
//! regenerates `packages/contracts/src/generated`. CI fails if either output
//! differs from what is committed, so Rust and TypeScript cannot drift.

// Test-only module (compiled under `cfg(all(test, feature = "ts"))`, which
// clippy does not recognise as test code): failing loudly is the point.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;

use crate::builtin::{register_all, test_support::FakeSystem};
use crate::executor::ToolExecutor;
use crate::intent::KeywordIntentResolver;
use crate::permission::PermissionGrants;
use crate::platform::Platform;
use crate::policy::PolicyEngine;
use crate::ports::SystemInfoProvider;
use crate::service::{AssistantService, CommandRequest};
use crate::tool::ToolRegistry;

fn write<T: Serialize>(name: &str, value: &T) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/contracts/fixtures");
    std::fs::create_dir_all(&dir).unwrap();
    let json = serde_json::to_string_pretty(value).unwrap() + "\n";
    std::fs::write(dir.join(format!("{name}.json")), json).unwrap();
}

#[test]
fn write_contract_fixtures() {
    let mut registry = ToolRegistry::default();
    register_all(&mut registry, Arc::new(FakeSystem::ok())).unwrap();
    let mut service = AssistantService::new(
        ToolExecutor::new(registry, PolicyEngine::new(Platform::Windows)),
        Box::new(KeywordIntentResolver),
        PermissionGrants::default(),
        || 1_767_225_600_000,
    );
    let outcome = service.submit(
        &CommandRequest {
            text: "memory".into(),
        },
        &mut |_| {},
    );
    let unavailable = service.submit(
        &CommandRequest {
            text: "open spotify".into(),
        },
        &mut |_| {},
    );

    let answer = service.submit(
        &CommandRequest {
            text: "hola".into(),
        },
        &mut |_| {},
    );
    write("system-snapshot", &FakeSystem::ok().snapshot().unwrap());
    write("assistant-snapshot", &service.snapshot());
    write("command-outcome-completed", &outcome);
    write("command-outcome-unavailable", &unavailable);
    write("command-outcome-answer", &answer);
    write("activity", &service.recent_activity(10));
    write("tool-definitions", &service.tool_definitions());
}
