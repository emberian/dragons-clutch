import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketDiscoveryWorkspace from './MarketDiscoveryWorkspace';

describe('Market discovery route', () => {
  const html = renderToStaticMarkup(<MarketDiscoveryWorkspace />);

  it('offers known-address and program-scan discovery against a local finalized endpoint', () => {
    expect(html).toContain('Market discovery · finalized reads only');
    expect(html).toContain('http://127.0.0.1:8899');
    expect(html).toContain('Known Market addresses');
    expect(html).toContain('Enumerate Markets from the Core program');
    expect(html).toContain('Read finalized Market discovery');
    expect(html).toContain('Registry program · optional');
    expect(html).toContain('dClutch publishes no index');
  });

  it('states the provenance and refusal contract every card is held to', () => {
    expect(html).toContain('CHAIN · finalized slot');
    expect(html).toContain('REFUSED');
    expect(html).toContain('every undecoded surface names its reason');
    expect(html).toContain('no capability is asserted from the root alone');
    expect(html).toContain('manifest-only');
  });

  it('keeps the honest empty state instead of showing placeholder Markets', () => {
    expect(html).toContain('No finalized Market discovery has been read.');
    expect(html).toContain('this surface stays empty rather than showing placeholder Markets');
    expect(html).toContain('No Core program enumeration has been attempted.');
  });

  it('presents raw atoms and never a market-data metric', () => {
    expect(html).toContain('raw u64 atoms');
    expect(html).toContain('Hoard principal is never liquidity or TVL');
    expect(html).toContain('No volume · no odds · no probability · no yield');
    // Market-data vocabulary may appear only inside the sentences that refuse it.
    const disclaimers = [
      'There is no volume, price, odds, probability, or yield here, because none of those are facts this chain persists.',
      'Hoard principal is never liquidity or TVL',
      'No volume · no odds · no probability · no yield',
    ];
    let remainder = html;
    for (const disclaimer of disclaimers) {
      expect(remainder).toContain(disclaimer);
      remainder = remainder.split(disclaimer).join('');
    }
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'Probability', 'TVL', '24h', 'APR', 'APY', 'yield', 'Total value locked', '$']) {
      expect(remainder).not.toContain(forbidden);
    }
  });

  it('never exposes a signing or submission control on a discovery surface', () => {
    expect(html).not.toContain('Sign');
    expect(html).not.toContain('Submit');
    expect(html).not.toContain('Connect identity');
  });
});
