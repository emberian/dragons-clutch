import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import manifest from '../package.json';

/**
 * The mechanism finding, closed.
 *
 * Before this file, `npm test` was 232 green assertions that said NOTHING about
 * whether `lib/generated/` still agreed with the Rust and Lean authorities it
 * was generated from. The verify scripts existed and were correct; they were
 * simply not part of the gate a lane actually runs. So a lane could delete the
 * program that owned a constant, leave the browser deciding an account width by
 * itself, and watch the suite stay green — which is exactly what happened to
 * `REPLAY_STATE_BYTES`, and what a whole `/economic` route did for two schemas
 * no Rust defines anywhere.
 *
 * Every `abi:*:verify` now runs inside vitest, so VITEST GREEN IMPLIES ABI
 * TRUTH. Each script is its own test case, so a failure names the surface that
 * drifted rather than one opaque red.
 *
 * These are byte-comparisons against a regenerated artifact, not a checksum: a
 * verify goes red when the authority moved and the checked-in module did not.
 * The fix is always to regenerate (`npm run <the same script without :verify>`)
 * and to look at the diff, never to relax the check.
 *
 * The Lean-backed verifiers shell out to `lake`, exactly as the Rust
 * `check-generated.sh` scripts do. If `lake` is not on PATH, they fail rather
 * than skip: an unverifiable ABI is not a verified one. Two consequences worth
 * knowing before blaming this file: a lane building `formal/` at the same
 * moment shares lake's lock, and a Lean schema edited but not yet re-emitted to
 * `lib/generated/` will turn the WEB suite red. Both are the gate working — the
 * browser really is stale in that second case — but the fix lives in the other
 * lane's tree, not here.
 */

const webRoot = fileURLToPath(new URL('..', import.meta.url));
const scripts: Readonly<Record<string, string>> = manifest.scripts;

const verifiers = Object.keys(scripts).filter((name) => name.startsWith('abi:') && name.endsWith(':verify')).sort();
const generators = Object.keys(scripts)
  .filter((name) => name.startsWith('abi:') && !name.endsWith(':verify') && name !== 'abi:coverage')
  .sort();

function run(script: string): void {
  const command = scripts[script];
  // Every generator is invoked as `node scripts/<x>.mjs …`; running it directly
  // rather than through `npm run` keeps twelve npm launches out of the suite.
  const parts = command.split(/\s+/);
  expect(parts[0], `${script} is not a direct node invocation; teach this runner about it`).toBe('node');
  execFileSync(process.execPath, parts.slice(1), { cwd: webRoot, stdio: 'pipe' });
}

describe('generated ABI modules still agree with their authorities', () => {
  it('pairs every abi:* generator with a verify script', () => {
    // A generator without a verifier is a module that can drift silently. This
    // is the ratchet that stops the next one being added without its gate.
    expect(verifiers.map((name) => name.slice(0, -':verify'.length))).toEqual(generators);
    expect(generators.length).toBeGreaterThan(0);
  });

  for (const script of verifiers) {
    it(`${script} is green`, () => {
      try {
        run(script);
      } catch (error) {
        const detail = error as Readonly<{ stdout?: Buffer; stderr?: Buffer }>;
        const output = `${detail.stderr?.toString() ?? ''}${detail.stdout?.toString() ?? ''}`.trim();
        throw new Error(`${script} refused — the authority moved and lib/generated/ did not follow. Regenerate with \`npm run ${script.slice(0, -':verify'.length)}\` and read the diff.\n${output}`);
      }
    }, 180_000);
  }
});
