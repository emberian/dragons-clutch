/** Maximum byte width Solana permits one program to place in return data. */
export const SOLANA_RETURN_DATA_MAX_BYTES_V1 = 1_024;

/** Exact program return data preserved from one finalized transaction. */
export type TransactionReturnDataObservationV1 = Readonly<{
  /** Canonical base58 program which produced the final return-data value. */
  programId: string;
  /** Exact bytes decoded from the RPC's canonical base64 tuple. */
  data: Uint8Array;
}>;

const BASE58_ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
const MAX_BASE64_CHARACTERS = Math.ceil(SOLANA_RETURN_DATA_MAX_BYTES_V1 / 3) * 4;

function plain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function exactProgramId(value: unknown): string {
  if (typeof value !== 'string' || value.length < 32 || value.length > 44) {
    throw new Error('transaction return-data producer is not one canonical Pubkey');
  }
  let numeric = 0n;
  for (const character of value) {
    const digit = BASE58_ALPHABET.indexOf(character);
    if (digit < 0) throw new Error('transaction return-data producer is not one canonical Pubkey');
    numeric = numeric * 58n + BigInt(digit);
  }
  const significant: number[] = [];
  while (numeric > 0n) {
    significant.push(Number(numeric & 0xffn));
    numeric >>= 8n;
  }
  significant.reverse();
  let leadingZeroes = 0;
  while (leadingZeroes < value.length && value[leadingZeroes] === '1') leadingZeroes += 1;
  if (leadingZeroes + significant.length !== 32) {
    throw new Error('transaction return-data producer is not one canonical Pubkey');
  }
  const bytes = new Uint8Array(32);
  bytes.set(significant, leadingZeroes);
  if (encodeBase58(bytes) !== value) {
    throw new Error('transaction return-data producer is not one canonical Pubkey');
  }
  return value;
}

function encodeBase58(bytes: Uint8Array): string {
  let leadingZeroes = 0;
  while (leadingZeroes < bytes.length && bytes[leadingZeroes] === 0) leadingZeroes += 1;
  let numeric = 0n;
  for (const byte of bytes) numeric = (numeric << 8n) + BigInt(byte);
  let encoded = '';
  while (numeric > 0n) {
    encoded = BASE58_ALPHABET[Number(numeric % 58n)] + encoded;
    numeric /= 58n;
  }
  return `${'1'.repeat(leadingZeroes)}${encoded}`;
}

function exactBase64Bytes(value: unknown): Uint8Array {
  if (typeof value !== 'string' || value.length > MAX_BASE64_CHARACTERS
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error('transaction return data is not canonical bounded base64');
  }
  try {
    const decoded = atob(value);
    if (decoded.length > SOLANA_RETURN_DATA_MAX_BYTES_V1 || btoa(decoded) !== value) {
      throw new Error('noncanonical');
    }
    return Uint8Array.from(decoded, (character) => character.charCodeAt(0));
  } catch {
    throw new Error('transaction return data is not canonical bounded base64');
  }
}

/**
 * Hostile-decode Solana's `meta.returnData` boundary.
 *
 * Absence is one explicit `null`. A present value must be exactly
 * `{ programId, data: [base64, "base64"] }`; unknown fields, encodings,
 * noncanonical Pubkeys, noncanonical base64, and oversized bytes are refused.
 */
export function decodeTransactionReturnDataV1(value: unknown): TransactionReturnDataObservationV1 | null {
  if (value === undefined || value === null) return null;
  if (!plain(value) || Object.keys(value).sort().join(',') !== 'data,programId') {
    throw new Error('transaction returnData is not the exact Solana RPC object');
  }
  if (!Array.isArray(value.data) || value.data.length !== 2 || value.data[1] !== 'base64') {
    throw new Error('transaction returnData does not carry one exact base64 tuple');
  }
  return Object.freeze({
    programId: exactProgramId(value.programId),
    data: exactBase64Bytes(value.data[0]),
  });
}
