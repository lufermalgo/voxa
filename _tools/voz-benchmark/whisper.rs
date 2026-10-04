#[path = "../../src-tauri/src/whisper_inference.rs"]
mod whisper_inference;
use std::{io::{self, BufRead}, path::Path, time::Instant};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = std::env::args().nth(1).ok_or("model path required")?;
    let start = Instant::now();
    let mut engine = whisper_inference::WhisperEngine::new(Path::new(&model))?;
    println!("{}", json!({"ready": true, "load_ms": start.elapsed().as_secs_f64()*1000.0}));
    for line in io::stdin().lock().lines() {
        let result = (|| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
            let request: serde_json::Value = serde_json::from_str(&line?)?;
            let path = request["pcm"].as_str().ok_or("pcm path required")?;
            let bytes = std::fs::read(path)?;
            if bytes.is_empty() || bytes.len() % 4 != 0 { return Err("invalid float32 PCM".into()); }
            let samples: Vec<f32> = bytes.chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
            let start = Instant::now();
            let text = engine.transcribe(&samples, "de", "")?;
            Ok(json!({"text": text, "stt_ms": start.elapsed().as_secs_f64()*1000.0}))
        })();
        println!("{}", result.unwrap_or_else(|e| json!({"error": e.to_string()})));
    }
    Ok(())
}
