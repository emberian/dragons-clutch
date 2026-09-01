/**
 * Public, key-free production of the checked evidence a Direct caller needs.
 *
 * The Rust successor remains the only author of both artifacts. This exterior
 * names its exact inputs, pins every local byte source by SHA-256, invokes its
 * read-only devnet commands, and admits only their machine reports. It derives
 * no account, release identity, route row, or lookup-table fact itself.
 */
import { spawnSync } from 'node:child_process';
import { isAbsolute } from 'node:path';

import type { CliContext } from '../context';
import { devnetGenesisAcknowledgment } from '../mutation';
import type { Io } from '../output';
import { successorBinary } from '../successor';

export const CHECKED_EXECUTION_RELEASE_COMMAND_V1 = 'devnet-checked-execution-release-v1';
export const DIRECT_HOT_ROUTE_COMMAND_V3 = 'devnet-direct-hot-route-manifest-v3';

const MAX_CHILD_OUTPUT_BYTES = 16 * 1024 * 1024;
const MAX_STDERR_CHARACTERS = 4_096;

type CheckedRoleV1 = 'core' | 'claims' | 'trading' | 'resolution' | 'custody';
const CHECKED_ROLE_ORDER_V1: ReadonlyArray<CheckedRoleV1> = Object.freeze([
  'core', 'claims', 'trading', 'resolution', 'custody',
]);

export type CheckedExecutionReleaseInvocationV1 = Readonly<{
  rpcUrl: string;
  acknowledgment: string;
  plan: string;
  planSha256: string;
  checked: Readonly<Record<CheckedRoleV1, Readonly<{ path: string; sha256: string }>>>;
  output: string;
}>;

export type DirectHotRouteInvocationV3 = Readonly<{
  rpcUrl: string;
  acknowledgment: string;
  session: string;
  checkedExecutionRelease: Readonly<{ path: string; sha256: string }>;
  registryChecked: Readonly<{ path: string; sha256: string }>;
  rentChecked: Readonly<{ path: string; sha256: string }>;
  output: string;
}>;

export function checkedExecutionReleaseArgumentsV1(
  invocation: CheckedExecutionReleaseInvocationV1,
): ReadonlyArray<string> {
  const args = [
    CHECKED_EXECUTION_RELEASE_COMMAND_V1,
    '--rpc-url', invocation.rpcUrl,
    '--i-mean-devnet', invocation.acknowledgment,
    '--plan', invocation.plan,
    '--expected-plan-sha256', invocation.planSha256,
  ];
  for (const role of CHECKED_ROLE_ORDER_V1) {
    args.push(
      `--${role}-checked`, invocation.checked[role].path,
      `--expected-${role}-checked-sha256`, invocation.checked[role].sha256,
    );
  }
  args.push('--output', invocation.output);
  return Object.freeze(args);
}

export function directHotRouteArgumentsV3(
  invocation: DirectHotRouteInvocationV3,
): ReadonlyArray<string> {
  return Object.freeze([
    DIRECT_HOT_ROUTE_COMMAND_V3,
    '--rpc-url', invocation.rpcUrl,
    '--i-mean-devnet', invocation.acknowledgment,
    '--session', invocation.session,
    '--checked-execution-release', invocation.checkedExecutionRelease.path,
    '--expected-checked-execution-release-sha256', invocation.checkedExecutionRelease.sha256,
    '--registry-checked', invocation.registryChecked.path,
    '--expected-registry-checked-sha256', invocation.registryChecked.sha256,
    '--rent-checked', invocation.rentChecked.path,
    '--expected-rent-checked-sha256', invocation.rentChecked.sha256,
    '--output', invocation.output,
  ]);
}

export type RouteSpawnResultV1 = Readonly<{
  status: number | null;
  signal: string | null;
  stdout: string | null;
  stderr: string | null;
  error?: Error;
}>;

export type RouteCommandDependenciesV1 = Readonly<{
  binary: typeof successorBinary;
  spawn: (
    binary: string,
    args: ReadonlyArray<string>,
    options: Readonly<{ encoding: 'utf8'; env: NodeJS.ProcessEnv }>,
  ) => RouteSpawnResultV1;
}>;

const ROUTE_COMMAND_DEPENDENCIES_V1: RouteCommandDependenciesV1 = Object.freeze({
  binary: successorBinary,
  spawn: (binary, args, options) => spawnSync(binary, [...args], {
    ...options,
    maxBuffer: MAX_CHILD_OUTPUT_BYTES,
    stdio: ['ignore', 'pipe', 'pipe'],
  }),
});

function absoluteFlagV1(context: CliContext, name: string): string {
  const value = context.flags[name];
  if (typeof value !== 'string' || value === '') throw new Error(`pass --${name} <absolute path>`);
  if (!isAbsolute(value)) throw new Error(`--${name} must be an absolute path; ${value} is not`);
  return value;
}

function sha256FlagV1(context: CliContext, name: string): string {
  const value = context.flags[name];
  if (typeof value !== 'string' || !/^[0-9a-f]{64}$/.test(value)) {
    throw new Error(`pass --${name} <exact lowercase SHA-256 hex>`);
  }
  return value;
}

function boundedStderrV1(value: string | null): string {
  const text = value?.trim() ?? '';
  return text.length > MAX_STDERR_CHARACTERS ? `${text.slice(0, MAX_STDERR_CHARACTERS)}…` : text;
}

function parseReportV1(
  stdout: string | null,
  expectedSchema: string,
  expectedOutput: string,
): Readonly<Record<string, unknown>> {
  const text = stdout?.trim() ?? '';
  if (text === '') throw new Error('the read-only route producer exited 0 without a machine report');
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error('the read-only route producer did not emit one JSON machine report');
  }
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('the read-only route producer report is not one object');
  }
  const report = value as Record<string, unknown>;
  if (report.schema !== expectedSchema || report.output !== expectedOutput) {
    throw new Error(`the read-only route producer report is not ${expectedSchema} for ${expectedOutput}`);
  }
  return Object.freeze(report);
}

function runProducerV1(
  binary: string,
  args: ReadonlyArray<string>,
  env: NodeJS.ProcessEnv,
  dependencies: RouteCommandDependenciesV1,
  expectedSchema: string,
  expectedOutput: string,
  io: Io,
): Readonly<Record<string, unknown>> {
  const result = dependencies.spawn(binary, args, { encoding: 'utf8', env });
  if (result.error !== undefined) throw new Error(`the read-only route producer could not start: ${result.error.message}`);
  const stderr = boundedStderrV1(result.stderr);
  if (stderr !== '') io.err(stderr);
  if (result.status !== 0) {
    throw new Error(`the read-only route producer exited ${result.status ?? `by signal ${result.signal ?? 'unknown'}`}${stderr === '' ? '' : `: ${stderr}`}`);
  }
  return parseReportV1(result.stdout, expectedSchema, expectedOutput);
}

function reportLineV1(report: Readonly<Record<string, unknown>>, field: string): string {
  const value = report[field];
  return typeof value === 'string' || typeof value === 'number' ? String(value) : 'not stated';
}

/** Produce one checked multiprogram release set or one public Direct route. */
export async function routeCommand(
  context: CliContext,
  io: Io,
  subcommand: string | undefined,
  env: NodeJS.ProcessEnv,
  dependencies: RouteCommandDependenciesV1 = ROUTE_COMMAND_DEPENDENCIES_V1,
): Promise<number> {
  if (subcommand !== 'release-set' && subcommand !== 'direct') {
    throw new Error('usage: dclutch route release-set|direct [pinned producer inputs] --output <absolute new file>');
  }
  const acknowledgment = devnetGenesisAcknowledgment(context);
  const output = absoluteFlagV1(context, 'output');
  let args: ReadonlyArray<string>;
  let schema: string;
  if (subcommand === 'release-set') {
    const checked = {} as Record<CheckedRoleV1, { path: string; sha256: string }>;
    for (const role of CHECKED_ROLE_ORDER_V1) {
      checked[role] = Object.freeze({
        path: absoluteFlagV1(context, `${role}-checked`),
        sha256: sha256FlagV1(context, `expected-${role}-checked-sha256`),
      });
    }
    args = checkedExecutionReleaseArgumentsV1({
      rpcUrl: context.rpcUrl,
      acknowledgment,
      plan: absoluteFlagV1(context, 'plan'),
      planSha256: sha256FlagV1(context, 'expected-plan-sha256'),
      checked: Object.freeze(checked),
      output,
    });
    schema = 'dclutch-devnet-checked-execution-release-report-v1';
  } else {
    args = directHotRouteArgumentsV3({
      rpcUrl: context.rpcUrl,
      acknowledgment,
      session: absoluteFlagV1(context, 'session'),
      checkedExecutionRelease: Object.freeze({
        path: absoluteFlagV1(context, 'checked-execution-release'),
        sha256: sha256FlagV1(context, 'expected-checked-execution-release-sha256'),
      }),
      registryChecked: Object.freeze({
        path: absoluteFlagV1(context, 'registry-checked'),
        sha256: sha256FlagV1(context, 'expected-registry-checked-sha256'),
      }),
      rentChecked: Object.freeze({
        path: absoluteFlagV1(context, 'rent-checked'),
        sha256: sha256FlagV1(context, 'expected-rent-checked-sha256'),
      }),
      output,
    });
    schema = 'dclutch-devnet-direct-hot-route-manifest-report-v1';
  }

  const binary = dependencies.binary(context, env);
  const report = runProducerV1(binary, args, env, dependencies, schema, output, io);
  if (context.json) {
    io.out(JSON.stringify(report));
    return 0;
  }
  if (subcommand === 'release-set') {
    io.out(`checked execution release set written to ${output}`);
    io.out(`  ${reportLineV1(report, 'bytes')} bytes · sha256 ${reportLineV1(report, 'sha256')} · no key read; no transaction submitted`);
  } else {
    io.out(`portable Direct route manifest written to ${output}`);
    io.out(`  market ${reportLineV1(report, 'market')} · lookup table ${reportLineV1(report, 'lookupTable')}`);
    io.out(`  sha256 ${reportLineV1(report, 'sha256')} · checked infrastructure ${reportLineV1(report, 'checkedInfrastructureSha256')} · no key read; no transaction submitted`);
  }
  return 0;
}
