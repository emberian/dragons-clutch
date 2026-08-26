import { PublicKey } from '@solana/web3.js';

import { isZero, sha256 } from './bytes';
import * as Abi from './generated/rationalTerminalHotV3';

const MAX_U64 = 18_446_744_073_709_551_615n;
const ABSENT_REVISION = MAX_U64;
const OPEN_MAGIC = new TextEncoder().encode('DCRROH03');
const OPEN_VERSION = 3;
const CALLER_ROLE_TRADING = 2;

export const OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3 = Uint8Array.from([
  0x24, 0x6d, 0x5c, 0x88, 0xd4, 0x18, 0x63, 0x85, 0x5d, 0xec, 0xe2, 0xa4, 0xbb, 0x87, 0x4e, 0xac,
  0x87, 0x33, 0xc7, 0x81, 0x06, 0x0c, 0x1b, 0xb3, 0x48, 0x8c, 0xb4, 0xde, 0xaf, 0x6a, 0xdb, 0x06,
]);

export type RationalOpenActionV3 = 'denominate' | 'reconstitute' | 'issue-structured' | 'unwrap-structured';

export type RationalOpenAssetV3 = Readonly<{
  shardMint: string;
  actorShardAccount: string;
  structuredCustodyAccount: string;
  claimsCustodyOwner: string;
  coefficient: bigint;
  expectedShardSupply: bigint;
  expectedActorShards: bigint;
  expectedStructuredShards: bigint;
}>;

export type RationalOpenHotInputV3 = Readonly<{
  action: RationalOpenActionV3;
  releaseSet: Uint8Array;
  market: string;
  graphId: Uint8Array;
  descriptorId: Uint8Array;
  actor: string;
  receiptMint: string;
  receiptAccount: string | null;
  representationAuthority: string;
  tokenProgram: string;
  expectedRepresentationRevision: bigint;
  expectedClaimsMarketRevision: bigint;
  expectedActorPositionRevision: bigint;
  expectedCustodyPositionRevision: bigint;
  generation: bigint;
  quantity: bigint;
  denominator: bigint;
  expectedReceiptSupply: bigint;
  outcomeCount: number;
  selectedOutcome: number | null;
  assets: ReadonlyArray<RationalOpenAssetV3>;
}>;

export type RationalOpenCompiledV3 = Readonly<{
  action: RationalOpenActionV3;
  familyBytes: Uint8Array;
  familyDigest: Uint8Array;
  childRequest: Uint8Array;
  childDigest: Uint8Array;
  assetCount: number;
  claimsAccountCount: number;
  rawQuantity: bigint;
  rawReceiptDelta: bigint;
  rawShardDeltas: ReadonlyArray<bigint>;
}>;

function actionTag(action: RationalOpenActionV3): number {
  switch (action) {
    case 'denominate': return 1;
    case 'reconstitute': return 2;
    case 'issue-structured': return 3;
    case 'unwrap-structured': return 4;
  }
}

function structured(action: RationalOpenActionV3): boolean {
  return action === 'issue-structured' || action === 'unwrap-structured';
}

function identity(value: Uint8Array, field: string): Uint8Array {
  if (value.length !== 32 || isZero(value)) throw new Error(`${field} must be one nonzero 32-byte identity`);
  return value;
}

function key(value: string, field: string): Uint8Array {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return identity(parsed.toBytes(), field);
}

function u64(value: bigint, field: string): bigint {
  if (value < 0n || value > MAX_U64) throw new Error(`${field} is outside canonical u64`);
  return value;
}

function putU16(output: Uint8Array, offset: number, value: number): void {
  new DataView(output.buffer, output.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU32(output: Uint8Array, offset: number, value: number): void {
  new DataView(output.buffer, output.byteOffset + offset, 4).setUint32(0, value, true);
}

function putU64(output: Uint8Array, offset: number, value: bigint, field: string): void {
  new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, u64(value, field), true);
}

function exactProduct(left: bigint, right: bigint, field: string): bigint {
  const value = u64(left, `${field} left operand`) * u64(right, `${field} right operand`);
  return u64(value, field);
}

function requireDistinct(values: ReadonlyArray<Uint8Array>, field: string): void {
  const keys = values.map((value) => new PublicKey(value).toBase58());
  if (new Set(keys).size !== keys.length) throw new Error(`${field} aliases two distinct physical identities`);
}

function validateShape(input: RationalOpenHotInputV3): Readonly<{
  isStructured: boolean;
  receipt: Uint8Array;
  selected: number;
}> {
  const isStructured = structured(input.action);
  if (!Number.isInteger(input.outcomeCount) || input.outcomeCount <= 0 || input.outcomeCount > 0xffff_ffff) {
    throw new Error('Product outcome count is outside runtime u32');
  }
  if (input.quantity === 0n || input.denominator === 0n || input.generation === 0n) {
    throw new Error('open quantity, denominator, and Market generation must be nonzero');
  }
  u64(input.expectedRepresentationRevision, 'representation replay revision');
  u64(input.expectedReceiptSupply, 'receipt Mint supply');
  const expectedAssets = isStructured ? input.outcomeCount : 1;
  if (input.assets.length !== expectedAssets) {
    throw new Error(`open action requires exactly ${isStructured ? 'N' : 'one'} asset row; received ${input.assets.length}`);
  }
  const selected = input.selectedOutcome ?? 0xffff_ffff;
  if (isStructured) {
    if (input.selectedOutcome !== null || input.receiptAccount === null
        || input.expectedClaimsMarketRevision !== ABSENT_REVISION
        || input.expectedActorPositionRevision !== ABSENT_REVISION
        || input.expectedCustodyPositionRevision !== ABSENT_REVISION) {
      throw new Error('Structured open must use selected=u32::MAX, one receipt account, and absent Claims/Position revisions');
    }
  } else if (input.receiptAccount !== null || input.selectedOutcome === null
      || !Number.isInteger(input.selectedOutcome) || input.selectedOutcome < 0
      || input.selectedOutcome >= input.outcomeCount
      || input.expectedClaimsMarketRevision === ABSENT_REVISION
      || input.expectedActorPositionRevision === ABSENT_REVISION
      || input.expectedCustodyPositionRevision === ABSENT_REVISION) {
    throw new Error('selected open must bind one in-domain outcome, no receipt account, and live Claims/Position revisions');
  }
  const receipt = input.receiptAccount === null ? new Uint8Array(32) : key(input.receiptAccount, 'actor receipt account');
  if (!isStructured && selected !== input.selectedOutcome) throw new Error('selected outcome is not canonical');
  return Object.freeze({ isStructured, receipt, selected });
}

function assetDeltas(input: RationalOpenHotInputV3): Readonly<{ receipt: bigint; shards: ReadonlyArray<bigint> }> {
  const deltas = input.assets.map((asset, index) => {
    const coefficient = u64(asset.coefficient, `asset ${index} coefficient`);
    if (coefficient === 0n) throw new Error(`asset ${index} coefficient is zero`);
    const amount = exactProduct(coefficient, input.quantity, `asset ${index} raw shard delta`);
    if ((input.action === 'reconstitute' || input.action === 'issue-structured') && asset.expectedActorShards < amount) {
      throw new Error(`asset ${index} actor balance cannot fund the exact raw shard debit`);
    }
    return amount;
  });
  const receipt = structured(input.action) ? input.quantity : 0n;
  if (input.action === 'unwrap-structured' && input.expectedReceiptSupply < receipt) {
    throw new Error('receipt Mint supply cannot fund the exact raw Structured burn');
  }
  return Object.freeze({ receipt, shards: Object.freeze(deltas) });
}

/** Encode the sole parent-free open-family wire. All economic values are raw atoms. */
export function encodeRationalOpenHotRequestV3(input: RationalOpenHotInputV3): Uint8Array {
  const shape = validateShape(input);
  assetDeltas(input);
  const output = new Uint8Array(Abi.REQUEST_HEADER_BYTES_V2 + Abi.ASSET_BYTES_V2 * input.assets.length);
  output.set(OPEN_MAGIC, Abi.REQUEST_MAGIC_OFFSET);
  putU16(output, Abi.REQUEST_VERSION_OFFSET, OPEN_VERSION);
  output[Abi.REQUEST_ACTION_OFFSET] = actionTag(input.action);
  output[Abi.REQUEST_CALLER_ROLE_OFFSET] = CALLER_ROLE_TRADING;
  const identities: ReadonlyArray<readonly [number, Uint8Array]> = [
    [Abi.REQUEST_RELEASE_SET_OFFSET, identity(input.releaseSet, 'release set')],
    [Abi.REQUEST_MARKET_OFFSET, key(input.market, 'Market')],
    [Abi.REQUEST_GRAPH_ID_OFFSET, identity(input.graphId, 'representation graph')],
    [Abi.REQUEST_DESCRIPTOR_ID_OFFSET, identity(input.descriptorId, 'representation descriptor')],
    [Abi.REQUEST_ACTOR_OFFSET, key(input.actor, 'actor')],
    [Abi.REQUEST_RECEIPT_MINT_OFFSET, key(input.receiptMint, 'receipt Mint')],
    [Abi.REQUEST_RECEIPT_ACCOUNT_OFFSET, shape.receipt],
    [Abi.REQUEST_REPRESENTATION_AUTHORITY_OFFSET, key(input.representationAuthority, 'representation authority')],
    [Abi.REQUEST_TOKEN_PROGRAM_OFFSET, key(input.tokenProgram, 'Token program')],
  ];
  identities.forEach(([offset, value]) => output.set(value, offset));
  const headerKeys = identities.filter(([offset]) => offset !== Abi.REQUEST_RECEIPT_ACCOUNT_OFFSET).map(([, value]) => value);
  requireDistinct(headerKeys.slice(1), 'open header');
  const scalars: ReadonlyArray<readonly [number, bigint, string]> = [
    [Abi.REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET, input.expectedRepresentationRevision, 'representation replay revision'],
    [Abi.REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET, input.expectedClaimsMarketRevision, 'Claims Market revision'],
    [Abi.REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET, input.expectedActorPositionRevision, 'actor Position revision'],
    [Abi.REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET, input.expectedCustodyPositionRevision, 'custody Position revision'],
    [Abi.REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET, ABSENT_REVISION, 'absent Custody replay revision'],
    [Abi.REQUEST_GENERATION_OFFSET, input.generation, 'Market generation'],
    [Abi.REQUEST_QUANTITY_OFFSET, input.quantity, 'raw action quantity'],
    [Abi.REQUEST_DENOMINATOR_OFFSET, input.denominator, 'descriptor denominator'],
    [Abi.REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET, input.expectedReceiptSupply, 'receipt Mint supply'],
  ];
  scalars.forEach(([offset, value, field]) => putU64(output, offset, value, field));
  putU32(output, Abi.REQUEST_OUTCOME_COUNT_OFFSET, input.outcomeCount);
  putU32(output, Abi.REQUEST_SELECTED_OUTCOME_OFFSET, shape.selected);
  putU32(output, Abi.REQUEST_ASSET_COUNT_OFFSET, input.assets.length);

  const physical: Uint8Array[] = [];
  input.assets.forEach((asset, index) => {
    const offset = Abi.REQUEST_HEADER_BYTES_V2 + index * Abi.ASSET_BYTES_V2;
    const keys = [
      key(asset.shardMint, `asset ${index} shard Mint`),
      key(asset.actorShardAccount, `asset ${index} actor shard account`),
      key(asset.structuredCustodyAccount, `asset ${index} Structured custody account`),
      key(asset.claimsCustodyOwner, `asset ${index} Claims custody owner`),
    ];
    requireDistinct(keys.slice(0, 3), `asset ${index}`);
    physical.push(...keys.slice(0, 3));
    output.set(keys[0], offset + Abi.ASSET_SHARD_MINT_OFFSET);
    output.set(keys[1], offset + Abi.ASSET_ACTOR_SHARD_ACCOUNT_OFFSET);
    output.set(keys[2], offset + Abi.ASSET_STRUCTURED_CUSTODY_ACCOUNT_OFFSET);
    output.set(keys[3], offset + Abi.ASSET_CLAIMS_CUSTODY_OWNER_OFFSET);
    putU64(output, offset + Abi.ASSET_COEFFICIENT_OFFSET, asset.coefficient, `asset ${index} coefficient`);
    putU64(output, offset + Abi.ASSET_EXPECTED_SHARD_SUPPLY_OFFSET, asset.expectedShardSupply, `asset ${index} shard supply`);
    putU64(output, offset + Abi.ASSET_EXPECTED_ACTOR_SHARDS_OFFSET, asset.expectedActorShards, `asset ${index} actor shards`);
    putU64(output, offset + Abi.ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET, asset.expectedStructuredShards, `asset ${index} Structured shards`);
  });
  requireDistinct(physical, 'open asset physical accounts');
  return output;
}

/** Specialize a family into the exact Claims request using its own SHA-256 parent. */
export async function compileRationalOpenHotV3(input: RationalOpenHotInputV3): Promise<RationalOpenCompiledV3> {
  const familyBytes = encodeRationalOpenHotRequestV3(input);
  const familyDigest = await sha256(familyBytes);
  const childRequest = familyBytes.slice();
  childRequest.set(Abi.REQUEST_MAGIC_V2, Abi.REQUEST_MAGIC_OFFSET);
  putU16(childRequest, Abi.REQUEST_VERSION_OFFSET, Abi.PHYSICAL_ABI_VERSION_V2);
  childRequest.set(familyDigest, Abi.REQUEST_PARENT_CONTEXT_OFFSET);
  const childDigest = await sha256(childRequest);
  const deltas = assetDeltas(input);
  return Object.freeze({
    action: input.action,
    familyBytes,
    familyDigest,
    childRequest,
    childDigest,
    assetCount: input.assets.length,
    claimsAccountCount: 32 + 4 * input.assets.length,
    rawQuantity: input.quantity,
    rawReceiptDelta: deltas.receipt,
    rawShardDeltas: deltas.shards,
  });
}

export const RATIONAL_OPEN_ABSENT_REVISION_V3 = ABSENT_REVISION;
