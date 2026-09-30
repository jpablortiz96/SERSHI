//! Owns the in-memory application catalog and the platform adapter.
//!
//! The catalog is built lazily on first use (or by an explicit refresh), never
//! by polling. A request that finds nothing in a catalog older than
//! [`STALE_AFTER_MS`] rescans once, so newly installed applications are found
//! without periodic scans.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;

use super::catalog::{ApplicationCatalog, CatalogNames, Resolution};
use super::model::ApplicationSummary;
use crate::ports::{ApplicationPlatform, PortError};
use crate::service::Clock;

/// Age after which a "not found" triggers one rescan.
pub const STALE_AFTER_MS: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum CatalogState {
    Unsupported,
    NotScanned,
    Ready,
    Failed,
}

/// What Settings and Developer Mode may show about the catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct CatalogStatus {
    pub state: CatalogState,
    pub count: u32,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub refreshed_at_ms: Option<u64>,
    pub scan_duration_ms: Option<u32>,
}

#[derive(Debug, Default)]
struct Inner {
    catalog: Option<ApplicationCatalog>,
    refreshed_at_ms: Option<u64>,
    scan_duration_ms: Option<u32>,
    failed: bool,
}

#[derive(Debug)]
pub struct ApplicationManager {
    platform: Arc<dyn ApplicationPlatform>,
    clock: Clock,
    inner: Mutex<Inner>,
}

impl ApplicationManager {
    pub fn new(platform: Arc<dyn ApplicationPlatform>, clock: Clock) -> Self {
        Self {
            platform,
            clock,
            inner: Mutex::new(Inner::default()),
        }
    }

    pub fn platform(&self) -> &dyn ApplicationPlatform {
        self.platform.as_ref()
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>, PortError> {
        self.inner
            .lock()
            .map_err(|_| PortError::Platform("application catalog lock poisoned".to_owned()))
    }

    pub fn status(&self) -> CatalogStatus {
        let supported = self.platform.is_supported();
        match self.lock() {
            Ok(inner) => status_of(&inner, supported),
            Err(_) => CatalogStatus {
                state: CatalogState::Failed,
                count: 0,
                refreshed_at_ms: None,
                scan_duration_ms: None,
            },
        }
    }

    /// Rescans installed applications.
    pub fn refresh(&self) -> Result<CatalogStatus, PortError> {
        let mut inner = self.lock()?;
        self.scan(&mut inner)?;
        Ok(status_of(&inner, self.platform.is_supported()))
    }

    /// Applications for Developer Mode inspection (no paths).
    pub fn applications(&self) -> Vec<ApplicationSummary> {
        self.lock()
            .ok()
            .and_then(|inner| inner.catalog.as_ref().map(ApplicationCatalog::list))
            .unwrap_or_default()
    }

    /// Names of every trusted application (scanning first if needed), for
    /// scored matching. Paths never leave the catalog.
    pub fn names(&self) -> Result<Vec<CatalogNames>, PortError> {
        let mut inner = self.lock()?;
        if inner.catalog.is_none() {
            self.scan(&mut inner)?;
        }
        Ok(inner
            .catalog
            .as_ref()
            .map(ApplicationCatalog::names)
            .unwrap_or_default())
    }

    pub fn resolve(&self, query: &str) -> Result<Resolution, PortError> {
        let mut inner = self.lock()?;
        if inner.catalog.is_none() {
            self.scan(&mut inner)?;
        }
        let first = inner
            .catalog
            .as_ref()
            .map_or(Resolution::NotFound, |c| c.resolve(query));
        let stale = inner
            .refreshed_at_ms
            .is_none_or(|at| (self.clock)().saturating_sub(at) >= STALE_AFTER_MS);
        if first != Resolution::NotFound || !stale {
            return Ok(first);
        }
        self.scan(&mut inner)?;
        Ok(inner
            .catalog
            .as_ref()
            .map_or(Resolution::NotFound, |c| c.resolve(query)))
    }

    fn scan(&self, inner: &mut Inner) -> Result<(), PortError> {
        let started = Instant::now();
        match self.platform.discover() {
            Ok(apps) => {
                inner.catalog = Some(ApplicationCatalog::build(apps));
                inner.refreshed_at_ms = Some((self.clock)());
                inner.scan_duration_ms =
                    Some(u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX));
                inner.failed = false;
                Ok(())
            }
            Err(error) => {
                inner.failed = true;
                Err(error)
            }
        }
    }
}

fn status_of(inner: &Inner, supported: bool) -> CatalogStatus {
    let state = if !supported {
        CatalogState::Unsupported
    } else if inner.failed {
        CatalogState::Failed
    } else if inner.catalog.is_some() {
        CatalogState::Ready
    } else {
        CatalogState::NotScanned
    };
    CatalogStatus {
        state,
        count: inner
            .catalog
            .as_ref()
            .map_or(0, |c| u32::try_from(c.len()).unwrap_or(u32::MAX)),
        refreshed_at_ms: inner.refreshed_at_ms,
        scan_duration_ms: inner.scan_duration_ms,
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};

    use super::super::catalog::fixtures;
    use super::super::model::{AppSource, ApplicationDescriptor, CloseSupport, LaunchTarget};
    use super::*;
    use crate::ports::{ApplicationError, RunningState};

    /// A scripted platform: fixture catalog, recorded launches/closes.
    #[derive(Debug)]
    pub struct FakeApps {
        pub apps: Mutex<Vec<ApplicationDescriptor>>,
        pub scans: AtomicU32,
        pub launched: Mutex<Vec<LaunchTarget>>,
        pub closed: Mutex<Vec<CloseSupport>>,
        pub launch_result: Mutex<Result<(), ApplicationError>>,
        pub running: Mutex<RunningState>,
    }

    impl FakeApps {
        pub fn new() -> Arc<Self> {
            Arc::new(Self {
                apps: Mutex::new(vec![
                    fixtures::packaged("Spotify", "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
                    fixtures::exe("Google Chrome", AppSource::StartMenu, "chrome.exe"),
                    fixtures::exe("Visual Studio Code", AppSource::StartMenu, "Code.exe"),
                    fixtures::exe("Visual Studio 2022", AppSource::StartMenu, "devenv.exe"),
                    fixtures::builtin("windows.explorer", "File Explorer", &["explorer"]),
                ]),
                scans: AtomicU32::new(0),
                launched: Mutex::new(vec![]),
                closed: Mutex::new(vec![]),
                launch_result: Mutex::new(Ok(())),
                running: Mutex::new(RunningState::Running { windows: 1 }),
            })
        }

        pub fn launches(&self) -> usize {
            self.launched.lock().unwrap().len()
        }

        pub fn closes(&self) -> usize {
            self.closed.lock().unwrap().len()
        }
    }

    impl ApplicationPlatform for FakeApps {
        fn is_supported(&self) -> bool {
            true
        }
        fn discover(&self) -> Result<Vec<ApplicationDescriptor>, PortError> {
            self.scans.fetch_add(1, Ordering::SeqCst);
            Ok(self.apps.lock().unwrap().clone())
        }
        fn launch(&self, target: &LaunchTarget) -> Result<(), ApplicationError> {
            let result = self.launch_result.lock().unwrap().clone();
            if result.is_ok() {
                self.launched.lock().unwrap().push(target.clone());
            }
            result
        }
        fn running_state(&self, _: &CloseSupport) -> Result<RunningState, ApplicationError> {
            Ok(*self.running.lock().unwrap())
        }
        fn close(&self, close: &CloseSupport) -> Result<RunningState, ApplicationError> {
            self.closed.lock().unwrap().push(close.clone());
            Ok(*self.running.lock().unwrap())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::atomic::Ordering;

    use super::test_support::FakeApps;
    use super::*;
    use crate::apps::catalog::fixtures;
    use crate::apps::model::AppSource;

    // Each test runs on its own thread, so a thread-local clock is isolated.
    thread_local!(static NOW: Cell<u64> = const { Cell::new(0) });
    fn clock() -> u64 {
        NOW.with(Cell::get)
    }
    fn set_now(ms: u64) {
        NOW.with(|n| n.set(ms));
    }

    #[test]
    fn scans_lazily_once_and_reports_status() {
        set_now(1_000);
        let fake = FakeApps::new();
        let manager = ApplicationManager::new(fake.clone(), clock);
        assert_eq!(manager.status().state, CatalogState::NotScanned);
        assert_eq!(fake.scans.load(Ordering::SeqCst), 0, "no scan at start-up");

        manager.resolve("Spotify").unwrap();
        manager.resolve("Chrome").unwrap();
        assert_eq!(
            fake.scans.load(Ordering::SeqCst),
            1,
            "cached after first use"
        );
        let status = manager.status();
        assert_eq!(status.state, CatalogState::Ready);
        assert_eq!(status.count, 5);
        assert_eq!(status.refreshed_at_ms, Some(1_000));
    }

    #[test]
    fn a_stale_miss_rescans_once_and_finds_new_installs() {
        set_now(0);
        let fake = FakeApps::new();
        let manager = ApplicationManager::new(fake.clone(), clock);
        assert_eq!(manager.resolve("Blender").unwrap(), Resolution::NotFound);

        fake.apps.lock().unwrap().push(fixtures::exe(
            "Blender",
            AppSource::StartMenu,
            "blender.exe",
        ));
        // Fresh catalog: no rescan for a miss.
        assert_eq!(manager.resolve("Blender").unwrap(), Resolution::NotFound);
        assert_eq!(fake.scans.load(Ordering::SeqCst), 1);

        set_now(STALE_AFTER_MS);
        assert!(matches!(
            manager.resolve("Blender").unwrap(),
            Resolution::Found { .. }
        ));
        assert_eq!(fake.scans.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn unsupported_platforms_say_so() {
        let manager =
            ApplicationManager::new(Arc::new(crate::ports::UnsupportedApplications), clock);
        assert_eq!(manager.status().state, CatalogState::Unsupported);
        assert!(manager.resolve("Spotify").is_err());
    }
}
