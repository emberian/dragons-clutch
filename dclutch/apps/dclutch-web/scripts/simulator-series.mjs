#!/usr/bin/env node
// Capture the load simulator's own record as two files this site can serve.
//
//   node scripts/simulator-series.mjs [--work <dir>] [--census <file>] \
//                                      [--points <n>] [--check]
//   node scripts/simulator-series.mjs --no-status --census <file> \
//                                      --cluster devnet --market <address>
//
// `--census` NAMES THE OBSERVATION ARRAY INSTEAD OF FINDING IT. The default is
// the newest `<work>/census/cycle-NNN.json`, which is right for a poller that
// numbers its own cycles. A cohort's resolution chain is censused by
// `ledger-census` directly, at boundaries a poller never drove -- `--prior`
// makes each file reload its predecessor and append, exactly as the poller's
// does -- so those files are the same `ObservationV1` array under a different
// name, and this flag is the only thing that stood between them and a chart.
// Everything downstream already handled a named boundary: `cycle` has been the
// census's own order rather than a number parsed out of a stage name since the
// chained-census fix, and `recorded_at` is null for any stage no journal
// covers.
//
// WHY THIS EXISTS AT ALL. The simulator writes its status and its journals into
// a work directory outside the repository, and the publish path is
// `git archive <commit>` — only committed bytes ever reach the host. So an
// artifact that is not committed is an artifact no reader will ever see, which
// is exactly why /pulse was dark on the live site for a whole day while the
// simulator ran healthily 24 seconds a cycle. Capturing is therefore an
// AUTHORING step, the same shape as scripts/og-cards.sh: run it, read what it
// says, commit the two files, publish.
//
// WHAT IT IS NOT. It is not a second collector. It opens no RPC connection and
// observes no chain. Every number it writes was written by the simulator or by
// the census the simulator runs; this script joins them and drops the fields a
// public page has no business carrying. The simulator owns those schemas:
//   tools/load-simulator/simcore.py     StatusWriter  (status.json)
//   tools/load-simulator/simcore.py     CycleJournal  (journal/*/cycle.json)
//   tools/gauntlet/journey/src/ledger.rs  ObservationV1 (census/*.json)
//
// THE ONE JOIN THAT MATTERS. A census file is not one observation — it is the
// whole array from cycle 1 up to the cycle that wrote it, because the census
// reloads its predecessor and re-serializes the chain. So the NEWEST census
// file alone is the complete series, and no history has to be accumulated by
// anyone. The journals supply the wall-clock instant per cycle, which the
// census does not carry, so the two are joined on the cycle number parsed from
// the observation's stage name.
//
// AND THE ONE THING THAT ARRAY IS NOT. It is not a count of the run. Carrying
// every observation forever cost the directory O(N²) — 28 MB by cycle 123, and
// on 2026-08-30 it filled the machine's data volume and killed the run — so
// the simulator now holds that array to a fixed window
// (tools/load-simulator/simcore.py CensusRetention). The newest file is still
// the whole series this script draws; it stops being the whole HISTORY. The
// run's true cycle count therefore comes from status.json, which has always
// known it, and `points_omitted_before` is measured against that rather than
// against a window reporting its own length back as the total.
//
// EXACTNESS. Every quantity crosses into the artifact as a decimal string, the
// way atoms already do everywhere in this app. Only the drawing turns them into
// floats, and only in the browser.

import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const SERIES_SCHEMA = 'dclutch-simulator-series-v2';
const STATUS_SCHEMA = 'dclutch-load-simulator-status-v1';
const CENSUS_STAGE = /^load-sim-cycle-(\d+)$/;

/**
 * v2 carries the conservation laws' NAMES, which v1 dropped.
 *
 * v1 reduced each cycle's verdicts to three integers. A count is the least
 * interesting true thing about a law: the census has always recorded WHICH law
 * (`L1`..`L7`) and a sentence saying what it checked, and a page that can say
 * "the Hoard still covers the worst outcome, at every one of 414 boundaries"
 * is saying something a count cannot. The counts stay — they are what the run
 * halts on — and the names now travel beside them.
 *
 * Per cycle the verdicts cross as ONE compact string, one character per law in
 * `law_ids` order, because this field is repeated on every point and the whole
 * artifact is downloaded by every reader of /pulse.
 */
const LAW_STATUS_CHARS = { holds: 'h', violated: 'v', inapplicable: 'i' };

const HERE = path.dirname(new URL(import.meta.url).pathname);
const APP = path.resolve(HERE, '..');
const DEFAULT_WORK = '/private/tmp/dclutch-sim-devnet-market18';
/** The last N cycles are kept. Older points are counted, never silently lost. */
const DEFAULT_POINTS = 240;

function usage(message) {
  console.error(`simulator-series: ${message}`);
  console.error('usage: node scripts/simulator-series.mjs [--work <dir>] [--census <file>] [--points <n>] [--check]');
  console.error('       node scripts/simulator-series.mjs --no-status --census <file> --cluster devnet --market <address> [--points <n>] [--check]');
  process.exit(2);
}

function args(argv) {
  const out = { work: DEFAULT_WORK, census: null, points: DEFAULT_POINTS, check: false, noStatus: false, cluster: null, market: null };
  for (let i = 0; i < argv.length; i += 1) {
    const flag = argv[i];
    if (flag === '--check') out.check = true;
    else if (flag === '--no-status') out.noStatus = true;
    else if (flag === '--work') out.work = argv[i += 1] ?? usage('--work needs a directory');
    else if (flag === '--census') out.census = argv[i += 1] ?? usage('--census needs a file');
    else if (flag === '--cluster') out.cluster = argv[i += 1] ?? usage('--cluster needs local or devnet');
    else if (flag === '--market') out.market = argv[i += 1] ?? usage('--market needs an address');
    else if (flag === '--points') out.points = Number(argv[i += 1]);
    else usage(`unknown argument ${flag}`);
  }
  if (!Number.isSafeInteger(out.points) || out.points < 1) usage('--points needs a positive whole number');
  if (out.noStatus) {
    if (out.census === null) usage('--no-status needs --census: with no status artifact there is no work directory to find a census in');
    if (out.cluster !== 'local' && out.cluster !== 'devnet') usage('--no-status needs --cluster local|devnet, which the status artifact would otherwise have said');
    if (out.market === null) usage('--no-status needs --market: a census observation does not record which Market it was bound to');
  } else if (out.cluster !== null || out.market !== null) {
    usage('--cluster and --market are for --no-status only; a status artifact is the author of both');
  }
  return out;
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

/**
 * An exact non-negative decimal string when the census recorded one, else null.
 *
 * The three fields this serves -- `mint_supply`, `payer_lamports` and
 * `position_totals` -- have been read by lib/simulatorSeries.ts since v3 and
 * written by scripts/campaign-series.mjs since v3, and this producer dropped
 * all three on the floor. The campaign one REQUIRES them, which is right for a
 * transcript it also authors; a poller census is older than the fields, so an
 * absent one here is a capture taken before the census recorded them and is a
 * true thing to say rather than a refusal.
 */
function optional(value, field) {
  return value === undefined || value === null ? null : exact(value, field);
}

/** An exact non-negative decimal string, whatever JSON handed us. */
function exact(value, field) {
  if (typeof value === 'string' && /^(0|[1-9][0-9]*)$/.test(value)) return value;
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) return String(value);
  if (typeof value === 'bigint' && value >= 0n) return value.toString();
  throw new Error(`${field} is not an exact non-negative whole number: ${JSON.stringify(value)}`);
}

/** The newest census file: the one that carries the whole series. */
function newestCensus(workDir) {
  const dir = path.join(workDir, 'census');
  if (!fs.existsSync(dir)) throw new Error(`no census directory at ${dir} — has the simulator run with a census configured?`);
  const files = fs.readdirSync(dir).filter((name) => /^cycle-\d+\.json$/.test(name)).sort();
  if (files.length === 0) throw new Error(`no census files in ${dir}`);
  return path.join(dir, files[files.length - 1]);
}

/** cycle number -> the instant the simulator recorded that cycle. */
function journalInstants(workDir) {
  const dir = path.join(workDir, 'journal');
  const instants = new Map();
  if (!fs.existsSync(dir)) return instants;
  for (const entry of fs.readdirSync(dir).sort()) {
    const file = path.join(dir, entry, 'cycle.json');
    if (!fs.existsSync(file)) continue;
    const body = readJson(file);
    if (typeof body.cycle === 'number' && typeof body.recorded_at === 'string') {
      instants.set(body.cycle, body.recorded_at);
    }
  }
  return instants;
}

function main() {
  const { work, census, points: keep, check, noStatus, cluster: clusterFlag, market: marketFlag } = args(process.argv.slice(2));

  /**
   * A CENSUS CHAIN NOBODY POLLED HAS NO STATUS, and pretending otherwise is how
   * /pulse published a dead market.
   *
   * `--census` already admits that the observation array may come from
   * `ledger-census` run at boundaries no poller drove. The status requirement
   * was the half of that admission nobody finished: with no simulator there is
   * no `status.json`, and the nearest one belongs to a run against a DIFFERENT
   * cohort's market. Copying it through would have put a closed cohort's
   * address in `series.market` and a halted run's heartbeat on the page.
   *
   * So `--no-status` writes the series ALONE and takes the two fields the
   * status supplied as arguments. The third, `mode`, is not an argument: a
   * census chain is `finite` by construction — it has exactly the boundaries
   * somebody took — and there is no run to keep going.
   */
  const statusPath = path.join(work, 'status.json');
  if (!noStatus && !fs.existsSync(statusPath)) throw new Error(`no status artifact at ${statusPath}`);
  const statusBytes = noStatus ? null : fs.readFileSync(statusPath);
  const status = noStatus ? null : JSON.parse(statusBytes.toString('utf8'));
  if (status !== null && status.schema !== STATUS_SCHEMA) throw new Error(`status artifact has another schema: ${status.schema}`);

  // A named census must still be a census: the flag chooses which file, never
  // what shape it has to be, and the array check below is the same one.
  const censusPath = census === null ? newestCensus(work) : path.resolve(census);
  if (!fs.existsSync(censusPath)) throw new Error(`no census artifact at ${censusPath}`);
  const observations = readJson(censusPath);
  if (!Array.isArray(observations) || observations.length === 0) {
    throw new Error(`${censusPath} is not a non-empty observation array`);
  }
  const instants = noStatus ? new Map() : journalInstants(work);

  // The law names come from the NEWEST observation and every other cycle is
  // held to them. A cycle that recorded a different set is a cycle whose
  // verdicts cannot be laid under these names without misattributing one law's
  // result to another, so it carries no verdict string rather than a shifted
  // one — and the decoder in lib/simulatorSeries.ts refuses a length mismatch
  // outright, so a bug here cannot reach a chart quietly.
  const newestVerdicts = Array.isArray(observations[observations.length - 1].verdicts)
    ? observations[observations.length - 1].verdicts
    : [];
  const lawIds = newestVerdicts.map((verdict) => String(verdict.law));
  let cyclesWithoutLaws = 0;

  /**
   * A journal instant is attributed only to a stage name that occurs ONCE.
   *
   * The journal is keyed by the poller's cycle number, and a census that
   * chains two runs contains `load-sim-cycle-000001` twice — one of them from
   * a run whose journal is not in this work directory. Handing both the same
   * instant made the cadence line read zero seconds across four thousand
   * slots, which is not a slow reading, it is a false one. An unattributable
   * boundary carries no instant, and the cadence line is drawn only when every
   * interval on it was measured.
   */
  const stageOccurrences = new Map();
  for (const observation of observations) {
    const stage = String(observation.stage ?? '');
    stageOccurrences.set(stage, (stageOccurrences.get(stage) ?? 0) + 1);
  }

  const all = observations.map((observation, index) => {
    const stage = String(observation.stage ?? '');
    const matched = CENSUS_STAGE.exec(stage);
    const pollerCycle = matched === null ? null : Number(matched[1]);
    const verdicts = Array.isArray(observation.verdicts) ? observation.verdicts : [];
    const aligned = verdicts.length === lawIds.length
      && verdicts.every((verdict, cell) => String(verdict.law) === lawIds[cell]);
    if (!aligned && lawIds.length > 0) cyclesWithoutLaws += 1;
    return {
      law_statuses: !aligned || lawIds.length === 0
        ? null
        : verdicts.map((verdict) => LAW_STATUS_CHARS[verdict.status] ?? 'i').join(''),
      // The boundary's OWN NAME, verbatim, so the axis can say what happened
      // there rather than counting. `lib/simulatorSeries.ts` has decoded this
      // field since v3 and no producer had ever written it, so every pulse
      // axis read `cycle N` for a record that knew better.
      stage: stage.length === 0 ? null : stage,
      slot: exact(observation.slot, `observation ${index} slot`),
      recorded_at: pollerCycle === null || stageOccurrences.get(stage) !== 1 ? null : instants.get(pollerCycle) ?? null,
      supply: (observation.aggregate_supply ?? []).map((atoms, cell) => exact(atoms, `observation ${index} aggregate_supply ${cell}`)),
      // What the POSITIONS hold, against what the aggregate says was issued.
      // L3 is exactly the comparison of these two, and until now the artifact
      // carried only one side of it.
      position_totals: (Array.isArray(observation.position_totals) ? observation.position_totals : [])
        .map((atoms, cell) => exact(atoms, `observation ${index} position_totals ${cell}`)),
      // THE MARKET'S PHASE, which decides whether a law applies at all.
      // `ledger-census --market` reads it off the chain and L4 retires at
      // Terminal on it; the producer dropped it, so /pulse could not tell a
      // paid market from a broken one and its whole rule for the last drawn
      // point was written as if every law applied at every phase. Absent
      // wherever no Market was bound, which is what a null means here.
      market_phase: typeof observation.market_phase === 'string' && observation.market_phase !== ''
        ? observation.market_phase
        : null,
      hoard_atoms: exact(observation.hoard_atoms, `observation ${index} hoard_atoms`),
      tracked_collateral: exact(observation.tracked_collateral, `observation ${index} tracked_collateral`),
      // The Mint's whole supply, which is what L1 compares the tracked total
      // against, and the fee payer's balance, which is the only quantity in a
      // census-only record that a run can make move by itself.
      mint_supply: optional(observation.mint_supply, `observation ${index} mint_supply`),
      payer_lamports: optional(observation.payer_lamports, `observation ${index} payer_lamports`),
      // A law that does not apply at this boundary is neither held nor broken,
      // so it is counted apart rather than folded into either number.
      checks_held: verdicts.filter((verdict) => verdict.status === 'holds').length,
      checks_broken: verdicts.filter((verdict) => verdict.status === 'violated').length,
      checks_inapplicable: verdicts.filter((verdict) => verdict.status === 'inapplicable').length,
    };
  });

  const window = all.slice(-keep);
  // The run's own count of itself, when it is at least what the census still
  // holds. A windowed census can only ever UNDERSTATE how many cycles ran, so
  // the larger of the two is the honest number and never an invented one.
  const cyclesRun = typeof status?.cycles?.run === 'number' && Number.isSafeInteger(status.cycles.run)
    ? Math.max(status.cycles.run, all.length)
    : all.length;

  /**
   * THE X-AXIS IS THE CENSUS'S OWN ORDER, not a number parsed out of a name.
   *
   * `cycle` used to be the integer inside the stage name `load-sim-cycle-NNN`,
   * falling back to the array position when a stage did not match. That is
   * exactly right for one continuous poller run and WRONG the moment a census
   * chains boundaries from more than one run: cohort-13's record runs
   * `load-sim-cycle-000001`, `load-sim-cycle-000002`, `load-sim-cycle-000001`
   * (a second run's first cycle) and `cohort13-post-fee-settlement` (a
   * boundary no poller drove at all), which numbered 1, 2, 1, 4 — and
   * `seriesBody` in lib/simulatorSeries.ts REFUSES a series whose points do
   * not ascend, so the producer could publish an artifact its own reader
   * rejects and /pulse would have shown "it did not decode".
   *
   * A census array is ordered by construction: each file reloads its
   * predecessor and appends. So the honest ordinal is the position in that
   * record, offset by whatever the window dropped — which is identical to the
   * old number for every continuous run, and is defined for chained ones. The
   * boundary's identity travels beside it in `stage`.
   */
  const omittedBefore = cyclesRun - window.length;
  const kept = window.map((point, index) => ({ ...point, cycle: omittedBefore + index + 1 }));
  const outcomeCount = kept.length === 0 ? 0 : kept[kept.length - 1].supply.length;
  for (const point of kept) {
    if (point.supply.length !== outcomeCount) {
      throw new Error(`boundary ${point.stage ?? point.cycle} carries ${point.supply.length} outcomes and the newest carries ${outcomeCount}; this series would be drawing two different markets on one axis`);
    }
    // Refused here as well as in the decoder, so a bad width is a producer
    // error naming its boundary rather than a browser error naming a point.
    if (point.position_totals.length !== 0 && point.position_totals.length !== outcomeCount) {
      throw new Error(`boundary ${point.stage ?? point.cycle} carries ${point.position_totals.length} position totals against ${outcomeCount} outcomes`);
    }
  }

  // Who is holding what, as of the newest observation only.
  //
  // The labels are the OPERATOR'S, supplied in the run's census config, and
  // they cross into the artifact exactly as written — this script does not get
  // to decide that `hoard` means the market's vault or that `p1` is a person.
  // Whatever a label means is said on the page, beside the number, where a
  // reader can see it is a gloss rather than a chain fact.
  const newest = observations[observations.length - 1];
  const accounts = newest.accounts ?? {};
  const addressFor = (label) => (accounts[label]?.address ?? null);
  const lamportsFor = (label) => (accounts[label] === undefined ? null : exact(accounts[label].lamports, `accounts ${label} lamports`));

  const positions = Object.entries(newest.position_balances ?? {})
    .map(([label, claims]) => ({
      label,
      address: addressFor(label),
      lamports: lamportsFor(label),
      claims: claims.map((atoms, cell) => exact(atoms, `position ${label} claim ${cell}`)),
    }))
    .map((entry) => ({
      ...entry,
      total_claims: entry.claims.reduce((sum, atoms) => sum + BigInt(atoms), 0n).toString(),
    }))
    .sort((left, right) => (BigInt(right.total_claims) === BigInt(left.total_claims)
      ? left.label.localeCompare(right.label)
      : (BigInt(right.total_claims) > BigInt(left.total_claims) ? 1 : -1)));

  const collateralHolders = Object.entries(newest.token_atoms ?? {})
    .map(([label, atoms]) => ({
      label,
      address: addressFor(label),
      atoms: exact(atoms, `token_atoms ${label}`),
    }))
    .sort((left, right) => (BigInt(right.atoms) === BigInt(left.atoms)
      ? left.label.localeCompare(right.label)
      : (BigInt(right.atoms) > BigInt(left.atoms) ? 1 : -1)));

  const series = {
    schema: SERIES_SCHEMA,
    captured_at: new Date().toISOString().replace(/\.\d{3}Z$/, '+00:00'),
    law_ids: lawIds,
    // The newest cycle's verdicts in full, sentences included. Those sentences
    // are the CENSUS's — `tracked 1000000000 atoms across 4 accounts == Mint
    // supply 1000000000` — and they cross verbatim. A page may say what a law
    // is for; it may not restate what the law found.
    laws: newestVerdicts.map((verdict) => ({
      id: String(verdict.law),
      status: LAW_STATUS_CHARS[verdict.status] === undefined ? 'inapplicable' : verdict.status,
      detail: String(verdict.detail ?? ''),
    })),
    positions,
    collateral_holders: collateralHolders,
    cluster: noStatus ? clusterFlag : status.cluster?.label ?? null,
    market: noStatus ? marketFlag : status.market?.address ?? null,
    mode: noStatus ? 'finite' : status.mode,
    outcome_count: outcomeCount,
    cycles_recorded: cyclesRun,
    points_omitted_before: omittedBefore,
    census_file: path.basename(censusPath),
    points: kept,
  };

  const outputs = [
    { file: path.join(APP, 'public', 'simulator-series.json'), body: `${JSON.stringify(series, null, 2)}\n` },
    // The status artifact is copied through UNCHANGED. It already redacts its
    // own endpoint credential (tools/load-simulator/simcore.py redact_endpoint),
    // and a copy that edits it would be a second author of someone else's file.
    // Under `--no-status` there is no such file to be the second author of, and
    // this producer does not write one: an absent status is what /pulse renders
    // as "nothing is running", which is the true answer when nothing is.
    ...(noStatus ? [] : [{ file: path.join(APP, 'public', 'simulator-status.json'), body: statusBytes.toString('utf8') }]),
  ];

  // Defense in depth. publish.sh refuses a subtree carrying the live endpoint
  // key, and that refusal must never be the first thing that notices.
  const keyFile = path.join(process.env.HOME ?? '', '.helius-key');
  const key = fs.existsSync(keyFile) ? fs.readFileSync(keyFile, 'utf8').trim() : '';
  for (const { file, body } of outputs) {
    if (key.length > 0 && body.includes(key)) {
      throw new Error(`REFUSED: ${path.basename(file)} would carry the live endpoint credential`);
    }
  }

  /**
   * `--check` COULD NOT PASS, and had not been able to since it was written.
   *
   * The series document stamps `captured_at` with the instant it was built, so
   * a byte comparison against a committed artifact compares "now" with "then"
   * and reports a difference on every run. A verify that can only ever be red
   * has exactly as much authority as one that can only ever be green: nobody
   * can act on it, and a REAL divergence -- the work directory having moved on
   * -- is indistinguishable from the clock having ticked.
   *
   * So the capture instant is normalised out of both sides, and the check says
   * that it did. Everything else, including every quantity and every boundary
   * name, is still compared byte for byte.
   */
  const CAPTURED_AT = /^(\s*"captured_at":\s*)"[^"]*"/m;
  const comparable = (text) => text.replace(CAPTURED_AT, '$1"<capture instant, excluded from --check>"');

  let stale = 0;
  for (const { file, body } of outputs) {
    const before = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : null;
    if (check) {
      if (before === null || comparable(before) !== comparable(body)) {
        console.error(`simulator-series: ${path.relative(APP, file)} differs from the work directory (the capture instant is not compared)`);
        stale += 1;
      }
      continue;
    }
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, body);
    console.log(`simulator-series: wrote ${path.relative(APP, file)} (${body.length} bytes, sha256 ${createHash('sha256').update(body).digest('hex').slice(0, 12)})`);
  }
  if (check) process.exit(stale > 0 ? 1 : 0);

  const first = kept[0];
  const last = kept[kept.length - 1];
  console.log(`simulator-series: ${kept.length} of ${cyclesRun} boundaries kept, ${first.stage ?? `cycle ${first.cycle}`} to ${last.stage ?? `cycle ${last.cycle}`}`);
  console.log(`simulator-series: slot ${first.slot} to ${last.slot}, ${outcomeCount} outcomes, ${status === null ? 'no status artifact, so no trade count' : `${status.trades?.landed ?? '?'} trades landed`}`);
  const moved = ['slot', 'hoard_atoms', 'tracked_collateral', 'mint_supply', 'payer_lamports']
    .filter((field) => new Set(kept.map((point) => point[field])).size > 1);
  console.log(`simulator-series: fields that actually move across these cycles: ${moved.join(', ') || 'none but the supply vector, if that'}`);
  console.log(`simulator-series: ${lawIds.length} laws named (${lawIds.join(' ')})${
    cyclesWithoutLaws === 0 ? '' : `, ${cyclesWithoutLaws} cycles recorded a different set and carry no verdict string`}`);
  // The one number worth printing loudly at the end of a capture: a broken law
  // is the only thing in this artifact that is an emergency rather than a
  // measurement, and the operator should not have to open the JSON to see it.
  const broken = kept.reduce((sum, point) => sum + (point.law_statuses ?? '').split('').filter((c) => c === 'v').length, 0);
  console.log(`simulator-series: ${broken === 0 ? 'no law was violated at any drawn cycle' : `*** ${broken} LAW VIOLATIONS ACROSS THE DRAWN CYCLES ***`}`);
}

try {
  main();
} catch (error) {
  console.error(`simulator-series: ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
}
