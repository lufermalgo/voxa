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
    hallucination_set: HashSet<String>,
}

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
            
        Ok(Self {
            context,
            hallucination_set: build_hallucination_set(),
        })
    }

    pub fn transcribe(&self, audio_data: &[f32], language: &str, initial_prompt: &str) -> Result<String, String> {
        log::info!("WHISPER: Starting transcription with {} samples, language: {}, prompt: \"{}\"", audio_data.len(), language, initial_prompt);
        
        // BeamSearch with beam_size=5 is Whisper's default and significantly reduces hallucinations
        // compared to Greedy, especially on uncertain audio (high entropy segments).
        let mut params = FullParams::new(SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 });

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

        log::debug!("WHISPER: Creating state...");
        let mut state = self.context.create_state().map_err(|e| {
            log::error!("WHISPER: Failed to create state: {}", e);
            e.to_string()
        })?;

        log::debug!("WHISPER: Running inference (full)...");
        state.full(params, audio_data).map_err(|e| {
            log::error!("WHISPER: Inference failed: {}", e);
            e.to_string()
        })?;

        let num_segments = state.full_n_segments().map_err(|e| e.to_string())?;
        log::debug!("WHISPER: Finished inference. Segments found: {}", num_segments);

        let mut result = String::new();
        for i in 0..num_segments {
            let segment = state.full_get_segment_text(i).map_err(|e| e.to_string())?;
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
