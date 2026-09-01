import { PublicKey } from '@solana/web3.js';

import { fromHex } from './bytes';
import {
  RATIONAL_OPEN_INPUT_FORMAT_V1,
  RATIONAL_OPEN_REQUEST_SCHEMA_HEX_V3,
} from './generated/rationalOpenWasmV1';
import {
  loadRationalOpenWasmV1,
  parseRationalOpenWasmPlanV1,
} from './rationalOpenWasmV1';

const MAX_U64 = 18_446_744_073_709_551_615n;
const ABSENT_REVISION = MAX_U64;

/** Rust-owned request schema identity generated with the Rational-open WASM artifact. */
export const OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3 = fromHex(
  RATIONAL_OPEN_REQUEST_SCHEMA_HEX_V3,
  'Rational-open request schema',
);

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

function identity(value: Uint8Array, field: string): string {
  if (value.length !== 32 || value.every((byte) => byte === 0)) {
    throw new Error(`${field} must be one nonzero 32-byte identity`);
  }
  return new PublicKey(value).toBase58();
}

function u64Text(value: bigint, field: string): string {
  if (value < 0n || value > MAX_U64) throw new Error(`${field} is outside canonical u64`);
  return value.toString();
}

function revision(value: bigint, field: string): string | null {
  return value === ABSENT_REVISION ? null : u64Text(value, field);
}

function exactInputJson(input: RationalOpenHotInputV3): string {
  return JSON.stringify({
    format: RATIONAL_OPEN_INPUT_FORMAT_V1,
    action: input.action,
    releaseSet: identity(input.releaseSet, 'release set'),
    market: input.market,
    graphId: identity(input.graphId, 'representation graph'),
    descriptorId: identity(input.descriptorId, 'representation descriptor'),
    actor: input.actor,
    receiptMint: input.receiptMint,
    receiptAccount: input.receiptAccount,
    representationAuthority: input.representationAuthority,
    tokenProgram: input.tokenProgram,
    expectedRepresentationRevision: u64Text(input.expectedRepresentationRevision, 'representation replay revision'),
    expectedClaimsMarketRevision: revision(input.expectedClaimsMarketRevision, 'Claims Market revision'),
    expectedActorPositionRevision: revision(input.expectedActorPositionRevision, 'actor Position revision'),
    expectedCustodyPositionRevision: revision(input.expectedCustodyPositionRevision, 'custody Position revision'),
    generation: u64Text(input.generation, 'Market generation'),
    quantity: u64Text(input.quantity, 'raw action quantity'),
    denominator: u64Text(input.denominator, 'descriptor denominator'),
    expectedReceiptSupply: u64Text(input.expectedReceiptSupply, 'receipt Mint supply'),
    outcomeCount: input.outcomeCount,
    selectedOutcome: input.selectedOutcome,
    assets: input.assets.map((asset, index) => ({
      shardMint: asset.shardMint,
      actorShardAccount: asset.actorShardAccount,
      structuredCustodyAccount: asset.structuredCustodyAccount,
      claimsCustodyOwner: asset.claimsCustodyOwner,
      coefficient: u64Text(asset.coefficient, `asset ${index} coefficient`),
      expectedShardSupply: u64Text(asset.expectedShardSupply, `asset ${index} shard supply`),
      expectedActorShards: u64Text(asset.expectedActorShards, `asset ${index} actor shards`),
      expectedStructuredShards: u64Text(asset.expectedStructuredShards, `asset ${index} Structured shards`),
    })),
  });
}

/**
 * Compile the exact parent-free Trading family and Claims child through the
 * canonical Rust request owner. The browser supplies data, never wire layout.
 */
export async function compileRationalOpenHotV3(input: RationalOpenHotInputV3): Promise<RationalOpenCompiledV3> {
  const source = exactInputJson(input);
  const wasm = await loadRationalOpenWasmV1();
  const plan = await parseRationalOpenWasmPlanV1(wasm.plan_rational_open_v1(source));
  const structured = input.action === 'issue-structured' || input.action === 'unwrap-structured';
  const expectedDeltas = input.assets.map((asset) => (structured ? asset.coefficient : input.denominator) * input.quantity);
  if (plan.action !== input.action || plan.assetCount !== input.assets.length
      || plan.rawQuantity !== input.quantity
      || plan.rawShardDeltas.length !== expectedDeltas.length
      || plan.rawShardDeltas.some((delta, index) => delta !== expectedDeltas[index])) {
    throw new Error('Rational-open Rust plan does not match the exact requested action and atoms');
  }
  return Object.freeze({
    action: plan.action,
    familyBytes: plan.familyBytes,
    familyDigest: plan.familyDigest,
    childRequest: plan.claimsChild,
    childDigest: plan.claimsChildDigest,
    assetCount: plan.assetCount,
    claimsAccountCount: plan.logicalClaimsAccounts,
    rawQuantity: plan.rawQuantity,
    rawReceiptDelta: plan.rawReceiptDelta,
    rawShardDeltas: plan.rawShardDeltas,
  });
}

export const RATIONAL_OPEN_ABSENT_REVISION_V3 = ABSENT_REVISION;
