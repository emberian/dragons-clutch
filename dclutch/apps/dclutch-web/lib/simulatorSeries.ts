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

/**
 * v2 adds the one thing v1 threw away: the conservation laws' NAMES.
 *
 * v1 reduced each cycle's verdicts to three integers — held, broken,
 * inapplicable — and a count is the least interesting true thing about a law.
 * The census has always recorded which law it was (`L1`..`L7`) and a sentence
 * saying what it checked; v2 carries both across, so a reader can watch a
 * NAMED law hold rather than watch a number stay at six.
 *
 * v1 documents remain readable and are decoded as a series with no laws
 * recorded, which is a true thing to say about a capture taken before this
 * existed — never a decode failure. The three counts stay in both versions:
 * they are what the run itself halts on.
 */
export const SIMULATOR_SERIES_SCHEMA_V2 = 'dclutch-simulator-series-v2';

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
  /**
   * Each named law's verdict at this cycle, index-aligned with the series'
   * `lawIds`. Empty under v1 and under any capture that recorded no verdicts.
   */
  lawStatuses: ReadonlyArray<ConservationLawStatusV1>;
}>;

/**
 * What a conservation law did at one cycle boundary.
 *
 * `inapplicable` is a third state on purpose and is not a soft failure: the
 * first census has no predecessor to compare against, and an externally driven
 * census cannot account for fees it did not pay. A law that does not apply is
 * neither evidence for nor against the ledger, and folding it into either
 * number would make one of them a lie.
 */
export type ConservationLawStatusV1 = 'holds' | 'violated' | 'inapplicable';

/** The wire's one character per status. Compact because it is repeated per cycle. */
const LAW_STATUS_CHARS: Readonly<Record<string, ConservationLawStatusV1>> = Object.freeze({
  h: 'holds',
  v: 'violated',
  i: 'inapplicable',
});

/** One named law at the newest recorded cycle, with the sentence it wrote. */
export type ConservationLawV1 = Readonly<{
  /** The census's own identifier for the law, e.g. `L4`. */
  id: string;
  status: ConservationLawStatusV1;
  /** The census's own sentence about what it checked. Never this site's words. */
  detail: string;
}>;

/**
 * One holder of claims, at the newest recorded cycle.
 *
 * `label` is the OPERATOR'S word from the run's census configuration, not a
 * name this site chose and not anything the chain stores. It is rendered as
 * the label it is, and any gloss on what a particular label means is the
 * surface's editorial, said beside it.
 */
export type SimulatorPositionV1 = Readonly<{
  label: string;
  address: string | null;
  lamports: string | null;
  /** Claim atoms held per outcome, ordered by claim index. */
  claims: ReadonlyArray<string>;
  /** Every claim atom this position holds, across all outcomes. */
  totalClaims: string;
}>;

/** One token account holding the market's collateral, at the newest cycle. */
export type SimulatorCollateralHolderV1 = Readonly<{
  label: string;
  address: string | null;
  atoms: string;
}>;

export type SimulatorSeriesV1 = Readonly<{
  schema: typeof SIMULATOR_SERIES_SCHEMA_V1 | typeof SIMULATOR_SERIES_SCHEMA_V2;
  capturedAt: string;
  /**
   * The conservation laws this capture recorded, in the census's own order.
   * Every point's `lawStatuses` is index-aligned with this. Empty under v1.
   */
  lawIds: ReadonlyArray<string>;
  /** Those same laws at the NEWEST cycle, with the sentence each wrote. */
  laws: ReadonlyArray<ConservationLawV1>;
  /** Claim holders, largest total first. Empty when the capture recorded none. */
  positions: ReadonlyArray<SimulatorPositionV1>;
  /** Collateral token accounts, largest first. Empty when none were recorded. */
  collateralHolders: ReadonlyArray<SimulatorCollateralHolderV1>;
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

/**
 * One cycle's law verdicts, off the compact wire string.
 *
 * A capture that recorded no laws yields none. A capture that recorded a
 * different NUMBER of them than the series declares is a defect and says so:
 * misaligned statuses would attribute one law's verdict to another law's name,
 * which is the exact failure a named band exists to prevent.
 */
function lawStatuses(value: unknown, field: string, lawCount: number): ReadonlyArray<ConservationLawStatusV1> {
  if (value === null || value === undefined) return Object.freeze([]);
  if (typeof value !== 'string') throw new Error(`${field} must be one status string`);
  if (value.length !== lawCount) {
    throw new Error(`${field} carries ${value.length} verdicts and the series declares ${lawCount} laws`);
  }
  return Object.freeze(Array.from(value, (character, cell) => {
    const status = LAW_STATUS_CHARS[character];
    if (status === undefined) throw new Error(`${field} verdict ${cell} is not one of h, v, i`);
    return status;
  }));
}

function point(value: unknown, index: number, outcomeCount: number, lawCount: number): SimulatorSeriesPointV1 {
  const body = object(value, `point ${index}`);
  if (!Array.isArray(body.supply)) throw new Error(`point ${index} supply must be one list`);
  if (body.supply.length !== outcomeCount) {
    throw new Error(`point ${index} carries ${body.supply.length} outcomes and the series declares ${outcomeCount}`);
  }
  return Object.freeze({
    lawStatuses: lawStatuses(body.law_statuses, `point ${index} law_statuses`, lawCount),
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
  const schema = root.schema;
  if (schema !== SIMULATOR_SERIES_SCHEMA_V1 && schema !== SIMULATOR_SERIES_SCHEMA_V2) {
    throw new Error('simulator series has another schema');
  }
  const cluster = root.cluster;
  if (cluster !== 'local' && cluster !== 'devnet') throw new Error('cluster must be local or devnet');
  const mode = root.mode;
  if (mode !== 'finite' && mode !== 'sustain') throw new Error('mode must be finite or sustain');
  if (!Array.isArray(root.points)) throw new Error('points must be one list');
  const outcomeCount = count(root.outcome_count, 'outcome_count');
  // The law names come first: every point's verdict string is checked against
  // this list's length, so a series can never draw seven verdicts under six
  // names.
  const lawIds = !Array.isArray(root.law_ids)
    ? Object.freeze([])
    : Object.freeze(root.law_ids.map((entry, index) => text(entry, `law_ids ${index}`)));
  const laws = !Array.isArray(root.laws) ? Object.freeze([]) : Object.freeze(root.laws.map((entry, index) => {
    const body = object(entry, `law ${index}`);
    const status = body.status;
    if (status !== 'holds' && status !== 'violated' && status !== 'inapplicable') {
      throw new Error(`law ${index} status must be holds, violated or inapplicable`);
    }
    return Object.freeze({
      id: text(body.id, `law ${index} id`),
      status,
      detail: text(body.detail, `law ${index} detail`),
    });
  }));
  if (laws.length > 0 && laws.length !== lawIds.length) {
    throw new Error(`${laws.length} laws are described and ${lawIds.length} are named`);
  }
  for (const [index, law] of laws.entries()) {
    if (law.id !== lawIds[index]) throw new Error(`law ${index} is ${law.id} and law_ids names ${lawIds[index]}`);
  }
  const points = Object.freeze(root.points.map((entry, index) => point(entry, index, outcomeCount, lawIds.length)));
  // A line is only honest if its x-axis is ordered. Two points out of order
  // would draw a shape that never happened, and no decoder below this one
  // would catch it.
  for (let index = 1; index < points.length; index += 1) {
    if (points[index].cycle <= points[index - 1].cycle) {
      throw new Error(`point ${index} does not come after the point before it`);
    }
  }
  // Holders are optional: a capture taken before this app recorded them is a
  // capture with none, which is a true thing to say and not a decode failure.
  const positions = !Array.isArray(root.positions) ? Object.freeze([]) : Object.freeze(root.positions.map((entry, index) => {
    const body = object(entry, `position ${index}`);
    if (!Array.isArray(body.claims)) throw new Error(`position ${index} claims must be one list`);
    return Object.freeze({
      label: text(body.label, `position ${index} label`),
      address: body.address === null || body.address === undefined ? null : address(body.address, `position ${index} address`),
      lamports: body.lamports === null || body.lamports === undefined ? null : atoms(body.lamports, `position ${index} lamports`),
      claims: Object.freeze(body.claims.map((entry_, cell) => atoms(entry_, `position ${index} claim ${cell}`))),
      totalClaims: atoms(body.total_claims, `position ${index} total_claims`),
    });
  }));
  const collateralHolders = !Array.isArray(root.collateral_holders) ? Object.freeze([]) : Object.freeze(root.collateral_holders.map((entry, index) => {
    const body = object(entry, `collateral holder ${index}`);
    return Object.freeze({
      label: text(body.label, `collateral holder ${index} label`),
      address: body.address === null || body.address === undefined ? null : address(body.address, `collateral holder ${index} address`),
      atoms: atoms(body.atoms, `collateral holder ${index} atoms`),
    });
  }));

  return Object.freeze({
    schema,
    capturedAt: instant(root.captured_at, 'captured_at'),
    lawIds,
    laws,
    positions,
    collateralHolders,
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

/** True when a position holds the same number of claims on every outcome. */
export function isCompleteSetV1(position: SimulatorPositionV1): boolean {
  return position.claims.length > 0 && position.claims.every((amount) => amount === position.claims[0]);
}

export type HoldingsReadingV1 = Readonly<{
  positionCount: number;
  /** Whether ordering these positions is a ranking at all. */
  rankable: boolean;
  /** Every recorded position holds one claim of every outcome. */
  allComplete: boolean;
  /** One plain sentence: what this list is, and what it is not. */
  sentence: string;
}>;

/**
 * What may honestly be said about who holds this market's claims.
 *
 * A leaderboard is a ranking, and a ranking of one is not one. This exists so
 * a surface cannot accidentally imply competition where the record shows a
 * single founding position and nobody who has traded — the table is worth
 * showing either way, because who is standing in a market before anything
 * happens is a real thing to know, but it must not be dressed as a contest.
 */
export function holdingsReadingV1(series: SimulatorSeriesV1): HoldingsReadingV1 {
  const positions = series.positions;
  const allComplete = positions.length > 0 && positions.every(isCompleteSetV1);
  if (positions.length === 0) {
    return Object.freeze({
      positionCount: 0,
      rankable: false,
      allComplete: false,
      sentence: 'No position was recorded on this market, so there is nobody to list.',
    });
  }
  if (positions.length === 1) {
    return Object.freeze({
      positionCount: 1,
      rankable: false,
      allComplete,
      sentence: allComplete
        ? 'One position exists, and it holds the same number of claims on every outcome — a complete set, which is worth the same whatever the answer turns out to be. There is nothing here to rank yet.'
        : 'One position exists on this market, so there is nothing here to rank yet.',
    });
  }
  return Object.freeze({
    positionCount: positions.length,
    rankable: true,
    allComplete,
    sentence: `${positions.length} positions, ordered by the total claims each holds. That is a count of claims held, not a score and not a return.`,
  });
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

/**
 * THE HEARTBEAT: the part of this record that actually moves.
 *
 * A census-only run signs nothing, so it spends nothing, so every quantity it
 * observes is expected to hold still — and on the recorded devnet run every
 * one of them does, for hundreds of cycles. Drawing those and only those is
 * how a true record ends up reading as a dead one.
 *
 * Two things are nevertheless moving the whole time, and both are chain facts
 * rather than simulator facts. The chain advanced between one reading and the
 * next, and the run took a measurable amount of wall-clock to come back. That
 * is a heartbeat: not a market's price, but proof that something is on the
 * other end of the line, which is the question a stranger is actually asking.
 *
 * TWO MEASURES, NEVER ONE PAIR OF AXES. Slots and seconds are different
 * dimensions at different magnitudes; they are returned as separate lines for
 * separate figures and are never stacked on a shared scale.
 */
export type SimulatorHeartbeatV1 = Readonly<{
  /** Slots the chain advanced between consecutive recordings, exact. */
  slotAdvance: SimulatorSeriesLineV1;
  /** Whole seconds between consecutive recordings, or null when any is unknown. */
  cadence: SimulatorSeriesLineV1 | null;
  /** One label per interval, index-aligned with the lines. */
  xLabels: ReadonlyArray<string>;
  /** Intervals drawn: one fewer than the points, because an interval needs two. */
  intervals: number;
  /**
   * Slots per second across the whole drawn window, to two places, or null
   * when the run did not record enough instants to divide by. Measured — the
   * exact totals it comes from are on the span beside it, never a chain
   * constant assumed and printed as if observed.
   */
  measuredSlotRate: string | null;
  /** The longest a recording ever went without a successor, in whole seconds. */
  longestGapSeconds: string | null;
  /** The shortest such interval, in whole seconds. */
  shortestGapSeconds: string | null;
}>;

export function simulatorHeartbeatV1(series: SimulatorSeriesV1): SimulatorHeartbeatV1 | null {
  const points = series.points;
  if (points.length < 2) return null;

  const slotAdvance: string[] = [];
  const xLabels: string[] = [];
  const seconds: string[] = [];
  let everyInstantKnown = true;
  for (let index = 1; index < points.length; index += 1) {
    const before = points[index - 1];
    const after = points[index];
    // A slot count is a u64 and stays exact: BigInt in, decimal string out.
    const advance = BigInt(after.slot) - BigInt(before.slot);
    slotAdvance.push((advance < 0n ? -advance : advance).toString());
    xLabels.push(`cycle ${before.cycle} → ${after.cycle}`);
    if (before.recordedAt === null || after.recordedAt === null) {
      everyInstantKnown = false;
      continue;
    }
    const gap = Math.round((Date.parse(after.recordedAt) - Date.parse(before.recordedAt)) / 1000);
    seconds.push(String(Math.max(0, gap)));
  }

  // The cadence line is drawn only when EVERY interval on it was measured. A
  // line with holes silently redrawn as a shorter line would compress the
  // x-axis and put an interval under another interval's label.
  const cadence = everyInstantKnown && seconds.length === slotAdvance.length
    ? Object.freeze({ label: 'seconds between recordings', values: Object.freeze([...seconds]) })
    : null;

  const totalSlots = BigInt(points[points.length - 1].slot) - BigInt(points[0].slot);
  const totalSeconds = cadence === null
    ? 0n
    : seconds.reduce((sum, value) => sum + BigInt(value), 0n);
  const measuredSlotRate = totalSeconds === 0n
    ? null
    // Two places, computed on integers so the rounding is the only float here.
    : (Number((totalSlots * 100n) / totalSeconds) / 100).toFixed(2);

  const extremum = (pick: (left: bigint, right: bigint) => bigint) =>
    (cadence === null ? null : seconds.reduce((best, value) => pick(best, BigInt(value)), BigInt(seconds[0])).toString());

  return Object.freeze({
    slotAdvance: Object.freeze({ label: 'slots the chain advanced', values: Object.freeze(slotAdvance) }),
    cadence,
    xLabels: Object.freeze(xLabels),
    intervals: slotAdvance.length,
    measuredSlotRate,
    longestGapSeconds: extremum((left, right) => (right > left ? right : left)),
    shortestGapSeconds: extremum((left, right) => (right < left ? right : left)),
  });
}

/**
 * One named conservation law, across every drawn cycle.
 *
 * The counts a v1 series carries answer "how many held"; this answers "which
 * one, and what did it check" — and the second question is the one a reader
 * who does not already trust us needs answered. The sentence on each row is
 * the census's own, from the newest cycle, and is never rewritten here.
 */
export type ConservationLawRowV1 = Readonly<{
  id: string;
  /** This law's verdict at each drawn cycle, oldest first. */
  statuses: ReadonlyArray<ConservationLawStatusV1>;
  held: number;
  violated: number;
  inapplicable: number;
  /** The census's sentence at the newest cycle, or null when none was recorded. */
  detail: string | null;
}>;

/**
 * The cycle numbers a law band is drawn against.
 *
 * Not every point necessarily carries a verdict set — a cycle whose census
 * recorded a DIFFERENT set of laws carries none rather than a set laid under
 * the wrong names — so the band's x-axis is these cycles, which can be fewer
 * than the series' points. Every row filters identically, so the rows stay
 * aligned with each other and with this.
 */
export function lawBandCyclesV1(series: SimulatorSeriesV1): ReadonlyArray<number> {
  return Object.freeze(series.points.filter((entry) => entry.lawStatuses.length > 0).map((entry) => entry.cycle));
}

export function conservationLawRowsV1(series: SimulatorSeriesV1): ReadonlyArray<ConservationLawRowV1> {
  return Object.freeze(series.lawIds.map((id, cell) => {
    const statuses = Object.freeze(series.points
      .filter((entry) => entry.lawStatuses.length > cell)
      .map((entry) => entry.lawStatuses[cell]));
    const tally = (wanted: ConservationLawStatusV1) => statuses.filter((status) => status === wanted).length;
    return Object.freeze({
      id,
      statuses,
      held: tally('holds'),
      violated: tally('violated'),
      inapplicable: tally('inapplicable'),
      detail: series.laws[cell]?.detail ?? null,
    });
  }));
}

/**
 * One plain sentence about the laws, or null when the capture recorded none.
 *
 * It leads with the violation when there is one. A run that broke a law halts
 * itself, so a reader arriving at a page that shows one is looking at the
 * single most important fact on it, and it must not be the third clause.
 */
export function conservationReadingV1(series: SimulatorSeriesV1): string | null {
  const rows = conservationLawRowsV1(series);
  if (rows.length === 0) return null;
  const violated = rows.filter((row) => row.violated > 0);
  if (violated.length > 0) {
    return `${violated.map((row) => row.id).join(', ')} did not hold. The run halts on exactly this, and the market's collateral is the thing in question.`;
  }
  const held = rows.reduce((sum, row) => sum + row.held, 0);
  const skipped = rows.reduce((sum, row) => sum + row.inapplicable, 0);
  const drawn = rows[0]?.statuses.length ?? 0;
  // "240 did not apply" beside "240 cycle boundaries" reads as the same 240.
  // The noun is what disambiguates them, so the noun is always said.
  return `${rows.length} laws, re-checked at every one of ${drawn} cycle boundaries: ${held} checks held and none broke.${
    skipped === 0 ? '' : ` ${skipped} checks did not apply at the boundary they were on, which is neither a pass nor a failure.`}`;
}
