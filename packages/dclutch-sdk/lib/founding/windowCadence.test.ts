import { describe, expect, it } from 'vitest';

import {
  PROVIDER_WINDOW_REFUSALS_V1,
  PYTH_NOMINAL_CADENCE_SECONDS_V1,
  PYTH_SOL_USD_MEASURED_P50_SECONDS_V1,
  TERMINAL_WINDOW_GUIDANCE_SECONDS_V1,
  TERMINAL_WINDOW_ROBUST_SECONDS_V1,
  WINDOW_CADENCE_TABLE_V1,
  assessWindowWidthV1,
  resolutionDeadlineV1,
} from './windowCadence';

describe('the §12.3 window-width table', () => {
  it('reproduces every printed probability from the model the section states', () => {
    // The table prints figures rounded for reading; the model is
    // `1 - exp(-W / 313)`. Recomputing each row is how a transcription slip in
    // the table shows up as a disagreement rather than as plausible-looking prose.
    const expected = ['~0.3%', '~62%', '~85%', '~98%', '~99.7%'];
    WINDOW_CADENCE_TABLE_V1.forEach((row, index) => {
      const computed = assessWindowWidthV1(row.seconds).publicationProbability * 100;
      const printed = Number(expected[index].replace(/[~%]/g, ''));
      expect(row.publishedProbability).toBe(expected[index]);
      // Within half of the last printed digit, which is what "~" claims.
      const tolerance = printed < 1 ? 0.05 : printed > 99 ? 0.05 : 0.5;
      expect(Math.abs(computed - printed)).toBeLessThanOrEqual(tolerance);
    });
  });

  it('keeps the measured p50 and the nominal cadence as different numbers', () => {
    // 313 is what the probability is computed from; 300 is what the row labels
    // count, and the section mixes them: its 300 s and 600 s rows are one and
    // two *nominal* cadences, while 1,250 is four *measured* ones rounded down
    // (4 x 313 = 1,252). Both readings are pinned here so the discrepancy is
    // recorded rather than quietly resolved in one direction.
    expect(PYTH_SOL_USD_MEASURED_P50_SECONDS_V1).toBe(313);
    expect(PYTH_NOMINAL_CADENCE_SECONDS_V1).toBe(300);
    expect(TERMINAL_WINDOW_GUIDANCE_SECONDS_V1).toBeGreaterThan(4 * PYTH_NOMINAL_CADENCE_SECONDS_V1);
    expect(Math.abs(TERMINAL_WINDOW_GUIDANCE_SECONDS_V1 - 4 * PYTH_SOL_USD_MEASURED_P50_SECONDS_V1)).toBeLessThanOrEqual(2);
  });

  it('rises monotonically with width and stays inside [0, 1]', () => {
    let previous = -1;
    for (const seconds of [0, 1, 60, 300, 600, 1_250, 1_800]) {
      const { publicationProbability } = assessWindowWidthV1(seconds);
      expect(publicationProbability).toBeGreaterThan(previous);
      expect(publicationProbability).toBeGreaterThanOrEqual(0);
      expect(publicationProbability).toBeLessThan(1);
      previous = publicationProbability;
    }
    // Far outside the table, `1 - exp(-W/313)` saturates to exactly 1.0 in
    // float64 (around W = 11,000 s). That is a display artefact and not a claim
    // that publication is certain; the assessment must stay in range and must
    // never exceed 1, which is all a bar or a percentage needs of it.
    const day = assessWindowWidthV1(86_400).publicationProbability;
    expect(day).toBeLessThanOrEqual(1);
    expect(day).toBeGreaterThanOrEqual(assessWindowWidthV1(1_800).publicationProbability);
  });
});

describe('assessing one chosen width', () => {
  it('names the one-second shape unanswerable rather than merely unlikely', () => {
    const assessment = assessWindowWidthV1(1);
    expect(assessment.confidence).toBe('unanswerable');
    expect(assessment.detail).toMatch(/asked a question nothing could answer/);
  });

  it('classifies each guidance boundary on the side the section puts it', () => {
    expect(assessWindowWidthV1(299).confidence).toBe('below-one-cadence');
    expect(assessWindowWidthV1(300).confidence).toBe('below-guidance');
    expect(assessWindowWidthV1(TERMINAL_WINDOW_GUIDANCE_SECONDS_V1 - 1).confidence).toBe('below-guidance');
    expect(assessWindowWidthV1(TERMINAL_WINDOW_GUIDANCE_SECONDS_V1).confidence).toBe('meets-guidance');
    expect(assessWindowWidthV1(TERMINAL_WINDOW_ROBUST_SECONDS_V1 - 1).confidence).toBe('meets-guidance');
    expect(assessWindowWidthV1(TERMINAL_WINDOW_ROBUST_SECONDS_V1).confidence).toBe('meets-robust-guidance');
  });

  it('admits a zero-width window rather than refusing it', () => {
    // Terminal requires only `start <= end`. A degenerate window is legal and
    // the wizard's job is to price it, not to forbid it.
    expect(assessWindowWidthV1(0).confidence).toBe('unanswerable');
    expect(() => assessWindowWidthV1(-1)).toThrow(/whole number of seconds/);
    expect(() => assessWindowWidthV1(1.5)).toThrow(/whole number of seconds/);
  });
});

describe('the deadline budget §12.3 insists is separate', () => {
  it('places the failure walk immediately after the primary deadline, with no gap', () => {
    const deadline = resolutionDeadlineV1(1_000, 2_250, 600);
    expect(deadline.primaryDeadline).toBe(2_850);
    expect(deadline.failureWalkOpensAt).toBe(2_851);
  });

  it('is unmoved by widening the window alone', () => {
    // Widening the window raises the chance a publication is *about* the
    // period. It does nothing for whether a keeper can land the transaction in
    // time, which is what `max_age` bounds.
    const narrow = resolutionDeadlineV1(1_000, 1_001, 600);
    const wide = resolutionDeadlineV1(1_000, 2_800, 600);
    expect(wide.maxAgeSeconds).toBe(narrow.maxAgeSeconds);
    expect(wide.primaryDeadline - wide.windowEnd).toBe(narrow.primaryDeadline - narrow.windowEnd);
  });

  it('requires start <= end and a whole-second max age', () => {
    expect(() => resolutionDeadlineV1(2_000, 1_000, 600)).toThrow(/start <= end/);
    expect(() => resolutionDeadlineV1(1_000, 2_000, -1)).toThrow(/whole number of seconds/);
    expect(resolutionDeadlineV1(1_000, 1_000, 0).primaryDeadline).toBe(1_000);
  });
});

describe('the three named provider refusals', () => {
  it('separates the two come-back-later answers from the one that is not', () => {
    expect(PROVIDER_WINDOW_REFUSALS_V1.map((entry) => entry.code)).toEqual(['0x8011', '0x8012', '0x8013']);
    expect(PROVIDER_WINDOW_REFUSALS_V1.filter((entry) => entry.retriable).map((entry) => entry.name)).toEqual(['ProviderWindow', 'ProviderFreshness']);
    expect(PROVIDER_WINDOW_REFUSALS_V1.filter((entry) => !entry.retriable).map((entry) => entry.name)).toEqual(['ProviderConfiguration']);
  });
});
