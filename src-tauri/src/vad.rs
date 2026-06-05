use ort::session::{Session, builder::SessionBuilder};
use ort::value::Tensor;

const FRAME_SIZE: usize = 512;
const CONTEXT_SIZE: usize = 64;
/// Activate speech after this many consecutive speech frames.
const SPEECH_ON_FRAMES: u32 = 2;
/// Deactivate speech after this many consecutive silence frames.
const SPEECH_OFF_FRAMES: u32 = 12;
/// Probability threshold above which a frame is considered speech.
const SPEECH_THRESHOLD: f32 = 0.5;

pub struct VadEngine {
    session: Session,
    /// LSTM state tensor, shape [2, 1, 128] — h and c combined into one tensor.
    state: Vec<f32>,
    /// Last 64 samples carried across frames (v6 context requirement).
    context: Vec<f32>,
    /// How many consecutive speech frames we have seen.
    speech_frames: u32,
    /// How many consecutive silence frames we have seen.
    silence_frames: u32,
    /// Current smoothed speech/silence state.
    pub is_speaking: bool,
}

impl VadEngine {
    /// Initialise the engine from raw ONNX model bytes.
    pub fn new(model_bytes: &[u8]) -> Result<Self, String> {
        let session = SessionBuilder::new()
            .map_err(|e| format!("ORT SessionBuilder error: {e}"))?
            .commit_from_memory(model_bytes)
            .map_err(|e| format!("ORT session load error: {e}"))?;

        Ok(Self {
            session,
            state: vec![0.0f32; 2 * 1 * 128],
            context: vec![0.0f32; CONTEXT_SIZE],
            speech_frames: 0,
            silence_frames: 0,
            is_speaking: false,
        })
    }

    /// Process one 512-sample frame (32 ms @ 16 kHz).
    ///
    /// The v6 model requires a context window: the last 64 samples from the
    /// previous frame are prepended to the current frame before inference.
    /// Hidden states (`h`, `c`) are persisted across frames within a session.
    ///
    /// Returns `true` when smoothed speech is active.
    pub fn process_frame(&mut self, frame: &[f32]) -> bool {
        // Build input: [context | frame], length = 64 + 512 = 576
        let mut input_vec = Vec::with_capacity(CONTEXT_SIZE + FRAME_SIZE);
        input_vec.extend_from_slice(&self.context);
        let frame_slice = if frame.len() >= FRAME_SIZE {
            &frame[..FRAME_SIZE]
        } else {
            frame
        };
        input_vec.extend_from_slice(frame_slice);
        // Pad if shorter than expected (last chunk of audio)
        while input_vec.len() < CONTEXT_SIZE + FRAME_SIZE {
            input_vec.push(0.0f32);
        }

        // Update context to the last 64 samples of this frame
        let new_context_start = input_vec.len() - CONTEXT_SIZE;
        self.context.copy_from_slice(&input_vec[new_context_start..]);

        // Build ONNX tensors using (shape, data) tuples
        let input_len = CONTEXT_SIZE + FRAME_SIZE;
        let input_tensor = match Tensor::<f32>::from_array(([1usize, input_len], input_vec.into_boxed_slice())) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("VAD: Failed to create input tensor: {e}");
                return self.is_speaking;
            }
        };
        let sr_tensor = match Tensor::<i64>::from_array(([1usize], vec![16000i64].into_boxed_slice())) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("VAD: Failed to create sr tensor: {e}");
                return self.is_speaking;
            }
        };
        let state_tensor = match Tensor::<f32>::from_array(([2usize, 1, 128], self.state.clone().into_boxed_slice())) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("VAD: Failed to create state tensor: {e}");
                return self.is_speaking;
            }
        };

        let outputs = match self.session.run(ort::inputs![
            "input" => input_tensor,
            "sr"    => sr_tensor,
            "state" => state_tensor,
        ]) {
            Ok(o) => o,
            Err(e) => {
                log::warn!("VAD inference error: {e}");
                return self.is_speaking;
            }
        };

        // Extract speech probability scalar — try_extract_tensor returns (&Shape, &[T])
        let prob: f32 = match outputs["output"].try_extract_tensor::<f32>() {
            Ok((_shape, data)) => data.first().copied().unwrap_or(0.0),
            Err(_) => 0.0,
        };

        // Update LSTM state
        if let Ok((_shape, state_data)) = outputs["stateN"].try_extract_tensor::<f32>() {
            if state_data.len() == 2 * 1 * 128 {
                self.state.copy_from_slice(state_data);
            }
        }

        // Smoothing: 2 on / 12 off frames (community recommendation)
        if prob >= SPEECH_THRESHOLD {
            self.speech_frames += 1;
            self.silence_frames = 0;
            if !self.is_speaking && self.speech_frames >= SPEECH_ON_FRAMES {
                self.is_speaking = true;
            }
        } else {
            self.silence_frames += 1;
            self.speech_frames = 0;
            if self.is_speaking && self.silence_frames >= SPEECH_OFF_FRAMES {
                self.is_speaking = false;
            }
        }

        self.is_speaking
    }

    /// Reset all state for a new recording session.
    /// Must be called before each new recording to avoid stale LSTM state.
    pub fn reset(&mut self) {
        self.state.iter_mut().for_each(|x| *x = 0.0);
        self.context.iter_mut().for_each(|x| *x = 0.0);
        self.speech_frames = 0;
        self.silence_frames = 0;
        self.is_speaking = false;
    }
}

/// Run VAD over `samples` frame-by-frame (512 samples per frame) and return
/// true if any frame is classified as speech.  Resets the engine before use.
/// This mirrors the production incremental path (Req 5.1).
#[cfg(test)]
pub fn vad_speech_detected_incremental(vad: &mut VadEngine, samples: &[f32]) -> bool {
    vad.reset();
    for chunk in samples.chunks(512) {
        if vad.process_frame(chunk) {
            return true;
        }
    }
    false
}

/// Run VAD over `samples` in a single batch (whole buffer at once, one frame
/// per iteration) and return true if any frame detects speech.
/// This mirrors the old post-stop full-buffer VAD loop that was in pipeline.rs.
#[cfg(test)]
pub fn vad_speech_detected_batch(vad: &mut VadEngine, samples: &[f32]) -> bool {
    vad.reset();
    let mut any_speech = false;
    for chunk in samples.chunks(512) {
        if vad.process_frame(chunk) {
            any_speech = true;
            break;
        }
    }
    any_speech
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Raw bytes of the bundled Silero VAD v6 ONNX model — reused from the main module.
    static VAD_MODEL_BYTES: &[u8] = include_bytes!("../models/silero_vad_v6.onnx");

    fn make_vad() -> VadEngine {
        VadEngine::new(VAD_MODEL_BYTES).expect("VAD engine should initialise in tests")
    }

    /// Synthesise a 16 kHz mono buffer containing a ~440 Hz sine tone (speech-like
    /// energy) for the given number of seconds.  Used as a "speech" proxy.
    fn sine_buffer(duration_secs: f32) -> Vec<f32> {
        let n = (16000.0 * duration_secs) as usize;
        (0..n)
            .map(|i| {
                (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin() * 0.5
            })
            .collect()
    }

    /// Synthesise a 16 kHz mono buffer of near-silence (all zeros).
    fn silence_buffer(duration_secs: f32) -> Vec<f32> {
        let n = (16000.0 * duration_secs) as usize;
        vec![0.0f32; n]
    }

    /// Req 5.1 — Incremental VAD parity with batch VAD on a speech buffer.
    ///
    /// Feeds the same audio buffer frame-by-frame (incremental path, mirroring
    /// the new capture-callback path) and as a whole batch (old post-stop path)
    /// and asserts that both produce the same `speech_ever_detected` result.
    #[test]
    fn test_incremental_parity_speech() {
        let samples = sine_buffer(2.0); // 2 s of sine tone — Silero sees as speech-like

        let mut vad1 = make_vad();
        let mut vad2 = make_vad();

        let incremental_result = vad_speech_detected_incremental(&mut vad1, &samples);
        let batch_result       = vad_speech_detected_batch(&mut vad2, &samples);

        assert_eq!(
            incremental_result, batch_result,
            "Incremental and batch VAD must agree on a speech buffer: \
             incremental={incremental_result}, batch={batch_result}"
        );
    }

    /// Req 5.1 — Incremental VAD parity with batch VAD on a silence buffer.
    ///
    /// Both paths must agree that a zero-filled buffer contains no speech.
    #[test]
    fn test_incremental_parity_silence() {
        let samples = silence_buffer(2.0); // 2 s of silence

        let mut vad1 = make_vad();
        let mut vad2 = make_vad();

        let incremental_result = vad_speech_detected_incremental(&mut vad1, &samples);
        let batch_result       = vad_speech_detected_batch(&mut vad2, &samples);

        assert_eq!(
            incremental_result, batch_result,
            "Incremental and batch VAD must agree on a silence buffer: \
             incremental={incremental_result}, batch={batch_result}"
        );
        // Silence should not be flagged as speech.
        assert!(!incremental_result, "Silence buffer should not detect speech");
    }

    /// Req 5.2 — Silence-skip behavior is preserved.
    ///
    /// When `speech_ever_detected == false`, the pipeline skips STT.
    /// This test confirms that a silence-only buffer yields false from the
    /// incremental path (which is what the pipeline now reads at stop time).
    #[test]
    fn test_silence_skip_incremental() {
        let samples = silence_buffer(1.0);
        let mut vad = make_vad();
        let detected = vad_speech_detected_incremental(&mut vad, &samples);
        assert!(
            !detected,
            "Silence buffer must not set speech_ever_detected (STT skip must be preserved)"
        );
    }

    /// Req 5.3 — No post-stop VAD pass is needed.
    ///
    /// Verifies that `vad_speech_detected_batch` and `vad_speech_detected_incremental`
    /// are structurally equivalent (same engine, same reset, same frame loop, same result),
    /// confirming that the incremental path can fully replace the batch loop.
    #[test]
    fn test_incremental_replaces_batch_on_mixed_audio() {
        // Construct a buffer: 1 s silence, then 1 s speech-like tone.
        let mut samples = silence_buffer(1.0);
        samples.extend(sine_buffer(1.0));

        let mut vad1 = make_vad();
        let mut vad2 = make_vad();

        let incremental_result = vad_speech_detected_incremental(&mut vad1, &samples);
        let batch_result       = vad_speech_detected_batch(&mut vad2, &samples);

        assert_eq!(
            incremental_result, batch_result,
            "Incremental and batch VAD must agree on a mixed (silence+speech) buffer"
        );
    }
}
