import { AddressLookupTableAccount, PublicKey } from '@solana/web3.js';

import { hex } from './bytes';
import { describe, expect, it } from 'vitest';

import { HOT_FIXED_ACCOUNT_COUNT_V3 } from './generated/directInlineV3';
import {
  RATIONAL_LIFECYCLE_CLAIMS_COMMON_ACCOUNTS_V2,
  RATIONAL_LIFECYCLE_COMPACT_OUTER_BYTES_V4,
  RATIONAL_LIFECYCLE_COMPACT_REQUEST_BYTES_V4,
  RATIONAL_LIFECYCLE_VACANCY_ACCOUNTS_V2,
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
  it('hostile-decodes descriptor-owned K and derives only its ordered positive support S', () => {
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
    expect(request).toHaveLength(RATIONAL_LIFECYCLE_COMPACT_REQUEST_BYTES_V4);
    expect(new TextDecoder().decode(request.slice(0, 8))).toBe('DCRLHC04');
    expect(request[10]).toBe(3);
    expect(new DataView(request.buffer).getUint32(380, true)).toBe(0);
    expect(new DataView(request.buffer).getBigUint64(392, true)).toBe(110n);
    const decoded = decodeRationalRepresentationDescriptorV3(descriptor(), bytes(21));
    const support = deriveRationalRetireReceiptSupportV4(address(30), bytes(21), decoded.support, address(31));
    const digest = await deriveRationalRetireReceiptChildDigestV4(request, support);
    expect(digest).toHaveLength(32);
    // A REGRESSION PIN, and it is not an authority. Nothing in this tree can
    // check this client's child against the program's: the lifecycle contract
    // owns the layout and publishes no byte fixture, so this value came from
    // this encoder rather than from Rust. What it catches is the hole the
    // coordinates' own red proof exposed -- move
    // LIFECYCLE_ROW_CUSTODY_OWNER_OFFSET in the emitted module and the family
    // encoder, the child encoder and every other assertion here stay green
    // while the wallet signs a different child. A Rust-emitted example child
    // would make this a real cross-boundary check; it is owed.
    expect(hex(digest)).toBe('cee208ad10b26cde3bbcfb9567c7d8a6fa1f28dd8bdda2b75d1553b7b965a630');
    await expect(deriveRationalRetireReceiptChildDigestV4(request, [support[1], support[0], support[2]])).rejects.toThrow(/unordered/);
    await expect(deriveRationalRetireReceiptChildDigestV4(request, [])).rejects.toThrow(/wrong exact width/);
  });

  it('compiles a wallet-signable candidate only with the exact Hot frame and an active ALT', () => {
    // The Hot fixed frame width is a protocol fact, not a number this test may
    // pin: hard-coding it left the candidate a frame short of what the chain
    // requires the moment the frame grew.
    const fixed = Array.from({ length: HOT_FIXED_ACCOUNT_COUNT_V3 }, (_, index) => Object.freeze({ address: address(40 + index), isSigner: false, isWritable: index === 1 }));
    const decoded = decodeRationalRepresentationDescriptorV3(descriptor(), bytes(21));
    const support = deriveRationalRetireReceiptSupportV4(address(30), bytes(21), decoded.support, address(31));
    // Ten Claims-common entries are physical aliases of the fixed Hot frame.
    // Count the compiled message, never the 59+5K source metas: ALT changes
    // packet encoding but not the runtime lock set.
    const common = [
      address(90), fixed[25]!.address, fixed[26]!.address, address(91), address(92),
      fixed[27]!.address, fixed[22]!.address, fixed[28]!.address, address(93), fixed[8]!.address,
      fixed[9]!.address, address(94), address(95), address(96), address(97), address(98),
      address(99), fixed[0]!.address, fixed[23]!.address, fixed[24]!.address,
    ].map((value, index) => Object.freeze({ address: value, isSigner: false, isWritable: index === 12 || index === 14 }));

    /**
     * One inspection over the first `rows` coordinates of the descriptor's support.
     *
     * The vacancy group is FIVE accounts, in the contract's physical order:
     * shard Mint, Structured custody, custody OWNER, Position, admission. This
     * file built four of them, which is the same count the source held, and
     * that is what moved every number below -- a four-account group understates
     * the lock budget by exactly one account per support row, so a width this
     * arm called compilable is one the runtime would refuse.
     */
    const inspect = (rows: number, alt: 'active' | 'empty' = 'active') => {
      const selected = support.slice(0, rows);
      const claims = Object.freeze([
        ...common,
        ...selected.flatMap((row) => [row.shardMint, row.structuredCustody, row.owner, row.position, row.admission]
          .map((value) => Object.freeze({ address: value, isSigner: false, isWritable: false }))),
      ]);
      const addresses = alt === 'empty' ? [] : Array.from(new Set([...fixed, ...claims].map((meta) => meta.address))).map((value) => new PublicKey(value));
      return Object.freeze({
        observedSlot: '10', payer: address(201), fixedAccounts: fixed, claimsAccounts: claims, support: selected,
        lookupTable: new AddressLookupTableAccount({
          key: new PublicKey(bytes(200)),
          state: { deactivationSlot: 18_446_744_073_709_551_615n, lastExtendedSlot: 0, lastExtendedSlotStartIndex: 0, authority: undefined, addresses },
        }),
        market: address(14), generation: 14n, releaseSet: bytes(15), descriptorId: bytes(21), graphId: bytes(11),
        representationAuthority: address(22), receiptMint: address(16), claimsProgram: address(30), claimsRevision: 3n,
        representationWidth: 5, resultOutcomeCount: 258, rentCredit: address(24), rentProgram: address(25), receiptLamports: 10n,
        receiptRentPrincipal: 10n, rentCreditBefore: 100n, familyBytes: family(), familyDigest: bytes(202),
        childDigest: bytes(203), rootDigest: bytes(204), callerAuthority: address(205), executionStatus: 'ready' as const,
      }) satisfies RationalRetireReceiptInspectionV4;
    };

    const plan = buildRationalRetireReceiptCandidateV4(inspect(2), address(206));
    expect(plan.outerBytes).toHaveLength(RATIONAL_LIFECYCLE_COMPACT_OUTER_BYTES_V4);
    expect(plan.accountCount).toBe(HOT_FIXED_ACCOUNT_COUNT_V3 + RATIONAL_LIFECYCLE_CLAIMS_COMMON_ACCOUNTS_V2 + RATIONAL_LIFECYCLE_VACANCY_ACCOUNTS_V2 * 2);
    expect(plan.supportCount).toBe(2);
    expect(plan.loadedAddresses).toBeGreaterThan(0);
    expect(plan.accountLocks).toBe(60);
    expect(plan.wireBytes.length).toBeLessThanOrEqual(1232);
    expect(plan.requiredSigners).toEqual([address(201)]);
    expect(plan.executionStatus).toBe('ready');
    expect(() => buildRationalRetireReceiptCandidateV4(inspect(2, 'empty'), address(206))).toThrow();

    // The lock ceiling this fixture's aliasing allows, stated rather than
    // assumed: three coordinates need five more locks than two, and that is
    // over devnet's cap. The old four-account group put this same width at 62
    // and called it ready.
    expect(() => buildRationalRetireReceiptCandidateV4(inspect(3), address(206))).toThrow(/65 unique account locks.*64-lock/);
  });
});
