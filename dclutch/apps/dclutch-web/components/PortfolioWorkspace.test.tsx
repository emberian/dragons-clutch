import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import PortfolioWorkspace from './PortfolioWorkspace';

/**
 * The product inversion this surface carries: the ONE input is an owner
 * identity. The endpoint, the programs, and the Market list all come from the
 * baked deployment; connecting a wallet auto-loads. These tests pin that no
 * infrastructure form creeps back in.
 */
describe('Portfolio route', () => {
  const html = renderToStaticMarkup(<PortfolioWorkspace />);

  /**
   * Renegotiated 2026-08-31. The hero used to open on our architecture --
   * dClutch runs no indexer, this browser will not pretend to be one, your
   * claims live at an address worked out rather than looked up. A reader
   * asking "what do I hold?" needs none of it, and the no-index property is
   * still enforced by the code path, not by the sentence. What is pinned now
   * is that the page opens on the reader's question and never CLAIMS an index.
   */
  it('opens on what the reader holds, and never claims an index', () => {
    expect(html).toContain('to see what claims it holds in every market on this deployment');
    expect(html).toContain('Market by market');
    expect(html).not.toContain('indexer');
  });

  it('asks only for an owner identity — every other input comes from the deployment', () => {
    expect(html).toContain('Whose wallet?');
    expect(html).toContain('Or paste any owner address');
    expect(html).not.toContain('Finalized RPC endpoint');
    expect(html).not.toContain('Core program</span>');
    expect(html).not.toContain('Claims program</span>');
    expect(html).not.toContain('Known Market addresses');
    expect(html).not.toContain('<textarea');
  });

  it('makes the browser wallet identity-only and the paste path equal', () => {
    // Renegotiated 2026-08-31: "no signature, no approval" and "reading a
    // derived address requires no authority at all" said the same thing twice
    // and the second half was about our derivation, not about the reader.
    expect(html).toContain('Connecting reads your address. Nothing is signed.');
    // The server render asserts nothing about installed extensions.
    expect(html).toContain('No browser wallet found.');
  });

  it('keeps the honest empty state instead of showing placeholder holdings', () => {
    // Renegotiated 2026-08-31: each empty state used to explain that it was
    // staying empty ON PURPOSE rather than showing placeholders. An empty
    // section that says "Nothing read yet." has already made that point.
    expect(html.split('>Nothing read yet.<').length - 1).toBeGreaterThanOrEqual(2);
    expect(html).not.toContain('rather than showing');
    expect(html).not.toContain('>0</strong>');
  });

  it('carries the across-Markets bound, and states the sum as the true answer rather than a caution', () => {
    // Renegotiated 2026-08-31: the blurb used to argue for its own
    // arithmetic -- that two unrelated markets exclude nothing so the sum is
    // exact, "the true number, not a cautious one". The panel below states the
    // bound; the defence of it is deleted.
    expect(html).toContain('Across everything you hold');
    expect(html).toContain('The most and the least all of it can pay, added up.');
    expect(html).not.toContain('not a cautious one');
  });

  it('presents raw atoms and never a market-data metric', () => {
    // The product nav links to the pre-existing Dealer surface at /liquidity;
    // that route name is not this surface's vocabulary and is excluded here.
    const remainder = html.replace(/<nav>[\s\S]*?<\/nav>/, '');
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'Probability', 'TVL', 'liquidity', 'Liquidity', '24h', 'APR', 'APY', 'yield', 'Total value locked', '$', 'price', 'Price', 'portfolio value', 'P&L']) {
      expect(remainder).not.toContain(forbidden);
    }
  });

  it('never exposes a signing or submission control on a read-only portfolio', () => {
    const withoutWalletContract = html.split('Requesting a signature is always a separate explicit action').join('');
    expect(withoutWalletContract).not.toContain('Sign');
    expect(withoutWalletContract).not.toContain('Submit');
  });
});

describe('Redemption route', () => {
  const html = renderToStaticMarkup(<PortfolioWorkspace mode="redemption" />);

  it('starts with the connected wallet and the live Market set instead of the representation console', () => {
    // The headline used to offer a payout this page cannot make; then -- once
    // redemption shipped here -- to deny one it can; then to state a COUNT of
    // resolved markets, which went false the day one resolved. All three are
    // refused by name. What stands is a statement about the page, and the
    // per-market fact is read live below where it cannot go stale.
    expect(html).toContain('Your winning claims');
    expect(html).not.toContain('Payout is not open yet');
    expect(html).not.toContain('Nothing has resolved yet');
    expect(html).not.toContain('no market on this deployment has reached an answer');
    expect(html).toContain('Cashed in here');
    expect(html).toContain('no file and no operator');
    expect(html).not.toContain('Redeem your winning claims');
    // Renegotiated 2026-08-31: the wallet panel no longer takes a `purpose`
    // string describing why this page wants an address. The heading says it.
    expect(html).toContain('Connect your wallet');
    expect(html).not.toContain('Or paste any owner address');
    expect(html).not.toContain('Authenticate exact transfer route');
  });

  it('states every boundary that remains before a payout can reach a wallet', () => {
    // Renegotiated 2026-08-31. This is a SIGNING surface, so the boundaries
    // that change what a reader should do stay: which chains are refused
    // outright, and that nothing is signed until everything checks out. What
    // is deleted is the self-description around them -- that the plan is
    // "Rust-authored", produced outside this browser, and not invented from
    // partial state. Those are facts about our build, not about their keys.
    // Renegotiated 2026-08-31 again: the surviving half was still a paragraph
    // about what this page does before it signs. The load-bearing fact for
    // somebody holding keys is which chains it will touch at all.
    expect(html).toContain('Devnet only — mainnet and testnet are refused.');
    expect(html).not.toContain('Rust-authored');
  });
});
