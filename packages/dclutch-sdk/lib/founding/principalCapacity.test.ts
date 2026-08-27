import { describe, expect, it } from 'vitest';

import {
  BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1,
  CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1,
  CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1,
  PRINCIPAL_ADMISSION_CASES_V1,
} from '../generated/principalCapacityV1';
import {
  DEFAULT_CHAIN_STATE_CAPACITY_V1,
  admitPrincipalCapacityV1,
  formatCapacityV1,
  largestAdmittedPrincipalV1,
  type PrincipalCapacityV1,
} from './principalCapacity';

function bounded(numerator: bigint, denominator: bigint): PrincipalCapacityV1 {
  return { kind: 'bounded', numerator, denominator };
}

describe('the kappa founding predicate against the Lean-emitted corpus', () => {
  it('agrees with every emitted admission case', () => {
    expect(PRINCIPAL_ADMISSION_CASES_V1.length).toBe(17);
    for (const entry of PRINCIPAL_ADMISSION_CASES_V1) {
      const verdict = admitPrincipalCapacityV1(
        bounded(BigInt(entry.numerator), BigInt(entry.denominator)),
        entry.floorAtoms,
        entry.principalAtoms,
      );
      expect(
        verdict.admitted,
        `kappa ${entry.numerator}/${entry.denominator}, floor ${entry.floorAtoms}, principal ${entry.principalAtoms}`,
      ).toBe(entry.admitted);
    }
  });

  it('holds the default boundary exactly, at the atom', () => {
    // kappa = 1/4 against the 18,618,074,000-lamport graduation floor.
    const largest = largestAdmittedPrincipalV1(DEFAULT_CHAIN_STATE_CAPACITY_V1, BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1);
    expect(largest).toBe(4_654_518_500n);
    expect(admitPrincipalCapacityV1(DEFAULT_CHAIN_STATE_CAPACITY_V1, BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1, largest!).admitted).toBe(true);
    expect(admitPrincipalCapacityV1(DEFAULT_CHAIN_STATE_CAPACITY_V1, BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1, largest! + 1n).admitted).toBe(false);
  });

  it('takes kappa and the floor from the generated module, not from a literal here', () => {
    expect(DEFAULT_CHAIN_STATE_CAPACITY_V1).toEqual({
      kind: 'bounded',
      numerator: BigInt(CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1),
      denominator: BigInt(CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1),
    });
    expect(formatCapacityV1(DEFAULT_CHAIN_STATE_CAPACITY_V1)).toBe('1/4');
  });
});

describe('the kappa predicate names why it refused', () => {
  it('separates an unstated capacity from an exceeded one', () => {
    expect(admitPrincipalCapacityV1({ kind: 'unstated' }, 1000n, 1n).refusal).toBe('PrincipalCapacityUnstated');
    expect(admitPrincipalCapacityV1(bounded(1n, 4n), 1000n, 251n).refusal).toBe('PrincipalExceedsCapacity');
  });

  it('separates a zero denominator from a zero principal and a zero bound', () => {
    expect(admitPrincipalCapacityV1(bounded(1n, 0n), 1000n, 1n).refusal).toBe('NonCanonicalCapacity');
    expect(admitPrincipalCapacityV1(bounded(1n, 4n), 1000n, 0n).refusal).toBe('ZeroCapacity');
    expect(admitPrincipalCapacityV1(bounded(0n, 1n), 1000n, 1n).refusal).toBe('PrincipalExceedsCapacity');
    expect(admitPrincipalCapacityV1(bounded(1n, 4n), 0n, 1n).refusal).toBe('PrincipalExceedsCapacity');
  });

  it('refuses rather than errors when the left-hand side leaves u128', () => {
    // `overflow_is_exact`: the right-hand side is u32 x u64 and stays below
    // 2^96, so a left-hand side above u128 is genuinely larger. The refusal is
    // exact, not conservative, and it must not surface as a thrown error.
    const verdict = admitPrincipalCapacityV1(bounded(1n, 0xffff_ffffn), 0xffff_ffff_ffff_ffffn, (1n << 128n) - 1n);
    expect(verdict.admitted).toBe(false);
    expect(verdict.refusal).toBe('PrincipalExceedsCapacity');
    expect(verdict.scaled).toBeNull();
  });

  it('is monotone in the principal, as the model proves', () => {
    const floor = 1_000_000n;
    const largest = largestAdmittedPrincipalV1(bounded(3n, 7n), floor)!;
    for (const principal of [1n, largest / 2n, largest]) {
      expect(admitPrincipalCapacityV1(bounded(3n, 7n), floor, principal).admitted).toBe(true);
    }
    for (const principal of [largest + 1n, largest * 2n, floor * 4n]) {
      expect(admitPrincipalCapacityV1(bounded(3n, 7n), floor, principal).admitted).toBe(false);
    }
  });

  it('never divides to decide, only to display', () => {
    // A kappa whose bound is not divisible by the denominator would admit one
    // atom too many if the predicate rounded up rather than cross-multiplying.
    const verdict = admitPrincipalCapacityV1(bounded(1n, 3n), 10n, 4n);
    expect(verdict.admitted).toBe(false);
    expect(verdict.bound).toBe(10n);
    expect(verdict.scaled).toBe(12n);
    expect(verdict.largestAdmittedPrincipal).toBe(3n);
  });
});

describe('the kappa verdict states its own enforcement', () => {
  it('always reports that no on-chain route applies it', () => {
    // This is the field a UI must render beside the verdict. If kappa is ever
    // wired into a founding route, this test is the thing that has to change,
    // and its failure is the reminder to change the copy with it.
    for (const principal of [1n, 4_654_518_500n, 4_654_518_501n]) {
      expect(admitPrincipalCapacityV1(DEFAULT_CHAIN_STATE_CAPACITY_V1, BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1, principal).enforcement).toBe('off-chain-only');
    }
    expect(admitPrincipalCapacityV1({ kind: 'unstated' }, 1n, 1n).enforcement).toBe('off-chain-only');
  });
});

describe('the kappa predicate refuses inputs outside their declared widths', () => {
  it('refuses a floor above u64, a principal above u128, and a kappa term above u32', () => {
    expect(() => admitPrincipalCapacityV1(bounded(1n, 4n), 1n << 64n, 1n)).toThrow(/outside u64/);
    expect(() => admitPrincipalCapacityV1(bounded(1n, 4n), 1n, 1n << 128n)).toThrow(/outside u128/);
    expect(() => admitPrincipalCapacityV1(bounded(1n << 32n, 4n), 1n, 1n)).toThrow(/numerator is outside u32/);
    expect(() => admitPrincipalCapacityV1(bounded(1n, 1n << 32n), 1n, 1n)).toThrow(/denominator is outside u32/);
  });
});
