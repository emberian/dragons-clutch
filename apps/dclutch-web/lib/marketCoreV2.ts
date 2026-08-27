import { PublicKey } from '@solana/web3.js';

import { ascii, fromHex, hex, isZero, pubkey, requireNonzero, slice, u16, u64 } from './bytes';
import {
  CORE_PHASE_FOUNDING_TAG,
  CORE_PHASE_OPEN_TAG,
  CORE_PHASE_RETIRED_TAG,
  CORE_PHASE_RETIRING_TAG,
  CORE_PHASE_TERMINAL_TAG,
  CORE_READINESS_CONSUMED_TAG,
  CORE_READINESS_PREPAID_TAG,
  CORE_READINESS_READY_TAG,
  CORE_STATE_BYTES,
  CORE_STATE_CAPABILITY_MANIFEST_OFFSET,
  CORE_STATE_GENERATION_OFFSET,
  CORE_STATE_IDENTITY_REALM_OFFSET,
  CORE_STATE_MAGIC,
  CORE_STATE_MARKET_ID_OFFSET,
  CORE_STATE_OUTSTANDING_CAPABILITIES_OFFSET,
  CORE_STATE_PHASE_OFFSET,
  CORE_STATE_PRODUCT_ID_OFFSET,
  CORE_STATE_PRODUCT_RECORD_OFFSET,
  CORE_STATE_READINESS_OFFSET,
  CORE_STATE_REGISTRY_PROGRAM_OFFSET,
  CORE_STATE_RENT_BENEFICIARY_OFFSET,
  CORE_STATE_RESOLUTION_POLICY_OFFSET,
  CORE_STATE_SELECTED_RELEASE_SET_OFFSET,
  CORE_STATE_TERMINAL_RECEIPT_OFFSET,
  CORE_STATE_TERMINAL_WINNER_OFFSET,
  CORE_STATE_VERSION_OFFSET,
  CORE_VERSION,
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
  LIABILITY_BASIS_MARKET_SEED_V2,
  LIABILITY_BASIS_POSITION_BASIS_OFFSET,
  LIABILITY_BASIS_POSITION_CLAIM_COUNT_OFFSET,
  LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
  LIABILITY_BASIS_POSITION_MAGIC_V2,
  LIABILITY_BASIS_POSITION_MARKET_OFFSET,
  LIABILITY_BASIS_POSITION_OWNER_OFFSET,
  LIABILITY_BASIS_POSITION_REVISION_OFFSET,
  LIABILITY_BASIS_POSITION_SEED_V2,
  LIABILITY_BASIS_STATE_VERSION_V2,
  CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
  CUSTODY_COMPARTMENT_HOARD_PRINCIPAL_TAG_V1,
  CUSTODY_VAULT_PDA_DOMAIN_V1,
  MARKET_CORE_STATE_PDA_DOMAIN_V2,
} from './generated/coreFound';

/**
 * The Market a real dClutch chain actually holds.
 *
 * `DCLTCOR2` is the live Core state: 352 fixed bytes emitted by
 * `formal/dclutch-semantics/EmitMarketCoreRust.lean` into
 * `crates/dclutch-market-core-codec/src/generated.rs`, from which every offset
 * below is generated rather than retyped. It carries IDENTITY and LIFECYCLE and
 * nothing else.
 *
 * What it deliberately does NOT carry is the economics. There is no Hoard
 * figure, no per-claim supply vector and no settlement summary in these bytes:
 *
 *   - the per-claim SUPPLY vector lives in Claims-owned LiabilityBasisV2
 *     aggregate state (`DCLLBM02`) at a PDA derived from the Market;
 *   - each owner's BALANCE vector lives in a LiabilityBasisV2 Position
 *     (`DCLLBP02`) at a PDA derived from that aggregate and the owner;
 *   - the Hoard is a Custody vault whose address is namespaced by the
 *     founding's action context, which the Claims aggregate persists and the
 *     Market root does not — see `deriveMarketHoardAddressV1`.
 *
 * So a surface that wants to show a Market's economics has to go and read the
 * accounts that hold them. It must not read a number off the Market root,
 * because there is none there to read.
 */

export type MarketCorePhaseV2 = 'Founding' | 'Open' | 'Terminal' | 'Retiring' | 'Retired';
export type MarketCoreReadinessV2 = 'Prepaid' | 'Ready' | 'Consumed';

const PHASE_BY_TAG: ReadonlyMap<number, MarketCorePhaseV2> = new Map([
  [CORE_PHASE_FOUNDING_TAG, 'Founding'],
  [CORE_PHASE_OPEN_TAG, 'Open'],
  [CORE_PHASE_TERMINAL_TAG, 'Terminal'],
  [CORE_PHASE_RETIRING_TAG, 'Retiring'],
  [CORE_PHASE_RETIRED_TAG, 'Retired'],
]);

const READINESS_BY_TAG: ReadonlyMap<number, MarketCoreReadinessV2> = new Map([
  [CORE_READINESS_PREPAID_TAG, 'Prepaid'],
  [CORE_READINESS_READY_TAG, 'Ready'],
  [CORE_READINESS_CONSUMED_TAG, 'Consumed'],
]);

/**
 * Where this Market's collateral principal actually sits.
 *
 * The Hoard is a Custody Vault at
 * `[custody-vault-domain, market, release_set, context, HoardPrincipal]`. The
 * `context` is chosen by whoever founded the Market — the atomic founding pins
 * `SHA-256(projected-hoard-context-domain ‖ found.context)` and the local
 * campaign's own action-context domain is a campaign-local string, not a
 * protocol constant — so it is not, and can never be, a function of the Market
 * address.
 *
 * It is not a secret either. The Claims aggregate persists it: `FoundingV5`
 * writes the namespace it authenticated against the Core-owned permit, the Lock
 * receipt, the realization receipt and the live replay, and every on-chain
 * payout route derives from that same field. A reader holding the aggregate
 * therefore names the Hoard by exactly the derivation the chain uses.
 *
 * This browser used to refuse the Hoard outright, because the aggregate said
 * `market` where the founding meant a digest and there was no honest coordinate
 * to read. That field now tells the truth, so the refusal retired with it.
 * Deriving an address is still not the same as authenticating one: see
 * `MARKET_HOARD_UNAUTHENTICATED_V1` and the checks the discovery surface runs
 * before it will show a figure.
 */
export function deriveMarketHoardAddressV1(
  custodyProgramId: string,
  marketAddress: string,
  selectedReleaseSetId: string,
  custodyContext: string,
): string {
  return PublicKey.findProgramAddressSync(
    [
      CUSTODY_VAULT_PDA_DOMAIN_V1,
      new PublicKey(marketAddress).toBytes(),
      fromHex(selectedReleaseSetId, 'selected release set'),
      fromHex(custodyContext, 'Custody namespace'),
      Uint8Array.from([CUSTODY_COMPARTMENT_HOARD_PRINCIPAL_TAG_V1]),
    ],
    new PublicKey(custodyProgramId),
  )[0].toBase58();
}

/**
 * The one transfer authority every Vault of one Market/release set shares.
 *
 * Context-free on purpose: `CustodyAuthoritySeedsV1` is `[domain, market,
 * release_set]`. A token account claiming to be this Market's Hoard whose owner
 * is not this address is somebody else's account at a colliding name.
 */
export function deriveCustodyAuthorityAddressV1(
  custodyProgramId: string,
  marketAddress: string,
  selectedReleaseSetId: string,
): string {
  return PublicKey.findProgramAddressSync(
    [
      CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
      new PublicKey(marketAddress).toBytes(),
      fromHex(selectedReleaseSetId, 'selected release set'),
    ],
    new PublicKey(custodyProgramId),
  )[0].toBase58();
}

/** Why a derived Hoard address is still not a Hoard figure. */
export const MARKET_HOARD_UNAUTHENTICATED_V1 = 'A Hoard address can be derived from the Market, its release set and the Custody namespace the Claims aggregate persists, but an address is not an authentication. This browser shows a principal only for an account the Custody program owns the authority of, holding the Realm\'s collateral mint, under that authority.';

/** The nine ordered seeds a Core V2 Market state address is derived from. */
export type MarketCoreIdentityV2 = Readonly<{
  realmId: string;
  productRecordId: string;
  productInstanceId: string;
  resolutionPolicyId: string;
  capabilityManifestId: string;
  selectedReleaseSetId: string;
  registryProgram: string;
  generation: string;
}>;

export type MarketCoreSettlementV2 =
  | Readonly<{ status: 'open'; label: string }>
  | Readonly<{ status: 'terminal'; label: string; winner: number; receiptId: string }>;

export type MarketCoreStateV2 = Readonly<{
  address: string;
  accountBytes: number;
  version: number;
  phase: MarketCorePhaseV2;
  readiness: MarketCoreReadinessV2;
  marketId: string;
  identity: MarketCoreIdentityV2;
  outstandingCapabilities: string;
  rentBeneficiary: string;
  settlement: MarketCoreSettlementV2;
}>;

function identityBytes(bytes: Uint8Array): ReadonlyArray<Uint8Array> {
  const generation = slice(bytes, CORE_STATE_GENERATION_OFFSET, 8);
  return Object.freeze([
    MARKET_CORE_STATE_PDA_DOMAIN_V2,
    slice(bytes, CORE_STATE_IDENTITY_REALM_OFFSET, 32),
    slice(bytes, CORE_STATE_PRODUCT_RECORD_OFFSET, 32),
    slice(bytes, CORE_STATE_PRODUCT_ID_OFFSET, 32),
    slice(bytes, CORE_STATE_RESOLUTION_POLICY_OFFSET, 32),
    slice(bytes, CORE_STATE_CAPABILITY_MANIFEST_OFFSET, 32),
    slice(bytes, CORE_STATE_SELECTED_RELEASE_SET_OFFSET, 32),
    slice(bytes, CORE_STATE_REGISTRY_PROGRAM_OFFSET, 32),
    generation,
  ]);
}

/**
 * The Market address a Core V2 state's OWN bytes derive.
 *
 * `market_id` is not one of the nine seeds, so this is a genuine check rather
 * than a restatement: the account must derive the address it was found at, from
 * the eight identities and the generation it declares.
 */
export function deriveMarketCoreAddressV2(coreProgramId: string, bytes: Uint8Array): string {
  return PublicKey.findProgramAddressSync(
    identityBytes(bytes) as Uint8Array[],
    new PublicKey(coreProgramId),
  )[0].toBase58();
}

/** Decode one live `DCLTCOR2` Core Market state account. */
export function decodeMarketCoreStateV2(address: string, bytes: Uint8Array): MarketCoreStateV2 {
  if (bytes.length !== CORE_STATE_BYTES) throw new Error(`Core Market state is ${bytes.length} bytes; the exact width is ${CORE_STATE_BYTES}`);
  if (ascii(bytes, 0, 8) !== ascii(CORE_STATE_MAGIC, 0, 8)) throw new Error(`Core Market magic is not ${ascii(CORE_STATE_MAGIC, 0, 8)}`);
  const version = u16(bytes, CORE_STATE_VERSION_OFFSET);
  if (version !== CORE_VERSION) throw new Error(`Core Market state version ${version} is unsupported`);
  const phase = PHASE_BY_TAG.get(bytes[CORE_STATE_PHASE_OFFSET]);
  if (phase === undefined) throw new Error(`Core Market phase tag ${bytes[CORE_STATE_PHASE_OFFSET]} is undefined`);
  const readiness = READINESS_BY_TAG.get(bytes[CORE_STATE_READINESS_OFFSET]);
  if (readiness === undefined) throw new Error(`Core Market readiness tag ${bytes[CORE_STATE_READINESS_OFFSET]} is undefined`);

  const identity: MarketCoreIdentityV2 = Object.freeze({
    realmId: hex(slice(bytes, CORE_STATE_IDENTITY_REALM_OFFSET, 32)),
    productRecordId: hex(slice(bytes, CORE_STATE_PRODUCT_RECORD_OFFSET, 32)),
    productInstanceId: hex(slice(bytes, CORE_STATE_PRODUCT_ID_OFFSET, 32)),
    resolutionPolicyId: hex(slice(bytes, CORE_STATE_RESOLUTION_POLICY_OFFSET, 32)),
    capabilityManifestId: hex(slice(bytes, CORE_STATE_CAPABILITY_MANIFEST_OFFSET, 32)),
    selectedReleaseSetId: hex(slice(bytes, CORE_STATE_SELECTED_RELEASE_SET_OFFSET, 32)),
    registryProgram: pubkey(slice(bytes, CORE_STATE_REGISTRY_PROGRAM_OFFSET, 32), 'Core Market Registry program'),
    generation: u64(bytes, CORE_STATE_GENERATION_OFFSET).toString(),
  });
  for (const [field, offset] of [
    ['Realm identity', CORE_STATE_IDENTITY_REALM_OFFSET],
    ['Product record identity', CORE_STATE_PRODUCT_RECORD_OFFSET],
    ['Product instance identity', CORE_STATE_PRODUCT_ID_OFFSET],
    ['resolution policy identity', CORE_STATE_RESOLUTION_POLICY_OFFSET],
    ['capability manifest identity', CORE_STATE_CAPABILITY_MANIFEST_OFFSET],
    ['selected release-set identity', CORE_STATE_SELECTED_RELEASE_SET_OFFSET],
  ] as const) requireNonzero(slice(bytes, offset, 32), field);

  const receipt = slice(bytes, CORE_STATE_TERMINAL_RECEIPT_OFFSET, 32);
  const settlement: MarketCoreSettlementV2 = isZero(receipt)
    ? Object.freeze({ status: 'open', label: 'no terminal receipt' })
    : Object.freeze({
      status: 'terminal',
      label: 'terminal receipt accepted',
      winner: bytes[CORE_STATE_TERMINAL_WINNER_OFFSET],
      receiptId: hex(receipt),
    });
  // A phase and a receipt are two facts and must agree; a Market claiming a
  // winner without a receipt, or a receipt without a terminal phase, is refused
  // rather than rendered.
  const terminalPhase = phase === 'Terminal' || phase === 'Retiring' || phase === 'Retired';
  if (settlement.status === 'terminal' && !terminalPhase) throw new Error(`Core Market is ${phase} but carries a terminal receipt`);
  if (settlement.status === 'open' && phase === 'Terminal') throw new Error('Core Market is Terminal but carries no terminal receipt');

  return Object.freeze({
    address,
    accountBytes: bytes.length,
    version,
    phase,
    readiness,
    marketId: pubkey(slice(bytes, CORE_STATE_MARKET_ID_OFFSET, 32), 'Core Market identity'),
    identity,
    outstandingCapabilities: u64(bytes, CORE_STATE_OUTSTANDING_CAPABILITIES_OFFSET).toString(),
    rentBeneficiary: pubkey(slice(bytes, CORE_STATE_RENT_BENEFICIARY_OFFSET, 32), 'Core Market rent beneficiary'),
    settlement,
  });
}

// -------------------------------------------------- Claims LiabilityBasisV2

/** Where a Market's per-claim supply vector lives. */
export function deriveClaimsAggregateAddressV2(claimsProgramId: string, marketAddress: string): string {
  return PublicKey.findProgramAddressSync(
    [LIABILITY_BASIS_MARKET_SEED_V2, new PublicKey(marketAddress).toBytes()],
    new PublicKey(claimsProgramId),
  )[0].toBase58();
}

/**
 * Where one owner's claim balances live.
 *
 * Keyed by the AGGREGATE, not by the Market: `ProtocolPositionSeedsV2` in
 * `dclutch-claims-svm` takes the aggregate account and the owner. Deriving from
 * the Market instead produces a plausible address that never holds anything.
 */
export function deriveClaimsPositionAddressV2(claimsProgramId: string, aggregateAddress: string, owner: string): string {
  return PublicKey.findProgramAddressSync(
    [LIABILITY_BASIS_POSITION_SEED_V2, new PublicKey(aggregateAddress).toBytes(), new PublicKey(owner).toBytes()],
    new PublicKey(claimsProgramId),
  )[0].toBase58();
}

export type ClaimsAggregateV2 = Readonly<{
  address: string;
  claimCount: number;
  revision: string;
  logicalMarket: string;
  selectedReleaseSetId: string;
  registryProgram: string;
  productInstanceId: string;
  liabilityBasisId: string;
  realmId: string;
  custodyContext: string;
  generation: string;
  supplyAtoms: ReadonlyArray<string>;
  maximumSupplyAtoms: string;
}>;

export type ClaimsPositionV2 = Readonly<{
  address: string;
  claimCount: number;
  revision: string;
  aggregate: string;
  owner: string;
  liabilityBasisId: string;
  balances: ReadonlyArray<string>;
  completeSetsAtoms: string;
}>;

function u32(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(offset, true);
}

function header(bytes: Uint8Array, magic: Uint8Array, field: string): void {
  if (ascii(bytes, 0, 8) !== ascii(magic, 0, 8)) throw new Error(`${field} magic is not ${ascii(magic, 0, 8)}`);
  const version = u16(bytes, 8);
  if (version !== LIABILITY_BASIS_STATE_VERSION_V2) throw new Error(`${field} state version ${version} is unsupported`);
}

/** Decode one `DCLLBM02` Claims LiabilityBasisV2 aggregate. */
export function decodeClaimsAggregateV2(address: string, bytes: Uint8Array): ClaimsAggregateV2 {
  header(bytes, LIABILITY_BASIS_MARKET_MAGIC_V2, 'Claims aggregate');
  const claimCount = u32(bytes, LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET);
  const expected = LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + claimCount * 8;
  if (bytes.length !== expected) throw new Error(`Claims aggregate is ${bytes.length} bytes; ${claimCount} claims demand exactly ${expected}`);
  if (claimCount === 0) throw new Error('Claims aggregate declares zero claims');
  const supplies = Array.from({ length: claimCount }, (_, index) => u64(bytes, LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + index * 8));
  return Object.freeze({
    address,
    claimCount,
    revision: u64(bytes, LIABILITY_BASIS_MARKET_REVISION_OFFSET).toString(),
    logicalMarket: pubkey(slice(bytes, LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET, 32), 'Claims aggregate logical Market'),
    selectedReleaseSetId: hex(slice(bytes, LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET, 32)),
    registryProgram: pubkey(slice(bytes, LIABILITY_BASIS_MARKET_REGISTRY_OFFSET, 32), 'Claims aggregate Registry program'),
    productInstanceId: hex(slice(bytes, LIABILITY_BASIS_MARKET_PRODUCT_OFFSET, 32)),
    liabilityBasisId: hex(slice(bytes, LIABILITY_BASIS_MARKET_BASIS_OFFSET, 32)),
    realmId: hex(slice(bytes, LIABILITY_BASIS_MARKET_REALM_OFFSET, 32)),
    custodyContext: hex(slice(bytes, LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET, 32)),
    generation: u64(bytes, LIABILITY_BASIS_MARKET_GENERATION_OFFSET).toString(),
    supplyAtoms: Object.freeze(supplies.map((amount) => amount.toString())),
    maximumSupplyAtoms: supplies.reduce((maximum, amount) => (amount > maximum ? amount : maximum), 0n).toString(),
  });
}

/** Decode one `DCLLBP02` Claims LiabilityBasisV2 Position. */
export function decodeClaimsPositionV2(address: string, bytes: Uint8Array): ClaimsPositionV2 {
  header(bytes, LIABILITY_BASIS_POSITION_MAGIC_V2, 'Claims Position');
  const claimCount = u32(bytes, LIABILITY_BASIS_POSITION_CLAIM_COUNT_OFFSET);
  const expected = LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + claimCount * 8;
  if (bytes.length !== expected) throw new Error(`Claims Position is ${bytes.length} bytes; ${claimCount} claims demand exactly ${expected}`);
  if (claimCount === 0) throw new Error('Claims Position declares zero claims');
  const balances = Array.from({ length: claimCount }, (_, index) => u64(bytes, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + index * 8));
  return Object.freeze({
    address,
    claimCount,
    revision: u64(bytes, LIABILITY_BASIS_POSITION_REVISION_OFFSET).toString(),
    aggregate: pubkey(slice(bytes, LIABILITY_BASIS_POSITION_MARKET_OFFSET, 32), 'Claims Position aggregate'),
    owner: pubkey(slice(bytes, LIABILITY_BASIS_POSITION_OWNER_OFFSET, 32), 'Claims Position owner'),
    liabilityBasisId: hex(slice(bytes, LIABILITY_BASIS_POSITION_BASIS_OFFSET, 32)),
    balances: Object.freeze(balances.map((amount) => amount.toString())),
    // A complete set is one atom of every claim, so the number of complete sets
    // these balances admit is the smallest balance among them. Arithmetic on
    // what is owned, not an offer and not a quote.
    completeSetsAtoms: balances.reduce((smallest, amount) => (amount < smallest ? amount : smallest), balances[0] ?? 0n).toString(),
  });
}
