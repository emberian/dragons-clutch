import { PublicKey } from '@solana/web3.js';

import { isZero, sha256 } from './bytes';
import * as Abi from './generated/rationalTerminalHotV3';

const MAX_U64 = 18_446_744_073_709_551_615n;

export type RationalTerminalAssetV3 = Readonly<{
  shardMint: string;
  actorShardAccount: string;
  structuredCustodyAccount: string;
  claimsCustodyOwner: string;
  coefficient: bigint;
  expectedShardSupply: bigint;
  expectedActorShards: bigint;
  expectedStructuredShards: bigint;
}>;

export type RationalTerminalHotInputV3 = Readonly<{
  releaseSet: Uint8Array;
  market: string;
  graphId: Uint8Array;
  descriptorId: Uint8Array;
  actor: string;
  receiptMint: string;
  representationAuthority: string;
  tokenProgram: string;
  realm: string;
  collateralRecipient: string;
  expectedRepresentationRevision: bigint;
  expectedClaimsMarketRevision: bigint;
  expectedCustodyPositionRevision: bigint;
  expectedCustodyReplayRevision: bigint;
  generation: bigint;
  quantity: bigint;
  denominator: bigint;
  expectedReceiptSupply: bigint;
  outcomeCount: number;
  selectedOutcome: number;
  asset: RationalTerminalAssetV3;
}>;

export type RationalTerminalCompiledV3 = Readonly<{
  familyBytes: Uint8Array;
  familyDigest: Uint8Array;
  childRequest: Uint8Array;
  childDigest: Uint8Array;
  claimsAccountCount: 49;
  rawQuantity: bigint;
  rawShardBurn: bigint;
  payoutPolicy: 'product-derived-including-zero';
}>;

function identity(value: Uint8Array, field: string): Uint8Array {
  if (value.length !== 32 || isZero(value)) throw new Error(`${field} must be one nonzero 32-byte identity`);
  return value;
}

function key(value: string, field: string): Uint8Array {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return identity(parsed.toBytes(), field);
}

function unsigned(value: bigint, field: string): bigint {
  if (value < 0n || value > MAX_U64) throw new Error(`${field} is outside canonical u64`);
  return value;
}

function putU16(output: Uint8Array, offset: number, value: number): void {
  new DataView(output.buffer, output.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU32(output: Uint8Array, offset: number, value: number): void {
  new DataView(output.buffer, output.byteOffset + offset, 4).setUint32(0, value, true);
}

function putU64(output: Uint8Array, offset: number, value: bigint): void {
  new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, unsigned(value, `u64 at ${offset}`), true);
}

function distinct(values: ReadonlyArray<Uint8Array>, field: string): void {
  const keys = values.map((value) => new PublicKey(value).toBase58());
  if (new Set(keys).size !== keys.length) throw new Error(`${field} aliases two distinct roles`);
}

export function encodeRationalTerminalHotRequestV3(input: RationalTerminalHotInputV3): Uint8Array {
  if (!Number.isInteger(input.outcomeCount) || input.outcomeCount <= 0 || input.outcomeCount > 0xffff_ffff
      || !Number.isInteger(input.selectedOutcome) || input.selectedOutcome < 0
      || input.selectedOutcome >= input.outcomeCount) {
    throw new Error('Product outcome width or selected outcome is outside runtime u32');
  }
  if (input.quantity === 0n || input.denominator === 0n || input.generation === 0n
      || input.expectedRepresentationRevision === MAX_U64) {
    throw new Error('terminal quantity, denominator, and next replay revision must be executable');
  }
  const rawShardBurn = unsigned(input.denominator, 'terminal denominator') * unsigned(input.quantity, 'terminal quantity');
  unsigned(rawShardBurn, 'terminal raw shard burn');
  const output = new Uint8Array(Abi.RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3);
  output.set(Abi.RATIONAL_TERMINAL_HOT_MAGIC_V3, Abi.RATIONAL_TERMINAL_HOT_MAGIC_OFFSET_V3);
  putU16(output, Abi.RATIONAL_TERMINAL_HOT_VERSION_OFFSET_V3, Abi.RATIONAL_TERMINAL_HOT_VERSION_V3);
  output[Abi.RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3] = Abi.ACTION_REDEEM_TERMINAL;
  output[Abi.RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3] = Abi.CALLER_ROLE_TRADING;
  const identities: ReadonlyArray<readonly [number, Uint8Array]> = [
    [Abi.REQUEST_RELEASE_SET_OFFSET, identity(input.releaseSet, 'release set')],
    [Abi.REQUEST_MARKET_OFFSET, key(input.market, 'Market')],
    [Abi.REQUEST_GRAPH_ID_OFFSET, identity(input.graphId, 'representation graph')],
    [Abi.REQUEST_DESCRIPTOR_ID_OFFSET, identity(input.descriptorId, 'representation descriptor')],
    [Abi.REQUEST_ACTOR_OFFSET, key(input.actor, 'actor')],
    [Abi.REQUEST_RECEIPT_MINT_OFFSET, key(input.receiptMint, 'receipt Mint')],
    [Abi.REQUEST_REPRESENTATION_AUTHORITY_OFFSET, key(input.representationAuthority, 'representation authority')],
    [Abi.REQUEST_TOKEN_PROGRAM_OFFSET, key(input.tokenProgram, 'Token program')],
    [Abi.REQUEST_REALM_OFFSET, key(input.realm, 'Realm')],
    [Abi.REQUEST_COLLATERAL_RECIPIENT_OFFSET, key(input.collateralRecipient, 'collateral recipient')],
  ];
  for (const [offset, value] of identities) output.set(value, offset);
  // Receipt Account and parent context are canonically absent in this terminal
  // family request; the authenticated Hot adapter owns the latter digest.
  const scalars: ReadonlyArray<readonly [number, bigint]> = [
    [Abi.REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET, input.expectedRepresentationRevision],
    [Abi.REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET, input.expectedClaimsMarketRevision],
    [Abi.REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET, MAX_U64],
    [Abi.REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET, input.expectedCustodyPositionRevision],
    [Abi.REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET, input.expectedCustodyReplayRevision],
    [Abi.REQUEST_GENERATION_OFFSET, input.generation], [Abi.REQUEST_QUANTITY_OFFSET, input.quantity],
    [Abi.REQUEST_DENOMINATOR_OFFSET, input.denominator], [Abi.REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET, input.expectedReceiptSupply],
  ];
  for (const [offset, value] of scalars) putU64(output, offset, value);
  putU32(output, Abi.REQUEST_OUTCOME_COUNT_OFFSET, input.outcomeCount);
  putU32(output, Abi.REQUEST_SELECTED_OUTCOME_OFFSET, input.selectedOutcome);
  putU32(output, Abi.REQUEST_ASSET_COUNT_OFFSET, Abi.RATIONAL_TERMINAL_HOT_FIXED_ASSET_COUNT_V3);
  const assetOffset = Abi.REQUEST_HEADER_BYTES_V2;
  const assetIdentities = [
    key(input.asset.shardMint, 'shard Mint'), key(input.asset.actorShardAccount, 'actor shard account'),
    key(input.asset.structuredCustodyAccount, 'Structured custody account'), key(input.asset.claimsCustodyOwner, 'Claims custody owner'),
  ];
  distinct(assetIdentities.slice(0, 3), 'terminal asset');
  output.set(assetIdentities[0], assetOffset + Abi.ASSET_SHARD_MINT_OFFSET);
  output.set(assetIdentities[1], assetOffset + Abi.ASSET_ACTOR_SHARD_ACCOUNT_OFFSET);
  output.set(assetIdentities[2], assetOffset + Abi.ASSET_STRUCTURED_CUSTODY_ACCOUNT_OFFSET);
  output.set(assetIdentities[3], assetOffset + Abi.ASSET_CLAIMS_CUSTODY_OWNER_OFFSET);
  for (const [offset, value] of [
    [Abi.ASSET_COEFFICIENT_OFFSET, input.asset.coefficient],
    [Abi.ASSET_EXPECTED_SHARD_SUPPLY_OFFSET, input.asset.expectedShardSupply],
    [Abi.ASSET_EXPECTED_ACTOR_SHARDS_OFFSET, input.asset.expectedActorShards],
    [Abi.ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET, input.asset.expectedStructuredShards],
  ] as const) putU64(output, assetOffset + offset, value);
  if (input.asset.expectedActorShards < rawShardBurn) {
    throw new Error('actor shard balance cannot fund exact terminal burn');
  }
  return output;
}

export async function specializeRationalTerminalChildV2(family: Uint8Array): Promise<Readonly<{
  familyDigest: Uint8Array;
  childRequest: Uint8Array;
}>> {
  if (family.length !== Abi.RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3
      || !family.slice(0, 8).every((value, index) => value === Abi.RATIONAL_TERMINAL_HOT_MAGIC_V3[index])
      || !family.slice(Abi.REQUEST_PARENT_CONTEXT_OFFSET, Abi.REQUEST_PARENT_CONTEXT_OFFSET + 32).every((value) => value === 0)) {
    throw new Error('Rational terminal family bytes are not canonical Hot V3');
  }
  const familyDigest = await sha256(family);
  const childRequest = family.slice();
  childRequest.set(Abi.REQUEST_MAGIC_V2, Abi.REQUEST_MAGIC_OFFSET);
  putU16(childRequest, Abi.REQUEST_VERSION_OFFSET, Abi.PHYSICAL_ABI_VERSION_V2);
  childRequest.set(familyDigest, Abi.REQUEST_PARENT_CONTEXT_OFFSET);
  return Object.freeze({ familyDigest, childRequest });
}

/**
 * Compile the fixed terminal family and exact Claims child without guessing a
 * payout. The ProductV3 evaluator owns payout and explicitly admits zero.
 */
export async function compileRationalTerminalHotV3(input: RationalTerminalHotInputV3): Promise<RationalTerminalCompiledV3> {
  const familyBytes = encodeRationalTerminalHotRequestV3(input);
  const specialized = await specializeRationalTerminalChildV2(familyBytes);
  const childDigest = await sha256(specialized.childRequest);
  return Object.freeze({
    familyBytes,
    familyDigest: specialized.familyDigest,
    childRequest: specialized.childRequest,
    childDigest,
    claimsAccountCount: 49,
    rawQuantity: input.quantity,
    rawShardBurn: input.denominator * input.quantity,
    payoutPolicy: 'product-derived-including-zero',
  });
}
