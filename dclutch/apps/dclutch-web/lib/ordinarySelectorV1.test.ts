import { describe, expect, it } from 'vitest';

import { ordinarySelectorJoinV1, selectOrdinaryV1 } from './ordinarySelectorV1';

/**
 * The program's own vectors, ported rather than invented.
 *
 * `crates/dclutch-product-runtime-v2/tests/{partition,runtime_width}.rs` prove
 * `select_ordinary` two ways: five literal points at the extremes of `i128` and
 * `u64`, and three exhaustive sweeps against an INDEPENDENTLY WRITTEN interval
 * predicate — one written from the convention sentence rather than from the
 * function, so agreement is evidence and not a function compared with itself.
 * Both are reproduced here against the TypeScript mirror, with the same
 * expected counts, so a mirror that drifts from the chain fails on the chain's
 * own numbers.
 */

/**
 * How many regions declare they own this coordinate, and the lowest such.
 *
 * A direct port of `partition.rs`'s `declared_owners`, written from the
 * sentence: region `0` is `x < c[0]/d`, interior region `i` is
 * `c[i-1]/d <= x < c[i]/d`, region `R-1` is `x >= c[R-2]/d`. It does not call
 * `selectOrdinaryV1`. `boundaryBelow` flips to the opposite half-open
 * convention and exists only as this file's control.
 */
function declaredOwners(
  cuts: ReadonlyArray<bigint>,
  cutDenominator: bigint,
  numerator: bigint,
  denominator: bigint,
  boundaryBelow: boolean,
): Readonly<{ owners: number; first: number }> {
  const atLeast = (cut: bigint): boolean => {
    const left = numerator * cutDenominator;
    const right = cut * denominator;
    return boundaryBelow ? left > right : left >= right;
  };
  const regionCount = cuts.length + 1;
  let owners = 0;
  let first = 0;
  for (let region = 0; region < regionCount; region += 1) {
    const lowerOk = region === 0 || atLeast(cuts[region - 1]!);
    const upperOk = region === regionCount - 1 || !atLeast(cuts[region]!);
    if (lowerOk && upperOk) {
      if (owners === 0) first = region;
      owners += 1;
    }
  }
  return { owners, first };
}

/**
 * The Rust's quotient-then-remainder comparison, kept as a second opinion.
 *
 * `compare_unsigned_rational` divides first and cross-multiplies only the
 * remainders, because an `i128 * u64` product overflows. The shipped mirror
 * cross-multiplies outright, which a `bigint` may do safely; this function is
 * the form the chain actually executes, and the sweeps below run both so the
 * simplification is measured over every fixture rather than asserted once.
 */
function compareQuotientFirstV1(left: bigint, leftDenominator: bigint, right: bigint, rightDenominator: bigint): number {
  const negative = (value: bigint): boolean => value < 0n;
  if (negative(left) && !negative(right)) return -1;
  if (!negative(left) && negative(right)) return 1;
  const absLeft = left < 0n ? -left : left;
  const absRight = right < 0n ? -right : right;
  const leftQuotient = absLeft / leftDenominator;
  const rightQuotient = absRight / rightDenominator;
  let order: number;
  if (leftQuotient < rightQuotient) order = -1;
  else if (leftQuotient > rightQuotient) order = 1;
  else {
    const leftRemainder = (absLeft % leftDenominator) * rightDenominator;
    const rightRemainder = (absRight % rightDenominator) * leftDenominator;
    order = leftRemainder < rightRemainder ? -1 : leftRemainder > rightRemainder ? 1 : 0;
  }
  return negative(left) && negative(right) ? -order : order;
}

function selectQuotientFirstV1(numerator: bigint, denominator: bigint, cuts: ReadonlyArray<bigint>, cutDenominator: bigint): number {
  let selector = 0;
  for (const cut of cuts) {
    if (compareQuotientFirstV1(numerator, denominator, cut, cutDenominator) < 0) return selector;
    selector += 1;
  }
  return selector;
}

function sweep(
  cutDenominator: bigint,
  cuts: ReadonlyArray<bigint>,
  from: bigint,
  to: bigint,
  denominators: ReadonlyArray<bigint>,
): Readonly<{ swept: number; reached: number; boundaryDisagreements: number }> {
  const regionCount = cuts.length + 1;
  const reached = new Array<boolean>(regionCount).fill(false);
  let boundaryDisagreements = 0;
  let swept = 0;
  for (const denominator of denominators) {
    for (let numerator = from; numerator <= to; numerator += 1n) {
      const honest = declaredOwners(cuts, cutDenominator, numerator, denominator, false);
      expect(honest.owners, `${numerator}/${denominator} is owned by ${honest.owners} declared regions, not exactly one`).toBe(1);
      const selected = selectOrdinaryV1(numerator, denominator, cuts, cutDenominator, regionCount, 0);
      expect(selected, `selectOrdinaryV1 disagreed with the declared interval at ${numerator}/${denominator}`).toBe(honest.first);
      expect(selectQuotientFirstV1(numerator, denominator, cuts, cutDenominator),
        `the quotient-first comparison disagreed at ${numerator}/${denominator}`).toBe(selected);
      expect(selected).toBeLessThan(regionCount);
      reached[selected] = true;
      const flipped = declaredOwners(cuts, cutDenominator, numerator, denominator, true);
      if (flipped.owners !== honest.owners || flipped.first !== honest.first) boundaryDisagreements += 1;
      swept += 1;
    }
  }
  return { swept, reached: reached.filter((hit) => hit).length, boundaryDisagreements };
}

const RANGE_CUTS_V1 = Object.freeze([-10n, 0n, 25n]);
const WIDE_CUTS_V1 = Object.freeze(Array.from({ length: 300 }, (unused, index) => BigInt(index - 150)));

describe('the ordinary selector, mirrored from the Resolution program', () => {
  it('reproduces the five literal points runtime_width.rs probes', () => {
    // crates/dclutch-product-runtime-v2/tests/runtime_width.rs:37-41. The last
    // two are the extremes the Rust needs a quotient-first comparison to
    // survive at all.
    const cuts = WIDE_CUTS_V1;
    const select = (numerator: bigint, denominator: bigint) => selectOrdinaryV1(numerator, denominator, cuts, 3n, cuts.length + 1, 0);
    expect(cuts).toHaveLength(300);
    expect(select(-151n, 3n)).toBe(0);
    expect(select(-150n, 3n)).toBe(1);
    expect(select(149n, 3n)).toBe(300);
    expect(select(-(2n ** 127n), 2n ** 64n - 1n)).toBe(0);
    expect(select(2n ** 127n - 1n, 1n)).toBe(300);
  });

  it('sweeps the narrow domain exhaustively, disjointly, and boundary-sensitively', () => {
    // partition.rs:139. Cuts -10, 0 and 25 over ten: coordinates -1, 0, 2.5.
    const result = sweep(10n, RANGE_CUTS_V1, -400n, 400n, [1n, 3n, 7n, 10n]);
    expect(result.swept).toBe(3_204);
    expect(result.reached).toBe(4);
    expect(result.boundaryDisagreements).toBe(9);
  });

  it('sweeps the 300-cut domain and reaches every one of its 301 regions', () => {
    // partition.rs:150.
    const result = sweep(3n, WIDE_CUTS_V1, -460n, 460n, [1n, 3n]);
    expect(result.swept).toBe(1_842);
    expect(result.reached).toBe(301);
    expect(result.boundaryDisagreements).toBe(400);
  });

  it('gives a cutless domain the whole line, and never the failure cell', () => {
    // partition.rs:163.
    const result = sweep(1n, [], -50n, 50n, [1n, 2n, 3n]);
    expect(result.swept).toBe(303);
    expect(result.reached).toBe(1);
    expect(result.boundaryDisagreements).toBe(0);
  });

  it('refuses a zero denominator and a partition too narrow for its cuts', () => {
    expect(() => selectOrdinaryV1(1n, 0n, RANGE_CUTS_V1, 10n, 4, 0)).toThrow('observation denominator is zero');
    expect(() => selectOrdinaryV1(1n, 1n, RANGE_CUTS_V1, 0n, 4, 0)).toThrow('partition cut denominator is zero');
    expect(() => selectOrdinaryV1(400n, 1n, RANGE_CUTS_V1, 10n, 3, 0)).toThrow('fewer ordinary cells than it has cuts');
    expect(() => selectOrdinaryV1(1n, 1n, RANGE_CUTS_V1, 10n, 4, 19)).toThrow('outside the admitted range');
    expect(() => selectOrdinaryV1(1n, 1n, RANGE_CUTS_V1, 10n, 4, -19)).toThrow('outside the admitted range');
  });
});

describe('the certificate-to-partition join', () => {
  /** Cohort-14b, market B, exactly as the chain recorded it on 2026-09-03. */
  const COHORT14B_V1 = Object.freeze({
    partition: Object.freeze({ cuts: Object.freeze([9_900n, 10_300n]), cutDenominator: 100n, regionCount: 3 }),
    observation: Object.freeze({ numerator: 10_062_091_764n, denominator: 1n }),
    committed: 2,
  });

  it('derives the cell cohort-14b actually committed', () => {
    // The number this join was said to be undecidable about. Both cuts compare
    // Less than the observation on the chain's own arithmetic, so the loop
    // falls off the end at selector 2 -- which is byte for byte the selector
    // the live certificate carries and the `terminal_winner` Core wrote.
    const join = ordinarySelectorJoinV1(COHORT14B_V1.partition, COHORT14B_V1.observation, COHORT14B_V1.committed, 0);
    expect(join.derived).toBe(2);
    expect(join.agrees).toBe(true);
    expect(join.refusal).toBeNull();
  });

  it('reports a disagreement rather than throwing or printing the derived cell', () => {
    const join = ordinarySelectorJoinV1(COHORT14B_V1.partition, COHORT14B_V1.observation, 0, 0);
    expect(join.derived).toBe(2);
    expect(join.committed).toBe(0);
    expect(join.agrees).toBe(false);
    expect(join.refusal).toBeNull();
  });

  it('refuses the failure cell and a certificate with no observation', () => {
    const failure = ordinarySelectorJoinV1(COHORT14B_V1.partition, COHORT14B_V1.observation, 3, 0);
    expect(failure.derived).toBeNull();
    expect(failure.agrees).toBe(false);
    expect(failure.refusal).toContain('source-failure outcome');
    const absent = ordinarySelectorJoinV1(COHORT14B_V1.partition, null, 1, 0);
    expect(absent.derived).toBeNull();
    expect(absent.refusal).toContain('carries no observation');
  });

  it('names the cell cohort-14b’s price was actually in, once the factor is declared', () => {
    // The two readings side by side. The first is what the protocol DID: no
    // record carried a factor, the declared scale was therefore the identity,
    // and cell 2 is the honest output of the arithmetic the program was handed.
    // The second is what the market MEANT: the cuts are dollars authored in
    // cents, the observation is raw Pyth atoms at exponent -8, and $100.62 is
    // inside the $99-$103 band, which is cell 1 and pays zero.
    //
    // Nothing here reinterprets the chain. Selector 2 is not a mistake in
    // arithmetic; it is a mistake in units, made at founding, in a record that
    // had nowhere to put the number. The same call with -8 is the reading the
    // market's own StatisticSpec would have produced had it carried one.
    const asPaid = ordinarySelectorJoinV1(
      COHORT14B_V1.partition,
      COHORT14B_V1.observation,
      COHORT14B_V1.committed,
      0,
    );
    expect(asPaid.derived).toBe(2);
    expect(asPaid.agrees).toBe(true);

    const onTheCutsScale = ordinarySelectorJoinV1(
      COHORT14B_V1.partition,
      COHORT14B_V1.observation,
      COHORT14B_V1.committed,
      -8,
    );
    expect(onTheCutsScale.derived).toBe(1);
    expect(onTheCutsScale.agrees).toBe(false);

    // And the declared factor reaches the same cell as pre-dividing the
    // observation by hand, which is how this file used to state it. Two
    // arithmetic routes, one answer.
    expect(
      selectOrdinaryV1(
        COHORT14B_V1.observation.numerator,
        100_000_000n,
        COHORT14B_V1.partition.cuts,
        COHORT14B_V1.partition.cutDenominator,
        COHORT14B_V1.partition.regionCount,
        0,
      ),
    ).toBe(1);
  });
});
