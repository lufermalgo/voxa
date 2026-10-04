// Minimal data types for importing the production helpers without the Tauri UI.
mod models {
    pub struct ModelManager { pub base_path: std::path::PathBuf }
    impl ModelManager {
        pub fn get_llama_path(&self) -> std::path::PathBuf {
            self.base_path.join("qwen2.5-1.5b-instruct-q4_k_m.gguf")
        }
    }
}
#[allow(dead_code)] mod db {
    pub struct FormattingHint {
        pub id: i64, pub profile_id: i64, pub pattern: String,
        pub hint: String, pub frequency: i64, pub is_promoted: bool,
    }
}
#[path = "../../src-tauri/src/proc.rs"] mod proc;
#[path = "../../src-tauri/src/formatting.rs"] mod formatting;
#[path = "../../src-tauri/src/llama_inference.rs"] mod llama_inference;
use std::{io::{self, BufRead}, path::Path, time::Instant};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 { return Err("model, server, profile text and PID file required".into()); }
    let profile = std::fs::read_to_string(&args[3])?;
    let prompt = format!("{}\n\n{}", profile.trim(), formatting::build_formatting_block("plain", "de", &[]));
    let start = Instant::now();
    let mut engine = llama_inference::LlamaEngine::new(Path::new(&args[1]), Path::new(&args[2]), Some(args[4].clone().into()))?;
    println!("{}", json!({"ready": true, "load_ms": start.elapsed().as_secs_f64()*1000.0, "server_pid": engine.pid(), "system_prompt": prompt}));
    for line in io::stdin().lock().lines() {
        let result = (|| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
            let request: serde_json::Value = serde_json::from_str(&line?)?;
            let text = request["text"].as_str().ok_or("text required")?;
            let start = Instant::now();
            let refined = engine.refine_text(text, &prompt, "de", "", "")?;
            Ok(json!({"text": refined, "llm_ms": start.elapsed().as_secs_f64()*1000.0}))
        })();
        println!("{}", result.unwrap_or_else(|e| json!({"error": e.to_string()})));
    }
    engine.terminate();
    Ok(())
}
