import { PublicKey } from '@solana/web3.js';

import { u16, u64 } from './bytes';
import {
  inspectMarketDiscoveryV1,
  type MarketCapabilityManifestV1,
  type MarketCollateralV1,
  type MarketDiscoveryCardV1,
  type MarketLiabilityV1,
  type MarketProvenanceV1,
} from './marketDiscovery';
import { decodeClaimsPositionV2, type MarketCorePhaseV2 } from './marketCoreV2';
import {
  PROTOCOL_POSITION_ADMISSION_SEED_V2,
  PROTOCOL_POSITION_STATE_SEED_V2,
} from './generated/directParticipantV1';
import { type RequiredBackingBasisV2 } from './marketDiscovery';
import { lamportsAsSolV1 } from './openerTerms';
import { type SolanaRpcClient } from './rpc';
import {
  PROTOCOL_POSITION_CLAIMS_CAPABILITY_SEED_V2,
} from './generated/protocolConstantsV1';
import {
  POSITION_ADMISSION_POSITION_LAMPORTS_OFFSET_V2,
  POSITION_ADMISSION_POSITION_RENT_OFFSET_V2,
  PROTOCOL_POSITION_ADMISSION_BYTES_V2,
  PROTOCOL_POSITION_ADMISSION_MAGIC_V2,
  PROTOCOL_POSITION_ADMISSION_SEED_V2,
  PROTOCOL_POSITION_STATE_SEED_V2,
  PROTOCOL_POSITION_WIRE_VERSION_V2,
} from './generated/directParticipantV1';

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
  /**
   * What the founder staked on their own oracle, when the caller read it.
   *
   * The bond is the other half of the outage answer and it points the opposite
   * way from the failure column: the column is what the founder would have
   * been PAID under the legacy scale, and the bond is what they LOSE if the
   * feed they chose goes quiet. `null` is unread, never "none" -- and the
   * value's own `bondLamports` distinguishes a market that posted nothing
   * (`'0'`) from an escrow this page could not read (`null`).
   */
  founderBond: FounderBondV1 | null;
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
    /**
     * This market's founder bond, from `founderBondV1`. Absent is unread: the
     * disclosure then says nothing about a bond rather than denying one.
     */
    founderBond?: FounderBondV1 | null;
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
  // THE BOND IS PART OF THE OUTAGE ANSWER, so it is appended to the headline
  // rather than left as a field the caller may forget to render. A reader who
  // is told what an outage pays and not told what the founder loses by it has
  // been told the smaller half.
  const founderBond = input.founderBond ?? null;
  // THREE ARMS, LIKE `payee`, AND IT USED TO HAVE TWO. `refundsOnFailure` is
  // `null` when the caller has not read this market's payout scale, and the
  // two-armed version answered a null by stating the LEGACY outcome as fact:
  // a reader of a refunding market was told, in the page's own derived voice
  // and directly beside a `payee` sentence saying the scale had not been
  // read, that the whole collateral goes to whoever holds the failure claim.
  // That is the one sentence on this page a buyer cannot afford to have
  // wrong, and an unread fact must read as unread.
  const headline = refundsOnFailure === true
    ? `If the data source never reports, this market settles on outcome ${failureOutcome} \u2014 its failure outcome \u2014 and HOLDERS ARE REFUNDED: the collateral goes back to whoever holds an ordinary outcome, whichever of them would have been right. The failure claim is paid nothing.`
    : refundsOnFailure === false
      ? `If the data source never reports, this market settles on outcome ${failureOutcome} \u2014 its failure outcome \u2014 and the whole collateral is paid to whoever holds that claim. Everyone holding one of the other outcomes is paid nothing, whichever of them would have been right.`
      : `If the data source never reports, this market settles on outcome ${failureOutcome} \u2014 its failure outcome. WHO THAT PAYS DEPENDS ON THIS MARKET'S PAYOUT SCALE, and this page has not read it: at the legacy scale the whole collateral goes to whoever holds the failure claim, and at the refunding scale the failure claim is paid nothing and the collateral goes back to the ordinary holders. Read the market's own basis record before trading on either answer.`;
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
    founderBond,
    headline: founderBond === null ? headline : `${headline} ${founderBond.sentence}`,
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

/**
 * The two accounts this market's failure escrow keeps, derived rather than read.
 *
 * A Position and its admission, at `ProtocolPositionSeedsV2(aggregate, owner)`
 * and `ProtocolPositionAdmissionSeedsV2(aggregate, owner)` with the owner
 * `failureEscrowOwnerV1` derives. The Position holds the lamports; the
 * admission holds what the FOUNDING recorded about them, which is the only
 * thing that makes today's balance readable as a bond rather than as rent.
 */
export function failureEscrowAccountsV1(
  claimsProgramId: string,
  aggregateAddress: string,
  escrowOwner: string,
): Readonly<{ position: string; admission: string }> {
  const claims = new PublicKey(claimsProgramId);
  const seeds = [new PublicKey(aggregateAddress).toBytes(), new PublicKey(escrowOwner).toBytes()];
  return Object.freeze({
    position: PublicKey.findProgramAddressSync([PROTOCOL_POSITION_STATE_SEED_V2, ...seeds], claims)[0].toBase58(),
    admission: PublicKey.findProgramAddressSync([PROTOCOL_POSITION_ADMISSION_SEED_V2, ...seeds], claims)[0].toBase58(),
  });
}

/** Which way the bond leaves the escrow once the answer is in. */
export type FounderBondExitV1 = 'honest' | 'exhausted';

/**
 * What the founder staked on their own oracle, and which way it will go.
 *
 * `bondLamports` HAS THREE STATES AND THEY ARE NOT INTERCHANGEABLE. `null` is
 * "this page did not read the escrow" and asserts nothing. `'0'` is a read
 * fact: this market posted no bond, because it is categorical (decision 0033
 * seats the bond in the failure escrow, and a categorical founding seats no
 * escrow) or because its escrow holds nothing above the rent it recorded.
 * Collapsing the two would let an unreachable RPC read on the page as a
 * founder who staked nothing, which is a claim about a person.
 */
export type FounderBondV1 = Readonly<{
  /** Lamports the escrow holds ABOVE its recorded rent, today. */
  bondLamports: string | null;
  /** `position_rent_principal`: what the founding recorded as the rent. */
  recordedRentLamports: string | null;
  /** `observed_position_lamports`: rent plus bond, as of the founding. */
  postedAtFoundingLamports: string | null;
  /** Which exit the answer selects, or `null` while the answer is not in. */
  exit: FounderBondExitV1 | null;
  /** The whole fact in one sentence, in the disclosure's voice. */
  sentence: string;
}>;

/** Canonical unsigned decimal lamports, or `null` -- never a silent zero. */
function lamportsTextV1(value: string | null): bigint | null {
  return value === null || !/^(0|[1-9][0-9]*)$/.test(value) ? null : BigInt(value);
}

/**
 * What the escrow's admission recorded at founding, or `null` if it is not one.
 *
 * The two numbers are read at the offsets the kernel names --
 * `EVIDENCE_POSITION_RENT_OFFSET` and `EVIDENCE_POSITION_LAMPORTS_OFFSET` in
 * `crates/dclutch-claims/src/protocol_position_v2.rs` -- through the generated
 * mirror, so neither is a literal typed on this side.
 */
function escrowFoundingRecordV1(bytes: Uint8Array | null): Readonly<{ rent: bigint; atFounding: bigint }> | null {
  if (bytes === null || bytes.length !== PROTOCOL_POSITION_ADMISSION_BYTES_V2) return null;
  if (PROTOCOL_POSITION_ADMISSION_MAGIC_V2.some((byte, index) => bytes[index] !== byte)) return null;
  if (u16(bytes, PROTOCOL_POSITION_ADMISSION_MAGIC_V2.length) !== PROTOCOL_POSITION_WIRE_VERSION_V2) return null;
  return Object.freeze({
    rent: u64(bytes, POSITION_ADMISSION_POSITION_RENT_OFFSET_V2),
    atFounding: u64(bytes, POSITION_ADMISSION_POSITION_LAMPORTS_OFFSET_V2),
  });
}

/**
 * The founder bond, DERIVED from two accounts and never from a written figure.
 *
 * A refunding market's failure escrow is funded at founding to more than its
 * own rent, and the excess is the founder's bond: capital they put up against
 * the oracle THEY chose. Nothing on chain stores the bond as a field. What is
 * stored is the escrow admission's `position_rent_principal` -- the rent the
 * founding paid and recorded in the same instruction -- so the bond is the
 * subtraction, and it is exact because both sides are u64 lamports.
 *
 * WHICH WAY IT GOES IS THE ANSWER'S TO DECIDE, not the founder's. On an
 * ordinary winner the bond is theirs and comes back with the escrow's rent at
 * retirement. On the failure outcome -- the feed went quiet, on the market's
 * own terms -- it is paid pro rata to the holders of ordinary claims through
 * their own redemptions, each drawing its share and the last drawing the rest.
 * That is the whole point of it: the founder is the one person who cannot
 * profit from their oracle going silent, and this sentence is how a buyer
 * learns that before trading rather than after.
 *
 * The exit is named only on a market whose payout scale the caller READ. An
 * unread scale still gets the amount and both branches, because the amount is
 * a subtraction of two numbers this page holds and does not depend on the
 * scale; only the direction does.
 */
export function founderBondV1(
  input: Readonly<{
    /** The escrow Position's own balance, from the account read. */
    escrowPositionLamports: string | null;
    /** The escrow admission's 512 bytes, from the same read. */
    escrowAdmissionBytes: Uint8Array | null;
    /** `ProductBasisFactsV3.refundsOnFailure`; `null` when it was not read. */
    refundsOnFailure: boolean | null;
    /** The Market's phase, which says whether an honest exit has happened yet. */
    phase: MarketCorePhaseV2 | null;
    /** The outcome that won, or `null` while there is no answer. */
    terminalWinner: number | null;
    /** This market's failure outcome: the last cell. */
    failureOutcome: number;
  }>,
): FounderBondV1 | null {
  if (!Number.isSafeInteger(input.failureOutcome) || input.failureOutcome < 1) return null;
  // R3: the bond lives in the failure escrow and a categorical founding seats
  // none. This is the one arm that may answer without reading an account,
  // because the record itself settles it.
  if (input.refundsOnFailure === false) {
    return Object.freeze({
      bondLamports: '0',
      recordedRentLamports: null,
      postedAtFoundingLamports: null,
      exit: null,
      sentence: 'This market posted no founder bond: it was founded before the bond, and an outage pays whoever holds the failure column.',
    });
  }
  const record = escrowFoundingRecordV1(input.escrowAdmissionBytes);
  if (record === null) {
    return Object.freeze({
      bondLamports: null,
      recordedRentLamports: null,
      postedAtFoundingLamports: null,
      exit: null,
      sentence: 'This page has not read this market’s failure escrow admission — the account the founding recorded its own rent in — so it cannot say whether a founder bond stands behind this oracle, or what it would be worth.',
    });
  }
  const held = lamportsTextV1(input.escrowPositionLamports);
  if (held === null) {
    return Object.freeze({
      bondLamports: null,
      recordedRentLamports: record.rent.toString(),
      postedAtFoundingLamports: record.atFounding.toString(),
      exit: null,
      sentence: `This page read this market’s failure escrow admission, which records ${record.rent.toString()} lamports of rent and ${record.atFounding.toString()} lamports held at founding, but not the escrow Position’s own balance — so it cannot say what the founder’s bond is worth today.`,
    });
  }
  const read = Object.freeze({
    recordedRentLamports: record.rent.toString(),
    postedAtFoundingLamports: record.atFounding.toString(),
  });
  const bond = held > record.rent ? held - record.rent : 0n;
  if (bond === 0n) {
    return Object.freeze({
      ...read,
      bondLamports: '0',
      exit: null,
      sentence: `This market’s failure escrow holds ${held.toString()} lamports, no more than the ${record.rent.toString()} of rent it recorded at founding, so no founder bond stands behind its oracle.`,
    });
  }
  const exit: FounderBondExitV1 | null = input.refundsOnFailure === true && input.terminalWinner !== null
    ? (input.terminalWinner === input.failureOutcome ? 'exhausted' : 'honest')
    : null;
  const amount = `${bond.toString()} lamports (${lamportsAsSolV1(bond)} SOL)`;
  const posted = record.atFounding > record.rent ? record.atFounding - record.rent : 0n;
  // A bond smaller than the one posted is a bond partly DRAWN, which only the
  // exhausted walk can do. Stated as the difference rather than left for a
  // reader to subtract two other rows.
  const drawn = posted > bond
    ? ` Of the ${posted.toString()} lamports posted at founding, ${bond.toString()} are still in the escrow.`
    : '';
  const sentence = exit === null
    ? `The founder posted a bond of ${amount} against their own oracle: returned to the founder on an honest answer, paid pro rata to the holders of ordinary claims if the feed goes quiet.`
    : exit === 'honest'
      ? input.phase === 'Retired'
        ? `The data source reported, so the founder’s bond of ${amount} was theirs: it went back to the founder’s refund wallet, with the escrow’s rent, when this market retired.`
        : `The data source reported, so the founder’s bond of ${amount} is theirs: it goes back to the founder’s refund wallet, with the escrow’s rent, when this market retires.`
      : `The data source never reported, so the founder’s bond is paid out rather than returned: ${amount} go pro rata to the holders of ordinary claims, each redemption drawing its own share and the last one drawing the rest.${drawn}`;
  return Object.freeze({ ...read, bondLamports: bond.toString(), exit, sentence });
}

/** One market's derived failure escrow: the three addresses the host derives. */
export type FailureEscrowV1 = Readonly<{
  /** The failure coordinate: the last cell. */
  failureSelector: number;
  /** The `ClaimsCapability` owner PDA at (market, failure selector); it has no key. */
  owner: string;
  /** The escrow's `LiabilityBasisV2` Position under that owner and the aggregate. */
  position: string;
  /** The escrow's protocol-Position admission record under the same pair. */
  admission: string;
}>;

/**
 * The whole escrow, derived rather than read -- the port of
 * `dclutch_claims::protocol_position_v2::failure_escrow_v1`, which is the one
 * author the host's `BeginRetiring` preflight, the journey census and the
 * checkpointed retirement builder all read. Seeds, in the Rust file's order:
 * owner = `(CLAIMS_CAPABILITY_OWNER_SEED_V2, market, u32le selector)`;
 * position = `(PROTOCOL_POSITION_STATE_SEED_V2, aggregate, owner)`;
 * admission = `(PROTOCOL_POSITION_ADMISSION_SEED_V2, aggregate, owner)`.
 *
 * A width below two seats no escrow (`FailureEscrowErrorV1::Width`) and is
 * refused here the same way rather than derived into nonsense.
 */
export function failureEscrowV1(
  claimsProgramId: string,
  marketAddress: string,
  aggregateAddress: string,
  outcomeCount: number,
): FailureEscrowV1 {
  if (!Number.isSafeInteger(outcomeCount) || outcomeCount < 2) {
    throw new Error('a refunding complete set needs one ordinary coordinate and one failure coordinate; this width seats no escrow');
  }
  const failureSelector = outcomeCount - 1;
  const owner = failureEscrowOwnerV1(claimsProgramId, marketAddress, failureSelector);
  const { position, admission } = failureEscrowAccountsV1(claimsProgramId, aggregateAddress, owner);
  return Object.freeze({ failureSelector, owner, position, admission });
}

/** Whether a market's failure column is seated in its own escrow, read off the escrow's Position. */
export type EscrowSeatingV1 = Readonly<{
  /** The escrow Position exists, is Claims-owned and decodes at this width. */
  present: boolean;
  /** It holds the WHOLE failure column and nothing else, and the column is nonzero. */
  seated: boolean;
  /** What it holds at the failure coordinate. */
  heldAtoms: string;
  /**
   * `seated`, under the name a caller wants -- and it IMPLIES IN ONE DIRECTION
   * ONLY.
   *
   * A refunding founding (v6) seats the whole failure column in exactly this
   * Position and nothing else does, so `true` here is evidence a market
   * refunds: it is what the host's `failure_escrow_v1` derivation
   * (HOST-RETIRE/17B) reads before it lets a market retire.
   *
   * `false` IS NOT EVIDENCE OF THE OPPOSITE, and a caller that treats it as
   * such prints the sentence `outageDisclosureV1` exists to prevent. A market
   * founded to refund BEFORE the founding that seats the escrow was built
   * carries the column with its founder and reads `false` here; the
   * disclosure's own comment says deriving the payee from the seating alone
   * "would tell a buyer on a refunding market that the founder takes
   * everything, which is the opposite of what would happen", and its
   * `refundsOnFailure` input is `ProductBasisFactsV3.refundsOnFailure` -- a
   * payout-scale fact. Pass `true` through when this is `true`; pass the
   * UNREAD `null` when it is not, or read the record.
   */
  refundsOnFailure: boolean;
}>;

/**
 * Derive `refundsOnFailure` from the escrow Position's presence and contents.
 *
 * `account` is the observed escrow Position (or `null` when the address is
 * empty). The decode is `decodeClaimsPositionV2`'s -- the same reader the
 * activity leaderboard uses -- so the offsets have one author.
 */
export function refundsOnFailureFromEscrowV1(
  input: Readonly<{
    escrow: FailureEscrowV1;
    claimsProgramId: string;
    outcomeCount: number;
    supplyAtoms: ReadonlyArray<string>;
    account: Readonly<{ owner: string; executable: boolean; data: Uint8Array }> | null;
  }>,
): EscrowSeatingV1 {
  const absent = Object.freeze({ present: false, seated: false, heldAtoms: '0', refundsOnFailure: false });
  const account = input.account;
  if (account === null || account.executable || account.owner !== input.claimsProgramId) return absent;
  let balances: ReadonlyArray<string>;
  try {
    const position = decodeClaimsPositionV2(input.escrow.position, account.data);
    if (position.claimCount !== input.outcomeCount || position.owner !== input.escrow.owner) return absent;
    balances = position.balances;
  } catch {
    return absent;
  }
  const selector = input.escrow.failureSelector;
  const held = balances[selector] ?? '0';
  let supply: bigint;
  let heldAtoms: bigint;
  try {
    supply = BigInt(input.supplyAtoms[selector] ?? '');
    heldAtoms = BigInt(held);
  } catch {
    return Object.freeze({ present: true, seated: false, heldAtoms: held, refundsOnFailure: false });
  }
  const nothingElse = balances.every((atoms, index) => index === selector || atoms === '0');
  const seated = supply > 0n && heldAtoms === supply && nothingElse;
  return Object.freeze({ present: true, seated, heldAtoms: held, refundsOnFailure: seated });
}
