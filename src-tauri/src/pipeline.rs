// Dictation pipeline — state types, pipeline loop, and cancellation handle.

use std::sync::{Arc, Mutex, mpsc, atomic::{AtomicBool, Ordering}};
use tauri::{Manager, Emitter};
use crate::audio::{self, AudioEngine};
use crate::db::{self, DbState, SettingsCache};
use crate::llama_inference::{self, LlamaEngine};
use crate::whisper_inference;
use crate::models;

// ---------------------------------------------------------------------------
// State types (pub — used by commands and event_tap)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum DictationEvent {
    StartRecording { pre_text: String, post_text: String },
    StopRecording,
    CancelRecording,
}

pub struct DictationSender(pub Mutex<mpsc::Sender<DictationEvent>>);
pub struct RecordingState(pub AtomicBool);
pub struct FrontmostApp(pub Mutex<i32>); // PID of the app that was active when recording started
pub struct ManualProfileOverride(pub Mutex<Option<String>>); // profile name set explicitly by user this session

pub struct EngineState {
    pub whisper: Mutex<Option<whisper_inference::WhisperEngine>>,
    pub llama:   Mutex<Option<llama_inference::LlamaEngine>>,
}

/// Managed state that allows graceful shutdown of background threads.
pub struct PipelineHandle {
    pub cancelled: Arc<AtomicBool>,
}

/// Cursor context captured at recording start — passed to LLM at refinement time.
pub struct CursorContext {
    pub pre_text:  Mutex<String>,
    pub post_text: Mutex<String>,
}

// ---------------------------------------------------------------------------
// Auto-profile detection
// ---------------------------------------------------------------------------

/// Returns the bundle ID of the running application with the given PID, or None.
#[cfg(target_os = "macos")]
fn bundle_id_for_pid(pid: i32) -> Option<String> {
    if pid <= 0 { return None; }
    unsafe {
        let running_app: cocoa::base::id = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: pid
        ];
        if running_app.is_null() { return None; }
        let bundle_id: cocoa::base::id = msg_send![running_app, bundleIdentifier];
        if bundle_id.is_null() { return None; }
        let bytes: *const std::os::raw::c_char = msg_send![bundle_id, UTF8String];
        if bytes.is_null() { return None; }
        Some(std::ffi::CStr::from_ptr(bytes).to_string_lossy().into_owned())
    }
}

#[cfg(not(target_os = "macos"))]
fn bundle_id_for_pid(_pid: i32) -> Option<String> { None }

/// Maps a bundle ID to a profile name keyword used in `detect_profile_for_pid`.
fn bundle_id_to_profile_keyword(bundle_id: &str) -> Option<&'static str> {
    let b = bundle_id.to_lowercase();
    // Code editors / IDEs
    if b == "com.apple.dt.xcode"
        || b == "com.microsoft.vscode"
        || b == "com.todesktop.230313mzl4w4u92" // Cursor
        || b.starts_with("com.jetbrains.")
    {
        return Some("Code");
    }
    // Chat / messaging
    if b == "com.tinyspeck.slackmacgap"
        || b == "com.hnc.discord"
        || b == "com.microsoft.teams2"
        || b == "ru.keepcoder.telegram"
    {
        return Some("Informal");
    }
    // Notes / writing
    if b == "com.apple.notes"
        || b == "notion.id"
        || b == "com.evernote.evernote"
        || b == "md.obsidian"
    {
        return Some("Elegant");
    }
    // Email
    if b == "com.apple.mail" || b == "com.microsoft.outlook" {
        return Some("Elegant");
    }
    None
}

/// Given a PID, returns the best matching profile name (keyword) or None if no match.
pub fn detect_profile_keyword_for_pid(pid: i32) -> Option<&'static str> {
    let bundle_id = bundle_id_for_pid(pid)?;
    log::debug!("Auto-profile: bundle_id={}", bundle_id);
    bundle_id_to_profile_keyword(&bundle_id)
}

// ---------------------------------------------------------------------------
// LLM helper — eliminates duplicated refine_text blocks
// ---------------------------------------------------------------------------

fn run_llm_refinement(
    llama: &mut LlamaEngine,
    raw_text: &str,
    system_prompt: &str,
    pre_text: &str,
    post_text: &str,
    app: &tauri::AppHandle,
) -> String {
    match llama.refine_text(raw_text, system_prompt, pre_text, post_text) {
        Ok(refined) => refined,
        Err(e) => {
            log::error!("LLM refinement failed: {}", e);
            let _ = app.emit("pipeline-error", format!("Refinement Error: {}", e));
            raw_text.to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// Profile resolution
// ---------------------------------------------------------------------------

/// Resolves the system_prompt to use for this dictation.
/// Priority: manual override > auto-detected by bundle ID > active_profile_id setting.
fn resolve_system_prompt(app: &tauri::AppHandle, db_state: &db::DbState) -> (String, String) {
    let conn = db_state.conn.lock().unwrap();

    // Check if auto-detect is enabled in settings
    let auto_detect_enabled = app
        .state::<db::SettingsCache>()
        .get("auto_detect_profile")
        .map(|v| v != "false")
        .unwrap_or(true);

    // Check manual override
    let manual_override = app
        .state::<ManualProfileOverride>()
        .0
        .lock()
        .unwrap()
        .clone();

    if let Some(ref name) = manual_override {
        // User explicitly chose a profile — find it by name
        if let Ok(profiles) = db::get_profiles(&conn) {
            if let Some(p) = profiles.into_iter().find(|p| &p.name == name) {
                return (p.system_prompt, p.name);
            }
        }
    }

    if auto_detect_enabled {
        let pid = *app.state::<FrontmostApp>().0.lock().unwrap();
        if let Some(keyword) = detect_profile_keyword_for_pid(pid) {
            if let Ok(profiles) = db::get_profiles(&conn) {
                if let Some(p) = profiles.into_iter().find(|p| p.name == keyword) {
                    log::info!("Auto-profile: matched '{}' for PID {}", keyword, pid);
                    return (p.system_prompt, p.name);
                }
            }
        }
    }

    // Fall back to the user's currently selected profile
    let p = db::get_active_profile(&conn).unwrap_or_default();
    let name = p.as_ref().map(|x| x.name.clone()).unwrap_or_default();
    let prompt = p.map(|x| x.system_prompt).unwrap_or_default();
    (prompt, name)
}

// ---------------------------------------------------------------------------
// Pipeline loop
// ---------------------------------------------------------------------------

pub fn start_pipeline(app: tauri::AppHandle, rx: mpsc::Receiver<DictationEvent>) {
    std::thread::spawn(move || {
        for event in rx {
            // Check cancellation flag before processing each event
            if app.state::<PipelineHandle>().cancelled.load(Ordering::SeqCst) {
                break;
            }

            match event {
                DictationEvent::StartRecording { pre_text, post_text } => {
                    // Store cursor context so StopRecording can pass it to the LLM
                    {
                        let ctx = app.state::<CursorContext>();
                        *ctx.pre_text.lock().unwrap()  = pre_text;
                        *ctx.post_text.lock().unwrap() = post_text;
                    }
                    if let Some(audio_engine) = app.try_state::<AudioEngine>() {
                        let mic_id = app.state::<SettingsCache>().get("mic_id");
                        match audio::setup_stream(&audio_engine, mic_id) {
                            Ok(_) => {
                                #[cfg(target_os = "macos")]
                                if let Some(pid) = crate::event_tap::get_frontmost_app_pid() {
                                    *app.state::<FrontmostApp>().0.lock().unwrap() = pid;
                                }
                                app.state::<RecordingState>().0.store(true, Ordering::SeqCst);
                                if let Some(win) = app.get_webview_window("main") {
                                    let _ = win.set_ignore_cursor_events(false);
                                }
                                let _ = app.emit("pipeline-status", "recording");

                                // Level-polling thread (~30 fps)
                                let level_app    = app.clone();
                                let level_atomic = Arc::clone(&audio_engine.current_level);
                                let cancelled    = Arc::clone(&app.state::<PipelineHandle>().cancelled);
                                std::thread::spawn(move || {
                                    loop {
                                        if cancelled.load(Ordering::SeqCst) { break; }
                                        if !level_app.state::<RecordingState>().0.load(Ordering::SeqCst) {
                                            let _ = level_app.emit("audio-level", 0.0f32);
                                            break;
                                        }
                                        let rms        = f32::from_bits(level_atomic.load(Ordering::Relaxed));
                                        let normalized = (rms / 0.15).min(1.0);
                                        let _ = level_app.emit("audio-level", normalized);
                                        std::thread::sleep(std::time::Duration::from_millis(33));
                                    }
                                });
                            }
                            Err(e) => {
                                let _ = app.emit("pipeline-error", format!("Audio Error: {}", e));
                            }
                        }
                    }
                }

                DictationEvent::StopRecording => {
                    let _ = app.emit("pipeline-status", "processing");

                    let audio_engine  = app.state::<AudioEngine>();
                    let engine_state  = app.state::<EngineState>();
                    let model_manager = app.state::<models::ModelManager>();
                    let db_state      = app.state::<DbState>();

                    let mic_id = app.state::<SettingsCache>().get("mic_id");

                    let t_pipeline = std::time::Instant::now();
                    let samples = match audio::stop_stream(&audio_engine, mic_id) {
                        Ok(s)  => s,
                        Err(e) => {
                            log::error!("Audio stream stop failed: {}", e);
                            let _ = app.emit("pipeline-error", e);
                            let _ = app.emit("pipeline-status", "idle");
                            continue;
                        }
                    };

                    app.state::<RecordingState>().0.store(false, Ordering::SeqCst);
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.set_ignore_cursor_events(true);
                    }

                    if samples.is_empty() {
                        let _ = app.emit("pipeline-status", "idle");
                        continue;
                    }

                    // Skip silence (peak check, not RMS — see architecture doc)
                    let peak = samples.iter().cloned().fold(0.0f32, f32::max);
                    if peak < 0.05 {
                        log::debug!("STT: Skipped — silence detected (peak {:.4})", peak);
                        let _ = app.emit("pipeline-status", "idle");
                        continue;
                    }

                    // --- Whisper transcription ---
                    let raw_text = {
                        let mut whisper_lock = engine_state.whisper.lock().unwrap();
                        if whisper_lock.is_none() {
                            let model_path = model_manager.get_whisper_path();
                            let _ = app.emit("pipeline-status", "loading_whisper");
                            let t_load = std::time::Instant::now();
                            match whisper_inference::WhisperEngine::new(&model_path) {
                                Ok(e) => {
                                    let size_mb = std::fs::metadata(&model_path)
                                        .map(|m: std::fs::Metadata| m.len() as f64 / 1_048_576.0)
                                        .unwrap_or(0.0);
                                    log::info!("Whisper loaded  {:.0}MB  {:.2}s", size_mb, t_load.elapsed().as_secs_f64());
                                    *whisper_lock = Some(e);
                                }
                                Err(e) => {
                                    log::error!("Whisper load failed: {}", e);
                                    let _ = app.emit("pipeline-error", e);
                                    let _ = app.emit("pipeline-status", "idle");
                                    continue;
                                }
                            }
                        }
                        let whisper = whisper_lock.as_ref().unwrap();
                        let (language, initial_prompt) = {
                            let lang = app.state::<SettingsCache>()
                                .get("language")
                                .unwrap_or_else(|| "es".to_string());
                            let dict = {
                                let conn = db_state.conn.lock().unwrap();
                                db::get_custom_dictionary(&conn).unwrap_or_default()
                            };
                            let prompt = if dict.is_empty() {
                                "".to_string()
                            } else {
                                format!("Vocabulary: {}.", dict.join(", "))
                            };
                            (lang, prompt)
                        };
                        let t_stt    = std::time::Instant::now();
                        let audio_secs = samples.len() as f64 / 16000.0;
                        match whisper.transcribe(&samples, &language, &initial_prompt) {
                            Ok(t) => {
                                let elapsed = t_stt.elapsed().as_secs_f64();
                                let words   = t.split_whitespace().count();
                                log::info!(
                                    "STT: {:.1}s audio → {} words  ({:.2}s, RTF {:.2}x)",
                                    audio_secs, words, elapsed, elapsed / audio_secs.max(0.01)
                                );
                                log::info!("Transcription: {}", t);
                                t
                            }
                            Err(e) => {
                                log::error!("Transcription failed: {}", e);
                                let _ = app.emit("pipeline-error", e);
                                let _ = app.emit("pipeline-status", "idle");
                                continue;
                            }
                        }
                    };

                    if raw_text.is_empty() {
                        let _ = app.emit("pipeline-status", "idle");
                        continue;
                    }

                    // --- Vocabulary replacement (before LLM) ---
                    let raw_text = {
                        let replacements = {
                            let conn = db_state.conn.lock().unwrap();
                            db::get_replacement_entries(&conn).unwrap_or_default()
                        };
                        if replacements.is_empty() {
                            raw_text
                        } else {
                            let mut text = raw_text;
                            for entry in &replacements {
                                let replacement = entry.replacement_word.as_deref().unwrap_or("");
                                // Case-insensitive word-boundary replacement
                                let pattern = format!(r"(?i)\b{}\b", regex::escape(&entry.word));
                                if let Ok(re) = regex::Regex::new(&pattern) {
                                    if re.is_match(&text) {
                                        text = re.replace_all(&text, replacement).to_string();
                                        let conn = db_state.conn.lock().unwrap();
                                        let _ = db::increment_usage_count(&conn, &entry.word);
                                        log::info!("Vocab replacement: '{}' → '{}'", entry.word, replacement);
                                    }
                                }
                            }
                            text
                        }
                    };

                    let _ = app.emit("pipeline-text-raw", &raw_text);
                    let _ = app.emit("pipeline-status", "refining");

                    // Read cursor context captured at StartRecording
                    let (cursor_pre, cursor_post) = {
                        let ctx = app.state::<CursorContext>();
                        let pre  = ctx.pre_text.lock().unwrap().clone();
                        let post = ctx.post_text.lock().unwrap().clone();
                        (pre, post)
                    };
                    if !cursor_pre.is_empty() || !cursor_post.is_empty() {
                        log::debug!(
                            "Cursor context — pre: {} chars, post: {} chars",
                            cursor_pre.len(), cursor_post.len()
                        );
                    }

                    // --- LLM refinement ---
                    let refined_text = {
                        let mut llama_lock = engine_state.llama.lock().unwrap();

                        if llama_lock.is_none() {
                            let model_path  = model_manager.get_llama_path();
                            let server_path = model_manager.get_effective_llama_server();

                            if !model_path.exists() {
                                log::warn!("Llama model not found, skipping refinement.");
                                raw_text.clone()
                            } else if server_path.is_none() {
                                log::warn!("llama-server not available, skipping refinement.");
                                raw_text.clone()
                            } else {
                                let server_path = server_path.unwrap();
                                log::info!("Starting llama-server from {:?}", server_path);
                                let _ = app.emit("pipeline-status", "loading_llama");
                                let t_llm_load = std::time::Instant::now();
                                match llama_inference::LlamaEngine::new(&model_path, &server_path) {
                                    Ok(e) => {
                                        let size_mb = std::fs::metadata(&model_path)
                                            .map(|m: std::fs::Metadata| m.len() as f64 / 1_048_576.0)
                                            .unwrap_or(0.0);
                                        log::info!(
                                            "LlamaEngine ready  {:.0}MB  {:.2}s",
                                            size_mb,
                                            t_llm_load.elapsed().as_secs_f64()
                                        );
                                        *llama_lock = Some(e);
                                        let llama = llama_lock.as_mut().unwrap();
                                        let (system_prompt, _) = resolve_system_prompt(&app, &db_state);
                                        if system_prompt.is_empty() {
                                            raw_text.clone()
                                        } else {
                                            let t_llm = std::time::Instant::now();
                                            let result = run_llm_refinement(llama, &raw_text, &system_prompt, &cursor_pre, &cursor_post, &app);
                                            log::info!(
                                                "LLM: {:.2}s  in={} chars  out={} chars",
                                                t_llm.elapsed().as_secs_f64(), raw_text.len(), result.len()
                                            );
                                            result
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("LlamaEngine init failed: {}", e);
                                        let _ = app.emit("pipeline-error", format!("Llama Loading Error: {}", e));
                                        raw_text.clone()
                                    }
                                }
                            }
                        } else {
                            let llama = llama_lock.as_mut().unwrap();
                            let (system_prompt, profile_name) = resolve_system_prompt(&app, &db_state);
                            log::info!(
                                "LLM Profile: '{}' | Prompt[:80]: {}",
                                profile_name,
                                &system_prompt.chars().take(80).collect::<String>()
                            );
                            if system_prompt.is_empty() {
                                raw_text.clone()
                            } else {
                                let t_llm  = std::time::Instant::now();
                                let result = run_llm_refinement(llama, &raw_text, &system_prompt, &cursor_pre, &cursor_post, &app);
                                log::info!(
                                    "LLM: {:.2}s  in={} chars  out={} chars",
                                    t_llm.elapsed().as_secs_f64(), raw_text.len(), result.len()
                                );
                                result
                            }
                        }
                    };

                    log::info!("Refined: {}", refined_text);
                    log::info!("Pipeline total: {:.2}s", t_pipeline.elapsed().as_secs_f64());

                    {
                        let conn = db_state.conn.lock().unwrap();
                        let _ = db::insert_transcript(&conn, &refined_text, &raw_text);
                    }

                    use tauri_plugin_clipboard_manager::ClipboardExt;
                    app.clipboard().write_text(refined_text.clone()).unwrap_or_else(|e| {
                        log::error!("Clipboard write failed: {}", e);
                        let _ = app.emit("pipeline-error", format!("Clipboard Error: {}", e));
                    });

                    #[cfg(target_os = "macos")]
                    {
                        let target_pid = *app.state::<FrontmostApp>().0.lock().unwrap();
                        crate::event_tap::activate_app_by_pid(target_pid);
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        crate::event_tap::simulate_paste();
                    }
                    #[cfg(not(target_os = "macos"))]
                    crate::event_tap::simulate_paste();

                    let _ = app.emit("pipeline-results", &refined_text);
                    let _ = app.emit("pipeline-status", "idle");
                }

                DictationEvent::CancelRecording => {
                    app.state::<RecordingState>().0.store(false, Ordering::SeqCst);
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.set_ignore_cursor_events(true);
                    }
                    let audio_engine = app.state::<AudioEngine>();
                    let mic_id = app.state::<SettingsCache>().get("mic_id");
                    let _ = audio::stop_stream(&audio_engine, mic_id);
                    let _ = app.emit("pipeline-status", "idle");
                }
            }
        }
    });
}
