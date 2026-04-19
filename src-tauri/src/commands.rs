// All #[tauri::command] functions (excluding shortcut commands, which live in shortcuts.rs).

use std::sync::Arc;
use tauri::{State, Emitter};
use rusqlite::params;

use tauri::Manager;
use crate::audio::{self, AudioEngine};
use crate::db::{self, DbState, SettingsCache, Transcript};
use crate::pipeline::{DictationEvent, DictationSender, EngineState, ManualProfileOverride, RecordingState, FrontmostApp};

// ---------------------------------------------------------------------------
// Transcripts
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_transcripts(state: State<'_, DbState>) -> Result<Vec<Transcript>, String> {
    let conn = Arc::clone(&state.conn);
    tokio::task::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::get_all_transcripts(&guard).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_transcript(state: State<'_, DbState>, id: i64) -> Result<(), String> {
    let conn = Arc::clone(&state.conn);
    tokio::task::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::delete_transcript(&guard, id).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn clear_transcripts(state: State<'_, DbState>) -> Result<(), String> {
    let conn = Arc::clone(&state.conn);
    tokio::task::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::clear_all_transcripts(&guard).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn update_transcript(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    id: i64,
    new_content: String,
    raw_content: String,
) -> Result<Vec<String>, String> {
    let conn              = Arc::clone(&state.conn);
    let new_content_clone = new_content.clone();
    let raw_content_clone = raw_content.clone();
    let learned = tokio::task::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::update_transcript_content(&guard, id, &new_content_clone).map_err(|e| e.to_string())?;
        let learned = extract_new_words(&raw_content_clone, &new_content_clone);
        for word in &learned {
            let _ = guard.execute(
                "INSERT OR IGNORE INTO custom_dict (word) VALUES (?1)",
                params![word],
            );
        }
        Ok::<Vec<String>, String>(learned)
    }).await.map_err(|e| e.to_string())??;

    if !learned.is_empty() {
        let _ = app.emit("dictionary-updated", &learned);
        log::info!("Added to dictionary: {}", learned.join(", "));
    }

    Ok(learned)
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_settings(
    state: State<'_, DbState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let conn = Arc::clone(&state.conn);
    tokio::task::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::get_settings(&guard).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn update_setting(
    app: tauri::AppHandle,
    state: tauri::State<DbState>,
    cache: tauri::State<SettingsCache>,
    key: String,
    value: String,
) -> Result<(), String> {
    let conn = state.conn.lock().unwrap();
    db::update_setting(&conn, &key, &value).map_err(|e| e.to_string())?;
    cache.invalidate(&key, &value);
    let _ = app.emit("settings-updated", ());
    Ok(())
}

// ---------------------------------------------------------------------------
// Audio devices
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_audio_devices() -> Result<Vec<audio::AudioDevice>, String> {
    audio::get_input_devices()
}

// ---------------------------------------------------------------------------
// Profiles
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_profiles(state: State<'_, DbState>) -> Result<Vec<db::Profile>, String> {
    let conn = Arc::clone(&state.conn);
    tokio::task::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::get_profiles(&guard).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn update_profile(
    app: tauri::AppHandle,
    state: tauri::State<DbState>,
    id: i64,
    name: String,
    prompt: String,
    icon: Option<String>,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::update_profile(&conn, id, &name, &prompt, icon).map_err(|e| e.to_string())?;
    let _ = app.emit("profiles-updated", ());
    Ok(())
}

#[tauri::command]
pub fn create_profile(
    app: tauri::AppHandle,
    state: tauri::State<DbState>,
    name: String,
    prompt: String,
    icon: Option<String>,
) -> Result<i64, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let id = db::create_profile(&conn, &name, &prompt, icon).map_err(|e| e.to_string())?;
    let _ = app.emit("profiles-updated", ());
    Ok(id)
}

#[tauri::command]
pub fn delete_profile(
    app: tauri::AppHandle,
    state: tauri::State<DbState>,
    id: i64,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::delete_profile(&conn, id).map_err(|e| e.to_string())?;
    let _ = app.emit("profiles-updated", ());
    Ok(())
}

/// Set (or clear) the manual profile override for this session.
/// Pass `None` to clear and let auto-detection resume.
#[tauri::command]
pub fn set_manual_profile_override(
    app: tauri::AppHandle,
    profile_name: Option<String>,
) -> Result<(), String> {
    *app.state::<ManualProfileOverride>().0.lock().unwrap() = profile_name;
    Ok(())
}

#[tauri::command]
pub fn update_profile_formatting_mode(
    app: tauri::AppHandle,
    state: tauri::State<DbState>,
    id: i64,
    mode: String,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::update_profile_formatting_mode(&conn, id, &mode).map_err(|e| e.to_string())?;
    let _ = app.emit("profiles-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn submit_correction(
    db_state: State<'_, DbState>,
    engine_state: State<'_, EngineState>,
    profile_id: i64,
    original_text: String,
    corrected_text: String,
) -> Result<(), String> {
    let hint = {
        let mut guard = engine_state.llama.lock().map_err(|e| e.to_string())?;
        if let Some(ref mut llama) = *guard {
            let system = "Extract ONE formatting rule from the difference between Original and Corrected text. Return a single imperative instruction, max 15 words. Return ONLY the instruction.";
            let input = format!("Original: {}\nCorrected: {}", original_text, corrected_text);
            llama.refine_text(&input, system, "en", "", "").unwrap_or_default()
        } else {
            String::new()
        }
    };
    if hint.trim().is_empty() { return Ok(()); }
    let pattern: String = original_text.chars().take(60).collect();
    let conn = db_state.conn.lock().map_err(|e| e.to_string())?;
    db::upsert_formatting_hint(&conn, profile_id, &pattern, hint.trim()).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Custom dictionary
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_custom_dictionary(state: State<'_, DbState>) -> Result<Vec<String>, String> {
    let conn = Arc::clone(&state.conn);
    tokio::task::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::get_custom_dictionary(&guard).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_dictionary_entries(state: State<'_, DbState>) -> Result<Vec<db::DictionaryEntry>, String> {
    let conn = Arc::clone(&state.conn);
    tokio::task::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::get_dictionary_entries(&guard).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn update_replacement_word(state: State<'_, DbState>, word: String, replacement: Option<String>) -> Result<(), String> {
    let conn = Arc::clone(&state.conn);
    tokio::task::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::update_replacement_word(&guard, &word, replacement.as_deref()).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn add_to_dictionary(state: State<'_, DbState>, word: String) -> Result<(), String> {
    let conn = Arc::clone(&state.conn);
    tokio::task::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        guard
            .execute("INSERT OR IGNORE INTO custom_dict (word) VALUES (?1)", params![word])
            .map_err(|e| e.to_string())?;
        Ok(())
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn remove_from_dictionary(state: State<'_, DbState>, word: String) -> Result<(), String> {
    let conn = Arc::clone(&state.conn);
    tokio::task::spawn_blocking(move || {
        let guard = conn.lock().map_err(|e| e.to_string())?;
        db::remove_from_dictionary(&guard, &word).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

// ---------------------------------------------------------------------------
// Recording controls
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn set_window_interactive(app: tauri::AppHandle, interactive: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        let win: tauri::WebviewWindow = window;
        win.set_ignore_cursor_events(!interactive).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn stop_and_transcribe(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let pid = app.state::<FrontmostApp>().0.lock().unwrap().pid;
        crate::event_tap::activate_app_by_pid(pid);
    }
    let sender = app.state::<DictationSender>();
    let _ = sender.0.lock().unwrap().send(DictationEvent::StopRecording);
    Ok(())
}

#[tauri::command]
pub fn cancel_recording(app: tauri::AppHandle, engine: State<'_, AudioEngine>) -> Result<(), String> {
    let sender = app.state::<DictationSender>();
    let _ = sender.0.lock().unwrap().send(DictationEvent::CancelRecording);
    let _ = audio::stop_stream(&engine, None);
    app.state::<RecordingState>().0.store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

// ---------------------------------------------------------------------------
// Settings window
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn show_settings(app: tauri::AppHandle, tab: Option<String>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        #[cfg(target_os = "macos")]
        unsafe {
            let ns_app: cocoa::base::id = msg_send![class!(NSApplication), sharedApplication];
            let () = msg_send![ns_app, activateIgnoringOtherApps: true as objc::runtime::BOOL];
        }
        let _ = window.set_focus();
        if let Some(t) = tab {
            let _ = window.emit("show-tab", t);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

#[tauri::command]
pub fn check_accessibility_permissions() -> bool {
    unsafe { AXIsProcessTrusted() }
}

#[tauri::command]
pub fn get_system_locale() -> String {
    use sys_locale::get_locale;
    get_locale()
        .unwrap_or_else(|| "en".to_string())
        .split('-')
        .next()
        .unwrap_or("en")
        .to_string()
}

#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    use crate::pipeline::PipelineHandle;
    app.state::<PipelineHandle>()
        .cancelled
        .store(true, std::sync::atomic::Ordering::SeqCst);
    std::thread::sleep(std::time::Duration::from_millis(200));
    app.exit(0);
}

// ---------------------------------------------------------------------------
// Word-learning helper (used by update_transcript)
// ---------------------------------------------------------------------------

fn extract_new_words(raw: &str, corrected: &str) -> Vec<String> {
    const SKIP: &[&str] = &[
        "a","an","the","and","or","but","in","on","at","to","for",
        "of","with","is","it","i","my","me","we","you","he","she",
        "they","this","that","was","are","be","as","by","from","un",
        "el","la","los","las","de","del","en","y","o","que","se",
        "no","si","su","al","es","por","con","le","lo","una","pero",
    ];

    let raw_words: std::collections::HashSet<String> = raw
        .split(|c: char| !c.is_alphanumeric() && c != '.' && c != '+' && c != '#' && c != '-')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect();

    corrected
        .split(|c: char| !c.is_alphanumeric() && c != '.' && c != '+' && c != '#' && c != '-')
        .filter(|w| w.len() >= 2)
        .filter(|w| !SKIP.contains(&w.to_lowercase().as_str()))
        .filter(|w| !raw_words.contains(&w.to_lowercase()))
        .map(|w| w.to_string())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

// ---------------------------------------------------------------------------
// Pill window resize (warning card expand/collapse)
// ---------------------------------------------------------------------------

/// Expands the pill window upward by `extra_height` physical pixels,
/// moving the window position up by the same amount so the pill stays fixed.
/// Call with expand=true to show warning card, expand=false to collapse.
#[tauri::command]
pub fn set_pill_warning_mode(app: tauri::AppHandle, expand: bool) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("main window not found")?;
    let extra_height: i32 = 200; // physical pixels for the warning card

    let current_pos = window.outer_position().map_err(|e| e.to_string())?;
    let current_size = window.outer_size().map_err(|e| e.to_string())?;

    if expand {
        // Move window up by extra_height, increase height by extra_height
        let new_y = current_pos.y - extra_height;
        let new_height = current_size.height as i32 + extra_height;
        window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(current_pos.x, new_y)))
            .map_err(|e| e.to_string())?;
        window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(current_size.width, new_height as u32)))
            .map_err(|e| e.to_string())?;
    } else {
        // Move window down by extra_height, decrease height by extra_height
        let new_y = current_pos.y + extra_height;
        let new_height = (current_size.height as i32 - extra_height).max(80) as u32;
        window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(current_pos.x, new_y)))
            .map_err(|e| e.to_string())?;
        window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(current_size.width, new_height)))
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}



/// Returns the frontmost app's name and icon (base64 PNG) on demand.
/// Useful for displaying app context in UI without waiting for a recording event.
#[tauri::command]
pub fn get_active_app() -> Option<serde_json::Value> {
    #[cfg(target_os = "macos")]
    {
        let pid = crate::event_tap::get_frontmost_app_pid()?;
        let info = crate::event_tap::get_app_info_for_pid(pid)?;
        Some(serde_json::json!({
            "name": info.name,
            "icon": info.icon_base64,
        }))
    }
    #[cfg(not(target_os = "macos"))]
    None
}

