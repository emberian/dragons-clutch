import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { hex } from './bytes';
import {
  decodeCompactIntentV1,
  decodeControllerInstructionV1,
  decodeMarketProfileV1,
  encodeCompactIntentV1,
  encodeControllerInstructionV1,
  encodeMarketProfileV1,
} from './directCodec';

const vectors = Object.fromEntries(
  readFileSync(new URL('../../../formal/dclutch-semantics/vectors/direct-controller-v1.txt', import.meta.url), 'utf8')
    .trim()
    .split('\n')
    .map((line) => {
      const separator = line.indexOf('=');
      if (separator < 1) throw new Error('malformed Lean Direct-controller vector');
      return [line.slice(0, separator), line.slice(separator + 1)];
    }),
);

function bytes(name: string): Uint8Array {
  const value = vectors[name];
  if (value === undefined || value.length % 2 !== 0) throw new Error(`missing ${name} vector`);
  return Uint8Array.from(value.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
}

describe('Lean-owned compiled Direct ABI', () => {
  it('round-trips all exact Lean vectors byte-for-byte', () => {
    const seller = decodeCompactIntentV1(bytes('seller_intent'));
    const buyer = decodeCompactIntentV1(bytes('buyer_intent'));
    const controller = decodeControllerInstructionV1(bytes('controller'));
    const profile = decodeMarketProfileV1(bytes('market_profile'));

    expect(hex(encodeCompactIntentV1(seller))).toBe(vectors.seller_intent);
    expect(hex(encodeCompactIntentV1(buyer))).toBe(vectors.buyer_intent);
    expect(hex(encodeControllerInstructionV1(controller))).toBe(vectors.controller);
    expect(hex(encodeMarketProfileV1(profile))).toBe(vectors.market_profile);
    expect(controller.seller).toEqual(seller);
    expect(controller.buyer).toEqual(buyer);
  });

  it('refuses hostile width, magic, version, and reserved bytes', () => {
    const canonical = bytes('seller_intent');
    expect(() => decodeCompactIntentV1(canonical.slice(0, -1))).toThrow(/exactly 136/);

    const magic = canonical.slice();
    magic[0] ^= 1;
    expect(() => decodeCompactIntentV1(magic)).toThrow(/domain/);

    const version = canonical.slice();
    version[8] = 2;
    expect(() => decodeCompactIntentV1(version)).toThrow(/version/);

    const reserved = canonical.slice();
    reserved[13] = 1;
    expect(() => decodeCompactIntentV1(reserved)).toThrow(/reserved/);
  });

  it('refuses non-32-byte keys and out-of-range exact integers', () => {
    const intent = decodeCompactIntentV1(bytes('seller_intent'));
    expect(() => encodeCompactIntentV1({ ...intent, executionProfile: new Uint8Array(31) })).toThrow(/32 bytes/);
    expect(() => encodeCompactIntentV1({ ...intent, feeBasisPoints: 65_536 })).toThrow(/u16/);
    expect(() => encodeCompactIntentV1({ ...intent, nonce: -1n })).toThrow(/u64/);
  });
});
