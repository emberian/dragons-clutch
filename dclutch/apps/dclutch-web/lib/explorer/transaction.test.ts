import {
  ComputeBudgetProgram,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2 } from '../generated/coreFound';
import { PROTOCOL_REFUSALS, REFUSAL_BANDS } from '../generated/routeCensus';
import type { TransactionMetaObservation } from '../rpc';
import { magicText } from './accountRecords';
import { inspectTransaction, projectTransaction } from './transaction';

const PAYER = new PublicKey(new Uint8Array(32).fill(3));
const RENT_PROGRAM = new PublicKey(new Uint8Array(32).fill(21));
const SIGNATURE = '5'.repeat(88);

/** A two-instruction transaction: one Rent lifecycle call, one System transfer. */
function transactionBytes(): Uint8Array {
  const data = new Uint8Array(128);
  data.set(LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2, 0);
  new DataView(data.buffer).setUint16(8, 2, true);
  const message = new TransactionMessage({
    payerKey: PAYER,
    recentBlockhash: PublicKey.default.toBase58(),
    instructions: [
      new TransactionInstruction({
        programId: RENT_PROGRAM,
        keys: [{ pubkey: PAYER, isSigner: true, isWritable: true }],
        data: Buffer.from(data),
      }),
      SystemProgram.transfer({
        fromPubkey: PAYER,
        toPubkey: new PublicKey(new Uint8Array(32).fill(9)),
        lamports: 5,
      }),
    ],
  }).compileToV0Message();
  return new VersionedTransaction(message).serialize();
}

function meta(overrides: Partial<TransactionMetaObservation> = {}): TransactionMetaObservation {
  const bytes = transactionBytes();
  const addresses = VersionedTransaction.deserialize(bytes).message.staticAccountKeys.map((key) =>
    key.toBase58(),
  );
  return Object.freeze({
    signature: SIGNATURE,
    slot: '4242',
    blockTime: '1790000000',
    succeeded: true,
    errorText: null,
    error: null,
    feeLamports: '5000',
    computeUnits: '18000',
    accountAddresses: Object.freeze(addresses),
    preBalances: Object.freeze(addresses.map(() => '1000000')),
    postBalances: Object.freeze(addresses.map(() => '999000')),
    logMessages: Object.freeze([]),
    innerInstructions: Object.freeze([]),
    returnData: null,
    transactionBytes: bytes,
    ...overrides,
  });
}

describe('the transaction view', () => {
  it('decodes an instruction against the route its magic selects', () => {
    const projected = projectTransaction(meta());
    const rent = projected.instructions[0];
    expect(rent.decoded.magic).toBe(magicText(LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2));
    // The census enumerates three Rent routes behind this one magic; all three
    // are shown, because the magic alone does not choose between them.
    expect(rent.decoded.routes.map((route) => route.routeId).sort()).toEqual([
      'rent/process_close_v2#Close',
      'rent/process_create_v2#Create',
      'rent/process_sweep_v2#Sweep',
    ]);
    expect(rent.decoded.routes.every((route) => route.program === 'rent')).toBe(true);
    expect(rent.decoded.routes[0].summary).toBeTruthy();
    expect(rent.decoded.routes[0].provenance).toMatch(/^programs\/dclutch-rent-sbf\//);
  });

  it('names a program only when the runtime owns it or the reader said so', () => {
    const projected = projectTransaction(meta(), { [RENT_PROGRAM.toBase58()]: 'Rent (selected)' });
    expect(projected.instructions[0].programLabel).toBe('Rent (selected)');
    const system = projected.instructions.find(
      (entry) => entry.programAddress === SystemProgram.programId.toBase58(),
    );
    expect(system?.programLabel).toBe('System Program');
    // With no reader label, a dClutch program is unnamed rather than guessed.
    expect(projectTransaction(meta()).instructions[0].programLabel).toBeNull();
  });

  it('says plainly when a magic selects no route the census enumerates', () => {
    const data = new Uint8Array(16);
    data.set(new TextEncoder().encode('NOTAMAGC'), 0);
    const message = new TransactionMessage({
      payerKey: PAYER,
      recentBlockhash: PublicKey.default.toBase58(),
      instructions: [
        new TransactionInstruction({
          programId: RENT_PROGRAM,
          keys: [{ pubkey: PAYER, isSigner: true, isWritable: true }],
          data: Buffer.from(data),
        }),
      ],
    }).compileToV0Message();
    const bytes = new VersionedTransaction(message).serialize();
    const addresses = VersionedTransaction.deserialize(bytes).message.staticAccountKeys.map((key) =>
      key.toBase58(),
    );
    const projected = projectTransaction(
      meta({ transactionBytes: bytes, accountAddresses: Object.freeze(addresses) }),
    );
    expect(projected.instructions[0].decoded.magic).toBe('NOTAMAGC');
    expect(projected.instructions[0].decoded.routes).toEqual([]);
    // Renegotiated 2026-08-31 with the copy pass: same behaviour (an
    // unrecognized magic claims no route), new plain wording.
    expect(projected.instructions[0].decoded.note).toContain('No dClutch route');
  });

  it('renders inner CPI frames from the chain’s own metadata, under their outer', () => {
    const projected = projectTransaction(
      meta({
        innerInstructions: Object.freeze([
          Object.freeze({
            outerIndex: 0,
            programIdIndex: 1,
            accounts: Object.freeze([0]),
            // '1' is base58 for a single zero byte: a short, valid payload.
            data: '1',
            stackHeight: 2,
          }),
        ]),
      }),
    );
    const inner = projected.instructions.filter((entry) => entry.innerIndex !== null);
    expect(inner).toHaveLength(1);
    expect(inner[0].outerIndex).toBe(0);
    expect(inner[0].stackHeight).toBe(2);
    // It sits immediately after its outer instruction, not at the end.
    expect(projected.instructions[1]).toBe(inner[0]);
  });

  it('names a refusal, attributes it to the frame that raised it, and says what it means', () => {
    const refusal = PROTOCOL_REFUSALS.find((entry) => entry.program === 'rent' && entry.meaning !== null);
    expect(refusal).toBeDefined();
    if (refusal === undefined) return;
    const projected = projectTransaction(
      meta({
        succeeded: false,
        error: { InstructionError: [0, { Custom: refusal.code }] },
        errorText: JSON.stringify({ InstructionError: [0, { Custom: refusal.code }] }),
        logMessages: Object.freeze([
          `Program ${RENT_PROGRAM.toBase58()} invoke [1]`,
          `Program ${RENT_PROGRAM.toBase58()} failed: custom program error: 0x${refusal.code.toString(16)}`,
        ]),
      }),
    );
    expect(projected.refusal?.code).toBe(refusal.code);
    expect(projected.refusal?.program).toBe(RENT_PROGRAM.toBase58());
    expect(projected.refusal?.attribution.disposition).toBe('named');
    if (projected.refusal?.attribution.disposition === 'named') {
      expect(projected.refusal.attribution.refusal.variant).toBe(refusal.variant);
      expect(projected.refusal.attribution.band.label).toBe('rent');
    }
    expect(projected.runtimeError).toBeNull();
  });

  it('does not claim a foreign program’s error as a dClutch refusal', () => {
    const foreign = 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA';
    const projected = projectTransaction(
      meta({
        succeeded: false,
        error: { InstructionError: [0, { Custom: 1 }] },
        logMessages: Object.freeze([
          `Program ${foreign} invoke [2]`,
          `Program ${foreign} failed: custom program error: 0x1`,
        ]),
      }),
    );
    expect(projected.refusal?.attribution.disposition).toBe('foreign');
    expect(REFUSAL_BANDS.every((band) => band.base > 1)).toBe(true);
  });

  it('reports a runtime refusal in the runtime’s own words when it carries no code', () => {
    const projected = projectTransaction(
      meta({
        succeeded: false,
        error: { InstructionError: [1, 'PrivilegeEscalation'] },
        logMessages: Object.freeze([]),
      }),
    );
    expect(projected.refusal).toBeNull();
    expect(projected.runtimeError).toBe('InstructionError #1: PrivilegeEscalation');
  });

  it('distinguishes an unreadable transaction from an empty one', () => {
    const projected = projectTransaction(meta({ transactionBytes: new Uint8Array([1, 2, 3]) }));
    expect(projected.instructions).toEqual([]);
    // Renegotiated 2026-08-31 with the copy pass: same behaviour, plain wording.
    expect(projected.note).toContain('could not be read');
  });

  it('carries compute units and the invoked frames the logs report', () => {
    const projected = projectTransaction(
      meta({
        logMessages: Object.freeze([
          `Program ${RENT_PROGRAM.toBase58()} invoke [1]`,
          `Program ${SystemProgram.programId.toBase58()} invoke [2]`,
        ]),
      }),
    );
    expect(projected.computeUnits).toBe('18000');
    expect(projected.invoked.map((frame) => frame.depth)).toEqual([1, 2]);
  });
});

/**
 * A transaction that declares its budget and then faults.
 *
 * Built through `ComputeBudgetProgram` rather than by hand, so the bytes the
 * view has to decode are the bytes a real client emits — including the
 * discriminants, which the view reads from `genericFoundingV1` and this file
 * therefore never states.
 */
function budgetedMeta(heapBytes: number, unitLimit: number, faultAddress: number): TransactionMetaObservation {
  const message = new TransactionMessage({
    payerKey: PAYER,
    recentBlockhash: PublicKey.default.toBase58(),
    instructions: [
      ComputeBudgetProgram.setComputeUnitLimit({ units: unitLimit }),
      ComputeBudgetProgram.requestHeapFrame({ bytes: heapBytes }),
      new TransactionInstruction({
        programId: RENT_PROGRAM,
        keys: [{ pubkey: PAYER, isSigner: true, isWritable: true }],
        data: Buffer.alloc(8),
      }),
    ],
  }).compileToV0Message();
  const bytes = new VersionedTransaction(message).serialize();
  const addresses = VersionedTransaction.deserialize(bytes).message.staticAccountKeys.map((key) => key.toBase58());
  return Object.freeze({
    signature: SIGNATURE,
    slot: '4242',
    blockTime: '1790000000',
    succeeded: false,
    errorText: null,
    error: { InstructionError: [2, 'ProgramFailedToComplete'] },
    feeLamports: '5000',
    computeUnits: '203408',
    accountAddresses: Object.freeze(addresses),
    preBalances: Object.freeze(addresses.map(() => '1000000')),
    postBalances: Object.freeze(addresses.map(() => '1000000')),
    logMessages: Object.freeze([
      `Program ${RENT_PROGRAM.toBase58()} invoke [1]`,
      `Program ${RENT_PROGRAM.toBase58()} consumed 203408 of ${unitLimit} compute units`,
      `Program ${RENT_PROGRAM.toBase58()} failed: Access violation writing 8 bytes at address 0x${faultAddress.toString(16)}`,
    ]),
    innerInstructions: Object.freeze([]),
    returnData: null,
    transactionBytes: bytes,
  });
}

describe('a transaction that aborted rather than refusing', () => {
  it('reads the budget it declared out of its own ComputeBudget instructions', () => {
    const projected = projectTransaction(budgetedMeta(65_536, 1_400_000, 0x30000fa58));
    expect(projected.budget.heapFrameBytes).toBe(65_536);
    expect(projected.budget.computeUnitLimit).toBe(1_400_000);
  });

  it('names the fault, places it, and says what can be done — where it used to show a discriminant', () => {
    const projected = projectTransaction(budgetedMeta(65_536, 1_400_000, 0x30000fa58));
    expect(projected.refusal).toBeNull();
    // The discriminant is still reported, because the node reported it. It is
    // no longer the whole of what the reader is told.
    expect(projected.runtimeError).toBe('InstructionError #2: ProgramFailedToComplete');
    expect(projected.abort?.program).toBe(RENT_PROGRAM.toBase58());
    expect(projected.abort?.fault?.region).toBe('heap');
    expect(projected.abort?.fault?.offset).toBe(64_088);
    expect(projected.abortDiagnosis?.title).toContain('the runtime had not mapped it');
    expect(projected.abortDiagnosis?.remedy).toBeTruthy();
  });

  it('leaves a refusal alone: a program that returned a code did not fault', () => {
    const refusal = PROTOCOL_REFUSALS.find((entry) => entry.program === 'rent');
    expect(refusal).toBeDefined();
    if (refusal === undefined) return;
    const projected = projectTransaction(
      meta({
        succeeded: false,
        error: { InstructionError: [0, { Custom: refusal.code }] },
        logMessages: Object.freeze([
          `Program ${RENT_PROGRAM.toBase58()} failed: custom program error: 0x${refusal.code.toString(16)}`,
        ]),
      }),
    );
    expect(projected.refusal?.code).toBe(refusal.code);
    expect(projected.abort).toBeNull();
    expect(projected.abortDiagnosis).toBeNull();
  });

  it('carries a null budget for a transaction that declared none', () => {
    const projected = projectTransaction(meta());
    expect(projected.budget).toEqual({ heapFrameBytes: null, computeUnitLimit: null });
  });
});

describe('reading a transaction that is not there', () => {
  it('says the node does not serve it, rather than returning an empty one', async () => {
    const result = await inspectTransaction({ transaction: async () => null }, { signature: SIGNATURE });
    expect(result.status).toBe('absent');
    if (result.status !== 'absent') return;
    expect(result.reason).toContain('does not serve this signature');
  });

  it('projects what the node does serve', async () => {
    const observation = meta();
    const result = await inspectTransaction(
      { transaction: async () => observation },
      { signature: SIGNATURE },
    );
    expect(result.status).toBe('found');
    if (result.status !== 'found') return;
    expect(result.transaction.slot).toBe('4242');
  });
});
