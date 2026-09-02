import { hex, sha256 } from './bytes';
import type { CompiledProductV2 } from './productV2';
import {
  PRODUCT_V2_BYTES,
  PRODUCT_V2_MAGIC,
  PRODUCT_V2_VERSION,
} from './generated/productV2Payoff';
import {
  PRODUCT_PAYOFF_V2_MAX_COORDINATES_V1,
  PRODUCT_PAYOFF_V2_REQUEST_FORMAT_V1,
  PRODUCT_PAYOFF_V2_RESPONSE_FORMAT_V1,
  PRODUCT_PAYOFF_V2_WASM_BYTES_V1,
  PRODUCT_PAYOFF_V2_WASM_SHA256_V1,
} from './generated/productPayoffV2WasmV1';

/**
 * The compiled Product V2 payoff evaluator, and the browser's half of the seam.
 *
 * THE MIRROR THIS REPLACES. `lib/productV2.ts` carried `evaluateProductV2`, a
 * hand-written reimplementation of `ProductPayoffV2::evaluate_rational` with
 * its own `ramp` and its own rational comparison, and the Studio drew a payout
 * curve out of it. Two authorities for one piece of exact arithmetic, and the
 * one the chain runs was not the one on the screen. It survived a whole lane
 * that was FIXING mirrors, named "untouched, unexcused", because the answer to
 * a mirror is never a second mirror — it is compiling the owner.
 *
 * WHAT THIS FILE DELIBERATELY DOES NOT DO is arithmetic. It carries a record's
 * BYTES and a list of exact rational coordinates across, and carries the
 * codec's own answers back. It cannot round differently, because it does not
 * round.
 *
 * The bytes are the input on purpose: the boundary evaluates the artifact that
 * would be published, not the fields typed beside it, and a record the codec
 * cannot decode is refused BY NAME before any coordinate is evaluated.
 */

/** One exact signed-rational coordinate, as decimal text so no float exists. */
export type PayoffCoordinateV1 = Readonly<{ numerator: bigint; denominator: bigint }>;

/** What the compiled evaluator states about one record and its samples. */
export type ProductPayoffEvaluationV1 = Readonly<{
  productId: bigint;
  domainId: bigint;
  coordinateUnitId: bigint;
  payoutScale: bigint;
  knotDenominator: bigint;
  knotCount: number;
  termCount: number;
  liabilityBound: bigint;
  /** One scaled payout per requested coordinate, in the order requested. */
  payouts: ReadonlyArray<bigint>;
}>;

/** The four functions the compiled evaluator exposes. */
export type ProductPayoffV2WasmV1 = Readonly<{
  evaluate_product_payoff_v2_wasm(requestJson: string): string;
  product_payoff_v2_bytes_v1(): number;
  product_payoff_v2_magic_v1(): string;
  product_payoff_v2_version_v1(): number;
}>;

function decimal(value: unknown, field: string): bigint {
  if (typeof value !== 'string' || !/^-?(0|[1-9][0-9]*)$/.test(value)) {
    throw new Error(`payoff evaluation ${field} is not exact decimal text`);
  }
  return BigInt(value);
}

function count(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`payoff evaluation ${field} is not a count`);
  }
  return value;
}

/** The request the boundary accepts, built from a record and its samples. */
export function productPayoffEvaluationRequestV1(
  recordBase64: string,
  coordinates: ReadonlyArray<PayoffCoordinateV1>,
): string {
  if (coordinates.length > PRODUCT_PAYOFF_V2_MAX_COORDINATES_V1) {
    throw new Error(`a payoff evaluation carries at most ${PRODUCT_PAYOFF_V2_MAX_COORDINATES_V1} coordinates`);
  }
  return JSON.stringify({
    format: PRODUCT_PAYOFF_V2_REQUEST_FORMAT_V1,
    recordBase64,
    coordinates: coordinates.map((coordinate) => ({
      numerator: coordinate.numerator.toString(),
      denominator: coordinate.denominator.toString(),
    })),
  });
}

/**
 * Hostile-decode the evaluator's own answer.
 *
 * The width, magic and version checks are not defensive noise: the record's
 * coordinates come from `lib/generated/productV2Payoff.ts`, emitted from the
 * codec, and the WASM pins the same three by constant name at compile time. A
 * response that states a different one means the emitted facts and the artifact
 * came from different trees — which a matching digest cannot rule out.
 */
export function parseProductPayoffEvaluationV1(
  source: string,
  expected: number,
): ProductPayoffEvaluationV1 {
  let parsed: unknown;
  try { parsed = JSON.parse(source); } catch { throw new Error('payoff evaluation is not JSON'); }
  if (parsed === null || typeof parsed !== 'object') throw new Error('payoff evaluation is not an object');
  const answer = parsed as Record<string, unknown>;
  if (typeof answer.error === 'string') throw new Error(answer.error);
  if (answer.format !== PRODUCT_PAYOFF_V2_RESPONSE_FORMAT_V1) {
    throw new Error('payoff evaluation is not the exact accepted format');
  }
  if (answer.recordBytes !== PRODUCT_V2_BYTES) {
    throw new Error(`payoff evaluator states a ${String(answer.recordBytes)}-byte record where the codec has ${PRODUCT_V2_BYTES}`);
  }
  if (answer.magic !== PRODUCT_V2_MAGIC || answer.version !== PRODUCT_V2_VERSION) {
    throw new Error('payoff evaluator states another record magic or version than the codec');
  }
  const payouts = answer.payouts;
  if (!Array.isArray(payouts) || payouts.length !== expected) {
    throw new Error(`payoff evaluation returned ${Array.isArray(payouts) ? payouts.length : 0} payouts for ${expected} coordinates`);
  }
  return Object.freeze({
    productId: decimal(answer.productId, 'product id'),
    domainId: decimal(answer.domainId, 'domain id'),
    coordinateUnitId: decimal(answer.coordinateUnitId, 'coordinate unit id'),
    payoutScale: decimal(answer.payoutScale, 'payout scale'),
    knotDenominator: decimal(answer.knotDenominator, 'knot denominator'),
    knotCount: count(answer.knotCount, 'knot count'),
    termCount: count(answer.termCount, 'term count'),
    liabilityBound: decimal(answer.liabilityBound, 'liability bound'),
    payouts: Object.freeze(payouts.map((payout, index) => decimal(payout, `payout ${index}`))),
  });
}

/** Load the checked Rust evaluator blob; unverified fetched bytes never execute. */
export async function loadProductPayoffV2WasmV1(
  fetcher: typeof fetch = (input, init) => globalThis.fetch(input, init),
): Promise<ProductPayoffV2WasmV1> {
  const url = new URL('./generated/productPayoffV2Wasm/product_payoff_v2_bg.wasm', import.meta.url);
  const response = await fetcher(url);
  if (!response.ok) throw new Error(`payoff evaluator WASM fetch failed with HTTP ${response.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length !== PRODUCT_PAYOFF_V2_WASM_BYTES_V1
      || hex(await sha256(bytes)) !== PRODUCT_PAYOFF_V2_WASM_SHA256_V1) {
    throw new Error('payoff evaluator WASM bytes do not match the generated Rust artifact identity');
  }
  const wasmModule = await import('./generated/productPayoffV2Wasm/product_payoff_v2.js');
  await wasmModule.default({ module_or_path: bytes });
  // A blob can match its digest and still come from a different tree, so the
  // loader asks the evaluator its own record width, magic and version and
  // refuses if any disagrees with the codec-emitted facts.
  const width = wasmModule.product_payoff_v2_bytes_v1();
  const magic = wasmModule.product_payoff_v2_magic_v1();
  const version = wasmModule.product_payoff_v2_version_v1();
  if (width !== PRODUCT_V2_BYTES || magic !== PRODUCT_V2_MAGIC || version !== PRODUCT_V2_VERSION) {
    throw new Error(`payoff evaluator reports a ${width}-byte ${magic} v${version} record where the codec has ${PRODUCT_V2_BYTES}-byte ${PRODUCT_V2_MAGIC} v${PRODUCT_V2_VERSION}`);
  }
  return Object.freeze({
    evaluate_product_payoff_v2_wasm: wasmModule.evaluate_product_payoff_v2_wasm,
    product_payoff_v2_bytes_v1: wasmModule.product_payoff_v2_bytes_v1,
    product_payoff_v2_magic_v1: wasmModule.product_payoff_v2_magic_v1,
    product_payoff_v2_version_v1: wasmModule.product_payoff_v2_version_v1,
  });
}

/** Evaluate one record at every coordinate, through the compiled owner. */
export async function evaluateProductPayoffV2WasmV1(
  boundary: ProductPayoffV2WasmV1,
  recordBase64: string,
  coordinates: ReadonlyArray<PayoffCoordinateV1>,
): Promise<ProductPayoffEvaluationV1> {
  const request = productPayoffEvaluationRequestV1(recordBase64, coordinates);
  return parseProductPayoffEvaluationV1(boundary.evaluate_product_payoff_v2_wasm(request), coordinates.length);
}

/** One knot of a payout curve, evaluated by the compiled owner. */
export type PayoutCurveKnotV1 = Readonly<{ numerator: string; payoutAtoms: string }>;

/**
 * The exact knot evaluations a payout curve is drawn from.
 *
 * Every term is piecewise linear between knots and constant outside them, so
 * the curve IS its control polygon and nothing is sampled: the knots are the
 * whole curve. This used to run in TypeScript, which meant the shape on the
 * screen was drawn by a second implementation of the payoff. It is one call
 * across the boundary for the whole curve.
 */
export async function payoutCurveKnotsV1(
  boundary: ProductPayoffV2WasmV1,
  compiled: CompiledProductV2,
  toBase64: (bytes: Uint8Array) => string,
): Promise<ReadonlyArray<PayoutCurveKnotV1>> {
  const knots = compiled.input.knots;
  const answer = await evaluateProductPayoffV2WasmV1(
    boundary,
    toBase64(compiled.bytes),
    knots.map((numerator) => ({ numerator, denominator: compiled.input.knotDenominator })),
  );
  return Object.freeze(knots.map((numerator, index) => Object.freeze({
    numerator: numerator.toString(),
    payoutAtoms: answer.payouts[index].toString(),
  })));
}
