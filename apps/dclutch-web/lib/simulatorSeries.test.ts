import { describe, expect, it } from 'vitest';

import published from '@/public/simulator-series.json';
import {
  everyLineFlatV1,
  issuedSupplyLinesV1,
  parseSimulatorSeriesV1,
  readSimulatorSeriesV1,
  simulatorSeriesSpanV1,
  SIMULATOR_SERIES_SCHEMA_V1,
  SIMULATOR_SERIES_URL_V1,
} from './simulatorSeries';

/**
 * The series artifact is captured by hand (scripts/simulator-series.mjs) and
 * committed, because `git archive` is the only path to the live host. So the
 * committed file is the thing readers actually get, and it is checked here:
 * a capture that produced an artifact this decoder refuses would otherwise
 * ship as a silent "absent" and nobody would learn why the chart went away.
 */
describe('the published simulator series', () => {
  const series = parseSimulatorSeriesV1(published);

  it('decodes exactly as committed', () => {
    expect(series.schema).toBe(SIMULATOR_SERIES_SCHEMA_V1);
    expect(series.cluster).toBe('devnet');
    expect(series.points.length).toBeGreaterThan(0);
    expect(series.outcomeCount).toBeGreaterThan(0);
  });

  it('carries a market, so every line drawn from it can name what it is about', () => {
    expect(series.market).not.toBeNull();
  });

  it('keeps its cycles in order and its quantities exact', () => {
    for (const [index, point] of series.points.entries()) {
      if (index > 0) expect(point.cycle).toBeGreaterThan(series.points[index - 1].cycle);
      expect(point.slot).toMatch(/^(0|[1-9][0-9]*)$/);
      expect(point.supply).toHaveLength(series.outcomeCount);
      for (const atoms of point.supply) expect(atoms).toMatch(/^(0|[1-9][0-9]*)$/);
    }
  });

  it('never claims a conservation check was broken when the run is still going', () => {
    // A broken check halts the simulator. If one is ever recorded here, the
    // page must show it — this pin exists so a real violation cannot be
    // introduced by a careless capture rather than by the chain.
    const span = simulatorSeriesSpanV1(series);
    expect(span).not.toBeNull();
    expect(span?.checksBroken).toBe(0);
    expect(span?.checksHeld).toBeGreaterThan(0);
  });
});

describe('the series decoder', () => {
  const one = {
    schema: 'dclutch-simulator-series-v1',
    captured_at: '2026-08-30T16:40:07+00:00',
    cluster: 'devnet',
    market: '9JwhTHyxGhaoVsvSyT9VsJxV7PoQcPcjyhMLuJtY38Uq',
    mode: 'sustain',
    outcome_count: 2,
    cycles_recorded: 2,
    points_omitted_before: 0,
    census_file: 'cycle-000002.json',
    points: [
      { cycle: 1, slot: '10', recorded_at: '2026-08-30T15:58:11+00:00', supply: ['5', '5'], hoard_atoms: '5', tracked_collateral: '10', checks_held: 6, checks_broken: 0, checks_inapplicable: 1 },
      { cycle: 2, slot: '20', recorded_at: '2026-08-30T15:58:42+00:00', supply: ['6', '4'], hoard_atoms: '5', tracked_collateral: '10', checks_held: 6, checks_broken: 0, checks_inapplicable: 1 },
    ],
  };

  it('accepts a well-formed series', () => {
    const series = parseSimulatorSeriesV1(one);
    expect(series.points).toHaveLength(2);
    expect(series.points[1].supply).toEqual(['6', '4']);
    expect(series.censusFile).toBe('cycle-000002.json');
  });

  it('refuses cycles that run backwards, which would draw a shape that never happened', () => {
    const backwards = { ...one, points: [one.points[1], one.points[0]] };
    expect(() => parseSimulatorSeriesV1(backwards)).toThrow(/does not come after/);
  });

  it('refuses a point whose outcome count disagrees with the series', () => {
    const ragged = { ...one, points: [one.points[0], { ...one.points[1], supply: ['6'] }] };
    expect(() => parseSimulatorSeriesV1(ragged)).toThrow(/carries 1 outcomes/);
  });

  it('refuses a quantity that is not an exact decimal', () => {
    const rounded = { ...one, points: [{ ...one.points[0], supply: ['5.5', '5'] }, one.points[1]] };
    expect(() => parseSimulatorSeriesV1(rounded)).toThrow(/must be one exact decimal quantity/);
  });

  it('refuses another schema outright', () => {
    expect(() => parseSimulatorSeriesV1({ ...one, schema: 'something-else' })).toThrow(/another schema/);
  });

  it('names the field it refused, every time', () => {
    expect(() => parseSimulatorSeriesV1({ ...one, market: 'not-an-address' })).toThrow(/market must be one canonical Solana address/);
  });
});

describe('reading the published series over a static host', () => {
  it('reads a missing artifact as absent, not as an error', async () => {
    const read = await readSimulatorSeriesV1(async () => ({ ok: false, text: async () => '' }));
    expect(read.kind).toBe('absent');
  });

  it('reads the host fallback page as absent, because that is what a 200 HTML body means here', async () => {
    const read = await readSimulatorSeriesV1(async () => ({ ok: true, text: async () => '<!doctype html><title>404</title>' }));
    expect(read.kind).toBe('absent');
  });

  it('reads a real JSON document that fails the decoder as REFUSED, never as absent', async () => {
    const read = await readSimulatorSeriesV1(async () => ({ ok: true, text: async () => '{"schema":"dclutch-simulator-series-v1"}' }));
    expect(read.kind).toBe('refused');
    // The reason names the first field that failed, so the reader is told
    // which part of the document was wrong rather than that it "did not load".
    if (read.kind === 'refused') expect(read.reason).toBe('cluster must be local or devnet');
  });

  it('reads a good document as loaded, from the one pinned URL', async () => {
    let asked: string | null = null;
    const read = await readSimulatorSeriesV1(async (url) => {
      asked = url;
      return { ok: true, text: async () => JSON.stringify(one) };
    });
    expect(asked).toBe(SIMULATOR_SERIES_URL_V1);
    expect(read.kind).toBe('loaded');
  });

  const one = {
    schema: 'dclutch-simulator-series-v1',
    captured_at: '2026-08-30T16:40:07+00:00',
    cluster: 'devnet',
    market: null,
    mode: 'sustain',
    outcome_count: 1,
    cycles_recorded: 1,
    points_omitted_before: 0,
    census_file: 'cycle-000001.json',
    points: [{ cycle: 1, slot: '10', recorded_at: null, supply: ['5'], hoard_atoms: '5', tracked_collateral: '5', checks_held: 6, checks_broken: 0, checks_inapplicable: 1 }],
  };
});

describe('turning a series into lines', () => {
  const series = parseSimulatorSeriesV1({
    schema: 'dclutch-simulator-series-v1',
    captured_at: '2026-08-30T16:40:07+00:00',
    cluster: 'devnet',
    market: null,
    mode: 'sustain',
    outcome_count: 2,
    cycles_recorded: 2,
    points_omitted_before: 3,
    census_file: 'cycle-000005.json',
    points: [
      { cycle: 4, slot: '10', recorded_at: '2026-08-30T15:00:00+00:00', supply: ['5', '5'], hoard_atoms: '5', tracked_collateral: '10', checks_held: 6, checks_broken: 0, checks_inapplicable: 1 },
      { cycle: 5, slot: '30', recorded_at: '2026-08-30T15:30:00+00:00', supply: ['5', '5'], hoard_atoms: '5', tracked_collateral: '10', checks_held: 6, checks_broken: 0, checks_inapplicable: 1 },
    ],
  });

  it('names a line by its claim index, and adds the editorial outcome when there is one', () => {
    expect(issuedSupplyLinesV1(series)).toEqual([
      { label: 'claim 0', values: ['5', '5'] },
      { label: 'claim 1', values: ['5', '5'] },
    ]);
    expect(issuedSupplyLinesV1(series, ['below', 'above'])[1].label).toBe('claim 1 · above');
  });

  it('reports a wholly flat set of lines as flat', () => {
    expect(everyLineFlatV1(issuedSupplyLinesV1(series))).toBe(true);
  });

  it('measures the drawn window from the run’s own recorded instants, never from a slot rate', () => {
    const span = simulatorSeriesSpanV1(series);
    expect(span?.cycles).toBe(2);
    expect(span?.slotsCovered).toBe('20');
    expect(span?.minutesCovered).toBe(30);
    expect(span?.checksHeld).toBe(12);
  });

  it('counts the cycles it left out rather than pretending the run started here', () => {
    expect(series.pointsOmittedBefore).toBe(3);
    expect(series.points[0].cycle).toBe(4);
  });
});
