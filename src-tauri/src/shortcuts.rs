// Shortcut registration and management.
// Owns NATIVE_SHORTCUTS global state and the three command-level functions.

use std::sync::{Mutex, OnceLock};
use tauri::Manager;
use crate::pipeline::{DictationEvent, DictationSender, RecordingState};
use crate::db::DbState;
use crate::db;

pub struct NativeShortcuts {
    pub ptt: String,
    pub hands_free: String,
    pub paste: String,
    pub cancel: String,
}

pub static NATIVE_SHORTCUTS: OnceLock<Mutex<NativeShortcuts>> = OnceLock::new();

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn apply_all_shortcuts(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{Shortcut, ShortcutState, GlobalShortcutExt};
    use std::str::FromStr;

    let db_state = app_handle.state::<DbState>();
    let settings = {
        let conn = db_state.conn.lock().unwrap();
        db::get_settings(&conn).unwrap_or_default()
    };

    let ptt_str   = settings.get("shortcut_push_to_talk").cloned().unwrap_or_else(|| "Alt+Space".to_string());
    let hf_str    = settings.get("shortcut_hands_free").cloned().unwrap_or_else(|| "F5".to_string());
    let paste_str = settings.get("shortcut_paste").cloned().unwrap_or_else(|| "CommandOrControl+Shift+V".to_string());
    let cancel_str= settings.get("shortcut_cancel").cloned().unwrap_or_else(|| "Escape".to_string());

    // Sync native shortcuts first
    if let Some(mutex) = NATIVE_SHORTCUTS.get() {
        if let Ok(mut native_shortcuts) = mutex.lock() {
            native_shortcuts.ptt        = ptt_str.clone();
            native_shortcuts.hands_free = hf_str.clone();
            native_shortcuts.paste      = paste_str.clone();
            native_shortcuts.cancel     = cancel_str.clone();
        }
    }

    let global_shortcut = app_handle.global_shortcut();
    let _ = global_shortcut.unregister_all();

    // Returns true for shortcuts that should be handled by the native CGEventTap
    // rather than registered with the Tauri global-shortcut plugin.
    let is_reserved = |s: &str, name: &str| {
        let is_hardware   = s == "Dictation" || s == "176" || s == "Function" || s == "179" || s == "F5";
        let has_modifiers = s.contains("CommandOrControl+") || s.contains("Alt+")
                         || s.contains("Control+") || s.contains("Shift+");
        let is_f_key      = s.starts_with('F') && s.len() > 1;

        if name == "paste" || name == "cancel" { return true; }

        is_hardware || (!has_modifiers && !is_f_key) || s == "Alt+Space" || s == "CommandOrControl+Space"
    };

    let register_and_handle = |shortcut_str: &str, name: &str, app_handle: &tauri::AppHandle| {
        if is_reserved(shortcut_str, name) { return; }

        if let Ok(shortcut) = Shortcut::from_str(shortcut_str) {
            let name_clone = name.to_string();
            let _ = app_handle.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, event| {
                if let Some(tx_state) = app.try_state::<DictationSender>() {
                    match name_clone.as_str() {
                        "push_to_talk" => {
                            if event.state() == ShortcutState::Pressed {
                                if let Ok(tx) = tx_state.0.lock() {
                                    app.state::<RecordingState>().0.store(true, std::sync::atomic::Ordering::SeqCst);
                                    let (pre, post) = crate::event_tap::get_cursor_context();
                                    let _ = tx.send(DictationEvent::StartRecording { pre_text: pre, post_text: post });
                                }
                            } else if event.state() == ShortcutState::Released {
                                if let Ok(tx) = tx_state.0.lock() {
                                    let rs = app.state::<RecordingState>();
                                    if rs.0.load(std::sync::atomic::Ordering::SeqCst) {
                                        rs.0.store(false, std::sync::atomic::Ordering::SeqCst);
                                        let _ = tx.send(DictationEvent::StopRecording);
                                    }
                                }
                            }
                        }
                        "hands_free" => {
                            if event.state() == ShortcutState::Pressed {
                                if let Ok(tx) = tx_state.0.lock() {
                                    let rs = app.state::<RecordingState>();
                                    let currently_recording = rs.0.load(std::sync::atomic::Ordering::SeqCst);
                                    if currently_recording {
                                        rs.0.store(false, std::sync::atomic::Ordering::SeqCst);
                                        let _ = tx.send(DictationEvent::StopRecording);
                                    } else {
                                        rs.0.store(true, std::sync::atomic::Ordering::SeqCst);
                                        let (pre, post) = crate::event_tap::get_cursor_context();
                                        let _ = tx.send(DictationEvent::StartRecording { pre_text: pre, post_text: post });
                                    }
                                }
                            }
                        }
                        "paste" => {
                            if event.state() == ShortcutState::Pressed {
                                if let Ok(conn) = app.state::<DbState>().conn.lock() {
                                    if let Ok(transcripts) = db::get_all_transcripts(&conn) {
                                        if let Some(last) = transcripts.first() {
                                            use tauri_plugin_clipboard_manager::ClipboardExt;
                                            let _ = app.clipboard().write_text(last.content.clone());
                                            crate::event_tap::simulate_paste();
                                        }
                                    }
                                }
                            }
                        }
                        "cancel" => {
                            if event.state() == ShortcutState::Pressed {
                                let rs = app.state::<RecordingState>();
                                if rs.0.load(std::sync::atomic::Ordering::SeqCst) {
                                    rs.0.store(false, std::sync::atomic::Ordering::SeqCst);
                                    if let Ok(tx) = tx_state.0.lock() {
                                        let _ = tx.send(DictationEvent::CancelRecording);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            });

            if let Err(e) = app_handle.global_shortcut().register(shortcut) {
                log::error!("Shortcut registration failed for {} ('{}'): {}", name, shortcut_str, e);
            }
        } else {
            log::error!("Invalid shortcut accelerator: '{}'", shortcut_str);
        }
    };

    log::info!("Shortcuts: PTT={} HF={} Paste={} Cancel={}", ptt_str, hf_str, paste_str, cancel_str);
    register_and_handle(&ptt_str,   "push_to_talk", &app_handle);
    register_and_handle(&hf_str,    "hands_free",   &app_handle);
    register_and_handle(&paste_str, "paste",        &app_handle);
    if cancel_str != "None" {
        register_and_handle(&cancel_str, "cancel", &app_handle);
    }

    Ok(())
}

#[tauri::command]
pub fn unregister_all_shortcuts(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    app_handle.global_shortcut().unregister_all().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_native_key_capture(app_handle: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        use tokio::sync::oneshot;
        let (tx, rx) = oneshot::channel();
        let tx_mutex = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));

        app_handle.run_on_main_thread(move || {
            use core_graphics::event::{
                CGEventTapLocation, CGEventTapPlacement, CGEventTapOptions,
                CGEventType, CGEventTapProxy, CGEvent, CGEventField, CGEventFlags,
            };
            use core_foundation::base::TCFType;

            let block_tx = tx_mutex.clone();

            let callback = move |_proxy: CGEventTapProxy, _type: CGEventType, event: &CGEvent| -> Option<CGEvent> {
                let is_system_event = (_type as u32) == 14;
                let key_code = event.get_integer_value_field(9 as CGEventField) as u16;
                let flags    = event.get_flags();
                let key_name = crate::event_tap::macos_keycode_to_name(key_code);

                if matches!(_type, CGEventType::KeyDown | CGEventType::KeyUp | CGEventType::FlagsChanged) || is_system_event {
                    if matches!(_type, CGEventType::KeyDown) || is_system_event {
                        let mut accel = String::new();
                        if flags.contains(CGEventFlags::CGEventFlagCommand)  { accel.push_str("CommandOrControl+"); }
                        if flags.contains(CGEventFlags::CGEventFlagAlternate){ accel.push_str("Alt+"); }
                        if flags.contains(CGEventFlags::CGEventFlagControl)  { accel.push_str("Control+"); }
                        if flags.contains(CGEventFlags::CGEventFlagShift)    { accel.push_str("Shift+"); }
                        accel.push_str(&key_name);

                        if let Ok(mut tx_guard) = block_tx.lock() {
                            if let Some(tx) = tx_guard.take() {
                                log::debug!("NATIVE TAP (CAPTURE): Successfully captured '{}'", accel);
                                let _ = tx.send(accel);
                            }
                        }
                        return None;
                    }
                    return None;
                }

                Some(event.clone())
            };

            let events = vec![
                CGEventType::KeyDown,
                CGEventType::KeyUp,
                CGEventType::FlagsChanged,
            ];

            let tap_result = core_graphics::event::CGEventTap::new(
                CGEventTapLocation::HID,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                events,
                callback,
            );

            match tap_result {
                Ok(tap) => {
                    tap.enable();
                    let tap_leaked = Box::leak(Box::new(tap));
                    if let Ok(loop_source) = tap_leaked.mach_port.create_runloop_source(0) {
                        unsafe {
                            let main_loop = core_foundation::runloop::CFRunLoopGetMain();
                            core_foundation::runloop::CFRunLoopAddSource(
                                main_loop,
                                loop_source.as_concrete_TypeRef() as *mut _,
                                core_foundation::runloop::kCFRunLoopCommonModes,
                            );
                        }
                    } else {
                        log::error!("Native tap: failed to create run loop source.");
                    }
                }
                Err(e) => {
                    log::error!("Native tap creation failed: {:?}", e);
                }
            }
        }).map_err(|e| e.to_string())?;

        rx.await.map_err(|_| "Capture failed. Make sure Voxa has Accessibility permissions in System Settings.".to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Native key capture is only supported on macOS.".to_string())
    }
}
