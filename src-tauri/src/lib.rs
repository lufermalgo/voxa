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
mod proc;
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

use std::sync::{atomic::{AtomicBool, AtomicU64}, Arc, Mutex, mpsc};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    ).try_init();

    // Block SIGINT/SIGTERM on the main thread BEFORE any other thread is spawned, so
    // every thread inherits the block and these signals are delivered only to the
    // dedicated waiter thread (spawned in `setup` via `sigwait`). This prevents the
    // kernel from running the default (terminate) disposition on some worker thread,
    // which would skip llama-server cleanup. (Requirement 3.2)
    #[cfg(unix)]
    block_shutdown_signals();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::Builder::new().build())
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
            app.manage(SettingsCache::new(initial_settings.clone()));
            app.manage(DbState { conn: Arc::new(Mutex::new(conn)) });

            // Reconcile the OS login item with the persisted `launch_at_login`
            // setting, in case it drifted (e.g. user removed it from System
            // Settings, or the app was reinstalled).
            {
                use tauri_plugin_autostart::ManagerExt;
                let want_enabled = initial_settings
                    .get("launch_at_login")
                    .map(|v| v == "true")
                    .unwrap_or(false);
                let manager = app.autolaunch();
                let is_enabled = manager.is_enabled().unwrap_or(false);
                if want_enabled && !is_enabled {
                    if let Err(e) = manager.enable() {
                        log::error!("Failed to enable launch at login on startup: {}", e);
                    }
                } else if !want_enabled && is_enabled {
                    if let Err(e) = manager.disable() {
                        log::error!("Failed to disable launch at login on startup: {}", e);
                    }
                }
            }

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
                generation: AtomicU64::new(0),
            });

            // Spawn the dedicated shutdown-signal waiter (Requirement 3.2). It blocks in
            // `sigwait` for SIGTERM/SIGINT (already blocked process-wide at the top of
            // `run`), then terminates the llama-server child and exits the app cleanly so
            // the normal `RunEvent` exit path also runs. This guarantees the child is
            // reaped on signal-driven shutdown, not only on GUI quit — `Drop` stays as a
            // best-effort fallback (Requirement 3.3).
            #[cfg(unix)]
            {
                let app_signals = app.handle().clone();
                std::thread::spawn(move || {
                    if let Some(signal) = wait_for_shutdown_signal() {
                        log::info!("Received shutdown signal {} — terminating llama-server", signal);
                        terminate_llama_server(&app_signals);
                        app_signals.exit(0);
                    }
                });
            }

            // Pre-warm LlamaEngine in background (build OUTSIDE the mutex — see invariants)
            let app_warmup = app.handle().clone();
            std::thread::spawn(move || {
                let model_manager = app_warmup.state::<models::ModelManager>();
                let engine_state  = app_warmup.state::<EngineState>();

                // PID file lives at <app_data_dir>/llama-server.pid (base_path is
                // <app_data_dir>/models), so its parent is the app data dir.
                let pid_file = model_manager.base_path.parent()
                    .map(crate::proc::llama_pid_file);

                // Startup reconciliation (Requirements 1.1, 1.2, 1.4, 5.1): reap any
                // Voxa-owned `llama-server` left behind by a previous (possibly crashed)
                // session. This runs here, at the top of the background pre-warm thread,
                // for two reasons:
                //   * It is off the main thread, so it never delays window display or
                //     shortcut registration (those happen synchronously in setup).
                //   * It runs BEFORE the 3s sleep + spawn below, so reaping always
                //     completes before this instance spawns its own server. The fresh
                //     child does not exist yet at this point, so it can never be matched
                //     and killed. Every step inside fails safe (logs + continues).
                reconcile_llama_servers(
                    &crate::proc::voxa_model_path_marker(&model_manager),
                    pid_file.as_deref(),
                );

                // Bypass gate (Requirements 4.1, 4.3): if the user runs with
                // `bypass_llm` enabled, the LLM is never used, so skip pre-warming
                // `llama-server` and avoid reserving ~950 MB for nothing. Reconciliation
                // above is intentionally NOT gated — orphans from previous non-bypassed
                // sessions must still be reaped. The Whisper pre-warm thread is separate
                // and is unaffected, so Whisper still warms normally when bypassed.
                //
                // Toggle-off-later (Requirement 4.2) is handled without duplication by the
                // lazy-start path in `pipeline.rs`: on each dictation it re-reads
                // `bypass_llm`, and when it is `false` and no engine is loaded it creates
                // the `LlamaEngine` on demand. So nothing extra is needed here.
                let bypass_llm = app_warmup.state::<SettingsCache>().get("bypass_llm").map(|v| v == "true").unwrap_or(false);
                if bypass_llm {
                    log::info!("LLM bypass enabled — skipping llama-server pre-warm");
                    return;
                }

                std::thread::sleep(std::time::Duration::from_secs(3));
                let model_path    = model_manager.get_llama_path();
                if !model_path.exists() { return; }
                let server_path = match model_manager.get_effective_llama_server() {
                    Some(p) => p, None => return,
                };
                { let lock = engine_state.llama.lock().unwrap(); if lock.is_some() { return; } }
                log::info!("Pre-loading LlamaEngine from {:?}", model_path);
                match llama_inference::LlamaEngine::new(&model_path, &server_path, pid_file) {
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
            commands::set_launch_at_login,
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Deterministic shutdown (Requirement 3.1): terminate the llama-server child
            // on every quit path. All GUI quit routes — Cmd+Q, the tray "Quit" item, and
            // the `exit_app` command (which calls `app.exit`) — funnel through Tauri's
            // `RunEvent` exit, so centralizing termination here covers them all rather
            // than duplicating cleanup at each call site. `ExitRequested` is the earliest
            // deterministic point; `Exit` is the final guarantee. `terminate_llama_server`
            // is idempotent, so handling both is safe, and `Drop` on `LlamaEngine` remains
            // a best-effort fallback (Requirement 3.3).
            match event {
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
                    terminate_llama_server(app_handle);
                }
                _ => {}
            }
        });
}

/// Terminate the current `llama-server` child (if any) and remove its PID file.
///
/// This is the deterministic shutdown path shared by the `RunEvent` exit handler and the
/// SIGTERM/SIGINT handler. It locks `EngineState.llama` and, if a server is loaded, calls
/// the explicit [`llama_inference::LlamaEngine::terminate`] (kill child + remove PID file).
/// `Drop` on `LlamaEngine` stays as a best-effort fallback (Requirement 3.3).
///
/// Fails safe: if `EngineState` is not yet managed or the mutex is poisoned, it logs and
/// returns rather than panicking, so shutdown is never blocked. Calling it more than once
/// is harmless — `terminate` is idempotent (killing an already-dead child and removing an
/// absent PID file are both no-ops).
fn terminate_llama_server(app_handle: &tauri::AppHandle) {
    let Some(engine_state) = app_handle.try_state::<EngineState>() else {
        return;
    };
    let lock_result = engine_state.llama.lock();
    match lock_result {
        Ok(mut guard) => {
            if let Some(engine) = guard.as_mut() {
                log::info!("Shutdown: terminating llama-server child");
                engine.terminate();
            }
        }
        Err(e) => log::warn!("Shutdown: llama engine lock poisoned, skipping terminate: {}", e),
    }
}

/// Block `SIGINT` and `SIGTERM` for the current (main) thread so the block is inherited by
/// every thread spawned afterwards. With the signals blocked process-wide, the kernel will
/// not run the default terminate disposition on an arbitrary thread; instead the dedicated
/// waiter thread consumes them synchronously via [`wait_for_shutdown_signal`].
#[cfg(unix)]
fn block_shutdown_signals() {
    // Safety: standard libc sigset_t manipulation. We zero-initialize the set, add the two
    // signals, and install the mask with SIG_BLOCK. All pointers are to valid local state.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        // pthread_sigmask on the main thread before spawning others => inherited by all.
        let _ = libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
    }
}

/// Block until `SIGINT` or `SIGTERM` arrives, returning the signal number that was
/// delivered (or `None` if `sigwait` fails). Must only be called after
/// [`block_shutdown_signals`] has run, so the signals are pending-deliverable to this
/// thread rather than acted on by their default disposition.
#[cfg(unix)]
fn wait_for_shutdown_signal() -> Option<i32> {
    // Safety: we build a sigset containing exactly the signals blocked in
    // `block_shutdown_signals`, then `sigwait` for one of them. `sig` is written by
    // `sigwait` on success.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        let mut sig: libc::c_int = 0;
        if libc::sigwait(&set, &mut sig) == 0 {
            Some(sig as i32)
        } else {
            None
        }
    }
}

/// Startup reconciliation: reap any Voxa-owned `llama-server` left behind by a prior
/// (possibly crashed) session, so wired `--mlock` memory is not leaked across sessions.
///
/// Ordering (see design.md, Component 3) — runs off the main thread, before this
/// instance spawns its own server:
///   1. If the PID file names a live Voxa-owned server, terminate it and clear the file.
///   2. Enumerate any *remaining* Voxa-owned orphans and terminate them too.
///
/// Every step fails safe: the `proc` helpers never panic and never return errors —
/// enumeration yields an empty list on failure (so nothing is reaped on bad data), a
/// missing/corrupt PID file is ignored, and `terminate_pid` is a no-op on a dead PID.
/// This guarantees Requirement 1.4 / 5.1: a failure here logs and continues, never
/// blocking launch.
///
/// Safety: only PIDs reported by [`crate::proc::find_voxa_llama_servers`] are touched, so
/// the current instance's not-yet-spawned child and any unrelated `llama-server` are
/// never killed (Requirement 1.3).
fn reconcile_llama_servers(model_path_marker: &str, pid_file: Option<&std::path::Path>) {
    // Authoritative snapshot of live Voxa-owned servers. Used both to confirm the
    // PID-file server is still alive & ours and to find any other orphans.
    let voxa_pids = crate::proc::find_voxa_llama_servers(model_path_marker);

    // Step 1: reap the server named by the PID file, if it is still a live Voxa-owned
    // process. Either way, clear the (now stale) PID file.
    let mut reaped_from_pid_file: Option<u32> = None;
    if let Some(path) = pid_file {
        if let Some(pid) = crate::proc::read_pid_file(path) {
            if voxa_pids.contains(&pid) {
                log::info!("Startup reconcile: reaping tracked llama-server pid {} from PID file", pid);
                crate::proc::terminate_pid(pid);
                reaped_from_pid_file = Some(pid);
            } else {
                log::info!(
                    "Startup reconcile: PID file named {} but it is not a live Voxa-owned server; clearing",
                    pid
                );
            }
            crate::proc::remove_pid_file(path);
        }
    }

    // Step 2: reap any remaining Voxa-owned orphans (e.g. a crash that never wrote a PID
    // file, or stale servers from older sessions). Skip the one already handled above.
    for pid in voxa_pids {
        if Some(pid) == reaped_from_pid_file {
            continue;
        }
        log::info!("Startup reconcile: reaping orphaned Voxa-owned llama-server pid {}", pid);
        crate::proc::terminate_pid(pid);
    }
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
