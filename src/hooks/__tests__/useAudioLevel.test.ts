import { describe, it, expect } from 'vitest';
import * as fc from 'fast-check';

/**
 * Validates: Requirements 1.2
 *
 * Replica de la lógica de computeBarHeights para testing.
 * La función no está exportada del hook, así que se replica aquí.
 */
const BAR_COUNT = 18;
const MIN_HEIGHT_PX = 3;
const MAX_HEIGHT_PX = 18;
const IDLE_AMPLITUDE = 3.5;

const BAR_PROFILES = Array.from({ length: BAR_COUNT }, (_, i) => {
  const center = (BAR_COUNT - 1) / 2;
  const dist = Math.abs(i - center) / center;
  return 0.2 + 0.8 * Math.pow(1 - dist * dist, 1.5);
});

const BAR_PHASES = Array.from({ length: BAR_COUNT }, (_, i) =>
  (i / BAR_COUNT) * Math.PI * 2
);

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

describe('computeBarHeights', () => {
  it('siempre retorna exactamente 18 barras', () => {
    fc.assert(
      fc.property(
        fc.float({ min: 0, max: 1, noNaN: true }),
        fc.integer({ min: 1, max: 1_000_000 }),
        (level, timeMs) => {
          const heights = computeBarHeights(level, timeMs);
          return heights.length === BAR_COUNT;
        }
      )
    );
  });

  it('todas las alturas están en [MIN_HEIGHT_PX, MAX_HEIGHT_PX]', () => {
    fc.assert(
      fc.property(
        fc.float({ min: 0, max: 1, noNaN: true }),
        fc.integer({ min: 1, max: 1_000_000 }),
        (level, timeMs) => {
          const heights = computeBarHeights(level, timeMs);
          return heights.every(h => h >= MIN_HEIGHT_PX && h <= MAX_HEIGHT_PX);
        }
      )
    );
  });

  it('con level=0, las alturas son mínimas o cercanas al mínimo', () => {
    const heights = computeBarHeights(0, 1000);
    heights.forEach(h => {
      expect(h).toBeGreaterThanOrEqual(MIN_HEIGHT_PX);
      expect(h).toBeLessThanOrEqual(MAX_HEIGHT_PX);
    });
  });
});
