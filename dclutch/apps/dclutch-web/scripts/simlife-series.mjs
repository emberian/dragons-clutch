#!/usr/bin/env node
// Capture a simlife WORLD -- many markets at once -- as one file this site can
// serve.
//
//   node scripts/simlife-series.mjs --work <dir> [--points <n>] [--out <file>] [--check]
//
// WHY A FOURTH SCHEMA VERSION RATHER THAN A FOURTH FILE.
//
// `simulator-series.mjs` captures one market a poller watched; `campaign-series.mjs`
// captures one market a campaign lived through. Both are one market, because
// until now the simulator only ever had one. A simlife run has a POPULATION:
// markets of different archetypes, different widths, different fuses, some
// resolving and some sleeping, all on one chain and all censused at the same
// ticks. Splitting that into N files would lose the only thing the population
// has that a single market does not -- that these markets are contemporaries,
// read at the same boundaries, so their lines share an x-axis and can honestly
// be drawn beside each other.
//
// So v4 is v3 plus two blocks and NOTHING ELSE changes: the top level still
// describes ONE market exactly as v3 does (the primary, so every existing
// surface keeps drawing without knowing this version exists), and beside it
//
//   `world`    what was drawn, from what seed, against what substrate, and --
//              route by route -- what the substrate could not do. This is the
//              honest half: a world plans nine kinds of thing and today's
//              substrate can do one of them, and the artifact says which.
//   `markets`  one v3-shaped sub-series per OBSERVED market, each carrying its
//              own archetype, width, laws and points.
//
// WHAT THIS SCRIPT WILL NOT DO. It observes no chain and invents no point.
// Every number comes from a census file the simlife run wrote or from the
// run's own ledger of what it attempted. A planned market that was never
// observed appears in `world.planned` and NEVER in `markets`: it has no points,
// and a market with no points must not be drawn as a market with a flat line.

import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const SERIES_SCHEMA = 'dclutch-simulator-series-v4';
const WORLD_SCHEMA = 'dclutch-simlife-world-v1';
const LEDGER_SCHEMA = 'dclutch-simlife-ledger-v1';
const CENSUS_STAGE = /^simlife-(.+)-tick-(\d+)$/;
const LAW_STATUS_CHARS = { holds: 'h', violated: 'v', inapplicable: 'i' };

const HERE = path.dirname(new URL(import.meta.url).pathname);
const APP = path.resolve(HERE, '..');
const DEFAULT_POINTS = 240;

function usage(message) {
  console.error(`simlife-series: ${message}`);
  console.error('usage: node scripts/simlife-series.mjs --work <dir> [--points <n>] [--out <file>] [--check]');
  process.exit(2);
}

function args(argv) {
  const out = { work: null, points: DEFAULT_POINTS, check: false, out: null };
  for (let i = 0; i < argv.length; i += 1) {
    const flag = argv[i];
    if (flag === '--check') out.check = true;
    else if (flag === '--work') out.work = argv[i += 1] ?? usage('--work needs a directory');
    else if (flag === '--out') out.out = argv[i += 1] ?? usage('--out needs a file');
    else if (flag === '--points') out.points = Number(argv[i += 1]);
    else usage(`unknown argument ${flag}`);
  }
  if (out.work === null) usage('--work is required');
  if (!Number.isSafeInteger(out.points) || out.points < 1) usage('--points needs a positive whole number');
  return out;
}

const readJson = (file) => JSON.parse(fs.readFileSync(file, 'utf8'));

/** An exact non-negative decimal string, whatever JSON handed us. */
function exact(value, field) {
  if (typeof value === 'string' && /^(0|[1-9][0-9]*)$/.test(value)) return value;
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) return String(value);
  if (typeof value === 'bigint' && value >= 0n) return value.toString();
  throw new Error(`${field} is not an exact non-negative whole number: ${JSON.stringify(value)}`);
}

/**
 * One market's census chain, folded into a v3-shaped series body.
 *
 * The chain is per market by construction (`simlife_drive.MarketCensus`): the
 * conservation ledger has one Hoard, one aggregate and one Mint, and a delta
 * law that reads exactly one predecessor, so two markets sharing a chain would
 * compare one's Hoard against the other's. The newest file in the directory is
 * the whole chain, exactly as it is for the single-market capture.
 */
function marketSeries(marketId, censusDir, planned, keep) {
  const files = fs.readdirSync(censusDir).filter((name) => /^cycle-\d+\.json$/.test(name)).sort();
  if (files.length === 0) return null;
  const censusPath = path.join(censusDir, files[files.length - 1]);
  const observations = readJson(censusPath);
  if (!Array.isArray(observations) || observations.length === 0) return null;

  const newest = observations[observations.length - 1];
  const newestVerdicts = Array.isArray(newest.verdicts) ? newest.verdicts : [];
  const lawIds = newestVerdicts.map((verdict) => String(verdict.law));

  const all = observations.map((observation, index) => {
    const stage = String(observation.stage ?? '');
    const matched = CENSUS_STAGE.exec(stage);
    // The TICK is the x-axis, and it comes out of the census's own stage name
    // rather than from this script's loop counter: a chain that was resumed has
    // a gap in it, and a counter would silently close the gap by renumbering.
    const tick = matched === null ? index : Number(matched[2]);
    const verdicts = Array.isArray(observation.verdicts) ? observation.verdicts : [];
    const aligned = verdicts.length === lawIds.length
      && verdicts.every((verdict, cell) => String(verdict.law) === lawIds[cell]);
    return {
      // `cycle` keeps v3's name because the decoder and every chart already
      // read it; what it holds here is the world's tick, and `stage` says so
      // in words on every single point.
      cycle: tick,
      stage,
      slot: exact(observation.slot, `${marketId} observation ${index} slot`),
      recorded_at: null,
      supply: (observation.aggregate_supply ?? []).map(
        (atoms, cell) => exact(atoms, `${marketId} observation ${index} supply ${cell}`),
      ),
      position_totals: (observation.position_totals ?? []).map(
        (atoms, cell) => exact(atoms, `${marketId} observation ${index} position_totals ${cell}`),
      ),
      mint_supply: observation.mint_supply === undefined
        ? null
        : exact(observation.mint_supply, `${marketId} observation ${index} mint_supply`),
      payer_lamports: observation.payer_lamports === undefined
        ? null
        : exact(observation.payer_lamports, `${marketId} observation ${index} payer_lamports`),
      hoard_atoms: exact(observation.hoard_atoms, `${marketId} observation ${index} hoard_atoms`),
      tracked_collateral: exact(observation.tracked_collateral, `${marketId} observation ${index} tracked_collateral`),
      law_statuses: !aligned || lawIds.length === 0
        ? null
        : verdicts.map((verdict) => LAW_STATUS_CHARS[verdict.status] ?? 'i').join(''),
      checks_held: verdicts.filter((v) => v.status === 'holds').length,
      checks_broken: verdicts.filter((v) => v.status === 'violated').length,
      checks_inapplicable: verdicts.filter((v) => v.status === 'inapplicable').length,
    };
  });

  const kept = all.slice(-keep);
  const outcomeCount = kept.length === 0 ? 0 : kept[kept.length - 1].supply.length;
  for (const point of kept) {
    if (point.supply.length !== outcomeCount) {
      throw new Error(
        `${marketId} tick ${point.cycle} carries ${point.supply.length} outcomes and the newest `
        + `carries ${outcomeCount}; one market cannot change width mid-run`,
      );
    }
    if (point.position_totals.length !== 0 && point.position_totals.length !== outcomeCount) {
      throw new Error(`${marketId} tick ${point.cycle} position totals do not match its width`);
    }
  }
  // A market whose ticks do not ascend would draw a shape that never happened.
  // Duplicates are dropped rather than refused: a rerun re-observes the same
  // tick, which is a fact about the rerun and not a defect in the chain.
  const ordered = [];
  for (const point of kept) {
    if (ordered.length > 0 && point.cycle <= ordered[ordered.length - 1].cycle) continue;
    ordered.push(point);
  }

  const accounts = newest.accounts ?? {};
  const addressFor = (label) => accounts[label]?.address ?? null;
  const lamportsFor = (label) => (accounts[label] === undefined
    ? null
    : exact(accounts[label].lamports, `${marketId} accounts ${label} lamports`));

  const positions = Object.entries(newest.position_balances ?? {})
    .map(([label, claims]) => {
      const values = claims.map((atoms, cell) => exact(atoms, `${marketId} position ${label} ${cell}`));
      return {
        label,
        address: addressFor(label),
        lamports: lamportsFor(label),
        claims: values,
        total_claims: values.reduce((sum, atoms) => sum + BigInt(atoms), 0n).toString(),
      };
    })
    .sort((left, right) => (BigInt(right.total_claims) === BigInt(left.total_claims)
      ? left.label.localeCompare(right.label)
      : (BigInt(right.total_claims) > BigInt(left.total_claims) ? 1 : -1)));

  const collateralHolders = Object.entries(newest.token_atoms ?? {})
    .map(([label, atoms]) => ({ label, address: addressFor(label), atoms: exact(atoms, `${marketId} token_atoms ${label}`) }))
    .sort((left, right) => (BigInt(right.atoms) === BigInt(left.atoms)
      ? left.label.localeCompare(right.label)
      : (BigInt(right.atoms) > BigInt(left.atoms) ? 1 : -1)));

  return {
    market_id: marketId,
    // The world's own words for what kind of market this is. Editorial belongs
    // to the page; this is the archetype the generator drew, verbatim.
    archetype: planned?.archetype ?? null,
    basis: planned?.basis ?? null,
    destiny: planned?.destiny ?? null,
    deadline_slots: planned?.deadline_slots ?? null,
    personas: (planned?.participants ?? []).map((p) => p.persona),
    law_ids: lawIds,
    laws: newestVerdicts.map((verdict) => ({
      id: String(verdict.law),
      status: LAW_STATUS_CHARS[verdict.status] === undefined ? 'inapplicable' : verdict.status,
      detail: String(verdict.detail ?? ''),
    })),
    positions,
    collateral_holders: collateralHolders,
    claim_unit_atoms: null,
    settlement: null,
    outcome_count: outcomeCount,
    cycles_recorded: all.length,
    points_omitted_before: all.length - ordered.length,
    census_file: path.basename(censusPath),
    points: ordered,
  };
}

/**
 * What the world tried and could not do, grouped so a reader sees ONE sentence
 * per reason rather than four hundred copies of it.
 *
 * This block is the artifact's conscience. A world plans nine kinds of thing;
 * this substrate does one. Every other route is `unattempted` with the driver
 * that owns it named, and rolling those up here is what stops a page from
 * reading a census-only run as if it were a trading one.
 */
function unattemptedSummary(ledger) {
  const grouped = new Map();
  for (const entry of ledger.entries ?? []) {
    const outcome = entry.result?.outcome;
    if (outcome === 'executed') continue;
    const key = `${entry.route} ${outcome} ${entry.result?.detail ?? ''}`;
    const row = grouped.get(key) ?? { route: entry.route, outcome, reason: entry.result?.detail ?? '', count: 0 };
    row.count += 1;
    grouped.set(key, row);
  }
  return [...grouped.values()].sort((left, right) => (
    right.count - left.count || left.route.localeCompare(right.route)
  ));
}

/**
 * The run's own history, tick by tick: how many planned events ended each of
 * the four ways, and which routes did it.
 *
 * `not_done` above says WHAT could not happen and why; this says WHEN. They
 * answer different questions and a population page needs both -- a run that
 * founded four markets in its first six ticks and then only observed them for
 * twenty more is a different run from one that mutated throughout, and the
 * grouped summary cannot tell them apart.
 *
 * Census events are counted separately from mutations. A tick's census count is
 * simply how many markets were alive at it, and mixing that into the same bar
 * as the tick's foundings would bury four foundings under forty observations.
 */
function eventTimeline(ledger) {
  const byTick = new Map();
  for (const entry of ledger.entries ?? []) {
    const tick = entry.tick;
    const row = byTick.get(tick) ?? {
      tick,
      executed: 0, refused: 0, unattempted: 0, blocked: 0,
      mutations_executed: 0, mutations_refused: 0,
      census_executed: 0,
      routes: [],
    };
    const outcome = entry.result?.outcome;
    if (outcome in row) row[outcome] += 1;
    if (entry.route === 'census') {
      if (outcome === 'executed') row.census_executed += 1;
    } else {
      if (outcome === 'executed') row.mutations_executed += 1;
      if (outcome === 'refused') row.mutations_refused += 1;
      if (outcome === 'executed' || outcome === 'refused') {
        const label = `${entry.route}:${outcome}`;
        if (!row.routes.includes(label)) row.routes.push(label);
      }
    }
    byTick.set(tick, row);
  }
  return [...byTick.values()]
    .sort((left, right) => left.tick - right.tick)
    .map((row) => ({ ...row, routes: row.routes.sort() }));
}

/**
 * The world's own settling histogram, carried through unchanged when the plan
 * has one and rebuilt from nothing when it does not.
 *
 * Optional for the same reason the timeline is: a capture taken before this
 * existed is still a complete v4 document, and an absent block is an absent
 * block rather than a claim that nothing settled.
 */
/**
 * A run's spend record, with its lamport quantities rendered the way every
 * other atom-scale quantity in this document is: as exact decimal strings.
 *
 * A lamport ceiling can pass 2^53 and a double would round it, so the three
 * quantities go through `exact` while the observation COUNT stays a number.
 * Passing the Python object through unchanged published numbers where this
 * app's decoder wanted strings, and the decoder refused the whole document.
 *
 * `payers` is deliberately dropped: it is a map of live pubkeys to balances,
 * which is per-run operational detail rather than anything a page draws, and
 * the fewer addresses a published artifact carries the better.
 */
function spendRecord(body) {
  if (body === undefined || body === null) return null;
  const bounded = body.bounded === true;
  return {
    max_lamports_spent: body.max_lamports_spent === null || body.max_lamports_spent === undefined
      ? null
      : exact(body.max_lamports_spent, 'spend max_lamports_spent'),
    spent_lamports: exact(body.spent_lamports ?? 0, 'spend spent_lamports'),
    credited_lamports: exact(body.credited_lamports ?? 0, 'spend credited_lamports'),
    observations: Number(body.observations ?? 0),
    bounded,
  };
}


function outcomeSpread(world) {
  const body = world.outcome_spread;
  if (body === undefined || body === null) return null;
  // COUNTS ARE NUMBERS AND THE COORDINATE IS A STRING, and the split is not
  // cosmetic. `exact` renders a decimal STRING because atom-scale quantities do
  // not survive a double; a count of markets is small, and the decoder reads it
  // with a whole-number check that a string fails. Emitting both through
  // `exact` published a document this app's own decoder refuses -- caught by
  // running the emitter and feeding it back, which is the only thing that could
  // have caught it, because the unit fixture was hand-written to match the
  // decoder rather than the emitter.
  const whole = (value, field) => {
    const parsed = typeof value === 'string' ? Number(value) : value;
    if (!Number.isSafeInteger(parsed) || parsed < 0) {
      throw new Error(`${field} is not an exact non-negative count: ${JSON.stringify(value)}`);
    }
    return parsed;
  };
  const counts = (raw, field) => Object.fromEntries(
    Object.entries(raw ?? {}).map(([key, value]) => [String(key), whole(value, `${field} ${key}`)]),
  );
  return {
    resolving_markets: whole(body.resolving_markets, 'outcome_spread resolving_markets'),
    distinct_cells: whole(body.distinct_cells, 'outcome_spread distinct_cells'),
    counts: counts(body.counts, 'outcome_spread counts'),
    positioned_markets: whole(body.positioned_markets, 'outcome_spread positioned_markets'),
    position_counts: counts(body.position_counts, 'outcome_spread position_counts'),
    distinct_positions: whole(body.distinct_positions, 'outcome_spread distinct_positions'),
    heaviest_position_tenths: body.heaviest_position_tenths === null
      || body.heaviest_position_tenths === undefined
      ? null
      : whole(body.heaviest_position_tenths, 'outcome_spread heaviest_position_tenths'),
    heaviest_share_percent: whole(body.heaviest_share_percent, 'outcome_spread heaviest_share_percent'),
    degenerate_threshold_percent: whole(
      body.degenerate_threshold_percent, 'outcome_spread degenerate_threshold_percent',
    ),
    degenerate: body.degenerate === true,
    // Atom-scale: a Pyth coordinate is raw price atoms and a devnet feed's can
    // pass 2^53, so this one stays a string.
    coordinate_anchor: exact(body.coordinate_anchor ?? 0, 'outcome_spread coordinate_anchor'),
  };
}


function main() {
  const { work, points: keep, check, out } = args(process.argv.slice(2));

  const worldPath = path.join(work, 'world.json');
  const ledgerPath = path.join(work, 'ledger.json');
  for (const [file, schema] of [[worldPath, WORLD_SCHEMA], [ledgerPath, LEDGER_SCHEMA]]) {
    if (!fs.existsSync(file)) throw new Error(`no ${path.basename(file)} in ${work} — has a simlife run finished there?`);
    const body = readJson(file);
    if (body.schema !== schema) throw new Error(`${path.basename(file)} has another schema: ${body.schema}`);
  }
  const world = readJson(worldPath);
  const ledger = readJson(ledgerPath);
  if (world.plan_digest !== ledger.plan_digest) {
    throw new Error(
      'world.json and ledger.json describe different plans; this capture would be joining '
      + 'one run\'s markets to another run\'s events',
    );
  }

  const plannedById = new Map((world.markets ?? []).map((market) => [market.market_id, market]));
  const censusRoot = path.join(work, 'census');
  const observedIds = fs.existsSync(censusRoot)
    ? fs.readdirSync(censusRoot).filter((name) => fs.existsSync(path.join(censusRoot, name)))
    : [];

  const markets = [];
  for (const marketId of observedIds.sort()) {
    const series = marketSeries(marketId, path.join(censusRoot, marketId), plannedById.get(marketId), keep);
    // A directory with no census file in it is a market that was never
    // observed. It gets no entry, because a market with no points drawn as a
    // market with no points is a market drawn as a flat line at zero.
    if (series !== null) markets.push(series);
  }
  if (markets.length === 0) {
    throw new Error(`no market in ${censusRoot} has a census file; there is nothing to draw`);
  }

  // THE PRIMARY. The top level of a v4 document is one market, so every surface
  // written against v1/v2/v3 keeps drawing without knowing v4 exists. The one
  // chosen is the market with the most points -- the longest-observed, which is
  // the most informative single line the run has -- and ties break on the id so
  // the choice is stable across captures.
  const primary = [...markets].sort((left, right) => (
    right.points.length - left.points.length || left.market_id.localeCompare(right.market_id)
  ))[0];

  const substrate = ledger.substrate ?? {};
  const series = {
    ...primary,
    schema: SERIES_SCHEMA,
    captured_at: new Date().toISOString().replace(/\.\d{3}Z$/, '+00:00'),
    cluster: substrate.cluster ?? 'local',
    market: null,
    mode: 'finite',
    campaign: null,
    world: {
      seed: world.seed,
      plan_digest: world.plan_digest,
      spec: world.spec,
      substrate: {
        name: substrate.name ?? null,
        label: substrate.label ?? null,
        cluster: substrate.cluster ?? null,
        rpc_origin: substrate.rpc_origin ?? null,
        source_revision: substrate.source_revision ?? null,
        routes: substrate.routes ?? [],
        routes_absent: substrate.routes_absent ?? [],
        basis_kinds: substrate.basis_kinds ?? [],
        basis_kinds_absent: substrate.basis_kinds_absent ?? [],
        // What this run was ALLOWED to spend and what it did spend. A run with
        // no ceiling says so here rather than leaving a missing field to
        // interpret. It rides on the SUBSTRATE rather than beside it because
        // the payer whose lamports these are is the substrate's.
        spend: spendRecord(substrate.spend),
      },
      markets_planned: (world.markets ?? []).length,
      markets_observed: markets.length,
      markets_founded_by_this_run: ledger.markets_founded_by_this_run ?? [],
      markets_pre_founded: ledger.markets_pre_founded ?? [],
      tally: ledger.tally ?? {},
      // Every market the world drew, observed or not, in the world's own terms.
      // A reader who wants to know what the run was FOR reads this; a reader
      // who wants to know what happened reads `markets`.
      planned: (world.markets ?? []).map((market) => ({
        market_id: market.market_id,
        archetype: market.archetype,
        basis: market.basis,
        destiny: market.destiny,
        outcome_count: market.outcome_count,
        deadline_slots: market.deadline_slots,
        fee_basis_points: market.fee_basis_points,
        founding_collateral_atoms: String(market.founding_collateral_atoms),
        participants: (market.participants ?? []).map((p) => ({
          persona: p.persona, stake_atoms: String(p.stake_atoms), redeems: p.redeems,
        })),
        observed: markets.some((entry) => entry.market_id === market.market_id),
      })),
      not_done: unattemptedSummary(ledger),
      timeline: eventTimeline(ledger),
      // WHERE THE WORLD'S ANSWERS LANDED, and whether they landed anywhere at
      // all. A population that resolves every market into the same cell has one
      // measurement copied N times, and until 2026-08-31 every local world did
      // exactly that -- bands drawn in USD cents against an observation arriving
      // as raw price atoms. Carried so the page can say so rather than draw a
      // healthy-looking chart over it.
      outcome_spread: outcomeSpread(world),
    },
    markets,
  };

  const target = out === null ? path.join(APP, 'public', 'simlife-series.json') : path.resolve(out);
  const body = `${JSON.stringify(series, null, 2)}\n`;

  // Defense in depth, the same rule simulator-series.mjs keeps: publish.sh
  // refuses a subtree carrying the live endpoint key, and that refusal must
  // never be the first thing that notices.
  const keyFile = path.join(process.env.HOME ?? '', '.helius-key');
  const key = fs.existsSync(keyFile) ? fs.readFileSync(keyFile, 'utf8').trim() : '';
  if (key.length > 0 && body.includes(key)) {
    throw new Error('REFUSED: the capture would carry the live endpoint credential');
  }

  if (check) {
    const before = fs.existsSync(target) ? fs.readFileSync(target, 'utf8') : null;
    if (before === body) process.exit(0);
    console.error(`simlife-series: ${target} differs from the work directory`);
    process.exit(1);
  }
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, body);
  console.log(`simlife-series: wrote ${target} (${body.length} bytes, sha256 ${createHash('sha256').update(body).digest('hex').slice(0, 12)})`);
  console.log(`simlife-series: seed ${series.world.seed.preimage} (${series.world.seed.sha256.slice(0, 12)})`);
  console.log(`simlife-series: ${series.world.markets_observed} of ${series.world.markets_planned} planned markets observed; primary ${primary.market_id} (${primary.archetype}), ${primary.points.length} points`);
  for (const market of markets) {
    const moved = ['slot', 'hoard_atoms', 'tracked_collateral']
      .filter((field) => new Set(market.points.map((p) => p[field])).size > 1);
    console.log(
      `simlife-series:   ${market.market_id} ${market.archetype ?? '?'} `
      + `${market.outcome_count} cells, ${market.points.length} points, `
      + `moves: ${moved.join(', ') || 'nothing but the supply vector, if that'}`,
    );
  }
  const broken = markets.reduce((sum, market) => sum + market.points.reduce(
    (inner, point) => inner + (point.law_statuses ?? '').split('').filter((c) => c === 'v').length, 0,
  ), 0);
  console.log(`simlife-series: ${broken === 0 ? 'no law was violated on any market at any drawn tick' : `*** ${broken} LAW VIOLATIONS ***`}`);
  for (const row of series.world.not_done.slice(0, 6)) {
    console.log(`simlife-series: ${row.route} x${row.count} ${row.outcome}: ${row.reason.slice(0, 100)}`);
  }
}

try {
  main();
} catch (error) {
  console.error(`simlife-series: ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
}
