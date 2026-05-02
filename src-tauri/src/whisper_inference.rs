use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};
use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;
use regex::Regex;

static HALLUCINATION_RE: OnceLock<Regex> = OnceLock::new();

fn get_hallucination_re() -> &'static Regex {
    HALLUCINATION_RE.get_or_init(|| {
        Regex::new(
            r"(?i)\[(?:música|music|silencio|silence|aplausos|applause|ruido|noise|inaudible|risas|laughter|música suave|background music)[^\]]*\]|¡?suscríbete!?|subscribe|¡gracias por ver!|subtítulos\s+por[^\n]*|subtitles\s+by[^\n]*|♪+\s*♪*"
        ).expect("Invalid hallucination regex")
    })
}

fn build_hallucination_set() -> HashSet<String> {
    let raw = include_str!("hallucination_phrases.txt");
    raw.lines()
        .map(|l| l.trim().to_lowercase())
        .filter(|l| l.chars().count() >= 3)
        .collect()
}

fn is_hallucination(text: &str, set: &HashSet<String>) -> bool {
    let normalized = text.trim().to_lowercase();
    normalized.len() >= 3 && set.contains(&normalized)
}

/// Removes Whisper hallucination tokens that appear when processing silence or background noise.
/// Pattern: bracketed tokens like [MÚSICA], [Silencio], [Applause], [Music], ♪♪, etc.
fn strip_hallucinations(text: &str) -> String {
    let re = get_hallucination_re();
    let cleaned = re.replace_all(text, "");
    cleaned.trim().to_string()
}

pub struct WhisperEngine {
    context: WhisperContext,
    state: whisper_rs::WhisperState,
    hallucination_set: HashSet<String>,
    /// Number of transcriptions since last state reset.
    /// Metal command buffers can leak ~30MB per inference; resetting the state
    /// periodically reclaims that memory without the full ~2-5s model reload.
    uses_since_reset: u32,
}

/// Maximum number of transcriptions before we recreate the WhisperState.
/// This reclaims Metal command buffer memory (~30MB per inference) while
/// keeping the context (model weights) in GPU memory.
const MAX_USES_BEFORE_RESET: u32 = 20;

impl WhisperEngine {
    pub fn new(model_path: &Path) -> Result<Self, String> {
        if !model_path.exists() {
            return Err("Whisper model file not found".to_string());
        }
        
        let path_str = model_path.to_str().ok_or("Invalid path")?;
        let mut wparams = WhisperContextParameters::default();
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            wparams.use_gpu = true;
        }
        let context = WhisperContext::new_with_params(path_str, wparams)
            .map_err(|e| format!("Failed to create whisper context: {}", e))?;

        // Create the state once and keep it alive for the lifetime of the engine.
        // This avoids re-initializing the Metal GPU backend on every transcription
        // (~2-5s overhead per call).
        let state = context.create_state().map_err(|e| {
            format!("Failed to create whisper state: {}", e)
        })?;

        log::info!("WhisperEngine: context + state created (Metal backend initialized once)");

        Ok(Self {
            context,
            state,
            hallucination_set: build_hallucination_set(),
            uses_since_reset: 0,
        })
    }

    /// Recreate the WhisperState to reclaim Metal command buffer memory.
    /// The WhisperContext (model weights) stays in GPU memory — only the
    /// inference state is reset. This is much faster than a full reload
    /// (~200ms vs ~2-5s) but still frees accumulated Metal buffers.
    pub fn reset_state(&mut self) -> Result<(), String> {
        self.state = self.context.create_state().map_err(|e| {
            format!("Failed to recreate whisper state: {}", e)
        })?;
        self.uses_since_reset = 0;
        log::info!("WhisperEngine: state reset (Metal buffers reclaimed)");
        Ok(())
    }

    pub fn transcribe(&mut self, audio_data: &[f32], language: &str, initial_prompt: &str) -> Result<String, String> {
        log::info!("WHISPER: Starting transcription with {} samples, language: {}, prompt: \"{}\"", audio_data.len(), language, initial_prompt);
        
        // Greedy (best_of=1) is 3-5x faster than BeamSearch on Metal and gives equivalent
        // quality for clean microphone audio. Hallucination protection comes from the HashSet
        // filter and no_speech_thold — BeamSearch is not needed here.
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        params.set_n_threads(4);
        params.set_language(Some(language));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        // Skip segments where Whisper detects no speech — prevents [MÚSICA]/[Silencio] hallucinations
        // on silent audio at the end of the recording.
        params.set_no_speech_thold(0.6);
        if !initial_prompt.is_empty() {
            params.set_initial_prompt(initial_prompt);
        }

        log::debug!("WHISPER: Running inference (full) with persistent state...");
        self.state.full(params, audio_data).map_err(|e: whisper_rs::WhisperError| {
            log::error!("WHISPER: Inference failed: {}", e);
            e.to_string()
        })?;

        let num_segments = self.state.full_n_segments().map_err(|e: whisper_rs::WhisperError| e.to_string())?;
        log::debug!("WHISPER: Finished inference. Segments found: {}", num_segments);

        let mut result = String::new();
        for i in 0..num_segments {
            let segment = self.state.full_get_segment_text(i).map_err(|e: whisper_rs::WhisperError| e.to_string())?;
            let cleaned_segment = strip_hallucinations(&segment);
            if !is_hallucination(&cleaned_segment, &self.hallucination_set) {
                result.push_str(&cleaned_segment);
            }
        }

        // Strip Whisper hallucination tokens that appear on silence/music/noise.
        // These are always enclosed in brackets: [MÚSICA], [Silencio], [Applause], etc.
        // Additionally check the full assembled result against the known-hallucination set.
        let after_regex = strip_hallucinations(result.trim());
        let final_text = if is_hallucination(&after_regex, &self.hallucination_set) {
            String::new()
        } else {
            after_regex
        };

        // Periodically reset state to reclaim Metal command buffer memory.
        // See: https://github.com/lufermalgo/voxa/issues/68#issuecomment-community
        self.uses_since_reset += 1;
        if self.uses_since_reset >= MAX_USES_BEFORE_RESET {
            log::info!("WhisperEngine: {} uses reached, resetting state to reclaim Metal memory", self.uses_since_reset);
            if let Err(e) = self.reset_state() {
                log::error!("WhisperEngine: state reset failed: {}", e);
            }
        }

        Ok(final_text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_set() -> HashSet<String> {
        build_hallucination_set()
    }

    #[test]
    fn test_known_hallucination_is_filtered() {
        let set = test_set();
        assert!(is_hallucination("thank you for watching", &set));
    }

    #[test]
    fn test_real_speech_not_filtered() {
        let set = test_set();
        assert!(!is_hallucination("the deployment pipeline is broken today", &set));
    }

    #[test]
    fn test_short_phrases_excluded_from_set() {
        let set = test_set();
        assert!(!set.contains("a"));
        assert!(!set.contains("ok"));
        assert!(set.iter().all(|p| p.chars().count() >= 3));
    }

    #[test]
    fn test_empty_string_no_panic() {
        let set = test_set();
        assert!(!is_hallucination("", &set));
    }

    #[test]
    fn test_mixed_segments() {
        let set = test_set();
        // Real speech should not be filtered
        assert!(!is_hallucination("hello, how are you doing today?", &set));
        // Known hallucination should be filtered
        assert!(is_hallucination("thank you for watching", &set));
    }
}
