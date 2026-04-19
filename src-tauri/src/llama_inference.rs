use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Finds a free localhost port by asking the OS to assign one.
fn find_free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind to find a free port");
    listener.local_addr().unwrap().port()
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

        let mut cmd = Command::new(server_path);
        cmd.arg("--model").arg(model_path)
            .arg("--port").arg(port.to_string())
            .arg("--host").arg("127.0.0.1")
            .arg("-ngl").arg("99")       // offload all layers to GPU (Metal/CUDA); CPU fallback is automatic
            .arg("--ctx-size").arg("4096")
            .arg("--log-disable")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        // Flash Attention and memory locking are Metal-specific optimizations.
        // --flash-attn: 20-40% speedup on attention layers via Metal.
        // --mlock: prevents model weights from being paged out under macOS memory pressure.
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            cmd.arg("--flash-attn");
            cmd.arg("--mlock");
        }

        let mut process = cmd.spawn()
            .map_err(|e| format!("Failed to start llama-server: {}", e))?;

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| format!("HTTP client build failed: {}", e))?;

        // Poll /health until the server is ready (model fully loaded into GPU/CPU memory).
        // On M3 with Metal, Qwen2.5-1.5B Q4_K_M loads in ~1-2s.
        let health_url = format!("http://127.0.0.1:{}/health", port);
        let mut ready = false;
        let start = std::time::Instant::now();
        for attempt in 0..240 {
            std::thread::sleep(Duration::from_millis(500));
            let ok = client.get(&health_url).send()
                .map(|r| r.status().is_success())
                .unwrap_or(false);
            if ok {
                let elapsed = start.elapsed().as_secs_f32();
                log::info!("LlamaEngine ready in {:.1}s (attempt {})", elapsed, attempt);
                ready = true;
                break;
            }
        }

        if !ready {
            let _ = process.kill();
            return Err(format!(
                "llama-server failed to become ready within 120s (port {})", port
            ));
        }

        log::info!("LlamaServer ready on port {}", port);
        Ok(Self { _process: process, port, client })
    }

    /// Sends the transcription + profile system prompt to the running llama-server
    /// and returns the transformed text.
    ///
    /// `pre_text` and `post_text` are the text immediately before/after the cursor
    /// in the target application at the time recording started. When non-empty they
    /// are injected into the user message so the model can match capitalization, tone,
    /// and spacing to the surrounding document context. Pass empty strings to get the
    /// same behaviour as before this feature was added.
    pub fn refine_text(
        &mut self,
        text: &str,
        system_prompt: &str,
        pre_text: &str,
        post_text: &str,
    ) -> Result<String, String> {
        if system_prompt.is_empty() {
            return Ok(text.to_string());
        }

        // Build the user message. When cursor context is available, wrap the
        // transcription in XML tags alongside the surrounding text so the model
        // can match capitalization, spacing, and tone to the document.
        let user_message = if pre_text.is_empty() && post_text.is_empty() {
            text.to_string()
        } else {
            format!(
                "<before_text>{}</before_text>\n<transcription>{}</transcription>\n<after_text>{}</after_text>\n\nOutput ONLY the formatted transcription that should be inserted at the cursor. Match capitalization, spacing, and tone to the surrounding text. Do not include the before_text or after_text in your response.",
                pre_text, text, post_text
            )
        };

        // ChatML format — compatible with Qwen2.5-Instruct and most modern instruct models.
        // The language guard prevents Qwen from translating the output when the system prompt
        // is written in a different language than the user's dictation.
        let prompt = format!(
            "<|im_start|>system\nIMPORTANT: Always respond in the SAME language as the user's text. Never translate.\n\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
            system_prompt, user_message
        );

        let body = serde_json::json!({
            "prompt": prompt,
            "n_predict": 1200,
            "temperature": 0.0,
            "stop": ["<|im_end|>", "<|endoftext|>", "<|im_start|>"],
            "stream": false,
            "cache_prompt": true
        });

        let url = format!("http://127.0.0.1:{}/completion", self.port);

        let response = self.client.post(&url).json(&body).send()
            .map_err(|e| format!("llama-server request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("llama-server returned HTTP {}", response.status()));
        }

        let json: serde_json::Value = response.json()
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
