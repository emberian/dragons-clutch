import { ascii, requireZero, slice, u16, u64 } from './bytes';

export const COMPACT_INTENT_BYTES = 136;
export const CONTROLLER_INSTRUCTION_BYTES = 304;
export const MARKET_PROFILE_BYTES = 136;

const VERSION = 1;

export interface CompactIntentV1 {
  side: number;
  outcome: number;
  lifecycle: number;
  executionProfile: Uint8Array;
  generation: bigint;
  nonce: bigint;
  validFrom: bigint;
  validThrough: bigint;
  maximumFill: bigint;
  limitPrice: bigint;
  feeBasisPoints: number;
  collateralAccount: Uint8Array;
}

export interface ControllerInstructionV1 {
  controllerBump: number;
  sellerReplayBump: number;
  buyerReplayBump: number;
  sellerPositionBump: number;
  buyerPositionBump: number;
  fill: bigint;
  executionPrice: bigint;
  seller: CompactIntentV1;
  buyer: CompactIntentV1;
}

export interface MarketProfileV1 {
  phase: number;
  outcomeCount: number;
  generation: bigint;
  priceScale: bigint;
  feeBasisPoints: number;
  tokenProgram: Uint8Array;
  collateralMint: Uint8Array;
  feeRecipient: Uint8Array;
}

function exact(bytes: Uint8Array, width: number, magic: string): void {
  if (bytes.length !== width) throw new Error(`expected exactly ${width} bytes`);
  if (ascii(bytes, 0, 8) !== magic) throw new Error('wrong compiled Direct domain');
  if (u16(bytes, 8) !== VERSION) throw new Error('unsupported compiled Direct version');
}

function byte(value: number, field: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 255) throw new Error(`${field} is not a byte`);
  return value;
}

function word(value: number, field: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 65_535) throw new Error(`${field} is not a u16`);
  return value;
}

function key(value: Uint8Array, field: string): Uint8Array {
  if (value.length !== 32) throw new Error(`${field} is not 32 bytes`);
  return value;
}

function putU16(output: Uint8Array, offset: number, value: number): void {
  new DataView(output.buffer, output.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU64(output: Uint8Array, offset: number, value: bigint, field: string): void {
  if (value < 0n || value > 18_446_744_073_709_551_615n) throw new Error(`${field} is not a u64`);
  new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, value, true);
}

export function decodeCompactIntentV1(bytes: Uint8Array): CompactIntentV1 {
  exact(bytes, COMPACT_INTENT_BYTES, 'DCLTDIR3');
  requireZero(bytes, 13, 3, 'compact intent header');
  requireZero(bytes, 98, 6, 'compact intent fee padding');
  return Object.freeze({
    side: bytes[10],
    outcome: bytes[11],
    lifecycle: bytes[12],
    executionProfile: slice(bytes, 16, 32),
    generation: u64(bytes, 48),
    nonce: u64(bytes, 56),
    validFrom: u64(bytes, 64),
    validThrough: u64(bytes, 72),
    maximumFill: u64(bytes, 80),
    limitPrice: u64(bytes, 88),
    feeBasisPoints: u16(bytes, 96),
    collateralAccount: slice(bytes, 104, 32),
  });
}

export function encodeCompactIntentV1(intent: CompactIntentV1): Uint8Array {
  const output = new Uint8Array(COMPACT_INTENT_BYTES);
  output.set(new TextEncoder().encode('DCLTDIR3'), 0);
  putU16(output, 8, VERSION);
  output[10] = byte(intent.side, 'side');
  output[11] = byte(intent.outcome, 'outcome');
  output[12] = byte(intent.lifecycle, 'lifecycle');
  output.set(key(intent.executionProfile, 'execution profile'), 16);
  putU64(output, 48, intent.generation, 'generation');
  putU64(output, 56, intent.nonce, 'nonce');
  putU64(output, 64, intent.validFrom, 'valid from');
  putU64(output, 72, intent.validThrough, 'valid through');
  putU64(output, 80, intent.maximumFill, 'maximum fill');
  putU64(output, 88, intent.limitPrice, 'limit price');
  putU16(output, 96, word(intent.feeBasisPoints, 'fee basis points'));
  output.set(key(intent.collateralAccount, 'collateral account'), 104);
  return output;
}

export function decodeControllerInstructionV1(bytes: Uint8Array): ControllerInstructionV1 {
  exact(bytes, CONTROLLER_INSTRUCTION_BYTES, 'DCLTCTL1');
  requireZero(bytes, 15, 1, 'controller header');
  return Object.freeze({
    controllerBump: bytes[10],
    sellerReplayBump: bytes[11],
    buyerReplayBump: bytes[12],
    sellerPositionBump: bytes[13],
    buyerPositionBump: bytes[14],
    fill: u64(bytes, 16),
    executionPrice: u64(bytes, 24),
    seller: decodeCompactIntentV1(slice(bytes, 32, COMPACT_INTENT_BYTES)),
    buyer: decodeCompactIntentV1(slice(bytes, 168, COMPACT_INTENT_BYTES)),
  });
}

export function encodeControllerInstructionV1(instruction: ControllerInstructionV1): Uint8Array {
  const output = new Uint8Array(CONTROLLER_INSTRUCTION_BYTES);
  output.set(new TextEncoder().encode('DCLTCTL1'), 0);
  putU16(output, 8, VERSION);
  output[10] = byte(instruction.controllerBump, 'controller bump');
  output[11] = byte(instruction.sellerReplayBump, 'seller replay bump');
  output[12] = byte(instruction.buyerReplayBump, 'buyer replay bump');
  output[13] = byte(instruction.sellerPositionBump, 'seller Position bump');
  output[14] = byte(instruction.buyerPositionBump, 'buyer Position bump');
  putU64(output, 16, instruction.fill, 'fill');
  putU64(output, 24, instruction.executionPrice, 'execution price');
  output.set(encodeCompactIntentV1(instruction.seller), 32);
  output.set(encodeCompactIntentV1(instruction.buyer), 168);
  return output;
}

export function decodeMarketProfileV1(bytes: Uint8Array): MarketProfileV1 {
  exact(bytes, MARKET_PROFILE_BYTES, 'DCLTPRF1');
  requireZero(bytes, 12, 4, 'Market profile header');
  requireZero(bytes, 34, 6, 'Market profile fee padding');
  return Object.freeze({
    phase: bytes[10],
    outcomeCount: bytes[11],
    generation: u64(bytes, 16),
    priceScale: u64(bytes, 24),
    feeBasisPoints: u16(bytes, 32),
    tokenProgram: slice(bytes, 40, 32),
    collateralMint: slice(bytes, 72, 32),
    feeRecipient: slice(bytes, 104, 32),
  });
}

export function encodeMarketProfileV1(profile: MarketProfileV1): Uint8Array {
  const output = new Uint8Array(MARKET_PROFILE_BYTES);
  output.set(new TextEncoder().encode('DCLTPRF1'), 0);
  putU16(output, 8, VERSION);
  output[10] = byte(profile.phase, 'phase');
  output[11] = byte(profile.outcomeCount, 'outcome count');
  putU64(output, 16, profile.generation, 'generation');
  putU64(output, 24, profile.priceScale, 'price scale');
  putU16(output, 32, word(profile.feeBasisPoints, 'fee basis points'));
  output.set(key(profile.tokenProgram, 'token program'), 40);
  output.set(key(profile.collateralMint, 'collateral mint'), 72);
  output.set(key(profile.feeRecipient, 'fee recipient'), 104);
  return output;
}
