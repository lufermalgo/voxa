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
use std::sync::{Mutex, mpsc, atomic::AtomicBool, OnceLock};
use rusqlite::params;
use tauri::menu::Menu;
use tauri::tray::TrayIconBuilder;
use sys_locale::get_locale;


struct NativeShortcuts {
    ptt: String,
    hands_free: String,
    paste: String,
    cancel: String,
}

static NATIVE_SHORTCUTS: OnceLock<Mutex<NativeShortcuts>> = OnceLock::new();
static LAST_EVENT_TIME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static IS_PTT_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "macos")]
fn macos_keycode_to_name(keycode: u16) -> String {
    match keycode {
        36 => "Enter".to_string(),
        48 => "Tab".to_string(),
        49 => "Space".to_string(),
        51 => "Backspace".to_string(),
        53 => "Escape".to_string(),
        115 => "Home".to_string(),
        116 => "PageUp".to_string(),
        117 => "Delete".to_string(),
        119 => "End".to_string(),
        121 => "PageDown".to_string(),
        122 => "F1".to_string(),
        120 => "F2".to_string(),
        99  => "F3".to_string(),
        118 => "F4".to_string(),
        96  => "F5".to_string(),
        80  => "F5".to_string(),
        176 => "F5".to_string(), // Hardware Dictation/Microphone key
        97  => "F6".to_string(),
        98  => "F7".to_string(),
        100 => "F8".to_string(),
        101 => "F9".to_string(),
        109 => "F10".to_string(),
        103 => "F11".to_string(),
        111 => "F12".to_string(),
        123 => "Left".to_string(),
        124 => "Right".to_string(),
        125 => "Down".to_string(),
        126 => "Up".to_string(),
        179 => "F5".to_string(),
        160 => "MissionControl".to_string(),
        // Common Alphanumeric (A-Z)
        0   => "A".to_string(),
        1   => "S".to_string(),
        2   => "D".to_string(),
        3   => "F".to_string(),
        4   => "H".to_string(),
        5   => "G".to_string(),
        6   => "Z".to_string(),
        7   => "X".to_string(),
        8   => "C".to_string(),
        9   => "V".to_string(),
        11  => "B".to_string(),
        12  => "Q".to_string(),
        13  => "W".to_string(),
        14  => "E".to_string(),
        15  => "R".to_string(),
        16  => "Y".to_string(),
        17  => "T".to_string(),
        31  => "O".to_string(),
        32  => "U".to_string(),
        34  => "I".to_string(),
        35  => "P".to_string(),
        37  => "L".to_string(),
        38  => "J".to_string(),
        40  => "K".to_string(),
        45  => "N".to_string(),
        46  => "M".to_string(),
        _   => format!("Key_{}", keycode),
    }
}

// Helper to convert CGEventFlags to our internal representation
#[cfg(target_os = "macos")]
fn flags_to_string(flags: core_graphics::event::CGEventFlags) -> String {
    use core_graphics::event::CGEventFlags;
    let mut s = String::new();
    if flags.contains(CGEventFlags::CGEventFlagCommand) { s.push_str("CommandOrControl+"); }
    if flags.contains(CGEventFlags::CGEventFlagAlternate) { s.push_str("Alt+"); }
    if flags.contains(CGEventFlags::CGEventFlagControl) { s.push_str("Control+"); }
    if flags.contains(CGEventFlags::CGEventFlagShift) { s.push_str("Shift+"); }
    s
}

#[cfg(target_os = "macos")]
#[cfg(target_os = "macos")]
#[cfg(target_os = "macos")]
#[cfg(target_os = "macos")]
mod native_ffi {
    pub type CGEventRef = *mut std::os::raw::c_void;
    pub type CFMachPortRef = *mut std::os::raw::c_void;
    pub type CFRunLoopRef = *mut std::os::raw::c_void;
    pub type CFRunLoopSourceRef = *mut std::os::raw::c_void;
    pub type CFStringRef = *mut std::os::raw::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGEventTapCreate(
            tap: core_graphics::event::CGEventTapLocation,
            place: core_graphics::event::CGEventTapPlacement,
            options: core_graphics::event::CGEventTapOptions,
            eventsOfInterest: u64,
            callback: unsafe extern "C" fn(
                proxy: *mut std::os::raw::c_void,
                type_: u32,
                event: CGEventRef,
                refcon: *mut std::os::raw::c_void,
            ) -> CGEventRef,
            refcon: *mut std::os::raw::c_void,
        ) -> CFMachPortRef;

        pub fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
        pub fn CGEventGetFlags(event: CGEventRef) -> u64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn CFMachPortCreateRunLoopSource(allocator: *mut std::os::raw::c_void, port: CFMachPortRef, order: isize) -> CFRunLoopSourceRef;
        pub fn CFRunLoopGetMain() -> CFRunLoopRef;
        pub fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
        pub static kCFRunLoopCommonModes: CFStringRef;
    }
}

fn play_sound(name: &str) {
    let path = format!("/System/Library/Sounds/{}.aiff", name);
    let _ = std::process::Command::new("afplay")
        .arg(path)
        .spawn();
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn native_tap_callback(
    _proxy: *mut std::os::raw::c_void,
    _type: u32,
    event_ref: native_ffi::CGEventRef,
    refcon: *mut std::os::raw::c_void,
) -> native_ffi::CGEventRef {
    use core_graphics::event::CGEventFlags;

    let app_handle = &*(refcon as *const tauri::AppHandle);
    
    // NSSystemDefined: 14, KeyDown: 10, KeyUp: 11, FlagsChanged: 12
    let is_system_event = _type == 14;
    let is_key_down = _type == 10;
    let is_key_up = _type == 11;
    let is_flags_changed = _type == 12;

    // field 9 is kCGKeyboardEventKeycode
    let key_code = unsafe { native_ffi::CGEventGetIntegerValueField(event_ref, 9) } as u16;
    let raw_flags = unsafe { native_ffi::CGEventGetFlags(event_ref) };
    let flags = CGEventFlags::from_bits_truncate(raw_flags);
    let key_name = macos_keycode_to_name(key_code);
    
    if is_key_down || is_system_event {
        let mut current_accel = flags_to_string(flags);
        current_accel.push_str(&key_name);

        if let Some(shortcuts_mutex) = NATIVE_SHORTCUTS.get() {
            if let Ok(shortcuts) = shortcuts_mutex.lock() {
                let mut matched = false;
                let mut event_to_send = None;

                let has_modifiers = current_accel.contains("CommandOrControl+") || 
                                    current_accel.contains("Alt+") || 
                                    current_accel.contains("Control+") || 
                                    current_accel.contains("Shift+");
                let is_hardware_key = key_code == 176 || key_code == 179;

                if current_accel == shortcuts.ptt {
                    let is_autorepeat = unsafe { native_ffi::CGEventGetIntegerValueField(event_ref, 7) } != 0;
                    if is_autorepeat {
                        return std::ptr::null_mut();
                    }
                    
                    let is_recording = app_handle.state::<RecordingState>().0.load(std::sync::atomic::Ordering::SeqCst);
                    if !is_recording {
                        matched = true;
                        event_to_send = Some(DictationEvent::StartRecording);
                    } else {
                        // Already recording, just swallow the event to prevent system interference
                        return std::ptr::null_mut();
                    }
                } else if current_accel == shortcuts.hands_free || (key_code == 176 && (shortcuts.hands_free == "F5" || shortcuts.hands_free == "Dictation")) {
                    matched = true;
                    let is_recording = app_handle.state::<RecordingState>().0.load(std::sync::atomic::Ordering::SeqCst);
                    event_to_send = if is_recording { Some(DictationEvent::StopRecording) } else { Some(DictationEvent::StartRecording) };
                } else if current_accel == shortcuts.paste {
                    matched = true;
                } else if current_accel == shortcuts.cancel {
                    let is_recording = app_handle.state::<RecordingState>().0.load(std::sync::atomic::Ordering::SeqCst);
                    if is_recording {
                        matched = true;
                        event_to_send = Some(DictationEvent::CancelRecording);
                    }
                }

                if matched && !is_hardware_key && !has_modifiers {
                    if key_name != "Escape" || current_accel != shortcuts.cancel {
                        matched = false;
                        event_to_send = None;
                    }
                }

                if matched {
                    if current_accel == shortcuts.paste {
                        simulate_paste();
                    }
                    if key_code == 176 {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        let last = LAST_EVENT_TIME.load(std::sync::atomic::Ordering::SeqCst);
                        if now - last < 300 {
                            return std::ptr::null_mut();
                        }
                        LAST_EVENT_TIME.store(now, std::sync::atomic::Ordering::SeqCst);
                    }

                    if let Some(ev) = event_to_send {
                        if let Ok(tx) = app_handle.state::<DictationSender>().0.lock() {
                            match ev {
                                DictationEvent::StartRecording => {
                                    app_handle.state::<RecordingState>().0.store(true, std::sync::atomic::Ordering::SeqCst);
                                    if current_accel == shortcuts.ptt {
                                        IS_PTT_ACTIVE.store(true, std::sync::atomic::Ordering::SeqCst);
                                    }
                                    play_sound("Tink");
                                },
                                DictationEvent::StopRecording | DictationEvent::CancelRecording => {
                                    app_handle.state::<RecordingState>().0.store(false, std::sync::atomic::Ordering::SeqCst);
                                    IS_PTT_ACTIVE.store(false, std::sync::atomic::Ordering::SeqCst);
                                    play_sound("Pop");
                                }
                            }
                            let _ = tx.send(ev);
                        }
                    }
                    return std::ptr::null_mut();
                }
            }
        }
    } else if is_key_up {
        let mut current_accel = flags_to_string(flags);
        current_accel.push_str(&key_name);
        
        if let Some(shortcuts_mutex) = NATIVE_SHORTCUTS.get() {
            if let Ok(shortcuts) = shortcuts_mutex.lock() {
                if current_accel == shortcuts.ptt || IS_PTT_ACTIVE.load(std::sync::atomic::Ordering::SeqCst) {
                    if shortcuts.ptt.ends_with(&key_name) {
                        if let Ok(tx) = app_handle.state::<DictationSender>().0.lock() {
                            app_handle.state::<RecordingState>().0.store(false, std::sync::atomic::Ordering::SeqCst);
                            IS_PTT_ACTIVE.store(false, std::sync::atomic::Ordering::SeqCst);
                            let _res = tx.send(DictationEvent::StopRecording);
                            play_sound("Pop");
                        }
                        return std::ptr::null_mut();
                    }
                }
                if current_accel == shortcuts.hands_free || (key_code == 176 && (shortcuts.hands_free == "F5" || shortcuts.hands_free == "Dictation")) {
                    return std::ptr::null_mut();
                }
            }
        }
    } else if is_flags_changed {
        if IS_PTT_ACTIVE.load(std::sync::atomic::Ordering::SeqCst) {
            if let Some(shortcuts_mutex) = NATIVE_SHORTCUTS.get() {
                if let Ok(shortcuts) = shortcuts_mutex.lock() {
                    let current_modifiers = flags_to_string(flags);
                    if !shortcuts.ptt.starts_with(&current_modifiers) {
                        if let Ok(tx) = app_handle.state::<DictationSender>().0.lock() {
                            app_handle.state::<RecordingState>().0.store(false, std::sync::atomic::Ordering::SeqCst);
                            IS_PTT_ACTIVE.store(false, std::sync::atomic::Ordering::SeqCst);
                            let _res = tx.send(DictationEvent::StopRecording);
                            play_sound("Pop");
                        }
                    }
                }
            }
        }
    }
    
    event_ref
}

#[cfg(target_os = "macos")]
fn setup_native_event_tap(app_handle: tauri::AppHandle) {
    use core_graphics::event::{CGEventTapLocation, CGEventTapPlacement, CGEventTapOptions};
    
    // Leak the AppHandle so the raw pointer remains valid
    let handle_ptr = Box::into_raw(Box::new(app_handle));
    
    // Build the mask manually: 
    // KeyDown (10), KeyUp (11), FlagsChanged (12), NSSystemDefined (14)
    let mask = (1 << 10) | (1 << 11) | (1 << 12) | (1 << 14);
    
    unsafe {
        let tap_port = native_ffi::CGEventTapCreate(
            CGEventTapLocation::HID,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            mask,
            native_tap_callback,
            handle_ptr as *mut _,
        );

        if !tap_port.is_null() {
            let loop_source_ref = native_ffi::CFMachPortCreateRunLoopSource(std::ptr::null_mut(), tap_port, 0);
            if !loop_source_ref.is_null() {
                let main_loop = native_ffi::CFRunLoopGetMain();
                native_ffi::CFRunLoopAddSource(
                    main_loop,
                    loop_source_ref,
                    native_ffi::kCFRunLoopCommonModes
                );
                println!("NATIVE TAP: Persistent FFI tap initialized (Mask: {:#b}).", mask);
            }
        } else {
            println!("NATIVE TAP ERROR: Failed to create FFI event tap. Check Accessibility permissions.");
        }
    }
}



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

#[derive(Debug, Clone, Copy)]
pub enum DictationEvent {
    StartRecording,
    StopRecording,
    CancelRecording,
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
async fn start_recording(app: tauri::AppHandle, engine: State<'_, AudioEngine>, db_state: State<'_, DbState>) -> Result<(), String> {
    let mic_id = {
        let conn = db_state.conn.lock().unwrap();
        db::get_settings(&conn).unwrap_or_default().get("mic_id").cloned()
    };
    audio::setup_stream(&engine, mic_id)?;
    app.state::<RecordingState>().0.store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
async fn stop_recording(app: tauri::AppHandle, engine: State<'_, AudioEngine>, db_state: State<'_, DbState>) -> Result<Vec<f32>, String> {
    let mic_id = {
        let conn = db_state.conn.lock().unwrap();
        db::get_settings(&conn).unwrap_or_default().get("mic_id").cloned()
    };
    let samples = audio::stop_stream(&engine, mic_id)?;
    app.state::<RecordingState>().0.store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(samples)
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
fn apply_all_shortcuts(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{Shortcut, ShortcutState, GlobalShortcutExt};
    use std::str::FromStr;

    let db_state = app_handle.state::<DbState>();
    let settings = {
        let conn = db_state.conn.lock().unwrap();
        db::get_settings(&conn).unwrap_or_default()
    };
    
    let ptt_str = settings.get("shortcut_push_to_talk").cloned().unwrap_or_else(|| "Alt+Space".to_string());
    let hf_str = settings.get("shortcut_hands_free").cloned().unwrap_or_else(|| "F5".to_string());
    let paste_str = settings.get("shortcut_paste").cloned().unwrap_or_else(|| "CommandOrControl+Shift+V".to_string());
    let cancel_str = settings.get("shortcut_cancel").cloned().unwrap_or_else(|| "Escape".to_string());
    
    // --- NATIVE SHORTCUTS SYNC (Do this first) ---
    if let Some(mutex) = NATIVE_SHORTCUTS.get() {
        if let Ok(mut native_shortcuts) = mutex.lock() {
            println!("DEBUG: Syncing shortcuts - PTT: '{}', HF: '{}', Paste: '{}', Cancel: '{}'", ptt_str, hf_str, paste_str, cancel_str);
            native_shortcuts.ptt = ptt_str.clone();
            native_shortcuts.hands_free = hf_str.clone();
            native_shortcuts.paste = paste_str.clone();
            native_shortcuts.cancel = cancel_str.clone();
            println!("SHORTCUT: Native state synchronized.");
        }
    }

    let global_shortcut = app_handle.global_shortcut();
    let _ = global_shortcut.unregister_all();
    
    // Helper to check if a shortcut should be handled NATIVELY (bypassing Tauri plugin)
    // We reserve all hardware keys, BARE keys (no modifiers), and specific shortcuts
    // like 'paste' and 'cancel' to avoid registration conflicts and swallowing typing.
    let is_reserved = |s: &str, name: &str| {
        let is_hardware = s == "Dictation" || s == "176" || s == "Function" || s == "179" || s == "F5";
        let has_modifiers = s.contains("CommandOrControl+") || s.contains("Alt+") || 
                            s.contains("Control+") || s.contains("Shift+");
        let is_f_key = s.starts_with("F") && s.len() > 1; // F1, F2...
        
        // Explicitly reserve paste and cancel to be handled by native tap (bypasses plugin errors)
        if name == "paste" || name == "cancel" {
            return true;
        }
        
        // Reserve if it's hardware (including F5), OR if it's a bare key that isn't a function key, OR known system conflicts
        is_hardware || (!has_modifiers && !is_f_key) || s == "Alt+Space" || s == "CommandOrControl+Space"
    };

    let register_and_handle = |shortcut_str: &str, name: &str, app_handle: &tauri::AppHandle| {
        if is_reserved(shortcut_str, name) {
            println!("SHORTCUT: {} is reserved ({}), handled by native tap.", name, shortcut_str);
            return;
        }

        if let Ok(shortcut) = Shortcut::from_str(shortcut_str) {
            // Attach handler
            let name_clone = name.to_string();
            let _ = app_handle.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, event| {
                if let Some(tx_state) = app.try_state::<DictationSender>() {
                    match name_clone.as_str() {
                        "push_to_talk" => {
                            if event.state() == ShortcutState::Pressed {
                                if let Ok(tx) = tx_state.0.lock() {
                                    let _ = app.state::<RecordingState>().0.store(true, std::sync::atomic::Ordering::SeqCst);
                                    let _ = tx.send(DictationEvent::StartRecording);
                                }
                            } else if event.state() == ShortcutState::Released {
                                if let Ok(tx) = tx_state.0.lock() {
                                    let is_recording_state = app.state::<RecordingState>();
                                    if is_recording_state.0.load(std::sync::atomic::Ordering::SeqCst) {
                                        is_recording_state.0.store(false, std::sync::atomic::Ordering::SeqCst);
                                        let _ = tx.send(DictationEvent::StopRecording);
                                    }
                                }
                            }
                        },
                        "hands_free" => {
                            if event.state() == ShortcutState::Pressed {
                                if let Ok(tx) = tx_state.0.lock() {
                                    let is_recording_state = app.state::<RecordingState>();
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
                        },
                        "paste" => {
                            if event.state() == ShortcutState::Pressed {
                                if let Ok(conn) = app.state::<DbState>().conn.lock() {
                                    if let Ok(transcripts) = db::get_all_transcripts(&conn) {
                                        if let Some(last) = transcripts.first() {
                                            use tauri_plugin_clipboard_manager::ClipboardExt;
                                            let _ = app.clipboard().write_text(last.content.clone());
                                            crate::simulate_paste();
                                        }
                                    }
                                }
                            }
                        },
                        "cancel" => {
                            if event.state() == ShortcutState::Pressed {
                                let is_recording_state = app.state::<RecordingState>();
                                if is_recording_state.0.load(std::sync::atomic::Ordering::SeqCst) {
                                    is_recording_state.0.store(false, std::sync::atomic::Ordering::SeqCst);
                                    if let Ok(tx) = tx_state.0.lock() {
                                        let _ = tx.send(DictationEvent::CancelRecording);
                                    }
                                }
                            }
                        },
                        _ => {}
                    }
                }
            });

            if let Err(e) = app_handle.global_shortcut().register(shortcut) {
                println!("SHORTCUT ERROR: Failed to register {} with string '{}': {}", name, shortcut_str, e);
            } else {
                println!("SHORTCUT: {} ('{}') registered successfully.", name, shortcut_str);
            }
        } else {
            println!("SHORTCUT ERROR: Invalid accelerator: '{}'", shortcut_str);
        }
    };

    println!("--- SHORTCUT MIGRATION START ---");
    register_and_handle(&ptt_str, "push_to_talk", &app_handle);
    register_and_handle(&hf_str, "hands_free", &app_handle);
    register_and_handle(&paste_str, "paste", &app_handle);
    
    if cancel_str != "None" {
        register_and_handle(&cancel_str, "cancel", &app_handle);
    }

    Ok(())
}

#[tauri::command]
fn unregister_all_shortcuts(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    app_handle.global_shortcut().unregister_all().map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_native_key_capture(app_handle: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        use tokio::sync::oneshot;
        let (tx, rx) = oneshot::channel();
        let tx_mutex = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));

        app_handle.run_on_main_thread(move || {
            println!("NATIVE TAP: Initializing tap on main thread...");
            
            use core_graphics::event::{
                CGEventTapLocation, CGEventTapPlacement, CGEventTapOptions, 
                CGEventType, CGEventTapProxy, CGEvent, CGEventField, CGEventFlags
            };
            use core_foundation::base::TCFType;
            
            let block_tx = tx_mutex.clone();

            let callback = move |_proxy: CGEventTapProxy, _type: CGEventType, event: &CGEvent| -> Option<CGEvent> {
                let is_system_event = (_type as u32) == 14;
                let key_code = event.get_integer_value_field(9 as CGEventField) as u16;
                let flags = event.get_flags();
                let key_name = macos_keycode_to_name(key_code);
                
                if matches!(_type, CGEventType::KeyDown | CGEventType::KeyUp | CGEventType::FlagsChanged) || is_system_event {
                    // Filter: Allow capturing if it's a KeyDown OR is a system event
                    if matches!(_type, CGEventType::KeyDown) || is_system_event {
                        let mut accel = String::new();
                        if flags.contains(CGEventFlags::CGEventFlagCommand) { accel.push_str("CommandOrControl+"); }
                        if flags.contains(CGEventFlags::CGEventFlagAlternate) { accel.push_str("Alt+"); }
                        if flags.contains(CGEventFlags::CGEventFlagControl) { accel.push_str("Control+"); }
                        if flags.contains(CGEventFlags::CGEventFlagShift) { accel.push_str("Shift+"); }
                        accel.push_str(&key_name);

                        if let Ok(mut tx_guard) = block_tx.lock() {
                            if let Some(tx) = tx_guard.take() {
                                println!("NATIVE TAP (CAPTURE): Successfully captured '{}'", accel);
                                let _ = tx.send(accel);
                            }
                        }
                        return None;
                    }
                    // Swallow to prevent system action during capture
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
                callback
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
                                core_foundation::runloop::kCFRunLoopCommonModes
                            );
                        }
                        println!("NATIVE TAP: Tap registered in main run loop (HID level).");
                    } else {
                        println!("NATIVE TAP ERROR: Failed to create run loop source.");
                    }
                },
                Err(e) => {
                    println!("NATIVE TAP ERROR: Failed to create: {:?}", e);
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

// macOS Accessibility permission check
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

#[tauri::command]
fn check_accessibility_permissions() -> bool {
    let trusted = unsafe { AXIsProcessTrusted() };
    println!("PERMISSIONS: Accessibility trusted: {}", trusted);
    trusted
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
                                 Ok(s) => {
                                     println!("PIPELINE: Stopped audio stream. Captured {} samples.", s.len());
                                     s
                                 },
                                 Err(e) => {
                                     println!("PIPELINE ERROR: Failed to stop stream: {}", e);
                                     let _ = app_clone.emit("pipeline-error", e);
                                     let _ = app_clone.emit("pipeline-status", "idle");
                                     continue;
                                 }
                             };

                            if samples.is_empty() { 
                                let _ = app_clone.emit("pipeline-status", "idle");
                                continue; 
                            }

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
                                            let _ = app_clone.emit("pipeline-status", "idle");
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
                                        let _ = app_clone.emit("pipeline-status", "idle");
                                        continue;
                                    }
                                }
                            };

                            if raw_text.is_empty() { 
                                let _ = app_clone.emit("pipeline-status", "idle");
                                continue; 
                            }
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

                            println!("PIPELINE: Copying result to clipboard and simulating paste.");
                            use tauri_plugin_clipboard_manager::ClipboardExt;
                            app_clone.clipboard().write_text(refined_text.clone()).unwrap_or_else(|e| {
                                println!("PIPELINE ERROR: Clipboard Error: {}", e);
                                let _ = app_clone.emit("pipeline-error", format!("Clipboard Error: {}", e));
                            });
                            
                            simulate_paste();

                            let _ = app_clone.emit("pipeline-results", &refined_text);
                            println!("PIPELINE: DONE. Resetting status to idle.");
                            let _ = app_clone.emit("pipeline-status", "idle");
                        }
                        DictationEvent::CancelRecording => {
                            let _ = app_clone.emit("pipeline-status", "idle");
                            let audio_engine = app_clone.state::<AudioEngine>();
                            let db_state = app_clone.state::<DbState>();
                            let mic_id = {
                                let conn = db_state.conn.lock().unwrap();
                                db::get_settings(&conn).unwrap_or_default().get("mic_id").cloned()
                            };
                            let _ = audio::stop_stream(&audio_engine, mic_id);
                            // We do nothing with the samples, just discard
                        }
                    }
                }
            });
            
            // Initialize Native Event Tap for special keys (Mic/Dictation)
            // Check permissions first
            let is_trusted = unsafe { AXIsProcessTrusted() };
            if !is_trusted {
                println!("WARNING: Accessibility permissions NOT granted. Key capture will NOT work.");
                println!("Go to System Settings > Privacy & Security > Accessibility and add the terminal or app.");
            }
            
            setup_native_event_tap(app.handle().clone());

            // Call apply_all_shortcuts during setup
            if let Err(e) = apply_all_shortcuts(app.handle().clone()) {
                eprintln!("Failed to register global shortcuts on startup: {}", e);
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
            apply_all_shortcuts,
            unregister_all_shortcuts,
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
            exit_app,
            start_native_key_capture,
            check_accessibility_permissions
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
