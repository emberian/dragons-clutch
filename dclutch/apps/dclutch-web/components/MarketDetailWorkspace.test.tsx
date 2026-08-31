import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketDetailWorkspace from './MarketDetailWorkspace';

const ADDRESS = '7CuJSi6uEyTFD7TUmyiUyszv51b5v1K4tXGXhvC5Y8DU';

describe('Market detail route', () => {
  const html = renderToStaticMarkup(<MarketDetailWorkspace address={ADDRESS} />);

  it('names the four sections a Market detail owes its reader', () => {
    expect(html).toContain('What this market is');
    expect(html).toContain('The money');
    expect(html).toContain('What it pays out in');
    expect(html).toContain('What it is allowed to do');
    expect(html).toContain(ADDRESS);
    // The read starts on its own: the address is in the URL and the programs
    // come from the baked deployment, so nothing is asked for first.
    expect(html).toContain('>Reading…</button>');
    expect(html).toContain('from the active Devnet deployment');
    expect(html).not.toContain('Finalized RPC endpoint');
    expect(html).not.toContain('Registry program · optional');
    expect(html).not.toContain('<input');
  });

  it('carries a provenance chip and an explicit refusal on every section before any read', () => {
    // Four sections, and nothing has been read: each one says REFUSED and why,
    // rather than rendering as empty-but-fine.
    expect(html.split('REFUSED').length - 1).toBeGreaterThanOrEqual(3);
    expect(html).toContain('The market account has not been read yet');
    expect(html).toContain('so there is no fingerprint to work an address out from');
    expect(html).toContain('so there is no list to check');
    expect(html).toContain('Reading this market from the chain…');
    // And it does not yet say a Market can never trade, because it has not read
    // one. That verdict is only ever spoken from an authenticated manifest.
    expect(html).not.toContain('can never trade');
  });

  it('states the Hoard and capability funding contracts it is held to', () => {
    // The Hoard has no derivable account, and the surface says so before it is
    // ever asked to show a number.
    expect(html).toContain('where the vault cannot be worked out we say so rather than guess');
    expect(html).toContain('The market account itself stores neither the claim counts nor the vault balance');
    expect(html).toContain('seven separate compartments, with SOL and collateral totalled apart and never merged into one number');
    expect(html).toContain('it stores a fingerprint of one');
  });

  it('presents raw atoms and never a market-data metric', () => {
    // The product nav links to the pre-existing Dealer surface at /liquidity;
    // that route name is not this surface's vocabulary and is excluded here.
    const withoutNav = html.replace(/<nav>[\s\S]*?<\/nav>/, '');
    const disclaimers = [
      'Raw amounts, read where the chain keeps them. The market account itself stores neither the claim counts nor the vault balance, so nothing in this section comes from it: the counts come from the claims ledger this market points at, and where the vault cannot be worked out we say so rather than guess.',
    ];
    let remainder = withoutNav;
    for (const disclaimer of disclaimers) {
      expect(remainder).toContain(disclaimer);
      remainder = remainder.split(disclaimer).join('');
    }
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'Probability', 'TVL', 'liquidity', 'Liquidity', '24h', 'APR', 'APY', 'yield', 'Total value locked', '$', 'price', 'Price']) {
      expect(remainder).not.toContain(forbidden);
    }
  });

  it('never exposes a signing or submission control on a read-only detail surface', () => {
    expect(html).not.toContain('Sign');
    expect(html).not.toContain('Submit');
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
    expect(html).toContain('SOL/USD range — the first public market');
    expect(html).toContain('Where does the SOL/USD price finish this market&#x27;s window');
    // The address does not disappear behind the name.
    expect(html).toContain(FLAGSHIP);
  });

  it('tells the permanent disposition as history, not breakage', () => {
    expect(html).toContain('never switched on');
    expect(html).toContain('readable forever');
    expect(html).not.toContain('broken');
  });

  it('says in words how the question settles', () => {
    expect(html).toContain('settles from Pyth');
    expect(html).toContain('silence is an outcome here, not a stall');
  });

  it('says whose words the name and story are, right where they render', () => {
    expect(html).toContain('the chain stores no names');
  });

  it('renders no editorial words for an address the registry does not know', () => {
    const bare = renderToStaticMarkup(<MarketDetailWorkspace address={ADDRESS} />);
    expect(bare).not.toContain('market-question');
    expect(bare).not.toContain('market-story');
    expect(bare).not.toContain('the chain stores no names');
  });
});
