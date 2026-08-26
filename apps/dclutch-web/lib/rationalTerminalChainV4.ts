import { ascii, hex, requireNonzero, requireZero, sha256, slice, u16 } from './bytes';
import * as Hot from './generated/directInlineV3';
import { inspectRationalCapabilityCommonV4 } from './rationalCapabilityChainV4';
import {
  acquireRationalHotAccountsV4,
  authenticateFinalizedRationalHotRecordV4,
  type RationalHotRpcV4,
} from './rationalRetireReceiptV4';
import {
  decodeRationalProductBasisV3,
  evaluateRationalTerminalPayoutV3,
  type RationalProductBasisViewV3,
  type RationalTerminalPayoutV3,
} from './rationalTerminalHotV3';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';

const MAX_U64 = 18_446_744_073_709_551_615n;
const RATIONAL_TERMINAL_HOT_REQUEST_SCHEMA_ID_V3 = Uint8Array.from([
  0x8b,0xab,0xcd,0x90,0x65,0xc6,0x52,0x25,0x32,0xe2,0x6c,0x60,0x63,0x61,0x56,0x72,
  0x4f,0x7f,0x6c,0xfc,0xfb,0xa2,0x60,0x82,0x75,0x86,0x27,0xc4,0x9c,0xdc,0x80,0x3b,
]);
const GRADED_BASIS_RECORD_SCHEMA_ID_V3 = Uint8Array.from([
  0x0b,0x68,0xd1,0xa8,0x31,0xe9,0xee,0x4b,0x7c,0x7f,0x04,0x7f,0x69,0x82,0xea,0xde,
  0x18,0x01,0xc7,0x59,0x68,0xcd,0x2e,0x49,0x8b,0x76,0x62,0xe7,0xee,0x62,0xd0,0xe8,
]);
const TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2 = Uint8Array.from([
  0xa8,0x66,0x06,0x2a,0xe7,0x6d,0x3d,0xc3,0xa7,0xc7,0xce,0xe5,0x34,0x0a,0xc9,0xe4,
  0x1f,0x20,0x22,0x69,0xcb,0x23,0xe9,0xb7,0x04,0x61,0xb0,0x16,0xf1,0x8d,0x5f,0x61,
]);
const SEMANTIC_BASIS_CONTENT_DOMAIN_V3 = new TextEncoder().encode('dclutch/product-basis/semantic/v3');

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function concatenate(parts: ReadonlyArray<Uint8Array>): Uint8Array {
  const total = parts.reduce((sum, part) => sum + part.length, 0);
  if (!Number.isSafeInteger(total)) throw new Error('terminal semantic preimage exceeds the browser exact-length bound');
  const output = new Uint8Array(total); let offset = 0;
  for (const part of parts) { output.set(part, offset); offset += part.length; }
  return output;
}

function readI64(bytes: Uint8Array, offset: number): bigint {
  const value = slice(bytes, offset, 8);
  return new DataView(value.buffer, value.byteOffset, value.byteLength).getBigInt64(0, true);
}

function readU32(bytes: Uint8Array, offset: number): number {
  const value = slice(bytes, offset, 4);
  return new DataView(value.buffer, value.byteOffset, value.byteLength).getUint32(0, true);
}

function decodeTerminalCoordinateV2(bytes: Uint8Array): Readonly<{ numerator: bigint; denominator: bigint }> {
  if (bytes.length !== 32 || ascii(bytes, 0, 8) !== 'DCLTRC02' || u16(bytes, 8) !== 2) {
    throw new Error('terminal coordinate has the wrong exact V2 ABI');
  }
  requireZero(bytes, 10, 6, 'terminal coordinate header'); requireZero(bytes, 28, 4, 'terminal coordinate tail');
  const denominator = BigInt(readU32(bytes, 24));
  if (denominator === 0n) throw new Error('terminal coordinate denominator is zero');
  return Object.freeze({ numerator: readI64(bytes, 16), denominator });
}

async function authenticateProductBasisV3(
  client: RationalHotRpcV4,
  common: Awaited<ReturnType<typeof inspectRationalCapabilityCommonV4>>,
): Promise<Readonly<{ basis: RationalProductBasisViewV3; digest: Uint8Array; semanticBasisId: Uint8Array }>> {
  const rawAddress = common.fixed[Hot.HOT_LINKED_BASIS_RAW_ACCOUNT_V3]?.address ?? '';
  const stagingAddress = common.fixed[Hot.HOT_LINKED_BASIS_STAGING_ACCOUNT_V3]?.address ?? '';
  const raw = common.accounts.get(rawAddress);
  if (raw === null || raw === undefined) throw new Error('ProductBasisV3 raw record is absent at finalized commitment');
  const digest = await sha256(raw.data);
  const addresses = deriveFinalizedRecordAddressesV1(common.registry, GRADED_BASIS_RECORD_SCHEMA_ID_V3, digest);
  if (addresses.record !== rawAddress || addresses.staging !== stagingAddress) {
    throw new Error('Hot linked-basis coordinates are not the content-addressed ProductBasisV3 record');
  }
  await authenticateFinalizedRationalHotRecordV4(client, common.accounts, common.registry, rawAddress, stagingAddress,
    GRADED_BASIS_RECORD_SCHEMA_ID_V3, digest, 'ProductBasisV3');
  const basis = decodeRationalProductBasisV3(raw.data);
  const coordinateDomain = slice(common.domainRaw.data, 64, 32);
  const resultUnit = slice(common.domainRaw.data, 96, 32);
  const semanticBasisId = await sha256(concatenate([
    SEMANTIC_BASIS_CONTENT_DOMAIN_V3, basis.bytes.slice(0, 32), basis.bytes.slice(96),
  ]));
  requireNonzero(semanticBasisId, 'Product semantic basis');
  if (!same(basis.productId, common.product.productId) || !same(basis.resultDomainId, common.domainDigest)
      || !same(basis.coordinateDomainId, coordinateDomain) || !same(basis.resultUnitId, resultUnit)
      || !same(semanticBasisId, slice(common.domainRaw.data, 128, 32))
      || basis.width !== common.descriptor.outcomeCount) {
    throw new Error('ProductBasisV3 does not join Product/domain semantics or representation K');
  }
  return Object.freeze({ basis, digest, semanticBasisId });
}

export type RationalTerminalReadinessV4 = Readonly<{
  observedSlot: string;
  market: string;
  generation: bigint;
  actor: string;
  descriptorId: Uint8Array;
  capabilityDigest: Uint8Array;
  basisDigest: Uint8Array;
  semanticBasisId: Uint8Array;
  resultOutcomeCount: number;
  representationWidth: number;
  terminalWinner: number;
  selectedOutcome: number;
  rawQuantity: bigint;
  rawShardBurn: bigint;
  terminalCoordinate: Readonly<{ numerator: bigint; denominator: bigint }> | null;
  payout: RationalTerminalPayoutV3;
  executionStatus: 'blocked';
  refusal: string;
}>;

/**
 * Read the complete immutable Product/representation terminal semantics.
 * This is an untrusted browser projection; the Rust operator remains the sole
 * emitter of SignedDeltaV3 and Custody authority/digest material.
 */
export async function inspectRationalTerminalReadinessV4(
  client: RationalHotRpcV4,
  input: Readonly<{
    payer: string;
    actor: string;
    fixedAccounts: ReadonlyArray<string>;
    lookupTable: string;
    descriptorId: string;
    selectedOutcome: number;
    rawQuantity: bigint;
  }>,
): Promise<RationalTerminalReadinessV4> {
  if (!Number.isInteger(input.selectedOutcome) || input.selectedOutcome < 0 || input.selectedOutcome > 0xffff_ffff) {
    throw new Error('selected representation outcome must be one canonical u32 index');
  }
  if (input.rawQuantity <= 0n || input.rawQuantity > MAX_U64) throw new Error('terminal quantity must be 1..u64::MAX raw units');
  const common = await inspectRationalCapabilityCommonV4(client, {
    phase: 'terminal', selector: 5, requestSchema: RATIONAL_TERMINAL_HOT_REQUEST_SCHEMA_ID_V3,
    payer: input.payer, actor: input.actor, fixedAccounts: input.fixedAccounts, lookupTable: input.lookupTable,
    descriptorId: input.descriptorId,
  });
  const admitted = await authenticateProductBasisV3(client, common);
  if (input.selectedOutcome >= admitted.basis.width) throw new Error('selected representation outcome is outside Product basis K');
  if (common.market.terminalWinner >= common.product.outcomeCount) throw new Error('Core terminal winner is outside Product result N');
  let terminalCoordinate: Readonly<{ numerator: bigint; denominator: bigint }> | null = null;
  let observedSlot = common.observedSlot;
  if (admitted.basis.kind === 'graded-exact-complement'
      && common.market.terminalWinner !== common.product.outcomeCount - 1) {
    const addresses = deriveFinalizedRecordAddressesV1(common.coreProgram, TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2, common.market.terminalReceipt);
    const observation = await acquireRationalHotAccountsV4(client, [addresses.record, addresses.staging], common.observedSlot);
    const raw = await authenticateFinalizedRationalHotRecordV4(client, observation.accounts, common.coreProgram,
      addresses.record, addresses.staging, TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2, common.market.terminalReceipt, 'Core terminal coordinate');
    terminalCoordinate = decodeTerminalCoordinateV2(raw.data); observedSlot = observation.slot;
  }
  const payout = evaluateRationalTerminalPayoutV3({
    basis: admitted.basis.bytes, resultOutcomeCount: common.product.outcomeCount,
    terminalWinner: common.market.terminalWinner, selectedOutcome: input.selectedOutcome,
    rawQuantity: input.rawQuantity, terminalCoordinate,
  });
  const rawShardBurn = common.descriptor.denominator * input.rawQuantity;
  if (rawShardBurn > MAX_U64) throw new Error('terminal raw shard burn exceeds u64::MAX');
  return Object.freeze({ observedSlot, market: common.marketAddress, generation: common.market.generation,
    actor: common.actor, descriptorId: common.descriptorId, capabilityDigest: common.capabilitySelection.digest,
    basisDigest: admitted.digest, semanticBasisId: admitted.semanticBasisId,
    resultOutcomeCount: common.product.outcomeCount, representationWidth: admitted.basis.width,
    terminalWinner: common.market.terminalWinner, selectedOutcome: input.selectedOutcome,
    rawQuantity: input.rawQuantity, rawShardBurn, terminalCoordinate, payout,
    executionStatus: 'blocked',
    refusal: 'Positive and exact-zero terminal execution are SBF-tested, but the browser has no binding to the canonical Rust SignedDeltaV3/Custody emitter. It will not reconstruct that authority or request digest in TypeScript.',
  });
}

export function rationalTerminalReadinessSummaryV4(value: RationalTerminalReadinessV4): Readonly<Record<string, string>> {
  return Object.freeze({ market: value.market, descriptor: hex(value.descriptorId), basis: hex(value.basisDigest),
    widths: `K=${value.representationWidth} claims over N=${value.resultOutcomeCount} terminal results`,
    result: `${value.payout.scenario} · winner ${value.terminalWinner} · claim ${value.selectedOutcome}`,
    economics: `${value.rawShardBurn.toString()} shard atoms → ${value.payout.rawPayout.toString()} collateral atoms${value.payout.losing ? ' (exact zero)' : ''}` });
}
