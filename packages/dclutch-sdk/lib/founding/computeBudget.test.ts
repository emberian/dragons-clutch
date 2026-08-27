import { Keypair, PublicKey, TransactionInstruction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  FOUNDING_HEAP_FRAME_BYTES_V1,
  LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT_V1,
  REQUEST_HEAP_FRAME_DISCRIMINANT_V1,
  SET_COMPUTE_UNIT_LIMIT_DISCRIMINANT_V1,
} from '../generated/genericFoundingV1';
import {
  COMPUTE_BUDGET_PROGRAM_ID,
  boundedInstructionsV1,
  requestHeapFrameV1,
  setComputeUnitLimitV1,
} from './computeBudget';

function ordinary(): TransactionInstruction {
  return new TransactionInstruction({ programId: Keypair.generate().publicKey, keys: [], data: Buffer.alloc(0) });
}

function decode(instruction: TransactionInstruction): Readonly<{ discriminant: number; value: number }> {
  const data = Uint8Array.from(instruction.data);
  expect(data.length).toBe(5);
  return { discriminant: data[0], value: new DataView(data.buffer, data.byteOffset + 1, 4).getUint32(0, true) };
}

describe('the ComputeBudget declarations', () => {
  it('encodes the limit as the discriminant and little-endian u32 the runtime reads', () => {
    const instruction = setComputeUnitLimitV1();
    expect(instruction.programId.toBase58()).toBe(COMPUTE_BUDGET_PROGRAM_ID);
    expect(instruction.keys).toHaveLength(0);
    expect(decode(instruction)).toEqual({ discriminant: SET_COMPUTE_UNIT_LIMIT_DISCRIMINANT_V1, value: LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT_V1 });
  });

  it('defaults to a limit that actually covers a founding', () => {
    // Found31 spends over a million CU authenticating whole program ELFs and
    // DCLTGMF1 measured 1,209,776. The default 200,000 is not slow for these
    // routes, it is fatal.
    expect(LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT_V1).toBeGreaterThan(1_209_776);
    expect(LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT_V1).toBeLessThanOrEqual(1_400_000);
  });

  it('encodes the heap frame at the profile the two founding routes declare', () => {
    expect(decode(requestHeapFrameV1())).toEqual({ discriminant: REQUEST_HEAP_FRAME_DISCRIMINANT_V1, value: FOUNDING_HEAP_FRAME_BYTES_V1 });
    expect(FOUNDING_HEAP_FRAME_BYTES_V1).toBe(256 * 1024);
  });

  it('refuses a heap frame outside the bounds agave itself enforces', () => {
    // `sanitize_requested_heap_size`: a whole number of KiB in [32, 256].
    expect(() => requestHeapFrameV1(16 * 1024)).toThrow(/32 KiB, 256 KiB/);
    expect(() => requestHeapFrameV1(512 * 1024)).toThrow(/32 KiB, 256 KiB/);
    expect(() => requestHeapFrameV1(64 * 1024 + 1)).toThrow(/whole number of KiB/);
    expect(() => requestHeapFrameV1(64 * 1024)).not.toThrow();
  });

  it('refuses an argument outside u32', () => {
    expect(() => setComputeUnitLimitV1(2 ** 32)).toThrow(/outside u32/);
    expect(() => setComputeUnitLimitV1(-1)).toThrow(/outside u32/);
  });
});

describe('prepending declarations to a transaction', () => {
  it('puts the limit first and keeps the caller’s order after it', () => {
    const [first, second] = [ordinary(), ordinary()];
    const bounded = boundedInstructionsV1([first, second]);
    expect(bounded).toHaveLength(3);
    expect(bounded[0].programId.toBase58()).toBe(COMPUTE_BUDGET_PROGRAM_ID);
    expect(bounded[1]).toBe(first);
    expect(bounded[2]).toBe(second);
  });

  it('adds the heap frame only when asked, and after the limit', () => {
    expect(boundedInstructionsV1([ordinary()])).toHaveLength(2);
    const withHeap = boundedInstructionsV1([ordinary()], { heapFrameBytes: FOUNDING_HEAP_FRAME_BYTES_V1 });
    expect(withHeap).toHaveLength(3);
    expect(decode(withHeap[0]).discriminant).toBe(SET_COMPUTE_UNIT_LIMIT_DISCRIMINANT_V1);
    expect(decode(withHeap[1]).discriminant).toBe(REQUEST_HEAP_FRAME_DISCRIMINANT_V1);
  });

  it('refuses to prepend past a signature precompile', () => {
    // A precompile carries the instruction INDEX of the instruction whose data
    // it verifies, inside its own payload. Prepending silently re-points it at
    // a different instruction, which is a defect to fix at the call site.
    for (const program of [
      'Ed25519SigVerify111111111111111111111111111',
      'KeccakSecp256k11111111111111111111111111111',
      'Secp256r1SigVerify1111111111111111111111111',
    ]) {
      expect(() => boundedInstructionsV1([new TransactionInstruction({
        programId: new PublicKey(program), keys: [], data: Buffer.alloc(0),
      })])).toThrow(/cannot be prepended past/);
    }
  });

  it('refuses a caller-supplied duplicate declaration', () => {
    expect(() => boundedInstructionsV1([setComputeUnitLimitV1(), ordinary()])).toThrow(/owned by this builder/);
  });
});
