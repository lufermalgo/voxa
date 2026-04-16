import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

const DEFAULT_MAX_SECONDS = 90;
const WARNING_THRESHOLD = 0.8; // 80% → 72s

interface RecordingDuration {
  progress: number;   // 0.0 – 1.0
  isWarning: boolean; // true when progress >= WARNING_THRESHOLD
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
  };
}
