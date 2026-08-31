import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import published from '@/public/simlife-series.json';
import { parseSimulatorSeriesV1, SIMULATOR_SERIES_SCHEMA_V3 } from '@/lib/simulatorSeries';

import PopulationWorkspace, { NO_POPULATION_SENTENCE_V1, populationOrRefusalV1 } from './PopulationWorkspace';

/**
 * The population surface, and the two rules it exists to enforce.
 *
 * ONE: it draws a POPULATION or it draws nothing. A v1/v2/v3 capture decodes
 * perfectly well and is one market; every caption on this page would be false
 * about it, so it is refused by name rather than drawn under a heading that
 * promises many.
 *
 * TWO: the honesty strip is load-bearing, not decorative. A run that mutated
 * nothing and a run that founded its own markets must not render the same, and
 * the four words must never be added together on the way to the page.
 */
const capture = parseSimulatorSeriesV1(published);

describe('the committed population capture', () => {
  it('decodes as a population against a loopback chain', () => {
    expect(capture.world).not.toBeNull();
    expect(capture.cluster).toBe('local');
    expect(capture.world?.substrate.rpcOrigin?.startsWith('http://127.0.0.1:')).toBe(true);
    expect(capture.markets.length).toBeGreaterThan(0);
  });

  it('draws only markets it observed, and every planned market is accounted for', () => {
    const observed = new Set(capture.markets.map((market) => market.marketId));
    for (const planned of capture.world?.planned ?? []) {
      expect(planned.observed).toBe(observed.has(planned.marketId));
    }
    expect(capture.world?.marketsObserved).toBe(capture.markets.length);
  });

  it('renders every section without a market it did not observe appearing as a line', () => {
    const html = renderToStaticMarkup(
      <PopulationWorkspace preloaded={{ kind: 'loaded', series: capture }} />,
    );
    for (const market of capture.markets) expect(html).toContain(market.marketId);
    const unobserved = (capture.world?.planned ?? []).filter((market) => !market.observed);
    for (const market of unobserved) {
      // It may be NAMED in a table of what the world drew; what it must never
      // have is a chart card of its own.
      expect(html).not.toContain(`>${market.marketId}</strong>`);
    }
  });
});

describe('what this page refuses to draw', () => {
  it('refuses a single-market capture rather than drawing one line under a plural heading', () => {
    const single = parseSimulatorSeriesV1({ ...published, schema: SIMULATOR_SERIES_SCHEMA_V3, world: null, markets: [] });
    const state = populationOrRefusalV1({ kind: 'loaded', series: single });
    expect(state.kind).toBe('refused');
    expect(state.kind === 'refused' && state.reason).toContain('carries no world block');
  });

  it('says nothing is published rather than drawing an empty axis', () => {
    const html = renderToStaticMarkup(<PopulationWorkspace preloaded={{ kind: 'absent' }} />);
    expect(html).toContain(NO_POPULATION_SENTENCE_V1.slice(0, 40));
  });

  it('shows the decoder’s own words when a capture is refused', () => {
    const html = renderToStaticMarkup(
      <PopulationWorkspace preloaded={{ kind: 'refused', reason: 'world says m09 was observed and no series carries it' }} />,
    );
    expect(html).toContain('world says m09 was observed and no series carries it');
  });
});

describe('the honesty strip', () => {
  it('prints every route the world planned, with its four endings kept apart', () => {
    const html = renderToStaticMarkup(
      <PopulationWorkspace preloaded={{ kind: 'loaded', series: capture }} />,
    );
    const routes = new Set([
      ...Object.keys(capture.world?.tally ?? {}),
      ...(capture.world?.notDone ?? []).map((row) => row.route),
    ]);
    for (const route of routes) expect(html).toContain(`>${route}</th>`);
    // The three not-done words appear as their own columns, so no total can
    // stand in for them.
    expect(html).toContain('refused');
    expect(html).toContain('not attempted');
    expect(html).toContain('blocked');
  });

  it('carries the substrate’s own sentence for what it could not do', () => {
    const html = renderToStaticMarkup(
      <PopulationWorkspace preloaded={{ kind: 'loaded', series: capture }} />,
    );
    const leading = (capture.world?.notDone ?? []).slice().sort((a, b) => b.count - a.count)[0];
    if (leading !== undefined) expect(html).toContain(leading.reason.slice(0, 40));
  });
});
