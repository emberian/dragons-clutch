import { type BoardOfferV1, type TicketBoardListingV1 } from '@/lib/ticketBoard';

/**
 * The screen the board must pass before it renders.
 *
 * **The board must never show an offer the flow would refuse two steps later.**
 * A relay cannot forge a ticket, but it can happily hand over one that is
 * expired, signed for a different generation, authored by the reader
 * themselves, or buy-side in a build whose wallet preparation only takes
 * sell-side. Every one of those produces a real, correct, named refusal at
 * step 5 or 6 -- and a refusal a reader walked four steps to reach is a
 * refusal the surface should have spent nothing to prevent.
 *
 * So the filters live HERE, on the client, where the route context and the
 * connected wallet are, and they run before render rather than after a click.
 * The relay is not asked to be trusted with any of it: this is the taker
 * checking candidates, which is exactly the permitted category -- callers may
 * supply hints and candidates, and the release-selected state verifies every
 * authoritative identity itself.
 *
 * Each drop is SILENT in the list and COUNTABLE in the drawer. Silent because
 * a list of things you cannot have is not a list of offers; countable because
 * "3 offers hidden -- why?" is the difference between a filter and a lie.
 */

/** Why one offer did not reach the list. */
export type BoardDropReasonV1 =
  | 'expired'
  | 'not-yet-valid'
  | 'wrong-outcome'
  | 'outcome-width'
  | 'generation'
  | 'fee'
  | 'self-authored'
  | 'buy-side';

/** One offer, and the reason the reader is not being shown it. */
export type BoardDropV1 = Readonly<{
  offer: BoardOfferV1;
  reason: BoardDropReasonV1;
  /** One sentence, in the reader's terms, for the drawer. */
  detail: string;
}>;

export type BoardScreenV1 = Readonly<{
  /** Offers the flow would accept, best for the reader first. */
  offers: ReadonlyArray<BoardOfferV1>;
  /** Offers held back, with the reason each was. */
  hidden: ReadonlyArray<BoardDropV1>;
  /** Expired offers the relay itself already swept, which never reached us. */
  droppedExpired: number;
  /** Offers the relay could not decode at all. Its count, not its contents. */
  refusedCount: number;
}>;

/** The route and reader facts a drop decision needs. */
export type BoardScreenContextV1 = Readonly<{
  /** The connected wallet, for the self-authored drop. Null when none is. */
  connectedWallet: string | null;
  /** The Market's current generation, from the authenticated spine. */
  generation: bigint;
  /** The Market's immutable fee, from the Direct config record. */
  feeBasisPoints: number;
  /** The Product's width: an outcome at or above it cannot exist. */
  outcomeCount: number;
  /** The claim the reader picked. Null shows every admissible outcome. */
  outcome: number | null;
  /** The finalized slot validity is judged against. Null skips the two time drops. */
  finalizedSlot: bigint | null;
}>;

function dropReasonV1(
  offer: BoardOfferV1,
  context: BoardScreenContextV1,
): Readonly<{ reason: BoardDropReasonV1; detail: string }> | null {
  const { intent } = offer.ticket;
  if (intent.side !== 0) {
    return Object.freeze({
      reason: 'buy-side' as const,
      detail: 'This is a buy offer. This build crosses your wallet as the buyer against a maker’s sell offer, and it will not silently reverse the two roles.',
    });
  }
  if (context.connectedWallet !== null && offer.ticket.maker === context.connectedWallet) {
    return Object.freeze({
      reason: 'self-authored' as const,
      detail: 'You signed this offer. A Direct fill settles two distinct makers against each other, so you cannot take your own.',
    });
  }
  if (intent.outcome >= context.outcomeCount) {
    return Object.freeze({
      reason: 'outcome-width' as const,
      detail: `This offer names claim ${intent.outcome}, and this Market’s Product is only ${context.outcomeCount} claims wide.`,
    });
  }
  if (context.outcome !== null && intent.outcome !== context.outcome) {
    return Object.freeze({
      reason: 'wrong-outcome' as const,
      detail: `This offer is signed for claim ${intent.outcome}, and you picked claim ${context.outcome}.`,
    });
  }
  if (intent.generation !== context.generation) {
    return Object.freeze({
      reason: 'generation' as const,
      detail: `This offer was signed for generation ${intent.generation.toString()}, and this Market is now at generation ${context.generation.toString()}.`,
    });
  }
  if (intent.feeBasisPoints !== context.feeBasisPoints) {
    return Object.freeze({
      reason: 'fee' as const,
      detail: `This offer was signed at ${intent.feeBasisPoints} bps, and this Market’s immutable fee is ${context.feeBasisPoints} bps.`,
    });
  }
  if (context.finalizedSlot !== null) {
    if (intent.validThrough < context.finalizedSlot) {
      return Object.freeze({
        reason: 'expired' as const,
        detail: `This offer expired at slot ${intent.validThrough.toString()}, and the chain is finalized through slot ${context.finalizedSlot.toString()}.`,
      });
    }
    if (intent.validFrom > context.finalizedSlot) {
      return Object.freeze({
        reason: 'not-yet-valid' as const,
        detail: `This offer opens at slot ${intent.validFrom.toString()}, which the chain has not reached yet.`,
      });
    }
  }
  return null;
}

/**
 * Screen and order one listing.
 *
 * Ordering is by the ticket's signed limit price, ascending, because every
 * offer that survives the screen is one the reader would be BUYING from -- so
 * cheaper is unambiguously better for them, and "best first" has a meaning
 * rather than being a preference the surface picked.
 */
export function screenBoardOffersV1(
  listing: TicketBoardListingV1,
  context: BoardScreenContextV1,
): BoardScreenV1 {
  const offers: BoardOfferV1[] = [];
  const hidden: BoardDropV1[] = [];
  for (const offer of listing.offers) {
    const drop = dropReasonV1(offer, context);
    if (drop === null) offers.push(offer);
    else hidden.push(Object.freeze({ offer, reason: drop.reason, detail: drop.detail }));
  }
  offers.sort((left, right) => {
    if (left.ticket.intent.limitPrice < right.ticket.intent.limitPrice) return -1;
    if (left.ticket.intent.limitPrice > right.ticket.intent.limitPrice) return 1;
    return left.digest < right.digest ? -1 : left.digest > right.digest ? 1 : 0;
  });
  return Object.freeze({
    offers: Object.freeze(offers),
    hidden: Object.freeze(hidden),
    droppedExpired: listing.droppedExpired,
    refusedCount: listing.refused.length,
  });
}
