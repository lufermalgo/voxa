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
    let default_device = host.default_input_device().map(|d| d.name().unwrap_or_default());

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

pub fn setup_stream(engine: &AudioEngine, mic_id: Option<String>) -> Result<(), String> {
    let host = cpal::default_host();
    
    let device = if let Some(id) = mic_id {
        if let Ok(devices) = host.input_devices() {
            devices.filter_map(|d| if d.name().unwrap_or_default() == id { Some(d) } else { None })
                   .next()
                   .unwrap_or_else(|| host.default_input_device().expect("No input device found"))
        } else {
            host.default_input_device().ok_or("No input device found")?
        }
    } else {
        host.default_input_device().ok_or("No input device found")?
    };

    let config: cpal::SupportedStreamConfig = device.default_input_config().map_err(|e| e.to_string())?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    
    println!("Recording at {} Hz, {} channels", sample_rate, channels);

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
        if let Ok(devices) = host.input_devices() {
            devices.filter_map(|d| if d.name().unwrap_or_default() == id { Some(d) } else { None })
                   .next()
                   .unwrap_or_else(|| host.default_input_device().expect("No input device found"))
        } else {
            host.default_input_device().ok_or("No input device found")?
        }
    } else {
        host.default_input_device().ok_or("No input device found")?
    };
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let original_sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    let mut state = engine.state.lock().map_err(|e| e.to_string())?;
    state.stream = None; 
    
    let mut buffer = state.buffer.lock().map_err(|e| e.to_string())?;
    let data = std::mem::take(&mut *buffer);
    
    if data.is_empty() {
        return Ok(Vec::new());
    }

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

    // 2. Resampling to 16000Hz (required by Whisper)
    if original_sample_rate != 16000 {
        println!("Resampling from {} to 16000", original_sample_rate);
        let mut resampled = Vec::new();
        let ratio = original_sample_rate as f32 / 16000.0;
        let mut i = 0.0;
        while (i as usize) < mono_data.len() {
            resampled.push(mono_data[i as usize]);
            i += ratio;
        }
        mono_data = resampled;
    }
    
    Ok(mono_data)
}
