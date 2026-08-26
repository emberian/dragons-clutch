import { AddressLookupTableAccount, PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  buildRationalRetireReceiptCandidateV4,
  decodeRationalRepresentationDescriptorV3,
  deriveRationalRetireReceiptChildDigestV4,
  deriveRationalRetireReceiptSupportV4,
  encodeRationalRetireReceiptFamilyV4,
  type RationalRetireReceiptInspectionV4,
} from './rationalRetireReceiptV4';

function bytes(value: number): Uint8Array { return new Uint8Array(32).fill(value); }
function address(value: number): string { return new PublicKey(bytes(value)).toBase58(); }
function putU16(output: Uint8Array, offset: number, value: number): void { new DataView(output.buffer).setUint16(offset, value, true); }
function putU32(output: Uint8Array, offset: number, value: number): void { new DataView(output.buffer).setUint32(offset, value, true); }
function putU64(output: Uint8Array, offset: number, value: bigint): void { new DataView(output.buffer).setBigUint64(offset, value, true); }

function descriptor(): Uint8Array {
  const output = new Uint8Array(256 + 5 * 8);
  output.set(new TextEncoder().encode('DCRRDSC3')); putU16(output, 8, 3);
  [11, 12, 13, 14, 15, 16, 17].forEach((value, index) => output.set(bytes(value), 16 + index * 32));
  putU32(output, 240, 5); putU64(output, 248, 10n);
  [0n, 7n, 5n, 0n, 9n].forEach((value, index) => putU64(output, 256 + index * 8, value));
  return output;
}

function family() {
  return encodeRationalRetireReceiptFamilyV4({
    releaseSet: bytes(15), market: address(14), graphId: bytes(11), descriptorId: bytes(21),
    representationAuthority: address(22), receiptMint: address(16), rentCredit: address(24), rentProgram: address(25),
    generation: 14n, claimsRevision: 3n, receiptLamports: 10n, receiptRent: 10n, outcomeCount: 5, rentBefore: 100n,
  });
}

describe('compact Rational RetireReceipt V4', () => {
  it('hostile-decodes descriptor-owned N and derives only its ordered positive support K', () => {
    const decoded = decodeRationalRepresentationDescriptorV3(descriptor(), bytes(21));
    expect(decoded.outcomeCount).toBe(5);
    expect(decoded.support).toEqual([
      { outcome: 1, coefficient: 7n }, { outcome: 2, coefficient: 5n }, { outcome: 4, coefficient: 9n },
    ]);
    const rows = deriveRationalRetireReceiptSupportV4(address(30), bytes(21), decoded.support, address(31));
    expect(rows.map((row) => row.outcome)).toEqual([1, 2, 4]);
    expect(new Set(rows.flatMap((row) => [row.owner, row.shardMint, row.structuredCustody, row.position, row.admission])).size).toBe(15);
  });

  it('rejects reserved bytes, empty support, and a substituted descriptor width', () => {
    const reserved = descriptor(); reserved[10] = 1;
    expect(() => decodeRationalRepresentationDescriptorV3(reserved, bytes(21))).toThrow(/reserved/);
    const empty = descriptor(); empty.fill(0, 256);
    expect(() => decodeRationalRepresentationDescriptorV3(empty, bytes(21))).toThrow(/empty support/);
    const short = descriptor().slice(0, -8);
    expect(() => decodeRationalRepresentationDescriptorV3(short, bytes(21))).toThrow(/width/);
  });

  it('encodes exact fixed400 family facts and derives a support-sensitive Claims child digest', async () => {
    const request = family();
    expect(request).toHaveLength(400);
    expect(new TextDecoder().decode(request.slice(0, 8))).toBe('DCRLHC04');
    expect(request[10]).toBe(3);
    expect(new DataView(request.buffer).getUint32(380, true)).toBe(0);
    expect(new DataView(request.buffer).getBigUint64(392, true)).toBe(110n);
    const decoded = decodeRationalRepresentationDescriptorV3(descriptor(), bytes(21));
    const support = deriveRationalRetireReceiptSupportV4(address(30), bytes(21), decoded.support, address(31));
    const digest = await deriveRationalRetireReceiptChildDigestV4(request, support);
    expect(digest).toHaveLength(32);
    await expect(deriveRationalRetireReceiptChildDigestV4(request, [support[1], support[0], support[2]])).rejects.toThrow(/unordered/);
    await expect(deriveRationalRetireReceiptChildDigestV4(request, [])).rejects.toThrow(/wrong exact width/);
  });

  it('compiles the exact fixed38 plus Claims20+4K v0 candidate while refusing execution', async () => {
    const fixed = Array.from({ length: 38 }, (_, index) => Object.freeze({ address: address(40 + index), isSigner: false, isWritable: index === 1 }));
    const decoded = decodeRationalRepresentationDescriptorV3(descriptor(), bytes(21));
    const support = deriveRationalRetireReceiptSupportV4(address(30), bytes(21), decoded.support, address(31));
    const claims = Array.from({ length: 20 + 4 * support.length }, (_, index) => Object.freeze({ address: address(90 + index), isSigner: false, isWritable: index === 12 || index === 14 }));
    const table = new AddressLookupTableAccount({
      key: new PublicKey(bytes(200)),
      state: { deactivationSlot: 18_446_744_073_709_551_615n, lastExtendedSlot: 0, lastExtendedSlotStartIndex: 0, authority: undefined,
        addresses: [...fixed, ...claims].map((meta) => new PublicKey(meta.address)) },
    });
    const request = family();
    const inspection = Object.freeze({
      observedSlot: '10', payer: address(201), fixedAccounts: fixed, claimsAccounts: claims, support, lookupTable: table,
      market: address(14), generation: 14n, releaseSet: bytes(15), descriptorId: bytes(21), graphId: bytes(11),
      representationAuthority: address(22), receiptMint: address(16), claimsProgram: address(30), claimsRevision: 3n,
      productOutcomeCount: 5, rentCredit: address(24), rentProgram: address(25), receiptLamports: 10n,
      receiptRentPrincipal: 10n, rentCreditBefore: 100n, familyBytes: request, familyDigest: bytes(202),
      childDigest: bytes(203), rootDigest: bytes(204), callerAuthority: address(205), executionStatus: 'blocked' as const,
      refusal: 'EffectV4 pending',
    }) satisfies RationalRetireReceiptInspectionV4;
    const plan = buildRationalRetireReceiptCandidateV4(inspection, address(206));
    expect(plan.outerBytes).toHaveLength(528);
    expect(plan.accountCount).toBe(70);
    expect(plan.supportCount).toBe(3);
    expect(plan.loadedAddresses).toBeGreaterThan(0);
    expect(plan.wireBytes.length).toBeLessThanOrEqual(1232);
    expect(plan.requiredSigners).toEqual([address(201)]);
    expect(plan.executionStatus).toBe('blocked');
  });
});
