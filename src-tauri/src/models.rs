use std::path::PathBuf;
use std::fs;
use tauri::{AppHandle, Manager, Emitter, State};
use futures_util::StreamExt;
use reqwest::Client;
use std::io::Write;
use scopeguard;

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
}

impl ModelManager {
    pub fn new<R: tauri::Runtime>(app_handle: &tauri::AppHandle<R>) -> Result<Self, String> {
        let app_dir = app_handle.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
        let models_dir = app_dir.join("models");
        if !models_dir.exists() {
            fs::create_dir_all(&models_dir).map_err(|e| format!("Failed to create models directory: {}", e))?;
        }
        Ok(Self { 
            base_path: models_dir,
            is_downloading: std::sync::atomic::AtomicBool::new(false),
        })
    }

    pub fn get_whisper_path(&self) -> PathBuf {
        self.base_path.join("ggml-small.bin")
    }

    pub fn get_llama_path(&self) -> PathBuf {
        self.base_path.join("qwen2-1.5b-instruct-q4_k_m.gguf")
    }

    pub fn models_exist(&self) -> bool {
        self.get_whisper_path().exists() && self.get_llama_path().exists()
    }
}

#[tauri::command]
pub async fn get_models_info(app_handle: AppHandle) -> Result<ModelsStateInfo, String> {
    let manager = ModelManager::new(&app_handle)?;
    let models_meta = vec![
        ("Whisper Small (General)", "ggml-small.bin"),
        ("Qwen2-1.5B (Refinement)", "qwen2-1.5b-instruct-q4_k_m.gguf"),
    ];

    let mut models = Vec::new();

    for (display_name, filename) in models_meta {
        let path = manager.base_path.join(filename);
        let exists = path.exists();
        let size_mb = if exists {
            fs::metadata(&path).map(|m| m.len() as f64 / 1_048_576.0).unwrap_or(0.0)
        } else {
            0.0
        };

        models.push(ModelDetail {
            display_name: display_name.to_string(),
            filename: filename.to_string(),
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
pub async fn open_models_folder(app_handle: AppHandle) -> Result<(), String> {
    let manager = ModelManager::new(&app_handle)?;
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
pub async fn check_models_status(app_handle: AppHandle) -> Result<bool, String> {
    let manager = ModelManager::new(&app_handle)?;
    Ok(manager.models_exist())
}

#[tauri::command]
pub async fn download_models(app_handle: AppHandle, manager: State<'_, ModelManager>) -> Result<(), String> {
    // Prevent multiple concurrent downloads
    if manager.is_downloading.swap(true, std::sync::atomic::Ordering::SeqCst) {
        println!("Download already in progress, skipping.");
        return Ok(());
    }

    // Ensure we reset the flag even if we error out
    let _guard = scopeguard::guard((), |_| {
        manager.is_downloading.store(false, std::sync::atomic::Ordering::SeqCst);
    });

    let client = Client::new();

    let models = vec![
        ("ggml-small.bin", "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin"),
        ("qwen2-1.5b-instruct-q4_k_m.gguf", "https://huggingface.co/bartowski/Qwen2-1.5B-Instruct-GGUF/resolve/main/Qwen2-1.5B-Instruct-Q4_K_M.gguf"),
    ];

    let current_model_names: std::collections::HashSet<&str> = models.iter().map(|(name, _)| *name).collect();

    for (name, url) in &models {
        let target_path = manager.base_path.join(name);
        println!("Checking model: {}", name);
        
        if target_path.exists() {
            let meta = fs::metadata(&target_path).map_err(|e| e.to_string())?;
            if meta.len() > 1_000_000 {
                println!("Model {} already exists and is valid.", name);
                continue;
            } else {
                println!("Model {} is too small, redownloading...", name);
                fs::remove_file(&target_path).map_err(|e| e.to_string())?;
            }
        }

        let temp_path = manager.base_path.join(format!("{}.download", name));
        println!("Downloading {} from {}", name, url);
        
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
                        model: name.to_string(),
                        progress,
                        total: total_size,
                        current: downloaded,
                    }).unwrap_or_default();
                }
            }
            file.flush().map_err(|e| e.to_string())?;
        }
        
        println!("Finalizing {}...", name);
        fs::rename(&temp_path, &target_path).map_err(|e| e.to_string())?;
        println!("Model {} ready.", name);
    }

    // Cleanup: Remove old models that are no longer in the current list
    println!("Cleaning up old models...");
    if let Ok(entries) = fs::read_dir(&manager.base_path) {
        for entry in entries.flatten() {
            if let Some(file_name) = entry.file_name().to_str() {
                // Don't delete our current models and don't delete hidden files
                if !current_model_names.contains(file_name) && !file_name.starts_with('.') && !file_name.ends_with(".download") {
                    println!("Removing unused model file: {}", file_name);
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    println!("All models provisioned successfully.");
    app_handle.emit("download-complete", ()).unwrap_or_default();

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
        assert!(llama_path.to_str().unwrap().contains("qwen2-1.5b-instruct"));
    }
}
