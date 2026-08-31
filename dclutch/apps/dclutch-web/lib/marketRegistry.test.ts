import { describe, expect, it } from 'vitest';

import {
  fallbackMarketTitleV1,
  MARKET_EDITORIAL_NOTE_V1,
  MARKET_REGISTRY_V1,
  marketEditorialV1,
  parseMarketRegistryV1,
} from './marketRegistry';

/**
 * The registry is the one place the site is allowed to put words in a
 * market's mouth, so these tests hold it to its own charter: names and
 * stories only, declared as editorial, keyed by canonical addresses — and a
 * market the file does not know gets a generated label that pretends nothing.
 */
describe('the shipped devnet market registry', () => {
  it('parses, is devnet-scoped, and declares its own provenance', () => {
    expect(MARKET_REGISTRY_V1.schema).toBe('dclutch-market-registry-v1');
    expect(MARKET_REGISTRY_V1.cluster).toBe('devnet');
    expect(MARKET_REGISTRY_V1.provenance).toContain('site-side editorial');
    expect(MARKET_REGISTRY_V1.provenance).toContain('read from the chain');
  });

  it('names the two markets that finished founding on public devnet', () => {
    const flagship = MARKET_REGISTRY_V1.markets['7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC'];
    expect(flagship).toBeDefined();
    expect(flagship.title).toBe('SOL/USD range — first public market');
    expect(flagship.question).toContain('SOL/USD');
    expect(flagship.outcomes).toHaveLength(4);
    // How it settles, in words: the design, not an operational promise.
    expect(flagship.resolution).toContain('Pyth');
    expect(flagship.resolution).toContain('source-failure outcome');
    // Renegotiated 2026-08-31. Every story in this file was a paragraph of
    // our own build history -- which driver refused its own success, which
    // session switched trading on, what the cohort widened. They are now one
    // to three sentences about the MARKET. What is still pinned is the fact a
    // reader acts on: what never happened, and what is still there.
    expect(flagship.story).toContain('never switched on');
    expect(flagship.story).toContain('stay on devnet');
    for (const entry of Object.values(MARKET_REGISTRY_V1.markets)) {
      expect(entry.story ?? '').not.toMatch(/session|cohort|driver|lane|build-out|tooling/i);
      expect((entry.story ?? '').length).toBeLessThan(220);
    }

    const orphan = MARKET_REGISTRY_V1.markets['CasyDFowGxqREDW5iWvKRgSMCgk5HnLQjnjegvRsSNPM'];
    expect(orphan).toBeDefined();
    expect(orphan.story).toContain('Trading was never switched on');
  });

  /**
   * The market the front door headlines. Its entry is the one place a reader
   * meets a tradeable market in this site's own words, so it has to promise
   * exactly what happened and nothing that has not: a trade is possible, and
   * no trade has been made.
   */
  it('names the market the public cut headlines, and does not pretend it has traded', () => {
    const open = MARKET_REGISTRY_V1.markets['6WZXJ7jBPPA3eFZPc8hQmmNsf3R4zAZN4DRZzfhcV7a4'];
    expect(open).toBeDefined();
    expect(open.title).toContain('open for trading');
    expect(open.outcomes).toHaveLength(4);
    expect(open.resolution).toContain('Pyth');
    // Renegotiated 2026-08-31: "No trade has been made on it yet" is a chain
    // fact the card already prints, and one that rots. The story now carries
    // the two things the chain does NOT say: the fee and what the collateral
    // is worth.
    expect(open.story).toContain('No fee');
    expect(open.story).toContain('devnet test token');
  });

  it('keeps every entry inside the editorial charter: no numbers-in-words drift', () => {
    for (const [address, entry] of Object.entries(MARKET_REGISTRY_V1.markets)) {
      // Titles and questions are prose, not data: they may never carry a raw
      // atom count or a slot number that would rot against the chain.
      for (const text of [entry.title, entry.question, ...(entry.outcomes ?? []), entry.story ?? '']) {
        expect(text, `${address} editorial text carries a slot-sized number`).not.toMatch(/\d{9,}/);
      }
    }
  });

  it('looks up by address, and a market the file does not know reads as null', () => {
    expect(marketEditorialV1('7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC')?.title).toContain('first public market');
    expect(marketEditorialV1('CasyDFowGxqREDW5iWvKRgSMCgk5HnLQjnjegvRsSNPM')?.title).toContain('never activated');
    expect(marketEditorialV1('pSVpRyDGYVp9Lv5TeyuTH5eMp7bh9myr7JPdr71GETB')).toBeNull();
  });

  it('generates phase-aware fallback labels that invent nothing', () => {
    expect(fallbackMarketTitleV1('Founding', 'pSVpRyDGYVp9Lv5TeyuTH5eMp7bh9myr7JPdr71GETB')).toBe('Unfinished · pSVp…GETB');
    expect(fallbackMarketTitleV1('Open', 'pSVpRyDGYVp9Lv5TeyuTH5eMp7bh9myr7JPdr71GETB')).toBe('Unnamed · pSVp…GETB');
    expect(fallbackMarketTitleV1(null, 'pSVpRyDGYVp9Lv5TeyuTH5eMp7bh9myr7JPdr71GETB')).toBe('Unnamed · pSVp…GETB');
  });

  it('states whose words the editorial fields are, for the surfaces to render', () => {
    expect(MARKET_EDITORIAL_NOTE_V1).toContain('editorial');
    expect(MARKET_EDITORIAL_NOTE_V1).toContain('the chain stores no names');
  });

  it('refuses malformed registries instead of guessing', () => {
    expect(() => parseMarketRegistryV1(null)).toThrow('must be one object');
    expect(() => parseMarketRegistryV1({ schema: 'dclutch-market-registry-v1', cluster: 'devnet', provenance: 'x', markets: {}, extra: 1 })).toThrow('missing or unknown fields');
    expect(() => parseMarketRegistryV1({ schema: 'other', cluster: 'devnet', provenance: 'x', markets: {} })).toThrow('another schema or cluster');
    expect(() => parseMarketRegistryV1({ schema: 'dclutch-market-registry-v1', cluster: 'devnet', provenance: 'x', markets: { 'not-an-address': { title: 't', question: 'q' } } })).toThrow('canonical Solana address');
    expect(() => parseMarketRegistryV1({
      schema: 'dclutch-market-registry-v1', cluster: 'devnet', provenance: 'x',
      markets: { '7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC': { title: 't', question: 'q', unknown: true } },
    })).toThrow('unknown field');
    expect(() => parseMarketRegistryV1({
      schema: 'dclutch-market-registry-v1', cluster: 'devnet', provenance: 'x',
      markets: { '7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC': { title: '  padded ', question: 'q' } },
    })).toThrow('trimmed');
    expect(() => parseMarketRegistryV1({
      schema: 'dclutch-market-registry-v1', cluster: 'devnet', provenance: 'x',
      markets: { '7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC': { title: 't', question: 'q', outcomes: [] } },
    })).toThrow('non-empty array');
  });
});
