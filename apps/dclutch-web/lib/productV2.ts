import { hex, sha256 } from './bytes';
import {
  PRODUCT_V2_BYTES,
  PRODUCT_V2_KNOTS_OFFSET,
  PRODUCT_V2_MAGIC,
  PRODUCT_V2_MAX_KNOTS,
  PRODUCT_V2_MAX_TERMS,
  PRODUCT_V2_TERM_BYTES,
  PRODUCT_V2_TERMS_OFFSET,
  PRODUCT_V2_VERSION,
} from './generated/productV2Payoff';

/**
 * Authoring and exact evaluation of the canonical Product V2 payoff record.
 *
 * This surface owns semantic data and nothing else: a 576-byte DCLTPAY2 record,
 * its content identity, its conservative liability bound, and exact signed-rational
 * evaluation of the payoff it denotes. No Market, deployment, or transaction is
 * implied by anything here.
 *
 * It used to compose one more thing — a 10-account evidence plus 28-account
 * liability-admission transaction — and that half was deleted on 2026-08-27. Two
 * independent findings retired it. It targeted `dclutch-product-payoff-v2-svm`
 * (DCLTPRQ2/DCLTPCF2) and `dclutch-product-admission-contract`
 * (DCLTPAR1/DCLTPAB1/DCLTPAC1); no package under `programs/` links either crate,
 * `dclutch-product-admission-contract` has no dependents at all, and the
 * byte-identical on-chain half was `programs/dclutch-product-evidence-sbf`, already
 * banished. So the browser was building an unsigned transaction that no deployed
 * program could ever execute. Independently, its dozen hand-mirrored 32-byte
 * identities had drifted: it pinned RESOLUTION_CONTROLLER_RELEASE_ID_V3 while every
 * Rust consumer had moved to V4, which would have refused the artifact the chain
 * actually publishes.
 *
 * Live Product admission is `programs/dclutch-product-runtime-v2-sbf` over
 * `dclutch-product-runtime-v2-admission`, a different wire (DCLTPRM2 / DCLTPRQ2 at
 * 112 bytes / DCLTPRA2). That surface now exists, as
 * `lib/productRuntimeV2Admission.ts` — built against the live decoder, with every
 * coordinate generated out of the two live source files, because DCLTPRQ2 names two
 * incompatible 112-byte requests: the dead evaluator request wrote 1 at byte 10
 * where the live decoder requires zero. It is a separate module on purpose. This
 * one owns authored semantic data; that one owns a wire.
 *
 * Superseded source: ~/dev/dclutch-legacy/dclutch-web-product-v2-liability/.
 */

export { PRODUCT_V2_BYTES, PRODUCT_V2_MAX_KNOTS, PRODUCT_V2_MAX_TERMS };

const MAX_U64 = (BigInt(1) << BigInt(64)) - BigInt(1);
const MIN_I128 = -(BigInt(1) << BigInt(127));
const MAX_I128 = (BigInt(1) << BigInt(127)) - BigInt(1);
const TWO_128 = BigInt(1) << BigInt(128);

export type ProductShapeV2 = 'constant' | 'ramp-up' | 'ramp-down' | 'tent';
export type ProductTermV2 = Readonly<{ shape: ProductShapeV2; left: number; peak: number; right: number; amplitude: bigint }>;
export type ProductAuthoringV2 = Readonly<{ productId: bigint; domainId: bigint; coordinateUnitId: bigint; payoutScale: bigint; knotDenominator: bigint; knots: ReadonlyArray<bigint>; terms: ReadonlyArray<ProductTermV2> }>;
export type ProductRegionV2 = Readonly<{ label: string; left: string; right: string }>;
export type CompiledProductV2 = Readonly<{ input: ProductAuthoringV2; bytes: Uint8Array; digest: Uint8Array; digestHex: string; liabilityBound: bigint; regions: ReadonlyArray<ProductRegionV2> }>;

function exactU64(value: bigint, field: string, nonzero = false): bigint { if (value < (nonzero ? BigInt(1) : BigInt(0)) || value > MAX_U64) throw new Error(`${field} is outside ${nonzero ? '1..' : '0..'}u64::MAX`); return value; }
function exactI128(value: bigint, field: string): bigint { if (value < MIN_I128 || value > MAX_I128) throw new Error(`${field} is outside i128`); return value; }
function parseInteger(value: string, field: string): bigint { if (!/^-?(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} must be a canonical base-10 integer`); return BigInt(value); }
function putU64(output: Uint8Array, offset: number, value: bigint): void { new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, exactU64(value, 'u64'), true); }
function putI128(output: Uint8Array, offset: number, value: bigint): void { let encoded = exactI128(value, 'i128'); if (encoded < 0) encoded += TWO_128; for (let index = 0; index < 16; index += 1) { output[offset + index] = Number(encoded & BigInt(255)); encoded >>= BigInt(8); } }
function shapeKey(term: ProductTermV2): number { return term.shape === 'constant' ? 0 : term.shape === 'ramp-up' ? 4096 + term.left * 16 + term.right : term.shape === 'ramp-down' ? 8192 + term.left * 16 + term.right : 12288 + term.left * 256 + term.peak * 16 + term.right; }

export function parseProductKnots(value: string): ReadonlyArray<bigint> {
  const lines = value.split(/\r?\n/).filter((line) => line.length > 0); return Object.freeze(lines.map((line, index) => parseInteger(line, `knot ${index}`)));
}

export function parseProductTerms(value: string): ReadonlyArray<ProductTermV2> {
  const lines = value.split(/\r?\n/).filter((line) => line.length > 0);
  return Object.freeze(lines.map((line, index) => {
    if (line.trim() !== line || /\s{2,}/.test(line)) throw new Error(`term ${index} must use canonical single spaces`);
    const parts = line.split(' '); const shape = parts[0] as ProductShapeV2;
    if (shape === 'constant' && parts.length === 2) return Object.freeze({ shape, left: 0, peak: 0, right: 0, amplitude: parseInteger(parts[1], `term ${index} amplitude`) });
    if ((shape === 'ramp-up' || shape === 'ramp-down') && parts.length === 4) return Object.freeze({ shape, left: Number(parseInteger(parts[1], `term ${index} left`)), peak: 0, right: Number(parseInteger(parts[2], `term ${index} right`)), amplitude: parseInteger(parts[3], `term ${index} amplitude`) });
    if (shape === 'tent' && parts.length === 5) return Object.freeze({ shape, left: Number(parseInteger(parts[1], `term ${index} left`)), peak: Number(parseInteger(parts[2], `term ${index} peak`)), right: Number(parseInteger(parts[3], `term ${index} right`)), amplitude: parseInteger(parts[4], `term ${index} amplitude`) });
    throw new Error(`term ${index} must be “constant amplitude”, “ramp-up left right amplitude”, “ramp-down left right amplitude”, or “tent left peak right amplitude”`);
  }));
}

function validateProduct(input: ProductAuthoringV2): Readonly<{ terms: ReadonlyArray<ProductTermV2>; liabilityBound: bigint }> {
  exactU64(input.productId, 'product scalar ID', true); exactU64(input.domainId, 'domain scalar ID', true); exactU64(input.coordinateUnitId, 'coordinate-unit scalar ID', true); exactU64(input.payoutScale, 'payout scale', true); exactU64(input.knotDenominator, 'knot denominator', true);
  if (input.knots.length < 2 || input.knots.length > PRODUCT_V2_MAX_KNOTS) throw new Error('Product V2 requires 2..16 active knots');
  input.knots.forEach((value, index) => { exactI128(value, `knot ${index}`); if (index > 0 && value <= input.knots[index - 1]) throw new Error('active knot numerators must be strictly increasing'); });
  if (input.terms.length < 1 || input.terms.length > PRODUCT_V2_MAX_TERMS) throw new Error('Product V2 requires 1..16 active terms');
  const terms = [...input.terms].sort((left, right) => shapeKey(left) - shapeKey(right)); let liabilityBound = BigInt(0); let prior = -1;
  terms.forEach((term, index) => {
    exactU64(term.amplitude, `term ${index} amplitude`, true); const keyValue = shapeKey(term); if (keyValue <= prior) throw new Error('payoff terms contain a duplicate or noncanonical shape key'); prior = keyValue;
    const validIndex = (value: number) => Number.isSafeInteger(value) && value >= 0 && value < input.knots.length;
    if (term.shape === 'constant') { if (term.left !== 0 || term.peak !== 0 || term.right !== 0) throw new Error('constant term carries inactive indices'); }
    else if (term.shape === 'tent') { if (!validIndex(term.left) || !validIndex(term.peak) || !validIndex(term.right) || !(term.left < term.peak && term.peak < term.right)) throw new Error('tent indices must be ordered active knots'); }
    else if (!validIndex(term.left) || !validIndex(term.right) || !(term.left < term.right) || term.peak !== 0) throw new Error('ramp indices must be ordered active knots');
    liabilityBound += term.amplitude; exactU64(liabilityBound, 'sum-of-amplitudes liability bound', true);
  });
  return Object.freeze({ terms: Object.freeze(terms), liabilityBound });
}

export async function compileProductV2(input: ProductAuthoringV2): Promise<CompiledProductV2> {
  const validated = validateProduct(input); const bytes = new Uint8Array(PRODUCT_V2_BYTES); bytes.set(new TextEncoder().encode(PRODUCT_V2_MAGIC)); new DataView(bytes.buffer).setUint16(8, PRODUCT_V2_VERSION, true); bytes[10] = input.knots.length; bytes[11] = validated.terms.length;
  [input.productId, input.domainId, input.coordinateUnitId, input.payoutScale, input.knotDenominator].forEach((value, index) => putU64(bytes, 16 + index * 8, value)); input.knots.forEach((value, index) => putI128(bytes, PRODUCT_V2_KNOTS_OFFSET + index * 16, value));
  validated.terms.forEach((term, index) => { const offset = PRODUCT_V2_TERMS_OFFSET + index * PRODUCT_V2_TERM_BYTES; bytes[offset] = term.shape === 'constant' ? 0 : term.shape === 'ramp-up' ? 1 : term.shape === 'ramp-down' ? 2 : 3; bytes[offset + 1] = term.left; bytes[offset + 2] = term.peak; bytes[offset + 3] = term.right; putU64(bytes, offset + 8, term.amplitude); });
  const canonicalInput = Object.freeze({ ...input, knots: Object.freeze([...input.knots]), terms: validated.terms }); const digest = await sha256(bytes);
  const rational = (value: bigint) => `${value}/${input.knotDenominator}`; const regions: ProductRegionV2[] = [{ label: 'left clamped tail', left: '−∞', right: rational(input.knots[0]) }, ...input.knots.slice(0, -1).map((value, index) => ({ label: `interpolation segment ${index}`, left: rational(value), right: rational(input.knots[index + 1]) })), { label: 'right clamped tail', left: rational(input.knots[input.knots.length - 1]), right: '+∞' }];
  return Object.freeze({ input: canonicalInput, bytes, digest, digestHex: hex(digest), liabilityBound: validated.liabilityBound, regions: Object.freeze(regions) });
}

function compareRational(left: bigint, leftDenominator: bigint, right: bigint, rightDenominator: bigint): number { const difference = left * rightDenominator - right * leftDenominator; return difference < 0 ? -1 : difference > 0 ? 1 : 0; }
function ramp(amplitude: bigint, left: bigint, right: bigint, knotDenominator: bigint, numerator: bigint, denominator: bigint, rising: boolean): bigint {
  if (compareRational(numerator, denominator, left, knotDenominator) <= 0) return rising ? BigInt(0) : amplitude;
  if (compareRational(numerator, denominator, right, knotDenominator) >= 0) return rising ? amplitude : BigInt(0);
  const coordinate = numerator * knotDenominator; const leftScaled = left * denominator; const rightScaled = right * denominator; const elapsed = rising ? coordinate - leftScaled : rightScaled - coordinate; return amplitude * elapsed / (rightScaled - leftScaled);
}

export function evaluateProductV2(product: CompiledProductV2, numerator: bigint, denominator: bigint): bigint {
  exactI128(numerator, 'result numerator'); exactU64(denominator, 'result denominator', true); let payout = BigInt(0); const knots = product.input.knots; const kd = product.input.knotDenominator;
  for (const term of product.input.terms) {
    if (term.shape === 'constant') payout += term.amplitude;
    else if (term.shape === 'ramp-up') payout += ramp(term.amplitude, knots[term.left], knots[term.right], kd, numerator, denominator, true);
    else if (term.shape === 'ramp-down') payout += ramp(term.amplitude, knots[term.left], knots[term.right], kd, numerator, denominator, false);
    else payout += [ramp(term.amplitude, knots[term.left], knots[term.peak], kd, numerator, denominator, true), ramp(term.amplitude, knots[term.peak], knots[term.right], kd, numerator, denominator, false)].reduce((a, b) => a < b ? a : b);
  }
  return payout;
}

export function productInteger(value: string, field: string): bigint { return parseInteger(value, field); }
