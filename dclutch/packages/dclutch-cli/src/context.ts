/**
 * Session context: where the chain is, who signs, and which programs are the
 * protocol.
 *
 * Resolution order, most explicit first:
 *   1. command-line flags (`--rpc`, `--core-program`, ...)
 *   2. a session file (`--session`), which is whatever JSON carries the run:
 *      a successor run spec (`dclutch-local-successor-run-spec-v2`), a run
 *      evidence document, or the compact session `dclutch found` writes
 *   3. environment (`DCLUTCH_RPC`, `DCLUTCH_KEYPAIR`, `DCLUTCH_SESSION`)
 *
 * Nothing here dials anything: constructing a context performs no I/O beyond
 * reading the named local files. AGENTS.md's authority rules hold — the CLI
 * signs only with a keypair the caller explicitly named, and never goes
 * looking for wallet files on its own.
 */
import { readFileSync } from 'node:fs';

import { SolanaRpcClient } from '@dclutch/sdk/rpc';
import { Keypair } from '@solana/web3.js';

export type ProgramRoleV1 = 'registry' | 'core' | 'claims' | 'trading' | 'resolution' | 'custody' | 'rentCredit';

export type SessionV1 = Readonly<{
  rpcUrl: string | null;
  programs: Readonly<Partial<Record<ProgramRoleV1, string>>>;
  markets: ReadonlyArray<string>;
}>;

export const EMPTY_SESSION: SessionV1 = Object.freeze({ rpcUrl: null, programs: Object.freeze({}), markets: Object.freeze([]) });

const ROLE_KEYS: ReadonlyArray<readonly [ProgramRoleV1, string]> = Object.freeze([
  ['registry', 'registry'],
  ['core', 'core'],
  ['claims', 'claims'],
  ['trading', 'trading'],
  ['resolution', 'resolution'],
  ['custody', 'custody'],
  ['rentCredit', 'rent_credit'],
]);

function plain(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/**
 * Read a session out of any of the three JSON shapes a local run produces.
 *
 * - run spec (`dclutch-local-successor-run-spec-v2`): roles at the top level,
 *   each `{ program_id: ... }`, plus `rpc_url`;
 * - run evidence (`dclutch-local-successor-run-evidence-v2`): an `accounts`
 *   map whose `market` / `founding_market` rows name the founded markets;
 * - the compact `{ schema: 'dclutch-cli-session-v1' }` file `dclutch found`
 *   writes, which is just this type serialized.
 */
export function decodeSession(value: unknown): SessionV1 {
  if (!plain(value)) throw new Error('session file must hold one JSON object');
  const programs: Partial<Record<ProgramRoleV1, string>> = {};
  const markets: string[] = [];
  let rpcUrl: string | null = null;
  if (value.schema === 'dclutch-cli-session-v1') {
    if (typeof value.rpcUrl === 'string') rpcUrl = value.rpcUrl;
    if (plain(value.programs)) {
      for (const [role] of ROLE_KEYS) {
        const id = (value.programs as Record<string, unknown>)[role];
        if (typeof id === 'string') programs[role] = id;
      }
    }
    if (Array.isArray(value.markets)) for (const entry of value.markets) if (typeof entry === 'string') markets.push(entry);
    return Object.freeze({ rpcUrl, programs: Object.freeze(programs), markets: Object.freeze(markets) });
  }
  if (typeof value.rpc_url === 'string') rpcUrl = value.rpc_url;
  for (const [role, specKey] of ROLE_KEYS) {
    const entry = value[specKey];
    if (plain(entry) && typeof entry.program_id === 'string') programs[role] = entry.program_id;
  }
  if (plain(value.accounts)) {
    for (const name of ['founding_market', 'market']) {
      const entry = (value.accounts as Record<string, unknown>)[name];
      if (plain(entry) && typeof entry.address === 'string') markets.push(entry.address);
      else if (typeof entry === 'string') markets.push(entry);
    }
  }
  return Object.freeze({ rpcUrl, programs: Object.freeze(programs), markets: Object.freeze(markets) });
}

export function loadSession(path: string): SessionV1 {
  return decodeSession(JSON.parse(readFileSync(path, 'utf8')));
}

export type CliContext = Readonly<{
  rpcUrl: string;
  session: SessionV1;
  flags: Readonly<Record<string, string | boolean | undefined>>;
  json: boolean;
}>;

export function resolveContext(flags: Readonly<Record<string, string | boolean | undefined>>, env: NodeJS.ProcessEnv): CliContext {
  const sessionPath = typeof flags.session === 'string' ? flags.session : env.DCLUTCH_SESSION;
  const session = sessionPath !== undefined && sessionPath !== '' ? loadSession(sessionPath) : EMPTY_SESSION;
  const rpcUrl = (typeof flags.rpc === 'string' ? flags.rpc : undefined) ?? env.DCLUTCH_RPC ?? session.rpcUrl ?? 'http://127.0.0.1:20890/';
  return Object.freeze({ rpcUrl, session, flags, json: flags.json === true });
}

export function rpcClient(context: CliContext): SolanaRpcClient {
  return new SolanaRpcClient(context.rpcUrl);
}

const FLAG_BY_ROLE: Readonly<Record<ProgramRoleV1, string>> = Object.freeze({
  registry: 'registry-program',
  core: 'core-program',
  claims: 'claims-program',
  trading: 'trading-program',
  resolution: 'resolution-program',
  custody: 'custody-program',
  rentCredit: 'rent-credit-program',
});

/** A program id, or an error that says every place it could have come from. */
export function programId(context: CliContext, role: ProgramRoleV1): string {
  const flag = context.flags[FLAG_BY_ROLE[role]];
  if (typeof flag === 'string' && flag.length > 0) return flag;
  const fromSession = context.session.programs[role];
  if (fromSession !== undefined) return fromSession;
  throw new Error(`the ${role} program id is not known: pass --${FLAG_BY_ROLE[role]} <address> or --session <spec/evidence/session json>`);
}

export function optionalProgramId(context: CliContext, role: ProgramRoleV1): string | null {
  try {
    return programId(context, role);
  } catch {
    return null;
  }
}

/**
 * The exact key file the caller named, without opening it.
 *
 * Separated from `loadKeypair` for the one case that needs the path and not
 * the secret: a command that hands the file to a child process which is the
 * thing that signs. The resolution rule is the same and lives only here.
 */
export function keypairPath(context: CliContext, env: NodeJS.ProcessEnv, flag = 'keypair'): string {
  const path = (typeof context.flags[flag] === 'string' ? (context.flags[flag] as string) : undefined) ?? (flag === 'keypair' ? env.DCLUTCH_KEYPAIR : undefined);
  if (path === undefined || path === '') {
    throw new Error(`no signer: pass --${flag} <path to a Solana JSON keypair> ${flag === 'keypair' ? 'or set DCLUTCH_KEYPAIR ' : ''}(this tool never reads a default wallet path)`);
  }
  return path;
}

/**
 * Load the signing keypair from the exact file the caller named — the
 * standard Solana JSON array of 64 bytes — via `--keypair` or
 * `$DCLUTCH_KEYPAIR`. Refuses to guess: no `~/.config/solana/id.json`
 * fallback, deliberately.
 */
export function loadKeypair(context: CliContext, env: NodeJS.ProcessEnv, flag = 'keypair'): Keypair {
  const path = keypairPath(context, env, flag);
  const raw: unknown = JSON.parse(readFileSync(path, 'utf8'));
  if (!Array.isArray(raw) || raw.length !== 64 || raw.some((value) => typeof value !== 'number' || !Number.isInteger(value) || value < 0 || value > 255)) {
    throw new Error(`${path} is not a Solana JSON keypair (an array of exactly 64 bytes)`);
  }
  return Keypair.fromSecretKey(Uint8Array.from(raw as number[]));
}
