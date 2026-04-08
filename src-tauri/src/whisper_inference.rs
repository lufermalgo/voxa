use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};
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

/// Removes Whisper hallucination tokens that appear when processing silence or background noise.
/// Pattern: bracketed tokens like [MÚSICA], [Silencio], [Applause], [Music], ♪♪, etc.
fn strip_hallucinations(text: &str) -> String {
    let re = get_hallucination_re();
    let cleaned = re.replace_all(text, "");
    cleaned.trim().to_string()
}

pub struct WhisperEngine {
    context: WhisperContext,
}

impl WhisperEngine {
    pub fn new(model_path: &Path) -> Result<Self, String> {
        if !model_path.exists() {
            return Err("Whisper model file not found".to_string());
        }
        
        let path_str = model_path.to_str().ok_or("Invalid path")?;
        let context = WhisperContext::new_with_params(path_str, WhisperContextParameters::default())
            .map_err(|e| format!("Failed to create whisper context: {}", e))?;
            
        Ok(Self { context })
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
            result.push_str(&segment);
        }

        // Strip Whisper hallucination tokens that appear on silence/music/noise.
        // These are always enclosed in brackets: [MÚSICA], [Silencio], [Applause], etc.
        let final_text = strip_hallucinations(result.trim());
        Ok(final_text)
    }
}
