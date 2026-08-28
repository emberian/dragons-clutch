import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { describe, it } from 'vitest';

/**
 * GITSCAN-2's G-4: gen-1 had a real dependency/license SBOM
 * (`dragons-clutch/scripts/dependency_license_check.py`) wired in as a
 * declared, byte-gated release gate; gen-3 had none at all — a regression
 * on a strictly larger surface, since this repository also has an npm tree
 * gen-1 never did, and the Pages workflow now distributes the frontend,
 * which makes the AGPL-3.0-or-later source-offer obligation live rather
 * than theoretical.
 *
 * `tools/sbom/sbom_check.py --verify` is the replacement, following the
 * exact convention `abiVerification.test.ts` established above: the check
 * script already exists and is correct on its own, the failure mode this
 * file closes is a script that exists but is not part of the gate anyone
 * actually runs. Wiring it into this suite means VITEST GREEN IMPLIES THE
 * COMMITTED SBOM IS CURRENT — a dependency added anywhere in this
 * repository's discovered Cargo workspaces or tracked npm package trees
 * without regenerating `tools/sbom/SBOM.md`/`NOTICES.md` turns this test
 * red, naming the drift rather than leaving it unnoticed.
 *
 * A flagged (copyleft/copyleft-adjacent/license-file-only/unrecognized)
 * license row does NOT fail this test — see `tools/sbom/SBOM.md`'s
 * "Flagged for review" section, which is the reviewable deliverable, not a
 * defect — and neither does a stale lockfile in some other lane's
 * mini-workspace (`tools/sbom/SBOM.md`'s "Unresolvable manifests"
 * section): a reproducibility gap there is real but not this test's, and
 * not a license question. What fails here: a genuinely unclassified
 * license, a forbidden dependency source, or the committed SBOM/notices
 * drifting from a fresh run.
 */

const repoRoot = fileURLToPath(new URL('../../..', import.meta.url));

describe('dependency/license SBOM', () => {
  it('tools/sbom/SBOM.md and NOTICES.md are current and every dependency is classified', () => {
    try {
      execFileSync('python3', ['tools/sbom/sbom_check.py', '--verify'], {
        cwd: repoRoot,
        stdio: 'pipe',
      });
    } catch (error) {
      const detail = error as Readonly<{ stdout?: Buffer; stderr?: Buffer }>;
      const output = `${detail.stderr?.toString() ?? ''}${detail.stdout?.toString() ?? ''}`.trim();
      throw new Error(
        `tools/sbom/sbom_check.py --verify refused — a dependency changed without regenerating the SBOM, or something is genuinely unclassified. Regenerate with \`python3 tools/sbom/sbom_check.py\` from the repository root and read the diff.\n${output}`,
      );
    }
  }, 120_000);
});
