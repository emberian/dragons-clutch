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
  Founding: 'Not running yet. Nobody holds a claim and it holds no collateral.',
  Open: 'Trading. Put collateral in and you get one claim on every outcome \u2014 a complete set; hand a complete set back and take the collateral out again. No answer yet, so nothing can be cashed in.',
  Terminal: 'The answer is in and one outcome won. Claims can no longer be created or unwound; the winning claim cashes in at one unit of collateral per claim.',
  Retiring: 'Closing down. No new claims. If it got an answer, winning claims can still be cashed in; if not, it is holding nothing.',
  Retired: 'Finished. Nothing is left in it.',
});

/** What the exact required backing is measured against at this phase. */
export const REQUIRED_BACKING_MEANING_V1: Readonly<Record<RequiredBackingBasisV2, string>> = Object.freeze({
  'maximum-claim-supply': 'Measured against the biggest claim count on any one outcome.',
  'winning-claim-supply': 'Measured against the claim count on the outcome that won.',
});

export function marketPhaseMeaningV1(phase: MarketCorePhaseV2): string {
  return MARKET_PHASE_MEANING_V1[phase];
}

/** What a resolved market's answer is, and what it leaves each holder holding. */
export type TerminalOutcomeMeaningV1 = Readonly<{
  /** Whether the outcome that won is the market's source-failure outcome. */
  sourceFailure: boolean;
  /** One sentence naming what won. */
  headline: string;
  /** What it leaves the winning side holding. */
  forTheWinners: string;
  /** What it leaves everybody else holding. */
  forEveryoneElse: string;
}>;

/**
 * The answer, in the words a holder needs, including the one nobody had.
 *
 * A resolved market already said WHICH claim won -- `Resolved — <outcome>` and
 * a `won` / `lost · pays nothing` beside every cell. What it never said is what
 * that IS. Cohort-13 resolved to its source-failure outcome, and a reader who
 * did not already know that the last cell is the failure cell saw only an
 * outcome name and no reason.
 *
 * SOURCE FAILURE IS NOT AN ERROR STATE and the page must not let it read as
 * one. It is the fallback the market wrote down and prepaid for before it
 * opened, so that a silent data source could never strand it. Someone holding
 * that claim is paid exactly the way any other winner is paid.
 *
 * The failure outcome is the LAST one, which is the same rule
 * `derivedOutcomeLabelsV1` places its label by and the same one the terminal
 * payout reads (`terminalWinner === resultOutcomeCount - 1`). It is stated once
 * here rather than a third time at the call site.
 */
export function terminalOutcomeMeaningV1(
  input: Readonly<{ winner: number; outcomeCount: number; outcomeName?: string | undefined }>,
): TerminalOutcomeMeaningV1 {
  const sourceFailure = input.outcomeCount > 0 && input.winner === input.outcomeCount - 1;
  // The failure cell's DERIVED name is the sentence "The source failed to
  // report", so naming it inside a sentence that already says the source never
  // reported says it twice and reads as a stutter. There it is called by its
  // index, which is what a holder matches against their own position anyway.
  const named = sourceFailure || input.outcomeName === undefined || input.outcomeName === ''
    ? `claim ${input.winner}`
    : input.outcomeName;
  return Object.freeze({
    sourceFailure,
    headline: sourceFailure
      ? `The data source never reported. This market did not get stuck: it settled on the fallback outcome it named and paid for before it opened, which is ${named}.`
      : `The data source reported, and ${named} is the outcome that won.`,
    forTheWinners: `Anyone holding ${named} can cash in, at one unit of collateral for every claim atom they hold. Nobody has to ask permission and there is no deadline on it.`,
    forEveryoneElse: 'Every other claim on this market is worth exactly nothing. It is not stuck and it is not pending: the market answered, that claim was not the answer, and it pays zero. Claims can no longer be created or unwound either way.',
  });
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
      : refusedSection('The market account did not read.'),
    liabilityProvenance: card.status === 'decoded'
      ? liabilityProvenanceV1(card.liability)
      : refusedSection('The market account did not read.'),
    capabilityProvenance: card.status === 'decoded'
      ? capabilityProvenanceV1(card.capabilities)
      : refusedSection('The market account did not read.'),
    reason: card.status === 'decoded'
      ? `Market ${card.address} decoded at finalized floor ${discovery.floorSlot}.`
      : `Market ${card.address} refused at finalized floor ${discovery.floorSlot}: ${card.refusal}`,
  });
}
