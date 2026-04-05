mod audio;
mod db;
mod models;
mod whisper_inference;
mod llama_inference;

use audio::AudioEngine;
use db::{Transcript, DbState};
use tauri::{Manager, State, AppHandle, Emitter};
use std::sync::{Mutex, mpsc, atomic::AtomicBool};
use rusqlite::params;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;

#[cfg(target_os = "macos")]
fn simulate_paste() {
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to keystroke \"v\" using command down")
        .spawn();
}

#[cfg(not(target_os = "macos"))]
fn simulate_paste() {
    // Placeholder for non-macOS platforms
}

pub enum DictationEvent {
    StartRecording,
    StopRecording,
}

pub struct DictationSender(pub Mutex<mpsc::Sender<DictationEvent>>);
pub struct RecordingState(pub AtomicBool);

pub struct EngineState {
    pub whisper: Mutex<Option<whisper_inference::WhisperEngine>>,
    pub llama: Mutex<Option<llama_inference::LlamaEngine>>,
}

#[tauri::command]
async fn get_transcripts(state: State<'_, DbState>) -> Result<Vec<Transcript>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::get_all_transcripts(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_transcript(state: State<'_, DbState>, id: i64) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::delete_transcript(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_settings(state: State<'_, DbState>) -> Result<std::collections::HashMap<String, String>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::get_settings(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_setting(state: State<'_, DbState>, key: String, value: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::update_setting(&conn, &key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_audio_devices() -> Result<Vec<audio::AudioDevice>, String> {
    audio::get_input_devices()
}

#[tauri::command]
async fn get_profiles(state: State<'_, DbState>) -> Result<Vec<db::Profile>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::get_profiles(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_custom_dictionary(state: State<'_, DbState>) -> Result<Vec<String>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::get_custom_dictionary(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_to_dictionary(state: State<'_, DbState>, word: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("INSERT OR IGNORE INTO custom_dict (word) VALUES (?1)", params![word])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn remove_from_dictionary(state: State<'_, DbState>, word: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM custom_dict WHERE word = ?1", params![word])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn start_recording(engine: State<'_, AudioEngine>, db_state: State<'_, DbState>) -> Result<(), String> {
    let mic_id = {
        let conn = db_state.conn.lock().unwrap();
        db::get_settings(&conn).unwrap_or_default().get("mic_id").cloned()
    };
    audio::setup_stream(&engine, mic_id)
}

#[tauri::command]
async fn stop_recording(engine: State<'_, AudioEngine>, db_state: State<'_, DbState>) -> Result<Vec<f32>, String> {
    let mic_id = {
        let conn = db_state.conn.lock().unwrap();
        db::get_settings(&conn).unwrap_or_default().get("mic_id").cloned()
    };
    audio::stop_stream(&engine, mic_id)
}

#[tauri::command]
async fn run_pipeline(
    _app_handle: AppHandle,
    audio_engine: State<'_, AudioEngine>,
    _db_state: State<'_, DbState>,
    engine_state: State<'_, EngineState>,
    model_manager: State<'_, models::ModelManager>,
) -> Result<(), String> {
    // 1. Ensure engines are loaded
    {
        let mut whisper = engine_state.whisper.lock().unwrap();
        if whisper.is_none() {
            *whisper = Some(whisper_inference::WhisperEngine::new(&model_manager.get_whisper_path())?);
        }
    }
    
    // 2. Start Recording
    let mic_id = {
        let conn = _db_state.conn.lock().unwrap();
        db::get_settings(&conn).unwrap_or_default().get("mic_id").cloned()
    };
    audio::setup_stream(&audio_engine, mic_id)?;
    
    Ok(())
}

#[tauri::command]
fn apply_shortcut(app_handle: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{Shortcut, ShortcutState, GlobalShortcutExt};
    use std::str::FromStr;

    let parsed_shortcut = match Shortcut::from_str(&shortcut) {
        Ok(s) => s,
        Err(e) => return Err(e.to_string()),
    };
    
    let _ = app_handle.global_shortcut().unregister_all();
    
    app_handle.global_shortcut().on_shortcut(parsed_shortcut, move |app, _shortcut, event| {
        if let Some(tx_state) = app.try_state::<DictationSender>() {
            let tx = tx_state.0.lock().unwrap();
            let db_state = app.state::<DbState>();
            let is_recording_state = app.state::<RecordingState>();
            
            let mode = {
                let conn = db_state.conn.lock().unwrap();
                db::get_settings(&conn).unwrap_or_default().get("interaction_mode").cloned().unwrap_or_else(|| "push_to_talk".to_string())
            };

            if mode == "push_to_talk" {
                if event.state() == ShortcutState::Pressed {
                    is_recording_state.0.store(true, std::sync::atomic::Ordering::SeqCst);
                    let _ = tx.send(DictationEvent::StartRecording);
                } else {
                    is_recording_state.0.store(false, std::sync::atomic::Ordering::SeqCst);
                    let _ = tx.send(DictationEvent::StopRecording);
                }
            } else if mode == "hands_free" {
                if event.state() == ShortcutState::Pressed {
                    let currently_recording = is_recording_state.0.load(std::sync::atomic::Ordering::SeqCst);
                    if currently_recording {
                        is_recording_state.0.store(false, std::sync::atomic::Ordering::SeqCst);
                        let _ = tx.send(DictationEvent::StopRecording);
                    } else {
                        is_recording_state.0.store(true, std::sync::atomic::Ordering::SeqCst);
                        let _ = tx.send(DictationEvent::StartRecording);
                    }
                }
            }
        }
    }).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
fn show_settings(app: tauri::AppHandle, tab: Option<String>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
        if let Some(t) = tab {
            let _ = window.emit("show-tab", t);
        }
    }
    Ok(())
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // Position main window at the bottom center of the screen (Dock-aware)
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = window.primary_monitor() {
                    let size = monitor.size();
                    let position = monitor.position();
                    let win_size = window.inner_size().unwrap_or(tauri::PhysicalSize::new(300, 160));
                    
                    let x = position.x + (size.width as i32 / 2) - (win_size.width as i32 / 2);
                    // Use total height minus window height, plus a bit of padding for the dock
                    let y = position.y + size.height as i32 - win_size.height as i32 - 15;
                    
                    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(x, y)));
                    // Window will be shown by App.tsx once it's ready, or we can show it here
                    // Let's show it here for immediate feedback since we already positioned it.
                    let _ = window.show();
                }
            }

            let _tray_menu = Menu::with_items(app.handle(), &[
                &MenuItem::with_id(app.handle(), "status", "Voxa is Ready", true, None::<&str>)?,
                &MenuItem::with_id(app.handle(), "profiles", "Profiles...", true, None::<&str>)?,
                &MenuItem::with_id(app.handle(), "language", "Language...", true, None::<&str>)?,
                &tauri::menu::PredefinedMenuItem::separator(app.handle())?,
                &MenuItem::with_id(app.handle(), "settings", "Settings...", true, None::<&str>)?,
                &MenuItem::with_id(app.handle(), "dictionary", "Dictionary...", true, None::<&str>)?,
                &tauri::menu::PredefinedMenuItem::quit(app.handle(), None)?,
            ])?;

            let tray_icon = tauri::image::Image::from_path("icons/tray-icon.png")?;

            let _tray = TrayIconBuilder::new()
                .icon(tray_icon)
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { 
                        button: tauri::tray::MouseButton::Left, 
                        button_state: tauri::tray::MouseButtonState::Up, 
                        rect, 
                        .. 
                    } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("tray_menu") {
                            let scale_factor = window.scale_factor().unwrap_or(1.0);
                            
                            let pos = match rect.position {
                                tauri::Position::Physical(p) => p.to_logical::<f64>(scale_factor),
                                tauri::Position::Logical(p) => p,
                            };
                            let size = match rect.size {
                                tauri::Size::Physical(s) => s.to_logical::<f64>(scale_factor),
                                tauri::Size::Logical(s) => s,
                            };

                            let is_visible = window.is_visible().unwrap_or(false);
                            if is_visible {
                                let _ = window.hide();
                            } else {
                                // Positioning: Center under the tray icon
                                let window_size = window.inner_size().unwrap_or(tauri::PhysicalSize::new(360, 500)).to_logical::<f64>(scale_factor);
                                
                                let x = pos.x + (size.width / 2.0) - (window_size.width / 2.0);
                                let y = pos.y + size.height + 5.0; // 5px padding
                                
                                let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Setup blur listener for tray menu to auto-hide
            if let Some(window) = app.get_webview_window("tray_menu") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(focused) = event {
                        if !focused {
                            let _ = w.hide();
                        }
                    }
                });
            }

            let conn = db::init(app.handle())?;
            app.manage(DbState {
                conn: std::sync::Mutex::new(conn),
            });
            app.manage(AudioEngine::new());
            
            // Initialize Model Manager (handles directory creation)
            let model_manager = models::ModelManager::new(app.handle())?;
            app.manage(model_manager);
            
            app.manage(EngineState {
                whisper: Mutex::new(None),
                llama: Mutex::new(None),
            });
            
            let (tx, rx) = mpsc::channel::<DictationEvent>();
            app.manage(DictationSender(Mutex::new(tx)));
            app.manage(RecordingState(AtomicBool::new(false)));
            
            let app_clone = app.handle().clone();
            std::thread::spawn(move || {
                for event in rx {
                    match event {
                        DictationEvent::StartRecording => {
                            if let Some(audio_engine) = app_clone.try_state::<AudioEngine>() {
                                let mic_id = {
                                    let db_state = app_clone.state::<DbState>();
                                    let conn = db_state.conn.lock().unwrap();
                                    db::get_settings(&conn).unwrap_or_default().get("mic_id").cloned()
                                };
                                match audio::setup_stream(&audio_engine, mic_id) {
                                    Ok(_) => {
                                        let _ = app_clone.emit("pipeline-status", "recording");
                                    }
                                    Err(e) => {
                                        let _ = app_clone.emit("pipeline-error", format!("Audio Error: {}", e));
                                    }
                                }
                            }
                        }
                        DictationEvent::StopRecording => {
                            let _ = app_clone.emit("pipeline-status", "processing");
                            
                            let audio_engine = app_clone.state::<AudioEngine>();
                            let engine_state = app_clone.state::<EngineState>();
                            let model_manager = app_clone.state::<models::ModelManager>();
                            let db_state = app_clone.state::<DbState>();
                            
                            let mic_id = {
                                let conn = db_state.conn.lock().unwrap();
                                db::get_settings(&conn).unwrap_or_default().get("mic_id").cloned()
                            };
                            let samples = match audio::stop_stream(&audio_engine, mic_id) {
                                Ok(s) => s,
                                Err(e) => {
                                    let _ = app_clone.emit("pipeline-error", e);
                                    continue;
                                }
                            };

                            if samples.is_empty() { continue; }

                            let raw_text = {
                                let mut whisper_lock = engine_state.whisper.lock().unwrap();
                                if whisper_lock.is_none() {
                                    let _ = app_clone.emit("pipeline-status", "loading_whisper");
                                    match whisper_inference::WhisperEngine::new(&model_manager.get_whisper_path()) {
                                        Ok(e) => *whisper_lock = Some(e),
                                        Err(e) => {
                                            let _ = app_clone.emit("pipeline-error", e);
                                            continue;
                                        }
                                    }
                                }
                                let whisper = whisper_lock.as_ref().unwrap();
                                let (language, initial_prompt) = {
                                    let conn = db_state.conn.lock().unwrap();
                                    let lang = db::get_settings(&conn).unwrap_or_default().get("language").cloned().unwrap_or_else(|| "es".to_string());
                                    let dict = db::get_custom_dictionary(&conn).unwrap_or_default();
                                    let prompt = if dict.is_empty() { 
                                        "".to_string() 
                                    } else { 
                                        format!("Vocabulary: {}.", dict.join(", ")) 
                                    };
                                    (lang, prompt)
                                };
                                match whisper.transcribe(&samples, &language, &initial_prompt) {
                                    Ok(t) => t,
                                    Err(e) => {
                                        let _ = app_clone.emit("pipeline-error", e);
                                        continue;
                                    }
                                }
                            };

                            if raw_text.is_empty() { continue; }
                            let _ = app_clone.emit("pipeline-text-raw", &raw_text);
                            let _ = app_clone.emit("pipeline-status", "refining");

                            let refined_text = {
                                let mut llama_lock = engine_state.llama.lock().unwrap();
                                if llama_lock.is_none() {
                                    let _ = app_clone.emit("pipeline-status", "loading_llama");
                                    match llama_inference::LlamaEngine::new(&model_manager.get_llama_path()) {
                                        Ok(e) => *llama_lock = Some(e),
                                        Err(e) => {
                                            let _ = app_clone.emit("pipeline-error", e);
                                            continue;
                                        }
                                    }
                                }
                                let llama = llama_lock.as_ref().unwrap();
                                let system_prompt = {
                                    let conn = db_state.conn.lock().unwrap();
                                    db::get_active_profile(&conn).unwrap_or_default().map(|p| p.system_prompt).unwrap_or_default()
                                };
                                
                                if system_prompt.is_empty() {
                                    raw_text.clone()
                                } else {
                                    match llama.refine_text(&raw_text, &system_prompt) {
                                        Ok(t) => t,
                                        Err(e) => {
                                            let _ = app_clone.emit("pipeline-error", format!("Refinement Error: {}", e));
                                            raw_text.clone()
                                        },
                                    }
                                }
                            };

                            {
                                let conn = db_state.conn.lock().unwrap();
                                let _ = db::insert_transcript(&conn, &refined_text, &raw_text);
                            }

                            use tauri_plugin_clipboard_manager::ClipboardExt;
                            app_clone.clipboard().write_text(refined_text.clone()).unwrap_or_else(|e| {
                                let _ = app_clone.emit("pipeline-error", format!("Clipboard Error: {}", e));
                            });
                            
                            simulate_paste();

                            let _ = app_clone.emit("pipeline-results", &refined_text);
                            let _ = app_clone.emit("pipeline-status", "idle");
                        }
                    }
                }
            });
            
            let db_state = app.state::<DbState>();
            let shortcut_str = {
                let conn = db_state.conn.lock().unwrap();
                db::get_settings(&conn).unwrap_or_default().get("global_shortcut").cloned().unwrap_or_else(|| "Alt+Space".to_string())
            };
            if let Err(e) = apply_shortcut(app.handle().clone(), shortcut_str) {
                // Initial shortcut application fallback
                eprintln!("Failed to register initial global shortcut: {}", e);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_transcripts,
            delete_transcript,
            get_settings,
            update_setting,
            get_audio_devices,
            start_recording,
            stop_recording,
            apply_shortcut,
            run_pipeline,
            get_profiles,
            get_custom_dictionary,
            add_to_dictionary,
            remove_from_dictionary,
            models::check_models_status,
            models::download_models,
            show_settings,
            exit_app
        ])
        .plugin(tauri_plugin_clipboard_manager::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn test_tray_icon_path() {
        // Ensure the icon file exists and is not empty
        // In test mode, path resolution might be different, but we check relative to src-tauri root
        let icon_path = Path::new("icons/tray-icon.png");
        assert!(icon_path.exists(), "Tray icon must exist at icons/tray-icon.png");
        let metadata = std::fs::metadata(icon_path).expect("Failed to get icon metadata");
        assert!(metadata.len() > 0, "Tray icon file is empty");
    }
}
