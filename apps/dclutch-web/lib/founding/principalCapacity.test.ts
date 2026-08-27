import { describe, expect, it } from 'vitest';

import vectors from '../../fixtures/founding/generic-founding-vectors.json';

import {
  BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1,
  CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1,
  CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1,
  PRINCIPAL_ADMISSION_CASES_V1,
} from '../generated/principalCapacityV1';
import {
  DEFAULT_CHAIN_STATE_CAPACITY_V1,
  admitFoundingPrincipalV1,
  admitPrincipalCapacityV1,
  decodeManipulationFloorV1,
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

describe('recognizing a real ManipulationFloorV1 record', () => {
  const floors = (vectors as Readonly<{ manipulationFloors: ReadonlyArray<Readonly<{
    name: string; bytes: string; floorAtoms: string; basis: string; largestAdmitted: string; admitsBound: boolean; admitsBoundPlusOne: boolean;
  }>> }>).manipulationFloors;

  const bytesOf = (value: string) => Uint8Array.from(value.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));

  it('decodes every record the first-party Rust encoder emitted', () => {
    // The browser could read this record's layout but never recognize one: the
    // generated module carried MANIPULATION_FLOOR_V1_MAGIC_OFFSET and no magic
    // VALUE. For a number that bounds founding principal, that is the
    // difference between a check and a decoration.
    expect(floors.length).toBeGreaterThan(0);
    for (const vector of floors) {
      const floor = decodeManipulationFloorV1(bytesOf(vector.bytes));
      expect(floor.floorAtoms, vector.name).toBe(BigInt(vector.floorAtoms));
      expect(floor.basis, vector.name).toBe(vector.basis);
      expect(floor.sourceSpecId).toBe('11'.repeat(32));
      expect(floor.adapterConfigId).toBe('22'.repeat(32));
      expect(floor.collateralUnitId).toBe('33'.repeat(32));
      expect(floor.derivationReleaseId).toBe('44'.repeat(32));
    }
  });

  it('reaches the same verdict the contract reached, on every emitted floor', () => {
    for (const vector of floors) {
      const floor = decodeManipulationFloorV1(bytesOf(vector.bytes));
      const bound = BigInt(vector.largestAdmitted);
      const binding = { sourceSpecId: floor.sourceSpecId, adapterConfigId: floor.adapterConfigId, collateralUnitId: floor.collateralUnitId };
      expect(admitFoundingPrincipalV1(DEFAULT_CHAIN_STATE_CAPACITY_V1, floor, binding, bound).admitted, vector.name).toBe(vector.admitsBound);
      expect(admitFoundingPrincipalV1(DEFAULT_CHAIN_STATE_CAPACITY_V1, floor, binding, bound + 1n).admitted, vector.name).toBe(vector.admitsBoundPlusOne);
    }
  });

  it('treats a zero floor as representable and admitting nothing', () => {
    // "Found nothing against this Source" is a statement, not a decode error.
    const zero = floors.find((vector) => vector.name === 'zero-floor');
    expect(zero).toBeDefined();
    const floor = decodeManipulationFloorV1(bytesOf(zero!.bytes));
    expect(floor.floorAtoms).toBe(0n);
    expect(admitPrincipalCapacityV1(DEFAULT_CHAIN_STATE_CAPACITY_V1, floor.floorAtoms, 1n).refusal).toBe('PrincipalExceedsCapacity');
  });

  it('binds the floor to its Source, venue configuration and collateral unit first', () => {
    // A floor derived for something else is not a weaker bound; it is an answer
    // to a different question, and §6.5 applied to it looks exactly like a real
    // verdict. So the binding is checked before the arithmetic, never after.
    const floor = decodeManipulationFloorV1(bytesOf(floors[0].bytes));
    const correct = { sourceSpecId: floor.sourceSpecId, adapterConfigId: floor.adapterConfigId, collateralUnitId: floor.collateralUnitId };
    expect(() => admitFoundingPrincipalV1(DEFAULT_CHAIN_STATE_CAPACITY_V1, floor, correct, 1n)).not.toThrow();
    for (const field of ['sourceSpecId', 'adapterConfigId', 'collateralUnitId'] as const) {
      expect(() => admitFoundingPrincipalV1(DEFAULT_CHAIN_STATE_CAPACITY_V1, floor, { ...correct, [field]: 'ee'.repeat(32) }, 1n))
        .toThrow(/derived for another Source, venue configuration, or collateral unit/);
    }
  });
});

describe('the floor decoder refuses', () => {
  const canonical = () => Uint8Array.from(
    ((vectors as Readonly<{ manipulationFloors: ReadonlyArray<Readonly<{ bytes: string }>> }>).manipulationFloors[0].bytes.match(/../g) ?? []),
    (byte) => Number.parseInt(byte, 16),
  );

  it('bytes that are not a floor record at all', () => {
    const wrong = canonical();
    wrong[0] ^= 0xff;
    expect(() => decodeManipulationFloorV1(wrong)).toThrow(/not a ManipulationFloorV1 record/);
  });

  it('the wrong width, in either direction', () => {
    expect(() => decodeManipulationFloorV1(canonical().slice(0, 159))).toThrow(/not the 160 its schema declares/);
    expect(() => decodeManipulationFloorV1(new Uint8Array(161))).toThrow(/not the 160 its schema declares/);
  });

  it('an unsupported schema version', () => {
    const stale = canonical();
    stale[8] = 2;
    expect(() => decodeManipulationFloorV1(stale)).toThrow(/unsupported schema version/);
  });

  it('a nonzero reserved run, in the header or the tail', () => {
    for (const offset of [11, 152]) {
      const dirty = canonical();
      dirty[offset] = 1;
      expect(() => decodeManipulationFloorV1(dirty)).toThrow(/reserved/);
    }
  });

  it('an unknown derivation basis, including the zero tag', () => {
    for (const tag of [0, 3, 0xff]) {
      const unknown = canonical();
      unknown[10] = tag;
      expect(() => decodeManipulationFloorV1(unknown)).toThrow(/unknown derivation basis/);
    }
  });

  it('an all-zero identity in any of the four slots', () => {
    for (const [offset, field] of [[16, 'Source spec'], [48, 'adapter config'], [80, 'collateral unit'], [112, 'derivation release']] as const) {
      const vacuous = canonical();
      vacuous.set(new Uint8Array(32), offset);
      expect(() => decodeManipulationFloorV1(vacuous)).toThrow(new RegExp(`${field} is the reserved all-zero identity`));
    }
  });
});
