import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';

import published from '@/public/simulator-series.json';
import { parseSimulatorSeriesV1 } from '@/lib/simulatorSeries';
import { loadSimulatorSeriesV1, resetSimulatorSeriesCacheV1 } from '@/lib/simulatorSeriesClient';

import MarketIssuanceHistory from './MarketIssuanceHistory';

const series = parseSimulatorSeriesV1(published);
const OTHER_MARKET = '7Mcu1ZT9RJC3JLBLJmCLLCyj8LNTNRXpbGh9Ux2LZLhL';

describe('a market’s recorded issuance', () => {
  it('draws the run for the market the run is about', () => {
    const html = renderToStaticMarkup(
      <MarketIssuanceHistory address={series.market ?? ''} preloaded={{ kind: 'loaded', series }} />,
    );
    expect(html).toContain('<polyline');
    expect(html).toContain('recorded cycles');
    expect(html).toContain('The run continues past the last point');
  });

  /**
   * The attribution rule. One run exists, about one market; drawing its line
   * under a different market's heading would be the most convincing lie this
   * chart could tell, because every number in it would be real.
   */
  it('draws NOTHING under a market the run is not about', () => {
    const html = renderToStaticMarkup(
      <MarketIssuanceHistory address={OTHER_MARKET} preloaded={{ kind: 'loaded', series }} />,
    );
    expect(html).toBe('');
  });

  /**
   * On a listing, absence must be silent. An empty figure per card would
   * report a measurement nobody took, on markets nobody is exercising — and
   * twenty of them would read as twenty failures. The decoder's refusal still
   * gets said out loud, but on /pulse, which is the surface that owns the
   * artifact.
   */
  it('draws nothing, and says nothing, when no run was published', () => {
    expect(renderToStaticMarkup(
      <MarketIssuanceHistory address={series.market ?? ''} preloaded={{ kind: 'absent' }} />,
    )).toBe('');
  });

  it('draws nothing on a card when the artifact failed to decode', () => {
    expect(renderToStaticMarkup(
      <MarketIssuanceHistory address={series.market ?? ''} preloaded={{ kind: 'refused', reason: 'cluster must be local or devnet' }} />,
    )).toBe('');
  });

  it('names the outcomes when the caller supplies them, and the claim index always', () => {
    const html = renderToStaticMarkup(<MarketIssuanceHistory
      address={series.market ?? ''}
      outcomes={['first outcome', 'second outcome', 'third outcome', 'fourth outcome']}
      preloaded={{ kind: 'loaded', series }}
    />);
    expect(html).toContain('claim 0 · first outcome');
    expect(html).toContain('claim 3 · fourth outcome');
  });
});

describe('the shared series read', () => {
  afterEach(() => {
    resetSimulatorSeriesCacheV1();
    vi.unstubAllGlobals();
  });

  /**
   * A listing draws one card per market. If each card read the artifact for
   * itself, a twenty-market page would ask the host for the same file twenty
   * times — the same fan-out mistake as an unbatched chain read, and the same
   * fix: one read, shared.
   */
  it('asks the host once however many charts want the series', async () => {
    let calls = 0;
    vi.stubGlobal('fetch', async () => {
      calls += 1;
      return { ok: true, text: async () => JSON.stringify(published) };
    });
    const [a, b, c] = await Promise.all([
      loadSimulatorSeriesV1(),
      loadSimulatorSeriesV1(),
      loadSimulatorSeriesV1(),
    ]);
    expect(calls).toBe(1);
    expect(a.kind).toBe('loaded');
    expect(b).toBe(a);
    expect(c).toBe(a);
  });
});
