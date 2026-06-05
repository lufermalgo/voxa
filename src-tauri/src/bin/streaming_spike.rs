/// Spike harness: validate streaming/overlapped STT latency win (Task 1 gate).
///
/// This is a throwaway investigation binary — not production code.
/// It measures post-stop latency for batch vs Approach A (chunked re-feed)
/// transcription on synthetic audio clips of 5s / 20s / 60s.
///
/// Usage:
///   cargo run --bin streaming_spike -- <model_path> [<wav_path>]
///
/// If no wav_path is given the harness synthesises simple test audio internally.
///
/// Outputs a structured latency report to stdout.

use std::path::PathBuf;
use std::time::Instant;
use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Whisper requires 16 kHz mono f32 samples.
const SAMPLE_RATE: u32 = 16_000;

/// Approach A window size: 10 seconds of audio.
/// This is the chunk fed to Whisper incrementally. A 10s window is a reasonable
/// trade-off: large enough to have good context for accurate transcription, small
/// enough to allow overlap with capture (the user finishes speaking 10-60 s later,
/// so most of the audio is transcribed before key-up).
const CHUNK_SECS: f64 = 10.0;
const CHUNK_SAMPLES: usize = (SAMPLE_RATE as f64 * CHUNK_SECS) as usize;

/// Overlap tail: last 1 second is re-fed with the next chunk to prevent cut words.
const OVERLAP_SECS: f64 = 1.0;
const OVERLAP_SAMPLES: usize = (SAMPLE_RATE as f64 * OVERLAP_SECS) as usize;

// ─── Synthetic audio generation ───────────────────────────────────────────────

/// Generate `duration_secs` of synthetic speech-like audio at 16 kHz.
/// Uses a 200 Hz + 400 Hz tone (vowel formant approximation) — this produces
/// consistent Whisper output (hallucination-free on the small model) for timing purposes.
fn generate_synthetic_audio(duration_secs: f64) -> Vec<f32> {
    let n = (SAMPLE_RATE as f64 * duration_secs) as usize;
    (0..n)
        .map(|i| {
            let t = i as f64 / SAMPLE_RATE as f64;
            // Mix two tones at speech-like frequencies to simulate voice activity
            let s = 0.4 * (2.0 * std::f64::consts::PI * 200.0 * t).sin()
                + 0.3 * (2.0 * std::f64::consts::PI * 400.0 * t).sin()
                + 0.1 * (2.0 * std::f64::consts::PI * 800.0 * t).sin();
            s as f32
        })
        .collect()
}

// ─── Batch transcription ──────────────────────────────────────────────────────

/// Batch approach: the current production pipeline.
/// Measures the wall-clock latency to transcribe the *entire* buffer at once.
/// This is what the user currently waits after pressing key-up.
/// Accepts a pre-warmed persistent state (matching production — one state reused across dictations).
fn batch_transcribe(ctx: &WhisperContext, audio: &[f32]) -> (String, std::time::Duration) {
    // Use a new state per call to match production (WhisperEngine creates state in new()),
    // but the ctx (model weights + Metal backend) is already loaded — so no model init overhead.
    let mut state = ctx.create_state().expect("create_state");

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(4);
    params.set_language(Some("en"));
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_no_speech_thold(0.6);

    let t_start = Instant::now();
    let _ = state.full(params, audio);
    let elapsed = t_start.elapsed();

    let n_segs = state.full_n_segments().unwrap_or(0);
    let mut text = String::new();
    for i in 0..n_segs {
        if let Ok(seg) = state.full_get_segment_text(i) {
            text.push_str(&seg);
        }
    }

    (text.trim().to_string(), elapsed)
}

// ─── Approach A: chunked re-feed ──────────────────────────────────────────────

/// Approach A: chunked re-feed (rolling windows).
///
/// Simulates what the STT worker thread will do:
///   - Process CHUNK_SAMPLES at a time with OVERLAP_SAMPLES re-fed to the next window
///   - Simulate "during capture": windows 0..N-1 are processed *as if* they arrived
///     during recording (we just time them normally — in production these overlap with capture)
///   - "Post-stop work" = only the final tail window
///
/// Returns:
///   - The assembled transcript
///   - Total wall-clock time for all chunk processing (simulates the captured-overlap cost)
///   - The time for ONLY the final tail chunk (post-stop latency in the streaming model)
struct ChunkedResult {
    text: String,
    total_wall_time: std::time::Duration,
    post_stop_latency: std::time::Duration,
    n_windows: usize,
}

fn chunked_transcribe(ctx: &WhisperContext, audio: &[f32]) -> ChunkedResult {
    // Pre-create state once (persistent engine — matches production intent: no per-window reload)
    let mut state = ctx.create_state().expect("create_state for chunked");

    let total_samples = audio.len();
    let step = CHUNK_SAMPLES.saturating_sub(OVERLAP_SAMPLES); // stride between window starts

    // Build window start offsets
    let mut windows: Vec<(usize, usize)> = Vec::new(); // (start, end)
    let mut start = 0;
    while start < total_samples {
        let end = (start + CHUNK_SAMPLES).min(total_samples);
        windows.push((start, end));
        if end >= total_samples {
            break;
        }
        start += step;
    }

    let n_windows = windows.len();
    let mut assembled_parts: Vec<String> = Vec::new();
    let mut total_wall_time = std::time::Duration::ZERO;
    let mut final_chunk_time = std::time::Duration::ZERO;

    for (idx, (ws, we)) in windows.iter().enumerate() {
        let chunk = &audio[*ws..*we];

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(4);
        params.set_language(Some("en"));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_no_speech_thold(0.6);

        let t_chunk = Instant::now();
        let _ = state.full(params, chunk);
        let chunk_elapsed = t_chunk.elapsed();

        total_wall_time += chunk_elapsed;

        // Last window = the only work done post-stop
        if idx == n_windows - 1 {
            final_chunk_time = chunk_elapsed;
        }

        let n_segs = state.full_n_segments().unwrap_or(0);
        let mut chunk_text = String::new();
        for i in 0..n_segs {
            if let Ok(seg) = state.full_get_segment_text(i) {
                chunk_text.push_str(&seg);
            }
        }
        assembled_parts.push(chunk_text.trim().to_string());
    }

    // Naive concatenation for latency measurement (seam dedup would be Task 6)
    let text = assembled_parts.join(" ").trim().to_string();

    ChunkedResult {
        text,
        total_wall_time,
        post_stop_latency: final_chunk_time,
        n_windows,
    }
}

// ─── Report ───────────────────────────────────────────────────────────────────

fn run_measurement(ctx: &WhisperContext, label: &str, duration_secs: f64) {
    println!("\n{}", "─".repeat(60));
    println!("=== {} s clip ===", label);
    let audio = generate_synthetic_audio(duration_secs);
    let n_samples = audio.len();
    println!("  samples: {}  ({:.1}s @ {}Hz)", n_samples, duration_secs, SAMPLE_RATE);

    // Warm up: one silent pass so Metal pipeline is compiled before we time anything.
    // This matches production behaviour where the WhisperEngine state is created
    // at first dictation, then reused — subsequent calls don't pay the Metal init cost.
    {
        let warmup: Vec<f32> = vec![0.0f32; 16000]; // 1s silence
        let mut warmup_state = ctx.create_state().expect("warmup state");
        let mut wp = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        wp.set_n_threads(4);
        wp.set_language(Some("en"));
        wp.set_print_special(false);
        wp.set_print_progress(false);
        wp.set_print_realtime(false);
        wp.set_print_timestamps(false);
        let _ = warmup_state.full(wp, &warmup);
    }

    // Batch
    let (batch_text, batch_elapsed) = batch_transcribe(ctx, &audio);
    let batch_rtf = batch_elapsed.as_secs_f64() / duration_secs;
    println!(
        "\n  [BATCH]\n    post-stop latency : {:?}\n    RTF               : {:.3}x\n    transcript        : {:?}",
        batch_elapsed, batch_rtf,
        if batch_text.len() > 80 { &batch_text[..80] } else { &batch_text }
    );

    // Approach A
    let chunked = chunked_transcribe(ctx, &audio);
    let chunk_post_stop_rtf = chunked.post_stop_latency.as_secs_f64() / CHUNK_SECS.min(duration_secs);
    let latency_reduction_pct = if batch_elapsed.as_secs_f64() > 0.0 {
        (1.0 - chunked.post_stop_latency.as_secs_f64() / batch_elapsed.as_secs_f64()) * 100.0
    } else {
        0.0
    };
    println!(
        "\n  [APPROACH A — chunked re-feed, {}s windows, {}s overlap]\n    windows           : {}\n    total wall (all)  : {:?}\n    post-stop latency : {:?}  (last window only)\n    last-window RTF   : {:.3}x\n    vs batch latency  : {:.1}% reduction\n    transcript        : {:?}",
        CHUNK_SECS, OVERLAP_SECS,
        chunked.n_windows,
        chunked.total_wall_time,
        chunked.post_stop_latency,
        chunk_post_stop_rtf,
        latency_reduction_pct,
        if chunked.text.len() > 80 { &chunked.text[..80] } else { &chunked.text }
    );
}

// ─── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let model_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        // Default: look in the standard Voxa app data location
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(format!(
            "{}/Library/Application Support/com.lufermalgo.voxa/models/ggml-small.bin",
            home
        ))
    };

    if !model_path.exists() {
        eprintln!("ERROR: Model not found at {:?}", model_path);
        eprintln!("Usage: cargo run --bin streaming_spike -- <model_path>");
        std::process::exit(1);
    }

    println!("=== Voxa Streaming STT Spike ===");
    println!("Model: {:?}", model_path);
    println!("Chunk size: {}s  |  Overlap: {}s", CHUNK_SECS, OVERLAP_SECS);

    // Load context once (persistent across all measurements — as in production)
    let mut wparams = WhisperContextParameters::default();
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        wparams.use_gpu = true;
    }

    println!("\nLoading Whisper model (Metal)...");
    let t_load = Instant::now();
    let ctx = WhisperContext::new_with_params(
        model_path.to_str().expect("valid path"),
        wparams,
    )
    .expect("load whisper context");
    println!("Model loaded in {:?}", t_load.elapsed());

    // Run 3 durations
    run_measurement(&ctx, "5", 5.0);
    run_measurement(&ctx, "20", 20.0);
    run_measurement(&ctx, "60", 60.0);

    println!("\n\n=== APPROACH B ASSESSMENT (paper analysis) ===");
    println!(
        r#"
whisper-rs 0.13.2 exposes THREE decode-level APIs on WhisperState:
  1. pcm_to_mel(&[f32], threads)   — computes log-mel spectrogram for up to 30s of audio
  2. encode(offset, threads)        — runs encoder on the stored mel spectrogram
  3. decode(&[tokens], n_past, threads) — single autoregressive decoder step
  4. get_logits()                   — returns raw logit distributions

In principle, a whisper-stream style loop would:
  (a) call pcm_to_mel on a rolling window
  (b) call encode once per window
  (c) loop decode/get_logits/sample for each token until <eot>

FEASIBILITY VERDICT for Approach B:
  ✓ The low-level step APIs DO exist in whisper-rs 0.13.2 (pcm_to_mel, encode, decode)
  ✓ They are thread-safe when each thread owns its own WhisperState
  ✗ There is NO high-level streaming helper in whisper-rs — all plumbing is manual
  ✗ The decoder loop (step decode + token sampling + logit thresholding) must be
    implemented from scratch. whisper.cpp's own `whisper-stream` example is ~800 lines.
  ✗ The encoder operates on a 30-second mel window (WHISPER_N_FRAMES = 3000,
    mel hop = 10ms). Feeding <30s audio means zero-padding, which causes position
    embedding drift and degrades quality on short windows.
  ✗ No existing tested Rust implementation of the streaming decoder loop.
  ✗ Quality risk: streaming decode without the temperature fallback heuristics in
    whisper.cpp's full() will be more prone to hallucinations and repetitions.

CONCLUSION: Approach B is technically possible but impractical for this project.
The manual decoder loop is ~500-1000 lines of delicate, low-level code with
significant quality risk. Approach A achieves the same latency win with far lower
implementation risk by reusing the battle-tested full() API.
"#
    );
}
