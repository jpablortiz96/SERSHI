//! Voice orchestration: push-to-talk capture, local recognition, spoken
//! replies and model downloads.
//!
//! The shell only wires adapters to the core. Every decision about *what
//! happens* stays in `sershi-core`: the state machine says when the
//! microphone may open, and a recognised transcript is submitted through
//! `AssistantService::submit_transcript`, which is the typed-command path.
//! Nothing here can approve, read a confirmation id or grant a permission.
//!
//! Latency (Gate 3B): recognition runs on the GPU when available; the end
//! of speech is detected adaptively; an early (speculative) decode starts
//! as soon as the user pauses and is used only if nothing was said after
//! it; engines stay warm between commands. Early or partial text is never
//! executed — only the final transcript, after capture has ended, is
//! submitted.
//!
//! Threads and locks: audio callbacks only send over channels; one worker
//! thread owns each capture session (and the microphone handle); playback
//! runs on its own thread. No lock is held while joining an audio thread or
//! calling into the service.
//!
//! Voice session (Gate 4.1): after an explicit start, SERSHI listens again
//! after each reply without a click, until the user says goodbye, stops it,
//! or stays silent for `IDLE_TIMEOUT_MS`. The core's
//! [`VoiceSession`] decides the turn-taking; this module only opens and
//! closes the microphone accordingly. Half duplex: the microphone is never
//! open while SERSHI speaks or while a trusted confirmation is pending.
//!
//! Privacy: audio lives in memory for the length of one utterance and is
//! dropped after recognition. Nothing is written to disk or logs; the UI
//! receives only the transcript to show, a bounded 0–1 level and timings.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use sershi_core::activity::{ActivityKind, NewActivity};
use sershi_core::assistant::AssistantState;
use sershi_core::ipc::{IpcError, IpcErrorCode};
use sershi_core::service::{CommandOutcome, ServiceEvent};
use sershi_core::voice::endpoint::{Endpoint, EndpointConfig, EndpointDetector};
use sershi_core::voice::latency::{
    Acceleration, SPECULATE_AFTER_MS, VoiceTimings, release_due, speculation_usable,
    vocabulary_context,
};
use sershi_core::voice::ports::{
    ActiveCapture, ActivePlayback, CaptureError, CaptureFormat, CaptureSink, PlaybackEnd,
    PlaybackSink, SpeechToTextPort, SttError, TranscribeOptions, VoiceChoice,
};
use sershi_core::voice::session::{
    Heard, IDLE_TIMEOUT_MS, Next, REPLY_GRACE_MS, SessionEndReason, VoiceSession,
    VoiceSessionStatus, recognition_deadline_ms, speech_offset,
};
use sershi_core::voice::signal::{LevelMeter, RECOGNITION_RATE, downmix, resample};
use sershi_core::voice::stabilize::{self, LanguageContext, RecentLanguages};
use sershi_core::voice::transcript::{self, Transcript, Verdict};
use sershi_core::voice::{
    CaptureStart, LanguageTag, LevelSource, MicrophoneStatus, ModelError, ModelInfo, ModelProgress,
    ModelState, SpeechProfile, SttModel, VoiceFailure, VoiceLevel, VoiceSettings, VoiceStatus,
    VoiceUpdate,
};
use sershi_platform::voice::{self as platform, ModelStore, VoicePlatform};
use tauri::{AppHandle, Emitter, Manager};

use crate::confirmation;
use crate::integration;
use crate::runtime::{Runtime, broadcast, drive, schedule_settle};
use crate::surfaces::{COMPANION, MAIN};

/// Voice events for the Command Center (`VoiceUpdate`).
pub const VOICE_EVENT: &str = "sershi://voice";
/// Bounded audio level for visuals (`VoiceLevel`), Command Center and companion.
pub const LEVEL_EVENT: &str = "sershi://voice-level";
/// Model download progress (`ModelProgress`), Command Center only.
pub const MODEL_EVENT: &str = "sershi://voice-model";
/// The voice session's status (`VoiceSessionStatus`), Command Center and
/// companion (the "session active" indicator).
pub const SESSION_EVENT: &str = "sershi://voice-session";

/// After SERSHI stops talking, wait this long before the microphone opens
/// again, so the end of its own voice (and the room's echo) is never heard.
const ECHO_GUARD: Duration = Duration::from_millis(350);

/// At most 25 level updates per second.
const LEVEL_INTERVAL: Duration = Duration::from_millis(40);
/// Longest reply text accepted for speech.
const MAX_SPEECH_CHARS: usize = 600;
/// Throttle for download progress events.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

enum Msg {
    Audio(Vec<f32>),
    CaptureFailed(CaptureError),
    Stop,
    Cancel,
}

/// What ended a capture session.
enum Finish {
    Transcribe,
    NoSpeech,
    Silent,
    Cancel,
    Failed(VoiceFailure),
}

#[derive(Default)]
struct Inner {
    settings: VoiceSettings,
    /// Control channel of the running capture session.
    capture: Option<(u64, Sender<Msg>)>,
    /// Cancel flag of the recognition in progress (after capture).
    recognition: Option<Arc<AtomicBool>>,
    playback: Option<(u64, Box<dyn ActivePlayback>)>,
    next_id: u64,
    /// Model download in progress: (model id, cancel flag).
    download: Option<(String, Arc<AtomicBool>)>,
    /// Transient model states (downloading) that the store cannot know.
    model_states: HashMap<String, ModelState>,
    /// The chosen microphone was missing at the last capture.
    fallback: bool,
    /// When the last spoken command's outcome arrived (speech latency).
    answered_at: Option<Instant>,
    /// Incremented whenever speech is stopped (a new request, the
    /// microphone, the Stop button). A reply still being synthesized when
    /// that happens is dropped instead of starting late over the next
    /// interaction (Gate 3C busy-state audit).
    speech_epoch: u64,
    /// Recently, confidently detected conversation languages (Gate 3C.1;
    /// memory only, bounded, a hint for language retries).
    recent_languages: RecentLanguages,
    /// The hands-free voice session, while one is active (Gate 4.1).
    session: Option<VoiceSession>,
    session_seq: u64,
    /// A finished turn waiting for its reply to end before listening again.
    awaiting: Option<u64>,
    resume_seq: u64,
    /// A reply is being synthesized (it will play shortly).
    synthesizing: bool,
}

/// Loaded recognition engines, kept warm between commands.
#[derive(Default)]
struct Engines {
    loaded: HashMap<&'static str, Arc<dyn SpeechToTextPort>>,
    last_used: Option<Instant>,
}

pub struct Voice {
    platform: VoicePlatform,
    store: ModelStore,
    inner: Mutex<Inner>,
    engines: Arc<Mutex<Engines>>,
    acceleration: OnceLock<(Acceleration, Option<String>)>,
}

impl Voice {
    pub fn new(models_dir: PathBuf) -> Self {
        let store = ModelStore::new(models_dir);
        // An interrupted download from a previous run leaves nothing usable.
        store.remove_partials();
        let engines = Arc::new(Mutex::new(Engines::default()));
        release_when_idle(engines.clone());
        Self {
            platform: platform::voice_platform(),
            store,
            inner: Mutex::new(Inner::default()),
            engines,
            acceleration: OnceLock::new(),
        }
    }

    fn locked(&self) -> Result<MutexGuard<'_, Inner>, IpcError> {
        self.inner
            .lock()
            .map_err(|_| IpcError::new(IpcErrorCode::Internal, "Voice is unavailable."))
    }

    /// The accelerator speech recognition uses (probed once).
    fn acceleration(&self) -> (Acceleration, Option<String>) {
        if !self.platform.supported {
            return (Acceleration::Cpu, None);
        }
        self.acceleration
            .get_or_init(platform::speech_acceleration)
            .clone()
    }
}

/// Releases the models' memory after a long idle period (a warm engine is
/// hundreds of MB of RAM or video memory).
fn release_when_idle(engines: Arc<Mutex<Engines>>) {
    let _ = thread::Builder::new()
        .name("sershi-voice-idle".to_owned())
        .spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(60));
                if let Ok(mut e) = engines.lock()
                    && release_due(!e.loaded.is_empty(), e.last_used.map(|t| t.elapsed()))
                {
                    e.loaded.clear();
                    eprintln!("SERSHI voice: speech models released after inactivity");
                }
            }
        });
}

fn managed(app: &AppHandle) -> Option<tauri::State<'_, Voice>> {
    app.try_state::<Voice>()
}

fn failure(reason: VoiceFailure) -> IpcError {
    IpcError::new(IpcErrorCode::Unavailable, format!("voice:{reason:?}"))
}

fn emit_update(app: &AppHandle, update: VoiceUpdate) {
    let _ = app.emit_to(MAIN, VOICE_EVENT, update);
}

fn emit_level(app: &AppHandle, level: f32, source: LevelSource) {
    let payload = VoiceLevel {
        level: level.clamp(0.0, 1.0),
        source,
    };
    let _ = app.emit_to(MAIN, LEVEL_EVENT, payload);
    let _ = app.emit_to(COMPANION, LEVEL_EVENT, payload);
}

fn millis(since: Instant) -> u32 {
    u32::try_from(since.elapsed().as_millis()).unwrap_or(u32::MAX)
}

fn capture_failure(error: &CaptureError) -> VoiceFailure {
    match error {
        CaptureError::NoDevice | CaptureError::DeviceNotFound => VoiceFailure::NoMicrophone,
        CaptureError::PermissionDenied => VoiceFailure::PermissionDenied,
        CaptureError::Unsupported => VoiceFailure::Unsupported,
        CaptureError::DeviceUnavailable
        | CaptureError::UnsupportedFormat
        | CaptureError::Failed(_) => VoiceFailure::DeviceLost,
    }
}

// ── Status and settings ───────────────────────────────────────────────────

fn model_state(voice: &Voice, inner: &Inner, model: &SttModel) -> ModelState {
    inner
        .model_states
        .get(model.id)
        .cloned()
        .unwrap_or_else(|| voice.store.state(model))
}

pub fn status(app: &AppHandle) -> Result<VoiceStatus, IpcError> {
    let voice = managed(app).ok_or_else(|| failure(VoiceFailure::Unsupported))?;
    // Device, voice and accelerator enumeration happen outside the lock.
    let devices = if voice.platform.supported {
        voice.platform.capture.input_devices().unwrap_or_default()
    } else {
        Vec::new()
    };
    let voices = voice.platform.synthesis.voices().unwrap_or_default();
    let access = platform::microphone_access();
    let (acceleration, accelerator) = voice.acceleration();
    let inner = voice.locked()?;
    let fallback = inner
        .settings
        .microphone
        .as_ref()
        .is_some_and(|id| !devices.iter().any(|d| &d.id == id))
        || inner.fallback;
    Ok(VoiceStatus {
        supported: voice.platform.supported,
        microphone: MicrophoneStatus {
            access,
            devices,
            fallback,
        },
        models: ModelInfo::catalog(|m| model_state(&voice, &inner, m)),
        profile: inner.settings.profile(),
        model: inner.settings.model().id.to_owned(),
        acceleration,
        accelerator,
        voices,
        capturing: inner.capture.is_some(),
        speaking: inner.playback.is_some(),
    })
}

pub fn configure(app: &AppHandle, settings: VoiceSettings) -> Result<VoiceStatus, IpcError> {
    if !settings.is_valid() {
        return Err(IpcError::new(
            IpcErrorCode::NotAllowed,
            "Invalid voice settings.",
        ));
    }
    let voice = managed(app).ok_or_else(|| failure(VoiceFailure::Unsupported))?;
    let model = settings.model();
    {
        let mut inner = voice.locked()?;
        if inner.settings.microphone != settings.microphone {
            inner.fallback = false;
        }
        inner.settings = settings;
    }
    // Replies (and the Agent Brain's messages) follow the interface
    // language, never what recognition detected.
    if let Some(tag) = voice
        .locked()
        .ok()
        .and_then(|i| i.settings.interface_language.clone())
    {
        let runtime = app.state::<Runtime>();
        let _ = runtime.with_service(|s| s.set_response_language(tag.as_str()));
    }
    // Keep only the chosen profile's engine (the other one's memory is
    // released; a Fast engine may be re-used for its second pass later).
    if let Ok(mut e) = voice.engines.lock() {
        e.loaded.retain(|id, _| *id == model.id);
    }
    status(app)
}

// ── Capture ───────────────────────────────────────────────────────────────

/// Forwards audio from the capture thread to the session worker.
struct ChannelSink(Sender<Msg>);

impl CaptureSink for ChannelSink {
    fn audio(&mut self, interleaved: &[f32]) {
        let _ = self.0.send(Msg::Audio(interleaved.to_vec()));
    }
    fn failed(&mut self, error: CaptureError) {
        let _ = self.0.send(Msg::CaptureFailed(error));
    }
}

fn refused(reason: VoiceFailure) -> Result<CaptureStart, IpcError> {
    Ok(CaptureStart::Refused { reason })
}

/// Who opened the microphone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opener {
    /// The user pressed the microphone (or Interrupt).
    User,
    /// The voice session listening again after a turn.
    Session,
}

/// Push-to-talk (the microphone button). In a voice session it is also the
/// barge-in: SERSHI stops talking or working and listens.
pub fn start_capture(app: &AppHandle) -> Result<CaptureStart, IpcError> {
    open_microphone(app, Opener::User)
}

/// Opens the microphone only after the core agrees (which also cancels any
/// pending approval) and reports failures honestly.
fn open_microphone(app: &AppHandle, opener: Opener) -> Result<CaptureStart, IpcError> {
    let Some(voice) = managed(app) else {
        return refused(VoiceFailure::Unsupported);
    };
    if !voice.platform.supported {
        return refused(VoiceFailure::Unsupported);
    }
    let (model, device) = {
        let inner = voice.locked()?;
        if inner.capture.is_some() {
            return refused(VoiceFailure::Busy);
        }
        (inner.settings.model(), inner.settings.microphone.clone())
    };
    match voice.store.state(model) {
        ModelState::Installed => {}
        ModelState::Corrupt => return refused(VoiceFailure::ModelCorrupt),
        _ => return refused(VoiceFailure::ModelMissing),
    }
    if opener == Opener::User {
        // The user takes the turn: nothing scheduled may reopen the
        // microphone behind them. Talking over SERSHI is a barge-in.
        let interrupting = {
            let mut inner = voice.locked()?;
            inner.awaiting = None;
            inner.playback.is_some() || inner.synthesizing
        };
        let working = app
            .state::<Runtime>()
            .with_service(|s| s.snapshot().state.is_busy())
            .unwrap_or(false);
        if interrupting || working {
            with_session(app, VoiceSession::barge_in);
        }
    }
    // Half duplex: SERSHI stops talking before it listens, so its own
    // voice never reaches the recogniser.
    stop_speaking(app);

    let runtime = app.state::<Runtime>();
    let began = runtime.with_service(|s| {
        let began = s.begin_listening(&mut |e| broadcast(app, e));
        if began.is_ok() {
            // Load the semantic model (if installed) while the user speaks.
            s.prepare_understanding();
        }
        began
    })?;
    let Ok(cancelled) = began else {
        return refused(VoiceFailure::Busy);
    };
    if cancelled.is_some() {
        confirmation::cancelled(app, cancelled);
    }

    let (tx, rx) = mpsc::channel();
    let opened = voice
        .platform
        .capture
        .start(device.as_deref(), Box::new(ChannelSink(tx.clone())));
    let opened = match opened {
        Err(CaptureError::DeviceNotFound) if device.is_some() => {
            // The chosen microphone is gone: use the system default and say so.
            if let Ok(mut inner) = voice.locked() {
                inner.fallback = true;
            }
            emit_update(app, VoiceUpdate::DeviceFallback);
            voice
                .platform
                .capture
                .start(None, Box::new(ChannelSink(tx.clone())))
        }
        other => other,
    };
    let (format, handle) = match opened {
        Ok(opened) => opened,
        Err(error) => {
            let reason = capture_failure(&error);
            eprintln!("SERSHI voice: capture failed ({reason:?})");
            end_voice(app, Some(0), true);
            return refused(reason);
        }
    };

    let id = {
        let mut inner = voice.locked()?;
        inner.next_id += 1;
        let id = inner.next_id;
        inner.capture = Some((id, tx));
        id
    };
    integration::set_listening(app, true);
    with_session(app, VoiceSession::listening);
    // Load (or keep) the engine while the user is still speaking.
    warm_up(app, model);
    let app = app.clone();
    let spawned = thread::Builder::new()
        .name("sershi-voice".to_owned())
        .spawn(move || session(&app, id, rx, handle, format));
    if spawned.is_err() {
        return Err(IpcError::new(
            IpcErrorCode::Internal,
            "Voice is unavailable.",
        ));
    }
    Ok(CaptureStart::Started)
}

/// Sends a control message to the running session, if any.
fn control(app: &AppHandle, msg: Msg) -> bool {
    let Some(voice) = managed(app) else {
        return false;
    };
    let sender = voice
        .locked()
        .ok()
        .and_then(|i| i.capture.as_ref().map(|(_, tx)| tx.clone()));
    sender.is_some_and(|tx| tx.send(msg).is_ok())
}

/// The user clicked the microphone again: stop and transcribe.
pub fn stop_capture(app: &AppHandle) {
    control(app, Msg::Stop);
}

/// Escape, hiding the Command Center, a typed command or quitting.
pub fn cancel_capture(app: &AppHandle) {
    control(app, Msg::Cancel);
}

/// Escape or hiding the Command Center: stop listening, and drop an
/// utterance still being recognised so nothing runs after the user
/// dismissed it (the recognition itself is aborted too). While listening,
/// the session records the microphone duration itself.
pub fn cancel(app: &AppHandle) {
    end_session(app, SessionEndReason::Dismissed);
    cancel_capture(app);
    if let Some(voice) = managed(app)
        && let Ok(inner) = voice.locked()
        && let Some(flag) = inner.recognition.as_ref()
    {
        flag.store(true, Ordering::Relaxed);
    }
    let runtime = app.state::<Runtime>();
    let dropped = runtime
        .with_service(|s| {
            s.snapshot().state == AssistantState::Transcribing
                && s.cancel_voice(None, &mut |e| broadcast(app, e))
        })
        .unwrap_or(false);
    if dropped {
        emit_update(app, VoiceUpdate::Cancelled);
    }
}

/// Returns the assistant from a voice state (mic-off is recorded by the core).
fn end_voice(app: &AppHandle, mic_ms: Option<u32>, failed: bool) {
    let runtime = app.state::<Runtime>();
    if let Ok(snapshot) = runtime.with_service(|s| {
        if failed {
            s.voice_failed(mic_ms, &mut |e| broadcast(app, e));
        } else {
            s.cancel_voice(mic_ms, &mut |e| broadcast(app, e));
        }
        s.snapshot()
    }) {
        schedule_settle(app, &snapshot);
    }
}

// ── Recognition ───────────────────────────────────────────────────────────

/// One recognition run and what it cost.
struct Decoded {
    result: Result<Transcript, SttError>,
    stt_ms: u32,
    /// Verification + load, if this run paid for it.
    load_ms: Option<u32>,
    acceleration: Option<Acceleration>,
}

/// Recognition context: well-known installed application names (from the
/// trusted catalog), so Whisper spells them right. Never authority.
fn vocabulary(app: &AppHandle) -> Option<String> {
    let names: Vec<String> = app
        .state::<Runtime>()
        .apps
        .applications()
        .into_iter()
        .map(|a| a.display_name)
        .collect();
    vocabulary_context(names.iter().map(String::as_str))
}

fn options(app: &AppHandle, voice: &Voice, cancel: Arc<AtomicBool>) -> TranscribeOptions {
    TranscribeOptions {
        language: voice
            .locked()
            .ok()
            .and_then(|i| i.settings.language.clone()),
        context: vocabulary(app),
        cancel: Some(cancel),
    }
}

/// The warm engine for `model`, verifying and loading it if needed. Holds
/// the engine cache while loading, so concurrent callers load once.
fn engine(
    voice: &Voice,
    model: &'static SttModel,
) -> Result<(Arc<dyn SpeechToTextPort>, Option<u32>), SttError> {
    let mut engines = voice
        .engines
        .lock()
        .map_err(|_| SttError::Failed("engine cache".to_owned()))?;
    engines.last_used = Some(Instant::now());
    if let Some(engine) = engines.loaded.get(model.id) {
        return Ok((engine.clone(), None));
    }
    let t0 = Instant::now();
    // Full integrity check before the first load in this session.
    match voice.store.verify(model) {
        Ok(true) => {}
        Ok(false) => return Err(SttError::ModelCorrupt),
        Err(_) => return Err(SttError::ModelMissing),
    }
    let verify_ms = millis(t0);
    let (acceleration, _) = voice.acceleration();
    let engine = platform::load_recognizer(&voice.store.path(model), acceleration)?;
    let load_ms = millis(t0);
    eprintln!(
        "SERSHI voice: model {} verified in {verify_ms} ms, loaded on {:?} in {load_ms} ms total",
        model.id,
        engine.acceleration()
    );
    engines.loaded.insert(model.id, engine.clone());
    Ok((engine, Some(load_ms)))
}

/// Starts loading the engine in the background (while the user speaks, or
/// after a model was installed).
fn warm_up(app: &AppHandle, model: &'static SttModel) {
    let app = app.clone();
    let _ = thread::Builder::new()
        .name("sershi-voice-load".to_owned())
        .spawn(move || {
            if let Some(voice) = managed(&app) {
                let _ = engine(&voice, model);
            }
        });
}

/// Resamples a device-rate utterance and recognises it.
fn decode(
    voice: &Voice,
    model: &'static SttModel,
    audio: &[f32],
    rate: u32,
    options: &TranscribeOptions,
) -> Decoded {
    let pcm = resample(audio, rate, RECOGNITION_RATE);
    match engine(voice, model) {
        Err(error) => Decoded {
            result: Err(error),
            stt_ms: 0,
            load_ms: None,
            acceleration: None,
        },
        Ok((engine, load_ms)) => {
            let t0 = Instant::now();
            let result = engine.transcribe(&pcm, options);
            Decoded {
                result,
                stt_ms: millis(t0),
                load_ms,
                acceleration: Some(engine.acceleration()),
            }
        }
    }
}

/// An early decode started when the user paused.
struct Speculation {
    /// Samples it covers (device rate).
    samples: usize,
    cancel: Arc<AtomicBool>,
    result: Receiver<Decoded>,
}

impl Speculation {
    fn abandon(self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn speculate(app: &AppHandle, audio: &[f32], rate: u32, offset: usize) -> Option<Speculation> {
    let voice = managed(app)?;
    let model = voice.locked().ok()?.settings.model();
    let cancel = Arc::new(AtomicBool::new(false));
    let options = options(app, &voice, cancel.clone());
    // Covers everything captured (`samples`), decodes from the speech on.
    let snapshot = audio[offset.min(audio.len())..].to_vec();
    let (tx, rx) = mpsc::channel();
    let app = app.clone();
    thread::Builder::new()
        .name("sershi-voice-early".to_owned())
        .spawn(move || {
            if let Some(voice) = managed(&app) {
                let _ = tx.send(decode(&voice, model, &snapshot, rate, &options));
            }
        })
        .ok()?;
    Some(Speculation {
        samples: audio.len(),
        cancel,
        result: rx,
    })
}

/// Times of one capture, for the latency record.
struct Timeline {
    speech_started_ms: Option<u32>,
    speech_ended_ms: Option<u32>,
    /// Capture length when it stopped.
    capture_ms: u32,
    stopped: Instant,
}

/// One push-to-talk session: owns the microphone handle, gathers audio,
/// detects the end of speech (overlapping an early decode with the final
/// silence) and hands off to recognition.
fn session(
    app: &AppHandle,
    id: u64,
    rx: Receiver<Msg>,
    handle: Box<dyn ActiveCapture>,
    format: CaptureFormat,
) {
    let started = Instant::now();
    let endpoint_override =
        managed(app).and_then(|v| v.locked().ok().and_then(|i| i.settings.endpoint_ms));
    let mut config = match endpoint_override {
        Some(ms) => EndpointConfig::default().with_silence(ms),
        None => EndpointConfig::default(),
    };
    // In a voice session the microphone waits longer for the next request;
    // silence that long ends the session (Gate 4.1).
    let session_turn = session_active(app);
    if session_turn {
        config.no_speech_timeout_ms = IDLE_TIMEOUT_MS;
        // The user may start speaking late in the wait (Gate 4.1.1).
        config.max_capture_ms = IDLE_TIMEOUT_MS + 30_000;
    }
    let mut detector = EndpointDetector::new(format.sample_rate, config);
    let max_samples =
        (u64::from(format.sample_rate) * u64::from(config.max_capture_ms) / 1000) as usize;
    let mut audio: Vec<f32> = Vec::with_capacity(format.sample_rate as usize * 5);
    let mut meter = LevelMeter::new();
    let mut last_level = Instant::now();
    let mut mono = Vec::new();
    let mut speculation: Option<Speculation> = None;

    let finish = loop {
        let msg = match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(msg) => msg,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Hard cap even if the device stops delivering audio.
                if started.elapsed()
                    > Duration::from_millis(u64::from(config.max_capture_ms) + 2_000)
                {
                    break Finish::Transcribe;
                }
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break Finish::Cancel,
        };
        match msg {
            Msg::Audio(chunk) => {
                mono.clear();
                downmix(&chunk, format.channels, &mut mono);
                let room = max_samples.saturating_sub(audio.len());
                audio.extend_from_slice(&mono[..mono.len().min(room)]);
                let level = meter.push(&mono);
                if last_level.elapsed() >= LEVEL_INTERVAL {
                    last_level = Instant::now();
                    emit_level(app, level, LevelSource::Input);
                }
                let endpoint = detector.push(&mono);
                // Speech resumed after an early decode: it no longer covers
                // everything that was said.
                if speculation
                    .as_ref()
                    .is_some_and(|s| !speculation_usable(s.samples, detector.last_speech_sample()))
                    && let Some(stale) = speculation.take()
                {
                    stale.abandon();
                }
                if speculation.is_none()
                    && detector.pause_ms().is_some_and(|p| p >= SPECULATE_AFTER_MS)
                {
                    let offset = speech_offset(
                        detector.speech_started_ms(),
                        format.sample_rate,
                        audio.len(),
                    );
                    speculation = speculate(app, &audio, format.sample_rate, offset);
                }
                match endpoint {
                    Endpoint::Continue => {}
                    Endpoint::SpeechEnded | Endpoint::MaxDuration => break Finish::Transcribe,
                    Endpoint::NoSpeech if detector.digital_silence() => break Finish::Silent,
                    Endpoint::NoSpeech => break Finish::NoSpeech,
                }
            }
            Msg::CaptureFailed(error) => break Finish::Failed(capture_failure(&error)),
            Msg::Stop => {
                break if detector.digital_silence() && detector.elapsed_ms() >= 500 {
                    Finish::Silent
                } else if detector.heard_speech() {
                    Finish::Transcribe
                } else {
                    Finish::NoSpeech
                };
            }
            Msg::Cancel => break Finish::Cancel,
        }
    };

    // Release the microphone before anything else.
    handle.stop();
    let mic_ms = millis(started);
    let timeline = Timeline {
        speech_started_ms: detector.speech_started_ms(),
        speech_ended_ms: detector.speech_ended_ms(),
        capture_ms: detector.elapsed_ms(),
        stopped: Instant::now(),
    };
    eprintln!("SERSHI voice: capture stopped after {mic_ms} ms");
    // The core records "microphone off" in the transition below; only then
    // is the session released (a typed command waits for this).
    let release = || {
        if let Some(voice) = managed(app)
            && let Ok(mut inner) = voice.locked()
            && inner.capture.as_ref().is_some_and(|(cid, _)| *cid == id)
        {
            inner.capture = None;
        }
        integration::set_listening(app, false);
        emit_level(app, 0.0, LevelSource::Input);
    };
    let usable = speculation.filter(|s| {
        let ok = speculation_usable(s.samples, detector.last_speech_sample());
        if !ok {
            s.cancel.store(true, Ordering::Relaxed);
        }
        ok
    });

    match finish {
        Finish::Cancel => {
            if let Some(s) = usable {
                s.abandon();
            }
            end_voice(app, Some(mic_ms), false);
            release();
            emit_update(app, VoiceUpdate::Cancelled);
        }
        Finish::Failed(reason) => {
            if let Some(s) = usable {
                s.abandon();
            }
            end_session(app, SessionEndReason::Failed);
            end_voice(app, Some(mic_ms), true);
            release();
            emit_update(app, VoiceUpdate::Failed { reason });
        }
        Finish::Silent => {
            if let Some(s) = usable {
                s.abandon();
            }
            end_session(app, SessionEndReason::Failed);
            end_voice(app, Some(mic_ms), true);
            release();
            emit_update(
                app,
                VoiceUpdate::Failed {
                    reason: VoiceFailure::MicrophoneSilent,
                },
            );
        }
        Finish::NoSpeech if session_turn && session_active(app) => {
            // Nobody spoke for the session's idle timeout: the session ends
            // and the microphone closes. Nothing is said or run.
            if let Some(s) = usable {
                s.abandon();
            }
            end_session(app, SessionEndReason::Timeout);
            end_voice(app, Some(mic_ms), false);
            release();
            emit_update(app, VoiceUpdate::Cancelled);
        }
        Finish::NoSpeech => {
            if let Some(s) = usable {
                s.abandon();
            }
            let transcribing = begin_transcribing(app, mic_ms);
            release();
            if transcribing {
                unusable(app, VoiceUpdate::NoSpeech);
            }
        }
        Finish::Transcribe => {
            let transcribing = begin_transcribing(app, mic_ms);
            release();
            drop(mono);
            if transcribing {
                // Never transcribe the silence before speech (a pre-roll
                // keeps the first syllable).
                let offset =
                    speech_offset(timeline.speech_started_ms, format.sample_rate, audio.len());
                audio.drain(..offset);
                recognise(app, audio, format.sample_rate, usable, &timeline);
            } else if let Some(s) = usable {
                s.abandon();
            }
        }
    }
}

fn begin_transcribing(app: &AppHandle, mic_ms: u32) -> bool {
    with_session(app, VoiceSession::transcribing);
    let runtime = app.state::<Runtime>();
    runtime
        .with_service(|s| s.end_listening(mic_ms, &mut |e| broadcast(app, e)))
        .unwrap_or(false)
}

/// Nothing usable was heard: Warning, and nothing runs.
fn unusable(app: &AppHandle, update: VoiceUpdate) {
    let runtime = app.state::<Runtime>();
    if let Ok(snapshot) = runtime.with_service(|s| {
        s.voice_unusable(&mut |e| broadcast(app, e));
        s.snapshot()
    }) {
        schedule_settle(app, &snapshot);
    }
    emit_update(app, update);
}

fn stt_failure(error: &SttError) -> VoiceFailure {
    match error {
        SttError::ModelMissing => VoiceFailure::ModelMissing,
        SttError::ModelCorrupt => VoiceFailure::ModelCorrupt,
        SttError::Unsupported => VoiceFailure::Unsupported,
        SttError::Cancelled | SttError::UnsupportedLanguage | SttError::Failed(_) => {
            VoiceFailure::RecognitionFailed
        }
    }
}

/// A language retry and what it found (Gate 3C.1, docs/VOICE.md).
struct LanguageRetry {
    first_language: Option<LanguageTag>,
    /// What the first pass heard (shown next to an accepted retry).
    first_text: String,
    ms: u32,
    /// The second pass, when it replaced the first.
    accepted: Option<Transcript>,
}

/// One second pass in the conversation's language for a short, unresolved
/// first result in an unexpected language ([`stabilize::plan_retry`]). The
/// result is only text: it is submitted like any transcript, through
/// understanding and policy.
fn retry_language(
    voice: &Voice,
    model: &'static SttModel,
    audio: &[f32],
    rate: u32,
    options: &TranscribeOptions,
    first: &Transcript,
    language: LanguageTag,
) -> LanguageRetry {
    let t0 = Instant::now();
    let mut retry = LanguageRetry {
        first_language: first.language.clone(),
        first_text: transcript::clean(&first.text),
        ms: 0,
        accepted: None,
    };
    let mut forced = options.clone();
    forced.language = Some(language);
    let second = decode(voice, model, audio, rate, &forced);
    if let Ok(second) = second.result
        && stabilize::accept_retry(&second).is_some()
    {
        retry.accepted = Some(second);
    }
    retry.ms = millis(t0);
    retry
}

/// Recognises the final utterance (re-using an early decode that covered
/// it), then submits it exactly like typed text.
fn recognise(
    app: &AppHandle,
    audio: Vec<f32>,
    rate: u32,
    speculation: Option<Speculation>,
    timeline: &Timeline,
) {
    let Some(voice) = managed(app) else {
        return;
    };
    let (profile, model) = match voice.locked() {
        Ok(inner) => (inner.settings.profile(), inner.settings.model()),
        Err(_) => return,
    };
    let cancel = Arc::new(AtomicBool::new(false));
    if let Ok(mut inner) = voice.locked() {
        inner.recognition = Some(cancel.clone());
    }
    let opts = options(app, &voice, cancel.clone());
    let detect = opts.language.is_none();
    // A cold model (first turn after start or after idle release): say
    // "Preparing voice…" instead of pretending to transcribe (Gate 4.1.1).
    let warm = voice
        .engines
        .try_lock()
        .is_ok_and(|e| e.loaded.contains_key(model.id));
    if !warm {
        emit_update(app, VoiceUpdate::Preparing);
        with_session(app, VoiceSession::preparing);
    }
    // Watchdog (Gate 4.1.1): recognition never leaves SERSHI stuck in
    // "Transcribing". Past the deadline the decode is cancelled and the
    // turn recovers (a voice session listens again).
    let finished = Arc::new(AtomicBool::new(false));
    let timed_out = Arc::new(AtomicBool::new(false));
    {
        let gpu = voice.acceleration().0 == Acceleration::Vulkan;
        let deadline = Duration::from_millis(u64::from(recognition_deadline_ms(gpu, !warm)));
        let (finished, timed_out, cancel) = (finished.clone(), timed_out.clone(), cancel.clone());
        let _ = thread::Builder::new()
            .name("sershi-voice-watchdog".to_owned())
            .spawn(move || {
                let started = Instant::now();
                while started.elapsed() < deadline {
                    if finished.load(Ordering::Relaxed) || cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                if !finished.load(Ordering::Relaxed) {
                    timed_out.store(true, Ordering::Relaxed);
                    cancel.store(true, Ordering::Relaxed);
                }
            });
    }

    let mut speculative = false;
    let mut decoded = match speculation {
        Some(early) => {
            // Cancelling the command also aborts the early decode.
            let flag = early.cancel.clone();
            let watch = cancel.clone();
            let mut waited = None;
            while waited.is_none() {
                match early.result.recv_timeout(Duration::from_millis(50)) {
                    Ok(d) => waited = Some(d),
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if watch.load(Ordering::Relaxed) {
                            flag.store(true, Ordering::Relaxed);
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            match waited {
                Some(d) if d.result.is_ok() => {
                    speculative = true;
                    d
                }
                _ => decode(&voice, model, &audio, rate, &opts),
            }
        }
        None => decode(&voice, model, &audio, rate, &opts),
    };

    // Automatic language: a short result in an unexpected language gets one
    // second pass in ES/EN/PT, only when the evidence supports it.
    let speech_ms = timeline
        .speech_started_ms
        .zip(timeline.speech_ended_ms)
        .map(|(a, b)| b.saturating_sub(a));
    let mut opts = opts;
    let mut language_retry = None;
    if detect
        && !cancel.load(Ordering::Relaxed)
        && let Ok(first) = decoded.result.as_ref()
    {
        let now = Instant::now();
        let context = match voice.locked() {
            Ok(mut inner) => {
                inner.recent_languages.observe(first, now);
                LanguageContext {
                    interface: inner.settings.interface_language.clone(),
                    recent: inner.recent_languages.recent(now),
                }
            }
            Err(_) => LanguageContext::default(),
        };
        let unexpected = first
            .language
            .as_ref()
            .is_some_and(|l| !stabilize::in_prior(l));
        // Only an unexpected language needs the (sub-millisecond,
        // model-free, side-effect-free) understanding check.
        let resolves = unexpected
            && app
                .state::<Runtime>()
                .with_service(|s| s.resolves_deterministically(&transcript::clean(&first.text)))
                .unwrap_or(false);
        let plan = stabilize::plan_retry(
            stabilize::FirstPass {
                transcript: first,
                speech_ms,
                automatic: detect,
                resolves,
            },
            &context,
        );
        // Language tags and flags only, never words.
        if unexpected {
            eprintln!(
                "SERSHI voice: language check first={} resolves={resolves} interface={}                  recent={} → retry {}",
                first.language.as_ref().map_or("-", LanguageTag::as_str),
                context.interface.as_ref().map_or("-", LanguageTag::as_str),
                context.recent.as_ref().map_or("-", LanguageTag::as_str),
                plan.as_ref().map_or("none", LanguageTag::as_str),
            );
        }
        if let Some(language) = plan {
            let retry = retry_language(&voice, model, &audio, rate, &opts, first, language);
            decoded.stt_ms = decoded.stt_ms.saturating_add(retry.ms);
            if let Some(second) = retry.accepted.clone() {
                // A later second opinion keeps the chosen language.
                opts.language = second.language.clone();
                decoded.result = Ok(second);
            }
            language_retry = Some(retry);
        }
    }

    // A Fast result too unclear to act on gets one second opinion from the
    // Accurate model, when it is installed and a GPU makes that quick.
    let unclear = decoded
        .result
        .as_ref()
        .is_ok_and(|t| matches!(transcript::assess(t), Verdict::Unclear));
    let accurate = SttModel::for_profile(SpeechProfile::Accurate);
    if unclear
        && profile == SpeechProfile::Fast
        && decoded.acceleration == Some(Acceleration::Vulkan)
        && voice.store.state(accurate) == ModelState::Installed
        && !cancel.load(Ordering::Relaxed)
    {
        let second = decode(&voice, accurate, &audio, rate, &opts);
        eprintln!(
            "SERSHI voice: unclear fast result; accurate second pass took {} ms",
            second.stt_ms
        );
        decoded.stt_ms += second.stt_ms;
        decoded.load_ms = second.load_ms.or(decoded.load_ms);
        decoded.result = second.result;
    }
    drop(audio);
    if let Ok(mut inner) = voice.locked() {
        inner.recognition = None;
    }
    finished.store(true, Ordering::Relaxed);
    if timed_out.load(Ordering::Relaxed) {
        // Too slow: nothing is submitted; the turn fails visibly and a
        // voice session recovers to Listening.
        eprintln!("SERSHI voice: recognition exceeded its deadline and was cancelled");
        end_voice(app, None, true);
        emit_update(
            app,
            VoiceUpdate::Failed {
                reason: VoiceFailure::RecognitionFailed,
            },
        );
        if let Some(next) = with_session(app, VoiceSession::unheard) {
            after_turn(app, next);
        }
        return;
    }
    if cancel.load(Ordering::Relaxed) {
        // Dismissed while recognising: the result is discarded, never run.
        return;
    }

    let post_capture_ms = millis(timeline.stopped);
    let mut timings = VoiceTimings {
        speech_start_ms: timeline.speech_started_ms,
        speech_ms: timeline
            .speech_started_ms
            .zip(timeline.speech_ended_ms)
            .map(|(a, b)| b.saturating_sub(a)),
        endpoint_ms: timeline
            .speech_ended_ms
            .map(|end| timeline.capture_ms.saturating_sub(end)),
        model_load_ms: decoded.load_ms,
        stt_ms: Some(decoded.stt_ms),
        post_capture_ms: Some(post_capture_ms),
        speech_end_to_transcript_ms: None,
        pipeline_ms: None,
        tool_ms: None,
        speculative,
        detected_language: detect,
        language: decoded
            .result
            .as_ref()
            .ok()
            .and_then(|t| t.language.clone()),
        confidence_pct: decoded
            .result
            .as_ref()
            .ok()
            .map(|t| (t.confidence.clamp(0.0, 1.0) * 100.0).round() as u8),
        first_language: language_retry
            .as_ref()
            .and_then(|r| r.first_language.clone()),
        retry_accepted: language_retry
            .as_ref()
            .is_some_and(|r| r.accepted.is_some()),
        language_retry_ms: language_retry.as_ref().map(|r| r.ms),
        acceleration: decoded.acceleration,
        model: model.id.to_owned(),
    };
    timings.speech_end_to_transcript_ms = timings
        .endpoint_ms
        .map(|e| e.saturating_add(post_capture_ms));

    let heard = match decoded.result {
        Ok(heard) => heard,
        Err(SttError::Cancelled) => return,
        Err(error) => {
            let reason = stt_failure(&error);
            eprintln!("SERSHI voice: recognition failed ({reason:?})");
            end_voice(app, None, true);
            emit_update(app, VoiceUpdate::Failed { reason });
            report(app, timings);
            return;
        }
    };
    match transcript::assess(&heard) {
        Verdict::NoSpeech | Verdict::Unclear => {
            let update = if matches!(transcript::assess(&heard), Verdict::NoSpeech) {
                VoiceUpdate::NoSpeech
            } else {
                VoiceUpdate::Unclear
            };
            unusable(app, update);
            if let Some(next) = with_session(app, VoiceSession::unheard) {
                after_turn(app, next);
            }
        }
        Verdict::Accept(text) => {
            emit_update(
                app,
                VoiceUpdate::Heard {
                    text: text.clone(),
                    language: heard.language.clone(),
                    first_heard: language_retry
                        .as_ref()
                        .filter(|r| r.accepted.is_some())
                        .map(|r| r.first_text.clone()),
                },
            );
            // In a voice session, a farewell ends it and "sí" to "anything
            // else?" only prompts; neither is a request.
            match with_session(app, |s| s.heard(&text)) {
                Some(Heard::Farewell) => {
                    leave_transcribing(app);
                    end_session(app, SessionEndReason::Farewell);
                    report(app, timings);
                    return;
                }
                Some(Heard::Prompt) => {
                    leave_transcribing(app);
                    emit_update(app, VoiceUpdate::Prompt);
                    after_turn(app, Next::Listen);
                    report(app, timings);
                    return;
                }
                Some(Heard::Submit) | None => {}
            }
            // Exactly the typed-command path (see AssistantService docs).
            let t0 = Instant::now();
            let runtime = app.state::<Runtime>();
            // Only a supported conversation language is a useful hint for
            // understanding; a misdetected one would only mislead it.
            let language = heard
                .language
                .as_ref()
                .filter(|l| stabilize::in_prior(l))
                .map(|l| l.as_str().to_owned());
            let started = runtime.with_service(|s| {
                s.begin_transcript(
                    &text,
                    Some(heard.confidence),
                    language.as_deref(),
                    &mut |e| broadcast(app, e),
                )
            });
            // Exactly the typed path: brain inference and plan steps run
            // without holding the service.
            let submitted = match started {
                Ok(Some(progress)) => {
                    drive(app, progress).zip(runtime.with_service(|s| s.snapshot()).ok())
                }
                _ => None,
            };
            if let Some((outcome, snapshot)) = submitted {
                timings.pipeline_ms = Some(millis(t0));
                if let Some(trace) = outcome.understanding.as_ref() {
                    // How it was understood, never what was said.
                    eprintln!(
                        "SERSHI voice: understood status={:?} tier={:?} confidence={:.2} \
                         semantic={:?} understanding={:.1}ms semantic_ms={}",
                        outcome.status,
                        trace.tier,
                        trace.confidence,
                        trace.semantic,
                        trace.understanding_ms,
                        trace
                            .semantic_ms
                            .map_or_else(|| "-".to_owned(), |v| v.to_string()),
                    );
                }
                timings.tool_ms = outcome.duration_ms;
                confirmation::sync(app);
                schedule_settle(app, &snapshot);
                if let Ok(mut inner) = voice.locked() {
                    inner.answered_at = Some(Instant::now());
                }
                // The user may have moved on while this ran (pressed to
                // talk again): the outcome is shown, never spoken, and the
                // session follows the new turn instead.
                let superseded = voice.locked().map_or(true, |i| i.capture.is_some());
                let turn = if superseded {
                    None
                } else {
                    with_session(app, |s| s.answered(&outcome))
                };
                emit_update(
                    app,
                    VoiceUpdate::Answered {
                        outcome: Box::new(outcome),
                        speak: !superseded,
                        anything_else: turn.is_some_and(|t| t.anything_else),
                    },
                );
                if turn.is_some() {
                    after_turn(app, Next::Listen);
                }
            }
        }
    }
    report(app, timings);
}

/// Logs and publishes where the time went (never what was said).
fn report(app: &AppHandle, timings: VoiceTimings) {
    let ms = |v: Option<u32>| v.map_or_else(|| "-".to_owned(), |v| v.to_string());
    eprintln!(
        "SERSHI voice: timings model={} backend={:?} detect={} speculative={} language={} \
         confidence={}% first_language={} retry_accepted={} retry={}ms endpoint={}ms \
         load={}ms stt={}ms post_capture={}ms end_to_transcript={}ms pipeline={}ms tool={}ms",
        timings.model,
        timings.acceleration,
        timings.detected_language,
        timings.speculative,
        timings.language.as_ref().map_or("-", LanguageTag::as_str),
        timings
            .confidence_pct
            .map_or_else(|| "-".to_owned(), |v| v.to_string()),
        timings
            .first_language
            .as_ref()
            .map_or("-", LanguageTag::as_str),
        timings.retry_accepted,
        ms(timings.language_retry_ms),
        ms(timings.endpoint_ms),
        ms(timings.model_load_ms),
        ms(timings.stt_ms),
        ms(timings.post_capture_ms),
        ms(timings.speech_end_to_transcript_ms),
        ms(timings.pipeline_ms),
        ms(timings.tool_ms),
    );
    emit_update(app, VoiceUpdate::Timings { timings });
}

// ── Speech ────────────────────────────────────────────────────────────────

/// Reports playback progress from the playback thread.
struct SpeechSink {
    app: AppHandle,
    id: u64,
    meter: LevelMeter,
    last: Instant,
}

impl PlaybackSink for SpeechSink {
    fn played(&mut self, mono: &[f32]) {
        let level = self.meter.push(mono);
        if self.last.elapsed() >= LEVEL_INTERVAL {
            self.last = Instant::now();
            emit_level(&self.app, level, LevelSource::Output);
        }
    }

    fn finished(&mut self, end: PlaybackEnd) {
        emit_level(&self.app, 0.0, LevelSource::Output);
        if end == PlaybackEnd::Failed {
            eprintln!("SERSHI voice: playback failed");
        }
        // Only the current playback ends Speaking; a replaced or stopped one
        // was already accounted for by `stop_speaking`.
        let taken = managed(&self.app).and_then(|v| {
            v.locked().ok().and_then(|mut inner| {
                if inner
                    .playback
                    .as_ref()
                    .is_some_and(|(id, _)| *id == self.id)
                {
                    inner.playback.take()
                } else {
                    None
                }
            })
        });
        if let Some((_, handle)) = taken {
            // Dropping on the playback thread does not join it (see the
            // adapter); the thread ends right after this callback.
            drop(handle);
            finish_speaking(&self.app);
            reply_finished(&self.app);
        }
    }
}

fn finish_speaking(app: &AppHandle) {
    let runtime = app.state::<Runtime>();
    let _ = runtime.with_service(|s| s.end_speaking(&mut |e| broadcast(app, e)));
}

/// Speaks a reply the Command Center phrased in the user's language. Output
/// only: it cannot trigger anything. Ignored while the microphone is open.
pub fn speak(app: &AppHandle, text: &str, language: Option<&str>) -> Result<(), IpcError> {
    let text = text.trim();
    let invalid = || IpcError::new(IpcErrorCode::NotAllowed, "Invalid speech request.");
    if text.is_empty() || text.chars().count() > MAX_SPEECH_CHARS {
        return Err(invalid());
    }
    let language = match language {
        Some(tag) => Some(LanguageTag::parse(tag).map_err(|_| invalid())?),
        None => None,
    };
    let voice = managed(app).ok_or_else(|| failure(VoiceFailure::Unsupported))?;
    let voice_id = {
        let inner = voice.locked()?;
        if inner.capture.is_some() {
            return Ok(());
        }
        inner.settings.voice.clone()
    };
    stop_speaking(app);
    let epoch = {
        let mut inner = voice.locked()?;
        inner.synthesizing = true;
        inner.speech_epoch
    };
    let t0 = Instant::now();
    let synthesized = voice.platform.synthesis.synthesize(
        text,
        VoiceChoice {
            voice_id: voice_id.as_deref(),
            language: language.as_ref(),
        },
    );
    if let Ok(mut inner) = voice.locked() {
        inner.synthesizing = false;
    }
    let Ok(audio) = synthesized else {
        reply_finished(app);
        return Err(failure(VoiceFailure::SpeechUnavailable));
    };
    let id = {
        let mut inner = voice.locked()?;
        if inner.capture.is_some() || inner.speech_epoch != epoch {
            // The user started talking, typed, or pressed Stop while SERSHI
            // prepared its reply.
            drop(inner);
            reply_finished(app);
            return Ok(());
        }
        inner.next_id += 1;
        inner.next_id
    };
    let runtime = app.state::<Runtime>();
    let _ = runtime.with_service(|s| s.begin_speaking(&mut |e| broadcast(app, e)));
    let sink = SpeechSink {
        app: app.clone(),
        id,
        meter: LevelMeter::new(),
        last: Instant::now(),
    };
    // Register before playing so a very short clip's end finds itself.
    let (ready_tx, ready_rx) = mpsc::channel::<()>();
    let gate = GatedSink {
        inner: sink,
        ready: Some(ready_rx),
    };
    match voice.platform.output.play(audio, Box::new(gate)) {
        Ok(handle) => {
            // Swap under the lock, drop outside it: dropping a playback
            // joins its thread, whose `finished` callback takes this lock.
            // If speech was stopped meanwhile, this reply never registers.
            let (previous, stale) = match voice.locked() {
                Ok(mut inner) if inner.speech_epoch == epoch && inner.capture.is_none() => {
                    (inner.playback.replace((id, handle)), None)
                }
                _ => (None, Some(handle)),
            };
            drop(previous);
            let _ = ready_tx.send(());
            if let Some(stale) = stale {
                stale.stop();
                finish_speaking(app);
                reply_finished(app);
                return Ok(());
            }
            with_session(app, VoiceSession::speaking);
            eprintln!(
                "SERSHI voice: speech started {} ms after request",
                t0.elapsed().as_millis()
            );
            // Result → speech start, for a spoken command's latency record.
            let answered = voice.locked().ok().and_then(|mut i| i.answered_at.take());
            if let Some(at) = answered.filter(|at| at.elapsed() < Duration::from_secs(10)) {
                emit_update(app, VoiceUpdate::SpeechLatency { ms: millis(at) });
            }
            Ok(())
        }
        Err(_) => {
            finish_speaking(app);
            reply_finished(app);
            Err(failure(VoiceFailure::OutputUnavailable))
        }
    }
}

/// Holds `finished` until the playback handle has been registered.
struct GatedSink {
    inner: SpeechSink,
    ready: Option<Receiver<()>>,
}

impl PlaybackSink for GatedSink {
    fn played(&mut self, mono: &[f32]) {
        self.inner.played(mono);
    }
    fn finished(&mut self, end: PlaybackEnd) {
        if let Some(ready) = self.ready.take() {
            let _ = ready.recv_timeout(Duration::from_secs(1));
        }
        self.inner.finished(end);
    }
}

/// Stops a spoken reply (the "Stop speaking" button, a new request, the
/// microphone). Safe to call when nothing is playing.
pub fn stop_speaking(app: &AppHandle) {
    let Some(voice) = managed(app) else {
        return;
    };
    let taken = voice.locked().ok().and_then(|mut i| {
        i.speech_epoch = i.speech_epoch.wrapping_add(1);
        i.playback.take()
    });
    if let Some((_, handle)) = taken {
        // Joins the playback thread; no lock is held here.
        handle.stop();
        finish_speaking(app);
        reply_finished(app);
    }
}

/// Before a typed command: SERSHI stops listening and talking. A voice
/// session stays open: after the typed request it listens again (one
/// conversation, either modality).
pub fn interrupt(app: &AppHandle) {
    if let Some(voice) = managed(app)
        && let Ok(mut inner) = voice.locked()
    {
        inner.awaiting = None;
    }
    cancel_capture(app);
    stop_speaking(app);
    // Give the session worker a moment to release the microphone and leave
    // the voice state, so the typed request is not refused as busy.
    for _ in 0..20 {
        let busy = managed(app).is_some_and(|v| v.locked().is_ok_and(|i| i.capture.is_some()));
        if !busy {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

// ── Models ────────────────────────────────────────────────────────────────

fn emit_model(app: &AppHandle, model: &str, state: ModelState, error: Option<ModelError>) {
    let _ = app.emit_to(
        MAIN,
        MODEL_EVENT,
        ModelProgress {
            model: model.to_owned(),
            state,
            error,
        },
    );
}

/// Downloads a catalog model after the user asked for it in Settings.
pub fn download_model(app: &AppHandle, model_id: &str) -> Result<(), IpcError> {
    let model = SttModel::find(model_id)
        .ok_or_else(|| IpcError::new(IpcErrorCode::NotAllowed, "Unknown speech model."))?;
    let voice = managed(app).ok_or_else(|| failure(VoiceFailure::Unsupported))?;
    if !voice.platform.supported {
        return Err(failure(VoiceFailure::Unsupported));
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut inner = voice.locked()?;
        if inner.download.is_some() {
            return Err(failure(VoiceFailure::Busy));
        }
        inner.download = Some((model.id.to_owned(), cancel.clone()));
        inner.model_states.insert(
            model.id.to_owned(),
            ModelState::Downloading { received_bytes: 0 },
        );
    }
    emit_model(
        app,
        model.id,
        ModelState::Downloading { received_bytes: 0 },
        None,
    );
    let app = app.clone();
    let spawned = thread::Builder::new()
        .name("sershi-model-download".to_owned())
        .spawn(move || {
            let Some(voice) = managed(&app) else {
                return;
            };
            let t0 = Instant::now();
            let mut last = Instant::now();
            let mut progress = |received: u64| {
                if last.elapsed() >= PROGRESS_INTERVAL || received == model.size_bytes {
                    last = Instant::now();
                    let state = ModelState::Downloading {
                        received_bytes: received,
                    };
                    if let Ok(mut inner) = voice.locked() {
                        inner
                            .model_states
                            .insert(model.id.to_owned(), state.clone());
                    }
                    emit_model(&app, model.id, state, None);
                }
            };
            let result = platform::download_model(&voice.store, model, &mut progress, &cancel);
            if let Ok(mut inner) = voice.locked() {
                inner.download = None;
                inner.model_states.remove(model.id);
            }
            match result {
                Ok(()) => {
                    eprintln!(
                        "SERSHI voice: model {} installed in {} s",
                        model.id,
                        t0.elapsed().as_secs()
                    );
                    emit_model(&app, model.id, ModelState::Installed, None);
                    // With a GPU, compile its shaders now (up to ~25 s the
                    // first time on a machine) instead of on the first
                    // command. Background only: no microphone, no command.
                    if voice.acceleration().0 == Acceleration::Vulkan
                        && voice
                            .locked()
                            .is_ok_and(|i| i.settings.model().id == model.id)
                    {
                        warm_up(&app, model);
                    }
                }
                Err(error) => {
                    eprintln!("SERSHI voice: model download failed ({error:?})");
                    emit_model(&app, model.id, voice.store.state(model), Some(error));
                }
            }
        });
    if spawned.is_err() {
        if let Ok(mut inner) = voice.locked() {
            inner.download = None;
            inner.model_states.remove(model.id);
        }
        return Err(IpcError::new(
            IpcErrorCode::Internal,
            "Voice is unavailable.",
        ));
    }
    Ok(())
}

pub fn cancel_download(app: &AppHandle) {
    if let Some(voice) = managed(app)
        && let Ok(inner) = voice.locked()
        && let Some((_, cancel)) = inner.download.as_ref()
    {
        cancel.store(true, Ordering::Relaxed);
    }
}

/// Quitting: release the microphone and the speaker, stop downloads.
pub fn shutdown(app: &AppHandle) {
    end_session(app, SessionEndReason::Dismissed);
    cancel_capture(app);
    stop_speaking(app);
    cancel_download(app);
    // Wait briefly for the session worker to release the microphone.
    for _ in 0..50 {
        let capturing = managed(app).is_some_and(|v| v.locked().is_ok_and(|i| i.capture.is_some()));
        if !capturing {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

// ── Voice session (Gate 4.1) ──────────────────────────────────────────────

fn wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn emit_session(app: &AppHandle, status: VoiceSessionStatus) {
    let _ = app.emit_to(MAIN, SESSION_EVENT, status);
    let _ = app.emit_to(COMPANION, SESSION_EVENT, status);
}

/// Applies `f` to the active session and publishes its status if it
/// changed. `None` without an active session.
fn with_session<R>(app: &AppHandle, f: impl FnOnce(&mut VoiceSession) -> R) -> Option<R> {
    let voice = managed(app)?;
    let (result, changed) = {
        let mut inner = voice.locked().ok()?;
        let session = inner.session.as_mut()?;
        let before = session.status();
        let result = f(session);
        let after = session.status();
        (result, (after != before).then_some(after))
    };
    if let Some(status) = changed {
        emit_session(app, status);
    }
    Some(result)
}

pub fn session_active(app: &AppHandle) -> bool {
    managed(app).is_some_and(|v| v.locked().is_ok_and(|i| i.session.is_some()))
}

/// The session's status (`None` when no session is active).
pub fn session_status(app: &AppHandle) -> Option<VoiceSessionStatus> {
    managed(app)?
        .locked()
        .ok()?
        .session
        .as_ref()
        .map(VoiceSession::status)
}

fn audit(app: &AppHandle, activity: NewActivity) {
    let runtime = app.state::<Runtime>();
    if let Ok(entry) = runtime.with_service(|s| s.record(activity)) {
        broadcast(app, ServiceEvent::Activity(entry));
    }
}

/// Starts a hands-free voice session (the explicit button; there is no
/// wake word). The microphone opens now and after each reply until the
/// session ends.
pub fn start_session(app: &AppHandle) -> Result<CaptureStart, IpcError> {
    let Some(voice) = managed(app) else {
        return refused(VoiceFailure::Unsupported);
    };
    let status = {
        let mut inner = voice.locked()?;
        if inner.session.is_some() {
            return Ok(CaptureStart::Started);
        }
        inner.session_seq += 1;
        let session = VoiceSession::start(inner.session_seq, wall_ms());
        let status = session.status();
        inner.session = Some(session);
        inner.awaiting = None;
        status
    };
    emit_session(app, status);
    audit(
        app,
        NewActivity::new(ActivityKind::VoiceSessionStarted, "Voice session started"),
    );
    // Warm the Agent Brain while the user speaks (it is released after the
    // usual idle period once the session ends).
    let _ = app.state::<Runtime>().with_service(|s| s.prepare_brain());
    let started = open_microphone(app, Opener::User);
    if !matches!(started, Ok(CaptureStart::Started)) {
        end_session(app, SessionEndReason::Failed);
    }
    started
}

/// The Stop button.
pub fn stop_session(app: &AppHandle) {
    end_session(app, SessionEndReason::Stopped);
    // An utterance still being recognised is dropped too.
    cancel(app);
}

/// Ends the session (if any): the microphone closes and nothing scheduled
/// reopens it.
pub fn end_session(app: &AppHandle, reason: SessionEndReason) {
    let Some(voice) = managed(app) else {
        return;
    };
    let status = {
        let Ok(mut inner) = voice.locked() else {
            return;
        };
        let Some(mut session) = inner.session.take() else {
            return;
        };
        inner.awaiting = None;
        session.end(reason);
        session.status()
    };
    emit_session(app, status);
    let summary = match reason {
        SessionEndReason::Stopped => "Voice session ended (stopped)",
        SessionEndReason::Farewell => "Voice session ended (the user said goodbye)",
        SessionEndReason::Timeout => "Voice session ended (no one spoke)",
        SessionEndReason::NotHeard => "Voice session ended (nothing could be heard)",
        SessionEndReason::MaxDuration => "Voice session ended (time limit)",
        SessionEndReason::Dismissed => "Voice session ended (dismissed)",
        SessionEndReason::Failed => "Voice session ended (microphone unavailable)",
    };
    audit(
        app,
        NewActivity::new(ActivityKind::VoiceSessionEnded, summary),
    );
    cancel_capture(app);
}

/// Leaves Transcribing without a request (a farewell, a prompt): no
/// command runs and an open question is dropped.
fn leave_transcribing(app: &AppHandle) {
    let runtime = app.state::<Runtime>();
    if let Ok(snapshot) = runtime.with_service(|s| {
        s.cancel_voice(None, &mut |e| broadcast(app, e));
        s.dismiss(&mut |e| broadcast(app, e));
        s.snapshot()
    }) {
        schedule_settle(app, &snapshot);
    }
}

/// After a turn: listen again once the reply has been spoken (or at once,
/// if none is), keep waiting, or end the session.
fn after_turn(app: &AppHandle, next: Next) {
    match next {
        Next::Listen => arm(app),
        Next::Wait => {}
        Next::End(reason) => end_session(app, reason),
    }
}

/// Schedules "listen again": when the reply finishes, or after a short
/// grace period if no reply starts (spoken replies may be off).
fn arm(app: &AppHandle) {
    let Some(voice) = managed(app) else {
        return;
    };
    let token = {
        let Ok(mut inner) = voice.locked() else {
            return;
        };
        if inner.session.is_none() {
            return;
        }
        inner.resume_seq += 1;
        inner.awaiting = Some(inner.resume_seq);
        inner.resume_seq
    };
    let app = app.clone();
    let _ = thread::Builder::new()
        .name("sershi-voice-turn".to_owned())
        .spawn(move || {
            thread::sleep(Duration::from_millis(REPLY_GRACE_MS));
            let silent = managed(&app).is_some_and(|v| {
                v.locked().is_ok_and(|i| {
                    i.awaiting == Some(token) && i.playback.is_none() && !i.synthesizing
                })
            });
            if silent {
                resume(&app, token);
            }
        });
}

/// SERSHI's reply ended (or was stopped or failed): a waiting turn listens
/// again after the echo guard.
fn reply_finished(app: &AppHandle) {
    let Some(token) = managed(app).and_then(|v| v.locked().ok().and_then(|i| i.awaiting)) else {
        return;
    };
    let app = app.clone();
    let _ = thread::Builder::new()
        .name("sershi-voice-turn".to_owned())
        .spawn(move || {
            thread::sleep(ECHO_GUARD);
            resume(&app, token);
        });
}

/// Opens the microphone for the next turn, if this turn is still the one
/// waiting and the session says so.
fn resume(app: &AppHandle, token: u64) {
    let Some(voice) = managed(app) else {
        return;
    };
    let next = {
        let Ok(mut inner) = voice.locked() else {
            return;
        };
        if inner.awaiting != Some(token)
            || inner.capture.is_some()
            || inner.playback.is_some()
            || inner.synthesizing
        {
            return;
        }
        inner.awaiting = None;
        match inner.session.as_mut() {
            Some(session) => session.reply_done(wall_ms()),
            None => return,
        }
    };
    match next {
        Next::Listen => {
            if !matches!(
                open_microphone(app, Opener::Session),
                Ok(CaptureStart::Started)
            ) {
                end_session(app, SessionEndReason::Failed);
            }
        }
        Next::Wait => {}
        Next::End(reason) => end_session(app, reason),
    }
}

/// A typed request finished during a voice session: the session follows it
/// (one conversation for both modalities) and listens again.
pub fn after_typed(app: &AppHandle, outcome: &CommandOutcome) {
    if with_session(app, |s| s.answered(outcome)).is_some() {
        arm(app);
    }
}

/// The trusted confirmation window decided (or the approval expired or was
/// cancelled). If nothing else is waiting for approval, the session
/// listens again.
pub fn confirmation_resolved(app: &AppHandle) {
    let pending = app
        .state::<Runtime>()
        .with_service(|s| s.pending_confirmation().is_some())
        .unwrap_or(false);
    if pending {
        return;
    }
    if let Some(next) = with_session(app, VoiceSession::confirmation_resolved) {
        after_turn(app, next);
    }
}

/// Follows the assistant's working states in the session indicator. Never
/// blocks: called while the core is locked, so it skips if voice is busy.
pub fn observe(app: &AppHandle, state: AssistantState) {
    let Some(voice) = managed(app) else {
        return;
    };
    let changed = {
        let Ok(mut inner) = voice.inner.try_lock() else {
            return;
        };
        let Some(session) = inner.session.as_mut() else {
            return;
        };
        let before = session.status();
        session.working(state);
        let after = session.status();
        (after != before).then_some(after)
    };
    if let Some(status) = changed {
        emit_session(app, status);
    }
}
