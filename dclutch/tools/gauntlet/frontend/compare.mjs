#!/usr/bin/env node
// Grade what the browser rendered against an independent decode of the chain.
//
// `expect.mjs` decoded the finalized accounts with `chain-witness.mjs`, which
// shares no code with `apps/dclutch-web`. `drive.mjs` harvested what a real
// Chromium actually painted. This compares them field by field and prints a
// table, so "the surface is correct" stops being a claim about a screenshot and
// becomes a claim about two independent decodes agreeing.
//
// Exit status is the verdict: nonzero if any expected fact is missing from the
// page, or present with a different value.

import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

function argument(name, fallback) {
  const index = process.argv.indexOf(`--${name}`);
  if (index < 0) {
    if (fallback === undefined) throw new Error(`missing required --${name}`);
    return fallback;
  }
  return process.argv[index + 1];
}

const expected = JSON.parse(readFileSync(argument('expected'), 'utf8'));
const rendered = JSON.parse(readFileSync(argument('rendered'), 'utf8'));
const outDir = argument('out-dir', '');

const rows = [];

function record(surface, fact, chain, browser, ok, note = '') {
  rows.push({ surface, fact, chain, browser, verdict: ok ? 'MATCH' : 'MISMATCH', note });
}

/** Find one rendered label/value pair, tile, or vector entry by its label. */
const SURFACES = { '/markets': 'discovery', '/markets/:address': 'detail', '/portfolio': 'portfolio' };

function lookup(surface, label) {
  const page = rendered[SURFACES[surface] ?? surface];
  if (page === undefined) return null;
  for (const source of [page.facts ?? [], page.tiles ?? [], page.outcomeVector ?? []]) {
    const hit = source.find((entry) => entry.label === label);
    if (hit !== undefined) return hit.value;
  }
  return null;
}

/**
 * A rendered value may be a short form or carry a copy affordance, so an exact
 * match is not always the right test. `contains` is the weaker check and is
 * used only where the surface deliberately abbreviates.
 */
function check(surface, fact, label, chainValue, mode = 'exact') {
  const browser = lookup(surface, label);
  if (browser === null) {
    record(surface, fact, chainValue, '(label not rendered)', false, `no rendered field labelled "${label}"`);
    return;
  }
  const ok = mode === 'exact' ? browser === String(chainValue) : browser.includes(String(chainValue));
  record(surface, fact, chainValue, browser, ok);
}

function checkText(surface, fact, chainValue, needle = null) {
  const body = rendered[SURFACES[surface] ?? surface]?.bodyText ?? '';
  const target = needle ?? String(chainValue);
  record(surface, fact, chainValue, body.includes(target) ? target : '(absent from the page)', body.includes(target));
}

const market = expected.market;
const liability = expected.economics;
const position = expected.founderPosition;

// ------------------------------------------------------------- /markets
{
  const note = rendered.enumeration?.status?.[0] ?? '';
  const claimed = /(\d+) carry the DCLTCOR2 Market header/.exec(note);
  const found = claimed === null ? null : Number(claimed[1]);
  record('/markets', 'Markets found by the Core program scan', String(expected.scan.coreMarketAddresses.length), String(found), found === expected.scan.coreMarketAddresses.length);
  checkText('/markets', 'the Open Market appears in the listing', market.address);
  checkText('/markets', 'phase chip', 'Open', 'Open');
  checkText('/markets', 'per-claim supply vector', liability.supplyAtoms.join(' · '));
  checkText('/markets', 'exact required backing', liability.requiredBackingAtoms);
  checkText('/markets', 'the Hoard is refused, not shown', 'namespaced by the founding action context');
}

// ----------------------------------------------------- /markets/:address
{
  check('/markets/:address', 'schema and version', 'Schema', 'DCLTCOR2 · version 2');
  check('/markets/:address', 'account width', 'Account width', `${market.accountBytes} bytes, exact`);
  check('/markets/:address', 'phase', 'Phase', market.phase);
  check('/markets/:address', 'founding readiness', 'Founding readiness', market.readiness);
  check('/markets/:address', 'generation', 'Generation', market.generation);
  check('/markets/:address', 'outstanding capabilities', 'Outstanding capabilities', market.outstandingCapabilities);
  check('/markets/:address', 'selected Registry program', 'Selected Registry program', market.registryProgram);
  check('/markets/:address', 'rent beneficiary', 'Rent beneficiary', market.rentBeneficiary);
  check('/markets/:address', 'Realm identity', 'Realm', market.realmId);
  check('/markets/:address', 'Product record identity', 'Product record', market.productRecordId);
  check('/markets/:address', 'Product instance identity', 'Product instance', market.productInstanceId);
  check('/markets/:address', 'resolution policy identity', 'Resolution policy', market.resolutionPolicyId);
  check('/markets/:address', 'capability manifest identity', 'Capability manifest', market.capabilityManifestId);
  check('/markets/:address', 'selected execution release set', 'Selected execution release set', market.selectedReleaseSetId);
  check('/markets/:address', 'Claims aggregate account', 'Claims aggregate account', expected.claimsAggregate.address);
  check('/markets/:address', 'liability basis identity', 'Liability basis', expected.claimsAggregate.liabilityBasisId);
  check('/markets/:address', 'exact required backing', 'Exact required backing', liability.requiredBackingAtoms);
  check('/markets/:address', 'claim count', 'Claim count', String(expected.claimsAggregate.claimCount));
  check('/markets/:address', 'aggregate revision', 'Aggregate revision', expected.claimsAggregate.revision);
  check('/markets/:address', 'Realm record address', 'Realm account', expected.realm.recordAddress);
  check('/markets/:address', 'collateral mint', 'Collateral mint', expected.realm.collateralMint, 'contains');
  check('/markets/:address', 'token program', 'Token program', expected.realm.tokenProgram);
  check('/markets/:address', 'collateral adapter release', 'Collateral adapter release ID', expected.realm.adapterReleaseId);
  check('/markets/:address', 'mint authority policy', 'Mint authority policy', expected.realm.mintAuthorityPolicy);
  check('/markets/:address', 'freeze authority policy', 'Freeze authority policy', expected.realm.freezeAuthorityPolicy);
  check('/markets/:address', 'manifest record address', 'Registry record', expected.capabilityManifest.recordAddress);
  check('/markets/:address', 'manifest content identity', 'Manifest content ID', expected.capabilityManifest.contentDigest);
  check('/markets/:address', 'manifest entry count', 'Entries', String(expected.capabilityManifest.entries.length));
  for (const entry of expected.capabilityManifest.entries) {
    checkText('/markets/:address', `capability ${entry.index} kind identity`, entry.kindId);
    checkText('/markets/:address', `capability ${entry.index} config identity`, entry.configId);
  }
  const vector = rendered.detail;
  const claims = (vector.outcomeVector ?? []).map((entry) => entry.value);
  record('/markets/:address', 'per-claim supply vector', liability.supplyAtoms.join(' · '), claims.join(' · '), claims.join(' · ') === liability.supplyAtoms.join(' · '));
  checkText('/markets/:address', 'the Hoard is refused, not shown', 'namespaced by the founding action context');
}

// ----------------------------------------------------------- /portfolio
{
  check('/portfolio', 'derived Claims aggregate', 'Derived Claims aggregate', expected.claimsAggregate.address.slice(0, 8), 'contains');
  check('/portfolio', 'derived Position address', 'Derived Position address', position.address.slice(0, 8), 'contains');
  check('/portfolio', 'Position revision', 'Position revision', position.revision);
  check('/portfolio', 'claim width', 'Claim width', String(position.claimCount));
  check('/portfolio', 'Market generation', 'Market generation', market.generation);
  const page = rendered.portfolio;
  const balances = (page.outcomeVector ?? []).map((entry) => entry.value);
  record('/portfolio', 'owned claim balances', position.balances.join(' · '), balances.join(' · '), balances.join(' · ') === position.balances.join(' · '));
  const merge = (page.tiles ?? []).find((tile) => tile.label === 'Complete sets mergeable');
  record('/portfolio', 'complete sets mergeable', liability.completeSetsAtoms, merge?.value ?? '(not rendered)', merge?.value === liability.completeSetsAtoms);
  checkText('/portfolio', 'one Position holds state', '1 of 1 derived Claims Position hold state');
}

// ------------------------------------------------------------------ report
const failures = rows.filter((row) => row.verdict !== 'MATCH');
const width = (key) => Math.max(key.length, ...rows.map((row) => String(row[key]).length));
const columns = ['surface', 'fact', 'chain', 'browser', 'verdict'];
const widths = Object.fromEntries(columns.map((column) => [column, Math.min(width(column), 68)]));
const line = (values) => `| ${columns.map((column, index) => String(values[index]).slice(0, widths[column]).padEnd(widths[column])).join(' | ')} |`;

const table = [
  line(columns.map((column) => column.toUpperCase())),
  `|${columns.map((column) => '-'.repeat(widths[column] + 2)).join('|')}|`,
  ...rows.map((row) => line(columns.map((column) => row[column]))),
].join('\n');

process.stdout.write(`${table}\n\n`);
process.stdout.write(`${rows.length - failures.length} of ${rows.length} checks MATCH.\n`);
if (failures.length > 0) {
  for (const failure of failures) process.stdout.write(`MISMATCH ${failure.surface} ${failure.fact}: chain ${failure.chain} vs browser ${failure.browser} ${failure.note}\n`);
}
if (outDir !== '') {
  writeFileSync(join(outDir, 'verification.json'), `${JSON.stringify({ rows, matched: rows.length - failures.length, total: rows.length }, null, 2)}\n`);
  writeFileSync(join(outDir, 'verification.md'), `${table}\n\n${rows.length - failures.length} of ${rows.length} checks MATCH.\n`);
}
process.exit(failures.length === 0 ? 0 : 1);
