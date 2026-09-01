/**
 * `dclutch-terminal found` has two explicit, non-overlapping modes.
 *
 * - `--spec` drives the complete private-validator run owned by the successor.
 * - `--found-operation` prepares or executes permanent-devnet founding and
 *   first-participant admission through one durable exterior journal.
 *
 * The retired demo-market producer is deliberately not reachable here.
 */
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { isAbsolute, resolve } from 'node:path';

import { decodeSession, type CliContext } from '../context';
import { runFoundOperationV1 } from '../foundOperation';
import { block, type Io } from '../output';
import { successorBinary } from '../successor';

export async function found(context: CliContext, io: Io, env: NodeJS.ProcessEnv): Promise<number> {
  const binary = successorBinary(context, env);

  const operationPath = context.flags['found-operation'];
  const journalPath = context.flags['found-journal'];
  if (typeof operationPath === 'string' || typeof journalPath === 'string') {
    if (typeof operationPath !== 'string' || typeof journalPath !== 'string') {
      throw new Error('devnet founding requires both --found-operation ABSOLUTE_JSON and --found-journal ABSOLUTE_JSON');
    }
    if (context.flags.spec !== undefined || context.flags['keypair-seed'] !== undefined) {
      throw new Error('--found-operation cannot be combined with the private-validator --spec or --keypair-seed mode');
    }
    const sessionOut = typeof context.flags['session-out'] === 'string' ? context.flags['session-out'] : null;
    return runFoundOperationV1(
      context,
      io,
      binary,
      operationPath,
      journalPath,
      sessionOut,
      context.flags.execute === true,
    );
  }

  if (context.flags.execute !== undefined) {
    throw new Error('--execute belongs to --found-operation; a private-validator --spec run already owns its complete lifecycle');
  }

  const specPath = context.flags.spec;
  if (typeof specPath !== 'string') {
    throw new Error('pass --spec <private-validator run-spec.json>, or both --found-operation and --found-journal for permanent devnet');
  }
  const absoluteSpec = isAbsolute(specPath) ? specPath : resolve(process.cwd(), specPath);
  const spec: unknown = JSON.parse(readFileSync(absoluteSpec, 'utf8'));
  const args = ['run', '--spec', absoluteSpec];
  const seed = context.flags['keypair-seed'];
  if (typeof seed === 'string') args.push('--keypair-seed', seed);

  io.out(`private-validator lifecycle via ${binary}`);
  io.out(`  spec ${absoluteSpec}`);
  // The producer pins its launcher to one exact RPC origin via
  // $DCLUTCH_RPC_PORT (default 20890). The spec already names the origin, so
  // derive the pin from the spec instead of making the caller repeat it.
  const childEnv: NodeJS.ProcessEnv = { ...env };
  const rpcUrl = typeof spec === 'object' && spec !== null ? (spec as Record<string, unknown>).rpc_url : undefined;
  if (typeof rpcUrl === 'string' && childEnv.DCLUTCH_RPC_PORT === undefined) {
    const port = new URL(rpcUrl).port;
    if (port !== '') {
      childEnv.DCLUTCH_RPC_PORT = port;
      io.out(`  rpc port ${port} (derived from the spec's rpc_url)`);
    }
  }
  const result = spawnSync(binary, args, { stdio: ['ignore', 'inherit', 'inherit'], env: childEnv });
  if (result.status !== 0) {
    io.err(`bootstrap run exited ${result.status ?? 'by signal'}`);
    return result.status ?? 1;
  }

  // The evidence document the spec pointed at names the founded markets.
  const outputPath = typeof spec === 'object' && spec !== null && typeof (spec as Record<string, unknown>).output === 'string'
    ? (spec as Record<string, unknown>).output as string
    : null;
  if (outputPath === null || !existsSync(outputPath)) {
    io.out('run complete; the spec named no readable evidence output, so no session file was written');
    return 0;
  }
  const evidence: unknown = JSON.parse(readFileSync(outputPath, 'utf8'));
  const fromSpec = decodeSession(spec);
  const fromEvidence = decodeSession(evidence);
  const session = Object.freeze({
    schema: 'dclutch-cli-session-v1',
    rpcUrl: fromSpec.rpcUrl,
    programs: fromSpec.programs,
    markets: fromEvidence.markets.length > 0 ? fromEvidence.markets : fromSpec.markets,
  });
  block(io, [
    ['rpc', session.rpcUrl ?? 'unstated'],
    ['markets', session.markets.join(', ') || 'none named in evidence'],
    ['evidence', outputPath],
  ]);
  const sessionOut = context.flags['session-out'];
  if (typeof sessionOut === 'string') {
    writeFileSync(sessionOut, `${JSON.stringify(session, null, 2)}\n`);
    io.out(`session written to ${sessionOut} — pass --session ${sessionOut} to the other commands`);
  }
  return 0;
}
