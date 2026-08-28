import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from '@/lib/deployments';

import MarketDiscoveryWorkspace, { EmptyMarkets } from './MarketDiscoveryWorkspace';

/**
 * The product inversion this surface carries: /markets lands on CONTENT. The
 * deployment manifest supplies the endpoint and the Core authority, the list
 * auto-loads, and there is no infrastructure form anywhere on the page. These
 * tests pin the inversion so the ask-the-visitor pattern cannot creep back.
 */
describe('Market discovery route', () => {
  const html = renderToStaticMarkup(<MarketDiscoveryWorkspace />);

  it('lands on the market list of the baked deployment, loading with zero typing', () => {
    expect(html).toContain('Markets on Devnet');
    expect(html).toContain('Reading the finalized market list…');
    expect(html).toContain('enumerated from the Core program itself');
    expect(html).toContain('whole current-compatible market list');
    // The one button is a refresh, disabled while the auto-load is in flight.
    expect(html).toContain('>Reading…</button>');
  });

  it('asks the visitor for NO endpoint and NO program address', () => {
    expect(html).not.toContain('Finalized RPC endpoint');
    expect(html).not.toContain('Core program</span>');
    expect(html).not.toContain('Registry program · optional');
    expect(html).not.toContain('Known Market addresses');
    expect(html).not.toContain('<textarea');
    expect(html).not.toContain('<input');
  });

  it('states the provenance and refusal contract every card is held to', () => {
    expect(html).toContain('CHAIN · finalized slot');
    expect(html).toContain('REFUSED');
    expect(html).toContain('never partially invented');
  });

  it('presents raw atoms and never a market-data metric', () => {
    expect(html).toContain('raw u64 atoms');
    expect(html).toContain('No volume · no odds · no probability · no yield');
    // Market-data vocabulary may appear only inside the sentences that refuse it.
    const disclaimers = [
      'There is no volume, price, odds, probability, or yield here, because none of those are facts this chain persists.',
      'Supplies come from the Claims aggregate, never from the root, in raw u64 atoms.',
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

  it('renders historical incompatible accounts without listing them as current markets', () => {
    const legacyAddress = '3Dhpq9tufPuBMroMfUNaWhfZMPfLFh6MG7vwhJFfqjMm';
    const empty = renderToStaticMarkup(<EmptyMarkets
      deployment={DEVNET_DEPLOYMENT_V1}
      enumeration={{
        mode: 'program-scan',
        note: 'test scan',
        scanSlot: '489269449',
        addresses: Object.freeze([]),
        scannedAccounts: 2,
        incompatibleMarketAccounts: Object.freeze([
          Object.freeze({ address: legacyAddress, magic: 'DCLTCOR2', accountBytes: 352 }),
        ]),
      }}
    />);
    expect(empty).toContain('No current compatible market is listed on devnet');
    expect(empty).toContain('1 historical DCLTCOR2 Market account');
    expect(empty).toContain('disclosed here but not listed as current');
    expect(empty).toContain(legacyAddress);
    expect(empty).toContain(`/explorer?view=account&amp;q=${legacyAddress}`);
    expect(empty).not.toContain('No markets on devnet');
  });
});
