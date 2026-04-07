use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};
use std::path::Path;

/// Removes Whisper hallucination tokens that appear when processing silence or background noise.
/// Pattern: bracketed tokens like [MÚSICA], [Silencio], [Applause], [Music], ♪♪, etc.
fn strip_hallucinations(text: &str) -> String {
    // Known hallucination patterns — Whisper produces these on silent/noisy segments
    const HALLUCINATIONS: &[&str] = &[
        "[MÚSICA]", "[Música]", "[música]", "[MUSIC]", "[Music]", "[music]",
        "[SILENCIO]", "[Silencio]", "[silencio]", "[SILENCE]", "[Silence]",
        "[APLAUSOS]", "[Aplausos]", "[APPLAUSE]", "[Applause]",
        "[RUIDO]", "[Ruido]", "[NOISE]", "[Noise]",
        "[INAUDIBLE]", "[Inaudible]",
        "[RISAS]", "[Risas]", "[LAUGHTER]", "[Laughter]",
        // YouTube-style hallucinations common when Whisper processes silence
        "¡Suscríbete!", "Suscríbete", "Subscribe", "¡Gracias por ver!",
        "Subtítulos por", "Subtitles by",
        "♪♪", "♪ ♪", "♪",
    ];
    let mut result = text.to_string();
    for token in HALLUCINATIONS {
        result = result.replace(token, "");
    }
    result.trim().to_string()
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
        println!("WHISPER: Starting transcription with {} samples, language: {}, prompt: \"{}\"", audio_data.len(), language, initial_prompt);
        
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 0 });

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

        println!("WHISPER: Creating state...");
        let mut state = self.context.create_state().map_err(|e| {
            println!("WHISPER ERROR: Failed to create state: {}", e);
            e.to_string()
        })?;

        println!("WHISPER: Running inference (full)...");
        state.full(params, audio_data).map_err(|e| {
            println!("WHISPER ERROR: Inference failed: {}", e);
            e.to_string()
        })?;

        let num_segments = state.full_n_segments().map_err(|e| e.to_string())?;
        println!("WHISPER: Finished inference. Segments found: {}", num_segments);

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
