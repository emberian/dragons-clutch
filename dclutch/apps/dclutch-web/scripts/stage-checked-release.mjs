/**
 * Ingest a sealing driver's checked-release fragment into the public cut.
 *
 *   node scripts/stage-checked-release.mjs <fragment.json> --release-set HEX64
 *
 * The fragment is the driver's own output and is copied, never retyped: a
 * hand-entered 64-hex triple is indistinguishable from a mistyped one, and the
 * row it produces would be exactly as authoritative either way.
 *
 * `--release-set` is the execution release set the cut's Market actually
 * selects, and it must be READ OFF THE CHAIN: the market page prints it and
 * `inspectDirectTradeSpineV1` returns it as `releaseSetId`. It is required and
 * it is not taken from the fragment, because the fragment is the thing being
 * checked -- a fragment for another deployment's release set would otherwise
 * turn the trade spine's `release` wall off for a market the release was never
 * checked against, which is the exact failure the wall exists to prevent.
 *
 * SO DO NOT COPY IT OUT OF THE PRODUCER'S OWN OUTPUT. The sealing driver's
 * `prepare` puts the same string in its result JSON as `release_set_id` and
 * uses it as the fragment's single key, so pasting it from there is quicker
 * and silently destroys the guarantee: two independent proofs collapse into
 * one producer asserting the same value twice. The producer proves the row's
 * VALUE is what its artifact derives; this argument proves the row is ABOUT
 * the set the market selects. Only a chain read can supply the second.
 *
 * The fixture is replaced ATOMICALLY through a temporary file on the same
 * filesystem, and only after the result parses as a cut, so a refused stage
 * leaves the last accepted fixture byte-for-byte intact.
 */
import { execFileSync } from 'node:child_process';
import { readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';

const fixture = fileURLToPath(new URL('../../../packages/dclutch-sdk/fixtures/public-cut.devnet.json', import.meta.url));
const [fragmentPath, ...rest] = process.argv.slice(2);
const flag = rest.indexOf('--release-set');
const releaseSet = flag < 0 ? null : rest[flag + 1];
if (fragmentPath === undefined || releaseSet === undefined || releaseSet === null) {
  throw new Error('usage: node scripts/stage-checked-release.mjs <fragment.json> --release-set HEX64');
}

// The staging rules have ONE owner -- `lib/publicCutStaging.ts`, which is what
// the tests exercise -- and this script must not restate them. Node cannot
// import that module directly: it is TypeScript and it resolves the `@/` alias
// the app's bundler owns. So it is bundled here, with esbuild (already present
// for the dev server), into a temporary module beside the app so its
// `node_modules` resolve, imported, and deleted.
//
// This is the step that was missing when this script first shipped: it was
// written, its LIBRARY was tested, and the script itself had never been run
// once. It failed on its first real invocation with ERR_MODULE_NOT_FOUND.
const bundle = fileURLToPath(new URL('../.stage-checked-release.bundle.mjs', import.meta.url));
execFileSync(fileURLToPath(new URL('../node_modules/.bin/esbuild', import.meta.url)), [
  fileURLToPath(new URL('../lib/publicCutStaging.ts', import.meta.url)),
  '--bundle', '--format=esm', '--platform=node',
  `--alias:@=${fileURLToPath(new URL('..', import.meta.url)).replace(/\/$/, '')}`,
  '--external:@solana/web3.js', '--log-level=silent',
  `--outfile=${bundle}`,
], { stdio: ['ignore', 'ignore', 'inherit'] });
let staging;
try {
  staging = await import(pathToFileURL(bundle).href);
} finally {
  rmSync(bundle, { force: true });
}
const { parseCheckedReleaseFragmentV1, parsePublicDevnetCutV1, stageCheckedReleaseV1 } = staging;

const fragment = parseCheckedReleaseFragmentV1(JSON.parse(readFileSync(fragmentPath, 'utf8')));
const cut = parsePublicDevnetCutV1(JSON.parse(readFileSync(fixture, 'utf8')));
const staged = stageCheckedReleaseV1(cut, fragment, releaseSet);

const serialized = `${JSON.stringify(staged, null, 2)}\n`;
// Re-parse what is about to be written. A generator that validates its INPUT
// and not its output is a generator that can still emit a shape the consumer
// refuses.
parsePublicDevnetCutV1(JSON.parse(serialized));
const temporary = `${fixture}.staging`;
writeFileSync(temporary, serialized);
renameSync(temporary, fixture);
const row = staged.checkedReleases[releaseSet];
console.log(`staged checked release for execution release set ${releaseSet}`);
console.log(`  gate digest ${row.gateDigest}`);
console.log(`  sealed set  ${row.sealedSet}`);
console.log(`Both digests were copied from ${fragmentPath}; neither was read from this terminal.`);
