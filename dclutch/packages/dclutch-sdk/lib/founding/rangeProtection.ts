/**
 * Range protection on a Pyth source — the wizard's first-class product.
 *
 * A range-protection Product is a categorical partition of one coordinate
 * domain by an ordered list of cuts, plus an explicit failure outcome, plus a
 * portfolio of per-outcome coefficients saying what each outcome pays. For the
 * protection shape the payoff is one unit of the liability basis in *either*
 * tail and nothing inside the band — which is what a holder buys when they want
 * to be made whole if a price leaves a range they can live with.
 *
 * The composition below is the same one
 * `tools/local-validator/bootstrap/successor/src/market.rs::demo_market_input`
 * builds for the SOL/USD demo Market: cuts at 12,000 and 18,000 over a
 * denominator of 100, coefficients `[1, 0, 1, 0]`. That is not a coincidence
 * and it is not copied prose — it is the arithmetic below run at those inputs,
 * and `rangeProtection.test.ts` pins the agreement against the campaign's own
 * recorded market input.
 *
 * THE INVARIANTS ARE THE PRODUCT. `decodeCoreFoundProductGraphV2` re-derives
 * every one of these from the finalized records at Found time: cuts strictly
 * increasing, regions exactly `cuts + 1`, outcomes exactly `regions + 1`,
 * portfolio gcd-normalized and not identically zero. Composing them here means
 * a wizard cannot walk an operator to a Found preflight that then refuses.
 */

const MAX_U64 = 0xffff_ffff_ffff_ffffn;

export type RangeProtectionInputV1 = Readonly<{
  /** What the coordinate is, for labels only. Never protocol truth. */
  coordinateLabel: string;
  /** Ticks per whole unit of the coordinate, the Product's `cut_denominator`. */
  cutDenominator: bigint;
  /** The band's lower edge, in ticks. Below it, protection pays. */
  lowerEdgeTicks: bigint;
  /** The band's upper edge, in ticks. At or above it, protection pays. */
  upperEdgeTicks: bigint;
}>;

export type RangeProtectionOutcomeV1 = Readonly<{
  index: number;
  /** Display label. Preview metadata, never decoded from a Market account. */
  label: string;
  /** This outcome's portfolio coefficient: what one claim pays if it wins. */
  coefficient: bigint;
  kind: 'below-band' | 'inside-band' | 'above-band' | 'resolution-failure';
}>;

export type RangeProtectionProductV1 = Readonly<{
  cutDenominator: bigint;
  /** Ordered, strictly increasing cuts, as the result domain encodes them. */
  cuts: ReadonlyArray<bigint>;
  portfolioDenominator: bigint;
  coefficients: ReadonlyArray<bigint>;
  outcomes: ReadonlyArray<RangeProtectionOutcomeV1>;
  regions: number;
  outcomeCount: number;
  /** The explicit failure outcome, always the last index. */
  failureOutcomeIndex: number;
}>;

function gcd(left: bigint, right: bigint): bigint {
  let a = left < 0n ? -left : left;
  let b = right < 0n ? -right : right;
  while (b !== 0n) [a, b] = [b, a % b];
  return a;
}

/** Format ticks as a decimal, exactly, without touching floating point. */
export function formatTicksV1(ticks: bigint, denominator: bigint): string {
  if (denominator <= 0n) throw new Error('cut denominator must be positive');
  const negative = ticks < 0n;
  const magnitude = negative ? -ticks : ticks;
  const whole = magnitude / denominator;
  const remainder = magnitude % denominator;
  if (remainder === 0n) return `${negative ? '-' : ''}${whole}`;
  const width = denominator.toString().length - 1;
  const fraction = remainder.toString().padStart(width, '0').replace(/0+$/, '');
  return `${negative ? '-' : ''}${whole}.${fraction === '' ? '0' : fraction}`;
}

/**
 * Compose the categorical Product a range-protection market sells.
 *
 * Two cuts give three ordinary regions — below the band, inside it, at or above
 * it — and the failure outcome follows them, so the width is four. The
 * portfolio is `[1, 0, 1, 0]` before normalization and stays that way after,
 * since its gcd is already one.
 */
export function composeRangeProtectionV1(input: RangeProtectionInputV1): RangeProtectionProductV1 {
  const { cutDenominator, lowerEdgeTicks, upperEdgeTicks } = input;
  if (typeof cutDenominator !== 'bigint' || cutDenominator <= 0n || cutDenominator > MAX_U64) {
    throw new Error('cut denominator must be a positive u64');
  }
  if (typeof lowerEdgeTicks !== 'bigint' || typeof upperEdgeTicks !== 'bigint') {
    throw new Error('band edges must be whole numbers of ticks');
  }
  if (lowerEdgeTicks >= upperEdgeTicks) {
    // The result domain requires strictly increasing cuts, so a band whose
    // edges meet or cross is not a narrow band; it is not a partition at all.
    throw new Error('the band’s lower edge must be strictly below its upper edge');
  }

  const cuts = Object.freeze([lowerEdgeTicks, upperEdgeTicks]);
  const coefficients = [1n, 0n, 1n, 0n];
  const divisor = coefficients.reduce((total, coefficient) => gcd(total, coefficient), 1n);
  if (divisor !== 1n || coefficients.every((coefficient) => coefficient === 0n)) {
    throw new Error('the range-protection portfolio is empty or not gcd-normalized');
  }

  const low = formatTicksV1(lowerEdgeTicks, cutDenominator);
  const high = formatTicksV1(upperEdgeTicks, cutDenominator);
  const label = input.coordinateLabel;
  const outcomes: ReadonlyArray<RangeProtectionOutcomeV1> = Object.freeze([
    Object.freeze({ index: 0, label: `${label} < ${low}`, coefficient: coefficients[0], kind: 'below-band' as const }),
    Object.freeze({ index: 1, label: `${low} ≤ ${label} < ${high}`, coefficient: coefficients[1], kind: 'inside-band' as const }),
    Object.freeze({ index: 2, label: `${label} ≥ ${high}`, coefficient: coefficients[2], kind: 'above-band' as const }),
    Object.freeze({ index: 3, label: 'Resolution failure', coefficient: coefficients[3], kind: 'resolution-failure' as const }),
  ]);

  const regions = cuts.length + 1;
  const outcomeCount = regions + 1;
  if (outcomes.length !== outcomeCount || coefficients.length !== outcomeCount) {
    throw new Error('the composed outcome width does not equal regions + 1');
  }
  return Object.freeze({
    cutDenominator,
    cuts,
    portfolioDenominator: 1n,
    coefficients: Object.freeze(coefficients),
    outcomes,
    regions,
    outcomeCount,
    failureOutcomeIndex: outcomeCount - 1,
  });
}

/**
 * What a founding principal buys, at the composed payoff.
 *
 * Founding mints one complete set per unit of principal, so the founder holds
 * `principal` claims of every outcome. The exact backing a Market must hold
 * while unresolved is `max(supply)` across outcomes; after resolution it is the
 * winning supply alone. Both are stated in raw atoms, because a formatted
 * amount is only valid after decoding the Mint's own display precision.
 */
export function rangeProtectionBackingV1(product: RangeProtectionProductV1, principalAtoms: bigint): Readonly<{
  completeSets: bigint;
  perOutcomeSupplyAtoms: ReadonlyArray<bigint>;
  requiredBackingWhileUnresolvedAtoms: bigint;
  payingOutcomes: ReadonlyArray<number>;
}> {
  if (typeof principalAtoms !== 'bigint' || principalAtoms <= 0n || principalAtoms > MAX_U64) {
    throw new Error('founding principal must be a nonzero u64 of raw collateral atoms');
  }
  const supply = product.outcomes.map(() => principalAtoms);
  return Object.freeze({
    completeSets: principalAtoms,
    perOutcomeSupplyAtoms: Object.freeze(supply),
    requiredBackingWhileUnresolvedAtoms: supply.reduce((most, entry) => (entry > most ? entry : most), 0n),
    payingOutcomes: Object.freeze(product.outcomes.filter((outcome) => outcome.coefficient > 0n).map((outcome) => outcome.index)),
  });
}

/**
 * Where the coordinate actually is, which is the property shape never had.
 *
 * THE DEFECT THIS CLOSES is ember's standing complaint — "SOL/USD always
 * resolves into the same bucket" — and the composer above passed it with full
 * marks. Cuts strictly increasing, regions exactly `cuts + 1`, portfolio
 * gcd-normalized and not identically zero: every invariant this file was
 * written to keep, kept, on a market that resolves into its top cell one
 * hundred percent of the time. Exhaustive, disjoint, ordered and canonical are
 * properties of a partition's SHAPE. None of them says the question is one
 * anybody has to think about.
 *
 * The shipped wizard sent `cutDenominator=100` with edges `12000/18000` — a
 * band at 120.00 to 180.00 — for a coordinate whose Source returns raw
 * provider price atoms unrescaled (`crates/dclutch-source-contract/src/lib.rs`
 * line 612). A Pyth SOL/USD observation is on the order of 10^10 there, so the
 * whole band sits three orders of magnitude below anything the market can ever
 * observe. `crates/dclutch-product-compiler/src/partition_quality.rs` now
 * refuses exactly that as `DegenerateOutcomePartition`, measured at
 * `0 / 0 / 0 / 0 / 10000` basis points.
 *
 * WHAT THIS IS NOT. It is not that gate, and it must not be read as one. The
 * compiler's measure is a triangular ex-ante mass model over a characteristic
 * displacement of `volatility_bps × sqrt(window / 10,000 slots)`, and
 * reimplementing it here in TypeScript would be the same defect this lane
 * spent its day convicting elsewhere: a browser restating semantics a Rust
 * owner already holds. This is a strictly weaker, exactly decidable necessary
 * condition — a UNIT-SANITY check — that catches the convicted case and admits
 * the lopsided ones the compiler's own tests admit.
 */
export type RangeProtectionPlacementV1 = Readonly<{
  /** The outcome a coordinate that never moved from here would win. */
  outcomeIndex: number;
  kind: RangeProtectionOutcomeV1['kind'];
  /** Whether the band is unreachably far from the observation. */
  certain: boolean;
  /** The compiler refusal this partition would meet, or null. */
  refusal: 'DegenerateOutcomePartition' | null;
}>;

/**
 * How many band widths away from the band an observation may sit before this
 * market is called unreachable rather than lopsided.
 *
 * PROVISIONAL. It is not derived from anything: it is a factor chosen to sit
 * far above "off-centre by a displacement or so", which the compiler's gate
 * deliberately admits, and far below the three orders of magnitude the
 * convicted defect was out by. Lifting plan: delete it, and call
 * `require_interesting_partition_v1` with the market's own founding band once
 * `dclutch-product-compiler` reaches the browser. Until then this catches the
 * unit error and makes no claim about outcome mass.
 */
const UNREACHABLE_BAND_WIDTHS_V1 = 32n;

export const RANGE_PLACEMENT_PROVISIONAL_NOTE_V1 =
  'This is a provisional unit-sanity bound, not an outcome-mass measure: it '
  + 'refuses a band the observation cannot plausibly reach and admits every '
  + 'merely lopsided one. The exact measure is '
  + 'require_interesting_partition_v1 in dclutch-product-compiler, which this '
  + 'browser cannot yet call.';

/** Place one founding observation, in the product's own ticks, in its partition. */
export function rangeProtectionPlacementV1(
  product: RangeProtectionProductV1,
  observationTicks: bigint,
): RangeProtectionPlacementV1 {
  if (typeof observationTicks !== 'bigint') throw new Error('the founding observation must be a whole number of ticks');
  const [low, high] = [product.cuts[0], product.cuts[1]];
  const outcomeIndex = observationTicks < low ? 0 : observationTicks < high ? 1 : 2;
  const width = high - low;
  const distance = observationTicks < low ? low - observationTicks
    : observationTicks >= high ? observationTicks - high
    : 0n;
  const certain = distance > width * UNREACHABLE_BAND_WIDTHS_V1;
  return Object.freeze({
    outcomeIndex,
    kind: product.outcomes[outcomeIndex].kind,
    certain,
    refusal: certain ? ('DegenerateOutcomePartition' as const) : null,
  });
}
