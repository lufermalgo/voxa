use std::path::PathBuf;
use std::fs;
use tauri::{AppHandle, Manager, Emitter};
use futures_util::StreamExt;
use reqwest::Client;
use std::io::Write;

#[derive(serde::Serialize, Clone)]
pub struct DownloadProgress {
    pub model: String,
    pub progress: f64,
    pub total: u64,
    pub current: u64,
}

pub struct ModelManager {
    pub base_path: PathBuf,
}

impl ModelManager {
    pub fn new<R: tauri::Runtime>(app_handle: &tauri::AppHandle<R>) -> Result<Self, String> {
        let app_dir = app_handle.path().app_data_dir().map_err(|e| format!("Failed to get app data dir: {}", e))?;
        let models_dir = app_dir.join("models");
        if !models_dir.exists() {
            fs::create_dir_all(&models_dir).map_err(|e| format!("Failed to create models directory: {}", e))?;
        }
        Ok(Self { base_path: models_dir })
    }

    pub fn get_whisper_path(&self) -> PathBuf {
        self.base_path.join("ggml-base.bin")
    }

    pub fn get_llama_path(&self) -> PathBuf {
        self.base_path.join("smollm-135m-instruct.gguf")
    }

    pub fn models_exist(&self) -> bool {
        self.get_whisper_path().exists() && self.get_llama_path().exists()
    }
}

#[tauri::command]
pub async fn check_models_status(app_handle: AppHandle) -> Result<bool, String> {
    let manager = ModelManager::new(&app_handle)?;
    Ok(manager.models_exist())
}

#[tauri::command]
pub async fn download_models(app_handle: AppHandle) -> Result<(), String> {
    let manager = ModelManager::new(&app_handle)?;
    let client = Client::new();

    let models = vec![
        ("ggml-base.bin", "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin"),
        ("smollm-135m-instruct.gguf", "https://huggingface.co/HuggingFaceTB/SmolLM-135M-Instruct-GGUF/resolve/main/smollm-135m-instruct-add-it-token.q4_k_m.gguf"),
    ];

    for (name, url) in models {
        let target_path = manager.base_path.join(name);
        if target_path.exists() {
            continue;
        }

        let response = client.get(url).send().await.map_err(|e| e.to_string())?;
        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        let mut file = fs::File::create(&target_path).map_err(|e| e.to_string())?;

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
    }

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
        assert!(whisper_path.to_str().unwrap().contains("ggml-base.bin"));
        assert!(llama_path.to_str().unwrap().contains("smollm-135m-instruct.gguf"));
    }
}
