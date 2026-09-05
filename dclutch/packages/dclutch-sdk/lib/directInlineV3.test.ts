import {
  AddressLookupTableAccount,
  ComputeBudgetProgram,
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
  DIRECT_INLINE_CURRENT_LOOKUP_ADDRESSES_V3,
  DIRECT_INLINE_CURRENT_RUNTIME_TAIL_ACCOUNTS_V3,
  DIRECT_INLINE_CURRENT_WIRE_BYTES_V3,
  decodeHotBumpHintsV1,
  encodeCompactIntentSigningMessageV2,
  encodeDirectInlineOrdinaryRequestV3,
  encodeHotBumpHintsV1,
  HOT_BUMP_HINTS_ABSENT_V1,
  HOT_BUMP_HINTS_OFFSET_V1,
  HOT_BUMP_HINT_COUNT_V1,
  type HotBumpHintsV1,
  previewDirectInlineV3,
  projectDirectInlineSealedExecutionRouteV3,
  validateDirectInlineInstructionSequenceV3,
  validateDirectNativeEvidenceInstructionV3,
  validateRuntimeAccountProfileV2,
} from './directInlineV3';
import {
  COMPACT_INTENT_OUTCOME_OFFSET_V2,
  COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2,
  CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
  DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3,
  DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3,
  DIRECT_NATIVE_EVIDENCE_BUYER_MAKER_OFFSET_V3,
  DIRECT_NATIVE_EVIDENCE_BUYER_MESSAGE_OFFSET_V3,
  DIRECT_NATIVE_EVIDENCE_DESCRIPTOR_BYTES_V3,
  DIRECT_NATIVE_EVIDENCE_DIRECT_BIAS_V3,
  DIRECT_NATIVE_EVIDENCE_HEADERLESS_REGISTRY_BIAS_V4,
  DIRECT_NATIVE_EVIDENCE_SELLER_MAKER_OFFSET_V3,
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

/**
 * Eight distinct nonzero bumps, standing in for a mined block.
 *
 * The MINING is proven elsewhere and against the other language:
 * `directHotBumpHintsV1.test.ts` reproduces the Rust vector's block byte for
 * byte. What this file owns is the WIRE -- that the block lands at its offset,
 * that filling it moves nothing else, and that the packet does not grow -- so
 * these are inputs rather than answers, and distinct so a swapped slot shows.
 */
const MINED: HotBumpHintsV1 = Object.freeze({
  market: 254,
  root: 253,
  lifecycle: Object.freeze([252, 251] as const),
  childCaller: Object.freeze([250, 249] as const),
  childRelay: Object.freeze([248, 247] as const),
});

function key(seed: number): string {
  return new PublicKey(new Uint8Array(32).fill(seed)).toBase58();
}

function intent(side: 0 | 1, market: string, collateral: string, outcome = 70_000): CompactIntentV2Input {
  return Object.freeze({
    side,
    lifecycle: 0,
    outcome,
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

function participants(market: string, outcome = 70_000): Readonly<{ seller: SignedDirectIntentV3; buyer: SignedDirectIntentV3 }> {
  return Object.freeze({
    seller: Object.freeze({ maker: key(2), signature: new Uint8Array(64).fill(11), intent: intent(0, market, key(3), outcome) }),
    buyer: Object.freeze({ maker: key(4), signature: new Uint8Array(64).fill(12), intent: intent(1, market, key(5), outcome) }),
  });
}

function runtimeProfile(): Uint8Array {
  const fixedAccounts = 5 + DIRECT_INLINE_CURRENT_RUNTIME_TAIL_ACCOUNTS_V3;
  const predicateBytes = 16;
  const profileHeader = 48 + predicateBytes;
  const output = new Uint8Array(profileHeader + fixedAccounts * 16);
  output.set(new TextEncoder().encode('DCLTAP02'), 0);
  const view = new DataView(output.buffer);
  view.setUint16(8, 2, true);
  view.setUint16(10, 14, true);
  view.setUint16(12, fixedAccounts, true);
  view.setUint16(14, 0, true);
  view.setUint16(20, 1, true);
  view.setUint16(42, 1, true);
  output[48] = 1;
  output[56] = 0x41;
  const writableRules = new Set([0, 5, 6, 7, 8, 12, 28, 29, 33, 35, 36, 41]);
  const executableRules = new Set([9, 10, 21, 22, 24, 26, 38, 43]);
  for (let accountIndex = 0; accountIndex < fixedAccounts; accountIndex += 1) {
    output[profileHeader + accountIndex * 16] = (accountIndex === 6 ? 1 : 0)
      | (writableRules.has(accountIndex) ? 2 : 0)
      | (executableRules.has(accountIndex) ? 4 : 0);
    view.setUint32(profileHeader + accountIndex * 16 + 8, 1, true);
  }
  return output;
}

function account(address: string, isWritable = false, executable = false, isSigner = false): DirectHotAccountMetaV3 {
  return Object.freeze({ address, isSigner, isWritable, executable });
}

function containsSlice(bytes: Uint8Array, needle: Uint8Array): boolean {
  for (let offset = 0; offset + needle.length <= bytes.length; offset += 1) {
    if (needle.every((value, index) => bytes[offset + index] === value)) return true;
  }
  return false;
}

function route(checked = true, productionGeometry = false): DirectInlineHotRouteV3 {
  const market = key(10);
  const fixed = Array.from({ length: HOT_FIXED_ACCOUNT_COUNT_V3 }, (_, index) => account(key(20 + index)));
  fixed[HOT_MARKET_ACCOUNT_V3] = account(market);
  fixed[HOT_ROOT_ACCOUNT_V3] = account(key(11), true);
  fixed[HOT_TRADING_PROGRAM_ACCOUNT_V3] = account(key(12), false, true);
  fixed[HOT_RENT_SYSVAR_ACCOUNT_V3] = account(SYSVAR_RENT_PUBKEY.toBase58());
  fixed[HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3] = account(SYSVAR_INSTRUCTIONS_PUBKEY.toBase58());
  const runtimeFixed = new Map([
    [8, 37], [9, 31], [10, 32], [11, 33], [12, 35], [13, 28], [14, 0],
    [15, 22], [16, 27], [17, 25], [18, 26], [21, 23], [22, 24],
  ]);
  const writable = new Set([0, 1, 2, 3, 7, 23, 24, 28, 30, 31, 36]);
  const executable = new Set([4, 5, 16, 17, 19, 21, 33, 38]);
  const runtimeAccounts = Object.freeze(Array.from({ length: DIRECT_INLINE_CURRENT_RUNTIME_TAIL_ACCOUNTS_V3 }, (_, index) => {
    const address = index === 1 ? key(91) : runtimeFixed.has(index) ? fixed[runtimeFixed.get(index)!]!.address : key(100 + index);
    return account(address, writable.has(index), executable.has(index), index === 1);
  }));
  const routeAccounts = Object.freeze({
    payer: key(91),
    tradingProgram: key(12),
    fixedAccounts: Object.freeze(fixed),
    strategyAccounts: Object.freeze([]),
    runtimeAccounts,
  });
  const projected = projectDirectInlineSealedExecutionRouteV3(Object.freeze({
    ...routeAccounts,
    market,
    releaseSet: new Uint8Array(32).fill(31),
    generation: 19n,
    rootPrestateDigest: new Uint8Array(32).fill(32),
    outcomeCount: productionGeometry ? 51 : 70_001,
    priceScale: 1_000_000n,
    feeBasisPoints: 25,
    accountProfile: runtimeProfile(),
    selectedProgramSchema: CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
    selectedProgram: new Uint8Array(32).fill(33),
    observedSlot: 1_000n,
    recentBlockhash: key(92),
    blockhashObservedSlot: 1_001n,
    lastValidBlockHeight: 2_000n,
    lookupTableCreationSlot: 700n,
    lookupTables: Object.freeze([]),
    outerEvidence: checked
      ? Object.freeze({ status: 'checked' as const, tradingArtifactRelease: '11'.repeat(32), checkedManifestDigest: '12'.repeat(32) })
      : Object.freeze({ status: 'unavailable' as const, reason: 'common hot outer has no accepted checked release' }),
  }));
  const lookupTable = new AddressLookupTableAccount({
    key: new PublicKey(key(90)),
    state: {
      deactivationSlot: MAX_U64,
      lastExtendedSlot: 800,
      lastExtendedSlotStartIndex: 0,
      authority: undefined,
      addresses: [...canonicalDirectInlineLookupAddressesV3(projected)],
    },
  });
  return Object.freeze({ ...projected, lookupTables: Object.freeze([lookupTable]) });
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

  it('the wallet wire carries the caller-mined hint block and does not grow by a byte', () => {
    const candidate = route(true, true);
    const { seller, buyer } = participants(candidate.market, 0);
    const plan = compileDirectInlineTransactionV3({
      route: candidate, seller, buyer, fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n,
      bumpHints: MINED,
    });
    expect([...plan.hotInstructionBytes.slice(HOT_BUMP_HINTS_OFFSET_V1, HOT_BUMP_HINTS_OFFSET_V1 + HOT_BUMP_HINT_COUNT_V1)])
      .toEqual([...encodeHotBumpHintsV1(MINED)]);
    expect(plan.bumpHints).toEqual(MINED);
    expect(plan.minedBumpHintSlots).toBe(8);
    // The block is inside the envelope, before the family request, which is why
    // filling it moves no digest and no signed message -- and why the packet is
    // still the exact 1,167 bytes the devnet driver and the release preflight
    // both pin. Every geometry pin below is the unhinted one, unchanged.
    expect(HOT_BUMP_HINTS_OFFSET_V1 + HOT_BUMP_HINT_COUNT_V1).toBe(HOT_EXECUTION_ENVELOPE_BYTES_V3);
    expect(plan.wireBytes).toHaveLength(DIRECT_INLINE_CURRENT_WIRE_BYTES_V3);
    expect(plan.wireBytes.length).toBeLessThanOrEqual(1_232);
    expect(plan.transaction.message.staticAccountKeys).toHaveLength(4);
    expect(plan.loadedAddresses).toBe(DIRECT_INLINE_CURRENT_LOOKUP_ADDRESSES_V3);
    expect(plan.transaction.message.compiledInstructions[plan.tradingInstructionIndex]?.accountKeyIndexes).toHaveLength(78);
  });

  it('an omitted hint block is the absent wire, which searches exactly as it used to', () => {
    // Backward compatibility, executed. Every caller written before this block
    // existed emits eight zeros, and eight zeros is ABSENT rather than a value:
    // Trading falls back to find_program_address for every address the block
    // could have named. No account, no market and no caller needs migrating.
    const candidate = route(true, true);
    const { seller, buyer } = participants(candidate.market, 0);
    const plan = compileDirectInlineTransactionV3({ route: candidate, seller, buyer, fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n });
    expect([...plan.hotInstructionBytes.slice(HOT_BUMP_HINTS_OFFSET_V1, HOT_BUMP_HINTS_OFFSET_V1 + HOT_BUMP_HINT_COUNT_V1)]).toEqual([0, 0, 0, 0, 0, 0, 0, 0]);
    expect(plan.bumpHints).toEqual(HOT_BUMP_HINTS_ABSENT_V1);
    expect(plan.minedBumpHintSlots).toBe(0);
    expect(plan.wireBytes).toHaveLength(DIRECT_INLINE_CURRENT_WIRE_BYTES_V3);
  });

  it('a hint moves the eight envelope bytes and nothing else the wire signs', () => {
    // The safety argument for taking eight bytes from a stranger, executed
    // rather than argued. If a hint could reach the family request it would
    // move the parent request digest, every child caller authority derived from
    // it, and the two maker Ed25519 windows -- so a hinted trade would not be
    // the same trade. Both wires are compiled from the same inputs here, and
    // the ONLY difference is the block itself.
    const candidate = route(true, true);
    const { seller, buyer } = participants(candidate.market, 0);
    const common = { route: candidate, seller, buyer, fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n } as const;
    const bare = compileDirectInlineTransactionV3(common);
    const hinted = compileDirectInlineTransactionV3({ ...common, bumpHints: MINED });
    expect(hinted.hotInstructionBytes).toHaveLength(bare.hotInstructionBytes.length);
    const moved = [...bare.hotInstructionBytes].flatMap((value, offset) => value === hinted.hotInstructionBytes[offset] ? [] : [offset]);
    expect(moved.every((offset) => offset >= HOT_BUMP_HINTS_OFFSET_V1 && offset < HOT_EXECUTION_ENVELOPE_BYTES_V3)).toBe(true);
    expect(moved.length).toBe(HOT_BUMP_HINT_COUNT_V1);
    expect([...hinted.requestBytes]).toEqual([...bare.requestBytes]);
    // The native evidence is byte-identical: its maker and message coordinates
    // are ABSOLUTE offsets at or past HOT_FAMILY_REQUEST_OFFSET_V3, which the
    // block sits before and therefore cannot move.
    expect([...hinted.nativeEvidenceBytes]).toEqual([...bare.nativeEvidenceBytes]);
    expect(hinted.nativeMessageOffsets).toEqual(bare.nativeMessageOffsets);
    expect(hinted.wireBytes).toHaveLength(bare.wireBytes.length);
    expect(hinted.preview).toEqual(bare.preview);
  });

  it('a hinted Trading instruction still passes its own evidence and sequence validators', () => {
    // This row exists because the check it replaces was the bug. The builder
    // used to require the eight bytes to be ZERO before it would encode
    // evidence for a wire, which meant a mined wire could not pass the encoder
    // that was supposed to authenticate it. The Rust codec never had that
    // check: `split_instruction` reads magic, version, profile and request
    // width, and treats the hint span as data.
    const candidate = route();
    const { seller, buyer } = participants(candidate.market);
    const plan = compileDirectInlineTransactionV3({
      route: candidate, seller, buyer, fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n,
      bumpHints: MINED,
    });
    const trading = new TransactionInstruction({
      programId: new PublicKey(candidate.tradingProgram), keys: [], data: plan.hotInstructionBytes as Buffer,
    });
    const evidence = new TransactionInstruction({
      programId: Ed25519Program.programId, keys: [], data: plan.nativeEvidenceBytes as Buffer,
    });
    expect(() => validateDirectNativeEvidenceInstructionV3(
      evidence, trading, plan.tradingInstructionIndex, new PublicKey(candidate.tradingProgram),
    )).not.toThrow();
    expect(() => validateDirectInlineInstructionSequenceV3([
      ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }),
      ComputeBudgetProgram.requestHeapFrame({ bytes: 65_536 }),
      evidence, trading,
    ], new PublicKey(candidate.tradingProgram))).not.toThrow();
    // And the container check the relaxation must NOT have weakened: Hot bytes
    // behind a 128-byte header are still not an authenticated container.
    const headered = new Uint8Array(128 + plan.hotInstructionBytes.length);
    headered.set(plan.hotInstructionBytes, 128);
    expect(() => validateDirectNativeEvidenceInstructionV3(
      evidence,
      new TransactionInstruction({ programId: new PublicKey(candidate.tradingProgram), keys: [], data: headered as Buffer }),
      plan.tradingInstructionIndex, new PublicKey(candidate.tradingProgram),
    )).toThrow(/canonical direct-bias-zero Hot envelope/);
  });

  it('refuses a hint block that is not exactly eight one-byte slots', () => {
    const candidate = route(true, true);
    const { seller, buyer } = participants(candidate.market, 0);
    const common = { route: candidate, seller, buyer, fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n } as const;
    // A slot outside a byte is the wrong-sized block a hand-built caller
    // actually produces: the block is fixed-width, so an over-wide slot has
    // nowhere to go and must refuse before it truncates into its neighbour.
    for (const hostile of [
      { ...MINED, market: 256 },
      { ...MINED, root: -1 },
      { ...MINED, lifecycle: [1.5, 2] as const },
      { ...MINED, childRelay: [1, 0x1_00] as const },
    ]) {
      expect(() => compileDirectInlineTransactionV3({ ...common, bumpHints: hostile })).toThrow(/not one byte/);
    }
    // A short or long TUPLE is a type error in TypeScript and a decode refusal
    // on the wire, so the decoder owns that half.
    expect(() => decodeHotBumpHintsV1(new Uint8Array(HOT_BUMP_HINT_COUNT_V1 - 1))).toThrow(/not the exact 8/);
    expect(() => decodeHotBumpHintsV1(new Uint8Array(HOT_BUMP_HINT_COUNT_V1 + 1))).toThrow(/not the exact 8/);
  });

  it('compiles the exact adjacent Ed25519 + Trading v0 batch and fails closed without a checked outer', () => {
    const candidate = route(true, true);
    const { seller, buyer } = participants(candidate.market, 0);
    const plan = compileDirectInlineTransactionV3({ route: candidate, seller, buyer, fill: 2_000n, executionPrice: 500_000n, clockSlot: 1_000n });
    expect(plan.hotInstructionBytes).toHaveLength(HOT_EXECUTION_ENVELOPE_BYTES_V3 + DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3);
    expect(plan.requiredSigners).toEqual([candidate.payer]);
    expect(plan.transaction.message.compiledInstructions).toHaveLength(4);
    expect(plan.nativeEvidenceBytes).toHaveLength(158);
    expect(plan.nativeEvidenceInstructionIndex).toBe(2);
    expect(plan.tradingInstructionIndex).toBe(3);
    expect(plan.nativeMessageOffsets).toEqual([
      DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3,
      DIRECT_NATIVE_EVIDENCE_BUYER_MESSAGE_OFFSET_V3,
    ]);
    expect(containsSlice(plan.nativeEvidenceBytes, encodeCompactIntentSigningMessageV2(seller.intent))).toBe(false);
    expect(containsSlice(plan.nativeEvidenceBytes, encodeCompactIntentSigningMessageV2(buyer.intent))).toBe(false);
    expect(plan.wireBytes).toHaveLength(DIRECT_INLINE_CURRENT_WIRE_BYTES_V3);
    // 65, not 73: the top-level route now carries RequestHeapFrame, which costs
    // exactly 8 packet bytes (program index, empty account vector, five data
    // bytes). ComputeBudget was already a static key, so no account is added.
    expect(1_232 - plan.wireBytes.length).toBe(65);
    expect(plan.loadedAddresses).toBe(DIRECT_INLINE_CURRENT_LOOKUP_ADDRESSES_V3);
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
    const compute = ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 });
    const heap = ComputeBudgetProgram.requestHeapFrame({ bytes: 65_536 });
    expect(() => validateDirectInlineInstructionSequenceV3(
      [compute, heap, evidence, trading], new PublicKey(candidate.tradingProgram),
    )).not.toThrow();
    expect(() => validateDirectInlineInstructionSequenceV3(
      [compute, heap, trading, evidence], new PublicKey(candidate.tradingProgram),
    )).toThrow(/immediately adjacent/);
    expect(() => validateDirectInlineInstructionSequenceV3(
      [evidence, trading], new PublicKey(candidate.tradingProgram),
    )).toThrow(/exactly ComputeBudget/);
    expect(() => validateDirectInlineInstructionSequenceV3(
      [ComputeBudgetProgram.setComputeUnitLimit({ units: 1_399_999 }), heap, evidence, trading],
      new PublicKey(candidate.tradingProgram),
    )).toThrow(/SetComputeUnitLimit/);
    // The grant is not merely present, it is the exact size the program was
    // built for. A smaller frame compiles, submits, and is refused on chain.
    expect(() => validateDirectInlineInstructionSequenceV3(
      [compute, ComputeBudgetProgram.requestHeapFrame({ bytes: 32_768 }), evidence, trading],
      new PublicKey(candidate.tradingProgram),
    )).toThrow(/RequestHeapFrame/);
    // Omitting it entirely is the mistake a caller written against the old
    // three-instruction shape actually makes.
    expect(() => validateDirectInlineInstructionSequenceV3(
      [compute, evidence, trading], new PublicKey(candidate.tradingProgram),
    )).toThrow(/exactly ComputeBudget, RequestHeapFrame/);

    const offset = plan.nativeEvidenceBytes.slice();
    new DataView(offset.buffer).setUint16(2 + 8, DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3 - 1, true);
    expect(() => validateDirectNativeEvidenceInstructionV3(
      new TransactionInstruction({ programId: Ed25519Program.programId, keys: [], data: offset as Buffer }),
      trading, plan.tradingInstructionIndex, new PublicKey(candidate.tradingProgram),
    )).toThrow(/offset or instruction index/);

    const publicKeyOffset = plan.nativeEvidenceBytes.slice();
    new DataView(publicKeyOffset.buffer).setUint16(2 + 4, DIRECT_NATIVE_EVIDENCE_SELLER_MAKER_OFFSET_V3 - 1, true);
    expect(() => validateDirectNativeEvidenceInstructionV3(
      new TransactionInstruction({ programId: Ed25519Program.programId, keys: [], data: publicKeyOffset as Buffer }),
      trading, plan.tradingInstructionIndex, new PublicKey(candidate.tradingProgram),
    )).toThrow(/offset or instruction index/);

    const publicKeyInstructionIndex = plan.nativeEvidenceBytes.slice();
    new DataView(publicKeyInstructionIndex.buffer).setUint16(2 + 6, 0xffff, true);
    expect(() => validateDirectNativeEvidenceInstructionV3(
      new TransactionInstruction({ programId: Ed25519Program.programId, keys: [], data: publicKeyInstructionIndex as Buffer }),
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

  it('pins the exact compact bias-zero native-evidence wire against the Rust codec vector', () => {
    // Expected bytes are the vector asserted by
    // crates/dclutch-trading/src/native_evidence_v3.rs, test
    // `direct_and_headerless_registry_use_exact_current_instruction_offsets`:
    // 158-byte evidence, Trading-referenced maker/message offsets, self-contained
    // signatures, and
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
    expect(evidence).toHaveLength(158);
    expect(evidence[0]).toBe(2);
    expect(evidence[1]).toBe(0);
    for (const [descriptor, signature, maker, message] of [
      [2, 30, DIRECT_NATIVE_EVIDENCE_SELLER_MAKER_OFFSET_V3, 192],
      [16, 94, DIRECT_NATIVE_EVIDENCE_BUYER_MAKER_OFFSET_V3, 396],
    ]) {
      expect(u16le(descriptor)).toBe(signature);
      expect(u16le(descriptor + 2)).toBe(0xffff);
      expect(u16le(descriptor + 4)).toBe(maker);
      expect(u16le(descriptor + 6)).toBe(plan.tradingInstructionIndex);
      expect(u16le(descriptor + 8)).toBe(message);
      expect(u16le(descriptor + 10)).toBe(172);
      expect(u16le(descriptor + 12)).toBe(plan.tradingInstructionIndex);
    }
    expect([...evidence.slice(30, 94)]).toEqual([...seller.signature]);
    expect([...evidence.slice(94, 158)]).toEqual([...buyer.signature]);
    expect([...plan.hotInstructionBytes.slice(DIRECT_NATIVE_EVIDENCE_SELLER_MAKER_OFFSET_V3, DIRECT_NATIVE_EVIDENCE_SELLER_MAKER_OFFSET_V3 + 32)]).toEqual([...new PublicKey(seller.maker).toBytes()]);
    expect([...plan.hotInstructionBytes.slice(DIRECT_NATIVE_EVIDENCE_BUYER_MAKER_OFFSET_V3, DIRECT_NATIVE_EVIDENCE_BUYER_MAKER_OFFSET_V3 + 32)]).toEqual([...new PublicKey(buyer.maker).toBytes()]);
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
    const logical = [
      candidate.fixedAccounts[HOT_ROOT_ACCOUNT_V3]!, candidate.fixedAccounts[8]!, candidate.fixedAccounts[30]!, candidate.fixedAccounts[34]!, candidate.fixedAccounts[36]!,
      ...candidate.runtimeAccounts,
    ];
    expect(() => validateRuntimeAccountProfileV2(candidate.accountProfile, candidate.outcomeCount, [
      account(logical[0]!.address, false), ...logical.slice(1),
    ])).toThrow(/privilege/);
    expect(() => validateRuntimeAccountProfileV2(candidate.accountProfile, candidate.outcomeCount, logical,
      logical.map((_, index) => new Uint8Array([index === 0 ? 0x42 : 0])))).toThrow(/data prestate/);
    const { seller, buyer } = participants(candidate.market);
    expect(() => previewDirectInlineV3({ ...candidate, outcomeCount: 70_000 }, seller, buyer, 2_000n, 500_000n, 1_000n)).toThrow(/seller intent/);
  });
});
