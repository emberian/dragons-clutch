import { describe, expect, it } from 'vitest';

import {
  CAPABILITY_FUNDING_QUOTE_OFFSET,
  CAPABILITY_MANIFEST_ENTRY_BYTES,
  CAPABILITY_MANIFEST_HEADER_BYTES,
  decodeCapabilityManifestV1,
  FUNDING_COMPARTMENTS_V1,
  recognizeCapabilityKindV1,
} from './capabilityManifest';

type Compartment = Readonly<{ name: string; assetClass: 0 | 1 | 2; amount: bigint }>;
type Entry = Readonly<{ kind: number; policy?: 0 | 1; deadline?: bigint; dependencies?: ReadonlyArray<number>; compartments?: ReadonlyArray<Compartment>; binding?: number }>;

/**
 * Write the exact canonical `DCLTFQ01` quote the Rust contract emits, so a
 * manifest built here is one the canonical contract would also accept.
 */
function quote(bytes: Uint8Array, view: DataView, offset: number, entry: Entry): void {
  bytes.set(new TextEncoder().encode('DCLTFQ01'), offset);
  view.setUint16(offset + 8, 1, true);
  const amounts = offset + 176;
  let native = BigInt(0);
  let collateral = BigInt(0);
  for (const compartment of entry.compartments ?? []) {
    const slot = FUNDING_COMPARTMENTS_V1.find((candidate) => candidate.name === compartment.name);
    if (slot === undefined) throw new Error(`unknown compartment ${compartment.name}`);
    bytes[amounts + slot.offset] = compartment.assetClass;
    view.setBigUint64(amounts + slot.offset + 8, compartment.amount, true);
    if (compartment.assetClass === 1) native += compartment.amount;
    if (compartment.assetClass === 2) collateral += compartment.amount;
  }
  view.setBigUint64(amounts + 112, native, true);
  view.setBigUint64(amounts + 120, collateral, true);
  if (entry.binding !== undefined) {
    bytes[offset + 10] = 1;
    for (let part = 0; part < 5; part += 1) {
      bytes.fill(entry.binding + part, offset + 16 + part * 32, offset + 16 + part * 32 + 32);
    }
  }
}

function manifest(entries: ReadonlyArray<Entry>): Uint8Array {
  const bytes = new Uint8Array(CAPABILITY_MANIFEST_HEADER_BYTES + CAPABILITY_MANIFEST_ENTRY_BYTES * entries.length);
  bytes.set(new TextEncoder().encode('DCLTCAP1'));
  const view = new DataView(bytes.buffer);
  view.setUint16(8, 1, true);
  view.setUint16(10, 1, true);
  view.setUint16(12, entries.length, true);
  entries.forEach((entry, index) => {
    const offset = CAPABILITY_MANIFEST_HEADER_BYTES + CAPABILITY_MANIFEST_ENTRY_BYTES * index;
    for (let identity = 0; identity < 6; identity += 1) {
      bytes.fill(entry.kind + identity, offset + identity * 32, offset + identity * 32 + 32);
    }
    const policy = entry.policy ?? 0;
    bytes[offset + 192] = policy;
    const dependencies = entry.dependencies ?? [];
    bytes[offset + 193] = dependencies.length;
    view.setBigUint64(offset + 200, entry.deadline ?? BigInt(0), true);
    dependencies.forEach((dependency, position) => { bytes[offset + 208 + position] = dependency; });
    quote(bytes, view, offset + CAPABILITY_FUNDING_QUOTE_OFFSET, entry);
  });
  return bytes;
}

describe('immutable capability manifest', () => {
  it('decodes every entry of one ordered manifest, not only a selected one', () => {
    const entries = decodeCapabilityManifestV1(manifest([{ kind: 1 }, { kind: 9, policy: 1, deadline: BigInt(4242), dependencies: [0] }]));
    expect(entries).toHaveLength(2);
    expect(entries[0]).toMatchObject({ index: 0, activation: 'immediate', deadline: BigInt(0), dependencies: [] });
    expect(entries[1]).toMatchObject({ index: 1, activation: 'deadline', deadline: BigInt(4242), dependencies: [1 - 1] });
    expect(entries[1].kind.every((byte) => byte === 9)).toBe(true);
    expect(entries[1].derivation.every((byte) => byte === 14)).toBe(true);
  });

  it('refuses a header, width, ordering, policy, or dependency list that is not canonical', () => {
    const wrongMagic = manifest([{ kind: 1 }]);
    wrongMagic[0] = 0x44 + 1;
    expect(() => decodeCapabilityManifestV1(wrongMagic)).toThrow(/wrong exact header/);

    const trailing = new Uint8Array(manifest([{ kind: 1 }]).length + 1);
    trailing.set(manifest([{ kind: 1 }]));
    expect(() => decodeCapabilityManifestV1(trailing)).toThrow(/width is invalid/);

    expect(() => decodeCapabilityManifestV1(manifest([{ kind: 9 }, { kind: 1 }]))).toThrow(/not strictly ordered/);
    expect(() => decodeCapabilityManifestV1(manifest([{ kind: 1 }, { kind: 9 }]))).not.toThrow();
    expect(() => decodeCapabilityManifestV1(manifest([{ kind: 1, policy: 1, deadline: BigInt(0) }]))).toThrow(/activation policy is noncanonical/);
    expect(() => decodeCapabilityManifestV1(manifest([{ kind: 1, deadline: BigInt(5) }]))).toThrow(/activation policy is noncanonical/);
    expect(() => decodeCapabilityManifestV1(manifest([{ kind: 1, dependencies: [0] }]))).toThrow(/dependency list is noncanonical/);
    expect(() => decodeCapabilityManifestV1(manifest([{ kind: 1 }, { kind: 9, dependencies: [3] }]))).toThrow(/dependency list is noncanonical/);

    const zeroIdentity = manifest([{ kind: 1 }]);
    zeroIdentity.fill(0, CAPABILITY_MANIFEST_HEADER_BYTES + 64, CAPABILITY_MANIFEST_HEADER_BYTES + 96);
    expect(() => decodeCapabilityManifestV1(zeroIdentity)).toThrow(/identity 2/);

    const stray = manifest([{ kind: 1 }]);
    stray[CAPABILITY_MANIFEST_HEADER_BYTES + 208] = 1;
    expect(() => decodeCapabilityManifestV1(stray)).toThrow(/inactive dependency is nonzero/);
  });

  it('keeps the seven funding compartments typed and never merges them into one number', () => {
    const [entry] = decodeCapabilityManifestV1(manifest([{
      kind: 1,
      binding: 40,
      compartments: [
        { name: 'Rent', assetClass: 1, amount: BigInt(2_039_280) },
        { name: 'Creation', assetClass: 1, amount: BigInt(5_000) },
        { name: 'Work', assetClass: 2, amount: BigInt(700) },
        { name: 'Bounty', assetClass: 2, amount: BigInt(25) },
      ],
    }]));
    expect(entry.funding.compartments.map((compartment) => compartment.compartment))
      .toEqual(['Rent', 'Creation', 'Work', 'Provider', 'Bounty', 'Liquidity', 'Service']);
    expect(entry.funding.compartments.map((compartment) => [compartment.assetClass, compartment.amount])).toEqual([
      ['native-lamports', BigInt(2_039_280)],
      ['native-lamports', BigInt(5_000)],
      ['realm-collateral', BigInt(700)],
      ['not-applicable', BigInt(0)],
      ['realm-collateral', BigInt(25)],
      ['not-applicable', BigInt(0)],
      ['not-applicable', BigInt(0)],
    ]);
    // Two physical dimensions, two independent checked totals. Nothing here
    // ever adds a lamport to a collateral atom.
    expect(entry.funding.nativeLamportsTotal).toBe(BigInt(2_044_280));
    expect(entry.funding.realmCollateralTotal).toBe(BigInt(725));
    expect(entry.funding.realmCollateral?.mint.every((byte) => byte === 43)).toBe(true);
    expect(entry.funding.compartments[0].assetPolicy).toBe('native-lamports-only');
    expect(entry.funding.compartments[2].assetPolicy).toBe('capability-selected');
  });

  it('refuses a funding quote the canonical contract would refuse', () => {
    const absent = manifest([{ kind: 1 }]);
    absent.fill(0, CAPABILITY_MANIFEST_HEADER_BYTES + CAPABILITY_FUNDING_QUOTE_OFFSET);
    expect(() => decodeCapabilityManifestV1(absent)).toThrow(/funding quote magic is not DCLTFQ01/);

    const strayTotal = manifest([{ kind: 1, compartments: [{ name: 'Work', assetClass: 1, amount: BigInt(9) }] }]);
    const nativeTotal = CAPABILITY_MANIFEST_HEADER_BYTES + CAPABILITY_FUNDING_QUOTE_OFFSET + 176 + 112;
    new DataView(strayTotal.buffer).setBigUint64(nativeTotal, BigInt(10), true);
    expect(() => decodeCapabilityManifestV1(strayTotal)).toThrow(/asset totals differ from its own typed compartments/);

    // A positive amount with the not-applicable class, and a zero amount with a
    // real class, are both noncanonical: class and amount state one fact.
    const untypedAmount = manifest([{ kind: 1, compartments: [{ name: 'Service', assetClass: 0, amount: BigInt(3) }] }]);
    expect(() => decodeCapabilityManifestV1(untypedAmount)).toThrow(/Service compartment is not one canonical typed amount/);
    const zeroClassed = manifest([{ kind: 1, compartments: [{ name: 'Service', assetClass: 1, amount: BigInt(0) }] }]);
    expect(() => decodeCapabilityManifestV1(zeroClassed)).toThrow(/Service compartment is not one canonical typed amount/);

    // Rent and Creation are intrinsically native lamports.
    const collateralRent = manifest([{ kind: 1, binding: 40, compartments: [{ name: 'Rent', assetClass: 2, amount: BigInt(1) }] }]);
    expect(() => decodeCapabilityManifestV1(collateralRent)).toThrow(/Rent compartment is intrinsically native lamports/);

    // Collateral without its immutable binding, and a binding without any
    // collateral, are each a refusal rather than a partially shown quote.
    const unbound = manifest([{ kind: 1, compartments: [{ name: 'Work', assetClass: 2, amount: BigInt(4) }] }]);
    expect(() => decodeCapabilityManifestV1(unbound)).toThrow(/binding does not match its own collateral total/);
    const strayBinding = manifest([{ kind: 1, binding: 40 }]);
    expect(() => decodeCapabilityManifestV1(strayBinding)).toThrow(/binding does not match its own collateral total/);

    const zeroMint = manifest([{ kind: 1, binding: 40, compartments: [{ name: 'Work', assetClass: 2, amount: BigInt(4) }] }]);
    const mintOffset = CAPABILITY_MANIFEST_HEADER_BYTES + CAPABILITY_FUNDING_QUOTE_OFFSET + 16 + 96;
    zeroMint.fill(0, mintOffset, mintOffset + 32);
    expect(() => decodeCapabilityManifestV1(zeroMint)).toThrow(/Realm collateral collateral mint is the reserved all-zero identity/);
  });

  it('names only kinds it recognizes and never invents a meaning', () => {
    const known = Uint8Array.from((
      '8e8a063932339a7eb910608e76b1e70ad0f41b999b6252eeab890ffb733b5474'.match(/../g) ?? []
    ), (pair) => Number.parseInt(pair, 16));
    expect(recognizeCapabilityKindV1(known)).toBe('Product payoff admission');
    expect(recognizeCapabilityKindV1(new Uint8Array(32).fill(3))).toBeNull();
  });
});
