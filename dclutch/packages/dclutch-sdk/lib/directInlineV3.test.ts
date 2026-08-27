import {
  AddressLookupTableAccount,
  Ed25519Program,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SYSVAR_RENT_PUBKEY,
  TransactionInstruction,
} from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  type CompactIntentV2Input,
  type DirectHotAccountMetaV3,
  type DirectInlineHotRouteV3,
  type SignedDirectIntentV3,
  canonicalDirectInlineLookupAddressesV3,
  compileDirectInlineTransactionV3,
  encodeCompactIntentSigningMessageV2,
  encodeDirectInlineOrdinaryRequestV3,
  previewDirectInlineV3,
  validateDirectNativeEvidenceInstructionV3,
  validateRuntimeAccountProfileV2,
} from './directInlineV3';
import {
  COMPACT_INTENT_OUTCOME_OFFSET_V2,
  COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2,
  CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
  DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3,
  DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3,
  DIRECT_NATIVE_EVIDENCE_BUYER_MESSAGE_OFFSET_V3,
  DIRECT_NATIVE_EVIDENCE_BYTES_V3,
  DIRECT_NATIVE_EVIDENCE_DESCRIPTOR_BYTES_V3,
  DIRECT_NATIVE_EVIDENCE_DIRECT_BIAS_V3,
  DIRECT_NATIVE_EVIDENCE_HEADERLESS_REGISTRY_BIAS_V4,
  DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3,
  DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
  HOT_EXECUTION_ENVELOPE_BYTES_V3,
  HOT_FIXED_ACCOUNT_COUNT_V3,
  HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
  HOT_MARKET_ACCOUNT_V3,
  HOT_RENT_SYSVAR_ACCOUNT_V3,
  HOT_ROOT_ACCOUNT_V3,
  HOT_TRADING_PROGRAM_ACCOUNT_V3,
} from './generated/directInlineV3';

const MAX_U64 = 18_446_744_073_709_551_615n;

function key(seed: number): string {
  return new PublicKey(new Uint8Array(32).fill(seed)).toBase58();
}

function intent(side: 0 | 1, market: string, collateral: string): CompactIntentV2Input {
  return Object.freeze({
    side,
    lifecycle: 0,
    outcome: 70_000,
    market,
    generation: 19n,
    nonce: BigInt(side),
    validFrom: 900n,
    validThrough: 1_100n,
    maximumFill: 2_000n,
    limitPrice: side === 0 ? 400_000n : 600_000n,
    feeBasisPoints: 25,
    collateralAccount: collateral,
  });
}

function participants(market: string): Readonly<{ seller: SignedDirectIntentV3; buyer: SignedDirectIntentV3 }> {
  return Object.freeze({
    seller: Object.freeze({ maker: key(2), signature: new Uint8Array(64).fill(11), intent: intent(0, market, key(3)) }),
    buyer: Object.freeze({ maker: key(4), signature: new Uint8Array(64).fill(12), intent: intent(1, market, key(5)) }),
  });
}

function runtimeProfile(): Uint8Array {
  const fixedAccounts = 5;
  const predicateBytes = 16;
  const profileHeader = 48 + predicateBytes;
  const output = new Uint8Array(profileHeader + fixedAccounts * 16);
  output.set(new TextEncoder().encode('DCLTAP02'), 0);
  const view = new DataView(output.buffer);
  view.setUint16(8, 2, true);
  view.setUint16(10, 14, true);
  view.setUint16(12, fixedAccounts, true);
  view.setUint16(20, 1, true);
  view.setUint16(42, 1, true);
  output[48] = 1;
  output[56] = 0x41;
  output[profileHeader] = 2;
  for (let accountIndex = 0; accountIndex < fixedAccounts; accountIndex += 1) {
    view.setUint32(profileHeader + accountIndex * 16 + 8, 1, true);
  }
  return output;
}

function account(address: string, isWritable = false, executable = false): DirectHotAccountMetaV3 {
  return Object.freeze({ address, isSigner: false, isWritable, executable });
}

function containsSlice(bytes: Uint8Array, needle: Uint8Array): boolean {
  for (let offset = 0; offset + needle.length <= bytes.length; offset += 1) {
    if (needle.every((value, index) => bytes[offset + index] === value)) return true;
  }
  return false;
}

function route(checked = true): DirectInlineHotRouteV3 {
  const market = key(10);
  const fixed = Array.from({ length: HOT_FIXED_ACCOUNT_COUNT_V3 }, (_, index) => account(key(20 + index)));
  fixed[HOT_MARKET_ACCOUNT_V3] = account(market);
  fixed[HOT_ROOT_ACCOUNT_V3] = account(key(11), true);
  fixed[HOT_TRADING_PROGRAM_ACCOUNT_V3] = account(key(12), false, true);
  fixed[HOT_RENT_SYSVAR_ACCOUNT_V3] = account(SYSVAR_RENT_PUBKEY.toBase58());
  fixed[HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3] = account(SYSVAR_INSTRUCTIONS_PUBKEY.toBase58());
  const routeAccounts = Object.freeze({
    payer: key(91),
    tradingProgram: key(12),
    fixedAccounts: Object.freeze(fixed),
    strategyAccounts: Object.freeze([]),
    runtimeAccounts: Object.freeze([]),
  });
  const lookupTable = new AddressLookupTableAccount({
    key: new PublicKey(key(90)),
    state: {
      deactivationSlot: MAX_U64,
      lastExtendedSlot: 800,
      lastExtendedSlotStartIndex: 0,
      authority: undefined,
      addresses: [...canonicalDirectInlineLookupAddressesV3(routeAccounts)],
    },
  });
  return Object.freeze({
    ...routeAccounts,
    market,
    releaseSet: new Uint8Array(32).fill(31),
    generation: 19n,
    rootPrestateDigest: new Uint8Array(32).fill(32),
    outcomeCount: 70_001,
    priceScale: 1_000_000n,
    feeBasisPoints: 25,
    accountProfile: runtimeProfile(),
    selectedProgramSchema: CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
    selectedProgram: new Uint8Array(32).fill(33),
    recentBlockhash: key(92),
    lookupTables: Object.freeze([lookupTable]),
    outerEvidence: checked
      ? Object.freeze({ status: 'checked' as const, tradingArtifactRelease: '11'.repeat(32), checkedManifestDigest: '12'.repeat(32) })
      : Object.freeze({ status: 'unavailable' as const, reason: 'common hot outer has no accepted checked release' }),
  });
}

describe('Direct V3 inline transaction construction', () => {
  it('encodes runtime-u32 maker intents and both exact signing slices', () => {
    const market = key(10);
    const { seller, buyer } = participants(market);
    const signing = encodeCompactIntentSigningMessageV2(seller.intent);
    const request = encodeDirectInlineOrdinaryRequestV3(seller, buyer, 2_000n, 500_000n);
    expect(signing).toHaveLength(COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2);
    expect(new DataView(signing.buffer, signing.byteOffset + 32 + COMPACT_INTENT_OUTCOME_OFFSET_V2, 4).getUint32(0, true)).toBe(70_000);
    expect(request).toHaveLength(DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3);
    expect(request.slice(DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + 32, DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + 32 + signing.length)).toEqual(signing);
    expect(request.slice(DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 32, DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 32 + signing.length)).toEqual(encodeCompactIntentSigningMessageV2(buyer.intent));
  });

  it('previews one named collateral rounding boundary exactly', () => {
    const candidate = route();
    const { seller, buyer } = participants(candidate.market);
    expect(previewDirectInlineV3(candidate, seller, buyer, 2_000n, 500_000n, 1_000n)).toEqual({
      fill: 2_000n,
      executionPrice: 500_000n,
      grossCollateral: 1_000n,
      sellerFee: 2n,
      buyerFee: 2n,
      sellerNetCollateralCredit: 998n,
      buyerCollateralDebit: 1_002n,
      totalFeeTransfer: 4n,
    });
    expect(() => previewDirectInlineV3(candidate, seller, buyer, 2_000n, 500_001n, 1_000n)).toThrow(/not exactly representable/);
  });

  it('compiles the exact adjacent Ed25519 + Trading v0 batch and fails closed without a checked outer', () => {
    const candidate = route();
    const { seller, buyer } = participants(candidate.market);
    const plan = compileDirectInlineTransactionV3({ route: candidate, seller, buyer, fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n });
    expect(plan.hotInstructionBytes).toHaveLength(HOT_EXECUTION_ENVELOPE_BYTES_V3 + DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3);
    expect(plan.requiredSigners).toEqual([candidate.payer]);
    expect(plan.transaction.message.compiledInstructions).toHaveLength(2);
    expect(plan.nativeEvidenceBytes).toHaveLength(DIRECT_NATIVE_EVIDENCE_BYTES_V3);
    expect(plan.nativeEvidenceInstructionIndex).toBe(0);
    expect(plan.tradingInstructionIndex).toBe(1);
    expect(plan.nativeMessageOffsets).toEqual([
      DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3,
      DIRECT_NATIVE_EVIDENCE_BUYER_MESSAGE_OFFSET_V3,
    ]);
    expect(containsSlice(plan.nativeEvidenceBytes, encodeCompactIntentSigningMessageV2(seller.intent))).toBe(false);
    expect(containsSlice(plan.nativeEvidenceBytes, encodeCompactIntentSigningMessageV2(buyer.intent))).toBe(false);
    expect(plan.wireBytes.length).toBeLessThanOrEqual(1_232);
    expect(plan.loadedAddresses).toBeGreaterThan(20);
    expect(() => compileDirectInlineTransactionV3({ route: route(false), seller, buyer, fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n })).toThrow(/unavailable/);
  });

  it('binds native evidence to exact Trading offsets and assembler-derived instruction index', () => {
    const candidate = route();
    const { seller, buyer } = participants(candidate.market);
    const plan = compileDirectInlineTransactionV3({ route: candidate, seller, buyer, fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n });
    const trading = new TransactionInstruction({
      programId: new PublicKey(candidate.tradingProgram),
      keys: [],
      data: plan.hotInstructionBytes as Buffer,
    });
    const evidence = new TransactionInstruction({
      programId: Ed25519Program.programId,
      keys: [],
      data: plan.nativeEvidenceBytes as Buffer,
    });
    expect(() => validateDirectNativeEvidenceInstructionV3(
      evidence, trading, plan.tradingInstructionIndex, new PublicKey(candidate.tradingProgram),
    )).not.toThrow();

    const offset = plan.nativeEvidenceBytes.slice();
    new DataView(offset.buffer).setUint16(2 + 8, DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3 - 1, true);
    expect(() => validateDirectNativeEvidenceInstructionV3(
      new TransactionInstruction({ programId: Ed25519Program.programId, keys: [], data: offset as Buffer }),
      trading, plan.tradingInstructionIndex, new PublicKey(candidate.tradingProgram),
    )).toThrow(/offset or instruction index/);

    const instructionIndex = plan.nativeEvidenceBytes.slice();
    new DataView(instructionIndex.buffer).setUint16(2 + DIRECT_NATIVE_EVIDENCE_DESCRIPTOR_BYTES_V3 + 12, 0, true);
    expect(() => validateDirectNativeEvidenceInstructionV3(
      new TransactionInstruction({ programId: Ed25519Program.programId, keys: [], data: instructionIndex as Buffer }),
      trading, plan.tradingInstructionIndex, new PublicKey(candidate.tradingProgram),
    )).toThrow(/offset or instruction index/);

    expect(() => validateDirectNativeEvidenceInstructionV3(
      new TransactionInstruction({ programId: new PublicKey(key(88)), keys: [], data: plan.nativeEvidenceBytes as Buffer }),
      trading, plan.tradingInstructionIndex, new PublicKey(candidate.tradingProgram),
    )).toThrow(/substitutes the Ed25519 program/);
    expect(() => validateDirectNativeEvidenceInstructionV3(
      evidence,
      new TransactionInstruction({ programId: new PublicKey(key(87)), keys: [], data: plan.hotInstructionBytes as Buffer }),
      plan.tradingInstructionIndex,
      new PublicKey(candidate.tradingProgram),
    )).toThrow(/not the authenticated Trading program/);
  });

  it('pins the exact bias-zero native-evidence wire against the Rust codec vector', () => {
    // Expected bytes are the vector asserted by
    // crates/dclutch-direct-codec/src/native_evidence_v3.rs, test
    // `direct_and_headerless_registry_use_exact_current_instruction_offsets`:
    // 222-byte evidence, message offsets 192 and 396, u16::MAX sentinels, and
    // the same coordinates for the headerless Registry successor.
    const candidate = route();
    const { seller, buyer } = participants(candidate.market);
    const plan = compileDirectInlineTransactionV3({ route: candidate, seller, buyer, fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n });
    const evidence = plan.nativeEvidenceBytes;
    const view = new DataView(evidence.buffer, evidence.byteOffset, evidence.byteLength);
    const u16le = (offset: number): number => view.getUint16(offset, true);
    expect(DIRECT_NATIVE_EVIDENCE_DIRECT_BIAS_V3).toBe(0);
    expect(DIRECT_NATIVE_EVIDENCE_HEADERLESS_REGISTRY_BIAS_V4).toBe(DIRECT_NATIVE_EVIDENCE_DIRECT_BIAS_V3);
    expect(DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3).toBe(192);
    expect(DIRECT_NATIVE_EVIDENCE_BUYER_MESSAGE_OFFSET_V3).toBe(396);
    expect(evidence).toHaveLength(222);
    expect(evidence[0]).toBe(2);
    expect(evidence[1]).toBe(0);
    for (const [descriptor, publicKey, message] of [[2, 30, 192], [16, 126, 396]]) {
      expect(u16le(descriptor)).toBe(publicKey + 32);
      expect(u16le(descriptor + 2)).toBe(0xffff);
      expect(u16le(descriptor + 4)).toBe(publicKey);
      expect(u16le(descriptor + 6)).toBe(0xffff);
      expect(u16le(descriptor + 8)).toBe(message);
      expect(u16le(descriptor + 10)).toBe(172);
      expect(u16le(descriptor + 12)).toBe(plan.tradingInstructionIndex);
    }
    expect([...evidence.slice(30, 62)]).toEqual([...new PublicKey(seller.maker).toBytes()]);
    expect([...evidence.slice(62, 126)]).toEqual([...seller.signature]);
    expect([...evidence.slice(126, 158)]).toEqual([...new PublicKey(buyer.maker).toBytes()]);
    expect([...evidence.slice(158, 222)]).toEqual([...buyer.signature]);
    expect([...plan.hotInstructionBytes.slice(192, 192 + 172)]).toEqual([...encodeCompactIntentSigningMessageV2(seller.intent)]);
    expect([...plan.hotInstructionBytes.slice(396, 396 + 172)]).toEqual([...encodeCompactIntentSigningMessageV2(buyer.intent)]);
  });

  it('refuses the retired 128-byte-headered Registry container as the current instruction', () => {
    // Mirrors `retired_headered_registry_shape_refuses_without_output_mutation`:
    // the Registry continuation is headerless, so Hot bytes behind a fixed
    // 128-byte header are no longer an authenticated evidence container.
    const candidate = route();
    const { seller, buyer } = participants(candidate.market);
    const plan = compileDirectInlineTransactionV3({ route: candidate, seller, buyer, fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n });
    const headered = new Uint8Array(128 + plan.hotInstructionBytes.length);
    headered.set(plan.hotInstructionBytes, 128);
    expect(() => validateDirectNativeEvidenceInstructionV3(
      new TransactionInstruction({ programId: Ed25519Program.programId, keys: [], data: plan.nativeEvidenceBytes as Buffer }),
      new TransactionInstruction({ programId: new PublicKey(candidate.tradingProgram), keys: [], data: headered as Buffer }),
      plan.tradingInstructionIndex,
      new PublicKey(candidate.tradingProgram),
    )).toThrow(/canonical direct-bias-zero Hot envelope/);
  });

  it('refuses every noncanonical lookup-table shape before transaction construction', () => {
    const candidate = route();
    const { seller, buyer } = participants(candidate.market);
    expect(() => compileDirectInlineTransactionV3({
      route: { ...candidate, lookupTables: [] }, seller, buyer,
      fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n,
    })).toThrow(/exactly one canonical/);
    const substituted = new AddressLookupTableAccount({
      key: new PublicKey(key(89)),
      state: { ...candidate.lookupTables[0].state, addresses: candidate.lookupTables[0].state.addresses.slice(1) },
    });
    expect(() => compileDirectInlineTransactionV3({
      route: { ...candidate, lookupTables: [substituted] }, seller, buyer,
      fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n,
    })).toThrow(/sole canonical address sequence/);
    expect(() => compileDirectInlineTransactionV3({
      route: { ...candidate, selectedProgramSchema: new Uint8Array(32).fill(44) }, seller, buyer,
      fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n,
    })).toThrow(/does not select CapabilityProgramV4/);
  });

  it('refuses AccountProfile privilege substitution and intent over-width coordinates', () => {
    const candidate = route();
    expect(() => validateRuntimeAccountProfileV2(candidate.accountProfile, candidate.outcomeCount, [
      account(key(11), false), account(key(12)), account(key(13)), account(key(14)), account(key(15)),
    ])).toThrow(/privilege/);
    expect(() => validateRuntimeAccountProfileV2(candidate.accountProfile, candidate.outcomeCount, [
      account(key(11), true), account(key(12)), account(key(13)), account(key(14)), account(key(15)),
    ], [new Uint8Array([0x42]), new Uint8Array(1), new Uint8Array(1), new Uint8Array(1), new Uint8Array(1)])).toThrow(/data prestate/);
    const { seller, buyer } = participants(candidate.market);
    expect(() => previewDirectInlineV3({ ...candidate, outcomeCount: 70_000 }, seller, buyer, 2_000n, 500_000n, 1_000n)).toThrow(/seller intent/);
  });
});
