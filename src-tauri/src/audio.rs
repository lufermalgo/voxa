use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, Ordering};
use rubato::{
    SincFixedIn, SincInterpolationParameters, SincInterpolationType,
    WindowFunction, Resampler,
};

pub struct SendStream(pub cpal::Stream);
unsafe impl Send for SendStream {}

pub struct AudioState {
    pub stream: Option<SendStream>,
    pub buffer: Arc<Mutex<Vec<f32>>>,
}

pub struct AudioEngine {
    pub state: Mutex<AudioState>,
    /// Current RMS level of the mic input (f32 bits stored as u32).
    /// Updated by the audio callback on every chunk (~10ms). Read by the
    /// level-polling thread to drive the real-time waveform animation.
    pub current_level: Arc<AtomicU32>,
}

impl AudioEngine {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(AudioState {
                stream: None,
                buffer: Arc::new(Mutex::new(Vec::new())),
            }),
            current_level: Arc::new(AtomicU32::new(0)),
        }
    }
}

#[derive(serde::Serialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub fn get_input_devices() -> Result<Vec<AudioDevice>, String> {
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(|e| e.to_string())?;
    let default_device = host.default_input_device().and_then(|d| d.name().ok());

    let mut result = Vec::new();
    for device in devices {
        let name = device.name().unwrap_or_else(|_| "Unknown Device".to_string());
        result.push(AudioDevice {
            id: name.clone(),
            is_default: Some(&name) == default_device.as_ref(),
            name,
        });
    }
    Ok(result)
}

pub fn get_default_input_device_name() -> Option<String> {
    let host = cpal::default_host();
    host.default_input_device().and_then(|d| d.name().ok())
}

pub fn setup_stream(engine: &AudioEngine, mic_id: Option<String>) -> Result<(), String> {
    let host = cpal::default_host();
    
    let device = if let Some(id) = mic_id {
        if id == "auto" {
            host.default_input_device().ok_or("No input device found")?
        } else {
            if let Ok(devices) = host.input_devices() {
                devices.into_iter().find(|d| d.name().unwrap_or_default() == id)
                       .unwrap_or_else(|| host.default_input_device().expect("No input device found"))
            } else {
                host.default_input_device().ok_or("No input device found")?
            }
        }
    } else {
        host.default_input_device().ok_or("No input device found")?
    };

    let config: cpal::SupportedStreamConfig = device.default_input_config().map_err(|e| e.to_string())?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    
    let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
    log::info!("Recording at {} Hz, {} channels from device: {}", sample_rate, channels, device_name);

    let buffer = {
        let state_lock = engine.state.lock().map_err(|e| e.to_string())?;
        let b = Arc::clone(&state_lock.buffer);
        {
            let mut b_lock = b.lock().map_err(|e| e.to_string())?;
            b_lock.clear();
        }
        b
    };

    let level_atomic = Arc::clone(&engine.current_level);

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let level = Arc::clone(&level_atomic);
            device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &_| {
                    if let Ok(mut b) = buffer.lock() {
                        b.extend_from_slice(data);
                    }
                    // Compute RMS of this chunk and publish for the animation thread
                    if !data.is_empty() {
                        let rms = (data.iter().map(|s| s * s).sum::<f32>() / data.len() as f32).sqrt();
                        level.store(rms.to_bits(), Ordering::Relaxed);
                    }
                },
                |err| log::error!("Stream error: {}", err),
                None,
            )
        },
        cpal::SampleFormat::I16 => {
            let level = Arc::clone(&level_atomic);
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &_| {
                    let f32_data: Vec<f32> = data.iter().map(|&x| x as f32 / i16::MAX as f32).collect();
                    if let Ok(mut b) = buffer.lock() {
                        b.extend_from_slice(&f32_data);
                    }
                    if !f32_data.is_empty() {
                        let rms = (f32_data.iter().map(|s| s * s).sum::<f32>() / f32_data.len() as f32).sqrt();
                        level.store(rms.to_bits(), Ordering::Relaxed);
                    }
                },
                |err| log::error!("Stream error: {}", err),
                None,
            )
        },
        _ => return Err(format!("Unsupported sample format: {:?}", config.sample_format())),
    }.map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;
    
    let mut state = engine.state.lock().map_err(|e| e.to_string())?;
    state.stream = Some(SendStream(stream));

    Ok(())
}

pub fn stop_stream(engine: &AudioEngine, mic_id: Option<String>) -> Result<Vec<f32>, String> {
    let host = cpal::default_host();
    
    let device = if let Some(id) = mic_id {
        if id == "auto" {
            host.default_input_device().ok_or("No input device found")?
        } else {
            if let Ok(devices) = host.input_devices() {
                devices.into_iter().find(|d| d.name().unwrap_or_default() == id)
                       .unwrap_or_else(|| host.default_input_device().expect("No input device found"))
            } else {
                host.default_input_device().ok_or("No input device found")?
            }
        }
    } else {
        host.default_input_device().ok_or("No input device found")?
    };
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let original_sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    let mut state = engine.state.lock().map_err(|e| e.to_string())?;
    if let Some(SendStream(stream)) = state.stream.take() {
        log::debug!("AUDIO: Explicitly pausing and dropping stream...");
        let _ = stream.pause();
        drop(stream);
    }

    let mut buffer = state.buffer.lock().map_err(|e| e.to_string())?;
    let data = std::mem::take(&mut *buffer);

    if data.is_empty() {
        log::debug!("STOP STREAM: Buffer is EMPTY!");
        return Ok(Vec::new());
    }

    log::debug!("STOP STREAM: Captured {} samples", data.len());
    let max = data.iter().fold(f32::MIN, |a, &b| a.max(b));
    let min = data.iter().fold(f32::MAX, |a, &b| a.min(b));
    let avg = data.iter().map(|&x| x.abs()).sum::<f32>() / data.len() as f32;
    log::debug!("STOP STREAM: Signal Stats - Max: {:.4}, Min: {:.4}, Avg Abs: {:.4}", max, min, avg);

    // 1. Mono conversion
    let mut mono_data = if channels > 1 {
        let mut mono = Vec::with_capacity(data.len() / channels);
        for chunk in data.chunks_exact(channels) {
            mono.push(chunk.iter().sum::<f32>() / channels as f32);
        }
        mono
    } else {
        data
    };

    // 2. Resampling to 16000Hz (required by Whisper) using sinc interpolation
    if original_sample_rate != 16000 {
        log::debug!("AUDIO: Resampling from {} to 16000 (sinc)", original_sample_rate);
        mono_data = resample_to_16k(mono_data, original_sample_rate)?;
    }

    // 3. Normalization: Whisper performs much better with standardized levels
    let max_abs = mono_data.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    if max_abs > 0.0 && max_abs < 0.2 {
        let factor = 0.6 / max_abs;
        log::debug!("AUDIO: Normalizing signal (Peak: {:.4} -> {:.2})", max_abs, 0.6);
        for x in mono_data.iter_mut() {
            *x *= factor;
        }
    }
    
    Ok(mono_data)
}

fn resample_to_16k(mono_data: Vec<f32>, source_rate: u32) -> Result<Vec<f32>, String> {
    if source_rate == 16000 {
        return Ok(mono_data);
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 128,
        window: WindowFunction::BlackmanHarris2,
    };

    let ratio = 16000.0 / source_rate as f64;
    let mut resampler = SincFixedIn::<f32>::new(
        ratio,
        2.0,
        params,
        mono_data.len(),
        1,
    ).map_err(|e| format!("Resampler init failed: {e}"))?;

    let waves_in = vec![mono_data];
    let waves_out = resampler.process(&waves_in, None)
        .map_err(|e| format!("Resample failed: {e}"))?;

    Ok(waves_out.into_iter().next().unwrap_or_default())
}
