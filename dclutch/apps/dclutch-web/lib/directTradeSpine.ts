import { PublicKey } from '@solana/web3.js';

import { hex, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import { capabilityRootAddressV1, decodeCapabilityManifestV1 } from './capabilityManifest';
import { describeDirectDecodeVintageV1 } from './directDecodeVintage';
import {
  decodeDirectDescriptorV4,
  decodeDirectProgramSetV2,
} from './directHotChain';
import * as DirectAbi from './generated/directInlineV3';
import { CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1 } from './generated/coreFound';
import {
  decodeClaimsAggregateV2,
  decodeMarketCoreStateV2,
  deriveClaimsAggregateAddressV2,
  deriveClaimsPositionAddressV2,
  type MarketCoreStateV2,
} from './marketCoreV2';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/**
 * The Direct trade spine: what the chain itself says about trading this
 * Market, before any route manifest exists.
 *
 * The full Hot route needs a 39-account frame, per-outcome runtime accounts
 * and one canonical lookup table — transport an operator publishes. But the
 * facts a TRADER needs first are all reachable from the Market alone: whether
 * a Direct capability is in the manifest, whether its activated root exists,
 * the immutable price scale and fee the config pins, and the Product width.
 * This module derives exactly those, from finalized reads, and names every
 * wall it hits in the chain's own vocabulary — because on this protocol a
 * refusal with its reason IS the honest product surface.
 */

export type DirectTradeWallV1 = Readonly<{
  name: string;
  detail: string;
}>;

export type DirectTradeSpineV1 = Readonly<{
  status: 'inspected';
  observedSlot: string;
  marketAddress: string;
  phase: MarketCoreStateV2['phase'];
  generation: string;
  entryIndex: number;
  manifestRecordAddress: string;
  programSetId: string;
  configId: string;
  descriptorId: string;
  /** The execution release set this Market selects. The `release` wall is about it. */
  releaseSetId: string;
  priceScale: bigint;
  feeBasisPoints: number;
  feeRecipient: string;
  outcomeCount: number | null;
  aggregateAddress: string | null;
  rootAddress: string | null;
  rootExists: boolean | null;
  positionAddress: string | null;
  positionExists: boolean | null;
  walls: ReadonlyArray<DirectTradeWallV1>;
  tradable: boolean;
  reason: string;
}> | Readonly<{ status: 'refused'; reason: string }>;

export type DirectTradeSpineRequestV1 = Readonly<{
  marketAddress: string;
  coreProgramId: string;
  registryProgramId: string;
  tradingProgramId?: string | null;
  claimsProgramId?: string | null;
  /** The connected wallet, if any: lets the spine check the trading prestate. */
  owner?: string | null;
  /**
   * Execution release sets known to have a CHECKED execution release, or null.
   *
   * A Direct fill is admitted at the route boundary only against a checked
   * release, and that artifact is produced offline -- no account on chain says
   * whether one exists, so this spine cannot read the answer and must be
   * handed it. An array is an answer: a release set absent from it raises the
   * `release` wall. `null` or omitted is NOT an answer -- it means nobody
   * consulted a deployment record -- and raises nothing, because a wall
   * asserted from an absence of evidence is the failure mode this whole file
   * exists to avoid.
   */
  checkedReleaseSetIds?: ReadonlyArray<string> | null;
}>;

function canonical(value: string, field: string): string {
  const key = new PublicKey(value).toBase58();
  if (key !== value) throw new Error(`${field} must be canonical base58 text`);
  return key;
}

function optional(value: string | null | undefined, field: string): string | null {
  return value === undefined || value === null || value === '' ? null : canonical(value, field);
}

async function finalizedRecordBody(
  client: Pick<SolanaRpcClient, 'multipleAccounts'>,
  registryProgramId: string,
  schema: Uint8Array,
  digest: Uint8Array,
  floor: string,
  field: string,
): Promise<Readonly<{ address: string; data: Uint8Array }>> {
  const derived = deriveFinalizedRecordAddressesV1(registryProgramId, schema, digest);
  const observation = await client.multipleAccounts([derived.record], floor);
  const account = observation.accounts[0]?.account ?? null;
  if (account === null) throw new Error(`${field} record is absent at ${derived.record}`);
  if (account.owner !== registryProgramId || account.executable) throw new Error(`${field} record is not Registry-owned finalized data`);
  const observedDigest = await sha256(account.data);
  if (hex(observedDigest) !== hex(digest)) throw new Error(`${field} record bytes differ from their selected content identity`);
  return Object.freeze({ address: derived.record, data: account.data });
}

function decodeDirectConfig(bytes: Uint8Array): Readonly<{ priceScale: bigint; feeBasisPoints: number; feeRecipient: string }> {
  if (bytes.length !== DirectAbi.DIRECT_EXECUTION_CONFIG_BYTES_V1
      || hex(slice(bytes, DirectAbi.DIRECT_CONFIG_MAGIC_OFFSET_V1, 8)) !== hex(DirectAbi.DIRECT_CONFIG_MAGIC_V1)
      || u16(bytes, DirectAbi.DIRECT_CONFIG_VERSION_OFFSET_V1) !== 1) throw new Error('Direct config has the wrong exact ABI');
  requireZero(bytes, DirectAbi.DIRECT_CONFIG_RESERVED_A_OFFSET_V1, 6, 'Direct config header');
  requireZero(bytes, DirectAbi.DIRECT_CONFIG_RESERVED_B_OFFSET_V1, 6, 'Direct config fee field');
  const feeRecipient = slice(bytes, DirectAbi.DIRECT_CONFIG_FEE_RECIPIENT_OFFSET_V1, 32);
  requireNonzero(feeRecipient, 'Direct fee recipient');
  const priceScale = u64(bytes, DirectAbi.DIRECT_CONFIG_PRICE_SCALE_OFFSET_V1);
  const feeBasisPoints = u16(bytes, DirectAbi.DIRECT_CONFIG_FEE_BPS_OFFSET_V1);
  if (priceScale === 0n || feeBasisPoints > 10_000) throw new Error('Direct config price scale or fee rate is invalid');
  return Object.freeze({ priceScale, feeBasisPoints, feeRecipient: new PublicKey(feeRecipient).toBase58() });
}

/** Exact canonical budgeted packet measurement pinned by the real v0 gate. */
export const DIRECT_PACKET_BUDGET_EVIDENCE_V1 = Object.freeze({
  wireBytes: 1_204,
  packetLimit: 1_232,
  marginBytes: 28,
  computeUnitLimit: 1_400_000,
});

/** Name a packet wall only when measured caller geometry actually exceeds it. */
export function directPacketWallV1(wireBytes: number): DirectTradeWallV1 | null {
  if (!Number.isSafeInteger(wireBytes) || wireBytes < 0) throw new Error('Direct packet measurement must be a nonnegative safe integer');
  if (wireBytes <= DIRECT_PACKET_BUDGET_EVIDENCE_V1.packetLimit) return null;
  return Object.freeze({
    name: 'packet',
    detail: `Your measured Direct transaction is ${wireBytes.toLocaleString('en-US')} bytes, above the network’s ${DIRECT_PACKET_BUDGET_EVIDENCE_V1.packetLimit.toLocaleString('en-US')}-byte limit. Reduce its account or instruction geometry before signing.`,
  });
}

export const DIRECT_PRESTATE_WALL_V1: DirectTradeWallV1 = Object.freeze({
  name: 'prestate',
  detail: 'Your wallet does not have a Claims Position on this Market yet. A devnet admission command now exists, but this public page does not create or sign one and has no authenticated admission dossier for your distinct Token-2022 collateral account.',
});

/** Inspect everything the chain can say about Direct trading this Market. */
export async function inspectDirectTradeSpineV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>,
  request: DirectTradeSpineRequestV1,
): Promise<DirectTradeSpineV1> {
  try {
    const marketAddress = canonical(request.marketAddress, 'Market address');
    const coreProgramId = canonical(request.coreProgramId, 'Core program');
    const registryProgramId = canonical(request.registryProgramId, 'Registry program');
    const tradingProgramId = optional(request.tradingProgramId, 'Trading program');
    const claimsProgramId = optional(request.claimsProgramId, 'Claims program');
    const owner = optional(request.owner, 'owner');

    const floor = await client.finalizedSlot();
    const marketObservation = await client.multipleAccounts([marketAddress], floor);
    const marketAccount = marketObservation.accounts[0]?.account ?? null;
    if (marketAccount === null) return Object.freeze({ status: 'refused', reason: `no account exists at ${marketAddress} at finalized commitment` });
    if (marketAccount.owner !== coreProgramId || marketAccount.executable) {
      return Object.freeze({ status: 'refused', reason: `the account at ${marketAddress} is not owned by the selected Core program (owner ${marketAccount.owner})` });
    }
    const market = decodeMarketCoreStateV2(marketAddress, marketAccount.data);
    if (market.identity.registryProgram !== registryProgramId) {
      return Object.freeze({ status: 'refused', reason: `this Market selected Registry program ${market.identity.registryProgram}, not ${registryProgramId}` });
    }

    const manifestDigest = Uint8Array.from((market.identity.capabilityManifestId.match(/../g) ?? []).map((value) => Number.parseInt(value, 16)));
    const manifestRecord = await finalizedRecordBody(client, registryProgramId, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, manifestDigest, floor, 'capability manifest');
    const entries = decodeCapabilityManifestV1(manifestRecord.data);
    const directEntry = entries.find((entry) => hex(entry.kind) === hex(DirectAbi.DIRECT_SUCCESSOR_KIND_ID_V3)) ?? null;
    if (directEntry === null) {
      return Object.freeze({
        status: 'refused',
        reason: `this Market's authenticated capability manifest lists ${entries.length} entr${entries.length === 1 ? 'y' : 'ies'} and none is the Direct successor kind — Direct trading was never part of this Market's founding, which is the Market's own choice, not an outage`,
      });
    }

    const programSetRecord = await finalizedRecordBody(client, registryProgramId, DirectAbi.CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, directEntry.programSet, floor, 'CapabilityProgramSetV2');
    const selected = decodeDirectProgramSetV2(programSetRecord.data);
    // Not a new gate -- the same equality as before, saying more. A Market
    // whose release is newer than this build is not a broken Market, and the
    // sentence should not read as an accusation: it names the chain's release
    // identity, both schemas, and what this build is.
    if (hex(selected.schema) !== hex(DirectAbi.CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID)) {
      return Object.freeze({
        status: 'refused',
        reason: `this Market (release set ${market.identity.selectedReleaseSetId}) selects descriptor schema ${hex(selected.schema)}; this build decodes ${hex(DirectAbi.CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID)} — if the Market's release is newer, this build predates it: ${describeDirectDecodeVintageV1()}`,
      });
    }
    const descriptorRecord = await finalizedRecordBody(client, registryProgramId, selected.schema, selected.program, floor, 'CapabilityProgramV4 descriptor');
    // The descriptor decoder names the one disagreeing field and both values;
    // this adds the chain's release identity, so a vintage refusal carries
    // everything needed to place it without a manual chain diff. Rethrown
    // unchanged otherwise -- corruption must keep reading as corruption.
    let descriptor;
    try {
      descriptor = decodeDirectDescriptorV4(descriptorRecord.data);
    } catch (error) {
      throw new Error(`${error instanceof Error ? error.message : String(error)} (this Market's release set is ${market.identity.selectedReleaseSetId}; ${describeDirectDecodeVintageV1()})`);
    }
    const configRecord = await finalizedRecordBody(client, registryProgramId, descriptor.configSchema, directEntry.config, floor, 'Direct config');
    const config = decodeDirectConfig(configRecord.data);

    const walls: DirectTradeWallV1[] = [];
    if (market.phase !== 'Open') {
      walls.push(Object.freeze({ name: 'phase', detail: `this Market is ${market.phase} — trading is only open while a Market is Open` }));
    }

    let rootAddress: string | null = null;
    let rootExists: boolean | null = null;
    let aggregateAddress: string | null = null;
    let outcomeCount: number | null = null;
    let positionAddress: string | null = null;
    let positionExists: boolean | null = null;
    const probes: string[] = [];
    if (tradingProgramId !== null) {
      rootAddress = capabilityRootAddressV1(
        tradingProgramId,
        marketAddress,
        BigInt(market.identity.generation),
        manifestDigest,
        directEntry,
      );
      probes.push(rootAddress);
    }
    if (claimsProgramId !== null) {
      aggregateAddress = deriveClaimsAggregateAddressV2(claimsProgramId, marketAddress);
      probes.push(aggregateAddress);
    }
    let probeAccounts = new Map<string, RpcAccount | null>();
    if (probes.length > 0) {
      const observation = await client.multipleAccounts(probes, floor);
      probeAccounts = new Map(observation.accounts.map((entry) => [entry.address, entry.account]));
    }
    if (rootAddress !== null) {
      const root = probeAccounts.get(rootAddress) ?? null;
      rootExists = root !== null && root.owner === tradingProgramId && root.data.length > 0;
      if (!rootExists) {
        walls.push(Object.freeze({ name: 'activation', detail: `this Market founded a Direct trading capability but never switched it on — no activation root exists at ${rootAddress}. Activation is the operator’s move, not yours.` }));
      }
    }
    if (aggregateAddress !== null && claimsProgramId !== null) {
      const aggregateAccount = probeAccounts.get(aggregateAddress) ?? null;
      if (aggregateAccount !== null && aggregateAccount.owner === claimsProgramId) {
        const aggregate = decodeClaimsAggregateV2(aggregateAddress, aggregateAccount.data);
        outcomeCount = aggregate.claimCount;
      }
      if (owner !== null) {
        positionAddress = deriveClaimsPositionAddressV2(claimsProgramId, aggregateAddress, owner);
        const positionObservation = await client.multipleAccounts([positionAddress], floor);
        positionExists = (positionObservation.accounts[0]?.account ?? null) !== null;
        if (!positionExists) {
          walls.push(DIRECT_PRESTATE_WALL_V1);
        }
      }
    }
    /*
      THE WALL A READER USED TO MEET AT THE PREVIEW BUTTON.

      Everything above says this market can take a fill: phase Open, capability
      activated, packet inside the limit. The fill still refuses, at the route
      admission boundary, unless a checked execution release exists for the
      release set the Market selects -- and a full-redeploy cohort cannot
      produce one, so this is the standing state of every cohort founded that
      way rather than a transient. Nothing on chain carries the answer, so it
      arrives from the caller's own deployment record and is asserted only when
      one was supplied.
    */
    if (request.checkedReleaseSetIds != null && !request.checkedReleaseSetIds.includes(market.identity.selectedReleaseSetId)) {
      walls.push(Object.freeze({
        name: 'release',
        detail: `no checked execution release is on file for this Market’s execution release set ${market.identity.selectedReleaseSetId}, so a Direct fill refuses at the route admission boundary. Joining this market and putting collateral in are unaffected. Whether a release for this set can ever be produced is a question about how the set was minted, not about this market, so nothing here promises one is coming.`,
      }));
    }
    const packetWall = directPacketWallV1(DIRECT_PACKET_BUDGET_EVIDENCE_V1.wireBytes);
    if (packetWall !== null) walls.push(packetWall);

    const tradable = market.phase === 'Open' && (rootExists ?? false) && !walls.some((entry) => entry.name === 'release');
    return Object.freeze({
      status: 'inspected',
      observedSlot: floor,
      marketAddress,
      phase: market.phase,
      generation: market.identity.generation,
      entryIndex: directEntry.index,
      manifestRecordAddress: manifestRecord.address,
      releaseSetId: market.identity.selectedReleaseSetId,
      programSetId: hex(directEntry.programSet),
      configId: hex(directEntry.config),
      descriptorId: hex(selected.program),
      priceScale: config.priceScale,
      feeBasisPoints: config.feeBasisPoints,
      feeRecipient: config.feeRecipient,
      outcomeCount,
      aggregateAddress,
      rootAddress,
      rootExists,
      positionAddress,
      positionExists,
      walls: Object.freeze(walls),
      tradable,
      reason: tradable
        ? walls.length === 0
          ? `Direct entry ${directEntry.index} is founded and activated. Its canonical transaction includes the ${DIRECT_PACKET_BUDGET_EVIDENCE_V1.computeUnitLimit.toLocaleString('en-US')}-unit compute declaration and measures ${DIRECT_PACKET_BUDGET_EVIDENCE_V1.wireBytes.toLocaleString('en-US')} of ${DIRECT_PACKET_BUDGET_EVIDENCE_V1.packetLimit.toLocaleString('en-US')} bytes, leaving ${DIRECT_PACKET_BUDGET_EVIDENCE_V1.marginBytes} bytes.`
          : `Direct entry ${directEntry.index} is founded and activated: immutable price scale ${config.priceScale}, fee ${config.feeBasisPoints} bps. Executing a fill still crosses the named walls below.`
        : `Direct entry ${directEntry.index} is founded (price scale ${config.priceScale}, fee ${config.feeBasisPoints} bps) and ${walls.length} named wall${walls.length === 1 ? '' : 's'} stand${walls.length === 1 ? 's' : ''} between a signed intent and an executed fill.`,
    });
  } catch (error) {
    return Object.freeze({ status: 'refused', reason: error instanceof Error ? error.message : 'the Direct spine inspection refused without a usable reason' });
  }
}

/** Convenience: does a nonzero balance make sense to offer at this width. */
export function spineAdmitsOutcomeV1(spine: DirectTradeSpineV1, outcome: number): boolean {
  return spine.status === 'inspected' && spine.outcomeCount !== null && Number.isInteger(outcome) && outcome >= 0 && outcome < spine.outcomeCount;
}
