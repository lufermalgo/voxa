import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

const EMA_ALPHA = 0.15;      // lower = smoother, less reactive to noise spikes
const BAR_COUNT = 18;
const MIN_HEIGHT_PX = 3;
const MAX_HEIGHT_PX = 32;
const IDLE_AMPLITUDE = 3.5;
const NOISE_FLOOR = 0.08;    // levels below this are treated as silence

// Bell curve profile: center = 1.0, edges = 0.2
const BAR_PROFILES = Array.from({ length: BAR_COUNT }, (_, i) => {
  const center = (BAR_COUNT - 1) / 2;
  const dist = Math.abs(i - center) / center;
  return 0.2 + 0.8 * Math.pow(1 - dist * dist, 1.5);
});

// Fixed phase offset per bar: uniformly distributed across 0..2π
const BAR_PHASES = Array.from({ length: BAR_COUNT }, (_, i) =>
  (i / BAR_COUNT) * Math.PI * 2
);

// Each bar has its own frequency: 1.2..2.4 Hz
const BAR_FREQS = Array.from({ length: BAR_COUNT }, (_, i) =>
  1.8 + Math.sin(i * 1.7) * 0.6
);

function computeBarHeights(level: number, timeMs: number): number[] {
  const timeSec = timeMs / 1000;

  return BAR_PROFILES.map((profile, i) => {
    const phase = BAR_PHASES[i];
    const freq = BAR_FREQS[i];
    const oscillation = Math.sin(timeSec * freq * Math.PI * 2 + phase);

    const speechAmp = level * profile * (MAX_HEIGHT_PX - MIN_HEIGHT_PX);
    const amplitude = IDLE_AMPLITUDE * profile + speechAmp * 0.6;

    const baseline = MIN_HEIGHT_PX + speechAmp * 0.4;
    const h = baseline + oscillation * amplitude;
    return Math.max(MIN_HEIGHT_PX, Math.min(MAX_HEIGHT_PX, h));
  });
}

export function useAudioLevel(isRecording: boolean): number[] {
  const [barHeights, setBarHeights] = useState<number[]>(
    Array(BAR_COUNT).fill(MIN_HEIGHT_PX)
  );
  const smoothedRef = useRef(0);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    if (!isRecording) {
      smoothedRef.current = 0;
      if (intervalRef.current) clearInterval(intervalRef.current);
      setBarHeights(Array(BAR_COUNT).fill(MIN_HEIGHT_PX));
      return;
    }

    const unlistenPromise = listen<number>("audio-level", (event) => {
      const raw = event.payload;
      // Apply noise floor: anything below threshold is treated as silence
      const gated = raw < NOISE_FLOOR ? 0 : raw;
      // Amplify voice signal so the animation reacts more dramatically to actual speech
      const amplified = Math.min(gated * 1.8, 1.0);
      smoothedRef.current = EMA_ALPHA * amplified + (1 - EMA_ALPHA) * smoothedRef.current;
    });

    intervalRef.current = setInterval(() => {
      setBarHeights(computeBarHeights(smoothedRef.current, performance.now()));
    }, 33);

    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
      unlistenPromise.then((f) => f());
      smoothedRef.current = 0;
    };
  }, [isRecording]);

  return barHeights;
}
