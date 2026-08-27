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
