import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  PublicKey,
} from '@solana/web3.js';

import { ascii, hex, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import {
  CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
  CORE_STATE_BYTES,
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
} from './generated/coreFound';
import { decodeCoreFoundProductGraphV2 } from './coreFound';
import * as DirectAbi from './generated/directInlineV3';
import {
  ACCOUNT_SCHEMA_RELEASE_ID,
  CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V1,
  CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1,
  CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V1,
  CAPABILITY_PROGRAM_V3_BYTES,
  DESCRIPTORCONTRACT_SCHEMA_RELEASE_ID,
  DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3,
  DIRECT_ORDINARY_COMMON_IDENTITIES_V3,
  DIRECT_ORDINARY_COMMON_SCALARS_V3,
  DIRECT_SUCCESSOR_KIND_ID_V3,
  EFFECT_SCHEMA_RELEASE_ID,
  EXECUTION_STRATEGY_ARTIFACT_PROFILE_V2,
  EXECUTION_STRATEGY_PROGRAM_BYTES_V2,
  EXECUTION_STRATEGY_PROGRAM_MAGIC_V2,
  EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
  EXECUTION_STRATEGY_SCHEMA_VERSION_V2,
  HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
  HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
  HOT_ACTIVATION_CACHE_ACCOUNT_V3,
  HOT_CONFIG_RAW_ACCOUNT_V3,
  HOT_CONFIG_STAGING_ACCOUNT_V3,
  HOT_CORE_PROGRAM_ACCOUNT_V3,
  HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
  HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
  HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
  HOT_EFFECT_RAW_ACCOUNT_V3,
  HOT_EFFECT_STAGING_ACCOUNT_V3,
  HOT_FIXED_ACCOUNT_COUNT_V3,
  HOT_LIFECYCLE_RAW_ACCOUNT_V3,
  HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
  HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
  HOT_LINKED_BASIS_STAGING_ACCOUNT_V3,
  HOT_MANIFEST_RAW_ACCOUNT_V3,
  HOT_MANIFEST_STAGING_ACCOUNT_V3,
  HOT_MARKET_ACCOUNT_V3,
  HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
  HOT_PROGRAM_SET_STAGING_ACCOUNT_V3,
  HOT_PRODUCT_RAW_ACCOUNT_V3,
  HOT_PRODUCT_STAGING_ACCOUNT_V3,
  HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3,
  HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3,
  HOT_PORTFOLIO_RAW_ACCOUNT_V3,
  HOT_PORTFOLIO_STAGING_ACCOUNT_V3,
  HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
  HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
  HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3,
  HOT_ROOT_ACCOUNT_V3,
  HOT_STRATEGY_RAW_ACCOUNT_V3,
  HOT_STRATEGY_STAGING_ACCOUNT_V3,
  HOT_TRADING_PROGRAM_ACCOUNT_V3,
  HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
  HOT_TRANSITION_RAW_ACCOUNT_V3,
  HOT_TRANSITION_STAGING_ACCOUNT_V3,
  BASIS_HEADER_BYTES_V3,
  BASIS_MAGIC_V3,
  BASIS_SCHEMA_V3,
  BASIS_WIDTH_OFFSET_V3,
  EXACT_CATEGORICAL_BOUNDARY_V3,
  GRADED_BASIS_RECORD_SCHEMA_ID_V3,
  KNOT_BYTES_V3,
  TERM_BYTES_V3,
  TERM_FLOOR_EXACT_COMPLEMENT_BOUNDARY_V3,
  IDENTITY_BUYER_NATIVE_SIGNER_V3,
  IDENTITY_SELLER_NATIVE_SIGNER_V3,
  LIFECYCLE_SCHEMA_RELEASE_ID,
  REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID,
  STRATEGY_DISPOSITION_OFFSET_V2,
  STRATEGY_TRANSITION_PROGRAM_OFFSET_V2,
  STRATEGY_TRANSITION_SCHEMA_OFFSET_V2,
  TRANSITION_SCHEMA_RELEASE_ID,
} from './generated/directInlineV3';
import {
  type CheckedHotOuterEvidenceV3,
  type DirectHotAccountMetaV3,
  type DirectInlineHotRouteV3,
  validateRuntimeAccountProfileV2,
} from './directInlineV3';
import {
  ARTIFACT_RELEASE_BYTES,
  SYSTEM_PROGRAM_ID,
  authenticateArtifactDeploymentV1,
  deriveFinalizedRecordAddressesV1,
} from './releaseRegistry';
import { decodeCheckedInfrastructureV1 } from './infrastructure';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

const MARKET_RELEASE_SET_OFFSET = 208;
const MARKET_REGISTRY_OFFSET = 240;
const MARKET_GENERATION_OFFSET = 272;
const MARKET_MANIFEST_OFFSET = 176;
const MARKET_PRODUCT_RECORD_OFFSET = 80;
const ACCOUNT_PROFILE_OPERATION_BYTES = 16;
const ACCOUNT_PROFILE_TAIL_COUNT_OPCODE = 8;
const ACTIVATION_CACHE_TRADING_OFFSET = 48 + 2 * (32 + ARTIFACT_RELEASE_BYTES);
const BASIS_SEMANTIC_DOMAIN_V3 = new TextEncoder().encode('dclutch/product-basis/semantic/v3');

export type DirectHotRouteCoordinateV3 = Readonly<{ address: string; isSigner: boolean; isWritable: boolean }>;

export type DirectHotRouteManifestV3 = Readonly<{
  payer: string;
  fixedAccounts: ReadonlyArray<DirectHotRouteCoordinateV3>;
  strategyAccounts: ReadonlyArray<DirectHotRouteCoordinateV3>;
  runtimeAccounts: ReadonlyArray<DirectHotRouteCoordinateV3>;
  lookupTables: ReadonlyArray<string>;
  checkedInfrastructure: Uint8Array | null;
}>;

export type DirectHotRouteInspectionV3 = Readonly<{
  observedSlot: string;
  route: DirectInlineHotRouteV3;
  selectedProgramDigest: string;
  programSetDigest: string;
  accountProfileDigest: string;
  strategyDigest: string;
  transitionDigest: string;
  checkedOuter: CheckedHotOuterEvidenceV3;
}>;

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function readU32(bytes: Uint8Array, offset: number): number {
  const value = slice(bytes, offset, 4);
  return new DataView(value.buffer, value.byteOffset, value.byteLength).getUint32(0, true);
}

function concat(...parts: ReadonlyArray<Uint8Array>): Uint8Array {
  const output = new Uint8Array(parts.reduce((total, part) => total + part.length, 0));
  let offset = 0;
  for (const part of parts) { output.set(part, offset); offset += part.length; }
  return output;
}

export async function validateProductBasisV3(
  bytes: Uint8Array,
  productId: Uint8Array,
  resultDomainDigest: Uint8Array,
  domain: Uint8Array,
): Promise<number> {
  if (bytes.length < BASIS_HEADER_BYTES_V3 || !same(slice(bytes, 0, 8), BASIS_MAGIC_V3)
      || u16(bytes, 8) !== BASIS_SCHEMA_V3 || u16(bytes, 10) !== BASIS_HEADER_BYTES_V3
      || readU32(bytes, 12) !== bytes.length) throw new Error('Product basis has the wrong exact V3 header or width');
  requireZero(bytes, 18, 2, 'Product basis header');
  requireZero(bytes, 208, 48, 'Product basis header tail');
  const kind = bytes[16];
  const rounding = bytes[17];
  const basisWidth = readU32(bytes, BASIS_WIDTH_OFFSET_V3);
  const knotCount = readU32(bytes, 24);
  const termCount = readU32(bytes, 28);
  const payoutScale = u64(bytes, 160);
  const knotDenominator = u64(bytes, 168);
  [32, 64, 96, 128, 176].forEach((offset) => requireNonzero(slice(bytes, offset, 32), 'Product basis identity'));
  if (!same(slice(bytes, 32, 32), productId)
      || !same(slice(bytes, 64, 32), resultDomainDigest)
      || !same(slice(bytes, 96, 32), slice(domain, 64, 32))
      || !same(slice(bytes, 128, 32), slice(domain, 96, 32))) {
    throw new Error('Product basis does not join the authenticated Product and result domain');
  }
  const semantic = await sha256(concat(BASIS_SEMANTIC_DOMAIN_V3, slice(bytes, 0, 32), slice(bytes, 96, bytes.length - 96)));
  if (!same(semantic, slice(domain, 128, 32))) throw new Error('Product basis semantic identity differs from Product-owned liability basis');
  if (basisWidth === 0 || payoutScale === 0n) throw new Error('Product basis has zero width or payout scale');
  if (kind === 1) {
    if (rounding !== EXACT_CATEGORICAL_BOUNDARY_V3 || payoutScale !== 1n || knotDenominator !== 1n
        || knotCount !== 0 || termCount !== 0 || bytes.length !== BASIS_HEADER_BYTES_V3) {
      throw new Error('categorical Product basis is not canonical Q=1');
    }
    return basisWidth;
  }
  if (kind !== 2 || rounding !== TERM_FLOOR_EXACT_COMPLEMENT_BOUNDARY_V3 || basisWidth < 2
      || knotDenominator === 0n || termCount === 0) throw new Error('graded Product basis kind or counts are not canonical');
  const expected = BASIS_HEADER_BYTES_V3 + basisWidth * 8 + knotCount * KNOT_BYTES_V3 + termCount * TERM_BYTES_V3;
  if (!Number.isSafeInteger(expected) || expected !== bytes.length) throw new Error('graded Product basis runtime tail has the wrong exact width');
  let payoutTotal = 0n;
  for (let index = 0; index < basisWidth; index += 1) {
    const payout = u64(bytes, BASIS_HEADER_BYTES_V3 + index * 8);
    if (payout > payoutScale) throw new Error('graded Product failure payout exceeds its exact scale');
    payoutTotal += payout;
  }
  if (payoutTotal !== payoutScale) throw new Error('graded Product failure payouts are not an exact partition');
  let priorKnot: bigint | null = null;
  const knotStart = BASIS_HEADER_BYTES_V3 + basisWidth * 8;
  for (let index = 0; index < knotCount; index += 1) {
    const offset = knotStart + index * KNOT_BYTES_V3;
    const unsigned = new DataView(bytes.buffer, bytes.byteOffset + offset, 16);
    const low = unsigned.getBigUint64(0, true);
    const high = unsigned.getBigInt64(8, true);
    const knot = (high << 64n) | low;
    if (priorKnot !== null && knot <= priorKnot) throw new Error('graded Product knots are not strictly ordered');
    priorKnot = knot;
  }
  const termStart = knotStart + knotCount * KNOT_BYTES_V3;
  let priorKey = '';
  let lastClaim = -1;
  const terms: Array<Readonly<{ tag: number; left: number; peak: number; right: number; amplitude: bigint }>> = [];
  for (let index = 0; index < termCount; index += 1) {
    const offset = termStart + index * TERM_BYTES_V3;
    const claim = readU32(bytes, offset);
    const tag = bytes[offset + 4];
    requireZero(bytes, offset + 5, 3, 'graded Product term');
    const left = readU32(bytes, offset + 8);
    const peak = readU32(bytes, offset + 12);
    const right = readU32(bytes, offset + 16);
    requireZero(bytes, offset + 20, 4, 'graded Product term');
    const amplitude = u64(bytes, offset + 24);
    const shapeValid = (tag === 0 && left === 0 && peak === 0 && right === 0)
      || ((tag === 1 || tag === 2) && peak === 0 && left < right && right < knotCount)
      || (tag === 3 && left < peak && peak < right && right < knotCount);
    if (!shapeValid || amplitude === 0n || claim >= basisWidth - 1 || (lastClaim < 0 ? claim !== 0 : claim !== lastClaim && claim !== lastClaim + 1)) {
      throw new Error('graded Product term is invalid or skips a primary claim');
    }
    const key = [claim, tag, left, peak, right].map((value) => value.toString().padStart(10, '0')).join(':');
    if (priorKey !== '' && key <= priorKey) throw new Error('graded Product terms are not in canonical order');
    priorKey = key; lastClaim = claim;
    terms.push(Object.freeze({ tag, left, peak, right, amplitude }));
  }
  if (lastClaim + 1 !== basisWidth - 1) throw new Error('graded Product terms do not cover every primary claim');
  const knots: bigint[] = [];
  for (let index = 0; index < knotCount; index += 1) {
    const offset = knotStart + index * KNOT_BYTES_V3;
    const view = new DataView(bytes.buffer, bytes.byteOffset + offset, 16);
    knots.push((view.getBigInt64(8, true) << 64n) | view.getBigUint64(0, true));
  }
  const evaluate = (term: (typeof terms)[number], x: bigint): bigint => {
    if (term.tag === 0) return term.amplitude;
    const left = knots[term.left];
    const right = knots[term.right];
    if (left === undefined || right === undefined) throw new Error('graded Product term selects an absent knot');
    const rising = x <= left ? 0n : x >= right ? term.amplitude : term.amplitude * (x - left) / (right - left);
    if (term.tag === 1) return rising;
    const falling = x <= left ? term.amplitude : x >= right ? 0n : term.amplitude * (right - x) / (right - left);
    if (term.tag === 2) return falling;
    const peak = knots[term.peak];
    if (peak === undefined) throw new Error('graded Product tent selects an absent peak');
    const tentRise = x <= left ? 0n : x >= peak ? term.amplitude : term.amplitude * (x - left) / (peak - left);
    const tentFall = x <= peak ? term.amplitude : x >= right ? 0n : term.amplitude * (right - x) / (right - peak);
    return tentRise < tentFall ? tentRise : tentFall;
  };
  const cells = knots.length < 2 ? [[knots[0] ?? 0n, knots[0] ?? 0n]] : knots.slice(0, -1).map((left, index) => [left, knots[index + 1]]);
  for (const [left, right] of cells) {
    let bound = 0n;
    for (const term of terms) {
      const leftValue = evaluate(term, left);
      const rightValue = evaluate(term, right);
      bound += leftValue > rightValue ? leftValue : rightValue;
    }
    if (bound > payoutScale) throw new Error('graded Product basis exceeds its checked cell envelope');
  }
  return basisWidth;
}

function key(value: string, field: string): PublicKey {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return parsed;
}

function required(accounts: ReadonlyMap<string, RpcAccount | null>, address: string, field: string): RpcAccount {
  const account = accounts.get(address);
  if (account === null || account === undefined) throw new Error(`${field} ${address} is absent at finalized commitment`);
  return account;
}

function chunks<T>(values: ReadonlyArray<T>, width: number): T[][] {
  const output: T[][] = [];
  for (let index = 0; index < values.length; index += width) output.push(values.slice(index, index + width));
  return output;
}

async function acquire(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>,
  addresses: ReadonlyArray<string>,
): Promise<Readonly<{ slot: string; accounts: ReadonlyMap<string, RpcAccount | null> }>> {
  const canonical = [...new Set(addresses.map((address, index) => key(address, `route address ${index}`).toBase58()))];
  const floor = await client.finalizedSlot();
  const accounts = new Map<string, RpcAccount | null>();
  let slot = floor;
  for (const group of chunks(canonical, 32)) {
    const observation = await client.multipleAccounts(group, floor);
    if (BigInt(observation.slot) < BigInt(floor)) throw new Error('route observation regressed below its finalized floor');
    slot = BigInt(observation.slot) > BigInt(slot) ? observation.slot : slot;
    observation.accounts.forEach((entry) => accounts.set(entry.address, entry.account));
  }
  return Object.freeze({ slot, accounts });
}

async function finalizedRecord(
  client: Pick<SolanaRpcClient, 'minimumBalanceForRentExemption'>,
  accounts: ReadonlyMap<string, RpcAccount | null>,
  registry: string,
  rawAddress: string,
  stagingAddress: string,
  schema: Uint8Array,
  expectedDigest: Uint8Array,
  field: string,
): Promise<RpcAccount> {
  const raw = required(accounts, rawAddress, `${field} raw`);
  const staging = accounts.get(stagingAddress);
  if (raw.owner !== registry || raw.executable) throw new Error(`${field} raw is not Registry-owned finalized data`);
  const digest = await sha256(raw.data);
  if (!same(digest, expectedDigest)) throw new Error(`${field} raw bytes differ from their selected content identity`);
  const derived = deriveFinalizedRecordAddressesV1(registry, schema, digest);
  if (derived.record !== rawAddress || derived.staging !== stagingAddress) throw new Error(`${field} raw/staging addresses are not canonical Registry PDAs`);
  if (staging !== null && staging !== undefined && (staging.owner !== SYSTEM_PROGRAM_ID || staging.executable || staging.data.length !== 0)) throw new Error(`${field} staging cursor is not vacant System-owned data`);
  const rent = await client.minimumBalanceForRentExemption(raw.data.length);
  if (BigInt(raw.lamports) < BigInt(rent.lamports)) throw new Error(`${field} raw record is below its current exact rent minimum`);
  return raw;
}

export type DirectRootSelectionV1 = Readonly<{
  entryIndex: number;
  manifest: Uint8Array;
  kind: Uint8Array;
  programSet: Uint8Array;
  config: Uint8Array;
}>;

export type DirectManifestEntryV1 = Readonly<{
  kind: Uint8Array;
  programSet: Uint8Array;
  config: Uint8Array;
  capacityProfile: Uint8Array;
  childSchema: Uint8Array;
  childDerivation: Uint8Array;
}>;

function compareIdentity(left: Uint8Array, right: Uint8Array): number {
  for (let index = 0; index < 32; index += 1) {
    const comparison = (left[index] ?? 0) - (right[index] ?? 0);
    if (comparison !== 0) return comparison;
  }
  return 0;
}

export function decodeDirectRootSelectionV1(bytes: Uint8Array): DirectRootSelectionV1 {
  if (bytes.length < DirectAbi.CAPABILITY_ROOT_HEADER_BYTES_V1
      || !same(slice(bytes, DirectAbi.CAPABILITY_ROOT_MAGIC_OFFSET, 8), DirectAbi.CAPABILITY_ROOT_MAGIC_V1)
      || u16(bytes, DirectAbi.CAPABILITY_ROOT_SCHEMA_VERSION_OFFSET) !== DirectAbi.CAPABILITY_ROOT_SCHEMA_VERSION_V1
      || u16(bytes, DirectAbi.CAPABILITY_ROOT_PROFILE_OFFSET) !== DirectAbi.CAPABILITY_ROOT_PROFILE_V1) {
    throw new Error('Direct capability root has the wrong canonical header');
  }
  requireZero(bytes, DirectAbi.CAPABILITY_ROOT_RESERVED_OFFSET, 4, 'capability root header');
  const selection = slice(bytes, DirectAbi.CAPABILITY_ROOT_SELECTION_OFFSET, DirectAbi.CAPABILITY_EXECUTION_SELECTION_BYTES_V1);
  if (!same(slice(selection, DirectAbi.CAPABILITY_EXECUTION_SELECTION_MAGIC_OFFSET, 8), DirectAbi.CAPABILITY_EXECUTION_SELECTION_MAGIC_V1)
      || u16(selection, DirectAbi.CAPABILITY_EXECUTION_SELECTION_SCHEMA_VERSION_OFFSET) !== DirectAbi.CAPABILITY_EXECUTION_SELECTION_SCHEMA_VERSION_V1
      || u16(selection, DirectAbi.CAPABILITY_EXECUTION_SELECTION_PROFILE_OFFSET) !== DirectAbi.CAPABILITY_EXECUTION_SELECTION_PROFILE_V1) {
    throw new Error('Direct capability selection has the wrong canonical header');
  }
  requireZero(selection, DirectAbi.CAPABILITY_EXECUTION_SELECTION_RESERVED_OFFSET, 2, 'capability selection header');
  const manifest = slice(selection, DirectAbi.CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET, 32);
  const kind = slice(selection, DirectAbi.CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET, 32);
  const programSet = slice(selection, DirectAbi.CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET, 32);
  const config = slice(selection, DirectAbi.CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET, 32);
  for (const [identity, field] of [[manifest, 'manifest'], [kind, 'kind'], [programSet, 'program set'], [config, 'config']] as const) {
    requireNonzero(identity, `capability selection ${field}`);
  }
  return Object.freeze({
    entryIndex: u16(selection, DirectAbi.CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET),
    manifest, kind, programSet, config,
  });
}

export function decodeSelectedDirectManifestEntryV1(bytes: Uint8Array, selection: DirectRootSelectionV1): DirectManifestEntryV1 {
  if (bytes.length < DirectAbi.MANIFEST_HEADER_BYTES
      || !same(slice(bytes, 0, 8), DirectAbi.MANIFEST_MAGIC)
      || u16(bytes, DirectAbi.MANIFEST_SCHEMA_OFFSET) !== 1
      || u16(bytes, DirectAbi.MANIFEST_PROFILE_OFFSET) !== 1) {
    throw new Error('capability manifest has the wrong exact header');
  }
  requireZero(bytes, DirectAbi.MANIFEST_RESERVED_OFFSET, 2, 'capability manifest header');
  const count = u16(bytes, DirectAbi.MANIFEST_COUNT_OFFSET);
  if (count === 0 || count > DirectAbi.MAX_CAPABILITIES
      || bytes.length !== DirectAbi.MANIFEST_HEADER_BYTES + count * DirectAbi.CAPABILITY_ENTRY_BYTES
      || selection.entryIndex >= count) {
    throw new Error('capability manifest width or selected entry index is invalid');
  }
  const dependencies: number[][] = [];
  let priorKind: Uint8Array | null = null;
  let selected: DirectManifestEntryV1 | null = null;
  for (let index = 0; index < count; index += 1) {
    const offset = DirectAbi.MANIFEST_HEADER_BYTES + index * DirectAbi.CAPABILITY_ENTRY_BYTES;
    const kind = slice(bytes, offset + DirectAbi.KIND_ID_OFFSET, 32);
    const programSet = slice(bytes, offset + DirectAbi.RELEASE_ID_OFFSET, 32);
    const config = slice(bytes, offset + DirectAbi.CONFIG_ID_OFFSET, 32);
    const capacityProfile = slice(bytes, offset + DirectAbi.CAPACITY_PROFILE_ID_OFFSET, 32);
    const childSchema = slice(bytes, offset + DirectAbi.CHILD_SCHEMA_ID_OFFSET, 32);
    const childDerivation = slice(bytes, offset + DirectAbi.CHILD_DERIVATION_ID_OFFSET, 32);
    for (const [identity, field] of [[kind, 'kind'], [programSet, 'release'], [config, 'config'], [capacityProfile, 'capacity'], [childSchema, 'child schema'], [childDerivation, 'child derivation']] as const) {
      requireNonzero(identity, `capability entry ${index} ${field}`);
    }
    if (priorKind !== null && compareIdentity(priorKind, kind) >= 0) throw new Error('capability manifest entries are not strictly ordered');
    priorKind = kind;
    requireZero(bytes, offset + DirectAbi.ENTRY_RESERVED_OFFSET, 6, `capability entry ${index}`);
    const policy = bytes[offset + DirectAbi.ACTIVATION_POLICY_OFFSET];
    const deadline = u64(bytes, offset + DirectAbi.ACTIVATION_DEADLINE_OFFSET);
    if ((policy !== 0 && policy !== 1) || (policy === 0 && deadline !== 0n) || (policy === 1 && deadline === 0n)) {
      throw new Error(`capability entry ${index} has a noncanonical activation policy`);
    }
    const dependencyCount = bytes[offset + DirectAbi.DEPENDENCY_COUNT_OFFSET] ?? 0;
    if (dependencyCount > DirectAbi.MAX_CAPABILITIES) throw new Error(`capability entry ${index} has too many dependencies`);
    const row: number[] = [];
    for (let position = 0; position < DirectAbi.MAX_CAPABILITIES; position += 1) {
      const dependency = bytes[offset + DirectAbi.DEPENDENCIES_OFFSET + position] ?? 0;
      if (position < dependencyCount) {
        if (dependency >= count || dependency === index || (row.length > 0 && (row.at(-1) ?? 0) >= dependency)) throw new Error(`capability entry ${index} dependency list is noncanonical`);
        row.push(dependency);
      } else if (dependency !== 0) throw new Error(`capability entry ${index} inactive dependency bytes are nonzero`);
    }
    dependencies.push(row);
    if (index === selection.entryIndex) selected = Object.freeze({ kind, programSet, config, capacityProfile, childSchema, childDerivation });
  }
  const resolved = new Set<number>();
  while (resolved.size < count) {
    const before = resolved.size;
    dependencies.forEach((row, index) => { if (!resolved.has(index) && row.every((dependency) => resolved.has(dependency))) resolved.add(index); });
    if (resolved.size === before) throw new Error('capability manifest dependency graph is cyclic');
  }
  if (selected === null || !same(selected.kind, selection.kind) || !same(selected.programSet, selection.programSet) || !same(selected.config, selection.config)) {
    throw new Error('selected capability manifest entry differs from the immutable root selection');
  }
  return selected;
}

function programSetSelection(bytes: Uint8Array): Uint8Array {
  if (bytes.length < CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1 || ascii(bytes, 0, 8) !== 'DCLTCPS1' || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 1) throw new Error('CapabilityProgramSet has the wrong exact header');
  if (new DataView(bytes.buffer, bytes.byteOffset + 12, 4).getUint32(0, true) !== 12 || bytes[16] !== 4 || bytes[17] !== 0) throw new Error('CapabilityProgramSet does not select canonical Direct u32 action offset 12');
  requireZero(bytes, 20, 12, 'CapabilityProgramSet header');
  const count = u16(bytes, 18);
  if (count === 0 || bytes.length !== CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1 + count * CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V1) throw new Error('CapabilityProgramSet has a noncanonical table width');
  let prior = -1;
  let selected: Uint8Array | null = null;
  for (let index = 0; index < count; index += 1) {
    const offset = CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1 + index * CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V1;
    const selector = new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
    if (selector <= prior) throw new Error('CapabilityProgramSet entries are not strictly ordered');
    prior = selector;
    requireZero(bytes, offset + 36, 4, 'CapabilityProgramSet entry');
    const digest = slice(bytes, offset + 4, 32);
    requireNonzero(digest, 'CapabilityProgramSet descriptor');
    if (selector === 1) selected = digest;
  }
  if (selected === null) throw new Error('CapabilityProgramSet has no InlineOrdinary action 1');
  return selected;
}

export function decodeDirectDescriptorV3(bytes: Uint8Array): Readonly<{
  configSchema: Uint8Array;
  rootSchema: Uint8Array;
  accountProfile: Uint8Array;
  lifecycle: Uint8Array;
  capacityProfile: Uint8Array;
  effect: Uint8Array;
  requestProfileSchema: Uint8Array;
  requestProfileProgram: Uint8Array;
  strategySchema: Uint8Array;
  strategyProgram: Uint8Array;
  rootStateBytes: number;
}> {
  if (bytes.length !== CAPABILITY_PROGRAM_V3_BYTES
      || !same(slice(bytes, 0, 8), DirectAbi.CAPABILITY_PROGRAM_V3_MAGIC)
      || u16(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_SCHEMA_VERSION_OFFSET) !== DirectAbi.CAPABILITY_PROGRAM_V3_SCHEMA_VERSION
      || u16(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE_OFFSET) !== DirectAbi.CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE
      || u16(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION_OFFSET) !== DirectAbi.CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION
      || u16(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION_OFFSET) !== DirectAbi.CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION) {
    throw new Error('CapabilityProgramV3 descriptor has the wrong exact ABI');
  }
  if (!same(slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_KIND_OFFSET, 32), DIRECT_SUCCESSOR_KIND_ID_V3)
      || !same(slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_CONFIG_SCHEMA_OFFSET, 32), DirectAbi.DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1)
      || !same(slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_REQUEST_SCHEMA_OFFSET, 32), DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3)
      || !same(slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_ROOT_SCHEMA_OFFSET, 32), DirectAbi.DIRECT_ROOT_SCHEMA_ID_V1)
      || !same(slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_OFFSET, 32), REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID)
      || !same(slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_OFFSET, 32), EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)) {
    throw new Error('selected descriptor is not the signed Direct V3 / Strategy V2 bundle');
  }
  requireZero(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_TAIL_RESERVED_OFFSET, 4, 'CapabilityProgramV3 tail');
  const rootStateBytes = readU32(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_ROOT_STATE_BYTES_OFFSET);
  if (rootStateBytes !== DirectAbi.DIRECT_ROOT_STATE_BYTES_V1) throw new Error('Direct descriptor selects another mutable root-tail width');
  return Object.freeze({
    configSchema: slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_CONFIG_SCHEMA_OFFSET, 32),
    rootSchema: slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_ROOT_SCHEMA_OFFSET, 32),
    accountProfile: slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_ACCOUNT_PROFILE_OFFSET, 32),
    lifecycle: slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_DERIVATION_POLICY_OFFSET, 32),
    capacityProfile: slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_CAPACITY_PROFILE_OFFSET, 32),
    effect: slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_EFFECT_PROGRAM_OFFSET, 32),
    requestProfileSchema: slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_OFFSET, 32),
    requestProfileProgram: slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_PROGRAM_OFFSET, 32),
    strategySchema: slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_OFFSET, 32),
    strategyProgram: slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V3_TRANSITION_PROGRAM_OFFSET, 32),
    rootStateBytes,
  });
}

export function validateDirectSignedRequestProfileV2(bytes: Uint8Array): void {
  if (bytes.length < DirectAbi.REQUEST_PROFILE_V2_HEADER_BYTES
      || !same(slice(bytes, 0, 8), DirectAbi.REQUEST_PROFILE_V2_MAGIC)
      || u16(bytes, 8) !== DirectAbi.REQUEST_PROFILE_V2_SCHEMA_VERSION
      || u16(bytes, 10) !== DirectAbi.REQUEST_PROFILE_V2_ARTIFACT_PROFILE) {
    throw new Error('RequestProfile V2 has the wrong exact header');
  }
  requireZero(bytes, DirectAbi.HEADER_RESERVED_OFFSET, 4, 'RequestProfile V2 header');
  const embeddedBytes = readU32(bytes, DirectAbi.EMBEDDED_V1_BYTES_OFFSET);
  const count = readU32(bytes, DirectAbi.REQUIREMENT_COUNT_OFFSET);
  const tail = DirectAbi.REQUEST_PROFILE_V2_HEADER_BYTES + embeddedBytes;
  if (count !== 2 || bytes.length !== tail + count * DirectAbi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1) {
    throw new Error('InlineOrdinary RequestProfile does not require exactly two bounded native signatures');
  }
  const embedded = slice(bytes, DirectAbi.REQUEST_PROFILE_V2_HEADER_BYTES, embeddedBytes);
  if (embedded.length < DirectAbi.REQUEST_PROFILE_HEADER_BYTES_V1
      || embedded.length > DirectAbi.REQUEST_PROFILE_MAX_BYTES_V1
      || !same(slice(embedded, 0, 8), DirectAbi.REQUEST_PROFILE_MAGIC_V1)
      || u16(embedded, DirectAbi.REQUEST_PROFILE_VERSION_OFFSET) !== DirectAbi.REQUEST_PROFILE_SCHEMA_VERSION_V1
      || u16(embedded, DirectAbi.REQUEST_PROFILE_ARTIFACT_OFFSET) !== DirectAbi.REQUEST_PROFILE_ARTIFACT_PROFILE_V1) {
    throw new Error('embedded RequestProfile V1 has the wrong exact header or bound');
  }
  const fixedRequestBytes = readU32(embedded, DirectAbi.REQUEST_PROFILE_FIXED_REQUEST_BYTES_OFFSET);
  const itemRequestBytes = readU32(embedded, DirectAbi.REQUEST_PROFILE_ITEM_REQUEST_BYTES_OFFSET);
  const fixedOperations = u16(embedded, DirectAbi.REQUEST_PROFILE_FIXED_OPERATIONS_OFFSET);
  const itemOperations = u16(embedded, DirectAbi.REQUEST_PROFILE_ITEM_OPERATIONS_OFFSET);
  const commonScalars = u16(embedded, DirectAbi.REQUEST_PROFILE_COMMON_SCALARS_OFFSET);
  const itemScalarStride = u16(embedded, DirectAbi.REQUEST_PROFILE_ITEM_SCALAR_STRIDE_OFFSET);
  const commonIdentities = u16(embedded, DirectAbi.REQUEST_PROFILE_COMMON_IDENTITIES_OFFSET);
  const itemIdentityStride = u16(embedded, DirectAbi.REQUEST_PROFILE_ITEM_IDENTITY_STRIDE_OFFSET);
  if (fixedRequestBytes !== DirectAbi.DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3
      || itemRequestBytes !== 0 || fixedOperations === 0 || itemOperations !== 0
      || itemScalarStride !== 0 || itemIdentityStride !== 0
      || commonScalars !== DIRECT_ORDINARY_COMMON_SCALARS_V3
      || commonIdentities !== DIRECT_ORDINARY_COMMON_IDENTITIES_V3
      || embedded.length !== DirectAbi.REQUEST_PROFILE_HEADER_BYTES_V1
        + fixedOperations * DirectAbi.REQUEST_PROFILE_OPERATION_BYTES_V1) {
    throw new Error('embedded RequestProfile does not select the exact fixed-width InlineOrdinary request');
  }
  const projected = new Set<string>();
  for (let index = 0; index < fixedOperations; index += 1) {
    const offset = DirectAbi.REQUEST_PROFILE_HEADER_BYTES_V1
      + index * DirectAbi.REQUEST_PROFILE_OPERATION_BYTES_V1;
    const opcode = embedded[offset + DirectAbi.REQUEST_OPERATION_OPCODE_OFFSET] ?? 0xff;
    const requestSpace = embedded[offset + DirectAbi.REQUEST_OPERATION_REQUEST_SPACE_OFFSET] ?? 0xff;
    const registerSpace = embedded[offset + DirectAbi.REQUEST_OPERATION_REGISTER_SPACE_OFFSET] ?? 0xff;
    const requestOffset = readU32(embedded, offset + DirectAbi.REQUEST_OPERATION_REQUEST_OFFSET_OFFSET);
    const register = u16(embedded, offset + DirectAbi.REQUEST_OPERATION_REGISTER_OFFSET);
    const immediate = u64(embedded, offset + DirectAbi.REQUEST_OPERATION_IMMEDIATE_OFFSET);
    if (requestSpace !== 0 || registerSpace !== 0
        || (embedded[offset + DirectAbi.REQUEST_OPERATION_RESERVED_BYTE_OFFSET] ?? 1) !== 0
        || u16(embedded, offset + DirectAbi.REQUEST_OPERATION_RESERVED_SHORT_OFFSET) !== 0
        || readU32(embedded, offset + DirectAbi.REQUEST_OPERATION_RESERVED_OFFSET) !== 0) {
      throw new Error(`embedded RequestProfile operation ${index} has noncanonical spaces or reserved bytes`);
    }
    const widths = [1, 2, 4, 8, 1, 1, 2, 4, 8, 32] as const;
    const width = widths[opcode];
    if (width === undefined || requestOffset + width > fixedRequestBytes) {
      throw new Error(`embedded RequestProfile operation ${index} has an unsupported opcode or request coordinate`);
    }
    if (opcode >= 5) {
      const identity = opcode === 9;
      const bound = identity ? commonIdentities : commonScalars;
      const target = `${identity ? 'i' : 's'}:${register}`;
      if (register >= bound || immediate !== 0n || projected.has(target)) {
        throw new Error(`embedded RequestProfile operation ${index} has an invalid or duplicate projection`);
      }
      projected.add(target);
    } else if (register !== 0 || (opcode === 4 && immediate === 0n)) {
      throw new Error(`embedded RequestProfile operation ${index} has a noncanonical requirement`);
    }
  }
  const destinations = new Set<number>();
  for (const [index, expectedOffset] of [[0, 192], [1, 396]] as const) {
    const offset = tail + index * DirectAbi.NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1;
    if (u16(bytes, offset) !== expectedOffset || u16(bytes, offset + 2) !== 172) {
      throw new Error(`InlineOrdinary signature requirement ${index} selects another current-instruction message`);
    }
    const destination = readU32(bytes, offset + 4);
    if (destination >= commonIdentities || destinations.has(destination)) throw new Error('InlineOrdinary signature requirements select an invalid or aliased identity register');
    destinations.add(destination);
  }
  if (!destinations.has(IDENTITY_SELLER_NATIVE_SIGNER_V3)
      || !destinations.has(IDENTITY_BUYER_NATIVE_SIGNER_V3)) {
    throw new Error('InlineOrdinary signature requirements do not target the canonical seller/buyer signer registers');
  }
}

function decodeInterpretedStrategy(bytes: Uint8Array): Readonly<{ transitionSchema: Uint8Array; transitionProgram: Uint8Array }> {
  if (bytes.length !== EXECUTION_STRATEGY_PROGRAM_BYTES_V2 || !same(slice(bytes, 0, 8), EXECUTION_STRATEGY_PROGRAM_MAGIC_V2)
      || u16(bytes, 8) !== EXECUTION_STRATEGY_SCHEMA_VERSION_V2 || u16(bytes, 10) !== EXECUTION_STRATEGY_ARTIFACT_PROFILE_V2
      || bytes[STRATEGY_DISPOSITION_OFFSET_V2] !== 0 || bytes[13] !== 0 || bytes[14] !== 0 || bytes[15] !== 0) {
    throw new Error('ExecutionStrategy is not the canonical interpreted V2 disposition');
  }
  const transitionSchema = slice(bytes, STRATEGY_TRANSITION_SCHEMA_OFFSET_V2, 32);
  const transitionProgram = slice(bytes, STRATEGY_TRANSITION_PROGRAM_OFFSET_V2, 32);
  if (!same(transitionSchema, TRANSITION_SCHEMA_RELEASE_ID)) throw new Error('interpreted strategy does not select TransitionVM V3');
  requireNonzero(transitionProgram, 'interpreted TransitionVM');
  return Object.freeze({ transitionSchema, transitionProgram });
}

function tailCountFromProfile(profile: Uint8Array, runtime: ReadonlyArray<RpcAccount>): number {
  const view = new DataView(profile.buffer, profile.byteOffset, profile.byteLength);
  const fixed = view.getUint16(12, true);
  const stride = view.getUint16(14, true);
  const fixedOperations = view.getUint16(16, true);
  const rules = fixed + stride;
  let found: number | null = null;
  for (let index = 0; index < fixedOperations; index += 1) {
    const offset = 32 + rules * 16 + index * ACCOUNT_PROFILE_OPERATION_BYTES;
    if (profile[offset] !== ACCOUNT_PROFILE_TAIL_COUNT_OPCODE) continue;
    if (found !== null || profile[offset + 1] !== 0) throw new Error('AccountProfile has ambiguous tail-count authority');
    const account = new DataView(profile.buffer, profile.byteOffset + offset + 2, 2).getUint16(0, true);
    const dataOffset = new DataView(profile.buffer, profile.byteOffset + offset + 8, 4).getUint32(0, true);
    const bytes = runtime[account]?.data;
    if (bytes === undefined || dataOffset + 4 > bytes.length) throw new Error('AccountProfile tail-count projection exceeds its authenticated runtime account');
    found = new DataView(bytes.buffer, bytes.byteOffset + dataOffset, 4).getUint32(0, true);
  }
  if (found === null || found === 0) throw new Error('AccountProfile has no positive Product-owned runtime tail count');
  return found;
}

function metas(
  coordinates: ReadonlyArray<DirectHotRouteCoordinateV3>,
  accounts: ReadonlyMap<string, RpcAccount | null>,
  field: string,
): ReadonlyArray<DirectHotAccountMetaV3> {
  return Object.freeze(coordinates.map((coordinate, index) => {
    const account = required(accounts, coordinate.address, `${field} ${index}`);
    return Object.freeze({ ...coordinate, executable: account.executable });
  }));
}

function lookupTable(address: string, account: RpcAccount): AddressLookupTableAccount {
  if (account.owner !== AddressLookupTableProgram.programId.toBase58() || account.executable) throw new Error(`lookup table ${address} has the wrong owner or executable bit`);
  let state: ReturnType<typeof AddressLookupTableAccount.deserialize>;
  try { state = AddressLookupTableAccount.deserialize(account.data); } catch { throw new Error(`lookup table ${address} has malformed data`); }
  const table = new AddressLookupTableAccount({ key: key(address, 'lookup table'), state });
  if (!table.isActive()) throw new Error(`lookup table ${address} is deactivated`);
  return table;
}

export async function inspectDirectHotRouteV3(
  client: SolanaRpcClient,
  manifest: DirectHotRouteManifestV3,
): Promise<DirectHotRouteInspectionV3> {
  if (manifest.fixedAccounts.length !== HOT_FIXED_ACCOUNT_COUNT_V3) throw new Error(`route manifest requires ${HOT_FIXED_ACCOUNT_COUNT_V3} fixed accounts`);
  if (manifest.strategyAccounts.length !== 0) throw new Error('interpreted ExecutionStrategy V2 admits no strategy-extra accounts');
  key(manifest.payer, 'payer');
  const addresses = [...manifest.fixedAccounts, ...manifest.strategyAccounts, ...manifest.runtimeAccounts].map((value) => value.address);
  const observation = await acquire(client, [...addresses, ...manifest.lookupTables]);
  const fixed = metas(manifest.fixedAccounts, observation.accounts, 'fixed account');
  if (new Set(fixed.map((value) => value.address)).size !== fixed.length) throw new Error('fixed hot frame aliases two roles');
  const strategyMetas = metas(manifest.strategyAccounts, observation.accounts, 'strategy account');
  const runtimeMetas = metas(manifest.runtimeAccounts, observation.accounts, 'runtime account');
  const marketAddress = fixed[HOT_MARKET_ACCOUNT_V3].address;
  const rootAddress = fixed[HOT_ROOT_ACCOUNT_V3].address;
  const coreProgram = fixed[HOT_CORE_PROGRAM_ACCOUNT_V3].address;
  const tradingProgram = fixed[HOT_TRADING_PROGRAM_ACCOUNT_V3].address;
  const registryProgram = fixed[HOT_REGISTRY_PROGRAM_ACCOUNT_V3].address;
  const market = required(observation.accounts, marketAddress, 'Market');
  const root = required(observation.accounts, rootAddress, 'capability root');
  if (market.owner !== coreProgram || market.executable || market.data.length !== CORE_STATE_BYTES || ascii(market.data, 0, 8) !== 'DCLTCOR2' || u16(market.data, 8) !== 2 || market.data[10] !== 1) throw new Error('Market is not one open Core V2 state owned by the selected Core program');
  if (root.owner !== tradingProgram || root.executable) throw new Error('Direct capability root has the wrong owner or executable bit');
  const selection = decodeDirectRootSelectionV1(root.data);
  const releaseSet = slice(market.data, MARKET_RELEASE_SET_OFFSET, 32);
  const generation = u64(market.data, MARKET_GENERATION_OFFSET);
  if (!same(slice(market.data, MARKET_REGISTRY_OFFSET, 32), key(registryProgram, 'Registry program').toBytes())
      || !same(slice(root.data, DirectAbi.CAPABILITY_ROOT_RELEASE_SET_OFFSET, 32), releaseSet)
      || !same(slice(root.data, DirectAbi.CAPABILITY_ROOT_MARKET_OFFSET, 32), key(marketAddress, 'Market').toBytes())
      || u64(root.data, DirectAbi.CAPABILITY_ROOT_GENERATION_OFFSET) !== generation
      || !same(selection.manifest, slice(market.data, MARKET_MANIFEST_OFFSET, 32))
      || !same(selection.kind, DIRECT_SUCCESSOR_KIND_ID_V3)) {
    throw new Error('Market, root, Registry, generation, release, or Direct selection does not join');
  }

  const manifestRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_MANIFEST_RAW_ACCOUNT_V3].address, fixed[HOT_MANIFEST_STAGING_ACCOUNT_V3].address,
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, slice(market.data, MARKET_MANIFEST_OFFSET, 32), 'capability manifest');
  const manifestEntry = decodeSelectedDirectManifestEntryV1(manifestRaw.data, selection);
  const programSetRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_PROGRAM_SET_RAW_ACCOUNT_V3].address, fixed[HOT_PROGRAM_SET_STAGING_ACCOUNT_V3].address,
    CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V1, selection.programSet, 'capability program set');
  const selectedProgram = programSetSelection(programSetRaw.data);
  const descriptorRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_DESCRIPTOR_RAW_ACCOUNT_V3].address, fixed[HOT_DESCRIPTOR_STAGING_ACCOUNT_V3].address,
    DESCRIPTORCONTRACT_SCHEMA_RELEASE_ID, selectedProgram, 'Direct descriptor');
  const descriptor = decodeDirectDescriptorV3(descriptorRaw.data);
  if (!same(manifestEntry.capacityProfile, descriptor.capacityProfile)
      || !same(manifestEntry.childSchema, descriptor.rootSchema)
      || !same(manifestEntry.childDerivation, descriptor.lifecycle)
      || root.data.length !== DirectAbi.CAPABILITY_ROOT_HEADER_BYTES_V1 + descriptor.rootStateBytes) {
    throw new Error('Direct descriptor, manifest entry, lifecycle, capacity, or mutable root width does not join');
  }
  const configRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_CONFIG_RAW_ACCOUNT_V3].address, fixed[HOT_CONFIG_STAGING_ACCOUNT_V3].address,
    descriptor.configSchema, selection.config, 'Direct config');
  if (configRaw.data.length !== DirectAbi.DIRECT_EXECUTION_CONFIG_BYTES_V1
      || !same(slice(configRaw.data, DirectAbi.DIRECT_CONFIG_MAGIC_OFFSET_V1, 8), DirectAbi.DIRECT_CONFIG_MAGIC_V1)
      || u16(configRaw.data, DirectAbi.DIRECT_CONFIG_VERSION_OFFSET_V1) !== 1) throw new Error('Direct config has the wrong exact ABI');
  requireZero(configRaw.data, DirectAbi.DIRECT_CONFIG_RESERVED_A_OFFSET_V1, 6, 'Direct config header');
  requireZero(configRaw.data, DirectAbi.DIRECT_CONFIG_RESERVED_B_OFFSET_V1, 6, 'Direct config fee field');
  requireNonzero(slice(configRaw.data, DirectAbi.DIRECT_CONFIG_FEE_RECIPIENT_OFFSET_V1, 32), 'Direct fee recipient');
  const priceScale = u64(configRaw.data, DirectAbi.DIRECT_CONFIG_PRICE_SCALE_OFFSET_V1);
  const feeBasisPoints = u16(configRaw.data, DirectAbi.DIRECT_CONFIG_FEE_BPS_OFFSET_V1);
  if (priceScale === 0n || feeBasisPoints > 10_000) throw new Error('Direct config price scale or fee rate is invalid');

  const productDigest = slice(market.data, MARKET_PRODUCT_RECORD_OFFSET, 32);
  const productRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_PRODUCT_RAW_ACCOUNT_V3].address, fixed[HOT_PRODUCT_STAGING_ACCOUNT_V3].address,
    PRODUCT_RECORD_SCHEMA_ID_V2, productDigest, 'Product Runtime V2 root');
  if (productRaw.data.length !== 112 || ascii(productRaw.data, 0, 8) !== 'DCLTPRM2' || u16(productRaw.data, 8) !== 2) {
    throw new Error('Product Runtime V2 root has the wrong exact ABI');
  }
  const resultDomainDigest = slice(productRaw.data, 48, 32);
  const portfolioDigest = slice(productRaw.data, 80, 32);
  const resultDomainRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3].address, fixed[HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3].address,
    RESULT_DOMAIN_SCHEMA_ID_V2, resultDomainDigest, 'Product result domain');
  const portfolioRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_PORTFOLIO_RAW_ACCOUNT_V3].address, fixed[HOT_PORTFOLIO_STAGING_ACCOUNT_V3].address,
    PORTFOLIO_SCHEMA_ID_V2, portfolioDigest, 'Product portfolio');
  const productGraph = decodeCoreFoundProductGraphV2(
    productRaw.data, resultDomainRaw.data, portfolioRaw.data, resultDomainDigest, portfolioDigest,
  );
  const linkedBasisRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_LINKED_BASIS_RAW_ACCOUNT_V3].address, fixed[HOT_LINKED_BASIS_STAGING_ACCOUNT_V3].address,
    GRADED_BASIS_RECORD_SCHEMA_ID_V3, await sha256(required(observation.accounts, fixed[HOT_LINKED_BASIS_RAW_ACCOUNT_V3].address, 'Product basis').data), 'Product basis');
  const basisWidth = await validateProductBasisV3(linkedBasisRaw.data, productGraph.productId, resultDomainDigest, resultDomainRaw.data);
  if (linkedBasisRaw.data[16] === 1 && basisWidth !== productGraph.outcomeCount) {
    throw new Error('categorical Product basis width differs from Product-owned outcome count');
  }

  const profileRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3].address, fixed[HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3].address,
    ACCOUNT_SCHEMA_RELEASE_ID, descriptor.accountProfile, 'AccountProfile V2');
  const requestProfileRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3].address, fixed[HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3].address,
    descriptor.requestProfileSchema, descriptor.requestProfileProgram, 'RequestProfile V2');
  validateDirectSignedRequestProfileV2(requestProfileRaw.data);
  await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_LIFECYCLE_RAW_ACCOUNT_V3].address, fixed[HOT_LIFECYCLE_STAGING_ACCOUNT_V3].address,
    LIFECYCLE_SCHEMA_RELEASE_ID, descriptor.lifecycle, 'state lifecycle policy');
  if (!same(descriptor.requestProfileSchema, REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID)
      || !same(descriptor.strategySchema, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)) throw new Error('descriptor selects another signed request or execution-strategy schema');
  const strategyRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_STRATEGY_RAW_ACCOUNT_V3].address, fixed[HOT_STRATEGY_STAGING_ACCOUNT_V3].address,
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, descriptor.strategyProgram, 'ExecutionStrategy V2');
  const interpreted = decodeInterpretedStrategy(strategyRaw.data);
  const transitionRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_TRANSITION_RAW_ACCOUNT_V3].address, fixed[HOT_TRANSITION_STAGING_ACCOUNT_V3].address,
    interpreted.transitionSchema, interpreted.transitionProgram, 'TransitionVM V3');
  await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HOT_EFFECT_RAW_ACCOUNT_V3].address, fixed[HOT_EFFECT_STAGING_ACCOUNT_V3].address,
    EFFECT_SCHEMA_RELEASE_ID, descriptor.effect, 'EffectProgram V3');
  const runtimeAccounts = manifest.runtimeAccounts.map((coordinate, index) => required(observation.accounts, coordinate.address, `runtime account ${index}`));
  const logicalRuntimeAccounts = [
    required(observation.accounts, fixed[HOT_ROOT_ACCOUNT_V3].address, 'logical capability root'),
    configRaw,
    productRaw,
    portfolioRaw,
    linkedBasisRaw,
    ...runtimeAccounts,
  ];
  const logicalRuntimeMetas = [
    fixed[HOT_ROOT_ACCOUNT_V3],
    fixed[HOT_CONFIG_RAW_ACCOUNT_V3],
    fixed[HOT_PRODUCT_RAW_ACCOUNT_V3],
    fixed[HOT_PORTFOLIO_RAW_ACCOUNT_V3],
    fixed[HOT_LINKED_BASIS_RAW_ACCOUNT_V3],
    ...runtimeMetas,
  ];
  const outcomeCount = tailCountFromProfile(profileRaw.data, logicalRuntimeAccounts);
  if (outcomeCount !== basisWidth) throw new Error('AccountProfile runtime width differs from Product-owned basis width');
  validateRuntimeAccountProfileV2(profileRaw.data, outcomeCount, logicalRuntimeMetas);

  let checkedOuter: CheckedHotOuterEvidenceV3 = Object.freeze({ status: 'unavailable', reason: 'no user-supplied checked infrastructure manifest recognizes this Trading release' });
  if (manifest.checkedInfrastructure !== null) {
    const checked = await decodeCheckedInfrastructureV1(manifest.checkedInfrastructure);
    if (checked.execution.releaseSet.id !== hex(releaseSet)) throw new Error('checked infrastructure selects another Market execution release set');
    const trading = checked.execution.artifacts.trading;
    const core = checked.execution.artifacts.core;
    if (trading.upgradeAuthority !== null || core.upgradeAuthority !== null) throw new Error('Core and Trading releases must both be immutable for checked hot execution');
    if (trading.program !== tradingProgram || core.program !== coreProgram) throw new Error('checked infrastructure selects another Core or Trading program');
    const tradingProgramAccount = required(observation.accounts, fixed[HOT_TRADING_PROGRAM_ACCOUNT_V3].address, 'Trading program');
    const tradingProgramData = required(observation.accounts, fixed[HOT_TRADING_PROGRAMDATA_ACCOUNT_V3].address, 'Trading ProgramData');
    const coreProgramAccount = required(observation.accounts, fixed[HOT_CORE_PROGRAM_ACCOUNT_V3].address, 'Core program');
    const coreProgramData = required(observation.accounts, fixed[HOT_CORE_PROGRAMDATA_ACCOUNT_V3].address, 'Core ProgramData');
    await authenticateArtifactDeploymentV1(tradingProgramAccount, tradingProgram, tradingProgramData, fixed[HOT_TRADING_PROGRAMDATA_ACCOUNT_V3].address, trading);
    await authenticateArtifactDeploymentV1(coreProgramAccount, coreProgram, coreProgramData, fixed[HOT_CORE_PROGRAMDATA_ACCOUNT_V3].address, core);
    const cache = required(observation.accounts, fixed[HOT_ACTIVATION_CACHE_ACCOUNT_V3].address, 'activation cache');
    if (cache.owner !== registryProgram || cache.executable || ascii(cache.data, 0, 8) !== 'DCLTACT1' || !same(slice(cache.data, 16, 32), releaseSet)) throw new Error('Registry activation cache does not select this Market release set');
    if (!same(slice(cache.data, ACTIVATION_CACHE_TRADING_OFFSET, 32), Uint8Array.from((checked.execution.releaseSet.roles.trading.artifactReleaseId.match(/../g) ?? []).map((value) => Number.parseInt(value, 16))))) throw new Error('activation cache Trading artifact differs from checked release evidence');
    checkedOuter = Object.freeze({ status: 'checked', tradingArtifactRelease: checked.execution.releaseSet.roles.trading.artifactReleaseId, checkedManifestDigest: checked.checkedInfrastructureId });
  }

  const latest = await client.latestBlockhash(observation.slot);
  const lookupTables = Object.freeze(manifest.lookupTables.map((address) => lookupTable(address, required(observation.accounts, address, 'lookup table'))));
  const route: DirectInlineHotRouteV3 = Object.freeze({
    payer: manifest.payer,
    tradingProgram,
    market: marketAddress,
    releaseSet,
    generation,
    rootPrestateDigest: await sha256(root.data),
    outcomeCount,
    priceScale,
    feeBasisPoints,
    accountProfile: profileRaw.data,
    fixedAccounts: fixed,
    strategyAccounts: strategyMetas,
    runtimeAccounts: runtimeMetas,
    recentBlockhash: latest.blockhash,
    lookupTables,
    outerEvidence: checkedOuter,
  });
  return Object.freeze({
    observedSlot: observation.slot,
    route,
    selectedProgramDigest: hex(selectedProgram),
    programSetDigest: hex(await sha256(programSetRaw.data)),
    accountProfileDigest: hex(await sha256(profileRaw.data)),
    strategyDigest: hex(await sha256(strategyRaw.data)),
    transitionDigest: hex(await sha256(transitionRaw.data)),
    checkedOuter,
  });
}
