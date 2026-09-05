import { PublicKey } from '@solana/web3.js';

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
import {
  PROTOCOL_POSITION_CLAIMS_CAPABILITY_SEED_V2,
} from './generated/protocolConstantsV1';

/** Claims `ClaimsCapability` Position-owner seed domain (`protocol_position_v2.rs`). */
const CLAIMS_CAPABILITY_OWNER_SEED_V2 = PROTOCOL_POSITION_CLAIMS_CAPABILITY_SEED_V2;

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

/** Who is paid if this market's data source never reports, read off the chain. */
export type OutageDisclosureV1 = Readonly<{
  /** The failure outcome's index: the last cell, which is the failure cell. */
  failureOutcome: number;
  /** The whole failure column's supply, from the Claims aggregate. */
  supplyAtoms: string;
  /** How much of it the Positions this page could read account for. */
  accountedAtoms: string;
  /** Supply this page could not attribute to any Position it read. */
  unaccountedAtoms: string;
  /** Every reader of the failure column this page saw, largest first. */
  holders: ReadonlyArray<Readonly<{ owner: string; atoms: string; wholeColumn: boolean }>>;
  /** Whether the read accounts for the whole column, so the answer is complete. */
  complete: boolean;
  /** This market's own derived failure escrow, when the caller derived one. */
  failureEscrowOwner: string | null;
  /** How much of the failure column that escrow holds, from the same read. */
  escrowAtoms: string;
  /** Whether the WHOLE failure column is seated in that escrow. */
  escrowSeated: boolean;
  /**
   * Whether an outage REFUNDS the ordinary holders, from the market's
   * authenticated payout scale; `null` when the caller has not read it.
   *
   * This and not the seating is what settles who an outage pays. A refunding
   * basis pays one atom to every ordinary claim and NOTHING to the failure
   * coordinate, whoever holds it, so a market founded to refund refunds even
   * while its failure column still sits with the founder. The seating is the
   * separate question of whether worth-nothing claims are in somebody's hands
   * to sell.
   */
  refundsOnFailure: boolean | null;
  /** Where the failure column actually sits, from the Positions this page read. */
  columnNote: string;
  /** What happens under an outage, in the words a buyer needs before trading. */
  headline: string;
  /** Who is paid, named from the read rather than asserted. */
  payee: string;
}>;

/**
 * What an oracle outage pays, and to whom, DERIVED rather than written down.
 *
 * A market resolves to its failure outcome when the data source never reports,
 * and that outcome's claims are paid exactly the way any other winner's are.
 * So the question "who is paid if the feed goes quiet" has a chain answer:
 * whoever holds the failure column. Nobody trades for a failure claim, so on
 * every market founded so far that is the founder, who also chose the oracle,
 * the window, and whether there is a recovery policy.
 *
 * THIS FUNCTION MUST NEVER BE REPLACED BY A SENTENCE IN THE REGISTRY. The
 * registry is editorial and a founder writes it; the payee under an outage is
 * the one fact a buyer cannot afford to take a founder's word for. It is read
 * from the Claims aggregate's own supply vector and the Position accounts
 * themselves.
 *
 * IT ALSO REPORTS WHAT IT COULD NOT SEE. The Positions this page reads are
 * harvested from the market's recent transactions and capped, so the set is not
 * guaranteed complete -- and a disclosure that quietly presented a partial read
 * as the whole answer would be worse than none. Comparing the failure balances
 * it did read against the aggregate's own supply at that coordinate settles it
 * exactly: equal means the column is fully accounted for, and any shortfall is
 * reported as a number rather than rounded away.
 */
export function outageDisclosureV1(
  input: Readonly<{
    outcomeCount: number;
    supplyAtoms: ReadonlyArray<string>;
    positions: ReadonlyArray<Readonly<{ owner: string; balances: ReadonlyArray<string> }>>;
    /** This market's own failure escrow, from `failureEscrowOwnerV1`. */
    failureEscrowOwner?: string | null;
    /**
     * `ProductBasisFactsV3.refundsOnFailure`, when the caller read the record.
     * Absent means unread, and the disclosure says so rather than guessing.
     */
    refundsOnFailure?: boolean | null;
  }>,
): OutageDisclosureV1 | null {
  if (input.outcomeCount < 2 || input.supplyAtoms.length !== input.outcomeCount) return null;
  const failureOutcome = input.outcomeCount - 1;
  let supply: bigint;
  try {
    supply = BigInt(input.supplyAtoms[failureOutcome] ?? '');
  } catch {
    return null;
  }
  const holders = input.positions
    .map((position) => {
      const raw = position.balances[failureOutcome];
      if (raw === undefined || position.balances.length !== input.outcomeCount) return null;
      let atoms: bigint;
      try {
        atoms = BigInt(raw);
      } catch {
        return null;
      }
      return atoms === 0n ? null : { owner: position.owner, atoms };
    })
    .filter((holder): holder is { owner: string; atoms: bigint } => holder !== null)
    .sort((left, right) => (right.atoms > left.atoms ? 1 : right.atoms < left.atoms ? -1 : 0));
  const accounted = holders.reduce((total, holder) => total + holder.atoms, 0n);
  const unaccounted = supply > accounted ? supply - accounted : 0n;
  const complete = supply > 0n && unaccounted === 0n;
  const named = holders.map((holder) => Object.freeze({
    owner: holder.owner,
    atoms: holder.atoms.toString(),
    wholeColumn: supply > 0n && holder.atoms === supply,
  }));
  const escrowOwner = input.failureEscrowOwner ?? null;
  const escrowAtoms = escrowOwner === null
    ? 0n
    : holders.find((holder) => holder.owner === escrowOwner)?.atoms ?? 0n;
  const escrowSeated = escrowOwner !== null && supply > 0n && escrowAtoms === supply;
  const refundsOnFailure = input.refundsOnFailure ?? null;
  // WHO IS PAID IS THE PAYOUT SCALE'S ANSWER, NOT THE SEATING'S. A refunding
  // basis pays one atom to every ordinary claim and nothing to the failure
  // coordinate, so it refunds whoever holds an ordinary outcome even while the
  // failure column still sits with the founder; a legacy basis pays the failure
  // column to whoever holds it. Deriving the payee from the seating alone would
  // tell a buyer on a refunding market that the founder takes everything, which
  // is the opposite of what would happen.
  const unread = ' This page has not read this market\u2019s payout scale, which is what settles whether the failure claim is paid at all: every market founded before this ruling pays it.';
  const paid = holders.length === 0
    ? `No Position this page could read holds any of the ${supply.toString()} atoms on the failure outcome, so this page cannot say who an outage would pay.`
    : complete && named.length === 1 && named[0]!.wholeColumn
      ? `One holder, ${named[0]!.owner}, holds every one of the ${supply.toString()} atoms on the failure outcome and would be paid all of the collateral.`
      : complete
        ? `${named.length} holders split the ${supply.toString()} atoms on the failure outcome and would be paid in proportion to what each holds.`
        : `The Positions read here account for ${accounted.toString()} of the ${supply.toString()} atoms on the failure outcome; ${unaccounted.toString()} sit in Positions this page did not read, so this is a partial answer.`;
  const payee = supply === 0n
    ? 'Nothing is issued on the failure outcome, so an outage pays nobody.'
    : refundsOnFailure === true
      ? 'This market is founded to REFUND. An outage pays one atom to every ordinary claim and nothing at all to the failure outcome, whoever holds it, so the collateral goes back to the people holding the outcomes \u2014 in proportion to what each holds. Nobody is paid for having chosen the oracle.'
      : refundsOnFailure === false
        ? paid
        : `${paid}${unread}`;
  const columnNote = supply === 0n
    ? 'Nothing is issued on the failure outcome, so there is no failure column to hold.'
    : escrowSeated
      ? `All ${supply.toString()} atoms on the failure outcome are seated in this market\u2019s own escrow, ${escrowOwner}, which is not a person.`
      : escrowOwner !== null && escrowAtoms > 0n
        ? `This market\u2019s escrow ${escrowOwner} holds ${escrowAtoms.toString()} of the ${supply.toString()} atoms on the failure outcome and the rest sits elsewhere: a partly seated escrow.`
        : escrowOwner !== null
          ? `None of the ${supply.toString()} atoms on the failure outcome is seated in this market\u2019s own escrow ${escrowOwner}; the column is held by ordinary Positions.`
          : `This page did not derive this market\u2019s failure escrow, so it says nothing about where the failure column is seated.`;
  return Object.freeze({
    failureOutcome,
    supplyAtoms: supply.toString(),
    accountedAtoms: accounted.toString(),
    unaccountedAtoms: unaccounted.toString(),
    holders: Object.freeze(named),
    complete,
    failureEscrowOwner: escrowOwner,
    escrowAtoms: escrowAtoms.toString(),
    escrowSeated,
    refundsOnFailure,
    columnNote,
    headline: refundsOnFailure === true
      ? `If the data source never reports, this market settles on outcome ${failureOutcome} \u2014 its failure outcome \u2014 and HOLDERS ARE REFUNDED: the collateral goes back to whoever holds an ordinary outcome, whichever of them would have been right. The failure claim is paid nothing.`
      : `If the data source never reports, this market settles on outcome ${failureOutcome} \u2014 its failure outcome \u2014 and the whole collateral is paid to whoever holds that claim. Everyone holding one of the other outcomes is paid nothing, whichever of them would have been right.`,
    payee,
  });
}

/**
 * This market's own failure escrow, derived rather than read.
 *
 * Decision 0025 seats a refunding market's failure coordinate in a Position
 * owned by an identity the MARKET derives and nobody controls: the Claims
 * `ClaimsCapability` owner PDA at (market, failure selector), which is the same
 * derivation `authenticate_failure_escrow` checks on chain and the same seed
 * domain the rational-representation custody owner already uses.
 *
 * It is a pure derivation from two addresses this page already holds, so the
 * disclosure costs no extra account read. What the page then states is whether
 * the failure column is ACTUALLY seated there -- which is the fact a buyer
 * needs, and a stronger one than the record's intent.
 */
export function failureEscrowOwnerV1(claimsProgramId: string, marketAddress: string, failureOutcome: number): string {
  if (!Number.isSafeInteger(failureOutcome) || failureOutcome < 0 || failureOutcome > 0xffffffff) {
    throw new Error('failure outcome is not an exact u32 selector');
  }
  const selector = new Uint8Array(4);
  new DataView(selector.buffer).setUint32(0, failureOutcome, true);
  return PublicKey.findProgramAddressSync(
    [CLAIMS_CAPABILITY_OWNER_SEED_V2, new PublicKey(marketAddress).toBytes(), selector],
    new PublicKey(claimsProgramId),
  )[0].toBase58();
}
