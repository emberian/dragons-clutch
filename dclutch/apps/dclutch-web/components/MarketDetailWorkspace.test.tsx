import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketDetailWorkspace from './MarketDetailWorkspace';

const ADDRESS = '7CuJSi6uEyTFD7TUmyiUyszv51b5v1K4tXGXhvC5Y8DU';

describe('Market detail route', () => {
  const html = renderToStaticMarkup(<MarketDetailWorkspace address={ADDRESS} />);

  it('leads with the four facts a reader needs to decide what to do next', () => {
    expect(html).toContain('aria-label="Market decision facts"');
    expect(html).toContain('Status');
    expect(html).toContain('Collateral held');
    expect(html).toContain('Leading outcome');
    expect(html).toContain('Settles');
    expect(html.split('>—</strong>').length - 1).toBe(4);
    expect(html).toContain('Connection &amp; provenance');
    expect(html).toContain('In the protocol&#x27;s own words · identity and immutable content');
    expect(html).toContain('Where claims sit');
    expect(html).toContain(ADDRESS);
    // The read starts on its own: the address is in the URL and the programs
    // come from the baked deployment, so nothing is asked for first.
    expect(html).toContain('>Reading…</button>');
    // The deployment is a terse fact; connection and provenance remain one
    // disclosure away instead of competing with the market decision.
    expect(html).toContain('<p>Devnet</p>');
    expect(html).not.toContain('Finalized RPC endpoint');
    expect(html).not.toContain('Registry program · optional');
    expect(html).not.toContain('<input');
    expect(html).not.toContain('Join this market');
  });

  it('carries a provenance chip and an explicit refusal on every section before any read', () => {
    // Four sections, and nothing has been read: each one says REFUSED and why,
    // rather than rendering as empty-but-fine.
    expect(html.split('REFUSED').length - 1).toBeGreaterThanOrEqual(3);
    // Renegotiated 2026-08-31: each unread section used to explain, in its own
    // sentence, WHY it had nothing (no fingerprint to work an address out
    // from, no list to check). The chip already says REFUSED with its reason;
    // the body says the short version once.
    expect(html.split('>Not read yet.<').length - 1).toBeGreaterThanOrEqual(3);
    expect(html).toContain('Reading this market…');
    // And it does not yet say a Market can never trade, because it has not read
    // one. That verdict is only ever spoken from an authenticated manifest.
    expect(html).not.toContain('can never trade');
  });

  /**
   * Renegotiated 2026-08-31. Three section blurbs used to state the contracts
   * this surface is held to: that the money section decodes nothing from the
   * Market root, that an underivable Hoard is said rather than guessed, that
   * capability funding is quoted in seven segregated compartments with SOL and
   * collateral never merged. Every one of those is a promise about US, and all
   * three are deleted. The contracts themselves are still enforced -- by the
   * refusal paths and the funding table -- so what is pinned here is the
   * BEHAVIOUR each blurb used to describe, checked where it actually lives.
   */
  it('keeps the contracts the section blurbs used to narrate', () => {
    // Nothing is read here, so the money section refuses rather than showing
    // a zero -- which is the contract the deleted blurb described.
    expect(html).toContain('REFUSED');
    expect(html).not.toContain('>0</strong>');
    // And no blurb anywhere argues for any of it.
    for (const sermon of ['rather than guess', 'never merged into one number', 'stores a fingerprint of one']) {
      expect(html).not.toContain(sermon);
    }
  });

  it('presents raw atoms and never a market-data metric', () => {
    // The product nav links to the pre-existing Dealer surface at /liquidity;
    // that route name is not this surface's vocabulary and is excluded here.
    // Renegotiated 2026-08-31: the one exempted disclaimer is deleted, so the
    // forbidden scan now runs over the whole document rather than over the
    // document minus a sentence written to be exempt from it.
    const remainder = html.replace(/<nav>[\s\S]*?<\/nav>/, '');
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'Probability', 'TVL', 'liquidity', 'Liquidity', '24h', 'APR', 'APY', 'yield', 'Total value locked', '$', 'price', 'Price']) {
      expect(remainder).not.toContain(forbidden);
    }
  });

  it('never exposes a signing or submission control on a read-only detail surface', () => {
    expect(html).not.toContain('Sign');
    expect(html).not.toContain('Submit');
  });

  /**
   * THE READER'S ORDER, pinned by position rather than by presence.
   *
   * Every section this page renders was present before and is present now; what
   * changed is which of them a reader meets first. So an assertion that merely
   * finds each string would have passed on the old page and says nothing. These
   * compare INDEXES: the answer and what it means come before the claims and
   * the history, those come before the trade, and the operator region comes
   * last -- after everything a reader needs to know what happened here.
   */
  it('orders the page for a reader and puts the operator sections last', () => {
    const at = (needle: string) => {
      const index = html.indexOf(needle);
      expect(index, `${needle} is not on the page`).toBeGreaterThan(-1);
      return index;
    };
    expect(at('aria-label="Market decision facts"')).toBeLessThan(at('Where claims sit'));
    expect(at('Where claims sit')).toBeLessThan(at('For operators and auditors'));
    expect(at('For operators and auditors')).toBeLessThan(at("In the protocol&#x27;s own words"));
    expect(at("In the protocol&#x27;s own words")).toBeLessThan(at('Advanced: full route workbench'));
    // The region names itself to a screen reader by its own heading, and every
    // section inside it is a real disclosure rather than a hidden div.
    expect(html).toContain('aria-labelledby="operator-fold-heading"');
    expect(html).toContain('id="operator-fold-heading"');
  });

  /**
   * The fold COUNTS itself. Before the read there is exactly one section in it
   * -- identity -- because a Realm that has not bound, a capability manifest
   * that has not authenticated and a retirement checkpoint on an undecoded
   * market are not sections, and a region that says "four" while rendering one
   * is the kind of claim this page exists not to make.
   */
  it('says how many sections the operator region actually holds', () => {
    expect(html).toContain('1 section this page already read');
    expect(html.split('<details').length - 1).toBeGreaterThanOrEqual(2);
  });
});

/**
 * The editorial layer on detail: a registered market leads with its name,
 * question, and story — and in the same breath says whose words those are.
 * The words render before any read succeeds, because they are not chain data
 * and must never pretend to be gated on it.
 */
describe('a market the shipped registry names', () => {
  const FLAGSHIP = '7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC';
  const html = renderToStaticMarkup(<MarketDetailWorkspace address={FLAGSHIP} />);

  it('leads with the registered name and question', () => {
    expect(html).toContain('SOL/USD range — first public market');
    expect(html).toContain('Where does the SOL/USD price finish this market&#x27;s window');
    // The address does not disappear behind the name.
    expect(html).toContain(FLAGSHIP);
  });

  it('tells the permanent disposition as history, not breakage', () => {
    expect(html).toContain('never switched on');
    expect(html).toContain('stay on devnet');
    expect(html).not.toContain('broken');
  });

  it('does not mistake editorial settlement design for a live deadline before the market is read', () => {
    expect(html).toContain('Settles');
    expect(html).toContain('Not read yet.');
    expect(html).not.toContain('Settles from Pyth');
    expect(html).not.toContain('settlement deadline');
  });

  /**
   * Renegotiated 2026-08-31. This pinned a note under the hero saying the
   * name, question and story are the site's editorial and the chain stores no
   * names. Deleted: a title does not need a footnote about who typed it. What
   * still matters is the sibling test below -- an address the registry does
   * not know gets NO editorial words at all rather than an invented name.
   */
  it('never carries an editorial provenance footnote', () => {
    expect(html).not.toContain('the chain stores no names');
    expect(html).not.toContain('editorial');
  });

  it('renders no editorial words for an address the registry does not know', () => {
    const bare = renderToStaticMarkup(<MarketDetailWorkspace address={ADDRESS} />);
    expect(bare).not.toContain('market-question');
    expect(bare).not.toContain('market-story');
    expect(bare).not.toContain('the chain stores no names');
  });
});
