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
/// Metadata about the app that was active when recording started.
#[derive(Clone, serde::Serialize, Default)]
pub struct AppInfo {
    pub pid: i32,
    pub name: String,
    pub icon_base64: Option<String>,
}

pub struct FrontmostApp(pub Mutex<AppInfo>);
pub struct ManualProfileOverride(pub Mutex<Option<String>>); // profile name set explicitly by user this session
pub struct DetectedProfile(pub Mutex<Option<(String, String)>>); // (system_prompt, profile_name)

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
    // Code editors / IDEs — explicit known IDs
    if b == "com.apple.dt.xcode"
        || b == "com.microsoft.vscode"
        || b == "com.todesktop.230313mzl4w4u92" // Cursor
        || b.starts_with("com.jetbrains.")
    {
        return Some("Code");
    }
    // AI coding assistants and dev tools — pattern-based
    // Kiro: dev.kiro.desktop, Windsurf: codeium.windsurf, Zed: dev.zed.Zed, etc.
    if b.starts_with("dev.kiro.")
        || b.starts_with("dev.zed.")
        || b.starts_with("codeium.")
        || b.contains("windsurf")
        || b.contains("antigravity") // Antigravity IDE
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

/// Maps a browser tab domain to a profile keyword.
fn domain_to_profile_keyword(domain: &str) -> Option<&'static str> {
    let d = domain;
    // Code contexts — dev tools and AI assistants
    if d == "github.com" || d == "gitlab.com" || d.ends_with(".atlassian.net")
        || d == "linear.app" || d == "bitbucket.org" { return Some("Code"); }
    if d == "claude.ai" || d == "chat.openai.com" || d == "chatgpt.com" {
        return Some("Code");
    }
    // Informal / chat
    if d.ends_with(".slack.com") || d == "discord.com" || d == "twitter.com"
        || d == "x.com" || d == "linkedin.com" { return Some("Informal"); }
    // Elegant / formal writing — email is formal, not informal
    if d == "mail.google.com" || d.contains("outlook.") || d == "outlook.com"
        || d == "notion.so" || d == "docs.google.com" || d == "coda.io"
        || d.contains("confluence") { return Some("Elegant"); }
    None
}

/// Given a PID, returns the best matching profile name (keyword) or None if no match.
pub fn detect_profile_keyword_for_pid(pid: i32) -> Option<&'static str> {
    let bundle_id = bundle_id_for_pid(pid)?;
    log::debug!("Auto-profile: bundle_id={}", bundle_id);

    // 1. Match by bundle ID (native apps)
    if let Some(kw) = bundle_id_to_profile_keyword(&bundle_id) {
        return Some(kw);
    }

    // 2. For browsers, match by active tab domain
    #[cfg(target_os = "macos")]
    if crate::event_tap::is_browser_bundle_id(&bundle_id) {
        let (tx, rx) = std::sync::mpsc::channel();
        let bid_clone = bundle_id.clone();
        std::thread::spawn(move || {
            let result = crate::event_tap::get_browser_tab_url(pid, &bid_clone);
            let _ = tx.send(result);
        });
        let url_opt = rx.recv_timeout(std::time::Duration::from_millis(50))
            .ok()
            .flatten();
        if let Some(url) = url_opt {
            if let Some(domain) = crate::event_tap::domain_from_url(&url) {
                log::debug!("Auto-profile: browser domain={}", domain);
                if let Some(kw) = domain_to_profile_keyword(&domain) {
                    return Some(kw);
                }
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// LLM helper — eliminates duplicated refine_text blocks
// ---------------------------------------------------------------------------

fn run_llm_refinement(
    llama: &mut LlamaEngine,
    raw_text: &str,
    system_prompt: &str,
    language: &str,
    pre_text: &str,
    post_text: &str,
    app: &tauri::AppHandle,
) -> String {
    match llama.refine_text(raw_text, system_prompt, language, pre_text, post_text) {
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

/// Resolves the composed system_prompt (base + formatting block) for this dictation.
/// Priority: manual override > auto-detected by bundle ID > active_profile_id setting.
/// Returns (composed_prompt, profile_name).
fn resolve_system_prompt(app: &tauri::AppHandle, db_state: &db::DbState) -> (String, String) {
    let conn = db_state.conn.lock().unwrap();
    let language = app
        .state::<db::SettingsCache>()
        .get("language")
        .unwrap_or_else(|| "es".to_string());

    let auto_detect_enabled = app
        .state::<db::SettingsCache>()
        .get("auto_detect_profile")
        .map(|v| v != "false")
        .unwrap_or(true);

    let manual_override = app
        .state::<ManualProfileOverride>()
        .0
        .lock()
        .unwrap()
        .clone();

    let profile = if let Some(ref name) = manual_override {
        db::get_profiles(&conn).ok()
            .and_then(|ps| ps.into_iter().find(|p| &p.name == name))
    } else if auto_detect_enabled {
        let pid = app.state::<FrontmostApp>().0.lock().unwrap().pid;
        detect_profile_keyword_for_pid(pid).and_then(|keyword| {
            db::get_profiles(&conn).ok()
                .and_then(|ps| ps.into_iter().find(|p| p.name == keyword))
                .inspect(|p| log::info!("Auto-profile: matched '{}' for PID {}", p.name, pid))
        })
    } else {
        None
    };

    let profile = profile.or_else(|| {
        db::get_active_profile(&conn).unwrap_or_default()
    });

    let (base_prompt, profile_name, formatting_mode, profile_id) = match profile {
        Some(p) => (p.system_prompt, p.name, p.formatting_mode, p.id),
        None => (String::new(), String::new(), "plain".to_string(), 0),
    };

    let hints = db::get_active_hints(&conn, profile_id).unwrap_or_default();
    let formatting_block = crate::formatting::build_formatting_block(&formatting_mode, &language, &hints);
    let composed = format!("{}\n\n{}", base_prompt, formatting_block);

    (composed, profile_name)
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
                        // Reset VAD state for a clean new session
                        if let Some(vad_arc) = &audio_engine.vad {
                            if let Ok(mut vad) = vad_arc.lock() {
                                vad.reset();
                            }
                        }
                        let mic_id = app.state::<SettingsCache>().get("mic_id");
                        match audio::setup_stream(&audio_engine, mic_id) {
                            Ok(_) => {
                                #[cfg(target_os = "macos")]
                                if let Some(pid) = crate::event_tap::get_frontmost_app_pid() {
                                    let info = crate::event_tap::get_app_info_for_pid(pid)
                                        .unwrap_or(AppInfo { pid, name: String::new(), icon_base64: None });
                                    let _ = app.emit("app-detected", serde_json::json!({
                                        "name": info.name,
                                        "icon": info.icon_base64,
                                    }));
                                    *app.state::<FrontmostApp>().0.lock().unwrap() = info;
                                }
                                // Resolve and cache the profile for this recording session.
                                // Must happen AFTER FrontmostApp is updated so detect_profile_keyword_for_pid
                                // reads the correct PID.
                                let db_state = app.state::<DbState>();
                                let resolved = resolve_system_prompt(&app, &db_state);
                                let is_auto = {
                                    let has_override = app.state::<ManualProfileOverride>().0.lock().unwrap().is_some();
                                    let auto_enabled = app.state::<db::SettingsCache>()
                                        .get("auto_detect_profile")
                                        .map(|v| v != "false")
                                        .unwrap_or(true);
                                    !has_override && auto_enabled
                                };
                                let _ = app.emit("profile-detected", serde_json::json!({
                                    "name": resolved.1,
                                    "is_auto": is_auto,
                                }));
                                *app.state::<DetectedProfile>().0.lock().unwrap() = Some(resolved);
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

                    // Silence detection: Silero VAD v6 (preferred) or peak-amplitude fallback.
                    // VAD processes the resampled 16 kHz samples in 512-sample frames.
                    let is_silent = if let Some(vad_arc) = &audio_engine.vad {
                        if let Ok(mut vad) = vad_arc.lock() {
                            let mut any_speech = false;
                            for chunk in samples.chunks(512) {
                                if vad.process_frame(chunk) {
                                    any_speech = true;
                                    break;
                                }
                            }
                            !any_speech
                        } else {
                            // Mutex poisoned — fall back to peak
                            let peak = samples.iter().cloned().fold(0.0f32, f32::max);
                            peak < 0.05
                        }
                    } else {
                        // VAD unavailable — fall back to peak-amplitude check
                        let peak = samples.iter().cloned().fold(0.0f32, f32::max);
                        peak < 0.05
                    };

                    if is_silent {
                        log::debug!("STT: Skipped — silence detected by VAD");
                        let _ = app.emit("pipeline-status", "idle");
                        continue;
                    }

                    // --- Get configured language ---
                    let language = app.state::<SettingsCache>()
                        .get("language")
                        .unwrap_or_else(|| "es".to_string());

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
                        let initial_prompt = {
                            let dict = {
                                let conn = db_state.conn.lock().unwrap();
                                db::get_custom_dictionary(&conn).unwrap_or_default()
                            };
                            if dict.is_empty() {
                                "".to_string()
                            } else {
                                format!("Vocabulary: {}.", dict.join(", "))
                            }
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
                                        let (system_prompt, _) = app.state::<DetectedProfile>().0.lock().unwrap()
                                            .clone()
                                            .unwrap_or_else(|| resolve_system_prompt(&app, &db_state));
                                        if system_prompt.is_empty() {
                                            raw_text.clone()
                                        } else {
                                            let t_llm = std::time::Instant::now();
                                            let result = run_llm_refinement(llama, &raw_text, &system_prompt, &language, &cursor_pre, &cursor_post, &app);
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
                            let (system_prompt, profile_name) = app.state::<DetectedProfile>().0.lock().unwrap()
                                .clone()
                                .unwrap_or_else(|| resolve_system_prompt(&app, &db_state));
                            log::info!(
                                "LLM Profile: '{}' | Prompt[:80]: {}",
                                profile_name,
                                &system_prompt.chars().take(80).collect::<String>()
                            );
                            if system_prompt.is_empty() {
                                raw_text.clone()
                            } else {
                                let t_llm  = std::time::Instant::now();
                                let result = run_llm_refinement(llama, &raw_text, &system_prompt, &language, &cursor_pre, &cursor_post, &app);
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
                        let target_pid = app.state::<FrontmostApp>().0.lock().unwrap().pid;
                        crate::event_tap::activate_app_by_pid(target_pid);
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        crate::event_tap::simulate_paste();
                    }
                    #[cfg(not(target_os = "macos"))]
                    crate::event_tap::simulate_paste();

                    let _ = app.emit("pipeline-results", &refined_text);
                    // Clear the cached profile — next recording will detect fresh.
                    *app.state::<DetectedProfile>().0.lock().unwrap() = None;
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
                    *app.state::<DetectedProfile>().0.lock().unwrap() = None;
                    let _ = app.emit("pipeline-status", "idle");
                }
            }
        }
    });
}
