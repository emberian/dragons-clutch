/**
 * §12.3 — a terminal window has width, and how wide it has to be.
 *
 * `docs/design/MAINNET_STATE_RELAY.md` §12.3. The section exists because the
 * opposite used to be true: `WindowSpecV1::new` refused `start != end` for a
 * terminal window, so every terminal market asked a question that could only be
 * answered on one exact second, and on a real cluster every one of them walked
 * to its failure outcome instead of resolving. The fixtures could not see it,
 * because each of them chose its window to match its own observation. Terminal
 * now requires only `start <= end`, and the width is the operator's to state.
 *
 * THE OPERATOR HAS NO DEFAULT, AND THAT IS A GAP RATHER THAN A DESIGN.
 * The board's own words, on the entry that opened this work: *"Founding callers
 * must now choose a window width. The operator has no default and the web has
 * no window UI at all. When the create wizard lands, §12.3's table is the
 * guidance it should encode."* This module is that encoding. It offers
 * guidance and it computes the consequence of any width; it does not pick one,
 * because the right width is a statement about the market being sold and not
 * about the type.
 *
 * WHAT THE PROBABILITY IS AND IS NOT. Publications are modelled as a Poisson
 * process at the measured devnet SOL/USD p50 of 313 s, giving
 * `P = 1 - exp(-W / 313)`. §12.3 marks this *provisional* and says why: Pyth
 * publishes on price movement and confidence thresholds rather than on a timer,
 * which makes the real process more regular near the median and heavier in the
 * tail. Treat the number as an order of magnitude, not a guarantee.
 */

/** Measured devnet SOL/USD p50 between publications (lane measurement, 2026-08-27). */
export const PYTH_SOL_USD_MEASURED_P50_SECONDS_V1 = 313;

/**
 * The nominal cadence constant the contract's own tests use.
 *
 * `programs/dclutch-resolution-proof-sbf/src/provider_v3.rs` pins
 * `CADENCE_SECONDS = 300` with the 313 s measurement in its doc comment. The
 * two numbers are deliberately different: 300 is the round figure the row
 * labels count in, 313 is what the probability is computed from.
 */
export const PYTH_NOMINAL_CADENCE_SECONDS_V1 = 300;

/** §12.3's operative guidance: at least four cadences. */
export const TERMINAL_WINDOW_GUIDANCE_SECONDS_V1 = 1_250;

/** §12.3's stronger guidance, for a market that should not fail for provider reasons. */
export const TERMINAL_WINDOW_ROBUST_SECONDS_V1 = 1_800;

export type WindowCadenceRowV1 = Readonly<{
  seconds: number;
  shape: string;
  /** The probability §12.3's own table prints, kept for display beside ours. */
  publishedProbability: string;
}>;

/** §12.3's table, transcribed with its own row labels and printed figures. */
export const WINDOW_CADENCE_TABLE_V1: ReadonlyArray<WindowCadenceRowV1> = Object.freeze([
  Object.freeze({ seconds: 1, shape: 'the old forced shape', publishedProbability: '~0.3%' }),
  Object.freeze({ seconds: 300, shape: 'one cadence', publishedProbability: '~62%' }),
  Object.freeze({ seconds: 600, shape: 'two cadences', publishedProbability: '~85%' }),
  Object.freeze({ seconds: 1_250, shape: 'four cadences', publishedProbability: '~98%' }),
  Object.freeze({ seconds: 1_800, shape: '30 minutes', publishedProbability: '~99.7%' }),
] as const);

export type WindowConfidenceV1 =
  | 'unanswerable'
  | 'below-one-cadence'
  | 'below-guidance'
  | 'meets-guidance'
  | 'meets-robust-guidance';

export type WindowWidthAssessmentV1 = Readonly<{
  seconds: number;
  /** `1 - exp(-W / 313)`, in [0, 1]. Provisional, per §12.3. */
  publicationProbability: number;
  /** `W / 313`, the number of measured cadences the window spans. */
  cadences: number;
  confidence: WindowConfidenceV1;
  headline: string;
  /** Why this width is or is not enough, in the section's own terms. */
  detail: string;
}>;

function classify(seconds: number): WindowConfidenceV1 {
  if (seconds < PYTH_NOMINAL_CADENCE_SECONDS_V1 / 10) return 'unanswerable';
  if (seconds < PYTH_NOMINAL_CADENCE_SECONDS_V1) return 'below-one-cadence';
  if (seconds < TERMINAL_WINDOW_GUIDANCE_SECONDS_V1) return 'below-guidance';
  if (seconds < TERMINAL_WINDOW_ROBUST_SECONDS_V1) return 'meets-guidance';
  return 'meets-robust-guidance';
}

const DETAIL: Readonly<Record<WindowConfidenceV1, string>> = Object.freeze({
  'unanswerable': 'A window this narrow is answered only when a publication happens to land inside it. This is the shape §12.3 was written to retire: the market does not fail because the provider was silent, it fails because it asked a question nothing could answer.',
  'below-one-cadence': 'Narrower than one publication cadence. More often than not no publication is about this period at all, and the market reaches its failure outcome for a reason that is not about the world.',
  'below-guidance': 'Wide enough that a publication usually lands, but below §12.3’s operative guidance of four cadences. The residual failure rate is a property of the window, not of the price.',
  'meets-guidance': 'Meets §12.3’s operative guidance of at least four cadences (~21 minutes). The failure walk remains reachable, and at this width reaching it means the provider really was silent.',
  'meets-robust-guidance': 'At or beyond the 30-minute width §12.3 names for a market that should not fail for provider reasons.',
});

/**
 * Assess one chosen width against §12.3.
 *
 * Returns the consequence, never a substitute choice. A width of one second is
 * legal and this function says so while telling the operator what it costs.
 */
export function assessWindowWidthV1(seconds: number): WindowWidthAssessmentV1 {
  if (!Number.isSafeInteger(seconds) || seconds < 0) throw new Error('window width must be a whole number of seconds');
  const cadences = seconds / PYTH_SOL_USD_MEASURED_P50_SECONDS_V1;
  const publicationProbability = 1 - Math.exp(-cadences);
  const confidence = classify(seconds);
  return Object.freeze({
    seconds,
    publicationProbability,
    cadences,
    confidence,
    headline: `${(publicationProbability * 100).toFixed(1)}% chance at least one publication is about this window`,
    detail: DETAIL[confidence],
  });
}

/**
 * The separate budget §12.3 insists is not this one.
 *
 * `max_age_seconds` bounds `now - publication_time` at the moment a keeper
 * submits, and it also sets the deadline `end + max_age`. It covers *submission*
 * latency, not publication cadence, and a window wide enough to be published
 * into is useless if no keeper can land the transaction inside `max_age` of that
 * publication. The two are surfaced together so a wizard cannot let an operator
 * widen one while believing it helped the other.
 */
export type ResolutionDeadlineV1 = Readonly<{
  windowStart: number;
  windowEnd: number;
  maxAgeSeconds: number;
  /** `end + max_age`: the last second a publication may resolve this Market. */
  primaryDeadline: number;
  /**
   * §12.3: the walk's `primary_deadline` and the configuration's own
   * `max_observation_age_seconds` are the same grace by construction, so the
   * last second a resolution may land and the first second the failure walk may
   * act are adjacent, with no gap where neither route can.
   */
  failureWalkOpensAt: number;
}>;

export function resolutionDeadlineV1(windowStart: number, windowEnd: number, maxAgeSeconds: number): ResolutionDeadlineV1 {
  if (!Number.isSafeInteger(windowStart) || !Number.isSafeInteger(windowEnd) || windowEnd < windowStart) {
    throw new Error('terminal window requires start <= end');
  }
  if (!Number.isSafeInteger(maxAgeSeconds) || maxAgeSeconds < 0) throw new Error('max age must be a whole number of seconds');
  const primaryDeadline = windowEnd + maxAgeSeconds;
  return Object.freeze({
    windowStart,
    windowEnd,
    maxAgeSeconds,
    primaryDeadline,
    failureWalkOpensAt: primaryDeadline + 1,
  });
}

/**
 * The three named refusals §12.3's amendment split apart, for UI copy.
 *
 * They existed inside the contract and did not reach the wire:
 * `ProviderJoinErrorV3::Provider` flattened all three into
 * `ProviderObservation` (`0x800A`), so an operator reading a validator log
 * could not act on the distinction. Two of the three mean "come back later"
 * and the third does not, which is exactly what a resolution screen has to
 * tell them apart to say.
 */
export const PROVIDER_WINDOW_REFUSALS_V1 = Object.freeze([
  Object.freeze({ code: '0x8011', name: 'ProviderWindow', carries: 'InvalidObservationSchedule', question: 'Is it about the period the Market sold?', retriable: true }),
  Object.freeze({ code: '0x8012', name: 'ProviderFreshness', carries: 'InvalidPublicationTime', question: 'Will this cluster still act on it?', retriable: true }),
  Object.freeze({ code: '0x8013', name: 'ProviderConfiguration', carries: 'InvalidPythObservation', question: 'Is it the feed, exponent and confidence this Market’s adapter configuration names?', retriable: false }),
] as const);
