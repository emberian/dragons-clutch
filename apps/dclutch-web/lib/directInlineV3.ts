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
  DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3,
  DIRECT_EXECUTION_REQUEST_MAGIC_V3,
  DIRECT_EXECUTION_REQUEST_VERSION_V3,
  DIRECT_INLINE_ORDINARY_ACTION_V3,
  DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3,
  DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
  HEADER_BYTES,
  HOT_EXECUTION_ENVELOPE_BYTES_V3,
  HOT_EXECUTION_MAGIC_V3,
  HOT_EXECUTION_PROFILE_V3,
  HOT_EXECUTION_VERSION_V3,
  HOT_FIXED_ACCOUNT_COUNT_V3,
  HOT_CONFIG_RAW_ACCOUNT_V3,
  HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
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
const ED25519_DESCRIPTOR_BYTES = 14;
const ED25519_HEADER_BYTES = 2 + 2 * ED25519_DESCRIPTOR_BYTES;
const ED25519_PARTICIPANT_BYTES = 96;

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
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
  preview: DirectInlineEconomicPreviewV3;
  loadedAddresses: number;
}>;

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

function nativeEd25519V3(seller: SignedDirectIntentV3, buyer: SignedDirectIntentV3): TransactionInstruction {
  const output = new Uint8Array(ED25519_HEADER_BYTES + 2 * ED25519_PARTICIPANT_BYTES);
  output[0] = 2;
  for (const [index, participant, messageOffset] of [
    [0, seller, HOT_EXECUTION_ENVELOPE_BYTES_V3 + 64],
    [1, buyer, HOT_EXECUTION_ENVELOPE_BYTES_V3 + 268],
  ] as const) {
    const descriptor = 2 + index * ED25519_DESCRIPTOR_BYTES;
    const publicKey = ED25519_HEADER_BYTES + index * ED25519_PARTICIPANT_BYTES;
    const signature = publicKey + 32;
    putU16(output, descriptor, signature);
    putU16(output, descriptor + 2, 0xffff);
    putU16(output, descriptor + 4, publicKey);
    putU16(output, descriptor + 6, 0xffff);
    putU16(output, descriptor + 8, messageOffset);
    putU16(output, descriptor + 10, COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2);
    putU16(output, descriptor + 12, 1);
    output.set(exactKey(participant.maker, `${index === 0 ? 'seller' : 'buyer'} maker`).toBytes(), publicKey);
    output.set(exactSignature(participant.signature, `${index === 0 ? 'seller' : 'buyer'} signature`), signature);
  }
  return new TransactionInstruction({ programId: Ed25519Program.programId, keys: [], data: output as Buffer });
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
): void {
  if (profile.length < HEADER_BYTES || new TextDecoder('ascii', { fatal: true }).decode(profile.slice(0, 8)) !== 'DCLTAP02') throw new Error('AccountProfile has the wrong V2 magic/width');
  const view = new DataView(profile.buffer, profile.byteOffset, profile.byteLength);
  if (view.getUint16(8, true) !== 2 || ![2, 3].includes(view.getUint16(10, true)) || view.getUint32(28, true) !== 0) throw new Error('AccountProfile header is unsupported or noncanonical');
  if (!Number.isInteger(outcomeCount) || outcomeCount <= 0 || outcomeCount > 0xffff_ffff) throw new Error('outcome count is outside runtime u32');
  const fixed = view.getUint16(12, true);
  const stride = view.getUint16(14, true);
  const fixedOperations = view.getUint16(16, true);
  const itemOperations = view.getUint16(18, true);
  const expectedProfile = HEADER_BYTES + (fixed + stride) * RULE_BYTES + (fixedOperations + itemOperations) * 16;
  const expectedAccounts = fixed + stride * outcomeCount;
  if (profile.length !== expectedProfile || accounts.length !== expectedAccounts) throw new Error('AccountProfile or expanded runtime account width differs');
  for (let coordinate = 0; coordinate < accounts.length; coordinate += 1) {
    const rule = coordinate < fixed ? coordinate : fixed + ((coordinate - fixed) % stride);
    const privileges = profile[HEADER_BYTES + rule * RULE_BYTES];
    if ((privileges & ~7) !== 0
        || accounts[coordinate].isSigner !== ((privileges & 1) !== 0)
        || accounts[coordinate].isWritable !== ((privileges & 2) !== 0)
        || accounts[coordinate].executable !== ((privileges & 4) !== 0)) {
      throw new Error(`runtime account ${coordinate} differs from its authenticated AccountProfile privilege rule`);
    }
    exactKey(accounts[coordinate].address, `runtime account ${coordinate}`);
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
  validateFixedFrame(input.route);
  validateRuntimeAccountProfileV2(input.route.accountProfile, input.route.outcomeCount, [
    input.route.fixedAccounts[HOT_ROOT_ACCOUNT_V3],
    input.route.fixedAccounts[HOT_CONFIG_RAW_ACCOUNT_V3],
    input.route.fixedAccounts[HOT_PRODUCT_RAW_ACCOUNT_V3],
    input.route.fixedAccounts[HOT_PORTFOLIO_RAW_ACCOUNT_V3],
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
  exactKey(input.route.recentBlockhash, 'recent blockhash');
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: exactKey(input.route.payer, 'payer'),
    recentBlockhash: input.route.recentBlockhash,
    instructions: [nativeEd25519V3(input.seller, input.buyer), trading],
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
    transaction,
    wireBytes,
    requiredSigners,
    preview,
    loadedAddresses: transaction.message.addressTableLookups.reduce((sum, value) => sum + value.readonlyIndexes.length + value.writableIndexes.length, 0),
  });
}
