import { PublicKey } from '@solana/web3.js';

import { fromHex, hex, sha256 } from './bytes';
import {
  GRADED_BASIS_RECORD_SCHEMA_ID_V3,
  PORTFOLIO_SCHEMA_ID_V2,
  PRICE_GATE_RECORD_SCHEMA_ID_V1,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
  SPLINE_PRODUCT_AUTHORING_COMMAND_V1,
  SPLINE_PRODUCT_AUTHORING_REPORT_SCHEMA_V1,
} from './generated/coreFound';
import { PRODUCT_RECORD_BYTES_V2 } from './generated/productRuntimeV2Admission';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';

const PRICE_GATE_BYTES_V1 = 320;

const RECORD_NAMES = ['product', 'result_domain', 'portfolio', 'product_basis', 'price_gate'] as const;
type RecordNameV1 = typeof RECORD_NAMES[number];

export type SplineProductArtifactFilesV1 = Readonly<{
  product: Uint8Array;
  resultDomain: Uint8Array;
  portfolio: Uint8Array;
  productBasis: Uint8Array;
  priceGate: Uint8Array;
}>;

export type InspectedSplineProductRecordV1 = Readonly<{
  file: string;
  bytes: Uint8Array;
  schemaId: string;
  contentSha256: string;
  rawAccount: string;
  stagingAccount: string;
}>;

export type InspectedSplineProductArtifactsV1 = Readonly<{
  schema: typeof SPLINE_PRODUCT_AUTHORING_REPORT_SCHEMA_V1;
  command: typeof SPLINE_PRODUCT_AUTHORING_COMMAND_V1;
  keyFree: true;
  signs: false;
  submits: false;
  inputSha256: string;
  registryProgram: string;
  productOutcomeCount: number;
  basisWidth: number;
  degree: 2 | 3;
  interiorMultiplicity: boolean;
  payoutScale: string;
  roundingBoundary: 'cumulative-floor-v3';
  semanticBasisId: string;
  records: Readonly<Record<RecordNameV1, InspectedSplineProductRecordV1>>;
  verifiedPriceGate: Readonly<{
    scale: number;
    mass: string;
    degree: 2 | 3;
    width: number;
    atomCount: number;
    prices: ReadonlyArray<string>;
  }>;
  /** Exact Registry raw-record coordinates to pass into `prepareCoreFoundV2`. */
  foundRecords: Readonly<{
    productRecord: string;
    resultDomainRecord: string;
    portfolioRecord: string;
    linkedBasisRecord: string;
    priceGateRecord: string;
  }>;
}>;

type JsonObject = Record<string, unknown>;

function object(value: unknown, field: string): JsonObject {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${field} must be an object`);
  return value as JsonObject;
}

function exactKeys(value: JsonObject, keys: ReadonlyArray<string>, field: string): void {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    throw new Error(`${field} has missing or unknown fields`);
  }
}

function string(value: unknown, field: string): string {
  if (typeof value !== 'string') throw new Error(`${field} must be a string`);
  return value;
}

function boolean(value: unknown, field: string): boolean {
  if (typeof value !== 'boolean') throw new Error(`${field} must be a boolean`);
  return value;
}

function integer(value: unknown, field: string, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) {
    throw new Error(`${field} must be an integer in ${minimum}..=${maximum}`);
  }
  return value as number;
}

function digest(value: unknown, field: string): string {
  const parsed = string(value, field);
  const bytes = fromHex(parsed, field);
  if (bytes.every((byte) => byte === 0)) throw new Error(`${field} is the reserved zero identity`);
  return parsed;
}

function decimal(value: unknown, field: string, allowZero = false): string {
  const parsed = string(value, field);
  if (!/^(0|[1-9][0-9]*)$/.test(parsed) || (!allowZero && parsed === '0')) {
    throw new Error(`${field} must be canonical ${allowZero ? 'nonnegative' : 'positive'} decimal text`);
  }
  return parsed;
}

function address(value: unknown, field: string): string {
  const parsed = string(value, field);
  const key = new PublicKey(parsed);
  if (key.toBase58() !== parsed) throw new Error(`${field} must be canonical base58 text`);
  return parsed;
}

/**
 * Verify the byte-for-byte handoff emitted by the Rust spline compiler.
 *
 * This deliberately does not port ProductBasisV3 evaluation or the price-gate
 * admission theorem into TypeScript. It verifies the compiler's declared file
 * identities, canonical Registry PDAs, fixed schemas, and report invariants,
 * then exposes only the five record coordinates Found needs.
 */
export async function inspectSplineProductAuthoringArtifactsV1(
  reportValue: unknown,
  files: SplineProductArtifactFilesV1,
): Promise<InspectedSplineProductArtifactsV1> {
  const report = object(reportValue, 'spline compiler report');
  exactKeys(report, [
    'schema', 'command', 'key_free', 'signs', 'submits', 'input_sha256', 'registry_program',
    'product_outcome_count', 'basis_width', 'degree', 'interior_multiplicity', 'payout_scale',
    'rounding_boundary', 'semantic_basis_id', 'records', 'verified_price_gate',
  ], 'spline compiler report');
  if (report.schema !== SPLINE_PRODUCT_AUTHORING_REPORT_SCHEMA_V1 || report.command !== SPLINE_PRODUCT_AUTHORING_COMMAND_V1) throw new Error('spline compiler report has the wrong schema or command');
  if (boolean(report.key_free, 'report.key_free') !== true
      || boolean(report.signs, 'report.signs') !== false
      || boolean(report.submits, 'report.submits') !== false) {
    throw new Error('spline compiler report is not the key-free, non-signing authoring seam');
  }
  const inputSha256 = digest(report.input_sha256, 'report.input_sha256');
  const registryProgram = address(report.registry_program, 'report.registry_program');
  const productOutcomeCount = integer(report.product_outcome_count, 'report.product_outcome_count', 2, 64);
  const basisWidth = integer(report.basis_width, 'report.basis_width', 1, 10);
  const degree = integer(report.degree, 'report.degree', 2, 3) as 2 | 3;
  const interiorMultiplicity = boolean(report.interior_multiplicity, 'report.interior_multiplicity');
  const payoutScale = decimal(report.payout_scale, 'report.payout_scale');
  if (report.rounding_boundary !== 'cumulative-floor-v3') throw new Error('spline compiler report names an unknown rounding boundary');
  const semanticBasisId = digest(report.semantic_basis_id, 'report.semantic_basis_id');

  const recordReports = object(report.records, 'report.records');
  exactKeys(recordReports, RECORD_NAMES, 'report.records');
  const fileByRecord: Readonly<Record<RecordNameV1, Uint8Array>> = Object.freeze({
    product: files.product,
    result_domain: files.resultDomain,
    portfolio: files.portfolio,
    product_basis: files.productBasis,
    price_gate: files.priceGate,
  });
  const expected: Readonly<Record<RecordNameV1, Readonly<{ file: string; schema: Uint8Array }>>> = Object.freeze({
    product: { file: 'product.bin', schema: PRODUCT_RECORD_SCHEMA_ID_V2 },
    result_domain: { file: 'result-domain.bin', schema: RESULT_DOMAIN_SCHEMA_ID_V2 },
    portfolio: { file: 'portfolio.bin', schema: PORTFOLIO_SCHEMA_ID_V2 },
    product_basis: { file: 'product-basis.bin', schema: GRADED_BASIS_RECORD_SCHEMA_ID_V3 },
    price_gate: { file: 'price-gate.bin', schema: PRICE_GATE_RECORD_SCHEMA_ID_V1 },
  });
  const inspected = {} as Record<RecordNameV1, InspectedSplineProductRecordV1>;
  for (const name of RECORD_NAMES) {
    const entry = object(recordReports[name], `report.records.${name}`);
    exactKeys(entry, ['file', 'bytes', 'schema_id', 'content_sha256', 'raw_account', 'staging_account'], `report.records.${name}`);
    const expectedRecord = expected[name];
    if (entry.file !== expectedRecord.file) throw new Error(`report.records.${name}.file is not the canonical compiler filename`);
    const bytes = new Uint8Array(fileByRecord[name]);
    const length = integer(entry.bytes, `report.records.${name}.bytes`, 1, 1_000_000);
    if (bytes.length !== length) throw new Error(`${expectedRecord.file} length differs from the compiler report`);
    if (name === 'product' && bytes.length !== PRODUCT_RECORD_BYTES_V2) throw new Error('product.bin has the wrong exact ABI width');
    if (name === 'price_gate' && bytes.length !== PRICE_GATE_BYTES_V1) throw new Error('price-gate.bin has the wrong exact ABI width');
    const schemaId = digest(entry.schema_id, `report.records.${name}.schema_id`);
    if (schemaId !== hex(expectedRecord.schema)) throw new Error(`report.records.${name}.schema_id differs from the generated authority`);
    const contentSha256 = digest(entry.content_sha256, `report.records.${name}.content_sha256`);
    if (hex(await sha256(bytes)) !== contentSha256) throw new Error(`${expectedRecord.file} content differs from the compiler report`);
    const rawAccount = address(entry.raw_account, `report.records.${name}.raw_account`);
    const stagingAccount = address(entry.staging_account, `report.records.${name}.staging_account`);
    const derived = deriveFinalizedRecordAddressesV1(registryProgram, expectedRecord.schema, fromHex(contentSha256, `${name} digest`));
    if (rawAccount !== derived.record || stagingAccount !== derived.staging) throw new Error(`report.records.${name} carries noncanonical Registry coordinates`);
    inspected[name] = Object.freeze({ file: expectedRecord.file, bytes, schemaId, contentSha256, rawAccount, stagingAccount });
  }

  const gate = object(report.verified_price_gate, 'report.verified_price_gate');
  exactKeys(gate, ['scale', 'mass', 'degree', 'width', 'atom_count', 'prices'], 'report.verified_price_gate');
  const gateScale = integer(gate.scale, 'report.verified_price_gate.scale', 1, 0xffff_ffff);
  const gateMass = decimal(gate.mass, 'report.verified_price_gate.mass');
  const gateDegree = integer(gate.degree, 'report.verified_price_gate.degree', 2, 3) as 2 | 3;
  const gateWidth = integer(gate.width, 'report.verified_price_gate.width', 1, 10);
  const atomCount = integer(gate.atom_count, 'report.verified_price_gate.atom_count', 1, 255);
  if (!Array.isArray(gate.prices) || gate.prices.length !== gateWidth) throw new Error('report.verified_price_gate.prices differs from its width');
  const prices = Object.freeze(gate.prices.map((value, index) => decimal(value, `report.verified_price_gate.prices[${index}]`, true)));
  if (gateDegree !== degree || gateWidth !== basisWidth || BigInt(payoutScale) !== BigInt(gateScale)) {
    throw new Error('verified price-gate summary differs from the ProductBasis summary');
  }

  const records = Object.freeze(inspected);
  return Object.freeze({
    schema: SPLINE_PRODUCT_AUTHORING_REPORT_SCHEMA_V1,
    command: SPLINE_PRODUCT_AUTHORING_COMMAND_V1,
    keyFree: true,
    signs: false,
    submits: false,
    inputSha256,
    registryProgram,
    productOutcomeCount,
    basisWidth,
    degree,
    interiorMultiplicity,
    payoutScale,
    roundingBoundary: 'cumulative-floor-v3',
    semanticBasisId,
    records,
    verifiedPriceGate: Object.freeze({ scale: gateScale, mass: gateMass, degree: gateDegree, width: gateWidth, atomCount, prices }),
    foundRecords: Object.freeze({
      productRecord: records.product.rawAccount,
      resultDomainRecord: records.result_domain.rawAccount,
      portfolioRecord: records.portfolio.rawAccount,
      linkedBasisRecord: records.product_basis.rawAccount,
      priceGateRecord: records.price_gate.rawAccount,
    }),
  });
}
