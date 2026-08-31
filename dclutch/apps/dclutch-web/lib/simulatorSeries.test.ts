import { describe, expect, it } from 'vitest';

import published from '@/public/simulator-series.json';
import simlife from '@/public/simlife-series.json';
import {
  campaignReadingV1,
  campaignSpendLineV1,
  campaignStageLabelsV1,
  campaignVolumeV1,
  conservationLawRowsV1,
  conservationReadingV1,
  everyLineFlatV1,
  hoardCoverageLinesV1,
  impliedOddsLinesV1,
  issuedSupplyLinesV1,
  lawBandCyclesV1,
  parseSimulatorSeriesV1,
  readSimulatorSeriesV1,
  settlementCellsV1,
  simulatorHeartbeatV1,
  simulatorSeriesSpanV1,
  SIMULATOR_SERIES_SCHEMA_V2,
  SIMULATOR_SERIES_SCHEMA_V3,
  SIMULATOR_SERIES_SCHEMA_V4,
  SIMULATOR_SERIES_URL_V1,
  archetypeCensusV1,
  eventTimelineLabelsV1,
  eventTimelineLinesV1,
  executedReadingV1,
  honestyRowsV1,
  marketOddsLinesV1,
  marketRowsV1,
  marketSlotLabelsV1,
  notDoneReadingV1,
  populationReadingV1,
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
    if (violated.length === 0) expect(reading).toContain('checks held and none broke');
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

/**
 * v3: the same series taken at a CAMPAIGN's stage boundaries.
 *
 * Everything here is about the two failure modes a campaign record makes newly
 * available. The first is misattribution — a per-cell figure laid under the
 * wrong cell, a settlement naming a column that is not on the chart. The
 * second is the one this whole project is arranged against: a local rehearsal
 * read as though it were devnet.
 */
describe('a campaign series', () => {
  /** A v2-shaped census record, for the "everything earlier still reads" pin. */
  const censusV1 = {
    schema: 'dclutch-simulator-series-v1',
    captured_at: '2026-08-30T16:40:07+00:00',
    cluster: 'devnet',
    market: '9JwhTHyxGhaoVsvSyT9VsJxV7PoQcPcjyhMLuJtY38Uq',
    mode: 'sustain',
    outcome_count: 2,
    cycles_recorded: 1,
    points_omitted_before: 0,
    census_file: 'cycle-000001.json',
    points: [
      { cycle: 1, slot: '10', recorded_at: '2026-08-30T15:58:11+00:00', supply: ['5', '5'], hoard_atoms: '5', tracked_collateral: '10', checks_held: 6, checks_broken: 0, checks_inapplicable: 1 },
    ],
  };

  const campaign = {
    schema: 'dclutch-simulator-series-v3',
    captured_at: '2026-08-30T18:00:00+00:00',
    cluster: 'local',
    market: '9JwhTHyxGhaoVsvSyT9VsJxV7PoQcPcjyhMLuJtY38Uq',
    mode: 'finite',
    outcome_count: 2,
    cycles_recorded: 3,
    points_omitted_before: 0,
    census_file: 'transcript.json',
    claim_unit_atoms: '2',
    campaign: {
      label: 'relayed-vertical success walk',
      source_revision: '0123456789abcdef0123456789abcdef01234567',
      walk: 'success',
      rpc_origin: 'http://127.0.0.1:31500/',
      transcript_file: 'transcript.json',
    },
    settlement: { selected_cell: 0, failure_cell: 1, certificate: '9JwhTHyxGhaoVsvSyT9VsJxV7PoQcPcjyhMLuJtY38Uq' },
    law_ids: ['L1'],
    laws: [{ id: 'L1', status: 'holds', detail: 'tracked 10 atoms == Mint supply 10' }],
    points: [
      { cycle: 1, slot: '10', recorded_at: null, stage: 'founding through Open', supply: ['5', '5'], position_totals: ['5', '5'], hoard_atoms: '5', tracked_collateral: '10', mint_supply: '10', payer_lamports: '900', transactions: 100, compute_units: '5000', fee_lamports: '700', law_statuses: 'h', checks_held: 1, checks_broken: 0, checks_inapplicable: 0 },
      { cycle: 2, slot: '20', recorded_at: null, stage: 'resolution funding active', supply: ['6', '4'], position_totals: ['6', '4'], hoard_atoms: '5', tracked_collateral: '10', mint_supply: '10', payer_lamports: '800', transactions: 10, compute_units: '600', fee_lamports: '70', law_statuses: 'h', checks_held: 1, checks_broken: 0, checks_inapplicable: 0 },
      { cycle: 3, slot: '30', recorded_at: null, stage: 'market terminalized', supply: ['6', '4'], position_totals: ['6', '4'], hoard_atoms: '5', tracked_collateral: '10', mint_supply: '10', payer_lamports: '700', transactions: 4, compute_units: '90', fee_lamports: '7', law_statuses: 'h', checks_held: 1, checks_broken: 0, checks_inapplicable: 0 },
    ],
  };

  it('decodes the campaign, the claim unit and the settlement', () => {
    const series = parseSimulatorSeriesV1(campaign);
    expect(series.schema).toBe(SIMULATOR_SERIES_SCHEMA_V3);
    expect(series.campaign?.sourceRevision).toBe('0123456789abcdef0123456789abcdef01234567');
    expect(series.claimUnitAtoms).toBe('2');
    expect(series.settlement).toEqual({ selectedCell: 0, failureCell: 1, certificate: '9JwhTHyxGhaoVsvSyT9VsJxV7PoQcPcjyhMLuJtY38Uq' });
    expect(series.points[0].stage).toBe('founding through Open');
    expect(campaignStageLabelsV1(series)).toEqual(['founding through Open', 'resolution funding active', 'market terminalized']);
  });

  /**
   * A selector past the end would light a column that is not on the chart, or
   * — worse, because nobody would notice — the wrong one.
   */
  it('refuses a settlement that names a cell the series does not have', () => {
    expect(() => parseSimulatorSeriesV1({ ...campaign, settlement: { selected_cell: 2 } }))
      .toThrow(/selects cell 2 and the series declares 2 outcomes/);
    expect(() => parseSimulatorSeriesV1({ ...campaign, settlement: { selected_cell: 0, failure_cell: 9 } }))
      .toThrow(/failure cell 9/);
  });

  it('refuses position totals laid under the wrong number of cells', () => {
    const ragged = { ...campaign, points: [{ ...campaign.points[0], position_totals: ['5'] }, campaign.points[1], campaign.points[2]] };
    expect(() => parseSimulatorSeriesV1(ragged)).toThrow(/1 position totals and the series declares 2 outcomes/);
  });

  it('refuses a campaign block that will not say which build it ran', () => {
    const anonymous = { ...campaign, campaign: { ...campaign.campaign, source_revision: '' } };
    expect(() => parseSimulatorSeriesV1(anonymous)).toThrow(/campaign source_revision/);
  });

  /**
   * The odds are the market's own liability supply as a share, floored to the
   * basis point on exact integers. 6 of 10 is 6,000 and not 60.00000001.
   */
  it('computes each cell’s share of the issued supply, exactly and floored', () => {
    const series = parseSimulatorSeriesV1(campaign);
    expect(impliedOddsLinesV1(series)).toEqual([
      { label: 'claim 0', values: ['5000', '6000', '6000'] },
      { label: 'claim 1', values: ['5000', '4000', '4000'] },
    ]);
    expect(impliedOddsLinesV1(series, ['no', 'yes'])[1].label).toBe('claim 1 · yes');
  });

  /**
   * A share of nothing is undefined, not zero. Drawing it as zero would put a
   * cell at 0% at a boundary where the market had issued nothing at all.
   */
  it('draws no odds line at all when any boundary issued nothing', () => {
    const empty = { ...campaign, points: [{ ...campaign.points[0], supply: ['0', '0'] }, campaign.points[1], campaign.points[2]] };
    expect(impliedOddsLinesV1(parseSimulatorSeriesV1(empty))).toEqual([]);
  });

  it('draws the vault against the tracked total and the Mint supply', () => {
    const lines = hoardCoverageLinesV1(parseSimulatorSeriesV1(campaign));
    expect(lines.map((line) => line.label)).toEqual([
      'in the market’s own Hoard',
      'tracked across every named account',
      'the collateral Mint’s whole supply',
    ]);
    expect(lines[0].values).toEqual(['5', '5', '5']);
  });

  /** A line with a hole would be redrawn shorter than the axis it sits on. */
  it('drops the Mint-supply line when one boundary did not record it', () => {
    const partial = { ...campaign, points: [{ ...campaign.points[0], mint_supply: null }, campaign.points[1], campaign.points[2]] };
    expect(hoardCoverageLinesV1(parseSimulatorSeriesV1(partial))).toHaveLength(2);
  });

  it('reports the work per boundary and its totals, never one axis for two dimensions', () => {
    const volume = campaignVolumeV1(parseSimulatorSeriesV1(campaign));
    expect(volume?.transactions?.values).toEqual(['100', '10', '4']);
    expect(volume?.computeUnits?.values).toEqual(['5000', '600', '90']);
    expect(volume?.totalTransactions).toBe('114');
    expect(volume?.totalComputeUnits).toBe('5690');
    expect(volume?.totalFeeLamports).toBe('777');
    expect(volume?.xLabels).toEqual(['founding through Open', 'resolution funding active', 'market terminalized']);
  });

  it('reports no volume at all for a record that carries none, rather than a line of zeroes', () => {
    const censusShaped = {
      ...campaign,
      points: campaign.points.map((point) => ({ ...point, transactions: null, compute_units: null, fee_lamports: null })),
    };
    expect(campaignVolumeV1(parseSimulatorSeriesV1(censusShaped))).toBeNull();
  });

  /**
   * The settlement is total: the selected cell is worth the claim unit and
   * every other cell is worth nothing. That is the only price move this record
   * contains, and it is stated rather than drawn.
   */
  it('states what one claim on each cell turned out to be worth', () => {
    expect(settlementCellsV1(parseSimulatorSeriesV1(campaign))).toEqual([
      { cell: 0, label: 'claim 0', selected: true, claimsIssued: '6', realizedAtomsPerClaim: '2', realizedAtoms: '12' },
      { cell: 1, label: 'claim 1', selected: false, claimsIssued: '4', realizedAtomsPerClaim: '0', realizedAtoms: '0' },
    ]);
  });

  /** Approximating a price primitive would make every per-cell figure approximate. */
  it('states nothing about a claim’s worth when the record carries no claim unit', () => {
    expect(settlementCellsV1(parseSimulatorSeriesV1({ ...campaign, claim_unit_atoms: null }))).toEqual([]);
    expect(settlementCellsV1(parseSimulatorSeriesV1({ ...campaign, settlement: null }))).toEqual([]);
  });

  /**
   * The fee payer's drawdown is a LEVEL, so it is its own reading and never
   * shares an axis with the per-interval counts above.
   */
  it('measures what the run spent as the exact drop from the first boundary', () => {
    expect(campaignSpendLineV1(parseSimulatorSeriesV1(campaign))?.values).toEqual(['0', '100', '200']);
  });

  it('draws no spend line for a payer whose balance rose, rather than a negative spend', () => {
    const toppedUp = {
      ...campaign,
      points: [campaign.points[0], { ...campaign.points[1], payer_lamports: '1000' }, campaign.points[2]],
    };
    expect(campaignSpendLineV1(parseSimulatorSeriesV1(toppedUp))).toBeNull();
  });

  it('leads its reading with where the run happened, because that is the fact most easily got wrong', () => {
    const reading = campaignReadingV1(parseSimulatorSeriesV1(campaign));
    expect(reading).toContain('a local rehearsal validator at http://127.0.0.1:31500/');
    expect(reading).toContain('cell 0 was selected');
    expect(campaignReadingV1(parseSimulatorSeriesV1(censusV1))).toBeNull();
  });

  /** v1 and v2 documents keep decoding, as records that carry none of this. */
  it('leaves every earlier capture readable, carrying none of the campaign fields', () => {
    const older = parseSimulatorSeriesV1(censusV1);
    expect(older.campaign).toBeNull();
    expect(older.settlement).toBeNull();
    expect(older.claimUnitAtoms).toBeNull();
    expect(older.points[0].stage).toBeNull();
    expect(older.points[0].positionTotals).toEqual([]);
    expect(campaignVolumeV1(older)).toBeNull();
  });
});

/**
 * v4: a POPULATION of markets, drawn from one seed and censused at the same
 * ticks.
 *
 * Three things are being pinned here and they are the three ways this schema
 * could quietly lie. A market with no points must never be drawn as a market
 * whose line is flat at zero, so `observed` and `markets` must agree. A caption
 * that counts more markets than the document carries is a caption about a chart
 * that is not there. And the block that says what the run COULD NOT DO is not
 * decoration: a census-only run drawn without it reads as a trading record.
 */
describe('a population of markets, v4', () => {
  const point = (cycle: number, supply: string[], slot: string) => ({
    cycle,
    stage: `simlife-m00-tick-${String(cycle).padStart(4, '0')}`,
    slot,
    recorded_at: null,
    supply,
    hoard_atoms: '500',
    tracked_collateral: '1000',
    mint_supply: '1000',
    position_totals: supply,
    law_statuses: 'hh',
    checks_held: 2,
    checks_broken: 0,
    checks_inapplicable: 0,
  });

  const marketBody = (id: string, archetype: string, supplies: string[][]) => ({
    market_id: id,
    archetype,
    basis: 'categorical-degree-0',
    destiny: 'resolves-clean',
    deadline_slots: 4096,
    personas: ['eager-maker', 'sleeper'],
    law_ids: ['L1', 'L4'],
    laws: [
      { id: 'L1', status: 'holds', detail: 'tracked == mint supply' },
      { id: 'L4', status: 'holds', detail: 'hoard covers the worst outcome' },
    ],
    positions: [],
    collateral_holders: [],
    claim_unit_atoms: '1',
    settlement: null,
    outcome_count: supplies[0].length,
    cycles_recorded: supplies.length,
    points_omitted_before: 0,
    census_file: 'cycle-000004.json',
    points: supplies.map((supply, index) => point(index, supply, String(900 + index * 7))),
  });

  const populated = () => {
    const first = marketBody('m00', 'coin-flip', [['500', '500'], ['500', '500'], ['500', '500']]);
    const second = marketBody('m03', 'wide-field', [['100', '100', '100'], ['100', '100', '100']]);
    return {
      ...first,
      schema: SIMULATOR_SERIES_SCHEMA_V4,
      captured_at: '2026-08-30T20:00:00+00:00',
      cluster: 'local',
      market: null,
      mode: 'finite',
      campaign: null,
      world: {
        seed: { preimage: 'dclutch/simlife/2026-08-30/first-light', sha256: 'a'.repeat(64) },
        plan_digest: 'b'.repeat(64),
        substrate: {
          name: 'ledger-census',
          label: 'a restarted loopback chain',
          cluster: 'local',
          rpc_origin: 'http://127.0.0.1:34500',
          source_revision: '533540056d61c05faabaae07e9b78e8c90214a8e',
          routes: ['census'],
          routes_absent: ['found', 'admit', 'fill'],
          basis_kinds: ['categorical-degree-0'],
          basis_kinds_absent: ['ramp-degree-1', 'tent-degree-1'],
        },
        markets_planned: 3,
        markets_observed: 2,
        markets_founded_by_this_run: [],
        markets_pre_founded: ['m00', 'm03'],
        planned: [
          {
            market_id: 'm00', archetype: 'coin-flip', basis: 'categorical-degree-0',
            destiny: 'resolves-clean', outcome_count: 2, deadline_slots: 4096,
            fee_basis_points: 0, founding_collateral_atoms: '1000',
            participants: [{ persona: 'eager-maker' }, { persona: 'sleeper' }], observed: true,
          },
          {
            market_id: 'm03', archetype: 'wide-field', basis: 'categorical-degree-0',
            destiny: 'resolves-clean', outcome_count: 3, deadline_slots: 40960,
            fee_basis_points: 0, founding_collateral_atoms: '3000',
            participants: [{ persona: 'crank' }], observed: true,
          },
          {
            market_id: 'm07', archetype: 'ladder', basis: 'ramp-degree-1',
            destiny: 'founded-then-sleepy', outcome_count: 6, deadline_slots: 90000,
            fee_basis_points: 0, founding_collateral_atoms: '9000',
            participants: [{ persona: 'patient-maker' }], observed: false,
          },
        ],
        not_done: [
          { route: 'found', outcome: 'unattempted', reason: 'this run founded nothing', count: 3 },
          { route: 'fill', outcome: 'blocked', reason: 'm07 was never founded', count: 11 },
        ],
      },
      markets: [first, second],
    };
  };

  it('decodes the population and keeps the primary market at the top level', () => {
    const series = parseSimulatorSeriesV1(populated());
    expect(series.schema).toBe(SIMULATOR_SERIES_SCHEMA_V4);
    // Every v3 reader still sees one market, with points, and never has to know
    // this version exists.
    expect(series.outcomeCount).toBe(2);
    expect(series.points).toHaveLength(3);
    expect(issuedSupplyLinesV1(series)).toHaveLength(2);
    expect(series.markets.map((market) => market.marketId)).toEqual(['m00', 'm03']);
    expect(series.markets[1].outcomeCount).toBe(3);
    expect(series.markets[1].archetype).toBe('wide-field');
  });

  it('carries the seed and the substrate a reader would need to re-run it', () => {
    const world = parseSimulatorSeriesV1(populated()).world;
    expect(world?.seedPreimage).toBe('dclutch/simlife/2026-08-30/first-light');
    expect(world?.substrate.routes).toEqual(['census']);
    expect(world?.substrate.routesAbsent).toContain('found');
    expect(world?.substrate.basisKindsAbsent).toContain('ramp-degree-1');
    expect(world?.marketsFoundedByThisRun).toEqual([]);
  });

  it('keeps a planned-but-unobserved market out of the drawn markets entirely', () => {
    const world = parseSimulatorSeriesV1(populated()).world;
    const sleepy = world?.planned.find((market) => market.marketId === 'm07');
    expect(sleepy?.observed).toBe(false);
    expect(world?.marketsPlanned).toBe(3);
    expect(world?.marketsObserved).toBe(2);
  });

  it('refuses a world that says a market was observed when no series carries it', () => {
    const body = populated();
    body.world.planned[2].observed = true;
    expect(() => parseSimulatorSeriesV1(body)).toThrow(/m07 was observed and no series carries it/);
  });

  it('refuses a world whose count disagrees with its own charts', () => {
    const body = populated();
    body.world.markets_observed = 5;
    expect(() => parseSimulatorSeriesV1(body)).toThrow(/claims 5 observed markets and carries 2/);
  });

  it('refuses two markets sharing one id, which would draw one over the other', () => {
    const body = populated();
    body.markets = [body.markets[0], body.markets[0]];
    body.world.markets_observed = 2;
    expect(() => parseSimulatorSeriesV1(body)).toThrow(/same id/);
  });

  it('holds a nested market to the same length checks as a market on its own', () => {
    const body = populated();
    body.markets[1].points[0].supply = ['1', '2'];
    expect(() => parseSimulatorSeriesV1(body)).toThrow(/carries 2 outcomes and the series declares 3/);
  });

  it('admits the one rate a market can be filled at, and refuses one outside the domain', () => {
    // This test used to assert the opposite -- that ANY nonzero rate is
    // refused -- on the reading that fee-bearing founding does not fit in one
    // transaction. That reading came from a document about the Direct fill's
    // fee leg and said nothing about founding, and the owned-loopback producer
    // admits exactly 50 bps: so the guard refused every capture of a world
    // whose markets could trade, and zero was the one rate that could not.
    // Renegotiated in the open rather than deleted: what survives is the
    // protocol's own domain, which is where a real impossibility lives.
    const tradeable = populated();
    tradeable.world.planned[0].fee_basis_points = 50;
    expect(parseSimulatorSeriesV1(tradeable).world?.planned[0].feeBasisPoints).toBe(50);
    const impossible = populated();
    impossible.world.planned[0].fee_basis_points = 10_001;
    expect(() => parseSimulatorSeriesV1(impossible)).toThrow(/outside the 0\.\.10000/);
  });

  it('carries what the run could not do, route by route, with its own reason', () => {
    const world = parseSimulatorSeriesV1(populated()).world;
    expect(world?.notDone).toHaveLength(2);
    expect(world?.notDone[0].outcome).toBe('unattempted');
    expect(world?.notDone[1].outcome).toBe('blocked');
    expect(world?.notDone[1].count).toBe(11);
  });

  it('leaves every earlier capture decoding as one that recorded no population', () => {
    const older = parseSimulatorSeriesV1({
      schema: 'dclutch-simulator-series-v1',
      captured_at: '2026-08-30T16:40:07+00:00',
      cluster: 'devnet', market: null, mode: 'sustain',
      outcome_count: 1, cycles_recorded: 1, points_omitted_before: 0,
      census_file: 'cycle-000001.json',
      points: [{ cycle: 1, slot: '10', recorded_at: null, supply: ['5'], hoard_atoms: '5', tracked_collateral: '5', checks_held: 6, checks_broken: 0, checks_inapplicable: 1 }],
    });
    expect(older.world).toBeNull();
    expect(older.markets).toEqual([]);
  });
});

/**
 * The readings a page needs before it draws a population, and the sentences it
 * must not be able to leave out.
 */
describe('a population that MUTATED, drawn', () => {
  const point = (cycle: number, supply: string[], slot: string) => ({
    cycle,
    stage: `simlife-m00-tick-${String(cycle).padStart(4, '0')}`,
    slot,
    recorded_at: null,
    supply,
    hoard_atoms: '500',
    tracked_collateral: '1000',
    mint_supply: '1000',
    position_totals: supply,
    law_statuses: 'hh',
    checks_held: 2,
    checks_broken: 0,
    checks_inapplicable: 0,
  });
  const market = (id: string, supplies: string[][]) => ({
    market_id: id,
    archetype: 'coin-flip',
    basis: 'categorical-degree-0',
    destiny: 'resolves-clean',
    deadline_slots: 4096,
    personas: ['eager-maker'],
    law_ids: ['L1', 'L4'],
    laws: [
      { id: 'L1', status: 'holds', detail: 'tracked == mint supply' },
      { id: 'L4', status: 'holds', detail: 'hoard covers the worst outcome' },
    ],
    positions: [],
    collateral_holders: [],
    claim_unit_atoms: '1',
    settlement: null,
    outcome_count: supplies[0].length,
    cycles_recorded: supplies.length,
    points_omitted_before: 0,
    census_file: 'cycle-000004.json',
    points: supplies.map((supply, index) => point(index, supply, String(900 + index * 7))),
  });
  const driven = (overrides: Record<string, unknown> = {}) => {
    const first = market('m00', [['500', '500'], ['400', '600']]);
    return {
      ...first,
      schema: SIMULATOR_SERIES_SCHEMA_V4,
      captured_at: '2026-08-30T22:00:00+00:00',
      cluster: 'local',
      market: null,
      mode: 'finite',
      campaign: null,
      world: {
        seed: { preimage: 'dclutch/simlife2/2026-08-30/hands', sha256: 'c'.repeat(64) },
        plan_digest: 'd'.repeat(64),
        substrate: {
          name: 'successor-bootstrap-lifecycle',
          label: 'a loopback chain this run founded its own markets on',
          cluster: 'local',
          rpc_origin: 'http://127.0.0.1:34500',
          source_revision: '533540056d61c05faabaae07e9b78e8c90214a8e',
          routes: ['found', 'admit', 'census'],
          routes_absent: ['compact'],
          basis_kinds: ['categorical-degree-0'],
          basis_kinds_absent: ['ramp-degree-1', 'tent-degree-1'],
        },
        markets_planned: 2,
        markets_observed: 1,
        markets_founded_by_this_run: ['m00'],
        markets_pre_founded: [],
        tally: {
          found: { executed: 1, refused: 0, unattempted: 1, blocked: 0 },
          admit: { executed: 2, refused: 1, unattempted: 0, blocked: 3 },
          census: { executed: 2, refused: 0, unattempted: 0, blocked: 4 },
          compact: { executed: 0, refused: 0, unattempted: 2, blocked: 0 },
        },
        planned: [
          {
            market_id: 'm00', archetype: 'coin-flip', basis: 'categorical-degree-0',
            destiny: 'resolves-clean', outcome_count: 2, deadline_slots: 4096,
            fee_basis_points: 0, founding_collateral_atoms: '1000',
            participants: [{ persona: 'eager-maker' }], observed: true,
          },
          {
            market_id: 'm01', archetype: 'ladder', basis: 'ramp-degree-1',
            destiny: 'resolves-clean', outcome_count: 6, deadline_slots: 90000,
            fee_basis_points: 0, founding_collateral_atoms: '9000',
            participants: [{ persona: 'sleeper' }], observed: false,
          },
        ],
        not_done: [
          { route: 'found', outcome: 'unattempted', reason: 'm01 asks for a ramp basis', count: 1 },
          { route: 'admit', outcome: 'refused', reason: 'the chain refused one admission', count: 1 },
          { route: 'admit', outcome: 'blocked', reason: 'm01 was never founded', count: 3 },
          { route: 'compact', outcome: 'unattempted', reason: 'no compaction CLI exists', count: 2 },
          { route: 'census', outcome: 'blocked', reason: 'm01 was never founded', count: 4 },
        ],
        timeline: [
          {
            tick: 0, executed: 1, refused: 0, unattempted: 1, blocked: 0,
            mutations_executed: 1, mutations_refused: 0, census_executed: 0, routes: ['found:executed'],
          },
          {
            tick: 1, executed: 3, refused: 1, unattempted: 1, blocked: 4,
            mutations_executed: 2, mutations_refused: 1, census_executed: 1, routes: ['admit:executed', 'admit:refused'],
          },
          {
            tick: 2, executed: 1, refused: 0, unattempted: 1, blocked: 3,
            mutations_executed: 0, mutations_refused: 0, census_executed: 1, routes: [],
          },
        ],
        ...overrides,
      },
      markets: [first],
    };
  };

  it('draws one odds path per observed market from that market own points', () => {
    const series = parseSimulatorSeriesV1(driven());
    const lines = marketOddsLinesV1(series.markets[0]);
    expect(lines.map((line) => line.label)).toEqual(['claim 0', 'claim 1']);
    // 400/1000 and 600/1000 in basis points, floored and exact.
    expect(lines[0].values).toEqual(['5000', '4000']);
    expect(lines[1].values).toEqual(['5000', '6000']);
    expect(marketSlotLabelsV1(series.markets[0])).toEqual(['slot 900', 'slot 907']);
  });

  it('splits the timeline into landed, refused and censused rather than one events line', () => {
    const series = parseSimulatorSeriesV1(driven());
    const lines = eventTimelineLinesV1(series);
    expect(lines.map((line) => line.label)).toEqual([
      'mutations that landed', 'mutations the chain refused', 'markets censused',
    ]);
    expect(lines[0].values).toEqual(['1', '2', '0']);
    expect(lines[1].values).toEqual(['0', '1', '0']);
    expect(lines[2].values).toEqual(['0', '1', '1']);
    expect(eventTimelineLabelsV1(series)).toEqual(['tick 0', 'tick 1', 'tick 2']);
  });

  it('refuses a timeline tick whose split disagrees with its own total', () => {
    // The caption-disagrees-with-its-chart species, one level down: a tick that
    // claims four executed events and accounts for two of them.
    expect(() => parseSimulatorSeriesV1(driven({
      timeline: [{
        tick: 0, executed: 4, refused: 0, unattempted: 0, blocked: 0,
        mutations_executed: 1, mutations_refused: 0, census_executed: 1, routes: [],
      }],
    }))).toThrow(/says 4 executed but splits into 1 mutations and 1 censuses/);
  });

  it('keeps a capture with no timeline decodable, because absence is not a defect', () => {
    const body = driven();
    delete (body.world as Record<string, unknown>).timeline;
    const series = parseSimulatorSeriesV1(body);
    expect(series.world?.timeline).toEqual([]);
    expect(eventTimelineLinesV1(series)).toEqual([]);
  });

  it('never adds the three not-done words together in the honesty strip', () => {
    const rows = honestyRowsV1(parseSimulatorSeriesV1(driven()));
    const admit = rows.find((row) => row.route === 'admit');
    expect(admit).toEqual({
      route: 'admit',
      executed: 2,
      refused: 1,
      unattempted: 0,
      blocked: 3,
      planned: 6,
      // The SHORT sentence, not the substrate's own note. The note is an
      // engineering register entry and stays in the capture; a page says what
      // happened.
      leadingReason: 'The market was never founded.',
      leadingOutcome: 'blocked',
    });
    // Compaction has no driver anywhere and that is `unattempted`, never a
    // refusal: a route nobody wrote is not a chain saying no.
    const compact = rows.find((row) => row.route === 'compact');
    expect(compact?.unattempted).toBe(2);
    expect(compact?.refused).toBe(0);
  });

  it('leads its reading with the mutations rather than the total', () => {
    const series = parseSimulatorSeriesV1(driven());
    expect(executedReadingV1(series)).toContain('3 mutations landed on the chain');
    expect(executedReadingV1(series)).toContain('1 found');
    expect(executedReadingV1(series)).toContain('2 admit');
    expect(executedReadingV1(series)).toContain('2 censuses');
  });

  it('says plainly when a run mutated nothing at all', () => {
    const body = driven();
    (body.world as Record<string, unknown>).tally = {
      census: { executed: 9, refused: 0, unattempted: 0, blocked: 0 },
    };
    expect(executedReadingV1(parseSimulatorSeriesV1(body)))
      .toBe('Nothing was mutated: this run took 9 censuses and signed nothing else.');
  });
});

describe('reading a population out loud', () => {
  const censusOnly = () => ({
    schema: SIMULATOR_SERIES_SCHEMA_V4,
    captured_at: '2026-08-30T20:00:00+00:00',
    cluster: 'local', market: null, mode: 'finite', campaign: null,
    market_id: 'm06', archetype: 'coin-flip', basis: 'categorical-degree-0',
    destiny: 'resolves-clean', deadline_slots: 2638, personas: ['sleeper'],
    law_ids: ['L1'], laws: [{ id: 'L1', status: 'holds', detail: 'tracked == mint supply' }],
    positions: [{ label: 'founder', address: null, lamports: null, claims: ['5', '5'], total_claims: '10' }],
    collateral_holders: [], claim_unit_atoms: '1', settlement: null,
    outcome_count: 2, cycles_recorded: 2, points_omitted_before: 0,
    census_file: 'cycle-000002.json',
    points: [
      { cycle: 0, stage: 'simlife-m06-tick-0000', slot: '100', recorded_at: null, supply: ['5', '5'], position_totals: ['5', '5'], hoard_atoms: '5', tracked_collateral: '10', mint_supply: '10', law_statuses: 'h', checks_held: 1, checks_broken: 0, checks_inapplicable: 0 },
      { cycle: 1, stage: 'simlife-m06-tick-0001', slot: '276', recorded_at: null, supply: ['5', '5'], position_totals: ['5', '5'], hoard_atoms: '5', tracked_collateral: '10', mint_supply: '10', law_statuses: 'h', checks_held: 1, checks_broken: 0, checks_inapplicable: 0 },
    ],
    world: {
      seed: { preimage: 'dclutch/simlife/2026-08-30/first-light', sha256: 'c'.repeat(64) },
      plan_digest: 'd'.repeat(64),
      substrate: {
        name: 'ledger-census', label: 'a loopback rehearsal chain', cluster: 'local',
        rpc_origin: 'http://127.0.0.1:34500', source_revision: 'e'.repeat(40),
        routes: ['census'], routes_absent: ['found', 'fill'],
        basis_kinds: [], basis_kinds_absent: ['categorical-degree-0', 'ramp-degree-1', 'tent-degree-1'],
      },
      markets_planned: 3, markets_observed: 1,
      markets_founded_by_this_run: [], markets_pre_founded: ['m06'],
      planned: [
        { market_id: 'm06', archetype: 'coin-flip', basis: 'categorical-degree-0', destiny: 'resolves-clean', outcome_count: 2, deadline_slots: 2638, fee_basis_points: 0, founding_collateral_atoms: '10', participants: [{ persona: 'sleeper' }], observed: true },
        { market_id: 'm03', archetype: 'short-fuse', basis: 'categorical-degree-0', destiny: 'commit-deadline-failure', outcome_count: 3, deadline_slots: 215, fee_basis_points: 0, founding_collateral_atoms: '30', participants: [{ persona: 'crank' }], observed: false },
        { market_id: 'm00', archetype: 'ladder', basis: 'ramp-degree-1', destiny: 'founded-then-sleepy', outcome_count: 7, deadline_slots: 11065, fee_basis_points: 0, founding_collateral_atoms: '70', participants: [{ persona: 'patient-maker' }], observed: false },
      ],
      not_done: [
        { route: 'found', outcome: 'unattempted', reason: 'this run founded nothing', count: 3 },
        { route: 'fill', outcome: 'blocked', reason: 'm03 was never founded', count: 9 },
        { route: 'found', outcome: 'refused', reason: 'custom program error 0x5182', count: 1 },
      ],
    },
    markets: [] as unknown[],
  });

  const withMarkets = () => {
    const body = censusOnly();
    const { world, markets, schema, captured_at, cluster, market, mode, campaign, ...primary } = body;
    body.markets = [primary];
    return body;
  };

  const withSpread = (spread: unknown, spend?: unknown) => {
    const body = withMarkets();
    (body.world as Record<string, unknown>).outcome_spread = spread;
    if (spend !== undefined) {
      ((body.world as Record<string, unknown>).substrate as Record<string, unknown>).spend = spend;
    }
    return body;
  };

  const HEALTHY_SPREAD = {
    resolving_markets: 9, distinct_cells: 6,
    counts: { '0/3': 3, '1/3': 2, '2/5': 2, '4/7': 1, '0/7': 1 },
    positioned_markets: 9,
    position_counts: { '0': 4, '5': 2, '6': 1, '10': 2 },
    distinct_positions: 4, heaviest_position_tenths: 0, heaviest_share_percent: 44,
    degenerate_threshold_percent: 70, degenerate: false, coordinate_anchor: '100000000',
  };

  it('carries where the answers landed, normalised to the market they landed in', () => {
    const world = parseSimulatorSeriesV1(withSpread(HEALTHY_SPREAD)).world;
    expect(world?.outcomeSpread?.distinctPositions).toBe(4);
    expect(world?.outcomeSpread?.coordinateAnchor).toBe('100000000');
    expect(world?.outcomeSpread?.degenerate).toBe(false);
    expect(world?.outcomeSpread?.positionCounts['10']).toBe(2);
  });

  it('is absent rather than empty on a capture taken before it existed', () => {
    // Every capture written before the histogram is still a complete v4, and
    // null is a different statement from "nothing settled".
    expect(parseSimulatorSeriesV1(withMarkets()).world?.outcomeSpread).toBeNull();
    expect(parseSimulatorSeriesV1(withMarkets()).world?.substrate.spend).toBeNull();
  });

  it('refuses a histogram whose bars do not add up to its own total', () => {
    expect(() => parseSimulatorSeriesV1(withSpread({
      ...HEALTHY_SPREAD, positioned_markets: 12,
    }))).toThrow(/positions sum to 9 under a total of 12/);
  });

  it('carries a spend record and refuses one that claims a bound it has not got', () => {
    const bounded = parseSimulatorSeriesV1(withSpread(HEALTHY_SPREAD, {
      max_lamports_spent: '5000', spent_lamports: '1200', credited_lamports: '0',
      observations: 40, bounded: true,
    }));
    expect(bounded.world?.substrate.spend?.maxLamportsSpent).toBe('5000');
    expect(bounded.world?.substrate.spend?.bounded).toBe(true);
    const unbounded = parseSimulatorSeriesV1(withSpread(HEALTHY_SPREAD, {
      max_lamports_spent: null, spent_lamports: '1200', credited_lamports: '3',
      observations: 40, bounded: false,
    }));
    expect(unbounded.world?.substrate.spend?.bounded).toBe(false);
    expect(() => parseSimulatorSeriesV1(withSpread(HEALTHY_SPREAD, {
      max_lamports_spent: null, spent_lamports: '1', credited_lamports: '0',
      observations: 1, bounded: true,
    }))).toThrow(/bounded=true with max_lamports_spent=null/);
  });

  it('leads its reading with the seed and ends with what the run founded', () => {
    const reading = populationReadingV1(parseSimulatorSeriesV1(withMarkets()));
    expect(reading).toContain('dclutch/simlife/2026-08-30/first-light');
    expect(reading).toContain('a loopback rehearsal chain');
    // The fact a reader would otherwise assume from seeing markets on a page.
    expect(reading).toContain('founded no market of its own');
  });

  it('keeps refused, unattempted and blocked apart in the sentence about them', () => {
    const reading = notDoneReadingV1(parseSimulatorSeriesV1(withMarkets()));
    expect(reading).toMatch(/^1 planned step was refused by the chain/);
    expect(reading).toContain('3 were never attempted');
    expect(reading).toContain('9 were blocked');
    expect(reading).toContain('three different things');
  });

  it('says a market did not move rather than drawing a flat line without comment', () => {
    const rows = marketRowsV1(parseSimulatorSeriesV1(withMarkets()));
    expect(rows).toHaveLength(1);
    expect(rows[0].marketId).toBe('m06');
    expect(rows[0].archetype).toBe('coin-flip');
    expect(rows[0].moved).toEqual([]);
    expect(rows[0].slotsCovered).toBe('176');
    expect(rows[0].checksHeld).toBe(2);
    expect(rows[0].checksBroken).toBe(0);
    expect(rows[0].positionCount).toBe(1);
  });

  it('counts the archetypes the WORLD drew, including the ones nothing observed', () => {
    const census = archetypeCensusV1(parseSimulatorSeriesV1(withMarkets()));
    expect(census.map((row) => row.archetype).sort()).toEqual(['coin-flip', 'ladder', 'short-fuse']);
    const ladder = census.find((row) => row.archetype === 'ladder');
    expect(ladder?.planned).toBe(1);
    // The whole point: a shape this world contains that no substrate could drive.
    expect(ladder?.observed).toBe(0);
    expect(ladder?.basis).toBe('ramp-degree-1');
  });

  it('says nothing at all about a capture that is not a population', () => {
    const single = parseSimulatorSeriesV1({
      schema: 'dclutch-simulator-series-v1',
      captured_at: '2026-08-30T16:40:07+00:00',
      cluster: 'devnet', market: null, mode: 'sustain',
      outcome_count: 1, cycles_recorded: 1, points_omitted_before: 0,
      census_file: 'cycle-000001.json',
      points: [{ cycle: 1, slot: '10', recorded_at: null, supply: ['5'], hoard_atoms: '5', tracked_collateral: '5', checks_held: 6, checks_broken: 0, checks_inapplicable: 1 }],
    });
    expect(populationReadingV1(single)).toBeNull();
    expect(notDoneReadingV1(single)).toBeNull();
    expect(marketRowsV1(single)).toEqual([]);
    expect(archetypeCensusV1(single)).toEqual([]);
  });
});

/**
 * The committed simlife capture, decoded exactly as a reader gets it.
 *
 * Same reason the single-market capture is pinned: the artifact is captured by
 * hand (`scripts/simlife-series.mjs`) and committed, because `git archive` is
 * the only path to the live host. A capture that produced a document this
 * decoder refuses would otherwise ship as a silent "absent" and nobody would
 * learn why the page went dark.
 */
describe('the committed simlife capture', () => {
  const series = parseSimulatorSeriesV1(simlife);

  it('decodes as a population, from a named seed, against a named substrate', () => {
    expect(series.schema).toBe(SIMULATOR_SERIES_SCHEMA_V4);
    expect(series.cluster).toBe('local');
    expect(series.world).not.toBeNull();
    expect(series.world?.seedPreimage.length).toBeGreaterThan(0);
    expect(series.world?.substrate.sourceRevision).toMatch(/^[0-9a-f]{40}$/);
    expect(series.markets.length).toBeGreaterThan(0);
    expect(series.points.length).toBeGreaterThan(1);
  });

  /**
   * The claim this capture must never be able to make silently: what the run
   * FOUNDED, and what it MUTATED, said in the ledger's own counts rather than
   * in a caption somebody could delete.
   *
   * This was written when the only substrate was census-only and it pinned that
   * one shape — `marketsFoundedByThisRun` empty, `routes` exactly `['census']`.
   * A substrate that founds its own markets now exists, and pinning the older
   * shape would have made the stronger capture the failing one. So the
   * invariant is stated as the IMPLICATION it always was: whichever way this
   * capture answers, the two halves of the artifact must agree, and the
   * reading a person sees must say the same thing as the counts.
   */
  it('agrees with itself about what it founded and what it mutated', () => {
    const world = series.world;
    expect(world).not.toBeNull();
    const founded = world?.marketsFoundedByThisRun ?? [];
    const foundingRoute = (world?.substrate.routes ?? []).includes('found');
    const reading = populationReadingV1(series);
    if (founded.length === 0) {
      expect(reading).toContain('founded no market of its own');
    } else {
      // A run cannot have founded a market through a route its own substrate
      // says it does not have.
      expect(foundingRoute).toBe(true);
      expect(reading).toContain(`founded ${founded.length} of them itself`);
      // And every market it says it founded must be one it also observed.
      const observed = new Set(series.markets.map((market) => market.marketId));
      for (const marketId of founded) expect(observed.has(marketId)).toBe(true);
    }
    // A capture with no route absent anywhere would be claiming a substrate
    // that can do everything, and none exists: compaction has no driver.
    expect(world?.substrate.routesAbsent.length ?? 0).toBeGreaterThan(0);
    expect(notDoneReadingV1(series)).not.toBeNull();
  });

  /**
   * THE EMITTER AND THIS DECODER MUST AGREE, and the unit fixtures above cannot
   * prove that: every one of them is hand-written to match the decoder, so a
   * capture the emitter actually writes can be refused by the very app that
   * publishes it and nothing turns red.
   *
   * That is not hypothetical. Running the emitter and feeding its output back
   * on 2026-08-31 found four disagreements at once -- the spend block written
   * beside the substrate instead of on it, histogram counts rendered as decimal
   * strings where this decoder wants numbers, lamport quantities rendered as
   * numbers where it wants strings, and a fee guard here that refused any
   * nonzero rate. The committed capture is the one artifact both halves touch,
   * so the assertions that would have caught them live on it.
   */
  it('carries the blocks the emitter writes, in the shapes this decoder reads', () => {
    const world = series.world;
    expect(world).not.toBeNull();
    const spread = world?.outcomeSpread ?? null;
    if (spread !== null) {
      // Not degenerate, and if it ever is the capture must not be published
      // with a healthy-looking chart drawn over it.
      expect(spread.degenerate).toBe(false);
      expect(spread.positionedMarkets).toBeGreaterThan(0);
      expect(spread.coordinateAnchor).toMatch(/^[0-9]+$/);
    }
    const spend = world?.substrate.spend ?? null;
    if (spend !== null) {
      expect(spend.spentLamports).toMatch(/^[0-9]+$/);
      if (spend.bounded) expect(spend.maxLamportsSpent).toMatch(/^[0-9]+$/);
      else expect(spend.maxLamportsSpent).toBeNull();
    }
  });

  it('never publishes a violated law without a reading that leads on it', () => {
    const broken = series.markets.reduce(
      (sum, market) => sum + market.points.reduce((inner, point) => inner + point.checksBroken, 0),
      0,
    );
    if (broken > 0) expect(conservationReadingV1(series)).toContain('did not hold');
    else expect(broken).toBe(0);
  });

  it('draws only markets it actually observed', () => {
    const observed = new Set(series.markets.map((market) => market.marketId));
    for (const planned of series.world?.planned ?? []) {
      expect(planned.observed).toBe(observed.has(planned.marketId));
    }
    // Every archetype the world drew is still counted, observed or not.
    const census = archetypeCensusV1(series);
    expect(census.reduce((sum, row) => sum + row.planned, 0)).toBe(series.world?.marketsPlanned);
  });

  it('carries a moving chain rather than a still one', () => {
    const rows = marketRowsV1(series);
    expect(rows.length).toBeGreaterThan(0);
    // The slot is the thing a census-only run can honestly show moving.
    for (const row of rows) expect(BigInt(row.slotsCovered ?? '0')).toBeGreaterThan(0n);
  });
});
