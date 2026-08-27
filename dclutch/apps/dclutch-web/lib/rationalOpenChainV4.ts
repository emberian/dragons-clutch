import {
  AddressLookupTableAccount,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { ascii, hex, requireNonzero, requireZero, slice, u16, u64 } from './bytes';
import {
  LIABILITY_BASIS_MARKET_SEED_V2 as CLAIMS_AGGREGATE_SEED,
  LIABILITY_BASIS_POSITION_SEED_V2 as POSITION_SEED,
} from './generated/coreFound';
import * as Hot from './generated/directInlineV3';
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
import { PACKET_DATA_SIZE } from './directTransaction';
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
const REPLAY_SEED = new TextEncoder().encode('dclutch:rational-replay:v2');
const CALLER_AUTHORITY_SEED = new TextEncoder().encode('dclutch:role-authority:v1');
const ACCOUNT_PROFILE_HEADER = 40;
const ACCOUNT_PROFILE_RULE = 16;
const ACCOUNT_PROFILE_OPERATION = 16;

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
  executionStatus: 'blocked';
  refusal: string;
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

export type RationalClaimsAggregateV2 = Readonly<{ revision: bigint; basis: Uint8Array; custodyContext: Uint8Array }>;
export function decodeRationalClaimsAggregateV2(bytes: Uint8Array, input: Readonly<{
  market: string; releaseSet: Uint8Array; registry: string; product: Uint8Array; realm: Uint8Array; generation: bigint; outcomes: number;
}>): RationalClaimsAggregateV2 {
  if (bytes.length !== 256 + input.outcomes * 8 || ascii(bytes, 0, 8) !== 'DCLLBM02' || u16(bytes, 8) !== 2 || u32(bytes, 12) !== input.outcomes) {
    throw new Error('Claims aggregate has the wrong exact runtime-width ABI');
  }
  requireZero(bytes, 10, 2, 'Claims aggregate');
  for (const [observed, expected, field] of [
    [slice(bytes, 24, 32), key(input.market, 'Market').toBytes(), 'Market'],
    [slice(bytes, 56, 32), input.releaseSet, 'release set'],
    [slice(bytes, 88, 32), key(input.registry, 'Registry').toBytes(), 'Registry'],
    [slice(bytes, 120, 32), input.product, 'Product record'],
    [slice(bytes, 184, 32), input.realm, 'Realm'],
  ] as const) if (!same(observed, expected)) throw new Error(`Claims aggregate ${field} differs from Core`);
  if (u64(bytes, 248) !== input.generation) throw new Error('Claims aggregate generation differs from Core');
  const basis = slice(bytes, 152, 32); requireNonzero(basis, 'Claims semantic basis');
  const custodyContext = slice(bytes, 216, 32); requireNonzero(custodyContext, 'Claims custody context');
  return Object.freeze({ revision: u64(bytes, 16), basis, custodyContext });
}

export function decodeRationalClaimsPositionV2(bytes: Uint8Array, aggregate: string, owner: string, basis: Uint8Array, outcomes: number): bigint {
  if (bytes.length !== 128 + outcomes * 8 || ascii(bytes, 0, 8) !== 'DCLLBP02' || u16(bytes, 8) !== 2 || u32(bytes, 12) !== outcomes) {
    throw new Error('Claims Position has the wrong exact runtime-width ABI');
  }
  requireZero(bytes, 10, 2, 'Claims Position'); requireZero(bytes, 120, 8, 'Claims Position');
  if (new PublicKey(slice(bytes, 24, 32)).toBase58() !== aggregate || new PublicKey(slice(bytes, 56, 32)).toBase58() !== owner
      || !same(slice(bytes, 88, 32), basis)) throw new Error('Claims Position aggregate, owner, or semantic basis differs');
  return u64(bytes, 16);
}

export function decodeRationalRepresentationReplayV2(account: RpcAccount, claims: string, descriptor: Uint8Array, actor: string): bigint {
  if (account.owner === claims) {
    if (account.executable || account.data.length !== 88 || ascii(account.data, 0, 8) !== 'DCRRREP2' || u16(account.data, 8) !== 2) throw new Error('representation replay has the wrong exact V2 ABI');
    requireZero(account.data, 10, 6, 'representation replay');
    if (!same(slice(account.data, 16, 32), descriptor) || new PublicKey(slice(account.data, 48, 32)).toBase58() !== actor) throw new Error('representation replay descriptor or actor differs');
    const revision = u64(account.data, 80); if (revision === MAX_U64) throw new Error('representation replay revision is exhausted'); return revision;
  }
  if (account.owner !== SYSTEM_PROGRAM_ID || account.executable || account.data.length !== 0 || BigInt(account.lamports) === 0n) {
    throw new Error('new representation replay is not a funded data-free System account');
  }
  return 0n;
}

function validateProgramAccount(address: string, account: RpcAccount, field: string): void {
  if (!account.executable || key(address, field).toBase58() !== address) throw new Error(`${field} is not executable runtime code`);
}

export function compactRationalProfile11AccountsV4(profile: Uint8Array, tailCount: number, injected: ReadonlyArray<Meta>, child: ReadonlyArray<Meta>, accounts: ReadonlyMap<string, RpcAccount | null>): ReadonlyArray<Meta> {
  if (profile.length < ACCOUNT_PROFILE_HEADER || ascii(profile, 0, 8) !== 'DCLTAP02' || u16(profile, 8) !== 2 || u16(profile, 10) !== 11) {
    throw new Error('selected AccountProfile is not exact authenticated-route-alias Profile11');
  }
  const fixed = u16(profile, 12); const stride = u16(profile, 14); const fixedOps = u16(profile, 16); const itemOps = u16(profile, 18);
  const expectedWidth = ACCOUNT_PROFILE_HEADER + (fixed + stride) * ACCOUNT_PROFILE_RULE + (fixedOps + itemOps) * ACCOUNT_PROFILE_OPERATION;
  const logical = [...injected, ...child];
  if (profile.length !== expectedWidth || logical.length !== fixed + stride * tailCount) throw new Error('Profile11 bytes or runtime logical width differs from Product N');
  const representative = (coordinate: number): number => {
    const item = coordinate < fixed ? -1 : Math.floor((coordinate - fixed) / stride);
    const local = coordinate < fixed ? coordinate : fixed + ((coordinate - fixed) % stride);
    const offset = ACCOUNT_PROFILE_HEADER + local * ACCOUNT_PROFILE_RULE;
    const alias = profile[offset + 2]; const index = u16(profile, offset + 4);
    if (alias === 0 && index === 0) return coordinate;
    if (alias === 1 && index < fixed && index < coordinate) return index;
    if (alias === 2 && item >= 0 && index < stride && index < (coordinate - fixed) % stride) return fixed + item * stride + index;
    throw new Error(`Profile11 logical coordinate ${coordinate} has a forward or undefined alias`);
  };
  const output: Meta[] = [];
  for (let coordinate = 0; coordinate < logical.length; coordinate += 1) {
    const itemLocal = coordinate < fixed ? coordinate : fixed + ((coordinate - fixed) % stride);
    const offset = ACCOUNT_PROFILE_HEADER + itemLocal * ACCOUNT_PROFILE_RULE;
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
          const bits = profile[ACCOUNT_PROFILE_HEADER + otherLocal * ACCOUNT_PROFILE_RULE] ?? 0;
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
    const replayRent = await client.minimumBalanceForRentExemption(88);
    if (BigInt(replayAccount.lamports) < BigInt(replayRent.lamports)) {
      throw new Error('new representation replay cannot fund its exact 88-byte rent minimum');
    }
  }
  const receiptMint = decodeToken2022BehaviorMintV2(descriptor.receiptMint, required(accounts, descriptor.receiptMint, 'receipt Mint'));
  if (receiptMint.controller !== authority.toBase58()) throw new Error('receipt Mint controller differs from the descriptor-derived authority');
  let actorPositionRevision = RATIONAL_OPEN_ABSENT_REVISION_V3;
  if (actorPosition !== null) {
    const account = required(accounts, actorPosition.toBase58(), 'actor Claims Position');
    if (account.owner !== activation.claims || account.executable) throw new Error('actor Claims Position owner/executable state differs');
    actorPositionRevision = decodeRationalClaimsPositionV2(account.data, aggregate.toBase58(), actor, claims.basis, descriptor.outcomeCount);
  }
  if (actorReceipt !== null) {
    const receipt = decodeToken2022BehaviorAccountV2(actorReceipt.toBase58(), required(accounts, actorReceipt.toBase58(), 'actor receipt account'));
    if (receipt.mint !== descriptor.receiptMint || receipt.owner !== actor) throw new Error('actor receipt account differs from the canonical actor/Mint ATA');
  }
  const assets: RationalOpenAssetV3[] = []; const childAssets: Array<Readonly<{ position: string; asset: RationalOpenAssetV3 }>> = [];
  let selectedCustodyRevision = RATIONAL_OPEN_ABSENT_REVISION_V3;
  for (const row of derived) {
    const positionAccount = required(accounts, row.position.toBase58(), `Claims custody Position outcome ${row.outcome}`);
    if (positionAccount.owner !== activation.claims || positionAccount.executable) throw new Error(`Claims custody Position ${row.outcome} owner/executable state differs`);
    const positionRevision = decodeRationalClaimsPositionV2(positionAccount.data, aggregate.toBase58(), row.owner.toBase58(), claims.basis, descriptor.outcomeCount);
    if (selectedAction(input.action)) selectedCustodyRevision = positionRevision;
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
  return Object.freeze({ observedSlot: dynamicObservation.slot, action: input.action, payer, actor, market: marketAddress,
    generation: market.generation, representationWidth: admittedBasis.basis.width,
    resultOutcomeCount: common.product.outcomeCount, selectedOutcome: input.selectedOutcome,
    rawQuantity: input.rawQuantity,
    displayDecimals: receiptMint.displayDecimals, descriptorId, tokenBehaviorDigest: configDigest,
    capabilityDigest: capabilitySelection.digest, rootDigest, family, fixedAccounts: fixed,
    physicalClaimsAccounts: physical, lookupTable: common.lookupTable, executionStatus: 'blocked',
    refusal: 'The CapabilityV4 family is chain-derived and packet-compilable, but no checked positive common-Hot real-SBF release attests this outer; wallet signing remains disabled.',
  });
}

export function buildRationalOpenCandidateV4(inspection: RationalOpenChainInspectionV4, recentBlockhash: string): RationalOpenCandidateV4 {
  key(recentBlockhash, 'recent blockhash');
  const outer = new Uint8Array(128 + inspection.family.familyBytes.length);
  outer.set(new TextEncoder().encode('DCLTHOT3'), 0); putU16(outer, 8, 3); putU16(outer, 10, 1);
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
  if (wireBytes.length > PACKET_DATA_SIZE) throw new Error(`Rational open packet is ${wireBytes.length} bytes, above ${PACKET_DATA_SIZE}`);
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
