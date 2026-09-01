import { fromHex, hex, sha256 } from './bytes';
import {
  RATIONAL_OPEN_PLAN_FORMAT_V1,
  RATIONAL_OPEN_WASM_BYTES_V1,
  RATIONAL_OPEN_WASM_SHA256_V1,
} from './generated/rationalOpenWasmV1';

const MAX_PLAN_CHARACTERS = 2 * 1024 * 1024;

export type RationalOpenWasmV1 = Readonly<{
  plan_rational_open_v1(source: string): string;
}>;

export type RationalOpenWasmPlanV1 = Readonly<{
  action: 'denominate' | 'reconstitute' | 'issue-structured' | 'unwrap-structured';
  familyBytes: Uint8Array;
  familyDigest: Uint8Array;
  claimsChild: Uint8Array;
  claimsChildDigest: Uint8Array;
  assetCount: number;
  logicalClaimsAccounts: number;
  rawQuantity: bigint;
  receiptEffect: 'none' | 'mint' | 'burn';
  rawReceiptDelta: bigint;
  shardEffect: 'mint-to-actor' | 'burn-from-actor' | 'actor-to-custody' | 'custody-to-actor';
  rawShardDeltas: ReadonlyArray<bigint>;
}>;

let loaded: Promise<RationalOpenWasmV1> | null = null;

function plain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function object(value: unknown, fields: ReadonlyArray<string>, label: string): Record<string, unknown> {
  if (!plain(value)) throw new Error(`${label} is not one object`);
  const actual = Object.keys(value).sort();
  const expected = [...fields].sort();
  if (actual.length !== expected.length || actual.some((field, index) => field !== expected[index])) {
    throw new Error(`${label} has missing or unknown fields`);
  }
  return value;
}

function base64(value: unknown, field: string): Uint8Array {
  if (typeof value !== 'string' || value.length % 4 !== 0
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error(`${field} is not canonical base64`);
  }
  const bytes = Uint8Array.from(atob(value), (character) => character.charCodeAt(0));
  let encoded = '';
  for (let offset = 0; offset < bytes.length; offset += 8_192) {
    encoded += String.fromCharCode(...bytes.subarray(offset, offset + 8_192));
  }
  if (btoa(encoded) !== value) throw new Error(`${field} is not canonical base64`);
  return bytes;
}

function unsigned(value: unknown, field: string): bigint {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value)) {
    throw new Error(`${field} is not canonical unsigned decimal text`);
  }
  const parsed = BigInt(value);
  if (parsed > 18_446_744_073_709_551_615n) throw new Error(`${field} exceeds u64`);
  return parsed;
}

function safeUnsigned(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${field} is not one safe unsigned integer`);
  }
  return value;
}

function literal<T extends string>(value: unknown, options: ReadonlyArray<T>, field: string): T {
  if (typeof value !== 'string' || !options.includes(value as T)) throw new Error(`${field} is not recognized`);
  return value as T;
}

function digest(value: unknown, field: string): Uint8Array {
  if (typeof value !== 'string') throw new Error(`${field} is not hexadecimal text`);
  return fromHex(value, field);
}

/** Parse and independently authenticate the exact Rust/WASM plan output. */
export async function parseRationalOpenWasmPlanV1(source: string): Promise<RationalOpenWasmPlanV1> {
  if (source.length === 0 || source.length > MAX_PLAN_CHARACTERS) throw new Error('Rational-open plan is outside its bounded JSON size');
  let parsed: unknown;
  try { parsed = JSON.parse(source); } catch { throw new Error('Rational-open plan is not JSON'); }
  const raw = object(parsed, [
    'action', 'assetCount', 'claimsChildBase64', 'claimsChildSha256', 'familyBase64',
    'familySha256', 'format', 'logicalClaimsAccounts', 'rawQuantity', 'rawReceiptDelta',
    'rawShardDeltas', 'receiptEffect', 'shardEffect',
  ], 'Rational-open plan');
  if (raw.format !== RATIONAL_OPEN_PLAN_FORMAT_V1) throw new Error('Rational-open plan has another format');
  const action = literal(raw.action, ['denominate', 'reconstitute', 'issue-structured', 'unwrap-structured'] as const, 'Rational-open action');
  const familyBytes = base64(raw.familyBase64, 'Rational-open family');
  const claimsChild = base64(raw.claimsChildBase64, 'Rational-open Claims child');
  const familyDigest = digest(raw.familySha256, 'family digest');
  const claimsChildDigest = digest(raw.claimsChildSha256, 'Claims child digest');
  if (hex(await sha256(familyBytes)) !== hex(familyDigest)
      || hex(await sha256(claimsChild)) !== hex(claimsChildDigest)) {
    throw new Error('Rational-open plan digest does not authenticate its exact bytes');
  }
  const assetCount = safeUnsigned(raw.assetCount, 'asset count');
  const logicalClaimsAccounts = safeUnsigned(raw.logicalClaimsAccounts, 'logical Claims account count');
  if (assetCount === 0 || logicalClaimsAccounts !== 32 + 4 * assetCount) {
    throw new Error('Rational-open Claims account geometry changed');
  }
  if (!Array.isArray(raw.rawShardDeltas) || raw.rawShardDeltas.length !== assetCount) {
    throw new Error('Rational-open shard delta width changed');
  }
  const rawQuantity = unsigned(raw.rawQuantity, 'raw quantity');
  const receiptEffect = literal(raw.receiptEffect, ['none', 'mint', 'burn'] as const, 'receipt effect');
  const rawReceiptDelta = unsigned(raw.rawReceiptDelta, 'raw receipt delta');
  const shardEffect = literal(raw.shardEffect, ['mint-to-actor', 'burn-from-actor', 'actor-to-custody', 'custody-to-actor'] as const, 'shard effect');
  const effects = {
    denominate: ['none', 'mint-to-actor'],
    reconstitute: ['none', 'burn-from-actor'],
    'issue-structured': ['mint', 'actor-to-custody'],
    'unwrap-structured': ['burn', 'custody-to-actor'],
  } as const;
  if (receiptEffect !== effects[action][0] || shardEffect !== effects[action][1]
      || rawReceiptDelta !== (receiptEffect === 'none' ? 0n : rawQuantity)) {
    throw new Error('Rational-open plan effects do not agree with its action and exact quantity');
  }
  return Object.freeze({
    action,
    familyBytes,
    familyDigest,
    claimsChild,
    claimsChildDigest,
    assetCount,
    logicalClaimsAccounts,
    rawQuantity,
    receiptEffect,
    rawReceiptDelta,
    shardEffect,
    rawShardDeltas: Object.freeze(raw.rawShardDeltas.map((value, index) => unsigned(value, `raw shard delta ${index}`))),
  });
}

/** Load only the generated WASM artifact whose Rust-derived digest and size match. */
export async function loadRationalOpenWasmV1(
  fetcher: typeof fetch = (input, init) => globalThis.fetch(input, init),
): Promise<RationalOpenWasmV1> {
  loaded ??= (async () => {
    const wasmModule = await import('./generated/rationalOpenWasm/rational_open.js');
    const url = new URL('./generated/rationalOpenWasm/rational_open_bg.wasm', import.meta.url);
    const response = await fetcher(url);
    if (!response.ok) throw new Error(`Rational-open WASM fetch failed with HTTP ${response.status}`);
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (bytes.length !== RATIONAL_OPEN_WASM_BYTES_V1 || hex(await sha256(bytes)) !== RATIONAL_OPEN_WASM_SHA256_V1) {
      throw new Error('Rational-open WASM bytes do not match the generated Rust artifact identity');
    }
    await wasmModule.default({ module_or_path: bytes });
    return Object.freeze({ plan_rational_open_v1: wasmModule.plan_rational_open_v1 });
  })();
  try { return await loaded; } catch (error) { loaded = null; throw error; }
}
