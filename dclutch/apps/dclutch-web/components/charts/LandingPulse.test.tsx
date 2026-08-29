import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import LandingPulse, { emptyCurrentMarketPulseV1, partiallyReadPulseV1 } from './LandingPulse';

describe('LandingPulse', () => {
  it('renders every count as unread while nothing has been read, never as zero', () => {
    const html = renderToStaticMarkup(<LandingPulse />);
    expect(html).toContain('Current markets listed');
    expect(html).toContain('Collateral in listed markets');
    expect(html).toContain('Resolutions in listed markets');
    expect(html.split('>—</strong>').length - 1).toBe(3);
    expect(html).toContain('Reading live from the chain…');
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
    // The facts are unchanged; who they are addressed to is not. A reader on
    // the landing page does not know what DCLTCOR2 or a 352-byte layout is,
    // and does not need to in order to understand that some older markets
    // exist and are not in the count.
    expect(state.provenance).toContain('holds no market this page can read');
    expect(state.provenance).toContain('2 older markets');
    expect(state.provenance).toContain('not counted above');
    expect(state.provenance).not.toContain('owns no Market');
    expect(state.provenance).not.toContain('DCLTCOR2');
  });
});

describe('a scan that answered and a join that did not', () => {
  const enumeration = Object.freeze({
    mode: 'program-scan' as const,
    note: 'test scan',
    scanSlot: '489905402',
    addresses: Object.freeze(['36CHzLdAujpE8c23ThGiKLNJLVndC2R5ogit1HHNFXFQ']),
    scannedAccounts: 16,
    incompatibleMarketAccounts: Object.freeze([]),
  });

  it('keeps the count it actually read instead of blanking the whole strip', () => {
    // The scan is one request; the join is roughly four per market. Against a
    // throttling public endpoint the second can fail after the first answered,
    // and the front page is the worst place to throw away a number we hold.
    const state = partiallyReadPulseV1('Devnet', enumeration, 'the endpoint is rate-limiting this browser (HTTP 429).');
    expect(state.stats[0]).toMatchObject({ label: 'Current markets listed', value: '1' });
    expect(state.provenance).toContain('holds 1 market');
    expect(state.provenance).toContain('Reading inside them did not finish');
    expect(state.provenance).toContain('rate-limiting');
  });

  it('leaves the two it did not read as dashes, never as zeroes', () => {
    // A zero here would be a claim about collateral and resolutions that no
    // read supports. The page's own rule: a dash means we could not read it.
    const state = partiallyReadPulseV1('Devnet', enumeration, 'network down');
    expect(state.stats[1].value).toBeNull();
    expect(state.stats[2].value).toBeNull();
    expect(state.stats[1].detail).toBe('not read this time');
  });
});
