import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import PortfolioWorkspace from './PortfolioWorkspace';

describe('Portfolio route', () => {
  const html = renderToStaticMarkup(<PortfolioWorkspace />);

  it('derives Position addresses instead of claiming an index it does not have', () => {
    expect(html).toContain('dClutch runs no indexer and this browser will not pretend to be one');
    expect(html).toContain('program-derived address of the Position seed domain plus the exact Market and owner keys');
    expect(html).toContain('Derive and read Positions');
    expect(html).toContain('Derived Positions');
  });

  it('states the discovery gap a derivation cannot close', () => {
    expect(html).toContain('Positions are derived, but Markets are not');
    expect(html).toContain('needs an index dClutch does not publish');
    expect(html).toContain('Enumerate Markets from the Core program');
    expect(html).toContain('Add this Market');
    expect(html).toContain('Known Market addresses');
    expect(html).toContain('Markets to derive against');
  });

  it('makes the browser wallet optional and identity-only', () => {
    expect(html).toContain('A browser wallet is optional here');
    expect(html).toContain('Owner address · wallet or pasted');
    expect(html).toContain('Connecting reads a public address only');
    // The server render asserts nothing about installed extensions.
    expect(html).toContain('No Wallet Standard registry exists in this runtime');
  });

  it('keeps the honest empty state instead of showing placeholder holdings', () => {
    expect(html).toContain('No finalized Position state has been read.');
    expect(html).toContain('this surface stays empty rather than showing placeholder holdings');
    expect(html).toContain('No Core program enumeration has been attempted.');
  });

  it('presents raw atoms and never a market-data metric', () => {
    // The product nav links to the pre-existing Dealer surface at /liquidity;
    // that route name is not this surface's vocabulary and is excluded here.
    const remainder = html.replace(/<nav>[\s\S]*?<\/nav>/, '');
    expect(remainder).toContain('raw u64');
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
