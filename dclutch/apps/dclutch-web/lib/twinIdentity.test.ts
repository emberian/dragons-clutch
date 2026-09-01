import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

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
 * unless it is named in that arm's `DIVERGED` map with the reason.
 *
 * Every `DIVERGED` map is a two-way ratchet. An entry that has BECOME identical
 * fails too, because an exemption outliving its reason is a hole nobody is
 * watching, and deleting the line is the fix. That is also how the absorption
 * backlog below is meant to shrink: absorb a file, delete its line, and the
 * pair is gated from then on.
 *
 * This gate reads BOTH trees, so it is WEB_ONLY in `sync-from-web.mjs` — an
 * absorbed copy would compare a package against itself and pass on anything.
 */

const repoRoot = fileURLToPath(new URL('../../..', import.meta.url));
const webRoot = join(repoRoot, 'apps', 'dclutch-web');
const sdkRoot = join(repoRoot, 'packages', 'dclutch-sdk');

/**
 * Shared generator basenames that are deliberately NOT byte-identical.
 *
 * Keep this at the size of its justifications: anything here is a generator two
 * lanes can move independently without either noticing.
 */
const DIVERGED_SCRIPTS: Readonly<Record<string, string>> = Object.freeze({
  // The coverage census walks its own package's module list and holds its own
  // baseline of it; the two trees do not export the same modules. `sync-from-web.mjs`
  // records the same divergence for the same reason.
  'scripts/abi-coverage.mjs': 'each package censuses its own module inventory against its own baseline',
});

/** The two classes of module divergence, so a line says which debt it is. */
const DELIBERATE = 'deliberate SDK-side edit; sync-from-web.mjs lists it as merge-by-hand';
const REEXPORT = 'the web file is a compatibility shim re-exporting its SDK semantic owner';
const BACKLOG = 'already drifted: web-side change awaiting SDK absorption (node scripts/sync-from-web.mjs)';

/**
 * Shared module paths that are NOT byte-identical today, and why.
 *
 * The BACKLOG entries are debt, named as debt. They are the part of the
 * 99-file absorption backlog that has a twin on both sides, so each one is a
 * place where the browser and the SDK already answer differently and no test
 * says which is right. This map exists so the list can only ever shrink.
 */
const DIVERGED_MODULES: Readonly<Record<string, string>> = Object.freeze({
  'lib/rpc.ts': DELIBERATE,
  'lib/rpc.test.ts': DELIBERATE,
  'lib/localSuccessor.ts': DELIBERATE,
  'lib/operatorSurface.ts': DELIBERATE,
  'lib/operatorSurface.test.ts': DELIBERATE,
  'lib/founding/principalCapacity.test.ts': DELIBERATE,
  'lib/marketDiscovery.ts': REEXPORT,
  'lib/rationalTerminalChainV4.ts': REEXPORT,
  'lib/rationalOpenHotV3.ts': REEXPORT,
  'lib/rationalOpenChainV4.ts': REEXPORT,
  'lib/rationalOpenWasmV1.testSupport.ts': REEXPORT,
  'lib/founding/principalCapacity.ts': REEXPORT,
  'lib/directOfferAuthoring.ts': REEXPORT,
  'lib/ticketBoard.ts': REEXPORT,
  'lib/activity.ts': BACKLOG,
  'lib/activity.test.ts': BACKLOG,
  'lib/capabilityModel.ts': BACKLOG,
  'lib/claimsCustodyReplay.test.ts': BACKLOG,
  'lib/deployments.ts': REEXPORT,
  'lib/directMakerReplay.ts': BACKLOG,
  'lib/directMakerReplay.test.ts': BACKLOG,
  'lib/directParticipant.ts': BACKLOG,
  'lib/directTicket.ts': BACKLOG,
  'lib/directTicket.test.ts': BACKLOG,
  'lib/directTradeSpine.ts': BACKLOG,
  'lib/founding/lookupTable.ts': BACKLOG,
  'lib/rationalRetireReceiptV4.ts': REEXPORT,
  'lib/resolutionCertificateV2.ts': BACKLOG,
  'lib/slotClock.ts': BACKLOG,
  'lib/slotClock.test.ts': BACKLOG,
  'lib/supplyShares.ts': BACKLOG,
  'lib/walletHandoff.ts': REEXPORT,
});

function walk(directory: string, prefix: string, keep: (name: string) => boolean): ReadonlyArray<string> {
  const output: string[] = [];
  for (const entry of readdirSync(join(webRoot, directory), { withFileTypes: true }).sort((left, right) => left.name.localeCompare(right.name))) {
    const path = `${prefix}${entry.name}`;
    if (entry.isDirectory()) output.push(...walk(join(directory, entry.name), `${path}/`, keep));
    else if (keep(entry.name)) output.push(path);
  }
  return output;
}

/** Web-tree paths that also exist at the same path under the SDK. */
function shared(paths: ReadonlyArray<string>): ReadonlyArray<string> {
  return paths.filter((path) => {
    try {
      readFileSync(join(sdkRoot, path));
      return true;
    } catch {
      return false;
    }
  });
}

function twinArm(title: string, paths: ReadonlyArray<string>, diverged: Readonly<Record<string, string>>, minimum: number, witness: string): void {
  describe(title, () => {
    it('has shared files to speak about at all', () => {
      // A walk that silently matched nothing would make every assertion below
      // vacuous, which is the one way this file could lie.
      expect(paths.length).toBeGreaterThanOrEqual(minimum);
      expect(paths).toContain(witness);
    });

    it('names no exemption that is not a real shared pair', () => {
      for (const path of Object.keys(diverged)) expect(paths, `${path} is exempted but is not a shared pair`).toContain(path);
    });

    for (const path of paths) {
      const reason = diverged[path];
      it(reason === undefined ? `${path} is byte-identical in both trees` : `${path} still diverges: ${reason}`, () => {
        const identical = readFileSync(join(sdkRoot, path)).equals(readFileSync(join(webRoot, path)));
        if (reason === undefined) {
          expect(
            identical,
            `${path} differs between apps/dclutch-web and packages/dclutch-sdk. These are hand-maintained copies of one file: copy the corrected one over the other (\`node scripts/sync-from-web.mjs --copy --only ${path}\` from packages/dclutch-sdk absorbs the web side), or — if the divergence is deliberate — add it to this file's DIVERGED map with the reason.`,
          ).toBe(true);
        } else {
          expect(
            identical,
            `${path} is exempted here but the two trees now agree byte for byte. The exemption has outlived its reason: delete the line.`,
          ).toBe(false);
        }
      });
    }
  });
}

twinArm(
  'the twin generator trees',
  shared(walk('scripts', 'scripts/', (name) => name.endsWith('.mjs'))),
  DIVERGED_SCRIPTS,
  10,
  'scripts/generate-general-successor-v5.mjs',
);

twinArm(
  'the twin client modules',
  shared(walk('lib', 'lib/', (name) => name.endsWith('.ts'))),
  DIVERGED_MODULES,
  60,
  'lib/marketDetail.ts',
);
