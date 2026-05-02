// `objc` 0.2.x uses `cfg(cargo-clippy)` internally via its `sel_impl` macro.
// It can't be upgraded because `cocoa` pins it to 0.2. Suppress the spurious warning.
#![allow(unexpected_cfgs)]

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

mod audio;
mod commands;
mod db;
mod formatting;
mod vad;
mod event_tap;
mod llama_inference;
mod models;
mod pipeline;
mod shortcuts;
mod tray;
mod whisper_inference;
mod window_utils;

use crate::audio::AudioEngine;
use crate::db::{DbState, SettingsCache};
use crate::pipeline::{
    CursorContext, DetectedProfile, DictationEvent, DictationSender, EngineState, FrontmostApp, ManualProfileOverride,
    PipelineHandle, RecordingState,
};
use crate::shortcuts::{NativeShortcuts, NATIVE_SHORTCUTS};

use std::sync::{atomic::AtomicBool, Arc, Mutex, mpsc};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    ).try_init();

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
            // Initialize NATIVE_SHORTCUTS global state
            let _ = NATIVE_SHORTCUTS.get_or_init(|| Mutex::new(NativeShortcuts {
                ptt: String::new(),
                hands_free: String::new(),
                paste: String::new(),
                cancel: String::new(),
            }));

            // Position main window at the bottom center of the screen (Dock-aware)
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = window.primary_monitor() {
                    let monitor_size = monitor.size();
                    let monitor_pos  = monitor.position();
                    let win_size = window.outer_size().unwrap_or(tauri::PhysicalSize::new(300, 160));
                    let new_pos = window_utils::calculate_pill_position(
                        *monitor_size, *monitor_pos, win_size, 10,
                    );
                    let _ = window.set_position(tauri::Position::Physical(new_pos));
                    let _ = window.set_always_on_top(true);
                    let _ = window.set_skip_taskbar(true);

                    #[cfg(target_os = "macos")]
                    {
                        use cocoa::appkit::NSWindowCollectionBehavior;
                        if let Ok(ns_window) = window.ns_window() {
                            unsafe {
                                let ns_win_id = ns_window as cocoa::base::id;
                                let behavior =
                                    NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
                                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle;
                                let () = msg_send![ns_win_id, setCollectionBehavior: behavior];
                            }
                        }
                        unsafe {
                            let ns_app: cocoa::base::id = msg_send![class!(NSApplication), sharedApplication];
                            let () = msg_send![ns_app, setActivationPolicy: 1i64]; // Accessory
                        }
                    }
                }
                let _ = window.set_ignore_cursor_events(true);
                let _ = window.show();
            }

            let conn = db::init(app.handle())?;
            let initial_settings = db::get_settings(&conn).unwrap_or_default();
            app.manage(SettingsCache::new(initial_settings));
            app.manage(DbState { conn: Arc::new(Mutex::new(conn)) });

            tray::build_tray(app)?;

            log::info!("Voxa started.");
            app.manage(AudioEngine::new());

            let model_manager = models::ModelManager::new(app.handle())?;
            app.manage(model_manager);

            app.manage(EngineState { whisper: Mutex::new(None), llama: Mutex::new(None) });

            let (tx, rx) = mpsc::channel::<DictationEvent>();
            app.manage(DictationSender(Mutex::new(tx)));
            app.manage(RecordingState(AtomicBool::new(false)));
            app.manage(FrontmostApp(Mutex::new(pipeline::AppInfo::default())));
            app.manage(ManualProfileOverride(Mutex::new(None)));
            app.manage(DetectedProfile(Mutex::new(None)));
            app.manage(PipelineHandle { cancelled: Arc::new(AtomicBool::new(false)) });
            app.manage(CursorContext {
                pre_text:  Mutex::new(String::new()),
                post_text: Mutex::new(String::new()),
            });

            // Pre-warm LlamaEngine in background (build OUTSIDE the mutex — see invariants)
            let app_warmup = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(3));
                let model_manager = app_warmup.state::<models::ModelManager>();
                let engine_state  = app_warmup.state::<EngineState>();
                let model_path    = model_manager.get_llama_path();
                if !model_path.exists() { return; }
                let server_path = match model_manager.get_effective_llama_server() {
                    Some(p) => p, None => return,
                };
                { let lock = engine_state.llama.lock().unwrap(); if lock.is_some() { return; } }
                log::info!("Pre-loading LlamaEngine from {:?}", model_path);
                match llama_inference::LlamaEngine::new(&model_path, &server_path) {
                    Ok(e) => {
                        let mut lock = engine_state.llama.lock().unwrap();
                        if lock.is_none() { *lock = Some(e); }
                        let size_mb = std::fs::metadata(&model_path)
                            .map(|m| m.len() as f64 / 1_048_576.0).unwrap_or(0.0);
                        log::info!("LlamaEngine ready — {:.0}MB", size_mb);
                    }
                    Err(e) => log::error!("LlamaEngine warmup failed: {}", e),
                }
            });

            // Pre-warm WhisperEngine in background — loads model + initializes Metal
            // GPU backend so the first dictation doesn't pay the ~5s init cost.
            let app_whisper_warmup = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let model_manager = app_whisper_warmup.state::<models::ModelManager>();
                let engine_state  = app_whisper_warmup.state::<EngineState>();
                let model_path    = model_manager.get_whisper_path();
                if !model_path.exists() { return; }
                { let lock = engine_state.whisper.lock().unwrap(); if lock.is_some() { return; } }
                log::info!("Pre-loading WhisperEngine from {:?}", model_path);
                let t_load = std::time::Instant::now();
                match whisper_inference::WhisperEngine::new(&model_path) {
                    Ok(e) => {
                        let mut lock = engine_state.whisper.lock().unwrap();
                        if lock.is_none() { *lock = Some(e); }
                        let size_mb = std::fs::metadata(&model_path)
                            .map(|m| m.len() as f64 / 1_048_576.0).unwrap_or(0.0);
                        log::info!("WhisperEngine ready — {:.0}MB  {:.2}s", size_mb, t_load.elapsed().as_secs_f64());
                    }
                    Err(e) => log::error!("WhisperEngine warmup failed: {}", e),
                }
            });

            pipeline::start_pipeline(app.handle().clone(), rx);

            // Request Accessibility permission with a prompt if not already trusted.
            // AXIsProcessTrustedWithOptions forces macOS to re-evaluate the TCC entry
            // for the current binary hash — fixes cases where the binary was updated
            // (new build) but the TCC database still has the old hash.
            #[cfg(target_os = "macos")]
            {
                use core_foundation::dictionary::CFDictionary;
                use core_foundation::string::CFString;
                use core_foundation::boolean::CFBoolean;
                use core_foundation::base::TCFType;

                extern "C" {
                    fn AXIsProcessTrustedWithOptions(options: core_foundation::dictionary::CFDictionaryRef) -> bool;
                }

                let key = CFString::new("AXTrustedCheckOptionPrompt");
                let val = CFBoolean::true_value();
                let opts = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), val.as_CFType())]);
                let trusted = unsafe { AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef()) };
                if !trusted {
                    log::warn!("Accessibility not granted — showing system prompt.");
                }
            }
            event_tap::setup_native_event_tap(app.handle().clone());

            if let Err(e) = shortcuts::apply_all_shortcuts(app.handle().clone()) {
                log::error!("Failed to register global shortcuts on startup: {}", e);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_transcripts,
            commands::delete_transcript,
            commands::clear_transcripts,
            commands::get_settings,
            commands::update_setting,
            commands::get_audio_devices,
            commands::cancel_recording,
            commands::stop_and_transcribe,
            commands::set_window_interactive,
            shortcuts::apply_all_shortcuts,
            shortcuts::unregister_all_shortcuts,
            commands::get_profiles,
            commands::get_custom_dictionary,
            commands::get_dictionary_entries,
            commands::add_to_dictionary,
            commands::remove_from_dictionary,
            commands::update_replacement_word,
            commands::update_transcript,
            commands::update_profile,
            commands::update_profile_formatting_mode,
            commands::create_profile,
            commands::delete_profile,
            commands::submit_correction,
            models::check_models_status,
            models::download_models,
            models::get_models_info,
            models::open_models_folder,
            commands::show_settings,
            commands::set_manual_profile_override,
            commands::get_system_locale,
            commands::exit_app,
            shortcuts::start_native_key_capture,
            commands::check_accessibility_permissions,
            commands::get_active_app,
            commands::set_pill_warning_mode,
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
        let icon_path = Path::new("icons/tray-icon.png");
        assert!(icon_path.exists(), "Tray icon must exist at icons/tray-icon.png");
        let metadata = std::fs::metadata(icon_path).expect("Failed to get icon metadata");
        assert!(metadata.len() > 0, "Tray icon file is empty");
    }
}
