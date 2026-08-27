import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  ComputeBudgetProgram,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { ascii, fromHex, hex, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import { decodeCapabilityManifestV1 } from './capabilityManifest';
import { PACKET_DATA_SIZE } from './directTransaction';
import {
  ARTIFACT_RELEASE_SCHEMA_ID_V1,
  NATIVE_LOADER_ID,
  RENT_SYSVAR_ID,
  SYSTEM_PROGRAM_ID,
  SYSVAR_OWNER_ID,
  authenticateArtifactDeploymentV1,
  decodeArtifactReleaseV1,
  deriveFinalizedRecordAddressesV1,
  type ArtifactReleaseV1,
} from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

export const PRODUCT_V2_BYTES = 576;
export const PRODUCT_V2_MAX_KNOTS = 16;
export const PRODUCT_V2_MAX_TERMS = 16;
export const PAYOFF_REQUEST_BYTES_V2 = 112;
export const PAYOFF_CERTIFICATE_BYTES_V2 = 232;
export const PAYOFF_ADMISSION_REQUEST_BYTES_V1 = 128;
export const PAYOFF_ADMISSION_RECEIPT_BYTES_V1 = 448;
export const PRODUCT_EVALUATOR_ACCOUNT_COUNT = 10;
export const PRODUCT_LIABILITY_ADMISSION_ACCOUNT_COUNT = 28;
export const PRODUCT_MAX_COMPUTE_UNITS = 1_400_000;

const MAX_U64 = (BigInt(1) << BigInt(64)) - BigInt(1);
const MIN_I128 = -(BigInt(1) << BigInt(127));
const MAX_I128 = (BigInt(1) << BigInt(127)) - BigInt(1);
const TWO_128 = BigInt(1) << BigInt(128);
const PAYOFF_RECORD_SCHEMA = fromHex('af30893d5ee759bb2b74560e0d6f6d42a5808bdeed40da105b65323d80aaade0', 'Product V2 schema');
const CAPABILITY_MANIFEST_SCHEMA = fromHex('6bcef7b28367cb8d089710ba58e684312f434c4bc420eefd0f7a150a908288df', 'capability manifest schema');
const PRODUCT_INSTANCE_SCHEMA = fromHex('9620bcd9f31a01ca6f42091c84579d9acc484127c08d86acc40fdd5a4cab1f14', 'Product instance schema');
const RESULT_DOMAIN_SCHEMA = fromHex('373d8df36073e84554eda98911b83a9c13cb0774548f680cba662913dd660e14', 'result domain schema');
const PRODUCT_PAYOFF_ADAPTER_RELEASE = 'a9f66d205efb985d6c93f8d712d58c91575d8446ec9563a3a2ae856eeea7cc29';
const PRODUCT_PAYOFF_ADMISSION_RELEASE = 'e04a351d243b5e7f96e11b671d5fc7bbfbd98f28d98c994dd9003cb24241af33';
const PRODUCT_PAYOFF_ROUNDING_RELEASE = '582a80dce7f8db2af474aa1f0c81c89f2187057d39e2ff538e1b65a37a54498a';
const PRODUCT_PAYOFF_ADMISSION_KIND = '8e8a063932339a7eb910608e76b1e70ad0f41b999b6252eeab890ffb733b5474';
const PRODUCT_PAYOFF_RECEIPT_SCHEMA = '27561f5ad6bf181302bf5d3922dc9eeb4ee0212af3430c6360332724555a8eca';
const PRODUCT_PAYOFF_RECEIPT_DERIVATION = 'd7caab457c4c40ba869fe6d26cd7d06d1309af196f5c6b86eec2617d49cf1b2e';
const RESOLUTION_CONTROLLER_RELEASE = '9a62c2e46da3b4fa80d1c75acdfccb448c19211a631abcb129b826b55aa8253b';
const RESULT_DOMAIN_RELEASE = '1aa41f18fa8deee09da1a1326065a990ca971a0fc59b7733c87bc38cb09253f7';
const RESULT_DOMAIN_CONTENT_DOMAIN = new TextEncoder().encode('dclutch.result-domain.v1');
const CERTIFICATE_SEED = new TextEncoder().encode('dclutch:payoff-certificate:v2');
const RECEIPT_SEED = new TextEncoder().encode('dclutch:payoff-admission:v1');
const MARKET_SEED = new TextEncoder().encode('dclutch/market-root/v1');

export type ProductShapeV2 = 'constant' | 'ramp-up' | 'ramp-down' | 'tent';
export type ProductTermV2 = Readonly<{ shape: ProductShapeV2; left: number; peak: number; right: number; amplitude: bigint }>;
export type ProductAuthoringV2 = Readonly<{ productId: bigint; domainId: bigint; coordinateUnitId: bigint; payoutScale: bigint; knotDenominator: bigint; knots: ReadonlyArray<bigint>; terms: ReadonlyArray<ProductTermV2> }>;
export type ProductRegionV2 = Readonly<{ label: string; left: string; right: string }>;
export type CompiledProductV2 = Readonly<{ input: ProductAuthoringV2; bytes: Uint8Array; digest: Uint8Array; digestHex: string; liabilityBound: bigint; roundingReleaseId: string; regions: ReadonlyArray<ProductRegionV2> }>;

type MarketView = Readonly<{ generation: bigint; identity: Uint8Array; identityDigest: Uint8Array; productInstanceId: Uint8Array; claimBasisId: Uint8Array; manifestId: Uint8Array; outcomeCount: number }>;
type InstanceView = Readonly<{ claimBasisId: Uint8Array; capacityProfileId: Uint8Array; resultDomainId: Uint8Array; outcomeCount: number }>;
type DomainView = Readonly<{ semanticId: Uint8Array; outcomeCount: number; coordinateDomainId: string; resultUnitId: string; denominator: bigint; cuts: ReadonlyArray<bigint> }>;
type BindingView = Readonly<{ bytes: Uint8Array; digest: Uint8Array; productInstanceId: Uint8Array; resultDomainId: Uint8Array; payoffRecordDigest: Uint8Array; payoffProgram: string; payoffArtifactDigest: Uint8Array; resolutionProgram: string; resolutionArtifactDigest: Uint8Array; admissionProgram: string; admissionArtifactDigest: Uint8Array; productId: bigint; domainId: bigint; coordinateUnitId: bigint; payoutScale: bigint; failurePayout: bigint }>;
type CapabilityView = Readonly<{ bindingDigest: Uint8Array; capacityProfileId: Uint8Array }>;

export type ProductV2LiabilityPlan = Readonly<{
  observedSlot: string;
  market: string;
  generation: string;
  registryProgram: string;
  bindingDigest: string;
  payoffProgram: string;
  admissionProgram: string;
  resolutionProgram: string;
  certificate: string;
  certificateDigest: string;
  certificateMode: 'create' | 'repeat';
  receipt: string;
  receiptDigest: string;
  receiptMode: 'create' | 'repeat';
  available: string;
  liabilityBound: string;
  failurePayout: string;
  totalBound: string;
  rentDebitLamports: string;
  requestBytes: Uint8Array;
  admissionBytes: Uint8Array;
  wireBytes: Uint8Array;
  transaction: VersionedTransaction;
  requiredSigners: ReadonlyArray<string>;
  lookupAddressesUsed: number;
  resultDomain: Readonly<{ record: string; semanticId: string; coordinateDomainId: string; resultUnitId: string; denominator: string; cuts: ReadonlyArray<string>; outcomeCount: number }>;
  artifactReleases: Readonly<Record<'evaluator' | 'admission' | 'resolution', Readonly<{ digest: string; semanticRelease: string; programData: string; deploymentSlot: string }>>>;
}>;

type ProductRpc = Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts' | 'minimumBalanceForRentExemption' | 'latestBlockhash' | 'accountInfo'>;

function key(value: string, field: string): PublicKey { const parsed = new PublicKey(value); if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`); return parsed; }
function same(left: Uint8Array, right: Uint8Array): boolean { return left.length === right.length && left.every((value, index) => value === right[index]); }
function concat(...values: ReadonlyArray<Uint8Array>): Uint8Array { const output = new Uint8Array(values.reduce((total, value) => total + value.length, 0)); let offset = 0; for (const value of values) { output.set(value, offset); offset += value.length; } return output; }
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
  const validated = validateProduct(input); const bytes = new Uint8Array(PRODUCT_V2_BYTES); bytes.set(new TextEncoder().encode('DCLTPAY2')); new DataView(bytes.buffer).setUint16(8, 2, true); bytes[10] = input.knots.length; bytes[11] = validated.terms.length;
  [input.productId, input.domainId, input.coordinateUnitId, input.payoutScale, input.knotDenominator].forEach((value, index) => putU64(bytes, 16 + index * 8, value)); input.knots.forEach((value, index) => putI128(bytes, 64 + index * 16, value));
  validated.terms.forEach((term, index) => { const offset = 320 + index * 16; bytes[offset] = term.shape === 'constant' ? 0 : term.shape === 'ramp-up' ? 1 : term.shape === 'ramp-down' ? 2 : 3; bytes[offset + 1] = term.left; bytes[offset + 2] = term.peak; bytes[offset + 3] = term.right; putU64(bytes, offset + 8, term.amplitude); });
  const canonicalInput = Object.freeze({ ...input, knots: Object.freeze([...input.knots]), terms: validated.terms }); const digest = await sha256(bytes);
  const rational = (value: bigint) => `${value}/${input.knotDenominator}`; const regions: ProductRegionV2[] = [{ label: 'left clamped tail', left: '−∞', right: rational(input.knots[0]) }, ...input.knots.slice(0, -1).map((value, index) => ({ label: `interpolation segment ${index}`, left: rational(value), right: rational(input.knots[index + 1]) })), { label: 'right clamped tail', left: rational(input.knots[input.knots.length - 1]), right: '+∞' }];
  return Object.freeze({ input: canonicalInput, bytes, digest, digestHex: hex(digest), liabilityBound: validated.liabilityBound, roundingReleaseId: PRODUCT_PAYOFF_ROUNDING_RELEASE, regions: Object.freeze(regions) });
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

function decodeMarket(bytes: Uint8Array, address: string, registry: string): Promise<MarketView> {
  return (async () => {
    if (bytes.length < 336 || ascii(bytes, 0, 8) !== 'DCLTCAT1' || u16(bytes, 8) !== 1 || bytes[11] !== 1) throw new Error('Market is not the canonical categorical profile'); const outcomeCount = bytes[10]; if (outcomeCount < 2 || outcomeCount > 16 || bytes.length !== 320 + outcomeCount * 8) throw new Error('Market outcome width is not canonical'); requireZero(bytes, 12, 4, 'Market header');
    if (ascii(bytes, 16, 8) !== 'DCLTROOT' || u16(bytes, 24) !== 1 || bytes[200] !== 0) throw new Error('liability admission requires a canonical Founding Market'); requireZero(bytes, 26, 6, 'Market root header'); requireZero(bytes, 201, 7, 'Market root body'); const identity = slice(bytes, 32, 168); const identityDigest = await sha256(identity); const derived = PublicKey.findProgramAddressSync([MARKET_SEED, identityDigest], key(registry, 'Registry program'))[0].toBase58(); if (derived !== address) throw new Error('Market is not its identity-derived Registry PDA');
    const ids = [slice(bytes, 64, 32), slice(bytes, 96, 32), slice(bytes, 160, 32)]; ids.forEach((value) => requireNonzero(value, 'Market identity')); const generation = u64(bytes, 192); if (generation === BigInt(0)) throw new Error('Market generation is zero'); return Object.freeze({ generation, identity, identityDigest, productInstanceId: ids[0], claimBasisId: ids[1], manifestId: ids[2], outcomeCount });
  })();
}

function decodeInstance(bytes: Uint8Array): InstanceView { if (bytes.length !== 192 || ascii(bytes, 0, 8) !== 'DCLTINS1' || u16(bytes, 8) !== 1) throw new Error('Product instance has the wrong exact layout'); requireZero(bytes, 10, 6, 'Product instance header'); requireZero(bytes, 148, 12, 'Product instance body'); const ids = [slice(bytes, 16, 32), slice(bytes, 48, 32), slice(bytes, 80, 32), slice(bytes, 112, 32), slice(bytes, 160, 32)]; ids.forEach((value) => requireNonzero(value, 'Product instance identity')); const count = new DataView(bytes.buffer, bytes.byteOffset + 144, 4).getUint32(0, true); if (count < 2) throw new Error('Product instance partition is too small'); return Object.freeze({ claimBasisId: ids[2], capacityProfileId: ids[3], resultDomainId: ids[4], outcomeCount: count }); }

async function decodeDomain(bytes: Uint8Array): Promise<DomainView> { if (bytes.length !== 352 || ascii(bytes, 0, 8) !== 'DCLTRDV1' || u16(bytes, 8) !== 1) throw new Error('finite result domain has the wrong exact layout'); requireZero(bytes, 10, 6, 'result domain header'); requireZero(bytes, 121, 7, 'result domain body'); const coordinate = slice(bytes, 16, 32); const unit = slice(bytes, 48, 32); const release = slice(bytes, 80, 32); [coordinate, unit].forEach((value) => requireNonzero(value, 'result domain identity')); if (hex(release) !== RESULT_DOMAIN_RELEASE) throw new Error('finite result domain uses a different semantic release'); const denominator = u64(bytes, 112); if (denominator === BigInt(0)) throw new Error('finite result-domain denominator is zero'); const regions = bytes[120]; if (regions < 1 || regions > 15) throw new Error('finite result-domain region count is outside 1..15'); const cuts: bigint[] = []; for (let index = 0; index < 14; index += 1) { const value = readI128(bytes, 128 + index * 16); if (index < regions - 1) { if (cuts.length > 0 && value <= cuts[cuts.length - 1]) throw new Error('finite result-domain cuts are not strictly ordered'); cuts.push(value); } else if (value !== BigInt(0)) throw new Error('finite result-domain inactive cuts are nonzero'); } const semanticId = await sha256(concat(RESULT_DOMAIN_CONTENT_DOMAIN, Uint8Array.of(0), bytes)); return Object.freeze({ semanticId, outcomeCount: regions + 1, coordinateDomainId: hex(coordinate), resultUnitId: hex(unit), denominator, cuts: Object.freeze(cuts) }); }

function readI128(bytes: Uint8Array, offset: number): bigint { let value = BigInt(0); for (let index = 15; index >= 0; index -= 1) value = (value << BigInt(8)) | BigInt(bytes[offset + index]); return value >= (BigInt(1) << BigInt(127)) ? value - TWO_128 : value; }

function decodeBinding(bytes: Uint8Array, digest: Uint8Array): BindingView { if (bytes.length !== 384 || ascii(bytes, 0, 8) !== 'DCLTPAB1' || u16(bytes, 8) !== 1 || bytes[10] !== 0) throw new Error('payoff binding has the wrong exact layout'); requireZero(bytes, 11, 5, 'payoff binding header'); requireZero(bytes, 376, 8, 'payoff binding tail'); const identities = [16, 48, 80, 112, 144, 176, 208, 240, 272].map((offset) => slice(bytes, offset, 32)); identities.forEach((value) => requireNonzero(value, 'payoff binding identity')); if (hex(slice(bytes, 304, 32)) !== PRODUCT_PAYOFF_ROUNDING_RELEASE) throw new Error('payoff binding names a different rounding release'); const scalars = [336, 344, 352, 360].map((offset) => u64(bytes, offset)); if (scalars.some((value) => value === BigInt(0))) throw new Error('payoff binding contains a zero semantic scalar'); return Object.freeze({ bytes, digest, productInstanceId: identities[0], resultDomainId: identities[1], payoffRecordDigest: identities[2], payoffProgram: new PublicKey(identities[3]).toBase58(), payoffArtifactDigest: identities[4], resolutionProgram: new PublicKey(identities[5]).toBase58(), resolutionArtifactDigest: identities[6], admissionProgram: new PublicKey(identities[7]).toBase58(), admissionArtifactDigest: identities[8], productId: scalars[0], domainId: scalars[1], coordinateUnitId: scalars[2], payoutScale: scalars[3], failurePayout: u64(bytes, 368) }); }

function decodeCapability(bytes: Uint8Array): CapabilityView {
  const selected = decodeCapabilityManifestV1(bytes).filter((entry) => hex(entry.kind) === PRODUCT_PAYOFF_ADMISSION_KIND);
  if (selected.length !== 1) throw new Error('capability manifest has no Product payoff admission entry');
  const [entry] = selected;
  if (hex(entry.programSet) !== PRODUCT_PAYOFF_ADMISSION_RELEASE || hex(entry.rootSchema) !== PRODUCT_PAYOFF_RECEIPT_SCHEMA || hex(entry.derivation) !== PRODUCT_PAYOFF_RECEIPT_DERIVATION || entry.activation !== 'immediate' || entry.deadline !== BigInt(0)) throw new Error('Product payoff capability does not select the founding admission release and receipt authority');
  return Object.freeze({ bindingDigest: entry.config, capacityProfileId: entry.capacity });
}

function buildRequest(productDigest: Uint8Array, artifactDigest: Uint8Array, available: bigint): Uint8Array { const bytes = new Uint8Array(PAYOFF_REQUEST_BYTES_V2); bytes.set(new TextEncoder().encode('DCLTPRQ2')); new DataView(bytes.buffer).setUint16(8, 2, true); bytes[10] = 1; bytes.set(productDigest, 16); bytes.set(artifactDigest, 48); putU64(bytes, 104, available); return bytes; }
function certificateBytes(registry: PublicKey, product: CompiledProductV2, artifactDigest: Uint8Array, available: bigint): Uint8Array { const bytes = new Uint8Array(PAYOFF_CERTIFICATE_BYTES_V2); bytes.set(new TextEncoder().encode('DCLTPCF2')); new DataView(bytes.buffer).setUint16(8, 2, true); bytes[10] = 1; bytes[11] = Number(product.liabilityBound <= available); bytes.set(registry.toBytes(), 16); bytes.set(product.digest, 48); bytes.set(artifactDigest, 80); bytes.set(fromHex(PRODUCT_PAYOFF_ROUNDING_RELEASE, 'rounding release'), 112); [product.input.productId, product.input.domainId, product.input.coordinateUnitId, product.input.payoutScale].forEach((value, index) => putU64(bytes, 144 + index * 8, value)); putU64(bytes, 200, available); putU64(bytes, 216, product.liabilityBound); return bytes; }
function admissionRequest(generation: bigint, bindingDigest: Uint8Array, certificateDigest: Uint8Array): Uint8Array { const bytes = new Uint8Array(PAYOFF_ADMISSION_REQUEST_BYTES_V1); bytes.set(new TextEncoder().encode('DCLTPAR1')); new DataView(bytes.buffer).setUint16(8, 1, true); putU64(bytes, 16, generation); bytes.set(bindingDigest, 24); bytes.set(certificateDigest, 56); return bytes; }
function receiptBytes(input: Readonly<{ market: PublicKey; marketView: MarketView; binding: BindingView; certificate: PublicKey; certificateDigest: Uint8Array; domain: DomainView; totalBound: bigint }>): Uint8Array { const bytes = new Uint8Array(PAYOFF_ADMISSION_RECEIPT_BYTES_V1); bytes.set(new TextEncoder().encode('DCLTPAC1')); new DataView(bytes.buffer).setUint16(8, 1, true); bytes.set(input.market.toBytes(), 16); bytes.set(input.marketView.identityDigest, 48); bytes.set(input.marketView.manifestId, 80); bytes.set(input.binding.digest, 112); bytes.set(input.marketView.productInstanceId, 144); bytes.set(input.domain.semanticId, 176); bytes.set(input.certificate.toBytes(), 208); bytes.set(input.certificateDigest, 240); bytes.set(input.binding.payoffArtifactDigest, 336); bytes.set(fromHex(PRODUCT_PAYOFF_ROUNDING_RELEASE, 'rounding release'), 368); putU64(bytes, 400, input.marketView.generation); putU64(bytes, 440, input.totalBound); return bytes; }

function metas(addresses: ReadonlyArray<Readonly<[string, boolean, boolean]>>) { return addresses.map(([address, signer, writable]) => ({ pubkey: key(address, 'instruction account'), isSigner: signer, isWritable: writable })); }

export function compileProductV2LiabilityTransaction(input: Readonly<{ payer: string; recentBlockhash: string; computeUnitLimit: number; lookupTable: AddressLookupTableAccount; request: Uint8Array; admissionRequest: Uint8Array; evaluatorProgram: string; admissionProgram: string; evaluatorAccounts: ReadonlyArray<string>; admissionAccounts: ReadonlyArray<string> }>): Readonly<{ transaction: VersionedTransaction; wireBytes: Uint8Array; requiredSigners: ReadonlyArray<string>; lookupAddressesUsed: number }> {
  if (input.request.length !== PAYOFF_REQUEST_BYTES_V2 || input.admissionRequest.length !== PAYOFF_ADMISSION_REQUEST_BYTES_V1 || input.evaluatorAccounts.length !== PRODUCT_EVALUATOR_ACCOUNT_COUNT || input.admissionAccounts.length !== PRODUCT_LIABILITY_ADMISSION_ACCOUNT_COUNT) throw new Error('Product evidence/admission request or account frame has the wrong exact width');
  if (!Number.isSafeInteger(input.computeUnitLimit) || input.computeUnitLimit < 1 || input.computeUnitLimit > PRODUCT_MAX_COMPUTE_UNITS) throw new Error(`compute limit must be within 1..${PRODUCT_MAX_COMPUTE_UNITS}`); if (!input.lookupTable.isActive()) throw new Error('address lookup table is deactivated');
  const evaluator = new TransactionInstruction({ programId: key(input.evaluatorProgram, 'evaluator program'), keys: metas(input.evaluatorAccounts.map((address, index) => [address, index === 0, index <= 1] as const)), data: input.request as Buffer });
  const admission = new TransactionInstruction({ programId: key(input.admissionProgram, 'admission program'), keys: metas(input.admissionAccounts.map((address, index) => [address, index === 0, index <= 1] as const)), data: input.admissionRequest as Buffer });
  if (new Set(input.evaluatorAccounts).size !== input.evaluatorAccounts.length || new Set(input.admissionAccounts).size !== input.admissionAccounts.length) throw new Error('Product instruction aliases roles that the SBF frame requires distinct');
  const transaction = new VersionedTransaction(new TransactionMessage({ payerKey: key(input.payer, 'payer'), recentBlockhash: key(input.recentBlockhash, 'recent blockhash').toBase58(), instructions: [ComputeBudgetProgram.setComputeUnitLimit({ units: input.computeUnitLimit }), evaluator, admission] }).compileToV0Message([input.lookupTable])); const wireBytes = transaction.serialize(); if (wireBytes.length > PACKET_DATA_SIZE) throw new Error(`Product evidence/admission transaction is ${wireBytes.length} bytes, above the ${PACKET_DATA_SIZE}-byte packet bound`); const lookedUp = transaction.message.addressTableLookups.reduce((total, lookup) => total + lookup.readonlyIndexes.length + lookup.writableIndexes.length, 0); return Object.freeze({ transaction, wireBytes, requiredSigners: Object.freeze(transaction.message.staticAccountKeys.slice(0, transaction.message.header.numRequiredSignatures).map((value) => value.toBase58())), lookupAddressesUsed: lookedUp });
}

function required(accounts: ReadonlyMap<string, RpcAccount | null>, address: string, field: string): RpcAccount { const account = accounts.get(address); if (account === null || account === undefined) throw new Error(`${field} ${address} is absent at finalized commitment`); return account; }
function vacantOrExact(account: RpcAccount | null | undefined, owner: string, expected: Uint8Array, minimum: bigint, field: string): 'create' | 'repeat' { if (account === null || account === undefined || (account.owner === SYSTEM_PROGRAM_ID && !account.executable && account.lamports === '0' && account.data.length === 0)) return 'create'; if (account.owner !== owner || account.executable || BigInt(account.lamports) < minimum || !same(account.data, expected)) throw new Error(`${field} is neither vacant nor byte-identical immutable replay state`); return 'repeat'; }
function finalizedRecord(accounts: ReadonlyMap<string, RpcAccount | null>, addresses: Readonly<{ record: string; staging: string }>, owner: string, expected: Uint8Array, rent: bigint, field: string): void { const record = required(accounts, addresses.record, `${field} record`); if (record.owner !== owner || record.executable || BigInt(record.lamports) < rent || !same(record.data, expected)) throw new Error(`${field} finalized record bytes, owner, or rent reserve differ`); const staging = accounts.get(addresses.staging); if (staging !== null && staging !== undefined && (staging.owner !== SYSTEM_PROGRAM_ID || staging.executable || staging.lamports !== '0' || staging.data.length !== 0)) throw new Error(`${field} staging cursor is not canonically vacant`); }

export async function prepareProductV2LiabilityTransaction(client: ProductRpc, product: CompiledProductV2, input: Readonly<{ registryProgram: string; payer: string; market: string; resultDomainRecord: string; lookupTable: string; available: bigint; computeUnitLimit: number }>): Promise<ProductV2LiabilityPlan> {
  const registry = key(input.registryProgram, 'Registry program'); const payer = key(input.payer, 'payer'); const marketAddress = key(input.market, 'Market'); key(input.resultDomainRecord, 'result-domain record'); key(input.lookupTable, 'address lookup table'); exactU64(input.available, 'available collateral');
  const floor = await client.finalizedSlot(); const first = await client.multipleAccounts([input.market, input.resultDomainRecord], floor); const firstMap = new Map(first.accounts.map((entry) => [entry.address, entry.account])); const marketAccount = required(firstMap, input.market, 'Market'); if (marketAccount.owner !== input.registryProgram || marketAccount.executable) throw new Error('Market owner or executable flag differs from Registry authority'); const market = await decodeMarket(marketAccount.data, input.market, input.registryProgram); const domainAccount = required(firstMap, input.resultDomainRecord, 'result-domain record'); if (domainAccount.owner !== input.registryProgram || domainAccount.executable) throw new Error('result-domain record owner or executable flag differs'); const domain = await decodeDomain(domainAccount.data);
  const manifestAddresses = deriveFinalizedRecordAddressesV1(input.registryProgram, CAPABILITY_MANIFEST_SCHEMA, market.manifestId); const instanceAddresses = deriveFinalizedRecordAddressesV1(input.registryProgram, PRODUCT_INSTANCE_SCHEMA, market.productInstanceId); const secondAddresses = [...new Set([manifestAddresses.record, instanceAddresses.record])]; const second = await client.multipleAccounts(secondAddresses, first.slot); const secondMap = new Map(second.accounts.map((entry) => [entry.address, entry.account])); const manifestBytes = required(secondMap, manifestAddresses.record, 'capability manifest').data; if (hex(await sha256(manifestBytes)) !== hex(market.manifestId)) throw new Error('capability manifest content differs from Market identity'); const capability = decodeCapability(manifestBytes); const instanceBytes = required(secondMap, instanceAddresses.record, 'Product instance').data; if (hex(await sha256(instanceBytes)) !== hex(market.productInstanceId)) throw new Error('Product instance content differs from Market identity'); const instance = decodeInstance(instanceBytes);
  const bindingAddresses = deriveFinalizedRecordAddressesV1(input.registryProgram, fromHex('7ecfd4fc07a4c69a295237b6d0ad81448f5a6814f167a17aa79a4e4395087791', 'payoff binding schema'), capability.bindingDigest); const third = await client.multipleAccounts([bindingAddresses.record], second.slot); const bindingBytes = required(new Map(third.accounts.map((entry) => [entry.address, entry.account])), bindingAddresses.record, 'payoff binding').data; if (hex(await sha256(bindingBytes)) !== hex(capability.bindingDigest)) throw new Error('payoff binding content differs from capability config'); const binding = decodeBinding(bindingBytes, capability.bindingDigest);
  if (!same(binding.payoffRecordDigest, product.digest) || binding.productId !== product.input.productId || binding.domainId !== product.input.domainId || binding.coordinateUnitId !== product.input.coordinateUnitId || binding.payoutScale !== product.input.payoutScale) throw new Error('authored payoff bytes/scalars do not equal the Market-selected binding'); if (!same(binding.productInstanceId, market.productInstanceId) || !same(binding.resultDomainId, domain.semanticId) || !same(instance.resultDomainId, domain.semanticId) || !same(instance.claimBasisId, market.claimBasisId) || !same(instance.capacityProfileId, capability.capacityProfileId) || instance.outcomeCount !== domain.outcomeCount || market.outcomeCount !== domain.outcomeCount) throw new Error('Market, Product instance, capability, and finite result domain do not join');
  const payoffAddresses = deriveFinalizedRecordAddressesV1(input.registryProgram, PAYOFF_RECORD_SCHEMA, product.digest); const domainDigest = await sha256(domainAccount.data); const expectedDomainAddresses = deriveFinalizedRecordAddressesV1(input.registryProgram, RESULT_DOMAIN_SCHEMA, domainDigest); if (expectedDomainAddresses.record !== input.resultDomainRecord) throw new Error('result-domain record is not its finalized Registry PDA');
  const artifactDigests = { evaluator: binding.payoffArtifactDigest, admission: binding.admissionArtifactDigest, resolution: binding.resolutionArtifactDigest }; const artifactAddresses = Object.fromEntries(Object.entries(artifactDigests).map(([name, digest]) => [name, deriveFinalizedRecordAddressesV1(input.registryProgram, ARTIFACT_RELEASE_SCHEMA_ID_V1, digest)])) as Record<'evaluator' | 'admission' | 'resolution', Readonly<{ record: string; staging: string }>>;
  const artifactRead = await client.multipleAccounts(Object.values(artifactAddresses).map((value) => value.record), third.slot); const artifactMap = new Map(artifactRead.accounts.map((entry) => [entry.address, entry.account])); const artifacts = Object.fromEntries((['evaluator', 'admission', 'resolution'] as const).map((name) => [name, decodeArtifactReleaseV1(required(artifactMap, artifactAddresses[name].record, `${name} artifact`).data)])) as Record<'evaluator' | 'admission' | 'resolution', ArtifactReleaseV1>;
  const programs = { evaluator: binding.payoffProgram, admission: binding.admissionProgram, resolution: binding.resolutionProgram }; const semantic = { evaluator: PRODUCT_PAYOFF_ADAPTER_RELEASE, admission: PRODUCT_PAYOFF_ADMISSION_RELEASE, resolution: RESOLUTION_CONTROLLER_RELEASE }; for (const name of ['evaluator', 'admission', 'resolution'] as const) if (artifacts[name].program !== programs[name] || artifacts[name].semanticReleaseId !== semantic[name]) throw new Error(`${name} artifact does not bind the selected program and semantic release`);
  const query = await sha256(concat(new Uint8Array(16), new Uint8Array(8), u64Bytes(input.available))); const certificate = PublicKey.findProgramAddressSync([CERTIFICATE_SEED, registry.toBytes(), product.digest, binding.payoffArtifactDigest, Uint8Array.of(1), query], key(binding.payoffProgram, 'payoff program'))[0]; const expectedCertificate = certificateBytes(registry, product, binding.payoffArtifactDigest, input.available); const certificateDigest = await sha256(expectedCertificate); const zero = new Uint8Array(32); const receipt = PublicKey.findProgramAddressSync([RECEIPT_SEED, marketAddress.toBytes(), u64Bytes(market.generation), Uint8Array.of(0), binding.digest, certificateDigest, zero], key(binding.admissionProgram, 'admission program'))[0]; const totalBound = product.liabilityBound > binding.failurePayout ? product.liabilityBound : binding.failurePayout; if (input.available < totalBound) throw new Error(`available collateral ${input.available} is below the evaluator/failure bound ${totalBound}`); const expectedReceipt = receiptBytes({ market: marketAddress, marketView: market, binding, certificate, certificateDigest, domain, totalBound }); const receiptDigest = await sha256(expectedReceipt);
  const evaluatorAccounts = [input.payer, certificate.toBase58(), payoffAddresses.record, payoffAddresses.staging, artifactAddresses.evaluator.record, artifactAddresses.evaluator.staging, binding.payoffProgram, artifacts.evaluator.programData, RENT_SYSVAR_ID, SYSTEM_PROGRAM_ID]; const admissionAccounts = [input.payer, receipt.toBase58(), certificate.toBase58(), input.market, manifestAddresses.record, manifestAddresses.staging, bindingAddresses.record, bindingAddresses.staging, instanceAddresses.record, instanceAddresses.staging, input.resultDomainRecord, expectedDomainAddresses.staging, payoffAddresses.record, payoffAddresses.staging, artifactAddresses.evaluator.record, artifactAddresses.evaluator.staging, binding.payoffProgram, artifacts.evaluator.programData, artifactAddresses.admission.record, artifactAddresses.admission.staging, binding.admissionProgram, artifacts.admission.programData, artifactAddresses.resolution.record, artifactAddresses.resolution.staging, binding.resolutionProgram, artifacts.resolution.programData, RENT_SYSVAR_ID, SYSTEM_PROGRAM_ID]; if (new Set(evaluatorAccounts).size !== evaluatorAccounts.length || new Set(admissionAccounts).size !== admissionAccounts.length) throw new Error('selected authority aliases SBF roles that must be distinct');
  const finalAddresses = [...new Set([...evaluatorAccounts, ...admissionAccounts, input.lookupTable])]; if (finalAddresses.length > 32) throw new Error('Product finalized dependency set exceeds the 32-account RPC batch'); const final = await client.multipleAccounts(finalAddresses, artifactRead.slot); const accounts = new Map(final.accounts.map((entry) => [entry.address, entry.account])); const payerAccount = required(accounts, payer.toBase58(), 'payer'); if (payerAccount.owner !== SYSTEM_PROGRAM_ID || payerAccount.executable || payerAccount.data.length !== 0) throw new Error('payer is not a System-owned data-free wallet'); const rentAccount = required(accounts, RENT_SYSVAR_ID, 'Rent sysvar'); const system = required(accounts, SYSTEM_PROGRAM_ID, 'System Program'); if (rentAccount.owner !== SYSVAR_OWNER_ID || rentAccount.executable || rentAccount.data.length !== 17 || system.owner !== NATIVE_LOADER_ID || !system.executable) throw new Error('Rent or System runtime account is not canonical');
  const finalMarket = required(accounts, input.market, 'Market'); if (finalMarket.owner !== input.registryProgram || finalMarket.executable || !same(finalMarket.data, marketAccount.data)) throw new Error('Market changed between finalized authority reads');
  const rentWidths = [...new Set([PRODUCT_V2_BYTES, 384, 192, 352, manifestBytes.length, 216, PAYOFF_CERTIFICATE_BYTES_V2, PAYOFF_ADMISSION_RECEIPT_BYTES_V1])]; const rentPairs = await Promise.all(rentWidths.map(async (width) => [width, BigInt((await client.minimumBalanceForRentExemption(width)).lamports)] as const)); const rents = new Map(rentPairs); finalizedRecord(accounts, payoffAddresses, input.registryProgram, product.bytes, rents.get(PRODUCT_V2_BYTES)!, 'Product payoff'); finalizedRecord(accounts, manifestAddresses, input.registryProgram, manifestBytes, rents.get(manifestBytes.length)!, 'capability manifest'); finalizedRecord(accounts, bindingAddresses, input.registryProgram, bindingBytes, rents.get(384)!, 'payoff binding'); finalizedRecord(accounts, instanceAddresses, input.registryProgram, instanceBytes, rents.get(192)!, 'Product instance'); finalizedRecord(accounts, expectedDomainAddresses, input.registryProgram, domainAccount.data, rents.get(352)!, 'finite result domain');
  for (const name of ['evaluator', 'admission', 'resolution'] as const) { finalizedRecord(accounts, artifactAddresses[name], input.registryProgram, artifacts[name].bytes, rents.get(216)!, `${name} artifact`); await authenticateArtifactDeploymentV1(required(accounts, programs[name], `${name} Program`), programs[name], required(accounts, artifacts[name].programData, `${name} ProgramData`), artifacts[name].programData, artifacts[name]); }
  const certificateMode = vacantOrExact(accounts.get(certificate.toBase58()), binding.payoffProgram, expectedCertificate, rents.get(PAYOFF_CERTIFICATE_BYTES_V2)!, 'payoff certificate'); const receiptMode = vacantOrExact(accounts.get(receipt.toBase58()), binding.admissionProgram, expectedReceipt, rents.get(PAYOFF_ADMISSION_RECEIPT_BYTES_V1)!, 'admission receipt'); const rentDebit = (certificateMode === 'create' ? rents.get(PAYOFF_CERTIFICATE_BYTES_V2)! : BigInt(0)) + (receiptMode === 'create' ? rents.get(PAYOFF_ADMISSION_RECEIPT_BYTES_V1)! : BigInt(0)); if (BigInt(payerAccount.lamports) < rentDebit) throw new Error('payer cannot cover exact certificate and admission-receipt rent');
  const lookupAccount = required(accounts, input.lookupTable, 'address lookup table'); if (lookupAccount.owner !== AddressLookupTableProgram.programId.toBase58() || lookupAccount.executable) throw new Error('address lookup table account owner or executable flag is invalid'); const lookupTable = new AddressLookupTableAccount({ key: key(input.lookupTable, 'lookup table'), state: AddressLookupTableAccount.deserialize(lookupAccount.data) }); const request = buildRequest(product.digest, binding.payoffArtifactDigest, input.available); const admission = admissionRequest(market.generation, binding.digest, certificateDigest); const blockhash = await client.latestBlockhash(final.slot); const compiled = compileProductV2LiabilityTransaction({ payer: input.payer, recentBlockhash: blockhash.blockhash, computeUnitLimit: input.computeUnitLimit, lookupTable, request, admissionRequest: admission, evaluatorProgram: binding.payoffProgram, admissionProgram: binding.admissionProgram, evaluatorAccounts, admissionAccounts });
  return Object.freeze({ observedSlot: final.slot, market: input.market, generation: market.generation.toString(), registryProgram: input.registryProgram, bindingDigest: hex(binding.digest), payoffProgram: binding.payoffProgram, admissionProgram: binding.admissionProgram, resolutionProgram: binding.resolutionProgram, certificate: certificate.toBase58(), certificateDigest: hex(certificateDigest), certificateMode, receipt: receipt.toBase58(), receiptDigest: hex(receiptDigest), receiptMode, available: input.available.toString(), liabilityBound: product.liabilityBound.toString(), failurePayout: binding.failurePayout.toString(), totalBound: totalBound.toString(), rentDebitLamports: rentDebit.toString(), requestBytes: request, admissionBytes: admission, wireBytes: compiled.wireBytes, transaction: compiled.transaction, requiredSigners: compiled.requiredSigners, lookupAddressesUsed: compiled.lookupAddressesUsed, resultDomain: Object.freeze({ record: input.resultDomainRecord, semanticId: hex(domain.semanticId), coordinateDomainId: domain.coordinateDomainId, resultUnitId: domain.resultUnitId, denominator: domain.denominator.toString(), cuts: Object.freeze(domain.cuts.map((value) => value.toString())), outcomeCount: domain.outcomeCount }), artifactReleases: Object.freeze(Object.fromEntries((['evaluator', 'admission', 'resolution'] as const).map((name) => [name, Object.freeze({ digest: hex(artifactDigests[name]), semanticRelease: semantic[name], programData: artifacts[name].programData, deploymentSlot: artifacts[name].deploymentSlot.toString() })])) as ProductV2LiabilityPlan['artifactReleases']) });
}

function u64Bytes(value: bigint): Uint8Array { const bytes = new Uint8Array(8); putU64(bytes, 0, value); return bytes; }

export function productInteger(value: string, field: string): bigint { return parseInteger(value, field); }
