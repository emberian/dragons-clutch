import { PublicKey } from '@solana/web3.js';

export const ZERO_32 = new Uint8Array(32);

export function ascii(bytes: Uint8Array, offset: number, width: number): string {
  return new TextDecoder('ascii', { fatal: true }).decode(slice(bytes, offset, width));
}

export function slice(bytes: Uint8Array, offset: number, width: number): Uint8Array {
  const end = offset + width;
  if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(width) || offset < 0 || width < 0 || end > bytes.length) {
    throw new Error('byte range is outside the canonical account width');
  }
  return bytes.slice(offset, end);
}

export function u16(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 2).getUint16(0, true);
}

export function u64(bytes: Uint8Array, offset: number): bigint {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 8).getBigUint64(0, true);
}

export function isZero(bytes: Uint8Array): boolean {
  return bytes.every((byte) => byte === 0);
}

export function requireZero(bytes: Uint8Array, offset: number, width: number, field: string): void {
  if (!isZero(slice(bytes, offset, width))) throw new Error(`${field} contains noncanonical reserved bytes`);
}

export function requireNonzero(bytes: Uint8Array, field: string): void {
  if (isZero(bytes)) throw new Error(`${field} is the reserved all-zero identity`);
}

export function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

export function fromHex(value: string, field: string): Uint8Array {
  if (!/^[0-9a-f]{64}$/.test(value)) throw new Error(`${field} must be exactly 32 lowercase hexadecimal bytes`);
  return Uint8Array.from(value.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));
}

export function pubkey(bytes: Uint8Array, field: string): string {
  if (bytes.length !== 32) throw new Error(`${field} is not 32 bytes`);
  requireNonzero(bytes, field);
  return new PublicKey(bytes).toBase58();
}

export function decodeBase64(value: unknown, field: string): Uint8Array {
  if (!Array.isArray(value) || value.length !== 2 || typeof value[0] !== 'string' || value[1] !== 'base64') {
    throw new Error(`${field} is not canonical base64 RPC account data`);
  }
  try {
    const decoded = atob(value[0]);
    return Uint8Array.from(decoded, (character) => character.charCodeAt(0));
  } catch {
    throw new Error(`${field} is not valid base64`);
  }
}

export async function sha256(bytes: Uint8Array): Promise<Uint8Array> {
  const input = new Uint8Array(bytes);
  return new Uint8Array(await crypto.subtle.digest('SHA-256', input));
}
