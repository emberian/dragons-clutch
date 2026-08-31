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

/**
 * v3 is the same series taken at a CAMPAIGN's stage boundaries rather than at
 * a poller's cycles, and it carries the four things that record has and a
 * census-only record does not.
 *
 * A census watches one market hold still and reports the same quantities every
 * cycle; that is what v1 and v2 were shaped for, and the honest drawing of it
 * is a flat line beside a moving heartbeat. A campaign is the other thing: it
 * founds a market, funds its resolution, drives it to a terminal answer and
 * retires it, and the boundary between two of its stages is a place where
 * quantities are SUPPOSED to move. So v3 adds
 *
 * - `stage`, the boundary's own name, because "cycle 3" is not what happened
 *   there and "resolution funding active" is;
 * - the work each interval cost — transactions, compute units, fee lamports —
 *   which is the only volume a market with no fills actually has;
 * - `claim_unit_atoms`, the collateral one claim of one outcome is worth. It
 *   is the price primitive: without it a claim count is a count of nothing in
 *   particular, and with it every per-cell figure on the page is in collateral;
 * - `settlement`, which cell the terminal certificate selected. That is the
 *   only price move a market without fills ever makes, and it is the whole
 *   one: the selected cell's claims become worth the claim unit and every
 *   other cell's become worth nothing.
 *
 * Every field is optional and every earlier document still decodes. A v1 or v2
 * capture is a capture that recorded no stages, no volume and no settlement,
 * which is a true thing to say about it.
 */
export const SIMULATOR_SERIES_SCHEMA_V3 = 'dclutch-simulator-series-v3';

/**
 * v4 is the same series taken across a POPULATION of markets rather than one.
 *
 * v1 through v3 all describe a single market, because until the simlife engine
 * (`tools/load-simulator/simlife.py`) the simulator only ever watched one. A
 * simlife run draws many markets from seeded archetypes — different widths,
 * different bases, different fuses, some resolving and some left alone — and
 * censuses all of them at the same ticks. That contemporaneity is the only
 * thing the population has that a single market does not: the lines share an
 * x-axis, so they can honestly be drawn beside each other.
 *
 * NOTHING ABOUT THE OLD SHAPE CHANGES. The top level of a v4 document still
 * describes exactly one market — the primary, the longest-observed — so every
 * surface written against v1, v2 or v3 keeps drawing without knowing this
 * version exists. Two blocks are added beside it:
 *
 * - `world`: the seed the population was drawn from, the substrate it was
 *   driven against, every market that was PLANNED whether or not it was
 *   observed, and — route by route — what the substrate could not do. That last
 *   part is the point. A world plans nine kinds of thing and a census-only
 *   substrate can do one of them; a page that draws such a run without saying so
 *   reads as a trading record, which it is not.
 * - `markets`: one sub-series per OBSERVED market, each with its own width,
 *   laws, holders and points. A planned market that was never observed appears
 *   in `world.planned` and never here, because a market with no points must not
 *   be drawn as a market whose line is flat at zero.
 *
 * Earlier documents decode unchanged, as a capture of one market that recorded
 * no world — which is a true thing to say about every capture taken before this.
 */
export const SIMULATOR_SERIES_SCHEMA_V4 = 'dclutch-simulator-series-v4';

/** The one URL the surfaces read. Pinned by test: the artifact's link check
 * cannot see a runtime fetch, so the string itself is the contract. */
export const SIMULATOR_SERIES_URL_V1 = '/simulator-series.json';

/**
 * The campaign record's own URL, and a SECOND artifact on purpose.
 *
 * The simulator's series is a devnet census; a campaign's series is a local
 * rehearsal validator's whole-life run. They are different clusters, different
 * x-axes and different claims, and folding them into one file would put a
 * reader one merge away from believing a local founding happened on devnet.
 * Two files, two reads, two captions.
 */
export const CAMPAIGN_SERIES_URL_V1 = '/campaign-series.json';

/**
 * Where a POPULATION capture is served from. A third file rather than a third
 * schema: `/simulator-series.json` is a poller's one market, `/campaign-series.json`
 * is one campaign's one market, and this is a whole world. All three decode
 * through the same parser, because a v4 document IS a v3 document with two more
 * blocks on it.
 */
export const SIMLIFE_SERIES_URL_V1 = '/simlife-series.json';

/** One plain sentence for the shipped default state of the campaign artifact. */
export const NO_CAMPAIGN_SENTENCE_V1 =
  'No campaign record is published beside this site right now, so there is no market’s life to draw and nothing below is a zero.';

/**
 * What a reader must be told beside EVERY chart drawn from a campaign record.
 *
 * Not once in a footnote. The demo-vs-product rule this project runs on is
 * that nothing on this site may imply trading that has not happened, and a
 * chart is exactly the surface that implies it — so the sentence travels with
 * the chart, and the `cluster` field is what decides whether it is said.
 */
export const CAMPAIGN_LOCAL_CAVEAT_V1 =
  'Produced on a local rehearsal validator — a private chain this project started for the run on 127.0.0.1, with its own genesis. Not the public devnet, not mainnet, and nobody traded against it but the campaign itself.';

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
  /**
   * The campaign stage boundary this census was taken at, in the campaign's
   * own words. Null for a poller's cycle, which has no name but its number.
   */
  stage: string | null;
  /** Every tracked position's claims summed per outcome. Empty when unrecorded. */
  positionTotals: ReadonlyArray<string>;
  /** The Mint's own supply at this boundary, or null when unrecorded. */
  mintSupply: string | null;
  /** Transactions the campaign submitted in the interval ending here. */
  transactions: number | null;
  /** Compute units those transactions consumed, exact. */
  computeUnits: string | null;
  /** Lamports they paid in fees, exact. */
  feeLamports: string | null;
  /** The fee payer's lamports at this boundary, exact. */
  payerLamports: string | null;
}>;

/**
 * Which run produced a series, in enough detail to go and re-run it.
 *
 * The revision is the load-bearing field: a campaign's numbers are about ONE
 * build of seven programs, and a figure whose commit is unknown is a figure
 * that cannot be reproduced or contradicted.
 */
export type CampaignRecordV1 = Readonly<{
  /** The campaign's own name for itself, e.g. `relayed-vertical success walk`. */
  label: string;
  /** The exact source revision the programs were built from. */
  sourceRevision: string;
  /** Which walk of the campaign this was, when it has more than one. */
  walk: string | null;
  /** The loopback origin the validator answered on. Always a 127.0.0.1 URL. */
  rpcOrigin: string;
  /** The transcript file this series was read out of. */
  transcriptFile: string;
}>;

/**
 * The terminal answer, once a market has one.
 *
 * `selectedCell` is the claim index the certificate selected — the cell that
 * pays. Everything else on the market pays nothing, which is the whole of a
 * settlement and the only price move a market with no fills ever makes.
 */
export type SettlementV1 = Readonly<{
  selectedCell: number;
  /** The cell a failure would have selected, when the market disclosed one. */
  failureCell: number | null;
  /** The certificate account, when the campaign recorded its address. */
  certificate: string | null;
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

/**
 * One market inside a population, with everything a single-market series has
 * plus the world's own words for what KIND of market it is.
 *
 * `archetype`, `basis`, `destiny` and `personas` are the generator's labels,
 * not the chain's and not this site's. They say what the run was trying to
 * make; the points say what the chain did. Keeping them on the same object and
 * naming them differently is what lets a surface caption a line as "a
 * short-fuse market, drawn to miss its deadline" without implying the chain
 * called it that.
 */
export type SimulatorMarketSeriesV1 = Readonly<{
  marketId: string;
  archetype: string | null;
  basis: string | null;
  destiny: string | null;
  deadlineSlots: number | null;
  personas: ReadonlyArray<string>;
  outcomeCount: number;
  lawIds: ReadonlyArray<string>;
  laws: ReadonlyArray<ConservationLawV1>;
  positions: ReadonlyArray<SimulatorPositionV1>;
  collateralHolders: ReadonlyArray<SimulatorCollateralHolderV1>;
  claimUnitAtoms: string | null;
  settlement: SettlementV1 | null;
  cyclesRecorded: number;
  pointsOmittedBefore: number;
  censusFile: string;
  points: ReadonlyArray<SimulatorSeriesPointV1>;
}>;

/** One market the world drew, whether or not anything ever observed it. */
export type SimulatorPlannedMarketV1 = Readonly<{
  marketId: string;
  archetype: string;
  basis: string;
  destiny: string;
  outcomeCount: number;
  deadlineSlots: number;
  feeBasisPoints: number;
  foundingCollateralAtoms: string;
  personas: ReadonlyArray<string>;
  /** True when a census of this market reached the artifact. */
  observed: boolean;
}>;

/**
 * One thing the run planned and did not execute, with its count and the
 * substrate's own sentence about why.
 *
 * `outcome` is the run's four-word vocabulary and the three that appear here
 * mean different things: `refused` is the chain saying no, `unattempted` is the
 * substrate having no such route, `blocked` is a prerequisite that never
 * happened. Folding them together would turn one wall into a hundred failures.
 */
export type SimulatorNotDoneV1 = Readonly<{
  route: string;
  outcome: 'refused' | 'unattempted' | 'blocked';
  reason: string;
  count: number;
}>;

/**
 * What a run was allowed to spend and what it did spend.
 *
 * `null` for a capture taken before the ceiling existed. `bounded: false` is a
 * different statement from that and a louder one: the run HAD no ceiling, so
 * nothing would have stopped it for spending.
 */
export type SimulatorSpendV1 = Readonly<{
  maxLamportsSpent: string | null;
  spentLamports: string;
  creditedLamports: string;
  observations: number;
  bounded: boolean;
}>;

/**
 * Where a world's answers landed, and whether they landed anywhere at all.
 *
 * A population that settles every market into the same place has ONE
 * measurement copied as many times as it has markets. `positionCounts` is keyed
 * by tenths of the way through a market's own ordinary cells, because cell 3 of
 * four and cell 3 of eleven are not the same answer; `counts` keeps the raw
 * `cell/width` reading beside it for a reader who wants the unnormalised view.
 */
export type SimulatorOutcomeSpreadV1 = Readonly<{
  resolvingMarkets: number;
  distinctCells: number;
  counts: Readonly<Record<string, number>>;
  positionedMarkets: number;
  positionCounts: Readonly<Record<string, number>>;
  distinctPositions: number;
  heaviestPositionTenths: number | null;
  heaviestSharePercent: number;
  degenerateThresholdPercent: number;
  /** True when one place takes more of the world than the threshold allows. */
  degenerate: boolean;
  /** The coordinate this world's substrate observes at resolution. */
  coordinateAnchor: string;
}>;

/** Where a population was driven, and what that place could and could not do. */
export type SimulatorSubstrateV1 = Readonly<{
  name: string | null;
  label: string | null;
  cluster: string | null;
  rpcOrigin: string | null;
  sourceRevision: string | null;
  routes: ReadonlyArray<string>;
  routesAbsent: ReadonlyArray<string>;
  basisKinds: ReadonlyArray<string>;
  basisKindsAbsent: ReadonlyArray<string>;
  /** Null for a capture taken before the spend ceiling existed. */
  spend: SimulatorSpendV1 | null;
}>;

/** One route's four counts, exactly as the conductor recorded them. */
export type SimulatorTallyRowV1 = Readonly<{
  executed: number;
  refused: number;
  unattempted: number;
  blocked: number;
}>;

/**
 * One tick of the run's own history: how many planned events ended each way.
 *
 * `notDone` says WHAT a run could not do; this says WHEN it did what it did.
 * Census events are counted apart from mutations on purpose -- a tick's census
 * count is just how many markets were alive at it, and adding that to the same
 * total as the tick's foundings buries four foundings under forty observations.
 */
export type SimulatorTimelineTickV1 = Readonly<{
  tick: number;
  executed: number;
  refused: number;
  unattempted: number;
  blocked: number;
  mutationsExecuted: number;
  mutationsRefused: number;
  censusExecuted: number;
  /** `route:outcome` for every mutation that actually reached the chain. */
  routes: ReadonlyArray<string>;
}>;

export type SimulatorWorldV1 = Readonly<{
  /** The sentence the run was seeded from, so it can be re-run by name. */
  seedPreimage: string;
  seedSha256: string;
  /** The digest of the plan every event in this run came out of. */
  planDigest: string;
  substrate: SimulatorSubstrateV1;
  marketsPlanned: number;
  marketsObserved: number;
  /** Market ids this run founded itself. Empty when it founded nothing. */
  marketsFoundedByThisRun: ReadonlyArray<string>;
  /** Market ids that existed on the chain before this run started. */
  marketsPreFounded: ReadonlyArray<string>;
  planned: ReadonlyArray<SimulatorPlannedMarketV1>;
  notDone: ReadonlyArray<SimulatorNotDoneV1>;
  /**
   * The run's own tally, route by route. `executed` is the only count here a
   * page should read: the other three are also in `notDone`, WITH their
   * reasons, and a reader who sees a number without its sentence is exactly the
   * reader this vocabulary was written for.
   */
  tally: Readonly<Record<string, SimulatorTallyRowV1>>;
  /**
   * The run tick by tick. EMPTY for a capture taken before the timeline
   * existed, which is a true thing to say about it rather than a defect: the
   * document is still a complete v4 and every other block still decodes.
   */
  timeline: ReadonlyArray<SimulatorTimelineTickV1>;
  /**
   * Where this world's answers landed. NULL for a capture taken before the
   * histogram existed, which is a true thing to say about that document rather
   * than a claim that nothing settled.
   */
  outcomeSpread: SimulatorOutcomeSpreadV1 | null;
}>;

export type SimulatorSeriesV1 = Readonly<{
  schema:
    | typeof SIMULATOR_SERIES_SCHEMA_V1
    | typeof SIMULATOR_SERIES_SCHEMA_V2
    | typeof SIMULATOR_SERIES_SCHEMA_V3
    | typeof SIMULATOR_SERIES_SCHEMA_V4;
  /** The population this capture came from, or null for a single-market one. */
  world: SimulatorWorldV1 | null;
  /** Every observed market. Empty for a single-market capture. */
  markets: ReadonlyArray<SimulatorMarketSeriesV1>;
  /** The campaign that produced this record, or null for a poller's census. */
  campaign: CampaignRecordV1 | null;
  /** Collateral atoms one claim of one outcome is worth, or null when unrecorded. */
  claimUnitAtoms: string | null;
  /** The terminal answer, or null while the market has not reached one. */
  settlement: SettlementV1 | null;
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

/** An exact quantity that a capture is allowed not to have recorded at all. */
function optionalAtoms(value: unknown, field: string): string | null {
  return value === null || value === undefined ? null : atoms(value, field);
}

function optionalCount(value: unknown, field: string): number | null {
  return value === null || value === undefined ? null : count(value, field);
}

function optionalText(value: unknown, field: string): string | null {
  return value === null || value === undefined ? null : text(value, field);
}

function point(value: unknown, index: number, outcomeCount: number, lawCount: number): SimulatorSeriesPointV1 {
  const body = object(value, `point ${index}`);
  if (!Array.isArray(body.supply)) throw new Error(`point ${index} supply must be one list`);
  if (body.supply.length !== outcomeCount) {
    throw new Error(`point ${index} carries ${body.supply.length} outcomes and the series declares ${outcomeCount}`);
  }
  // Position totals are per-outcome, so a list of another length would lay one
  // cell's holdings under another cell's name — the same misattribution the
  // law-status length check exists to prevent, and refused the same way.
  const positionTotals = !Array.isArray(body.position_totals) ? [] : body.position_totals;
  if (positionTotals.length !== 0 && positionTotals.length !== outcomeCount) {
    throw new Error(`point ${index} carries ${positionTotals.length} position totals and the series declares ${outcomeCount} outcomes`);
  }
  return Object.freeze({
    stage: optionalText(body.stage, `point ${index} stage`),
    positionTotals: Object.freeze(positionTotals.map((entry, cell) => atoms(entry, `point ${index} position_totals ${cell}`))),
    mintSupply: optionalAtoms(body.mint_supply, `point ${index} mint_supply`),
    transactions: optionalCount(body.transactions, `point ${index} transactions`),
    computeUnits: optionalAtoms(body.compute_units, `point ${index} compute_units`),
    feeLamports: optionalAtoms(body.fee_lamports, `point ${index} fee_lamports`),
    payerLamports: optionalAtoms(body.payer_lamports, `point ${index} payer_lamports`),
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

/**
 * The part of a series document that describes ONE market.
 *
 * Factored out because a v4 document contains this shape twice over: once at
 * the top level, for the primary market, and once per entry in `markets`. One
 * decoder for both is not a tidiness preference — it is what guarantees that a
 * market inside a population is held to exactly the same length checks,
 * ordering rule and settlement bound as a market on its own, so no figure
 * becomes admissible by being nested.
 */
function seriesBody(root: Record<string, unknown>, where: string) {
  if (!Array.isArray(root.points)) throw new Error(`${where} points must be one list`);
  const outcomeCount = count(root.outcome_count, `${where} outcome_count`);
  // The law names come first: every point's verdict string is checked against
  // this list's length, so a series can never draw seven verdicts under six
  // names.
  const lawIds = !Array.isArray(root.law_ids)
    ? Object.freeze([])
    : Object.freeze(root.law_ids.map((entry, index) => text(entry, `${where} law_ids ${index}`)));
  const laws = !Array.isArray(root.laws) ? Object.freeze([]) : Object.freeze(root.laws.map((entry, index) => {
    const body = object(entry, `${where} law ${index}`);
    const status = body.status;
    if (status !== 'holds' && status !== 'violated' && status !== 'inapplicable') {
      throw new Error(`${where} law ${index} status must be holds, violated or inapplicable`);
    }
    return Object.freeze({
      id: text(body.id, `${where} law ${index} id`),
      status,
      detail: text(body.detail, `${where} law ${index} detail`),
    });
  }));
  if (laws.length > 0 && laws.length !== lawIds.length) {
    throw new Error(`${where}: ${laws.length} laws are described and ${lawIds.length} are named`);
  }
  for (const [index, law] of laws.entries()) {
    if (law.id !== lawIds[index]) throw new Error(`${where} law ${index} is ${law.id} and law_ids names ${lawIds[index]}`);
  }
  const points = Object.freeze(root.points.map(
    (entry, index) => point(entry, index, outcomeCount, lawIds.length),
  ));
  // A line is only honest if its x-axis is ordered. Two points out of order
  // would draw a shape that never happened, and no decoder below this one
  // would catch it.
  for (let index = 1; index < points.length; index += 1) {
    if (points[index].cycle <= points[index - 1].cycle) {
      throw new Error(`${where} point ${index} does not come after the point before it`);
    }
  }
  // Holders are optional: a capture taken before this app recorded them is a
  // capture with none, which is a true thing to say and not a decode failure.
  const positions = !Array.isArray(root.positions) ? Object.freeze([]) : Object.freeze(root.positions.map((entry, index) => {
    const body = object(entry, `${where} position ${index}`);
    if (!Array.isArray(body.claims)) throw new Error(`${where} position ${index} claims must be one list`);
    return Object.freeze({
      label: text(body.label, `${where} position ${index} label`),
      address: body.address === null || body.address === undefined ? null : address(body.address, `${where} position ${index} address`),
      lamports: body.lamports === null || body.lamports === undefined ? null : atoms(body.lamports, `${where} position ${index} lamports`),
      claims: Object.freeze(body.claims.map((entry_, cell) => atoms(entry_, `${where} position ${index} claim ${cell}`))),
      totalClaims: atoms(body.total_claims, `${where} position ${index} total_claims`),
    });
  }));
  const collateralHolders = !Array.isArray(root.collateral_holders) ? Object.freeze([]) : Object.freeze(root.collateral_holders.map((entry, index) => {
    const body = object(entry, `${where} collateral holder ${index}`);
    return Object.freeze({
      label: text(body.label, `${where} collateral holder ${index} label`),
      address: body.address === null || body.address === undefined ? null : address(body.address, `${where} collateral holder ${index} address`),
      atoms: atoms(body.atoms, `${where} collateral holder ${index} atoms`),
    });
  }));

  // The terminal answer names a CELL, so it is checked against the number of
  // cells this series declares. A selector past the end would light a column
  // that is not on the chart, or worse, light the wrong one.
  const settlementBody = root.settlement === null || root.settlement === undefined
    ? null
    : object(root.settlement, `${where} settlement`);
  const settlement = settlementBody === null ? null : Object.freeze({
    selectedCell: count(settlementBody.selected_cell, `${where} settlement selected_cell`),
    failureCell: optionalCount(settlementBody.failure_cell, `${where} settlement failure_cell`),
    certificate: settlementBody.certificate === null || settlementBody.certificate === undefined
      ? null
      : address(settlementBody.certificate, `${where} settlement certificate`),
  });
  if (settlement !== null && settlement.selectedCell >= outcomeCount) {
    throw new Error(`${where} settlement selects cell ${settlement.selectedCell} and the series declares ${outcomeCount} outcomes`);
  }
  if (settlement !== null && settlement.failureCell !== null && settlement.failureCell >= outcomeCount) {
    throw new Error(`${where} settlement names failure cell ${settlement.failureCell} and the series declares ${outcomeCount} outcomes`);
  }

  return {
    outcomeCount,
    lawIds,
    laws,
    points,
    positions,
    collateralHolders,
    settlement,
    claimUnitAtoms: optionalAtoms(root.claim_unit_atoms, `${where} claim_unit_atoms`),
    cyclesRecorded: count(root.cycles_recorded, `${where} cycles_recorded`),
    pointsOmittedBefore: count(root.points_omitted_before, `${where} points_omitted_before`),
    censusFile: text(root.census_file, `${where} census_file`),
  };
}

function marketSeries(value: unknown, index: number): SimulatorMarketSeriesV1 {
  const body = object(value, `market ${index}`);
  const marketId = text(body.market_id, `market ${index} market_id`);
  const where = `market ${marketId}`;
  const personas = !Array.isArray(body.personas)
    ? Object.freeze([])
    : Object.freeze(body.personas.map((entry, cell) => text(entry, `${where} persona ${cell}`)));
  return Object.freeze({
    marketId,
    archetype: optionalText(body.archetype, `${where} archetype`),
    basis: optionalText(body.basis, `${where} basis`),
    destiny: optionalText(body.destiny, `${where} destiny`),
    deadlineSlots: optionalCount(body.deadline_slots, `${where} deadline_slots`),
    personas,
    ...seriesBody(body, where),
  });
}

function plannedMarket(value: unknown, index: number): SimulatorPlannedMarketV1 {
  const body = object(value, `planned market ${index}`);
  const where = `planned market ${index}`;
  return Object.freeze({
    marketId: text(body.market_id, `${where} market_id`),
    archetype: text(body.archetype, `${where} archetype`),
    basis: text(body.basis, `${where} basis`),
    destiny: text(body.destiny, `${where} destiny`),
    outcomeCount: count(body.outcome_count, `${where} outcome_count`),
    deadlineSlots: count(body.deadline_slots, `${where} deadline_slots`),
    // A RATE IS A BAND, and the guard that used to stand here refused any
    // nonzero one on the reading that "fee-bearing founding does not fit in one
    // transaction on today's wire". That reading came from a document about the
    // Direct FILL's fee leg and said nothing about founding; fee-bearing
    // foundings were measured landing on a loopback validator on 2026-08-30,
    // and the owned-loopback Direct producer admits exactly 50 bps -- so zero
    // was the one rate that could never be filled and this guard refused every
    // capture of a world that could trade.
    //
    // What survives is the protocol's own domain, which is where a real
    // impossibility lives.
    feeBasisPoints: (() => {
      const fee = count(body.fee_basis_points, `${where} fee_basis_points`);
      if (fee > 10_000) throw new Error(`${where} carries a ${fee} bp fee, outside the 0..10000 a rate can be`);
      return fee;
    })(),
    foundingCollateralAtoms: atoms(body.founding_collateral_atoms, `${where} founding_collateral_atoms`),
    personas: Object.freeze(
      (Array.isArray(body.participants) ? body.participants : []).map(
        (entry, cell) => text(object(entry, `${where} participant ${cell}`).persona, `${where} participant ${cell} persona`),
      ),
    ),
    observed: body.observed === true,
  });
}

function parseWorld(value: unknown, observedIds: ReadonlySet<string>): SimulatorWorldV1 {
  const body = object(value, 'world');
  const seed = object(body.seed, 'world seed');
  const substrate = object(body.substrate, 'world substrate');
  const strings = (raw: unknown, field: string) => (!Array.isArray(raw)
    ? Object.freeze([])
    : Object.freeze(raw.map((entry, index) => text(entry, `${field} ${index}`))));
  const planned = !Array.isArray(body.planned)
    ? Object.freeze([])
    : Object.freeze(body.planned.map(plannedMarket));
  // A planned market that claims to have been observed must actually appear in
  // `markets`. The two blocks are written by the same script and could drift,
  // and a page that trusts the flag would draw a caption for a line that is not
  // there.
  for (const market of planned) {
    if (market.observed && !observedIds.has(market.marketId)) {
      throw new Error(`world says ${market.marketId} was observed and no series carries it`);
    }
  }
  const notDone = !Array.isArray(body.not_done) ? Object.freeze([]) : Object.freeze(body.not_done.map((entry, index) => {
    const row = object(entry, `world not_done ${index}`);
    const outcome = row.outcome;
    if (outcome !== 'refused' && outcome !== 'unattempted' && outcome !== 'blocked') {
      throw new Error(`world not_done ${index} outcome must be refused, unattempted or blocked`);
    }
    return Object.freeze({
      route: text(row.route, `world not_done ${index} route`),
      outcome,
      reason: text(row.reason, `world not_done ${index} reason`),
      count: count(row.count, `world not_done ${index} count`),
    });
  }));
  // The timeline is OPTIONAL, and its absence is not a defect: every capture
  // taken before it existed is still a complete v4 document. What is refused is
  // a timeline that disagrees with itself -- a tick whose four outcome counts
  // do not add up to its own total is a caption disagreeing with its chart, the
  // same species as every other refusal in this decoder.
  const timeline = !Array.isArray(body.timeline) ? Object.freeze([]) : Object.freeze(body.timeline.map((entry, index) => {
    const row = object(entry, `world timeline ${index}`);
    const executed = count(row.executed, `world timeline ${index} executed`);
    const refused = count(row.refused, `world timeline ${index} refused`);
    const mutationsExecuted = count(row.mutations_executed, `world timeline ${index} mutations_executed`);
    const censusExecuted = count(row.census_executed, `world timeline ${index} census_executed`);
    if (mutationsExecuted + censusExecuted !== executed) {
      throw new Error(
        `world timeline ${index} says ${executed} executed but splits into ${mutationsExecuted} `
        + `mutations and ${censusExecuted} censuses`,
      );
    }
    return Object.freeze({
      tick: count(row.tick, `world timeline ${index} tick`),
      executed,
      refused,
      unattempted: count(row.unattempted, `world timeline ${index} unattempted`),
      blocked: count(row.blocked, `world timeline ${index} blocked`),
      mutationsExecuted,
      mutationsRefused: count(row.mutations_refused, `world timeline ${index} mutations_refused`),
      censusExecuted,
      routes: strings(row.routes, `world timeline ${index} routes`),
    });
  }));
  const tallyBody = body.tally === undefined || body.tally === null
    ? {}
    : object(body.tally, 'world tally');
  const tally: Record<string, SimulatorTallyRowV1> = {};
  for (const [route, counts] of Object.entries(tallyBody)) {
    const cell = object(counts, `world tally ${route}`);
    tally[route] = Object.freeze({
      executed: count(cell.executed, `world tally ${route} executed`),
      refused: count(cell.refused, `world tally ${route} refused`),
      unattempted: count(cell.unattempted, `world tally ${route} unattempted`),
      blocked: count(cell.blocked, `world tally ${route} blocked`),
    });
  }
  return Object.freeze({
    timeline,
    tally: Object.freeze(tally),
    seedPreimage: text(seed.preimage, 'world seed preimage'),
    seedSha256: text(seed.sha256, 'world seed sha256'),
    planDigest: text(body.plan_digest, 'world plan_digest'),
    substrate: Object.freeze({
      name: optionalText(substrate.name, 'world substrate name'),
      label: optionalText(substrate.label, 'world substrate label'),
      cluster: optionalText(substrate.cluster, 'world substrate cluster'),
      rpcOrigin: optionalText(substrate.rpc_origin, 'world substrate rpc_origin'),
      sourceRevision: optionalText(substrate.source_revision, 'world substrate source_revision'),
      routes: strings(substrate.routes, 'world substrate routes'),
      routesAbsent: strings(substrate.routes_absent, 'world substrate routes_absent'),
      basisKinds: strings(substrate.basis_kinds, 'world substrate basis_kinds'),
      basisKindsAbsent: strings(substrate.basis_kinds_absent, 'world substrate basis_kinds_absent'),
      spend: decodeSpend(substrate.spend),
    }),
    outcomeSpread: decodeOutcomeSpread(body.outcome_spread),
    marketsPlanned: count(body.markets_planned, 'world markets_planned'),
    marketsObserved: count(body.markets_observed, 'world markets_observed'),
    marketsFoundedByThisRun: strings(body.markets_founded_by_this_run, 'world markets_founded_by_this_run'),
    marketsPreFounded: strings(body.markets_pre_founded, 'world markets_pre_founded'),
    planned,
    notDone,
  });
}


/** A run's spend record, or null for a capture taken before one existed. */
function decodeSpend(value: unknown): SimulatorSpendV1 | null {
  if (value === undefined || value === null) return null;
  const body = object(value, 'world substrate spend');
  const bounded = body.bounded === true;
  const max = optionalAtoms(body.max_lamports_spent, 'spend max_lamports_spent');
  // A bound is a number or it is not a bound. A record claiming to be bounded
  // with no ceiling in it is the caption-disagrees-with-its-chart species.
  if (bounded !== (max !== null)) {
    throw new Error(
      `world substrate spend says bounded=${bounded} with max_lamports_spent=${String(max)}`,
    );
  }
  return Object.freeze({
    maxLamportsSpent: max,
    spentLamports: atoms(body.spent_lamports, 'spend spent_lamports'),
    creditedLamports: atoms(body.credited_lamports, 'spend credited_lamports'),
    observations: count(body.observations, 'spend observations'),
    bounded,
  });
}

/** Where a world's answers landed, or null when the capture predates it. */
function decodeOutcomeSpread(value: unknown): SimulatorOutcomeSpreadV1 | null {
  if (value === undefined || value === null) return null;
  const body = object(value, 'world outcome_spread');
  const histogram = (raw: unknown, field: string): Record<string, number> => {
    const table = object(raw, field);
    const out: Record<string, number> = {};
    for (const [key, entry] of Object.entries(table)) out[key] = count(entry, `${field} ${key}`);
    return out;
  };
  const positionCounts = histogram(body.position_counts, 'outcome_spread position_counts');
  const positioned = count(body.positioned_markets, 'outcome_spread positioned_markets');
  const summed = Object.values(positionCounts).reduce((total, entry) => total + entry, 0);
  // The histogram must be the same markets the header counts. A page drawing
  // bars under a total they do not add up to is the one defect this decoder
  // exists to refuse.
  if (summed !== positioned) {
    throw new Error(
      `world outcome_spread positions sum to ${summed} under a total of ${positioned}`,
    );
  }
  const heaviest = body.heaviest_position_tenths;
  return Object.freeze({
    resolvingMarkets: count(body.resolving_markets, 'outcome_spread resolving_markets'),
    distinctCells: count(body.distinct_cells, 'outcome_spread distinct_cells'),
    counts: Object.freeze(histogram(body.counts, 'outcome_spread counts')),
    positionedMarkets: positioned,
    positionCounts: Object.freeze(positionCounts),
    distinctPositions: count(body.distinct_positions, 'outcome_spread distinct_positions'),
    heaviestPositionTenths: heaviest === null || heaviest === undefined
      ? null
      : count(heaviest, 'outcome_spread heaviest_position_tenths'),
    heaviestSharePercent: count(body.heaviest_share_percent, 'outcome_spread heaviest_share_percent'),
    degenerateThresholdPercent: count(
      body.degenerate_threshold_percent, 'outcome_spread degenerate_threshold_percent',
    ),
    degenerate: body.degenerate === true,
    coordinateAnchor: atoms(body.coordinate_anchor, 'outcome_spread coordinate_anchor'),
  });
}

/** Decode one series document. Throws with the field named; never returns a
 * half-series, and never a series whose cycles run backwards. */
export function parseSimulatorSeriesV1(value: unknown): SimulatorSeriesV1 {
  const root = object(value, 'simulator series');
  const schema = root.schema;
  if (
    schema !== SIMULATOR_SERIES_SCHEMA_V1
    && schema !== SIMULATOR_SERIES_SCHEMA_V2
    && schema !== SIMULATOR_SERIES_SCHEMA_V3
    && schema !== SIMULATOR_SERIES_SCHEMA_V4
  ) {
    throw new Error('simulator series has another schema');
  }
  const cluster = root.cluster;
  if (cluster !== 'local' && cluster !== 'devnet') throw new Error('cluster must be local or devnet');
  const mode = root.mode;
  if (mode !== 'finite' && mode !== 'sustain') throw new Error('mode must be finite or sustain');
  const body = seriesBody(root, 'series');

  // Which run this was. A campaign block that names no revision is refused
  // rather than carried: a per-cell figure whose build is unknown cannot be
  // reproduced, and this is the field a reader goes back to the chain with.
  const campaignBody = root.campaign === null || root.campaign === undefined
    ? null
    : object(root.campaign, 'campaign');
  const campaign = campaignBody === null ? null : Object.freeze({
    label: text(campaignBody.label, 'campaign label'),
    sourceRevision: text(campaignBody.source_revision, 'campaign source_revision'),
    walk: optionalText(campaignBody.walk, 'campaign walk'),
    rpcOrigin: text(campaignBody.rpc_origin, 'campaign rpc_origin'),
    transcriptFile: text(campaignBody.transcript_file, 'campaign transcript_file'),
  });

  const markets = !Array.isArray(root.markets)
    ? Object.freeze([])
    : Object.freeze(root.markets.map(marketSeries));
  const observedIds = new Set(markets.map((market) => market.marketId));
  if (observedIds.size !== markets.length) {
    throw new Error('two markets in this capture carry the same id');
  }
  const world = root.world === null || root.world === undefined
    ? null
    : parseWorld(root.world, observedIds);
  // A world that counts more observed markets than it carries is a world whose
  // caption disagrees with its own charts.
  if (world !== null && world.marketsObserved !== markets.length) {
    throw new Error(`world claims ${world.marketsObserved} observed markets and carries ${markets.length}`);
  }

  return Object.freeze({
    schema,
    world,
    markets,
    campaign,
    capturedAt: instant(root.captured_at, 'captured_at'),
    cluster,
    market: root.market === null || root.market === undefined ? null : address(root.market, 'market'),
    mode,
    ...body,
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
  url: string = SIMULATOR_SERIES_URL_V1,
): Promise<SimulatorSeriesReadV1> {
  let body: string;
  try {
    const response = await fetchLike(url);
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

/**
 * The x-axis of a campaign: the boundaries, in the campaign's own words.
 *
 * A campaign's points are not evenly spaced instants and calling them "cycle
 * 1..N" would suggest they are. Where a point recorded its stage, that name is
 * the label; where it did not, the cycle number is, because inventing a name
 * for a boundary nobody named is worse than showing the number.
 */
export function campaignStageLabelsV1(series: SimulatorSeriesV1): ReadonlyArray<string> {
  return Object.freeze(series.points.map((entry) => entry.stage ?? `cycle ${entry.cycle}`));
}

/**
 * THE ODDS PATH: each cell's share of the issued claim supply, in basis points.
 *
 * This is what a prediction market means by odds, computed the only way this
 * record supports computing it — from the Claims aggregate's own liability
 * supply, which is what the market says it owes on each outcome. It is not a
 * price a buyer paid, because in this record nobody has bought anything; it is
 * the distribution the market is standing at.
 *
 * EXACT, AND FLOORED. `supply * 10000 / total` on BigInt, floored, so the
 * figures are integers a reader can add up and never a float that drifts. The
 * floor means the cells can sum to slightly under 10,000, which is a true
 * statement about integer division and is said in the caption rather than
 * hidden by scaling the last cell to make it come out even.
 *
 * A BOUNDARY WITH NOTHING ISSUED HAS NO ODDS. A share of zero supply is not
 * zero percent, it is undefined, so a series with any such boundary draws no
 * odds line at all — the same rule the heartbeat's cadence line follows.
 */
export function impliedOddsLinesV1(
  series: SimulatorSeriesV1,
  outcomes?: ReadonlyArray<string> | null,
): ReadonlyArray<SimulatorSeriesLineV1> {
  if (series.points.length === 0 || series.outcomeCount === 0) return Object.freeze([]);
  const totals = series.points.map((entry) => entry.supply.reduce((sum, value) => sum + BigInt(value), 0n));
  if (totals.some((total) => total === 0n)) return Object.freeze([]);
  return Object.freeze(Array.from({ length: series.outcomeCount }, (_unused, cell) => Object.freeze({
    label: outcomes?.[cell] === undefined ? `claim ${cell}` : `claim ${cell} · ${outcomes[cell]}`,
    values: Object.freeze(series.points.map((entry, index) => ((BigInt(entry.supply[cell]) * 10_000n) / totals[index]).toString())),
  })));
}

/**
 * THE MONEY: what the market's own vault holds, against everything tracked.
 *
 * Two lines and not one number, because the interesting thing is the gap. The
 * Hoard is the market's collateral; the tracked total is every atom of that
 * collateral the census could name anywhere. When the two move apart, atoms
 * left the vault for an account somebody still watches; when the tracked total
 * itself moves, an atom went somewhere nobody named, and L1 says so first.
 */
export function hoardCoverageLinesV1(series: SimulatorSeriesV1): ReadonlyArray<SimulatorSeriesLineV1> {
  if (series.points.length === 0) return Object.freeze([]);
  const lines: SimulatorSeriesLineV1[] = [
    Object.freeze({ label: 'in the market’s own Hoard', values: Object.freeze(series.points.map((entry) => entry.hoardAtoms)) }),
    Object.freeze({ label: 'tracked across every named account', values: Object.freeze(series.points.map((entry) => entry.trackedCollateral)) }),
  ];
  // The Mint's supply is drawn only when every boundary recorded it: a line
  // with a hole would be redrawn shorter than the axis it sits on.
  if (series.points.every((entry) => entry.mintSupply !== null)) {
    lines.push(Object.freeze({
      label: 'the collateral Mint’s whole supply',
      values: Object.freeze(series.points.map((entry) => entry.mintSupply as string)),
    }));
  }
  return Object.freeze(lines);
}

/**
 * THE VOLUME a market without fills actually has: the work its stages cost.
 *
 * There is no traded volume in this record and there must be no chart that
 * looks like one. What there is, exactly, is how many transactions each
 * boundary took, what they burned in compute, and what they paid in fees —
 * three separate dimensions kept on three separate figures for the same reason
 * the heartbeat keeps slots and seconds apart.
 *
 * Each line is drawn only when EVERY boundary recorded it.
 */
export type CampaignVolumeV1 = Readonly<{
  xLabels: ReadonlyArray<string>;
  transactions: SimulatorSeriesLineV1 | null;
  computeUnits: SimulatorSeriesLineV1 | null;
  feeLamports: SimulatorSeriesLineV1 | null;
  totalTransactions: string | null;
  totalComputeUnits: string | null;
  totalFeeLamports: string | null;
}>;

export function campaignVolumeV1(series: SimulatorSeriesV1): CampaignVolumeV1 | null {
  const points = series.points;
  if (points.length === 0) return null;
  const complete = <T,>(pick: (entry: SimulatorSeriesPointV1) => T | null): ReadonlyArray<T> | null => {
    const values = points.map(pick);
    return values.some((value) => value === null) ? null : (values as T[]);
  };
  const transactions = complete((entry) => entry.transactions);
  const computeUnits = complete((entry) => entry.computeUnits);
  const feeLamports = complete((entry) => entry.feeLamports);
  if (transactions === null && computeUnits === null && feeLamports === null) return null;
  const sum = (values: ReadonlyArray<string> | null) =>
    (values === null ? null : values.reduce((total, value) => total + BigInt(value), 0n).toString());
  const line = (label: string, values: ReadonlyArray<string> | null) =>
    (values === null ? null : Object.freeze({ label, values: Object.freeze([...values]) }));
  const transactionStrings = transactions === null ? null : transactions.map(String);
  return Object.freeze({
    xLabels: campaignStageLabelsV1(series),
    transactions: line('transactions submitted', transactionStrings),
    computeUnits: line('compute units consumed', computeUnits),
    feeLamports: line('lamports paid in fees', feeLamports),
    totalTransactions: sum(transactionStrings),
    totalComputeUnits: sum(computeUnits),
    totalFeeLamports: sum(feeLamports),
  });
}

/**
 * WHAT THE RUN HAS SPENT: the fee payer's balance, boundary by boundary.
 *
 * This is a level and not an interval, so it is deliberately not folded into
 * the volume above — adding a balance to a list of per-interval counts would
 * put two different kinds of number on one axis. It is drawn as the drop from
 * the first boundary rather than as the raw balance, because a genesis-funded
 * local payer starts at a number with eighteen digits and the interesting
 * quantity is the last six of them.
 *
 * Exact throughout: BigInt subtraction on the recorded balances, never a
 * difference of two doubles.
 */
export function campaignSpendLineV1(series: SimulatorSeriesV1): SimulatorSeriesLineV1 | null {
  const points = series.points;
  if (points.length === 0 || points.some((entry) => entry.payerLamports === null)) return null;
  const first = BigInt(points[0].payerLamports as string);
  // A payer whose balance ROSE is not spending; it was topped up, or this is
  // not the account that pays. Either way "spent so far" would be a negative
  // number wearing a positive name, so the line is dropped and said to be.
  if (points.some((entry) => BigInt(entry.payerLamports as string) > first)) return null;
  return Object.freeze({
    label: 'lamports the fee payer has spent since the first boundary',
    values: Object.freeze(points.map((entry) => (first - BigInt(entry.payerLamports as string)).toString())),
  });
}

/**
 * What one claim on each cell turned out to be worth, once the answer landed.
 *
 * This is the only price move in a record with no fills, and it is total: the
 * selected cell's claims are worth the claim unit in collateral and every
 * other cell's are worth nothing at all. It is stated per cell rather than
 * drawn as a line, because two points — before the answer and after it — is a
 * settlement, not a path, and drawing it as a path would invent the shape in
 * between.
 */
export type SettlementCellV1 = Readonly<{
  cell: number;
  label: string;
  selected: boolean;
  /** Claims the market had issued on this cell at the last boundary. */
  claimsIssued: string;
  /** Collateral atoms one claim of this cell is worth now. */
  realizedAtomsPerClaim: string;
  /** Those claims at that value: what this cell is owed in total. */
  realizedAtoms: string;
}>;

export function settlementCellsV1(
  series: SimulatorSeriesV1,
  outcomes?: ReadonlyArray<string> | null,
): ReadonlyArray<SettlementCellV1> {
  const settlement = series.settlement;
  const unit = series.claimUnitAtoms;
  if (settlement === null || unit === null || series.points.length === 0) return Object.freeze([]);
  const last = series.points[series.points.length - 1];
  return Object.freeze(Array.from({ length: series.outcomeCount }, (_unused, cell) => {
    const selected = cell === settlement.selectedCell;
    const issued = last.supply[cell];
    const perClaim = selected ? unit : '0';
    return Object.freeze({
      cell,
      label: outcomes?.[cell] === undefined ? `claim ${cell}` : `claim ${cell} · ${outcomes[cell]}`,
      selected,
      claimsIssued: issued,
      realizedAtomsPerClaim: perClaim,
      realizedAtoms: (BigInt(issued) * BigInt(perClaim)).toString(),
    });
  }));
}

/**
 * One plain sentence about what this campaign record is, or null when it is
 * not a campaign record at all.
 *
 * It leads with the cluster, because that is the fact a reader is most likely
 * to get wrong and the one this project has decided must never be implied.
 */
export function campaignReadingV1(series: SimulatorSeriesV1): string | null {
  const campaign = series.campaign;
  if (campaign === null) return null;
  const where = series.cluster === 'local'
    ? `a local rehearsal validator at ${campaign.rpcOrigin}`
    : `the ${series.cluster} cluster`;
  const boundaries = `${series.points.length} stage boundar${series.points.length === 1 ? 'y' : 'ies'}`;
  const settled = series.settlement === null
    ? 'The market has not reached a terminal answer in this record.'
    : `The market reached a terminal answer: cell ${series.settlement.selectedCell} was selected.`;
  return `${campaign.label}, run against ${where} from source revision ${campaign.sourceRevision.slice(0, 12)}, re-censused at ${boundaries}. ${settled}`;
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

/**
 * ONE ROW PER OBSERVED MARKET, for the table that has to come before any chart
 * of a population.
 *
 * Twelve lines on one pair of axes is not a picture of twelve markets, it is a
 * picture of nothing; a reader needs to know which markets exist, what shape
 * each is, and — the question they are actually asking — whether anything about
 * it moved. `moved` is computed from the record rather than asserted: a market
 * whose supply, Hoard and tracked collateral are the same at every boundary did
 * not move, and saying so is the honest caption for a flat line.
 */
export type MarketRowV1 = Readonly<{
  marketId: string;
  archetype: string | null;
  basis: string | null;
  destiny: string | null;
  outcomeCount: number;
  points: number;
  firstSlot: string | null;
  lastSlot: string | null;
  /** Slots the chain advanced between this market's first and last reading. */
  slotsCovered: string | null;
  checksHeld: number;
  checksBroken: number;
  /** Which recorded quantities took more than one value across the drawn ticks. */
  moved: ReadonlyArray<string>;
  /** How many holders the newest reading found, and how many hold everything. */
  positionCount: number;
}>;

export function marketRowsV1(series: SimulatorSeriesV1): ReadonlyArray<MarketRowV1> {
  return Object.freeze(series.markets.map((market) => {
    const points = market.points;
    const distinct = (pick: (entry: SimulatorSeriesPointV1) => string) =>
      new Set(points.map(pick)).size > 1;
    const moved: string[] = [];
    if (distinct((entry) => entry.supply.join(','))) moved.push('issued claims');
    if (distinct((entry) => entry.hoardAtoms)) moved.push('the Hoard');
    if (distinct((entry) => entry.trackedCollateral)) moved.push('tracked collateral');
    if (distinct((entry) => entry.positionTotals.join(','))) moved.push('who is holding');
    return Object.freeze({
      marketId: market.marketId,
      archetype: market.archetype,
      basis: market.basis,
      destiny: market.destiny,
      outcomeCount: market.outcomeCount,
      points: points.length,
      firstSlot: points.length === 0 ? null : points[0].slot,
      lastSlot: points.length === 0 ? null : points[points.length - 1].slot,
      slotsCovered: points.length < 2
        ? null
        : (BigInt(points[points.length - 1].slot) - BigInt(points[0].slot)).toString(),
      checksHeld: points.reduce((sum, entry) => sum + entry.checksHeld, 0),
      checksBroken: points.reduce((sum, entry) => sum + entry.checksBroken, 0),
      moved: Object.freeze(moved),
      positionCount: market.positions.length,
    });
  }));
}

/**
 * One plain sentence about what this run WAS, or null when it is not a
 * population capture.
 *
 * It leads with the seed, because a population's first claim on a reader is
 * that it is reproducible, and ends with what the run founded — which for a
 * census-only run is nothing, and must be said rather than left to be assumed
 * from the presence of markets on the page.
 */
export function populationReadingV1(series: SimulatorSeriesV1): string | null {
  const world = series.world;
  if (world === null) return null;
  const where = world.substrate.label ?? `the ${series.cluster} cluster`;
  const founded = world.marketsFoundedByThisRun.length;
  const existing = world.marketsPreFounded.length;
  const provenance = founded === 0
    ? `This run founded no market of its own; the ${existing === 1 ? 'one it observed' : `${existing} it observed`} already stood on that chain.`
    : `This run founded ${founded} of them itself.`;
  return `${world.marketsPlanned} markets drawn from the seed ${world.seedPreimage}, `
    + `walked against ${where}. ${world.marketsObserved} of them were observed. ${provenance}`;
}

/**
 * One plain sentence about what the run DID NOT DO, or null when it did
 * everything it planned.
 *
 * This exists because the alternative is a page that draws a census-only run
 * exactly as it would draw a trading one. It leads with `refused`, when there is
 * one, because a chain saying no is a different and more interesting fact than a
 * substrate having no route — and it never adds the three states together,
 * because that would turn one wall into a hundred failures.
 */
export function notDoneReadingV1(series: SimulatorSeriesV1): string | null {
  const world = series.world;
  if (world === null || world.notDone.length === 0) return null;
  const total = (wanted: SimulatorNotDoneV1['outcome']) => world.notDone
    .filter((row) => row.outcome === wanted)
    .reduce((sum, row) => sum + row.count, 0);
  const routes = (wanted: SimulatorNotDoneV1['outcome']) => Object.freeze([...new Set(
    world.notDone.filter((row) => row.outcome === wanted).map((row) => row.route),
  )]);
  const refused = total('refused');
  const clauses: string[] = [];
  if (refused > 0) {
    clauses.push(`${refused} planned ${refused === 1 ? 'step was' : 'steps were'} refused by the chain (${routes('refused').join(', ')})`);
  }
  const unattempted = total('unattempted');
  if (unattempted > 0) {
    clauses.push(`${unattempted} were never attempted, because this substrate has no route for ${routes('unattempted').join(', ')}`);
  }
  const blocked = total('blocked');
  if (blocked > 0) {
    clauses.push(`${blocked} were blocked behind a step that never happened`);
  }
  if (clauses.length === 0) return null;
  return `${clauses.join('; ')}. Those are three different things and this record keeps them apart.`;
}

/**
 * ONE MARKET'S ODDS PATH, inside a population.
 *
 * The single-market `impliedOddsLinesV1` reads `series.outcomeCount` and
 * `series.points`; a market nested in a population has exactly those two fields
 * and nothing else it needs, so the arithmetic is shared rather than copied.
 * Copying it would be a second place for the floored-BigInt rule to drift, and
 * the point of a population page is that its lines are comparable.
 */
export function marketOddsLinesV1(
  market: SimulatorMarketSeriesV1,
  outcomes?: ReadonlyArray<string> | null,
): ReadonlyArray<SimulatorSeriesLineV1> {
  if (market.points.length === 0 || market.outcomeCount === 0) return Object.freeze([]);
  const totals = market.points.map((entry) => entry.supply.reduce((sum, value) => sum + BigInt(value), 0n));
  if (totals.some((total) => total === 0n)) return Object.freeze([]);
  return Object.freeze(Array.from({ length: market.outcomeCount }, (_unused, cell) => Object.freeze({
    label: outcomes?.[cell] === undefined ? `claim ${cell}` : `claim ${cell} · ${outcomes[cell]}`,
    values: Object.freeze(market.points.map((entry, index) => ((BigInt(entry.supply[cell]) * 10_000n) / totals[index]).toString())),
  })));
}

/** The slots a market was read at, as x-axis labels a reader can hover. */
export function marketSlotLabelsV1(market: SimulatorMarketSeriesV1): ReadonlyArray<string> {
  return Object.freeze(market.points.map((entry) => `slot ${entry.slot}`));
}

/**
 * THE POPULATION'S EVENT TIMELINE, as lines over one shared tick axis.
 *
 * Three lines and not one, because the three things they count are not
 * interchangeable: mutations that reached the chain, mutations the chain
 * refused, and observations. A single "events" line would let a run that
 * founded four markets look identical to a run that failed four foundings and
 * censused a lot, which is the exact confusion this whole vocabulary exists to
 * prevent.
 *
 * `blocked` and `unattempted` are deliberately NOT lines here. They are
 * consequences of a shape rather than events in time — every tick after an
 * unfoundable market is drawn blocks the same way for the same reason — so they
 * belong in the honesty strip, which counts reasons, not in a chart that
 * implies they happened at a moment.
 */
export function eventTimelineLinesV1(series: SimulatorSeriesV1): ReadonlyArray<SimulatorSeriesLineV1> {
  const timeline = series.world?.timeline ?? Object.freeze([]);
  if (timeline.length === 0) return Object.freeze([]);
  return Object.freeze([
    Object.freeze({
      label: 'mutations that landed',
      values: Object.freeze(timeline.map((row) => String(row.mutationsExecuted))),
    }),
    Object.freeze({
      label: 'mutations the chain refused',
      values: Object.freeze(timeline.map((row) => String(row.mutationsRefused))),
    }),
    Object.freeze({
      label: 'markets censused',
      values: Object.freeze(timeline.map((row) => String(row.censusExecuted))),
    }),
  ]);
}

export function eventTimelineLabelsV1(series: SimulatorSeriesV1): ReadonlyArray<string> {
  const timeline = series.world?.timeline ?? Object.freeze([]);
  return Object.freeze(timeline.map((row) => `tick ${row.tick}`));
}

/** One route's four endings, counted over the whole run. */
export type HonestyRowV1 = Readonly<{
  route: string;
  executed: number;
  refused: number;
  unattempted: number;
  blocked: number;
  planned: number;
  /**
   * A SHORT DESCRIPTIVE sentence for the commonest thing that was not done.
   *
   * NOT the substrate's own note. Those are engineering register entries and
   * they carry file paths, test names, hour estimates and raw nested Rust
   * errors; the full text stays in the capture, where it belongs, and a page
   * shows what happened. See `publicReasonV1`.
   */
  leadingReason: string | null;
  leadingOutcome: SimulatorNotDoneV1['outcome'] | null;
}>;

/**
 * One short sentence for a step that did not happen.
 *
 * The substrate's own reason is an INTERNAL NOTE. It is written for whoever has
 * to fix the thing and it reads like it: `programs/dclutch-claims-sbf/tests/…`,
 * `a_market_retires_a_sleeping_holders_position_…`, `6-10 hours plus 1-2 for the
 * gauntlet binding`, `Error: Error("authority keypair public key 66LV… differs
 * from authenticated input EahM…")`. None of that is public copy, and rendering
 * it verbatim was this page publishing our ticket queue.
 *
 * The note is NOT deleted. It stays on `world.not_done[].reason` in the capture,
 * which is the record; this is the render layer, and it says what happened.
 *
 * EVERY BRANCH MUST BE TRUE OF EVERY ROW IT MATCHES. The fallbacks are keyed on
 * the outcome word alone, because those three words are defined and a sentence
 * built from one of them cannot be wrong about a reason it did not read.
 */
export function publicReasonV1(entry: Readonly<{
  outcome: SimulatorNotDoneV1['outcome'];
  reason: string;
}>): string {
  const note = entry.reason.toLowerCase();
  if (entry.outcome === 'unattempted') {
    // `unattempted` means the substrate has no such route, by definition of the
    // word, so this sentence is true of every row that carries it.
    return 'No tool exists yet for this step.';
  }
  if (entry.outcome === 'blocked') {
    if (note.includes('basis')) return 'This market’s payout shape is one the local compiler cannot emit.';
    if (note.includes('never founded')) return 'The market was never founded.';
    if (note.includes('terminal answer')) return 'The market never reached a terminal answer.';
    if (note.includes('already retired')) return 'The market was already retired.';
    return 'A step this one depends on did not happen.';
  }
  if (note.includes('fee') && note.includes('basis points')) return 'Refused: the market’s fee rate is not the one this release can trade at.';
  if (note.includes('execution root')) return 'Refused: the market’s trading capability was never activated.';
  if (note.includes('finalized')) return 'Refused: the chain had not finalized a prerequisite yet.';
  if (note.includes('authority') || note.includes('keypair')) return 'Refused: a key did not match the one the step authenticates.';
  if (note.includes('balance') || note.includes('insufficient')) return 'Refused: the account did not hold enough to cover it.';
  return 'The chain refused this step.';
}

/**
 * THE HONESTY STRIP: every route the world planned, and what became of it.
 *
 * This is the table a reader should be able to check the rest of the page
 * against. `executed` comes from the run's own tally; the other three come from
 * the grouped `not_done` block, which carries the reason as well as the count.
 * Nothing is summed across the three: a route with one refusal and forty blocks
 * is not a route with forty-one failures, and a strip that added them would say
 * it was.
 */
export function honestyRowsV1(series: SimulatorSeriesV1): ReadonlyArray<HonestyRowV1> {
  const world = series.world;
  if (world === null) return Object.freeze([]);
  const rows = new Map<string, {
    route: string; executed: number; refused: number; unattempted: number; blocked: number;
    leadingReason: string | null; leadingOutcome: SimulatorNotDoneV1['outcome'] | null; leadingCount: number;
  }>();
  const row = (route: string) => {
    const found = rows.get(route) ?? {
      route, executed: 0, refused: 0, unattempted: 0, blocked: 0,
      leadingReason: null, leadingOutcome: null, leadingCount: 0,
    };
    rows.set(route, found);
    return found;
  };
  for (const [route, counts] of Object.entries(world.tally)) {
    row(route).executed = counts.executed;
  }
  for (const entry of world.notDone) {
    const target = row(entry.route);
    target[entry.outcome] += entry.count;
    if (entry.count > target.leadingCount) {
      target.leadingCount = entry.count;
      target.leadingReason = publicReasonV1(entry);
      target.leadingOutcome = entry.outcome;
    }
  }
  return Object.freeze([...rows.values()]
    .map((entry) => Object.freeze({
      route: entry.route,
      executed: entry.executed,
      refused: entry.refused,
      unattempted: entry.unattempted,
      blocked: entry.blocked,
      planned: entry.executed + entry.refused + entry.unattempted + entry.blocked,
      leadingReason: entry.leadingReason,
      leadingOutcome: entry.leadingOutcome,
    }))
    .sort((left, right) => right.executed - left.executed || left.route.localeCompare(right.route)));
}

/**
 * One sentence about what this run DID, which is the half `notDoneReadingV1`
 * deliberately does not cover.
 *
 * It leads with mutations rather than with the total, because a run whose only
 * executed events are censuses is a watcher and a run that founded its own
 * markets is a participant, and the number that separates them is the one a
 * reader wants first.
 */
export function executedReadingV1(series: SimulatorSeriesV1): string | null {
  const world = series.world;
  if (world === null) return null;
  const rows = honestyRowsV1(series);
  const mutations = rows.filter((entry) => entry.route !== 'census');
  const executed = mutations.reduce((sum, entry) => sum + entry.executed, 0);
  const census = rows.find((entry) => entry.route === 'census')?.executed ?? 0;
  if (executed === 0) {
    return `Nothing was mutated: this run took ${census} censuses and signed nothing else.`;
  }
  const named = mutations.filter((entry) => entry.executed > 0)
    .map((entry) => `${entry.executed} ${entry.route}`)
    .join(', ');
  return `${executed} mutations landed on the chain (${named}), and ${census} censuses read the `
    + 'result back through the same conservation ledger.';
}

/**
 * The archetypes a world drew, counted — including the ones nothing observed.
 *
 * A population's shape is a fact about the PLAN, and it stays true whether or
 * not a substrate could drive it. Reporting it from `world.planned` rather than
 * from `markets` is what lets a page say "this world contains three short-fuse
 * markets" on a run where none of them could be founded.
 */
export function archetypeCensusV1(series: SimulatorSeriesV1): ReadonlyArray<Readonly<{
  archetype: string;
  planned: number;
  observed: number;
  basis: string;
}>> {
  const world = series.world;
  if (world === null) return Object.freeze([]);
  const table = new Map<string, { archetype: string; planned: number; observed: number; basis: string }>();
  for (const market of world.planned) {
    const row = table.get(market.archetype)
      ?? { archetype: market.archetype, planned: 0, observed: 0, basis: market.basis };
    row.planned += 1;
    if (market.observed) row.observed += 1;
    table.set(market.archetype, row);
  }
  return Object.freeze([...table.values()]
    .sort((left, right) => right.planned - left.planned || left.archetype.localeCompare(right.archetype))
    .map((row) => Object.freeze(row)));
}
