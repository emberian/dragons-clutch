import { describe, expect, it } from 'vitest';

import { screenBoardOffersV1, type BoardScreenContextV1 } from './tradeFlowBoard';
import { type BoardOfferV1, type TicketBoardListingV1 } from './deploymentTicketBoard';

const MAKER_V1 = '8bcRzB3v6PxbbtkVCiX9ceW2whwakA6gX7qvSYbeMHLq';
const MARKET_V1 = '5F8wMRFMdYGMkjWQUye6WfbgRVWEo9yyKo9aFPk2TLaD';
const COLLATERAL_V1 = '7xwJ3uceuBV7KyCsdJsBs9Ljfh1bL3WB7NbGpwUNeJ2o';
const READER_V1 = '4fQNy8k7G7bZ9cak6pb2VnigV2F5fbhs7YnYFWQ2LQYH';

/** One board offer, admissible unless the caller bends a field. */
function offerV1(
  digest: string,
  intent: Partial<BoardOfferV1['ticket']['intent']> = {},
  maker = MAKER_V1,
): BoardOfferV1 {
  return Object.freeze({
    digest,
    text: `{"digest":"${digest}"}`,
    postedAtSlot: 100n,
    ticket: Object.freeze({
      maker,
      signature: new Uint8Array(64).fill(7),
      intent: Object.freeze({
        side: 0 as const,
        lifecycle: 1 as const,
        outcome: 0,
        market: MARKET_V1,
        generation: 7n,
        nonce: 9n,
        validFrom: 0n,
        validThrough: 4_294_967_295n,
        maximumFill: 100_000_000n,
        limitPrice: 500_000n,
        feeBasisPoints: 50,
        collateralAccount: COLLATERAL_V1,
        ...intent,
      }),
    }),
  }) as BoardOfferV1;
}

function listingV1(offers: ReadonlyArray<BoardOfferV1>): TicketBoardListingV1 {
  return Object.freeze({
    offers, slotBasis: 1_000n, droppedExpired: 0, refused: [],
  }) as TicketBoardListingV1;
}

const CONTEXT_V1: BoardScreenContextV1 = Object.freeze({
  connectedWallet: READER_V1,
  generation: 7n,
  feeBasisPoints: 50,
  outcomeCount: 2,
  outcome: 0,
  finalizedSlot: 1_000n,
});

const hiddenReasons = (offers: ReadonlyArray<BoardOfferV1>, context = CONTEXT_V1) =>
  screenBoardOffersV1(listingV1(offers), context).hidden.map((drop) => drop.reason);

describe('the board screen', () => {
  it('shows an offer the flow would actually accept', () => {
    const screen = screenBoardOffersV1(listingV1([offerV1('a')]), CONTEXT_V1);
    expect(screen.offers.map((offer) => offer.digest)).toEqual(['a']);
    expect(screen.hidden).toEqual([]);
  });

  /**
   * Every one of these produces a real, correct, named refusal two or three
   * steps later. A board that shows them is a board that spends a reader's
   * attention walking them toward a wall it could already see.
   */
  it('holds back every offer the flow would refuse, and says which is which', () => {
    expect(hiddenReasons([offerV1('buy', { side: 1 as const })])).toEqual(['buy-side']);
    expect(hiddenReasons([offerV1('mine', {}, READER_V1)])).toEqual(['self-authored']);
    expect(hiddenReasons([offerV1('wide', { outcome: 9 })])).toEqual(['outcome-width']);
    expect(hiddenReasons([offerV1('other', { outcome: 1 })])).toEqual(['wrong-outcome']);
    expect(hiddenReasons([offerV1('old', { generation: 6n })])).toEqual(['generation']);
    expect(hiddenReasons([offerV1('fee', { feeBasisPoints: 10 })])).toEqual(['fee']);
    expect(hiddenReasons([offerV1('gone', { validThrough: 999n })])).toEqual(['expired']);
    expect(hiddenReasons([offerV1('early', { validFrom: 1_001n })])).toEqual(['not-yet-valid']);
  });

  it('gives every held-back offer a reason a person could read', () => {
    const screen = screenBoardOffersV1(listingV1([
      offerV1('buy', { side: 1 as const }),
      offerV1('mine', {}, READER_V1),
      offerV1('gone', { validThrough: 999n }),
    ]), CONTEXT_V1);
    expect(screen.offers).toEqual([]);
    for (const drop of screen.hidden) {
      expect(drop.detail.length).toBeGreaterThan(20);
      expect(drop.detail.endsWith('.')).toBe(true);
    }
    expect(screen.hidden[1]!.detail).toContain('You signed this offer');
    expect(screen.hidden[2]!.detail).toContain('expired at slot 999');
  });

  /**
   * Every surviving offer is one the reader would be BUYING from, so cheaper
   * is unambiguously better for them and "best first" means something.
   */
  it('sorts the offers a reader can take cheapest first', () => {
    const screen = screenBoardOffersV1(listingV1([
      offerV1('dear', { limitPrice: 800_000n }),
      offerV1('cheap', { limitPrice: 100_000n }),
      offerV1('middle', { limitPrice: 500_000n }),
    ]), CONTEXT_V1);
    expect(screen.offers.map((offer) => offer.digest)).toEqual(['cheap', 'middle', 'dear']);
  });

  it('breaks a price tie on the digest, so the order never wobbles between reads', () => {
    const first = screenBoardOffersV1(listingV1([offerV1('b'), offerV1('a')]), CONTEXT_V1);
    const second = screenBoardOffersV1(listingV1([offerV1('a'), offerV1('b')]), CONTEXT_V1);
    expect(first.offers.map((offer) => offer.digest)).toEqual(['a', 'b']);
    expect(second.offers.map((offer) => offer.digest)).toEqual(['a', 'b']);
  });

  it('shows every admissible claim when no claim is picked yet', () => {
    const screen = screenBoardOffersV1(listingV1([offerV1('zero'), offerV1('one', { outcome: 1 })]), {
      ...CONTEXT_V1, outcome: null,
    });
    expect(screen.offers.map((offer) => offer.digest).sort()).toEqual(['one', 'zero']);
  });

  it('skips the two time drops when no finalized slot is known, rather than guessing one', () => {
    const screen = screenBoardOffersV1(listingV1([offerV1('gone', { validThrough: 1n })]), {
      ...CONTEXT_V1, finalizedSlot: null,
    });
    expect(screen.offers.map((offer) => offer.digest)).toEqual(['gone']);
  });

  it('carries the relay’s own sweep and decode counts through for the drawer', () => {
    const screen = screenBoardOffersV1(Object.freeze({
      offers: [offerV1('a')],
      slotBasis: 1_000n,
      droppedExpired: 4,
      refused: [{ digest: 'x', reason: 'ticket is not valid JSON' }],
    }) as TicketBoardListingV1, CONTEXT_V1);
    expect(screen.droppedExpired).toBe(4);
    expect(screen.refusedCount).toBe(1);
  });

  it('cannot hide an offer from a reader who has no wallet connected', () => {
    const screen = screenBoardOffersV1(listingV1([offerV1('mine', {}, READER_V1)]), {
      ...CONTEXT_V1, connectedWallet: null,
    });
    expect(screen.offers.map((offer) => offer.digest)).toEqual(['mine']);
  });
});
