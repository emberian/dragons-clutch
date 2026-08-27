import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { decodeBase58 } from './base58';

/**
 * `PublicKey` is the oracle: it encodes 32 bytes to base58 with the same
 * alphabet the RPC uses, so a round trip through it checks this decoder against
 * an implementation nobody here wrote.
 */
function roundTrip(bytes: Uint8Array): Uint8Array {
  return decodeBase58(new PublicKey(bytes).toBase58());
}

describe('base58 decoding', () => {
  it('round-trips every byte value through the reference encoder', () => {
    const bytes = Uint8Array.from({ length: 32 }, (_unused, index) => (index * 8 + 3) % 256);
    expect([...roundTrip(bytes)]).toEqual([...bytes]);
  });

  it('preserves leading zero bytes, which positional decoding alone loses', () => {
    const bytes = new Uint8Array(32);
    bytes[31] = 1;
    const decoded = roundTrip(bytes);
    expect(decoded.length).toBe(32);
    expect([...decoded]).toEqual([...bytes]);
  });

  it('decodes the all-zero key as thirty-two zero bytes', () => {
    const decoded = roundTrip(new Uint8Array(32));
    expect(decoded.length).toBe(32);
    expect(decoded.every((byte) => byte === 0)).toBe(true);
  });

  it('decodes the all-ones key', () => {
    const bytes = new Uint8Array(32).fill(0xff);
    expect([...roundTrip(bytes)]).toEqual([...bytes]);
  });

  it('handles arbitrary lengths, which instruction data has and a public key does not', () => {
    // 'JxF12TrwUP45BMd' is the canonical base58 of the ASCII text below in the
    // Bitcoin test vectors; checking one non-32-byte value keeps the decoder
    // honest about lengths it will actually see.
    expect(new TextDecoder().decode(decodeBase58('JxF12TrwUP45BMd'))).toBe('Hello World');
    expect(decodeBase58('')).toEqual(new Uint8Array(0));
    expect([...decodeBase58('1')]).toEqual([0]);
    expect([...decodeBase58('111')]).toEqual([0, 0, 0]);
  });

  it('refuses a character outside the alphabet instead of skipping it', () => {
    // Skipping would produce plausible bytes for data that was never sent.
    expect(() => decodeBase58('abc0def')).toThrow(/outside the alphabet/);
    expect(() => decodeBase58('abcOdef')).toThrow(/outside the alphabet/);
    expect(() => decodeBase58('abc def')).toThrow(/outside the alphabet/);
  });
});
