import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { ascii, fromHex, hex, isZero, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import { decodeCapabilityManifestV1 } from './capabilityManifest';
import { decodeCoreFoundProductGraphV2 } from './coreFound';
import {
  decodeMarketCoreStateV2,
  type MarketCorePhaseV2,
  type MarketCoreReadinessV2,
} from './marketCoreV2';
import {
  CALLER_AUTHORITY_PDA_DOMAIN_V1,
  EXECUTION_ROLE_CLAIMS_V1,
  EXECUTION_ROLE_CORE_V1,
  EXECUTION_ROLE_CUSTODY_V1,
  EXECUTION_ROLE_RESOLUTION_V1,
  EXECUTION_ROLE_TRADING_V1,
} from './generated/claimsCustodyReplayV1';
import {
  CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
  LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 as CLAIMS_MARKET_HEADER_BYTES,
  LIABILITY_BASIS_MARKET_MAGIC_V2,
  LIABILITY_BASIS_MARKET_SEED_V2 as CLAIMS_MARKET_SEED,
  LIABILITY_BASIS_POSITION_SEED_V2 as POSITION_SEED,
  LIFECYCLE_RENT_CREDIT_BYTES_V2,
  LIFECYCLE_RENT_CREDIT_MAGIC_V2,
  LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
} from './generated/coreFound';
import { MAX_TX_ACCOUNT_LOCKS_V2 } from './generated/genericFoundingV1';
import * as Lifecycle from './generated/rationalLifecycleRequestV2';
import {
  DESCRIPTOR_COEFFICIENT_BYTES,
  DESCRIPTOR_DENOMINATOR_OFFSET,
  DESCRIPTOR_GRAPH_DIGEST_OFFSET,
  DESCRIPTOR_GRAPH_ID_OFFSET,
  DESCRIPTOR_HEADER_BYTES,
  DESCRIPTOR_MAGIC_BYTES,
  DESCRIPTOR_MAGIC_OFFSET,
  DESCRIPTOR_MAGIC_V3,
  DESCRIPTOR_MARKET_ID_OFFSET,
  DESCRIPTOR_OUTCOME_COUNT_OFFSET,
  DESCRIPTOR_RECEIPT_MINT_OFFSET,
  DESCRIPTOR_RELEASE_SET_ID_OFFSET,
  DESCRIPTOR_RESERVED_BYTES,
  DESCRIPTOR_RESERVED_HEADER_BYTES,
  DESCRIPTOR_RESERVED_HEADER_OFFSET,
  DESCRIPTOR_RESERVED_OFFSET,
  DESCRIPTOR_ROOT_ID_OFFSET,
  DESCRIPTOR_SCHEMA_VERSION_V3,
  DESCRIPTOR_TOKEN_PROGRAM_OFFSET,
  DESCRIPTOR_VERSION_OFFSET,
  REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
} from './generated/rationalRepresentationDescriptorV3';
import * as Hot from './generated/directInlineV3';
import {
  ACTIVATION_CACHE_BYTES,
  ARTIFACT_RELEASE_BYTES,
  NATIVE_LOADER_ID,
  RENT_SYSVAR_ID,
  SYSTEM_PROGRAM_ID,
  UPGRADEABLE_LOADER_ID,
  decodeArtifactReleaseV1,
  decodeExecutionReleaseSetV1,
  deriveFinalizedRecordAddressesV1,
  REGISTRY_ACTIVATION_PDA_SEED_V1 as ACTIVATION_SEED,
} from './releaseRegistry';
import { decodeToken2022BehaviorMintV2, TOKEN_2022_PROGRAM_ID } from './rationalTokenV2';
import {
  decodeRationalProductBasisV3,
  type RationalProductBasisViewV3,
} from './rationalTerminalHotV3';
import { type RpcAccount, type SolanaRpcClient } from './rpc';
import { SOLANA_PACKET_BYTES_V1 } from './solanaLimits';
import {
  ACTIVATION_CACHE_MAGIC_V1,
  EXECUTION_RELEASE_SET_MAGIC_V1,
  PROTOCOL_POSITION_CLAIMS_CAPABILITY_SEED_V2,
  RATIONAL_RECEIPT_MINT_SEED_V2,
  RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
  RATIONAL_SHARD_MINT_SEED_V2,
  RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
} from './generated/protocolConstantsV1';
import {
  PROTOCOL_POSITION_ADMISSION_SEED_V2,
} from './generated/directParticipantV1';

export const RATIONAL_LIFECYCLE_COMPACT_REQUEST_BYTES_V4 = Lifecycle.RATIONAL_LIFECYCLE_COMPACT_HOT_REQUEST_BYTES_V4;
export const RATIONAL_LIFECYCLE_COMPACT_OUTER_BYTES_V4 = Hot.HOT_EXECUTION_ENVELOPE_BYTES_V3 + RATIONAL_LIFECYCLE_COMPACT_REQUEST_BYTES_V4;
export const RATIONAL_LIFECYCLE_CLAIMS_COMMON_ACCOUNTS_V2 = Lifecycle.LIFECYCLE_COMMON_ACCOUNT_COUNT_V2;
/**
 * The Claims accounts one proven-vacant support row adds.
 *
 * FIVE, and this file said four until the emission above arrived. `e78fa027d`
 * gave the compact row its custody-owner account so the effect could PROJECT
 * the address rather than bake it into the artifacts; the contract went to
 * five, the client stayed at four, and a compact RetireReceipt built here
 * carried a frame one account per row short of the one the program reads.
 */
export const RATIONAL_LIFECYCLE_VACANCY_ACCOUNTS_V2 = Lifecycle.LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2;
export const RATIONAL_REPRESENTATION_DESCRIPTOR_SCHEMA_V3 = REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3;
export const RATIONAL_LIFECYCLE_COMPACT_REQUEST_SCHEMA_V4 = Lifecycle.RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_RELEASE_ID_V4;
export const CAPABILITY_PROGRAM_V4_SCHEMA = Uint8Array.from([
  0x2d, 0x85, 0xb2, 0x21, 0x7c, 0x9b, 0x58, 0xbb, 0x59, 0xb8, 0x5d, 0x43, 0x7f, 0xf4, 0xd1, 0x7f,
  0xa0, 0x70, 0x58, 0xd9, 0x5d, 0xee, 0xb7, 0xd2, 0x58, 0x43, 0xa6, 0xea, 0x31, 0x30, 0x11, 0x62,
]);

const MAX_U64 = 18_446_744_073_709_551_615n;
const SYSTEM_INSTRUCTIONS_SYSVAR = 'Sysvar1nstructions1111111111111111111111111';
const CALLER_AUTHORITY_SEED = CALLER_AUTHORITY_PDA_DOMAIN_V1;
const REPRESENTATION_AUTHORITY_SEED = RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2;
const RECEIPT_MINT_SEED = RATIONAL_RECEIPT_MINT_SEED_V2;
const SHARD_MINT_SEED = RATIONAL_SHARD_MINT_SEED_V2;
const STRUCTURED_CUSTODY_SEED = RATIONAL_STRUCTURED_CUSTODY_SEED_V2;
const CLAIMS_CUSTODY_OWNER_SEED = PROTOCOL_POSITION_CLAIMS_CAPABILITY_SEED_V2;
const ADMISSION_SEED = PROTOCOL_POSITION_ADMISSION_SEED_V2;
const SEMANTIC_BASIS_CONTENT_DOMAIN_V3 = new TextEncoder().encode('dclutch/product-basis/semantic/v3');

export type RationalHotAccountMetaV4 = Readonly<{ address: string; isSigner: boolean; isWritable: boolean }>;
type AccountMetaV4 = RationalHotAccountMetaV4;
export type CompactSupportRowV4 = Readonly<{
  outcome: number;
  coefficient: bigint;
  owner: string;
  shardMint: string;
  structuredCustody: string;
  position: string;
  admission: string;
}>;

export type RationalRetireReceiptInspectionV4 = Readonly<{
  observedSlot: string;
  payer: string;
  fixedAccounts: ReadonlyArray<AccountMetaV4>;
  claimsAccounts: ReadonlyArray<AccountMetaV4>;
  support: ReadonlyArray<CompactSupportRowV4>;
  lookupTable: AddressLookupTableAccount;
  market: string;
  generation: bigint;
  releaseSet: Uint8Array;
  descriptorId: Uint8Array;
  graphId: Uint8Array;
  representationAuthority: string;
  receiptMint: string;
  claimsProgram: string;
  claimsRevision: bigint;
  representationWidth: number;
  resultOutcomeCount: number;
  rentCredit: string;
  rentProgram: string;
  receiptLamports: bigint;
  receiptRentPrincipal: bigint;
  rentCreditBefore: bigint;
  familyBytes: Uint8Array;
  familyDigest: Uint8Array;
  childDigest: Uint8Array;
  rootDigest: Uint8Array;
  callerAuthority: string;
  executionStatus: 'ready';
}>;

export type RationalRetireReceiptCandidateV4 = Readonly<{
  transaction: VersionedTransaction;
  instruction: TransactionInstruction;
  familyBytes: Uint8Array;
  outerBytes: Uint8Array;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
  loadedAddresses: number;
  accountLocks: number;
  accountCount: number;
  supportCount: number;
  executionStatus: 'ready';
}>;

export type RationalHotRpcV4 = Pick<
  SolanaRpcClient,
  'finalizedSlot' | 'multipleAccounts' | 'minimumBalanceForRentExemption'
>;
type RetireRpc = RationalHotRpcV4;

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function concatenate(parts: ReadonlyArray<Uint8Array>): Uint8Array {
  const total = parts.reduce((sum, part) => sum + part.length, 0);
  if (!Number.isSafeInteger(total)) throw new Error('semantic basis preimage exceeds the browser exact-length bound');
  const output = new Uint8Array(total); let offset = 0;
  for (const part of parts) { output.set(part, offset); offset += part.length; }
  return output;
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
  if (value < 0n || value > MAX_U64) throw new Error('lifecycle scalar is outside canonical u64');
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

function chunks<T>(values: ReadonlyArray<T>, width: number): T[][] {
  const output: T[][] = [];
  for (let index = 0; index < values.length; index += width) output.push(values.slice(index, index + width));
  return output;
}

export async function acquireRationalHotAccountsV4(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>,
  addresses: ReadonlyArray<string>,
  minimumSlot?: string,
): Promise<Readonly<{ slot: string; accounts: ReadonlyMap<string, RpcAccount | null> }>> {
  const canonical = [...new Set(addresses.map((address, index) => key(address, `route address ${index}`).toBase58()))];
  const floor = minimumSlot ?? await client.finalizedSlot();
  const accounts = new Map<string, RpcAccount | null>();
  let slot = floor;
  for (const group of chunks(canonical, 32)) {
    const observation = await client.multipleAccounts(group, floor);
    if (BigInt(observation.slot) < BigInt(floor)) throw new Error('retirement observation regressed below its finalized floor');
    if (BigInt(observation.slot) > BigInt(slot)) slot = observation.slot;
    observation.accounts.forEach((entry) => accounts.set(entry.address, entry.account));
  }
  return Object.freeze({ slot, accounts });
}

function vacant(account: RpcAccount | null | undefined, field: string): void {
  if (account === null || account === undefined) return;
  if (account.owner !== SYSTEM_PROGRAM_ID || account.executable || account.data.length !== 0 || account.lamports !== '0') {
    throw new Error(`${field} is not an absent or zero-lamport vacant System account`);
  }
}

export function decodeRationalHotLookupTableV4(address: string, account: RpcAccount): AddressLookupTableAccount {
  if (account.owner !== AddressLookupTableProgram.programId.toBase58() || account.executable) {
    throw new Error('address lookup table has the wrong owner or executable bit');
  }
  let state: ReturnType<typeof AddressLookupTableAccount.deserialize>;
  try { state = AddressLookupTableAccount.deserialize(account.data); } catch { throw new Error('address lookup table has malformed data'); }
  const table = new AddressLookupTableAccount({ key: key(address, 'address lookup table'), state });
  if (!table.isActive()) throw new Error('address lookup table is deactivated');
  return table;
}

export async function authenticateFinalizedRationalHotRecordV4(
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
  if (raw.owner !== registry || raw.executable) throw new Error(`${field} is not Registry-owned finalized data`);
  const digest = await sha256(raw.data);
  if (!same(digest, expectedDigest)) throw new Error(`${field} bytes differ from their selected content identity`);
  const derived = deriveFinalizedRecordAddressesV1(registry, schema, digest);
  if (derived.record !== rawAddress || derived.staging !== stagingAddress) throw new Error(`${field} raw/staging accounts are not canonical Registry PDAs`);
  vacant(staging, `${field} staging cursor`);
  const rent = await client.minimumBalanceForRentExemption(raw.data.length);
  if (BigInt(raw.lamports) < BigInt(rent.lamports)) throw new Error(`${field} is below its current exact rent minimum`);
  return raw;
}

/**
 * Authenticate one finalized ProductBasisV3 and its exact semantic join.
 * Representation/native-claims width `K` is independent from Product result
 * width `N`; callers supply only `K` here and validate `N` in Product state.
 */
export async function authenticateRationalProductBasisRecordV3(
  client: Pick<SolanaRpcClient, 'minimumBalanceForRentExemption'>,
  accounts: ReadonlyMap<string, RpcAccount | null>,
  input: Readonly<{
    registry: string;
    rawAddress: string;
    stagingAddress: string;
    productId: Uint8Array;
    domainDigest: Uint8Array;
    domainBytes: Uint8Array;
    representationWidth: number;
  }>,
): Promise<Readonly<{
  basis: RationalProductBasisViewV3;
  digest: Uint8Array;
  semanticBasisId: Uint8Array;
}>> {
  const raw = required(accounts, input.rawAddress, 'ProductBasisV3 raw');
  const digest = await sha256(raw.data);
  await authenticateFinalizedRationalHotRecordV4(
    client,
    accounts,
    input.registry,
    input.rawAddress,
    input.stagingAddress,
    Hot.GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    digest,
    'ProductBasisV3',
  );
  const basis = decodeRationalProductBasisV3(raw.data);
  const semanticBasisId = await sha256(concatenate([
    SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
    basis.bytes.slice(0, 32),
    basis.bytes.slice(96),
  ]));
  requireNonzero(semanticBasisId, 'Product semantic basis');
  if (!same(basis.productId, input.productId)
      || !same(basis.resultDomainId, input.domainDigest)
      || !same(basis.coordinateDomainId, slice(input.domainBytes, 64, 32))
      || !same(basis.resultUnitId, slice(input.domainBytes, 96, 32))
      || !same(semanticBasisId, slice(input.domainBytes, 128, 32))
      || basis.width !== input.representationWidth) {
    throw new Error('ProductBasisV3 does not join Product/domain semantics or representation K');
  }
  return Object.freeze({ basis, digest, semanticBasisId });
}

export type RationalHotCoreViewV3 = Readonly<{
  phase: MarketCorePhaseV2;
  readiness: MarketCoreReadinessV2;
  terminalWinner: number;
  terminalReceipt: Uint8Array;
  realm: Uint8Array;
  resolutionPolicy: Uint8Array;
  productRecord: Uint8Array;
  productId: Uint8Array;
  manifest: Uint8Array;
  releaseSet: Uint8Array;
  registry: string;
  generation: bigint;
  principalCapSets: bigint;
  rentBeneficiary: string;
}>;

/** Authenticate account ownership around the canonical current Core decoder. */
export function authenticateRationalHotCoreV3(address: string, account: RpcAccount, coreProgram: string): RationalHotCoreViewV3 {
  if (account.owner !== coreProgram || account.executable) {
    throw new Error('Market is not nonexecutable state under the selected Core program');
  }
  const state = decodeMarketCoreStateV2(address, account.data);
  if (state.marketId !== key(address, 'Market').toBase58()) throw new Error('Core Market identity differs from its account address');
  const terminal = state.settlement.status === 'terminal' ? state.settlement : null;
  return Object.freeze({
    phase: state.phase,
    readiness: state.readiness,
    terminalWinner: terminal?.winner ?? 0,
    terminalReceipt: terminal === null ? new Uint8Array(32) : fromHex(terminal.receiptId, 'Core terminal receipt'),
    realm: fromHex(state.identity.realmId, 'Core Realm identity'),
    productRecord: fromHex(state.identity.productRecordId, 'Core Product record identity'),
    productId: fromHex(state.identity.productInstanceId, 'Core Product identity'),
    resolutionPolicy: fromHex(state.identity.resolutionPolicyId, 'Core Resolution policy identity'),
    manifest: fromHex(state.identity.capabilityManifestId, 'Core capability manifest identity'),
    releaseSet: fromHex(state.identity.selectedReleaseSetId, 'Core selected release set'),
    registry: state.identity.registryProgram,
    generation: BigInt(state.identity.generation),
    principalCapSets: BigInt(state.principalCapSets),
    rentBeneficiary: state.rentBeneficiary,
  });
}

export type RationalHotRootSelectionV4 = Readonly<{ entryIndex: number; manifest: Uint8Array; kind: Uint8Array; programSet: Uint8Array; config: Uint8Array }>;
type RootSelection = RationalHotRootSelectionV4;

export function decodeRationalHotRootV4(bytes: Uint8Array, releaseSet: Uint8Array, market: string, generation: bigint): RootSelection {
  if (bytes.length <= Hot.CAPABILITY_ROOT_HEADER_BYTES_V1 || !same(slice(bytes, 0, 8), Hot.CAPABILITY_ROOT_MAGIC_V1) || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 1) {
    throw new Error('capability root has the wrong exact V1 header or no mutable state');
  }
  requireZero(bytes, 12, 4, 'capability root header');
  if (!same(slice(bytes, 16, 32), releaseSet) || new PublicKey(slice(bytes, 48, 32)).toBase58() !== market || u64(bytes, 80) !== generation) {
    throw new Error('capability root release, Market, or generation differs from Core');
  }
  if (!same(slice(bytes, 88, 8), Hot.CAPABILITY_EXECUTION_SELECTION_MAGIC_V1) || u16(bytes, 96) !== 1 || u16(bytes, 98) !== 1) {
    throw new Error('capability root selection has the wrong exact ABI');
  }
  requireZero(bytes, 102, 2, 'capability selection');
  const coordinates = [104, 136, 168, 200].map((offset) => slice(bytes, offset, 32));
  coordinates.forEach((coordinate, index) => requireNonzero(coordinate, `capability selection identity ${index}`));
  return Object.freeze({ entryIndex: u16(bytes, 100), manifest: coordinates[0], kind: coordinates[1], programSet: coordinates[2], config: coordinates[3] });
}

export type RationalHotManifestEntryV4 = Readonly<{ capacity: Uint8Array; rootSchema: Uint8Array; derivation: Uint8Array }>;
type ManifestEntry = RationalHotManifestEntryV4;

export function selectRationalHotManifestEntryV4(bytes: Uint8Array, selection: RootSelection): ManifestEntry {
  const entries = decodeCapabilityManifestV1(bytes);
  if (selection.entryIndex >= entries.length) throw new Error('capability manifest width or selected index is invalid');
  const entry = entries[selection.entryIndex];
  if (!same(entry.kind, selection.kind) || !same(entry.programSet, selection.programSet) || !same(entry.config, selection.config)) {
    throw new Error('selected manifest entry differs from the immutable capability-root selection');
  }
  return Object.freeze({ capacity: entry.capacity, rootSchema: entry.rootSchema, derivation: entry.derivation });
}

export function selectRationalHotCapabilityV4(set: Uint8Array, selector: number): Readonly<{ schema: Uint8Array; digest: Uint8Array }> {
  if (set.length < Hot.CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2 || !same(slice(set, 0, 8), Hot.CAPABILITY_PROGRAM_SET_MAGIC_V2) || u16(set, 8) !== 2 || u16(set, 10) !== 2) {
    throw new Error('compact retirement does not select CapabilityProgramSetV2');
  }
  const selectorOffset = u32(set, 12);
  const selectorWidth = set[16];
  const count = u16(set, 18);
  if (selectorOffset !== 10 || selectorWidth !== 1 || set[17] !== 0 || count === 0 || count > 32
      || set.length !== Hot.CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2 + count * Hot.CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2) {
    throw new Error('compact retirement ProgramSet has noncanonical selector geometry');
  }
  requireZero(set, 20, 12, 'CapabilityProgramSetV2 header');
  let prior = -1;
  let selected: Readonly<{ schema: Uint8Array; digest: Uint8Array }> | null = null;
  for (let index = 0; index < count; index += 1) {
    const offset = Hot.CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2 + index * Hot.CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2;
    const value = u32(set, offset);
    if (value <= prior || value > 255) throw new Error('CapabilityProgramSetV2 selectors are not canonical');
    prior = value;
    requireZero(set, offset + 68, 4, 'CapabilityProgramSetV2 entry');
    const schema = slice(set, offset + 4, 32);
    const digest = slice(set, offset + 36, 32);
    requireNonzero(schema, 'CapabilityProgramSetV2 descriptor schema');
    requireNonzero(digest, 'CapabilityProgramSetV2 descriptor content');
    if (value === selector) selected = Object.freeze({ schema, digest });
  }
  if (selected === null || !same(selected.schema, CAPABILITY_PROGRAM_V4_SCHEMA)) {
    throw new Error('RetireReceipt does not select one CapabilityProgramV4 descriptor');
  }
  return selected;
}

export type RationalHotCapabilityV4 = Readonly<{
  kind: Uint8Array;
  configSchema: Uint8Array;
  requestSchema: Uint8Array;
  rootSchema: Uint8Array;
  derivation: Uint8Array;
  capacity: Uint8Array;
  artifacts: ReadonlyArray<Readonly<{ schema: Uint8Array; digest: Uint8Array }>>;
  rootStateBytes: number;
}>;

export function decodeRationalHotCapabilityV4(bytes: Uint8Array): RationalHotCapabilityV4 {
  if (bytes.length !== Hot.CAPABILITY_PROGRAM_V4_BYTES || !same(slice(bytes, 0, 8), Hot.CAPABILITY_PROGRAM_V4_MAGIC) || u16(bytes, 8) !== 4 || u16(bytes, 10) !== 4) {
    throw new Error('selected descriptor is not the exact CapabilityProgramV4 ABI');
  }
  requireZero(bytes, 12, 4, 'CapabilityProgramV4 header');
  requireZero(bytes, 596, 4, 'CapabilityProgramV4 tail');
  const common = [16, 48, 80, 112, 144, 176].map((offset) => slice(bytes, offset, 32));
  const artifacts = [208, 272, 336, 400, 464, 528].map((offset) => Object.freeze({
    schema: slice(bytes, offset, 32), digest: slice(bytes, offset + 32, 32),
  }));
  [...common, ...artifacts.flatMap((artifact) => [artifact.schema, artifact.digest])]
    .forEach((identity, index) => requireNonzero(identity, `CapabilityProgramV4 identity ${index}`));
  const rootStateBytes = u32(bytes, 592);
  if (rootStateBytes === 0) throw new Error('CapabilityProgramV4 selects a zero mutable root width');
  return Object.freeze({
    kind: common[0], configSchema: common[1], requestSchema: common[2], rootSchema: common[3],
    derivation: common[4], capacity: common[5], artifacts: Object.freeze(artifacts), rootStateBytes,
  });
}

export type RationalRepresentationDescriptorViewV3 = Readonly<{
  id: Uint8Array;
  graphId: Uint8Array;
  graphDigest: Uint8Array;
  rootId: Uint8Array;
  market: string;
  releaseSet: Uint8Array;
  receiptMint: string;
  tokenProgram: string;
  outcomeCount: number;
  denominator: bigint;
  support: ReadonlyArray<Readonly<{ outcome: number; coefficient: bigint }>>;
}>;

/**
 * The descriptor's seven 32-byte identities, in the order the view names them.
 *
 * The names are listed; the coordinates are not. Each one is the Lean-emitted
 * offset, so a field that moves in the schema moves here by regeneration.
 */
const DESCRIPTOR_IDENTITY_OFFSETS_V3 = [
  DESCRIPTOR_GRAPH_ID_OFFSET, DESCRIPTOR_GRAPH_DIGEST_OFFSET, DESCRIPTOR_ROOT_ID_OFFSET,
  DESCRIPTOR_MARKET_ID_OFFSET, DESCRIPTOR_RELEASE_SET_ID_OFFSET, DESCRIPTOR_RECEIPT_MINT_OFFSET,
  DESCRIPTOR_TOKEN_PROGRAM_OFFSET,
] as const;

export function decodeRationalRepresentationDescriptorV3(bytes: Uint8Array, id: Uint8Array): RationalRepresentationDescriptorViewV3 {
  if (bytes.length < DESCRIPTOR_HEADER_BYTES
      || !same(slice(bytes, DESCRIPTOR_MAGIC_OFFSET, DESCRIPTOR_MAGIC_BYTES), DESCRIPTOR_MAGIC_V3)
      || u16(bytes, DESCRIPTOR_VERSION_OFFSET) !== DESCRIPTOR_SCHEMA_VERSION_V3) {
    throw new Error('Rational representation descriptor has the wrong exact V3 header');
  }
  requireZero(bytes, DESCRIPTOR_RESERVED_HEADER_OFFSET, DESCRIPTOR_RESERVED_HEADER_BYTES, 'representation descriptor header');
  requireZero(bytes, DESCRIPTOR_RESERVED_OFFSET, DESCRIPTOR_RESERVED_BYTES, 'representation descriptor body');
  const outcomeCount = u32(bytes, DESCRIPTOR_OUTCOME_COUNT_OFFSET);
  if (outcomeCount === 0 || bytes.length !== DESCRIPTOR_HEADER_BYTES + outcomeCount * DESCRIPTOR_COEFFICIENT_BYTES) {
    throw new Error('representation descriptor outcome width is inconsistent');
  }
  const identities = DESCRIPTOR_IDENTITY_OFFSETS_V3.map((offset) => slice(bytes, offset, 32));
  identities.forEach((identity, index) => requireNonzero(identity, `representation descriptor identity ${index}`));
  const denominator = u64(bytes, DESCRIPTOR_DENOMINATOR_OFFSET);
  if (denominator === 0n) throw new Error('representation descriptor denominator is zero');
  const support: Array<Readonly<{ outcome: number; coefficient: bigint }>> = [];
  for (let outcome = 0; outcome < outcomeCount; outcome += 1) {
    const coefficient = u64(bytes, DESCRIPTOR_HEADER_BYTES + outcome * DESCRIPTOR_COEFFICIENT_BYTES);
    if (coefficient !== 0n) support.push(Object.freeze({ outcome, coefficient }));
  }
  if (support.length === 0) throw new Error('representation descriptor has empty support');
  return Object.freeze({
    id, graphId: identities[0], graphDigest: identities[1], rootId: identities[2], market: new PublicKey(identities[3]).toBase58(),
    releaseSet: identities[4], receiptMint: new PublicKey(identities[5]).toBase58(),
    tokenProgram: new PublicKey(identities[6]).toBase58(), outcomeCount, denominator,
    support: Object.freeze(support),
  });
}

function decodeClaimsMarket(bytes: Uint8Array, market: string, releaseSet: Uint8Array, registry: string, productRecord: Uint8Array, realm: Uint8Array, generation: bigint, outcomeCount: number): bigint {
  if (bytes.length !== CLAIMS_MARKET_HEADER_BYTES + outcomeCount * 8 || !same(slice(bytes, 0, 8), LIABILITY_BASIS_MARKET_MAGIC_V2) || u16(bytes, 8) !== 2 || u32(bytes, 12) !== outcomeCount) {
    throw new Error('Claims aggregate has the wrong exact runtime-width ABI');
  }
  requireZero(bytes, 10, 2, 'Claims aggregate header');
  const joins = [
    [slice(bytes, 24, 32), key(market, 'Market').toBytes(), 'Market'],
    [slice(bytes, 56, 32), releaseSet, 'release set'],
    [slice(bytes, 88, 32), key(registry, 'Registry program').toBytes(), 'Registry'],
    [slice(bytes, 120, 32), productRecord, 'Product record'],
    [slice(bytes, 184, 32), realm, 'Realm'],
  ] as const;
  for (const [observed, expected, field] of joins) if (!same(observed, expected)) throw new Error(`Claims aggregate ${field} differs from Core`);
  requireNonzero(slice(bytes, 152, 32), 'Claims aggregate basis');
  requireNonzero(slice(bytes, 216, 32), 'Claims aggregate custody context');
  if (u64(bytes, 248) !== generation) throw new Error('Claims aggregate generation differs from Core');
  return u64(bytes, 16);
}

function decodeLifecycleRentCredit(address: string, account: RpcAccount, market: string, releaseSet: Uint8Array, generation: bigint): Readonly<{ program: string; balance: bigint }> {
  if (account.executable || account.data.length !== LIFECYCLE_RENT_CREDIT_BYTES_V2 || !same(slice(account.data, 0, 8), LIFECYCLE_RENT_CREDIT_MAGIC_V2) || u16(account.data, 8) !== 2) {
    throw new Error('RentCredit is not the exact LifecycleRentCreditV2 ABI');
  }
  requireZero(account.data, 11, 5, 'LifecycleRentCreditV2 header');
  requireZero(account.data, 120, 8, 'LifecycleRentCreditV2 tail');
  const refundWallet = slice(account.data, 16, 32); requireNonzero(refundWallet, 'LifecycleRentCreditV2 refund wallet');
  if (!same(slice(account.data, 48, 32), key(market, 'Market').toBytes()) || !same(slice(account.data, 80, 32), releaseSet)
      || u64(account.data, 112) !== generation) throw new Error('LifecycleRentCreditV2 differs from the Market lifecycle');
  if (same(refundWallet, key(market, 'Market').toBytes()) || same(refundWallet, releaseSet)
      || same(key(market, 'Market').toBytes(), releaseSet)) throw new Error('LifecycleRentCreditV2 aliases immutable lifecycle identities');
  const [expectedKey, expectedBump] = PublicKey.findProgramAddressSync([
    LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, key(market, 'Market').toBytes(), le64(generation),
  ], key(account.owner, 'Rent program'));
  if (expectedKey.toBase58() !== address || account.data[10] !== expectedBump) {
    throw new Error('LifecycleRentCreditV2 is not the exact Market+generation PDA under its owner program');
  }
  return Object.freeze({ program: account.owner, balance: BigInt(account.lamports) });
}

function le32(value: number): Uint8Array {
  const bytes = new Uint8Array(4); putU32(bytes, 0, value); return bytes;
}

function le64(value: bigint): Uint8Array {
  const bytes = new Uint8Array(8); putU64(bytes, 0, value); return bytes;
}

export function deriveRationalRetireReceiptSupportV4(claimsProgram: string, descriptorId: Uint8Array, support: RationalRepresentationDescriptorViewV3['support'], aggregate: string): ReadonlyArray<CompactSupportRowV4> {
  const program = key(claimsProgram, 'Claims program');
  return Object.freeze(support.map(({ outcome, coefficient }) => {
    const selector = le32(outcome);
    const owner = PublicKey.findProgramAddressSync([CLAIMS_CUSTODY_OWNER_SEED, descriptorId, selector], program)[0];
    const shardMint = PublicKey.findProgramAddressSync([SHARD_MINT_SEED, descriptorId, selector], program)[0];
    const structured = PublicKey.findProgramAddressSync([STRUCTURED_CUSTODY_SEED, descriptorId, selector], program)[0];
    const position = PublicKey.findProgramAddressSync([POSITION_SEED, key(aggregate, 'Claims aggregate').toBytes(), owner.toBytes()], program)[0];
    const admission = PublicKey.findProgramAddressSync([ADMISSION_SEED, key(aggregate, 'Claims aggregate').toBytes(), owner.toBytes()], program)[0];
    return Object.freeze({ outcome, coefficient, owner: owner.toBase58(), shardMint: shardMint.toBase58(), structuredCustody: structured.toBase58(), position: position.toBase58(), admission: admission.toBase58() });
  }));
}

export function encodeRationalRetireReceiptFamilyV4(input: Readonly<{
  releaseSet: Uint8Array; market: string; graphId: Uint8Array; descriptorId: Uint8Array;
  representationAuthority: string; receiptMint: string; rentCredit: string; rentProgram: string;
  generation: bigint; claimsRevision: bigint; receiptLamports: bigint; receiptRent: bigint;
  outcomeCount: number; rentBefore: bigint;
}>): Uint8Array {
  if (input.outcomeCount === 0 || input.generation === 0n) throw new Error('compact receipt retirement has a zero representation width or Market generation');
  const distinct = [key(input.market, 'Market'), key(input.representationAuthority, 'representation authority'),
    key(input.receiptMint, 'receipt Mint'), key(TOKEN_2022_PROGRAM_ID, 'Token-2022 program'),
    key(input.rentCredit, 'RentCredit'), key(input.rentProgram, 'Rent program')]
    .map((address) => hex(address.toBytes()));
  distinct.push(hex(input.descriptorId));
  if (new Set(distinct).size !== distinct.length) throw new Error('compact receipt retirement aliases two lifecycle identities');
  for (const [field, value] of [['release set', input.releaseSet], ['graph', input.graphId], ['descriptor', input.descriptorId]] as const) {
    if (value.length !== 32) throw new Error(`${field} is not one exact identity`);
    requireNonzero(value, field);
  }
  const after = input.rentBefore + input.receiptLamports;
  if (after > MAX_U64 || input.receiptLamports < input.receiptRent || input.receiptRent === 0n) throw new Error('receipt retirement rent accounting does not balance');
  const output = new Uint8Array(RATIONAL_LIFECYCLE_COMPACT_REQUEST_BYTES_V4);
  output.set(Lifecycle.RATIONAL_LIFECYCLE_COMPACT_HOT_MAGIC_V4, Lifecycle.LIFECYCLE_MAGIC_OFFSET);
  putU16(output, Lifecycle.LIFECYCLE_VERSION_OFFSET, Lifecycle.RATIONAL_LIFECYCLE_COMPACT_HOT_VERSION_V4);
  output[Lifecycle.LIFECYCLE_ACTION_OFFSET] = Lifecycle.LIFECYCLE_ACTION_RETIRE_RECEIPT_V2;
  output.set(input.releaseSet, Lifecycle.LIFECYCLE_RELEASE_SET_OFFSET);
  output.set(key(input.market, 'Market').toBytes(), Lifecycle.LIFECYCLE_MARKET_OFFSET);
  output.set(input.graphId, Lifecycle.LIFECYCLE_GRAPH_ID_OFFSET);
  output.set(input.descriptorId, Lifecycle.LIFECYCLE_DESCRIPTOR_ID_OFFSET);
  output.set(key(input.representationAuthority, 'representation authority').toBytes(), Lifecycle.LIFECYCLE_REPRESENTATION_AUTHORITY_OFFSET);
  output.set(key(input.receiptMint, 'receipt Mint').toBytes(), Lifecycle.LIFECYCLE_RECEIPT_MINT_OFFSET);
  output.set(key(TOKEN_2022_PROGRAM_ID, 'Token-2022 program').toBytes(), Lifecycle.LIFECYCLE_TOKEN_PROGRAM_OFFSET);
  output.set(key(input.rentCredit, 'RentCredit').toBytes(), Lifecycle.LIFECYCLE_RENT_CREDIT_OFFSET);
  output.set(key(input.rentProgram, 'Rent program').toBytes(), Lifecycle.LIFECYCLE_RENT_PROGRAM_OFFSET);
  putU64(output, Lifecycle.LIFECYCLE_GENERATION_OFFSET, input.generation);
  putU64(output, Lifecycle.LIFECYCLE_EXPECTED_MARKET_REVISION_OFFSET, input.claimsRevision);
  putU64(output, Lifecycle.LIFECYCLE_OBSERVED_RECEIPT_LAMPORTS_OFFSET, input.receiptLamports);
  putU64(output, Lifecycle.LIFECYCLE_RECEIPT_RENT_PRINCIPAL_OFFSET, input.receiptRent);
  putU64(output, Lifecycle.LIFECYCLE_EXPECTED_RECEIPT_SUPPLY_OFFSET, 0n);
  putU32(output, Lifecycle.LIFECYCLE_OUTCOME_COUNT_OFFSET, input.outcomeCount);
  putU32(output, Lifecycle.LIFECYCLE_COORDINATE_COUNT_OFFSET, 0);
  putU64(output, Lifecycle.LIFECYCLE_RENT_CREDIT_BEFORE_OFFSET, input.rentBefore);
  putU64(output, Lifecycle.LIFECYCLE_RENT_CREDIT_AFTER_OFFSET, after);
  return output;
}

export async function deriveRationalRetireReceiptChildDigestV4(family: Uint8Array, support: ReadonlyArray<CompactSupportRowV4>): Promise<Uint8Array> {
  if (family.length !== RATIONAL_LIFECYCLE_COMPACT_REQUEST_BYTES_V4
      || !same(slice(family, Lifecycle.LIFECYCLE_MAGIC_OFFSET, Lifecycle.LIFECYCLE_MAGIC_BYTES), Lifecycle.RATIONAL_LIFECYCLE_COMPACT_HOT_MAGIC_V4)
      || support.length === 0) {
    throw new Error('compact family or descriptor support has the wrong exact width');
  }
  let prior = -1;
  const identities: string[] = [];
  for (const row of support) {
    if (!Number.isSafeInteger(row.outcome) || row.outcome <= prior || row.coefficient <= 0n || row.coefficient > MAX_U64) {
      throw new Error('descriptor support is empty, unordered, or outside exact scalar bounds');
    }
    prior = row.outcome;
    identities.push(row.owner, row.shardMint, row.structuredCustody, row.position, row.admission);
  }
  identities.forEach((address, index) => key(address, `support identity ${index}`));
  if (new Set(identities).size !== identities.length) throw new Error('compact support aliases two physical identities');
  const familyDigest = await sha256(family);
  const child = new Uint8Array(RATIONAL_LIFECYCLE_COMPACT_REQUEST_BYTES_V4 + support.length * Lifecycle.LIFECYCLE_COORDINATE_BYTES_V2);
  child.set(family);
  child.set(Lifecycle.LIFECYCLE_REQUEST_MAGIC_V2, Lifecycle.LIFECYCLE_MAGIC_OFFSET);
  putU16(child, Lifecycle.LIFECYCLE_VERSION_OFFSET, Lifecycle.LIFECYCLE_VERSION_V2);
  child.set(familyDigest, Lifecycle.LIFECYCLE_PARENT_CONTEXT_OFFSET);
  putU32(child, Lifecycle.LIFECYCLE_COORDINATE_COUNT_OFFSET, support.length);
  support.forEach((row, index) => {
    const offset = RATIONAL_LIFECYCLE_COMPACT_REQUEST_BYTES_V4 + index * Lifecycle.LIFECYCLE_COORDINATE_BYTES_V2;
    putU32(child, offset + Lifecycle.LIFECYCLE_ROW_OUTCOME_OFFSET, row.outcome);
    putU64(child, offset + Lifecycle.LIFECYCLE_ROW_COEFFICIENT_OFFSET, row.coefficient);
    child.set(key(row.shardMint, 'shard Mint').toBytes(), offset + Lifecycle.LIFECYCLE_ROW_SHARD_MINT_OFFSET);
    child.set(key(row.structuredCustody, 'Structured custody').toBytes(), offset + Lifecycle.LIFECYCLE_ROW_STRUCTURED_CUSTODY_OFFSET);
    child.set(key(row.owner, 'Claims custody owner').toBytes(), offset + Lifecycle.LIFECYCLE_ROW_CUSTODY_OWNER_OFFSET);
    child.set(key(row.position, 'Claims Position').toBytes(), offset + Lifecycle.LIFECYCLE_ROW_CUSTODY_POSITION_OFFSET);
    child.set(key(row.admission, 'Position admission').toBytes(), offset + Lifecycle.LIFECYCLE_ROW_POSITION_ADMISSION_OFFSET);
    putU64(child, offset + Lifecycle.LIFECYCLE_ROW_POSITION_REVISION_OFFSET, Lifecycle.LIFECYCLE_ABSENT_POSITION_REVISION_V2);
  });
  return sha256(child);
}

export function rationalHotFixedMetasV4(addresses: ReadonlyArray<string>): ReadonlyArray<AccountMetaV4> {
  if (addresses.length !== Hot.HOT_FIXED_ACCOUNT_COUNT_V3) throw new Error(`Hot fixed frame must contain exactly ${Hot.HOT_FIXED_ACCOUNT_COUNT_V3} addresses`);
  const canonical = addresses.map((address, index) => key(address, `Hot fixed account ${index}`).toBase58());
  if (new Set(canonical).size !== canonical.length) throw new Error('Hot fixed frame aliases two physical roles');
  return Object.freeze(canonical.map((address, index) => Object.freeze({ address, isSigner: false, isWritable: index === Hot.HOT_ROOT_ACCOUNT_V3 })));
}

export async function authenticateRationalHotActivationV4(
  cache: RpcAccount,
  cacheAddress: string,
  registry: string,
  releaseSet: Uint8Array,
  core: string,
  coreProgramData: string,
  trading: string,
  tradingProgramData: string,
): Promise<Readonly<{
  core: string; coreProgramData: string;
  claims: string; claimsProgramData: string;
  trading: string; tradingProgramData: string;
  custody: string; custodyProgramData: string;
  resolution: string; resolutionProgramData: string;
}>> {
  if (cache.owner !== registry || cache.executable || cache.data.length !== ACTIVATION_CACHE_BYTES || ascii(cache.data, 0, 8) !== ACTIVATION_CACHE_MAGIC_V1 || u16(cache.data, 8) !== 1 || u16(cache.data, 10) !== 1) {
    throw new Error('activation cache has the wrong Registry owner or exact ABI');
  }
  requireZero(cache.data, 12, 4, 'activation cache');
  if (!same(slice(cache.data, 16, 32), releaseSet)) throw new Error('activation cache selects another execution release set');
  const expectedCache = PublicKey.findProgramAddressSync([ACTIVATION_SEED, releaseSet], key(registry, 'Registry program'))[0].toBase58();
  if (expectedCache !== cacheAddress) throw new Error('activation cache is not the release-derived Registry PDA');
  const releaseBytes = new Uint8Array(336); releaseBytes.set(new TextEncoder().encode(EXECUTION_RELEASE_SET_MAGIC_V1), 0);
  putU16(releaseBytes, 8, 1); putU16(releaseBytes, 10, 1);
  const artifacts = [];
  for (let role = 0; role < 5; role += 1) {
    const offset = 48 + role * (32 + ARTIFACT_RELEASE_BYTES);
    const artifactId = slice(cache.data, offset, 32); const artifactBytes = slice(cache.data, offset + 32, ARTIFACT_RELEASE_BYTES);
    if (!same(await sha256(artifactBytes), artifactId)) throw new Error(`activation cache role ${role} artifact differs from its content identity`);
    const artifact = decodeArtifactReleaseV1(artifactBytes); artifacts.push(artifact);
    releaseBytes.set(key(artifact.program, `activated role ${role}`).toBytes(), 16 + role * 64);
    releaseBytes.set(artifactId, 48 + role * 64);
  }
  const decodedRelease = await decodeExecutionReleaseSetV1(releaseBytes);
  if (decodedRelease.id !== hex(releaseSet)) throw new Error('activation cache does not reconstruct the Core-selected release set');
  if (artifacts[EXECUTION_ROLE_CORE_V1].program !== core || artifacts[EXECUTION_ROLE_CORE_V1].programData !== coreProgramData
      || artifacts[EXECUTION_ROLE_TRADING_V1].program !== trading || artifacts[EXECUTION_ROLE_TRADING_V1].programData !== tradingProgramData) {
    throw new Error('activation cache Core/Trading deployments differ from Hot fixed programs');
  }
  return Object.freeze({
    core: artifacts[EXECUTION_ROLE_CORE_V1].program, coreProgramData: artifacts[EXECUTION_ROLE_CORE_V1].programData,
    claims: artifacts[EXECUTION_ROLE_CLAIMS_V1].program, claimsProgramData: artifacts[EXECUTION_ROLE_CLAIMS_V1].programData,
    trading: artifacts[EXECUTION_ROLE_TRADING_V1].program, tradingProgramData: artifacts[EXECUTION_ROLE_TRADING_V1].programData,
    custody: artifacts[EXECUTION_ROLE_CUSTODY_V1].program, custodyProgramData: artifacts[EXECUTION_ROLE_CUSTODY_V1].programData,
    resolution: artifacts[EXECUTION_ROLE_RESOLUTION_V1].program, resolutionProgramData: artifacts[EXECUTION_ROLE_RESOLUTION_V1].programData,
  });
}

async function authenticateSelectedArtifacts(
  client: RetireRpc,
  accounts: ReadonlyMap<string, RpcAccount | null>,
  fixed: ReadonlyArray<AccountMetaV4>,
  registry: string,
  selection: RootSelection,
): Promise<Readonly<{ descriptor: RationalRepresentationDescriptorViewV3; capability: RationalHotCapabilityV4 }>> {
  const manifestRaw = await authenticateFinalizedRationalHotRecordV4(client, accounts, registry, fixed[2].address, fixed[3].address,
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, selection.manifest, 'capability manifest');
  const manifest = selectRationalHotManifestEntryV4(manifestRaw.data, selection);
  const set = await authenticateFinalizedRationalHotRecordV4(client, accounts, registry, fixed[4].address, fixed[5].address,
    Uint8Array.from([0x37,0xdf,0x09,0xe7,0xde,0xeb,0xdd,0x0a,0xd0,0xd1,0x25,0x13,0xa7,0x8d,0xd4,0x4c,0x97,0x24,0x30,0x37,0x99,0xb7,0x54,0x4d,0xc9,0x1b,0x3b,0x6a,0x2e,0x6d,0x62,0x96]),
    selection.programSet, 'CapabilityProgramSetV2');
  const selected = selectRationalHotCapabilityV4(set.data, 3);
  const capabilityRaw = await authenticateFinalizedRationalHotRecordV4(client, accounts, registry, fixed[6].address, fixed[7].address, selected.schema, selected.digest, 'compact CapabilityProgramV4');
  const capability = decodeRationalHotCapabilityV4(capabilityRaw.data);
  if (!same(capability.kind, selection.kind) || !same(capability.configSchema, RATIONAL_REPRESENTATION_DESCRIPTOR_SCHEMA_V3)
      || !same(capability.requestSchema, RATIONAL_LIFECYCLE_COMPACT_REQUEST_SCHEMA_V4)) {
    throw new Error('CapabilityProgramV4 does not select the persisted kind, Rational descriptor, and compact V4 request schema');
  }
  if (!same(capability.capacity, manifest.capacity) || !same(capability.rootSchema, manifest.rootSchema)
      || !same(capability.derivation, manifest.derivation)) {
    throw new Error('CapabilityProgramV4 capacity, root schema, or lifecycle differs from the selected manifest entry');
  }
  const descriptorRaw = await authenticateFinalizedRationalHotRecordV4(client, accounts, registry, fixed[8].address, fixed[9].address,
    capability.configSchema, selection.config, 'Rational representation descriptor');
  const descriptor = decodeRationalRepresentationDescriptorV3(descriptorRaw.data, selection.config);
  const rawIndexes = [10, 12, 18, 20, 14, 16];
  for (let index = 0; index < capability.artifacts.length; index += 1) {
    const raw = rawIndexes[index]; const artifact = capability.artifacts[index];
    await authenticateFinalizedRationalHotRecordV4(client, accounts, registry, fixed[raw].address, fixed[raw + 1].address, artifact.schema, artifact.digest, `compact artifact ${index}`);
  }
  return Object.freeze({ descriptor, capability });
}

export async function inspectRationalRetireReceiptV4(
  client: RetireRpc,
  input: Readonly<{ payer: string; fixedAccounts: ReadonlyArray<string>; lookupTable: string }>,
): Promise<RationalRetireReceiptInspectionV4> {
  const payer = key(input.payer, 'payer').toBase58();
  const fixed = rationalHotFixedMetasV4(input.fixedAccounts);
  const first = await acquireRationalHotAccountsV4(client, [...fixed.map((meta) => meta.address), payer, input.lookupTable]);
  const marketAddress = fixed[Hot.HOT_MARKET_ACCOUNT_V3].address;
  const coreProgram = fixed[Hot.HOT_CORE_PROGRAM_ACCOUNT_V3].address;
  const tradingProgram = fixed[Hot.HOT_TRADING_PROGRAM_ACCOUNT_V3].address;
  const registry = fixed[Hot.HOT_REGISTRY_PROGRAM_ACCOUNT_V3].address;
  const market = authenticateRationalHotCoreV3(marketAddress, required(first.accounts, marketAddress, 'Market'), coreProgram);
  if (market.phase !== 'Retiring' || market.readiness !== 'Consumed' || isZero(market.terminalReceipt)) {
    throw new Error('receipt retirement requires a Retiring, readiness-consumed Core Market with terminal receipt');
  }
  if (market.registry !== registry) throw new Error('current Core Market selects another Registry program');
  if (fixed[Hot.HOT_RENT_SYSVAR_ACCOUNT_V3].address !== RENT_SYSVAR_ID || fixed[Hot.HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3].address !== SYSTEM_INSTRUCTIONS_SYSVAR) {
    throw new Error('Hot fixed frame substitutes a runtime sysvar');
  }
  const rootAccount = required(first.accounts, fixed[Hot.HOT_ROOT_ACCOUNT_V3].address, 'capability root');
  if (rootAccount.owner !== tradingProgram || rootAccount.executable) throw new Error('capability root is not Trading-owned nonexecutable state');
  const selection = decodeRationalHotRootV4(rootAccount.data, market.releaseSet, marketAddress, market.generation);
  if (!same(selection.manifest, market.manifest)) throw new Error('capability root and Core select different manifests');
  const activation = await authenticateRationalHotActivationV4(
    required(first.accounts, fixed[Hot.HOT_ACTIVATION_CACHE_ACCOUNT_V3].address, 'activation cache'),
    fixed[Hot.HOT_ACTIVATION_CACHE_ACCOUNT_V3].address, registry, market.releaseSet,
    coreProgram, fixed[Hot.HOT_CORE_PROGRAMDATA_ACCOUNT_V3].address,
    tradingProgram, fixed[Hot.HOT_TRADING_PROGRAMDATA_ACCOUNT_V3].address,
  );
  const selected = await authenticateSelectedArtifacts(client, first.accounts, fixed, registry, selection);
  const descriptor = selected.descriptor;
  if (descriptor.market !== marketAddress || !same(descriptor.releaseSet, market.releaseSet) || descriptor.tokenProgram !== TOKEN_2022_PROGRAM_ID || descriptor.outcomeCount === 0) {
    throw new Error('representation descriptor differs from the Market lifecycle or Token-2022 successor');
  }
  if (rootAccount.data.length !== Hot.CAPABILITY_ROOT_HEADER_BYTES_V1 + selected.capability.rootStateBytes) {
    throw new Error('capability root width differs from the selected CapabilityProgramV4 capacity');
  }
  const authority = PublicKey.findProgramAddressSync([REPRESENTATION_AUTHORITY_SEED, descriptor.id], key(activation.claims, 'Claims program'))[0];
  const receipt = PublicKey.findProgramAddressSync([RECEIPT_MINT_SEED, descriptor.graphDigest, key(marketAddress, 'Market').toBytes(), market.releaseSet], key(activation.claims, 'Claims program'))[0];
  if (receipt.toBase58() !== descriptor.receiptMint) throw new Error('descriptor receipt Mint is not the graph+Market+release Claims PDA');
  const aggregate = PublicKey.findProgramAddressSync([CLAIMS_MARKET_SEED, key(marketAddress, 'Market').toBytes()], key(activation.claims, 'Claims program'))[0].toBase58();
  const support = deriveRationalRetireReceiptSupportV4(activation.claims, descriptor.id, descriptor.support, aggregate);
  const dynamic = [activation.claims, activation.claimsProgramData, aggregate, market.rentBeneficiary, descriptor.receiptMint,
    TOKEN_2022_PROGRAM_ID, SYSTEM_PROGRAM_ID,
    ...support.flatMap((row) => [row.shardMint, row.structuredCustody, row.position, row.admission])];
  const second = await acquireRationalHotAccountsV4(client, dynamic, first.slot);
  const accounts = new Map([...first.accounts, ...second.accounts]);
  const payerAccount = required(accounts, payer, 'payer');
  if (payerAccount.owner !== SYSTEM_PROGRAM_ID || payerAccount.executable || payerAccount.data.length !== 0) throw new Error('payer is not a System-owned data-free wallet');
  const claimsProgram = required(accounts, activation.claims, 'Claims program');
  const claimsProgramData = required(accounts, activation.claimsProgramData, 'Claims ProgramData');
  if (!claimsProgram.executable || claimsProgram.owner !== UPGRADEABLE_LOADER_ID || claimsProgram.data.length !== 36 || u32(claimsProgram.data, 0) !== 2
      || new PublicKey(slice(claimsProgram.data, 4, 32)).toBase58() !== activation.claimsProgramData
      || claimsProgramData.owner !== UPGRADEABLE_LOADER_ID || claimsProgramData.executable) {
    throw new Error('activated Claims program and ProgramData do not form one exact Loader-v3 deployment');
  }
  const productRaw = await authenticateFinalizedRationalHotRecordV4(client, accounts, registry, fixed[30].address, fixed[31].address, PRODUCT_RECORD_SCHEMA_ID_V2, market.productRecord, 'Product Runtime V2 root');
  const domainDigest = slice(productRaw.data, 48, 32); const portfolioDigest = slice(productRaw.data, 80, 32);
  const domainRaw = await authenticateFinalizedRationalHotRecordV4(client, accounts, registry, fixed[32].address, fixed[33].address, RESULT_DOMAIN_SCHEMA_ID_V2, domainDigest, 'Product result domain');
  const portfolioRaw = await authenticateFinalizedRationalHotRecordV4(client, accounts, registry, fixed[34].address, fixed[35].address, PORTFOLIO_SCHEMA_ID_V2, portfolioDigest, 'Product portfolio');
  const product = decodeCoreFoundProductGraphV2(productRaw.data, domainRaw.data, portfolioRaw.data, domainDigest, portfolioDigest);
  if (!same(product.productId, market.productId)) throw new Error('Product identity differs from the Core Market');
  const admittedBasis = await authenticateRationalProductBasisRecordV3(client, accounts, {
    registry,
    rawAddress: fixed[Hot.HOT_LINKED_BASIS_RAW_ACCOUNT_V3].address,
    stagingAddress: fixed[Hot.HOT_LINKED_BASIS_STAGING_ACCOUNT_V3].address,
    productId: product.productId,
    domainDigest,
    domainBytes: domainRaw.data,
    representationWidth: descriptor.outcomeCount,
  });
  const aggregateAccount = required(accounts, aggregate, 'Claims aggregate');
  if (aggregateAccount.owner !== activation.claims || aggregateAccount.executable) throw new Error('Claims aggregate has the wrong owner or executable bit');
  const claimsRevision = decodeClaimsMarket(aggregateAccount.data, marketAddress, market.releaseSet, registry, market.productRecord, market.realm, market.generation, admittedBasis.basis.width);
  const creditAccount = required(accounts, market.rentBeneficiary, 'LifecycleRentCreditV2');
  const credit = decodeLifecycleRentCredit(market.rentBeneficiary, creditAccount, marketAddress, market.releaseSet, market.generation);
  const creditRent = BigInt((await client.minimumBalanceForRentExemption(LIFECYCLE_RENT_CREDIT_BYTES_V2)).lamports);
  if (credit.balance < creditRent) throw new Error('LifecycleRentCreditV2 is below its current exact rent minimum');
  const rentObservation = await acquireRationalHotAccountsV4(client, [credit.program], second.slot);
  const rentProgramAccount = required(rentObservation.accounts, credit.program, 'Rent program');
  if (!rentProgramAccount.executable) throw new Error('LifecycleRentCreditV2 owner is not an executable Rent program');
  const tokenProgramAccount = required(accounts, TOKEN_2022_PROGRAM_ID, 'Token-2022 program');
  const systemProgramAccount = required(accounts, SYSTEM_PROGRAM_ID, 'System program');
  if (!tokenProgramAccount.executable || !systemProgramAccount.executable || systemProgramAccount.owner !== NATIVE_LOADER_ID) {
    throw new Error('Token-2022 or System program is not executable runtime code');
  }
  const mintAccount = required(accounts, descriptor.receiptMint, 'receipt Mint');
  const mint = decodeToken2022BehaviorMintV2(descriptor.receiptMint, mintAccount);
  if (mint.controller !== authority.toBase58() || mint.rawSupply !== 0n) throw new Error('receipt Mint controller differs from the descriptor authority or its supply is nonzero');
  const receiptRent = BigInt((await client.minimumBalanceForRentExemption(mintAccount.data.length)).lamports);
  for (const row of support) {
    for (const [address, field] of [[row.shardMint, 'shard Mint'], [row.structuredCustody, 'Structured custody'], [row.position, 'Claims Position'], [row.admission, 'Position admission']] as const) {
      vacant(accounts.get(address), `${field} outcome ${row.outcome}`);
    }
  }
  const familyBytes = encodeRationalRetireReceiptFamilyV4({
    releaseSet: market.releaseSet, market: marketAddress, graphId: descriptor.graphId, descriptorId: descriptor.id,
    representationAuthority: authority.toBase58(), receiptMint: descriptor.receiptMint, rentCredit: market.rentBeneficiary,
    rentProgram: credit.program, generation: market.generation, claimsRevision,
    receiptLamports: BigInt(mintAccount.lamports), receiptRent, outcomeCount: admittedBasis.basis.width, rentBefore: credit.balance,
  });
  const familyDigest = await sha256(familyBytes);
  const exactChildDigest = await deriveRationalRetireReceiptChildDigestV4(familyBytes, support);
  const caller = PublicKey.findProgramAddressSync([
    CALLER_AUTHORITY_SEED, market.releaseSet, key(marketAddress, 'Market').toBytes(), Uint8Array.of(2), familyDigest, exactChildDigest,
  ], key(tradingProgram, 'Trading program'))[0].toBase58();
  const claimsAccounts: AccountMetaV4[] = [
    { address: caller, isSigner: false, isWritable: false },
    { address: tradingProgram, isSigner: false, isWritable: false },
    { address: fixed[26].address, isSigner: false, isWritable: false },
    { address: activation.claims, isSigner: false, isWritable: false },
    { address: activation.claimsProgramData, isSigner: false, isWritable: false },
    { address: registry, isSigner: false, isWritable: false },
    { address: fixed[22].address, isSigner: false, isWritable: false },
    { address: RENT_SYSVAR_ID, isSigner: false, isWritable: false },
    { address: SYSTEM_PROGRAM_ID, isSigner: false, isWritable: false },
    { address: fixed[8].address, isSigner: false, isWritable: false },
    { address: fixed[9].address, isSigner: false, isWritable: false },
    { address: authority.toBase58(), isSigner: false, isWritable: false },
    { address: descriptor.receiptMint, isSigner: false, isWritable: true },
    { address: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
    { address: market.rentBeneficiary, isSigner: false, isWritable: true },
    { address: credit.program, isSigner: false, isWritable: false },
    { address: aggregate, isSigner: false, isWritable: false },
    { address: marketAddress, isSigner: false, isWritable: false },
    { address: coreProgram, isSigner: false, isWritable: false },
    { address: fixed[24].address, isSigner: false, isWritable: false },
    ...support.flatMap((row) => [
      // The exact vacancy group, in the physical order the contract reads:
      // shard Mint, Structured custody, Claims custody OWNER, Position, admission.
      { address: row.shardMint, isSigner: false, isWritable: false },
      { address: row.structuredCustody, isSigner: false, isWritable: false },
      { address: row.owner, isSigner: false, isWritable: false },
      { address: row.position, isSigner: false, isWritable: false },
      { address: row.admission, isSigner: false, isWritable: false },
    ]),
  ];
  const lookupTable = decodeRationalHotLookupTableV4(input.lookupTable, required(accounts, input.lookupTable, 'address lookup table'));
  return Object.freeze({
    observedSlot: rentObservation.slot, payer, fixedAccounts: fixed, claimsAccounts: Object.freeze(claimsAccounts), support,
    lookupTable, market: marketAddress, generation: market.generation, releaseSet: market.releaseSet,
    descriptorId: descriptor.id, graphId: descriptor.graphId, representationAuthority: authority.toBase58(),
    receiptMint: descriptor.receiptMint, claimsProgram: activation.claims, claimsRevision,
    representationWidth: admittedBasis.basis.width, resultOutcomeCount: product.outcomeCount,
    rentCredit: market.rentBeneficiary, rentProgram: credit.program,
    receiptLamports: BigInt(mintAccount.lamports), receiptRentPrincipal: receiptRent, rentCreditBefore: credit.balance,
    familyBytes, familyDigest, childDigest: exactChildDigest, rootDigest: await sha256(rootAccount.data), callerAuthority: caller,
    // This path reaches here only after the selected CapabilityProgramV4,
    // every selected artifact, the active Trading program, and the exact ALT
    // have all been reacquired at one finalized floor. The generic Hot
    // interpreter admits EffectV4 from that descriptor; there is no browser
    // override or fallback route.
    executionStatus: 'ready',
  });
}

export function buildRationalRetireReceiptCandidateV4(
  inspection: RationalRetireReceiptInspectionV4,
  recentBlockhash: string,
): RationalRetireReceiptCandidateV4 {
  key(recentBlockhash, 'recent blockhash');
  if (inspection.fixedAccounts.length !== Hot.HOT_FIXED_ACCOUNT_COUNT_V3
      || inspection.support.length === 0
      || inspection.claimsAccounts.length !== RATIONAL_LIFECYCLE_CLAIMS_COMMON_ACCOUNTS_V2
        + RATIONAL_LIFECYCLE_VACANCY_ACCOUNTS_V2 * inspection.support.length
      || inspection.familyBytes.length !== RATIONAL_LIFECYCLE_COMPACT_REQUEST_BYTES_V4
      || !same(slice(inspection.familyBytes, Lifecycle.LIFECYCLE_MAGIC_OFFSET, Lifecycle.LIFECYCLE_MAGIC_BYTES), Lifecycle.RATIONAL_LIFECYCLE_COMPACT_HOT_MAGIC_V4)
      || inspection.rootDigest.length !== 32 || isZero(inspection.rootDigest)) {
    throw new Error('compact RetireReceipt inspection has inconsistent fixed, support, Claims, or request geometry');
  }
  const outer = new Uint8Array(RATIONAL_LIFECYCLE_COMPACT_OUTER_BYTES_V4);
  outer.set(Hot.HOT_EXECUTION_MAGIC_V3, 0); putU16(outer, 8, 3); putU16(outer, 10, 1);
  putU32(outer, 12, RATIONAL_LIFECYCLE_COMPACT_REQUEST_BYTES_V4); outer.set(inspection.releaseSet, 16);
  outer.set(key(inspection.market, 'Market').toBytes(), 48); putU64(outer, 80, inspection.generation);
  outer.set(inspection.rootDigest, 88); outer.set(inspection.familyBytes, 128);
  const keys = [...inspection.fixedAccounts, ...inspection.claimsAccounts].map((meta) => ({
    pubkey: key(meta.address, 'compact retirement account'), isSigner: meta.isSigner, isWritable: meta.isWritable,
  }));
  const instruction = new TransactionInstruction({
    programId: key(inspection.fixedAccounts[Hot.HOT_TRADING_PROGRAM_ACCOUNT_V3]?.address ?? '', 'Trading program'),
    keys, data: outer as Buffer,
  });
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: key(inspection.payer, 'payer'), recentBlockhash, instructions: [instruction],
  }).compileToV0Message([inspection.lookupTable]));
  const wireBytes = transaction.serialize();
  if (wireBytes.length > SOLANA_PACKET_BYTES_V1) throw new Error(`compact RetireReceipt is ${wireBytes.length} bytes, above the ${SOLANA_PACKET_BYTES_V1}-byte packet bound`);
  const requiredSigners = Object.freeze(transaction.message.staticAccountKeys.slice(0, transaction.message.header.numRequiredSignatures).map((value) => value.toBase58()));
  if (requiredSigners.length !== 1 || requiredSigners[0] !== inspection.payer) throw new Error('compact RetireReceipt has an unexpected wallet signer set');
  const loadedAddresses = transaction.message.addressTableLookups.reduce((total, lookup) => total + lookup.readonlyIndexes.length + lookup.writableIndexes.length, 0);
  if (loadedAddresses === 0) throw new Error('selected ALT did not contribute to compact RetireReceipt');
  const accountLocks = transaction.message.staticAccountKeys.length + loadedAddresses;
  if (accountLocks > MAX_TX_ACCOUNT_LOCKS_V2) {
    throw new Error(`compact RetireReceipt needs ${accountLocks} unique account locks, above devnet's ${MAX_TX_ACCOUNT_LOCKS_V2}-lock limit`);
  }
  return Object.freeze({
    transaction, instruction, familyBytes: inspection.familyBytes, outerBytes: outer, wireBytes,
    requiredSigners, loadedAddresses, accountLocks, accountCount: keys.length, supportCount: inspection.support.length,
    executionStatus: 'ready',
  });
}

export function compactRetireReceiptSummaryV4(inspection: RationalRetireReceiptInspectionV4): Readonly<{
  descriptorId: string; familyDigest: string; childDigest: string; frame: string;
}> {
  return Object.freeze({
    descriptorId: hex(inspection.descriptorId), familyDigest: hex(inspection.familyDigest), childDigest: hex(inspection.childDigest),
    frame: `${RATIONAL_LIFECYCLE_CLAIMS_COMMON_ACCOUNTS_V2}+${RATIONAL_LIFECYCLE_VACANCY_ACCOUNTS_V2}×${inspection.support.length}`,
  });
}
