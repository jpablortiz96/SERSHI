//! SERSHI's optional local language models: the Gate 3C semantic router
//! (docs/SEMANTIC.md) and the Prompt 4 Agent Brain (docs/AGENT_BRAIN.md).
//!
//! Both are data in `%LOCALAPPDATA%\dev.sershi.desktop\models\<role>`,
//! installed only when the user asks (free-space check, HTTPS, pinned
//! revision, exact size, SHA-256, atomic rename) and hashed again before
//! their first use in a session — a damaged or replaced file is never
//! loaded. Each runs in its own `sershi-semantic` engine process next to
//! SERSHI (crash isolation, full memory release), loaded lazily and ended
//! after [`IDLE_RELEASE`] without use.
//!
//! **One language model at a time.** Measured on a 6 GB GPU, the brain and
//! the semantic router loaded together slowed both about fourfold (video
//! memory contention): the brain's median went from 1.2 s to 4.6 s. So
//! while the Agent Brain is active it also covers short, imperfect
//! commands, and the semantic router stays on standby (not loaded); it
//! returns when the brain is turned off or removed (docs/AGENT_BRAIN.md).
//!
//! The models only interpret and decide; nothing they produce runs without
//! the core's validation, the executor, policy and — when required — the
//! trusted confirmation window. Without them, SERSHI keeps working
//! (deterministic understanding, then the semantic router if installed).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use sershi_core::brain::{LocalAgentBrain, unavailable};
use sershi_core::ipc::{IpcError, IpcErrorCode};
use sershi_core::understanding::models::{
    DEFAULT_BRAIN_MODEL, DEFAULT_SEMANTIC_MODEL, SemanticModel,
};
use sershi_core::understanding::semantic::LocalSemanticRouter;
use sershi_core::understanding::status::{SemanticModelInfo, SemanticSettings, SemanticStatus};
use sershi_core::voice::latency::Acceleration;
use sershi_core::voice::{ModelError, ModelProgress, ModelState};
use sershi_platform::semantic::{EngineConfig, EngineGenerator, IDLE_RELEASE};
use sershi_platform::voice::{self as platform, ModelStore};
use tauri::{AppHandle, Emitter, Manager};

use crate::runtime::Runtime;

/// Download progress (Command Center only).
pub const SEMANTIC_MODEL_EVENT: &str = "sershi://semantic-model";
pub const BRAIN_MODEL_EVENT: &str = "sershi://brain-model";
const MAIN: &str = "main";
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
/// How often idle engines are checked for release.
const IDLE_CHECK: Duration = Duration::from_secs(30);

/// Which local model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Gate 3C: short, imperfect commands.
    Semantic,
    /// Prompt 4: conversation, follow-ups and plans.
    Brain,
}

impl Role {
    fn model(self) -> &'static SemanticModel {
        match self {
            Self::Semantic => DEFAULT_SEMANTIC_MODEL,
            Self::Brain => DEFAULT_BRAIN_MODEL,
        }
    }

    fn event(self) -> &'static str {
        match self {
            Self::Semantic => SEMANTIC_MODEL_EVENT,
            Self::Brain => BRAIN_MODEL_EVENT,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::Brain => "brain",
        }
    }

    fn config(self, program: PathBuf, model: PathBuf, gpu: bool) -> EngineConfig {
        let mut config = EngineConfig::new(program, model, gpu);
        // Installer builds compile in the engine's hash (build.rs).
        config.program_sha256 = option_env!("SERSHI_ENGINE_SHA256");
        if self == Self::Brain {
            // Instructions + capabilities + context + a plan (measured
            // prompts stay under 2,500 tokens).
            config.context = 4096;
            config.request_timeout = Duration::from_secs(if gpu { 20 } else { 90 });
        }
        config
    }
}

struct Inner {
    enabled: bool,
    /// The installed file was hashed this session and matched.
    verified: bool,
    generator: Option<EngineGenerator>,
    download: Option<Arc<AtomicBool>>,
    received: Option<u64>,
    corrupt: bool,
}

pub struct LocalModel {
    role: Role,
    store: ModelStore,
    /// `sershi-semantic.exe` next to SERSHI (the engine for both roles).
    program: PathBuf,
    inner: Mutex<Inner>,
}

/// Tauri state is keyed by type: one newtype per role.
pub struct SemanticModelState(pub LocalModel);
pub struct BrainModelState(pub LocalModel);

fn failure(message: &str) -> IpcError {
    IpcError::new(IpcErrorCode::Unavailable, message)
}

impl LocalModel {
    pub fn new(role: Role, models_dir: PathBuf) -> Self {
        let program = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("sershi-semantic.exe")))
            .unwrap_or_default();
        let store = ModelStore::new(models_dir);
        store.remove_partials();
        Self {
            role,
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
            .map_err(|_| failure("The local model is unavailable."))
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
        self.store.state(self.role.model())
    }
}

fn managed(app: &AppHandle, role: Role) -> Option<&LocalModel> {
    match role {
        Role::Semantic => app.try_state::<SemanticModelState>().map(|s| &s.inner().0),
        Role::Brain => app.try_state::<BrainModelState>().map(|s| &s.inner().0),
    }
}

pub fn status(app: &AppHandle, role: Role) -> Result<SemanticStatus, IpcError> {
    let local = managed(app, role).ok_or_else(|| failure("Not ready yet."))?;
    let inner = local.locked()?;
    let engine = inner
        .generator
        .as_ref()
        .map(EngineGenerator::status)
        .unwrap_or_default();
    let gpu = engine
        .backend
        .as_deref()
        .map_or_else(LocalModel::gpu, |b| b != "cpu");
    let model = role.model();
    Ok(SemanticStatus {
        model: SemanticModelInfo::of(model, local.state(&inner), gpu),
        enabled: inner.enabled,
        available: local.available(),
        running: engine.running,
        backend: engine.backend,
        device: engine.device,
        load_ms: engine.load_ms,
        last_ms: engine.last_ms,
        free_space_ok: platform::has_room_for(local.store.dir(), model.size_bytes),
        standby: role == Role::Semantic && brain_active(app),
    })
}

pub fn configure(
    app: &AppHandle,
    role: Role,
    settings: SemanticSettings,
) -> Result<SemanticStatus, IpcError> {
    let local = managed(app, role).ok_or_else(|| failure("Not ready yet."))?;
    let changed = {
        let mut inner = local.locked()?;
        let changed = inner.enabled != settings.enabled;
        inner.enabled = settings.enabled;
        changed
    };
    if changed {
        if settings.enabled {
            activate(app, role);
        } else {
            deactivate(app, role);
        }
    }
    status(app, role)
}

/// At start-up: use installed models (after verifying them) and start the
/// idle release. Off the main thread.
pub fn setup(app: &AppHandle) {
    activate(app, Role::Semantic);
    activate(app, Role::Brain);
    let app = app.clone();
    let _ = thread::Builder::new()
        .name("sershi-models-idle".to_owned())
        .spawn(move || {
            loop {
                thread::sleep(IDLE_CHECK);
                for role in [Role::Semantic, Role::Brain] {
                    let Some(local) = managed(&app, role) else {
                        return;
                    };
                    let generator = local.inner.lock().ok().and_then(|i| i.generator.clone());
                    if generator.is_some_and(|g| g.release_if_idle(IDLE_RELEASE)) {
                        eprintln!("SERSHI {}: model released after inactivity", role.name());
                    }
                }
            }
        });
}

/// The Agent Brain is installed in the core right now.
fn brain_active(app: &AppHandle) -> bool {
    managed(app, Role::Brain)
        .and_then(|b| b.inner.lock().ok().map(|i| i.generator.is_some()))
        .unwrap_or(false)
}

/// Verifies an installed model and installs it into the core.
fn activate(app: &AppHandle, role: Role) {
    let app = app.clone();
    let _ = thread::Builder::new()
        .name(format!("sershi-{}-activate", role.name()))
        .spawn(move || {
            let Some(local) = managed(&app, role) else {
                return;
            };
            let model = role.model();
            let ready = local
                .inner
                .lock()
                .is_ok_and(|i| i.enabled && i.generator.is_none() && i.download.is_none());
            if !ready || !local.available() || local.store.state(model) != ModelState::Installed {
                return;
            }
            // One language model at a time: the brain covers what the
            // semantic router would do.
            if role == Role::Semantic && brain_active(&app) {
                return;
            }
            let verified = local.inner.lock().is_ok_and(|i| i.verified);
            if !verified {
                let t0 = Instant::now();
                if local.store.verify(model).ok() == Some(true) {
                    eprintln!(
                        "SERSHI {}: model verified in {} ms",
                        role.name(),
                        t0.elapsed().as_millis()
                    );
                } else {
                    // Never load a file that does not match: report it and
                    // offer a reinstall in Settings.
                    eprintln!(
                        "SERSHI {}: installed model failed verification",
                        role.name()
                    );
                    if let Ok(mut inner) = local.inner.lock() {
                        inner.corrupt = true;
                    }
                    emit(&app, role, ModelState::Corrupt, Some(ModelError::Integrity));
                    return;
                }
            }
            let generator = EngineGenerator::new(role.config(
                local.program.clone(),
                local.store.path(model),
                LocalModel::gpu(),
            ));
            {
                let Ok(mut inner) = local.inner.lock() else {
                    return;
                };
                if !inner.enabled {
                    return;
                }
                inner.verified = true;
                inner.corrupt = false;
                inner.generator = Some(generator.clone());
            }
            let runtime = app.state::<Runtime>();
            let _ = runtime.with_service(|s| match role {
                Role::Semantic => s.set_semantic_router(Some(Arc::new(LocalSemanticRouter::new(
                    generator,
                    model.template,
                )))),
                Role::Brain => s.set_agent_brain(
                    Some(Arc::new(LocalAgentBrain::new(generator, model.template))),
                    unavailable(&sershi_platform::capabilities()),
                ),
            });
            if role == Role::Brain {
                // The router goes on standby (its memory is released).
                release_engine(&app, Role::Semantic);
            }
            eprintln!("SERSHI {}: ready", role.name());
        });
}

/// Removes a model from the core and ends its engine, keeping the user's
/// choice (used for the semantic router's standby).
fn release_engine(app: &AppHandle, role: Role) {
    let runtime = app.state::<Runtime>();
    let _ = runtime.with_service(|s| match role {
        Role::Semantic => s.set_semantic_router(None),
        Role::Brain => s.set_agent_brain(None, Vec::new()),
    });
    if let Some(local) = managed(app, role) {
        let generator = local.inner.lock().ok().and_then(|mut i| i.generator.take());
        if let Some(generator) = generator {
            generator.release();
        }
    }
}

fn deactivate(app: &AppHandle, role: Role) {
    release_engine(app, role);
    if role == Role::Brain {
        // Without the brain, the semantic router returns (if installed
        // and enabled).
        activate(app, Role::Semantic);
    }
}

fn emit(app: &AppHandle, role: Role, state: ModelState, error: Option<ModelError>) {
    let _ = app.emit_to(
        MAIN,
        role.event(),
        ModelProgress {
            model: role.model().id.to_owned(),
            state,
            error,
        },
    );
}

/// Downloads a model (user-initiated from Settings). Refused up front when
/// the disk cannot hold it with a safety margin.
pub fn download(app: &AppHandle, role: Role) -> Result<(), IpcError> {
    let local = managed(app, role).ok_or_else(|| failure("Not ready yet."))?;
    if !local.available() {
        return Err(failure(
            "This local model isn't available on this computer.",
        ));
    }
    let model = role.model();
    if !platform::has_room_for(local.store.dir(), model.size_bytes) {
        emit(
            app,
            role,
            local.store.state(model),
            Some(ModelError::DiskFull),
        );
        return Err(failure(
            "There isn't enough free disk space for this model.",
        ));
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut inner = local.locked()?;
        if inner.download.is_some() {
            return Err(failure("The model is already downloading."));
        }
        inner.download = Some(cancel.clone());
        inner.received = Some(0);
    }
    // A running engine may hold the old file open.
    deactivate(app, role);
    emit(
        app,
        role,
        ModelState::Downloading { received_bytes: 0 },
        None,
    );
    let app = app.clone();
    let spawned = thread::Builder::new()
        .name(format!("sershi-{}-download", role.name()))
        .spawn(move || {
            let Some(local) = managed(&app, role) else {
                return;
            };
            let t0 = Instant::now();
            let mut last = Instant::now();
            let mut progress = |received: u64| {
                if last.elapsed() >= PROGRESS_INTERVAL || received == model.size_bytes {
                    last = Instant::now();
                    if let Ok(mut inner) = local.inner.lock() {
                        inner.received = Some(received);
                    }
                    emit(
                        &app,
                        role,
                        ModelState::Downloading {
                            received_bytes: received,
                        },
                        None,
                    );
                }
            };
            let result =
                platform::download_file(&local.store, model, &model.url(), &mut progress, &cancel);
            if let Ok(mut inner) = local.inner.lock() {
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
                        "SERSHI {}: model installed in {} s",
                        role.name(),
                        t0.elapsed().as_secs()
                    );
                    emit(&app, role, ModelState::Installed, None);
                    activate(&app, role);
                }
                Err(error) => {
                    eprintln!("SERSHI {}: model download failed ({error:?})", role.name());
                    emit(&app, role, local.store.state(model), Some(error));
                }
            }
        });
    if spawned.is_err() {
        if let Ok(mut inner) = local.inner.lock() {
            inner.download = None;
            inner.received = None;
        }
        return Err(failure("The download couldn't start."));
    }
    Ok(())
}

pub fn cancel_download(app: &AppHandle, role: Role) {
    if let Some(local) = managed(app, role)
        && let Ok(inner) = local.inner.lock()
        && let Some(cancel) = inner.download.as_ref()
    {
        cancel.store(true, Ordering::Relaxed);
    }
}

/// SERSHI is exiting: end the engines (the job object would too).
pub fn shutdown(app: &AppHandle) {
    for role in [Role::Semantic, Role::Brain] {
        cancel_download(app, role);
        if let Some(local) = managed(app, role)
            && let Ok(inner) = local.inner.lock()
            && let Some(generator) = inner.generator.as_ref()
        {
            generator.release();
        }
    }
}
