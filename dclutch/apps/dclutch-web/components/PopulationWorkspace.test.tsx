import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import published from '@/public/simlife-series.json';
import { parseSimulatorSeriesV1, SIMULATOR_SERIES_SCHEMA_V3 } from '@/lib/simulatorSeries';

import PopulationWorkspace, { NO_POPULATION_SENTENCE_V1, OutcomeSpread, populationOrRefusalV1 } from './PopulationWorkspace';
import { honestyRowsV1 } from '@/lib/simulatorSeries';

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

  /**
   * THE PAGE MUST NOT PUBLISH OUR TICKET QUEUE.
   *
   * This asserted the opposite until 2026-08-31 -- that the substrate's own
   * sentence reaches the page -- and it was passing while `/population` rendered
   * a file path, a Rust test name, an hour estimate and a raw nested Rust error
   * as public copy. The substrate's notes are written for whoever fixes the
   * thing and read like it. They stay in the capture, which is the record; the
   * page shows a short descriptive sentence.
   */
  it('shows what happened, and never the register note behind it', () => {
    const html = renderToStaticMarkup(
      <PopulationWorkspace preloaded={{ kind: 'loaded', series: capture }} />,
    );
    // No row's register note reaches the page. Forty-eight characters is long
    // enough to be that row's own sentence rather than a common word.
    for (const row of capture.world?.notDone ?? []) {
      expect(html).not.toContain(row.reason.slice(0, 48));
    }
    // And every sentence the strip DOES print is one of the short ones. The
    // strip shows one reason per route -- the commonest -- so this is over the
    // rows it renders rather than over every row in the capture.
    const printed = honestyRowsV1(capture)
      .map((row) => row.leadingReason)
      .filter((reason): reason is string => reason !== null);
    expect(printed.length).toBeGreaterThan(0);
    for (const reason of printed) expect(html).toContain(reason);
  });

  /**
   * A vocabulary ratchet rather than a list of the four things DESIGN-2 found.
   * Every one of these is a shape that only appears in text written for us, and
   * a page that grows a fifth leak fails here without anybody adding a case.
   */
  it('renders no file path, test name, hour estimate or raw error anywhere', () => {
    const html = renderToStaticMarkup(
      <PopulationWorkspace preloaded={{ kind: 'loaded', series: capture }} />,
    );
    const body = html.replace(/<[^>]*>/g, ' ');
    for (const shape of [
      /[A-Za-z0-9_-]+\/[A-Za-z0-9_/-]+\.(rs|py|ts|tsx|mjs|toml)/,  // a file path
      /\b[a-z]+(_[a-z]+){3,}\b/,                                   // a Rust test name
      /\b\d+\s*-\s*\d+\s+hours\b/,                                // an hour estimate
      /Error\s*:\s*Error\(/,                                       // a nested Rust error
      /\bsha256\b/i,
      /::/,
    ]) {
      expect(body).not.toMatch(shape);
    }
  });
});


describe('where the answers landed', () => {
  const spread = (overrides: Record<string, unknown> = {}) => ({
    ...capture,
    world: capture.world === null ? null : {
      ...capture.world,
      outcomeSpread: {
        resolvingMarkets: 9, distinctCells: 6,
        counts: { '0/3': 3, '1/3': 2, '2/5': 2, '4/7': 1, '0/7': 1 },
        positionedMarkets: 9,
        positionCounts: { '0': 4, '5': 2, '6': 1, '10': 2 },
        distinctPositions: 4, heaviestPositionTenths: 0, heaviestSharePercent: 44,
        degenerateThresholdPercent: 70, degenerate: false, coordinateAnchor: '100000000',
        ...overrides,
      },
    },
  });

  it('draws every position including the ones nothing landed in', () => {
    const html = renderToStaticMarkup(<OutcomeSpread series={spread()} />);
    // All eleven, because a bucket nothing reached is a reading and leaving it
    // out would redraw the axis around the answer.
    for (let tenths = 0; tenths <= 10; tenths += 1) {
      expect(html).toContain(`>${tenths}/10</th>`);
    }
    expect(html).toContain('100000000');
    expect(html).not.toContain('over the');
  });

  it('says so when one position takes more of the world than the threshold', () => {
    const html = renderToStaticMarkup(<OutcomeSpread series={spread({
      positionCounts: { '10': 9 }, distinctPositions: 1,
      heaviestPositionTenths: 10, heaviestSharePercent: 100, degenerate: true,
    })} />);
    expect(html).toContain('100%');
    expect(html).toContain('70% threshold');
    expect(html).toContain('population-broken');
  });

  it('says a capture predating the histogram predates it, rather than drawing zero', () => {
    // Built explicitly rather than taken from the committed capture: that
    // capture now CARRIES a histogram, and a test that reached for it would
    // have quietly stopped exercising this branch the moment it did.
    const older = capture.world === null ? capture : {
      ...capture,
      world: { ...capture.world, outcomeSpread: null },
    };
    const html = renderToStaticMarkup(<OutcomeSpread series={older} />);
    expect(html).toContain('predates');
    expect(html).not.toContain('0/10');
  });
});
