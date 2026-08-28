import {
  AddressLookupTableAccount,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { boundedInstructionsV1 } from './founding/computeBudget';
import { routableAddressesV1 } from './founding/lookupTable';
import { SOLANA_PACKET_BYTES_V1 } from './solanaLimits';
import {
  CAPABILITY_ENTRY_ACTIVATION_DEADLINE_OFFSET_V1,
  CAPABILITY_ENTRY_ACTIVATION_POLICY_OFFSET_V1,
  CAPABILITY_ENTRY_BYTES_V1,
  CAPABILITY_ENTRY_CAPACITY_PROFILE_ID_OFFSET_V1,
  CAPABILITY_ENTRY_CHILD_DERIVATION_ID_OFFSET_V1,
  CAPABILITY_ENTRY_CHILD_SCHEMA_ID_OFFSET_V1,
  CAPABILITY_ENTRY_CONFIG_ID_OFFSET_V1,
  CAPABILITY_ENTRY_DEPENDENCIES_OFFSET_V1,
  CAPABILITY_ENTRY_DEPENDENCY_COUNT_OFFSET_V1,
  CAPABILITY_ENTRY_KIND_ID_OFFSET_V1,
  CAPABILITY_ENTRY_QUOTE_OFFSET_V1,
  CAPABILITY_ENTRY_RELEASE_ID_OFFSET_V1,
  CAPABILITY_ENTRY_RESERVED_BYTES_V1,
  CAPABILITY_ENTRY_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_ALLOCATION_AMOUNT_OFFSET_V1,
  CAPABILITY_FUNDING_ALLOCATION_CLASS_OFFSET_V1,
  CAPABILITY_FUNDING_ALLOCATION_RESERVED_BYTES_V1,
  CAPABILITY_FUNDING_ALLOCATION_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_AMOUNTS_NATIVE_TOTAL_OFFSET_V1,
  CAPABILITY_FUNDING_AMOUNTS_REALM_TOTAL_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_BENEFICIARY_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_BYTES_V1,
  CAPABILITY_FUNDING_BINDING_MINT_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_REALM_ID_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_RELEASE_ID_OFFSET_V1,
  CAPABILITY_FUNDING_BINDING_TOKEN_PROGRAM_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_AMOUNTS_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_BINDING_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_BYTES_V1,
  CAPABILITY_FUNDING_QUOTE_COLLATERAL_KIND_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_MAGIC_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_MAGIC_V1,
  CAPABILITY_FUNDING_QUOTE_RESERVED_BYTES_V1,
  CAPABILITY_FUNDING_QUOTE_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_SCHEMA_OFFSET_V1,
  CAPABILITY_FUNDING_QUOTE_SCHEMA_VERSION_V1,
  CAPABILITY_MANIFEST_ARTIFACT_PROFILE_V1,
  CAPABILITY_MANIFEST_COUNT_OFFSET_V1,
  CAPABILITY_MANIFEST_HEADER_BYTES_V1,
  CAPABILITY_MANIFEST_HEADER_RESERVED_BYTES_V1,
  CAPABILITY_MANIFEST_MAGIC_OFFSET_V1,
  CAPABILITY_MANIFEST_MAGIC_V1,
  CAPABILITY_MANIFEST_PROFILE_OFFSET_V1,
  CAPABILITY_MANIFEST_RESERVED_OFFSET_V1,
  CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1,
  CAPABILITY_MANIFEST_SCHEMA_VERSION_V1,
  FUNDING_COMPARTMENTS_V1,
  MAX_CAPABILITIES_V1,
  MAX_DEPENDENCIES_PER_CAPABILITY_V1,
} from './generated/capabilityManifestV1';
import {
  REALM_ADAPTER_RELEASE_ID_OFFSET_V1,
  REALM_BYTES_V1,
  REALM_COLLATERAL_MINT_OFFSET_V1,
  REALM_FREEZE_AUTHORITY_POLICY_OFFSET_V1,
  REALM_MAGIC_OFFSET_V1,
  REALM_MAGIC_V1,
  REALM_MINT_AUTHORITY_POLICY_OFFSET_V1,
  REALM_RESERVED_BYTES_V1,
  REALM_RESERVED_OFFSET_V1,
  REALM_SCHEMA_VERSION_OFFSET_V1,
  REALM_SCHEMA_VERSION_V1,
  REALM_TOKEN_PROGRAM_OFFSET_V1,
} from './generated/realmPositionV1';
import {
  CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
  ARTIFACT_RELEASE_SCHEMA_ID_V1,
  GRADED_BASIS_RECORD_SCHEMA_ID_V3,
  MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
  SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
  SOURCE_SPEC_SCHEMA_ID_V1,
  CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2,
  CORE_ACTION_FOUND_TAG,
  CORE_FOUND_ACCOUNT_COUNT_V3,
  CORE_FOUND_ACCOUNT_ROLES_V3,
  CORE_REQUEST_BYTES,
  CORE_REQUEST_MAGIC,
  CORE_STATE_BYTES,
  CORE_VERSION,
  LIFECYCLE_RENT_ACTION_CREATE_V2,
  LIFECYCLE_RENT_CREDIT_BYTES_V2,
  LIFECYCLE_RENT_CREDIT_MAGIC_OFFSET_V2,
  LIFECYCLE_RENT_CREDIT_MAGIC_V2,
  LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
  LIFECYCLE_RENT_INSTRUCTION_ACTION_OFFSET_V2,
  LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2,
  LIFECYCLE_RENT_SCHEMA_VERSION_V2,
  MARKET_CORE_STATE_PDA_DOMAIN_V2,
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  REALM_SCHEMA_RELEASE_ID_V1,
  RESULT_DOMAIN_SCHEMA_ID_V2,
  SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
} from './generated/coreFound';
import { inspectProtocolInfrastructureV1, type ProtocolInfrastructureInspectionV1 } from './infrastructure';
import {
  NATIVE_LOADER_ID,
  RENT_SYSVAR_ID,
  SYSTEM_PROGRAM_ID,
  SYSVAR_OWNER_ID,
  deriveFinalizedRecordAddressesV1,
} from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';
import { ascii, hex, isZero, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';

const PRODUCT_RECORD_BYTES = 112;
const DOMAIN_HEADER_BYTES = 240;
const PORTFOLIO_HEADER_BYTES = 208;
const SOURCE_MATERIAL_BYTES = 240;
const MAX_U32 = 0xffff_ffff;

export type CoreFoundInputV2 = Readonly<{
  payer: string;
  registryProgram: string;
  activationCache: string;
  refundWallet: string;
  realmRecord: string;
  productRecord: string;
  resultDomainRecord: string;
  portfolioRecord: string;
  linkedBasisRecord: string;
  sourceMaterialRecord: string;
  sourceSpecRecord: string;
  capacityProfileRecord: string;
  manipulationFloorRecord: string;
  capabilityManifestRecord: string;
  generation: bigint;
  /**
   * The finalized routing table Found37 rides, read back off the chain.
   *
   * Absent, `prepareCoreFoundV2` still derives everything and still reports
   * `routableAddresses`, but compiling refuses: with its required ComputeBudget
   * declaration the inline frame is ten bytes over the packet bound. That
   * refusal is the correct answer, and it is how a caller learns it owes a
   * table rather than discovering it at submission.
   */
  lookupTable?: AddressLookupTableAccount;
}>;

export type CoreFoundPlanV2 = Readonly<{
  observedSlot: string;
  market: string;
  rentCredit: string;
  coreProgram: string;
  registryProgram: string;
  rentProgram: string;
  productRecordDigest: string;
  productId: string;
  outcomeCount: number;
  executionReleaseSetId: string;
  infrastructureProfile: string;
  infrastructureRecognition: ProtocolInfrastructureInspectionV1['recognition'];
  marketRentTopUp: string;
  rentCreditRentDebit: string;
  lastValidBlockHeight: string;
  accountAddresses: ReadonlyArray<string>;
  /** Every non-signer key a routing table may carry, in first-seen order. */
  routableAddresses: ReadonlyArray<string>;
  requiredSigners: ReadonlyArray<string>;
  requestBytes: Uint8Array;
  /** `vacant` when this plan must create the credit, `created` when it exists. */
  rentCreditState: 'vacant' | 'created';
  rentCreateRequestBytes: Uint8Array | null;
  rentCreateTransaction: VersionedTransaction | null;
  rentCreateWireBytes: Uint8Array | null;
  /**
   * The compiled Found37 packet, or null when it could not be compiled.
   *
   * Null is not a failure of the derivation: everything above it -- the Market
   * address, the 37-account frame, the exact rent debit -- is derived and
   * correct. It means only that this frame does not fit a packet without a
   * routing table, which is a fact about the frame and not about the caller.
   * `foundRefusal` says so in words.
   */
  transaction: VersionedTransaction | null;
  wireBytes: Uint8Array | null;
  foundRefusal: string | null;
}>;

export type CompiledCoreFoundTransactionV2 = Readonly<{
  requestBytes: Uint8Array;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
}>;

export type CompiledLifecycleRentCreateTransactionV2 = Readonly<{
  rentCredit: string;
  requestBytes: Uint8Array;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
}>;

type RecordAuthority = Readonly<{
  raw: string;
  staging: string;
  schema: Uint8Array;
  digest: Uint8Array;
  bytes: Uint8Array;
  rentMinimum: bigint;
}>;

type ProductGraph = Readonly<{ productId: Uint8Array; outcomeCount: number }>;

function key(value: string, field: string): PublicKey {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return parsed;
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function accountMap(entries: Awaited<ReturnType<SolanaRpcClient['multipleAccounts']>>): ReadonlyMap<string, RpcAccount | null> {
  return new Map(entries.accounts.map((entry) => [entry.address, entry.account]));
}

/** Acquire one finalized observation floor without exceeding the RPC's 32-key bound. */
export async function acquireFinalizedAccountsInChunksV1(
  client: Pick<SolanaRpcClient, 'multipleAccounts'>,
  addresses: ReadonlyArray<string>,
  minimumContextSlot: string,
): Promise<Awaited<ReturnType<SolanaRpcClient['multipleAccounts']>>> {
  if (addresses.length === 0 || new Set(addresses).size !== addresses.length) {
    throw new Error('chunked finalized acquisition requires distinct nonempty addresses');
  }
  const accounts: Array<{ address: string; account: RpcAccount | null }> = [];
  let observedSlot: string | null = null;
  for (let offset = 0; offset < addresses.length; offset += 32) {
    const chunk = addresses.slice(offset, offset + 32);
    const observation = await client.multipleAccounts(chunk, minimumContextSlot);
    if (BigInt(observation.slot) < BigInt(minimumContextSlot)) {
      throw new Error('chunked finalized acquisition regressed below its context floor');
    }
    if (observedSlot !== null && observation.slot !== observedSlot) {
      throw new Error('chunked finalized acquisition returned different context slots');
    }
    observedSlot = observation.slot;
    accounts.push(...observation.accounts);
  }
  if (observedSlot === null) throw new Error('chunked finalized acquisition returned no context');
  return Object.freeze({ slot: observedSlot, accounts: Object.freeze(accounts) });
}

function required(accounts: ReadonlyMap<string, RpcAccount | null>, address: string, field: string): RpcAccount {
  const account = accounts.get(address);
  if (account === undefined || account === null) throw new Error(`${field} ${address} is absent at finalized commitment`);
  return account;
}

function vacant(account: RpcAccount | null | undefined, field: string): void {
  if (account === null || account === undefined) return;
  if (account.owner !== SYSTEM_PROGRAM_ID || account.executable || account.data.length !== 0) {
    throw new Error(`${field} is not a vacant System-owned account`);
  }
}

function u32(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
}

function i128(bytes: Uint8Array, offset: number): bigint {
  const view = new DataView(bytes.buffer, bytes.byteOffset + offset, 16);
  return (view.getBigInt64(8, true) << 64n) + view.getBigUint64(0, true);
}

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function compareBytes(left: Uint8Array, right: Uint8Array): number {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return left[index] < right[index] ? -1 : 1;
  }
  return 0;
}

function gcd(left: bigint, right: bigint): bigint {
  let a = left;
  let b = right;
  while (b !== 0n) [a, b] = [b, a % b];
  return a;
}

async function recordAuthority(
  client: Pick<SolanaRpcClient, 'minimumBalanceForRentExemption'>,
  registry: string,
  raw: string,
  schema: Uint8Array,
  account: RpcAccount,
  field: string,
): Promise<RecordAuthority> {
  if (account.owner !== registry || account.executable) throw new Error(`${field} is not a nonexecutable Registry-owned raw record`);
  const digest = await sha256(account.data);
  const coordinates = deriveFinalizedRecordAddressesV1(registry, schema, digest);
  if (coordinates.record !== raw) throw new Error(`${field} is not the schema/content-derived Registry raw PDA`);
  const rent = await client.minimumBalanceForRentExemption(account.data.length);
  if (BigInt(account.lamports) < BigInt(rent.lamports)) throw new Error(`${field} is below its exact current rent minimum`);
  return Object.freeze({ raw, staging: coordinates.staging, schema, digest, bytes: account.data, rentMinimum: BigInt(rent.lamports) });
}

function validateRealm(bytes: Uint8Array): void {
  if (bytes.length !== REALM_BYTES_V1
      || ascii(bytes, REALM_MAGIC_OFFSET_V1, 8) !== REALM_MAGIC_V1
      || u16(bytes, REALM_SCHEMA_VERSION_OFFSET_V1) !== REALM_SCHEMA_VERSION_V1) throw new Error('Realm record has the wrong exact ABI');
  if (bytes[REALM_MINT_AUTHORITY_POLICY_OFFSET_V1] > 1 || bytes[REALM_FREEZE_AUTHORITY_POLICY_OFFSET_V1] > 1) throw new Error('Realm authority policy is undefined');
  requireZero(bytes, REALM_RESERVED_OFFSET_V1, REALM_RESERVED_BYTES_V1, 'Realm header');
  [REALM_TOKEN_PROGRAM_OFFSET_V1, REALM_COLLATERAL_MINT_OFFSET_V1, REALM_ADAPTER_RELEASE_ID_OFFSET_V1]
    .forEach((offset) => requireNonzero(slice(bytes, offset, 32), 'Realm identity'));
}

export function validateCoreFoundSourceMaterialV3(bytes: Uint8Array, productDigest: Uint8Array): void {
  if (bytes.length !== SOURCE_MATERIAL_BYTES || ascii(bytes, 0, 8) !== 'DCLTSMV3' || u16(bytes, 8) !== 3) throw new Error('SourceMaterialV3 has the wrong exact ABI');
  if (bytes[10] > 1 || (bytes[11] !== 1 && bytes[11] !== 2)) throw new Error('SourceMaterialV3 tag is undefined');
  requireZero(bytes, 12, 4, 'SourceMaterialV3 header');
  if (!same(slice(bytes, 16, 32), productDigest)) throw new Error('SourceMaterialV3 selects a different Product record digest');
  [16, 48, 80, 112, 176].forEach((offset) => requireNonzero(slice(bytes, offset, 32), 'SourceMaterialV3 identity'));
  const recovery = slice(bytes, 144, 32);
  const floor = slice(bytes, 208, 32);
  if ((bytes[10] === 0 && !isZero(recovery)) || (bytes[10] === 1 && isZero(recovery))) throw new Error('SourceMaterialV3 recovery policy is noncanonical');
  if ((bytes[11] === 1 && !isZero(floor)) || (bytes[11] === 2 && isZero(floor))) throw new Error('SourceMaterialV3 principal policy is noncanonical');
}

function validateFundingQuote(bytes: Uint8Array): Readonly<{ rent: bigint; creation: bigint }> {
  const kindOffset = CAPABILITY_FUNDING_QUOTE_COLLATERAL_KIND_OFFSET_V1;
  if (bytes.length !== CAPABILITY_FUNDING_QUOTE_BYTES_V1
      || ascii(bytes, CAPABILITY_FUNDING_QUOTE_MAGIC_OFFSET_V1, 8) !== CAPABILITY_FUNDING_QUOTE_MAGIC_V1
      || u16(bytes, CAPABILITY_FUNDING_QUOTE_SCHEMA_OFFSET_V1) !== CAPABILITY_FUNDING_QUOTE_SCHEMA_VERSION_V1
      || bytes[kindOffset] > 1) throw new Error('capability funding quote has the wrong exact ABI');
  requireZero(bytes, CAPABILITY_FUNDING_QUOTE_RESERVED_OFFSET_V1, CAPABILITY_FUNDING_QUOTE_RESERVED_BYTES_V1, 'capability funding quote header');
  const binding = slice(bytes, CAPABILITY_FUNDING_QUOTE_BINDING_OFFSET_V1, CAPABILITY_FUNDING_BINDING_BYTES_V1);
  if (bytes[kindOffset] === 0) requireZero(binding, 0, CAPABILITY_FUNDING_BINDING_BYTES_V1, 'absent Realm funding binding');
  else [
    CAPABILITY_FUNDING_BINDING_REALM_ID_OFFSET_V1,
    CAPABILITY_FUNDING_BINDING_RELEASE_ID_OFFSET_V1,
    CAPABILITY_FUNDING_BINDING_TOKEN_PROGRAM_OFFSET_V1,
    CAPABILITY_FUNDING_BINDING_MINT_OFFSET_V1,
    CAPABILITY_FUNDING_BINDING_BENEFICIARY_OFFSET_V1,
  ].forEach((offset) => requireNonzero(slice(binding, offset, 32), 'Realm funding binding'));
  let nativeTotal = 0n;
  let realmTotal = 0n;
  const amounts: bigint[] = [];
  const amountsOffset = CAPABILITY_FUNDING_QUOTE_AMOUNTS_OFFSET_V1;
  for (let index = 0; index < FUNDING_COMPARTMENTS_V1.length; index += 1) {
    const offset = amountsOffset + FUNDING_COMPARTMENTS_V1[index].offset;
    const asset = bytes[offset + CAPABILITY_FUNDING_ALLOCATION_CLASS_OFFSET_V1];
    const amount = u64(bytes, offset + CAPABILITY_FUNDING_ALLOCATION_AMOUNT_OFFSET_V1);
    requireZero(bytes, offset + CAPABILITY_FUNDING_ALLOCATION_RESERVED_OFFSET_V1, CAPABILITY_FUNDING_ALLOCATION_RESERVED_BYTES_V1, 'capability funding allocation');
    const nativeOnly = FUNDING_COMPARTMENTS_V1[index].assetPolicy === 'native-lamports-only';
    if (asset > 2 || (amount === 0n) !== (asset === 0) || (nativeOnly && asset === 2)) throw new Error('capability funding allocation has a noncanonical asset class');
    if (asset === 1) nativeTotal += amount;
    if (asset === 2) realmTotal += amount;
    if (nativeTotal > 0xffff_ffff_ffff_ffffn || realmTotal > 0xffff_ffff_ffff_ffffn) {
      throw new Error('capability funding compartment total overflows u64');
    }
    amounts.push(amount);
  }
  if (u64(bytes, amountsOffset + CAPABILITY_FUNDING_AMOUNTS_NATIVE_TOTAL_OFFSET_V1) !== nativeTotal
      || u64(bytes, amountsOffset + CAPABILITY_FUNDING_AMOUNTS_REALM_TOTAL_OFFSET_V1) !== realmTotal) throw new Error('capability funding totals do not equal their typed compartments');
  if ((realmTotal === 0n) !== (bytes[kindOffset] === 0)) throw new Error('Realm funding binding does not match Realm collateral use');
  return Object.freeze({ rent: amounts[0], creation: amounts[1] });
}

export function validateCoreFoundCapabilityManifestV1(bytes: Uint8Array): void {
  if (bytes.length < CAPABILITY_MANIFEST_HEADER_BYTES_V1
      || ascii(bytes, CAPABILITY_MANIFEST_MAGIC_OFFSET_V1, 8) !== CAPABILITY_MANIFEST_MAGIC_V1
      || u16(bytes, CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1) !== CAPABILITY_MANIFEST_SCHEMA_VERSION_V1
      || u16(bytes, CAPABILITY_MANIFEST_PROFILE_OFFSET_V1) !== CAPABILITY_MANIFEST_ARTIFACT_PROFILE_V1) throw new Error('capability manifest has the wrong exact header');
  requireZero(bytes, CAPABILITY_MANIFEST_RESERVED_OFFSET_V1, CAPABILITY_MANIFEST_HEADER_RESERVED_BYTES_V1, 'capability manifest header');
  const count = u16(bytes, CAPABILITY_MANIFEST_COUNT_OFFSET_V1);
  if (count > MAX_CAPABILITIES_V1 || bytes.length !== CAPABILITY_MANIFEST_HEADER_BYTES_V1 + count * CAPABILITY_ENTRY_BYTES_V1) throw new Error('capability manifest width is not canonical');
  const dependencies: number[][] = [];
  let previous: Uint8Array | null = null;
  for (let index = 0; index < count; index += 1) {
    const offset = CAPABILITY_MANIFEST_HEADER_BYTES_V1 + index * CAPABILITY_ENTRY_BYTES_V1;
    const kind = slice(bytes, offset + CAPABILITY_ENTRY_KIND_ID_OFFSET_V1, 32);
    [
      CAPABILITY_ENTRY_KIND_ID_OFFSET_V1,
      CAPABILITY_ENTRY_RELEASE_ID_OFFSET_V1,
      CAPABILITY_ENTRY_CONFIG_ID_OFFSET_V1,
      CAPABILITY_ENTRY_CAPACITY_PROFILE_ID_OFFSET_V1,
      CAPABILITY_ENTRY_CHILD_SCHEMA_ID_OFFSET_V1,
      CAPABILITY_ENTRY_CHILD_DERIVATION_ID_OFFSET_V1,
    ].forEach((relative) => requireNonzero(slice(bytes, offset + relative, 32), 'capability entry identity'));
    if (previous !== null && compareBytes(previous, kind) >= 0) throw new Error('capability entries are not strictly ordered');
    previous = kind;
    const policy = bytes[offset + CAPABILITY_ENTRY_ACTIVATION_POLICY_OFFSET_V1];
    const dependencyCount = bytes[offset + CAPABILITY_ENTRY_DEPENDENCY_COUNT_OFFSET_V1];
    if (policy > 1 || dependencyCount > MAX_DEPENDENCIES_PER_CAPABILITY_V1) throw new Error('capability entry policy or dependency count is undefined');
    requireZero(bytes, offset + CAPABILITY_ENTRY_RESERVED_OFFSET_V1, CAPABILITY_ENTRY_RESERVED_BYTES_V1, 'capability entry header');
    const active: number[] = [];
    for (let position = 0; position < MAX_DEPENDENCIES_PER_CAPABILITY_V1; position += 1) {
      const dependency = bytes[offset + CAPABILITY_ENTRY_DEPENDENCIES_OFFSET_V1 + position];
      if (position < dependencyCount) {
        if (dependency >= count || dependency === index || (active.length > 0 && active[active.length - 1] >= dependency)) throw new Error('capability dependency is invalid or noncanonical');
        active.push(dependency);
      } else if (dependency !== 0) throw new Error('inactive capability dependency is nonzero');
    }
    const deadline = u64(bytes, offset + CAPABILITY_ENTRY_ACTIVATION_DEADLINE_OFFSET_V1);
    const quote = validateFundingQuote(slice(bytes, offset + CAPABILITY_ENTRY_QUOTE_OFFSET_V1, CAPABILITY_FUNDING_QUOTE_BYTES_V1));
    if ((policy === 0 && deadline !== 0n) || (policy === 1 && (deadline === 0n || (quote.rent === 0n && quote.creation === 0n)))) throw new Error('capability activation policy and prepaid creation facts do not join');
    dependencies.push(active);
  }
  const resolved = new Set<number>();
  while (resolved.size < count) {
    const before = resolved.size;
    dependencies.forEach((entry, index) => { if (!resolved.has(index) && entry.every((dependency) => resolved.has(dependency))) resolved.add(index); });
    if (resolved.size === before) throw new Error('capability dependency graph is cyclic');
  }
}

export function decodeCoreFoundProductGraphV2(product: Uint8Array, domain: Uint8Array, portfolio: Uint8Array, domainDigest: Uint8Array, portfolioDigest: Uint8Array): ProductGraph {
  if (product.length !== PRODUCT_RECORD_BYTES || ascii(product, 0, 8) !== 'DCLTPRM2' || u16(product, 8) !== 2) throw new Error('Product Runtime V2 root has the wrong exact ABI');
  requireZero(product, 10, 6, 'Product Runtime V2 root');
  const productId = slice(product, 16, 32);
  requireNonzero(productId, 'Product identity');
  if (!same(slice(product, 48, 32), domainDigest) || !same(slice(product, 80, 32), portfolioDigest)) throw new Error('Product root does not select the supplied domain and portfolio');

  if (domain.length < DOMAIN_HEADER_BYTES || ascii(domain, 0, 8) !== 'DCLTPRD2' || u16(domain, 8) !== 2 || u16(domain, 10) !== DOMAIN_HEADER_BYTES || u32(domain, 12) !== domain.length) throw new Error('Product result domain has the wrong exact ABI');
  requireZero(domain, 24, 8, 'Product result-domain header');
  requireZero(domain, 232, 8, 'Product result-domain tail');
  const regions = u32(domain, 16);
  const cuts = u32(domain, 20);
  if (regions === 0 || regions !== cuts + 1 || domain.length !== DOMAIN_HEADER_BYTES + cuts * 16) throw new Error('Product result-domain width is inconsistent');
  [32, 64, 96, 128, 160, 192].forEach((offset) => requireNonzero(slice(domain, offset, 32), 'Product result-domain identity'));
  if (!same(slice(domain, 32, 32), productId) || u64(domain, 224) === 0n) throw new Error('Product result-domain identity or denominator differs');
  let prior: bigint | null = null;
  for (let index = 0; index < cuts; index += 1) {
    const cut = i128(domain, DOMAIN_HEADER_BYTES + index * 16);
    if (prior !== null && cut <= prior) throw new Error('Product result-domain cuts are not strictly increasing');
    prior = cut;
  }

  if (portfolio.length < PORTFOLIO_HEADER_BYTES || ascii(portfolio, 0, 8) !== 'DCLTPRF2' || u16(portfolio, 8) !== 2 || u16(portfolio, 10) !== PORTFOLIO_HEADER_BYTES || u32(portfolio, 12) !== portfolio.length) throw new Error('Product portfolio has the wrong exact ABI');
  const coefficientCount = u32(portfolio, 16);
  if (coefficientCount === 0 || coefficientCount !== regions + 1 || portfolio.length !== PORTFOLIO_HEADER_BYTES + coefficientCount * 8 || portfolio[20] !== 1) throw new Error('Product portfolio width or rounding boundary is inconsistent');
  requireZero(portfolio, 21, 11, 'Product portfolio header');
  requireZero(portfolio, 200, 8, 'Product portfolio tail');
  [32, 64, 96, 128, 160].forEach((offset) => requireNonzero(slice(portfolio, offset, 32), 'Product portfolio identity'));
  if (!same(slice(portfolio, 32, 32), productId) || !same(slice(portfolio, 64, 32), domainDigest) || !same(slice(portfolio, 128, 32), slice(domain, 128, 32)) || !same(slice(portfolio, 160, 32), slice(domain, 160, 32))) throw new Error('Product domain and portfolio identities do not join');
  let divisor = u64(portfolio, 192);
  let nonzero = false;
  if (divisor === 0n) throw new Error('Product portfolio denominator is zero');
  for (let index = 0; index < coefficientCount; index += 1) {
    const coefficient = u64(portfolio, PORTFOLIO_HEADER_BYTES + index * 8);
    nonzero ||= coefficient !== 0n;
    divisor = gcd(divisor, coefficient);
  }
  if (!nonzero || divisor !== 1n) throw new Error('Product portfolio is empty or not gcd-normalized');
  const outcomeCount = regions + 1;
  if (outcomeCount > MAX_U32) throw new Error('Product outcome width exceeds u32');
  return Object.freeze({ productId, outcomeCount });
}

function generationBytes(generation: bigint): Uint8Array {
  if (generation <= 0n || generation > 0xffff_ffff_ffff_ffffn) throw new Error('Market generation is outside lifecycle u64');
  const bytes = new Uint8Array(8);
  putU64(bytes, 0, generation);
  return bytes;
}

function lifecycleRentCreateRequest(input: Readonly<{
  refundWallet: PublicKey;
  market: PublicKey;
  releaseSet: Uint8Array;
  generation: bigint;
  bump: number;
}>): Uint8Array {
  if (input.releaseSet.length !== 32 || isZero(input.releaseSet)) throw new Error('execution release set is not one nonzero 32-byte identity');
  if (same(input.refundWallet.toBytes(), input.market.toBytes())
      || same(input.refundWallet.toBytes(), input.releaseSet)
      || same(input.market.toBytes(), input.releaseSet)) {
    throw new Error('lifecycle Rent identities alias');
  }
  const output = new Uint8Array(CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2);
  output.set(LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2, 0);
  putU16(output, 8, LIFECYCLE_RENT_SCHEMA_VERSION_V2);
  // The action discriminator, not a reserved byte. This was a hand-written `0`
  // and `LifecycleRentActionV2::Create` is 1, so every packet this builder
  // produced was refused at `LifecycleRentInstructionV2::decode` before the
  // Rent program looked at a single account. Nothing caught it because `/found`
  // downloaded the packet and never submitted one; the first submission, from
  // the create wizard against a local validator, refused in 1,041 CU.
  output[LIFECYCLE_RENT_INSTRUCTION_ACTION_OFFSET_V2] = LIFECYCLE_RENT_ACTION_CREATE_V2;
  output.set(input.refundWallet.toBytes(), 16);
  output.set(input.market.toBytes(), 48);
  output.set(input.releaseSet, 80);
  output.set(generationBytes(input.generation), 112);
  output[120] = input.bump;
  return output;
}

export function compileLifecycleRentCreateTransactionV2(input: Readonly<{
  payer: string;
  refundWallet: string;
  market: string;
  releaseSet: Uint8Array;
  generation: bigint;
  rentProgram: string;
  recentBlockhash: string;
}>): CompiledLifecycleRentCreateTransactionV2 {
  const payer = key(input.payer, 'payer');
  const refundWallet = key(input.refundWallet, 'refund wallet');
  const market = key(input.market, 'Market');
  const rentProgram = key(input.rentProgram, 'Rent program');
  const generation = generationBytes(input.generation);
  const [rentCredit, bump] = PublicKey.findProgramAddressSync(
    [LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, market.toBytes(), generation],
    rentProgram,
  );
  const requestBytes = lifecycleRentCreateRequest({
    refundWallet,
    market,
    releaseSet: input.releaseSet,
    generation: input.generation,
    bump,
  });
  const instruction = new TransactionInstruction({
    programId: rentProgram,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: rentCredit, isSigner: false, isWritable: true },
      { pubkey: key(SYSTEM_PROGRAM_ID, 'System program'), isSigner: false, isWritable: false },
      { pubkey: key(RENT_SYSVAR_ID, 'Rent sysvar'), isSigner: false, isWritable: false },
    ],
    data: requestBytes as Buffer,
  });
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: payer,
    recentBlockhash: key(input.recentBlockhash, 'recent blockhash').toBase58(),
    instructions: [instruction],
  }).compileToV0Message());
  const wireBytes = transaction.serialize();
  if (wireBytes.length > SOLANA_PACKET_BYTES_V1) throw new Error('lifecycle Rent Create exceeds the packet bound');
  const requiredSigners = Object.freeze(transaction.message.staticAccountKeys
    .slice(0, transaction.message.header.numRequiredSignatures)
    .map((value) => value.toBase58()));
  if (requiredSigners.length !== 1 || requiredSigners[0] !== input.payer) throw new Error('lifecycle Rent Create requires an unexpected signer');
  return Object.freeze({ rentCredit: rentCredit.toBase58(), requestBytes, transaction, wireBytes, requiredSigners });
}

function foundRequest(generation: bigint, market: PublicKey): Uint8Array {
  if (generation < 0n || generation > 0xffff_ffff_ffff_ffffn) throw new Error('Market generation is outside u64');
  const output = new Uint8Array(CORE_REQUEST_BYTES);
  output.set(CORE_REQUEST_MAGIC, 0);
  putU16(output, 8, CORE_VERSION);
  output[10] = CORE_ACTION_FOUND_TAG;
  putU64(output, 32, generation);
  output.set(market.toBytes(), 40);
  return output;
}

export function compileCoreFoundTransactionV2(input: Readonly<{
  payer: string;
  coreProgram: string;
  market: string;
  generation: bigint;
  recentBlockhash: string;
  accountAddresses: ReadonlyArray<string>;
  lookupTable?: AddressLookupTableAccount;
}>): CompiledCoreFoundTransactionV2 {
  if (input.accountAddresses.length !== CORE_FOUND_ACCOUNT_COUNT_V3) {
    throw new Error(`Core Found requires exactly ${CORE_FOUND_ACCOUNT_COUNT_V3} account metas`);
  }
  if (new Set(input.accountAddresses).size !== CORE_FOUND_ACCOUNT_COUNT_V3) {
    throw new Error('Core Found account metas alias named roles');
  }
  if (input.accountAddresses[0] !== input.payer
      || input.accountAddresses[1] !== input.market
      || input.accountAddresses[25] !== input.coreProgram) {
    throw new Error('Core Found payer, Market, or Core program is at the wrong exact account index');
  }
  const payer = key(input.payer, 'payer');
  const market = key(input.market, 'Market');
  const requestBytes = foundRequest(input.generation, market);
  const metas = input.accountAddresses.map((address, index) => ({
    pubkey: key(address, `Found account ${index}`),
    isSigner: CORE_FOUND_ACCOUNT_ROLES_V3[index].signer,
    isWritable: CORE_FOUND_ACCOUNT_ROLES_V3[index].writable,
  }));
  const instruction = new TransactionInstruction({
    programId: key(input.coreProgram, 'Core program'),
    keys: metas,
    data: requestBytes as Buffer,
  });
  // The bounded-instruction owner supplies the configured limit. This builder
  // reproduces its bytes; it does not infer a compute margin from the old wire.
  // Current V2 routes require their own pass-count and 20-seed mean evidence.
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: payer,
    recentBlockhash: key(input.recentBlockhash, 'recent blockhash').toBase58(),
    instructions: [...boundedInstructionsV1([instruction])],
  }).compileToV0Message(input.lookupTable === undefined ? undefined : [input.lookupTable]));
  let wireBytes: Uint8Array;
  try {
    wireBytes = transaction.serialize();
  } catch (error) {
    throw new Error(`Core Found transaction exceeds the ${SOLANA_PACKET_BYTES_V1}-byte packet bound${input.lookupTable === undefined ? '; this frame requires a finalized routing table' : ''}`, { cause: error });
  }
  if (wireBytes.length > SOLANA_PACKET_BYTES_V1) {
    throw new Error(input.lookupTable === undefined
      ? `Core Found transaction is ${wireBytes.length} bytes inline, above the ${SOLANA_PACKET_BYTES_V1}-byte packet bound; this frame requires a finalized routing table`
      : `Core Found transaction is ${wireBytes.length} bytes, above the ${SOLANA_PACKET_BYTES_V1}-byte packet bound`);
  }
  const requiredSigners = Object.freeze(transaction.message.staticAccountKeys
    .slice(0, transaction.message.header.numRequiredSignatures)
    .map((value) => value.toBase58()));
  if (requiredSigners.length !== 1 || requiredSigners[0] !== input.payer) {
    throw new Error('Core Found transaction requires an unexpected signer');
  }
  return Object.freeze({ requestBytes, transaction, wireBytes, requiredSigners });
}

function infrastructureEqual(left: ProtocolInfrastructureInspectionV1, right: ProtocolInfrastructureInspectionV1): boolean {
  return left.registryProgram === right.registryProgram
    && left.activationCache === right.activationCache
    && left.executionReleaseSetId === right.executionReleaseSetId
    && left.profilePda === right.profilePda
    && left.profileDigest === right.profileDigest
    && JSON.stringify(left.core) === JSON.stringify(right.core)
    && JSON.stringify(left.registry) === JSON.stringify(right.registry)
    && JSON.stringify(left.rent) === JSON.stringify(right.rent);
}

export async function prepareCoreFoundV2(client: SolanaRpcClient, input: CoreFoundInputV2): Promise<CoreFoundPlanV2> {
  generationBytes(input.generation);
  key(input.payer, 'payer');
  const registry = key(input.registryProgram, 'Registry program');
  key(input.activationCache, 'activation cache');
  key(input.refundWallet, 'refund wallet');
  const rawAddresses = [input.realmRecord, input.productRecord, input.resultDomainRecord, input.portfolioRecord, input.linkedBasisRecord, input.sourceMaterialRecord, input.sourceSpecRecord, input.capacityProfileRecord, input.manipulationFloorRecord, input.capabilityManifestRecord];
  rawAddresses.forEach((address, index) => key(address, `finalized raw record ${index}`));
  if (new Set([input.registryProgram, input.activationCache, input.refundWallet, ...rawAddresses]).size !== 13) throw new Error('Found authority inputs alias named roles');

  const infrastructure = await inspectProtocolInfrastructureV1(client, { registryProgram: registry.toBase58(), activationCache: input.activationCache });
  const initial = await client.multipleAccounts(rawAddresses, infrastructure.observedSlot);
  const initialAccounts = accountMap(initial);
  const specs = [
    [input.realmRecord, REALM_SCHEMA_RELEASE_ID_V1, 'Realm'],
    [input.productRecord, PRODUCT_RECORD_SCHEMA_ID_V2, 'Product'],
    [input.resultDomainRecord, RESULT_DOMAIN_SCHEMA_ID_V2, 'result domain'],
    [input.portfolioRecord, PORTFOLIO_SCHEMA_ID_V2, 'portfolio'],
    [input.linkedBasisRecord, GRADED_BASIS_RECORD_SCHEMA_ID_V3, 'linked basis'],
    [input.sourceMaterialRecord, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, 'Source material'],
    [input.sourceSpecRecord, SOURCE_SPEC_SCHEMA_ID_V1, 'Source spec'],
    [input.capacityProfileRecord, SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1, 'capacity profile'],
    [input.manipulationFloorRecord, MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1, 'manipulation floor'],
    [input.capabilityManifestRecord, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, 'capability manifest'],
  ] as const;
  const authorities = await Promise.all(specs.map(([address, schema, field]) => recordAuthority(client, input.registryProgram, address, schema, required(initialAccounts, address, field), field)));
  const releaseSetDigest = Uint8Array.from(infrastructure.executionReleaseSetId.match(/../g) ?? [], (value) => Number.parseInt(value, 16));
  if (releaseSetDigest.length !== 32 || isZero(releaseSetDigest)) throw new Error('activation cache has an invalid release-set identity');

  const [realm, product, domain, portfolio, linkedBasis, source, sourceSpec, capacityProfile, manipulationFloor, manifest] = authorities;
  validateRealm(realm.bytes);
  const graph = decodeCoreFoundProductGraphV2(product.bytes, domain.bytes, portfolio.bytes, domain.digest, portfolio.digest);
  validateCoreFoundSourceMaterialV3(source.bytes, product.digest);
  if (!same(slice(source.bytes, 48, 32), sourceSpec.digest)
      || !same(slice(sourceSpec.bytes, 144, 32), capacityProfile.digest)
      || !same(slice(source.bytes, 208, 32), manipulationFloor.digest)) throw new Error('SourceMaterialV3 graph identities differ from the authenticated records');
  validateCoreFoundCapabilityManifestV1(manifest.bytes);

  const market = PublicKey.findProgramAddressSync([
    MARKET_CORE_STATE_PDA_DOMAIN_V2,
    realm.digest,
    product.digest,
    graph.productId,
    source.digest,
    manifest.digest,
    releaseSetDigest,
    registry.toBytes(),
    generationBytes(input.generation),
  ], key(infrastructure.core.program, 'Core program'))[0];
  const [rentCredit] = PublicKey.findProgramAddressSync([
    LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
    market.toBytes(),
    generationBytes(input.generation),
  ], key(infrastructure.rent.program, 'Rent program'));
  const registryArtifact = deriveFinalizedRecordAddressesV1(input.registryProgram, ARTIFACT_RELEASE_SCHEMA_ID_V1, Uint8Array.from(infrastructure.registry.artifactReleaseId.match(/../g) ?? [], (value) => Number.parseInt(value, 16)));
  const rentArtifact = deriveFinalizedRecordAddressesV1(input.registryProgram, ARTIFACT_RELEASE_SCHEMA_ID_V1, Uint8Array.from(infrastructure.rent.artifactReleaseId.match(/../g) ?? [], (value) => Number.parseInt(value, 16)));
  const accountAddresses = [
    input.payer, market.toBase58(), rentCredit.toBase58(), infrastructure.rent.program,
    realm.raw, realm.staging, product.raw, product.staging, domain.raw, domain.staging, portfolio.raw, portfolio.staging,
    linkedBasis.raw, linkedBasis.staging, source.raw, source.staging, sourceSpec.raw, sourceSpec.staging,
    capacityProfile.raw, capacityProfile.staging, manipulationFloor.raw, manipulationFloor.staging,
    manifest.raw, manifest.staging, input.activationCache, infrastructure.core.program, infrastructure.core.programData, input.registryProgram, RENT_SYSVAR_ID, SYSTEM_PROGRAM_ID,
    infrastructure.profilePda, registryArtifact.record, registryArtifact.staging, infrastructure.registry.programData,
    rentArtifact.record, rentArtifact.staging, infrastructure.rent.programData,
  ];
  if (accountAddresses.length !== CORE_FOUND_ACCOUNT_COUNT_V3 || new Set(accountAddresses).size !== CORE_FOUND_ACCOUNT_COUNT_V3) throw new Error('Found37 account projection aliases named roles');
  const observationAddresses = input.refundWallet === input.payer ? accountAddresses : [...accountAddresses, input.refundWallet];
  const finalObservation = await acquireFinalizedAccountsInChunksV1(client, observationAddresses, initial.slot);
  const finalAccounts = accountMap(finalObservation);
  const payerAccount = required(finalAccounts, input.payer, 'payer');
  if (payerAccount.owner !== SYSTEM_PROGRAM_ID || payerAccount.executable || payerAccount.data.length !== 0) throw new Error('payer is not a System-owned data-free wallet');
  vacant(finalAccounts.get(market.toBase58()), 'Market destination');
  const refundWallet = required(finalAccounts, input.refundWallet, 'refund wallet');
  if (refundWallet.owner !== SYSTEM_PROGRAM_ID || refundWallet.executable || refundWallet.data.length !== 0) throw new Error('refund wallet is not a System-owned data-free wallet');
  // The credit is a PRECONDITION of Found37 and the OUTPUT of its own
  // transaction, and those are different requirements. Demanding vacancy in
  // both places is what made a two-stage flow impossible: the moment the credit
  // landed, re-preparing Found37 against the same coordinates refused. So the
  // observation branches on what is actually there, and each branch states the
  // whole of its own requirement.
  const creditDestination = finalAccounts.get(rentCredit.toBase58()) ?? null;
  const creditExists = creditDestination !== null && creditDestination.owner === infrastructure.rent.program;
  if (creditExists) {
    if (creditDestination.executable
        || creditDestination.data.length !== LIFECYCLE_RENT_CREDIT_BYTES_V2
        || !same(slice(creditDestination.data, LIFECYCLE_RENT_CREDIT_MAGIC_OFFSET_V2, LIFECYCLE_RENT_CREDIT_MAGIC_V2.length), LIFECYCLE_RENT_CREDIT_MAGIC_V2)) {
      throw new Error('the existing lifecycle RentCredit is not a canonical Rent-owned credit account');
    }
  } else {
    vacant(creditDestination, 'lifecycle RentCredit destination');
    if ((creditDestination?.lamports ?? '0') !== '0') throw new Error('lifecycle RentCredit destination is prefunded');
  }
  for (const authority of authorities) {
    const raw = required(finalAccounts, authority.raw, 'finalized raw record');
    if (raw.owner !== input.registryProgram || raw.executable || BigInt(raw.lamports) < authority.rentMinimum || !same(raw.data, authority.bytes) || !same(await sha256(raw.data), authority.digest)) throw new Error('finalized raw record changed or lost Registry/rent authority');
    vacant(finalAccounts.get(authority.staging), 'finalized staging cursor');
  }
  const rentSysvar = required(finalAccounts, RENT_SYSVAR_ID, 'Rent sysvar');
  const system = required(finalAccounts, SYSTEM_PROGRAM_ID, 'System Program');
  // A real Agave observation of the System Program carries the 14-byte
  // NativeLoader metadata body `system_program`; requiring emptiness refuses
  // every read of a live cluster (measured 2026-08-27, same defect the Rust
  // operators fixed in `770610c` / `c25de27`).
  if (rentSysvar.owner !== SYSVAR_OWNER_ID || rentSysvar.executable || rentSysvar.data.length !== 17 || system.owner !== NATIVE_LOADER_ID || !system.executable) throw new Error('Rent or System runtime account is not canonical');
  const marketRent = await client.minimumBalanceForRentExemption(CORE_STATE_BYTES);
  const creditRent = await client.minimumBalanceForRentExemption(LIFECYCLE_RENT_CREDIT_BYTES_V2);
  const marketLamports = finalAccounts.get(market.toBase58())?.lamports ?? '0';
  const marketRentTopUp = BigInt(marketRent.lamports) > BigInt(marketLamports) ? BigInt(marketRent.lamports) - BigInt(marketLamports) : 0n;
  // A credit that already exists is already paid for.
  const totalRentDebit = marketRentTopUp + (creditExists ? 0n : BigInt(creditRent.lamports));
  if (BigInt(payerAccount.lamports) < totalRentDebit) throw new Error('payer cannot cover the exact current Market and lifecycle-credit rent debit');
  const confirmedInfrastructure = await inspectProtocolInfrastructureV1(client, { registryProgram: input.registryProgram, activationCache: input.activationCache });
  if (!infrastructureEqual(infrastructure, confirmedInfrastructure)) throw new Error('immutable infrastructure projection changed during Found construction');
  const blockhash = await client.latestMutationBlockhash(confirmedInfrastructure.observedSlot);
  const rentCreation = creditExists ? null : compileLifecycleRentCreateTransactionV2({
    payer: input.payer,
    refundWallet: input.refundWallet,
    market: market.toBase58(),
    releaseSet: releaseSetDigest,
    generation: input.generation,
    rentProgram: infrastructure.rent.program,
    recentBlockhash: blockhash.blockhash,
  });
  if (rentCreation !== null && rentCreation.rentCredit !== rentCredit.toBase58()) throw new Error('lifecycle RentCredit derivation changed during compilation');
  // The frame's routable set is derived whether or not it can be compiled,
  // because a caller who has just learned it needs a table needs to know what
  // to put in one. Signers never route: a lookup table cannot carry one.
  const foundInstruction = new TransactionInstruction({
    programId: key(infrastructure.core.program, 'Core program'),
    keys: accountAddresses.map((address, index) => ({
      pubkey: key(address, `Found account ${index}`),
      isSigner: CORE_FOUND_ACCOUNT_ROLES_V3[index].signer,
      isWritable: CORE_FOUND_ACCOUNT_ROLES_V3[index].writable,
    })),
    data: foundRequest(input.generation, market) as Buffer,
  });
  const routableAddresses = routableAddressesV1([foundInstruction], input.payer);
  let compiled: CompiledCoreFoundTransactionV2 | null = null;
  let foundRefusal: string | null = null;
  try {
    compiled = compileCoreFoundTransactionV2({
      payer: input.payer,
      coreProgram: infrastructure.core.program,
      market: market.toBase58(),
      generation: input.generation,
      recentBlockhash: blockhash.blockhash,
      accountAddresses,
      lookupTable: input.lookupTable,
    });
  } catch (error) {
    // Only a packet-bound refusal is recoverable into a plan. Anything else --
    // an aliased role, a misplaced payer, an unexpected signer -- is a defect
    // in the projection and must not be softened into a status line.
    const message = error instanceof Error ? error.message : String(error);
    if (!message.includes('packet bound')) throw error;
    foundRefusal = message;
  }
  return Object.freeze({
    observedSlot: finalObservation.slot,
    market: market.toBase58(),
    rentCredit: rentCredit.toBase58(),
    coreProgram: infrastructure.core.program,
    registryProgram: input.registryProgram,
    rentProgram: infrastructure.rent.program,
    productRecordDigest: hex(product.digest),
    productId: hex(graph.productId),
    outcomeCount: graph.outcomeCount,
    executionReleaseSetId: infrastructure.executionReleaseSetId,
    infrastructureProfile: infrastructure.profilePda,
    infrastructureRecognition: infrastructure.recognition,
    marketRentTopUp: marketRentTopUp.toString(),
    rentCreditRentDebit: creditRent.lamports,
    lastValidBlockHeight: blockhash.lastValidBlockHeight,
    accountAddresses: Object.freeze(accountAddresses),
    routableAddresses,
    requiredSigners: compiled?.requiredSigners ?? Object.freeze([input.payer]),
    requestBytes: compiled?.requestBytes ?? foundRequest(input.generation, market),
    rentCreditState: creditExists ? 'created' : 'vacant',
    rentCreateRequestBytes: rentCreation?.requestBytes ?? null,
    rentCreateTransaction: rentCreation?.transaction ?? null,
    rentCreateWireBytes: rentCreation?.wireBytes ?? null,
    transaction: compiled?.transaction ?? null,
    wireBytes: compiled?.wireBytes ?? null,
    foundRefusal,
  });
}
