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

/// Prints this machine's catalog (names and sources only) and how the
/// physically observed Gate 3C queries resolve. Manual:
/// `cargo test -p sershi-platform --test windows_applications -- --ignored --nocapture`.
#[test]
#[ignore = "inventory of the local machine for Gate 3C"]
fn print_catalog_inventory() {
    let platform = sershi_platform::application_platform();
    let Ok(discovered) = platform.discover() else {
        return;
    };
    let catalog = ApplicationCatalog::build(discovered);
    for app in catalog.list() {
        println!("{:?}\t{}\t{}", app.source, app.id, app.display_name);
    }
    for q in [
        "PowerShell",
        "Windows PowerShell",
        "Windows PowerShell ISE",
        "Word",
        "Microsoft Word",
        "World",
        "notas",
        "Outlook",
        "Catcode",
        "Excel",
    ] {
        println!("{q:?} → {:?}", catalog.resolve(q));
    }
}

/// Gate 4.1.1: Calculator (a packaged app drawn inside an
/// ApplicationFrameHost frame) can be found and politely closed. Manual — it
/// opens a window: `cargo test -p sershi-platform --test windows_applications
/// -- --ignored calculator --nocapture`.
#[test]
#[ignore = "opens and closes Calculator on this machine"]
fn calculator_can_be_closed_through_its_frame_window() {
    let platform = sershi_platform::application_platform();
    let catalog = ApplicationCatalog::build(platform.discover().expect("discovery"));
    let Resolution::Found {
        application: descriptor,
        ..
    } = catalog.resolve("calculadora")
    else {
        panic!("Calculator is in the catalog");
    };
    platform.launch(&descriptor.target).expect("launch");
    let wait = |want: fn(&RunningState) -> bool| {
        let started = Instant::now();
        loop {
            let state = platform.running_state(&descriptor.close);
            if state.as_ref().is_ok_and(want) || started.elapsed().as_secs() > 10 {
                return state;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    };
    let running = wait(|s| matches!(s, RunningState::Running { .. }));
    println!("calculator after launch: {running:?}");
    assert!(matches!(running, Ok(RunningState::Running { .. })));
    let closed = platform.close(&descriptor.close);
    println!("close request: {closed:?}");
    let after = wait(|s| *s == RunningState::NotRunning);
    println!("calculator after close: {after:?}");
    assert_eq!(after, Ok(RunningState::NotRunning));
}
