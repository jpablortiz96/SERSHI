//! Voice orchestration: push-to-talk capture, local recognition, spoken
//! replies and model downloads.
//!
//! The shell only wires adapters to the core. Every decision about *what
//! happens* stays in `sershi-core`: the state machine says when the
//! microphone may open, and a recognised transcript is submitted through
//! `AssistantService::submit_transcript`, which is the typed-command path.
//! Nothing here can approve, read a confirmation id or grant a permission.
//!
//! Threads and locks: audio callbacks only send over channels; one worker
//! thread owns each capture session (and the microphone handle); playback
//! runs on its own thread. No lock is held while joining an audio thread or
//! calling into the service.
//!
//! Privacy: audio lives in memory for the length of one utterance and is
//! dropped after recognition. Nothing is written to disk or logs; the UI
//! receives only the transcript to show and a bounded 0–1 level.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use sershi_core::ipc::{IpcError, IpcErrorCode};
use sershi_core::voice::endpoint::{Endpoint, EndpointConfig, EndpointDetector};
use sershi_core::voice::ports::{
    ActiveCapture, ActivePlayback, CaptureError, CaptureFormat, CaptureSink, PlaybackEnd,
    PlaybackSink, SpeechToTextPort, SttError, VoiceChoice,
};
use sershi_core::voice::signal::{LevelMeter, RECOGNITION_RATE, downmix, resample};
use sershi_core::voice::transcript::{self, Verdict};
use sershi_core::voice::{
    CaptureStart, LanguageTag, LevelSource, MicrophoneStatus, ModelError, ModelInfo, ModelProgress,
    ModelState, SttModel, VoiceFailure, VoiceLevel, VoiceSettings, VoiceStatus, VoiceUpdate,
};
use sershi_platform::voice::{self as platform, ModelStore, VoicePlatform};
use tauri::{AppHandle, Emitter, Manager};

use crate::confirmation;
use crate::integration;
use crate::runtime::{Runtime, broadcast, schedule_settle};
use crate::surfaces::{COMPANION, MAIN};

/// Voice events for the Command Center (`VoiceUpdate`).
pub const VOICE_EVENT: &str = "sershi://voice";
/// Bounded audio level for visuals (`VoiceLevel`), Command Center and companion.
pub const LEVEL_EVENT: &str = "sershi://voice-level";
/// Model download progress (`ModelProgress`), Command Center only.
pub const MODEL_EVENT: &str = "sershi://voice-model";

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
    playback: Option<(u64, Box<dyn ActivePlayback>)>,
    next_id: u64,
    /// Model download in progress: (model id, cancel flag).
    download: Option<(String, Arc<AtomicBool>)>,
    /// Transient model states (downloading) that the store cannot know.
    model_states: HashMap<String, ModelState>,
    /// The chosen microphone was missing at the last capture.
    fallback: bool,
}

/// Recognition engine, loaded lazily and kept while voice is in use.
#[derive(Default)]
struct Recognizer {
    loaded: Option<(String, Arc<dyn SpeechToTextPort>)>,
}

pub struct Voice {
    platform: VoicePlatform,
    store: ModelStore,
    inner: Mutex<Inner>,
    recognizer: Arc<Mutex<Recognizer>>,
}

impl Voice {
    pub fn new(models_dir: PathBuf) -> Self {
        let store = ModelStore::new(models_dir);
        // An interrupted download from a previous run leaves nothing usable.
        store.remove_partials();
        Self {
            platform: platform::voice_platform(),
            store,
            inner: Mutex::new(Inner::default()),
            recognizer: Arc::new(Mutex::new(Recognizer::default())),
        }
    }

    fn locked(&self) -> Result<MutexGuard<'_, Inner>, IpcError> {
        self.inner
            .lock()
            .map_err(|_| IpcError::new(IpcErrorCode::Internal, "Voice is unavailable."))
    }
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
    // Device and voice enumeration happen outside the lock.
    let devices = if voice.platform.supported {
        voice.platform.capture.input_devices().unwrap_or_default()
    } else {
        Vec::new()
    };
    let voices = voice.platform.synthesis.voices().unwrap_or_default();
    let access = platform::microphone_access();
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
        model: inner.settings.model().id.to_owned(),
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
    let model_changed = {
        let mut inner = voice.locked()?;
        let changed = inner.settings.model().id != settings.model().id;
        if inner.settings.microphone != settings.microphone {
            inner.fallback = false;
        }
        inner.settings = settings;
        changed
    };
    if model_changed {
        // Release the previous model's memory.
        if let Ok(mut r) = voice.recognizer.lock() {
            r.loaded = None;
        }
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

/// Push-to-talk. Opens the microphone only after the core agrees (which
/// also cancels any pending approval) and reports failures honestly.
pub fn start_capture(app: &AppHandle) -> Result<CaptureStart, IpcError> {
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
    // Half duplex: SERSHI stops talking before it listens, so its own
    // voice never reaches the recogniser.
    stop_speaking(app);

    let runtime = app.state::<Runtime>();
    let began = runtime.with_service(|s| s.begin_listening(&mut |e| broadcast(app, e)))?;
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
    warm_up(&voice, model);
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
/// dismissed it. (While listening, the session records the microphone
/// duration itself.)
pub fn cancel(app: &AppHandle) {
    cancel_capture(app);
    let runtime = app.state::<Runtime>();
    let dropped = runtime
        .with_service(|s| {
            s.snapshot().state == sershi_core::assistant::AssistantState::Transcribing
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

/// One push-to-talk session: owns the microphone handle, gathers audio,
/// detects the end of speech and hands off to recognition.
fn session(
    app: &AppHandle,
    id: u64,
    rx: Receiver<Msg>,
    handle: Box<dyn ActiveCapture>,
    format: CaptureFormat,
) {
    let started = Instant::now();
    let config = EndpointConfig::default();
    let mut detector = EndpointDetector::new(format.sample_rate, config);
    let max_samples =
        (u64::from(format.sample_rate) * u64::from(config.max_capture_ms) / 1000) as usize;
    let mut audio: Vec<f32> = Vec::with_capacity(format.sample_rate as usize * 5);
    let mut meter = LevelMeter::new();
    let mut last_level = Instant::now();
    let mut mono = Vec::new();

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
                match detector.push(&mono) {
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

    match finish {
        Finish::Cancel => {
            end_voice(app, Some(mic_ms), false);
            release();
            emit_update(app, VoiceUpdate::Cancelled);
        }
        Finish::Failed(reason) => {
            end_voice(app, Some(mic_ms), true);
            release();
            emit_update(app, VoiceUpdate::Failed { reason });
        }
        Finish::Silent => {
            end_voice(app, Some(mic_ms), true);
            release();
            emit_update(
                app,
                VoiceUpdate::Failed {
                    reason: VoiceFailure::MicrophoneSilent,
                },
            );
        }
        Finish::NoSpeech => {
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
                recognise(app, audio, format.sample_rate);
            }
        }
    }
}

fn begin_transcribing(app: &AppHandle, mic_ms: u32) -> bool {
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

fn recognise(app: &AppHandle, audio: Vec<f32>, rate: u32) {
    let Some(voice) = managed(app) else {
        return;
    };
    let (model, language) = match voice.locked() {
        Ok(inner) => (inner.settings.model(), inner.settings.language.clone()),
        Err(_) => return,
    };
    let t0 = Instant::now();
    let pcm = resample(&audio, rate, RECOGNITION_RATE);
    drop(audio);
    let result =
        recognizer(&voice, model).and_then(|engine| engine.transcribe(&pcm, language.as_ref()));
    drop(pcm);
    eprintln!(
        "SERSHI voice: recognition took {} ms ({})",
        t0.elapsed().as_millis(),
        model.id
    );
    let heard = match result {
        Ok(heard) => heard,
        Err(error) => {
            let reason = match error {
                SttError::ModelMissing => VoiceFailure::ModelMissing,
                SttError::ModelCorrupt => VoiceFailure::ModelCorrupt,
                SttError::Unsupported => VoiceFailure::Unsupported,
                SttError::UnsupportedLanguage | SttError::Failed(_) => {
                    VoiceFailure::RecognitionFailed
                }
            };
            eprintln!("SERSHI voice: recognition failed ({reason:?})");
            end_voice(app, None, true);
            emit_update(app, VoiceUpdate::Failed { reason });
            return;
        }
    };
    match transcript::assess(&heard) {
        Verdict::NoSpeech => unusable(app, VoiceUpdate::NoSpeech),
        Verdict::Unclear => unusable(app, VoiceUpdate::Unclear),
        Verdict::Accept(text) => {
            emit_update(
                app,
                VoiceUpdate::Heard {
                    text: text.clone(),
                    language: heard.language.clone(),
                },
            );
            // Exactly the typed-command path (see AssistantService docs).
            let runtime = app.state::<Runtime>();
            let submitted = runtime.with_service(|s| {
                let outcome = s.submit_transcript(&text, &mut |e| broadcast(app, e));
                (outcome, s.snapshot())
            });
            if let Ok((Some(outcome), snapshot)) = submitted {
                confirmation::sync(app);
                schedule_settle(app, &snapshot);
                emit_update(app, VoiceUpdate::Answered { outcome });
            }
        }
    }
}

/// The loaded engine for `model`, loading (and verifying) it if needed.
fn recognizer(voice: &Voice, model: &SttModel) -> Result<Arc<dyn SpeechToTextPort>, SttError> {
    let mut slot = voice
        .recognizer
        .lock()
        .map_err(|_| SttError::Failed("recognizer lock".to_owned()))?;
    if let Some((id, engine)) = slot.loaded.as_ref()
        && *id == model.id
    {
        return Ok(engine.clone());
    }
    slot.loaded = None;
    // Full integrity check before the first load in this session.
    match voice.store.verify(model) {
        Ok(true) => {}
        Ok(false) => return Err(SttError::ModelCorrupt),
        Err(_) => return Err(SttError::ModelMissing),
    }
    let t0 = Instant::now();
    let engine = platform::load_recognizer(&voice.store.path(model))?;
    eprintln!(
        "SERSHI voice: model {} verified and loaded in {} ms",
        model.id,
        t0.elapsed().as_millis()
    );
    slot.loaded = Some((model.id.to_owned(), engine.clone()));
    Ok(engine)
}

/// Starts loading the model while the user is still speaking.
fn warm_up(voice: &Voice, model: &'static SttModel) {
    let loaded = voice
        .recognizer
        .lock()
        .map(|r| r.loaded.as_ref().is_some_and(|(id, _)| id == model.id))
        .unwrap_or(true);
    if loaded {
        return;
    }
    let slot = voice.recognizer.clone();
    let store = voice.store.clone();
    let _ = thread::Builder::new()
        .name("sershi-voice-load".to_owned())
        .spawn(move || {
            // Same steps as `recognizer`, holding the slot so a waiting
            // transcription picks up the result instead of loading twice.
            let Ok(mut slot) = slot.lock() else {
                return;
            };
            if slot.loaded.as_ref().is_some_and(|(id, _)| id == model.id) {
                return;
            }
            if store.verify(model).ok() != Some(true) {
                return;
            }
            let t0 = Instant::now();
            if let Ok(engine) = platform::load_recognizer(&store.path(model)) {
                eprintln!(
                    "SERSHI voice: model {} verified and loaded in {} ms",
                    model.id,
                    t0.elapsed().as_millis()
                );
                slot.loaded = Some((model.id.to_owned(), engine));
            }
        });
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
    let t0 = Instant::now();
    let audio = voice
        .platform
        .synthesis
        .synthesize(
            text,
            VoiceChoice {
                voice_id: voice_id.as_deref(),
                language: language.as_ref(),
            },
        )
        .map_err(|_| failure(VoiceFailure::SpeechUnavailable))?;
    let id = {
        let mut inner = voice.locked()?;
        if inner.capture.is_some() {
            // The user started talking while SERSHI prepared its reply.
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
            let previous = voice
                .locked()
                .ok()
                .and_then(|mut inner| inner.playback.replace((id, handle)));
            drop(previous);
            let _ = ready_tx.send(());
            eprintln!(
                "SERSHI voice: speech started {} ms after request",
                t0.elapsed().as_millis()
            );
            Ok(())
        }
        Err(_) => {
            finish_speaking(app);
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
    let taken = voice.locked().ok().and_then(|mut i| i.playback.take());
    if let Some((_, handle)) = taken {
        // Joins the playback thread; no lock is held here.
        handle.stop();
        finish_speaking(app);
    }
}

/// Before a typed command: SERSHI stops listening and talking.
pub fn interrupt(app: &AppHandle) {
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
