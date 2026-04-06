use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

pub struct SendStream(pub cpal::Stream);
unsafe impl Send for SendStream {}

pub struct AudioState {
    pub stream: Option<SendStream>,
    pub buffer: Arc<Mutex<Vec<f32>>>,
}

pub struct AudioEngine {
    pub state: Mutex<AudioState>,
}

impl AudioEngine {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(AudioState {
                stream: None,
                buffer: Arc::new(Mutex::new(Vec::new())),
            }),
        }
    }
}

#[derive(serde::Serialize)]
pub struct AudioDevice {
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
    println!("Recording at {} Hz, {} channels from device: {}", sample_rate, channels, device_name);

    let buffer = {
        let state_lock = engine.state.lock().map_err(|e| e.to_string())?;
        let b = Arc::clone(&state_lock.buffer);
        {
            let mut b_lock = b.lock().map_err(|e| e.to_string())?;
            b_lock.clear();
        }
        b
    };

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &_| {
                if let Ok(mut b) = buffer.lock() {
                    b.extend_from_slice(data);
                }
            },
            |err| eprintln!("Stream error: {}", err),
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _: &_| {
                if let Ok(mut b) = buffer.lock() {
                    let f32_data: Vec<f32> = data.iter().map(|&x| x as f32 / i16::MAX as f32).collect();
                    b.extend_from_slice(&f32_data);
                }
            },
            |err| eprintln!("Stream error: {}", err),
            None,
        ),
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
        println!("AUDIO: Explicitly pausing and dropping stream...");
        let _ = stream.pause();
        drop(stream);
    }
    
    let mut buffer = state.buffer.lock().map_err(|e| e.to_string())?;
    let data = std::mem::take(&mut *buffer);
    
    if data.is_empty() {
        println!("STOP STREAM: Buffer is EMPTY!");
        return Ok(Vec::new());
    }

    println!("STOP STREAM: Captured {} samples", data.len());
    let max = data.iter().fold(f32::MIN, |a, &b| a.max(b));
    let min = data.iter().fold(f32::MAX, |a, &b| a.min(b));
    let avg = data.iter().map(|&x| x.abs()).sum::<f32>() / data.len() as f32;
    println!("STOP STREAM: Signal Stats - Max: {:.4}, Min: {:.4}, Avg Abs: {:.4}", max, min, avg);

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

    // 2. Resampling to 16000Hz (required by Whisper) with anti-aliasing (box filter)
    if original_sample_rate != 16000 {
        println!("AUDIO: Resampling from {} to 16000", original_sample_rate);
        let mut resampled = Vec::new();
        let ratio = original_sample_rate as f32 / 16000.0;
        
        let mut i = 0.0;
        while (i as usize) < mono_data.len() {
            let start = i as usize;
            let end = ((i + ratio).min(mono_data.len() as f32)) as usize;
            
            if end > start {
                // Averaging samples (Box Filter) to avoid aliasing
                let sum: f32 = mono_data[start..end].iter().sum();
                resampled.push(sum / (end - start) as f32);
            } else {
                resampled.push(mono_data[start]);
            }
            i += ratio;
        }
        mono_data = resampled;
    }

    // 3. Normalization: Whisper performs much better with standardized levels
    let max_abs = mono_data.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    if max_abs > 0.0 && max_abs < 0.2 {
        let factor = 0.6 / max_abs;
        println!("AUDIO: Normalizing signal (Peak: {:.4} -> {:.2})", max_abs, 0.6);
        for x in mono_data.iter_mut() {
            *x *= factor;
        }
    }
    
    Ok(mono_data)
}
