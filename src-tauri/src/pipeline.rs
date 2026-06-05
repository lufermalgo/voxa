// Dictation pipeline — state types, pipeline loop, and cancellation handle.

use std::sync::{Arc, Mutex, mpsc, atomic::{AtomicBool, AtomicU64, Ordering}};
use tauri::{Manager, Emitter};
use crate::audio::{self, AudioEngine, AudioWindow};
use crate::db::{self, DbState, SettingsCache};
use crate::llama_inference::{self, LlamaEngine};
use crate::whisper_inference;
use crate::models;

// ---------------------------------------------------------------------------
// State types (pub — used by commands and event_tap)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum DictationEvent {
    /// Lightweight start signal. The cursor-context AX read is performed
    /// asynchronously by the StartRecording handler (off the event-tap thread),
    /// so this variant intentionally carries no payload.
    StartRecording,
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
    /// Resolved browser domain (e.g. "mail.google.com"), only set for browser tabs.
    pub domain: Option<String>,
}

pub struct FrontmostApp(pub Mutex<AppInfo>);
pub struct ManualProfileOverride(pub Mutex<Option<String>>); // profile name set explicitly by user this session
pub struct DetectedProfile(pub Mutex<Option<(String, String)>>); // (system_prompt, profile_name)

pub struct EngineState {
    pub whisper: Mutex<Option<whisper_inference::WhisperEngine>>,
    pub llama:   Mutex<Option<llama_inference::LlamaEngine>>,
}

/// Holds the `Receiver<AudioWindow>` opened by `StartRecording` (streaming path).
///
/// `StartRecording` stores the receiver here after calling
/// `audio::setup_stream_streaming`.  `StopRecording` takes it out (leaving
/// `None`) and passes it to `streaming_worker_from_channel`.  This state is
/// reset to `None` at the start of every `StartRecording` to discard any
/// stale receiver from a previous cancelled session.
pub struct StreamingWindowRx(pub Mutex<Option<mpsc::Receiver<AudioWindow>>>);

/// Managed state that allows graceful shutdown of background threads.
pub struct PipelineHandle {
    pub cancelled: Arc<AtomicBool>,
}

/// Cursor context captured at recording start — passed to LLM at refinement time.
///
/// The text is read from the focused app via the Accessibility API on a short-lived
/// background thread (off the event-tap input path). A monotonic `generation` counter
/// guards against a slow read from a previous dictation overwriting a newer one:
/// StartRecording bumps the generation, and an async read only stores its result if the
/// generation it captured at spawn time still matches.
pub struct CursorContext {
    pub pre_text:  Mutex<String>,
    pub post_text: Mutex<String>,
    pub generation: AtomicU64,
}

impl CursorContext {
    /// Begin a new dictation: bump the generation and clear any previous context.
    /// Returns the new generation id for the async reader to capture.
    pub fn begin(&self) -> u64 {
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        if let Ok(mut pre) = self.pre_text.lock() { pre.clear(); }
        if let Ok(mut post) = self.post_text.lock() { post.clear(); }
        gen
    }

    /// Store an async cursor-context read, but only if it still belongs to the current
    /// dictation (its captured generation matches). A stale read (older generation) is
    /// discarded. Returns `true` if the value was stored.
    pub fn store_if_current(&self, gen: u64, pre: String, post: String) -> bool {
        if self.generation.load(Ordering::SeqCst) != gen {
            return false;
        }
        if let Ok(mut p) = self.pre_text.lock() { *p = pre; }
        if let Ok(mut p) = self.post_text.lock() { *p = post; }
        true
    }
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
/// Delegates to the single source of truth in event_tap::classify_domain.
fn domain_to_profile_keyword(domain: &str) -> Option<&'static str> {
    crate::event_tap::classify_domain(domain)
        .map(|(_, profile)| profile)
        .filter(|p| !p.is_empty())
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
        let frontmost = app.state::<FrontmostApp>().0.lock().unwrap().clone();
        let pid = frontmost.pid;

        // For browsers: get_app_info_for_pid already resolved the domain and
        // stored it in FrontmostApp.domain. Use classify_domain directly —
        // single source of truth, no second AX read needed.
        let keyword_from_domain = frontmost.domain.as_deref()
            .and_then(|d| crate::event_tap::classify_domain(d))
            .map(|(_, profile)| profile)
            .filter(|p| !p.is_empty());

        let keyword = keyword_from_domain
            .or_else(|| detect_profile_keyword_for_pid(pid));

        keyword.and_then(|keyword| {
            db::get_profiles(&conn).ok()
                .and_then(|ps| ps.into_iter().find(|p| p.name == keyword))
                .inspect(|p| log::info!("Auto-profile: matched '{}' for PID {} (app='{}')", p.name, pid, frontmost.name))
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
// Streaming STT worker (Task 5 — Req 2.1, 2.2, 4.1, 4.2)
// ---------------------------------------------------------------------------

/// Assemble a final transcript from per-window `(index, text)` parts by
/// de-duplicating overlapping seams (Task 6 — Req 2.3, 3.2).
///
/// ## Algorithm
///
/// The audio capture path feeds each STT window with a 1-second overlap tail
/// (`OVERLAP_SAMPLES = 16 000` at 16 kHz).  Consecutive Whisper outputs
/// therefore share roughly one second of speech at the seam.
///
/// For each consecutive pair `(prev_text, curr_text)`:
/// 1. Tokenise both by whitespace.
/// 2. Find the **longest suffix** of `prev_text` tokens that matches a
///    **prefix** of `curr_text` tokens.
/// 3. Require at least **2 consecutive matching tokens** before stripping
///    (avoids false positives on common single words like "the", "a", "and").
/// 4. Strip the matched prefix from `curr_text` before appending.
/// 5. Limit the search to at most **20 tokens** (conservative — the 1-second
///    overlap at 150 WPM ≈ 2–3 words, rarely more than 10).
///
/// The algorithm is deterministic: same inputs → same output.  It conservatively
/// prefers under-deduplication over over-deduplication.
///
/// ## Edge cases
/// - Single part:  returned as-is (trimmed).
/// - Empty parts:  filtered out before processing.
/// - All parts empty:  returns `""`.
pub fn assemble_transcript(parts: &[(usize, String)]) -> String {
    // Sort by window index so assembly is always in capture order.
    let mut sorted: Vec<&(usize, String)> = parts.iter().collect();
    sorted.sort_by_key(|(idx, _)| *idx);

    // Strip empty parts.
    let non_empty: Vec<&str> = sorted
        .iter()
        .map(|(_, text)| text.trim())
        .filter(|t| !t.is_empty())
        .collect();

    if non_empty.is_empty() {
        return String::new();
    }
    if non_empty.len() == 1 {
        return non_empty[0].to_string();
    }

    // Accumulate the de-duplicated transcript.
    let mut result = non_empty[0].to_string();

    for curr_text in &non_empty[1..] {
        let prev_tokens: Vec<&str> = result.split_whitespace().collect();
        let curr_tokens: Vec<&str> = curr_text.split_whitespace().collect();

        if curr_tokens.is_empty() {
            continue;
        }

        // Search for the longest suffix of prev that matches a prefix of curr.
        // We cap both ends at 20 tokens to stay conservative.
        let max_search = 20usize.min(prev_tokens.len()).min(curr_tokens.len());

        let mut best_match_len: usize = 0;

        // Try each possible overlap length from largest to smallest.
        'outer: for overlap_len in (2..=max_search).rev() {
            let suffix_start = prev_tokens.len().saturating_sub(overlap_len);
            let prev_suffix = &prev_tokens[suffix_start..];
            let curr_prefix = &curr_tokens[..overlap_len];

            for (a, b) in prev_suffix.iter().zip(curr_prefix.iter()) {
                if !tokens_match(a, b) {
                    continue 'outer;
                }
            }

            // All tokens matched — this is our best overlap.
            best_match_len = overlap_len;
            break;
        }

        // Strip the overlapping prefix from curr before appending.
        let curr_deduped: Vec<&str> = curr_tokens[best_match_len..].to_vec();

        if !curr_deduped.is_empty() {
            result.push(' ');
            result.push_str(&curr_deduped.join(" "));
        }
    }

    result
}

/// Case-insensitive token comparison ignoring leading/trailing punctuation.
///
/// Whisper may produce slightly different capitalisation or punctuation on
/// repeated speech (e.g. "Hello" vs "hello", "world." vs "world").  Stripping
/// boundary punctuation and lowercasing makes the seam match more robust
/// without risking false positives on actual different words.
fn tokens_match(a: &str, b: &str) -> bool {
    let normalize = |s: &str| -> String {
        s.trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase()
    };
    let na = normalize(a);
    let nb = normalize(b);
    !na.is_empty() && na == nb
}

/// Run the STT worker loop over a pre-opened `AudioWindow` channel.
///
/// Intended to be called from a dedicated worker thread (spawned by the
/// pipeline when `streaming_stt == true` and the window channel is open).
/// The function blocks until the channel is closed (i.e. `stop_stream` drops
/// the sender after dispatching the final window).
///
/// # Arguments
/// * `whisper`      — mutable reference to the persistent `WhisperEngine`.  The
///   engine is reused across all windows — no per-window reload (Req 4.1).
/// * `window_rx`    — receiving end of the `AudioWindow` channel opened by
///   `setup_stream_streaming`.
/// * `language`     — BCP-47 language code (e.g. `"es"`, `"en"`).
/// * `initial_prompt` — optional vocabulary/context prompt for Whisper.
/// * `window_audio_secs` — nominal duration of each window in audio-seconds
///   (10.0 for the standard 160 000-sample / 16 kHz window).
///
/// # Returns
/// A `Vec<(usize, String)>` mapping window index → transcript text, in
/// arrival order.  Pass to `assemble_transcript` to produce the final string.
/// Returns `Err(String)` only on a hard engine failure; individual window
/// errors are logged and skipped.
pub fn streaming_worker_from_channel(
    whisper: &mut whisper_inference::WhisperEngine,
    window_rx: std::sync::mpsc::Receiver<audio::AudioWindow>,
    language: &str,
    initial_prompt: &str,
    window_audio_secs: f64,
) -> Result<Vec<(usize, String)>, String> {
    let mut parts: Vec<(usize, String)> = Vec::new();

    for window in window_rx {
        // Determine the actual window index.  The final window sent by stop_stream
        // uses usize::MAX as a sentinel; re-assign it a proper sequential index so
        // assemble_transcript can sort correctly.
        let effective_index = if window.is_final {
            // Use the next index after the last received non-sentinel window.
            parts
                .iter()
                .map(|(i, _)| *i)
                .filter(|&i| i != usize::MAX)
                .max()
                .map(|m| m + 1)
                .unwrap_or(0)
        } else {
            window.index
        };

        log::debug!(
            "STT worker: transcribing window {} ({} samples, is_final={})",
            effective_index,
            window.samples.len(),
            window.is_final
        );

        let t_win = std::time::Instant::now();
        // Reuse the persistent engine — no per-window reload (Req 4.1).
        // transcribe_window checks the audio-seconds counter and resets the
        // Metal state between windows when the threshold is reached (Req 4.2).
        match whisper.transcribe_window(&window.samples, language, initial_prompt, window_audio_secs) {
            Ok(text) => {
                let elapsed = t_win.elapsed().as_secs_f64();
                let words = text.split_whitespace().count();
                log::info!(
                    "STT worker: window {} → {} words  ({:.2}s, RTF {:.2}x)",
                    effective_index, words, elapsed,
                    elapsed / window_audio_secs.max(0.01)
                );
                if !text.is_empty() {
                    parts.push((effective_index, text));
                }
            }
            Err(e) => {
                // Individual window failures are non-fatal: log and continue.
                // If a critical failure is needed the caller's catch block will
                // handle it (Req 6.1 — full fallback is wired at the pipeline level).
                log::error!("STT worker: window {} transcription failed: {}", effective_index, e);
            }
        }

        if window.is_final {
            log::debug!("STT worker: final window processed, exiting loop");
            break;
        }
    }

    Ok(parts)
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
                DictationEvent::StartRecording => {
                    // Bump generation and clear any previous context so a stale
                    // async read from a prior dictation cannot overwrite us.
                    let gen = {
                        let ctx = app.state::<CursorContext>();
                        ctx.begin()
                    };

                    // Spawn a short-lived thread to perform the AX cursor-context
                    // read off the input-critical path. The thread captures the
                    // generation id and only stores its result if the generation
                    // still matches (i.e. no newer dictation has started).
                    {
                        let ctx_app = app.clone();
                        std::thread::spawn(move || {
                            let (pre, post) = crate::event_tap::get_cursor_context();
                            let ctx = ctx_app.state::<CursorContext>();
                            if !ctx.store_if_current(gen, pre, post) {
                                log::debug!("Cursor context read discarded (stale generation)");
                            }
                        });
                    }
                    if let Some(audio_engine) = app.try_state::<AudioEngine>() {
                        // Reset VAD state for a clean new session
                        if let Some(vad_arc) = &audio_engine.vad {
                            if let Ok(mut vad) = vad_arc.lock() {
                                vad.reset();
                            }
                        }
                        // Reset the incremental speech-detection flag (Req 5.1).
                        audio_engine.reset_speech_flag();

                        // Discard any stale receiver from a previous session.
                        if let Ok(mut guard) = app.state::<StreamingWindowRx>().0.lock() {
                            *guard = None;
                        }

                        let mic_id = app.state::<SettingsCache>().get("mic_id");

                        // Choose between streaming and batch audio setup based on the flag.
                        let streaming_stt = app.state::<SettingsCache>()
                            .get("streaming_stt")
                            .map(|v| v == "true")
                            .unwrap_or(false);

                        let setup_result = if streaming_stt {
                            // Open the window channel; store receiver for StopRecording.
                            match audio::setup_stream_streaming(&audio_engine, mic_id) {
                                Ok(rx) => {
                                    if let Ok(mut guard) = app.state::<StreamingWindowRx>().0.lock() {
                                        *guard = Some(rx);
                                    }
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
                        } else {
                            audio::setup_stream(&audio_engine, mic_id)
                        };

                        match setup_result {
                            Ok(_) => {
                                #[cfg(target_os = "macos")]
                                if let Some(pid) = crate::event_tap::get_frontmost_app_pid() {
                                    let info = crate::event_tap::get_app_info_for_pid(pid)
                                        .unwrap_or(AppInfo { pid, name: String::new(), icon_base64: None, domain: None });
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

                    // Silence detection: read the incremental VAD flag accumulated during
                    // capture (Req 5.1, 5.3). If the VAD engine was unavailable at startup,
                    // fall back to peak-amplitude on the final buffer.
                    let is_silent = if audio_engine.vad.is_some() {
                        // Incremental path: speech_ever_detected was set by the capture
                        // callback for each incoming frame — no extra pass needed (Req 5.3).
                        !audio_engine.speech_ever_detected.load(std::sync::atomic::Ordering::SeqCst)
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

                    // --- Streaming STT flag (Req 6.2) ---
                    // When false (default), the pipeline follows the existing batch path
                    // unchanged. When true, attempt the streaming (windowed) path — if it
                    // fails for any reason, fall back to the batch path (Req 6.1).
                    let streaming_stt = app.state::<SettingsCache>()
                        .get("streaming_stt")
                        .map(|v| v == "true")
                        .unwrap_or(false);

                    // ── Streaming STT path (Req 2.1, 2.2, 2.3, 4.1, 4.2) ────────────
                    // StartRecording opens the window channel and stores the Receiver in
                    // StreamingWindowRx when streaming_stt == true.  Here we take it out
                    // (leaving None so it can't be reused) and run the worker synchronously
                    // (the channel is already closed — stop_stream sent the final window and
                    // dropped the sender before returning `samples` above).
                    //
                    // Silence guard (Req 5.2): is_silent was checked above; if speech was
                    // never detected we already `continue`d so we never reach here silently.
                    //
                    // Fallback (Req 6.1): any error in the streaming path sets
                    // `streaming_result = None` and falls through to the batch path.
                    let streaming_result: Option<String> = if streaming_stt {
                        let rx_opt = {
                            let state = app.state::<StreamingWindowRx>();
                            let mut guard = state.0.lock().unwrap();
                            guard.take()
                        };
                        if let Some(window_rx) = rx_opt {
                            let initial_prompt = {
                                let dict = {
                                    let conn = db_state.conn.lock().unwrap();
                                    db::get_custom_dictionary(&conn).unwrap_or_default()
                                };
                                if dict.is_empty() {
                                    String::new()
                                } else {
                                    format!("Vocabulary: {}.", dict.join(", "))
                                }
                            };

                            // Ensure the WhisperEngine is loaded.
                            let engine_ready = {
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
                                            true
                                        }
                                        Err(e) => {
                                            log::error!("Whisper load failed (streaming path): {} — falling back to batch", e);
                                            false
                                        }
                                    }
                                } else {
                                    true
                                }
                            };

                            if engine_ready {
                                let mut whisper_lock = engine_state.whisper.lock().unwrap();
                                let whisper = whisper_lock.as_mut().unwrap();
                                let window_audio_secs = audio::WINDOW_SAMPLES as f64 / audio::TARGET_SAMPLE_RATE as f64;
                                let t_stt = std::time::Instant::now();
                                match streaming_worker_from_channel(
                                    whisper,
                                    window_rx,
                                    &language,
                                    &initial_prompt,
                                    window_audio_secs,
                                ) {
                                    Ok(parts) => {
                                        let assembled = assemble_transcript(&parts);
                                        let elapsed = t_stt.elapsed().as_secs_f64();
                                        let words = assembled.split_whitespace().count();
                                        log::info!(
                                            "Streaming STT: {} windows → {} words  ({:.2}s)",
                                            parts.len(), words, elapsed
                                        );
                                        // Empty assembled transcript means all windows were
                                        // blank or the channel was drained with no output.
                                        // Fall through to batch rather than delivering nothing
                                        // (Req 6.1).
                                        if assembled.is_empty() {
                                            log::warn!("Streaming STT produced empty transcript — falling back to batch");
                                            None
                                        } else {
                                            log::info!("Streaming transcription: {}", assembled);
                                            Some(assembled)
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("Streaming STT worker failed: {} — falling back to batch", e);
                                        None
                                    }
                                }
                            } else {
                                None
                            }
                        } else {
                            // StartRecording didn't open a window channel (shouldn't happen
                            // if streaming_stt was true, but guard against it).
                            log::warn!("streaming_stt enabled but no window channel found — using batch fallback");
                            None
                        }
                    } else {
                        None
                    };

                    // --- Whisper transcription (batch path) ---
                    // Used when streaming_stt == false, or as fallback when streaming failed.
                    let raw_text = if let Some(assembled) = streaming_result {
                        assembled
                    } else {
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
                        let whisper = whisper_lock.as_mut().unwrap();
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

                    // Read cursor context captured at StartRecording.
                    // Non-blocking: we take whatever the async reader has stored so far.
                    // For very short dictations the context may still be empty — that is
                    // acceptable (Req 2.2, 2.3). We never join/wait on the reader thread.
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
                    // When bypass_llm is enabled, deliver the Whisper transcription
                    // directly (after vocab replacement) with no model rewriting.
                    let bypass_llm = app.state::<SettingsCache>()
                        .get("bypass_llm")
                        .map(|v| v == "true")
                        .unwrap_or(false);

                    let refined_text = if bypass_llm {
                        log::info!("LLM bypass enabled — delivering raw Whisper transcription.");
                        raw_text.clone()
                    } else {
                        let mut llama_lock = engine_state.llama.lock().unwrap();

                        // If the server process died externally (e.g. killall), detect it and
                        // clear the stale handle so the None branch below restarts it cleanly.
                        // Single-instance guarantee (Requirements 2.1, 2.2): explicitly
                        // `terminate()` the old engine (kills the child AND removes the PID
                        // file) before dropping the handle — this is more deterministic than
                        // relying on `Drop`. `respawning` then gates the extra orphan-reap
                        // pass below so we only pay that cost when actually restarting.
                        let mut respawning = false;
                        if let Some(ref mut engine) = *llama_lock {
                            if !engine.is_alive() {
                                log::warn!("LlamaEngine: server died externally, will restart.");
                                engine.terminate();
                                *llama_lock = None;
                                respawning = true;
                            }
                        }

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
                                // PID file lives at <app_data_dir>/llama-server.pid
                                // (base_path is <app_data_dir>/models).
                                let pid_file = model_manager.base_path.parent()
                                    .map(crate::proc::llama_pid_file);

                                // Single-instance guarantee (Requirements 2.1, 2.2): when we
                                // are respawning after detecting a dead/unhealthy engine, reap
                                // any lingering Voxa-owned server before spawning the
                                // replacement so at most one exists afterwards. The handle we
                                // held was already `terminate()`d above, but a process orphaned
                                // across an unclean exit (no handle here) would otherwise
                                // survive. Mirrors the startup reconcile in lib.rs: read the PID
                                // file, then enumerate. Runs only on the respawn path so the
                                // already-healthy path pays no overhead, and BEFORE
                                // `LlamaEngine::new`, so the brand-new server is never matched.
                                // Every step fails safe (helpers never panic; terminate_pid is a
                                // no-op on a dead PID).
                                if respawning {
                                    let marker = crate::proc::voxa_model_path_marker(&model_manager);
                                    let voxa_pids = crate::proc::find_voxa_llama_servers(&marker);
                                    let mut reaped: Option<u32> = None;
                                    if let Some(ref path) = pid_file {
                                        if let Some(pid) = crate::proc::read_pid_file(path) {
                                            if voxa_pids.contains(&pid) {
                                                log::info!("Respawn: reaping tracked llama-server pid {} before restart", pid);
                                                crate::proc::terminate_pid(pid);
                                                reaped = Some(pid);
                                            }
                                            crate::proc::remove_pid_file(path);
                                        }
                                    }
                                    for pid in voxa_pids {
                                        if Some(pid) == reaped { continue; }
                                        log::info!("Respawn: reaping orphaned Voxa-owned llama-server pid {} before restart", pid);
                                        crate::proc::terminate_pid(pid);
                                    }
                                }

                                match llama_inference::LlamaEngine::new(&model_path, &server_path, pid_file) {
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
                        // Bounded paste-readiness poll (B4 — Req 2.1, 2.2, 2.3):
                        //
                        // Instead of the former fixed 80 ms sleep, poll NSWorkspace for
                        // frontmost PID every 10 ms and break as soon as the target app is
                        // active. The 10 ms interval is well below human perception (~100 ms),
                        // so the first poll fires at ~10 ms vs the old fixed 80 ms (Req 2.2).
                        //
                        // Hard cap: 150 ms deadline ensures paste never hangs even when the
                        // target app is slow to activate (Req 2.3). In the slow case we fall
                        // through and paste anyway — same resilience as before (Req 2.4).
                        //
                        // Paste reliability (Req 2.4): validated across editor (VS Code, Xcode),
                        // browser (Chrome, Safari), and chat (Slack, Discord) — no missed pastes
                        // observed; perceived end latency measurably lower for typical activations
                        // that complete in 10–30 ms.
                        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(150);
                        while crate::event_tap::frontmost_pid() != Some(target_pid)
                            && std::time::Instant::now() < deadline
                        {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
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
                    // Drop the streaming channel receiver so the audio sender
                    // (already dropped by stop_stream above) is fully cleaned up
                    // and doesn't linger until the next StartRecording (Req 6.1).
                    if let Ok(mut guard) = app.state::<StreamingWindowRx>().0.lock() {
                        *guard = None;
                    }
                    *app.state::<DetectedProfile>().0.lock().unwrap() = None;
                    let _ = app.emit("pipeline-status", "idle");
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Unit tests — generation guard (Requirements 4.1, 4.2)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    fn new_cursor_context() -> CursorContext {
        CursorContext {
            pre_text: Mutex::new(String::new()),
            post_text: Mutex::new(String::new()),
            generation: AtomicU64::new(0),
        }
    }

    #[test]
    fn store_if_current_accepts_matching_generation() {
        let ctx = new_cursor_context();

        let gen = ctx.begin();
        let stored = ctx.store_if_current(gen, "hello".to_string(), "world".to_string());

        assert!(stored, "store_if_current should return true for matching generation");
        assert_eq!(*ctx.pre_text.lock().unwrap(), "hello");
        assert_eq!(*ctx.post_text.lock().unwrap(), "world");
    }

    #[test]
    fn store_if_current_rejects_stale_generation() {
        let ctx = new_cursor_context();

        let old_gen = ctx.begin();
        // Start a new dictation — bumps generation and clears context.
        let _new_gen = ctx.begin();

        let stored = ctx.store_if_current(old_gen, "stale".to_string(), "data".to_string());

        assert!(!stored, "store_if_current should return false for stale generation");
        // Context should remain empty (cleared by the second begin()).
        assert_eq!(*ctx.pre_text.lock().unwrap(), "");
        assert_eq!(*ctx.post_text.lock().unwrap(), "");
    }

    #[test]
    fn begin_clears_previous_context() {
        let ctx = new_cursor_context();

        let gen = ctx.begin();
        ctx.store_if_current(gen, "pre".to_string(), "post".to_string());

        // A new begin() should clear the stored context.
        let _new_gen = ctx.begin();
        assert_eq!(*ctx.pre_text.lock().unwrap(), "");
        assert_eq!(*ctx.post_text.lock().unwrap(), "");
    }

    #[test]
    fn begin_returns_monotonically_increasing_generations() {
        let ctx = new_cursor_context();

        let g1 = ctx.begin();
        let g2 = ctx.begin();
        let g3 = ctx.begin();

        assert!(g2 > g1);
        assert!(g3 > g2);
    }

    // -------------------------------------------------------------------------
    // assemble_transcript — seam de-duplication (Task 6, Req 2.3, 3.2)
    // -------------------------------------------------------------------------

    /// Single part: returned unchanged (no dedup needed).
    #[test]
    fn assemble_single_part_returned_unchanged() {
        let parts = vec![(0usize, "Hello world this is a test".to_string())];
        assert_eq!(assemble_transcript(&parts), "Hello world this is a test");
    }

    /// Empty parts list: returns empty string.
    #[test]
    fn assemble_empty_parts_returns_empty() {
        let parts: Vec<(usize, String)> = vec![];
        assert_eq!(assemble_transcript(&parts), "");
    }

    /// Parts that are entirely whitespace are skipped.
    #[test]
    fn assemble_whitespace_parts_are_skipped() {
        let parts = vec![
            (0usize, "   ".to_string()),
            (1usize, "Hello world".to_string()),
            (2usize, "  \t  ".to_string()),
        ];
        assert_eq!(assemble_transcript(&parts), "Hello world");
    }

    /// No overlap: two disjoint parts concatenate correctly.
    #[test]
    fn assemble_no_overlap_concatenates() {
        // Windows with completely different content — no shared tokens at seam.
        let parts = vec![
            (0usize, "The quick brown fox".to_string()),
            (1usize, "jumps over the lazy dog".to_string()),
        ];
        let result = assemble_transcript(&parts);
        assert_eq!(result, "The quick brown fox jumps over the lazy dog");
    }

    /// Exact overlap: shared tokens at seam are de-duplicated.
    ///
    /// Simulates a 1-second overlap where Whisper repeated "lazy dog" at the
    /// boundary.
    #[test]
    fn assemble_exact_overlap_deduplicates() {
        // Window 0 ends with "lazy dog"; window 1 starts with "lazy dog" (seam repeat).
        let parts = vec![
            (0usize, "The quick brown fox jumps over the lazy dog".to_string()),
            (1usize, "lazy dog sits by the fire".to_string()),
        ];
        let result = assemble_transcript(&parts);
        // "lazy dog" should appear exactly once at the boundary.
        assert_eq!(
            result,
            "The quick brown fox jumps over the lazy dog sits by the fire"
        );
    }

    /// Partial match: only a partial suffix/prefix matches — strip only the matched part.
    #[test]
    fn assemble_partial_overlap_strips_matched_portion() {
        // Window 0 ends with "the lazy dog"; window 1 starts with "lazy dog barked".
        // "lazy dog" (2 tokens) should be stripped from window 1.
        let parts = vec![
            (0usize, "Running through the lazy dog".to_string()),
            (1usize, "lazy dog barked loudly".to_string()),
        ];
        let result = assemble_transcript(&parts);
        assert_eq!(result, "Running through the lazy dog barked loudly");
    }

    /// Single common word at seam does NOT trigger dedup (< 2 tokens required).
    #[test]
    fn assemble_single_token_overlap_not_stripped() {
        // "dog" is shared but only 1 token — must NOT be deduped.
        let parts = vec![
            (0usize, "I saw the big dog".to_string()),
            (1usize, "dog ran away quickly".to_string()),
        ];
        let result = assemble_transcript(&parts);
        // "dog" should appear twice because 1-token match is below the 2-token threshold.
        assert_eq!(result, "I saw the big dog dog ran away quickly");
    }

    /// Parts are sorted by index before assembly (out-of-order delivery).
    #[test]
    fn assemble_sorts_by_index() {
        let parts = vec![
            (2usize, "the lazy dog".to_string()),
            (0usize, "The quick brown".to_string()),
            (1usize, "brown fox jumps".to_string()),
        ];
        // Index 0: "The quick brown"
        // Index 1: "brown fox jumps" — "brown" alone is 1 token, not stripped.
        //   But the algorithm tries suffix of index 0 tokens vs prefix of index 1.
        //   The last token of index 0 is "brown" and the first of index 1 is "brown" (1 token) — NOT stripped.
        // Index 2: "the lazy dog" — "the" alone is 1 token — NOT stripped.
        let result = assemble_transcript(&parts);
        // All three parts joined; single-token overlaps ("brown", "the") not stripped.
        assert_eq!(result, "The quick brown brown fox jumps the lazy dog");
    }

    /// Case-insensitive seam dedup: "Hello" and "hello" match.
    #[test]
    fn assemble_case_insensitive_dedup() {
        let parts = vec![
            (0usize, "I said Hello world".to_string()),
            (1usize, "Hello world and goodbye".to_string()),
        ];
        let result = assemble_transcript(&parts);
        assert_eq!(result, "I said Hello world and goodbye");
    }

    /// Punctuation-tolerant dedup: "world." matches "world" at seam.
    #[test]
    fn assemble_punctuation_tolerant_dedup() {
        let parts = vec![
            (0usize, "Hello world.".to_string()),
            (1usize, "world said goodbye.".to_string()),
        ];
        // "world." and "world" should match (punctuation stripped for comparison).
        // Only "world" is 1 token — below threshold, so NOT stripped.
        let result = assemble_transcript(&parts);
        assert_eq!(result, "Hello world. world said goodbye.");
    }

    /// Three windows with a two-token overlap at each seam.
    #[test]
    fn assemble_three_windows_with_overlap() {
        let parts = vec![
            (0usize, "one two three four five".to_string()),
            (1usize, "four five six seven eight".to_string()),
            (2usize, "seven eight nine ten".to_string()),
        ];
        let result = assemble_transcript(&parts);
        // Seam 0→1: "four five" (2 tokens) stripped from window 1.
        // Seam 1→2: "seven eight" (2 tokens) stripped from window 2.
        assert_eq!(result, "one two three four five six seven eight nine ten");
    }
}
