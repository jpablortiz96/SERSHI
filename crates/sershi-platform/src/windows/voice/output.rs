//! Speech playback through the default WASAPI output device via `cpal`.
//!
//! The synthesized utterance is resampled once to the device rate and
//! played from memory. The sink hears exactly what was handed to the device
//! (for the Speaking visual) and is told once how playback ended, so the
//! Speaking state follows real playback rather than a timer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use sershi_core::voice::ports::{
    ActivePlayback, AudioOutputPort, PlaybackEnd, PlaybackError, PlaybackSink, SpeechAudio,
};
use sershi_core::voice::signal::resample;

const OPEN_TIMEOUT: Duration = Duration::from_secs(5);
/// Extra time after the last sample so the device buffer drains before the
/// stream is dropped (otherwise the last syllable is cut).
const DRAIN: Duration = Duration::from_millis(200);

#[derive(Debug, Default, Clone, Copy)]
pub struct WasapiOutput;

enum Signal {
    Stop,
    Done,
    Failed,
}

struct Shared {
    samples: Vec<f32>,
    position: usize,
}

impl AudioOutputPort for WasapiOutput {
    fn play(
        &self,
        audio: SpeechAudio,
        sink: Box<dyn PlaybackSink>,
    ) -> Result<Box<dyn ActivePlayback>, PlaybackError> {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (signal_tx, signal_rx) = mpsc::channel::<Signal>();
        let stopper = signal_tx.clone();
        let stopped = Arc::new(AtomicBool::new(false));
        let stopped_flag = stopped.clone();
        let thread = thread::Builder::new()
            .name("sershi-speech".to_owned())
            .spawn(move || {
                let mut sink = sink;
                let stream = match open(audio, signal_tx) {
                    Ok((stream, shared)) => {
                        let _ = ready_tx.send(Ok(()));
                        (stream, shared)
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        sink.finished(PlaybackEnd::Failed);
                        return;
                    }
                };
                let (stream, shared) = stream;
                let mut reported = 0usize;
                let end = loop {
                    match signal_rx.recv_timeout(Duration::from_millis(40)) {
                        Ok(Signal::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                            break PlaybackEnd::Stopped;
                        }
                        Ok(Signal::Failed) => break PlaybackEnd::Failed,
                        Ok(Signal::Done) => {
                            report(&shared, &mut reported, sink.as_mut());
                            thread::sleep(DRAIN);
                            break if stopped_flag.load(Ordering::Relaxed) {
                                PlaybackEnd::Stopped
                            } else {
                                PlaybackEnd::Completed
                            };
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            report(&shared, &mut reported, sink.as_mut());
                        }
                    }
                };
                let _ = stream.pause();
                drop(stream);
                sink.finished(end);
            })
            .map_err(|e| PlaybackError::Failed(e.to_string()))?;
        match ready_rx.recv_timeout(OPEN_TIMEOUT) {
            Ok(Ok(())) => Ok(Box::new(RunningPlayback {
                stop: Some(stopper),
                stopped,
                thread: Some(thread),
            })),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(PlaybackError::NoOutput),
        }
    }
}

/// Hands newly played samples to the sink (off the audio thread).
fn report(shared: &Mutex<Shared>, reported: &mut usize, sink: &mut dyn PlaybackSink) {
    let block = match shared.lock() {
        Ok(s) if s.position > *reported => {
            let block = s.samples[*reported..s.position].to_vec();
            *reported = s.position;
            block
        }
        _ => return,
    };
    sink.played(&block);
}

type Opened = (cpal::Stream, Arc<Mutex<Shared>>);

fn open(audio: SpeechAudio, signal: mpsc::Sender<Signal>) -> Result<Opened, PlaybackError> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or(PlaybackError::NoOutput)?;
    let supported = device
        .default_output_config()
        .map_err(|_| PlaybackError::NoOutput)?;
    let config: StreamConfig = supported.config();
    let samples = resample(&audio.samples, audio.sample_rate, config.sample_rate);
    let shared = Arc::new(Mutex::new(Shared {
        samples,
        position: 0,
    }));
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, shared.clone(), signal, |s| s),
        SampleFormat::I16 => build::<i16>(&device, config, shared.clone(), signal, |s| {
            (s.clamp(-1.0, 1.0) * 32_767.0) as i16
        }),
        SampleFormat::I32 => build::<i32>(&device, config, shared.clone(), signal, |s| {
            (f64::from(s.clamp(-1.0, 1.0)) * 2_147_483_647.0) as i32
        }),
        _ => Err(PlaybackError::Failed(
            "unsupported output format".to_owned(),
        )),
    }?;
    stream
        .play()
        .map_err(|e| PlaybackError::Failed(e.to_string()))?;
    Ok((stream, shared))
}

fn build<T: cpal::SizedSample + Send + 'static>(
    device: &cpal::Device,
    config: StreamConfig,
    shared: Arc<Mutex<Shared>>,
    signal: mpsc::Sender<Signal>,
    convert: fn(f32) -> T,
) -> Result<cpal::Stream, PlaybackError> {
    let channels = usize::from(config.channels.max(1));
    let error_signal = signal.clone();
    let mut done = false;
    device
        .build_output_stream::<T, _, _>(
            config,
            move |out: &mut [T], _| {
                // The buffer arrives pre-filled with silence.
                let Ok(mut s) = shared.lock() else {
                    return;
                };
                for frame in out.chunks_mut(channels) {
                    let Some(&sample) = s.samples.get(s.position) else {
                        break;
                    };
                    s.position += 1;
                    let value = convert(sample);
                    for slot in frame.iter_mut() {
                        *slot = value;
                    }
                }
                if !done && s.position >= s.samples.len() {
                    done = true;
                    let _ = signal.send(Signal::Done);
                }
            },
            move |_| {
                let _ = error_signal.send(Signal::Failed);
            },
            Some(OPEN_TIMEOUT),
        )
        .map_err(|e| PlaybackError::Failed(e.to_string()))
}

#[derive(Debug)]
struct RunningPlayback {
    stop: Option<mpsc::Sender<Signal>>,
    stopped: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl RunningPlayback {
    fn release(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(Signal::Stop);
        }
        if let Some(thread) = self.thread.take() {
            // The sink may drop this handle from the playback thread itself
            // (in `finished`); never join ourselves.
            if thread.thread().id() != thread::current().id() {
                let _ = thread.join();
            }
        }
    }
}

impl ActivePlayback for RunningPlayback {
    fn stop(mut self: Box<Self>) {
        self.release();
    }
}

impl Drop for RunningPlayback {
    fn drop(&mut self) {
        self.release();
    }
}
