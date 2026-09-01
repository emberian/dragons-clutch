/**
 * Session context: where the chain is, who signs, and which programs are the
 * protocol.
 *
 * Resolution order, most explicit first:
 *   1. command-line flags (`--rpc`, `--core-program`, ...)
 *   2. a session file (`--session`), which is whatever JSON carries the run:
 *      a successor run spec (`dclutch-local-successor-run-spec-v2`), a run
 *      evidence document, or the compact session `dclutch-terminal found` writes
 *   3. environment (`DCLUTCH_RPC`, `DCLUTCH_KEYPAIR`, `DCLUTCH_SESSION`)
 *
 * Nothing here dials anything: constructing a context performs no I/O beyond
 * reading the named local files. AGENTS.md's authority rules hold — the CLI
 * signs only with a keypair the caller explicitly named, and never goes
 * looking for wallet files on its own.
 */
import { readFileSync } from 'node:fs';

import type { DeploymentV1 } from '@dclutch/sdk/deployments';
import type { MutationClusterAdmissionV1 } from '@dclutch/sdk/rpc';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';
import { Keypair } from '@solana/web3.js';

import {
  assertDeploymentIdentityV1,
  deploymentProgramIdV1,
  resolveClusterDeploymentV1,
  type DeploymentIdentityClientV1,
} from './deployment';

export type ProgramRoleV1 = 'registry' | 'core' | 'claims' | 'trading' | 'resolution' | 'custody' | 'rentCredit';

/** Every role a caller can name, in one place, so a new one cannot be missed. */
export const PROGRAM_ROLES_V1: ReadonlyArray<ProgramRoleV1> = Object.freeze([
  'registry', 'core', 'claims', 'trading', 'resolution', 'custody', 'rentCredit',
]);

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
 * - the compact `{ schema: 'dclutch-cli-session-v1' }` file `dclutch-terminal found`
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
  /** The deployment `--cluster` named, or null when the caller named none. */
  deployment: DeploymentV1 | null;
}>;

export function resolveContext(flags: Readonly<Record<string, string | boolean | undefined>>, env: NodeJS.ProcessEnv): CliContext {
  const sessionPath = typeof flags.session === 'string' ? flags.session : env.DCLUTCH_SESSION;
  const session = sessionPath !== undefined && sessionPath !== '' ? loadSession(sessionPath) : EMPTY_SESSION;
  const deployment = resolveClusterDeploymentV1(flags.cluster);
  // A named cluster carries its own endpoint, so `--cluster devnet` alone is a
  // complete instruction. It is still the LAST word before the loopback
  // default: an explicit `--rpc`, `$DCLUTCH_RPC`, or a session file's own URL
  // is what the caller said out loud, and `assertDeploymentIdentityV1` is what
  // proves whichever of them won is really the named chain.
  const rpcUrl = (typeof flags.rpc === 'string' ? flags.rpc : undefined)
    ?? env.DCLUTCH_RPC
    ?? session.rpcUrl
    ?? deployment?.endpoint
    ?? 'http://127.0.0.1:20890/';
  return Object.freeze({ rpcUrl, session, flags, json: flags.json === true, deployment });
}

export function rpcClient(context: CliContext): SolanaRpcClient {
  return new SolanaRpcClient(context.rpcUrl);
}

/**
 * Prove the endpoint is the named chain when — and only when — a program id in
 * this invocation would come from the shipped manifest rather than the caller.
 *
 * A caller who named every id explicitly gets no extra round trip and no new
 * refusal; they already said what they meant, and this client is not the
 * authority on their validator. This is the boundary that keeps "the manifest
 * says these are the programs" from becoming a claim about an endpoint nobody
 * checked. Call it before the first chain read of a command, not once per
 * process: admissions are deliberately not cached.
 */
export async function bindDeploymentIdentity(
  context: CliContext,
  client: DeploymentIdentityClientV1,
  boundary: string,
): Promise<MutationClusterAdmissionV1 | null> {
  if (context.deployment === null || !resolvesAnyProgramFromDeployment(context)) return null;
  return assertDeploymentIdentityV1(client, context.deployment, boundary);
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

/**
 * A program id the CALLER stated: an explicit role flag, or a session file.
 *
 * Separated from `programId` because the deployment manifest is a different
 * kind of answer — one this client ships rather than one the invocation
 * carries — and the identity boundary needs to know which kind it is about to
 * use before it spends a round trip proving the endpoint.
 */
export function statedProgramId(context: CliContext, role: ProgramRoleV1): string | null {
  const flag = context.flags[FLAG_BY_ROLE[role]];
  if (typeof flag === 'string' && flag.length > 0) return flag;
  return context.session.programs[role] ?? null;
}

/** Whether any role in this invocation would come from the shipped manifest. */
export function resolvesAnyProgramFromDeployment(context: CliContext): boolean {
  if (context.deployment === null) return false;
  return PROGRAM_ROLES_V1.some((role) => statedProgramId(context, role) === null);
}

/** A program id, or an error that says every place it could have come from. */
export function programId(context: CliContext, role: ProgramRoleV1): string {
  const stated = statedProgramId(context, role);
  if (stated !== null) return stated;
  if (context.deployment !== null) return deploymentProgramIdV1(context.deployment, role);
  throw new Error(`the ${role} program id is not known: pass --cluster <devnet|local>, --${FLAG_BY_ROLE[role]} <address>, or --session <spec/evidence/session json>`);
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
