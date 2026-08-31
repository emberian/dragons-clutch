#!/usr/bin/env node
// Capture ONE local-validator campaign's own transcript as a file this site can serve.
//
//   node scripts/campaign-series.mjs --transcript <abs.json> \
//        --label "relayed-vertical success walk" \
//        --source-revision HEX40 --rpc-origin http://127.0.0.1:PORT/ [--check]
//
// WHY A SECOND SCRIPT AND A SECOND ARTIFACT. `simulator-series.mjs` captures a
// devnet CENSUS: a poller that signs nothing, re-reads the same market every
// twenty seconds, and reports the same quantities every time. That record's
// honest drawing is a flat line beside a moving heartbeat, and SIMVIZ's
// 2026-08-30 evidence note is the measurement that says so — 432 observations,
// exactly one field that moves.
//
// A CAMPAIGN is the other kind of record. It founds a market, publishes its
// source graph, funds and activates its resolution, drives it to a terminal
// answer through a real transport, and retires it — and the boundary between
// two of its stages is a place where quantities are SUPPOSED to move. The two
// records have different clusters, different x-axes and different claims. One
// file each, so that no merge can ever put a local founding under a devnet
// caption.
//
// WHAT IT IS NOT. Not a collector. It opens no RPC connection and observes no
// chain. Every number it writes was written by the campaign's own conservation
// ledger (`tools/gauntlet/journey/src/ledger.rs`, `ObservationV1`) or by the
// campaign's own transcript; this script projects them into the site's series
// schema and drops what a public page has no business carrying.
//
// EXACTNESS. Every quantity crosses as a decimal string, the way atoms already
// do everywhere in this app. Only the drawing turns them into floats, and only
// in the browser.

import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const SERIES_SCHEMA = 'dclutch-simulator-series-v3';

/** The census's status words, as the wire's one character each. */
const LAW_STATUS_CHARS = { holds: 'h', violated: 'v', inapplicable: 'i' };

/**
 * Transcript schemas this script knows how to read.
 *
 * Named rather than pattern-matched: a transcript whose schema is not on this
 * list may well have an `observations` array and mean something else by it,
 * and guessing is how one campaign's numbers end up drawn under another
 * campaign's caption.
 */
const KNOWN_TRANSCRIPTS = new Set([
  'dclutch-relayed-vertical-transcript-v1',
  'dclutch-journey-transcript-v1',
]);

function usage(message) {
  if (message !== undefined) console.error(`campaign-series: ${message}`);
  console.error('usage: node scripts/campaign-series.mjs --transcript <abs.json> --label <text> \\');
  console.error('         --source-revision <hex40> --rpc-origin <http://127.0.0.1:PORT/> \\');
  console.error('         [--evidence <abs.json>] [--walk <name>] [--check]');
  process.exit(2);
}

function args(argv) {
  const out = { check: false, walk: null };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    if (flag === '--check') { out.check = true; continue; }
    const value = argv[index + 1];
    if (value === undefined) usage(`${flag} needs a value`);
    index += 1;
    if (flag === '--transcript') out.transcript = value;
    else if (flag === '--label') out.label = value;
    else if (flag === '--source-revision') out.sourceRevision = value;
    else if (flag === '--rpc-origin') out.rpcOrigin = value;
    else if (flag === '--walk') out.walk = value;
    else if (flag === '--evidence') out.evidence = value;
    // A second output path so this script can be exercised against a transcript
    // that is not the one being published — the alternative is a dry run that
    // writes over the committed artifact and hopes the next run replaces it.
    else if (flag === '--out') out.out = value;
    else usage(`unknown flag ${flag}`);
  }
  for (const required of ['transcript', 'label', 'sourceRevision', 'rpcOrigin']) {
    if (out[required] === undefined) usage(`--${required.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)} is required`);
  }
  if (!/^[0-9a-f]{40}$/.test(out.sourceRevision)) usage('--source-revision must be 40 lowercase hex');
  // The whole point of this artifact is that it is a REHEARSAL. An origin that
  // is not loopback would be a different claim, and this script is not the
  // place to make it quietly.
  if (!/^http:\/\/127\.0\.0\.1:\d+\/?$/.test(out.rpcOrigin)) {
    usage('--rpc-origin must be a literal 127.0.0.1 loopback URL; this artifact is labelled local on every chart drawn from it');
  }
  return out;
}

/**
 * Read a transcript WITHOUT losing the top bits of its u64s.
 *
 * Measured, not theorised: the relayed vertical's `payer_lamports` at its
 * first boundary is 500000009955591100, which is larger than 2^53, and plain
 * `JSON.parse` silently hands back 500000009955591200 — a lamport figure off
 * by a hundred, in an artifact whose entire discipline is that quantities
 * cross exactly. Every other number in this pipeline is already a string for
 * exactly this reason; the transcript's are not, because Rust wrote them as
 * JSON numbers.
 *
 * So the reviver reads the RAW SOURCE TEXT of each number (`context.source`,
 * Node 21+) and keeps it as a string whenever the double cannot represent it.
 * Small numbers stay numbers, so nothing downstream has to change. On a
 * runtime with no `context`, this returns the double and `exact` below refuses
 * it loudly rather than rounding it into the artifact.
 */
function parseExactJson(text) {
  return JSON.parse(text, function reviver(_key, value, context) {
    if (typeof value !== 'number' || context === undefined || typeof context.source !== 'string') return value;
    if (!/^-?\d+$/.test(context.source)) return value;
    return Number.isSafeInteger(value) ? value : context.source;
  });
}

/** A u64 the transcript wrote as a number or a string, crossing as a string. */
function exact(value, field) {
  if (typeof value === 'string' && /^(0|[1-9][0-9]*)$/.test(value)) return value;
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) return String(value);
  if (typeof value === 'bigint' && value >= 0n) return value.toString();
  throw new Error(`${field} is not an exact non-negative integer: ${JSON.stringify(value)}`);
}

/**
 * The campaign's per-stage work, summed off the stages the transcript reports.
 *
 * Some transcripts state it per stage (the journey does); some do not (the
 * relayed vertical's stage rows carry prose and no counts). Inventing them is
 * not on: this returns null for a transcript that does not state them, and the
 * work is then measured off the evidence instead. A zero is a measurement; an
 * absent field is not.
 */
function stageWork(transcript) {
  const stages = Array.isArray(transcript.stages) ? transcript.stages : [];
  const counted = stages.filter((stage) => typeof stage.transactions === 'number');
  if (counted.length === 0) return null;
  return stages;
}

/**
 * THE VOLUME, measured off the campaign's own transactions by the slot each
 * one landed in.
 *
 * A boundary is a finalized slot the conservation ledger censused at, so the
 * transactions "belonging" to a boundary are exactly the ones that landed
 * after the previous boundary's slot and at or before this one's. That is a
 * partition — every transaction lands in exactly one bucket, none is counted
 * twice, and a boundary taken at the same slot as its predecessor honestly
 * gets none, because nothing happened between them.
 *
 * Transactions after the LAST boundary are reported separately rather than
 * folded into it: the campaign kept working after its final census, and
 * silently attributing that work to a boundary that could not have seen it
 * would make the last bar a lie.
 */
function volumeBySlot(evidence, boundarySlots) {
  const transactions = Array.isArray(evidence?.transactions) ? evidence.transactions : [];
  if (transactions.length === 0) return null;
  const buckets = boundarySlots.map(() => ({ transactions: 0, computeUnits: 0n, feeLamports: 0n }));
  let after = 0;
  for (const entry of transactions) {
    if (typeof entry.slot !== 'number') return null;
    const slot = BigInt(entry.slot);
    let cell = boundarySlots.findIndex((boundary) => slot <= BigInt(boundary));
    if (cell < 0) { after += 1; continue; }
    // A run whose boundaries repeat a slot puts the transaction in the FIRST
    // boundary that can have seen it, which is the only one that can.
    buckets[cell].transactions += 1;
    buckets[cell].computeUnits += BigInt(entry.compute_units_consumed ?? 0);
    buckets[cell].feeLamports += BigInt(entry.fee_lamports ?? 0);
  }
  return { buckets, after, total: transactions.length };
}

function main() {
  const options = args(process.argv.slice(2));
  const transcriptPath = path.resolve(options.transcript);
  const transcript = parseExactJson(fs.readFileSync(transcriptPath, 'utf8'));
  if (!KNOWN_TRANSCRIPTS.has(transcript.schema)) {
    throw new Error(`${transcriptPath} carries schema ${transcript.schema}, which this script does not know how to read`);
  }
  const observations = transcript.observations;
  if (!Array.isArray(observations) || observations.length === 0) {
    throw new Error(`${transcriptPath} holds no observations, so there is no series in it`);
  }

  // The law names come from the NEWEST observation and every other boundary is
  // held to them; a boundary that recorded a different set carries no verdict
  // string rather than one laid under the wrong names. The decoder in
  // lib/simulatorSeries.ts refuses a length mismatch outright, so a bug here
  // cannot reach a chart quietly.
  const newest = observations[observations.length - 1];
  const newestVerdicts = Array.isArray(newest.verdicts) ? newest.verdicts : [];
  const lawIds = newestVerdicts.map((verdict) => String(verdict.law));

  const stages = stageWork(transcript);
  const boundarySlots = observations.map((observation, index) => exact(observation.slot, `observation ${index} slot`));
  const evidence = options.evidence === undefined
    ? null
    : parseExactJson(fs.readFileSync(path.resolve(options.evidence), 'utf8'));
  const volume = evidence === null ? null : volumeBySlot(evidence, boundarySlots);
  const points = observations.map((observation, index) => {
    const verdicts = Array.isArray(observation.verdicts) ? observation.verdicts : [];
    const aligned = verdicts.length === lawIds.length
      && verdicts.every((verdict, cell) => String(verdict.law) === lawIds[cell]);
    const stage = stages === null ? null : stages[index];
    const positionTotals = Array.isArray(observation.position_totals) ? observation.position_totals : [];
    return {
      // A boundary's own name is its x-axis label. `stage` on the observation
      // is the ledger's word for the boundary it censused, which is exactly
      // the thing a reader is looking at.
      stage: typeof observation.stage === 'string' && observation.stage.length > 0 ? observation.stage : null,
      cycle: index + 1,
      slot: exact(observation.slot, `observation ${index} slot`),
      // A campaign boundary is not a wall-clock sample. The transcript records
      // no instant per observation, so this is null rather than a timestamp
      // reconstructed from the file's own mtime.
      recorded_at: null,
      supply: (observation.aggregate_supply ?? []).map((atoms, cell) => exact(atoms, `observation ${index} aggregate_supply ${cell}`)),
      position_totals: positionTotals.map((atoms, cell) => exact(atoms, `observation ${index} position_totals ${cell}`)),
      hoard_atoms: exact(observation.hoard_atoms, `observation ${index} hoard_atoms`),
      tracked_collateral: exact(observation.tracked_collateral, `observation ${index} tracked_collateral`),
      mint_supply: exact(observation.mint_supply, `observation ${index} mint_supply`),
      payer_lamports: exact(observation.payer_lamports, `observation ${index} payer_lamports`),
      // The stage's own counts when it kept them; otherwise the ones measured
      // off the evidence by slot. Never both, and never a guess.
      transactions: stage !== undefined && stage !== null && typeof stage.transactions === 'number'
        ? stage.transactions
        : (volume === null ? null : volume.buckets[index].transactions),
      compute_units: stage !== undefined && stage !== null && typeof stage.compute_units === 'number'
        ? exact(stage.compute_units, `stage ${index} compute_units`)
        : (volume === null ? null : volume.buckets[index].computeUnits.toString()),
      fee_lamports: volume === null ? null : volume.buckets[index].feeLamports.toString(),
      law_statuses: !aligned || lawIds.length === 0
        ? null
        : verdicts.map((verdict) => LAW_STATUS_CHARS[verdict.status] ?? 'i').join(''),
      checks_held: verdicts.filter((verdict) => verdict.status === 'holds').length,
      checks_broken: verdicts.filter((verdict) => verdict.status === 'violated').length,
      checks_inapplicable: verdicts.filter((verdict) => verdict.status === 'inapplicable').length,
    };
  });

  const outcomeCount = points[points.length - 1].supply.length;
  for (const point of points) {
    if (point.supply.length !== outcomeCount) {
      throw new Error(`boundary ${point.cycle} carries ${point.supply.length} outcomes and the newest carries ${outcomeCount}; this series would be drawing two different markets on one axis`);
    }
    if (point.position_totals.length !== 0 && point.position_totals.length !== outcomeCount) {
      throw new Error(`boundary ${point.cycle} carries ${point.position_totals.length} position totals against ${outcomeCount} outcomes`);
    }
  }

  // Who is holding what, as of the newest boundary only. The labels are the
  // CAMPAIGN'S, from its own conservation ledger, and they cross exactly as
  // written — this script does not decide that `hoard` means the market's
  // vault. Whatever a label means is said on the page, beside the number.
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
    .map((entry) => ({ ...entry, total_claims: entry.claims.reduce((sum, atoms) => sum + BigInt(atoms), 0n).toString() }))
    .sort((left, right) => (BigInt(right.total_claims) === BigInt(left.total_claims)
      ? left.label.localeCompare(right.label)
      : (BigInt(right.total_claims) > BigInt(left.total_claims) ? 1 : -1)));

  const collateralHolders = Object.entries(newest.token_atoms ?? {})
    .map(([label, atoms]) => ({ label, address: addressFor(label), atoms: exact(atoms, `token_atoms ${label}`) }))
    .sort((left, right) => (BigInt(right.atoms) === BigInt(left.atoms)
      ? left.label.localeCompare(right.label)
      : (BigInt(right.atoms) > BigInt(left.atoms) ? 1 : -1)));

  // The terminal answer, taken only from the campaign's own recorded selector.
  // A market that did not terminalize carries none, and no page may infer one
  // from a phase name.
  const detail = transcript.walk_detail ?? {};
  const settlement = typeof detail.certificate_selector === 'number'
    ? {
      selected_cell: detail.certificate_selector,
      failure_cell: typeof detail.failure_selector === 'number' ? detail.failure_selector : null,
      certificate: typeof detail.certificate === 'string' ? detail.certificate : null,
    }
    : null;

  // THE CLAIM UNIT: collateral atoms one claim of one outcome is worth. It is
  // the price primitive — without it a claim count is a count of nothing in
  // particular — so it is taken only from a place that STATES it, never from a
  // ratio that happens to divide.
  //
  // The journey's transcript carries the field outright. The relayed
  // vertical's does not, but its conservation ledger writes L4's arithmetic in
  // words at every boundary — `Hoard H >= worst outcome W x unit U = H` — and
  // that `U` is the ledger's own figure, read from the Registry's published
  // `ProductBasisV3.payout_scale`. Parsing that sentence is reading the
  // ledger; dividing the admitted principal by the largest supply would be
  // GUESSING, and on this campaign the two happen to agree, which is exactly
  // the coincidence that would make a wrong guess invisible.
  let claimUnitAtoms = null;
  if (typeof transcript.claim_unit_atoms === 'number' || typeof transcript.claim_unit_atoms === 'string') {
    claimUnitAtoms = exact(transcript.claim_unit_atoms, 'claim_unit_atoms');
  } else {
    // Every boundary must state the SAME unit. A campaign whose payout scale
    // moved mid-run is one whose per-cell figures cannot be stated as one
    // number, and that is worth refusing rather than averaging.
    const stated = new Set();
    for (const observation of observations) {
      const l4 = (Array.isArray(observation.verdicts) ? observation.verdicts : []).find((verdict) => verdict.law === 'L4');
      const matched = l4 === undefined ? null : /\bx unit (\d+) =/.exec(String(l4.detail ?? ''));
      if (matched !== null) stated.add(matched[1]);
    }
    if (stated.size > 1) {
      throw new Error(`the ledger states more than one claim unit across this run (${[...stated].join(', ')}); there is no single per-claim value to publish`);
    }
    if (stated.size === 1) claimUnitAtoms = [...stated][0];
  }

  const series = {
    schema: SERIES_SCHEMA,
    captured_at: new Date().toISOString().replace(/\.\d{3}Z$/, '+00:00'),
    campaign: {
      label: options.label,
      source_revision: options.sourceRevision,
      walk: options.walk ?? (typeof transcript.walk === 'string' ? transcript.walk : null),
      rpc_origin: options.rpcOrigin,
      transcript_file: path.basename(transcriptPath),
    },
    claim_unit_atoms: claimUnitAtoms,
    settlement,
    law_ids: lawIds,
    // The newest boundary's verdicts in full, sentences included. Those
    // sentences are the LEDGER'S and cross verbatim. A page may say what a law
    // is for; it may not restate what the law found.
    laws: newestVerdicts.map((verdict) => ({
      id: String(verdict.law),
      status: LAW_STATUS_CHARS[verdict.status] === undefined ? 'inapplicable' : verdict.status,
      detail: String(verdict.detail ?? ''),
    })),
    positions,
    collateral_holders: collateralHolders,
    // Not a knob. This artifact exists to describe a loopback rehearsal and
    // the caveat every chart carries is keyed on this word.
    cluster: 'local',
    market: typeof transcript.market === 'string' ? transcript.market : null,
    mode: 'finite',
    outcome_count: outcomeCount,
    cycles_recorded: points.length,
    points_omitted_before: 0,
    census_file: path.basename(transcriptPath),
    points,
  };

  const target = options.out === undefined
    ? path.join(path.dirname(new URL(import.meta.url).pathname), '..', 'public', 'campaign-series.json')
    : path.resolve(options.out);
  const body = `${JSON.stringify(series, null, 2)}\n`;
  if (options.check) {
    const existing = fs.existsSync(target) ? fs.readFileSync(target, 'utf8') : '';
    // `captured_at` moves every run by construction, so the check compares
    // everything else. A drift check that always fails is a check nobody runs.
    const strip = (text) => text.replace(/^\s*"captured_at":.*$/m, '');
    if (strip(existing) !== strip(body)) {
      console.error('campaign-series: public/campaign-series.json does not match this transcript');
      process.exit(1);
    }
    console.log('campaign-series: public/campaign-series.json matches this transcript');
    return;
  }
  fs.writeFileSync(target, body);
  console.log(`campaign-series: wrote ${target}`);
  console.log(`  ${points.length} boundaries · ${outcomeCount} outcomes · market ${series.market ?? 'unnamed'}`);
  console.log(`  claim unit ${claimUnitAtoms ?? 'not derivable'} · settlement ${settlement === null ? 'none' : `cell ${settlement.selected_cell}`}`);
  if (volume !== null) {
    console.log(`  ${volume.total} transactions in the evidence; ${volume.total - volume.after} land at or before a boundary and ${volume.after} after the last one`);
  }
}

main();
