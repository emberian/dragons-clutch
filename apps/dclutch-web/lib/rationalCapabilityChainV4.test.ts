import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { authenticateRationalRepresentationGraphV2 } from './rationalCapabilityChainV4';
import { decodeRationalRepresentationDescriptorV3 } from './rationalRetireReceiptV4';

function bytes(value: number): Uint8Array { return new Uint8Array(32).fill(value); }
function putU16(output: Uint8Array, offset: number, value: number): void { new DataView(output.buffer).setUint16(offset, value, true); }
function putU32(output: Uint8Array, offset: number, value: number): void { new DataView(output.buffer).setUint32(offset, value, true); }
function putU64(output: Uint8Array, offset: number, value: bigint): void { new DataView(output.buffer).setBigUint64(offset, value, true); }

function descriptor() {
  const output = new Uint8Array(256 + 3 * 8); output.set(new TextEncoder().encode('DCRRDSC3')); putU16(output, 8, 3);
  [11, 12, 13, 14, 15, 16, 17].forEach((value, index) => output.set(bytes(value), 16 + index * 32));
  putU32(output, 240, 3); putU64(output, 248, 10n); [2n, 0n, 1n].forEach((value, index) => putU64(output, 256 + index * 8, value));
  return decodeRationalRepresentationDescriptorV3(output, bytes(20));
}

function graph(): Uint8Array {
  const output = new Uint8Array(104 + 64 + 3 * 8); output.set(new TextEncoder().encode('DCRRGRP2')); putU16(output, 8, 2);
  output.set(bytes(11), 16); output.set(bytes(13), 48); putU32(output, 80, 3); putU32(output, 84, 1); putU32(output, 88, 0); putU64(output, 96, 10n);
  output.set(bytes(13), 104); [2n, 0n, 1n].forEach((value, index) => putU64(output, 168 + index * 8, value));
  return output;
}

describe('common Rational CapabilityV4 chain joins', () => {
  it('checks every graph-root payoff, including descriptor zero coordinates', () => {
    const exact = graph();
    expect(() => authenticateRationalRepresentationGraphV2(exact, descriptor())).not.toThrow();
    const substituted = exact.slice(); putU64(substituted, 176, 1n);
    expect(() => authenticateRationalRepresentationGraphV2(substituted, descriptor())).toThrow(/outcome 1/);
  });

  it('refuses a repeated or missing selected graph root identity', () => {
    const missing = graph(); missing.set(new PublicKey(bytes(30)).toBytes(), 104);
    expect(() => authenticateRationalRepresentationGraphV2(missing, descriptor())).toThrow(/omits its selected root/);
  });
});
