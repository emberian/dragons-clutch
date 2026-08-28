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
  const candidates = [
    resolve(repoRoot, 'tools/local-validator/bootstrap/successor/target/release/dclutch-local-successor-bootstrap'),
    resolve(repoRoot, '../../tools/local-validator/bootstrap/successor/target/release/dclutch-local-successor-bootstrap'),
  ];
  for (const candidate of candidates) if (existsSync(candidate)) return candidate;
  throw new Error(`the successor bootstrap binary was not found (tried ${candidates.join(', ')}); build it with \`cargo build --release\` in tools/local-validator/bootstrap/successor, or pass --bootstrap-bin / set DCLUTCH_BOOTSTRAP_BIN`);
}
