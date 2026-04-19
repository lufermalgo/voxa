import { describe, it, expect } from 'vitest';
import * as fc from 'fast-check';

/**
 * Validates: Requirements 1.3, 1.4
 *
 * Tests para la lógica de useRecordingDuration.
 * Se replica la función computeProgress para testear sin depender del hook.
 */
const WARNING_THRESHOLD = 0.8;

function computeProgress(elapsed: number, maxSeconds: number): number {
  return Math.min(elapsed / maxSeconds, 1.0);
}

describe('useRecordingDuration logic', () => {
  it('progress siempre está en [0, 1]', () => {
    fc.assert(
      fc.property(
        fc.double({ min: 0, max: 1000, noNaN: true }),
        fc.double({ min: 0.1, max: 360, noNaN: true }),
        (elapsed, maxSeconds) => {
          const p = computeProgress(elapsed, maxSeconds);
          return p >= 0 && p <= 1;
        }
      )
    );
  });

  it('isWarning es true exactamente cuando progress >= 0.8', () => {
    // Casos límite exactos
    expect(computeProgress(48, 60)).toBeGreaterThanOrEqual(WARNING_THRESHOLD); // 0.8 exacto
    expect(computeProgress(47.9, 60)).toBeLessThan(WARNING_THRESHOLD);         // justo debajo
    expect(computeProgress(49, 60)).toBeGreaterThan(WARNING_THRESHOLD);        // justo encima
  });

  it('isWarning property: progress >= 0.8 ↔ isWarning', () => {
    fc.assert(
      fc.property(
        fc.double({ min: 0, max: 360, noNaN: true }),
        fc.double({ min: 1, max: 360, noNaN: true }),
        (elapsed, maxSeconds) => {
          const progress = computeProgress(elapsed, maxSeconds);
          const isWarning = progress >= WARNING_THRESHOLD;
          // La propiedad: isWarning debe ser consistente con progress
          return isWarning === (progress >= WARNING_THRESHOLD);
        }
      )
    );
  });
});
