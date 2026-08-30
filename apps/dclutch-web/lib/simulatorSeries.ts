import { PublicKey } from '@solana/web3.js';

/**
 * The simulator's record over time, decoded for the surfaces that draw it.
 *
 * Every chart in this app used to be a snapshot: one finalized floor, one set
 * of bars, no time anywhere. That is a fair drawing of what a single read can
 * see, and it is the wrong drawing of a market, whose whole subject is a
 * quantity that changes. This is the other axis.
 *
 * WHERE THE POINTS COME FROM. Not from a poller this app runs. The load
 * simulator already re-runs a full ledger census every cycle and keeps each
 * one; `apps/dclutch-web/scripts/simulator-series.mjs` joins those censuses to
 * the simulator's own cycle journals and writes the result beside the site.
 * Nothing here observes a chain — this file decodes what the robot recorded.
 *
 * WHAT THE READER IS OWED. The same three states the status artifact
 * distinguishes, for the same reason: a static host answers a missing path
 * with its fallback page, so `absent` is normal and must never be dressed up
 * as an empty chart, while a real JSON document that fails this decoder is a
 * defect and says so. And one more thing the status artifact does not need —
 * a captured instant, because a committed snapshot is a RECORD. The simulator
 * kept running after this file was written, and the page says so rather than
 * implying the last point is the present moment.
 */

export const SIMULATOR_SERIES_SCHEMA_V1 = 'dclutch-simulator-series-v1';

/** The one URL the surfaces read. Pinned by test: the artifact's link check
 * cannot see a runtime fetch, so the string itself is the contract. */
export const SIMULATOR_SERIES_URL_V1 = '/simulator-series.json';

/** One plain sentence for the shipped default state. */
export const NO_SERIES_SENTENCE_V1 =
  'No recorded run is published beside this site right now, so there is no line to draw and nothing below is a zero.';

/**
 * What a reader must be told beside every line drawn from this artifact: it is
 * a record that was captured once, not a feed.
 */
export const SERIES_RECORD_CAVEAT_V1 =
  'These points were captured from the run’s own records when this site was last published. The run continues past the last point; this page does not.';

export type SimulatorSeriesPointV1 = Readonly<{
  /** The simulator's own cycle number, ascending. */
  cycle: number;
  /** The finalized slot the census observed, exact. */
  slot: string;
  /** When the simulator recorded the cycle, or null when no journal had it. */
  recordedAt: string | null;
  /** Issued claim atoms per outcome, ordered by claim index, exact. */
  supply: ReadonlyArray<string>;
  /** Collateral the Market's Hoard held at this cycle, exact. */
  hoardAtoms: string;
  /** Every atom the census tracked at this cycle, exact. */
  trackedCollateral: string;
  /** Conservation laws that held, were broken, and did not apply here. */
  checksHeld: number;
  checksBroken: number;
  checksInapplicable: number;
}>;

export type SimulatorSeriesV1 = Readonly<{
  schema: typeof SIMULATOR_SERIES_SCHEMA_V1;
  capturedAt: string;
  cluster: 'local' | 'devnet';
  /** The Market every point describes, or null when the run named none. */
  market: string | null;
  mode: 'finite' | 'sustain';
  outcomeCount: number;
  /** Cycles the run had recorded when this was captured. */
  cyclesRecorded: number;
  /** Cycles older than the kept window: counted, never silently dropped. */
  pointsOmittedBefore: number;
  /** The census file the points were read out of. */
  censusFile: string;
  points: ReadonlyArray<SimulatorSeriesPointV1>;
}>;

function object(value: unknown, field: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${field} must be one object`);
  return value as Record<string, unknown>;
}

function text(value: unknown, field: string): string {
  if (typeof value !== 'string' || value.length === 0) throw new Error(`${field} must be one non-empty string`);
  return value;
}

function count(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new Error(`${field} must be one exact non-negative integer`);
  return value;
}

function instant(value: unknown, field: string): string {
  const raw = text(value, field);
  if (Number.isNaN(Date.parse(raw))) throw new Error(`${field} must be one parseable timestamp`);
  return raw;
}

/** A quantity that must survive as written: exact decimal, never a float. */
function atoms(value: unknown, field: string): string {
  const raw = text(value, field);
  if (!/^(0|[1-9][0-9]*)$/.test(raw)) throw new Error(`${field} must be one exact decimal quantity`);
  return raw;
}

function address(value: unknown, field: string): string {
  const raw = text(value, field);
  let parsed: PublicKey;
  try { parsed = new PublicKey(raw); } catch { throw new Error(`${field} must be one canonical Solana address`); }
  if (parsed.toBase58() !== raw) throw new Error(`${field} must be one canonical Solana address`);
  return raw;
}

function point(value: unknown, index: number, outcomeCount: number): SimulatorSeriesPointV1 {
  const body = object(value, `point ${index}`);
  if (!Array.isArray(body.supply)) throw new Error(`point ${index} supply must be one list`);
  if (body.supply.length !== outcomeCount) {
    throw new Error(`point ${index} carries ${body.supply.length} outcomes and the series declares ${outcomeCount}`);
  }
  return Object.freeze({
    cycle: count(body.cycle, `point ${index} cycle`),
    slot: atoms(body.slot, `point ${index} slot`),
    recordedAt: body.recorded_at === null || body.recorded_at === undefined
      ? null
      : instant(body.recorded_at, `point ${index} recorded_at`),
    supply: Object.freeze(body.supply.map((entry, cell) => atoms(entry, `point ${index} supply ${cell}`))),
    hoardAtoms: atoms(body.hoard_atoms, `point ${index} hoard_atoms`),
    trackedCollateral: atoms(body.tracked_collateral, `point ${index} tracked_collateral`),
    checksHeld: count(body.checks_held, `point ${index} checks_held`),
    checksBroken: count(body.checks_broken, `point ${index} checks_broken`),
    checksInapplicable: count(body.checks_inapplicable, `point ${index} checks_inapplicable`),
  });
}

/** Decode one series document. Throws with the field named; never returns a
 * half-series, and never a series whose cycles run backwards. */
export function parseSimulatorSeriesV1(value: unknown): SimulatorSeriesV1 {
  const root = object(value, 'simulator series');
  if (root.schema !== SIMULATOR_SERIES_SCHEMA_V1) throw new Error('simulator series has another schema');
  const cluster = root.cluster;
  if (cluster !== 'local' && cluster !== 'devnet') throw new Error('cluster must be local or devnet');
  const mode = root.mode;
  if (mode !== 'finite' && mode !== 'sustain') throw new Error('mode must be finite or sustain');
  if (!Array.isArray(root.points)) throw new Error('points must be one list');
  const outcomeCount = count(root.outcome_count, 'outcome_count');
  const points = Object.freeze(root.points.map((entry, index) => point(entry, index, outcomeCount)));
  // A line is only honest if its x-axis is ordered. Two points out of order
  // would draw a shape that never happened, and no decoder below this one
  // would catch it.
  for (let index = 1; index < points.length; index += 1) {
    if (points[index].cycle <= points[index - 1].cycle) {
      throw new Error(`point ${index} does not come after the point before it`);
    }
  }
  return Object.freeze({
    schema: SIMULATOR_SERIES_SCHEMA_V1,
    capturedAt: instant(root.captured_at, 'captured_at'),
    cluster,
    market: root.market === null || root.market === undefined ? null : address(root.market, 'market'),
    mode,
    outcomeCount,
    cyclesRecorded: count(root.cycles_recorded, 'cycles_recorded'),
    pointsOmittedBefore: count(root.points_omitted_before, 'points_omitted_before'),
    censusFile: text(root.census_file, 'census_file'),
    points,
  });
}

export type SimulatorSeriesReadV1 =
  | Readonly<{ kind: 'absent' }>
  | Readonly<{ kind: 'loaded'; series: SimulatorSeriesV1 }>
  | Readonly<{ kind: 'refused'; reason: string }>;

/**
 * Read the published series, guarded for a static host exactly the way the
 * status artifact is: a missing path answers with the host's fallback page —
 * an HTML body, sometimes under a 200 — so a non-OK answer and an unparseable
 * body both read as absent, and only a real JSON document that fails the
 * decoder reads as refused.
 */
export async function readSimulatorSeriesV1(
  fetchLike: (url: string) => Promise<{ ok: boolean; text(): Promise<string> }>,
): Promise<SimulatorSeriesReadV1> {
  let body: string;
  try {
    const response = await fetchLike(SIMULATOR_SERIES_URL_V1);
    if (!response.ok) return Object.freeze({ kind: 'absent' as const });
    body = await response.text();
  } catch {
    return Object.freeze({ kind: 'absent' as const });
  }
  let raw: unknown;
  try { raw = JSON.parse(body); } catch { return Object.freeze({ kind: 'absent' as const }); }
  try {
    return Object.freeze({ kind: 'loaded' as const, series: parseSimulatorSeriesV1(raw) });
  } catch (error) {
    const reason = error instanceof Error ? error.message : 'the series did not decode for a usable reason';
    return Object.freeze({ kind: 'refused' as const, reason });
  }
}

/** One line's worth of the series: a label and its values, oldest first. */
export type SimulatorSeriesLineV1 = Readonly<{ label: string; values: ReadonlyArray<string> }>;

/**
 * The issued-claim lines, one per outcome, in claim-index order.
 *
 * `outcomes` are this site's editorial names for the outcomes and may be
 * absent; the claim index is always what identifies a line, because that is
 * what the chain stores.
 */
export function issuedSupplyLinesV1(
  series: SimulatorSeriesV1,
  outcomes?: ReadonlyArray<string> | null,
): ReadonlyArray<SimulatorSeriesLineV1> {
  return Object.freeze(Array.from({ length: series.outcomeCount }, (_unused, cell) => Object.freeze({
    label: outcomes?.[cell] === undefined ? `claim ${cell}` : `claim ${cell} · ${outcomes[cell]}`,
    values: Object.freeze(series.points.map((entry) => entry.supply[cell])),
  })));
}

/** True when every value on every line is the same value. */
export function everyLineFlatV1(lines: ReadonlyArray<SimulatorSeriesLineV1>): boolean {
  return lines.every((line) => line.values.every((value) => value === line.values[0]));
}

export type SimulatorSeriesSpanV1 = Readonly<{
  /** Cycles drawn, and the chain slots the first and last were read at. */
  cycles: number;
  firstSlot: string;
  lastSlot: string;
  /** Slots the chain advanced across the whole drawn window. */
  slotsCovered: string;
  /** Wall-clock minutes between the first and last recorded cycle, or null. */
  minutesCovered: number | null;
  /** Every conservation check across every drawn cycle. */
  checksHeld: number;
  checksBroken: number;
}>;

/**
 * What the drawn window actually covers, so a caption can say it in numbers
 * rather than in adjectives. Minutes come from the simulator's own recorded
 * instants — never from a slot-rate estimate — and are null when the journals
 * did not supply them.
 */
export function simulatorSeriesSpanV1(series: SimulatorSeriesV1): SimulatorSeriesSpanV1 | null {
  if (series.points.length === 0) return null;
  const first = series.points[0];
  const last = series.points[series.points.length - 1];
  const minutes = first.recordedAt === null || last.recordedAt === null
    ? null
    : Math.max(0, Math.round((Date.parse(last.recordedAt) - Date.parse(first.recordedAt)) / 60_000));
  return Object.freeze({
    cycles: series.points.length,
    firstSlot: first.slot,
    lastSlot: last.slot,
    slotsCovered: (BigInt(last.slot) - BigInt(first.slot)).toString(),
    minutesCovered: minutes,
    checksHeld: series.points.reduce((sum, entry) => sum + entry.checksHeld, 0),
    checksBroken: series.points.reduce((sum, entry) => sum + entry.checksBroken, 0),
  });
}
