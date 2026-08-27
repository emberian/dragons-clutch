import {
  AddressLookupTableAccount,
  Ed25519Program,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SYSVAR_RENT_PUBKEY,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import {
  COMPACT_INTENT_BYTES_V2,
  COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2,
  COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2,
  COMPACT_INTENT_GENERATION_OFFSET_V2,
  COMPACT_INTENT_LIFECYCLE_OFFSET_V2,
  COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2,
  COMPACT_INTENT_MAGIC_OFFSET_V2,
  COMPACT_INTENT_MAGIC_V2,
  COMPACT_INTENT_MARKET_OFFSET_V2,
  COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2,
  COMPACT_INTENT_NONCE_OFFSET_V2,
  COMPACT_INTENT_OUTCOME_OFFSET_V2,
  COMPACT_INTENT_SIDE_OFFSET_V2,
  COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2,
  COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2,
  COMPACT_INTENT_VALID_FROM_OFFSET_V2,
  COMPACT_INTENT_VALID_THROUGH_OFFSET_V2,
  COMPACT_INTENT_VERSION_V2,
  CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
  DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3,
  DIRECT_EXECUTION_REQUEST_MAGIC_V3,
  DIRECT_EXECUTION_REQUEST_VERSION_V3,
  DIRECT_INLINE_ORDINARY_ACTION_V3,
  DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3,
  DIRECT_NATIVE_EVIDENCE_BUYER_MAKER_OFFSET_V3,
  DIRECT_NATIVE_EVIDENCE_BUYER_MESSAGE_OFFSET_V3,
  DIRECT_NATIVE_EVIDENCE_BYTES_V3,
  DIRECT_NATIVE_EVIDENCE_DESCRIPTOR_BYTES_V3,
  DIRECT_NATIVE_EVIDENCE_DIRECT_BIAS_V3,
  DIRECT_NATIVE_EVIDENCE_HEADER_BYTES_V3,
  DIRECT_NATIVE_EVIDENCE_PARTICIPANT_BYTES_V3,
  DIRECT_NATIVE_EVIDENCE_SELLER_MAKER_OFFSET_V3,
  DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3,
  DIRECT_NATIVE_EVIDENCE_SIGNATURE_COUNT_V3,
  DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
  FIXED_DATA_PREDICATE_ACCOUNT_OFFSET_V2,
  FIXED_DATA_PREDICATE_ARTIFACT_PROFILE,
  FIXED_DATA_PREDICATE_BYTES,
  FIXED_DATA_PREDICATE_COUNT_OFFSET,
  FIXED_DATA_PREDICATE_DATA_OFFSET_V2,
  FIXED_DATA_PREDICATE_DYNAMIC_SPAN_COUNT_OFFSET,
  FIXED_DATA_PREDICATE_HEADER_BYTES,
  FIXED_DATA_PREDICATE_HEADER_RESERVED_OFFSET,
  FIXED_DATA_PREDICATE_OPCODE_OFFSET_V2,
  FIXED_DATA_PREDICATE_PAYLOAD_OFFSET_V2,
  FIXED_DATA_PREDICATE_REQUIRE_U16,
  FIXED_DATA_PREDICATE_REQUIRE_U32,
  FIXED_DATA_PREDICATE_REQUIRE_U64,
  FIXED_DATA_PREDICATE_REQUIRE_U8,
  FIXED_DATA_PREDICATE_REQUIRE_ZERO_RANGE,
  FIXED_DATA_PREDICATE_RESERVED_OFFSET_V2,
  HEADER_BYTES,
  HOT_EXECUTION_ENVELOPE_BYTES_V3,
  HOT_EXECUTION_MAGIC_V3,
  HOT_EXECUTION_PROFILE_V3,
  HOT_EXECUTION_VERSION_V3,
  HOT_FIXED_ACCOUNT_COUNT_V3,
  HOT_CONFIG_RAW_ACCOUNT_V3,
  HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
  HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
  HOT_MARKET_ACCOUNT_V3,
  HOT_PORTFOLIO_RAW_ACCOUNT_V3,
  HOT_PRODUCT_RAW_ACCOUNT_V3,
  HOT_RENT_SYSVAR_ACCOUNT_V3,
  HOT_ROOT_ACCOUNT_V3,
  HOT_TRADING_PROGRAM_ACCOUNT_V3,
  RULE_BYTES,
} from './generated/directInlineV3';
import { PACKET_DATA_SIZE } from './directTransaction';

const MAX_U64 = 18_446_744_073_709_551_615n;
const MAX_U16 = 0xffff;

export type CompactIntentV2Input = Readonly<{
  side: 0 | 1;
  lifecycle: 0 | 1;
  outcome: number;
  market: string;
  generation: bigint;
  nonce: bigint;
  validFrom: bigint;
  validThrough: bigint;
  maximumFill: bigint;
  limitPrice: bigint;
  feeBasisPoints: number;
  collateralAccount: string;
}>;

export type SignedDirectIntentV3 = Readonly<{
  maker: string;
  signature: Uint8Array;
  intent: CompactIntentV2Input;
}>;

export type DirectHotAccountMetaV3 = Readonly<{
  address: string;
  isSigner: boolean;
  isWritable: boolean;
  executable: boolean;
}>;

export type CheckedHotOuterEvidenceV3 = Readonly<{
  status: 'checked';
  tradingArtifactRelease: string;
  checkedManifestDigest: string;
}> | Readonly<{
  status: 'unavailable';
  reason: string;
}>;

export type DirectInlineHotRouteV3 = Readonly<{
  payer: string;
  tradingProgram: string;
  market: string;
  releaseSet: Uint8Array;
  generation: bigint;
  rootPrestateDigest: Uint8Array;
  outcomeCount: number;
  priceScale: bigint;
  feeBasisPoints: number;
  accountProfile: Uint8Array;
  selectedProgramSchema: Uint8Array;
  selectedProgram: Uint8Array;
  fixedAccounts: ReadonlyArray<DirectHotAccountMetaV3>;
  strategyAccounts: ReadonlyArray<DirectHotAccountMetaV3>;
  runtimeAccounts: ReadonlyArray<DirectHotAccountMetaV3>;
  recentBlockhash: string;
  lookupTables: ReadonlyArray<AddressLookupTableAccount>;
  outerEvidence: CheckedHotOuterEvidenceV3;
}>;

export type DirectInlineEconomicPreviewV3 = Readonly<{
  fill: bigint;
  executionPrice: bigint;
  grossCollateral: bigint;
  sellerFee: bigint;
  buyerFee: bigint;
  sellerNetCollateralCredit: bigint;
  buyerCollateralDebit: bigint;
  totalFeeTransfer: bigint;
}>;

export type DirectInlineTransactionPlanV3 = Readonly<{
  requestBytes: Uint8Array;
  hotInstructionBytes: Uint8Array;
  nativeEvidenceBytes: Uint8Array;
  nativeEvidenceInstructionIndex: number;
  tradingInstructionIndex: number;
  nativeMessageOffsets: ReadonlyArray<number>;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
  preview: DirectInlineEconomicPreviewV3;
  loadedAddresses: number;
}>;

function compareKeys(left: PublicKey, right: PublicKey): number {
  const a = left.toBytes();
  const b = right.toBytes();
  for (let index = 0; index < 32; index += 1) {
    const order = (a[index] ?? 0) - (b[index] ?? 0);
    if (order !== 0) return order;
  }
  return 0;
}

/** The sole sorted, duplicate-free LUT sequence accepted by the Rust operator. */
export function canonicalDirectInlineLookupAddressesV3(
  route: Pick<DirectInlineHotRouteV3, 'payer' | 'tradingProgram' | 'fixedAccounts' | 'strategyAccounts' | 'runtimeAccounts'>,
): ReadonlyArray<PublicKey> {
  const payer = exactKey(route.payer, 'payer');
  const programs = new Set([Ed25519Program.programId.toBase58(), exactKey(route.tradingProgram, 'Trading program').toBase58()]);
  const signers = new Set([payer.toBase58()]);
  for (const account of [...route.fixedAccounts, ...route.strategyAccounts, ...route.runtimeAccounts]) {
    if (account.isSigner) signers.add(exactKey(account.address, 'Hot instruction signer').toBase58());
  }
  const byAddress = new Map<string, PublicKey>();
  for (const [index, account] of [...route.fixedAccounts, ...route.strategyAccounts, ...route.runtimeAccounts].entries()) {
    const address = exactKey(account.address, `Hot account ${index}`);
    const text = address.toBase58();
    if (!signers.has(text) && !programs.has(text)) byAddress.set(text, address);
  }
  const addresses = [...byAddress.values()].sort(compareKeys);
  if (addresses.length === 0 || addresses.length > 256) throw new Error('Direct InlineOrdinary canonical lookup sequence is empty or exceeds 256 addresses');
  return Object.freeze(addresses);
}

function validateCanonicalLookupTableV3(route: DirectInlineHotRouteV3): void {
  if (route.lookupTables.length !== 1) throw new Error('Direct InlineOrdinary requires exactly one canonical finalized lookup table');
  const observed = route.lookupTables[0];
  if (observed === undefined || !observed.isActive()) throw new Error('Direct InlineOrdinary lookup table is absent or deactivated');
  const expected = canonicalDirectInlineLookupAddressesV3(route);
  if (observed.state.addresses.length !== expected.length
      || observed.state.addresses.some((address, index) => !address.equals(expected[index] as PublicKey))) {
    throw new Error('Direct InlineOrdinary lookup table differs from the sole canonical address sequence');
  }
}

function exactKey(value: string, field: string): PublicKey {
  const key = new PublicKey(value);
  if (key.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return key;
}

function exactU64(value: bigint, field: string): bigint {
  if (value < 0n || value > MAX_U64) throw new Error(`${field} is outside u64`);
  return value;
}

function exactIdentity(value: Uint8Array, field: string): Uint8Array {
  if (value.length !== 32 || value.every((byte) => byte === 0)) throw new Error(`${field} must be one nonzero 32-byte identity`);
  return new Uint8Array(value);
}

function exactSignature(value: Uint8Array, field: string): Uint8Array {
  if (value.length !== 64 || value.every((byte) => byte === 0)) throw new Error(`${field} must be one nonzero 64-byte signature`);
  return new Uint8Array(value);
}

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU32(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 4).setUint32(0, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, exactU64(value, 'u64 field'), true);
}

function readU16(bytes: Uint8Array, offset: number): number {
  if (!Number.isInteger(offset) || offset < 0 || offset + 2 > bytes.length) throw new Error('native evidence u16 coordinate exceeds its exact wire');
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 2).getUint16(0, true);
}

function readU32(bytes: Uint8Array, offset: number): number {
  if (!Number.isInteger(offset) || offset < 0 || offset + 4 > bytes.length) throw new Error('Direct instruction u32 coordinate exceeds its exact wire');
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

export function encodeCompactIntentV2(input: CompactIntentV2Input): Uint8Array {
  if (!Number.isInteger(input.outcome) || input.outcome < 0 || input.outcome > 0xffff_ffff) throw new Error('outcome is outside the runtime u32 coordinate');
  if (input.side !== 0 && input.side !== 1) throw new Error('side must be Sell 0 or Buy 1');
  if (input.lifecycle !== 0 && input.lifecycle !== 1) throw new Error('inline lifecycle must be FOK 0 or IOC 1');
  if (!Number.isInteger(input.feeBasisPoints) || input.feeBasisPoints < 0 || input.feeBasisPoints > 10_000) throw new Error('fee basis points are outside 0..10000');
  if (input.validFrom > input.validThrough || input.maximumFill === 0n || input.limitPrice === 0n) throw new Error('intent slot interval, fill, or price is noncanonical');
  const output = new Uint8Array(COMPACT_INTENT_BYTES_V2);
  output.set(COMPACT_INTENT_MAGIC_V2, COMPACT_INTENT_MAGIC_OFFSET_V2);
  putU16(output, 8, COMPACT_INTENT_VERSION_V2);
  output[COMPACT_INTENT_SIDE_OFFSET_V2] = input.side;
  output[COMPACT_INTENT_LIFECYCLE_OFFSET_V2] = input.lifecycle;
  putU32(output, COMPACT_INTENT_OUTCOME_OFFSET_V2, input.outcome);
  output.set(exactKey(input.market, 'intent Market').toBytes(), COMPACT_INTENT_MARKET_OFFSET_V2);
  putU64(output, COMPACT_INTENT_GENERATION_OFFSET_V2, input.generation);
  putU64(output, COMPACT_INTENT_NONCE_OFFSET_V2, input.nonce);
  putU64(output, COMPACT_INTENT_VALID_FROM_OFFSET_V2, input.validFrom);
  putU64(output, COMPACT_INTENT_VALID_THROUGH_OFFSET_V2, input.validThrough);
  putU64(output, COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2, input.maximumFill);
  putU64(output, COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2, input.limitPrice);
  putU16(output, COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2, input.feeBasisPoints);
  output.set(exactKey(input.collateralAccount, 'intent collateral account').toBytes(), COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2);
  return output;
}

export function encodeCompactIntentSigningMessageV2(input: CompactIntentV2Input): Uint8Array {
  const output = new Uint8Array(COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2);
  output.set(COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2, 0);
  output.set(encodeCompactIntentV2(input), 32);
  return output;
}

export function encodeDirectInlineOrdinaryRequestV3(
  seller: SignedDirectIntentV3,
  buyer: SignedDirectIntentV3,
  fill: bigint,
  executionPrice: bigint,
): Uint8Array {
  if (seller.maker === buyer.maker) throw new Error('seller and buyer maker identities must differ');
  exactSignature(seller.signature, 'seller signature');
  exactSignature(buyer.signature, 'buyer signature');
  if (fill === 0n || executionPrice === 0n) throw new Error('fill and execution price must be positive');
  const output = new Uint8Array(DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3);
  output.set(DIRECT_EXECUTION_REQUEST_MAGIC_V3, 0);
  putU16(output, 8, DIRECT_EXECUTION_REQUEST_VERSION_V3);
  putU32(output, 12, DIRECT_INLINE_ORDINARY_ACTION_V3);
  putU32(output, 16, output.length - DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3);
  let offset = DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3;
  output.set(exactKey(seller.maker, 'seller maker').toBytes(), offset);
  output.set(encodeCompactIntentSigningMessageV2(seller.intent), offset + 32);
  offset += DIRECT_SIGNED_PARTICIPANT_BYTES_V3;
  output.set(exactKey(buyer.maker, 'buyer maker').toBytes(), offset);
  output.set(encodeCompactIntentSigningMessageV2(buyer.intent), offset + 32);
  offset += DIRECT_SIGNED_PARTICIPANT_BYTES_V3;
  putU64(output, offset, fill);
  putU64(output, offset + 8, executionPrice);
  return output;
}

type DirectNativeParticipantV3 = Readonly<{
  makerOffset: number;
  messageOffset: number;
  signature: Uint8Array;
  field: string;
}>;

function directNativeParticipantsV3(
  currentInstruction: TransactionInstruction,
  expectedTradingProgram: PublicKey,
  sellerSignature: Uint8Array,
  buyerSignature: Uint8Array,
): ReadonlyArray<DirectNativeParticipantV3> {
  if (!currentInstruction.programId.equals(expectedTradingProgram)) {
    throw new Error('native evidence current instruction is not the authenticated Trading program');
  }
  const bytes = new Uint8Array(currentInstruction.data);
  if (DIRECT_NATIVE_EVIDENCE_DIRECT_BIAS_V3 !== 0
      || bytes.length !== HOT_EXECUTION_ENVELOPE_BYTES_V3 + DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3
      || !same(bytes.slice(0, 8), HOT_EXECUTION_MAGIC_V3)
      || readU16(bytes, 8) !== HOT_EXECUTION_VERSION_V3
      || readU16(bytes, 10) !== HOT_EXECUTION_PROFILE_V3
      || readU32(bytes, 12) !== DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3
      || bytes.slice(120, HOT_EXECUTION_ENVELOPE_BYTES_V3).some((value) => value !== 0)) {
    throw new Error('native evidence current instruction is not one canonical direct-bias-zero Hot envelope');
  }
  const request = HOT_EXECUTION_ENVELOPE_BYTES_V3;
  if (!same(bytes.slice(request, request + 8), DIRECT_EXECUTION_REQUEST_MAGIC_V3)
      || readU16(bytes, request + 8) !== DIRECT_EXECUTION_REQUEST_VERSION_V3
      || readU16(bytes, request + 10) !== 0
      || readU32(bytes, request + 12) !== DIRECT_INLINE_ORDINARY_ACTION_V3
      || readU32(bytes, request + 16) !== DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3 - DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3
      || bytes.slice(request + 20, request + DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3).some((value) => value !== 0)) {
    throw new Error('native evidence current instruction does not contain canonical InlineOrdinary request bytes');
  }
  const participants = Object.freeze([
    Object.freeze({ makerOffset: DIRECT_NATIVE_EVIDENCE_SELLER_MAKER_OFFSET_V3, messageOffset: DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3, signature: exactSignature(sellerSignature, 'seller signature'), field: 'seller' }),
    Object.freeze({ makerOffset: DIRECT_NATIVE_EVIDENCE_BUYER_MAKER_OFFSET_V3, messageOffset: DIRECT_NATIVE_EVIDENCE_BUYER_MESSAGE_OFFSET_V3, signature: exactSignature(buyerSignature, 'buyer signature'), field: 'buyer' }),
  ]);
  for (const participant of participants) {
    if (participant.messageOffset + COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2 > bytes.length
        || bytes.slice(participant.makerOffset, participant.makerOffset + 32).every((value) => value === 0)
        || bytes.slice(participant.messageOffset, participant.messageOffset + COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2).every((value) => value === 0)) {
      throw new Error(`native evidence ${participant.field} range exceeds or is zero in the authenticated Trading instruction`);
    }
  }
  if (same(bytes.slice(participants[0].makerOffset, participants[0].makerOffset + 32), bytes.slice(participants[1].makerOffset, participants[1].makerOffset + 32))) {
    throw new Error('native evidence seller and buyer identities alias');
  }
  return participants;
}

/** Encode the canonical 222-byte native instruction by reference to the current Trading bytes. */
export function buildDirectNativeEvidenceInstructionV3(
  currentInstruction: TransactionInstruction,
  currentInstructionIndex: number,
  expectedTradingProgram: PublicKey,
  sellerSignature: Uint8Array,
  buyerSignature: Uint8Array,
): TransactionInstruction {
  if (!Number.isInteger(currentInstructionIndex) || currentInstructionIndex < 0 || currentInstructionIndex > MAX_U16) {
    throw new Error('native evidence current instruction index exceeds u16');
  }
  const participants = directNativeParticipantsV3(currentInstruction, expectedTradingProgram, sellerSignature, buyerSignature);
  const output = new Uint8Array(DIRECT_NATIVE_EVIDENCE_BYTES_V3);
  output[0] = DIRECT_NATIVE_EVIDENCE_SIGNATURE_COUNT_V3;
  for (const [index, participant] of participants.entries()) {
    const descriptor = 2 + index * DIRECT_NATIVE_EVIDENCE_DESCRIPTOR_BYTES_V3;
    const publicKey = DIRECT_NATIVE_EVIDENCE_HEADER_BYTES_V3 + index * DIRECT_NATIVE_EVIDENCE_PARTICIPANT_BYTES_V3;
    const signature = publicKey + 32;
    putU16(output, descriptor, signature);
    putU16(output, descriptor + 2, MAX_U16);
    putU16(output, descriptor + 4, publicKey);
    putU16(output, descriptor + 6, MAX_U16);
    putU16(output, descriptor + 8, participant.messageOffset);
    putU16(output, descriptor + 10, COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2);
    putU16(output, descriptor + 12, currentInstructionIndex);
    output.set(currentInstruction.data.slice(participant.makerOffset, participant.makerOffset + 32), publicKey);
    output.set(participant.signature, signature);
  }
  const instruction = new TransactionInstruction({ programId: Ed25519Program.programId, keys: [], data: output as Buffer });
  validateDirectNativeEvidenceInstructionV3(instruction, currentInstruction, currentInstructionIndex, expectedTradingProgram);
  return instruction;
}

/** Hostile-decode one native-evidence instruction against its exact current Trading instruction. */
export function validateDirectNativeEvidenceInstructionV3(
  evidence: TransactionInstruction,
  currentInstruction: TransactionInstruction,
  currentInstructionIndex: number,
  expectedTradingProgram: PublicKey,
): void {
  if (!evidence.programId.equals(Ed25519Program.programId) || evidence.keys.length !== 0) {
    throw new Error('Direct native evidence substitutes the Ed25519 program or account frame');
  }
  if (!Number.isInteger(currentInstructionIndex) || currentInstructionIndex < 0 || currentInstructionIndex > MAX_U16) {
    throw new Error('Direct native evidence current instruction index exceeds u16');
  }
  const participants = directNativeParticipantsV3(currentInstruction, expectedTradingProgram, new Uint8Array(64).fill(1), new Uint8Array(64).fill(1));
  const bytes = new Uint8Array(evidence.data);
  if (bytes.length !== DIRECT_NATIVE_EVIDENCE_BYTES_V3
      || bytes[0] !== DIRECT_NATIVE_EVIDENCE_SIGNATURE_COUNT_V3 || bytes[1] !== 0) {
    throw new Error('Direct native evidence has another count, reserved byte, or exact width');
  }
  for (const [index, participant] of participants.entries()) {
    const descriptor = 2 + index * DIRECT_NATIVE_EVIDENCE_DESCRIPTOR_BYTES_V3;
    const publicKey = DIRECT_NATIVE_EVIDENCE_HEADER_BYTES_V3 + index * DIRECT_NATIVE_EVIDENCE_PARTICIPANT_BYTES_V3;
    const signature = publicKey + 32;
    if (readU16(bytes, descriptor) !== signature
        || readU16(bytes, descriptor + 2) !== MAX_U16
        || readU16(bytes, descriptor + 4) !== publicKey
        || readU16(bytes, descriptor + 6) !== MAX_U16
        || readU16(bytes, descriptor + 8) !== participant.messageOffset
        || readU16(bytes, descriptor + 10) !== COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2
        || readU16(bytes, descriptor + 12) !== currentInstructionIndex) {
      throw new Error(`Direct native evidence descriptor ${index} substitutes an offset or instruction index`);
    }
    if (!same(bytes.slice(publicKey, publicKey + 32), currentInstruction.data.slice(participant.makerOffset, participant.makerOffset + 32))
        || bytes.slice(signature, signature + 64).every((value) => value === 0)) {
      throw new Error(`Direct native evidence participant ${index} substitutes its Trading maker or zero signature`);
    }
  }
}

function validateFixedFrame(route: DirectInlineHotRouteV3): void {
  if (route.fixedAccounts.length !== HOT_FIXED_ACCOUNT_COUNT_V3) throw new Error(`hot route requires exactly ${HOT_FIXED_ACCOUNT_COUNT_V3} fixed accounts`);
  const market = exactKey(route.market, 'Market').toBase58();
  const trading = exactKey(route.tradingProgram, 'Trading program').toBase58();
  if (route.fixedAccounts[HOT_MARKET_ACCOUNT_V3]?.address !== market
      || route.fixedAccounts[HOT_TRADING_PROGRAM_ACCOUNT_V3]?.address !== trading
      || route.fixedAccounts[HOT_RENT_SYSVAR_ACCOUNT_V3]?.address !== SYSVAR_RENT_PUBKEY.toBase58()
      || route.fixedAccounts[HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3]?.address !== SYSVAR_INSTRUCTIONS_PUBKEY.toBase58()) {
    throw new Error('hot fixed-account roles differ from the canonical V3 ABI');
  }
  if (new Set(route.fixedAccounts.map((account) => account.address)).size !== route.fixedAccounts.length) {
    throw new Error('hot fixed-account frame aliases two semantic roles');
  }
  for (const [index, account] of route.fixedAccounts.entries()) {
    exactKey(account.address, `fixed account ${index}`);
    const expectedWritable = index === HOT_ROOT_ACCOUNT_V3;
    if (account.isSigner || account.isWritable !== expectedWritable) throw new Error(`fixed account ${index} has noncanonical signer/writable privilege`);
  }
}

export function validateRuntimeAccountProfileV2(
  profile: Uint8Array,
  outcomeCount: number,
  accounts: ReadonlyArray<DirectHotAccountMetaV3>,
  accountData?: ReadonlyArray<Uint8Array>,
): void {
  if (profile.length < HEADER_BYTES || new TextDecoder('ascii', { fatal: true }).decode(profile.slice(0, 8)) !== 'DCLTAP02') throw new Error('AccountProfile has the wrong V2 magic/width');
  const view = new DataView(profile.buffer, profile.byteOffset, profile.byteLength);
  const artifactProfile = view.getUint16(10, true);
  if (view.getUint16(8, true) !== 2 || ![2, 3, FIXED_DATA_PREDICATE_ARTIFACT_PROFILE].includes(artifactProfile)) throw new Error('AccountProfile header is unsupported or noncanonical');
  if (!Number.isInteger(outcomeCount) || outcomeCount <= 0 || outcomeCount > 0xffff_ffff) throw new Error('outcome count is outside runtime u32');
  const fixed = view.getUint16(12, true);
  const stride = view.getUint16(14, true);
  const fixedOperations = view.getUint16(16, true);
  const itemOperations = view.getUint16(18, true);
  let profileHeader = HEADER_BYTES;
  let predicateCount = 0;
  if (artifactProfile === FIXED_DATA_PREDICATE_ARTIFACT_PROFILE) {
    if (profile.length < FIXED_DATA_PREDICATE_HEADER_BYTES
        || view.getUint16(FIXED_DATA_PREDICATE_DYNAMIC_SPAN_COUNT_OFFSET, true) !== 0
        || profile.slice(FIXED_DATA_PREDICATE_HEADER_RESERVED_OFFSET, FIXED_DATA_PREDICATE_HEADER_BYTES).some((value) => value !== 0)) {
      throw new Error('Profile14 has dynamic spans or noncanonical header bytes');
    }
    predicateCount = view.getUint16(FIXED_DATA_PREDICATE_COUNT_OFFSET, true);
    if (predicateCount === 0) throw new Error('Profile14 has no fixed-data prestate predicates');
    profileHeader = FIXED_DATA_PREDICATE_HEADER_BYTES + predicateCount * FIXED_DATA_PREDICATE_BYTES;
  } else if (view.getUint32(28, true) !== 0) {
    throw new Error('AccountProfile header is unsupported or noncanonical');
  }
  const expectedProfile = profileHeader + (fixed + stride) * RULE_BYTES + (fixedOperations + itemOperations) * 16;
  const expectedAccounts = fixed + stride * outcomeCount;
  if (profile.length !== expectedProfile || accounts.length !== expectedAccounts || (accountData !== undefined && accountData.length !== accounts.length)) throw new Error('AccountProfile or expanded runtime account width differs');
  for (let coordinate = 0; coordinate < accounts.length; coordinate += 1) {
    const rule = coordinate < fixed ? coordinate : fixed + ((coordinate - fixed) % stride);
    const privileges = profile[profileHeader + rule * RULE_BYTES];
    if ((privileges & ~7) !== 0
        || accounts[coordinate].isSigner !== ((privileges & 1) !== 0)
        || accounts[coordinate].isWritable !== ((privileges & 2) !== 0)
        || accounts[coordinate].executable !== ((privileges & 4) !== 0)) {
      throw new Error(`runtime account ${coordinate} differs from its authenticated AccountProfile privilege rule`);
    }
    exactKey(accounts[coordinate].address, `runtime account ${coordinate}`);
  }
  let priorAccount = -1;
  let priorOffset = -1;
  let priorEnd = -1;
  for (let index = 0; index < predicateCount; index += 1) {
    const predicate = FIXED_DATA_PREDICATE_HEADER_BYTES + index * FIXED_DATA_PREDICATE_BYTES;
    const opcode = profile[predicate + FIXED_DATA_PREDICATE_OPCODE_OFFSET_V2] ?? 0;
    const reserved = profile[predicate + FIXED_DATA_PREDICATE_RESERVED_OFFSET_V2] ?? 1;
    const account = view.getUint16(predicate + FIXED_DATA_PREDICATE_ACCOUNT_OFFSET_V2, true);
    const dataOffset = view.getUint32(predicate + FIXED_DATA_PREDICATE_DATA_OFFSET_V2, true);
    const payload = predicate + FIXED_DATA_PREDICATE_PAYLOAD_OFFSET_V2;
    let width: number;
    let expected: Uint8Array | null;
    if (opcode === FIXED_DATA_PREDICATE_REQUIRE_U8) {
      width = 1; expected = profile.slice(payload, payload + width);
      if (profile.slice(payload + width, payload + 8).some((value) => value !== 0)) throw new Error('Profile14 u8 predicate has nonzero inactive payload bytes');
    } else if (opcode === FIXED_DATA_PREDICATE_REQUIRE_U16) {
      width = 2; expected = profile.slice(payload, payload + width);
      if (profile.slice(payload + width, payload + 8).some((value) => value !== 0)) throw new Error('Profile14 u16 predicate has nonzero inactive payload bytes');
    } else if (opcode === FIXED_DATA_PREDICATE_REQUIRE_U32) {
      width = 4; expected = profile.slice(payload, payload + width);
      if (profile.slice(payload + width, payload + 8).some((value) => value !== 0)) throw new Error('Profile14 u32 predicate has nonzero inactive payload bytes');
    } else if (opcode === FIXED_DATA_PREDICATE_REQUIRE_U64) {
      width = 8; expected = profile.slice(payload, payload + width);
    } else if (opcode === FIXED_DATA_PREDICATE_REQUIRE_ZERO_RANGE) {
      width = view.getUint32(payload, true); expected = null;
      if (width === 0 || profile.slice(payload + 4, payload + 8).some((value) => value !== 0)) throw new Error('Profile14 zero-range predicate has a noncanonical width');
    } else {
      throw new Error('Profile14 fixed-data predicate has an unsupported opcode');
    }
    const end = dataOffset + width;
    const ruleOffset = profileHeader + account * RULE_BYTES;
    const ruleDataLength = account < fixed ? view.getUint32(ruleOffset + 8, true) : 0;
    const prestate = profile[ruleOffset + 3] ?? 0xff;
    if (reserved !== 0 || account >= fixed || (prestate !== 0 && prestate !== 1)
        || profile[ruleOffset + 2] !== 0 || view.getUint16(ruleOffset + 4, true) !== 0
        || view.getUint16(ruleOffset + 6, true) !== 0 || view.getUint32(ruleOffset + 12, true) !== 0
        || end > ruleDataLength || end > 0xffff_ffff
        || account < priorAccount || (account === priorAccount && (dataOffset <= priorOffset || dataOffset < priorEnd))) {
      throw new Error('Profile14 fixed-data predicate is not canonical for its exact fixed-account rule');
    }
    const observed = accountData?.[account];
    if (observed !== undefined && !(observed.length === 0 && prestate === 1)) {
      const actual = observed.slice(dataOffset, end);
      const accepted = actual.length === width && (expected === null
        ? actual.every((value) => value === 0)
        : actual.every((value, offset) => value === expected[offset]));
      if (!accepted) throw new Error(`runtime account ${account} violates its authenticated Profile14 data prestate`);
    }
    priorAccount = account;
    priorOffset = dataOffset;
    priorEnd = end;
  }
}

export function previewDirectInlineV3(
  route: Pick<DirectInlineHotRouteV3, 'market' | 'generation' | 'outcomeCount' | 'priceScale' | 'feeBasisPoints'>,
  seller: Readonly<{ intent: CompactIntentV2Input }>,
  buyer: Readonly<{ intent: CompactIntentV2Input }>,
  fill: bigint,
  executionPrice: bigint,
  clockSlot: bigint,
): DirectInlineEconomicPreviewV3 {
  if (route.priceScale <= 0n || route.priceScale > MAX_U64 || route.feeBasisPoints < 0 || route.feeBasisPoints > 10_000) throw new Error('immutable Direct price scale or fee rate is invalid');
  for (const [participant, side] of [[seller, 0], [buyer, 1]] as const) {
    const intent = participant.intent;
    if (intent.side !== side || intent.market !== route.market || intent.generation !== route.generation
        || intent.outcome >= route.outcomeCount || intent.maximumFill < fill
        || intent.feeBasisPoints !== route.feeBasisPoints || clockSlot < intent.validFrom || clockSlot > intent.validThrough
        || (intent.lifecycle === 0 && intent.maximumFill !== fill)) {
      throw new Error(`${side === 0 ? 'seller' : 'buyer'} intent does not admit this exact chain-derived execution`);
    }
  }
  if (seller.intent.outcome !== buyer.intent.outcome || executionPrice < seller.intent.limitPrice
      || executionPrice > buyer.intent.limitPrice || executionPrice > route.priceScale) {
    throw new Error('execution price or outcome does not cross both signed limits');
  }
  const scaled = exactU64(fill, 'fill') * exactU64(executionPrice, 'execution price');
  if (scaled % route.priceScale !== 0n) throw new Error('fill × price is not exactly representable at the immutable price scale');
  const gross = scaled / route.priceScale;
  if (gross > MAX_U64) throw new Error('gross collateral exceeds u64');
  const sellerFee = gross * BigInt(route.feeBasisPoints) / 10_000n;
  const buyerFee = sellerFee;
  return Object.freeze({
    fill,
    executionPrice,
    grossCollateral: gross,
    sellerFee,
    buyerFee,
    sellerNetCollateralCredit: gross - sellerFee,
    buyerCollateralDebit: gross + buyerFee,
    totalFeeTransfer: sellerFee + buyerFee,
  });
}

export function compileDirectInlineTransactionV3(input: Readonly<{
  route: DirectInlineHotRouteV3;
  seller: SignedDirectIntentV3;
  buyer: SignedDirectIntentV3;
  fill: bigint;
  executionPrice: bigint;
  clockSlot: bigint;
}>): DirectInlineTransactionPlanV3 {
  if (input.route.outerEvidence.status !== 'checked') throw new Error(`Direct V3 hot execution unavailable: ${input.route.outerEvidence.reason}`);
  exactIdentity(input.route.releaseSet, 'execution release set');
  exactIdentity(input.route.rootPrestateDigest, 'root prestate digest');
  const selectedProgramSchema = exactIdentity(input.route.selectedProgramSchema, 'selected capability-program schema');
  if (!selectedProgramSchema.every((value, index) => value === CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID[index])) {
    throw new Error('Direct InlineOrdinary route does not select CapabilityProgramV4');
  }
  exactIdentity(input.route.selectedProgram, 'selected CapabilityProgramV4 content');
  if (input.route.accountProfile.length < FIXED_DATA_PREDICATE_HEADER_BYTES
      || new DataView(input.route.accountProfile.buffer, input.route.accountProfile.byteOffset, input.route.accountProfile.byteLength).getUint16(10, true) !== FIXED_DATA_PREDICATE_ARTIFACT_PROFILE) {
    throw new Error('Direct InlineOrdinary requires canonical fixed-data-prestate Profile14');
  }
  validateFixedFrame(input.route);
  validateCanonicalLookupTableV3(input.route);
  validateRuntimeAccountProfileV2(input.route.accountProfile, input.route.outcomeCount, [
    input.route.fixedAccounts[HOT_ROOT_ACCOUNT_V3],
    input.route.fixedAccounts[HOT_CONFIG_RAW_ACCOUNT_V3],
    input.route.fixedAccounts[HOT_PRODUCT_RAW_ACCOUNT_V3],
    input.route.fixedAccounts[HOT_PORTFOLIO_RAW_ACCOUNT_V3],
    input.route.fixedAccounts[HOT_LINKED_BASIS_RAW_ACCOUNT_V3],
    ...input.route.runtimeAccounts,
  ]);
  const preview = previewDirectInlineV3(input.route, input.seller, input.buyer, input.fill, input.executionPrice, input.clockSlot);
  const requestBytes = encodeDirectInlineOrdinaryRequestV3(input.seller, input.buyer, input.fill, input.executionPrice);
  const hotInstructionBytes = new Uint8Array(HOT_EXECUTION_ENVELOPE_BYTES_V3 + requestBytes.length);
  hotInstructionBytes.set(HOT_EXECUTION_MAGIC_V3, 0);
  putU16(hotInstructionBytes, 8, HOT_EXECUTION_VERSION_V3);
  putU16(hotInstructionBytes, 10, HOT_EXECUTION_PROFILE_V3);
  putU32(hotInstructionBytes, 12, requestBytes.length);
  hotInstructionBytes.set(input.route.releaseSet, 16);
  hotInstructionBytes.set(exactKey(input.route.market, 'Market').toBytes(), 48);
  putU64(hotInstructionBytes, 80, input.route.generation);
  hotInstructionBytes.set(input.route.rootPrestateDigest, 88);
  hotInstructionBytes.set(requestBytes, HOT_EXECUTION_ENVELOPE_BYTES_V3);

  const toMeta = (account: DirectHotAccountMetaV3) => ({ pubkey: exactKey(account.address, 'hot route account'), isSigner: account.isSigner, isWritable: account.isWritable });
  const trading = new TransactionInstruction({
    programId: exactKey(input.route.tradingProgram, 'Trading program'),
    keys: [
      ...input.route.fixedAccounts.map(toMeta),
      ...input.route.strategyAccounts.map(toMeta),
      ...input.route.runtimeAccounts.map(toMeta),
    ],
    data: hotInstructionBytes as Buffer,
  });
  const instructions: TransactionInstruction[] = [];
  const nativeEvidenceInstructionIndex = instructions.length;
  const tradingInstructionIndex = nativeEvidenceInstructionIndex + 1;
  const nativeEvidence = buildDirectNativeEvidenceInstructionV3(
    trading,
    tradingInstructionIndex,
    trading.programId,
    input.seller.signature,
    input.buyer.signature,
  );
  instructions.push(nativeEvidence, trading);
  if (instructions[tradingInstructionIndex - 1] !== nativeEvidence
      || instructions[tradingInstructionIndex] !== trading) {
    throw new Error('Direct native evidence is not immediately adjacent to its current Trading instruction');
  }
  exactKey(input.route.recentBlockhash, 'recent blockhash');
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: exactKey(input.route.payer, 'payer'),
    recentBlockhash: input.route.recentBlockhash,
    instructions,
  }).compileToV0Message([...input.route.lookupTables]));
  const wireBytes = transaction.serialize();
  if (wireBytes.length > PACKET_DATA_SIZE) throw new Error(`Direct V3 transaction is ${wireBytes.length} bytes, above the ${PACKET_DATA_SIZE}-byte packet bound`);
  const requiredSigners = Object.freeze(transaction.message.staticAccountKeys
    .slice(0, transaction.message.header.numRequiredSignatures)
    .map((key) => key.toBase58()));
  if (requiredSigners.length !== 1 || requiredSigners[0] !== input.route.payer) throw new Error('Direct V3 message requires an unexpected transaction signer');
  return Object.freeze({
    requestBytes,
    hotInstructionBytes,
    nativeEvidenceBytes: new Uint8Array(nativeEvidence.data),
    nativeEvidenceInstructionIndex,
    tradingInstructionIndex,
    nativeMessageOffsets: Object.freeze([
      DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3,
      DIRECT_NATIVE_EVIDENCE_BUYER_MESSAGE_OFFSET_V3,
    ]),
    transaction,
    wireBytes,
    requiredSigners,
    preview,
    loadedAddresses: transaction.message.addressTableLookups.reduce((sum, value) => sum + value.readonlyIndexes.length + value.writableIndexes.length, 0),
  });
}
