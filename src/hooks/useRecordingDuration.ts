import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Testing value: 60s. Production target: 360s (6 minutes).
// Change DEFAULT_MAX_SECONDS to 360 once manual tests confirm the pipeline
// handles long recordings correctly end-to-end.
const DEFAULT_MAX_SECONDS = 60;
const WARNING_THRESHOLD = 0.8; // 80% → 48s (testing) / 288s (production)

interface RecordingDuration {
  progress: number;      // 0.0 – 1.0
  isWarning: boolean;    // true when progress >= WARNING_THRESHOLD
  timeRemaining: number; // seconds remaining (ceil), 0 when done
}

/**
 * Tracks elapsed recording time and auto-stops at maxSeconds.
 *
 * Progress is computed in the frontend via setInterval (not rAF — rAF is
 * throttled in Tauri accessory-mode windows). The auto-stop invokes
 * stop_and_transcribe, which is identical to the user releasing the hotkey.
 */
export function useRecordingDuration(
  isRecording: boolean,
  maxSeconds = DEFAULT_MAX_SECONDS
): RecordingDuration {
  const [progress, setProgress] = useState(0);
  const startTimeRef = useRef<number | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    if (!isRecording) {
      if (intervalRef.current) clearInterval(intervalRef.current);
      startTimeRef.current = null;
      setProgress(0);
      return;
    }

    startTimeRef.current = performance.now();

    intervalRef.current = setInterval(() => {
      if (startTimeRef.current === null) return;

      const elapsed = (performance.now() - startTimeRef.current) / 1000;
      const p = Math.min(elapsed / maxSeconds, 1.0);
      setProgress(p);

      if (p >= 1.0) {
        if (intervalRef.current) clearInterval(intervalRef.current);
        invoke("stop_and_transcribe").catch(() => {
          // Recording may have already stopped — ignore.
        });
      }
    }, 33); // ~30fps — matches audio-level polling cadence

    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [isRecording, maxSeconds]);

  return {
    progress,
    isWarning: progress >= WARNING_THRESHOLD,
    timeRemaining: Math.ceil((1 - progress) * maxSeconds),
  };
}
