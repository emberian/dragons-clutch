import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import {
  TWIN_CLASSES,
  TWIN_CLASSIFICATION,
  classifyWebPath,
  isPureReExport,
  twinsMustDiffer,
} from '../../../tools/twins/classification.mjs';

/**
 * The twin trees, held to the identity they claim.
 *
 * `apps/dclutch-web` and `packages/dclutch-sdk` carry hand-maintained copies of
 * the same generators and the same client modules. Until the web app's imports
 * flip to `@dclutch/sdk`, that duplication is the arrangement, and
 * `scripts/sync-from-web.mjs` is the tool for absorbing web-side changes into
 * the package.
 *
 * Nothing ran it. So the duplication had no gate at all, in either half:
 *
 *   * `lib/abiVerification.test.ts` proves every generated module still agrees
 *     with its Rust or Lean authority — while the GENERATORS doing the scraping
 *     were compared by nothing. Fix a scraper in one tree, forget the other,
 *     and both verifies stay green because each regenerates against its own
 *     copy. The General V5 `ENVELOPE_RESERVED_OFFSET` repair had to be applied
 *     twice by hand and only a terminal `cmp` said so.
 *   * the client modules were the same shape one level up. GRICE named the
 *     `marketDetail.ts` pair specifically: identical today, gated by nothing,
 *     and one lane's one-sided edit from being a silent fork — where the
 *     browser and every SDK consumer would disagree about the same market with
 *     no test anywhere able to notice.
 *
 * The rule: a file present at the same path in both trees is BYTE-IDENTICAL,
 * unless `tools/twins/classification.mjs` gives it a class that says otherwise.
 *
 * That table is a two-way ratchet. An entry that has BECOME identical fails
 * too, because an exemption outliving its reason is a hole nobody is watching,
 * and deleting the line is the fix. That is also how the absorption backlog is
 * meant to shrink: absorb a file, delete its line, and the pair is gated from
 * then on.
 *
 * THE TABLE IS SHARED WITH `sync-from-web.mjs`, and that is the point of it.
 * The two instruments held separate maps until 2026-09-04 and disagreed about
 * six files; the script would have absorbed a two-line browser shim over the
 * 286-line and 446-line SDK owners it re-exports, while this file called the
 * same arrangement deliberate. One classification, two readers, each asking its
 * own question of it.
 *
 * This gate reads BOTH trees, so its own class is `WEB-ONLY` — an absorbed copy
 * would compare a package against itself and pass on anything.
 */

const repoRoot = fileURLToPath(new URL('../../..', import.meta.url));
const webRoot = join(repoRoot, 'apps', 'dclutch-web');
const sdkRoot = join(repoRoot, 'packages', 'dclutch-sdk');

function walk(root: string, directory: string, prefix: string, keep: (name: string) => boolean): ReadonlyArray<string> {
  const output: string[] = [];
  for (const entry of readdirSync(join(root, directory), { withFileTypes: true }).sort((left, right) => left.name.localeCompare(right.name))) {
    const path = `${prefix}${entry.name}`;
    if (entry.isDirectory()) output.push(...walk(root, join(directory, entry.name), `${path}/`, keep));
    else if (keep(entry.name)) output.push(path);
  }
  return output;
}

const isModule = (name: string): boolean => name.endsWith('.ts');
const isGenerator = (name: string): boolean => name.endsWith('.mjs');

/** Web-tree paths that also exist at the same path under the SDK. */
function shared(paths: ReadonlyArray<string>): ReadonlyArray<string> {
  return paths.filter((path) => existsSync(join(sdkRoot, path)));
}

function twinArm(title: string, paths: ReadonlyArray<string>, minimum: number, witness: string): void {
  describe(title, () => {
    it('has shared files to speak about at all', () => {
      // A walk that silently matched nothing would make every assertion below
      // vacuous, which is the one way this file could lie.
      expect(paths.length).toBeGreaterThanOrEqual(minimum);
      expect(paths).toContain(witness);
    });

    for (const path of paths) {
      const twin = classifyWebPath(path, true);
      const mustDiffer = twinsMustDiffer(twin.class);
      it(mustDiffer ? `${path} still diverges (${twin.class}): ${twin.reason}` : `${path} is byte-identical in both trees`, () => {
        // `Buffer.compare`, not `Buffer.prototype.equals`: under this
        // `@types/node`, `readFileSync` returns `NonSharedBuffer`, which the
        // declarations give no `equals`. Same comparison, and it typechecks.
        const identical = Buffer.compare(readFileSync(join(sdkRoot, path)), readFileSync(join(webRoot, path))) === 0;
        if (mustDiffer) {
          expect(
            identical,
            `${path} is classified ${twin.class} in tools/twins/classification.mjs but the two trees now agree byte for byte. The exemption has outlived its reason: delete the line.`,
          ).toBe(false);
        } else {
          expect(
            identical,
            `${path} differs between apps/dclutch-web and packages/dclutch-sdk. These are hand-maintained copies of one file: copy the corrected one over the other (\`node scripts/sync-from-web.mjs --copy --only ${path}\` from packages/dclutch-sdk absorbs the web side), or — if the divergence is deliberate — classify it in tools/twins/classification.mjs with the reason.`,
          ).toBe(true);
        }
      });
    }
  });
}

/** The table, read by path rather than by literal key. */
const classified: Readonly<Record<string, ReadonlyArray<string> | undefined>> = TWIN_CLASSIFICATION;

const webModules = walk(webRoot, 'lib', 'lib/', isModule);
const sdkModules = walk(sdkRoot, 'lib', 'lib/', isModule);

twinArm('the twin generator trees', shared(walk(webRoot, 'scripts', 'scripts/', isGenerator)), 10, 'scripts/generate-general-successor-v5.mjs');
twinArm('the twin client modules', shared(webModules), 60, 'lib/marketDetail.ts');

/**
 * The table itself, held to the two trees it describes.
 *
 * The arms above only visit SHARED pairs, so for years the files that exist on
 * one side only — 154 web modules the package has never absorbed, 54 the
 * package owns outright — were described by nothing at all, and a line in the
 * table naming one of them would have sat there unread. These assertions close
 * that: every module on either side gets exactly one class, and the classes
 * that make a claim about a file's CONTENT are checked against the content.
 */
describe('the twin classification table', () => {
  it('classifies every module under both lib/ trees exactly once', () => {
    expect(webModules.length).toBeGreaterThanOrEqual(300);
    expect(sdkModules.length).toBeGreaterThanOrEqual(200);
    const web = new Set(webModules);
    for (const path of webModules) {
      const twin = classifyWebPath(path, existsSync(join(sdkRoot, path)));
      expect(TWIN_CLASSES, `${path} carries the unknown class ${twin.class}`).toContain(twin.class);
      expect(twin.reason.length, `${path} is classified ${twin.class} with no reason`).toBeGreaterThan(0);
    }
    // The package's own modules are SDK-OWNED by construction — the web tree
    // has no copy to classify — so the table must never name one.
    for (const path of sdkModules) {
      if (web.has(path)) continue;
      expect(Object.keys(TWIN_CLASSIFICATION), `${path} exists only under the SDK and needs no entry`).not.toContain(path);
    }
  });

  it('names no path that is not a real web-tree file', () => {
    for (const path of Object.keys(TWIN_CLASSIFICATION)) {
      expect(existsSync(join(webRoot, path)), `${path} is classified but does not exist under apps/dclutch-web`).toBe(true);
    }
  });

  it('gives every class it uses a behaviour both readers agree on', () => {
    for (const [path, entry] of Object.entries(TWIN_CLASSIFICATION)) {
      expect(TWIN_CLASSES, `${path} carries the unknown class ${entry[0]}`).toContain(entry[0]);
    }
  });

  it('proves every REEXPORT is one, by reading it', () => {
    // This is the class that costs the most when it is wrong: absorbing a
    // two-line shim over its owner deletes the owner. So it is checked against
    // the file rather than asserted — `lib/deployments.ts` re-exports 286 lines
    // and `lib/walletHandoff.ts` 446, and both were absent from the script's
    // map on 2026-09-04 while this file called them REEXPORT.
    const reexports = Object.entries(TWIN_CLASSIFICATION).filter(([, entry]) => entry[0] === 'REEXPORT');
    expect(reexports.length).toBeGreaterThanOrEqual(17);
    for (const [path] of reexports) {
      expect(
        isPureReExport(readFileSync(join(webRoot, path), 'utf8')),
        `${path} is classified REEXPORT but is not a bare \`export * from\`; it has grown a second implementation, or its class is wrong`,
      ).toBe(true);
      expect(existsSync(join(sdkRoot, path)), `${path} re-exports an SDK owner the package does not have`).toBe(true);
    }
  });

  it('proves every SHIM keeps its SDK owner and adds to it', () => {
    const shims = Object.entries(TWIN_CLASSIFICATION).filter(([, entry]) => entry[0] === 'SHIM');
    expect(shims.length).toBeGreaterThanOrEqual(3);
    for (const [path] of shims) {
      // A shim's own test carries no SDK import — it exercises the half the
      // shim ADDS, through the shim. So the SDK reach is asked of the module,
      // and a shim test is required to have one classified beside it.
      const owner = path.endsWith('.test.ts') ? path.replace(/\.test\.ts$/, '.ts') : path;
      expect(classified[owner]?.[0], `${path} is classified SHIM with no shim module beside it`).toBe('SHIM');
      expect(readFileSync(join(webRoot, owner), 'utf8'), `${owner} is classified SHIM but never reaches for the SDK`).toContain('@dclutch/sdk/');
      expect(isPureReExport(readFileSync(join(webRoot, path), 'utf8')), `${path} is classified SHIM but adds nothing: it is a REEXPORT`).toBe(false);
    }
  });

  it('proves every WEB-ONLY file has no package copy to drift from', () => {
    const webOnly = Object.entries(TWIN_CLASSIFICATION).filter(([, entry]) => entry[0] === 'WEB-ONLY');
    expect(webOnly.length).toBeGreaterThanOrEqual(4);
    for (const [path] of webOnly) {
      expect(existsSync(join(sdkRoot, path)), `${path} is classified WEB-ONLY but the package has a copy: it is a twin, not an exemption`).toBe(false);
    }
  });
});
