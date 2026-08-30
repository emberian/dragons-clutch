#!/usr/bin/env node
// Capture the load simulator's own record as two files this site can serve.
//
//   node scripts/simulator-series.mjs [--work <dir>] [--points <n>] [--check]
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
// EXACTNESS. Every quantity crosses into the artifact as a decimal string, the
// way atoms already do everywhere in this app. Only the drawing turns them into
// floats, and only in the browser.

import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const SERIES_SCHEMA = 'dclutch-simulator-series-v1';
const STATUS_SCHEMA = 'dclutch-load-simulator-status-v1';
const CENSUS_STAGE = /^load-sim-cycle-(\d+)$/;

const HERE = path.dirname(new URL(import.meta.url).pathname);
const APP = path.resolve(HERE, '..');
const DEFAULT_WORK = '/private/tmp/dclutch-sim-devnet-market18';
/** The last N cycles are kept. Older points are counted, never silently lost. */
const DEFAULT_POINTS = 240;

function usage(message) {
  console.error(`simulator-series: ${message}`);
  console.error('usage: node scripts/simulator-series.mjs [--work <dir>] [--points <n>] [--check]');
  process.exit(2);
}

function args(argv) {
  const out = { work: DEFAULT_WORK, points: DEFAULT_POINTS, check: false };
  for (let i = 0; i < argv.length; i += 1) {
    const flag = argv[i];
    if (flag === '--check') out.check = true;
    else if (flag === '--work') out.work = argv[i += 1] ?? usage('--work needs a directory');
    else if (flag === '--points') out.points = Number(argv[i += 1]);
    else usage(`unknown argument ${flag}`);
  }
  if (!Number.isSafeInteger(out.points) || out.points < 1) usage('--points needs a positive whole number');
  return out;
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
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
  const { work, points: keep, check } = args(process.argv.slice(2));

  const statusPath = path.join(work, 'status.json');
  if (!fs.existsSync(statusPath)) throw new Error(`no status artifact at ${statusPath}`);
  const statusBytes = fs.readFileSync(statusPath);
  const status = JSON.parse(statusBytes.toString('utf8'));
  if (status.schema !== STATUS_SCHEMA) throw new Error(`status artifact has another schema: ${status.schema}`);

  const censusPath = newestCensus(work);
  const observations = readJson(censusPath);
  if (!Array.isArray(observations) || observations.length === 0) {
    throw new Error(`${censusPath} is not a non-empty observation array`);
  }
  const instants = journalInstants(work);

  const all = observations.map((observation, index) => {
    const stage = String(observation.stage ?? '');
    const matched = CENSUS_STAGE.exec(stage);
    const cycle = matched === null ? index + 1 : Number(matched[1]);
    const verdicts = Array.isArray(observation.verdicts) ? observation.verdicts : [];
    return {
      cycle,
      slot: exact(observation.slot, `observation ${index} slot`),
      recorded_at: instants.get(cycle) ?? null,
      supply: (observation.aggregate_supply ?? []).map((atoms, cell) => exact(atoms, `observation ${index} aggregate_supply ${cell}`)),
      hoard_atoms: exact(observation.hoard_atoms, `observation ${index} hoard_atoms`),
      tracked_collateral: exact(observation.tracked_collateral, `observation ${index} tracked_collateral`),
      // A law that does not apply at this boundary is neither held nor broken,
      // so it is counted apart rather than folded into either number.
      checks_held: verdicts.filter((verdict) => verdict.status === 'holds').length,
      checks_broken: verdicts.filter((verdict) => verdict.status === 'violated').length,
      checks_inapplicable: verdicts.filter((verdict) => verdict.status === 'inapplicable').length,
    };
  });

  const kept = all.slice(-keep);
  const outcomeCount = kept.length === 0 ? 0 : kept[kept.length - 1].supply.length;
  for (const point of kept) {
    if (point.supply.length !== outcomeCount) {
      throw new Error(`cycle ${point.cycle} carries ${point.supply.length} outcomes and the newest carries ${outcomeCount}; this series would be drawing two different markets on one axis`);
    }
  }

  const series = {
    schema: SERIES_SCHEMA,
    captured_at: new Date().toISOString().replace(/\.\d{3}Z$/, '+00:00'),
    cluster: status.cluster?.label ?? null,
    market: status.market?.address ?? null,
    mode: status.mode,
    outcome_count: outcomeCount,
    cycles_recorded: all.length,
    points_omitted_before: all.length - kept.length,
    census_file: path.basename(censusPath),
    points: kept,
  };

  const outputs = [
    { file: path.join(APP, 'public', 'simulator-series.json'), body: `${JSON.stringify(series, null, 2)}\n` },
    // The status artifact is copied through UNCHANGED. It already redacts its
    // own endpoint credential (tools/load-simulator/simcore.py redact_endpoint),
    // and a copy that edits it would be a second author of someone else's file.
    { file: path.join(APP, 'public', 'simulator-status.json'), body: statusBytes.toString('utf8') },
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

  let stale = 0;
  for (const { file, body } of outputs) {
    const before = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : null;
    if (check) {
      if (before !== body) {
        console.error(`simulator-series: ${path.relative(APP, file)} differs from the work directory`);
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
  console.log(`simulator-series: ${kept.length} of ${all.length} cycles kept, cycle ${first.cycle} to ${last.cycle}`);
  console.log(`simulator-series: slot ${first.slot} to ${last.slot}, ${outcomeCount} outcomes, ${status.trades?.landed ?? '?'} trades landed`);
  const moved = ['slot', 'hoard_atoms', 'tracked_collateral']
    .filter((field) => new Set(kept.map((point) => point[field])).size > 1);
  console.log(`simulator-series: fields that actually move across these cycles: ${moved.join(', ') || 'none but the supply vector, if that'}`);
}

try {
  main();
} catch (error) {
  console.error(`simulator-series: ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
}
