import { AddressLookupTableAccount, AddressLookupTableProgram, PublicKey } from '@solana/web3.js';

import { ascii, hex, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import {
  decodeDirectRootSelectionV1,
  decodeSelectedDirectManifestEntryV1,
  validateProductBasisV3,
  type DirectHotRouteCoordinateV3,
} from './directHotChain';
import {
  type DealerEquityHotRouteV3,
  type DealerEquityRequestV3,
  decodeDealerEquityRequestV3,
} from './dealerEquityV3';
import { validateDealerAccountProfileV3 } from './dealerAccountProfileV3';
import { type CheckedHotOuterEvidenceV3, type DirectHotAccountMetaV3 } from './directInlineV3';
import * as HotAbi from './generated/directInlineV3';
import {
  CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
  CORE_STATE_BYTES,
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
} from './generated/coreFound';
import {
  DEALER_CONFIG_BYTES_V4,
  DEALER_CONFIG_LOCKED_CAPITAL_FLOOR_OFFSET_V4,
  DEALER_CONFIG_MAGIC_V4,
  DEALER_CONFIG_POSITION_OWNER_OFFSET_V4,
  DEALER_CONFIG_REALM_OFFSET_V4,
  DEALER_CONFIG_RELEASE_SET_OFFSET_V4,
  DEALER_CONFIG_SCHEMA_PREIMAGE_V4,
  DEALER_CONFIG_VERSION_V4,
  DEALER_EQUITY_REQUEST_SCHEMA_PREIMAGE_V3,
  DEALER_KIND_PREIMAGE_V2,
  DEALER_LP_POSITION_BYTES_V3,
  DEALER_LP_POSITION_MAGIC_V3,
  DEALER_LP_POSITION_VERSION_V3,
  DEALER_OBLIGATION_HEADER_BYTES_V3,
  DEALER_OBLIGATION_MAGIC_V3,
  DEALER_OBLIGATION_VERSION_V3,
  DEALER_ROOT_SCHEMA_PREIMAGE_V2,
  EXECUTION_STRATEGY_ARTIFACT_PROFILE_V2,
  EXECUTION_STRATEGY_PROGRAM_BYTES_V2,
  EXECUTION_STRATEGY_PROGRAM_MAGIC_V2,
  EXECUTION_STRATEGY_SCHEMA_VERSION_V2,
  STRATEGY_DISPOSITION_OFFSET_V2,
} from './generated/dealerEquityV3';
import { decodeCoreFoundProductGraphV2 } from './coreFound';
import { decodeCheckedInfrastructureV1 } from './infrastructure';
import { ARTIFACT_RELEASE_BYTES, SYSTEM_PROGRAM_ID, authenticateArtifactDeploymentV1, deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

const MARKET_PRODUCT_RECORD_OFFSET = 80;
const MARKET_MANIFEST_OFFSET = 176;
const MARKET_RELEASE_SET_OFFSET = 208;
const MARKET_REGISTRY_OFFSET = 240;
const MARKET_GENERATION_OFFSET = 272;
const ACTIVATION_CACHE_TRADING_OFFSET = 48 + 2 * (32 + ARTIFACT_RELEASE_BYTES);
const DEALER_ROOT_TAIL_BYTES_V3 = 384;
const REQUEST_PROFILE_V1_SCHEMA_PREIMAGE = new TextEncoder().encode('dclutch/schema/request-profile-v1');
const REQUEST_PROFILE_V3_SCHEMA_PREIMAGE = new TextEncoder().encode('dclutch/schema/request-profile-v3-borrowed-witness-v1');

export type DealerEquityRouteManifestV3 = Readonly<{
  payer: string;
  fixedAccounts: ReadonlyArray<DirectHotRouteCoordinateV3>;
  strategyAccounts: ReadonlyArray<DirectHotRouteCoordinateV3>;
  runtimeAccounts: ReadonlyArray<DirectHotRouteCoordinateV3>;
  lookupTables: ReadonlyArray<string>;
  checkedInfrastructure: Uint8Array | null;
}>;

export type DealerEquityRouteInspectionV3 = Readonly<{
  observedSlot: string;
  request: DealerEquityRequestV3;
  route: DealerEquityHotRouteV3;
  selectedProgramDigest: string;
  accountProfileDigest: string;
  strategyDigest: string;
  requestProfileDigest: string;
  checkedOuter: CheckedHotOuterEvidenceV3;
}>;

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function u32(bytes: Uint8Array, offset: number): number {
  const value = slice(bytes, offset, 4);
  return new DataView(value.buffer, value.byteOffset, value.byteLength).getUint32(0, true);
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

async function acquire(client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>, addresses: ReadonlyArray<string>) {
  const canonical = [...new Set(addresses.map((address, index) => key(address, `Dealer route address ${index}`).toBase58()))];
  if (canonical.length > 128) throw new Error('Dealer route exceeds the explicit 128-account browser reacquisition bound');
  const floor = await client.finalizedSlot();
  const accounts = new Map<string, RpcAccount | null>();
  let slot = floor;
  for (const group of chunks(canonical, 32)) {
    const observation = await client.multipleAccounts(group, floor);
    if (BigInt(observation.slot) < BigInt(floor)) throw new Error('Dealer route observation regressed below its finalized floor');
    if (BigInt(observation.slot) > BigInt(slot)) slot = observation.slot;
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
  if (raw.owner !== registry || raw.executable || !same(await sha256(raw.data), expectedDigest)) throw new Error(`${field} is not exact Registry-owned finalized content`);
  const derived = deriveFinalizedRecordAddressesV1(registry, schema, expectedDigest);
  if (derived.record !== rawAddress || derived.staging !== stagingAddress) throw new Error(`${field} raw/staging addresses are not canonical Registry PDAs`);
  if (staging !== null && staging !== undefined && (staging.owner !== SYSTEM_PROGRAM_ID || staging.executable || staging.data.length !== 0)) throw new Error(`${field} staging cursor is not vacant System-owned data`);
  const rent = await client.minimumBalanceForRentExemption(raw.data.length);
  if (BigInt(raw.lamports) < BigInt(rent.lamports)) throw new Error(`${field} raw record is below its exact rent minimum`);
  return raw;
}

function metas(coordinates: ReadonlyArray<DirectHotRouteCoordinateV3>, accounts: ReadonlyMap<string, RpcAccount | null>, field: string): ReadonlyArray<DirectHotAccountMetaV3> {
  return Object.freeze(coordinates.map((coordinate, index) => Object.freeze({
    ...coordinate, executable: required(accounts, coordinate.address, `${field} ${index}`).executable,
  })));
}

function lookupTable(address: string, account: RpcAccount): AddressLookupTableAccount {
  if (account.owner !== AddressLookupTableProgram.programId.toBase58() || account.executable) throw new Error(`lookup table ${address} has the wrong owner or executable bit`);
  let state: ReturnType<typeof AddressLookupTableAccount.deserialize>;
  try { state = AddressLookupTableAccount.deserialize(account.data); } catch { throw new Error(`lookup table ${address} has malformed data`); }
  const value = new AddressLookupTableAccount({ key: key(address, 'lookup table'), state });
  if (!value.isActive()) throw new Error(`lookup table ${address} is deactivated`);
  return value;
}

function selectDealerProgram(bytes: Uint8Array, selector: number): Uint8Array {
  if (bytes.length < HotAbi.CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1 || ascii(bytes, 0, 8) !== 'DCLTCPS1'
      || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 1 || u32(bytes, 12) !== 10 || bytes[16] !== 2 || bytes[17] !== 0) {
    throw new Error('Dealer CapabilityProgramSet has the wrong exact u16@10 selector header');
  }
  requireZero(bytes, 20, 12, 'Dealer ProgramSet header');
  const count = u16(bytes, 18);
  if (count !== 9 || bytes.length !== HotAbi.CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1 + count * HotAbi.CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V1) {
    throw new Error('Dealer CapabilityProgramSet is not the exact nine-selector authority');
  }
  let prior = 0;
  let selected: Uint8Array | null = null;
  for (let index = 0; index < count; index += 1) {
    const offset = HotAbi.CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1 + index * HotAbi.CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V1;
    const value = u32(bytes, offset);
    if (value <= prior || value > 0xffff) throw new Error('Dealer ProgramSet selectors are not canonical increasing u16 values');
    prior = value;
    requireZero(bytes, offset + 36, 4, 'Dealer ProgramSet entry');
    const digest = slice(bytes, offset + 4, 32);
    requireNonzero(digest, 'Dealer ProgramSet descriptor');
    if (value === selector) selected = digest;
  }
  if (selected === null) throw new Error('Dealer ProgramSet does not admit the selected executable equity action');
  return selected;
}

async function decodeDealerDescriptor(bytes: Uint8Array, request: DealerEquityRequestV3) {
  if (bytes.length !== HotAbi.CAPABILITY_PROGRAM_V3_BYTES || !same(slice(bytes, 0, 8), HotAbi.CAPABILITY_PROGRAM_V3_MAGIC)
      || u16(bytes, HotAbi.CAPABILITY_PROGRAM_V3_SCHEMA_VERSION_OFFSET) !== HotAbi.CAPABILITY_PROGRAM_V3_SCHEMA_VERSION
      || u16(bytes, HotAbi.CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE_OFFSET) !== HotAbi.CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE
      || u16(bytes, HotAbi.CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION_OFFSET) !== HotAbi.CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION
      || u16(bytes, HotAbi.CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION_OFFSET) !== HotAbi.CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION) {
    throw new Error('Dealer CapabilityProgramV3 descriptor has the wrong exact ABI');
  }
  const expectedRequestProfileSchema = await sha256(request.signedPositionCount === 0 ? REQUEST_PROFILE_V1_SCHEMA_PREIMAGE : REQUEST_PROFILE_V3_SCHEMA_PREIMAGE);
  if (!same(slice(bytes, HotAbi.CAPABILITY_PROGRAM_V3_KIND_OFFSET, 32), await sha256(DEALER_KIND_PREIMAGE_V2))
      || !same(slice(bytes, HotAbi.CAPABILITY_PROGRAM_V3_CONFIG_SCHEMA_OFFSET, 32), await sha256(DEALER_CONFIG_SCHEMA_PREIMAGE_V4))
      || !same(slice(bytes, HotAbi.CAPABILITY_PROGRAM_V3_REQUEST_SCHEMA_OFFSET, 32), await sha256(DEALER_EQUITY_REQUEST_SCHEMA_PREIMAGE_V3))
      || !same(slice(bytes, HotAbi.CAPABILITY_PROGRAM_V3_ROOT_SCHEMA_OFFSET, 32), await sha256(DEALER_ROOT_SCHEMA_PREIMAGE_V2))
      || !same(slice(bytes, HotAbi.CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_OFFSET, 32), expectedRequestProfileSchema)
      || !same(slice(bytes, HotAbi.CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_OFFSET, 32), HotAbi.EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2)
      || u32(bytes, HotAbi.CAPABILITY_PROGRAM_V3_ROOT_STATE_BYTES_OFFSET) !== DEALER_ROOT_TAIL_BYTES_V3) {
    throw new Error('selected descriptor is not the exact Dealer equity successor shape');
  }
  requireZero(bytes, HotAbi.CAPABILITY_PROGRAM_V3_TAIL_RESERVED_OFFSET, 4, 'Dealer descriptor tail');
  return Object.freeze({
    configSchema: slice(bytes, 48, 32), rootSchema: slice(bytes, 112, 32), accountProfile: slice(bytes, 144, 32),
    lifecycle: slice(bytes, 176, 32), capacityProfile: slice(bytes, 208, 32), effect: slice(bytes, 240, 32),
    requestProfileSchema: slice(bytes, 272, 32), requestProfileProgram: slice(bytes, 304, 32),
    strategySchema: slice(bytes, 336, 32), strategyProgram: slice(bytes, 368, 32),
  });
}

function validateDealerConfigV4(bytes: Uint8Array, request: DealerEquityRequestV3): void {
  if (bytes.length !== DEALER_CONFIG_BYTES_V4 || !same(slice(bytes, 0, 8), DEALER_CONFIG_MAGIC_V4)
      || u16(bytes, 8) !== DEALER_CONFIG_VERSION_V4) {
    throw new Error('Dealer config is not the exact acyclic V4 record');
  }
  requireZero(bytes, 10, 6, 'Dealer config header');
  requireZero(bytes, 120, 8, 'Dealer config tail');
  const realm = slice(bytes, DEALER_CONFIG_REALM_OFFSET_V4, 32);
  requireNonzero(realm, 'Dealer config Realm');
  if (!same(slice(bytes, DEALER_CONFIG_RELEASE_SET_OFFSET_V4, 32), request.releaseSet)
      || new PublicKey(slice(bytes, DEALER_CONFIG_POSITION_OWNER_OFFSET_V4, 32)).toBase58() !== request.dealerPositionOwner) {
    throw new Error('Dealer config release or Position owner does not join the request');
  }
  // Decode the exact scalar boundary even when a zero floor is selected. The
  // evaluator, not the browser, remains the authority for scenario solvency.
  u64(bytes, DEALER_CONFIG_LOCKED_CAPITAL_FLOOR_OFFSET_V4);
}

function validateAdmittedStrategy(bytes: Uint8Array, transition: Uint8Array): void {
  if (bytes.length !== EXECUTION_STRATEGY_PROGRAM_BYTES_V2 || !same(slice(bytes, 0, 8), EXECUTION_STRATEGY_PROGRAM_MAGIC_V2)
      || u16(bytes, 8) !== EXECUTION_STRATEGY_SCHEMA_VERSION_V2 || u16(bytes, 10) !== EXECUTION_STRATEGY_ARTIFACT_PROFILE_V2
      || bytes[STRATEGY_DISPOSITION_OFFSET_V2] !== 2 || bytes[13] !== 1 || bytes[14] !== 1 || bytes[15] !== 0
      || !same(slice(bytes, 16, 32), HotAbi.TRANSITION_SCHEMA_RELEASE_ID) || !same(slice(bytes, 48, 32), transition)) {
    throw new Error('Dealer ExecutionStrategy is not the admitted chunked-bank successor selecting this TransitionVM');
  }
  [80, 112, 144, 176, 208, 240].forEach((offset) => requireNonzero(slice(bytes, offset, 32), 'Dealer admitted strategy identity'));
}

async function validateDealerState(
  request: DealerEquityRequestV3,
  accounts: ReadonlyMap<string, RpcAccount | null>,
  tradingProgram: string,
  productId: Uint8Array,
  basisId: Uint8Array,
): Promise<void> {
  const obligation = required(accounts, request.obligation, 'Dealer obligation');
  if (obligation.owner !== tradingProgram || obligation.executable || obligation.data.length !== DEALER_OBLIGATION_HEADER_BYTES_V3 + 8 * request.width
      || !same(slice(obligation.data, 0, 8), DEALER_OBLIGATION_MAGIC_V3) || u16(obligation.data, 8) !== DEALER_OBLIGATION_VERSION_V3
      || u32(obligation.data, 12) !== request.width || u64(obligation.data, 16) !== request.obligationRevision
      || !same(slice(obligation.data, 24, 32), key(request.market, 'Market').toBytes())
      || !same(slice(obligation.data, 56, 32), productId) || !same(slice(obligation.data, 88, 32), basisId)
      || new PublicKey(slice(obligation.data, 120, 32)).toBase58() !== request.dealerPositionOwner
      || new PublicKey(slice(obligation.data, 152, 32)).toBase58() !== request.childRoot
      || !same(await sha256(obligation.data), request.obligationDigest)) {
    throw new Error('Dealer obligation does not rejoin the request, Product basis, revision, or exact digest');
  }
  const lp = required(accounts, request.lpPosition, 'Dealer LP Position');
  if (lp.owner !== tradingProgram || lp.executable || lp.data.length !== DEALER_LP_POSITION_BYTES_V3
      || !same(slice(lp.data, 0, 8), DEALER_LP_POSITION_MAGIC_V3) || u16(lp.data, 8) !== DEALER_LP_POSITION_VERSION_V3
      || u64(lp.data, 16) !== request.lpRevision || !same(slice(lp.data, 24, 32), request.releaseSet)
      || new PublicKey(slice(lp.data, 56, 32)).toBase58() !== request.market
      || new PublicKey(slice(lp.data, 88, 32)).toBase58() !== request.childRoot
      || new PublicKey(slice(lp.data, 120, 32)).toBase58() !== request.lpOwner
      || new PublicKey(slice(lp.data, 184, 32)).toBase58() !== request.obligation
      || u64(lp.data, 224) !== request.generation || !same(await sha256(lp.data), request.lpDigest)) {
    throw new Error('Dealer LP Position does not rejoin the request, revision, generation, or exact digest');
  }
}

export async function inspectDealerEquityRouteV3(
  client: SolanaRpcClient,
  manifest: DealerEquityRouteManifestV3,
  requestBytes: Uint8Array,
): Promise<DealerEquityRouteInspectionV3> {
  const request = await decodeDealerEquityRequestV3(requestBytes);
  if (manifest.fixedAccounts.length !== HotAbi.HOT_FIXED_ACCOUNT_COUNT_V3) throw new Error(`Dealer route manifest requires ${HotAbi.HOT_FIXED_ACCOUNT_COUNT_V3} fixed accounts`);
  key(manifest.payer, 'payer');
  const observation = await acquire(client, [
    ...manifest.fixedAccounts, ...manifest.strategyAccounts, ...manifest.runtimeAccounts,
  ].map((value) => value.address).concat(manifest.lookupTables));
  const fixed = metas(manifest.fixedAccounts, observation.accounts, 'fixed account');
  const strategy = metas(manifest.strategyAccounts, observation.accounts, 'strategy account');
  const runtime = metas(manifest.runtimeAccounts, observation.accounts, 'runtime account');
  const marketAddress = fixed[HotAbi.HOT_MARKET_ACCOUNT_V3]?.address ?? '';
  const rootAddress = fixed[HotAbi.HOT_ROOT_ACCOUNT_V3]?.address ?? '';
  const coreProgram = fixed[HotAbi.HOT_CORE_PROGRAM_ACCOUNT_V3]?.address ?? '';
  const tradingProgram = fixed[HotAbi.HOT_TRADING_PROGRAM_ACCOUNT_V3]?.address ?? '';
  const registryProgram = fixed[HotAbi.HOT_REGISTRY_PROGRAM_ACCOUNT_V3]?.address ?? '';
  const market = required(observation.accounts, marketAddress, 'Market');
  const root = required(observation.accounts, rootAddress, 'Dealer root');
  if (market.owner !== coreProgram || market.executable || market.data.length !== CORE_STATE_BYTES || ascii(market.data, 0, 8) !== 'DCLTCOR2'
      || u16(market.data, 8) !== 2 || market.data[10] !== 1 || root.owner !== tradingProgram || root.executable) {
    throw new Error('Dealer Market/root does not have the exact open Core/Trading ownership state');
  }
  const selection = decodeDirectRootSelectionV1(root.data);
  const releaseSet = slice(market.data, MARKET_RELEASE_SET_OFFSET, 32);
  const generation = u64(market.data, MARKET_GENERATION_OFFSET);
  if (request.market !== marketAddress || request.childRoot !== rootAddress || !same(request.releaseSet, releaseSet) || request.generation !== generation
      || !same(slice(market.data, MARKET_REGISTRY_OFFSET, 32), key(registryProgram, 'Registry program').toBytes())
      || !same(slice(root.data, HotAbi.CAPABILITY_ROOT_RELEASE_SET_OFFSET, 32), releaseSet)
      || !same(slice(root.data, HotAbi.CAPABILITY_ROOT_MARKET_OFFSET, 32), key(marketAddress, 'Market').toBytes())
      || u64(root.data, HotAbi.CAPABILITY_ROOT_GENERATION_OFFSET) !== generation
      || !same(selection.manifest, slice(market.data, MARKET_MANIFEST_OFFSET, 32))
      || !same(selection.kind, await sha256(DEALER_KIND_PREIMAGE_V2))) {
    throw new Error('Dealer request, Market, root, Registry, generation, release, or kind does not join');
  }

  const manifestRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_MANIFEST_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_MANIFEST_STAGING_ACCOUNT_V3]?.address ?? '',
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, slice(market.data, MARKET_MANIFEST_OFFSET, 32), 'capability manifest');
  const manifestEntry = decodeSelectedDirectManifestEntryV1(manifestRaw.data, selection);
  const setRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_PROGRAM_SET_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_PROGRAM_SET_STAGING_ACCOUNT_V3]?.address ?? '',
    HotAbi.CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V1, selection.programSet, 'Dealer ProgramSet');
  const selectedProgram = selectDealerProgram(setRaw.data, request.selector);
  const descriptorRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_DESCRIPTOR_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_DESCRIPTOR_STAGING_ACCOUNT_V3]?.address ?? '',
    HotAbi.DESCRIPTORCONTRACT_SCHEMA_RELEASE_ID, selectedProgram, 'Dealer descriptor');
  const descriptor = await decodeDealerDescriptor(descriptorRaw.data, request);
  if (!same(manifestEntry.capacityProfile, descriptor.capacityProfile) || !same(manifestEntry.childSchema, descriptor.rootSchema)
      || !same(manifestEntry.childDerivation, descriptor.lifecycle)
      || root.data.length !== HotAbi.CAPABILITY_ROOT_HEADER_BYTES_V1 + DEALER_ROOT_TAIL_BYTES_V3) {
    throw new Error('Dealer descriptor, manifest entry, or mutable root width does not join');
  }
  const configRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_CONFIG_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_CONFIG_STAGING_ACCOUNT_V3]?.address ?? '',
    descriptor.configSchema, selection.config, 'Dealer config');
  validateDealerConfigV4(configRaw.data, request);

  const productDigest = slice(market.data, MARKET_PRODUCT_RECORD_OFFSET, 32);
  const productRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_PRODUCT_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_PRODUCT_STAGING_ACCOUNT_V3]?.address ?? '',
    PRODUCT_RECORD_SCHEMA_ID_V2, productDigest, 'Product Runtime V2 root');
  const resultDomainDigest = slice(productRaw.data, 48, 32);
  const portfolioDigest = slice(productRaw.data, 80, 32);
  const domainRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3]?.address ?? '',
    RESULT_DOMAIN_SCHEMA_ID_V2, resultDomainDigest, 'Product result domain');
  const portfolioRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_PORTFOLIO_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_PORTFOLIO_STAGING_ACCOUNT_V3]?.address ?? '',
    PORTFOLIO_SCHEMA_ID_V2, portfolioDigest, 'Product portfolio');
  const graph = decodeCoreFoundProductGraphV2(productRaw.data, domainRaw.data, portfolioRaw.data, resultDomainDigest, portfolioDigest);
  const linkedBasisAccount = required(observation.accounts, fixed[HotAbi.HOT_LINKED_BASIS_RAW_ACCOUNT_V3]?.address ?? '', 'Product basis');
  const linkedBasisRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_LINKED_BASIS_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_LINKED_BASIS_STAGING_ACCOUNT_V3]?.address ?? '',
    HotAbi.GRADED_BASIS_RECORD_SCHEMA_ID_V3, await sha256(linkedBasisAccount.data), 'Product basis');
  const basisWidth = await validateProductBasisV3(linkedBasisRaw.data, graph.productId, resultDomainDigest, domainRaw.data);
  if (basisWidth !== request.width || graph.outcomeCount !== request.width) throw new Error('Dealer request width differs from Product-owned outcome/basis width');

  const profileRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3]?.address ?? '',
    HotAbi.ACCOUNT_SCHEMA_RELEASE_ID, descriptor.accountProfile, 'Dealer AccountProfile');
  const requestProfileRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3]?.address ?? '',
    descriptor.requestProfileSchema, descriptor.requestProfileProgram, 'Dealer RequestProfile');
  await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_LIFECYCLE_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_LIFECYCLE_STAGING_ACCOUNT_V3]?.address ?? '',
    HotAbi.LIFECYCLE_SCHEMA_RELEASE_ID, descriptor.lifecycle, 'Dealer lifecycle');
  const strategyRaw = await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_STRATEGY_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_STRATEGY_STAGING_ACCOUNT_V3]?.address ?? '',
    descriptor.strategySchema, descriptor.strategyProgram, 'Dealer ExecutionStrategy');
  const transitionDigest = slice(strategyRaw.data, 48, 32);
  validateAdmittedStrategy(strategyRaw.data, transitionDigest);
  await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_TRANSITION_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_TRANSITION_STAGING_ACCOUNT_V3]?.address ?? '',
    HotAbi.TRANSITION_SCHEMA_RELEASE_ID, transitionDigest, 'Dealer TransitionVM');
  await finalizedRecord(client, observation.accounts, registryProgram,
    fixed[HotAbi.HOT_EFFECT_RAW_ACCOUNT_V3]?.address ?? '', fixed[HotAbi.HOT_EFFECT_STAGING_ACCOUNT_V3]?.address ?? '',
    HotAbi.EFFECT_SCHEMA_RELEASE_ID, descriptor.effect, 'Dealer EffectProgram');

  const logicalMetas = [
    fixed[HotAbi.HOT_ROOT_ACCOUNT_V3], fixed[HotAbi.HOT_CONFIG_RAW_ACCOUNT_V3], fixed[HotAbi.HOT_PRODUCT_RAW_ACCOUNT_V3],
    fixed[HotAbi.HOT_PORTFOLIO_RAW_ACCOUNT_V3], fixed[HotAbi.HOT_LINKED_BASIS_RAW_ACCOUNT_V3], ...runtime,
  ].filter((value): value is DirectHotAccountMetaV3 => value !== undefined);
  const logicalData = [
    fixed[HotAbi.HOT_ROOT_ACCOUNT_V3], fixed[HotAbi.HOT_CONFIG_RAW_ACCOUNT_V3], fixed[HotAbi.HOT_PRODUCT_RAW_ACCOUNT_V3],
    fixed[HotAbi.HOT_PORTFOLIO_RAW_ACCOUNT_V3], fixed[HotAbi.HOT_LINKED_BASIS_RAW_ACCOUNT_V3], ...runtime,
  ].filter((value): value is DirectHotAccountMetaV3 => value !== undefined)
    .map((value, index) => required(observation.accounts, value.address, `Dealer logical account ${index}`).data);
  validateDealerAccountProfileV3(
    profileRaw.data,
    { kind: 'equity', selector: request.selector },
    request.width,
    logicalMetas,
    logicalData,
  );
  await validateDealerState(request, observation.accounts, tradingProgram, graph.productId, slice(domainRaw.data, 96, 32));

  let checkedOuter: CheckedHotOuterEvidenceV3 = Object.freeze({ status: 'unavailable', reason: 'no checked infrastructure manifest recognizes this Trading release' });
  if (manifest.checkedInfrastructure !== null) {
    const checked = await decodeCheckedInfrastructureV1(manifest.checkedInfrastructure);
    if (checked.execution.releaseSet.id !== hex(releaseSet)) throw new Error('checked infrastructure selects another Market execution release set');
    const trading = checked.execution.artifacts.trading;
    const core = checked.execution.artifacts.core;
    if (trading.upgradeAuthority !== null || core.upgradeAuthority !== null || trading.program !== tradingProgram || core.program !== coreProgram) {
      throw new Error('checked infrastructure does not recognize immutable Core/Trading programs for this route');
    }
    await authenticateArtifactDeploymentV1(required(observation.accounts, tradingProgram, 'Trading program'), tradingProgram,
      required(observation.accounts, fixed[HotAbi.HOT_TRADING_PROGRAMDATA_ACCOUNT_V3]?.address ?? '', 'Trading ProgramData'), fixed[HotAbi.HOT_TRADING_PROGRAMDATA_ACCOUNT_V3]?.address ?? '', trading);
    await authenticateArtifactDeploymentV1(required(observation.accounts, coreProgram, 'Core program'), coreProgram,
      required(observation.accounts, fixed[HotAbi.HOT_CORE_PROGRAMDATA_ACCOUNT_V3]?.address ?? '', 'Core ProgramData'), fixed[HotAbi.HOT_CORE_PROGRAMDATA_ACCOUNT_V3]?.address ?? '', core);
    const cache = required(observation.accounts, fixed[HotAbi.HOT_ACTIVATION_CACHE_ACCOUNT_V3]?.address ?? '', 'activation cache');
    const tradingArtifact = Uint8Array.from((checked.execution.releaseSet.roles.trading.artifactReleaseId.match(/../g) ?? []).map((value) => Number.parseInt(value, 16)));
    if (cache.owner !== registryProgram || cache.executable || ascii(cache.data, 0, 8) !== 'DCLTACT1'
        || !same(slice(cache.data, 16, 32), releaseSet) || !same(slice(cache.data, ACTIVATION_CACHE_TRADING_OFFSET, 32), tradingArtifact)) {
      throw new Error('Registry activation cache does not recognize this Trading release');
    }
    checkedOuter = Object.freeze({ status: 'checked', tradingArtifactRelease: checked.execution.releaseSet.roles.trading.artifactReleaseId, checkedManifestDigest: checked.checkedInfrastructureId });
  }

  const latest = await client.latestBlockhash(observation.slot);
  const lookupTables = Object.freeze(manifest.lookupTables.map((address) => lookupTable(address, required(observation.accounts, address, 'lookup table'))));
  const route: DealerEquityHotRouteV3 = Object.freeze({
    payer: manifest.payer, tradingProgram, market: marketAddress, releaseSet, generation,
    rootPrestateDigest: await sha256(root.data), observedSlot: BigInt(observation.slot), fixedAccounts: fixed,
    strategyAccounts: strategy, runtimeAccounts: runtime, recentBlockhash: latest.blockhash, lookupTables, outerEvidence: checkedOuter,
  });
  return Object.freeze({
    observedSlot: observation.slot, request, route, selectedProgramDigest: hex(selectedProgram),
    accountProfileDigest: hex(await sha256(profileRaw.data)), strategyDigest: hex(await sha256(strategyRaw.data)),
    requestProfileDigest: hex(await sha256(requestProfileRaw.data)), checkedOuter,
  });
}
