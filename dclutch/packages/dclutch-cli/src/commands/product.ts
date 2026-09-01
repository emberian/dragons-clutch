/**
 * Public, key-free entrance to the authoritative spline Product compiler.
 * This exterior checks only the caller boundary and admits the Rust
 * successor's machine completion; it owns no Product, basis, gate, or PDA fact.
 */
import { spawnSync } from 'node:child_process';
import { readFileSync, realpathSync, statSync } from 'node:fs';
import { basename, dirname, isAbsolute, join } from 'node:path';

import {
  inspectSplineProductAuthoringArtifactsV1,
  type InspectedSplineProductArtifactsV1,
  type SplineProductArtifactFilesV1,
} from '@dclutch/sdk';

import type { CliContext } from '../context';
import type { Io } from '../output';
import { successorBinary } from '../successor';

export const SPLINE_PRODUCT_COMMAND_V1 = 'product-spline-compile-v1';
export const SPLINE_PRODUCT_COMPLETION_SCHEMA_V1 = 'dclutch/product-spline-authoring-completion/v1';
export const SPLINE_PRODUCT_INSPECTION_SCHEMA_V1 = 'dclutch/product-spline-inspection/v1';

const MAX_CHILD_OUTPUT_BYTES = 16 * 1024 * 1024;
const MAX_STDERR_CHARACTERS = 4_096;
const MAX_LOCAL_ARTIFACT_BYTES = 1_000_000;

const SPLINE_PRODUCT_FILES_V1 = Object.freeze({
  product: 'product.bin',
  resultDomain: 'result-domain.bin',
  portfolio: 'portfolio.bin',
  productBasis: 'product-basis.bin',
  priceGate: 'price-gate.bin',
} as const);

export type ProductSpawnResultV1 = Readonly<{
  status: number | null;
  signal: string | null;
  stdout: string | null;
  stderr: string | null;
  error?: Error;
}>;

export type ProductCommandDependenciesV1 = Readonly<{
  binary: typeof successorBinary;
  spawn: (
    binary: string,
    args: ReadonlyArray<string>,
    options: Readonly<{ encoding: 'utf8'; env: NodeJS.ProcessEnv }>,
  ) => ProductSpawnResultV1;
}>;

const PRODUCT_COMMAND_DEPENDENCIES_V1: ProductCommandDependenciesV1 = Object.freeze({
  binary: successorBinary,
  spawn: (binary, args, options) => spawnSync(binary, [...args], {
    ...options,
    maxBuffer: MAX_CHILD_OUTPUT_BYTES,
    stdio: ['ignore', 'pipe', 'pipe'],
  }),
});

export type ProductInspectionDependenciesV1 = Readonly<{
  canonicalPath: (path: string) => string;
  metadata: (path: string) => Readonly<{ bytes: number; isFile: boolean }>;
  read: (path: string) => Uint8Array;
  inspect: typeof inspectSplineProductAuthoringArtifactsV1;
}>;

const PRODUCT_INSPECTION_DEPENDENCIES_V1: ProductInspectionDependenciesV1 = Object.freeze({
  canonicalPath: realpathSync,
  metadata: (path) => { const value = statSync(path); return Object.freeze({ bytes: value.size, isFile: value.isFile() }); },
  read: (path) => new Uint8Array(readFileSync(path)),
  inspect: inspectSplineProductAuthoringArtifactsV1,
});

function absoluteFlagV1(context: CliContext, name: string): string {
  const value = context.flags[name];
  if (typeof value !== 'string' || value === '') throw new Error(`pass --${name} <absolute path>`);
  if (!isAbsolute(value)) throw new Error(`--${name} must be an absolute path; ${value} is not`);
  return value;
}

export function splineProductArgumentsV1(input: string, outputDirectory: string): ReadonlyArray<string> {
  return Object.freeze([
    SPLINE_PRODUCT_COMMAND_V1,
    '--input', input,
    '--output-dir', outputDirectory,
  ]);
}

function boundedStderrV1(value: string | null): string {
  const text = value?.trim() ?? '';
  return text.length > MAX_STDERR_CHARACTERS ? `${text.slice(0, MAX_STDERR_CHARACTERS)}…` : text;
}

function localArtifactV1(path: string, noun: string, dependencies: ProductInspectionDependenciesV1): Uint8Array {
  if (!isAbsolute(path)) throw new Error(`${noun} must be an absolute path; ${path} is not`);
  let canonical: string;
  try { canonical = dependencies.canonicalPath(path); } catch (error) {
    throw new Error(`${noun} is unreadable: ${error instanceof Error ? error.message : 'no usable reason'}`);
  }
  if (canonical !== path) throw new Error(`${noun} is not canonical; expected ${canonical}`);
  const metadata = dependencies.metadata(path);
  if (!metadata.isFile || metadata.bytes === 0 || metadata.bytes > MAX_LOCAL_ARTIFACT_BYTES) {
    throw new Error(`${noun} must be a regular file of 1..=${MAX_LOCAL_ARTIFACT_BYTES.toLocaleString()} bytes`);
  }
  const value = new Uint8Array(dependencies.read(path));
  if (value.length !== metadata.bytes) throw new Error(`${noun} changed while it was being read`);
  return value;
}

function inspectionDocumentV1(report: string, inspected: InspectedSplineProductArtifactsV1) {
  const records = Object.fromEntries(Object.entries(inspected.records).map(([name, record]) => [name, Object.freeze({
    file: record.file,
    bytes: record.bytes.length,
    schema_id: record.schemaId,
    content_sha256: record.contentSha256,
    raw_account: record.rawAccount,
    staging_account: record.stagingAccount,
  })]));
  return Object.freeze({
    schema: SPLINE_PRODUCT_INSPECTION_SCHEMA_V1,
    report,
    key_free: inspected.keyFree,
    signs: inspected.signs,
    submits: inspected.submits,
    input_sha256: inspected.inputSha256,
    registry_program: inspected.registryProgram,
    product_outcome_count: inspected.productOutcomeCount,
    basis_width: inspected.basisWidth,
    degree: inspected.degree,
    interior_multiplicity: inspected.interiorMultiplicity,
    payout_scale: inspected.payoutScale,
    rounding_boundary: inspected.roundingBoundary,
    semantic_basis_id: inspected.semanticBasisId,
    records: Object.freeze(records),
    verified_price_gate: inspected.verifiedPriceGate,
    found_records: inspected.foundRecords,
  });
}

async function inspectProductV1(
  context: CliContext,
  io: Io,
  dependencies: ProductInspectionDependenciesV1,
): Promise<number> {
  const reportPath = absoluteFlagV1(context, 'report');
  if (basename(reportPath) !== 'report.json') throw new Error('--report must name the compiler output report.json');
  const reportBytes = localArtifactV1(reportPath, 'spline Product report', dependencies);
  let report: unknown;
  try { report = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(reportBytes)); } catch {
    throw new Error('spline Product report is not canonical UTF-8 JSON');
  }
  const directory = dirname(reportPath);
  const files = Object.fromEntries(Object.entries(SPLINE_PRODUCT_FILES_V1).map(([key, file]) => [
    key,
    localArtifactV1(join(directory, file), `spline Product artifact ${file}`, dependencies),
  ])) as SplineProductArtifactFilesV1;
  const inspected = await dependencies.inspect(report, Object.freeze(files));
  const document = inspectionDocumentV1(reportPath, inspected);
  if (context.json) {
    io.out(JSON.stringify(document));
    return 0;
  }
  io.out(`verified spline Product compiler handoff at ${reportPath}`);
  io.out(`  degree ${document.degree} · basis width ${document.basis_width} · ${document.product_outcome_count} Product outcomes · ${document.rounding_boundary}`);
  io.out(`  Registry ${document.registry_program} · semantic basis ${document.semantic_basis_id}`);
  for (const [field, address] of Object.entries(document.found_records)) io.out(`  ${field} ${address}`);
  io.out('  local inspection only; no chain read, wallet, signature, transaction, publication, or submission');
  io.out('  Found must still authenticate these five records live against the Registry');
  return 0;
}

export type SplineProductCompletionV1 = Readonly<{
  schema: typeof SPLINE_PRODUCT_COMPLETION_SCHEMA_V1;
  output_dir: string;
  report: string;
  report_sha256: string;
}>;

function completionV1(stdout: string | null, outputDirectory: string): SplineProductCompletionV1 {
  const text = stdout?.trim() ?? '';
  if (text === '') throw new Error('the spline Product compiler exited 0 without a machine completion');
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error('the spline Product compiler did not emit one JSON machine completion');
  }
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('the spline Product compiler completion is not one object');
  }
  const candidate = value as Record<string, unknown>;
  const expectedReport = join(outputDirectory, 'report.json');
  if (candidate.schema !== SPLINE_PRODUCT_COMPLETION_SCHEMA_V1
      || candidate.output_dir !== outputDirectory
      || candidate.report !== expectedReport
      || typeof candidate.report_sha256 !== 'string'
      || !/^[0-9a-f]{64}$/.test(candidate.report_sha256)) {
    throw new Error(`the spline Product compiler completion does not bind ${outputDirectory} and its canonical report`);
  }
  return Object.freeze({
    schema: SPLINE_PRODUCT_COMPLETION_SCHEMA_V1,
    output_dir: outputDirectory,
    report: expectedReport,
    report_sha256: candidate.report_sha256,
  });
}

/** Compile one canonical spline Product graph through the Rust semantic owner. */
export async function productCommand(
  context: CliContext,
  io: Io,
  subcommand: string | undefined,
  env: NodeJS.ProcessEnv,
  dependencies: ProductCommandDependenciesV1 = PRODUCT_COMMAND_DEPENDENCIES_V1,
  inspectionDependencies: ProductInspectionDependenciesV1 = PRODUCT_INSPECTION_DEPENDENCIES_V1,
): Promise<number> {
  if (subcommand === 'inspect') return inspectProductV1(context, io, inspectionDependencies);
  if (subcommand !== 'spline') throw new Error('usage: dclutch product spline --input <absolute canonical json> --output-dir <absolute new directory> | dclutch product inspect --report <absolute report.json>');
  const input = absoluteFlagV1(context, 'input');
  const outputDirectory = absoluteFlagV1(context, 'output-dir');
  const args = splineProductArgumentsV1(input, outputDirectory);
  const binary = dependencies.binary(context, env);
  const result = dependencies.spawn(binary, args, { encoding: 'utf8', env });
  if (result.error !== undefined) throw new Error(`the spline Product compiler could not start: ${result.error.message}`);
  const stderr = boundedStderrV1(result.stderr);
  if (stderr !== '') io.err(stderr);
  if (result.status !== 0) {
    throw new Error(`the spline Product compiler exited ${result.status ?? `by signal ${result.signal ?? 'unknown'}`}${stderr === '' ? '' : `: ${stderr}`}`);
  }
  const completion = completionV1(result.stdout, outputDirectory);
  if (context.json) {
    io.out(JSON.stringify(completion));
    return 0;
  }
  io.out(`canonical spline Product graph written to ${outputDirectory}`);
  io.out(`  report ${completion.report} · sha256 ${completion.report_sha256}`);
  io.out('  key-free compiler; no wallet, signature, transaction, or submission');
  return 0;
}
