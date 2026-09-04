/**
 * Compare this package's modules against their upstream twins in
 * `apps/dclutch-web` and report (or copy) the drift.
 *
 * Until the web app's imports flip to `@dclutch/sdk`, the web tree is where
 * frontend lanes land client-logic changes, and this package must absorb
 * them before the flip deletes the originals. This script is that
 * absorption, made mechanical and reviewable:
 *
 *   node scripts/sync-from-web.mjs            # report drift, exit 1 if any
 *   node scripts/sync-from-web.mjs --copy     # copy upstream-newer files in
 *   node scripts/sync-from-web.mjs --copy --only lib/deployments.ts
 *                                               # absorb one named seam
 *
 * Only git-TRACKED web files are considered — an untracked file is a lane's
 * work in progress and is deliberately invisible here.
 *
 * WHICH FILES IT MAY TOUCH IS NOT DECIDED HERE. This script used to hold its
 * own three sets, `apps/dclutch-web/lib/twinIdentity.test.ts` held its own
 * one, and the two disagreed about six files — including two whose absorption
 * would have overwritten an SDK owner with the two-line browser shim that
 * re-exports it. Both readers now ask `tools/twins/classification.mjs`, which
 * is the single table, and each asks its own question of the answer.
 */
import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { absorbsFromWeb, classifyWebPath, reportsWithoutCopying } from '../../../tools/twins/classification.mjs';

const sdkRoot = fileURLToPath(new URL('..', import.meta.url));
const repoRoot = fileURLToPath(new URL('../../..', import.meta.url));
const webRoot = join(repoRoot, 'apps', 'dclutch-web');

const tracked = execFileSync('git', ['-C', repoRoot, 'ls-files',
  'apps/dclutch-web/lib', 'apps/dclutch-web/scripts', 'apps/dclutch-web/fixtures',
], { encoding: 'utf8' }).trim().split('\n').filter((line) => line.length > 0);

const copy = process.argv.includes('--copy');
const only = new Set(process.argv.flatMap((argument, index, arguments_) => (
  argument === '--only' && arguments_[index + 1] !== undefined ? [arguments_[index + 1]] : []
)));
let drift = 0;
let diverged = 0;
let fresh = 0;
for (const file of tracked) {
  const rel = file.replace('apps/dclutch-web/', '');
  if (only.size > 0 && !only.has(rel)) continue;
  const webPath = join(webRoot, rel.split('/').join('/'));
  const sdkPath = join(sdkRoot, rel.split('/').join('/'));
  const twin = classifyWebPath(rel, existsSync(sdkPath));
  if (!absorbsFromWeb(twin.class) && !reportsWithoutCopying(twin.class)) continue;
  const upstream = readFileSync(webPath);
  if (!existsSync(sdkPath)) {
    fresh += 1;
    console.log(`new upstream module: ${rel}`);
    if (copy) {
      mkdirSync(dirname(sdkPath), { recursive: true });
      copyFileSync(webPath, sdkPath);
    }
    continue;
  }
  const local = readFileSync(sdkPath);
  if (upstream.equals(local)) continue;
  if (reportsWithoutCopying(twin.class)) {
    diverged += 1;
    console.log(`diverged (${twin.reason}), merge by hand: ${rel}`);
    continue;
  }
  drift += 1;
  console.log(`upstream moved: ${rel}`);
  if (copy) copyFileSync(webPath, sdkPath);
}

if (drift + fresh + diverged === 0) {
  console.log('in sync with apps/dclutch-web');
} else if (copy) {
  console.log(`copied ${drift + fresh} file(s); ${diverged} diverged file(s) left for hand-merge. Run the suite.`);
} else {
  console.log(`${drift + fresh} file(s) drifted (${fresh} new), ${diverged} need hand-merge. Re-run with --copy to absorb.`);
  process.exit(1);
}
