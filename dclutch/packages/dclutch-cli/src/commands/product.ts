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

/** Basis points as a percentage, exactly: 10,000 bps is one whole unit. */
export function basisPointsV1(value: number): string {
  const whole = Math.trunc(value / 100);
  const hundredths = value % 100;
  return `${whole}.${String(hundredths).padStart(2, '0')}%`;
}

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

/**
 * The two inspected keys this document does NOT project, because the document
 * owns them itself: it carries its own `schema`, and the compiler `command` is
 * a property of the compilation, not of an inspection of its output. Named
 * rather than implied, so that dropping a third one is a visible decision.
 */
const UNPROJECTED_INSPECTION_KEYS_V1: ReadonlyArray<string> = Object.freeze(['schema', 'command']);

/** camelCase to snake_case — the only naming rule between the two spellings. */
export function snakeCaseV1(name: string): string {
  return name.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

/** One record's report row. Explicit because `bytes` is a length, not bytes. */
function recordDocumentV1(record: InspectedSplineProductArtifactsV1['records'][keyof InspectedSplineProductArtifactsV1['records']]) {
  return Object.freeze({
    file: record.file,
    bytes: record.bytes.length,
    schema_id: record.schemaId,
    content_sha256: record.contentSha256,
    raw_account: record.rawAccount,
    staging_account: record.stagingAccount,
  });
}

/**
 * Project the verified artifacts into the report's own snake_case naming,
 * DERIVED FROM THE INSPECTED OBJECT'S OWN KEYS rather than restated.
 *
 * WHY IT IS A LOOP AND NOT A LIST. This function used to name its seventeen
 * fields one at a time and stop at `verified_price_gate`. A document assembled
 * by enumeration does not fail when the object grows a section: it does not
 * warn either, it just quietly describes an older version of the thing. When
 * the compiler grew `partition_quality` — the measurement that says whether an
 * author is about to found a degenerate market — this list would have dropped
 * it, and the one surface where an author can check that number before
 * founding would have been the surface that does not show it.
 *
 * The one thing the loop cannot derive is `records`, whose `bytes` field is a
 * LENGTH in the document and a byte array in the inspection; that row keeps an
 * explicit projection, and `product.test.ts` canaries its key set too.
 */
function inspectionDocumentV1(report: string, inspected: InspectedSplineProductArtifactsV1) {
  const projected: Record<string, unknown> = {};
  for (const [name, value] of Object.entries(inspected)) {
    if (UNPROJECTED_INSPECTION_KEYS_V1.includes(name)) continue;
    projected[snakeCaseV1(name)] = name === 'records'
      ? Object.freeze(Object.fromEntries(Object.entries(inspected.records).map(([key, record]) => [key, recordDocumentV1(record)])))
      : value;
  }
  return Object.freeze({
    ...projected,
    schema: SPLINE_PRODUCT_INSPECTION_SCHEMA_V1,
    report,
  }) as Readonly<{ schema: string; report: string } & Record<string, unknown>>;
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
  io.out(`  degree ${inspected.degree} · basis width ${inspected.basisWidth} · ${inspected.productOutcomeCount} Product outcomes · ${inspected.roundingBoundary}`);
  io.out(`  Registry ${inspected.registryProgram} · semantic basis ${inspected.semanticBasisId}`);
  // Partition quality goes above the record coordinates on purpose: it is the
  // one number here that can tell an author not to found this market at all,
  // and a reader who stops after the first screen must have seen it.
  const quality = inspected.partitionQuality;
  io.out(`  partition quality (${quality.model}): cell ${quality.dominantCell} holds ${basisPointsV1(quality.dominantShareBps)} of the plausible band, ceiling ${basisPointsV1(quality.maxCellShareBps)}`);
  io.out(`    shares by cell: ${quality.cellShareBps.map(basisPointsV1).join(' / ')}`);
  io.out(`    band: anchor ${quality.anchor} ± ${quality.plausibleHalfWidth}, from ${quality.volatilityBps} bps over ${quality.windowSlots} slots (characteristic displacement ${quality.characteristicDisplacement})`);
  for (const [field, address] of Object.entries(inspected.foundRecords)) io.out(`  ${field} ${address}`);
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
  if (subcommand !== 'spline') throw new Error('usage: dclutch-terminal product spline --input <absolute canonical json> --output-dir <absolute new directory> | dclutch-terminal product inspect --report <absolute report.json>');
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
