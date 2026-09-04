/**
 * Generate the devnet deployment manifest's ROWS from a sealed plan and the chain.
 *
 *   node scripts/derive-deployment-manifest.mjs <plan-seal.json>
 *   node scripts/derive-deployment-manifest.mjs <plan-seal.json> --write
 *   node scripts/derive-deployment-manifest.mjs <plan-seal.json> --endpoint <url>
 *
 * WHY THIS FILE EXISTS AT ALL. Cohorts 12, 14 and 15 each moved these rows, and
 * each time the derivation that produced them was performed in a scratch
 * directory and thrown away -- so the next cohort's lane re-wrote it, and the
 * shipped table's only durable provenance was a commit message describing a
 * script nobody could run. `0f1d75b27` and the second C-16 walk are the same
 * defect twice: the browser shipped a CLOSED cohort because moving the rows was
 * a human errand rather than a command.
 *
 * WHAT IS DERIVED, AND FROM WHERE:
 *
 * - the seven program ids are the `program_id` each role carries in the SEALED
 *   plan (`dclutch-local-successor-infrastructure-plan-v3`) -- the same document
 *   the deploy itself was driven from. Not typed, and not read out of an
 *   evidence markdown file, which is a transcription of the plan and not a
 *   second source.
 * - the ProgramData address beside each id is READ: the 32 bytes the Program
 *   account itself names at offset 4. It is NOT derived from the program id,
 *   even though it could be -- a derivation cannot tell a live cohort from a
 *   dead one, and this table's whole job is to know the difference.
 * - the deployment slot is the u64 at offset 4 of that ProgramData account's own
 *   Loader-v3 header, read finalized.
 *
 * IT REFUSES A CLOSED COHORT BY CONSTRUCTION. `solana program close` deletes the
 * ProgramData account and leaves the 36-byte Program stub behind -- executable,
 * loader-owned, still naming the address it used to have -- so every question
 * asked of the stub is answered identically by a live cohort and a dead one.
 * This script asks the account that holds the code, and emits no row at all when
 * any role's is vacant. A closed cohort cannot be written into the manifest.
 *
 * WHY IT IS NOT AN `abi:*` GENERATOR, for the same reason
 * `derive-activation-hint.mjs` is not: it reads a live cluster, so two runs of
 * one commit legitimately differ, and `tools/release/final-generated-convergence.py`
 * must never see it. Its home is a cohort boundary.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = fileURLToPath(new URL('../../..', import.meta.url));
const manifestPath = fileURLToPath(new URL('../lib/deployments.ts', import.meta.url));

/** The manifest's role order, and the plan's key for each role. */
const ROLES = Object.freeze(['registry', 'rent', 'custody', 'resolution', 'claims', 'trading', 'core']);
const PLAN_KEY = Object.freeze({
  registry: 'registry', rent: 'rent_credit', custody: 'custody',
  resolution: 'resolution', claims: 'claims', trading: 'trading', core: 'core',
});
const PLAN_SCHEMA = 'dclutch-local-successor-infrastructure-plan-v3';
const UPGRADEABLE_LOADER = 'BPFLoaderUpgradeab1e11111111111111111111111';
/** Loader-v3 account state tags: 2 is a Program stub, 3 is the ProgramData. */
const LOADER_STATE_PROGRAM = 2;
const LOADER_STATE_PROGRAM_DATA = 3;
/** The Loader-v3 ProgramData header: tag, deployment slot, authority option. */
const PROGRAM_DATA_HEADER_BYTES = 45;

const argv = process.argv.slice(2);
const write = argv.includes('--write');
const endpointIndex = argv.indexOf('--endpoint');
const endpointOverride = endpointIndex === -1 ? null : argv[endpointIndex + 1];
if (endpointIndex !== -1 && (endpointOverride === undefined || endpointOverride.startsWith('--'))) {
  throw new Error('--endpoint needs a URL');
}
const planPath = argv.find((argument) => !argument.startsWith('--') && argument !== endpointOverride);
if (planPath === undefined) {
  throw new Error('usage: node scripts/derive-deployment-manifest.mjs <plan-seal.json> [--endpoint <url>] [--write]');
}

/**
 * The shipped manifest, read as TEXT.
 *
 * Deliberately textual, exactly as `derive-activation-hint.mjs` reads it: this
 * script must be able to report on and rewrite a manifest whose current value is
 * wrong, and a rewrite through the module loader would have to serialise the
 * whole record back out and would reformat every line around it.
 */
const source = readFileSync(manifestPath, 'utf8');
const endpointMatch = /endpoint: '(https?:\/\/[^']+)'/.exec(source);
if (endpointMatch === null) throw new Error(`${manifestPath} carries no devnet endpoint literal`);
const endpoint = endpointOverride ?? endpointMatch[1];

const plan = JSON.parse(readFileSync(planPath, 'utf8'));
if (plan.schema !== PLAN_SCHEMA) {
  throw new Error(`${planPath} is a ${plan.schema ?? 'schemaless'} document; this script reads a ${PLAN_SCHEMA} sealed plan`);
}
const programs = {};
for (const role of ROLES) {
  const id = plan[PLAN_KEY[role]]?.program_id;
  if (typeof id !== 'string' || id === '') throw new Error(`the plan names no program_id for ${PLAN_KEY[role]}`);
  programs[role] = id;
}

async function rpc(method, params) {
  const response = await fetch(endpoint, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  });
  if (!response.ok) throw new Error(`${method}: HTTP ${response.status}`);
  const body = await response.json();
  if (body.error) throw new Error(`${method}: ${JSON.stringify(body.error)}`);
  return body.result;
}

/** base58, so this script depends on nothing the SDK does not already ship. */
const { PublicKey } = await import('@solana/web3.js');

const stubs = await rpc('getMultipleAccounts', [
  ROLES.map((role) => programs[role]),
  { encoding: 'base64', commitment: 'finalized' },
]);
const observedSlot = stubs.context.slot;
const programData = {};
for (const [index, role] of ROLES.entries()) {
  const account = stubs.value[index];
  if (account === null) throw new Error(`${role}: the Program account ${programs[role]} does not exist on ${endpoint}`);
  if (account.owner !== UPGRADEABLE_LOADER) throw new Error(`${role}: the Program account is owned by ${account.owner}, not the upgradeable loader`);
  if (account.executable !== true) throw new Error(`${role}: the Program account is not executable`);
  const bytes = Buffer.from(account.data[0], 'base64');
  if (bytes.length !== 36) throw new Error(`${role}: the Program account is ${bytes.length} bytes, not the 36 of a Loader-v3 Program`);
  if (bytes.readUInt32LE(0) !== LOADER_STATE_PROGRAM) throw new Error(`${role}: the Program account's Loader state tag is ${bytes.readUInt32LE(0)}, not ${LOADER_STATE_PROGRAM}`);
  programData[role] = new PublicKey(bytes.subarray(4)).toBase58();
}

const headers = await rpc('getMultipleAccounts', [
  ROLES.map((role) => programData[role]),
  { encoding: 'base64', commitment: 'finalized', dataSlice: { offset: 0, length: PROGRAM_DATA_HEADER_BYTES } },
]);
const evidence = {};
const closed = [];
for (const [index, role] of ROLES.entries()) {
  const account = headers.value[index];
  // THE ONE QUESTION A CLOSED COHORT ANSWERS DIFFERENTLY.
  if (account === null) { closed.push(`${role} (${programData[role]})`); continue; }
  if (account.owner !== UPGRADEABLE_LOADER) throw new Error(`${role}: the ProgramData account is owned by ${account.owner}`);
  if (account.executable !== false) throw new Error(`${role}: the ProgramData account is executable`);
  const bytes = Buffer.from(account.data[0], 'base64');
  if (bytes.readUInt32LE(0) !== LOADER_STATE_PROGRAM_DATA) throw new Error(`${role}: the ProgramData Loader state tag is ${bytes.readUInt32LE(0)}, not ${LOADER_STATE_PROGRAM_DATA}`);
  if (Number(account.space) <= PROGRAM_DATA_HEADER_BYTES) throw new Error(`${role}: the ProgramData account is ${account.space} bytes and carries no ELF`);
  evidence[role] = { programData: programData[role], deploymentSlot: bytes.readBigUInt64LE(4).toString() };
}
if (closed.length > 0) {
  process.stderr.write([
    'REFUSED: no row is emitted, because these roles have a VACANT ProgramData account:',
    ...closed.map((role) => `  ${role}`),
    '',
    'Their Program stubs are alive, executable and still name those addresses --',
    'that is what `solana program close` leaves behind. This cohort is CLOSED.',
    '',
  ].join('\n'));
  process.exit(1);
}

process.stdout.write([
  `endpoint     ${endpoint}`,
  `plan         ${planPath}`,
  `release set  ${plan.release_set_id ?? '(the plan names none)'}`,
  `read at      finalized slot ${observedSlot}`,
  '',
].join('\n'));

const programsBlock = [
  '  programs: Object.freeze({',
  ...ROLES.map((role) => `    ${role}: '${programs[role]}',`),
  '  }),',
].join('\n');
const evidenceBlock = [
  'export const DEVNET_PROGRAM_EVIDENCE_V1: Readonly<Record<ProtocolRoleV1, ProgramEvidenceV1>> = Object.freeze({',
  ...ROLES.map((role) => `  ${role}: Object.freeze({ programData: '${evidence[role].programData}', deploymentSlot: '${evidence[role].deploymentSlot}' }),`),
  '});',
].join('\n');

/**
 * The two regions this script owns, and NOTHING ELSE in the file.
 *
 * The prose around them -- which cohort, which commit, what was verified how --
 * is a human's to write and is deliberately not regenerated: a generator that
 * rewrote the provenance paragraph would produce a paragraph that says only what
 * a generator can know, which is the fact and never the finding.
 */
const devnetIndex = source.indexOf('export const DEVNET_DEPLOYMENT_V1');
if (devnetIndex === -1) throw new Error(`${manifestPath} declares no DEVNET_DEPLOYMENT_V1`);
const PROGRAMS_BLOCK = /^ {2}programs: Object\.freeze\(\{\n(?: {4}[a-z]+: '[1-9A-HJ-NP-Za-km-z]+',\n)+ {2}\}\),$/m;
const EVIDENCE_BLOCK = /^export const DEVNET_PROGRAM_EVIDENCE_V1[^\n]*Object\.freeze\(\{\n(?: {2}[a-z]+: Object\.freeze\(\{[^\n]*\}\),\n)+\}\);$/m;

const regions = [
  { name: 'DEVNET_DEPLOYMENT_V1.programs', pattern: PROGRAMS_BLOCK, from: devnetIndex, replacement: programsBlock },
  { name: 'DEVNET_PROGRAM_EVIDENCE_V1', pattern: EVIDENCE_BLOCK, from: 0, replacement: evidenceBlock },
];

let next = source;
const drifted = [];
for (const region of regions) {
  const tail = next.slice(region.from);
  const match = region.pattern.exec(tail);
  if (match === null) throw new Error(`${manifestPath}: could not locate the ${region.name} block to generate`);
  if (match[0] === region.replacement) continue;
  drifted.push(region.name);
  next = next.slice(0, region.from) + tail.slice(0, match.index) + region.replacement + tail.slice(match.index + match[0].length);
}

if (drifted.length === 0) {
  process.stdout.write('the manifest already carries these rows; nothing to write\n');
  process.exit(0);
}

for (const name of drifted) process.stdout.write(`DRIFT        ${name}\n`);

if (!write) {
  process.stdout.write([
    '',
    'The shipped manifest is not the cohort this plan and this cluster describe.',
    'Unlike the activation-cache hint, a session CANNOT follow past this: these',
    'are the program ids every account derivation and every owner check uses.',
    '',
    `  node ${relative(repoRoot, fileURLToPath(import.meta.url))} ${planPath} --write`,
    '',
  ].join('\n'));
  process.exit(1);
}

writeFileSync(manifestPath, next);
process.stdout.write([
  `WROTE        ${relative(repoRoot, manifestPath)}`,
  '',
  'Three things still want a human:',
  '  - the provenance prose above each block, which names the cohort and commit;',
  '  - `lib/deployments.test.ts` in BOTH trees, which pins these literals;',
  '  - `derive-activation-hint.mjs --write`, because the cache moved with them.',
  '',
].join('\n'));
