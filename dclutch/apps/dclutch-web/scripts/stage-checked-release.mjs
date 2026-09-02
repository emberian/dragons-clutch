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
 * selects, read off the chain -- the market page prints it, and
 * `inspectDirectTradeSpineV1` returns it as `releaseSetId`. It is required and
 * it is not taken from the fragment, because the fragment is the thing being
 * checked: a fragment for another deployment's release set would otherwise
 * turn the trade spine's `release` wall off for a market the release was never
 * checked against, which is the exact failure the wall exists to prevent.
 *
 * The fixture is replaced ATOMICALLY through a temporary file on the same
 * filesystem, and only after the result parses as a cut, so a refused stage
 * leaves the last accepted fixture byte-for-byte intact.
 */
import { readFileSync, renameSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const fixture = fileURLToPath(new URL('../fixtures/public-cut.devnet.json', import.meta.url));
const [fragmentPath, ...rest] = process.argv.slice(2);
const flag = rest.indexOf('--release-set');
const releaseSet = flag < 0 ? null : rest[flag + 1];
if (fragmentPath === undefined || releaseSet === undefined || releaseSet === null) {
  throw new Error('usage: node scripts/stage-checked-release.mjs <fragment.json> --release-set HEX64');
}

const { parseCheckedReleaseFragmentV1, parsePublicDevnetCutV1, stageCheckedReleaseV1 } =
  await import('../lib/publicCutStaging.ts');

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
console.log(`staged checked release for execution release set ${fragment.releaseSetId}`);
