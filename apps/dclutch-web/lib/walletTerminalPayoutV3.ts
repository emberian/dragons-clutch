import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  PublicKey,
  SYSVAR_RENT_PUBKEY,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { fromHex, hex, isZero, sha256, slice, u16, u64 } from './bytes';
import {
  CALLER_AUTHORITY_PDA_DOMAIN_V1,
  CUSTODY_ABI_VERSION_V1,
  CUSTODY_COMPARTMENT_EXTERNAL_V1,
  CUSTODY_COMPARTMENT_HOARD_PRINCIPAL_V1,
  CUSTODY_OPERATION_TRANSFER_V1,
  CUSTODY_REPLAY_BYTES_V1,
  CUSTODY_REPLAY_CALLER_PROGRAM_OFFSET_V1,
  CUSTODY_REPLAY_CALLER_ROLE_OFFSET_V1,
  CUSTODY_REPLAY_CONTEXT_OFFSET_V1,
  CUSTODY_REPLAY_GENERATION_OFFSET_V1,
  CUSTODY_REPLAY_MAGIC_V1,
  CUSTODY_REPLAY_MARKET_OFFSET_V1,
  CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1,
  CUSTODY_REPLAY_PDA_DOMAIN_V1,
  CUSTODY_REPLAY_REALM_OFFSET_V1,
  CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1,
  CUSTODY_REPLAY_STATUS_OFFSET_V1,
  CUSTODY_REPLAY_VERSION_OFFSET_V1,
  CUSTODY_REQUEST_AMOUNT_OFFSET_V1,
  CUSTODY_REQUEST_BYTES_V1,
  CUSTODY_REQUEST_CALLER_PROGRAM_OFFSET_V1,
  CUSTODY_REQUEST_CALLER_ROLE_OFFSET_V1,
  CUSTODY_REQUEST_CANDIDATE_OFFSET_V1,
  CUSTODY_REQUEST_CONTEXT_OFFSET_V1,
  CUSTODY_REQUEST_DESTINATION_COMPARTMENT_OFFSET_V1,
  CUSTODY_REQUEST_DESTINATION_OFFSET_V1,
  CUSTODY_REQUEST_DESTINATION_OWNER_OFFSET_V1,
  CUSTODY_REQUEST_EXPECTED_REVISION_OFFSET_V1,
  CUSTODY_REQUEST_GENERATION_OFFSET_V1,
  CUSTODY_REQUEST_MAGIC_V1,
  CUSTODY_REQUEST_MARKET_OFFSET_V1,
  CUSTODY_REQUEST_MINT_OFFSET_V1,
  CUSTODY_REQUEST_OPERATION_OFFSET_V1,
  CUSTODY_REQUEST_ORDER_NONCE_OFFSET_V1,
  CUSTODY_REQUEST_PARENT_REQUEST_DIGEST_OFFSET_V1,
  CUSTODY_REQUEST_RELEASE_SET_OFFSET_V1,
  CUSTODY_REQUEST_REALM_OFFSET_V1,
  CUSTODY_REQUEST_RESULTING_REVISION_OFFSET_V1,
  CUSTODY_REQUEST_SOURCE_COMPARTMENT_OFFSET_V1,
  CUSTODY_REQUEST_SOURCE_OFFSET_V1,
  CUSTODY_REQUEST_SOURCE_VAULT_CONTEXT_OFFSET_V1,
  CUSTODY_REQUEST_TOKEN_PROGRAM_OFFSET_V1,
  CUSTODY_REQUEST_TRANSFER_INDEX_OFFSET_V1,
  CUSTODY_REQUEST_VERSION_OFFSET_V1,
  EXECUTION_ROLE_CLAIMS_V1,
} from './generated/claimsCustodyReplayV1';
import {
  CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
  CUSTODY_COMPARTMENT_HOARD_PRINCIPAL_TAG_V1,
  CUSTODY_VAULT_PDA_DOMAIN_V1,
  GRADED_BASIS_RECORD_SCHEMA_ID_V3,
  LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
  LIABILITY_BASIS_MARKET_REVISION_OFFSET,
  LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
  LIABILITY_BASIS_POSITION_REVISION_OFFSET,
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  REALM_SCHEMA_RELEASE_ID_V1,
  RESULT_DOMAIN_SCHEMA_ID_V2,
} from './generated/coreFound';
import {
  CUSTODY_POSTSTATE_DOMAIN_V1,
  LIABILITY_BASIS_HEADER_RESERVED_BYTES_V2,
  LIABILITY_BASIS_HEADER_RESERVED_OFFSET_V2,
  LIABILITY_BASIS_POSITION_RESERVED_OFFSET_V2,
  SIGNED_DELTA_BASIS_OFFSET_V3,
  SIGNED_DELTA_BYTES_V3,
  SIGNED_DELTA_CALLER_ROLE_OFFSET_V3,
  SIGNED_DELTA_CLAIM_COUNT_OFFSET_V3,
  SIGNED_DELTA_DIRECTION_OFFSET_V3,
  SIGNED_DELTA_HEADER_RESERVED_OFFSET_V3,
  SIGNED_DELTA_HEADER_TAIL_RESERVED_OFFSET_V3,
  SIGNED_DELTA_LINKED_BASIS_OFFSET_V3,
  SIGNED_DELTA_MAGNITUDE_OFFSET_V3,
  SIGNED_DELTA_MARKET_OFFSET_V3,
  SIGNED_DELTA_MARKET_REVISION_OFFSET_V3,
  SIGNED_DELTA_PLAN_HEADER_BYTES_V3,
  SIGNED_DELTA_PLAN_MAGIC_V3,
  SIGNED_DELTA_POSITION_BYTES_V3,
  SIGNED_DELTA_POSITION_COUNT_OFFSET_V3,
  SIGNED_DELTA_POSITION_DELTA_COUNT_OFFSET_V3,
  SIGNED_DELTA_POSITION_OWNER_OFFSET_V3,
  SIGNED_DELTA_POSITION_REVISION_OFFSET_V3,
  SIGNED_DELTA_POST_RESOURCE_DIGEST_DOMAIN_V3,
  SIGNED_DELTA_PRODUCT_OFFSET_V3,
  SIGNED_DELTA_RELEASE_SET_OFFSET_V3,
  SIGNED_DELTA_REQUEST_OFFSET_V3,
  SIGNED_DELTA_RESERVED_OFFSET_V3,
  SIGNED_DELTA_ROW_BYTES_V3,
  SIGNED_DELTA_ROW_DELTA_OFFSET_V3,
  SIGNED_DELTA_ROW_OUTCOME_OFFSET_V3,
  SIGNED_DELTA_ROW_POSITION_INDEX_OFFSET_V3,
  SIGNED_DELTA_TABLE_DIGEST_DOMAIN_V3,
  SIGNED_DELTA_VERSION_OFFSET_V3,
  SIGNED_DELTA_WIRE_VERSION_V3,
  TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3,
  TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3,
  TERMINAL_SETTLEMENT_CERTIFICATE_ACCOUNT_V3,
  TERMINAL_SETTLEMENT_CLAIM_INDEX_OFFSET_V3,
  TERMINAL_SETTLEMENT_CLAIMS_PROGRAM_OFFSET_V3,
  TERMINAL_SETTLEMENT_COLLATERAL_MINT_OFFSET_V3,
  TERMINAL_SETTLEMENT_CUSTODY_PROGRAM_OFFSET_V3,
  TERMINAL_SETTLEMENT_CUSTODY_REVISION_OFFSET_V3,
  TERMINAL_SETTLEMENT_EXPOSURE_DIGEST_OFFSET_V3,
  TERMINAL_SETTLEMENT_EXPOSURE_ID_OFFSET_V3,
  TERMINAL_SETTLEMENT_GENERATION_OFFSET_V3,
  TERMINAL_SETTLEMENT_LINKED_BASIS_OFFSET_V3,
  TERMINAL_SETTLEMENT_MARKET_OFFSET_V3,
  TERMINAL_SETTLEMENT_MARKET_REVISION_OFFSET_V3,
  TERMINAL_SETTLEMENT_OWNER_OFFSET_V3,
  TERMINAL_SETTLEMENT_PARENT_CONTEXT_OFFSET_V3,
  TERMINAL_SETTLEMENT_POSITION_OFFSET_V3,
  TERMINAL_SETTLEMENT_POSITION_REVISION_OFFSET_V3,
  TERMINAL_SETTLEMENT_POST_RESOURCE_DOMAIN_V3,
  TERMINAL_SETTLEMENT_PRODUCT_RECORD_OFFSET_V3,
  TERMINAL_SETTLEMENT_QUANTITY_OFFSET_V3,
  TERMINAL_SETTLEMENT_REALM_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_BYTES_V3,
  TERMINAL_SETTLEMENT_RECEIPT_CUSTODY_RECEIPT_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_CUSTODY_REPLAY_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_CUSTODY_REQUEST_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_CUSTODY_TOKEN_POST_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_MAGIC_V3,
  TERMINAL_SETTLEMENT_RECEIPT_PAYOUT_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_POST_CUSTODY_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_POST_MARKET_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_POST_POSITION_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_POST_RESOURCE_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_PRE_CUSTODY_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_PRE_MARKET_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_PRE_POSITION_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_REQUEST_DIGEST_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_REQUEST_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_RESERVED_BYTES_V3,
  TERMINAL_SETTLEMENT_RECEIPT_RESERVED_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_SIGNED_PACKET_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_SIGNED_POST_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_SIGNED_TABLE_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECEIPT_TAIL_RESERVED_BYTES_V3,
  TERMINAL_SETTLEMENT_RECEIPT_TAIL_RESERVED_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECIPIENT_OWNER_OFFSET_V3,
  TERMINAL_SETTLEMENT_RECIPIENT_TOKEN_OFFSET_V3,
  TERMINAL_SETTLEMENT_RELEASE_OFFSET_V3,
  TERMINAL_SETTLEMENT_RESOLUTION_PROGRAM_ACCOUNT_V3,
  TERMINAL_SETTLEMENT_RESOLUTION_PROGRAMDATA_ACCOUNT_V3,
  TERMINAL_SETTLEMENT_REQUEST_BYTES_V3,
  TERMINAL_SETTLEMENT_REQUEST_MAGIC_V3,
  TERMINAL_SETTLEMENT_ROLE_OFFSET_V3,
  TERMINAL_SETTLEMENT_SEMANTIC_BASIS_OFFSET_V3,
  TERMINAL_SETTLEMENT_TERMINAL_RECORD_OFFSET_V3,
  TERMINAL_SETTLEMENT_TOKEN_POSTSTATE_DOMAIN_V3,
  TERMINAL_SETTLEMENT_TOKEN_PROGRAM_OFFSET_V3,
  TERMINAL_SETTLEMENT_TRANSFER_INDEX_OFFSET_V3,
  TERMINAL_SETTLEMENT_VERSION_V3,
  TOKEN_ACCOUNT_AMOUNT_OFFSET_V1,
  TOKEN_ACCOUNT_BYTES_V1,
  TOKEN_ACCOUNT_MINT_OFFSET_V1,
  TOKEN_ACCOUNT_OWNER_OFFSET_V1,
  TOKEN_ACCOUNT_STATE_OFFSET_V1,
  WALLET_TERMINAL_MAGIC_OFFSET_V3,
  WALLET_TERMINAL_VERSION_OFFSET_V3,
} from './generated/walletTerminalPayoutV3';
import {
  decodeClaimsAggregateV2,
  decodeClaimsPositionV2,
  deriveClaimsAggregateAddressV2,
  deriveClaimsPositionAddressV2,
} from './marketCoreV2';
import { decodeCoreFoundProductGraphV2 } from './coreFound';
import {
  acquireOperatorSurfaceV1,
  LIVE_DEVNET_OPERATOR_PRESET_V1,
} from './operatorSurface';
import { decodeRealmRecordV1 } from './realmRecord';
import {
  authenticateFinalizedRationalHotRecordV4,
  authenticateRationalHotCoreV3,
  authenticateRationalProductBasisRecordV3,
} from './rationalRetireReceiptV4';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import {
  bindTerminalResolutionCertificateV2,
  decodeResolutionCertificateV2,
} from './resolutionCertificateV2';
import { type RpcAccount, type SolanaRpcClient, type TransactionMetaObservation } from './rpc';

const U64_MAX = 0xffff_ffff_ffff_ffffn;
const PACKET_BYTES = 1_232;
const REQUEST_BYTES = TERMINAL_SETTLEMENT_REQUEST_BYTES_V3;
const RECEIPT_BYTES = TERMINAL_SETTLEMENT_RECEIPT_BYTES_V3;
const REQUEST_MAGIC = TERMINAL_SETTLEMENT_REQUEST_MAGIC_V3;
const RECEIPT_MAGIC = TERMINAL_SETTLEMENT_RECEIPT_MAGIC_V3;
const SIGNED_MAGIC = SIGNED_DELTA_PLAN_MAGIC_V3;
const VERSION = TERMINAL_SETTLEMENT_VERSION_V3;
const ACCOUNT_COUNT = TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3;
const TOKEN_ACCOUNT_BYTES = TOKEN_ACCOUNT_BYTES_V1;
const TOKEN_AMOUNT_OFFSET = TOKEN_ACCOUNT_AMOUNT_OFFSET_V1;
const TOKEN_STATE_OFFSET = TOKEN_ACCOUNT_STATE_OFFSET_V1;
const LEGACY_TOKEN_PROGRAM = 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA';
const TOKEN_2022_PROGRAM = 'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb';
const UPGRADEABLE_LOADER = new PublicKey('BPFLoaderUpgradeab1e11111111111111111111111');
const REPLAY_LAST_REQUEST_OFFSET = CUSTODY_REPLAY_GENERATION_OFFSET_V1 + 8;
const REPLAY_LAST_POSTSTATE_OFFSET = REPLAY_LAST_REQUEST_OFFSET + 32;
const COMPOSITION_EXPOSURE_SCHEMA_PREIMAGE_V3 = new TextEncoder().encode(
  'dclutch/schema/product-representation-exposure-bundle-v3',
);
const COMPOSITION_EXPOSURE_HEADER_BYTES_V3 = 304;

const SIGNED_TABLE_DOMAIN = SIGNED_DELTA_TABLE_DIGEST_DOMAIN_V3;
const SIGNED_POST_DOMAIN = SIGNED_DELTA_POST_RESOURCE_DIGEST_DOMAIN_V3;
const CANDIDATE_DOMAIN = TERMINAL_SETTLEMENT_CANDIDATE_DOMAIN_V3;
const TOKEN_POST_DOMAIN = TERMINAL_SETTLEMENT_TOKEN_POSTSTATE_DOMAIN_V3;
const TERMINAL_POST_DOMAIN = TERMINAL_SETTLEMENT_POST_RESOURCE_DOMAIN_V3;
const CUSTODY_POST_DOMAIN = CUSTODY_POSTSTATE_DOMAIN_V1;

const REQUEST_OFFSETS = Object.freeze({
  role: TERMINAL_SETTLEMENT_ROLE_OFFSET_V3, releaseSet: TERMINAL_SETTLEMENT_RELEASE_OFFSET_V3,
  market: TERMINAL_SETTLEMENT_MARKET_OFFSET_V3, realm: TERMINAL_SETTLEMENT_REALM_OFFSET_V3,
  parentContext: TERMINAL_SETTLEMENT_PARENT_CONTEXT_OFFSET_V3, productRecordDigest: TERMINAL_SETTLEMENT_PRODUCT_RECORD_OFFSET_V3,
  exposureId: TERMINAL_SETTLEMENT_EXPOSURE_ID_OFFSET_V3, exposureDigest: TERMINAL_SETTLEMENT_EXPOSURE_DIGEST_OFFSET_V3,
  terminalRecordDigest: TERMINAL_SETTLEMENT_TERMINAL_RECORD_OFFSET_V3, owner: TERMINAL_SETTLEMENT_OWNER_OFFSET_V3,
  position: TERMINAL_SETTLEMENT_POSITION_OFFSET_V3, recipientOwner: TERMINAL_SETTLEMENT_RECIPIENT_OWNER_OFFSET_V3,
  recipient: TERMINAL_SETTLEMENT_RECIPIENT_TOKEN_OFFSET_V3, claimsProgram: TERMINAL_SETTLEMENT_CLAIMS_PROGRAM_OFFSET_V3,
  custodyProgram: TERMINAL_SETTLEMENT_CUSTODY_PROGRAM_OFFSET_V3, collateralMint: TERMINAL_SETTLEMENT_COLLATERAL_MINT_OFFSET_V3,
  tokenProgram: TERMINAL_SETTLEMENT_TOKEN_PROGRAM_OFFSET_V3, semanticBasisId: TERMINAL_SETTLEMENT_SEMANTIC_BASIS_OFFSET_V3,
  linkedBasisRecordDigest: TERMINAL_SETTLEMENT_LINKED_BASIS_OFFSET_V3, generation: TERMINAL_SETTLEMENT_GENERATION_OFFSET_V3,
  marketRevision: TERMINAL_SETTLEMENT_MARKET_REVISION_OFFSET_V3, positionRevision: TERMINAL_SETTLEMENT_POSITION_REVISION_OFFSET_V3,
  custodyRevision: TERMINAL_SETTLEMENT_CUSTODY_REVISION_OFFSET_V3, quantity: TERMINAL_SETTLEMENT_QUANTITY_OFFSET_V3,
  claimIndex: TERMINAL_SETTLEMENT_CLAIM_INDEX_OFFSET_V3, transferIndex: TERMINAL_SETTLEMENT_TRANSFER_INDEX_OFFSET_V3,
});

const RECEIPT_OFFSETS = Object.freeze({
  request: TERMINAL_SETTLEMENT_RECEIPT_REQUEST_OFFSET_V3, requestDigest: TERMINAL_SETTLEMENT_RECEIPT_REQUEST_DIGEST_OFFSET_V3,
  signedPacket: TERMINAL_SETTLEMENT_RECEIPT_SIGNED_PACKET_OFFSET_V3, signedTable: TERMINAL_SETTLEMENT_RECEIPT_SIGNED_TABLE_OFFSET_V3,
  signedPost: TERMINAL_SETTLEMENT_RECEIPT_SIGNED_POST_OFFSET_V3, custodyRequest: TERMINAL_SETTLEMENT_RECEIPT_CUSTODY_REQUEST_OFFSET_V3,
  custodyReceipt: TERMINAL_SETTLEMENT_RECEIPT_CUSTODY_RECEIPT_OFFSET_V3, custodyReplay: TERMINAL_SETTLEMENT_RECEIPT_CUSTODY_REPLAY_OFFSET_V3,
  custodyTokenPost: TERMINAL_SETTLEMENT_RECEIPT_CUSTODY_TOKEN_POST_OFFSET_V3, postResource: TERMINAL_SETTLEMENT_RECEIPT_POST_RESOURCE_OFFSET_V3,
  payout: TERMINAL_SETTLEMENT_RECEIPT_PAYOUT_OFFSET_V3, preMarket: TERMINAL_SETTLEMENT_RECEIPT_PRE_MARKET_OFFSET_V3,
  postMarket: TERMINAL_SETTLEMENT_RECEIPT_POST_MARKET_OFFSET_V3, prePosition: TERMINAL_SETTLEMENT_RECEIPT_PRE_POSITION_OFFSET_V3,
  postPosition: TERMINAL_SETTLEMENT_RECEIPT_POST_POSITION_OFFSET_V3, preCustody: TERMINAL_SETTLEMENT_RECEIPT_PRE_CUSTODY_OFFSET_V3,
  postCustody: TERMINAL_SETTLEMENT_RECEIPT_POST_CUSTODY_OFFSET_V3,
});

function concatenate(parts: ReadonlyArray<Uint8Array>): Uint8Array {
  const output = new Uint8Array(parts.reduce((total, part) => total + part.length, 0));
  let offset = 0;
  for (const part of parts) { output.set(part, offset); offset += part.length; }
  return output;
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function key(value: string, field: string): PublicKey {
  let parsed: PublicKey;
  try { parsed = new PublicKey(value); } catch { throw new Error(`${field} must be canonical base58 text`); }
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  if (isZero(parsed.toBytes())) throw new Error(`${field} is the all-zero identity`);
  return parsed;
}

function decimal(value: string, field: string, nonzero = false): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} must be canonical unsigned decimal text`);
  const parsed = BigInt(value);
  if (parsed > U64_MAX || (nonzero && parsed === 0n)) throw new Error(`${field} is outside its exact u64 range`);
  return parsed;
}

function putU16(output: Uint8Array, offset: number, value: number): void {
  new DataView(output.buffer, output.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU32(output: Uint8Array, offset: number, value: number): void {
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) throw new Error('u32 coordinate is outside its exact range');
  new DataView(output.buffer, output.byteOffset + offset, 4).setUint32(0, value, true);
}

function putU64(output: Uint8Array, offset: number, value: bigint): void {
  if (value < 0n || value > U64_MAX) throw new Error('u64 field is outside its exact range');
  new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function u32At(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
}

function requireZero(bytes: Uint8Array, offset: number, width: number, field: string): void {
  if (!isZero(slice(bytes, offset, width))) throw new Error(`${field} contains noncanonical reserved bytes`);
}

async function hashv(...parts: ReadonlyArray<Uint8Array>): Promise<Uint8Array> {
  return sha256(concatenate(parts));
}

export type WalletTerminalPayoutRouteV3 = Readonly<{
  aggregate: string; linkedBasisRaw: string; linkedBasisStaging: string;
  productRaw: string; productStaging: string; resultDomainRaw: string;
  resultDomainStaging: string; portfolioRaw: string; portfolioStaging: string;
  market: string; activationCache: string; registryProgram: string;
  claimsProgram: string; claimsProgramData: string; coreProgram: string;
  coreProgramData: string; resolutionProgram: string; resolutionProgramData: string;
  position: string; exposureRaw: string; exposureStaging: string;
  custodyProgram: string; terminalCertificate: string;
  realmRaw: string; realmStaging: string;
  custodyReplay: string; collateralMint: string; hoard: string; recipient: string;
  custodyAuthority: string; tokenProgram: string;
}>;

export type WalletTerminalPayoutRequestV3 = Readonly<{
  releaseSet: string; market: string; realm: string; parentContext: string;
  productRecordDigest: string; exposureId: string; exposureDigest: string;
  terminalRecordDigest: string; owner: string; position: string;
  recipientOwner: string; recipient: string; claimsProgram: string;
  custodyProgram: string; collateralMint: string; tokenProgram: string;
  semanticBasisId: string; linkedBasisRecordDigest: string; generation: string;
  expectedMarketRevision: string; expectedPositionRevision: string;
  expectedCustodyRevision: string; quantity: string; claimIndex: number;
  transferIndex: number;
}>;

export type WalletTerminalPayoutBuildInputV3 = Readonly<{
  observedSlot: string;
  route: WalletTerminalPayoutRouteV3;
  custodyContext: string;
  request: WalletTerminalPayoutRequestV3;
  signedPacket: Uint8Array;
  payout: string;
  aggregateBytes: Uint8Array;
  positionBytes: Uint8Array;
  custodyReplayBytes: Uint8Array;
  hoardTokenBytes: Uint8Array;
  recipientTokenBytes: Uint8Array;
}>;

export type WalletTerminalPayoutReportV3 = Readonly<{
  observedSlot: string;
  route: WalletTerminalPayoutRouteV3;
  request: WalletTerminalPayoutRequestV3;
  requestBytes: Uint8Array;
  requestDigest: Uint8Array;
  signedPacket: Uint8Array;
  signedPacketDigest: Uint8Array;
  signedTableDigest: Uint8Array;
  payout: string;
  custodyCaller: string;
  custodyRequestDigest: Uint8Array;
  instruction: TransactionInstruction;
  preAggregateBytes: Uint8Array;
  prePositionBytes: Uint8Array;
  preCustodyReplayBytes: Uint8Array;
  preHoardTokenBytes: Uint8Array;
  preRecipientTokenBytes: Uint8Array;
}>;

function requestIdentity(value: string, field: string): Uint8Array {
  const bytes = fromHex(value, field);
  if (isZero(bytes)) throw new Error(`${field} is the all-zero identity`);
  return bytes;
}

/** Encode the byte-identical top-level Claims request accepted by Rust. */
export function encodeWalletTerminalPayoutRequestV3(input: WalletTerminalPayoutRequestV3): Uint8Array {
  const output = new Uint8Array(REQUEST_BYTES);
  output.set(REQUEST_MAGIC, 0);
  putU16(output, 8, VERSION);
  output[REQUEST_OFFSETS.role] = EXECUTION_ROLE_CLAIMS_V1;
  const addressFields = [
    [REQUEST_OFFSETS.market, input.market, 'Market'],
    [REQUEST_OFFSETS.owner, input.owner, 'Position owner'],
    [REQUEST_OFFSETS.position, input.position, 'Position'],
    [REQUEST_OFFSETS.recipientOwner, input.recipientOwner, 'recipient owner'],
    [REQUEST_OFFSETS.recipient, input.recipient, 'recipient token account'],
    [REQUEST_OFFSETS.claimsProgram, input.claimsProgram, 'Claims program'],
    [REQUEST_OFFSETS.custodyProgram, input.custodyProgram, 'Custody program'],
    [REQUEST_OFFSETS.collateralMint, input.collateralMint, 'collateral Mint'],
    [REQUEST_OFFSETS.tokenProgram, input.tokenProgram, 'Token program'],
  ] as const;
  for (const [offset, value, field] of addressFields) output.set(key(value, field).toBytes(), offset);
  const identityFields = [
    [REQUEST_OFFSETS.releaseSet, input.releaseSet, 'release set'],
    [REQUEST_OFFSETS.realm, input.realm, 'Realm'],
    [REQUEST_OFFSETS.parentContext, input.parentContext, 'parent context'],
    [REQUEST_OFFSETS.productRecordDigest, input.productRecordDigest, 'Product record digest'],
    [REQUEST_OFFSETS.exposureId, input.exposureId, 'exposure identity'],
    [REQUEST_OFFSETS.exposureDigest, input.exposureDigest, 'exposure digest'],
    [REQUEST_OFFSETS.terminalRecordDigest, input.terminalRecordDigest, 'terminal record digest'],
    [REQUEST_OFFSETS.semanticBasisId, input.semanticBasisId, 'semantic basis identity'],
    [REQUEST_OFFSETS.linkedBasisRecordDigest, input.linkedBasisRecordDigest, 'linked basis digest'],
  ] as const;
  for (const [offset, value, field] of identityFields) output.set(requestIdentity(value, field), offset);
  if (input.owner === input.position || input.recipientOwner === input.recipient || input.claimsProgram === input.custodyProgram) {
    throw new Error('terminal payout request aliases identities Rust requires distinct');
  }
  putU64(output, REQUEST_OFFSETS.generation, decimal(input.generation, 'generation'));
  const revisions = [
    decimal(input.expectedMarketRevision, 'expected Market revision'),
    decimal(input.expectedPositionRevision, 'expected Position revision'),
    decimal(input.expectedCustodyRevision, 'expected Custody revision'),
  ] as const;
  if (revisions.includes(U64_MAX)) throw new Error('a payout pre-revision cannot advance exactly once');
  putU64(output, REQUEST_OFFSETS.marketRevision, revisions[0]);
  putU64(output, REQUEST_OFFSETS.positionRevision, revisions[1]);
  putU64(output, REQUEST_OFFSETS.custodyRevision, revisions[2]);
  putU64(output, REQUEST_OFFSETS.quantity, decimal(input.quantity, 'quantity', true));
  putU32(output, REQUEST_OFFSETS.claimIndex, input.claimIndex);
  if (!Number.isSafeInteger(input.transferIndex) || input.transferIndex < 0 || input.transferIndex > 0xffff) throw new Error('transfer index is outside u16');
  putU16(output, REQUEST_OFFSETS.transferIndex, input.transferIndex);
  return output;
}

function validateSignedPacket(packet: Uint8Array, request: WalletTerminalPayoutRequestV3, requestDigest: Uint8Array): Readonly<{ positions: Uint8Array; aggregates: Uint8Array; deltas: Uint8Array }> {
  if (packet.length < SIGNED_DELTA_PLAN_HEADER_BYTES_V3 || !same(slice(packet, WALLET_TERMINAL_MAGIC_OFFSET_V3, SIGNED_MAGIC.length), SIGNED_MAGIC)
      || u16(packet, SIGNED_DELTA_VERSION_OFFSET_V3) !== SIGNED_DELTA_WIRE_VERSION_V3) throw new Error('SignedDelta packet has another magic, version, or width');
  if (packet[SIGNED_DELTA_CALLER_ROLE_OFFSET_V3] !== EXECUTION_ROLE_CLAIMS_V1) throw new Error('SignedDelta packet is not wallet/Claims authorized');
  requireZero(packet, SIGNED_DELTA_HEADER_RESERVED_OFFSET_V3, 5, 'SignedDelta header');
  requireZero(packet, SIGNED_DELTA_HEADER_TAIL_RESERVED_OFFSET_V3, 12, 'SignedDelta header tail');
  const claimCount = u32At(packet, SIGNED_DELTA_CLAIM_COUNT_OFFSET_V3);
  const positionCount = u32At(packet, SIGNED_DELTA_POSITION_COUNT_OFFSET_V3);
  const deltaCount = u32At(packet, SIGNED_DELTA_POSITION_DELTA_COUNT_OFFSET_V3);
  if (positionCount !== 1 || deltaCount !== 1 || claimCount === 0 || inputClaimIndex(request) >= claimCount) throw new Error('SignedDelta packet is not the canonical one-Position terminal shape');
  const positionsBytes = positionCount * SIGNED_DELTA_POSITION_BYTES_V3;
  const aggregateBytes = claimCount * SIGNED_DELTA_BYTES_V3;
  const deltaBytes = deltaCount * SIGNED_DELTA_ROW_BYTES_V3;
  if (packet.length !== SIGNED_DELTA_PLAN_HEADER_BYTES_V3 + positionsBytes + aggregateBytes + deltaBytes) throw new Error('SignedDelta packet width disagrees with its counts');
  const headerBindings = [
    [SIGNED_DELTA_RELEASE_SET_OFFSET_V3, requestIdentity(request.releaseSet, 'release set')],
    [SIGNED_DELTA_MARKET_OFFSET_V3, key(request.market, 'Market').toBytes()],
    [SIGNED_DELTA_REQUEST_OFFSET_V3, requestDigest],
    [SIGNED_DELTA_PRODUCT_OFFSET_V3, requestIdentity(request.productRecordDigest, 'Product record digest')],
    [SIGNED_DELTA_BASIS_OFFSET_V3, requestIdentity(request.semanticBasisId, 'semantic basis identity')],
    [SIGNED_DELTA_LINKED_BASIS_OFFSET_V3, requestIdentity(request.linkedBasisRecordDigest, 'linked basis digest')],
  ] as const;
  for (const [offset, expected] of headerBindings) if (!same(slice(packet, offset, 32), expected)) throw new Error('SignedDelta packet substitutes a request identity');
  if (u64(packet, SIGNED_DELTA_MARKET_REVISION_OFFSET_V3) !== decimal(request.expectedMarketRevision, 'expected Market revision')) throw new Error('SignedDelta packet substitutes the Market revision');
  const positions = slice(packet, SIGNED_DELTA_PLAN_HEADER_BYTES_V3, positionsBytes);
  const aggregates = slice(packet, SIGNED_DELTA_PLAN_HEADER_BYTES_V3 + positionsBytes, aggregateBytes);
  const deltas = slice(packet, SIGNED_DELTA_PLAN_HEADER_BYTES_V3 + positionsBytes + aggregateBytes, deltaBytes);
  if (!same(slice(positions, SIGNED_DELTA_POSITION_OWNER_OFFSET_V3, 32), key(request.owner, 'Position owner').toBytes())
      || u64(positions, SIGNED_DELTA_POSITION_REVISION_OFFSET_V3) !== decimal(request.expectedPositionRevision, 'expected Position revision')) throw new Error('SignedDelta Position table substitutes the wallet Position');
  const quantity = decimal(request.quantity, 'quantity', true);
  for (let index = 0; index < claimCount; index += 1) {
    const offset = index * SIGNED_DELTA_BYTES_V3;
    const selected = index === request.claimIndex;
    if (aggregates[offset + SIGNED_DELTA_DIRECTION_OFFSET_V3] !== (selected ? 2 : 0)
        || !isZero(slice(aggregates, offset + SIGNED_DELTA_RESERVED_OFFSET_V3, 7))
        || u64(aggregates, offset + SIGNED_DELTA_MAGNITUDE_OFFSET_V3) !== (selected ? quantity : 0n)) {
      throw new Error('SignedDelta aggregate table is not the exact one-coordinate debit');
    }
  }
  if (u32At(deltas, SIGNED_DELTA_ROW_POSITION_INDEX_OFFSET_V3) !== 0
      || u32At(deltas, SIGNED_DELTA_ROW_OUTCOME_OFFSET_V3) !== request.claimIndex
      || deltas[SIGNED_DELTA_ROW_DELTA_OFFSET_V3 + SIGNED_DELTA_DIRECTION_OFFSET_V3] !== 2
      || !isZero(slice(deltas, SIGNED_DELTA_ROW_DELTA_OFFSET_V3 + SIGNED_DELTA_RESERVED_OFFSET_V3, 7))
      || u64(deltas, SIGNED_DELTA_ROW_DELTA_OFFSET_V3 + SIGNED_DELTA_MAGNITUDE_OFFSET_V3) !== quantity) throw new Error('SignedDelta Position delta is not the exact one-coordinate debit');
  return Object.freeze({ positions, aggregates, deltas });
}

function inputClaimIndex(request: WalletTerminalPayoutRequestV3): number {
  if (!Number.isSafeInteger(request.claimIndex) || request.claimIndex < 0 || request.claimIndex > 0xffff_ffff) throw new Error('claim index is outside u32');
  return request.claimIndex;
}

function validateToken(bytes: Uint8Array, mint: string, owner: string, field: string): bigint {
  if (bytes.length !== TOKEN_ACCOUNT_BYTES || bytes[TOKEN_STATE_OFFSET] !== 1
      || !same(slice(bytes, TOKEN_ACCOUNT_MINT_OFFSET_V1, 32), key(mint, `${field} Mint`).toBytes())
      || !same(slice(bytes, TOKEN_ACCOUNT_OWNER_OFFSET_V1, 32), key(owner, `${field} owner`).toBytes())) throw new Error(`${field} is not the exact initialized token account`);
  return u64(bytes, TOKEN_AMOUNT_OFFSET);
}

function validateReplay(bytes: Uint8Array, request: WalletTerminalPayoutRequestV3, context: Uint8Array): void {
  if (bytes.length !== CUSTODY_REPLAY_BYTES_V1
      || !same(slice(bytes, CUSTODY_REPLAY_VERSION_OFFSET_V1 - CUSTODY_REPLAY_MAGIC_V1.length, CUSTODY_REPLAY_MAGIC_V1.length), CUSTODY_REPLAY_MAGIC_V1)
      || u16(bytes, CUSTODY_REPLAY_VERSION_OFFSET_V1) !== CUSTODY_ABI_VERSION_V1
      || bytes[CUSTODY_REPLAY_STATUS_OFFSET_V1] !== 1
      || bytes[CUSTODY_REPLAY_CALLER_ROLE_OFFSET_V1] !== EXECUTION_ROLE_CLAIMS_V1
      || !same(slice(bytes, CUSTODY_REPLAY_RELEASE_SET_OFFSET_V1, 32), requestIdentity(request.releaseSet, 'release set'))
      || !same(slice(bytes, CUSTODY_REPLAY_MARKET_OFFSET_V1, 32), key(request.market, 'Market').toBytes())
      || !same(slice(bytes, CUSTODY_REPLAY_REALM_OFFSET_V1, 32), requestIdentity(request.realm, 'Realm'))
      || !same(slice(bytes, CUSTODY_REPLAY_CONTEXT_OFFSET_V1, 32), context)
      || !same(slice(bytes, CUSTODY_REPLAY_CALLER_PROGRAM_OFFSET_V1, 32), key(request.claimsProgram, 'Claims program').toBytes())
      || u64(bytes, CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1) !== decimal(request.expectedCustodyRevision, 'expected Custody revision')
      || u64(bytes, CUSTODY_REPLAY_GENERATION_OFFSET_V1) !== decimal(request.generation, 'generation')) throw new Error('Custody replay does not bind the exact payout request');
}

function validateClaims(input: WalletTerminalPayoutBuildInputV3): void {
  requireZero(input.aggregateBytes, LIABILITY_BASIS_HEADER_RESERVED_OFFSET_V2, LIABILITY_BASIS_HEADER_RESERVED_BYTES_V2, 'Claims aggregate header');
  requireZero(input.positionBytes, LIABILITY_BASIS_HEADER_RESERVED_OFFSET_V2, LIABILITY_BASIS_HEADER_RESERVED_BYTES_V2, 'Claims Position header');
  requireZero(input.positionBytes, LIABILITY_BASIS_POSITION_RESERVED_OFFSET_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 - LIABILITY_BASIS_POSITION_RESERVED_OFFSET_V2, 'Claims Position header tail');
  const aggregate = decodeClaimsAggregateV2(input.route.aggregate, input.aggregateBytes);
  const position = decodeClaimsPositionV2(input.route.position, input.positionBytes);
  const request = input.request;
  if (aggregate.logicalMarket !== request.market || aggregate.selectedReleaseSetId !== request.releaseSet
      || aggregate.realmId !== request.realm || aggregate.liabilityBasisId !== request.semanticBasisId
      || aggregate.registryProgram !== input.route.registryProgram || aggregate.generation !== request.generation
      || aggregate.revision !== request.expectedMarketRevision || aggregate.claimCount <= inputClaimIndex(request)
      || BigInt(aggregate.supplyAtoms[request.claimIndex] ?? '0') < decimal(request.quantity, 'quantity', true)
      || position.aggregate !== input.route.aggregate || position.owner !== request.owner
      || position.liabilityBasisId !== request.semanticBasisId || position.revision !== request.expectedPositionRevision
      || BigInt(position.balances[request.claimIndex] ?? '0') < decimal(request.quantity, 'quantity', true)) throw new Error('Claims aggregate or Position does not bind the exact payout request');
}

function validateRoute(input: WalletTerminalPayoutBuildInputV3, context: Uint8Array): void {
  const route = input.route; const request = input.request;
  for (const [field, value] of Object.entries(route)) key(value, `route ${field}`);
  const aggregate = deriveClaimsAggregateAddressV2(route.claimsProgram, route.market);
  const position = deriveClaimsPositionAddressV2(route.claimsProgram, aggregate, request.owner);
  const release = requestIdentity(request.releaseSet, 'release set');
  const market = key(route.market, 'Market').toBytes();
  const [replay] = PublicKey.findProgramAddressSync([
    CUSTODY_REPLAY_PDA_DOMAIN_V1, market, release,
    Uint8Array.of(EXECUTION_ROLE_CLAIMS_V1), context,
  ], key(route.custodyProgram, 'Custody program'));
  const [authority] = PublicKey.findProgramAddressSync([
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, market, release,
  ], key(route.custodyProgram, 'Custody program'));
  const [hoard] = PublicKey.findProgramAddressSync([
    CUSTODY_VAULT_PDA_DOMAIN_V1, market, release, context,
    Uint8Array.of(CUSTODY_COMPARTMENT_HOARD_PRINCIPAL_TAG_V1),
  ], key(route.custodyProgram, 'Custody program'));
  if (route.aggregate !== aggregate || route.position !== position || route.custodyReplay !== replay.toBase58()
      || route.custodyAuthority !== authority.toBase58() || route.hoard !== hoard.toBase58()) {
    throw new Error('physical payout route does not use its canonical Claims or Custody PDA');
  }
  if (route.tokenProgram !== LEGACY_TOKEN_PROGRAM && route.tokenProgram !== TOKEN_2022_PROGRAM) {
    throw new Error('payout route selects an unsupported token program');
  }
  validateTerminalAuthority(route, request);
}

function validateTerminalAuthority(route: WalletTerminalPayoutRouteV3, request: WalletTerminalPayoutRequestV3): void {
  const certificate = key(route.terminalCertificate, 'Resolution certificate');
  if (hex(certificate.toBytes()) !== request.terminalRecordDigest) {
    throw new Error('Resolution certificate differs from the Core terminal receipt identity');
  }
  const [programData] = PublicKey.findProgramAddressSync([
    key(route.resolutionProgram, 'Resolution program').toBytes(),
  ], UPGRADEABLE_LOADER);
  if (route.resolutionProgramData !== programData.toBase58()) {
    throw new Error('Resolution ProgramData is not the canonical Loader-v3 authority coordinate');
  }
}

async function custodyEvidence(input: WalletTerminalPayoutBuildInputV3, requestBytes: Uint8Array, requestDigest: Uint8Array, signedPacketDigest: Uint8Array): Promise<Readonly<{ caller: string; requestDigest: Uint8Array }>> {
  const payout = decimal(input.payout, 'payout');
  if (payout === 0n) return Object.freeze({ caller: input.route.claimsProgram, requestDigest: new Uint8Array(32) });
  const request = input.request;
  const context = requestIdentity(input.custodyContext, 'Custody context');
  const candidate = await hashv(CANDIDATE_DOMAIN, requestDigest, signedPacketDigest, le64(payout), requestIdentity(request.exposureDigest, 'exposure digest'), requestIdentity(request.terminalRecordDigest, 'terminal record digest'));
  const custody = new Uint8Array(CUSTODY_REQUEST_BYTES_V1);
  custody.set(CUSTODY_REQUEST_MAGIC_V1, 0);
  putU16(custody, CUSTODY_REQUEST_VERSION_OFFSET_V1, CUSTODY_ABI_VERSION_V1);
  custody[CUSTODY_REQUEST_OPERATION_OFFSET_V1] = CUSTODY_OPERATION_TRANSFER_V1;
  custody[CUSTODY_REQUEST_CALLER_ROLE_OFFSET_V1] = EXECUTION_ROLE_CLAIMS_V1;
  custody[CUSTODY_REQUEST_SOURCE_COMPARTMENT_OFFSET_V1] = CUSTODY_COMPARTMENT_HOARD_PRINCIPAL_V1;
  custody[CUSTODY_REQUEST_DESTINATION_COMPARTMENT_OFFSET_V1] = CUSTODY_COMPARTMENT_EXTERNAL_V1;
  putU16(custody, CUSTODY_REQUEST_TRANSFER_INDEX_OFFSET_V1, request.transferIndex);
  for (const [offset, value] of [
    [CUSTODY_REQUEST_RELEASE_SET_OFFSET_V1, requestIdentity(request.releaseSet, 'release set')],
    [CUSTODY_REQUEST_MARKET_OFFSET_V1, key(request.market, 'Market').toBytes()],
    [CUSTODY_REQUEST_REALM_OFFSET_V1, requestIdentity(request.realm, 'Realm')],
    [CUSTODY_REQUEST_CONTEXT_OFFSET_V1, context],
    [CUSTODY_REQUEST_CALLER_PROGRAM_OFFSET_V1, key(request.claimsProgram, 'Claims program').toBytes()],
    [CUSTODY_REQUEST_CANDIDATE_OFFSET_V1, candidate],
    [CUSTODY_REQUEST_DESTINATION_OWNER_OFFSET_V1, key(request.recipientOwner, 'recipient owner').toBytes()],
    [CUSTODY_REQUEST_PARENT_REQUEST_DIGEST_OFFSET_V1, requestDigest],
    [CUSTODY_REQUEST_SOURCE_OFFSET_V1, key(input.route.hoard, 'Hoard').toBytes()],
    [CUSTODY_REQUEST_DESTINATION_OFFSET_V1, key(input.route.recipient, 'recipient').toBytes()],
    [CUSTODY_REQUEST_SOURCE_VAULT_CONTEXT_OFFSET_V1, context],
    [CUSTODY_REQUEST_MINT_OFFSET_V1, key(input.route.collateralMint, 'collateral Mint').toBytes()],
    [CUSTODY_REQUEST_TOKEN_PROGRAM_OFFSET_V1, key(input.route.tokenProgram, 'Token program').toBytes()],
  ] as const) custody.set(value, offset);
  const expectedRevision = decimal(request.expectedCustodyRevision, 'expected Custody revision');
  putU64(custody, CUSTODY_REQUEST_EXPECTED_REVISION_OFFSET_V1, expectedRevision);
  putU64(custody, CUSTODY_REQUEST_RESULTING_REVISION_OFFSET_V1, expectedRevision + 1n);
  putU64(custody, CUSTODY_REQUEST_ORDER_NONCE_OFFSET_V1, decimal(request.expectedPositionRevision, 'expected Position revision'));
  putU64(custody, CUSTODY_REQUEST_GENERATION_OFFSET_V1, decimal(request.generation, 'generation'));
  putU64(custody, CUSTODY_REQUEST_AMOUNT_OFFSET_V1, payout);
  const digest = await sha256(custody);
  const [caller] = PublicKey.findProgramAddressSync([
    CALLER_AUTHORITY_PDA_DOMAIN_V1, requestIdentity(request.releaseSet, 'release set'),
    key(request.market, 'Market').toBytes(), Uint8Array.of(EXECUTION_ROLE_CLAIMS_V1), context, digest,
  ], key(request.claimsProgram, 'Claims program'));
  // Keep the request parameter named in this function: it is the exact byte
  // authority from which both candidate and caller were derived.
  if (!same(await sha256(requestBytes), requestDigest)) throw new Error('terminal request digest changed during Custody construction');
  return Object.freeze({ caller: caller.toBase58(), requestDigest: digest });
}

function le64(value: bigint): Uint8Array {
  const output = new Uint8Array(8); putU64(output, 0, value); return output;
}

function payoutMetas(route: WalletTerminalPayoutRouteV3, owner: string, custodyCaller: string) {
  const readonly = (value: string) => ({ pubkey: key(value, 'payout account'), isSigner: false, isWritable: false });
  const writable = (value: string) => ({ pubkey: key(value, 'payout account'), isSigner: false, isWritable: true });
  return [
    { pubkey: key(owner, 'Position owner'), isSigner: true, isWritable: false },
    writable(route.aggregate), readonly(route.linkedBasisRaw), readonly(route.linkedBasisStaging),
    readonly(route.productRaw), readonly(route.productStaging), readonly(route.resultDomainRaw),
    readonly(route.resultDomainStaging), readonly(route.portfolioRaw), readonly(route.portfolioStaging),
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false }, readonly(route.market),
    readonly(route.activationCache), readonly(route.registryProgram), readonly(route.claimsProgram),
    readonly(route.claimsProgramData), readonly(route.claimsProgram), readonly(route.claimsProgramData),
    readonly(route.coreProgram), readonly(route.coreProgramData), writable(route.position),
    readonly(route.exposureRaw), readonly(route.exposureStaging), readonly(custodyCaller),
    readonly(route.custodyProgram), readonly(route.terminalCertificate), readonly(route.resolutionProgram),
    readonly(route.resolutionProgramData),
    readonly(route.realmRaw), readonly(route.realmStaging), writable(route.custodyReplay),
    readonly(route.collateralMint), writable(route.hoard), writable(route.recipient),
    readonly(route.custodyAuthority), readonly(route.tokenProgram),
  ];
}

/** Build and independently bind one wallet terminal payout report. */
export async function buildWalletTerminalPayoutV3(input: WalletTerminalPayoutBuildInputV3): Promise<WalletTerminalPayoutReportV3> {
  decimal(input.observedSlot, 'observed finalized slot', true);
  const request = input.request;
  if (input.route.market !== request.market || input.route.position !== request.position
      || input.route.claimsProgram !== request.claimsProgram || input.route.custodyProgram !== request.custodyProgram
      || input.route.collateralMint !== request.collateralMint || input.route.tokenProgram !== request.tokenProgram
      || input.route.recipient !== request.recipient) throw new Error('physical payout route substitutes a request coordinate');
  validateClaims(input);
  const context = requestIdentity(input.custodyContext, 'Custody context');
  validateRoute(input, context);
  validateReplay(input.custodyReplayBytes, request, context);
  validateToken(input.hoardTokenBytes, request.collateralMint, input.route.custodyAuthority, 'Hoard');
  validateToken(input.recipientTokenBytes, request.collateralMint, request.recipientOwner, 'recipient');
  const requestBytes = encodeWalletTerminalPayoutRequestV3(request);
  const requestDigest = await sha256(requestBytes);
  const tables = validateSignedPacket(input.signedPacket, request, requestDigest);
  const signedPacketDigest = await sha256(input.signedPacket);
  const signedTableDigest = await hashv(SIGNED_TABLE_DOMAIN, tables.positions, tables.aggregates, tables.deltas);
  const custody = await custodyEvidence(input, requestBytes, requestDigest, signedPacketDigest);
  const metas = payoutMetas(input.route, request.owner, custody.caller);
  if (metas.length !== ACCOUNT_COUNT) throw new Error('terminal payout account frame is not exactly 36 accounts');
  for (const [index, address] of [
    [TERMINAL_SETTLEMENT_CERTIFICATE_ACCOUNT_V3, input.route.terminalCertificate],
    [TERMINAL_SETTLEMENT_RESOLUTION_PROGRAM_ACCOUNT_V3, input.route.resolutionProgram],
    [TERMINAL_SETTLEMENT_RESOLUTION_PROGRAMDATA_ACCOUNT_V3, input.route.resolutionProgramData],
  ] as const) {
    const meta = metas[index];
    if (meta === undefined || meta.pubkey.toBase58() !== address || meta.isSigner || meta.isWritable) {
      throw new Error('terminal payout certificate authority is not in its exact readonly frame coordinate');
    }
  }
  const instruction = new TransactionInstruction({ programId: key(request.claimsProgram, 'Claims program'), keys: metas, data: requestBytes as Buffer });
  return Object.freeze({
    observedSlot: input.observedSlot, route: input.route, request, requestBytes, requestDigest,
    signedPacket: new Uint8Array(input.signedPacket), signedPacketDigest, signedTableDigest,
    payout: decimal(input.payout, 'payout').toString(), custodyCaller: custody.caller,
    custodyRequestDigest: custody.requestDigest, instruction,
    preAggregateBytes: new Uint8Array(input.aggregateBytes), prePositionBytes: new Uint8Array(input.positionBytes),
    preCustodyReplayBytes: new Uint8Array(input.custodyReplayBytes), preHoardTokenBytes: new Uint8Array(input.hoardTokenBytes),
    preRecipientTokenBytes: new Uint8Array(input.recipientTokenBytes),
  });
}

/** Exact first-use lookup sequence; payer and owner stay static signers. */
export function canonicalWalletTerminalPayoutLookupAddressesV3(report: WalletTerminalPayoutReportV3, payer: string): ReadonlyArray<string> {
  const payerKey = key(payer, 'fee payer').toBase58();
  const seen = new Set<string>();
  const output: string[] = [];
  for (const address of [report.instruction.programId.toBase58(), ...report.instruction.keys.map((meta) => meta.pubkey.toBase58())]) {
    if (address === payerKey || address === report.request.owner || seen.has(address)) continue;
    seen.add(address); output.push(address);
  }
  if (output.length === 0 || output.length > 256) throw new Error('canonical payout lookup sequence has an invalid width');
  return Object.freeze(output);
}

export type WalletTerminalPayoutTransactionV3 = Readonly<{
  transaction: VersionedTransaction; wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>; report: WalletTerminalPayoutReportV3;
}>;

/** Compile through exactly one same-observation canonical lookup table. */
export function compileWalletTerminalPayoutV0(report: WalletTerminalPayoutReportV3, input: Readonly<{
  payer: string; recentBlockhash: string; lookupTable: AddressLookupTableAccount; lookupObservedSlot: string;
}>): WalletTerminalPayoutTransactionV3 {
  if (input.lookupObservedSlot !== report.observedSlot) throw new Error('lookup table was not read in the payout prestate observation');
  const observedSlot = decimal(input.lookupObservedSlot, 'lookup observation slot', true);
  if (input.lookupTable.state.deactivationSlot !== U64_MAX
      || BigInt(input.lookupTable.state.lastExtendedSlot) >= observedSlot) throw new Error('lookup table is deactivated or was not finalized before the payout observation');
  const expected = canonicalWalletTerminalPayoutLookupAddressesV3(report, input.payer);
  const observed = input.lookupTable.state.addresses.map((address) => address.toBase58());
  if (observed.length !== expected.length || observed.some((address, index) => address !== expected[index])) throw new Error('lookup table is not the sole canonical payout sequence');
  const payer = key(input.payer, 'fee payer');
  const transaction = new VersionedTransaction(new TransactionMessage({ payerKey: payer, recentBlockhash: key(input.recentBlockhash, 'recent blockhash').toBase58(), instructions: [report.instruction] }).compileToV0Message([input.lookupTable]));
  const wireBytes = transaction.serialize();
  if (wireBytes.length > PACKET_BYTES) throw new Error(`terminal payout transaction is ${wireBytes.length} bytes, above Solana's ${PACKET_BYTES}-byte packet bound`);
  const requiredSigners = Object.freeze(transaction.message.staticAccountKeys.slice(0, transaction.message.header.numRequiredSignatures).map((address) => address.toBase58()));
  const expectedSigners = input.payer === report.request.owner ? [input.payer] : [input.payer, report.request.owner];
  if (requiredSigners.length !== expectedSigners.length || requiredSigners.some((address, index) => address !== expectedSigners[index])) throw new Error('compiled payout message has another signer set');
  return Object.freeze({ transaction, wireBytes, requiredSigners, report });
}

export type WalletTerminalPayoutPoststateV3 = Readonly<{
  receiptBytes: Uint8Array; aggregateBytes: Uint8Array; positionBytes: Uint8Array;
  custodyReplayBytes: Uint8Array; hoardTokenBytes: Uint8Array; recipientTokenBytes: Uint8Array;
}>;

function debited(pre: Uint8Array, revisionOffset: number, vectorOffset: number, claimIndex: number, quantity: bigint): Uint8Array {
  const output = new Uint8Array(pre);
  putU64(output, revisionOffset, u64(output, revisionOffset) + 1n);
  const offset = vectorOffset + claimIndex * 8;
  const before = u64(output, offset);
  if (before < quantity) throw new Error('payout postcondition debit underflows');
  putU64(output, offset, before - quantity);
  return output;
}

function tokenAmount(pre: Uint8Array, amount: bigint): Uint8Array {
  const output = new Uint8Array(pre); putU64(output, TOKEN_AMOUNT_OFFSET, amount); return output;
}

/** Verify the exact persisted resources and Claims return receipt after finality. */
export async function verifyWalletTerminalPayoutPostconditionV3(report: WalletTerminalPayoutReportV3, post: WalletTerminalPayoutPoststateV3): Promise<void> {
  const quantity = decimal(report.request.quantity, 'quantity', true);
  const expectedAggregate = debited(report.preAggregateBytes, LIABILITY_BASIS_MARKET_REVISION_OFFSET, LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, report.request.claimIndex, quantity);
  const expectedPosition = debited(report.prePositionBytes, LIABILITY_BASIS_POSITION_REVISION_OFFSET, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, report.request.claimIndex, quantity);
  if (!same(post.aggregateBytes, expectedAggregate) || !same(post.positionBytes, expectedPosition)) throw new Error('Claims payout poststate differs from the exact one-coordinate debit');
  decodeClaimsAggregateV2(report.route.aggregate, post.aggregateBytes);
  decodeClaimsPositionV2(report.route.position, post.positionBytes);
  const payout = decimal(report.payout, 'payout');
  const beforeHoard = u64(report.preHoardTokenBytes, TOKEN_AMOUNT_OFFSET);
  const beforeRecipient = u64(report.preRecipientTokenBytes, TOKEN_AMOUNT_OFFSET);
  if (beforeHoard < payout || beforeRecipient + payout > U64_MAX) throw new Error('token payout postcondition overflows');
  const expectedHoard = tokenAmount(report.preHoardTokenBytes, beforeHoard - payout);
  const expectedRecipient = tokenAmount(report.preRecipientTokenBytes, beforeRecipient + payout);
  if (!same(post.hoardTokenBytes, expectedHoard) || !same(post.recipientTokenBytes, expectedRecipient)) throw new Error('token payout poststate differs from the exact transfer');
  const beforeRevision = u64(report.preCustodyReplayBytes, CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1);
  const afterRevision = beforeRevision + (payout === 0n ? 0n : 1n);
  const expectedReplay = new Uint8Array(report.preCustodyReplayBytes);
  if (payout !== 0n) {
    putU64(expectedReplay, CUSTODY_REPLAY_NEXT_REVISION_OFFSET_V1, afterRevision);
    expectedReplay.set(report.custodyRequestDigest, REPLAY_LAST_REQUEST_OFFSET);
    expectedReplay.set(await hashv(CUSTODY_POST_DOMAIN, report.custodyRequestDigest,
      key(report.route.hoard, 'Hoard').toBytes(), key(report.route.recipient, 'recipient').toBytes(),
      le64(beforeHoard), le64(beforeHoard - payout), le64(beforeRecipient), le64(beforeRecipient + payout), le64(0n)), REPLAY_LAST_POSTSTATE_OFFSET);
  }
  if (!same(post.custodyReplayBytes, expectedReplay)) throw new Error('Custody replay poststate differs from the exact payout transition');
  if (post.receiptBytes.length !== RECEIPT_BYTES
      || !same(slice(post.receiptBytes, WALLET_TERMINAL_MAGIC_OFFSET_V3, RECEIPT_MAGIC.length), RECEIPT_MAGIC)
      || u16(post.receiptBytes, WALLET_TERMINAL_VERSION_OFFSET_V3) !== VERSION) throw new Error('terminal payout receipt has another magic, version, or width');
  requireZero(post.receiptBytes, TERMINAL_SETTLEMENT_RECEIPT_RESERVED_OFFSET_V3, TERMINAL_SETTLEMENT_RECEIPT_RESERVED_BYTES_V3, 'terminal payout receipt header');
  requireZero(post.receiptBytes, TERMINAL_SETTLEMENT_RECEIPT_TAIL_RESERVED_OFFSET_V3, TERMINAL_SETTLEMENT_RECEIPT_TAIL_RESERVED_BYTES_V3, 'terminal payout receipt tail');
  if (!same(slice(post.receiptBytes, RECEIPT_OFFSETS.request, REQUEST_BYTES), report.requestBytes)) throw new Error('terminal receipt embeds another payout request');
  const signedPost = await hashv(SIGNED_POST_DOMAIN, post.aggregateBytes, post.positionBytes);
  const replayDigest = await sha256(post.custodyReplayBytes);
  const tokenPost = await hashv(TOKEN_POST_DOMAIN, post.hoardTokenBytes, post.recipientTokenBytes);
  const custodyReceiptDigest = slice(post.receiptBytes, RECEIPT_OFFSETS.custodyReceipt, 32);
  const postResource = await hashv(TERMINAL_POST_DOMAIN, report.requestDigest, signedPost, replayDigest, tokenPost, custodyReceiptDigest);
  for (const [offset, expected, field] of [
    [RECEIPT_OFFSETS.requestDigest, report.requestDigest, 'request digest'],
    [RECEIPT_OFFSETS.signedPacket, report.signedPacketDigest, 'SignedDelta packet digest'],
    [RECEIPT_OFFSETS.signedTable, report.signedTableDigest, 'SignedDelta table digest'],
    [RECEIPT_OFFSETS.signedPost, signedPost, 'Claims poststate digest'],
    [RECEIPT_OFFSETS.custodyRequest, report.custodyRequestDigest, 'Custody request digest'],
    [RECEIPT_OFFSETS.custodyReplay, replayDigest, 'Custody replay digest'],
    [RECEIPT_OFFSETS.custodyTokenPost, tokenPost, 'token poststate digest'],
    [RECEIPT_OFFSETS.postResource, postResource, 'postresource digest'],
  ] as const) if (!same(slice(post.receiptBytes, offset, 32), expected)) throw new Error(`terminal receipt substitutes the ${field}`);
  if ((payout === 0n) !== isZero(custodyReceiptDigest)) throw new Error('terminal receipt Custody shape disagrees with its payout');
  const revisions = [
    [RECEIPT_OFFSETS.payout, payout],
    [RECEIPT_OFFSETS.preMarket, decimal(report.request.expectedMarketRevision, 'expected Market revision')],
    [RECEIPT_OFFSETS.postMarket, decimal(report.request.expectedMarketRevision, 'expected Market revision') + 1n],
    [RECEIPT_OFFSETS.prePosition, decimal(report.request.expectedPositionRevision, 'expected Position revision')],
    [RECEIPT_OFFSETS.postPosition, decimal(report.request.expectedPositionRevision, 'expected Position revision') + 1n],
    [RECEIPT_OFFSETS.preCustody, beforeRevision], [RECEIPT_OFFSETS.postCustody, afterRevision],
  ] as const;
  for (const [offset, expected] of revisions) if (u64(post.receiptBytes, offset) !== expected) throw new Error('terminal receipt substitutes a payout or revision coordinate');
}

export function walletTerminalPayoutSummaryV3(report: WalletTerminalPayoutReportV3): Readonly<{ payout: string; requestDigest: string; signedPacketDigest: string; signedTableDigest: string; custodyRequestDigest: string }> {
  return Object.freeze({ payout: report.payout, requestDigest: hex(report.requestDigest), signedPacketDigest: hex(report.signedPacketDigest), signedTableDigest: hex(report.signedTableDigest), custodyRequestDigest: hex(report.custodyRequestDigest) });
}

export type WalletTerminalPayoutManifestV3 = Readonly<{
  format: 'dclutch-wallet-terminal-payout-v3';
  route: WalletTerminalPayoutRouteV3;
  custodyContext: string;
  request: WalletTerminalPayoutRequestV3;
  signedPacketBase64: string;
  payout: string;
  lookupTable: string;
}>;

export type CheckedLiveDevnetPayoutAdmissionV3 = Readonly<{
  genesisHash: string;
  observedSlot: string;
  market: string;
  position: string;
  recipient: string;
  releaseSet: string;
  lookupTable: string;
}>;

function plain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function exactKeys(value: Record<string, unknown>, fields: ReadonlyArray<string>, label: string): void {
  const observed = Object.keys(value).sort(); const expected = [...fields].sort();
  if (observed.length !== expected.length || observed.some((field, index) => field !== expected[index])) throw new Error(`${label} has missing or unknown fields`);
}

function textField(value: unknown, field: string, maximum = 512): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > maximum || value.trim() !== value) throw new Error(`${field} is not bounded canonical text`);
  return value;
}

function base64Bytes(value: unknown, field: string): Uint8Array {
  const text = textField(value, field, 8_192);
  if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(text)) throw new Error(`${field} is not canonical base64`);
  const binary = atob(text); const output = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  if (btoa(String.fromCharCode(...output)) !== text) throw new Error(`${field} is not canonical base64`);
  return output;
}

/** Hostile-decode one bounded Rust-authored wallet payout manifest. */
export function parseWalletTerminalPayoutManifestV3(source: string): WalletTerminalPayoutManifestV3 {
  if (source.length === 0 || source.length > 32_768) throw new Error('payout manifest must contain 1..32768 characters');
  let parsed: unknown;
  try { parsed = JSON.parse(source); } catch { throw new Error('payout manifest is not JSON'); }
  if (!plain(parsed)) throw new Error('payout manifest must be one object');
  exactKeys(parsed, ['format', 'route', 'custodyContext', 'request', 'signedPacketBase64', 'payout', 'lookupTable'], 'payout manifest');
  const routeValue = parsed.route; const requestValue = parsed.request;
  if (parsed.format !== 'dclutch-wallet-terminal-payout-v3' || !plain(routeValue) || !plain(requestValue)) throw new Error('payout manifest has another format or shape');
  const routeFields = ['aggregate', 'linkedBasisRaw', 'linkedBasisStaging', 'productRaw', 'productStaging', 'resultDomainRaw', 'resultDomainStaging', 'portfolioRaw', 'portfolioStaging', 'market', 'activationCache', 'registryProgram', 'claimsProgram', 'claimsProgramData', 'coreProgram', 'coreProgramData', 'resolutionProgram', 'resolutionProgramData', 'position', 'exposureRaw', 'exposureStaging', 'custodyProgram', 'terminalCertificate', 'realmRaw', 'realmStaging', 'custodyReplay', 'collateralMint', 'hoard', 'recipient', 'custodyAuthority', 'tokenProgram'] as const;
  const requestFields = ['releaseSet', 'market', 'realm', 'parentContext', 'productRecordDigest', 'exposureId', 'exposureDigest', 'terminalRecordDigest', 'owner', 'position', 'recipientOwner', 'recipient', 'claimsProgram', 'custodyProgram', 'collateralMint', 'tokenProgram', 'semanticBasisId', 'linkedBasisRecordDigest', 'generation', 'expectedMarketRevision', 'expectedPositionRevision', 'expectedCustodyRevision', 'quantity', 'claimIndex', 'transferIndex'] as const;
  exactKeys(routeValue, routeFields, 'payout route'); exactKeys(requestValue, requestFields, 'payout request');
  const route = Object.fromEntries(routeFields.map((field) => [field, textField(routeValue[field], `route ${field}`, 96)])) as WalletTerminalPayoutRouteV3;
  const requestText = Object.fromEntries(requestFields.filter((field) => field !== 'claimIndex' && field !== 'transferIndex').map((field) => [field, textField(requestValue[field], `request ${field}`, 128)]));
  const claimIndex = requestValue.claimIndex; const transferIndex = requestValue.transferIndex;
  if (typeof claimIndex !== 'number' || typeof transferIndex !== 'number') throw new Error('payout request indexes must be JSON numbers');
  const request = Object.freeze({ ...requestText, claimIndex, transferIndex }) as WalletTerminalPayoutRequestV3;
  // Run every fixed request validation now, before chain acquisition.
  encodeWalletTerminalPayoutRequestV3(request);
  for (const [field, value] of Object.entries(route)) key(value, `route ${field}`);
  validateTerminalAuthority(route, request);
  key(textField(parsed.lookupTable, 'lookup table', 96), 'lookup table');
  requestIdentity(textField(parsed.custodyContext, 'Custody context', 64), 'Custody context');
  decimal(textField(parsed.payout, 'payout', 32), 'payout');
  base64Bytes(parsed.signedPacketBase64, 'SignedDelta packet');
  return Object.freeze({
    format: 'dclutch-wallet-terminal-payout-v3', route: Object.freeze(route), request,
    custodyContext: parsed.custodyContext as string, signedPacketBase64: parsed.signedPacketBase64 as string,
    payout: parsed.payout as string, lookupTable: parsed.lookupTable as string,
  });
}

/**
 * Accept the Rust producer's exact JSON file. Keeping this as a separate name
 * makes the browser handoff explicit: a file is imported, never authored or
 * completed from partial chain state here.
 */
export function importRustWalletTerminalPayoutArtifactV3(source: string): WalletTerminalPayoutManifestV3 {
  return parseWalletTerminalPayoutManifestV3(source);
}

function material(account: RpcAccount | null, owner: string, field: string): RpcAccount {
  if (account === null || account.owner !== owner || account.executable || account.space !== account.data.length || decimal(account.lamports, `${field} lamports`) === 0n) throw new Error(`${field} is absent or has another owner, executable bit, space, or lamport shape`);
  return account;
}

export type PreparedWalletTerminalPayoutV3 = WalletTerminalPayoutTransactionV3 & Readonly<{ lookupTable: string }>;

function exactRouteDeploymentV3(manifest: WalletTerminalPayoutManifestV3, payer: string): void {
  const preset = LIVE_DEVNET_OPERATOR_PRESET_V1;
  const route = manifest.route; const request = manifest.request;
  const pairs = [
    [route.market, request.market, 'Market'],
    [route.position, request.position, 'Position'],
    [route.recipient, request.recipient, 'recipient token account'],
    [route.collateralMint, request.collateralMint, 'collateral Mint'],
    [route.tokenProgram, request.tokenProgram, 'token program'],
    [route.claimsProgram, request.claimsProgram, 'Claims program'],
    [route.custodyProgram, request.custodyProgram, 'Custody program'],
    [route.registryProgram, preset.coordinates.registry, 'checked Registry program'],
    [route.coreProgram, preset.coordinates.core, 'checked Core program'],
    [route.claimsProgram, preset.coordinates.claims, 'checked Claims program'],
    [route.custodyProgram, preset.coordinates.custody, 'checked Custody program'],
    [route.resolutionProgram, preset.coordinates.resolution, 'checked Resolution program'],
    [route.claimsProgramData, preset.evidence.claims.programData, 'checked Claims ProgramData'],
    [route.coreProgramData, preset.evidence.core.programData, 'checked Core ProgramData'],
    [route.resolutionProgramData, preset.evidence.resolution.programData, 'checked Resolution ProgramData'],
    [route.activationCache, preset.activationCache, 'checked activation cache'],
  ] as const;
  for (const [observed, expected, field] of pairs) {
    if (observed !== expected) throw new Error(`payout plan ${field} differs from the exact checked live-devnet coordinate`);
  }
  if (request.owner !== payer || request.recipientOwner !== payer) {
    throw new Error('payout plan owner and recipient owner must both be the connected wallet');
  }
}

function recordAddressesV3(
  registry: string,
  schema: Uint8Array,
  digest: Uint8Array,
  raw: string,
  staging: string,
  field: string,
): void {
  const expected = deriveFinalizedRecordAddressesV1(registry, schema, digest);
  if (expected.record !== raw || expected.staging !== staging) {
    throw new Error(`${field} does not use its exact content-derived Registry coordinates`);
  }
}

function validateCompositionExposureHeaderV3(
  bytes: Uint8Array,
  request: WalletTerminalPayoutRequestV3,
  domainDigest: Uint8Array,
  productWidth: number,
  representationWidth: number,
): void {
  if (bytes.length < COMPOSITION_EXPOSURE_HEADER_BYTES_V3
      || !same(slice(bytes, 0, 8), new TextEncoder().encode('DCRCEX03'))
      || u16(bytes, 8) !== 3) throw new Error('composition exposure has another exact ABI');
  requireZero(bytes, 10, 6, 'composition exposure header');
  requireZero(bytes, 256, 48, 'composition exposure tail');
  if (!same(slice(bytes, 16, 32), key(request.market, 'Market').toBytes())
      || !same(slice(bytes, 48, 32), domainDigest)
      || !same(slice(bytes, 80, 32), requestIdentity(request.releaseSet, 'release set'))
      || !same(slice(bytes, 144, 32), requestIdentity(request.semanticBasisId, 'semantic basis identity'))
      || u32At(bytes, 240) !== productWidth
      || u32At(bytes, 244) !== representationWidth
      || u32At(bytes, 248) !== representationWidth) {
    throw new Error('composition exposure differs from the Market, Product, release, or Claims basis');
  }
}

/**
 * Authenticate the imported Rust plan against the checked live-devnet
 * deployment and its current terminal Market before any wallet handoff.
 */
export async function authenticateCheckedLiveDevnetPayoutPlanV3(
  client: SolanaRpcClient,
  manifest: WalletTerminalPayoutManifestV3,
  payer: string,
): Promise<CheckedLiveDevnetPayoutAdmissionV3> {
  const canonicalPayer = key(payer, 'connected wallet').toBase58();
  const cluster = await client.assertMutationCluster();
  if (cluster.kind !== 'devnet') throw new Error('wallet terminal payout is enabled only on the exact checked live-devnet deployment');
  exactRouteDeploymentV3(manifest, canonicalPayer);
  const preset = LIVE_DEVNET_OPERATOR_PRESET_V1;
  const deployment = await acquireOperatorSurfaceV1(client, {
    ...preset.coordinates,
    market: manifest.route.market,
  }, preset);
  if (deployment.deploymentPreset === null
      || deployment.deploymentPreset.executionReleaseSetId !== manifest.request.releaseSet) {
    throw new Error('payout plan release set differs from the checked activation cache');
  }

  const route = manifest.route; const request = manifest.request;
  const addresses = [
    route.market, route.terminalCertificate, route.aggregate,
    route.productRaw, route.productStaging, route.resultDomainRaw, route.resultDomainStaging,
    route.portfolioRaw, route.portfolioStaging, route.linkedBasisRaw, route.linkedBasisStaging,
    route.exposureRaw, route.exposureStaging, route.realmRaw, route.realmStaging,
  ];
  const observation = await client.multipleAccounts(addresses, deployment.observedSlot);
  if (BigInt(observation.slot) < BigInt(deployment.observedSlot)) {
    throw new Error('payout-plan account observation predates the checked deployment');
  }
  const accounts = new Map(observation.accounts.map((entry) => [entry.address, entry.account] as const));
  const marketAccount = material(accounts.get(route.market) ?? null, route.coreProgram, 'Core Market');
  const market = authenticateRationalHotCoreV3(route.market, marketAccount, route.coreProgram);
  if (market.phase !== 'Terminal' || market.readiness !== 'Consumed'
      || market.registry !== route.registryProgram
      || hex(market.releaseSet) !== request.releaseSet
      || hex(market.realm) !== request.realm
      || hex(market.productRecord) !== request.productRecordDigest
      || market.generation.toString() !== request.generation
      || hex(market.terminalReceipt) !== request.terminalRecordDigest) {
    throw new Error('payout plan differs from the current terminal Market authority');
  }

  const productDigest = requestIdentity(request.productRecordDigest, 'Product record digest');
  const productRaw = await authenticateFinalizedRationalHotRecordV4(client, accounts, route.registryProgram,
    route.productRaw, route.productStaging, PRODUCT_RECORD_SCHEMA_ID_V2, productDigest, 'Product Runtime V2 root');
  const domainDigest = slice(productRaw.data, 48, 32); const portfolioDigest = slice(productRaw.data, 80, 32);
  const domainRaw = await authenticateFinalizedRationalHotRecordV4(client, accounts, route.registryProgram,
    route.resultDomainRaw, route.resultDomainStaging, RESULT_DOMAIN_SCHEMA_ID_V2, domainDigest, 'Product result domain');
  const portfolioRaw = await authenticateFinalizedRationalHotRecordV4(client, accounts, route.registryProgram,
    route.portfolioRaw, route.portfolioStaging, PORTFOLIO_SCHEMA_ID_V2, portfolioDigest, 'Product portfolio');
  const product = decodeCoreFoundProductGraphV2(productRaw.data, domainRaw.data, portfolioRaw.data, domainDigest, portfolioDigest);
  if (!same(product.productId, market.productId)) throw new Error('payout Product identity differs from the current Market');

  const aggregate = material(accounts.get(route.aggregate) ?? null, route.claimsProgram, 'Claims aggregate');
  const aggregateView = decodeClaimsAggregateV2(route.aggregate, aggregate.data);
  const basis = await authenticateRationalProductBasisRecordV3(client, accounts, {
    registry: route.registryProgram, rawAddress: route.linkedBasisRaw, stagingAddress: route.linkedBasisStaging,
    productId: product.productId, domainDigest, domainBytes: domainRaw.data,
    representationWidth: aggregateView.claimCount,
  });
  if (hex(basis.digest) !== request.linkedBasisRecordDigest || hex(basis.semanticBasisId) !== request.semanticBasisId) {
    throw new Error('payout Product basis differs from the Rust-authored request identities');
  }
  const realmRaw = await authenticateFinalizedRationalHotRecordV4(client, accounts, route.registryProgram,
    route.realmRaw, route.realmStaging, REALM_SCHEMA_RELEASE_ID_V1,
    requestIdentity(request.realm, 'Realm'), 'Realm');
  const realm = decodeRealmRecordV1(realmRaw.data);
  if (realm.tokenProgram !== request.tokenProgram || realm.collateralMint !== request.collateralMint) {
    throw new Error('payout recipient asset differs from the immutable Realm collateral coordinates');
  }
  const exposureSchema = await sha256(COMPOSITION_EXPOSURE_SCHEMA_PREIMAGE_V3);
  const exposureDigest = requestIdentity(request.exposureDigest, 'composition exposure digest');
  const exposureRaw = await authenticateFinalizedRationalHotRecordV4(client, accounts, route.registryProgram,
    route.exposureRaw, route.exposureStaging, exposureSchema, exposureDigest, 'composition exposure');
  validateCompositionExposureHeaderV3(exposureRaw.data, request, domainDigest, product.outcomeCount, aggregateView.claimCount);

  // Redundant PDA checks keep every imported Registry coordinate explicit in
  // this admission report instead of relying on a helper's implementation.
  for (const [schema, digest, raw, staging, field] of [
    [PRODUCT_RECORD_SCHEMA_ID_V2, productDigest, route.productRaw, route.productStaging, 'Product'],
    [RESULT_DOMAIN_SCHEMA_ID_V2, domainDigest, route.resultDomainRaw, route.resultDomainStaging, 'result domain'],
    [PORTFOLIO_SCHEMA_ID_V2, portfolioDigest, route.portfolioRaw, route.portfolioStaging, 'portfolio'],
    [GRADED_BASIS_RECORD_SCHEMA_ID_V3, basis.digest, route.linkedBasisRaw, route.linkedBasisStaging, 'Product basis'],
    [REALM_SCHEMA_RELEASE_ID_V1, requestIdentity(request.realm, 'Realm'), route.realmRaw, route.realmStaging, 'Realm'],
    [exposureSchema, exposureDigest, route.exposureRaw, route.exposureStaging, 'composition exposure'],
  ] as const) recordAddressesV3(route.registryProgram, schema, digest, raw, staging, field);

  const certificateAccount = material(accounts.get(route.terminalCertificate) ?? null, route.resolutionProgram, 'Resolution certificate');
  const certificateRent = await client.minimumBalanceForRentExemption(certificateAccount.data.length);
  if (BigInt(certificateAccount.lamports) < BigInt(certificateRent.lamports)) {
    throw new Error('Resolution certificate is below its current exact rent minimum');
  }
  bindTerminalResolutionCertificateV2(decodeResolutionCertificateV2(certificateAccount.data), {
    receiptAccount: key(route.terminalCertificate, 'Resolution certificate').toBytes(),
    market: key(route.market, 'Market').toBytes(), sourceMaterial: market.resolutionPolicy,
    productRecordDigest: market.productRecord, generation: market.generation,
    selector: market.terminalWinner, outcomeCount: product.outcomeCount,
  });
  return Object.freeze({
    genesisHash: cluster.genesisHash, observedSlot: observation.slot, market: route.market,
    position: route.position, recipient: route.recipient, releaseSet: request.releaseSet,
    lookupTable: manifest.lookupTable,
  });
}

export type PreparedCheckedLiveDevnetPayoutV3 = PreparedWalletTerminalPayoutV3 & Readonly<{
  admission: CheckedLiveDevnetPayoutAdmissionV3;
}>;

/** Authenticate checked devnet, then reuse the recoverable wallet transaction builder. */
export async function prepareCheckedLiveDevnetWalletTerminalPayoutV3(
  client: SolanaRpcClient,
  manifest: WalletTerminalPayoutManifestV3,
  payer: string,
): Promise<PreparedCheckedLiveDevnetPayoutV3> {
  const admission = await authenticateCheckedLiveDevnetPayoutPlanV3(client, manifest, payer);
  const prepared = await prepareWalletTerminalPayoutV3(client, manifest, payer, admission.observedSlot);
  return Object.freeze({ ...prepared, admission });
}

/** Reacquire every mutable pre-resource once and compile a fresh admitted message. */
export async function prepareWalletTerminalPayoutV3(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts' | 'latestMutationBlockhash'>,
  manifest: WalletTerminalPayoutManifestV3,
  payer: string,
  minimumFloor?: string,
): Promise<PreparedWalletTerminalPayoutV3> {
  const canonicalPayer = key(payer, 'fee payer').toBase58();
  if (canonicalPayer !== manifest.request.owner) throw new Error('the connected wallet is not the Position owner and sole payout authority');
  const addresses = [manifest.route.aggregate, manifest.route.position, manifest.route.custodyReplay, manifest.route.hoard, manifest.route.recipient, manifest.lookupTable];
  const floor = minimumFloor ?? await client.finalizedSlot();
  const observation = await client.multipleAccounts(addresses, floor);
  const account = (address: string) => observation.accounts.find((entry) => entry.address === address)?.account ?? null;
  const aggregate = material(account(manifest.route.aggregate), manifest.route.claimsProgram, 'Claims aggregate');
  const position = material(account(manifest.route.position), manifest.route.claimsProgram, 'Claims Position');
  const replay = material(account(manifest.route.custodyReplay), manifest.route.custodyProgram, 'Claims-role Custody replay');
  const hoard = material(account(manifest.route.hoard), manifest.route.tokenProgram, 'Hoard token account');
  const recipient = material(account(manifest.route.recipient), manifest.route.tokenProgram, 'recipient token account');
  const tableRaw = material(account(manifest.lookupTable), AddressLookupTableProgram.programId.toBase58(), 'address lookup table');
  let tableState: ReturnType<typeof AddressLookupTableAccount.deserialize>;
  try { tableState = AddressLookupTableAccount.deserialize(tableRaw.data); } catch { throw new Error('address lookup table has malformed data'); }
  const lookupTable = new AddressLookupTableAccount({ key: key(manifest.lookupTable, 'lookup table'), state: tableState });
  const report = await buildWalletTerminalPayoutV3({
    observedSlot: observation.slot, route: manifest.route, custodyContext: manifest.custodyContext,
    request: manifest.request, signedPacket: base64Bytes(manifest.signedPacketBase64, 'SignedDelta packet'),
    payout: manifest.payout, aggregateBytes: aggregate.data, positionBytes: position.data,
    custodyReplayBytes: replay.data, hoardTokenBytes: hoard.data, recipientTokenBytes: recipient.data,
  });
  const latest = await client.latestMutationBlockhash(observation.slot);
  return Object.freeze({ ...compileWalletTerminalPayoutV0(report, {
    payer: canonicalPayer, recentBlockhash: latest.blockhash, lookupTable, lookupObservedSlot: observation.slot,
  }), lookupTable: manifest.lookupTable });
}

function verifyFeeOnlyBalancesV3(transaction: TransactionMetaObservation, payer: string): void {
  if (transaction.accountAddresses.length !== transaction.preBalances.length
      || transaction.preBalances.length !== transaction.postBalances.length) {
    throw new Error('finalized payout balance vectors do not cover its exact account list');
  }
  const payerIndex = transaction.accountAddresses.indexOf(payer);
  if (payerIndex < 0 || transaction.accountAddresses.lastIndexOf(payer) !== payerIndex) {
    throw new Error('finalized payout does not name one exact fee payer');
  }
  const fee = decimal(transaction.feeLamports, 'finalized payout fee');
  for (let index = 0; index < transaction.preBalances.length; index += 1) {
    const before = decimal(transaction.preBalances[index]!, `finalized payout pre-balance ${index}`);
    const after = decimal(transaction.postBalances[index]!, `finalized payout post-balance ${index}`);
    if (index === payerIndex ? after + fee !== before : after !== before) {
      throw new Error('finalized payout lamport balances differ by more than the exact payer fee');
    }
  }
}

/**
 * Authenticate the finalized transaction envelope against one already-signed
 * packet. This is the sole client-side semantic owner for payout wire,
 * signature, fee-payer, lamport, and return-data completion facts.
 */
export function verifyFinalizedWalletTerminalPayoutTransactionV3(
  transaction: TransactionMetaObservation,
  signature: string,
  plan: PreparedWalletTerminalPayoutV3,
  signedWireBytes: Uint8Array,
): Uint8Array {
  if (transaction.signature !== signature || !transaction.succeeded) {
    throw new Error(`finalized payout signature or status refused: ${transaction.errorText ?? 'unknown failure'}`);
  }
  if (!(signedWireBytes instanceof Uint8Array) || signedWireBytes.length === 0) {
    throw new Error('saved signed payout packet is absent');
  }
  let expected: VersionedTransaction;
  let observed: VersionedTransaction;
  try {
    expected = VersionedTransaction.deserialize(signedWireBytes);
    observed = VersionedTransaction.deserialize(transaction.transactionBytes);
  } catch {
    throw new Error('saved or finalized payout packet is not one Solana transaction');
  }
  if (!same(expected.serialize(), signedWireBytes)
      || !same(observed.serialize(), transaction.transactionBytes)
      || !same(transaction.transactionBytes, signedWireBytes)) {
    throw new Error('finalized payout wire bytes differ from the exact signed journal packet');
  }
  if (!same(expected.message.serialize(), plan.transaction.message.serialize())
      || expected.signatures.length !== plan.requiredSigners.length
      || expected.signatures.some((candidate) => candidate.every((byte) => byte === 0))
      || observed.signatures.length !== expected.signatures.length
      || observed.signatures.some((candidate, index) => !same(candidate, expected.signatures[index]!))) {
    throw new Error('finalized payout message or signature vector differs from its exact saved plan');
  }
  const payer = expected.message.staticAccountKeys[0]?.toBase58();
  if (payer === undefined || plan.requiredSigners.length !== 1 || plan.requiredSigners[0] !== payer) {
    throw new Error('saved payout packet does not name its one exact fee payer and signer');
  }
  verifyFeeOnlyBalancesV3(transaction, payer);
  if (transaction.returnData === null || transaction.returnData.programId !== plan.report.route.claimsProgram) {
    throw new Error('finalized payout omitted the exact Claims-produced return receipt');
  }
  return new Uint8Array(transaction.returnData.data);
}

/** Read the finalized transaction and persisted resources, then verify all of them. */
export async function finalizeWalletTerminalPayoutV3(
  client: Pick<SolanaRpcClient, 'transaction' | 'finalizedSlot' | 'multipleAccounts'>,
  signature: string,
  plan: PreparedWalletTerminalPayoutV3,
  signedWireBytes: Uint8Array,
): Promise<Readonly<{ signature: string; observedSlot: string; payout: string }>> {
  const transaction = await client.transaction(signature);
  if (transaction === null) throw new Error('the payout transaction is not available at finalized commitment yet');
  const receiptBytes = verifyFinalizedWalletTerminalPayoutTransactionV3(
    transaction,
    signature,
    plan,
    signedWireBytes,
  );
  const floor = await client.finalizedSlot();
  if (BigInt(floor) < BigInt(transaction.slot)) throw new Error('the finalized account floor has not reached the payout transaction yet');
  const route = plan.report.route;
  const addresses = [route.aggregate, route.position, route.custodyReplay, route.hoard, route.recipient];
  const observation = await client.multipleAccounts(addresses, floor);
  if (BigInt(observation.slot) < BigInt(floor)) {
    throw new Error('payout poststate observation regressed below its finalized floor');
  }
  if (observation.accounts.length !== addresses.length
      || observation.accounts.some((entry, index) => entry.address !== addresses[index])) {
    throw new Error('payout poststate response substitutes its exact ordered account closure');
  }
  const account = (address: string) => observation.accounts.find((entry) => entry.address === address)?.account ?? null;
  const aggregate = material(account(route.aggregate), route.claimsProgram, 'post-payout Claims aggregate');
  const position = material(account(route.position), route.claimsProgram, 'post-payout Claims Position');
  const replay = material(account(route.custodyReplay), route.custodyProgram, 'post-payout Custody replay');
  const hoard = material(account(route.hoard), route.tokenProgram, 'post-payout Hoard');
  const recipient = material(account(route.recipient), route.tokenProgram, 'post-payout recipient');
  await verifyWalletTerminalPayoutPostconditionV3(plan.report, {
    receiptBytes, aggregateBytes: aggregate.data, positionBytes: position.data,
    custodyReplayBytes: replay.data, hoardTokenBytes: hoard.data, recipientTokenBytes: recipient.data,
  });
  return Object.freeze({ signature, observedSlot: observation.slot, payout: plan.report.payout });
}
