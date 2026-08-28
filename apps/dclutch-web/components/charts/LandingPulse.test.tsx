import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import LandingPulse, { emptyCurrentMarketPulseV1 } from './LandingPulse';

describe('LandingPulse', () => {
  it('renders every count as unread while nothing has been read, never as zero', () => {
    const html = renderToStaticMarkup(<LandingPulse />);
    expect(html).toContain('Current markets listed');
    expect(html).toContain('Collateral in listed markets');
    expect(html).toContain('Resolutions in listed markets');
    expect(html.split('>—</strong>').length - 1).toBe(3);
    expect(html).toContain('Reading finalized state from the active deployment…');
    expect(html).not.toContain('>0</strong>');
  });

  it('reports zero current listings without erasing incompatible historical Markets', () => {
    const state = emptyCurrentMarketPulseV1('Devnet', {
      mode: 'program-scan',
      note: 'test scan',
      scanSlot: '489269449',
      addresses: Object.freeze([]),
      scannedAccounts: 7,
      incompatibleMarketAccounts: Object.freeze([
        Object.freeze({ address: '3Dhpq9tufPuBMroMfUNaWhfZMPfLFh6MG7vwhJFfqjMm', magic: 'DCLTCOR2', accountBytes: 352 }),
        Object.freeze({ address: '8mQmwmQMwtUeW8SyzABrgM7W8wFb2UPpQMeavgcX87z', magic: 'DCLTCOR2', accountBytes: 352 }),
      ]),
    });
    expect(state.stats[0]).toMatchObject({ label: 'Current markets listed', value: '0' });
    expect(state.provenance).toContain('zero current compatible Markets are listed');
    expect(state.provenance).toContain('2 historical DCLTCOR2 Market accounts');
    expect(state.provenance).toContain('not listed as current');
    expect(state.provenance).not.toContain('owns no Market');
  });
});
