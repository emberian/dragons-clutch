import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

function refuse(message) { throw new Error(`successor checkpoint generation refused: ${message}`); }
function parseArguments(values) {
  const result = { check: false };
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (value === '--check') { result.check = true; continue; }
    if (!value.startsWith('--') || index + 1 >= values.length) refuse(`invalid argument ${value}`);
    result[value.slice(2)] = values[index += 1];
  }
  for (const field of ['plan', 'evidence', 'profile', 'output']) if (typeof result[field] !== 'string') refuse(`missing --${field}`);
  return result;
}
function load(path, schema) {
  const bytes = readFileSync(path); const value = JSON.parse(bytes.toString('utf8'));
  if (value.schema !== schema) refuse(`${path} has schema ${String(value.schema)}`);
  return { value, sha256: createHash('sha256').update(bytes).digest('hex') };
}
async function rpc(endpoint, method, params = []) {
  const response = await fetch(endpoint, { method: 'POST', redirect: 'error', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }) });
  if (!response.ok) refuse(`${method} returned HTTP ${response.status}`);
  const payload = await response.json(); if (payload.error || !('result' in payload)) refuse(`${method} returned an invalid JSON-RPC result`); return payload.result;
}
function origin(name) {
  if (name === 'registry.activation') return 'transaction-created';
  if (name.includes('.certificate.') && !name.endsWith('.occupied')) return 'transaction-created';
  if (name.endsWith('.occupied')) return 'genesis-prepared-refusal-sentinel';
  if (name.includes('.state') || name.includes('.funding.')) return 'genesis-prepared-then-transaction-mutated';
  if (name.startsWith('record.') || name.includes('.market') || name.startsWith('loader.')) return 'genesis-prepared';
  return 'ephemeral-runtime';
}

const options = parseArguments(process.argv.slice(2));
const plan = load(resolve(options.plan), 'dclutch-local-successor-genesis-plan-v1');
const evidence = load(resolve(options.evidence), 'dclutch-local-successor-bootstrap-evidence-v1');
const profile = load(resolve(options.profile), 'dclutch-successor-local-validator-profile-v1');
const endpoint = new URL(profile.value.network.rpc_url);
if (endpoint.protocol !== 'http:' || endpoint.hostname !== '127.0.0.1' || endpoint.port !== '20890') refuse('profile RPC is not the fixed loopback successor endpoint');
if (evidence.value.rpc_url !== `${endpoint.toString()}` || evidence.value.plan_path !== resolve(options.plan)) refuse('evidence does not bind the supplied plan and profile RPC');
if (plan.value.registry.program_id !== evidence.value.programs.registry.program_id || plan.value.resolution.program_id !== evidence.value.programs.resolution.program_id) refuse('plan and evidence program identities differ');

const transactions = [...evidence.value.transactions.filter((entry) => !entry.label.startsWith('airdrop_')), evidence.value.rollback.transaction];
const transactionFixtures = [];
for (const expected of transactions) {
  const observed = await rpc(endpoint, 'getTransaction', [expected.signature, { commitment: 'finalized', encoding: 'json', maxSupportedTransactionVersion: 0 }]);
  if (observed !== null && (observed.slot !== expected.slot || observed.meta?.computeUnitsConsumed !== expected.compute_units_consumed)) refuse(`transaction ${expected.label} differs at RPC`);
  transactionFixtures.push({ captured: expected, rpc_transaction: observed === null ? null : { slot: observed.slot, blockTime: observed.blockTime, transaction: observed.transaction, meta: { err: observed.meta.err, fee: observed.meta.fee, computeUnitsConsumed: observed.meta.computeUnitsConsumed, loadedAddresses: observed.meta.loadedAddresses, logMessages: observed.meta.logMessages } } });
}

const fixtureNames = ['registry.activation', 'primary.certificate.success', 'lifecycle.state', 'lifecycle.funding.failure', 'rollback.certificate.failure.occupied'];
const fixtureAddresses = fixtureNames.map((name) => evidence.value.accounts[name].address);
const fixtureRead = await rpc(endpoint, 'getMultipleAccounts', [fixtureAddresses, { commitment: 'finalized', encoding: 'base64' }]);
if (!fixtureRead?.context || !Array.isArray(fixtureRead.value) || fixtureRead.value.length !== fixtureNames.length) refuse('representative account read is incomplete');
const accountFixtures = Object.fromEntries(fixtureNames.map((name, index) => [name, { address: fixtureAddresses[index], account: fixtureRead.value[index] }]));

const output = {
  schema: 'dclutch-web-local-successor-checkpoint-v1',
  provenance: { tool_commit: '98f8588c5219ffaf836419ecad72b13c6177e429', exact_source_commit: '30dc6cbb2929de00ffd41cd1a720e9390f3a94fe', plan_sha256: plan.sha256, evidence_sha256: evidence.sha256, profile_sha256: profile.sha256 },
  network: { rpc_url: endpoint.toString(), genesis_hash: await rpc(endpoint, 'getGenesisHash'), version: await rpc(endpoint, 'getVersion') },
  evidence: { evidence_class: evidence.value.evidence_class, checked_production_release_claimed: evidence.value.checked_production_release_claimed, captured_release_identity_claimed: evidence.value.captured_release_identity_claimed, genesis_fixture_boundary: evidence.value.genesis_fixture_boundary, rollback: { state_unchanged: evidence.value.rollback.state_unchanged, certificate_unchanged: evidence.value.rollback.certificate_unchanged, funding_unchanged: evidence.value.rollback.funding_unchanged, worker_unchanged: evidence.value.rollback.worker_unchanged, before: evidence.value.rollback.before, after: evidence.value.rollback.after } },
  programs: evidence.value.programs,
  scenarios: { primary: plan.value.primary, lifecycle: plan.value.lifecycle, rollback: plan.value.rollback },
  expected_accounts: Object.fromEntries(Object.entries(evidence.value.accounts).filter(([name]) => name !== 'ephemeral.worker').map(([name, value]) => [name, { ...value, origin: origin(name) }])),
  expected_transactions: transactions.map((transaction) => { const entry = { ...transaction }; delete entry.logs; return entry; }),
  parser_fixtures: { accounts: accountFixtures, transactions: transactionFixtures },
};
const rendered = `${JSON.stringify(output, null, 2)}\n`; const outputPath = resolve(options.output);
if (options.check) { if (readFileSync(outputPath, 'utf8') !== rendered) refuse('committed checkpoint differs from regenerated output'); }
else writeFileSync(outputPath, rendered);
