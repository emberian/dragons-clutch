/**
 * `dclutch-terminal join` — admit one participant into an already-founded market.
 *
 * This file owns no protocol formula, derives no address, and builds no
 * transaction. The Rust successor's User Position admission remains the sole
 * author of the admission message, its rent and fee arithmetic, its durable
 * report, and every signature; this exterior only names the child's exact
 * inputs and hands them over.
 *
 * Two invariants are structural rather than advisory:
 *
 *   - The subcommand and the devnet acknowledgment are chosen together in
 *     `joinArgumentsV1`, so a loopback origin can never carry
 *     `--i-mean-devnet` and an external origin can never omit it.
 *   - The CLI reads a key file only for its PUBLIC key, and passes the file
 *     path through to the child, which is the process that signs. No secret
 *     byte is retained, printed, or written anywhere by this command.
 *
 * The finalized floor (`--minimum-finalized-slot`) is read from the endpoint
 * only when the caller did not state one, and only inside a fresh cluster
 * admission — a slot observed on some other chain is not a floor.
 */
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { isAbsolute } from 'node:path';

import type { SolanaRpcClient } from '@dclutch/sdk/rpc';
import { PublicKey } from '@solana/web3.js';

import { keypairPath, loadKeypair, rpcClient, type CliContext } from '../context';
import { assertExactDevnetMutation, devnetGenesisAcknowledgment } from '../mutation';
import { block, type Io } from '../output';
import { successorBinary } from '../successor';

export const JOIN_DEVNET_COMMAND_V1 = 'devnet-user-position-admission-v1';
export const JOIN_OWNED_LOOPBACK_COMMAND_V1 = 'local-private-validator-user-position-admission-v1';

const DEVNET_REPORT_SCHEMA_V1 = 'dclutch-devnet-user-position-admission-execution-v1';
const OWNED_LOOPBACK_REPORT_SCHEMA_V1 = 'dclutch-owned-loopback-user-position-admission-execution-v1';

const MAX_CHILD_OUTPUT_BYTES = 16 * 1024 * 1024;
const MAX_REPORT_BYTES = 16 * 1024 * 1024;
const MAX_STDERR_CHARACTERS = 4_096;
const U64_MAXIMUM = 18_446_744_073_709_551_615n;

export type JoinClusterV1 = 'devnet' | 'owned-loopback';

export type JoinSpawnResultV1 = Readonly<{
  status: number | null;
  signal: string | null;
  stdout: string | null;
  stderr: string | null;
  error?: Error;
}>;

export type JoinSpawnV1 = (
  binary: string,
  args: ReadonlyArray<string>,
  options: Readonly<{ encoding: 'utf8'; env: NodeJS.ProcessEnv }>,
) => JoinSpawnResultV1;

/** The only endpoint capability `join` uses: identity, then one floor. */
export type JoinRpcClientV1 = Pick<SolanaRpcClient, 'assertMutationCluster' | 'finalizedSlot'>;

export type JoinDependenciesV1 = Readonly<{
  spawn: JoinSpawnV1;
  client: (context: CliContext) => JoinRpcClientV1;
}>;

const DEFAULT_JOIN_DEPENDENCIES_V1: JoinDependenciesV1 = Object.freeze({
  spawn: (binary, args, options) => spawnSync(binary, [...args], {
    ...options,
    maxBuffer: MAX_CHILD_OUTPUT_BYTES,
    stdio: ['ignore', 'pipe', 'pipe'],
  }),
  client: rpcClient,
});

export type JoinCollateralV1 = Readonly<{
  sourceOwner: string;
  sourceOwnerKeypair: string;
  sourceAccount: string;
  quantityAtoms: string;
}>;

export type JoinInvocationV1 = Readonly<{
  cluster: JoinClusterV1;
  rpcUrl: string;
  /** Present for devnet, absent for an owned loopback. Never both, never neither. */
  acknowledgment: string | null;
  plan: string;
  campaignEvidence: string;
  positionOwner: string;
  positionOwnerKeypair: string;
  feePayer: string;
  feePayerKeypair: string;
  minimumFinalizedSlot: string;
  output: string;
  execute: boolean;
  collateral: JoinCollateralV1 | null;
}>;

/**
 * The exact child argv, in the successor's own documented order.
 *
 * Exported because the argv IS the interface: a test that reads it reads what
 * the protocol's admission driver will actually be told.
 */
export function joinArgumentsV1(invocation: JoinInvocationV1): ReadonlyArray<string> {
  const args: string[] = [];
  if (invocation.cluster === 'devnet') {
    if (invocation.acknowledgment === null) {
      throw new Error('pass --i-mean-devnet <full devnet genesis hash>');
    }
    args.push(JOIN_DEVNET_COMMAND_V1, '--rpc-url', invocation.rpcUrl, '--i-mean-devnet', invocation.acknowledgment);
  } else {
    if (invocation.acknowledgment !== null) {
      throw new Error(`--i-mean-devnet was given for the loopback origin ${invocation.rpcUrl}; a loopback origin needs no acknowledgment, so one of the two is a mistake and this refuses rather than guessing which`);
    }
    args.push(JOIN_OWNED_LOOPBACK_COMMAND_V1, '--rpc-url', invocation.rpcUrl);
  }
  args.push(
    '--plan', invocation.plan,
    '--campaign-evidence', invocation.campaignEvidence,
    '--position-owner', invocation.positionOwner,
    '--position-owner-keypair', invocation.positionOwnerKeypair,
    '--fee-payer', invocation.feePayer,
    '--fee-payer-keypair', invocation.feePayerKeypair,
    '--minimum-finalized-slot', invocation.minimumFinalizedSlot,
    '--output', invocation.output,
  );
  if (invocation.execute) args.push('--execute');
  if (invocation.collateral !== null) {
    args.push(
      '--collateral-source-owner', invocation.collateral.sourceOwner,
      '--collateral-source-owner-keypair', invocation.collateral.sourceOwnerKeypair,
      '--collateral-source-account', invocation.collateral.sourceAccount,
      '--collateral-quantity-atoms', invocation.collateral.quantityAtoms,
    );
  }
  return Object.freeze(args);
}

function isLoopbackHost(host: string): boolean {
  if (host === 'localhost' || host === '::1') return true;
  const octets = host.split('.');
  return octets.length === 4
    && octets.every((octet) => /^(0|[1-9][0-9]{0,2})$/.test(octet) && Number(octet) <= 255)
    && octets[0] === '127';
}

/**
 * Which admission subcommand this origin selects.
 *
 * A loopback HOST that fails the loopback SHAPE is a spelling to fix, not a
 * cluster to acknowledge: telling that caller to pass `--i-mean-devnet` would
 * be advice toward the wrong fix, so this refuses instead. Same reading the
 * successor's own origin parser gives.
 */
export function joinClusterV1(rpcUrl: string): JoinClusterV1 {
  let url: URL;
  try {
    url = new URL(rpcUrl);
  } catch {
    throw new Error(`the resolved RPC endpoint ${rpcUrl} is not a URL`);
  }
  const host = url.hostname.toLowerCase().replace(/^\[/, '').replace(/\]$/, '');
  if (!isLoopbackHost(host)) return 'devnet';
  if (url.protocol !== 'http:' || url.username !== '' || url.password !== ''
      || url.port === '' || url.pathname !== '/' || url.search !== '' || url.hash !== ''
      || host !== '127.0.0.1') {
    throw new Error(`the RPC origin ${rpcUrl} names a loopback host but is not the credential-free explicit-port http://127.0.0.1:PORT/ origin the successor answers on; this is a spelling to fix, not a cluster to acknowledge`);
  }
  return 'owned-loopback';
}

function absoluteFlagV1(context: CliContext, name: string, placeholder: string): string {
  const value = context.flags[name];
  if (typeof value !== 'string' || value === '') throw new Error(`pass --${name} <${placeholder}>`);
  if (!isAbsolute(value)) throw new Error(`--${name} must be an absolute path; ${value} is not`);
  return value;
}

function absoluteKeypairPathV1(context: CliContext, env: NodeJS.ProcessEnv, flag: string): string {
  const path = keypairPath(context, env, flag);
  if (!isAbsolute(path)) throw new Error(`--${flag} must be an absolute path; ${path} is not`);
  return path;
}

function decimalU64V1(value: string, label: string, positive: boolean): string {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${label} must be a canonical decimal u64`);
  const parsed = BigInt(value);
  if (parsed > U64_MAXIMUM) throw new Error(`${label} exceeds u64`);
  if (positive && parsed === 0n) throw new Error(`${label} must be nonzero`);
  return value;
}

function addressV1(value: string, label: string): string {
  let canonical: string;
  try {
    canonical = new PublicKey(value).toBase58();
  } catch {
    throw new Error(`${label} is not a base58 address`);
  }
  if (canonical !== value) throw new Error(`${label} is not a canonical base58 address`);
  return canonical;
}

/** The collateral tuple as stated: all three flags or none, and no file read. */
type StatedCollateralV1 = Omit<JoinCollateralV1, 'sourceOwner'>;

function statedCollateralV1(context: CliContext, env: NodeJS.ProcessEnv): StatedCollateralV1 | null {
  const keypairFlag = context.flags['collateral-source-owner-keypair'];
  const account = context.flags['collateral-source-account'];
  const quantity = context.flags['collateral-quantity-atoms'];
  const stated = [keypairFlag, account, quantity].filter((value) => typeof value === 'string' && value !== '');
  if (stated.length === 0) return null;
  if (stated.length !== 3) {
    throw new Error('funding the admitted position requires all three of --collateral-source-owner-keypair, --collateral-source-account, and --collateral-quantity-atoms, or none of them');
  }
  return Object.freeze({
    sourceOwnerKeypair: absoluteKeypairPathV1(context, env, 'collateral-source-owner-keypair'),
    sourceAccount: addressV1(account as string, '--collateral-source-account'),
    quantityAtoms: decimalU64V1(quantity as string, '--collateral-quantity-atoms', true),
  });
}

/**
 * The finalized floor, from the caller or from the endpoint's own identity.
 *
 * Reading a slot is the one thing this command does on the network, so it is
 * done inside a fresh admission of the cluster the invocation named.
 */
async function minimumFinalizedSlotV1(
  context: CliContext,
  cluster: JoinClusterV1,
  acknowledgment: string | null,
  dependencies: JoinDependenciesV1,
): Promise<string> {
  const stated = context.flags['minimum-finalized-slot'];
  if (typeof stated === 'string' && stated !== '') {
    return decimalU64V1(stated, '--minimum-finalized-slot', true);
  }
  const client = dependencies.client(context);
  if (cluster === 'devnet') {
    await assertExactDevnetMutation(client, acknowledgment ?? '', 'join finalized floor');
  } else {
    const admission = await client.assertMutationCluster();
    if (admission.kind !== 'loopback-local-validator') {
      throw new Error(`join finalized floor refused: ${context.rpcUrl} is addressed as a loopback validator but the endpoint reports ${admission.kind}`);
    }
  }
  return decimalU64V1(await client.finalizedSlot(), 'the endpoint\'s finalized slot', true);
}

function boundedStderrV1(value: string | null): string {
  const text = value?.trim() ?? '';
  return text.length > MAX_STDERR_CHARACTERS ? `${text.slice(0, MAX_STDERR_CHARACTERS)}…` : text;
}

function successorReportV1(path: string, cluster: JoinClusterV1): Record<string, unknown> {
  if (!existsSync(path)) throw new Error(`the admission driver exited 0 without writing its report at ${path}`);
  const stats = statSync(path);
  if (!stats.isFile() || stats.size === 0 || stats.size > MAX_REPORT_BYTES) {
    throw new Error(`the admission report ${path} is not a bounded regular file`);
  }
  const value: unknown = JSON.parse(readFileSync(path, 'utf8'));
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`the admission report ${path} is not one JSON object`);
  }
  const report = value as Record<string, unknown>;
  const schema = cluster === 'devnet' ? DEVNET_REPORT_SCHEMA_V1 : OWNED_LOOPBACK_REPORT_SCHEMA_V1;
  if (report.schema !== schema || report.cluster !== cluster) {
    throw new Error(`the admission report ${path} is not this cluster's ${schema}`);
  }
  return report;
}

/**
 * `dclutch-terminal join`: preflight by default, admission under `--execute`.
 *
 * Preflight is the successor's finalized read-only planning run. It writes the
 * same durable report the execution later resumes, and passes no `--execute`
 * to the child, so no key file is read for signing on that path.
 */
export async function join(
  context: CliContext,
  io: Io,
  env: NodeJS.ProcessEnv,
  dependencies: JoinDependenciesV1 = DEFAULT_JOIN_DEPENDENCIES_V1,
): Promise<number> {
  const plan = absoluteFlagV1(context, 'plan', 'absolute successor plan json');
  const campaignEvidence = absoluteFlagV1(context, 'campaign-evidence', 'absolute completed founding campaign evidence json');
  const output = absoluteFlagV1(context, 'output', 'absolute admission report path');
  const execute = context.flags.execute === true;

  const cluster = joinClusterV1(context.rpcUrl);
  const acknowledgment = cluster === 'devnet' ? devnetGenesisAcknowledgment(context) : null;
  if (cluster === 'owned-loopback' && context.flags['i-mean-devnet'] !== undefined) {
    throw new Error(`--i-mean-devnet was given for the loopback origin ${context.rpcUrl}; a loopback origin needs no acknowledgment, so one of the two is a mistake and this refuses rather than guessing which`);
  }

  const binary = successorBinary(context, env);
  // Every path and tuple the caller stated is checked before the endpoint is
  // touched and before any key file is opened.
  const stated = statedCollateralV1(context, env);
  const positionOwnerKeypair = absoluteKeypairPathV1(context, env, 'keypair');
  const feePayerStated = typeof context.flags['fee-payer-keypair'] === 'string' && context.flags['fee-payer-keypair'] !== '';
  const feePayerKeypair = feePayerStated ? absoluteKeypairPathV1(context, env, 'fee-payer-keypair') : positionOwnerKeypair;
  const minimumFinalizedSlot = await minimumFinalizedSlotV1(context, cluster, acknowledgment, dependencies);

  // The child signs; this process only needs each signer's public key, so the
  // key files are read here, last, and only through `loadKeypair`.
  const positionOwner = loadKeypair(context, env).publicKey.toBase58();
  const feePayer = feePayerStated ? loadKeypair(context, env, 'fee-payer-keypair').publicKey.toBase58() : positionOwner;
  const collateral: JoinCollateralV1 | null = stated === null ? null : Object.freeze({
    sourceOwner: loadKeypair(context, env, 'collateral-source-owner-keypair').publicKey.toBase58(),
    ...stated,
  });

  const args = joinArgumentsV1({
    cluster,
    rpcUrl: context.rpcUrl,
    acknowledgment,
    plan,
    campaignEvidence,
    positionOwner,
    positionOwnerKeypair,
    feePayer,
    feePayerKeypair,
    minimumFinalizedSlot,
    output,
    execute,
    collateral,
  });

  const progress: Io = context.json ? Object.freeze({ out: io.err, err: io.err }) : io;
  progress.out(`${cluster} participant admission via ${binary}`);
  progress.out(`  ${execute ? 'executing' : 'preflight only (pass --execute to admit)'} at finalized floor ${minimumFinalizedSlot}`);

  const result = dependencies.spawn(binary, args, { encoding: 'utf8', env });
  if (result.error !== undefined) {
    throw new Error(`the participant admission driver could not start: ${result.error.message}`);
  }
  const stderr = boundedStderrV1(result.stderr);
  if (stderr !== '') io.err(stderr);
  if (result.status !== 0) {
    throw new Error(`participant admission exited ${result.status ?? `by signal ${result.signal ?? 'unknown'}`}${stderr === '' ? '' : `: ${stderr}`}`);
  }

  const report = successorReportV1(output, cluster);
  const phase = typeof report.phase === 'string' ? report.phase : 'unstated';
  const authorized = report.authorizedMutation === true;
  if (execute && !authorized) {
    throw new Error(`the admission report ${output} does not record the authorization --execute requested`);
  }
  if (context.json) {
    io.out(JSON.stringify({
      cluster,
      execute,
      phase,
      authorizedMutation: authorized,
      minimumFinalizedSlot,
      positionOwner,
      feePayer,
      collateral: collateral === null ? null : { sourceOwner: collateral.sourceOwner, sourceAccount: collateral.sourceAccount, quantityAtoms: collateral.quantityAtoms },
      report: output,
    }));
    return 0;
  }
  io.out(`participant admission report ${output}`);
  block(io, [
    ['phase', phase],
    ['authorized mutation', authorized ? 'yes' : 'no (read-only planning)'],
    ['position owner', positionOwner],
    ['fee payer', feePayer === positionOwner ? `${feePayer} (the position owner)` : feePayer],
    ['collateral', collateral === null ? 'none requested' : `${collateral.quantityAtoms} atoms from ${collateral.sourceAccount}`],
  ]);
  return 0;
}
