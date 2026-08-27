import { PublicKey } from '@solana/web3.js';

import { fromHex, hex, sha256 } from './bytes';
import {
  decodeCapabilityManifestV1,
  recognizeCapabilityKindV1,
  type CapabilityActivationV1,
  type CapabilityFundingQuoteV1,
} from './capabilityManifest';
import { CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CORE_STATE_MAGIC, REALM_SCHEMA_RELEASE_ID_V1 } from './generated/coreFound';
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
 *   1. A Market is `DCLTCOR2` — the Lean-emitted 352-byte Core state. It holds
 *      identity and lifecycle only.
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

export type MarketEnumerationV1 =
  | Readonly<{ mode: 'address-list'; note: string; addresses: ReadonlyArray<string> }>
  | Readonly<{ mode: 'program-scan'; note: string; scanSlot: string; addresses: ReadonlyArray<string>; scannedAccounts: number }>
  | Readonly<{ mode: 'refused'; note: string; reason: string; addresses: ReadonlyArray<string> }>;

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

type EnumerationClient = Pick<SolanaRpcClient, 'programHeaders'>;

/**
 * Enumerate Market addresses from the Core program itself.
 *
 * `getProgramAccounts` is neither cheap nor universally available; a provider
 * that disables or bounds it is a refusal with its exact reason, not an empty
 * Market list. Callers fall back to an explicit address list.
 */
export async function enumerateCoreMarketAddressesV1(client: EnumerationClient, coreProgramId: string): Promise<MarketEnumerationV1> {
  const program = canonical(coreProgramId, 'Core program');
  try {
    const scan = await client.programHeaders(program);
    const addresses = scan.accounts
      .filter((entry) => isCoreMarketHeaderV2(entry.account.data))
      .map((entry) => entry.address)
      .sort((left, right) => left.localeCompare(right));
    const bounded = addresses.slice(0, MARKET_DISCOVERY_MAX_ADDRESSES);
    const dropped = addresses.length - bounded.length;
    return Object.freeze({
      mode: 'program-scan',
      scanSlot: scan.slot,
      addresses: Object.freeze(bounded),
      scannedAccounts: scan.accounts.length,
      note: `getProgramAccounts returned ${scan.accounts.length} finalized Core accounts at slot ${scan.slot}; ${addresses.length} carry the ${CORE_STATE_MAGIC_TEXT} Market header${dropped > 0 ? `, of which ${dropped} exceed the ${MARKET_DISCOVERY_MAX_ADDRESSES}-Market listing bound and were not read` : ''}.`,
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

async function readAccounts(
  client: DiscoveryClient,
  addresses: ReadonlyArray<string>,
  floor: string,
): Promise<ReadonlyMap<string, Readonly<{ account: RpcAccount | null; slot: string }>>> {
  const observed = new Map<string, Readonly<{ account: RpcAccount | null; slot: string }>>();
  for (const group of chunks(addresses, RPC_ACCOUNT_BATCH)) {
    const batch = await client.multipleAccounts(group, floor);
    for (const entry of batch.accounts) observed.set(entry.address, Object.freeze({ account: entry.account, slot: batch.slot }));
  }
  return observed;
}

/**
 * Reacquire one finalized Registry record by the content identity that names it.
 *
 * The record must exist at its derived PDA, be Registry-owned, have a vacant
 * staging cursor, and hash to the identity that asked for it. Anything else is a
 * refusal carrying its exact reason.
 */
async function finalizedRecordBody(
  client: DiscoveryClient,
  registryProgramId: string,
  schema: Uint8Array,
  identityHex: string,
  floor: string,
  field: string,
): Promise<Readonly<{ address: string; data: Uint8Array; observedSlot: string }>> {
  const digest = fromHex(identityHex, `${field} identity`);
  const addresses = deriveFinalizedRecordAddressesV1(registryProgramId, schema, digest);
  const batch = await client.multipleAccounts([addresses.record, addresses.staging], floor);
  const record = batch.accounts.find((entry) => entry.address === addresses.record)?.account ?? null;
  const staging = batch.accounts.find((entry) => entry.address === addresses.staging)?.account ?? null;
  if (record === null) throw new Error(`${field} record ${addresses.record} is absent at finalized slot ${batch.slot}`);
  if (record.owner !== registryProgramId || record.executable) throw new Error(`${field} record is not Registry-owned finalized data`);
  if (staging !== null) throw new Error(`${field} staging cursor is still present; the record is not canonically final`);
  const observedDigest = hex(await sha256(record.data));
  if (observedDigest !== identityHex) throw new Error(`${field} bytes differ from the identity the Market committed to`);
  return Object.freeze({ address: addresses.record, data: record.data, observedSlot: batch.slot });
}

async function authenticateCapabilityManifest(
  client: DiscoveryClient,
  registryProgramId: string,
  manifestIdHex: string,
  floor: string,
): Promise<MarketCapabilityManifestV1> {
  try {
    const record = await finalizedRecordBody(client, registryProgramId, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, manifestIdHex, floor, 'capability manifest');
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
  client: DiscoveryClient,
  registryProgramId: string | null,
  realmContentId: string,
  floor: string,
): Promise<MarketCollateralV1> {
  if (registryProgramId === null) {
    return Object.freeze({
      status: 'unread',
      realmContentId,
      reason: 'No Registry program was selected, so this Market\'s Realm record was not reacquired and no collateral binding may be asserted from the Market root alone.',
    });
  }
  try {
    const record = await finalizedRecordBody(client, registryProgramId, REALM_SCHEMA_RELEASE_ID_V1, realmContentId, floor, 'Realm');
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
  client: DiscoveryClient,
  claimsProgramId: string | null,
  marketAddress: string,
  marketGeneration: string,
  settled: boolean,
  floor: string,
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
    const batch = await client.multipleAccounts([aggregateAddress], floor);
    const account = batch.accounts[0]?.account ?? null;
    if (account === null) throw new Error(`no Claims LiabilityBasisV2 aggregate exists at ${aggregateAddress} at finalized slot ${batch.slot}`);
    if (account.owner !== claimsProgramId || account.executable) throw new Error('the derived aggregate address holds an account the selected Claims program does not own');
    const aggregate = decodeClaimsAggregateV2(aggregateAddress, account.data);
    if (aggregate.logicalMarket !== marketAddress) throw new Error(`the aggregate names Market ${aggregate.logicalMarket}, not ${marketAddress}`);
    if (aggregate.generation !== marketGeneration) throw new Error(`the aggregate is at generation ${aggregate.generation} while the Market is at ${marketGeneration}; these are two incarnations and are not shown as one`);
    return Object.freeze({
      status: 'bound',
      observedSlot: batch.slot,
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
  client: DiscoveryClient,
  custodyProgramId: string | null,
  marketAddress: string,
  selectedReleaseSetId: string,
  collateral: MarketCollateralV1,
  liability: MarketLiabilityV1,
  floor: string,
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
    const batch = await client.multipleAccounts([address], floor);
    const account = batch.accounts[0]?.account ?? null;
    if (account === null) throw new Error(`no account exists at the derived Hoard Vault ${address} at finalized slot ${batch.slot}`);
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
      observedSlot: batch.slot,
      address,
      custodyProgramId,
      custodyContext: liability.custodyContext,
      custodyAuthority: authority,
      collateralMint: mint,
      tokenProgram: collateral.tokenProgram,
      principalAtoms: readU64(bytes, 64).toString(),
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

/**
 * Read one finalized Market discovery listing.
 *
 * All Markets, their Realm records, their liability aggregates and their
 * capability manifests are read behind a single finalized floor slot so no card
 * mixes observation epochs.
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
    return Object.freeze({
      coreProgramId,
      registryProgramId,
      claimsProgramId,
      custodyProgramId,
      floorSlot: floor,
      enumeration,
      cards: Object.freeze([]),
      reason: 'No Market address has been supplied or enumerated at this finalized floor.',
    });
  }

  const observed = await readAccounts(client, addresses, floor);
  const cards: MarketDiscoveryCardV1[] = [];
  for (const address of addresses) {
    const entry = observed.get(address);
    if (entry === undefined || entry.account === null) {
      const reason = 'account is absent at the finalized observation floor';
      cards.push(Object.freeze({ status: 'refused', address, provenance: Object.freeze({ kind: 'refused', reason }), observedSlot: entry?.slot ?? floor, refusal: reason }));
      continue;
    }
    if (entry.account.owner !== coreProgramId || entry.account.executable) {
      const reason = 'account owner differs from the selected Core program, or it is executable program data';
      cards.push(Object.freeze({ status: 'refused', address, provenance: Object.freeze({ kind: 'refused', reason }), observedSlot: entry.slot, refusal: reason }));
      continue;
    }
    let state;
    try {
      state = decodeMarketCoreStateV2(address, entry.account.data);
    } catch (error) {
      const reason = error instanceof Error ? error.message : 'the account did not decode as one canonical Core Market state';
      cards.push(Object.freeze({ status: 'refused', address, provenance: Object.freeze({ kind: 'refused', reason }), observedSlot: entry.slot, refusal: reason }));
      continue;
    }

    // The account must derive the address it was found at, from the identities
    // it declares. `market_id` is not one of those seeds, so this is a real
    // check and not a restatement.
    const derived = deriveMarketCoreAddressV2(coreProgramId, entry.account.data);
    const bindings: BindingCheck[] = [
      Object.freeze({ label: 'Market PDA', ok: derived === address, detail: `identity seeds + generation ${state.identity.generation} → ${derived}` }),
      Object.freeze({ label: 'Market self-identity', ok: state.marketId === address, detail: `state names ${state.marketId}` }),
      Object.freeze({
        label: 'Registry authority',
        ok: registryProgramId === null || state.identity.registryProgram === registryProgramId,
        detail: registryProgramId === null ? `Market selects Registry ${state.identity.registryProgram}; none was selected here` : `Market selects ${state.identity.registryProgram}`,
      }),
    ];

    const collateral = await authenticateCollateral(client, registryProgramId, state.identity.realmId, floor);
    const liability = await readLiability(client, claimsProgramId, address, state.identity.generation, state.settlement.status === 'terminal', floor);
    const hoard = await readHoard(client, custodyProgramId, address, state.identity.selectedReleaseSetId, collateral, liability, floor);
    const capabilities: MarketCapabilityManifestV1 = registryProgramId === null
      ? Object.freeze({
        status: 'unread',
        manifestId: state.identity.capabilityManifestId,
        reason: 'No Registry program was selected, so this Market\'s capability manifest was not authenticated. No capability may be asserted from the Market root alone.',
      })
      : await authenticateCapabilityManifest(client, registryProgramId, state.identity.capabilityManifestId, floor);

    cards.push(Object.freeze({
      status: 'decoded',
      address,
      provenance: Object.freeze({ kind: 'chain', observedSlot: entry.slot }),
      observedSlot: entry.slot,
      phase: state.phase,
      readiness: state.readiness,
      generation: state.identity.generation,
      outstandingCapabilities: state.outstandingCapabilities,
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
      bindings: Object.freeze(bindings),
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
