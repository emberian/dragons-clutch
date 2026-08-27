#!/usr/bin/env node
// Test the checked-release contract's one load-bearing prediction.
//
// `dclutch-release-tool loader-accounts` CONSTRUCTS Loader V3 Program and
// ProgramData account bytes offline from an ELF. Every checked release then
// carries the digests and widths of those constructed bytes, and the browser's
// activation plan refuses unless the accounts a chain actually holds match them
// exactly. Nothing had ever checked that the construction and the runtime agree
// — `docs/evidence/CHECKED_RELEASE_CANDIDATE_2026_08_26.md` names it as the
// cheapest untested rung on the ladder and FD2 closed saying the same.
//
// This compares, per role, the constructed bytes against the finalized account
// bytes on a chain the campaign deployed to, and prints a verdict per role plus
// the exact first divergence when there is one.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { createHash } from 'node:crypto';

import { Rpc } from './chain-witness.mjs';

function argument(name, fallback) {
  const index = process.argv.indexOf(`--${name}`);
  if (index < 0) {
    if (fallback === undefined) throw new Error(`missing required --${name}`);
    return fallback;
  }
  return process.argv[index + 1];
}

const run = argument('run');
const endpoint = argument('endpoint', 'http://127.0.0.1:21890/');
const work = argument('work', join(run, 'checked-release'));

const plan = JSON.parse(readFileSync(join(run, 'plan.json'), 'utf8'));
const rpc = new Rpc(endpoint);
const floor = await rpc.finalizedSlot();

const ROLES = ['core', 'claims', 'trading', 'resolution', 'custody', 'registry', 'rent'];
const planKey = (role) => (role === 'rent' ? 'rent_credit' : role);
const digest = (bytes) => createHash('sha256').update(Buffer.from(bytes)).digest('hex');

function firstDivergence(left, right) {
  const shared = Math.min(left.length, right.length);
  for (let index = 0; index < shared; index += 1) {
    if (left[index] !== right[index]) return index;
  }
  return left.length === right.length ? -1 : shared;
}

const rows = [];
for (const role of ROLES) {
  const pin = plan[planKey(role)];
  const constructedProgram = new Uint8Array(readFileSync(join(work, 'evidence', role, 'program-account.bin')));
  const constructedProgramData = new Uint8Array(readFileSync(join(work, 'evidence', role, 'programdata-account.bin')));
  const deployedProgram = await rpc.account(pin.program_id, floor);
  const deployedProgramData = await rpc.account(pin.programdata_id, floor);
  if (deployedProgram === null || deployedProgramData === null) {
    rows.push({ role, verdict: 'ABSENT', detail: 'the chain holds no account at one of the two addresses' });
    continue;
  }
  for (const [what, constructed, deployed] of [
    ['Program', constructedProgram, deployedProgram.data],
    ['ProgramData', constructedProgramData, deployedProgramData.data],
  ]) {
    const index = firstDivergence(constructed, deployed);
    if (index === -1) {
      rows.push({ role, what, verdict: 'IDENTICAL', bytes: constructed.length, detail: `sha256 ${digest(constructed).slice(0, 16)}…` });
      continue;
    }
    const window = (bytes) => Buffer.from(bytes.slice(Math.max(0, index - 4), index + 12)).toString('hex');
    rows.push({
      role,
      what,
      verdict: 'DIFFERS',
      bytes: `${constructed.length} vs ${deployed.length}`,
      detail: `first divergence at byte ${index}: constructed ${window(constructed)} vs deployed ${window(deployed)}`,
    });
  }
}

const identical = rows.filter((row) => row.verdict === 'IDENTICAL').length;
for (const row of rows) {
  process.stdout.write(`${row.role.padEnd(11)} ${String(row.what ?? '').padEnd(12)} ${row.verdict.padEnd(10)} ${String(row.bytes ?? '').padEnd(20)} ${row.detail}\n`);
}
process.stdout.write(`\n${identical} of ${rows.length} constructed Loader accounts are byte-identical to the deployed accounts at finalized slot ${floor}.\n`);
process.stdout.write(`endpoint ${endpoint}, run ${run}\n`);
process.exit(identical === rows.length ? 0 : 1);
