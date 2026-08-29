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

  it('derives Position addresses instead of claiming an index it does not have', () => {
    expect(html).toContain('dClutch runs no indexer and this browser will not pretend to be one');
    expect(html).toContain('program-derived address of the Position seed domain plus the exact Market and owner keys');
    expect(html).toContain('Derived Positions');
  });

  it('asks only for an owner identity — every other input comes from the deployment', () => {
    expect(html).toContain('Whose Positions?');
    expect(html).toContain('the active Devnet deployment');
    expect(html).toContain('Or paste any owner address');
    expect(html).not.toContain('Finalized RPC endpoint');
    expect(html).not.toContain('Core program</span>');
    expect(html).not.toContain('Claims program</span>');
    expect(html).not.toContain('Known Market addresses');
    expect(html).not.toContain('<textarea');
  });

  it('makes the browser wallet identity-only and the paste path equal', () => {
    expect(html).toContain('no signature, no approval');
    expect(html).toContain('reading a derived address requires no authority at all');
    // The server render asserts nothing about installed extensions.
    expect(html).toContain('No Wallet Standard registry exists in this runtime');
  });

  it('keeps the honest empty state instead of showing placeholder holdings', () => {
    expect(html).toContain('No finalized Position state has been read yet.');
    expect(html).toContain('this surface stays empty rather than showing placeholder holdings');
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

describe('Redemption route', () => {
  const html = renderToStaticMarkup(<PortfolioWorkspace mode="redemption" />);

  it('starts with the connected wallet and the live Market set instead of the representation console', () => {
    // The headline used to offer a payout this page cannot make.
    expect(html).toContain('Your winning claims');
    expect(html).toContain('Payout is not open yet');
    expect(html).not.toContain('Redeem your winning claims');
    expect(html).toContain('find the winning claims you hold');
    expect(html).not.toContain('Or paste any owner address');
    expect(html).not.toContain('Authenticate exact transfer route');
  });

  it('states every boundary that remains before a payout can reach a wallet', () => {
    expect(html).toContain('permanently refuses Solana mainnet, testnet, and unknown non-local chains');
    expect(html).toContain('The payout plan is still produced outside this browser');
    expect(html).toContain('does not invent one from partial state');
    expect(html).toContain('Rust-authored payout plan');
    expect(html).toContain('exact Market, Position, owner, winning claim, recipient, programs, and lookup table');
  });
});
