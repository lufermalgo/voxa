use std::path::PathBuf;
use std::fs;
use tauri::{AppHandle, Manager, Emitter, State};
use futures_util::StreamExt;
use reqwest::Client;
use std::io::Write;
use scopeguard;

// Apple Silicon always has Metal GPU — no runtime check needed.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn detect_gpu() -> bool { true }

// Intel Mac: no Metal compute for LLMs worth using.
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn detect_gpu() -> bool { false }

// Windows / Linux: check for nvidia-smi in PATH.
#[cfg(not(target_os = "macos"))]
fn detect_gpu() -> bool {
    std::process::Command::new("nvidia-smi")
        .arg("--query-gpu=name")
        .arg("--format=csv,noheader")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[derive(serde::Serialize, Clone)]
pub struct DownloadProgress {
    pub model: String,
    pub progress: f64,
    pub total: u64,
    pub current: u64,
}

#[derive(serde::Serialize, Clone)]
pub struct ModelDetail {
    pub display_name: String,
    pub filename: String,
    pub path: String,
    pub size_mb: f64,
    pub downloaded: bool,
}

#[derive(serde::Serialize, Clone)]
pub struct ModelsStateInfo {
    pub base_path: String,
    pub models: Vec<ModelDetail>,
}

pub struct ModelManager {
    pub base_path: PathBuf,
    pub is_downloading: std::sync::atomic::AtomicBool,
    pub gpu_available: bool,
}

impl ModelManager {
    pub fn new<R: tauri::Runtime>(app_handle: &tauri::AppHandle<R>) -> Result<Self, String> {
        let app_dir = app_handle.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
        let models_dir = app_dir.join("models");
        if !models_dir.exists() {
            fs::create_dir_all(&models_dir).map_err(|e| format!("Failed to create models directory: {}", e))?;
        }
        let gpu_available = detect_gpu();
        println!("[ModelManager] GPU detected: {}", gpu_available);
        Ok(Self {
            base_path: models_dir,
            is_downloading: std::sync::atomic::AtomicBool::new(false),
            gpu_available,
        })
    }

    pub fn get_whisper_path(&self) -> PathBuf {
        self.base_path.join("ggml-small.bin")
    }

    pub fn get_llama_filename(&self) -> &'static str {
        if self.gpu_available {
            "qwen2.5-3b-instruct-q4_k_m.gguf"
        } else {
            "qwen2.5-1.5b-instruct-q4_k_m.gguf"
        }
    }

    pub fn get_llama_display_name(&self) -> &'static str {
        if self.gpu_available {
            "Qwen2.5-3B (Refinement, GPU)"
        } else {
            "Qwen2.5-1.5B (Refinement, CPU)"
        }
    }

    /// Short name used in download progress pill — keep it brief.
    pub fn get_llama_short_name(&self) -> &'static str {
        if self.gpu_available { "Qwen2.5-3B" } else { "Qwen2.5-1.5B" }
    }

    pub fn get_llama_download_url(&self) -> &'static str {
        if self.gpu_available {
            "https://huggingface.co/bartowski/Qwen2.5-3B-Instruct-GGUF/resolve/main/Qwen2.5-3B-Instruct-Q4_K_M.gguf"
        } else {
            "https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf"
        }
    }

    pub fn get_llama_path(&self) -> PathBuf {
        self.base_path.join(self.get_llama_filename())
    }

    pub fn get_llama_server_path(&self) -> PathBuf {
        #[cfg(target_os = "windows")]
        return self.base_path.join("llama-server.exe");
        #[cfg(not(target_os = "windows"))]
        return self.base_path.join("llama-server");
    }

    /// Returns the path to the usable llama-server binary, in priority order:
    /// 1. Our downloaded binary (app data dir, Windows/Linux)
    /// 2. `brew install llama.cpp` on macOS — verified via Cellar symlink, NOT `brew install ggml`
    pub fn get_effective_llama_server(&self) -> Option<PathBuf> {
        let downloaded = self.get_llama_server_path();
        if downloaded.exists() {
            return Some(downloaded);
        }

        #[cfg(target_os = "macos")]
        {
            let candidates = [
                "/opt/homebrew/bin/llama-server",  // Apple Silicon
                "/usr/local/bin/llama-server",      // Intel
            ];
            for candidate in candidates {
                let path = PathBuf::from(candidate);
                if !path.exists() {
                    continue;
                }
                if let Ok(real) = std::fs::canonicalize(&path) {
                    let real_str = real.to_string_lossy();
                    if real_str.contains("/Cellar/llama.cpp/") {
                        return Some(path);
                    }
                }
            }
        }

        None
    }

    pub fn models_exist(&self) -> bool {
        self.get_whisper_path().exists()
            && self.get_llama_path().exists()
            && self.get_effective_llama_server().is_some()
    }
}

#[tauri::command]
pub async fn get_models_info(manager: State<'_, ModelManager>) -> Result<ModelsStateInfo, String> {
    let models_meta = vec![
        ("Whisper Small (General)", "ggml-small.bin".to_string()),
        (manager.get_llama_display_name(), manager.get_llama_filename().to_string()),
    ];

    let mut models = Vec::new();

    for (display_name, filename) in models_meta {
        let path = manager.base_path.join(&filename);
        let exists = path.exists();
        let size_mb = if exists {
            fs::metadata(&path).map(|m| m.len() as f64 / 1_048_576.0).unwrap_or(0.0)
        } else {
            0.0
        };

        models.push(ModelDetail {
            display_name: display_name.to_string(),
            filename,
            path: path.to_string_lossy().to_string(),
            size_mb: (size_mb * 100.0).round() / 100.0,
            downloaded: exists && size_mb > 1.0,
        });
    }

    Ok(ModelsStateInfo {
        base_path: manager.base_path.to_string_lossy().to_string(),
        models,
    })
}

#[tauri::command]
pub async fn open_models_folder(manager: State<'_, ModelManager>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&manager.base_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn check_models_status(manager: State<'_, ModelManager>) -> Result<bool, String> {
    Ok(manager.models_exist())
}

#[tauri::command]
pub async fn download_models(app_handle: AppHandle, manager: State<'_, ModelManager>) -> Result<(), String> {
    // Prevent multiple concurrent downloads
    if manager.is_downloading.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Ok(());
    }

    // Ensure we reset the flag even if we error out
    let _guard = scopeguard::guard((), |_| {
        manager.is_downloading.store(false, std::sync::atomic::Ordering::SeqCst);
    });

    let client = Client::new();

    let llama_filename = manager.get_llama_filename();
    let llama_short = manager.get_llama_short_name();
    let llama_url = manager.get_llama_download_url();

    let models = vec![
        ("ggml-small.bin", "Whisper", "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"),
        (llama_filename, llama_short, llama_url),
    ];

    let current_model_names: std::collections::HashSet<&str> = models.iter().map(|(name, _, _)| *name).collect();

    for (name, display_name, url) in &models {
        let target_path = manager.base_path.join(name);
        if target_path.exists() {
            let meta = fs::metadata(&target_path).map_err(|e| e.to_string())?;
            if meta.len() > 1_000_000 {
                continue;
            } else {
                println!("[WARN] {} is incomplete, re-downloading.", name);
                fs::remove_file(&target_path).map_err(|e| e.to_string())?;
            }
        }

        let temp_path = manager.base_path.join(format!("{}.download", name));
        println!("[DOWNLOAD] {} …", display_name);
        
        let response = client.get(*url).send().await.map_err(|e| e.to_string())?;
        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        {
            let mut file = fs::File::create(&temp_path).map_err(|e| e.to_string())?;
            while let Some(item) = stream.next().await {
                let chunk = item.map_err(|e| e.to_string())?;
                file.write_all(&chunk).map_err(|e| e.to_string())?;
                downloaded += chunk.len() as u64;

                if total_size > 0 {
                    let progress = (downloaded as f64 / total_size as f64) * 100.0;
                    app_handle.emit("download-progress", DownloadProgress {
                        model: display_name.to_string(),
                        progress,
                        total: total_size,
                        current: downloaded,
                    }).unwrap_or_default();
                }
            }
            file.flush().map_err(|e| e.to_string())?;
        }
        
        fs::rename(&temp_path, &target_path).map_err(|e| e.to_string())?;
        println!("[DOWNLOAD] {} ready.", display_name);
    }

    // Cleanup stale model files
    if let Ok(entries) = fs::read_dir(&manager.base_path) {
        for entry in entries.flatten() {
            if let Some(file_name) = entry.file_name().to_str() {
                if !current_model_names.contains(file_name) && !file_name.starts_with('.')
                    && !file_name.ends_with(".download") && file_name != "llama-server" {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    // Download llama.cpp binary if needed
    if let Err(e) = download_llama_server(&manager, &app_handle, &client).await {
        eprintln!("[ERROR] llama-server download failed: {}", e);
        return Err(e);
    }

    println!("[INFO] All models ready.");
    app_handle.emit("download-complete", ()).unwrap_or_default();

    Ok(())
}

/// macOS: llama.cpp ships llama-server via `brew install llama.cpp`.
/// We rely on get_effective_llama_server() to detect it.
#[cfg(target_os = "macos")]
async fn download_llama_server(manager: &ModelManager, _app_handle: &AppHandle, _client: &Client) -> Result<(), String> {
    if manager.get_effective_llama_server().is_some() {
        return Ok(());
    }
    Err("llama-server not found. On macOS, run: brew install llama.cpp".to_string())
}

/// Non-macOS: download llama-server from the official llama.cpp GitHub release.
#[cfg(not(target_os = "macos"))]
async fn download_llama_server(manager: &ModelManager, app_handle: &AppHandle, client: &Client) -> Result<(), String> {
    if manager.get_effective_llama_server().is_some() {
        println!("llama-server already available, skipping download.");
        return Ok(());
    }

    let target_path = manager.get_llama_server_path();

    println!("Fetching latest llama.cpp release info...");
    app_handle.emit("download-progress", DownloadProgress {
        model: "llama.cpp".to_string(), progress: 0.0, total: 0, current: 0,
    }).unwrap_or_default();

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    let asset_pattern = "win-avx2-x64";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let asset_pattern = "ubuntu-x64";
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
    )))]
    let asset_pattern = "ubuntu-x64"; // fallback

    let release_resp = client
        .get("https://api.github.com/repos/ggml-org/llama.cpp/releases/latest")
        .header("User-Agent", "voxa-app/1.0")
        .send().await.map_err(|e| format!("GitHub API request failed: {}", e))?;

    let status = release_resp.status();
    let release_text = release_resp.text().await
        .map_err(|e| format!("GitHub API read failed: {}", e))?;

    if !status.is_success() {
        let msg = format!("GitHub API returned HTTP {}: {}", status, &release_text[..release_text.len().min(200)]);
        eprintln!("[ERROR] {}", msg);
        return Err(msg);
    }

    let release: serde_json::Value = serde_json::from_str(&release_text)
        .map_err(|e| format!("GitHub API parse failed: {}", e))?;

    let download_url = release["assets"]
        .as_array()
        .and_then(|assets| {
            assets.iter().find(|a| {
                a["name"].as_str()
                    .map(|n| n.contains(asset_pattern) && n.ends_with(".zip"))
                    .unwrap_or(false)
            })
        })
        .and_then(|a| a["browser_download_url"].as_str())
        .ok_or_else(|| format!("No llama.cpp release asset matching '{}' found", asset_pattern))?
        .to_string();

    println!("Downloading llama.cpp from {}", download_url);

    let response = client.get(&download_url).send().await.map_err(|e| e.to_string())?;
    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut zip_bytes: Vec<u8> = Vec::with_capacity(total_size as usize);

    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| e.to_string())?;
        zip_bytes.extend_from_slice(&chunk);
        downloaded += chunk.len() as u64;
        if total_size > 0 {
            app_handle.emit("download-progress", DownloadProgress {
                model: "llama.cpp".to_string(),
                progress: (downloaded as f64 / total_size as f64) * 100.0,
                total: total_size, current: downloaded,
            }).unwrap_or_default();
        }
    }

    #[cfg(target_os = "windows")]
    let binary_name = "llama-server.exe";
    #[cfg(not(target_os = "windows"))]
    let binary_name = "llama-server";

    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("Zip open failed: {}", e))?;

    let mut found = false;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let entry_name = file.name().to_string();
        if entry_name.ends_with(binary_name) {
            let mut out = fs::File::create(&target_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
            found = true;
            break;
        }
    }

    if !found {
        return Err(format!("'{}' not found inside llama.cpp release zip", binary_name));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&target_path, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }

    println!("llama-server installed at {:?}", target_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::mock_app;

    #[test]
    fn test_model_manager_init() {
        let app = mock_app();
        let manager = ModelManager::new(app.handle());
        assert!(manager.is_ok(), "ModelManager should initialize correctly with mock app");
        let manager = manager.unwrap();
        assert!(manager.base_path.ends_with("models"), "Base path should include models directory");
    }

    #[test]
    fn test_model_paths() {
        let app = mock_app();
        let manager = ModelManager::new(app.handle()).unwrap();
        let whisper_path = manager.get_whisper_path();
        let llama_path = manager.get_llama_path();
        assert!(whisper_path.to_str().unwrap().contains("ggml-small.bin"));
        assert!(llama_path.to_str().unwrap().contains("qwen2.5"));
    }
}
