#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

mod audio;
mod db;
mod llama_inference;
mod models;
mod whisper_inference;
mod window_utils;

use crate::audio::AudioEngine;
use db::{Transcript, DbState};
use tauri::{Manager, State, AppHandle, Emitter};
use std::sync::{Mutex, mpsc, atomic::AtomicBool};
use rusqlite::params;
use tauri::menu::{Menu, IconMenuItem};
use tauri::tray::TrayIconBuilder;
use sys_locale::get_locale;

#[cfg(target_os = "macos")]
fn simulate_paste() {
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to keystroke \"v\" using command down")
        .spawn();
}

// Vibrancy for the Pill and Settings is managed via Tauri's window configuration.
// The native macOS tray menu handles its own appearance according to system settings.


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
fn update_setting(app: tauri::AppHandle, state: tauri::State<DbState>, key: String, value: String) -> Result<(), String> {
    let conn = state.conn.lock().unwrap();
    db::update_setting(&conn, &key, &value).map_err(|e| e.to_string())?;
    let _ = app.emit("settings-updated", ());
    Ok(())
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
    db::remove_from_dictionary(&conn, &word).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_profile(app: tauri::AppHandle, state: tauri::State<DbState>, id: i64, name: String, prompt: String, icon: Option<String>) -> Result<(), String> {
    let conn = state.conn.lock().unwrap();
    db::update_profile(&conn, id, &name, &prompt, icon).map_err(|e| e.to_string())?;
    let _ = app.emit("profiles-updated", ());
    Ok(())
}

#[tauri::command]
fn create_profile(app: tauri::AppHandle, state: tauri::State<DbState>, name: String, prompt: String, icon: Option<String>) -> Result<i64, String> {
    let conn = state.conn.lock().unwrap();
    let id = db::create_profile(&conn, &name, &prompt, icon).map_err(|e| e.to_string())?;
    let _ = app.emit("profiles-updated", ());
    Ok(id)
}

#[tauri::command]
fn delete_profile(app: tauri::AppHandle, state: tauri::State<DbState>, id: i64) -> Result<(), String> {
    let conn = state.conn.lock().unwrap();
    db::delete_profile(&conn, id).map_err(|e| e.to_string())?;
    let _ = app.emit("profiles-updated", ());
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
fn get_system_locale() -> String {
    get_locale()
        .unwrap_or_else(|| "en".to_string())
        .split('-')
        .next()
        .unwrap_or("en")
        .to_string()
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "settings" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .setup(|app| {
            // Position main window at the bottom center of the screen (Dock-aware)
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = window.primary_monitor() {
                    let monitor_size = monitor.size();
                    let monitor_pos = monitor.position();
                    let win_size = window.outer_size().unwrap_or(tauri::PhysicalSize::new(300, 160));
                    
                    let new_pos = window_utils::calculate_pill_position(
                        *monitor_size,
                        *monitor_pos,
                        win_size,
                        10 // padding bottom
                    );
                    
                    let _ = window.set_position(tauri::Position::Physical(new_pos));
                    
                    // Global Overlay Configuration
                    let _ = window.set_always_on_top(true);
                    let _ = window.set_skip_taskbar(true); // Don't show in Dock as a separate app window
                    
                    // Enable visibility on all virtual desktops (Spaces) on macOS
                    #[cfg(target_os = "macos")]
                    {
                        use cocoa::appkit::NSWindowCollectionBehavior;
                        
                        // We use ns_window() which is available because we have "macos-private-api" feature enabled
                        if let Ok(ns_window) = window.ns_window() {
                            unsafe {
                                let collection_behavior = NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces 
                                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary 
                                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle;
                                
                                let () = msg_send![ns_window as cocoa::base::id, setCollectionBehavior: collection_behavior];
                            }
                        }

                        // No native vibrancy here, use CSS
                    }
                }
                let _ = window.show();
            }

            let conn = db::init(app.handle())?;
            app.manage(DbState {
                conn: std::sync::Mutex::new(conn),
            });

            // --- NATIVE MENU CONSTRUCTION ---
            use tauri::menu::{Submenu, MenuItem, PredefinedMenuItem, CheckMenuItem};
            
            let db_state = app.state::<DbState>();
            let (profiles, settings) = {
                let conn_guard = db_state.conn.lock().unwrap();
                let p = db::get_profiles(&conn_guard)?;
                let s = db::get_settings(&conn_guard)?;
                (p, s)
            };

            let sys_lang = get_system_locale();
            let is_es = sys_lang == "es";

            let active_profile_id = settings.get("active_profile_id").cloned().unwrap_or_else(|| "1".to_string());
            let current_language = settings.get("language").cloned().unwrap_or_else(|| "es".to_string());
            let current_mic = settings.get("mic_id").cloned().unwrap_or_else(|| "auto".to_string());

            let profiles_label = if is_es { "Perfiles" } else { "Profiles" };
            let profiles_menu = Submenu::with_id(app.handle(), "profiles_menu", profiles_label, true)?;
            for profile in profiles {
                let is_checked = profile.id.to_string() == active_profile_id;
                let item = CheckMenuItem::with_id(
                    app.handle(),
                    format!("profile_{}", profile.id),
                    &profile.name,
                    true,
                    is_checked,
                    None::<&str>
                )?;
                profiles_menu.append(&item)?;
            }

            let lang_label = if is_es { "Idioma" } else { "Language" };
            let language_menu = Submenu::with_id(app.handle(), "language_menu", lang_label, true)?;
            let lang_es = CheckMenuItem::with_id(app.handle(), "lang_es", "Español", true, current_language == "es", None::<&str>)?;
            let lang_en = CheckMenuItem::with_id(app.handle(), "lang_en", "English", true, current_language == "en", None::<&str>)?;
            language_menu.append(&lang_es)?;
            language_menu.append(&lang_en)?;

            let mic_label = if is_es { "Micrófono" } else { "Microphone" };
            let mic_menu = Submenu::with_id(app.handle(), "mic_menu", mic_label, true)?;
            
            // Auto-detect option
            let default_mic_name = audio::get_default_input_device_name().unwrap_or_else(|| "Unknown".to_string());
            let auto_mic_label = if is_es { 
                format!("Auto-detectar ({})", default_mic_name) 
            } else { 
                format!("Auto-detect ({})", default_mic_name) 
            };
            let auto_mic_item = CheckMenuItem::with_id(
                app.handle(),
                "mic_auto",
                &auto_mic_label,
                true,
                current_mic == "auto",
                None::<&str>
            )?;
            mic_menu.append(&auto_mic_item)?;
            mic_menu.append(&PredefinedMenuItem::separator(app.handle())?)?;

            let mics = audio::get_input_devices()?;
            for mic in mics {
                let item = CheckMenuItem::with_id(
                    app.handle(),
                    format!("mic_{}", mic.name),
                    &mic.name,
                    true,
                    current_mic == mic.name,
                    None::<&str>
                )?;
                mic_menu.append(&item)?;
            }

            let icon_bytes = include_bytes!("../icons/tray-icon.png");
            let tray_icon = tauri::image::Image::from_bytes(icon_bytes)?;

            let settings_label = if is_es { "Configuración..." } else { "Settings..." };
            let quit_label = if is_es { "Salir de Voxa" } else { "Quit Voxa" };

            let tray_menu = Menu::with_items(app.handle(), &[
                &profiles_menu,
                &mic_menu,
                &language_menu,
                &PredefinedMenuItem::separator(app.handle())?,
                &MenuItem::with_id(app.handle(), "settings", settings_label, true, None::<&str>)?,
                &PredefinedMenuItem::separator(app.handle())?,
                &PredefinedMenuItem::quit(app.handle(), Some(quit_label))?,
            ])?;

            let profiles_menu_c = profiles_menu.clone();
            let mic_menu_c = mic_menu.clone();
            let language_menu_c = language_menu.clone();

            let tray = TrayIconBuilder::with_id("main")
                .icon(tray_icon)
                .menu(&tray_menu)
                .on_menu_event(move |app, event| {
                    let id = event.id.as_ref();
                    if id == "settings" {
                        let _ = show_settings(app.clone(), None);
                    } else if id.starts_with("profile_") {
                        let profile_id = id.replace("profile_", "");
                        let db_state = app.state::<DbState>();
                        let _ = update_setting(app.clone(), db_state, "active_profile_id".to_string(), profile_id);
                        
                        // Update checkmarks in profiles menu
                        if let Ok(items) = profiles_menu_c.items() {
                            for item in items {
                                if let Some(cmi) = item.as_check_menuitem() {
                                    let _ = cmi.set_checked(item.id().as_ref() == id);
                                }
                            }
                        }
                    } else if id.starts_with("mic_") {
                        let mic_id = if id == "mic_auto" { "auto".to_string() } else { id.replace("mic_", "") };
                        let db_state = app.state::<DbState>();
                        let _ = update_setting(app.clone(), db_state, "mic_id".to_string(), mic_id);

                        // Update checkmarks in mic menu
                        if let Ok(items) = mic_menu_c.items() {
                            for item in items {
                                if let Some(cmi) = item.as_check_menuitem() {
                                    let _ = cmi.set_checked(item.id().as_ref() == id);
                                }
                            }
                        }
                    } else if id == "lang_es" || id == "lang_en" {
                        let lang = if id == "lang_es" { "es" } else { "en" };
                        let db_state = app.state::<DbState>();
                        let _ = update_setting(app.clone(), db_state, "language".to_string(), lang.to_string());

                        // Update checkmarks in language menu
                        if let Ok(items) = language_menu_c.items() {
                            for item in items {
                                if let Some(cmi) = item.as_check_menuitem() {
                                    let _ = cmi.set_checked(item.id().as_ref() == id);
                                }
                            }
                        }
                    }
                })
                .build(app)?;
            // Store the tray icon in the app state to prevent it from being dropped
            app.manage(tray);

            println!("SETUP: Native tray menu initialized.");

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
                                    let model_path = model_manager.get_whisper_path();
                                    println!("PIPELINE: Loading Whisper model from {:?}", model_path);
                                    let _ = app_clone.emit("pipeline-status", "loading_whisper");
                                    match whisper_inference::WhisperEngine::new(&model_path) {
                                        Ok(e) => {
                                            println!("PIPELINE: Whisper model loaded successfully.");
                                            *whisper_lock = Some(e);
                                        },
                                        Err(e) => {
                                            println!("PIPELINE ERROR: Failed to load Whisper: {}", e);
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
                                println!("PIPELINE: Calling whisper.transcribe...");
                                match whisper.transcribe(&samples, &language, &initial_prompt) {
                                    Ok(t) => {
                                        println!("PIPELINE: Whisper transcription complete: \"{}\"", t);
                                        t
                                    },
                                    Err(e) => {
                                        println!("PIPELINE ERROR: Transcription failed: {}", e);
                                        let _ = app_clone.emit("pipeline-error", e);
                                        continue;
                                    }
                                }
                            };

                            if raw_text.is_empty() { continue; }
                            let _ = app_clone.emit("pipeline-text-raw", &raw_text);
                            let _ = app_clone.emit("pipeline-status", "refining");

                            println!("PIPELINE: Refining text with Llama...");
                            let refined_text = {
                                let mut llama_lock = engine_state.llama.lock().unwrap();
                                if llama_lock.is_none() {
                                    let model_path = model_manager.get_llama_path();
                                    if !model_path.exists() {
                                        println!("PIPELINE WARNING: Llama model not found, skipping refinement.");
                                        raw_text.clone()
                                    } else {
                                        let _ = app_clone.emit("pipeline-status", "loading_llama");
                                        match llama_inference::LlamaEngine::new(&model_path) {
                                            Ok(e) => {
                                                *llama_lock = Some(e);
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
                                                        }
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                println!("PIPELINE ERROR: Failed to load Llama: {}. Falling back to raw text.", e);
                                                let _ = app_clone.emit("pipeline-error", format!("Llama Loading Error: {}", e));
                                                raw_text.clone()
                                            }
                                        }
                                    }
                                } else {
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
                                            }
                                        }
                                    }
                                }
                            };
                            
                            println!("Refined Final Output: \"{}\"", refined_text);

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
            update_profile,
            create_profile,
            delete_profile,
            models::check_models_status,
            models::download_models,
            models::get_models_info,
            models::open_models_folder,
            show_settings,
            get_system_locale,
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
