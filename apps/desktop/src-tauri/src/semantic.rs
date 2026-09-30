//! Natural command understanding: the optional local semantic model
//! (Gate 3C, docs/SEMANTIC.md).
//!
//! The model is data in `%LOCALAPPDATA%\dev.sershi.desktop\models\semantic`,
//! installed only when the user asks (HTTPS, pinned revision, exact size,
//! SHA-256, atomic rename) and hashed again before its first use in a
//! session — a damaged or replaced file is never loaded. It runs in the
//! `sershi-semantic` process next to SERSHI, loaded lazily (on the first
//! request that needs it, or when the microphone opens) and ended after
//! [`IDLE_RELEASE`] without use.
//!
//! The model only interprets. Its answers go through the core's
//! understanding policy, then the same executor, policy and trusted
//! confirmation as a typed command. Without it (not installed, disabled,
//! failing), understanding keeps working deterministically.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use sershi_core::ipc::{IpcError, IpcErrorCode};
use sershi_core::understanding::models::{DEFAULT_SEMANTIC_MODEL, SemanticModel};
use sershi_core::understanding::semantic::LocalSemanticRouter;
use sershi_core::understanding::status::{SemanticModelInfo, SemanticSettings, SemanticStatus};
use sershi_core::voice::latency::Acceleration;
use sershi_core::voice::{ModelError, ModelProgress, ModelState};
use sershi_platform::semantic::{EngineConfig, EngineGenerator, IDLE_RELEASE};
use sershi_platform::voice::{self as platform, ModelStore};
use tauri::{AppHandle, Emitter, Manager};

use crate::runtime::Runtime;

/// Download progress for the semantic model (Command Center only).
pub const SEMANTIC_MODEL_EVENT: &str = "sershi://semantic-model";
const MAIN: &str = "main";
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
/// How often an idle engine is checked for release.
const IDLE_CHECK: Duration = Duration::from_secs(30);

struct Inner {
    enabled: bool,
    /// The installed file was hashed this session and matched.
    verified: bool,
    generator: Option<EngineGenerator>,
    download: Option<Arc<AtomicBool>>,
    received: Option<u64>,
    corrupt: bool,
}

pub struct Semantic {
    store: ModelStore,
    /// `sershi-semantic.exe` next to SERSHI.
    program: PathBuf,
    inner: Mutex<Inner>,
}

fn model() -> &'static SemanticModel {
    DEFAULT_SEMANTIC_MODEL
}

fn failure(message: &str) -> IpcError {
    IpcError::new(IpcErrorCode::Unavailable, message)
}

impl Semantic {
    pub fn new(models_dir: PathBuf) -> Self {
        let program = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("sershi-semantic.exe")))
            .unwrap_or_default();
        let store = ModelStore::new(models_dir);
        store.remove_partials();
        Self {
            store,
            program,
            inner: Mutex::new(Inner {
                enabled: true,
                verified: false,
                generator: None,
                download: None,
                received: None,
                corrupt: false,
            }),
        }
    }

    fn locked(&self) -> Result<MutexGuard<'_, Inner>, IpcError> {
        self.inner
            .lock()
            .map_err(|_| failure("Natural understanding is unavailable."))
    }

    fn available(&self) -> bool {
        cfg!(windows) && self.program.is_file()
    }

    fn gpu() -> bool {
        platform::speech_acceleration().0 == Acceleration::Vulkan
    }

    fn state(&self, inner: &Inner) -> ModelState {
        if let Some(received) = inner.received {
            return ModelState::Downloading {
                received_bytes: received,
            };
        }
        if inner.corrupt {
            return ModelState::Corrupt;
        }
        self.store.state(model())
    }
}

fn managed(app: &AppHandle) -> Option<tauri::State<'_, Semantic>> {
    app.try_state::<Semantic>()
}

pub fn status(app: &AppHandle) -> Result<SemanticStatus, IpcError> {
    let semantic = managed(app).ok_or_else(|| failure("Not ready yet."))?;
    let inner = semantic.locked()?;
    let engine = inner
        .generator
        .as_ref()
        .map(EngineGenerator::status)
        .unwrap_or_default();
    let gpu = engine
        .backend
        .as_deref()
        .map_or_else(Semantic::gpu, |b| b != "cpu");
    Ok(SemanticStatus {
        model: SemanticModelInfo::of(model(), semantic.state(&inner), gpu),
        enabled: inner.enabled,
        available: semantic.available(),
        running: engine.running,
        backend: engine.backend,
        device: engine.device,
        load_ms: engine.load_ms,
        last_ms: engine.last_ms,
    })
}

pub fn configure(app: &AppHandle, settings: SemanticSettings) -> Result<SemanticStatus, IpcError> {
    let semantic = managed(app).ok_or_else(|| failure("Not ready yet."))?;
    let changed = {
        let mut inner = semantic.locked()?;
        let changed = inner.enabled != settings.enabled;
        inner.enabled = settings.enabled;
        changed
    };
    if changed {
        if settings.enabled {
            activate(app);
        } else {
            deactivate(app);
        }
    }
    status(app)
}

/// At start-up: use an installed model (after verifying it) and start the
/// idle release. Off the main thread.
pub fn setup(app: &AppHandle) {
    activate(app);
    let app = app.clone();
    let _ = thread::Builder::new()
        .name("sershi-semantic-idle".to_owned())
        .spawn(move || {
            loop {
                thread::sleep(IDLE_CHECK);
                let Some(semantic) = managed(&app) else {
                    return;
                };
                let generator = semantic.inner.lock().ok().and_then(|i| i.generator.clone());
                if generator.is_some_and(|g| g.release_if_idle(IDLE_RELEASE)) {
                    eprintln!("SERSHI semantic: model released after inactivity");
                }
            }
        });
}

/// Verifies the installed model and installs the router into the core.
fn activate(app: &AppHandle) {
    let app = app.clone();
    let _ = thread::Builder::new()
        .name("sershi-semantic-activate".to_owned())
        .spawn(move || {
            let Some(semantic) = managed(&app) else {
                return;
            };
            let model = model();
            let ready = semantic
                .inner
                .lock()
                .is_ok_and(|i| i.enabled && i.generator.is_none() && i.download.is_none());
            if !ready
                || !semantic.available()
                || semantic.store.state(model) != ModelState::Installed
            {
                return;
            }
            let verified = semantic.inner.lock().is_ok_and(|i| i.verified);
            if !verified {
                let t0 = Instant::now();
                match semantic.store.verify(model) {
                    Ok(true) => {
                        eprintln!(
                            "SERSHI semantic: model verified in {} ms",
                            t0.elapsed().as_millis()
                        );
                    }
                    _ => {
                        // Never load a file that does not match: report it
                        // and offer a reinstall in Settings.
                        eprintln!("SERSHI semantic: installed model failed verification");
                        if let Ok(mut inner) = semantic.inner.lock() {
                            inner.corrupt = true;
                        }
                        emit(&app, ModelState::Corrupt, Some(ModelError::Integrity));
                        return;
                    }
                }
            }
            let generator = EngineGenerator::new(EngineConfig::new(
                semantic.program.clone(),
                semantic.store.path(model),
                Semantic::gpu(),
            ));
            let router = Arc::new(LocalSemanticRouter::new(generator.clone(), model.template));
            {
                let Ok(mut inner) = semantic.inner.lock() else {
                    return;
                };
                if !inner.enabled {
                    return;
                }
                inner.verified = true;
                inner.corrupt = false;
                inner.generator = Some(generator);
            }
            let runtime = app.state::<Runtime>();
            let _ = runtime.with_service(|s| s.set_semantic_router(Some(router)));
            eprintln!("SERSHI semantic: natural understanding ready");
        });
}

fn deactivate(app: &AppHandle) {
    let runtime = app.state::<Runtime>();
    let _ = runtime.with_service(|s| s.set_semantic_router(None));
    if let Some(semantic) = managed(app) {
        let generator = semantic
            .inner
            .lock()
            .ok()
            .and_then(|mut i| i.generator.take());
        if let Some(generator) = generator {
            generator.release();
        }
    }
}

fn emit(app: &AppHandle, state: ModelState, error: Option<ModelError>) {
    let _ = app.emit_to(
        MAIN,
        SEMANTIC_MODEL_EVENT,
        ModelProgress {
            model: model().id.to_owned(),
            state,
            error,
        },
    );
}

/// Downloads the semantic model (user-initiated from Settings).
pub fn download(app: &AppHandle) -> Result<(), IpcError> {
    let semantic = managed(app).ok_or_else(|| failure("Not ready yet."))?;
    if !semantic.available() {
        return Err(failure(
            "Natural understanding isn't available on this computer.",
        ));
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut inner = semantic.locked()?;
        if inner.download.is_some() {
            return Err(failure("The model is already downloading."));
        }
        inner.download = Some(cancel.clone());
        inner.received = Some(0);
    }
    // A running engine may hold the old file open.
    deactivate(app);
    emit(app, ModelState::Downloading { received_bytes: 0 }, None);
    let app = app.clone();
    let spawned = thread::Builder::new()
        .name("sershi-semantic-download".to_owned())
        .spawn(move || {
            let Some(semantic) = managed(&app) else {
                return;
            };
            let model = model();
            let t0 = Instant::now();
            let mut last = Instant::now();
            let mut progress = |received: u64| {
                if last.elapsed() >= PROGRESS_INTERVAL || received == model.size_bytes {
                    last = Instant::now();
                    if let Ok(mut inner) = semantic.inner.lock() {
                        inner.received = Some(received);
                    }
                    emit(
                        &app,
                        ModelState::Downloading {
                            received_bytes: received,
                        },
                        None,
                    );
                }
            };
            let result = platform::download_file(
                &semantic.store,
                model,
                &model.url(),
                &mut progress,
                &cancel,
            );
            if let Ok(mut inner) = semantic.inner.lock() {
                inner.download = None;
                inner.received = None;
                if result.is_ok() {
                    // Verified while downloading.
                    inner.verified = true;
                    inner.corrupt = false;
                }
            }
            match result {
                Ok(()) => {
                    eprintln!(
                        "SERSHI semantic: model installed in {} s",
                        t0.elapsed().as_secs()
                    );
                    emit(&app, ModelState::Installed, None);
                    activate(&app);
                }
                Err(error) => {
                    eprintln!("SERSHI semantic: model download failed ({error:?})");
                    emit(&app, semantic.store.state(model), Some(error));
                }
            }
        });
    if spawned.is_err() {
        if let Ok(mut inner) = semantic.inner.lock() {
            inner.download = None;
            inner.received = None;
        }
        return Err(failure("The download couldn't start."));
    }
    Ok(())
}

pub fn cancel_download(app: &AppHandle) {
    if let Some(semantic) = managed(app)
        && let Ok(inner) = semantic.inner.lock()
        && let Some(cancel) = inner.download.as_ref()
    {
        cancel.store(true, Ordering::Relaxed);
    }
}

/// SERSHI is exiting: end the engine (the job object would too).
pub fn shutdown(app: &AppHandle) {
    cancel_download(app);
    if let Some(semantic) = managed(app)
        && let Ok(inner) = semantic.inner.lock()
        && let Some(generator) = inner.generator.as_ref()
    {
        generator.release();
    }
}
