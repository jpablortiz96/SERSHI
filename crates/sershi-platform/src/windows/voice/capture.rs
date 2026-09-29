//! Microphone capture through WASAPI (shared mode) via `cpal`.
//!
//! Each capture owns a dedicated thread that builds the stream, keeps it
//! alive and drops it on stop, so the microphone is released
//! deterministically — on stop, on drop of the handle, or if the thread's
//! owner goes away. Audio is delivered to the sink in memory only.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, DeviceId, ErrorKind, SampleFormat, StreamConfig};
use sershi_core::voice::ports::{
    ActiveCapture, AudioCapturePort, CaptureError, CaptureFormat, CaptureSink, InputDevice,
};

/// How long opening a device may take before SERSHI gives up.
const OPEN_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Default, Clone, Copy)]
pub struct WasapiCapture;

fn device_id(device: &Device) -> Option<String> {
    device.id().ok().map(|id| id.to_string())
}

fn device_name(device: &Device) -> String {
    device
        .description()
        .map(|d| d.name().to_owned())
        .unwrap_or_default()
}

fn map_error(error: &cpal::Error) -> CaptureError {
    match error.kind() {
        ErrorKind::PermissionDenied => CaptureError::PermissionDenied,
        ErrorKind::DeviceNotAvailable | ErrorKind::DeviceBusy | ErrorKind::StreamInvalidated => {
            CaptureError::DeviceUnavailable
        }
        ErrorKind::UnsupportedConfig => CaptureError::UnsupportedFormat,
        _ => CaptureError::Failed(error.to_string()),
    }
}

fn find_device(requested: Option<&str>) -> Result<Device, CaptureError> {
    let host = cpal::default_host();
    match requested {
        None => host.default_input_device().ok_or(CaptureError::NoDevice),
        Some(id) => {
            let id: DeviceId = id.parse().map_err(|_| CaptureError::DeviceNotFound)?;
            host.device_by_id(&id).ok_or(CaptureError::DeviceNotFound)
        }
    }
}

impl AudioCapturePort for WasapiCapture {
    fn input_devices(&self) -> Result<Vec<InputDevice>, CaptureError> {
        let host = cpal::default_host();
        let default = host.default_input_device().and_then(|d| device_id(&d));
        let devices = host.input_devices().map_err(|e| map_error(&e))?;
        Ok(devices
            .filter_map(|d| {
                let id = device_id(&d)?;
                Some(InputDevice {
                    is_default: default.as_deref() == Some(id.as_str()),
                    name: device_name(&d),
                    id,
                })
            })
            .collect())
    }

    fn start(
        &self,
        device: Option<&str>,
        sink: Box<dyn CaptureSink>,
    ) -> Result<(CaptureFormat, Box<dyn ActiveCapture>), CaptureError> {
        let requested = device.map(str::to_owned);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let thread = thread::Builder::new()
            .name("sershi-mic".to_owned())
            .spawn(move || {
                let opened = open(requested.as_deref(), sink);
                match opened {
                    Ok((format, stream)) => {
                        let _ = ready_tx.send(Ok(format));
                        // Keep the stream alive until stop (or the handle is
                        // dropped, which disconnects the channel).
                        let _ = stop_rx.recv();
                        let _ = stream.pause();
                        drop(stream);
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                    }
                }
            })
            .map_err(|e| CaptureError::Failed(e.to_string()))?;
        match ready_rx.recv_timeout(OPEN_TIMEOUT) {
            Ok(Ok(format)) => Ok((
                format,
                Box::new(RunningCapture {
                    stop: Some(stop_tx),
                    thread: Some(thread),
                }),
            )),
            Ok(Err(error)) => Err(error),
            // The thread will release the device when it notices the
            // dropped channel.
            Err(_) => Err(CaptureError::DeviceUnavailable),
        }
    }
}

fn open(
    requested: Option<&str>,
    sink: Box<dyn CaptureSink>,
) -> Result<(CaptureFormat, cpal::Stream), CaptureError> {
    let device = find_device(requested)?;
    let supported = device.default_input_config().map_err(|e| map_error(&e))?;
    let config: StreamConfig = supported.config();
    let format = CaptureFormat {
        sample_rate: config.sample_rate,
        channels: config.channels,
    };
    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, sink, |s| s),
        SampleFormat::I16 => build::<i16>(&device, config, sink, |s| f32::from(s) / 32_768.0),
        SampleFormat::I32 => build::<i32>(&device, config, sink, |s| s as f32 / 2_147_483_648.0),
        _ => Err(CaptureError::UnsupportedFormat),
    }?;
    stream.play().map_err(|e| map_error(&e))?;
    Ok((format, stream))
}

fn build<T: cpal::SizedSample + 'static>(
    device: &Device,
    config: StreamConfig,
    sink: Box<dyn CaptureSink>,
    convert: fn(T) -> f32,
) -> Result<cpal::Stream, CaptureError> {
    // The sink is shared between the data and error callbacks.
    let sink = std::sync::Arc::new(std::sync::Mutex::new(sink));
    let data_sink = sink.clone();
    let mut scratch: Vec<f32> = Vec::new();
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _| {
                scratch.clear();
                scratch.extend(data.iter().map(|s| convert(*s)));
                if let Ok(mut sink) = data_sink.lock() {
                    sink.audio(&scratch);
                }
            },
            move |error| {
                // Rerouting to a new default device keeps capturing.
                if error.kind() == ErrorKind::DeviceChanged || error.kind() == ErrorKind::Xrun {
                    return;
                }
                if let Ok(mut sink) = sink.lock() {
                    sink.failed(match map_error(&error) {
                        CaptureError::Failed(_) => CaptureError::DeviceUnavailable,
                        other => other,
                    });
                }
            },
            Some(OPEN_TIMEOUT),
        )
        .map_err(|e| map_error(&e))
}

#[derive(Debug)]
struct RunningCapture {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl RunningCapture {
    fn release(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl ActiveCapture for RunningCapture {
    fn stop(mut self: Box<Self>) {
        self.release();
    }
}

impl Drop for RunningCapture {
    fn drop(&mut self) {
        self.release();
    }
}
