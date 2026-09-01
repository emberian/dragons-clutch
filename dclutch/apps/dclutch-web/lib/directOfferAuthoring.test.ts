import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { inspectDirectMakerNonceV1 } from './directMakerReplay';
import { composeDirectSellOfferV1, sealDirectSellOfferV1 } from './directOfferAuthoring';
import { type ReadyDirectSellerV1 } from './directOfferAuthoring';

const TRADING = new PublicKey(Uint8Array.from({ length: 32 }, (_, index) => index + 1)).toBase58();
const MARKET = new PublicKey(Uint8Array.from({ length: 32 }, (_, index) => index + 33)).toBase58();
const MAKER = new PublicKey(Uint8Array.from({ length: 32 }, (_, index) => 255 - index)).toBase58();
const COLLATERAL = new PublicKey(Uint8Array.from({ length: 32 }, (_, index) => index + 65)).toBase58();

async function replay(slot = '50') {
  return inspectDirectMakerNonceV1({
    finalizedSlot: async () => slot,
    accountInfo: async () => Object.freeze({ slot, account: null }),
  }, { tradingProgram: TRADING, market: MARKET, generation: 7n, maker: MAKER });
}

function seller(overrides: Partial<ReadyDirectSellerV1> = {}): ReadyDirectSellerV1 {
  return Object.freeze({
    status: 'ready', observedSlot: '49', market: MARKET, generation: 7n, owner: MAKER,
    coordinates: Object.freeze({ aggregate: COLLATERAL, position: TRADING, collateral: COLLATERAL, custodyAuthority: MARKET }),
    collateralMint: MARKET, tokenProgram: TRADING, positionRevision: 3n,
    positionBalances: Object.freeze([500n, 900n]), collateralPrestate: 'vacant', reason: 'ready',
    ...overrides,
  });
}

const route = Object.freeze({
  market: MARKET, generation: 7n, outcomeCount: 2, priceScale: 1_000_000n,
  feeBasisPoints: 25, tradingProgram: TRADING,
});

describe('Direct sell offer authoring', () => {
  it('binds seller balance, canonical replay nonce, validity, fee, and destination into the signed bytes', async () => {
    const draft = composeDirectSellOfferV1({
      route, maker: MAKER, seller: seller(), replay: await replay(), outcome: 1,
      maximumFill: 400n, limitPrice: 350_000n, lifecycle: 1, durationSlots: 150n,
    });
    expect(draft.signingMessage).toHaveLength(172);
    expect(draft.intent).toMatchObject({
      side: 0, lifecycle: 1, outcome: 1, market: MARKET, generation: 7n,
      nonce: 0n, validFrom: 50n, validThrough: 200n, maximumFill: 400n,
      limitPrice: 350_000n, feeBasisPoints: 25, collateralAccount: COLLATERAL,
    });
    expect(draft.availableClaims).toBe(900n);

    const authored = sealDirectSellOfferV1(MAKER, draft, new Uint8Array(64).fill(7));
    expect(authored.ticket.maker).toBe(MAKER);
    expect(authored.ticket.intent).toEqual(draft.intent);
    expect(authored.text).toContain('dclutch/direct-intent-ticket/v1');
    expect(authored.signingMessage).toEqual(draft.signingMessage);
  });

  it('refuses overselling and observations for another maker or generation', async () => {
    const observed = await replay();
    const base = { route, maker: MAKER, seller: seller(), replay: observed, outcome: 0,
      maximumFill: 501n, limitPrice: 500_000n, lifecycle: 0 as const, durationSlots: 1n };
    expect(() => composeDirectSellOfferV1(base)).toThrow(/exceeds the claims/);
    expect(() => composeDirectSellOfferV1({ ...base, maximumFill: 1n, seller: seller({ generation: 8n }) }))
      .toThrow(/another maker, Market, or generation/);
  });

  it('refuses zero duration, overflowing validity, and noncanonical signatures', async () => {
    const observed = await replay('18446744073709551615');
    const base = { route, maker: MAKER, seller: seller({ observedSlot: '18446744073709551615' }), replay: observed,
      outcome: 0, maximumFill: 1n, limitPrice: 1n, lifecycle: 0 as const, durationSlots: 1n };
    expect(() => composeDirectSellOfferV1(base)).toThrow(/valid-through slot outside u64/);
    const draft = composeDirectSellOfferV1({ ...base, seller: seller(), replay: await replay(), durationSlots: 1n });
    expect(() => sealDirectSellOfferV1(MAKER, draft, new Uint8Array(64))).toThrow(/nonzero 64-byte/);
  });
});
