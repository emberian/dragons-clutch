import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import {
  boardHealthV1,
  listBoardOffersV1,
  postBoardOfferV1,
  ticketBoardConfigV1,
  TicketBoardError,
  type TicketBoardFetchV1,
} from './ticketBoard';

/**
 * The board client, held to the two rules that keep a relay from becoming an
 * authority.
 *
 * The tests that matter here are the ones where the BOARD LIES. A relay can
 * withhold and a relay can never forge, but only because every consumer
 * re-derives the truth itself — so this file spends most of its length proving
 * that a board serving a tampered ticket, a mislabelled digest, or an outright
 * fabrication cannot get that past this module into the flow.
 */

const VECTOR = JSON.parse(
  readFileSync(new URL('../fixtures/direct-intent-ticket.json', import.meta.url), 'utf8'),
) as Readonly<{ ticketText: string; maker: string; market: string; outcome: number }>;

const BOARD = { baseUrl: 'https://board.example' } as const;

/** A fetch that answers one canned response and records what it was asked. */
function stubFetch(
  status: number,
  body: unknown,
  calls: { url?: string; method?: string; body?: string } = {},
): TicketBoardFetchV1 {
  return (url, init) => {
    calls.url = url;
    calls.method = init?.method ?? 'GET';
    calls.body = init?.body;
    return Promise.resolve({
      ok: status >= 200 && status < 300,
      status,
      text: () => Promise.resolve(typeof body === 'string' ? body : JSON.stringify(body)),
    });
  };
}

function offer(text: string, digest = 'a'.repeat(64), postedAtSlot: string | null = '1200') {
  return { digest, text, postedAtSlot };
}

describe('ticketBoardConfigV1', () => {
  it('treats an absent board as a supported state and never an error', () => {
    for (const absent of [undefined, null, '', '   ']) {
      expect(ticketBoardConfigV1(absent)).toBeNull();
    }
  });

  it('trims a trailing slash so one URL cannot become two', () => {
    expect(ticketBoardConfigV1('https://board.example/')?.baseUrl).toBe('https://board.example');
    expect(ticketBoardConfigV1('https://board.example///')?.baseUrl).toBe('https://board.example');
  });

  it('refuses something that is not an http origin rather than fetching it', () => {
    for (const wrong of ['board.example', 'javascript:alert(1)', 'file:///etc/passwd']) {
      expect(() => ticketBoardConfigV1(wrong)).toThrow(TicketBoardError);
    }
  });
});

describe('listBoardOffersV1', () => {
  it('asks for exactly the market, outcome and slot it was given', async () => {
    const calls: { url?: string } = {};
    await listBoardOffersV1(
      BOARD,
      { market: VECTOR.market, outcome: 3, slot: 1234n },
      stubFetch(200, { offers: [], slotBasis: '1234', droppedExpired: 0 }, calls),
    );
    expect(calls.url).toBe(
      `https://board.example/tickets?market=${VECTOR.market}&outcome=3&slot=1234`,
    );
  });

  it('decodes a served offer into the SignedDirectIntentV3 step 3 contracts for', async () => {
    const listing = await listBoardOffersV1(
      BOARD,
      { market: VECTOR.market },
      stubFetch(200, {
        offers: [offer(VECTOR.ticketText)],
        slotBasis: '900',
        droppedExpired: 2,
      }),
    );
    expect(listing.offers).toHaveLength(1);
    expect(listing.refused).toHaveLength(0);
    expect(listing.offers[0].ticket.maker).toBe(VECTOR.maker);
    expect(listing.offers[0].ticket.intent.outcome).toBe(VECTOR.outcome);
    expect(listing.offers[0].ticket.intent.market).toBe(VECTOR.market);
    expect(listing.offers[0].text).toBe(VECTOR.ticketText);
    expect(listing.slotBasis).toBe(900n);
    expect(listing.droppedExpired).toBe(2);
  });

  // THE LOAD-BEARING TEST. The board's parse never reaches the flow.
  it('refuses an offer the local decoder rejects instead of rendering it', async () => {
    const tampered = VECTOR.ticketText.replace('"feeBasisPoints": 50', '"feeBasisPoints": 20000');
    expect(tampered).not.toBe(VECTOR.ticketText);

    const listing = await listBoardOffersV1(
      BOARD,
      { market: VECTOR.market },
      stubFetch(200, {
        offers: [offer(tampered, 'b'.repeat(64)), offer(VECTOR.ticketText)],
        slotBasis: null,
        droppedExpired: 0,
      }),
    );
    // The honest one survives; the broken one is reported, not rendered.
    expect(listing.offers).toHaveLength(1);
    expect(listing.offers[0].text).toBe(VECTOR.ticketText);
    expect(listing.refused).toHaveLength(1);
    expect(listing.refused[0].digest).toBe('b'.repeat(64));
    expect(listing.refused[0].reason).toContain('fee basis points');
  });

  it('refuses an offer the board serves with no ticket text at all', async () => {
    const listing = await listBoardOffersV1(
      BOARD,
      { market: VECTOR.market },
      stubFetch(200, { offers: [{ digest: 'c'.repeat(64) }], droppedExpired: 0, slotBasis: null }),
    );
    expect(listing.offers).toHaveLength(0);
    expect(listing.refused[0].reason).toContain('no ticket text');
  });

  it('carries the board refusal sentence rather than flattening it to a failure', async () => {
    await expect(
      listBoardOffersV1(
        BOARD,
        { market: VECTOR.market },
        stubFetch(400, { accepted: false, refusal: 'QUERY_INVALID', reason: 'a named sentence' }),
      ),
    ).rejects.toThrow('a named sentence');
  });

  it('names an unreachable board rather than throwing whatever fetch threw', async () => {
    await expect(
      listBoardOffersV1(BOARD, { market: VECTOR.market }, () => Promise.reject(new Error('ECONNREFUSED'))),
    ).rejects.toThrow('the offer board did not answer');
  });

  it('refuses a board that answers with something that is not JSON', async () => {
    await expect(
      listBoardOffersV1(BOARD, { market: VECTOR.market }, stubFetch(200, '<html>nope</html>')),
    ).rejects.toThrow('not JSON');
  });

  it('refuses a slot the board wrote as a number instead of canonical text', async () => {
    await expect(
      listBoardOffersV1(
        BOARD,
        { market: VECTOR.market },
        stubFetch(200, { offers: [], slotBasis: 1234, droppedExpired: 0 }),
      ),
    ).rejects.toThrow('canonical unsigned decimal');
  });
});

describe('postBoardOfferV1', () => {
  it('sends the authored bytes verbatim and returns the digest', async () => {
    const calls: { url?: string; method?: string; body?: string } = {};
    const posted = await postBoardOfferV1(
      BOARD,
      VECTOR.ticketText,
      stubFetch(201, { accepted: true, digest: 'd'.repeat(64), duplicate: false }, calls),
    );
    expect(calls.method).toBe('POST');
    expect(calls.url).toBe('https://board.example/tickets');
    // Verbatim: a re-serialization here would be a second writer of a canonical
    // shape, and the digest nobody else computes.
    expect(calls.body).toBe(VECTOR.ticketText);
    expect(posted).toEqual({ digest: 'd'.repeat(64), duplicate: false });
  });

  it('refuses a malformed ticket in this process before any request is made', async () => {
    let reached = false;
    await expect(
      postBoardOfferV1(BOARD, '{"kind":"wrong"}', () => {
        reached = true;
        return Promise.reject(new Error('should never be called'));
      }),
    ).rejects.toThrow();
    expect(reached, 'a malformed ticket must not travel to a relay first').toBe(false);
  });

  it('refuses a board that accepts without naming a digest', async () => {
    await expect(
      postBoardOfferV1(BOARD, VECTOR.ticketText, stubFetch(201, { accepted: true })),
    ).rejects.toThrow('without naming its digest');
  });

  it('reports a duplicate as an acceptance, because re-posting is not an error', async () => {
    const posted = await postBoardOfferV1(
      BOARD,
      VECTOR.ticketText,
      stubFetch(201, { accepted: true, digest: 'e'.repeat(64), duplicate: true }),
    );
    expect(posted.duplicate).toBe(true);
  });
});

describe('boardHealthV1', () => {
  it('reads the holdings and a board that names no clock', async () => {
    const health = await boardHealthV1(
      BOARD,
      stubFetch(200, { status: 'ok', offers: 7, observedSlot: null }),
    );
    expect(health).toEqual({ offers: 7, observedSlot: null });
  });
});

describe('the board never becomes an authority', () => {
  it('exposes no way to ask whether an offer is verified', async () => {
    const listing = await listBoardOffersV1(
      BOARD,
      { market: VECTOR.market },
      stubFetch(200, {
        offers: [offer(VECTOR.ticketText)],
        slotBasis: null,
        droppedExpired: 0,
        // A board asserting authority is ignored rather than believed.
        verified: true,
        trusted: true,
      }),
    );
    const keys = Object.keys(listing.offers[0]);
    expect(keys).toEqual(['digest', 'text', 'ticket', 'postedAtSlot']);
    for (const forbidden of ['verified', 'trusted', 'valid']) {
      expect(keys).not.toContain(forbidden);
    }
    expect(Object.keys(listing)).not.toContain('verified');
  });
});
