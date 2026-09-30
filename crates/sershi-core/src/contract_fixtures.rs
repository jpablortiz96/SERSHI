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

use crate::apps::manager::test_support::FakeApps;
use crate::apps::{ApplicationManager, tools as app_tools};
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

/// Submits and pins the (timed) understanding diagnostics so fixtures are
/// stable.
fn submit(service: &mut AssistantService, text: &str) -> crate::service::CommandOutcome {
    let mut outcome = service.submit(&CommandRequest { text: text.into() }, &mut |_| {});
    if let Some(trace) = outcome.understanding.as_mut() {
        trace.understanding_ms = 0.0;
        trace.semantic_ms = None;
    }
    outcome
}

#[test]
fn write_contract_fixtures() {
    let mut registry = ToolRegistry::default();
    register_all(&mut registry, Arc::new(FakeSystem::ok())).unwrap();
    let apps = Arc::new(ApplicationManager::new(FakeApps::new(), || {
        1_767_225_600_000
    }));
    app_tools::register(&mut registry, apps).unwrap();
    let mut service = AssistantService::new(
        ToolExecutor::new(registry, PolicyEngine::new(Platform::Windows)),
        Box::new(KeywordIntentResolver),
        PermissionGrants::default(),
        || 1_767_225_600_000,
    );
    let completed = submit(&mut service, "memory");
    let unavailable = submit(&mut service, "battery status");
    let answer = submit(&mut service, "hola");
    let opened = submit(&mut service, "Abre Spotify");
    let not_found = submit(&mut service, "Open Photoshop");
    let ambiguous = submit(&mut service, "open visual studio");
    let confirmation = submit(&mut service, "Close Spotify");

    // What only the trusted confirmation surface receives. Ids are random;
    // pin one so the fixture is stable.
    let mut request = serde_json::to_value(service.pending_confirmation().unwrap()).unwrap();
    request["id"] = "0123456789abcdef0123456789abcdef".into();

    write("system-snapshot", &FakeSystem::ok().snapshot().unwrap());
    write("assistant-snapshot", &service.snapshot());
    write("command-outcome-completed", &completed);
    write("command-outcome-unavailable", &unavailable);
    write("command-outcome-answer", &answer);
    write("command-outcome-opened", &opened);
    write("command-outcome-not-found", &not_found);
    write("command-outcome-ambiguous", &ambiguous);
    write("command-outcome-confirmation", &confirmation);
    write("confirmation-request", &request);

    // Gate 3C: with the catalog, an ambiguous request is a question, and a
    // repaired one says what SERSHI understood.
    let mut registry = ToolRegistry::default();
    register_all(&mut registry, Arc::new(FakeSystem::ok())).unwrap();
    let apps = Arc::new(ApplicationManager::new(FakeApps::new(), || {
        1_767_225_600_000
    }));
    app_tools::register(&mut registry, apps.clone()).unwrap();
    let mut understanding = AssistantService::new(
        ToolExecutor::new(registry, PolicyEngine::new(Platform::Windows)),
        Box::new(KeywordIntentResolver),
        PermissionGrants::default(),
        || 1_767_225_600_000,
    )
    .with_applications(apps);
    write(
        "command-outcome-clarification",
        &submit(&mut understanding, "open visual studio"),
    );
    write(
        "command-outcome-understood",
        &submit(&mut understanding, "Apreer Spotify"),
    );
    write("activity", &service.recent_activity(20));
    write("tool-definitions", &service.tool_definitions());
}
