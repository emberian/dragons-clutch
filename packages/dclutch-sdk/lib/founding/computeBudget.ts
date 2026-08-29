/**
 * The ComputeBudget declarations a founding transaction cannot go without.
 *
 * Solana's default is 200,000 compute units per transaction. Found37 and the
 * DCLTGMF3 outer reauthenticate release and record state under an explicitly
 * configured limit. A current performance claim for either route requires its
 * pass count and 20-seed mean; this module only reproduces the configured
 * declaration. Omitting it produces a transaction the runtime kills with
 * `Program failed to complete`, which reads like a program bug and is not one.
 *
 * The browser's first Found builder shipped without these,
 * and the first time anyone submitted its output — from the create wizard,
 * against a local validator — that is exactly the error it got.
 *
 * The two discriminants and both magnitudes are emitted from the reference
 * client's own `bounded_instructions`, which owns them there and refuses a
 * caller-supplied duplicate. This module keeps that rule: it refuses to build
 * a declaration list for instructions that already carry one.
 */

import { PublicKey, TransactionInstruction } from '@solana/web3.js';

import {
  FOUNDING_HEAP_FRAME_BYTES_V1,
  LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT_V1,
  REQUEST_HEAP_FRAME_DISCRIMINANT_V1,
  SET_COMPUTE_UNIT_LIMIT_DISCRIMINANT_V1,
} from '../generated/genericFoundingV1';

export const COMPUTE_BUDGET_PROGRAM_ID = 'ComputeBudget111111111111111111111111111111';

/** Signature precompiles carry instruction indices and cannot be prepended past. */
const PRECOMPILE_PROGRAM_IDS: ReadonlyArray<string> = Object.freeze([
  'Ed25519SigVerify111111111111111111111111111',
  'KeccakSecp256k11111111111111111111111111111',
  'Secp256r1SigVerify1111111111111111111111111',
]);

function u32Data(discriminant: number, value: number): Uint8Array {
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) throw new Error('ComputeBudget argument is outside u32');
  const data = new Uint8Array(5);
  data[0] = discriminant;
  new DataView(data.buffer).setUint32(1, value, true);
  return data;
}

/** `SetComputeUnitLimit(units)`. */
export function setComputeUnitLimitV1(units: number = LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT_V1): TransactionInstruction {
  return new TransactionInstruction({
    programId: new PublicKey(COMPUTE_BUDGET_PROGRAM_ID),
    keys: [],
    data: u32Data(SET_COMPUTE_UNIT_LIMIT_DISCRIMINANT_V1, units) as Buffer,
  });
}

/**
 * `RequestHeapFrame(bytes)`.
 *
 * Only the two routes on `declares_extended_heap_profile_v1` — DCLTGMF3 and
 * DCLTPCB2 — should carry this, and both present the instructions sysvar so
 * Trading's adapter can re-derive the grant from what the runtime serialized
 * rather than from a caller's claim. On any other route it costs compute and
 * changes nothing, which is why it is opt-in here rather than always applied.
 */
export function requestHeapFrameV1(bytes: number = FOUNDING_HEAP_FRAME_BYTES_V1): TransactionInstruction {
  if (bytes < 32 * 1024 || bytes > 256 * 1024 || bytes % 1024 !== 0) {
    throw new Error('heap frame must be a whole number of KiB in [32 KiB, 256 KiB]');
  }
  return new TransactionInstruction({
    programId: new PublicKey(COMPUTE_BUDGET_PROGRAM_ID),
    keys: [],
    data: u32Data(REQUEST_HEAP_FRAME_DISCRIMINANT_V1, bytes) as Buffer,
  });
}

/**
 * Prepend the declarations this transaction needs, refusing where prepending
 * would change what an instruction means.
 *
 * A signature precompile carries the *instruction index* of the instruction
 * whose data it verifies inside its own payload, so inserting anything ahead of
 * one silently re-points it. That is a defect to fix at the call site, never
 * something to prepend past.
 */
export function boundedInstructionsV1(
  instructions: ReadonlyArray<TransactionInstruction>,
  options: Readonly<{ computeUnitLimit?: number; heapFrameBytes?: number }> = {},
): ReadonlyArray<TransactionInstruction> {
  for (const instruction of instructions) {
    const programId = instruction.programId.toBase58();
    if (PRECOMPILE_PROGRAM_IDS.includes(programId)) {
      throw new Error('a signature precompile carries instruction indices in its payload and cannot be prepended past');
    }
    if (programId === COMPUTE_BUDGET_PROGRAM_ID) {
      throw new Error('the ComputeBudget declarations are owned by this builder; a duplicate is a transaction error');
    }
  }
  const declarations = [setComputeUnitLimitV1(options.computeUnitLimit)];
  if (options.heapFrameBytes !== undefined) declarations.push(requestHeapFrameV1(options.heapFrameBytes));
  return Object.freeze([...declarations, ...instructions]);
}
