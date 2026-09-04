import {
  AddressLookupTableAccount,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { hex, requireNonzero, requireZero, slice, u16, u64 } from './bytes';
import {
  LIABILITY_BASIS_MARKET_BASIS_OFFSET,
  LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET,
  LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET,
  LIABILITY_BASIS_MARKET_GENERATION_OFFSET,
  LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
  LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET,
  LIABILITY_BASIS_MARKET_MAGIC_V2,
  LIABILITY_BASIS_MARKET_PRODUCT_OFFSET,
  LIABILITY_BASIS_MARKET_REALM_OFFSET,
  LIABILITY_BASIS_MARKET_REGISTRY_OFFSET,
  LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET,
  LIABILITY_BASIS_MARKET_REVISION_OFFSET,
  LIABILITY_BASIS_MARKET_SEED_V2 as CLAIMS_AGGREGATE_SEED,
  LIABILITY_BASIS_POSITION_BASIS_OFFSET,
  LIABILITY_BASIS_POSITION_CLAIM_COUNT_OFFSET,
  LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
  LIABILITY_BASIS_POSITION_MAGIC_V2,
  LIABILITY_BASIS_POSITION_MARKET_OFFSET,
  LIABILITY_BASIS_POSITION_OWNER_OFFSET,
  LIABILITY_BASIS_POSITION_RESERVED_OFFSET,
  LIABILITY_BASIS_POSITION_REVISION_OFFSET,
  LIABILITY_BASIS_POSITION_SEED_V2 as POSITION_SEED,
  LIABILITY_BASIS_STATE_VERSION_V2,
} from './generated/coreFound';
import * as Hot from './generated/directInlineV3';
import {
  RATIONAL_REPLAY_ACTOR_OFFSET,
  RATIONAL_REPLAY_BYTES_V2,
  RATIONAL_REPLAY_DESCRIPTOR_OFFSET,
  RATIONAL_REPLAY_MAGIC_BYTES,
  RATIONAL_REPLAY_MAGIC_OFFSET,
  RATIONAL_REPLAY_MAGIC_V2,
  RATIONAL_REPLAY_RESERVED_BYTES,
  RATIONAL_REPLAY_RESERVED_OFFSET,
  RATIONAL_REPLAY_REVISION_OFFSET,
  RATIONAL_REPLAY_SEED_V2,
  RATIONAL_REPLAY_VERSION_OFFSET,
  RATIONAL_REPLAY_VERSION_V2,
} from './generated/rationalReplayV2';
import {
  OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3,
  RATIONAL_OPEN_ABSENT_REVISION_V3,
  compileRationalOpenHotV3,
  type RationalOpenActionV3,
  type RationalOpenAssetV3,
  type RationalOpenCompiledV3,
} from './rationalOpenHotV3';
import {
  acquireRationalHotAccountsV4,
  authenticateRationalProductBasisRecordV3,
  type RationalHotAccountMetaV4,
  type RationalHotRpcV4,
} from './rationalRetireReceiptV4';
import {
  TOKEN_2022_PROGRAM_ID,
  decodeToken2022BehaviorAccountV2,
  decodeToken2022BehaviorMintV2,
} from './rationalTokenV2';
import { SOLANA_PACKET_BYTES_V1 } from './solanaLimits';
import {
  UPGRADEABLE_LOADER_ID,
  RENT_SYSVAR_ID,
  SYSTEM_PROGRAM_ID,
} from './releaseRegistry';
import { type RpcAccount } from './rpc';
import { inspectRationalCapabilityCommonV4 } from './rationalCapabilityChainV4';

const MAX_U64 = 18_446_744_073_709_551_615n;
const ASSOCIATED_TOKEN_PROGRAM = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
const CUSTODY_OWNER_SEED = new TextEncoder().encode('dclutch:rational-claims:v2');
const SHARD_MINT_SEED = new TextEncoder().encode('dclutch:rational-shard-mint:v2');
const STRUCTURED_CUSTODY_SEED = new TextEncoder().encode('dclutch:rational-structured:v2');
const RECEIPT_MINT_SEED = new TextEncoder().encode('dclutch:rational-receipt:v2');
const REPRESENTATION_AUTHORITY_SEED = new TextEncoder().encode('dclutch:rational-authority:v2');
const REPLAY_SEED = new TextEncoder().encode(RATIONAL_REPLAY_SEED_V2);
const CALLER_AUTHORITY_SEED = new TextEncoder().encode('dclutch:role-authority:v1');
const ACCOUNT_PROFILE_HEADER = 40;

type Meta = RationalHotAccountMetaV4;
type OpenRpc = RationalHotRpcV4;

export type RationalOpenChainInspectionV4 = Readonly<{
  observedSlot: string;
  action: RationalOpenActionV3;
  payer: string;
  actor: string;
  market: string;
  generation: bigint;
  representationWidth: number;
  resultOutcomeCount: number;
  selectedOutcome: number | null;
  rawQuantity: bigint;
  displayDecimals: number;
  descriptorId: Uint8Array;
  tokenBehaviorDigest: Uint8Array;
  capabilityDigest: Uint8Array;
  rootDigest: Uint8Array;
  family: RationalOpenCompiledV3;
  fixedAccounts: ReadonlyArray<Meta>;
  physicalClaimsAccounts: ReadonlyArray<Meta>;
  lookupTable: AddressLookupTableAccount;
  poststate: RationalOpenPoststateV4;
  executionStatus: 'blocked';
  refusal: string;
}>;

export type RationalOpenTokenPoststateV4 = Readonly<{
  mint: string;
  mintSupply: bigint;
  actorAccount: string;
  actorAmount: bigint;
  structuredAccount: string;
  structuredAmount: bigint;
}>;

export type RationalOpenPositionPoststateV4 = Readonly<{
  address: string;
  owner: string;
  revision: bigint;
  balances: ReadonlyArray<bigint>;
}>;

export type RationalOpenPoststateContextV4 = Readonly<{
  claimsProgram: string;
  descriptorId: Uint8Array;
  actor: string;
  representationAuthority: string;
  aggregate: string;
  market: string;
  releaseSet: Uint8Array;
  registry: string;
  product: Uint8Array;
  realm: Uint8Array;
  generation: bigint;
  outcomes: number;
  basis: Uint8Array;
  custodyContext: Uint8Array;
}>;

/** Exact accounts and atoms that must exist after one finalized open action. */
export type RationalOpenPoststateV4 = Readonly<{
  context: RationalOpenPoststateContextV4;
  replay: Readonly<{ address: string; revision: bigint }>;
  aggregate: Readonly<{ address: string; revision: bigint; balances: ReadonlyArray<bigint> }> | null;
  positions: ReadonlyArray<RationalOpenPositionPoststateV4>;
  receipt: Readonly<{ mint: string; supply: bigint; account: string; amount: bigint }> | null;
  assets: ReadonlyArray<RationalOpenTokenPoststateV4>;
}>;

export type RationalOpenFinalizedPoststateV4 = Readonly<{
  observedSlot: string;
  action: RationalOpenActionV3;
  poststate: RationalOpenPoststateV4;
}>;

export type RationalOpenCandidateV4 = Readonly<{
  transaction: VersionedTransaction;
  instruction: TransactionInstruction;
  outerBytes: Uint8Array;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
  loadedAddresses: number;
  logicalClaimsAccounts: number;
  physicalClaimsAccounts: number;
  executionStatus: 'blocked';
  refusal: string;
}>;

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function u32(bytes: Uint8Array, offset: number): number {
  const value = slice(bytes, offset, 4);
  return new DataView(value.buffer, value.byteOffset, value.byteLength).getUint32(0, true);
}

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU32(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 4).setUint32(0, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  if (value < 0n || value > MAX_U64) throw new Error('Hot scalar is outside canonical u64');
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function addU64(left: bigint, right: bigint, field: string): bigint {
  const value = left + right;
  if (left < 0n || right < 0n || value > MAX_U64) throw new Error(`${field} overflows canonical u64`);
  return value;
}

function subtractU64(left: bigint, right: bigint, field: string): bigint {
  if (left < 0n || right < 0n || left < right) throw new Error(`${field} underflows canonical u64`);
  return left - right;
}

function withBalance(balances: ReadonlyArray<bigint>, index: number, value: bigint, field: string): ReadonlyArray<bigint> {
  if (!Number.isSafeInteger(index) || index < 0 || index >= balances.length) throw new Error(`${field} outcome is outside the Claims vector`);
  const output = [...balances];
  output[index] = value;
  return Object.freeze(output);
}

function requireExactBalances(observed: ReadonlyArray<bigint>, expected: ReadonlyArray<bigint>, field: string): void {
  if (observed.length !== expected.length || observed.some((value, index) => value !== expected[index])) {
    throw new Error(`${field} differs from the exact finalized poststate`);
  }
}

function key(value: string, field: string): PublicKey {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return parsed;
}

function required(accounts: ReadonlyMap<string, RpcAccount | null>, address: string, field: string): RpcAccount {
  const account = accounts.get(address);
  if (account === null || account === undefined) throw new Error(`${field} is absent at finalized commitment`);
  return account;
}

function roleMeta(address: string, isSigner = false, isWritable = false): Meta {
  return Object.freeze({ address: key(address, 'account meta').toBase58(), isSigner, isWritable });
}

function le32(value: number): Uint8Array {
  if (!Number.isInteger(value) || value < 0 || value > 0xffff_ffff) throw new Error('outcome is outside runtime u32');
  const bytes = new Uint8Array(4); putU32(bytes, 0, value); return bytes;
}

function associated(owner: PublicKey, mint: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([owner.toBytes(), key(TOKEN_2022_PROGRAM_ID, 'Token-2022 program').toBytes(), mint.toBytes()], ASSOCIATED_TOKEN_PROGRAM)[0];
}

function actionSelector(action: RationalOpenActionV3): number {
  return action === 'denominate' ? 1 : action === 'reconstitute' ? 2 : action === 'issue-structured' ? 3 : 4;
}

function selectedAction(action: RationalOpenActionV3): boolean {
  return action === 'denominate' || action === 'reconstitute';
}

export type RationalClaimsAggregateV2 = Readonly<{
  revision: bigint;
  basis: Uint8Array;
  custodyContext: Uint8Array;
  balances: ReadonlyArray<bigint>;
}>;
export function decodeRationalClaimsAggregateV2(bytes: Uint8Array, input: Readonly<{
  market: string; releaseSet: Uint8Array; registry: string; product: Uint8Array; realm: Uint8Array; generation: bigint; outcomes: number;
}>): RationalClaimsAggregateV2 {
  if (bytes.length !== LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + input.outcomes * 8
      || !same(slice(bytes, 0, 8), LIABILITY_BASIS_MARKET_MAGIC_V2)
      || u16(bytes, 8) !== LIABILITY_BASIS_STATE_VERSION_V2
      || u32(bytes, LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET) !== input.outcomes) {
    throw new Error('Claims aggregate has the wrong exact runtime-width ABI');
  }
  requireZero(bytes, 10, 2, 'Claims aggregate');
  for (const [observed, expected, field] of [
    [slice(bytes, LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET, 32), key(input.market, 'Market').toBytes(), 'Market'],
    [slice(bytes, LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET, 32), input.releaseSet, 'release set'],
    [slice(bytes, LIABILITY_BASIS_MARKET_REGISTRY_OFFSET, 32), key(input.registry, 'Registry').toBytes(), 'Registry'],
    [slice(bytes, LIABILITY_BASIS_MARKET_PRODUCT_OFFSET, 32), input.product, 'Product record'],
    [slice(bytes, LIABILITY_BASIS_MARKET_REALM_OFFSET, 32), input.realm, 'Realm'],
  ] as const) if (!same(observed, expected)) throw new Error(`Claims aggregate ${field} differs from Core`);
  if (u64(bytes, LIABILITY_BASIS_MARKET_GENERATION_OFFSET) !== input.generation) throw new Error('Claims aggregate generation differs from Core');
  const basis = slice(bytes, LIABILITY_BASIS_MARKET_BASIS_OFFSET, 32); requireNonzero(basis, 'Claims semantic basis');
  const custodyContext = slice(bytes, LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET, 32); requireNonzero(custodyContext, 'Claims custody context');
  const balances = Object.freeze(Array.from({ length: input.outcomes }, (_, index) => u64(bytes, LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + index * 8)));
  return Object.freeze({ revision: u64(bytes, LIABILITY_BASIS_MARKET_REVISION_OFFSET), basis, custodyContext, balances });
}

export type RationalClaimsPositionStateV2 = Readonly<{ revision: bigint; balances: ReadonlyArray<bigint> }>;
export function decodeRationalClaimsPositionStateV2(bytes: Uint8Array, aggregate: string, owner: string, basis: Uint8Array, outcomes: number): RationalClaimsPositionStateV2 {
  if (!Number.isSafeInteger(outcomes) || outcomes <= 0) throw new Error('Claims Position outcome width is not positive');
  const revision = decodeRationalClaimsPositionV2(bytes, aggregate, owner, basis, outcomes);
  const balances = Object.freeze(Array.from({ length: outcomes }, (_, index) => u64(bytes, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + index * 8)));
  return Object.freeze({ revision, balances });
}

export function decodeRationalClaimsPositionV2(bytes: Uint8Array, aggregate: string, owner: string, basis: Uint8Array, outcomes: number): bigint {
  if (bytes.length !== LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + outcomes * 8
      || !same(slice(bytes, 0, 8), LIABILITY_BASIS_POSITION_MAGIC_V2)
      || u16(bytes, 8) !== LIABILITY_BASIS_STATE_VERSION_V2
      || u32(bytes, LIABILITY_BASIS_POSITION_CLAIM_COUNT_OFFSET) !== outcomes) {
    throw new Error('Claims Position has the wrong exact runtime-width ABI');
  }
  requireZero(bytes, 10, 2, 'Claims Position'); requireZero(bytes, LIABILITY_BASIS_POSITION_RESERVED_OFFSET, 8, 'Claims Position');
  if (new PublicKey(slice(bytes, LIABILITY_BASIS_POSITION_MARKET_OFFSET, 32)).toBase58() !== aggregate
      || new PublicKey(slice(bytes, LIABILITY_BASIS_POSITION_OWNER_OFFSET, 32)).toBase58() !== owner
      || !same(slice(bytes, LIABILITY_BASIS_POSITION_BASIS_OFFSET, 32), basis)) throw new Error('Claims Position aggregate, owner, or semantic basis differs');
  return u64(bytes, LIABILITY_BASIS_POSITION_REVISION_OFFSET);
}

function decodeRationalRepresentationReplayStateV2(
  account: RpcAccount,
  claims: string,
  descriptor: Uint8Array,
  actor: string,
  allowVacant: boolean,
  refuseExhausted: boolean,
): bigint {
  if (account.owner === claims) {
    if (account.executable || account.data.length !== RATIONAL_REPLAY_BYTES_V2
        || !same(slice(account.data, RATIONAL_REPLAY_MAGIC_OFFSET, RATIONAL_REPLAY_MAGIC_BYTES), RATIONAL_REPLAY_MAGIC_V2)
        || u16(account.data, RATIONAL_REPLAY_VERSION_OFFSET) !== RATIONAL_REPLAY_VERSION_V2) throw new Error('representation replay has the wrong exact V2 ABI');
    requireZero(account.data, RATIONAL_REPLAY_RESERVED_OFFSET, RATIONAL_REPLAY_RESERVED_BYTES, 'representation replay');
    if (!same(slice(account.data, RATIONAL_REPLAY_DESCRIPTOR_OFFSET, 32), descriptor)
        || new PublicKey(slice(account.data, RATIONAL_REPLAY_ACTOR_OFFSET, 32)).toBase58() !== actor) throw new Error('representation replay descriptor or actor differs');
    const revision = u64(account.data, RATIONAL_REPLAY_REVISION_OFFSET);
    if (refuseExhausted && revision === MAX_U64) throw new Error('representation replay revision is exhausted');
    return revision;
  }
  if (allowVacant && account.owner === SYSTEM_PROGRAM_ID && !account.executable && account.data.length === 0 && BigInt(account.lamports) > 0n) {
    return 0n;
  }
  throw new Error(allowVacant
    ? 'new representation replay is not a funded data-free System account'
    : 'finalized representation replay is not exact Claims-owned V2 state');
}

export function decodeRationalRepresentationReplayV2(account: RpcAccount, claims: string, descriptor: Uint8Array, actor: string): bigint {
  return decodeRationalRepresentationReplayStateV2(account, claims, descriptor, actor, true, true);
}

/** Apply one Rust-owned raw-delta plan to authenticated Token-2022 prestates. */
export function projectRationalOpenTokenPoststateV4(input: Readonly<{
  action: RationalOpenActionV3;
  rawQuantity: bigint;
  rawShardDeltas: ReadonlyArray<bigint>;
  receipt: Readonly<{ mint: string; supply: bigint; account: string; amount: bigint }> | null;
  assets: ReadonlyArray<RationalOpenTokenPoststateV4>;
}>): Readonly<Pick<RationalOpenPoststateV4, 'receipt' | 'assets'>> {
  const selected = selectedAction(input.action);
  if (input.rawQuantity <= 0n || input.rawQuantity > MAX_U64
      || input.assets.length !== input.rawShardDeltas.length
      || (selected && (input.assets.length !== 1 || input.receipt !== null))
      || (!selected && (input.assets.length === 0 || input.receipt === null))) {
    throw new Error('Rational open Token poststate input differs from the exact action shape');
  }
  const assets = Object.freeze(input.assets.map((observed, index): RationalOpenTokenPoststateV4 => {
    const delta = input.rawShardDeltas[index];
    if (delta === undefined || delta < 0n || delta > MAX_U64) throw new Error(`Rational open plan has an invalid raw shard delta ${index}`);
    let mintSupply = observed.mintSupply;
    let actorAmount = observed.actorAmount;
    let structuredAmount = observed.structuredAmount;
    if (input.action === 'denominate') {
      mintSupply = addU64(mintSupply, delta, `shard Mint ${index} supply`);
      actorAmount = addU64(actorAmount, delta, `actor shard ${index} balance`);
    } else if (input.action === 'reconstitute') {
      mintSupply = subtractU64(mintSupply, delta, `shard Mint ${index} supply`);
      actorAmount = subtractU64(actorAmount, delta, `actor shard ${index} balance`);
    } else if (input.action === 'issue-structured') {
      actorAmount = subtractU64(actorAmount, delta, `actor shard ${index} balance`);
      structuredAmount = addU64(structuredAmount, delta, `Structured custody ${index} balance`);
    } else {
      actorAmount = addU64(actorAmount, delta, `actor shard ${index} balance`);
      structuredAmount = subtractU64(structuredAmount, delta, `Structured custody ${index} balance`);
    }
    return Object.freeze({ ...observed, mintSupply, actorAmount, structuredAmount });
  }));
  let receipt: RationalOpenPoststateV4['receipt'] = null;
  if (!selected) {
    const observed = input.receipt;
    if (observed === null) throw new Error('Structured Rational open lacks its authenticated receipt account');
    const issuing = input.action === 'issue-structured';
    receipt = Object.freeze({
      ...observed,
      supply: issuing
        ? addU64(observed.supply, input.rawQuantity, 'receipt Mint supply')
        : subtractU64(observed.supply, input.rawQuantity, 'receipt Mint supply'),
      amount: issuing
        ? addU64(observed.amount, input.rawQuantity, 'actor receipt balance')
        : subtractU64(observed.amount, input.rawQuantity, 'actor receipt balance'),
    });
  }
  return Object.freeze({ receipt, assets });
}

function validateProgramAccount(address: string, account: RpcAccount, field: string): void {
  if (!account.executable || key(address, field).toBase58() !== address) throw new Error(`${field} is not executable runtime code`);
}

export function compactRationalProfile11AccountsV4(profile: Uint8Array, tailCount: number, injected: ReadonlyArray<Meta>, child: ReadonlyArray<Meta>, accounts: ReadonlyMap<string, RpcAccount | null>): ReadonlyArray<Meta> {
  if (profile.length < ACCOUNT_PROFILE_HEADER || !same(slice(profile, 0, 8), Hot.MAGIC) || u16(profile, 8) !== 2 || u16(profile, 10) !== 11) {
    throw new Error('selected AccountProfile is not exact authenticated-route-alias Profile11');
  }
  const fixed = u16(profile, 12); const stride = u16(profile, 14); const fixedOps = u16(profile, 16); const itemOps = u16(profile, 18);
  const expectedWidth = ACCOUNT_PROFILE_HEADER + (fixed + stride) * Hot.RULE_BYTES + (fixedOps + itemOps) * Hot.OPERATION_BYTES;
  const logical = [...injected, ...child];
  if (profile.length !== expectedWidth || logical.length !== fixed + stride * tailCount) throw new Error('Profile11 bytes or runtime logical width differs from Product N');
  const representative = (coordinate: number): number => {
    const item = coordinate < fixed ? -1 : Math.floor((coordinate - fixed) / stride);
    const local = coordinate < fixed ? coordinate : fixed + ((coordinate - fixed) % stride);
    const offset = ACCOUNT_PROFILE_HEADER + local * Hot.RULE_BYTES;
    const alias = profile[offset + 2]; const index = u16(profile, offset + 4);
    if (alias === 0 && index === 0) return coordinate;
    if (alias === 1 && index < fixed && index < coordinate) return index;
    if (alias === 2 && item >= 0 && index < stride && index < (coordinate - fixed) % stride) return fixed + item * stride + index;
    throw new Error(`Profile11 logical coordinate ${coordinate} has a forward or undefined alias`);
  };
  const output: Meta[] = [];
  for (let coordinate = 0; coordinate < logical.length; coordinate += 1) {
    const itemLocal = coordinate < fixed ? coordinate : fixed + ((coordinate - fixed) % stride);
    const offset = ACCOUNT_PROFILE_HEADER + itemLocal * Hot.RULE_BYTES;
    const privileges = profile[offset] ?? 255;
    if ((privileges & ~7) !== 0) throw new Error(`Profile11 coordinate ${coordinate} has undefined privilege bits`);
    const observed = logical[coordinate]; const rep = representative(coordinate); const source = logical[rep];
    if (observed === undefined || source === undefined || observed.address !== source.address) throw new Error(`Profile11 alias ${coordinate} does not resolve to the same physical account`);
    const account = accounts.get(observed.address);
    const executable = account?.executable ?? false;
    if (observed.isWritable !== ((privileges & 2) !== 0) || (coordinate !== 5 && observed.isSigner !== ((privileges & 1) !== 0))
        || executable !== ((privileges & 4) !== 0)) throw new Error(`Profile11 coordinate ${coordinate} differs from authenticated route privileges`);
    const prestate = profile[offset + 3] ?? 255; const declared = u32(profile, offset + 8);
    const width = account?.data.length ?? 0;
    if (prestate > 4) throw new Error(`Profile11 coordinate ${coordinate} has an undefined prestate tag`);
    if ((prestate === 0 && width !== declared)
        || (prestate === 1 && width !== 0 && width !== declared)
        || (prestate === 2 && width < declared)) {
      throw new Error(`Profile11 coordinate ${coordinate} differs from its authenticated data geometry`);
    }
    if (rep === coordinate && coordinate >= injected.length) {
      let signer = false; let writable = false;
      for (let other = 0; other < logical.length; other += 1) {
        if (representative(other) === coordinate) {
          const otherLocal = other < fixed ? other : fixed + ((other - fixed) % stride);
          const bits = profile[ACCOUNT_PROFILE_HEADER + otherLocal * Hot.RULE_BYTES] ?? 0;
          signer ||= (bits & 1) !== 0; writable ||= (bits & 2) !== 0;
        }
      }
      output.push(roleMeta(observed.address, signer, writable));
    }
  }
  if (output.length === 0) throw new Error('Profile11 compacted the Claims route to an empty suffix');
  return Object.freeze(output);
}

// Build the exact 32+4*N Claims frame. Kept separate so tests can exercise
// ordered aliases without requiring an RPC fixture.
export function rationalOpenClaimsMetasV4(input: Readonly<{
  caller: string; trading: string; tradingProgramData: string; actor: string; authority: string;
  descriptorRaw: string; descriptorStaging: string; graphRaw: string; graphStaging: string;
  replay: string; aggregate: string; activation: string; claims: string; claimsProgramData: string; registry: string;
  market: string; core: string; coreProgramData: string; receiptMint: string; receiptAccount: string | null;
  actorPosition: string | null; linkedRaw: string; linkedStaging: string; productRaw: string; productStaging: string;
  domainRaw: string; domainStaging: string; portfolioRaw: string; portfolioStaging: string;
  assets: ReadonlyArray<Readonly<{ position: string; asset: RationalOpenAssetV3 }>>; structured: boolean;
}>): ReadonlyArray<Meta> {
  const placeholder = input.claims;
  const metas: Meta[] = [
    roleMeta(input.caller, true), roleMeta(input.trading), roleMeta(input.tradingProgramData), roleMeta(input.actor, true),
    roleMeta(input.authority), roleMeta(input.descriptorRaw), roleMeta(input.descriptorStaging), roleMeta(input.graphRaw),
    roleMeta(input.graphStaging), roleMeta(RENT_SYSVAR_ID), roleMeta(SYSTEM_PROGRAM_ID), roleMeta(input.replay, false, true),
    roleMeta(input.aggregate, false, !input.structured), roleMeta(input.activation), roleMeta(input.claims), roleMeta(input.claimsProgramData),
    roleMeta(input.registry), roleMeta(input.market), roleMeta(input.core), roleMeta(input.coreProgramData),
    roleMeta(input.receiptMint, false, input.structured), roleMeta(input.receiptAccount ?? placeholder, false, input.structured),
    roleMeta(TOKEN_2022_PROGRAM_ID), roleMeta(input.actorPosition ?? placeholder, false, !input.structured),
    roleMeta(input.linkedRaw), roleMeta(input.linkedStaging), roleMeta(input.productRaw), roleMeta(input.productStaging),
    roleMeta(input.domainRaw), roleMeta(input.domainStaging), roleMeta(input.portfolioRaw), roleMeta(input.portfolioStaging),
  ];
  // `distinct` lived here before physical ABI v3 and checked that a
  // coordinate's shard Mint, actor shard Account and Structured custody
  // Account named three different roles. Two of those three left the WIRE in
  // v3 -- they are derived now -- but all four still arrive in this FRAME, so
  // the check has its operands back and it is restored here rather than in the
  // request encoder where it used to live and would now compare one value with
  // itself.
  //
  // Its chain-side owner is `ClaimsSbfError::ReceiptAlias`, raised by
  // `authenticate_asset_identities` before it derives anything, and its
  // grammar-side owner is `ResolvedRequestV2::join`. This is the wallet-side
  // twin of the same property: a browser that assembles a frame naming the
  // receipt as a coordinate's own backing should say so before a signature is
  // requested, not discover it in a simulation log.
  const seen = new Map<string, string>();
  const claim = (address: string, role: string): void => {
    const prior = seen.get(address);
    if (prior !== undefined) throw new Error(`Claims frame names one account as both ${prior} and ${role}`);
    seen.set(address, role);
  };
  claim(input.receiptMint, 'the receipt Mint');
  if (input.receiptAccount !== null) claim(input.receiptAccount, 'the receipt Account');
  input.assets.forEach(({ position, asset }, index) => {
    claim(position, `Claims custody Position ${index}`);
    claim(asset.shardMint, `shard Mint ${index}`);
    claim(asset.actorShardAccount, `actor shard Account ${index}`);
    claim(asset.structuredCustodyAccount, `Structured custody Account ${index}`);
  });
  input.assets.forEach(({ position, asset }) => metas.push(
    roleMeta(position, false, !input.structured), roleMeta(asset.shardMint, false, !input.structured),
    roleMeta(asset.actorShardAccount, false, true), roleMeta(asset.structuredCustodyAccount, false, input.structured),
  ));
  return Object.freeze(metas);
}

export async function inspectRationalOpenChainV4(
  client: OpenRpc,
  input: Readonly<{
    action: RationalOpenActionV3; payer: string; actor: string; fixedAccounts: ReadonlyArray<string>;
    lookupTable: string; descriptorId: string; rawQuantity: bigint; selectedOutcome: number | null;
  }>,
): Promise<RationalOpenChainInspectionV4> {
  if (input.rawQuantity <= 0n || input.rawQuantity > MAX_U64) throw new Error('raw action quantity must be 1..u64::MAX atoms');
  const common = await inspectRationalCapabilityCommonV4(client, {
    phase: 'open', selector: actionSelector(input.action), requestSchema: OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3,
    payer: input.payer, actor: input.actor, fixedAccounts: input.fixedAccounts, lookupTable: input.lookupTable,
    descriptorId: input.descriptorId,
  });
  const { payer, actor, fixed, marketAddress, coreProgram, trading, registry, market, activation,
    capabilitySelection, configDigest, rootDigest, artifacts, descriptorId, descriptorAddresses,
    descriptor, graphAddresses } = common;
  const admittedBasis = await authenticateRationalProductBasisRecordV3(client, common.accounts, {
    registry,
    rawAddress: fixed[Hot.HOT_LINKED_BASIS_RAW_ACCOUNT_V3]?.address ?? '',
    stagingAddress: fixed[Hot.HOT_LINKED_BASIS_STAGING_ACCOUNT_V3]?.address ?? '',
    productId: common.product.productId,
    domainDigest: common.domainDigest,
    domainBytes: common.domainRaw.data,
    representationWidth: descriptor.outcomeCount,
  });
  if (descriptor.market !== marketAddress || !same(descriptor.releaseSet, market.releaseSet) || descriptor.tokenProgram !== TOKEN_2022_PROGRAM_ID) {
    throw new Error('representation descriptor differs from Market/release/TokenBehaviorV2');
  }
  const authority = PublicKey.findProgramAddressSync([REPRESENTATION_AUTHORITY_SEED, descriptorId], key(activation.claims, 'Claims program'))[0];
  const expectedReceipt = PublicKey.findProgramAddressSync([RECEIPT_MINT_SEED, descriptor.graphDigest, key(marketAddress, 'Market').toBytes(), market.releaseSet], key(activation.claims, 'Claims program'))[0];
  if (expectedReceipt.toBase58() !== descriptor.receiptMint) throw new Error('descriptor receipt Mint is not the graph+Market+release Claims PDA');
  const aggregate = PublicKey.findProgramAddressSync([CLAIMS_AGGREGATE_SEED, key(marketAddress, 'Market').toBytes()], key(activation.claims, 'Claims program'))[0];
  const replay = PublicKey.findProgramAddressSync([REPLAY_SEED, descriptorId, key(actor, 'actor').toBytes()], key(activation.claims, 'Claims program'))[0];
  if (selectedAction(input.action) && (input.selectedOutcome === null || input.selectedOutcome < 0 || input.selectedOutcome >= descriptor.outcomeCount)) throw new Error('selected action outcome is outside representation K');
  if (!selectedAction(input.action) && input.selectedOutcome !== null) throw new Error('Structured action cannot carry a selected outcome');
  const outcomes = selectedAction(input.action) ? [input.selectedOutcome as number] : Array.from({ length: descriptor.outcomeCount }, (_, index) => index);
  const actorPosition = selectedAction(input.action)
    ? PublicKey.findProgramAddressSync([POSITION_SEED, aggregate.toBytes(), key(actor, 'actor').toBytes()], key(activation.claims, 'Claims program'))[0]
    : null;
  const actorReceipt = selectedAction(input.action) ? null : associated(key(actor, 'actor'), expectedReceipt);
  const derived = outcomes.map((outcome) => {
    const selector = le32(outcome);
    const owner = PublicKey.findProgramAddressSync([CUSTODY_OWNER_SEED, descriptorId, selector], key(activation.claims, 'Claims program'))[0];
    const position = PublicKey.findProgramAddressSync([POSITION_SEED, aggregate.toBytes(), owner.toBytes()], key(activation.claims, 'Claims program'))[0];
    const mint = PublicKey.findProgramAddressSync([SHARD_MINT_SEED, descriptorId, selector], key(activation.claims, 'Claims program'))[0];
    return Object.freeze({ outcome, owner, position, mint, actorToken: associated(key(actor, 'actor'), mint), custody: PublicKey.findProgramAddressSync([STRUCTURED_CUSTODY_SEED, descriptorId, selector], key(activation.claims, 'Claims program'))[0] });
  });
  const dynamic = [activation.claims, activation.claimsProgramData, aggregate.toBase58(), replay.toBase58(), descriptor.receiptMint,
    ...(actorPosition ? [actorPosition.toBase58()] : []), ...(actorReceipt ? [actorReceipt.toBase58()] : []),
    ...derived.flatMap((row) => [row.position.toBase58(), row.mint.toBase58(), row.actorToken.toBase58(), row.custody.toBase58()])];
  const dynamicObservation = await acquireRationalHotAccountsV4(client, dynamic, common.observedSlot);
  const accounts = new Map([...common.accounts, ...dynamicObservation.accounts]);
  const claimsProgram = required(accounts, activation.claims, 'Claims program');
  const claimsProgramData = required(accounts, activation.claimsProgramData, 'Claims ProgramData');
  validateProgramAccount(activation.claims, claimsProgram, 'Claims program');
  if (claimsProgram.owner !== UPGRADEABLE_LOADER_ID || claimsProgram.data.length !== 36 || u32(claimsProgram.data, 0) !== 2
      || new PublicKey(slice(claimsProgram.data, 4, 32)).toBase58() !== activation.claimsProgramData
      || claimsProgramData.owner !== UPGRADEABLE_LOADER_ID || claimsProgramData.executable) {
    throw new Error('activated Claims program and ProgramData do not form one exact Loader-v3 deployment');
  }
  const aggregateAccount = required(accounts, aggregate.toBase58(), 'Claims aggregate');
  if (aggregateAccount.owner !== activation.claims || aggregateAccount.executable) throw new Error('Claims aggregate owner/executable state differs');
  const claims = decodeRationalClaimsAggregateV2(aggregateAccount.data, { market: marketAddress, releaseSet: market.releaseSet, registry,
    product: market.productRecord, realm: market.realm, generation: market.generation, outcomes: descriptor.outcomeCount });
  const replayAccount = required(accounts, replay.toBase58(), 'representation replay funding');
  const replayRevision = decodeRationalRepresentationReplayV2(replayAccount, activation.claims, descriptorId, actor);
  if (replayAccount.owner === SYSTEM_PROGRAM_ID) {
    const replayRent = await client.minimumBalanceForRentExemption(RATIONAL_REPLAY_BYTES_V2);
    if (BigInt(replayAccount.lamports) < BigInt(replayRent.lamports)) {
      throw new Error(`new representation replay cannot fund its exact ${RATIONAL_REPLAY_BYTES_V2}-byte rent minimum`);
    }
  }
  const receiptMint = decodeToken2022BehaviorMintV2(descriptor.receiptMint, required(accounts, descriptor.receiptMint, 'receipt Mint'));
  if (receiptMint.controller !== authority.toBase58()) throw new Error('receipt Mint controller differs from the descriptor-derived authority');
  let actorPositionRevision = RATIONAL_OPEN_ABSENT_REVISION_V3;
  let actorPositionState: RationalClaimsPositionStateV2 | null = null;
  if (actorPosition !== null) {
    const account = required(accounts, actorPosition.toBase58(), 'actor Claims Position');
    if (account.owner !== activation.claims || account.executable) throw new Error('actor Claims Position owner/executable state differs');
    actorPositionState = decodeRationalClaimsPositionStateV2(account.data, aggregate.toBase58(), actor, claims.basis, descriptor.outcomeCount);
    actorPositionRevision = actorPositionState.revision;
  }
  let actorReceiptState: ReturnType<typeof decodeToken2022BehaviorAccountV2> | null = null;
  if (actorReceipt !== null) {
    actorReceiptState = decodeToken2022BehaviorAccountV2(actorReceipt.toBase58(), required(accounts, actorReceipt.toBase58(), 'actor receipt account'));
    if (actorReceiptState.mint !== descriptor.receiptMint || actorReceiptState.owner !== actor) throw new Error('actor receipt account differs from the canonical actor/Mint ATA');
  }
  const assets: RationalOpenAssetV3[] = []; const childAssets: Array<Readonly<{ position: string; asset: RationalOpenAssetV3 }>> = [];
  const observedAssets: Array<Readonly<{
    position: string; positionState: RationalClaimsPositionStateV2;
    mint: ReturnType<typeof decodeToken2022BehaviorMintV2>;
    actor: ReturnType<typeof decodeToken2022BehaviorAccountV2>;
    structured: ReturnType<typeof decodeToken2022BehaviorAccountV2>;
  }>> = [];
  let selectedCustodyRevision = RATIONAL_OPEN_ABSENT_REVISION_V3;
  for (const row of derived) {
    const positionAccount = required(accounts, row.position.toBase58(), `Claims custody Position outcome ${row.outcome}`);
    if (positionAccount.owner !== activation.claims || positionAccount.executable) throw new Error(`Claims custody Position ${row.outcome} owner/executable state differs`);
    const positionState = decodeRationalClaimsPositionStateV2(positionAccount.data, aggregate.toBase58(), row.owner.toBase58(), claims.basis, descriptor.outcomeCount);
    if (selectedAction(input.action)) selectedCustodyRevision = positionState.revision;
    const mint = decodeToken2022BehaviorMintV2(row.mint.toBase58(), required(accounts, row.mint.toBase58(), `shard Mint ${row.outcome}`));
    const actorToken = decodeToken2022BehaviorAccountV2(row.actorToken.toBase58(), required(accounts, row.actorToken.toBase58(), `actor shard ATA ${row.outcome}`));
    const custody = decodeToken2022BehaviorAccountV2(row.custody.toBase58(), required(accounts, row.custody.toBase58(), `Structured custody ${row.outcome}`));
    if (mint.controller !== authority.toBase58() || actorToken.mint !== row.mint.toBase58() || actorToken.owner !== actor
        || custody.mint !== row.mint.toBase58() || custody.owner !== authority.toBase58()) throw new Error(`TokenBehaviorV2 asset ${row.outcome} authority/Mint joins differ`);
    const coefficient = descriptor.support.find((item) => item.outcome === row.outcome)?.coefficient ?? 0n;
    const asset = Object.freeze({ shardMint: row.mint.toBase58(), actorShardAccount: row.actorToken.toBase58(),
      structuredCustodyAccount: row.custody.toBase58(), claimsCustodyOwner: row.owner.toBase58(), coefficient,
      expectedShardSupply: mint.rawSupply, expectedActorShards: actorToken.rawAmount, expectedStructuredShards: custody.rawAmount });
    assets.push(asset); childAssets.push(Object.freeze({ position: row.position.toBase58(), asset }));
    observedAssets.push(Object.freeze({ position: row.position.toBase58(), positionState, mint, actor: actorToken, structured: custody }));
  }
  const family = await compileRationalOpenHotV3({
    action: input.action, releaseSet: market.releaseSet, market: marketAddress, graphId: descriptor.graphId, descriptorId,
    actor, receiptMint: descriptor.receiptMint, receiptAccount: actorReceipt?.toBase58() ?? null,
    representationAuthority: authority.toBase58(), tokenProgram: TOKEN_2022_PROGRAM_ID,
    expectedRepresentationRevision: replayRevision,
    expectedClaimsMarketRevision: selectedAction(input.action) ? claims.revision : RATIONAL_OPEN_ABSENT_REVISION_V3,
    expectedActorPositionRevision: actorPositionRevision, expectedCustodyPositionRevision: selectedCustodyRevision,
    generation: market.generation, quantity: input.rawQuantity, denominator: descriptor.denominator,
    expectedReceiptSupply: receiptMint.rawSupply, outcomeCount: descriptor.outcomeCount, selectedOutcome: input.selectedOutcome, assets,
  });
  const caller = PublicKey.findProgramAddressSync([CALLER_AUTHORITY_SEED, market.releaseSet, key(marketAddress, 'Market').toBytes(), Uint8Array.of(2), family.familyDigest, family.childDigest], key(trading, 'Trading program'))[0];
  const child = rationalOpenClaimsMetasV4({ caller: caller.toBase58(), trading, tradingProgramData: activation.tradingProgramData,
    actor, authority: authority.toBase58(), descriptorRaw: descriptorAddresses.record, descriptorStaging: descriptorAddresses.staging,
    graphRaw: graphAddresses.record, graphStaging: graphAddresses.staging, replay: replay.toBase58(), aggregate: aggregate.toBase58(),
    activation: fixed[Hot.HOT_ACTIVATION_CACHE_ACCOUNT_V3]?.address ?? '', claims: activation.claims, claimsProgramData: activation.claimsProgramData,
    registry, market: marketAddress, core: coreProgram, coreProgramData: activation.coreProgramData, receiptMint: descriptor.receiptMint,
    receiptAccount: actorReceipt?.toBase58() ?? null, actorPosition: actorPosition?.toBase58() ?? null,
    linkedRaw: fixed[Hot.HOT_LINKED_BASIS_RAW_ACCOUNT_V3]?.address ?? '', linkedStaging: fixed[Hot.HOT_LINKED_BASIS_STAGING_ACCOUNT_V3]?.address ?? '',
    productRaw: fixed[Hot.HOT_PRODUCT_RAW_ACCOUNT_V3]?.address ?? '', productStaging: fixed[Hot.HOT_PRODUCT_STAGING_ACCOUNT_V3]?.address ?? '',
    domainRaw: fixed[Hot.HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3]?.address ?? '', domainStaging: fixed[Hot.HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3]?.address ?? '',
    portfolioRaw: fixed[Hot.HOT_PORTFOLIO_RAW_ACCOUNT_V3]?.address ?? '', portfolioStaging: fixed[Hot.HOT_PORTFOLIO_STAGING_ACCOUNT_V3]?.address ?? '',
    assets: childAssets, structured: !selectedAction(input.action),
  });
  const injected = [fixed[Hot.HOT_ROOT_ACCOUNT_V3], fixed[Hot.HOT_CONFIG_RAW_ACCOUNT_V3], fixed[Hot.HOT_PRODUCT_RAW_ACCOUNT_V3], fixed[Hot.HOT_PORTFOLIO_RAW_ACCOUNT_V3], fixed[Hot.HOT_LINKED_BASIS_RAW_ACCOUNT_V3]];
  if (injected.some((meta) => meta === undefined)) throw new Error('Hot fixed frame omits one injected profile coordinate');
  const physical = compactRationalProfile11AccountsV4(artifacts[0]?.data ?? new Uint8Array(), selectedAction(input.action) ? 0 : descriptor.outcomeCount, injected as ReadonlyArray<Meta>, child, accounts);
  const context: RationalOpenPoststateContextV4 = Object.freeze({
    claimsProgram: activation.claims,
    descriptorId: new Uint8Array(descriptorId),
    actor,
    representationAuthority: authority.toBase58(),
    aggregate: aggregate.toBase58(),
    market: marketAddress,
    releaseSet: new Uint8Array(market.releaseSet),
    registry,
    product: new Uint8Array(market.productRecord),
    realm: new Uint8Array(market.realm),
    generation: market.generation,
    outcomes: descriptor.outcomeCount,
    basis: new Uint8Array(claims.basis),
    custodyContext: new Uint8Array(claims.custodyContext),
  });
  let aggregatePoststate: RationalOpenPoststateV4['aggregate'] = null;
  const positionPoststates: RationalOpenPositionPoststateV4[] = [];
  if (selectedAction(input.action)) {
    if (actorPosition === null || actorPositionState === null || input.selectedOutcome === null) {
      throw new Error('selected Rational open lacks one authenticated Claims Position prestate');
    }
    const custody = observedAssets[0];
    const asset = assets[0];
    if (custody === undefined || asset === undefined) throw new Error('selected Rational open lacks its one custody asset');
    const outcome = input.selectedOutcome;
    const actorBalance = actorPositionState.balances[outcome];
    const custodyBalance = custody.positionState.balances[outcome];
    if (actorBalance === undefined || custodyBalance === undefined) throw new Error('selected Claims balance is outside the authenticated vector');
    const denominating = input.action === 'denominate';
    const actorAfter = denominating
      ? subtractU64(actorBalance, input.rawQuantity, 'actor Claims balance')
      : addU64(actorBalance, input.rawQuantity, 'actor Claims balance');
    const custodyAfter = denominating
      ? addU64(custodyBalance, input.rawQuantity, 'custody Claims balance')
      : subtractU64(custodyBalance, input.rawQuantity, 'custody Claims balance');
    aggregatePoststate = Object.freeze({
      address: aggregate.toBase58(),
      revision: addU64(claims.revision, 1n, 'Claims aggregate revision'),
      balances: Object.freeze([...claims.balances]),
    });
    positionPoststates.push(
      Object.freeze({
        address: actorPosition.toBase58(),
        owner: actor,
        revision: addU64(actorPositionState.revision, 1n, 'actor Claims Position revision'),
        balances: withBalance(actorPositionState.balances, outcome, actorAfter, 'actor Claims Position'),
      }),
      Object.freeze({
        address: custody.position,
        owner: asset.claimsCustodyOwner,
        revision: addU64(custody.positionState.revision, 1n, 'custody Claims Position revision'),
        balances: withBalance(custody.positionState.balances, outcome, custodyAfter, 'custody Claims Position'),
      }),
    );
  }
  const tokenPoststate = projectRationalOpenTokenPoststateV4({
    action: input.action,
    rawQuantity: input.rawQuantity,
    rawShardDeltas: family.rawShardDeltas,
    receipt: actorReceipt === null || actorReceiptState === null ? null : Object.freeze({
      mint: descriptor.receiptMint,
      supply: receiptMint.rawSupply,
      account: actorReceipt.toBase58(),
      amount: actorReceiptState.rawAmount,
    }),
    assets: observedAssets.map((observed) => Object.freeze({
      mint: observed.mint.mint,
      mintSupply: observed.mint.rawSupply,
      actorAccount: observed.actor.address,
      actorAmount: observed.actor.rawAmount,
      structuredAccount: observed.structured.address,
      structuredAmount: observed.structured.rawAmount,
    })),
  });
  const poststate: RationalOpenPoststateV4 = Object.freeze({
    context,
    replay: Object.freeze({
      address: replay.toBase58(),
      revision: addU64(replayRevision, 1n, 'representation replay revision'),
    }),
    aggregate: aggregatePoststate,
    positions: Object.freeze(positionPoststates),
    receipt: tokenPoststate.receipt,
    assets: tokenPoststate.assets,
  });
  return Object.freeze({ observedSlot: dynamicObservation.slot, action: input.action, payer, actor, market: marketAddress,
    generation: market.generation, representationWidth: admittedBasis.basis.width,
    resultOutcomeCount: common.product.outcomeCount, selectedOutcome: input.selectedOutcome,
    rawQuantity: input.rawQuantity,
    displayDecimals: receiptMint.displayDecimals, descriptorId, tokenBehaviorDigest: configDigest,
    capabilityDigest: capabilitySelection.digest, rootDigest, family, fixedAccounts: fixed,
    poststate,
    physicalClaimsAccounts: physical, lookupTable: common.lookupTable, executionStatus: 'blocked',
    refusal: 'The CapabilityV4 family is chain-derived and packet-compilable, but no checked positive common-Hot real-SBF release attests this outer; wallet signing remains disabled.',
  });
}

/**
 * Reacquire every mutable account at finalized commitment and authenticate the
 * exact atom/revision vector promised before wallet handoff.
 */
export async function verifyRationalOpenFinalizedPoststateV4(
  client: Pick<OpenRpc, 'finalizedSlot' | 'multipleAccounts'>,
  action: RationalOpenActionV3,
  poststate: RationalOpenPoststateV4,
  minimumSlot?: string,
): Promise<RationalOpenFinalizedPoststateV4> {
  const selected = selectedAction(action);
  if ((selected && (poststate.aggregate === null || poststate.positions.length !== 2 || poststate.receipt !== null || poststate.assets.length !== 1))
      || (!selected && (poststate.aggregate !== null || poststate.positions.length !== 0 || poststate.receipt === null
        || poststate.assets.length !== poststate.context.outcomes))) {
    throw new Error('Rational open poststate shape differs from the selected action family');
  }
  const context = poststate.context;
  key(context.claimsProgram, 'Claims program');
  key(context.actor, 'actor');
  key(context.representationAuthority, 'representation authority');
  key(context.aggregate, 'Claims aggregate');
  key(context.market, 'Market');
  key(context.registry, 'Registry');
  if (context.descriptorId.length !== 32 || context.releaseSet.length !== 32 || context.product.length !== 32
      || context.realm.length !== 32 || context.basis.length !== 32 || context.custodyContext.length !== 32
      || context.outcomes <= 0 || !Number.isSafeInteger(context.outcomes)
      || context.generation < 0n || context.generation > MAX_U64) {
    throw new Error('Rational open poststate context is not one exact bounded authenticated identity set');
  }
  const positionAddresses = poststate.positions.map((row) => row.address);
  const tokenAddresses = poststate.assets.flatMap((row) => [row.mint, row.actorAccount, row.structuredAccount]);
  const receiptAddresses = poststate.receipt === null ? [] : [poststate.receipt.mint, poststate.receipt.account];
  const requested = [poststate.replay.address, ...(poststate.aggregate === null ? [] : [poststate.aggregate.address]),
    ...positionAddresses, ...receiptAddresses, ...tokenAddresses];
  if (new Set(requested).size !== requested.length) throw new Error('Rational open poststate aliases two semantically distinct mutable accounts');
  const observation = await acquireRationalHotAccountsV4(client, requested, minimumSlot);
  const replayAccount = required(observation.accounts, poststate.replay.address, 'finalized representation replay');
  const replayRevision = decodeRationalRepresentationReplayStateV2(
    replayAccount,
    context.claimsProgram,
    context.descriptorId,
    context.actor,
    false,
    false,
  );
  if (replayRevision !== poststate.replay.revision) throw new Error('representation replay revision differs from the exact finalized poststate');
  if (poststate.aggregate !== null) {
    if (poststate.aggregate.address !== context.aggregate) throw new Error('Claims aggregate poststate substitutes another aggregate');
    const account = required(observation.accounts, poststate.aggregate.address, 'finalized Claims aggregate');
    if (account.owner !== context.claimsProgram || account.executable) throw new Error('finalized Claims aggregate owner/executable state differs');
    const aggregate = decodeRationalClaimsAggregateV2(account.data, {
      market: context.market,
      releaseSet: context.releaseSet,
      registry: context.registry,
      product: context.product,
      realm: context.realm,
      generation: context.generation,
      outcomes: context.outcomes,
    });
    if (aggregate.revision !== poststate.aggregate.revision
        || !same(aggregate.basis, context.basis)
        || !same(aggregate.custodyContext, context.custodyContext)) {
      throw new Error('Claims aggregate identities or revision differ from the exact finalized poststate');
    }
    requireExactBalances(aggregate.balances, poststate.aggregate.balances, 'Claims aggregate balances');
  }
  for (const [index, expected] of poststate.positions.entries()) {
    const account = required(observation.accounts, expected.address, `finalized Claims Position ${index}`);
    if (account.owner !== context.claimsProgram || account.executable) throw new Error(`finalized Claims Position ${index} owner/executable state differs`);
    const position = decodeRationalClaimsPositionStateV2(account.data, context.aggregate, expected.owner, context.basis, context.outcomes);
    if (position.revision !== expected.revision) throw new Error(`Claims Position ${index} revision differs from the exact finalized poststate`);
    requireExactBalances(position.balances, expected.balances, `Claims Position ${index} balances`);
  }
  if (poststate.receipt !== null) {
    const mint = decodeToken2022BehaviorMintV2(poststate.receipt.mint, required(observation.accounts, poststate.receipt.mint, 'finalized receipt Mint'));
    const account = decodeToken2022BehaviorAccountV2(poststate.receipt.account, required(observation.accounts, poststate.receipt.account, 'finalized receipt account'));
    if (mint.controller !== context.representationAuthority || mint.rawSupply !== poststate.receipt.supply
        || account.mint !== poststate.receipt.mint || account.owner !== context.actor
        || account.rawAmount !== poststate.receipt.amount) {
      throw new Error('Structured receipt differs from the exact finalized poststate');
    }
  }
  for (const [index, expected] of poststate.assets.entries()) {
    const mint = decodeToken2022BehaviorMintV2(expected.mint, required(observation.accounts, expected.mint, `finalized shard Mint ${index}`));
    const actor = decodeToken2022BehaviorAccountV2(expected.actorAccount, required(observation.accounts, expected.actorAccount, `finalized actor shard account ${index}`));
    const structured = decodeToken2022BehaviorAccountV2(expected.structuredAccount, required(observation.accounts, expected.structuredAccount, `finalized Structured custody ${index}`));
    if (mint.controller !== context.representationAuthority || mint.rawSupply !== expected.mintSupply
        || actor.mint !== expected.mint || actor.owner !== context.actor || actor.rawAmount !== expected.actorAmount
        || structured.mint !== expected.mint || structured.owner !== context.representationAuthority
        || structured.rawAmount !== expected.structuredAmount) {
      throw new Error(`Rational shard asset ${index} differs from the exact finalized poststate`);
    }
  }
  return Object.freeze({ observedSlot: observation.slot, action, poststate });
}

export function buildRationalOpenCandidateV4(inspection: RationalOpenChainInspectionV4, recentBlockhash: string): RationalOpenCandidateV4 {
  key(recentBlockhash, 'recent blockhash');
  const outer = new Uint8Array(128 + inspection.family.familyBytes.length);
  outer.set(Hot.HOT_EXECUTION_MAGIC_V3, 0); putU16(outer, 8, 3); putU16(outer, 10, 1);
  putU32(outer, 12, inspection.family.familyBytes.length); outer.set(inspection.family.familyBytes, 128);
  // The selected family owns these immutable envelope joins; no caller DTO.
  const releaseSet = slice(inspection.family.familyBytes, 16, 32);
  outer.set(releaseSet, 16); outer.set(key(inspection.market, 'Market').toBytes(), 48); putU64(outer, 80, inspection.generation); outer.set(inspection.rootDigest, 88);
  const keys = [...inspection.fixedAccounts, ...inspection.physicalClaimsAccounts].map((meta) => ({ pubkey: key(meta.address, 'Rational open account'), isSigner: meta.isSigner, isWritable: meta.isWritable }));
  const instruction = new TransactionInstruction({ programId: key(inspection.fixedAccounts[Hot.HOT_TRADING_PROGRAM_ACCOUNT_V3]?.address ?? '', 'Trading program'), keys, data: outer as Buffer });
  const transaction = new VersionedTransaction(new TransactionMessage({ payerKey: key(inspection.payer, 'payer'), recentBlockhash, instructions: [instruction] }).compileToV0Message([inspection.lookupTable]));
  let wireBytes: Uint8Array;
  try { wireBytes = transaction.serialize(); } catch {
    throw new Error(`Rational open ${inspection.action} packet exceeds the Solana message/packet encoding bound`);
  }
  if (wireBytes.length > SOLANA_PACKET_BYTES_V1) throw new Error(`Rational open packet is ${wireBytes.length} bytes, above ${SOLANA_PACKET_BYTES_V1}`);
  const requiredSigners = Object.freeze(transaction.message.staticAccountKeys.slice(0, transaction.message.header.numRequiredSignatures).map((value) => value.toBase58()));
  const expectedSigners = inspection.payer === inspection.actor ? [inspection.payer] : [inspection.payer, inspection.actor].sort();
  if ([...requiredSigners].sort().join(':') !== expectedSigners.join(':')) throw new Error('Rational open packet has an unexpected wallet signer set');
  const loadedAddresses = transaction.message.addressTableLookups.reduce((total, table) => total + table.readonlyIndexes.length + table.writableIndexes.length, 0);
  if (loadedAddresses === 0) throw new Error('selected ALT did not contribute to Rational open');
  return Object.freeze({ transaction, instruction, outerBytes: outer, wireBytes, requiredSigners, loadedAddresses,
    logicalClaimsAccounts: inspection.family.claimsAccountCount, physicalClaimsAccounts: inspection.physicalClaimsAccounts.length,
    executionStatus: 'blocked', refusal: inspection.refusal });
}

export function rationalOpenChainSummaryV4(inspection: RationalOpenChainInspectionV4): Readonly<Record<string, string>> {
  return Object.freeze({ action: inspection.action, descriptor: hex(inspection.descriptorId), capability: hex(inspection.capabilityDigest),
    quantity: `${inspection.rawQuantity.toString()} raw atoms`,
    width: `K=${inspection.representationWidth} claims over N=${inspection.resultOutcomeCount} terminal results`,
    claims: `${inspection.family.claimsAccountCount} logical → ${inspection.physicalClaimsAccounts.length} physical`,
    decimals: `${inspection.displayDecimals} display-only` });
}
