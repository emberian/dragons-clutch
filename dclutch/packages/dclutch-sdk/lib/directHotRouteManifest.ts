import { PublicKey } from '@solana/web3.js';

import { sha256 } from './bytes';
import {
  inspectDirectHotRouteV3,
  type DirectHotRouteCoordinateV3,
  type DirectHotRouteReaderV3,
  type DirectHotRouteInspectionV3,
  type DirectHotRouteManifestV3,
} from './directHotChain';
import {
  DIRECT_INLINE_CURRENT_RUNTIME_TAIL_ACCOUNTS_V3,
  DIRECT_INLINE_NAMED_RUNTIME_FIXED_ALIASES_V3,
  DIRECT_INLINE_RUNTIME_TAIL_WRITABLE_V3,
} from './directInlineV3';
import { HOT_FIXED_ACCOUNT_COUNT_V3, HOT_ROOT_ACCOUNT_V3 } from './generated/directInlineV3';
import { CHECKED_INFRASTRUCTURE_BYTES_V1 } from './infrastructure';
import { type SolanaRpcClient } from './rpc';

/** The only public JSON envelope admitted for one Direct InlineOrdinary route. */
export const DIRECT_HOT_ROUTE_MANIFEST_FORMAT_V3 = 'dclutch-direct-hot-route-manifest-v3' as const;

export const DIRECT_HOT_ROUTE_MANIFEST_MAX_BYTES_V3 = 65_536 as const;
const DIRECT_HOT_ROUTE_MANIFEST_MAX_DEPTH_V3 = 8;
const DIRECT_HOT_ROUTE_MANIFEST_MAX_ARRAY_V3 = 256;
const DIRECT_HOT_ROUTE_MANIFEST_MAX_OBJECT_FIELDS_V3 = 16;
const DIRECT_HOT_ROUTE_MANIFEST_MAX_STRING_BYTES_V3 = 4_096;
const DIRECT_HOT_ROUTE_MANIFEST_MAX_NUMBER_BYTES_V3 = 24;
const DIRECT_HOT_ROUTE_MANIFEST_MAX_VALUES_V3 = 2_048;

/**
 * Reader labels for the exact generated 39-coordinate fixed frame.
 *
 * Labels are transport redundancy, never authority: the authenticated account
 * graph and generated coordinate constants still own the meaning of each row.
 */
export const DIRECT_HOT_FIXED_ROLE_LABELS_V3 = Object.freeze([
  'Market',
  'Direct root',
  'Manifest raw',
  'Manifest staging',
  'ProgramSet raw',
  'ProgramSet staging',
  'Descriptor raw',
  'Descriptor staging',
  'Config raw',
  'Config staging',
  'AccountProfile raw',
  'AccountProfile staging',
  'RequestProfile raw',
  'RequestProfile staging',
  'Transition raw',
  'Transition staging',
  'Effect raw',
  'Effect staging',
  'Lifecycle raw',
  'Lifecycle staging',
  'Strategy raw',
  'Strategy staging',
  'Activation cache',
  'Core program',
  'Core ProgramData',
  'Trading program',
  'Trading ProgramData',
  'Registry program',
  'Rent sysvar',
  'Instructions sysvar',
  'Product raw',
  'Product staging',
  'Result domain raw',
  'Result domain staging',
  'Portfolio raw',
  'Portfolio staging',
  'Product basis raw',
  'Product basis staging',
  'Capability seal',
] as const);

if (DIRECT_HOT_FIXED_ROLE_LABELS_V3.length !== HOT_FIXED_ACCOUNT_COUNT_V3) {
  throw new Error('Direct Hot fixed-role labels differ from the generated fixed-account count');
}

function fail(field: string, reason: string): never {
  throw new Error(`${field} is not exact bounded JSON: ${reason}`);
}

/**
 * Scan the original source before JSON.parse can erase duplicate keys.
 *
 * The scanner also bounds the input tree itself. A later shape error therefore
 * cannot be used to make the decoder first allocate an unbounded array, string,
 * number, object, or nesting chain.
 */
function exactBoundedJson(source: string | Uint8Array, field: string): unknown {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder('utf-8', { fatal: true });
  let text: string;
  let sourceBytes: Uint8Array;
  if (typeof source === 'string') {
    sourceBytes = encoder.encode(source);
    text = decoder.decode(sourceBytes);
    if (text !== source) fail(field, 'source is not canonical Unicode text');
  } else {
    sourceBytes = source;
    try {
      text = decoder.decode(source);
    } catch {
      return fail(field, 'source is not canonical UTF-8');
    }
    const roundTrip = encoder.encode(text);
    if (roundTrip.length !== source.length || roundTrip.some((byte, index) => byte !== source[index])) {
      fail(field, 'source is not canonical UTF-8');
    }
  }
  if (sourceBytes.length === 0 || sourceBytes.length > DIRECT_HOT_ROUTE_MANIFEST_MAX_BYTES_V3) {
    fail(field, `source is outside 1..${DIRECT_HOT_ROUTE_MANIFEST_MAX_BYTES_V3} bytes`);
  }

  let cursor = 0;
  let values = 0;
  const whitespace = (): void => {
    while (cursor < text.length && [' ', '\n', '\r', '\t'].includes(text[cursor] ?? '')) cursor += 1;
  };
  const string = (): string => {
    if (text[cursor] !== '"') return fail(field, 'expected one JSON string');
    const start = cursor;
    cursor += 1;
    for (;;) {
      if (cursor >= text.length) return fail(field, 'unterminated JSON string');
      const character = text[cursor] as string;
      if (character === '"') {
        cursor += 1;
        const token = text.slice(start, cursor);
        let decoded: string;
        try {
          decoded = JSON.parse(token) as string;
        } catch {
          return fail(field, 'invalid JSON string');
        }
        if (JSON.stringify(decoded) !== token) fail(field, 'string does not use canonical JSON encoding');
        if (encoder.encode(decoded).length > DIRECT_HOT_ROUTE_MANIFEST_MAX_STRING_BYTES_V3) {
          fail(field, `string exceeds ${DIRECT_HOT_ROUTE_MANIFEST_MAX_STRING_BYTES_V3} bytes`);
        }
        return decoded;
      }
      if (character.charCodeAt(0) < 0x20) fail(field, 'string contains an unescaped control character');
      if (character === '\\') {
        cursor += 1;
        if (cursor >= text.length) return fail(field, 'unterminated JSON escape');
        const escape = text[cursor] as string;
        if (escape === 'u') {
          if (!/^[0-9a-fA-F]{4}$/.test(text.slice(cursor + 1, cursor + 5))) fail(field, 'invalid Unicode escape');
          cursor += 5;
          continue;
        }
        if (!'"\\/bfnrt'.includes(escape)) fail(field, 'invalid JSON escape');
      }
      cursor += 1;
    }
  };
  const number = (): void => {
    const start = cursor;
    if (text[cursor] === '-') cursor += 1;
    if (text[cursor] === '0') cursor += 1;
    else {
      if (!/[1-9]/.test(text[cursor] ?? '')) fail(field, 'invalid JSON number');
      while (/[0-9]/.test(text[cursor] ?? '')) cursor += 1;
    }
    if (text[cursor] === '.') {
      cursor += 1;
      if (!/[0-9]/.test(text[cursor] ?? '')) fail(field, 'invalid JSON fraction');
      while (/[0-9]/.test(text[cursor] ?? '')) cursor += 1;
    }
    if (text[cursor] === 'e' || text[cursor] === 'E') {
      cursor += 1;
      if (text[cursor] === '+' || text[cursor] === '-') cursor += 1;
      if (!/[0-9]/.test(text[cursor] ?? '')) fail(field, 'invalid JSON exponent');
      while (/[0-9]/.test(text[cursor] ?? '')) cursor += 1;
    }
    if (cursor - start > DIRECT_HOT_ROUTE_MANIFEST_MAX_NUMBER_BYTES_V3) {
      fail(field, `number exceeds ${DIRECT_HOT_ROUTE_MANIFEST_MAX_NUMBER_BYTES_V3} bytes`);
    }
  };
  const value = (depth: number): void => {
    values += 1;
    if (values > DIRECT_HOT_ROUTE_MANIFEST_MAX_VALUES_V3) {
      fail(field, `tree exceeds ${DIRECT_HOT_ROUTE_MANIFEST_MAX_VALUES_V3} values`);
    }
    if (depth > DIRECT_HOT_ROUTE_MANIFEST_MAX_DEPTH_V3) {
      fail(field, `nesting exceeds ${DIRECT_HOT_ROUTE_MANIFEST_MAX_DEPTH_V3}`);
    }
    whitespace();
    const character = text[cursor];
    if (character === '{') {
      cursor += 1;
      whitespace();
      const keys = new Set<string>();
      let fields = 0;
      if (text[cursor] === '}') {
        cursor += 1;
        return;
      }
      for (;;) {
        const key = string();
        fields += 1;
        if (fields > DIRECT_HOT_ROUTE_MANIFEST_MAX_OBJECT_FIELDS_V3) {
          fail(field, `object exceeds ${DIRECT_HOT_ROUTE_MANIFEST_MAX_OBJECT_FIELDS_V3} fields`);
        }
        if (keys.has(key)) fail(field, `duplicate object key ${JSON.stringify(key)}`);
        keys.add(key);
        whitespace();
        if (text[cursor] !== ':') fail(field, 'object key has no colon');
        cursor += 1;
        value(depth + 1);
        whitespace();
        if (text[cursor] === '}') {
          cursor += 1;
          return;
        }
        if (text[cursor] !== ',') fail(field, 'object has no comma or closing brace');
        cursor += 1;
        whitespace();
      }
    }
    if (character === '[') {
      cursor += 1;
      whitespace();
      let entries = 0;
      if (text[cursor] === ']') {
        cursor += 1;
        return;
      }
      for (;;) {
        entries += 1;
        if (entries > DIRECT_HOT_ROUTE_MANIFEST_MAX_ARRAY_V3) {
          fail(field, `array exceeds ${DIRECT_HOT_ROUTE_MANIFEST_MAX_ARRAY_V3} entries`);
        }
        value(depth + 1);
        whitespace();
        if (text[cursor] === ']') {
          cursor += 1;
          return;
        }
        if (text[cursor] !== ',') fail(field, 'array has no comma or closing bracket');
        cursor += 1;
        whitespace();
      }
    }
    if (character === '"') {
      string();
      return;
    }
    for (const literal of ['true', 'false', 'null']) {
      if (text.startsWith(literal, cursor)) {
        cursor += literal.length;
        return;
      }
    }
    if (character === '-' || /[0-9]/.test(character ?? '')) {
      number();
      return;
    }
    fail(field, 'invalid JSON value');
  };

  value(0);
  whitespace();
  if (cursor !== text.length) fail(field, 'trailing data');
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return fail(field, 'ordinary value decoder refused');
  }
}

function object(value: unknown, field: string): Record<string, unknown> {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${field} must be one object`);
  return value as Record<string, unknown>;
}

function exactKeys(value: Record<string, unknown>, fields: ReadonlyArray<string>, field: string): void {
  const observed = Object.keys(value).sort();
  const expected = [...fields].sort();
  if (observed.length !== expected.length || observed.some((entry, index) => entry !== expected[index])) {
    throw new Error(`${field} has missing or unknown fields`);
  }
}

function array(value: unknown, field: string, minimum: number, maximum = minimum): unknown[] {
  if (!Array.isArray(value) || value.length < minimum || value.length > maximum) {
    throw new Error(`${field} must contain exactly ${minimum === maximum ? minimum : `${minimum}..${maximum}`} entries`);
  }
  return value;
}

function address(value: unknown, field: string): string {
  if (typeof value !== 'string') throw new Error(`${field} is not one canonical Solana address`);
  let parsed: PublicKey;
  try {
    parsed = new PublicKey(value);
  } catch {
    throw new Error(`${field} is not one canonical Solana address`);
  }
  if (parsed.toBase58() !== value) throw new Error(`${field} is not one canonical Solana address`);
  return value;
}

function identity(value: unknown, field: string): string {
  if (typeof value !== 'string' || !/^[0-9a-f]{64}$/.test(value) || /^0{64}$/.test(value)) {
    throw new Error(`${field} is not one nonzero lowercase 32-byte hex identity`);
  }
  return value;
}

function decimalU64(value: unknown, field: string): bigint {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]{0,19})$/.test(value)) {
    throw new Error(`${field} must be canonical decimal u64 text`);
  }
  const decoded = BigInt(value);
  if (decoded <= 0n || decoded > 0xffff_ffff_ffff_ffffn) throw new Error(`${field} is outside positive u64`);
  return decoded;
}

function coordinate(value: unknown, field: string): DirectHotRouteCoordinateV3 {
  const row = object(value, field);
  exactKeys(row, ['address', 'isSigner', 'isWritable'], field);
  if (typeof row.isSigner !== 'boolean' || typeof row.isWritable !== 'boolean') {
    throw new Error(`${field} signer and writable privileges must be explicit booleans`);
  }
  return Object.freeze({
    address: address(row.address, `${field} address`),
    isSigner: row.isSigner,
    isWritable: row.isWritable,
  });
}

function fixedCoordinate(value: unknown, index: number): DirectHotRouteCoordinateV3 {
  const field = `fixed account ${index}`;
  const row = object(value, field);
  exactKeys(row, ['role', 'address', 'isSigner', 'isWritable'], field);
  const expectedRole = DIRECT_HOT_FIXED_ROLE_LABELS_V3[index];
  if (row.role !== expectedRole) throw new Error(`${field} role must be exactly ${expectedRole}`);
  if (typeof row.isSigner !== 'boolean' || typeof row.isWritable !== 'boolean') {
    throw new Error(`${field} signer and writable privileges must be explicit booleans`);
  }
  const expectedWritable = index === HOT_ROOT_ACCOUNT_V3;
  if (row.isSigner || row.isWritable !== expectedWritable) {
    throw new Error(`${field} has noncanonical signer or writable privilege`);
  }
  return Object.freeze({
    address: address(row.address, `${field} address`),
    isSigner: row.isSigner,
    isWritable: row.isWritable,
  });
}

const BASE64_ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

function decodeBase64(value: unknown, field: string, exactBytes: number): Uint8Array {
  if (typeof value !== 'string' || value.length === 0 || value.length % 4 !== 0
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error(`${field} is not canonical base64 text`);
  }
  const output: number[] = [];
  for (let offset = 0; offset < value.length; offset += 4) {
    const a = BASE64_ALPHABET.indexOf(value[offset] ?? '');
    const b = BASE64_ALPHABET.indexOf(value[offset + 1] ?? '');
    const cText = value[offset + 2] ?? '';
    const dText = value[offset + 3] ?? '';
    const c = cText === '=' ? 0 : BASE64_ALPHABET.indexOf(cText);
    const d = dText === '=' ? 0 : BASE64_ALPHABET.indexOf(dText);
    if (a < 0 || b < 0 || c < 0 || d < 0) throw new Error(`${field} is not canonical base64 text`);
    output.push((a << 2) | (b >> 4));
    if (cText !== '=') output.push(((b & 15) << 4) | (c >> 2));
    if (dText !== '=') output.push(((c & 3) << 6) | d);
  }
  const bytes = Uint8Array.from(output);
  if (bytes.length !== exactBytes) throw new Error(`${field} must decode to exactly ${exactBytes} bytes`);
  return bytes;
}

function requireCanonicalNamedShape(
  payer: string,
  fixed: ReadonlyArray<DirectHotRouteCoordinateV3>,
  runtime: ReadonlyArray<DirectHotRouteCoordinateV3>,
  lookups: ReadonlyArray<string>,
): void {
  if (runtime.length !== DIRECT_INLINE_CURRENT_RUNTIME_TAIL_ACCOUNTS_V3
      || new Set(runtime.map((entry) => entry.address)).size !== runtime.length) {
    throw new Error(`runtimeAccounts must be the duplicate-free current ${DIRECT_INLINE_CURRENT_RUNTIME_TAIL_ACCOUNTS_V3}-account physical tail`);
  }
  if (new Set(fixed.map((entry) => entry.address)).size !== fixed.length) {
    throw new Error('fixedAccounts must retain the 39 distinct named pre-seal roles');
  }
  const payerMeta = runtime[1];
  if (payerMeta?.address !== payer || payerMeta.isSigner !== true || payerMeta.isWritable !== true
      || runtime.some((entry, index) => index !== 1 && entry.address === payer)) {
    throw new Error('route payer must be runtimeAccounts[1], writable, and the sole runtime signer alias');
  }
  if (fixed.some((entry) => entry.address === payer)) throw new Error('route payer aliases a fixed Hot role');
  const expectedCrossAliases = new Map<number, number>(DIRECT_INLINE_NAMED_RUNTIME_FIXED_ALIASES_V3);
  for (const [runtimeIndex, entry] of runtime.entries()) {
    const fixedIndex = fixed.findIndex((candidate) => candidate.address === entry.address);
    const expected = expectedCrossAliases.get(runtimeIndex);
    if ((expected === undefined ? fixedIndex !== -1 : fixedIndex !== expected)
        || entry.isSigner !== (runtimeIndex === 1)
        || entry.isWritable !== DIRECT_INLINE_RUNTIME_TAIL_WRITABLE_V3.includes(runtimeIndex as never)) {
      throw new Error(`runtime account ${runtimeIndex} has a noncanonical named fixed-account join`);
    }
  }
  const allInstruction = [...fixed, ...runtime];
  for (const [index, lookup] of lookups.entries()) {
    if (lookup === payer || allInstruction.some((entry) => entry.address === lookup)) {
      throw new Error(`lookup table ${index} aliases the payer or one Hot instruction account`);
    }
  }
}

async function parseUntrustedManifestV3(source: string | Uint8Array): Promise<DirectHotRouteManifestV3> {
  const input = object(exactBoundedJson(source, 'Direct Hot route manifest'), 'Direct Hot route manifest');
  exactKeys(input, [
    'format',
    'payer',
    'fixedAccounts',
    'strategyAccounts',
    'runtimeAccounts',
    'lookupTables',
    'lookupTableCreationSlot',
    'checkedInfrastructure',
    'checkedInfrastructureSha256',
  ], 'Direct Hot route manifest');
  if (input.format !== DIRECT_HOT_ROUTE_MANIFEST_FORMAT_V3) {
    throw new Error(`Direct Hot route manifest format must be ${DIRECT_HOT_ROUTE_MANIFEST_FORMAT_V3}`);
  }
  const payer = address(input.payer, 'route payer');
  const fixedAccounts = Object.freeze(array(input.fixedAccounts, 'fixedAccounts', HOT_FIXED_ACCOUNT_COUNT_V3)
    .map((entry, index) => fixedCoordinate(entry, index)));
  array(input.strategyAccounts, 'strategyAccounts', 0);
  const runtimeAccounts = Object.freeze(array(input.runtimeAccounts, 'runtimeAccounts', DIRECT_INLINE_CURRENT_RUNTIME_TAIL_ACCOUNTS_V3)
    .map((entry, index) => coordinate(entry, `runtime account ${index}`)));
  const lookupTables = Object.freeze(array(input.lookupTables, 'lookupTables', 1)
    .map((entry, index) => address(entry, `lookup table ${index}`)));
  const lookupTableCreationSlot = decimalU64(input.lookupTableCreationSlot, 'lookup table creation slot');
  requireCanonicalNamedShape(payer, fixedAccounts, runtimeAccounts, lookupTables);
  const checkedInfrastructure = decodeBase64(
    input.checkedInfrastructure,
    'checked infrastructure',
    CHECKED_INFRASTRUCTURE_BYTES_V1,
  );
  const expectedCheckedDigest = identity(input.checkedInfrastructureSha256, 'checked infrastructure digest');
  const observedCheckedDigest = Array.from(await sha256(checkedInfrastructure), (byte) => byte.toString(16).padStart(2, '0')).join('');
  if (observedCheckedDigest !== expectedCheckedDigest) {
    throw new Error('checked infrastructure bytes differ from their exact manifest digest');
  }
  return Object.freeze({
    payer,
    fixedAccounts,
    strategyAccounts: Object.freeze([]),
    runtimeAccounts,
    lookupTables,
    lookupTableCreationSlot,
    checkedInfrastructure,
  });
}

/**
 * Parse an untrusted public document and return only an authenticated route.
 *
 * Deserialization is intentionally private. The public boundary returns
 * nothing until the existing chain authenticator has reacquired every account,
 * joined every release and record, authenticated the exact frozen lookup table,
 * and recognized the checked outer deployment evidence. This function never
 * signs, submits, chooses a fee, or turns transport JSON into authority.
 */
export async function inspectDirectHotRouteManifestJsonV3(
  client: DirectHotRouteReaderV3,
  source: string | Uint8Array,
): Promise<DirectHotRouteInspectionV3> {
  const manifest = await parseUntrustedManifestV3(source);
  const inspection = await inspectDirectHotRouteV3(client, manifest);
  if (inspection.checkedOuter.status !== 'checked' || inspection.route.outerEvidence.status !== 'checked') {
    throw new Error('Direct Hot route manifest has no recognized checked outer deployment evidence');
  }
  return inspection;
}
