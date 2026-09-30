//! Windows voice adapters against real Windows APIs.
//!
//! - Always run (read-only, no hardware needed): list microphones and
//!   installed voices, check the privacy switch reader.
//! - `#[ignore]`d (need a local speech model, a microphone or speakers; run
//!   on a physical machine, see docs/TESTING.md):
//!   `cargo test -p sershi-platform --test windows_voice -- --ignored --nocapture`
//!   with `SERSHI_STT_MODEL` pointing at an installed model file.
//!
//! No audio is written to disk by these tests; generated speech stays in
//! memory.
#![cfg(windows)]

use std::sync::mpsc;
use std::time::{Duration, Instant};

use sershi_core::voice::ports::{
    CaptureError, CaptureSink, PlaybackEnd, PlaybackSink, SttError, TranscribeOptions, VoiceChoice,
};
use sershi_core::voice::signal::{RECOGNITION_RATE, resample, rms_dbfs};
use sershi_core::voice::transcript::{Verdict, assess};
use sershi_core::voice::{Acceleration, LanguageTag, STT_MODELS};
use sershi_platform::voice::{
    ModelStore, load_recognizer, microphone_access, speech_acceleration, voice_platform,
};

/// Resident memory of this process, in MB (for the validation record).
fn memory_mb() -> u64 {
    let Ok(pid) = sysinfo::get_current_pid() else {
        return 0;
    };
    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).map_or(0, |p| p.memory() / 1_000_000)
}

#[test]
fn lists_microphones_voices_and_privacy_state() {
    let platform = voice_platform();
    assert!(platform.supported);
    match platform.capture.input_devices() {
        Ok(devices) => {
            println!("microphones: {}", devices.len());
            for d in &devices {
                println!(
                    "  {}{}",
                    d.name,
                    if d.is_default { " (default)" } else { "" }
                );
                assert!(!d.id.is_empty());
            }
        }
        Err(e) => println!("microphones unavailable: {e}"),
    }
    let voices = platform.synthesis.voices().unwrap_or_default();
    println!("voices: {}", voices.len());
    for v in &voices {
        println!("  {} [{}]", v.name, v.language);
    }
    println!("microphone access: {:?}", microphone_access());
}

fn model_path() -> Option<std::path::PathBuf> {
    std::env::var_os("SERSHI_STT_MODEL").map(Into::into)
}

/// Offline round trip: Windows speech synthesis → resample → whisper.cpp.
#[test]
#[ignore = "needs SERSHI_STT_MODEL and an installed Windows voice"]
fn synthesized_speech_is_recognised_offline() {
    let Some(model) = model_path() else {
        panic!("set SERSHI_STT_MODEL to an installed model file");
    };
    let platform = voice_platform();
    // Integrity check as SERSHI performs it before the first load.
    let name = model
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    if let (Some(entry), Some(dir)) = (
        STT_MODELS.iter().find(|m| m.file_name == name),
        model.parent(),
    ) {
        let t = Instant::now();
        assert!(
            ModelStore::new(dir).verify(entry).expect("readable"),
            "sha256"
        );
        println!("sha256 verify: {} ms", t.elapsed().as_millis());
    }
    let voices = platform.synthesis.voices().expect("voices");
    let mut clips = Vec::new();
    for (text, language, expect) in [
        ("Abre Spotify.", "es", "spotify"),
        ("Open the calculator.", "en", "calculator"),
        ("Abra a calculadora.", "pt", "calculadora"),
    ] {
        let tag = LanguageTag::parse(language).unwrap();
        if !voices
            .iter()
            .any(|v| LanguageTag::parse(&v.language).is_ok_and(|l| l.same_language(&tag)))
        {
            println!("skip {language}: no installed voice");
            continue;
        }
        let speech = platform
            .synthesis
            .synthesize(
                text,
                VoiceChoice {
                    voice_id: None,
                    language: Some(&tag),
                },
            )
            .expect("synthesis");
        let pcm = resample(&speech.samples, speech.sample_rate, RECOGNITION_RATE);
        clips.push((tag, expect, pcm));
    }

    // The accelerator SERSHI would pick, then the CPU fallback.
    let (best, name) = speech_acceleration();
    println!("accelerator: {best:?} {name:?}");
    let mut backends = vec![best];
    if best != Acceleration::Cpu {
        backends.push(Acceleration::Cpu);
    }
    let context = Some("Chrome, Outlook, Spotify, Notepad.".to_owned());
    for backend in backends {
        let before = memory_mb();
        let t0 = Instant::now();
        let engine = load_recognizer(&model, backend).expect("model loads");
        println!(
            "[{backend:?}] load + warm-up: {} ms, memory {} → {} MB, engine on {:?}",
            t0.elapsed().as_millis(),
            before,
            memory_mb(),
            engine.acceleration()
        );
        assert!(backend == Acceleration::Cpu || engine.acceleration() == backend);
        for (tag, expect, pcm) in &clips {
            for language in [None, Some(tag.clone())] {
                for hints in [None, context.clone()] {
                    let options = TranscribeOptions {
                        language: language.clone(),
                        context: hints.clone(),
                        cancel: None,
                    };
                    let t = Instant::now();
                    let heard = engine.transcribe(pcm, &options).expect("recognition");
                    println!(
                        "[{backend:?} {} lang={:?} hints={}] {:?} conf={:.2} in {} ms",
                        tag.as_str(),
                        language.as_ref().map(LanguageTag::as_str),
                        hints.is_some(),
                        heard.text,
                        heard.confidence,
                        t.elapsed().as_millis()
                    );
                    let Verdict::Accept(text) = assess(&heard) else {
                        panic!("{}: not accepted", tag.as_str());
                    };
                    assert!(text.to_lowercase().contains(expect), "{text}");
                    // Hints are recognition context only: they never appear
                    // as words that were not said.
                    assert!(!text.contains("Outlook") && !text.contains("Notepad"));
                }
            }
        }
        println!("[{backend:?}] memory after recognition: {} MB", memory_mb());

        // Silence must never become words (hints included).
        let silence = vec![0.0f32; RECOGNITION_RATE as usize * 2];
        let heard = engine
            .transcribe(
                &silence,
                &TranscribeOptions {
                    context: context.clone(),
                    ..TranscribeOptions::default()
                },
            )
            .expect("recognition");
        println!("[{backend:?}] silence → {:?}", heard.text);
        assert!(
            !matches!(assess(&heard), Verdict::Accept(_)),
            "silence produced an accepted transcript"
        );

        // A cancelled recognition is discarded.
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let cancelled = engine.transcribe(
            &clips.first().map_or(silence.clone(), |c| c.2.clone()),
            &TranscribeOptions {
                cancel: Some(cancel),
                ..TranscribeOptions::default()
            },
        );
        assert_eq!(cancelled.err(), Some(SttError::Cancelled));
    }
}

struct Collect(mpsc::Sender<Result<usize, CaptureError>>);

impl CaptureSink for Collect {
    fn audio(&mut self, interleaved: &[f32]) {
        let _ = self.0.send(Ok(interleaved.len()));
        let _ = rms_dbfs(interleaved);
    }
    fn failed(&mut self, error: CaptureError) {
        let _ = self.0.send(Err(error));
    }
}

/// Opens the default microphone for one second and releases it.
#[test]
#[ignore = "needs a microphone"]
fn default_microphone_captures_and_releases() {
    let platform = voice_platform();
    let (tx, rx) = mpsc::channel();
    let t0 = Instant::now();
    let (format, handle) = platform
        .capture
        .start(None, Box::new(Collect(tx)))
        .expect("microphone opens");
    println!(
        "opened {} Hz × {} in {} ms",
        format.sample_rate,
        format.channels,
        t0.elapsed().as_millis()
    );
    let mut samples = 0usize;
    let until = Instant::now() + Duration::from_secs(1);
    while Instant::now() < until {
        if let Ok(Ok(n)) = rx.recv_timeout(Duration::from_millis(100)) {
            samples += n;
        }
    }
    let t1 = Instant::now();
    handle.stop();
    println!("released in {} ms", t1.elapsed().as_millis());
    assert!(samples > 0, "no audio delivered");
    // A second session opens cleanly after release (no zombie stream).
    let (tx, _rx) = mpsc::channel();
    let (_, again) = platform
        .capture
        .start(None, Box::new(Collect(tx)))
        .expect("reopens");
    again.stop();
}

struct Done(mpsc::Sender<PlaybackEnd>);

impl PlaybackSink for Done {
    fn played(&mut self, _: &[f32]) {}
    fn finished(&mut self, end: PlaybackEnd) {
        let _ = self.0.send(end);
    }
}

/// Speaks a short sentence through the default output device.
#[test]
#[ignore = "plays audio"]
fn speech_plays_to_the_end_and_can_be_stopped() {
    let platform = voice_platform();
    let speech = platform
        .synthesis
        .synthesize(
            "SERSHI.",
            VoiceChoice {
                voice_id: None,
                language: None,
            },
        )
        .expect("synthesis");
    let (tx, rx) = mpsc::channel();
    let handle = platform
        .output
        .play(speech.clone(), Box::new(Done(tx)))
        .expect("plays");
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(10)),
        Ok(PlaybackEnd::Completed)
    );
    drop(handle);

    let (tx, rx) = mpsc::channel();
    let handle = platform
        .output
        .play(speech, Box::new(Done(tx)))
        .expect("plays");
    handle.stop();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)),
        Ok(PlaybackEnd::Stopped)
    );
}

/// Downloads the Fast profile model over HTTPS (WinHTTP) into a
/// temporary folder with SERSHI's verification, then deletes it.
#[test]
#[ignore = "downloads 264 MB from the internet"]
fn model_download_is_verified_and_atomic() {
    use std::sync::atomic::AtomicBool;

    use sershi_core::voice::{ModelState, SttModel};
    use sershi_platform::voice::download_model;

    let model = SttModel::find("whisper-small-q8").expect("catalog");
    let dir = std::env::temp_dir().join(format!("sershi-download-{}", std::process::id()));
    let store = ModelStore::new(&dir);
    let t0 = Instant::now();
    let mut last = 0;
    download_model(
        &store,
        model,
        &mut |received| last = received,
        &AtomicBool::new(false),
    )
    .expect("download verified");
    println!(
        "downloaded {} MB in {} s",
        last / 1_000_000,
        t0.elapsed().as_secs()
    );
    assert_eq!(last, model.size_bytes);
    assert_eq!(store.state(model), ModelState::Installed);
    assert!(store.verify(model).expect("readable"));
    // Cancelling before the first byte leaves nothing behind.
    store.discard(model);
    let cancelled = download_model(&store, model, &mut |_| {}, &AtomicBool::new(true));
    assert!(cancelled.is_err());
    assert_eq!(store.state(model), ModelState::NotInstalled);
    let _ = std::fs::remove_dir_all(dir);
}

/// Gate 3B regression: an early decode is aborted while the final decode
/// starts on the same engine (as when the user resumes speaking). Both must
/// end cleanly: the aborted one as Cancelled, the final one with a result.
#[test]
#[ignore = "needs SERSHI_STT_MODEL"]
fn an_aborted_decode_never_breaks_the_next_one() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let Some(model) = model_path() else {
        panic!("set SERSHI_STT_MODEL to an installed model file");
    };
    let (best, _) = speech_acceleration();
    let engine = load_recognizer(&model, best).expect("model loads");
    let platform = voice_platform();
    let speech = platform
        .synthesis
        .synthesize(
            "Abre la calculadora, por favor.",
            VoiceChoice {
                voice_id: None,
                language: None,
            },
        )
        .expect("synthesis");
    let pcm = Arc::new(resample(
        &speech.samples,
        speech.sample_rate,
        RECOGNITION_RATE,
    ));
    let mut failures = 0;
    for round in 0..8 {
        let cancel = Arc::new(AtomicBool::new(false));
        let early = {
            let (engine, pcm, cancel) = (engine.clone(), pcm.clone(), cancel.clone());
            std::thread::spawn(move || {
                engine.transcribe(
                    &pcm,
                    &TranscribeOptions {
                        cancel: Some(cancel),
                        ..TranscribeOptions::default()
                    },
                )
            })
        };
        std::thread::sleep(Duration::from_millis(40 * (round % 4)));
        cancel.store(true, Ordering::Relaxed);
        let t = Instant::now();
        let last = engine.transcribe(&pcm, &TranscribeOptions::default());
        let early = early.join().expect("thread");
        println!(
            "round {round}: early={:?} final={:?} in {} ms",
            early.as_ref().map(|t| t.text.clone()),
            last.as_ref().map(|t| t.text.clone()),
            t.elapsed().as_millis()
        );
        if !matches!(early, Ok(_) | Err(SttError::Cancelled)) || last.is_err() {
            failures += 1;
        }
    }
    assert_eq!(failures, 0, "concurrent decodes failed");
}
/// Gate 3B regression: decodes that carry a (never set) cancel flag — as
/// every real command does — must not fail.
#[test]
#[ignore = "needs SERSHI_STT_MODEL"]
fn an_unset_cancel_flag_never_aborts() {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    let Some(model) = model_path() else {
        panic!("set SERSHI_STT_MODEL to an installed model file");
    };
    let (best, _) = speech_acceleration();
    let engine = load_recognizer(&model, best).expect("model loads");
    let speech = voice_platform()
        .synthesis
        .synthesize(
            "Abre Outlook.",
            VoiceChoice {
                voice_id: None,
                language: None,
            },
        )
        .expect("synthesis");
    let pcm = resample(&speech.samples, speech.sample_rate, RECOGNITION_RATE);
    let mut failures = Vec::new();
    for round in 0..12 {
        let result = engine.transcribe(
            &pcm,
            &TranscribeOptions {
                context: (round % 2 == 0).then(|| "Chrome, Outlook, Notepad.".to_owned()),
                cancel: Some(Arc::new(AtomicBool::new(false))),
                ..TranscribeOptions::default()
            },
        );
        println!(
            "round {round}: {:?}",
            result.as_ref().map(|t| t.text.clone())
        );
        if let Err(e) = result {
            failures.push(e);
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
