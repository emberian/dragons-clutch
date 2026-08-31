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
 * work in progress and is deliberately invisible here. Files listed in
 * DIVERGED carry deliberate SDK-side edits (they typecheck under the plain
 * node lib set where the web tree never typechecks at all); the script
 * reports them but never overwrites them — merge those by hand and record
 * why in the commit.
 */
import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const sdkRoot = fileURLToPath(new URL('..', import.meta.url));
const repoRoot = fileURLToPath(new URL('../../..', import.meta.url));
const webRoot = join(repoRoot, 'apps', 'dclutch-web');

/** Web files that stay web-side: browser coupling or repo-wide gates. */
const WEB_ONLY = new Set([
  'lib/walletStandard.ts',
  'lib/walletStandard.test.ts',
  'lib/sbomVerify.test.ts',
  // Repo-wide gates: they read BOTH trees, so a copy here would compare a
  // package against itself and pass on anything.
  'lib/twinIdentity.test.ts',
]);

/** App compatibility shims that already re-export their SDK semantic owner. */
const SDK_OWNED_REEXPORTS = new Set([
  'lib/founding/principalCapacity.ts',
  'lib/marketDiscovery.ts',
  'lib/rationalTerminalChainV4.ts',
  // The board's transport is SDK-owned and takes its URL as an argument; the
  // web file is the deployment half, reading the one `NEXT_PUBLIC_*` variable
  // that survives a static export. An SDK that reached for `process.env` would
  // be a second place a deployment is decided.
  'lib/ticketBoard.ts',
]);

/** SDK files with deliberate local edits; never auto-copied. */
const DIVERGED = new Set([
  'lib/founding/principalCapacity.test.ts',
  'lib/rpc.ts',
  'lib/rpc.test.ts',
  'lib/localSuccessor.ts',
  // The SDK authenticates the complete activation-cache contents and returns
  // its route-admission boundary; the UI copy intentionally remains lighter.
  'lib/operatorSurface.ts',
  'lib/operatorSurface.test.ts',
  'lib/generalPlanV5.test.ts',
  'scripts/abi-coverage.mjs',
  'scripts/abi-coverage.baseline.json',
]);

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
  if (WEB_ONLY.has(rel) || SDK_OWNED_REEXPORTS.has(rel)) continue;
  const webPath = join(webRoot, rel.split('/').join('/'));
  const sdkPath = join(sdkRoot, rel.split('/').join('/'));
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
  if (DIVERGED.has(rel)) {
    diverged += 1;
    console.log(`diverged (deliberate SDK edits, merge by hand): ${rel}`);
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
