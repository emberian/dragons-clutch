import { PublicKey } from '@solana/web3.js';

import { fromHex, hex, sha256 } from './bytes';
import {
  decodeCapabilityManifestV1,
  recognizeCapabilityKindV1,
  type CapabilityActivationV1,
  type CapabilityFundingQuoteV1,
} from './capabilityManifest';
import {
  CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
  CORE_STATE_BYTES,
  CORE_STATE_MAGIC,
  CORE_VERSION,
  REALM_SCHEMA_RELEASE_ID_V1,
} from './generated/coreFound';
import {
  MARKET_HOARD_UNAUTHENTICATED_V1,
  decodeClaimsAggregateV2,
  decodeMarketCoreStateV2,
  deriveClaimsAggregateAddressV2,
  deriveCustodyAuthorityAddressV1,
  deriveMarketCoreAddressV2,
  deriveMarketHoardAddressV1,
  type MarketCorePhaseV2,
  type MarketCoreReadinessV2,
  type MarketCoreSettlementV2,
} from './marketCoreV2';
import { decodeRealmRecordV1, type RealmAuthorityPolicy } from './realmRecord';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';
import { type BindingCheck } from './decoders';

/**
 * Market discovery: a bounded, finalized, chain-derived listing.
 *
 * Every value on a discovery card is decoded from a finalized account this
 * browser read, or the card says `REFUSED` and why. There is no volume, price,
 * odds, probability, APR, or aggregate.
 *
 * Three facts about the real representation shape this module, and each was
 * measured against a live chain rather than assumed:
 *
 *   1. A Market is `DCLTCOR3` — the Lean-emitted 360-byte Core state. It holds
 *      identity, lifecycle, and the authenticated source-derived principal cap.
 *      Older 352-byte devnet Markets are incompatible with this generation.
 *   2. The per-claim SUPPLY vector is not in it. It lives in a Claims-owned
 *      LiabilityBasisV2 aggregate at a PDA derived from the Market, so a card
 *      without a Claims program says its liabilities are UNREAD rather than
 *      showing an empty vector.
 *   3. The Realm is not a Core account. It is a finalized Registry record whose
 *      body hashes to the identity the Market committed to, reacquired and
 *      re-hashed here the same way the capability manifest is.
 *
 *   4. The Hoard is a Custody Vault namespaced by the founding's action
 *      context. That context is not a fact of the Market root and never can be
 *      -- it is caller-chosen -- but the Claims aggregate persists it, so a card
 *      that read the aggregate can name and AUTHENTICATE the Hoard, and a card
 *      that did not says so.
 */

export const MARKET_DISCOVERY_MAX_ADDRESSES = 32;
const RPC_ACCOUNT_BATCH = 32;
const CORE_STATE_MAGIC_TEXT = new TextDecoder().decode(CORE_STATE_MAGIC);

/**
 * One Core Market generation a program scan can see but this reader cannot
 * decode.
 *
 * A generation is (magic, version, width), not a magic: the bump tail widened
 * DCLTCOR3 from 360 to 368 bytes without changing the schema version, so two
 * live widths share one magic and only the account's space tells them apart.
 */
type HistoricalCoreGenerationV1 = Readonly<{
  magic: Uint8Array;
  magicText: string;
  version: number;
  accountBytes: number;
}>;

const HISTORICAL_CORE_GENERATIONS_V1: ReadonlyArray<HistoricalCoreGenerationV1> = (() => {
  if (CORE_VERSION !== 3 || CORE_STATE_BYTES !== 368) {
    throw new Error('historical Core generations must be re-answered for the new generated Core generation');
  }
  const asciiZero = 48;
  const versionDigitOffset = CORE_STATE_MAGIC.length - 1;
  if (CORE_STATE_MAGIC[versionDigitOffset] !== asciiZero + CORE_VERSION) {
    throw new Error('generated Core magic no longer ends with its one-digit schema version');
  }
  const generation = (version: number, accountBytes: number): HistoricalCoreGenerationV1 => {
    const magic = CORE_STATE_MAGIC.slice();
    magic[versionDigitOffset] = asciiZero + version;
    return Object.freeze({ magic, magicText: new TextDecoder().decode(magic), version, accountBytes });
  };
  return Object.freeze([generation(2, 352), generation(3, 360)]);
})();

export type MarketProvenanceV1 =
  | Readonly<{ kind: 'chain'; observedSlot: string }>
  | Readonly<{ kind: 'refused'; reason: string }>;

/** The single provenance chip every discovery surface renders. */
export function provenanceChipV1(provenance: MarketProvenanceV1): string {
  return provenance.kind === 'chain' ? `CHAIN · finalized slot ${provenance.observedSlot}` : 'REFUSED';
}

export function shortAddressV1(address: string, edge = 4): string {
  return address.length <= edge * 2 + 1 ? address : `${address.slice(0, edge)}…${address.slice(-edge)}`;
}

export type MarketCapabilityBadgeV1 = Readonly<{
  index: number;
  kindId: string;
  label: string;
  recognized: boolean;
  programSetId: string;
  configId: string;
  activation: CapabilityActivationV1;
  deadline: string | null;
  dependencies: ReadonlyArray<number>;
  funding: CapabilityFundingQuoteV1;
}>;

export type MarketCapabilityManifestV1 =
  | Readonly<{ status: 'authenticated'; manifestId: string; recordAddress: string; observedSlot: string; badges: ReadonlyArray<MarketCapabilityBadgeV1> }>
  | Readonly<{ status: 'unread'; manifestId: string; reason: string }>
  | Readonly<{ status: 'refused'; manifestId: string; reason: string }>;

export type MarketCollateralV1 =
  | Readonly<{
    status: 'bound';
    observedSlot: string;
    realmAddress: string;
    realmContentId: string;
    collateralMint: string;
    collateralMintShort: string;
    tokenProgram: string;
    adapterReleaseId: string;
    mintAuthorityPolicy: RealmAuthorityPolicy;
    freezeAuthorityPolicy: RealmAuthorityPolicy;
  }>
  | Readonly<{ status: 'refused'; realmAddress: string | null; realmContentId: string; reason: string }>
  | Readonly<{ status: 'unread'; realmContentId: string; reason: string }>;

/** Which supply the collateral must cover at this phase. */
export type RequiredBackingBasisV2 = 'maximum-claim-supply' | 'winning-claim-supply';

/**
 * A Market's liabilities, read where they actually live.
 *
 * `unread` is not `zero`. A card with no Claims program selected has not looked
 * at any liability state and says so; it does not render an empty supply vector,
 * which would read as "this Market has issued nothing".
 */
export type MarketLiabilityV1 =
  | Readonly<{
    status: 'bound';
    observedSlot: string;
    aggregateAddress: string;
    claimsProgramId: string;
    claimCount: number;
    revision: string;
    generation: string;
    liabilityBasisId: string;
    custodyContext: string;
    supplyAtoms: ReadonlyArray<string>;
    requiredBackingAtoms: string;
    requiredBackingBasis: RequiredBackingBasisV2;
  }>
  | Readonly<{ status: 'unread'; reason: string }>
  | Readonly<{ status: 'refused'; aggregateAddress: string | null; reason: string }>;

/**
 * The Market's collateral principal, derived and then authenticated.
 *
 * `derived` is the only status that carries a figure, and it is reached only
 * when the account at the derived Vault address is owned by the Realm's token
 * program, holds the Realm's collateral mint, and is owned by the Market's own
 * context-free Custody transfer authority. Anything short of that is `unread`
 * (a coordinate was not supplied) or `refused` (a coordinate was supplied and
 * did not authenticate) -- never a number.
 */
export type MarketHoardV1 =
  | Readonly<{
    status: 'derived';
    observedSlot: string;
    address: string;
    custodyProgramId: string;
    custodyContext: string;
    custodyAuthority: string;
    collateralMint: string;
    tokenProgram: string;
    principalAtoms: string;
    /**
     * The mint's own `decimals` byte, or null when the mint account was not
     * read or did not authenticate. It is DISPLAY metadata and nothing else:
     * `principalAtoms` is the economic quantity and is never scaled by it.
     */
    mintDisplayDecimals: number | null;
  }>
  | Readonly<{ status: 'unread'; reason: string }>
  | Readonly<{ status: 'refused'; address: string | null; reason: string }>;

/**
 * The immutable identities one Market root commits to, plus the exact artifact
 * profile its account bytes declare.
 */
export type MarketIdentityV1 = Readonly<{
  schemaMagic: string;
  schemaVersion: number;
  accountBytes: number;
  marketId: string;
  realmId: string;
  productRecordId: string;
  productInstanceId: string;
  resolutionPolicyId: string;
  capabilityManifestId: string;
  selectedReleaseSetId: string;
  registryProgram: string;
  rentBeneficiary: string;
}>;

export type MarketDiscoveryCardV1 =
  | Readonly<{
    status: 'decoded';
    address: string;
    provenance: MarketProvenanceV1;
    observedSlot: string;
    phase: MarketCorePhaseV2;
    readiness: MarketCoreReadinessV2;
    generation: string;
    outstandingCapabilities: string;
    principalCapSets: string;
    settlement: MarketCoreSettlementV2;
    identity: MarketIdentityV1;
    collateral: MarketCollateralV1;
    liability: MarketLiabilityV1;
    hoard: MarketHoardV1;
    capabilities: MarketCapabilityManifestV1;
    bindings: ReadonlyArray<BindingCheck>;
    refusal: null;
  }>
  | Readonly<{
    status: 'refused';
    address: string;
    provenance: MarketProvenanceV1;
    observedSlot: string;
    refusal: string;
  }>;

export type IncompatibleMarketAccountV1 = Readonly<{
  address: string;
  magic: string;
  accountBytes: number;
}>;

export type MarketEnumerationV1 =
  | Readonly<{ mode: 'address-list'; note: string; addresses: ReadonlyArray<string> }>
  | Readonly<{
    mode: 'program-scan';
    note: string;
    scanSlot: string;
    addresses: ReadonlyArray<string>;
    scannedAccounts: number;
    incompatibleMarketAccounts: ReadonlyArray<IncompatibleMarketAccountV1>;
  }>
  | Readonly<{ mode: 'refused'; note: string; reason: string; addresses: ReadonlyArray<string> }>;

export type CoreMarketScanEnumerationV1 = Exclude<MarketEnumerationV1, Readonly<{ mode: 'address-list' }>>;

export type MarketDiscoveryV1 = Readonly<{
  coreProgramId: string;
  registryProgramId: string | null;
  claimsProgramId: string | null;
  custodyProgramId: string | null;
  floorSlot: string;
  enumeration: MarketEnumerationV1;
  cards: ReadonlyArray<MarketDiscoveryCardV1>;
  reason: string;
}>;

export type MarketDiscoveryRequestV1 = Readonly<{
  coreProgramId: string;
  registryProgramId?: string | null;
  claimsProgramId?: string | null;
  custodyProgramId?: string | null;
  addresses: ReadonlyArray<string>;
  enumeration?: MarketEnumerationV1;
}>;

function canonical(value: string, field: string): string {
  let key: string;
  try {
    key = new PublicKey(value).toBase58();
  } catch {
    throw new Error(`${field} is not one canonical Solana address`);
  }
  if (key !== value) throw new Error(`${field} must be canonical base58 text`);
  return key;
}

function optional(value: string | null | undefined, field: string): string | null {
  return value === undefined || value === null || value === '' ? null : canonical(value, field);
}

/** Parse a user-entered list of known Market addresses without guessing. */
export function parseMarketAddressListV1(text: string): ReadonlyArray<string> {
  const addresses = text.split(/[\s,]+/).map((entry) => entry.trim()).filter((entry) => entry.length > 0);
  if (addresses.length > MARKET_DISCOVERY_MAX_ADDRESSES) {
    throw new Error(`address list holds ${addresses.length} entries, above the explicit ${MARKET_DISCOVERY_MAX_ADDRESSES}-Market browser bound`);
  }
  const canonicalised = addresses.map((address, index) => canonical(address, `Market address ${index + 1}`));
  if (new Set(canonicalised).size !== canonicalised.length) throw new Error('address list repeats a Market address');
  return Object.freeze(canonicalised);
}

function chunks<T>(values: ReadonlyArray<T>, width: number): T[][] {
  const output: T[][] = [];
  for (let index = 0; index < values.length; index += width) output.push(values.slice(index, index + width));
  return output;
}

/** Does this account carry the live Core Market magic? */
export function isCoreMarketHeaderV2(data: Uint8Array): boolean {
  if (data.length < CORE_STATE_MAGIC.length) return false;
  return CORE_STATE_MAGIC.every((byte, index) => data[index] === byte);
}

/**
 * Is this the current Core Market generation?
 *
 * The magic no longer settles it on its own, so the account's space is part of
 * the answer; a scan that listed on magic alone would offer a historical
 * account as current and fail to decode it.
 */
export function isCurrentCoreMarketAccountV1(account: Pick<RpcAccount, 'data' | 'space'>): boolean {
  return account.space === CORE_STATE_BYTES && isCoreMarketHeaderV2(account.data);
}

function historicalCoreGenerationV1(
  account: Pick<RpcAccount, 'data' | 'space'>,
): HistoricalCoreGenerationV1 | undefined {
  if (account.data.length < 10) return undefined;
  const version = new DataView(account.data.buffer, account.data.byteOffset, account.data.byteLength)
    .getUint16(8, true);
  return HISTORICAL_CORE_GENERATIONS_V1.find((generation) => generation.accountBytes === account.space
    && generation.version === version
    && generation.magic.every((byte, index) => account.data[index] === byte));
}

/** Recognize a historical Core generation without decoding it as current state. */
export function isIncompatibleCoreMarketAccountV1(account: Pick<RpcAccount, 'data' | 'space'>): boolean {
  return historicalCoreGenerationV1(account) !== undefined;
}

/** Name each historical generation a scan actually found, with its count. */
function historicalGenerationsTextV1(accounts: ReadonlyArray<IncompatibleMarketAccountV1>): string {
  return HISTORICAL_CORE_GENERATIONS_V1
    .map((generation) => ({
      generation,
      count: accounts.filter((account) => account.accountBytes === generation.accountBytes
        && account.magic === generation.magicText).length,
    }))
    .filter((entry) => entry.count > 0)
    .map((entry) => `${entry.count} ${entry.generation.magicText} at ${entry.generation.accountBytes} bytes`)
    .join(', ');
}

type EnumerationClient = Pick<SolanaRpcClient, 'programHeaders'>;

/**
 * Enumerate Market addresses from the Core program itself.
 *
 * `getProgramAccounts` is neither cheap nor universally available; a provider
 * that disables or bounds it is a refusal with its exact reason, not an empty
 * Market list. Callers fall back to an explicit address list.
 */
export async function enumerateCoreMarketAddressesV1(client: EnumerationClient, coreProgramId: string): Promise<CoreMarketScanEnumerationV1> {
  const program = canonical(coreProgramId, 'Core program');
  try {
    const scan = await client.programHeaders(program);
    const addresses = scan.accounts
      .filter((entry) => isCurrentCoreMarketAccountV1(entry.account))
      .map((entry) => entry.address)
      .sort((left, right) => left.localeCompare(right));
    const incompatibleMarketAccounts = scan.accounts
      .flatMap((entry) => {
        const generation = historicalCoreGenerationV1(entry.account);
        return generation === undefined ? [] : [Object.freeze({
          address: entry.address,
          magic: generation.magicText,
          accountBytes: generation.accountBytes,
        })];
      })
      .sort((left, right) => left.address.localeCompare(right.address));
    const bounded = addresses.slice(0, MARKET_DISCOVERY_MAX_ADDRESSES);
    const dropped = addresses.length - bounded.length;
    const incompatibleNote = incompatibleMarketAccounts.length === 0
      ? ''
      : ` The same scan found ${incompatibleMarketAccounts.length} historical Market account${incompatibleMarketAccounts.length === 1 ? '' : 's'} (${historicalGenerationsTextV1(incompatibleMarketAccounts)}); ${incompatibleMarketAccounts.length === 1 ? 'it is' : 'they are'} not listed as current.`;
    return Object.freeze({
      mode: 'program-scan',
      scanSlot: scan.slot,
      addresses: Object.freeze(bounded),
      scannedAccounts: scan.accounts.length,
      incompatibleMarketAccounts: Object.freeze(incompatibleMarketAccounts),
      note: `getProgramAccounts returned ${scan.accounts.length} finalized Core accounts at slot ${scan.slot}; ${addresses.length} carry the current ${CORE_STATE_MAGIC_TEXT} Market header${dropped > 0 ? `, of which ${dropped} exceed the ${MARKET_DISCOVERY_MAX_ADDRESSES}-Market listing bound and were not read` : ''}.${incompatibleNote}`,
    });
  } catch (error) {
    return Object.freeze({
      mode: 'refused',
      addresses: Object.freeze([]),
      reason: error instanceof Error ? error.message : 'program enumeration refused without a usable reason',
      note: 'This endpoint did not serve a bounded finalized getProgramAccounts scan. Enumerate from known Market addresses instead; dClutch has no index and this browser will not invent one.',
    });
  }
}

type DiscoveryClient = Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>;

type ObservedAccountV1 = Readonly<{ account: RpcAccount | null; slot: string }>;

/** One batched read the endpoint refused, held per address it was asked for. */
type UnreadAccountV1 = Readonly<{ failure: string }>;

type PrefetchedAccountsV1 = ReadonlyMap<string, ObservedAccountV1 | UnreadAccountV1>;

async function readAccounts(
  client: DiscoveryClient,
  addresses: ReadonlyArray<string>,
  floor: string,
): Promise<ReadonlyMap<string, ObservedAccountV1>> {
  const observed = new Map<string, ObservedAccountV1>();
  for (const group of chunks(addresses, RPC_ACCOUNT_BATCH)) {
    const batch = await client.multipleAccounts(group, floor);
    for (const entry of batch.accounts) observed.set(entry.address, Object.freeze({ account: entry.account, slot: batch.slot }));
  }
  return observed;
}

/**
 * Read one whole round of companion accounts in `RPC_ACCOUNT_BATCH` chunks.
 *
 * The join below used to spend a one- or two-address `getMultipleAccounts` per
 * Market per record, inside a sequential loop: ten Markets cost about forty
 * round trips against endpoints whose burst allowance is far smaller. The reads
 * are the same reads; they are now collected first and asked for together.
 *
 * A chunk the endpoint refuses does not sink the listing. Each address in it
 * carries that refusal forward, so the per-Market helper that asked for the
 * account states exactly the refusal it stated when it owned the call itself.
 */
async function prefetchRound(
  client: DiscoveryClient,
  addresses: ReadonlyArray<string>,
  floor: string,
): Promise<PrefetchedAccountsV1> {
  const observed = new Map<string, ObservedAccountV1 | UnreadAccountV1>();
  for (const group of chunks([...new Set(addresses)], RPC_ACCOUNT_BATCH)) {
    try {
      for (const [address, entry] of await readAccounts(client, group, floor)) observed.set(address, entry);
    } catch (error) {
      const failure = error instanceof Error ? error.message : 'this batched finalized account read was refused without a usable reason';
      for (const address of group) observed.set(address, Object.freeze({ failure }));
    }
  }
  return observed;
}

/** Take one prefetched account, or throw the refusal standing in its place. */
function prefetchedAccount(accounts: PrefetchedAccountsV1, address: string, field: string): ObservedAccountV1 {
  const entry = accounts.get(address);
  if (entry === undefined) throw new Error(`${field} ${address} was not collected into this listing's batched finalized read`);
  if ('failure' in entry) throw new Error(entry.failure);
  return entry;
}

/**
 * Collect the addresses one round will read, without deciding what they mean.
 *
 * Collection is best-effort by design: a derivation that refuses here is simply
 * not collected, and the helper that owns the refusal derives the same address
 * again and states it. Derivation is deterministic, so the only reads this can
 * miss are reads whose helper is about to refuse for the same reason.
 */
function collectAddresses(into: string[], derive: () => ReadonlyArray<string>): void {
  try {
    into.push(...derive());
  } catch {
    return;
  }
}

/**
 * Reacquire one finalized Registry record by the content identity that names it.
 *
 * The record must exist at its derived PDA, be Registry-owned, have a vacant
 * staging cursor, and hash to the identity that asked for it. Anything else is a
 * refusal carrying its exact reason.
 */
function finalizedRecordAddresses(
  registryProgramId: string,
  schema: Uint8Array,
  identityHex: string,
  field: string,
): Readonly<{ record: string; staging: string }> {
  return deriveFinalizedRecordAddressesV1(registryProgramId, schema, fromHex(identityHex, `${field} identity`));
}

async function finalizedRecordBody(
  accounts: PrefetchedAccountsV1,
  registryProgramId: string,
  schema: Uint8Array,
  identityHex: string,
  field: string,
): Promise<Readonly<{ address: string; data: Uint8Array; observedSlot: string }>> {
  const addresses = finalizedRecordAddresses(registryProgramId, schema, identityHex, field);
  const observed = prefetchedAccount(accounts, addresses.record, `${field} record`);
  const record = observed.account;
  const staging = prefetchedAccount(accounts, addresses.staging, `${field} staging cursor`).account;
  if (record === null) throw new Error(`${field} record ${addresses.record} is absent at finalized slot ${observed.slot}`);
  if (record.owner !== registryProgramId || record.executable) throw new Error(`${field} record is not Registry-owned finalized data`);
  if (staging !== null) throw new Error(`${field} staging cursor is still present; the record is not canonically final`);
  const observedDigest = hex(await sha256(record.data));
  if (observedDigest !== identityHex) throw new Error(`${field} bytes differ from the identity the Market committed to`);
  return Object.freeze({ address: addresses.record, data: record.data, observedSlot: observed.slot });
}

async function authenticateCapabilityManifest(
  accounts: PrefetchedAccountsV1,
  registryProgramId: string,
  manifestIdHex: string,
): Promise<MarketCapabilityManifestV1> {
  try {
    const record = await finalizedRecordBody(accounts, registryProgramId, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, manifestIdHex, 'capability manifest');
    const badges = decodeCapabilityManifestV1(record.data).map((entry) => {
      const kindId = hex(entry.kind);
      const recognized = recognizeCapabilityKindV1(entry.kind);
      return Object.freeze({
        index: entry.index,
        kindId,
        label: recognized ?? `unrecognized kind ${kindId.slice(0, 12)}…`,
        recognized: recognized !== null,
        programSetId: hex(entry.programSet),
        configId: hex(entry.config),
        activation: entry.activation,
        deadline: entry.activation === 'deadline' ? entry.deadline.toString() : null,
        dependencies: entry.dependencies,
        funding: entry.funding,
      });
    });
    return Object.freeze({ status: 'authenticated', manifestId: manifestIdHex, recordAddress: record.address, observedSlot: record.observedSlot, badges: Object.freeze(badges) });
  } catch (error) {
    return Object.freeze({
      status: 'refused',
      manifestId: manifestIdHex,
      reason: error instanceof Error ? error.message : 'capability manifest refused without a usable reason',
    });
  }
}

/**
 * The collateral binding, authenticated from the Registry.
 *
 * A Market names its Realm by content identity. On a live chain the canonical
 * Realm body is a finalized Registry record, not a Core account, so the Registry
 * program is required to read it — without one the card says the collateral is
 * UNREAD rather than claiming it is unbound.
 */
async function authenticateCollateral(
  accounts: PrefetchedAccountsV1,
  registryProgramId: string | null,
  realmContentId: string,
): Promise<MarketCollateralV1> {
  if (registryProgramId === null) {
    return Object.freeze({
      status: 'unread',
      realmContentId,
      reason: 'No Registry program was selected, so this Market\'s Realm record was not reacquired and no collateral binding may be asserted from the Market root alone.',
    });
  }
  try {
    const record = await finalizedRecordBody(accounts, registryProgramId, REALM_SCHEMA_RELEASE_ID_V1, realmContentId, 'Realm');
    const realm = decodeRealmRecordV1(record.data);
    return Object.freeze({
      status: 'bound',
      observedSlot: record.observedSlot,
      realmAddress: record.address,
      realmContentId,
      collateralMint: realm.collateralMint,
      collateralMintShort: shortAddressV1(realm.collateralMint),
      tokenProgram: realm.tokenProgram,
      adapterReleaseId: realm.adapterReleaseId,
      mintAuthorityPolicy: realm.mintAuthorityPolicy,
      freezeAuthorityPolicy: realm.freezeAuthorityPolicy,
    });
  } catch (error) {
    return Object.freeze({
      status: 'refused',
      realmAddress: null,
      realmContentId,
      reason: error instanceof Error ? error.message : 'Realm record refused without a usable reason',
    });
  }
}

/** Read one Market's liability state from the Claims program that owns it. */
async function readLiability(
  accounts: PrefetchedAccountsV1,
  claimsProgramId: string | null,
  marketAddress: string,
  marketGeneration: string,
  settled: boolean,
): Promise<MarketLiabilityV1> {
  if (claimsProgramId === null) {
    return Object.freeze({
      status: 'unread',
      reason: 'No Claims program was selected. A Market root carries no supply vector, so nothing about issued liabilities is asserted here — this is an unread section, not a Market with no claims.',
    });
  }
  let aggregateAddress: string | null = null;
  try {
    aggregateAddress = deriveClaimsAggregateAddressV2(claimsProgramId, marketAddress);
    const observed = prefetchedAccount(accounts, aggregateAddress, 'Claims LiabilityBasisV2 aggregate');
    const account = observed.account;
    if (account === null) throw new Error(`no Claims LiabilityBasisV2 aggregate exists at ${aggregateAddress} at finalized slot ${observed.slot}`);
    if (account.owner !== claimsProgramId || account.executable) throw new Error('the derived aggregate address holds an account the selected Claims program does not own');
    const aggregate = decodeClaimsAggregateV2(aggregateAddress, account.data);
    if (aggregate.logicalMarket !== marketAddress) throw new Error(`the aggregate names Market ${aggregate.logicalMarket}, not ${marketAddress}`);
    if (aggregate.generation !== marketGeneration) throw new Error(`the aggregate is at generation ${aggregate.generation} while the Market is at ${marketGeneration}; these are two incarnations and are not shown as one`);
    return Object.freeze({
      status: 'bound',
      observedSlot: observed.slot,
      aggregateAddress,
      claimsProgramId,
      claimCount: aggregate.claimCount,
      revision: aggregate.revision,
      generation: aggregate.generation,
      liabilityBasisId: aggregate.liabilityBasisId,
      custodyContext: aggregate.custodyContext,
      supplyAtoms: aggregate.supplyAtoms,
      requiredBackingAtoms: aggregate.maximumSupplyAtoms,
      requiredBackingBasis: settled ? 'winning-claim-supply' : 'maximum-claim-supply',
    });
  } catch (error) {
    return Object.freeze({
      status: 'refused',
      aggregateAddress,
      reason: error instanceof Error ? error.message : 'Claims liability state refused without a usable reason',
    });
  }
}

/** Exact base SPL Token account width, shared by both token programs. */
const TOKEN_ACCOUNT_BYTES_V1 = 165;

/** Exact base SPL Mint width, shared by both token programs. */
const MINT_ACCOUNT_BYTES_V1 = 82;

/** The Token-2022 account-type discriminator that follows a base account. */
const TOKEN_2022_ACCOUNT_TYPE_OFFSET_V1 = 165;
const TOKEN_2022_ACCOUNT_TYPE_MINT_V1 = 1;

/**
 * Read a collateral mint's `decimals` byte, or refuse to guess one.
 *
 * The mint joins the Hoard's own batched round at no extra round trip, because
 * the Realm names it before that round is collected. It is read for one reason:
 * a reader looking at `500000000` deserves to know the token counts in
 * millionths. It is never used to scale a quantity -- this protocol's economics
 * are raw u64 everywhere, and a display byte an arbitrary mint authority chose
 * may not touch them.
 *
 * A base mint is exactly 82 bytes. A Token-2022 mint carrying extensions is
 * longer and repeats the base layout, but so is a 165-byte token account, so
 * anything past the base width must also name itself a Mint at the account-type
 * byte before its decimals are believed.
 */
function readMintDisplayDecimalsV1(
  accounts: PrefetchedAccountsV1,
  mint: string,
  tokenProgram: string,
): number | null {
  const entry = accounts.get(mint);
  if (entry === undefined || 'failure' in entry) return null;
  const account = entry.account;
  if (account === null || account.executable || account.owner !== tokenProgram) return null;
  const bytes = account.data;
  if (bytes.length < MINT_ACCOUNT_BYTES_V1) return null;
  if (bytes.length !== MINT_ACCOUNT_BYTES_V1) {
    if (bytes.length <= TOKEN_2022_ACCOUNT_TYPE_OFFSET_V1) return null;
    if (bytes[TOKEN_2022_ACCOUNT_TYPE_OFFSET_V1] !== TOKEN_2022_ACCOUNT_TYPE_MINT_V1) return null;
  }
  if (bytes[45] !== 1) return null;
  return bytes[44] ?? null;
}

/**
 * Read one Market's Hoard, at the coordinate the chain itself derives.
 *
 * Four facts have to be in hand and each comes from a different authority: the
 * Market address and its selected release set from the Core root, the Custody
 * namespace from the Claims aggregate, and the collateral mint and token
 * program from the finalized Realm record. Missing any one of them is `unread`.
 *
 * Then the account is authenticated rather than assumed. The Vault seeds pin
 * `market` and `release_set` either side of the namespace, so a substituted
 * context can only ever name a compartment of THIS Market -- but a compartment
 * the founding never funded is still not a Hoard. The checks below are the ones
 * `claims-sbf` runs on the same account before it will move a payout: exact
 * width, the Realm's token program as account owner, the Realm's collateral
 * mint, the context-free Custody transfer authority as token owner, initialized,
 * and no delegate, native reserve, or close authority.
 */
async function readHoard(
  accounts: PrefetchedAccountsV1,
  custodyProgramId: string | null,
  marketAddress: string,
  selectedReleaseSetId: string,
  collateral: MarketCollateralV1,
  liability: MarketLiabilityV1,
): Promise<MarketHoardV1> {
  if (custodyProgramId === null) {
    return Object.freeze({
      status: 'unread',
      reason: 'No Custody program was selected. The Hoard is a Custody Vault, so without the program that owns it there is no address to derive and no authority to check.',
    });
  }
  if (liability.status !== 'bound') {
    return Object.freeze({
      status: 'unread',
      reason: 'The Claims aggregate is the only account that records this Market\'s Custody namespace, and it was not read. The founding chooses that namespace; it is not a function of the Market address.',
    });
  }
  if (collateral.status !== 'bound') {
    return Object.freeze({
      status: 'unread',
      reason: 'The Realm record names the collateral mint and token program a Hoard must hold, and it was not authenticated. An unauthenticated token account is not shown as a principal.',
    });
  }
  let address: string | null = null;
  try {
    address = deriveMarketHoardAddressV1(custodyProgramId, marketAddress, selectedReleaseSetId, liability.custodyContext);
    const authority = deriveCustodyAuthorityAddressV1(custodyProgramId, marketAddress, selectedReleaseSetId);
    const observed = prefetchedAccount(accounts, address, 'the derived Hoard Vault');
    const account = observed.account;
    if (account === null) throw new Error(`no account exists at the derived Hoard Vault ${address} at finalized slot ${observed.slot}`);
    if (account.executable) throw new Error('the derived Hoard Vault address holds an executable account');
    if (account.owner !== collateral.tokenProgram) throw new Error(`the derived Hoard Vault is owned by ${account.owner}, not the Realm's token program ${collateral.tokenProgram}`);
    const bytes = account.data;
    if (bytes.length < TOKEN_ACCOUNT_BYTES_V1) throw new Error(`the derived Hoard Vault is ${bytes.length} bytes; a Token account is at least ${TOKEN_ACCOUNT_BYTES_V1}`);
    const mint = new PublicKey(bytes.slice(0, 32)).toBase58();
    const owner = new PublicKey(bytes.slice(32, 64)).toBase58();
    if (mint !== collateral.collateralMint) throw new Error(`the derived Hoard Vault holds mint ${mint}, not the Realm's collateral mint ${collateral.collateralMint}`);
    if (owner !== authority) throw new Error(`the derived Hoard Vault is owned by ${owner}, not this Market's Custody transfer authority ${authority}`);
    if (bytes[108] !== 1) throw new Error(bytes[108] === 2 ? 'the derived Hoard Vault is frozen' : 'the derived Hoard Vault is not initialized');
    if (readU32(bytes, 72) !== 0 || readU64(bytes, 121) !== 0n) throw new Error('the derived Hoard Vault has a delegate');
    if (readU32(bytes, 109) !== 0) throw new Error('the derived Hoard Vault has a native reserve');
    if (readU32(bytes, 129) !== 0) throw new Error('the derived Hoard Vault has a separate close authority');
    return Object.freeze({
      status: 'derived',
      observedSlot: observed.slot,
      address,
      custodyProgramId,
      custodyContext: liability.custodyContext,
      custodyAuthority: authority,
      collateralMint: mint,
      tokenProgram: collateral.tokenProgram,
      principalAtoms: readU64(bytes, 64).toString(),
      mintDisplayDecimals: readMintDisplayDecimalsV1(accounts, mint, collateral.tokenProgram),
    });
  } catch (error) {
    return Object.freeze({
      status: 'refused',
      address,
      reason: error instanceof Error ? error.message : MARKET_HOARD_UNAUTHENTICATED_V1,
    });
  }
}

function readU32(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(offset, true);
}

function readU64(bytes: Uint8Array, offset: number): bigint {
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getBigUint64(offset, true);
}

type DecodedMarketCoreStateV2 = ReturnType<typeof decodeMarketCoreStateV2>;

type WalkedMarketV1 =
  | Readonly<{ kind: 'refused'; card: MarketDiscoveryCardV1 }>
  | Readonly<{
    kind: 'decoded';
    address: string;
    observedSlot: string;
    state: DecodedMarketCoreStateV2;
    bindings: ReadonlyArray<BindingCheck>;
  }>;

function refusedCard(address: string, observedSlot: string, reason: string): MarketDiscoveryCardV1 {
  return Object.freeze({
    status: 'refused',
    address,
    provenance: Object.freeze({ kind: 'refused', reason }),
    observedSlot,
    refusal: reason,
  });
}

/**
 * Decode one Market root, or produce the refusal card that stands for it.
 *
 * This is the whole of what the Market account itself can settle, and it reads
 * nothing further: the addresses of a Market's companion records are known only
 * once its identities are decoded, which is why the join below is staged.
 */
function walkMarketRoot(
  coreProgramId: string,
  registryProgramId: string | null,
  address: string,
  entry: ObservedAccountV1 | undefined,
  floor: string,
): WalkedMarketV1 {
  if (entry === undefined || entry.account === null) {
    const reason = 'account is absent at the finalized observation floor';
    return Object.freeze({ kind: 'refused', card: refusedCard(address, entry?.slot ?? floor, reason) });
  }
  if (entry.account.owner !== coreProgramId || entry.account.executable) {
    const reason = 'account owner differs from the selected Core program, or it is executable program data';
    return Object.freeze({ kind: 'refused', card: refusedCard(address, entry.slot, reason) });
  }
  let state: DecodedMarketCoreStateV2;
  try {
    state = decodeMarketCoreStateV2(address, entry.account.data);
  } catch (error) {
    const reason = error instanceof Error ? error.message : 'the account did not decode as one canonical Core Market state';
    return Object.freeze({ kind: 'refused', card: refusedCard(address, entry.slot, reason) });
  }

  // The account must derive the address it was found at, from the identities
  // it declares. `market_id` is not one of those seeds, so this is a real
  // check and not a restatement.
  const derived = deriveMarketCoreAddressV2(coreProgramId, entry.account.data);
  const bindings: ReadonlyArray<BindingCheck> = Object.freeze([
    Object.freeze({ label: 'Market PDA', ok: derived === address, detail: `identity seeds + generation ${state.identity.generation} → ${derived}` }),
    Object.freeze({ label: 'Market self-identity', ok: state.marketId === address, detail: `state names ${state.marketId}` }),
    Object.freeze({
      label: 'Registry authority',
      ok: registryProgramId === null || state.identity.registryProgram === registryProgramId,
      detail: registryProgramId === null ? `Market selects Registry ${state.identity.registryProgram}; none was selected here` : `Market selects ${state.identity.registryProgram}`,
    }),
  ]);
  return Object.freeze({ kind: 'decoded', address, observedSlot: entry.slot, state, bindings });
}

/**
 * Read one finalized Market discovery listing.
 *
 * All Markets, their Realm records, their liability aggregates and their
 * capability manifests are read behind a single finalized floor slot so no card
 * mixes observation epochs.
 *
 * The reads are staged into three rounds rather than a join per Market, because
 * each round's addresses are only derivable from the previous round's decode: a
 * Market root names its Realm and manifest, and only the Claims aggregate
 * records the Custody namespace a Hoard Vault sits under. Within a round every
 * address is read together in `RPC_ACCOUNT_BATCH` chunks. Three rounds is the
 * floor this data dependency imposes; it is not a budget choice.
 */
export async function inspectMarketDiscoveryV1(
  client: DiscoveryClient,
  request: MarketDiscoveryRequestV1,
): Promise<MarketDiscoveryV1> {
  const coreProgramId = canonical(request.coreProgramId, 'Core program');
  const registryProgramId = optional(request.registryProgramId, 'Registry program');
  const claimsProgramId = optional(request.claimsProgramId, 'Claims program');
  const custodyProgramId = optional(request.custodyProgramId, 'Custody program');
  const addresses = Object.freeze([...new Set(request.addresses.map((address, index) => canonical(address, `Market address ${index + 1}`)))]);
  if (addresses.length > MARKET_DISCOVERY_MAX_ADDRESSES) {
    throw new Error(`discovery requested ${addresses.length} Markets, above the explicit ${MARKET_DISCOVERY_MAX_ADDRESSES}-Market browser bound`);
  }
  const enumeration: MarketEnumerationV1 = request.enumeration ?? Object.freeze({
    mode: 'address-list',
    addresses,
    note: 'Markets were enumerated from explicitly supplied addresses. dClutch publishes no index and this browser will not synthesize one.',
  });
  const floor = await client.finalizedSlot();
  if (addresses.length === 0) {
    const incompatibleCount = enumeration.mode === 'program-scan'
      ? enumeration.incompatibleMarketAccounts.length
      : 0;
    return Object.freeze({
      coreProgramId,
      registryProgramId,
      claimsProgramId,
      custodyProgramId,
      floorSlot: floor,
      enumeration,
      cards: Object.freeze([]),
      reason: incompatibleCount === 0
        ? 'No current compatible Market address has been supplied or enumerated at this finalized floor.'
        : `No current compatible Market is listed at this finalized floor. The scan also found ${incompatibleCount} historical Core Market account${incompatibleCount === 1 ? '' : 's'} that the current reader cannot decode.`,
    });
  }

  // ROUND ONE: the Market roots themselves.
  const observed = await readAccounts(client, addresses, floor);
  const walked = addresses.map((address) => walkMarketRoot(coreProgramId, registryProgramId, address, observed.get(address), floor));

  // ROUND TWO: every companion record the decoded Markets name. A Realm record
  // and its staging cursor, a capability manifest record and its staging
  // cursor, and a Claims aggregate — collected across all Markets first, then
  // read in 32-address chunks.
  const recordRound: string[] = [];
  for (const market of walked) {
    if (market.kind !== 'decoded') continue;
    if (registryProgramId !== null) {
      collectAddresses(recordRound, () => {
        const realm = finalizedRecordAddresses(registryProgramId, REALM_SCHEMA_RELEASE_ID_V1, market.state.identity.realmId, 'Realm');
        return [realm.record, realm.staging];
      });
      collectAddresses(recordRound, () => {
        const manifest = finalizedRecordAddresses(registryProgramId, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, market.state.identity.capabilityManifestId, 'capability manifest');
        return [manifest.record, manifest.staging];
      });
    }
    if (claimsProgramId !== null) {
      collectAddresses(recordRound, () => [deriveClaimsAggregateAddressV2(claimsProgramId, market.address)]);
    }
  }
  const records = await prefetchRound(client, recordRound, floor);

  // ROUND THREE: the Hoard Vaults, which cannot join round two. A Vault's
  // address is namespaced by the Custody context only the Claims aggregate
  // carries, so it is not derivable until round two has been decoded.
  const hoardRound: string[] = [];
  const companions: Array<Readonly<{
    collateral: MarketCollateralV1;
    liability: MarketLiabilityV1;
    capabilities: MarketCapabilityManifestV1;
  }> | null> = [];
  for (const market of walked) {
    if (market.kind !== 'decoded') {
      companions.push(null);
      continue;
    }
    const collateral = await authenticateCollateral(records, registryProgramId, market.state.identity.realmId);
    const liability = await readLiability(records, claimsProgramId, market.address, market.state.identity.generation, market.state.settlement.status === 'terminal');
    const capabilities: MarketCapabilityManifestV1 = registryProgramId === null
      ? Object.freeze({
        status: 'unread',
        manifestId: market.state.identity.capabilityManifestId,
        reason: 'No Registry program was selected, so this Market\'s capability manifest was not authenticated. No capability may be asserted from the Market root alone.',
      })
      : await authenticateCapabilityManifest(records, registryProgramId, market.state.identity.capabilityManifestId);
    if (custodyProgramId !== null && collateral.status === 'bound' && liability.status === 'bound') {
      collectAddresses(hoardRound, () => [deriveMarketHoardAddressV1(custodyProgramId, market.address, market.state.identity.selectedReleaseSetId, liability.custodyContext)]);
      // The mint rides this round rather than earning one of its own: the
      // Realm named it in round two, so it costs no extra trip.
      collectAddresses(hoardRound, () => [collateral.collateralMint]);
    }
    companions.push(Object.freeze({ collateral, liability, capabilities }));
  }
  const hoards = await prefetchRound(client, hoardRound, floor);

  const cards: MarketDiscoveryCardV1[] = [];
  for (const [index, market] of walked.entries()) {
    const companion = companions[index];
    if (market.kind !== 'decoded' || companion === null || companion === undefined) {
      cards.push(market.kind === 'refused'
        ? market.card
        : refusedCard(market.address, market.observedSlot, 'this Market was walked but never joined to its companion records'));
      continue;
    }
    const { collateral, liability, capabilities } = companion;
    const state = market.state;
    const hoard = await readHoard(hoards, custodyProgramId, market.address, state.identity.selectedReleaseSetId, collateral, liability);

    cards.push(Object.freeze({
      status: 'decoded',
      address: market.address,
      provenance: Object.freeze({ kind: 'chain', observedSlot: market.observedSlot }),
      observedSlot: market.observedSlot,
      phase: state.phase,
      readiness: state.readiness,
      generation: state.identity.generation,
      outstandingCapabilities: state.outstandingCapabilities,
      principalCapSets: state.principalCapSets,
      settlement: state.settlement,
      identity: Object.freeze({
        schemaMagic: CORE_STATE_MAGIC_TEXT,
        schemaVersion: state.version,
        accountBytes: state.accountBytes,
        marketId: state.marketId,
        realmId: state.identity.realmId,
        productRecordId: state.identity.productRecordId,
        productInstanceId: state.identity.productInstanceId,
        resolutionPolicyId: state.identity.resolutionPolicyId,
        capabilityManifestId: state.identity.capabilityManifestId,
        selectedReleaseSetId: state.identity.selectedReleaseSetId,
        registryProgram: state.identity.registryProgram,
        rentBeneficiary: state.rentBeneficiary,
      }),
      collateral,
      liability,
      hoard,
      capabilities,
      bindings: market.bindings,
      refusal: null,
    }));
  }

  const decoded = cards.filter((card) => card.status === 'decoded').length;
  return Object.freeze({
    coreProgramId,
    registryProgramId,
    claimsProgramId,
    custodyProgramId,
    floorSlot: floor,
    enumeration,
    cards: Object.freeze(cards),
    reason: `${decoded} of ${cards.length} requested Market${cards.length === 1 ? '' : 's'} decoded at finalized floor ${floor}.`,
  });
}

/**
 * ---------------------------------------------------------------------------
 * Curation: the same cards, arranged the way a reader meets them.
 * ---------------------------------------------------------------------------
 *
 * Enumeration order is the chain's order, which is no order at all -- a Market
 * founded in an abandoned build-out sits between the two anyone came here to
 * see. Nothing below decides what is TRUE; every card is already decoded or
 * refused before it gets here. It decides only what a reader meets first.
 *
 * The rule is that the groups partition the listing exactly. Nothing is
 * dropped, nothing is counted twice, and the group a market lands in is a
 * function of its own phase -- so a founding attempt cannot be quietly promoted
 * into the headline, and a market that opened cannot be quietly buried.
 */

export type DecodedMarketDiscoveryCardV1 = Extract<MarketDiscoveryCardV1, Readonly<{ status: 'decoded' }>>;

export type MarketListingV1 = Readonly<{
  /** Founding is finished and the Market is live: phase `Open`. */
  open: ReadonlyArray<DecodedMarketDiscoveryCardV1>;
  /** The Market has reached or is unwinding from its answer. */
  settled: ReadonlyArray<DecodedMarketDiscoveryCardV1>;
  /** Founding was started and has not finished: phase `Founding`. */
  founding: ReadonlyArray<DecodedMarketDiscoveryCardV1>;
  /** The account exists and this reader could not decode it. */
  unreadable: ReadonlyArray<MarketDiscoveryCardV1>;
}>;

/**
 * Split a listing into the four groups, featured Market first among the open.
 *
 * `featured` is the address a deployment has published as its own headline
 * Market. It only ever REORDERS: a featured address that is not open, or not in
 * the listing at all, changes nothing, because promoting it would be the page
 * asserting a phase the chain did not.
 */
export function curateMarketListingV1(
  cards: ReadonlyArray<MarketDiscoveryCardV1>,
  featured: string | null = null,
): MarketListingV1 {
  const open: DecodedMarketDiscoveryCardV1[] = [];
  const settled: DecodedMarketDiscoveryCardV1[] = [];
  const founding: DecodedMarketDiscoveryCardV1[] = [];
  const unreadable: MarketDiscoveryCardV1[] = [];
  for (const card of cards) {
    if (card.status !== 'decoded') { unreadable.push(card); continue; }
    if (card.phase === 'Open') { open.push(card); continue; }
    if (card.phase === 'Founding') { founding.push(card); continue; }
    settled.push(card);
  }
  if (featured !== null) {
    const index = open.findIndex((card) => card.address === featured);
    if (index > 0) open.unshift(...open.splice(index, 1));
  }
  return Object.freeze({
    open: Object.freeze(open),
    settled: Object.freeze(settled),
    founding: Object.freeze(founding),
    unreadable: Object.freeze(unreadable),
  });
}

/**
 * One collateral token's total across every Hoard this listing authenticated.
 *
 * `principalAtoms` is the exact sum of the raw u64 balances of vaults holding
 * THIS mint, and of nothing else. `mintDisplayDecimals` is the mint's own
 * display byte when it was read; it is metadata beside the total, never a
 * scaling factor applied to it.
 */
export type CollateralSubtotalV1 = Readonly<{
  collateralMint: string;
  collateralMintShort: string;
  principalAtoms: string;
  vaults: number;
  mintDisplayDecimals: number | null;
}>;

/**
 * Total the authenticated collateral per token, and never across tokens.
 *
 * Atoms of different mints are different physical dimensions. A single figure
 * spanning two of them is not a rounding compromise, it is a category error --
 * so this returns one row per mint and leaves the reader to see that there are
 * two of them, rather than collapsing them into a number or into a dash.
 *
 * Only `derived` Hoards contribute. A vault that refused authentication may
 * still hold principal; it is absent from these rows and its absence is the
 * caller's to disclose, because a refused read is not a zero.
 */
export function collateralSubtotalsV1(
  cards: ReadonlyArray<MarketDiscoveryCardV1>,
): ReadonlyArray<CollateralSubtotalV1> {
  const totals = new Map<string, Readonly<{ atoms: bigint; vaults: number; decimals: number | null }>>();
  for (const card of cards) {
    if (card.status !== 'decoded' || card.hoard.status !== 'derived') continue;
    const { collateralMint, principalAtoms, mintDisplayDecimals } = card.hoard;
    const running = totals.get(collateralMint);
    totals.set(collateralMint, Object.freeze({
      atoms: (running?.atoms ?? 0n) + BigInt(principalAtoms),
      vaults: (running?.vaults ?? 0) + 1,
      // One mint has one decimals byte. If two reads of it disagree, neither is
      // trusted rather than one being picked.
      decimals: running === undefined || running.decimals === mintDisplayDecimals ? mintDisplayDecimals : null,
    }));
  }
  return Object.freeze([...totals.entries()]
    .map(([collateralMint, total]) => Object.freeze({
      collateralMint,
      collateralMintShort: shortAddressV1(collateralMint, 5),
      principalAtoms: total.atoms.toString(),
      vaults: total.vaults,
      mintDisplayDecimals: total.decimals,
    }))
    .sort((left, right) => {
      const leftAtoms = BigInt(left.principalAtoms);
      const rightAtoms = BigInt(right.principalAtoms);
      if (leftAtoms !== rightAtoms) return rightAtoms > leftAtoms ? 1 : -1;
      return left.collateralMint < right.collateralMint ? -1 : 1;
    }));
}

/**
 * Render raw atoms at a mint's display precision, exactly, without floating
 * point. The result is a presentation string; the atoms remain the quantity.
 */
export function formatAtomsV1(atoms: bigint | string, decimals: number): string {
  const value = typeof atoms === 'bigint' ? atoms : BigInt(atoms);
  if (!Number.isInteger(decimals) || decimals < 0 || decimals > 255) throw new Error('display decimals must be one u8');
  const scale = 10n ** BigInt(decimals);
  const whole = value / scale;
  const fraction = (value % scale).toString().padStart(decimals, '0').replace(/0+$/, '');
  return decimals === 0 || fraction === '' ? whole.toString() : `${whole}.${fraction}`;
}
