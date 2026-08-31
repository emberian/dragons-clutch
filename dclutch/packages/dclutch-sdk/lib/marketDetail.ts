import {
  inspectMarketDiscoveryV1,
  type MarketCapabilityManifestV1,
  type MarketCollateralV1,
  type MarketDiscoveryCardV1,
  type MarketLiabilityV1,
  type MarketProvenanceV1,
} from './marketDiscovery';
import { type MarketCorePhaseV2 } from './marketCoreV2';
import { type RequiredBackingBasisV2 } from './marketDiscovery';
import { type SolanaRpcClient } from './rpc';

/**
 * One Market's detail projection.
 *
 * This is `inspectMarketDiscoveryV1` narrowed to a single address plus the
 * per-section provenance a detail surface owes its reader. It decodes nothing
 * itself: a detail page that re-derived Market fields on its own would be a
 * second, unchecked layout owner. Sub-state that cannot be read is carried as
 * an explicit refusal with its reason, never as an empty-but-fine section.
 */

/**
 * What a phase means for what the Market will accept, stated from the
 * canonical Market and kernel transitions rather than from product language.
 */
export const MARKET_PHASE_MEANING_V1: Readonly<Record<MarketCorePhaseV2, string>> = Object.freeze({
  Founding: 'The market exists but is not running yet. Nobody holds a claim, it holds no collateral, and it cannot be given an answer.',
  Open: 'The market is trading. Put collateral in and you get one claim on every outcome \u2014 a complete set; hand a complete set back and you take the collateral out again. No answer has been recorded yet, so nothing can be cashed in.',
  Terminal: 'The answer is in. Exactly one outcome won, and that is now fixed. Claims can no longer be created or unwound; whoever holds the winning claim can cash it in, one unit of collateral per claim.',
  Retiring: 'The market is closing down and no new claims can be created. If it got an answer, winning claims can still be cashed in; if it never got one, it is holding nothing to pay out.',
  Retired: 'Finished. Nothing is left in it, and nothing more can be done with it.',
});

/** What the exact required backing is measured against at this phase. */
export const REQUIRED_BACKING_MEANING_V1: Readonly<Record<RequiredBackingBasisV2, string>> = Object.freeze({
  'maximum-claim-supply': 'biggest claim count on any one outcome \u2014 until the answer is in, every claim could still be the one that pays',
  'winning-claim-supply': 'claim count on the outcome that won \u2014 the answer is recorded, so only winning claims can still be paid',
});

export function marketPhaseMeaningV1(phase: MarketCorePhaseV2): string {
  return MARKET_PHASE_MEANING_V1[phase];
}

export function requiredBackingMeaningV1(basis: RequiredBackingBasisV2): string {
  return REQUIRED_BACKING_MEANING_V1[basis];
}

/** The provenance chip the Realm section carries. */
export function realmProvenanceV1(collateral: MarketCollateralV1): MarketProvenanceV1 {
  return collateral.status === 'bound'
    ? Object.freeze({ kind: 'chain', observedSlot: collateral.observedSlot })
    : Object.freeze({ kind: 'refused', reason: collateral.reason });
}

/**
 * The provenance chip the liabilities section carries.
 *
 * `unread` is a refusal to assert, not a blank: a Market root holds no supply
 * vector, so with no Claims program selected nothing about issued claims may be
 * stated.
 */
export function liabilityProvenanceV1(liability: MarketLiabilityV1): MarketProvenanceV1 {
  return liability.status === 'bound'
    ? Object.freeze({ kind: 'chain', observedSlot: liability.observedSlot })
    : Object.freeze({ kind: 'refused', reason: liability.reason });
}

/**
 * The provenance chip the capability section carries.
 *
 * An unread manifest is a refusal to assert, not a blank section: without a
 * Registry authority nothing about a Market's capabilities may be claimed.
 */
export function capabilityProvenanceV1(capabilities: MarketCapabilityManifestV1): MarketProvenanceV1 {
  return capabilities.status === 'authenticated'
    ? Object.freeze({ kind: 'chain', observedSlot: capabilities.observedSlot })
    : Object.freeze({ kind: 'refused', reason: capabilities.reason });
}

export type MarketDetailRequestV1 = Readonly<{
  coreProgramId: string;
  registryProgramId?: string | null;
  claimsProgramId?: string | null;
  custodyProgramId?: string | null;
  address: string;
}>;

export type MarketDetailV1 = Readonly<{
  coreProgramId: string;
  registryProgramId: string | null;
  claimsProgramId: string | null;
  custodyProgramId: string | null;
  floorSlot: string;
  address: string;
  card: MarketDiscoveryCardV1;
  phaseMeaning: string | null;
  realmProvenance: MarketProvenanceV1;
  liabilityProvenance: MarketProvenanceV1;
  capabilityProvenance: MarketProvenanceV1;
  reason: string;
}>;

/** Read one Market, its content-addressed Realm, and its capability manifest. */
export async function inspectMarketDetailV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>,
  request: MarketDetailRequestV1,
): Promise<MarketDetailV1> {
  const discovery = await inspectMarketDiscoveryV1(client, {
    coreProgramId: request.coreProgramId,
    registryProgramId: request.registryProgramId ?? null,
    claimsProgramId: request.claimsProgramId ?? null,
    custodyProgramId: request.custodyProgramId ?? null,
    addresses: [request.address],
  });
  const card = discovery.cards[0];
  if (card === undefined) throw new Error('Market detail requested exactly one address and received no card');
  const refusedSection = (reason: string): MarketProvenanceV1 => Object.freeze({ kind: 'refused', reason });
  return Object.freeze({
    coreProgramId: discovery.coreProgramId,
    registryProgramId: discovery.registryProgramId,
    claimsProgramId: discovery.claimsProgramId,
    custodyProgramId: discovery.custodyProgramId,
    floorSlot: discovery.floorSlot,
    address: card.address,
    card,
    phaseMeaning: card.status === 'decoded' ? marketPhaseMeaningV1(card.phase) : null,
    realmProvenance: card.status === 'decoded'
      ? realmProvenanceV1(card.collateral)
      : refusedSection('The market account did not read, so its collateral setup was never looked up.'),
    liabilityProvenance: card.status === 'decoded'
      ? liabilityProvenanceV1(card.liability)
      : refusedSection('The market account did not read, so we could not work out where its claims ledger is.'),
    capabilityProvenance: card.status === 'decoded'
      ? capabilityProvenanceV1(card.capabilities)
      : refusedSection('The market account did not read, so there is no list of what it can do to check.'),
    reason: card.status === 'decoded'
      ? `Market ${card.address} decoded at finalized floor ${discovery.floorSlot}.`
      : `Market ${card.address} refused at finalized floor ${discovery.floorSlot}: ${card.refusal}`,
  });
}
