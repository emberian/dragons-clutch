/**
 * Locate the repo's successor producer without guessing any wallet or cluster
 * state. Commands still have to name their own inputs and authority flags.
 */
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { type CliContext } from './context';

export function successorBinary(context: CliContext, env: NodeJS.ProcessEnv): string {
  const flag = context.flags['bootstrap-bin'];
  if (typeof flag === 'string' && flag.length > 0) return flag;
  if (env.DCLUTCH_BOOTSTRAP_BIN !== undefined && env.DCLUTCH_BOOTSTRAP_BIN !== '') return env.DCLUTCH_BOOTSTRAP_BIN;
  const repoRoot = fileURLToPath(new URL('../../..', import.meta.url));
  // One workspace, so one target directory: the driver lands at the repository
  // root whatever directory `cargo build` was run from. `CARGO_TARGET_DIR`
  // still wins when an operator has redirected the build.
  const targetRoot = env.CARGO_TARGET_DIR !== undefined && env.CARGO_TARGET_DIR !== ''
    ? env.CARGO_TARGET_DIR
    : resolve(repoRoot, 'target');
  const candidates = [
    resolve(targetRoot, 'release/dclutch-local-successor-bootstrap'),
    resolve(targetRoot, 'debug/dclutch-local-successor-bootstrap'),
  ];
  for (const candidate of candidates) if (existsSync(candidate)) return candidate;
  throw new Error(`the successor bootstrap binary was not found (tried ${candidates.join(', ')}); build it with \`cargo build --release -p dclutch-local-successor-bootstrap\`, or pass --bootstrap-bin / set DCLUTCH_BOOTSTRAP_BIN`);
}
