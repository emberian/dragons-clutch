import { readFileSync } from 'node:fs';

import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { u16 } from './bytes';
import {
  CAPABILITY_ENTRY_BYTES_V1,
  CAPABILITY_ENTRY_QUOTE_OFFSET_V1,
  CAPABILITY_MANIFEST_HEADER_BYTES_V1,
  MAX_CAPABILITIES_V1,
  capabilityEntryLedgerMaskV2,
  capabilityFundingLedgerAddressV2,
  capabilityRootAddressV1,
  decodeCapabilityManifestV1,
  FUNDING_COMPARTMENTS_V1,
  recognizeCapabilityKindV1,
} from './capabilityManifest';
import { capabilityRootSeedsV1 } from './directHotBumpHintsV1';

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
  const bytes = new Uint8Array(CAPABILITY_MANIFEST_HEADER_BYTES_V1 + CAPABILITY_ENTRY_BYTES_V1 * entries.length);
  bytes.set(new TextEncoder().encode('DCLTCAP1'));
  const view = new DataView(bytes.buffer);
  view.setUint16(8, 1, true);
  view.setUint16(10, 1, true);
  view.setUint16(12, entries.length, true);
  entries.forEach((entry, index) => {
    const offset = CAPABILITY_MANIFEST_HEADER_BYTES_V1 + CAPABILITY_ENTRY_BYTES_V1 * index;
    for (let identity = 0; identity < 6; identity += 1) {
      bytes.fill(entry.kind + identity, offset + identity * 32, offset + identity * 32 + 32);
    }
    const policy = entry.policy ?? 0;
    bytes[offset + 192] = policy;
    const dependencies = entry.dependencies ?? [];
    bytes[offset + 193] = dependencies.length;
    view.setBigUint64(offset + 200, entry.deadline ?? BigInt(0), true);
    dependencies.forEach((dependency, position) => { bytes[offset + 208 + position] = dependency; });
    quote(bytes, view, offset + CAPABILITY_ENTRY_QUOTE_OFFSET_V1, entry);
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
    zeroIdentity.fill(0, CAPABILITY_MANIFEST_HEADER_BYTES_V1 + 64, CAPABILITY_MANIFEST_HEADER_BYTES_V1 + 96);
    expect(() => decodeCapabilityManifestV1(zeroIdentity)).toThrow(/identity 2/);

    const stray = manifest([{ kind: 1 }]);
    stray[CAPABILITY_MANIFEST_HEADER_BYTES_V1 + 208] = 1;
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
    absent.fill(0, CAPABILITY_MANIFEST_HEADER_BYTES_V1 + CAPABILITY_ENTRY_QUOTE_OFFSET_V1);
    expect(() => decodeCapabilityManifestV1(absent)).toThrow(/funding quote magic is not DCLTFQ01/);

    const strayTotal = manifest([{ kind: 1, compartments: [{ name: 'Work', assetClass: 1, amount: BigInt(9) }] }]);
    const nativeTotal = CAPABILITY_MANIFEST_HEADER_BYTES_V1 + CAPABILITY_ENTRY_QUOTE_OFFSET_V1 + 176 + 112;
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
    const mintOffset = CAPABILITY_MANIFEST_HEADER_BYTES_V1 + CAPABILITY_ENTRY_QUOTE_OFFSET_V1 + 16 + 96;
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

/**
 * The two addresses a Market's own header names, against their second author.
 *
 * `capabilityRootAddressV1` is the FORWARD projection: a reader holding a
 * Market and its manifest entry names the root before reading it, which is the
 * whole difference between `needs-chain` and a verdict on a Direct card.
 * `directHotBumpHintsV1.capabilityRootSeedsV1` is the REVERSE one, recovering
 * the same eight seeds out of the root account's own immutable header, and its
 * fixture is emitted by `crates/dclutch-operator`'s vector test through the
 * Rust seed constructors. So the agreement below is between two independent
 * paths out of the chain's own author, and neither is this file.
 */
describe('the addresses a Market determines', () => {
  const vector = JSON.parse(
    readFileSync(new URL('../fixtures/direct-hot-bump-hints.json', import.meta.url), 'utf8'),
  ) as Readonly<{ tradingProgram: string; market: string; generation: string; capabilityRootHeaderHex: string }>;
  const seedBytes = (value: string): Uint8Array =>
    Uint8Array.from(value.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));

  it('names the capability root the root header’s own seeds reproduce', () => {
    const seeds = capabilityRootSeedsV1(seedBytes(vector.capabilityRootHeaderHex));
    expect(seeds).toHaveLength(8);
    // The header's Market is the vector's Market: without this the agreement
    // below could hold between two readings of the same header.
    expect(new PublicKey(seeds[1]!).toBase58()).toBe(vector.market);
    const reverse = PublicKey.findProgramAddressSync(
      seeds as Uint8Array[], new PublicKey(vector.tradingProgram),
    )[0].toBase58();

    const forward = capabilityRootAddressV1(
      vector.tradingProgram,
      vector.market,
      BigInt(vector.generation),
      seeds[3]!,
      Object.freeze({ index: u16(seeds[4]!, 0), kind: seeds[5]!, programSet: seeds[6]!, config: seeds[7]! }),
    );
    expect(forward).toBe(reverse);

    // Proved red-able: every seed past the domain is load-bearing, so a
    // neighbouring entry index is a DIFFERENT account and not a rounding.
    const neighbour = capabilityRootAddressV1(
      vector.tradingProgram,
      vector.market,
      BigInt(vector.generation),
      seeds[3]!,
      Object.freeze({ index: u16(seeds[4]!, 0) + 1, kind: seeds[5]!, programSet: seeds[6]!, config: seeds[7]! }),
    );
    expect(neighbour).not.toBe(forward);
  });

  /**
   * The singleton mask, which is a chain rule and not a convenience.
   *
   * `authenticate_ledger_controller` refuses a writable Trading-owned ledger
   * whose mask holds the acted-on entry's bit together with anything else, so
   * a controller ledger's whole selection is one bit and its address is a
   * function of the entry index. Nothing else about a funding ledger is
   * derivable from a Market, and the hostiles below are the boundary.
   */
  it('derives a controller funding ledger from the entry index alone', () => {
    const manifestId = new Uint8Array(32).fill(3);
    const market = new PublicKey(new Uint8Array(32).fill(7)).toBase58();
    const trading = new PublicKey(new Uint8Array(32).fill(13)).toBase58();
    expect(capabilityEntryLedgerMaskV2(0)).toBe(1);
    expect(capabilityEntryLedgerMaskV2(3)).toBe(8);
    const first = capabilityFundingLedgerAddressV2(trading, market, BigInt(2), manifestId, capabilityEntryLedgerMaskV2(0));
    const fourth = capabilityFundingLedgerAddressV2(trading, market, BigInt(2), manifestId, capabilityEntryLedgerMaskV2(3));
    expect(first).not.toBe(fourth);
    // A generation is part of the address, so a re-founded Market's ledger is
    // a different account rather than the same one reused.
    expect(capabilityFundingLedgerAddressV2(trading, market, BigInt(1), manifestId, 1)).not.toBe(first);

    expect(() => capabilityEntryLedgerMaskV2(-1)).toThrow(/entry index is/);
    expect(() => capabilityEntryLedgerMaskV2(MAX_CAPABILITIES_V1)).toThrow(/entry index is/);
    expect(() => capabilityFundingLedgerAddressV2(trading, market, BigInt(2), manifestId, 0)).toThrow(/nonzero u16/);
    expect(() => capabilityFundingLedgerAddressV2(trading, market, BigInt(2), manifestId, 0x1_0000)).toThrow(/nonzero u16/);
    expect(() => capabilityFundingLedgerAddressV2(trading, market, BigInt(-1), manifestId, 1)).toThrow(/generation is a u64/);
    expect(() => capabilityFundingLedgerAddressV2(trading, market, BigInt(2), new Uint8Array(32), 1)).toThrow(/reserved all-zero identity/);
    expect(() => capabilityFundingLedgerAddressV2(trading, market, BigInt(2), new Uint8Array(31).fill(3), 1)).toThrow(/identity is 32 bytes/);
  });
});
