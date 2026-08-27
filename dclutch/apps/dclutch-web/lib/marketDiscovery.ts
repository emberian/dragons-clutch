import { PublicKey } from '@solana/web3.js';

import { fromHex, hex, sha256 } from './bytes';
import {
  decodeCapabilityManifestV1,
  recognizeCapabilityKindV1,
  type CapabilityActivationV1,
  type CapabilityFundingQuoteV1,
} from './capabilityManifest';
import {
  classifyHeader,
  crossCheckBindings,
  decodeCoreAccount,
  deriveRealmAddress,
  verifyLocalBindings,
  type AccountProjection,
  type BindingCheck,
  type DecodedProjection,
  type FullAccountObservation,
  type MarketPhase,
  type MarketSettlement,
  type RealmAuthorityPolicy,
  type RequiredBackingBasis,
} from './decoders';
import { CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1 } from './generated/coreFound';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/**
 * Market discovery: a bounded, finalized, chain-derived listing.
 *
 * Every value on a discovery card is decoded from a finalized account this
 * browser read, or the card says `REFUSED` and why. There is no volume, price,
 * odds, probability, APR, or aggregate: Hoard atoms are the Market's raw
 * collateral integer and are never presented as liquidity, TVL, or an amount
 * available to anyone. Capability badges come only from the Market's own
 * authenticated capability manifest.
 */

export const MARKET_DISCOVERY_MAX_ADDRESSES = 32;
const RPC_ACCOUNT_BATCH = 32;

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
  | Readonly<{ status: 'refused'; realmAddress: string | null; realmContentId: string; reason: string }>;

/**
 * The immutable identities one Market root commits to, plus the exact artifact
 * profile its account bytes declare. These are content identities, not
 * addresses: a Market names what it is, and a reader reacquires each named
 * record from its own canonical authority.
 */
export type MarketIdentityV1 = Readonly<{
  schemaMagic: string;
  schemaVersion: number;
  categoricalProfile: number;
  accountBytes: number;
  realmId: string;
  productInstanceId: string;
  claimBasisId: string;
  resolutionPolicyId: string;
  capabilityManifestId: string;
  rentRefundAuthority: string;
}>;

export type MarketDiscoveryCardV1 =
  | Readonly<{
    status: 'decoded';
    address: string;
    provenance: MarketProvenanceV1;
    observedSlot: string;
    phase: MarketPhase;
    generation: string;
    outcomeCount: number;
    hoardAtoms: string;
    supplyAtoms: ReadonlyArray<string>;
    requiredBackingAtoms: string;
    requiredBackingBasis: RequiredBackingBasis;
    outstandingChildren: string;
    settlement: MarketSettlement;
    identity: MarketIdentityV1;
    collateral: MarketCollateralV1;
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
  floorSlot: string;
  enumeration: MarketEnumerationV1;
  cards: ReadonlyArray<MarketDiscoveryCardV1>;
  reason: string;
}>;

export type MarketDiscoveryRequestV1 = Readonly<{
  coreProgramId: string;
  registryProgramId?: string | null;
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
      .filter((entry) => classifyHeader(entry.account.data) === 'Market')
      .map((entry) => entry.address)
      .sort((left, right) => left.localeCompare(right));
    const bounded = addresses.slice(0, MARKET_DISCOVERY_MAX_ADDRESSES);
    const dropped = addresses.length - bounded.length;
    return Object.freeze({
      mode: 'program-scan',
      scanSlot: scan.slot,
      addresses: Object.freeze(bounded),
      scannedAccounts: scan.accounts.length,
      note: `getProgramAccounts returned ${scan.accounts.length} finalized Core accounts at slot ${scan.slot}; ${addresses.length} carry the Market header${dropped > 0 ? `, of which ${dropped} exceed the ${MARKET_DISCOVERY_MAX_ADDRESSES}-Market listing bound and were not read` : ''}.`,
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

function observation(address: string, account: RpcAccount, observedSlot: string): FullAccountObservation {
  return Object.freeze({
    address,
    owner: account.owner,
    executable: account.executable,
    lamports: account.lamports,
    observedSlot,
    data: account.data,
  });
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

async function authenticateCapabilityManifest(
  client: DiscoveryClient,
  registryProgramId: string,
  manifestIdHex: string,
  floor: string,
): Promise<MarketCapabilityManifestV1> {
  try {
    const digest = fromHex(manifestIdHex, 'capability manifest identity');
    const addresses = deriveFinalizedRecordAddressesV1(registryProgramId, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, digest);
    const batch = await client.multipleAccounts([addresses.record, addresses.staging], floor);
    const record = batch.accounts.find((entry) => entry.address === addresses.record)?.account ?? null;
    const staging = batch.accounts.find((entry) => entry.address === addresses.staging)?.account ?? null;
    if (record === null) throw new Error(`manifest record ${addresses.record} is absent at finalized slot ${batch.slot}`);
    if (record.owner !== registryProgramId || record.executable) throw new Error('manifest record is not Registry-owned finalized data');
    if (staging !== null) throw new Error('manifest staging cursor is still present; the record is not canonically final');
    const observedDigest = hex(await sha256(record.data));
    if (observedDigest !== manifestIdHex) throw new Error('manifest bytes differ from the identity the Market committed to');
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
    return Object.freeze({ status: 'authenticated', manifestId: manifestIdHex, recordAddress: addresses.record, observedSlot: batch.slot, badges: Object.freeze(badges) });
  } catch (error) {
    return Object.freeze({
      status: 'refused',
      manifestId: manifestIdHex,
      reason: error instanceof Error ? error.message : 'capability manifest refused without a usable reason',
    });
  }
}

function collateralFrom(realm: DecodedProjection | null, realmAddress: string, realmContentId: string): MarketCollateralV1 {
  if (realm === null || realm.semantics.kind !== 'Realm') {
    return Object.freeze({ status: 'refused', realmAddress, realmContentId, reason: `Realm ${realmAddress} did not decode at this finalized floor; collateral identity is unbound.` });
  }
  if (realm.semantics.contentDigest !== realmContentId) {
    return Object.freeze({ status: 'refused', realmAddress, realmContentId, reason: 'decoded Realm content digest differs from the identity the Market committed to' });
  }
  // Every Realm field comes from the one canonical Realm decoder; this module
  // does not restate a byte offset it does not own.
  const semantics = realm.semantics;
  return Object.freeze({
    status: 'bound',
    observedSlot: realm.observedSlot,
    realmAddress,
    realmContentId,
    collateralMint: semantics.collateralMint,
    collateralMintShort: shortAddressV1(semantics.collateralMint),
    tokenProgram: semantics.tokenProgram,
    adapterReleaseId: semantics.adapterReleaseId,
    mintAuthorityPolicy: semantics.mintAuthorityPolicy,
    freezeAuthorityPolicy: semantics.freezeAuthorityPolicy,
  });
}

/**
 * Read one finalized Market discovery listing.
 *
 * All Markets, their Realms, and their capability manifests are read behind a
 * single finalized floor slot so no card mixes observation epochs.
 */
export async function inspectMarketDiscoveryV1(
  client: DiscoveryClient,
  request: MarketDiscoveryRequestV1,
): Promise<MarketDiscoveryV1> {
  const coreProgramId = canonical(request.coreProgramId, 'Core program');
  const registryProgramId = request.registryProgramId === undefined || request.registryProgramId === null || request.registryProgramId === ''
    ? null
    : canonical(request.registryProgramId, 'Registry program');
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
      floorSlot: floor,
      enumeration,
      cards: Object.freeze([]),
      reason: 'No Market address has been supplied or enumerated at this finalized floor.',
    });
  }

  const marketAccounts = await readAccounts(client, addresses, floor);
  const marketProjections = new Map<string, AccountProjection>();
  for (const address of addresses) {
    const entry = marketAccounts.get(address);
    if (entry === undefined || entry.account === null) {
      marketProjections.set(address, Object.freeze({
        status: 'refused', kind: 'Market', address, lamports: '0',
        observedSlot: entry?.slot ?? floor, header: '',
        reason: 'account is absent at the finalized observation floor',
      }));
      continue;
    }
    const projection = decodeCoreAccount(observation(address, entry.account, entry.slot), coreProgramId);
    marketProjections.set(address, projection.status === 'decoded' ? await verifyLocalBindings(projection, coreProgramId) : projection);
  }

  const realmRequests = new Map<string, string>();
  for (const projection of marketProjections.values()) {
    if (projection.status !== 'decoded' || projection.semantics.kind !== 'Market') continue;
    try {
      realmRequests.set(projection.semantics.realmId, deriveRealmAddress(coreProgramId, projection.semantics.realmId));
    } catch {
      // A Market whose Realm identity will not derive is reported per card.
    }
  }
  const realmAddresses = Object.freeze([...new Set(realmRequests.values())]);
  const realmProjections = new Map<string, DecodedProjection>();
  if (realmAddresses.length > 0) {
    const realmAccounts = await readAccounts(client, realmAddresses, floor);
    for (const address of realmAddresses) {
      const entry = realmAccounts.get(address);
      if (entry === undefined || entry.account === null) continue;
      const projection = decodeCoreAccount(observation(address, entry.account, entry.slot), coreProgramId);
      if (projection.status === 'decoded') realmProjections.set(address, await verifyLocalBindings(projection, coreProgramId));
    }
  }

  const joined = crossCheckBindings([...marketProjections.values(), ...realmProjections.values()]);
  const joinedMarkets = new Map(joined.filter((projection) => addresses.includes(projection.address)).map((projection) => [projection.address, projection]));

  const cards: MarketDiscoveryCardV1[] = [];
  for (const address of addresses) {
    const projection = joinedMarkets.get(address) ?? marketProjections.get(address);
    if (projection === undefined || projection.status !== 'decoded' || projection.semantics.kind !== 'Market') {
      const reason = projection !== undefined && projection.status === 'refused'
        ? projection.reason
        : 'account did not decode as one canonical Core Market';
      cards.push(Object.freeze({
        status: 'refused',
        address,
        provenance: Object.freeze({ kind: 'refused', reason }),
        observedSlot: projection?.observedSlot ?? floor,
        refusal: reason,
      }));
      continue;
    }
    const semantics = projection.semantics;
    const realmAddress = realmRequests.get(semantics.realmId) ?? null;
    const collateral = realmAddress === null
      ? Object.freeze({ status: 'refused' as const, realmAddress: null, realmContentId: semantics.realmId, reason: 'Market Realm content identity did not derive one canonical Realm address' })
      : collateralFrom(realmProjections.get(realmAddress) ?? null, realmAddress, semantics.realmId);
    const capabilities: MarketCapabilityManifestV1 = registryProgramId === null
      ? Object.freeze({
        status: 'unread',
        manifestId: semantics.capabilityManifestId,
        reason: 'No Registry program was selected, so this Market\'s capability manifest was not authenticated. No capability may be asserted from the Market root alone.',
      })
      : await authenticateCapabilityManifest(client, registryProgramId, semantics.capabilityManifestId, floor);
    cards.push(Object.freeze({
      status: 'decoded',
      address,
      provenance: Object.freeze({ kind: 'chain', observedSlot: projection.observedSlot }),
      observedSlot: projection.observedSlot,
      phase: semantics.phase,
      generation: semantics.generation,
      outcomeCount: semantics.outcomeCount,
      hoardAtoms: semantics.hoardAtoms,
      supplyAtoms: semantics.supply,
      requiredBackingAtoms: semantics.requiredBackingAtoms,
      requiredBackingBasis: semantics.requiredBackingBasis,
      outstandingChildren: semantics.outstandingChildren,
      settlement: semantics.settlement,
      identity: Object.freeze({
        schemaMagic: 'DCLTCAT1',
        schemaVersion: semantics.schemaVersion,
        categoricalProfile: semantics.categoricalProfile,
        accountBytes: semantics.accountBytes,
        realmId: semantics.realmId,
        productInstanceId: semantics.productInstanceId,
        claimBasisId: semantics.claimBasisId,
        resolutionPolicyId: semantics.resolutionPolicyId,
        capabilityManifestId: semantics.capabilityManifestId,
        rentRefundAuthority: semantics.rentRefundAuthority,
      }),
      collateral,
      capabilities,
      bindings: projection.bindings,
      refusal: null,
    }));
  }

  const decoded = cards.filter((card) => card.status === 'decoded').length;
  return Object.freeze({
    coreProgramId,
    registryProgramId,
    floorSlot: floor,
    enumeration,
    cards: Object.freeze(cards),
    reason: `${decoded} of ${cards.length} requested Market${cards.length === 1 ? '' : 's'} decoded at finalized floor ${floor}.`,
  });
}
