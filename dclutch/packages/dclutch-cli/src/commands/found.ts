/**
 * `dclutch found` — founding, wrapped: the run-spec producer
 * (`dclutch-local-successor-bootstrap`) is the founding client of record,
 * and this command drives it end to end from a spec file, then leaves
 * behind a session file the rest of the CLI reads its program ids and
 * market addresses from.
 *
 *   dclutch found --spec run-spec.json [--session-out session.json]
 *   dclutch found --demo --registry-program <id>     # print the demo-market input
 *
 * The spec (`dclutch-local-successor-run-spec-v2`) names everything: the
 * seven role ELFs by hash, the launcher, the ledger, the market recipe, and
 * the evidence output path. The producer starts its own guarded validator;
 * nothing here deploys to any cluster a spec does not name.
 */
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { isAbsolute, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { decodeSession, type CliContext } from '../context';
import { block, type Io } from '../output';

function bootstrapBinary(context: CliContext, env: NodeJS.ProcessEnv): string {
  const flag = context.flags['bootstrap-bin'];
  if (typeof flag === 'string' && flag.length > 0) return flag;
  if (env.DCLUTCH_BOOTSTRAP_BIN !== undefined && env.DCLUTCH_BOOTSTRAP_BIN !== '') return env.DCLUTCH_BOOTSTRAP_BIN;
  // Repo-resident default: this package lives at packages/dclutch-cli.
  const repoRoot = fileURLToPath(new URL('../../..', import.meta.url));
  const candidates = [
    resolve(repoRoot, 'tools/local-validator/bootstrap/successor/target/release/dclutch-local-successor-bootstrap'),
    resolve(repoRoot, '../../tools/local-validator/bootstrap/successor/target/release/dclutch-local-successor-bootstrap'),
  ];
  for (const candidate of candidates) if (existsSync(candidate)) return candidate;
  throw new Error(`the successor bootstrap binary was not found (tried ${candidates.join(', ')}); build it with \`cargo build --release\` in tools/local-validator/bootstrap/successor, or pass --bootstrap-bin / set DCLUTCH_BOOTSTRAP_BIN`);
}

export async function found(context: CliContext, io: Io, env: NodeJS.ProcessEnv): Promise<number> {
  const binary = bootstrapBinary(context, env);

  if (context.flags.demo === true) {
    const registry = context.flags['registry-program'];
    if (typeof registry !== 'string') throw new Error('pass --registry-program <id> (the demo market binds resolution identities under it)');
    const result = spawnSync(binary, ['demo-market', '--registry-program-id', registry], { encoding: 'utf8' });
    if (result.status !== 0) {
      io.err(result.stderr ?? 'demo-market refused');
      return 1;
    }
    io.out(result.stdout.trimEnd());
    io.err('');
    io.err('this is the MarketRunInput half of a run spec (SOL/USD range protection, synthetic-local Pyth).');
    io.err('tools/gauntlet/run.sh assembles the full spec around it; then: dclutch found --spec <spec.json>');
    return 0;
  }

  const specPath = context.flags.spec;
  if (typeof specPath !== 'string') throw new Error('pass --spec <run-spec.json> (or --demo to print the demo-market input)');
  const absoluteSpec = isAbsolute(specPath) ? specPath : resolve(process.cwd(), specPath);
  const spec: unknown = JSON.parse(readFileSync(absoluteSpec, 'utf8'));
  const args = ['run', '--spec', absoluteSpec];
  const seed = context.flags['keypair-seed'];
  if (typeof seed === 'string') args.push('--keypair-seed', seed);

  io.out(`founding via ${binary}`);
  io.out(`  spec ${absoluteSpec}`);
  const result = spawnSync(binary, args, { stdio: ['ignore', 'inherit', 'inherit'] });
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
