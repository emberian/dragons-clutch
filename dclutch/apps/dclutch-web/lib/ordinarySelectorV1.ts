/**
 * The partition arithmetic the Resolution program performs, mirrored.
 *
 * WHAT WAS UNDECIDABLE, AND WHY IT NO LONGER IS. A resolution certificate
 * carries the observation as an exact integer ratio and the committed cell as a
 * `selector`, and the market's `ResultDomainV2` carries the cuts on a
 * denominator of its own. Cohort-14b settled with `10062091764/1` against cuts
 * `9900, 10300` over `100`, and the browser refused to name the cell because
 * those two are not on one scale and no exponent is published anywhere a reader
 * can reach.
 *
 * The refusal was right about the scales and wrong about the join. THE CHAIN
 * APPLIES NO EXPONENT EITHER. `ResultDomainV2::select_ordinary`
 * (crates/dclutch-product-runtime-v2/src/lib.rs:222) compares the observation
 * ratio against each cut ratio directly, and every producer of a certificate
 * pins `result_denominator` to the literal `1`
 * (crates/dclutch-resolution-core-v3-operator/src/provider_finalized_projection_v3.rs:632
 * refuses anything else). So the selector is a function of exactly the numbers
 * the browser already holds, and a reader can CHECK the number the chain
 * committed instead of guessing at it.
 *
 * WHAT THIS DOES NOT SETTLE, and must not be read as settling: that the cell is
 * the right one *economically*. Cohort-14b's cuts were authored in cents and
 * its observation arrived as raw Pyth atoms at exponent -8, and the two units
 * are declared on chain as two opaque identities with no factor between them.
 * Reproducing the chain's arithmetic proves what the protocol DID. See
 * `docs/design/OBSERVATION_SCALE_AUTHORITY.md` for the record that should
 * carry that factor and does not.
 *
 * THE ORDERING, from the program's own sentence
 * (crates/dclutch-product-runtime-v2/tests/partition.rs:45): region `0` is
 * `x < c[0]/d`, interior region `i` is `c[i-1]/d <= x < c[i]/d`, and region
 * `R-1` is `x >= c[R-2]/d`. Ascending, left-closed, right-open. There is no
 * ordering tag anywhere; the ascending convention is enforced by the decoder
 * refusing cuts that do not strictly increase, and this is the only reading
 * consistent with it.
 */

/** How the observed ratio compares with one cut ratio. Exact, never floating. */
function compareRatioV1(numerator: bigint, denominator: bigint, cut: bigint, cutDenominator: bigint): number {
  // The Rust compares quotients first and cross-multiplies only the
  // remainders, because `i128 * u64` overflows and its kernel must stay total.
  // A `bigint` cannot overflow, so the plain cross-multiplication is the same
  // order relation with nothing to guard against — and
  // `ordinarySelectorV1.test.ts` sweeps the quotient/remainder form beside this
  // one over every fixture the program tests, so the equivalence is measured
  // rather than argued.
  const left = numerator * cutDenominator;
  const right = cut * denominator;
  return left < right ? -1 : left > right ? 1 : 0;
}

/**
 * Map an exact signed-rational observation to one ordinary selector.
 *
 * Mirrors `ResultDomainV2::select_ordinary` including both of its refusals: a
 * zero denominator, and a region count the cut list cannot fill. It never
 * returns the failure selector — that outcome is reachable only through the
 * program's explicit failure commit, never from an observation.
 */
export function selectOrdinaryV1(
  numerator: bigint,
  denominator: bigint,
  cuts: ReadonlyArray<bigint>,
  cutDenominator: bigint,
  regionCount: number,
): number {
  if (denominator <= 0n) throw new Error('observation denominator is zero');
  if (cutDenominator <= 0n) throw new Error('partition cut denominator is zero');
  let selector = 0;
  for (const cut of cuts) {
    if (compareRatioV1(numerator, denominator, cut, cutDenominator) < 0) return selector;
    selector += 1;
  }
  if (selector >= regionCount) throw new Error('partition declares fewer ordinary cells than it has cuts');
  return selector;
}

/** The partition a join needs, exactly as `marketQuestion.ts` already reports it. */
export type OrdinaryPartitionV1 = Readonly<{
  cuts: ReadonlyArray<bigint>;
  cutDenominator: bigint;
  regionCount: number;
}>;

/** The observation a join needs, exactly as `marketResolution.ts` reports it. */
export type OrdinaryObservationV1 = Readonly<{
  numerator: bigint;
  denominator: bigint;
}>;

export type OrdinarySelectorJoinV1 = Readonly<{
  /** The cell this market's own partition puts the observation in, or null. */
  derived: number | null;
  /** The cell the chain committed, as the certificate carries it. */
  committed: number;
  /** True only when a cell was derived and it is the one the chain committed. */
  agrees: boolean;
  /** Why no derivation was possible, or null when one was. */
  refusal: string | null;
}>;

/**
 * Join a certificate's committed selector to the market's own partition.
 *
 * The failure cell is excluded on purpose: it is `regionCount`, no observation
 * can select it, and `bindTerminalResolutionCertificateV2` already proves that
 * identity from the certificate's kind. This function speaks only about
 * ORDINARY cells — the ones nothing had joined.
 *
 * A disagreement is reported, never thrown. If the chain committed a cell this
 * arithmetic does not reach, the honest surface is a reader told that the two
 * disagree, not a page that crashes or one that quietly prints the derived
 * number as though the chain had said it.
 */
export function ordinarySelectorJoinV1(
  partition: OrdinaryPartitionV1,
  observation: OrdinaryObservationV1 | null,
  committed: number,
): OrdinarySelectorJoinV1 {
  const refuse = (refusal: string): OrdinarySelectorJoinV1 =>
    Object.freeze({ derived: null, committed, agrees: false, refusal });
  if (observation === null) return refuse('the certificate carries no observation, so no cell can be derived from one');
  if (committed >= partition.regionCount) return refuse('the committed cell is the explicit source-failure outcome, which no observation selects');
  let derived: number;
  try {
    derived = selectOrdinaryV1(observation.numerator, observation.denominator, partition.cuts, partition.cutDenominator, partition.regionCount);
  } catch (error) {
    return refuse(error instanceof Error ? error.message : 'the partition could not be evaluated');
  }
  return Object.freeze({ derived, committed, agrees: derived === committed, refusal: null });
}
