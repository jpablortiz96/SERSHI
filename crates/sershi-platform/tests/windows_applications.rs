//! Read-only checks of the Windows application adapter against the real OS
//! (runs on the Windows CI runner). Nothing is launched or closed: CI must not
//! start GUI applications. Launch and close are covered by the physical
//! checklist in docs/WINDOWS_PLATFORM.md.

#![cfg(windows)]

use std::path::PathBuf;
use std::time::Instant;

use sershi_core::apps::{ApplicationCatalog, CloseSupport, MatchKind, Resolution};
use sershi_core::ports::RunningState;

fn found_id(catalog: &ApplicationCatalog, query: &str) -> Option<String> {
    match catalog.resolve(query) {
        Resolution::Found { application, .. } => Some(application.id),
        _ => None,
    }
}

#[test]
fn discovers_the_built_in_applications_quickly() {
    let platform = sershi_platform::application_platform();
    assert!(platform.is_supported());

    let started = Instant::now();
    let discovered = platform.discover().expect("discovery succeeds");
    let elapsed = started.elapsed();
    let catalog = ApplicationCatalog::build(discovered);
    let list = catalog.list();

    // Printed for the performance section of the validation record
    // (visible with `-- --nocapture`).
    println!(
        "application discovery: {} ms, {} applications",
        elapsed.as_millis(),
        list.len()
    );
    assert!(
        elapsed.as_secs() < 20,
        "discovery must finish within its timeout"
    );

    assert!(list.iter().all(|a| !a.display_name.trim().is_empty()));
    assert_eq!(
        found_id(&catalog, "Settings").as_deref(),
        Some("windows.settings")
    );
    assert_eq!(
        found_id(&catalog, "configuración").as_deref(),
        Some("windows.settings")
    );
    assert_eq!(
        found_id(&catalog, "explorador de arquivos").as_deref(),
        Some("windows.explorer")
    );
    let system32 = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .map(|root| root.join("System32"));
    if system32.is_some_and(|dir| dir.join("notepad.exe").is_file()) {
        assert!(matches!(
            catalog.resolve("bloc de notas"),
            Resolution::Found {
                matched: MatchKind::Alias,
                ..
            }
        ));
    }
}

#[test]
fn an_application_that_is_not_running_is_reported_without_side_effects() {
    let platform = sershi_platform::application_platform();
    let missing = CloseSupport::ExecutablePath(PathBuf::from(
        r"C:\SershiValidation\definitely-not-installed.exe",
    ));
    assert_eq!(
        platform.running_state(&missing),
        Ok(RunningState::NotRunning)
    );
    // File Explorer is never closable.
    assert!(platform.running_state(&CloseSupport::Unsupported).is_err());
}
