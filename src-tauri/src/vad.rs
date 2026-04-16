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
