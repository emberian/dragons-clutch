import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  buildFailureWalkTransactionV1,
  encodeCommitDeadlineFailureV1,
  type FailureWalkBookV1,
} from './failureWalk';
import { COMMIT_DEADLINE_FAILURE_FRAME_V1 } from './generated/relayTransportV1';

function key(byte: number): string {
  return new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
}

const BOOK: FailureWalkBookV1 = Object.freeze({
  resolutionProgram: key(1),
  market: key(2),
  coreProgram: key(3),
  registryActivation: key(4),
  sourceResolutionState: key(5),
  resolutionCertificate: key(6),
  sourceMaterial: key(7),
  sourceMaterialStagingVacancy: key(8),
  windowSpec: key(9),
  windowSpecStagingVacancy: key(10),
  productRecord: key(11),
  productRecordStagingVacancy: key(12),
  resultDomain: key(13),
  resultDomainStagingVacancy: key(14),
  portfolioRecord: key(15),
  portfolioRecordStagingVacancy: key(16),
  capabilityManifest: key(17),
  capabilityManifestStagingVacancy: key(18),
  resolutionFunding: key(19),
});

describe('CommitDeadlineFailure wire', () => {
  it('encodes the exact 32-byte layout: magic, schema, action 6, generation, terminal sequence', () => {
    const bytes = encodeCommitDeadlineFailureV1(7n, 1n);
    expect(bytes.length).toBe(32);
    expect(new TextDecoder().decode(bytes.slice(0, 8))).toBe('DCLTRIX1');
    const view = new DataView(bytes.buffer);
    expect(view.getUint16(8, true)).toBe(1);
    expect(bytes[10]).toBe(6);
    // Reserved header tail stays zero: the caller has nothing else to say.
    expect([...bytes.slice(11, 16)]).toEqual([0, 0, 0, 0, 0]);
    expect(view.getBigUint64(16, true)).toBe(7n);
    expect(view.getBigUint64(24, true)).toBe(1n);
  });

  it('refuses terminal sequence zero, which names no certificate', () => {
    expect(() => encodeCommitDeadlineFailureV1(1n, 0n)).toThrow(/positive/);
  });
});

describe('the walk transaction', () => {
  it('lays the 22-account frame in the contract order with the contract privileges', () => {
    const worker = key(20);
    const transaction = buildFailureWalkTransactionV1(BOOK, worker, 7n, 1n, new PublicKey(new Uint8Array(32).fill(21)).toBase58());
    const instruction = transaction.instructions[0];
    expect(instruction).toBeDefined();
    if (instruction === undefined) throw new Error('unreachable');
    expect(instruction.keys.length).toBe(COMMIT_DEADLINE_FAILURE_FRAME_V1.length);
    for (const [index, slot] of COMMIT_DEADLINE_FAILURE_FRAME_V1.entries()) {
      const meta = instruction.keys[index];
      expect(meta, `frame slot ${index} (${slot.name})`).toBeDefined();
      if (meta === undefined) throw new Error('unreachable');
      expect(meta.isSigner, `${slot.name} signer`).toBe(slot.signer);
      expect(meta.isWritable, `${slot.name} writable`).toBe(slot.writable);
    }
    // Exactly one signer (the worker) and exactly four writables — the walk
    // pays the worker, terminalizes the source, creates the certificate, and
    // spends the escrow; nothing else moves.
    expect(instruction.keys.filter((meta) => meta.isSigner).length).toBe(1);
    expect(instruction.keys.filter((meta) => meta.isWritable).length).toBe(4);
    expect(instruction.keys[0]?.pubkey.toBase58()).toBe(worker);
  });

  it('fits a legacy packet, the property the route exists to keep', () => {
    const worker = key(20);
    const transaction = buildFailureWalkTransactionV1(BOOK, worker, 7n, 1n, new PublicKey(new Uint8Array(32).fill(21)).toBase58());
    // Serialize unsigned to measure the wire; one 64-byte signature is added
    // by the worker at signing time.
    const wire = transaction.serialize({ requireAllSignatures: false, verifySignatures: false });
    expect(wire.length + 64).toBeLessThanOrEqual(1_232);
  });
});
