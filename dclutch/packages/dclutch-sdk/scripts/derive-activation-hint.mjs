/**
 * Generate the deployment manifest's activation-cache hint FROM THE CHAIN.
 *
 * `DEVNET_DEPLOYMENT_V1.activationCache` is a bootstrap hint, not an answer:
 * `openReleaseBoundSessionV1` follows past it when it has aged out. But a hint
 * a reader believes should still be true, and on 2026-08-29 it was four cohorts
 * stale in the shipped client — because the only way to move it was for a human
 * to notice, and nothing made them notice.
 *
 * So nothing here is typed by a human. This script asks the chain the same
 * question the client asks, using the SAME code the client uses — it imports
 * `discoverCurrentActivationCacheV1` rather than restating its rule — and then
 * either reports the drift or writes the answer into both manifest twins.
 *
 *   node scripts/derive-activation-hint.mjs            # report; exit 1 on drift
 *   node scripts/derive-activation-hint.mjs --write     # rewrite both twins
 *   node scripts/derive-activation-hint.mjs --endpoint <url>
 *
 * WHY IT IS NOT AN `abi:*` GENERATOR. Every other generator in this directory
 * is offline and deterministic, and `tools/release/final-generated-convergence.py`
 * runs the whole set from a clean tree and refuses any generated change outside
 * its owner list. This one reads a live cluster, so its output legitimately
 * differs between two runs of the same commit. It must never join that batch.
 * Its home is publish time: `tools/release/README.md` documents the call, and a
 * drift report there is informational, because a stale hint costs a reader
 * accuracy and costs a session nothing.
 *
 * WHY IT BUILDS A BUNDLE. The SDK ships TypeScript with no build step, and
 * Node's strip-only type stripping cannot load `lib/rpc.ts` (parameter
 * properties). esbuild is already present as a vitest dependency, so the script
 * bundles the two real modules into the package's own `node_modules/.cache` and
 * imports that. The alternative — re-implementing the cache walk in plain
 * JavaScript — would put a second author on the rule that decides which release
 * a client binds to, which is exactly the failure this module family exists to
 * end.
 */
import { build } from 'esbuild';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const sdkRoot = fileURLToPath(new URL('..', import.meta.url));
const repoRoot = fileURLToPath(new URL('../../..', import.meta.url));

/**
 * The manifest. ONE FILE, since `7170fd97` made deployment truth an SDK
 * semantic owner and reduced `apps/dclutch-web/lib/deployments.ts` to a two-line
 * `export * from '@dclutch/sdk/deployments'`.
 *
 * This was a two-entry twin list, and the shim carries none of the literals
 * `readManifest` looks for -- so the script threw
 * "does not carry the expected devnet registry, endpoint and activationCache
 * literals" before reaching the network, and the one tool that could notice a
 * stale hint had itself been stale since that commit. A generator that cannot
 * run is a generator whose output is hand-maintained by default.
 */
const MANIFEST_TWINS = Object.freeze([
  join(sdkRoot, 'lib', 'deployments.ts'),
]);

const argv = process.argv.slice(2);
const write = argv.includes('--write');
const endpointIndex = argv.indexOf('--endpoint');
const endpointOverride = endpointIndex === -1 ? null : argv[endpointIndex + 1];
if (endpointIndex !== -1 && (endpointOverride === undefined || endpointOverride.startsWith('--'))) {
  throw new Error('--endpoint needs a URL');
}

/**
 * The shipped manifest, read as text.
 *
 * Deliberately textual rather than imported: this script must be able to report
 * on and rewrite a manifest whose current value is wrong, and a rewrite that
 * went through the module loader would have to serialise the whole record back
 * out and would reformat everything around it.
 */
function readManifest(path) {
  const source = readFileSync(path, 'utf8');
  const registry = /registry: '([1-9A-HJ-NP-Za-km-z]+)'/.exec(source);
  const endpoint = /endpoint: '(https?:\/\/[^']+)'/.exec(source);
  const cache = /^(\s*)activationCache: '([1-9A-HJ-NP-Za-km-z]+)',$/m.exec(source);
  if (registry === null || endpoint === null || cache === null) {
    throw new Error(`${path} does not carry the expected devnet registry, endpoint and activationCache literals`);
  }
  return Object.freeze({ source, registry: registry[1], endpoint: endpoint[1], cache: cache[2] });
}

/** Bundle the real SDK modules so the discovery rule keeps its single author. */
async function loadSdk() {
  const cache = join(sdkRoot, 'node_modules', '.cache', 'dclutch-activation-hint');
  mkdirSync(cache, { recursive: true });
  const entry = join(cache, 'entry.ts');
  writeFileSync(entry, [
    `export { discoverCurrentActivationCacheV1 } from ${JSON.stringify(join(sdkRoot, 'lib', 'releaseIdentity.ts'))};`,
    `export { SolanaRpcClient } from ${JSON.stringify(join(sdkRoot, 'lib', 'rpc.ts'))};`,
    '',
  ].join('\n'));
  const outfile = join(cache, 'bundle.mjs');
  await build({
    entryPoints: [entry],
    outfile,
    bundle: true,
    format: 'esm',
    platform: 'node',
    target: 'node22',
    packages: 'external',
    logLevel: 'warning',
  });
  return import(`${pathToFileURL(outfile).href}?t=${Date.now()}`);
}

const manifests = MANIFEST_TWINS.map((path) => Object.freeze({ path, ...readManifest(path) }));
const [primary] = manifests;
const endpoint = endpointOverride ?? primary.endpoint;

const sdk = await loadSdk();
const client = new sdk.SolanaRpcClient(endpoint);
const identity = await sdk.discoverCurrentActivationCacheV1(client, primary.registry);

const roles = Object.entries(identity.roles)
  .map(([role, record]) => `${role} ${record.deploymentSlot}`)
  .join(', ');

process.stdout.write([
  `endpoint     ${endpoint}`,
  `registry     ${primary.registry}`,
  `current      ${identity.activationCache}`,
  `release set  ${identity.executionReleaseSetId}`,
  `observed at  finalized slot ${identity.observedSlot}`,
  `live slots   ${roles}`,
  '',
].join('\n'));

/**
 * The whole generated block: the provenance comment AND the address.
 *
 * The comment is regenerated with the address because a hint whose comment
 * still names an older reading is the same defect one layer down. Rewriting
 * the block whole also makes `--write` idempotent — it converges a manifest
 * whose address is already right but whose comment is not.
 */
const GENERATED_BLOCK_V1 = /(?:^[ \t]*\/\/[^\n]*\n)*^[ \t]*activationCache: '[1-9A-HJ-NP-Za-km-z]+',$/m;

/**
 * Deliberately NOT stamped with the slot the reading happened at.
 *
 * That slot advances every few hundred milliseconds, so stamping it would make
 * this generator report drift against its own last output and produce a diff on
 * every publish — a check that cries wolf gets turned off. What the block
 * records instead are facts about the ANSWER, which move only when a cohort
 * does: the cache address, the release set it selects, and the Core deployment
 * slot it pins.
 */
const block = [
  '  // Bootstrap hint, GENERATED — do not hand-edit. Regenerate with',
  '  // `node packages/dclutch-sdk/scripts/derive-activation-hint.mjs --write`.',
  '  //',
  '  // The one cache of those the Registry owns whose five pinned deployment',
  '  // slots equalled the five live ProgramData slots in a single reading.',
  `  // Release set ${identity.executionReleaseSetId},`,
  `  // pinning Core at deployment slot ${identity.roles.core.deploymentSlot}.`,
  '  // A session follows past this when it ages out; a reader cannot.',
  `  activationCache: '${identity.activationCache}',`,
].join('\n');

const drifted = manifests.filter((manifest) => {
  const match = GENERATED_BLOCK_V1.exec(manifest.source);
  if (match === null) throw new Error(`${manifest.path}: no activationCache block to generate`);
  return match[0] !== block;
});

if (drifted.length === 0) {
  process.stdout.write('both twins already carry this generated block; nothing to write\n');
  process.exit(0);
}

for (const manifest of drifted) {
  const why = manifest.cache === identity.activationCache ? 'comment is stale' : `ships ${manifest.cache}`;
  process.stdout.write(`DRIFT        ${relative(repoRoot, manifest.path)} — ${why}\n`);
}

if (!write) {
  process.stdout.write([
    '',
    'The shipped hint is not what the chain says. This does NOT break a session:',
    'openReleaseBoundSessionV1 discovers the current cache and binds to it, and',
    'reports source=discovered. It is wrong for a READER, so fix it:',
    '',
    '  node packages/dclutch-sdk/scripts/derive-activation-hint.mjs --write',
    '',
  ].join('\n'));
  process.exit(1);
}

for (const manifest of drifted) {
  writeFileSync(manifest.path, manifest.source.replace(GENERATED_BLOCK_V1, block));
  process.stdout.write(`WROTE        ${relative(repoRoot, manifest.path)}\n`);
}

process.stdout.write([
  '',
  'Both twins now carry the generated block. Two things still want a human:',
  '  - `apps/dclutch-web/lib/deployments.test.ts` pins the literal;',
  '  - a new cohort may want a new ABI table in `lib/releaseIdentity.ts`.',
  '',
].join('\n'));
