import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketDetailWorkspace from './MarketDetailWorkspace';

const ADDRESS = '7CuJSi6uEyTFD7TUmyiUyszv51b5v1K4tXGXhvC5Y8DU';

describe('Market detail route', () => {
  const html = renderToStaticMarkup(<MarketDetailWorkspace address={ADDRESS} />);

  it('names the four sections a Market detail owes its reader', () => {
    expect(html).toContain('Overview');
    expect(html).toContain('Economics');
    expect(html).toContain('Realm');
    expect(html).toContain('Capabilities');
    expect(html).toContain(ADDRESS);
    expect(html).toContain('Read this Market');
    expect(html).toContain('Registry program · optional');
    expect(html).toContain('Claims program · optional');
  });

  it('carries a provenance chip and an explicit refusal on every section before any read', () => {
    // Four sections, and nothing has been read: each one says REFUSED and why,
    // rather than rendering as empty-but-fine.
    expect(html.split('REFUSED').length - 1).toBeGreaterThanOrEqual(3);
    expect(html).toContain('No decoded Market root');
    expect(html).toContain('No Realm was reacquired, because no Market root has been decoded.');
    expect(html).toContain('No capability manifest identity exists to authenticate');
    expect(html).toContain('No finalized state has been read for this Market address.');
  });

  it('states the Hoard and capability funding contracts it is held to', () => {
    // The Hoard has no derivable account, and the surface says so before it is
    // ever asked to show a number.
    expect(html).toContain('the Hoard is stated as underivable rather than guessed');
    expect(html).toContain('A Core Market root carries no supply vector and no Hoard figure');
    expect(html).toContain('seven segregated compartments with separate native-lamport and Realm-collateral totals, never merged into one number');
    expect(html).toContain('content identity');
  });

  it('presents raw atoms and never a market-data metric', () => {
    // The product nav links to the pre-existing Dealer surface at /liquidity;
    // that route name is not this surface's vocabulary and is excluded here.
    const withoutNav = html.replace(/<nav>[\s\S]*?<\/nav>/, '');
    const disclaimers = [
      'Raw u64 atoms, read where the chain keeps them. A Core Market root carries no supply vector and no Hoard figure, so this section is not decoded from the Market at all: the per-claim supplies come from the Claims LiabilityBasisV2 aggregate this Market derives, and the Hoard is stated as underivable rather than guessed.',
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
