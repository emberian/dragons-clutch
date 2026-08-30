import { describe, expect, it } from 'vitest';

import published from '@/public/simulator-series.json';
import {
  conservationLawRowsV1,
  conservationReadingV1,
  everyLineFlatV1,
  issuedSupplyLinesV1,
  lawBandCyclesV1,
  parseSimulatorSeriesV1,
  readSimulatorSeriesV1,
  simulatorHeartbeatV1,
  simulatorSeriesSpanV1,
  SIMULATOR_SERIES_SCHEMA_V2,
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
    expect(series.schema).toBe(SIMULATOR_SERIES_SCHEMA_V2);
    expect(series.cluster).toBe('devnet');
    expect(series.points.length).toBeGreaterThan(0);
    expect(series.outcomeCount).toBeGreaterThan(0);
  });

  /**
   * The published capture must carry the laws by NAME, not only by count.
   * A capture that silently regressed to v1's three integers would still
   * decode and would still draw — as an empty band with a sentence explaining
   * itself, which is honest and is not what anyone published this for.
   */
  it('carries every conservation law by name, aligned to every drawn cycle', () => {
    expect(series.lawIds.length).toBeGreaterThan(0);
    expect(series.laws.map((law) => law.id)).toEqual([...series.lawIds]);
    for (const law of series.laws) expect(law.detail.length).toBeGreaterThan(0);
    const rows = conservationLawRowsV1(series);
    const cycles = lawBandCyclesV1(series);
    expect(rows).toHaveLength(series.lawIds.length);
    for (const row of rows) expect(row.statuses).toHaveLength(cycles.length);
  });

  it('never publishes a violated law without the page leading on it', () => {
    // A broken law halts the run. If one is ever captured, the reading must
    // put it first — this pin is the reason the sentence is built rather than
    // assembled at the call site.
    const reading = conservationReadingV1(series);
    expect(reading).not.toBeNull();
    const violated = conservationLawRowsV1(series).filter((row) => row.violated > 0);
    if (violated.length === 0) expect(reading).toContain('none broke');
    else expect(reading?.startsWith(violated[0].id)).toBe(true);
  });

  /**
   * The verdict this artifact exists to make renderable: the market's own
   * quantities do not move, and the chain does. If a future capture ever has a
   * moving supply this test does not fail — it just stops being the point.
   */
  it('has a chain clock that moves even while the market does not', () => {
    const heartbeat = simulatorHeartbeatV1(series);
    expect(heartbeat).not.toBeNull();
    expect(heartbeat?.slotAdvance.values.length).toBe(series.points.length - 1);
    expect(new Set(heartbeat?.slotAdvance.values).size).toBeGreaterThan(1);
    expect(heartbeat?.measuredSlotRate).toMatch(/^\d+\.\d\d$/);
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

  /**
   * v1 stays readable and decodes as a series with no laws recorded. A capture
   * taken before the names existed genuinely has none, and that is a true
   * thing to say about it — not a decode failure and not an invented set.
   */
  it('still reads a v1 document, as a series that recorded no law names', () => {
    const series = parseSimulatorSeriesV1(one);
    expect(series.lawIds).toEqual([]);
    expect(conservationLawRowsV1(series)).toEqual([]);
    expect(conservationReadingV1(series)).toBeNull();
    expect(series.points[0].lawStatuses).toEqual([]);
  });

  const withLaws = {
    ...one,
    schema: 'dclutch-simulator-series-v2',
    law_ids: ['L1', 'L2'],
    laws: [
      { id: 'L1', status: 'holds', detail: 'tracked 10 atoms == Mint supply 10' },
      { id: 'L2', status: 'inapplicable', detail: 'no predecessor to move from' },
    ],
    points: [
      { ...one.points[0], law_statuses: 'hi' },
      { ...one.points[1], law_statuses: 'hh' },
    ],
  };

  it('expands the compact verdict string into named statuses', () => {
    const series = parseSimulatorSeriesV1(withLaws);
    expect(series.points[0].lawStatuses).toEqual(['holds', 'inapplicable']);
    const rows = conservationLawRowsV1(series);
    expect(rows[1]).toMatchObject({ id: 'L2', held: 1, inapplicable: 1, violated: 0 });
    expect(rows[0].detail).toBe('tracked 10 atoms == Mint supply 10');
    expect(lawBandCyclesV1(series)).toEqual([1, 2]);
  });

  /**
   * The misalignment refusal, which is the whole reason the count is checked
   * rather than trusted: a shifted verdict string would report L2's result
   * under L1's name, and every downstream reader would believe it.
   */
  it('refuses a verdict string that does not match the number of named laws', () => {
    const shifted = { ...withLaws, points: [{ ...withLaws.points[0], law_statuses: 'h' }, withLaws.points[1]] };
    expect(() => parseSimulatorSeriesV1(shifted)).toThrow(/carries 1 verdicts and the series declares 2 laws/);
  });

  it('refuses a verdict character it does not know, instead of guessing at it', () => {
    const unknown = { ...withLaws, points: [{ ...withLaws.points[0], law_statuses: 'hx' }, withLaws.points[1]] };
    expect(() => parseSimulatorSeriesV1(unknown)).toThrow(/verdict 1 is not one of h, v, i/);
  });

  it('refuses laws whose order disagrees with the names they are drawn under', () => {
    const swapped = { ...withLaws, laws: [withLaws.laws[1], withLaws.laws[0]] };
    expect(() => parseSimulatorSeriesV1(swapped)).toThrow(/law 0 is L2 and law_ids names L1/);
  });

  it('leads on the violation when there is one', () => {
    const broken = parseSimulatorSeriesV1({
      ...withLaws,
      points: [{ ...withLaws.points[0], law_statuses: 'hi' }, { ...withLaws.points[1], law_statuses: 'vh' }],
    });
    expect(conservationReadingV1(broken)?.startsWith('L1 did not hold')).toBe(true);
  });
});

/**
 * The heartbeat: the derivation that decides whether this page reads as alive.
 *
 * Its risks are arithmetic ones. A slot delta is a u64 difference and must not
 * round; a cadence line with a hole in it must not be silently redrawn shorter
 * than its own x-axis; and a rate is a claim about the chain that must come
 * from the run's own two totals rather than from a constant.
 */
describe('the heartbeat', () => {
  const build = (points: ReadonlyArray<unknown>) => parseSimulatorSeriesV1({
    schema: 'dclutch-simulator-series-v2',
    captured_at: '2026-08-30T16:40:07+00:00',
    cluster: 'devnet',
    market: null,
    mode: 'sustain',
    outcome_count: 1,
    cycles_recorded: points.length,
    points_omitted_before: 0,
    census_file: 'cycle-000003.json',
    points,
  });
  const point = (cycle: number, slot: string, recordedAt: string | null) => ({
    cycle, slot, recorded_at: recordedAt, supply: ['5'], hoard_atoms: '5', tracked_collateral: '5',
    checks_held: 6, checks_broken: 0, checks_inapplicable: 1,
  });

  const series = build([
    point(1, '100', '2026-08-30T15:00:00+00:00'),
    point(2, '160', '2026-08-30T15:00:20+00:00'),
    point(3, '400', '2026-08-30T15:01:00+00:00'),
  ]);

  it('measures the chain advance between readings exactly, and labels each interval', () => {
    const heartbeat = simulatorHeartbeatV1(series);
    expect(heartbeat?.slotAdvance.values).toEqual(['60', '240']);
    expect(heartbeat?.cadence?.values).toEqual(['20', '40']);
    expect(heartbeat?.xLabels).toEqual(['cycle 1 → 2', 'cycle 2 → 3']);
    expect(heartbeat?.intervals).toBe(2);
  });

  it('divides the run’s own totals for the rate rather than assuming one', () => {
    // 300 slots over 60 recorded seconds. Nothing about Solana's nominal rate
    // is consulted, which is the point: this is what THIS run observed.
    expect(simulatorHeartbeatV1(series)?.measuredSlotRate).toBe('5.00');
    expect(simulatorHeartbeatV1(series)?.longestGapSeconds).toBe('40');
    expect(simulatorHeartbeatV1(series)?.shortestGapSeconds).toBe('20');
  });

  it('drops the cadence line entirely when an instant is missing, rather than shortening it', () => {
    const holed = simulatorHeartbeatV1(build([
      point(1, '100', '2026-08-30T15:00:00+00:00'),
      point(2, '160', null),
      point(3, '400', '2026-08-30T15:01:00+00:00'),
    ]));
    expect(holed?.slotAdvance.values).toEqual(['60', '240']);
    expect(holed?.cadence).toBeNull();
    expect(holed?.measuredSlotRate).toBeNull();
  });

  it('keeps a u64 slot difference exact, where a float would not', () => {
    const wide = simulatorHeartbeatV1(build([
      point(1, '18446744073709551000', null),
      point(2, '18446744073709551615', null),
    ]));
    expect(wide?.slotAdvance.values).toEqual(['615']);
  });

  it('has nothing to measure between when only one cycle was recorded', () => {
    expect(simulatorHeartbeatV1(build([point(1, '100', null)]))).toBeNull();
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
