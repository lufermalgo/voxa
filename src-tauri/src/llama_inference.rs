use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Finds a free localhost port starting from the given base.
fn find_free_port() -> u16 {
    (18474u16..18600)
        .find(|&p| TcpListener::bind(format!("127.0.0.1:{}", p)).is_ok())
        .unwrap_or(18474)
}

pub struct LlamaEngine {
    _process: Child,
    port: u16,
    client: reqwest::blocking::Client,
}

impl LlamaEngine {
    /// Spawns `llama-server` with the given model and waits until it reports healthy.
    /// The server stays alive for the lifetime of this struct (killed on Drop).
    pub fn new(model_path: &Path, server_path: &Path) -> Result<Self, String> {
        let port = find_free_port();

        let mut process = Command::new(server_path)
            .arg("--model").arg(model_path)
            .arg("--port").arg(port.to_string())
            .arg("--host").arg("127.0.0.1")
            .arg("-ngl").arg("99")       // offload all layers to GPU (Metal/CUDA); CPU fallback is automatic
            .arg("--ctx-size").arg("8192")
            .arg("--log-disable")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start llama-server: {}", e))?;

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| format!("HTTP client build failed: {}", e))?;

        // Poll /health until the server is ready (model fully loaded into GPU/CPU memory).
        // On M3 with Metal, SmolLM2-1.7B Q4_K_M loads in ~1-2s.
        let health_url = format!("http://127.0.0.1:{}/health", port);
        let mut ready = false;
        for _ in 0..60 {
            std::thread::sleep(Duration::from_millis(500));
            if let Ok(resp) = client.get(&health_url).send() {
                if resp.status().is_success() {
                    ready = true;
                    break;
                }
            }
        }

        if !ready {
            let _ = process.kill();
            return Err(format!(
                "llama-server failed to become ready within 30s (port {})", port
            ));
        }

        println!("[LlamaServer] Ready on port {}", port);
        Ok(Self { _process: process, port, client })
    }

    /// Sends the transcription + profile system prompt to the running llama-server
    /// and returns the transformed text.
    pub fn refine_text(&mut self, text: &str, system_prompt: &str) -> Result<String, String> {
        if system_prompt.is_empty() {
            return Ok(text.to_string());
        }

        // ChatML format — compatible with Qwen2.5-Instruct and most modern instruct models
        let prompt = format!(
            "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
            system_prompt, text
        );

        let body = serde_json::json!({
            "prompt": prompt,
            "n_predict": 600,
            "temperature": 0.0,
            "stop": ["<|im_end|>", "<|endoftext|>", "<|im_start|>"],
            "stream": false,
            "cache_prompt": true
        });

        let url = format!("http://127.0.0.1:{}/completion", self.port);
        let resp = self.client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| format!("llama-server request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("llama-server returned HTTP {}", resp.status()));
        }

        let json: serde_json::Value = resp.json()
            .map_err(|e| format!("llama-server response parse failed: {}", e))?;

        let content = json["content"]
            .as_str()
            .ok_or_else(|| "llama-server response missing 'content' field".to_string())?
            .trim()
            .to_string();

        Ok(content)
    }
}

impl Drop for LlamaEngine {
    fn drop(&mut self) {
        let _ = self._process.kill();
        let _ = self._process.wait();
    }
}
