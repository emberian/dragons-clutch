import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import TicketCard, { ticketGrossAtomsV1 } from './TicketCard';
import { type SignedDirectIntentV3 } from '@dclutch/sdk/directInlineV3';
import { type DenominationV1 } from '@dclutch/sdk/quantity';

const MAKER_V1 = '8bcRzB3v6PxbbtkVCiX9ceW2whwakA6gX7qvSYbeMHLq';
const MARKET_V1 = '5F8wMRFMdYGMkjWQUye6WfbgRVWEo9yyKo9aFPk2TLaD';
const COLLATERAL_V1 = '7xwJ3uceuBV7KyCsdJsBs9Ljfh1bL3WB7NbGpwUNeJ2o';

const SIX_DECIMALS_V1: DenominationV1 = Object.freeze({ decimals: 6, unit: 'USDC', mint: 'mint' });
const PRICE_SCALE_V1 = 1_000_000n;

function ticketV1(intent: Partial<SignedDirectIntentV3['intent']> = {}): SignedDirectIntentV3 {
  return Object.freeze({
    maker: MAKER_V1,
    signature: new Uint8Array(64).fill(0xab),
    intent: Object.freeze({
      side: 0 as const,
      lifecycle: 0 as const,
      outcome: 1,
      market: MARKET_V1,
      generation: 7n,
      nonce: 9n,
      validFrom: 11n,
      validThrough: 4_294_967_295n,
      maximumFill: 500_000_000n,
      limitPrice: 350_000n,
      feeBasisPoints: 0,
      collateralAccount: COLLATERAL_V1,
      ...intent,
    }),
  }) as SignedDirectIntentV3;
}

const render = (ticket: SignedDirectIntentV3) => renderToStaticMarkup(<TicketCard
  ticket={ticket}
  denomination={SIX_DECIMALS_V1}
  priceScale={PRICE_SCALE_V1}
  outcomeLabel={(index) => (index === 1 ? 'Above one eighty' : `claim ${index}`)}
  clock={null}
  nowMs={null}
/>);

describe('the parsed ticket card', () => {
  const html = render(ticketV1());

  it('reads the twelve signed fields as an offer rather than as JSON', () => {
    expect(html).toContain('offers to SELL');
    // The editorial name, at the one place a person decides with it.
    expect(html).toContain('Above one eighty');
    // 500000000 atoms at 6 decimals is 500 claims, grouped.
    expect(html).toContain('500 claims');
    expect(html).toContain('35¢');
    expect(html).toContain('All or nothing');
    expect(html).toContain('Fee 0 bps each side');
  });

  /**
   * The reader is the counterparty, so the maker's SELL is their buy. Stating
   * only the maker's verb is the quiet trap: a reader scanning offers reads
   * the verb nearest their eye as theirs.
   */
  it('states the direction from the reader’s side too', () => {
    expect(html).toContain('you would pay');
    expect(render(ticketV1({ side: 1 as const }))).toContain('you would receive');
    expect(render(ticketV1({ side: 1 as const }))).toContain('offers to BUY');
  });

  it('costs the offer out at its own signed price', () => {
    // 500000000 * 350000 / 1000000 = 175000000 atoms = 175.00 at 6 decimals.
    expect(ticketGrossAtomsV1(500_000_000n, 350_000n, PRICE_SCALE_V1)).toBe(175_000_000n);
    expect(html).toContain('175 USDC');
  });

  /**
   * THE CHIP. The browser checked SHAPE. Only the chain verifies a signature,
   * at the Ed25519 program, when the trade executes -- so the chip must never
   * borrow that authority, and the word is pinned here so it cannot drift back.
   */
  it('says the signature is well-formed and never that it is verified', () => {
    expect(html).toContain('well-formed');
    expect(html).not.toContain('verified');
    expect(html).not.toContain('Verified');
    expect(html).not.toContain('valid signature');
    expect(html).toContain('Only the chain verifies');
  });

  it('keeps every exact field one click away, in a drawer that is really there', () => {
    // A collapsed <details>, fully in the markup: the exact twin is present,
    // not fetched, and a `not.toContain` guard over this card is meaningful.
    expect(html).toContain('<details class="ticket-fields"><summary>The exact signed fields</summary>');
    expect(html).not.toContain('<details class="ticket-fields" open');
    expect(html).toContain(MARKET_V1);
    expect(html).toContain(COLLATERAL_V1);
    expect(html).toContain('ab'.repeat(64));
    expect(html).toContain('500000000 claim atoms');
    expect(html).toContain('350000 / 1000000');
    expect(html).toContain('slot 4294967295');
  });

  it('renders a deadline as the slot it is when no clock was measured', () => {
    expect(html).toContain('Valid through slot 4294967295');
  });

  it('says partial fills are allowed when the maker signed for them', () => {
    expect(render(ticketV1({ lifecycle: 1 as const }))).toContain('Partial fills allowed');
  });

  it('never invents a market-data metric on a trading surface', () => {
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'TVL', 'APR', 'APY', '$', '24h', 'P&L']) {
      expect(html).not.toContain(forbidden);
    }
  });
});
