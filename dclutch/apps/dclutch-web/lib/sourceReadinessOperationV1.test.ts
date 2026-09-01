import { PublicKey, TransactionInstruction, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  markClientOperationSubmittedV1,
  transactionSignatureV1,
  writeUnsignedClientOperationJournalV1,
  type ClientOperationJournalStorageV1,
} from './clientOperationJournal';
import { SOURCE_READINESS_PLAN_FORMAT_V1 } from './generated/sourceReadinessWasmV1';
import {
  restoreSourceReadinessJournalV1,
  sourceReadinessJournalInputV1,
  sourceReadinessPoststateCompletesV1,
} from './sourceReadinessOperationV1';
import { parseSourceReadinessPlanV1 } from './sourceReadinessV1';

const address = (byte: number) => new PublicKey(new Uint8Array(32).fill(byte)).toBase58();

class MemoryStorage implements ClientOperationJournalStorageV1 {
  readonly values = new Map<string, string>();
  get length() { return this.values.size; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, value); }
  removeItem(key: string) { this.values.delete(key); }
}

function fixture() {
  const owner = address(1);
  const route = 'create' as const;
  const planJson = JSON.stringify({
    format: SOURCE_READINESS_PLAN_FORMAT_V1,
    route,
    observedSlot: '7',
    instruction: { program: address(2), accounts: [{ address: address(3), isSigner: false, isWritable: true }], dataBase64: 'AQ==', },
    prepay: { destination: address(4), lamports: '0' },
    accounts: { protocolWritable: [address(3)], completion: [address(3)] },
    geometry: {
      protocolAccountCount: 1, protocolUniqueAccountCount: 2, protocolWritableCount: 1,
      protocolSignerCount: 0, protocolDataLen: 1,
      transactionInstructionCountWithoutComputeBudget: 1, transactionLockCountWithoutPayer: 2,
    },
    facts: {},
  });
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: new PublicKey(owner),
    recentBlockhash: address(5),
    instructions: [new TransactionInstruction({ programId: new PublicKey(address(2)), keys: [], data: Buffer.of(1) })],
  }).compileToLegacyMessage());
  const wireBytes = transaction.serialize();
  return {
    scope: { clusterGenesis: address(6), market: address(7), owner },
    acquisition: { plan: parseSourceReadinessPlanV1(planJson), planJson, snapshotJson: '{}', observationAddresses: Object.freeze([]) },
    transaction: { transaction, wireBytes, payer: owner, route, observedSlot: '7', lastValidBlockHeight: '20' },
  };
}

describe('Source readiness crash journal', () => {
  it('round-trips one exact sole-payer plan and refuses wire substitution', async () => {
    const value = fixture();
    const input = await sourceReadinessJournalInputV1(value.scope, value.acquisition, value.transaction);
    const journal = await writeUnsignedClientOperationJournalV1(new MemoryStorage(), input);
    const restored = await restoreSourceReadinessJournalV1(journal);
    expect(restored.intent.route).toBe('create');
    expect(restored.wireBytes).toEqual(value.transaction.wireBytes);

    const changed = JSON.parse(journal.plan) as Record<string, unknown>;
    changed.lastValidBlockHeight = '21';
    await expect(restoreSourceReadinessJournalV1({ ...journal, plan: JSON.stringify(changed) }))
      .rejects.toThrow(/digest does not authenticate/);
  });

  it('accepts only the exact adjacent finalized route', () => {
    expect(sourceReadinessPoststateCompletesV1('create', 'activate')).toBe(true);
    expect(sourceReadinessPoststateCompletesV1('activate', 'accept')).toBe(true);
    expect(sourceReadinessPoststateCompletesV1('accept', 'complete')).toBe(true);
    expect(sourceReadinessPoststateCompletesV1('create', 'complete')).toBe(false);
    expect(sourceReadinessPoststateCompletesV1('activate', 'activate')).toBe(false);
  });

  it('refuses a canonical signed packet whose message substitutes the saved unsigned act', async () => {
    const value = fixture();
    const storage = new MemoryStorage();
    const input = await sourceReadinessJournalInputV1(value.scope, value.acquisition, value.transaction);
    const unsigned = await writeUnsignedClientOperationJournalV1(storage, input);
    const substituted = new VersionedTransaction(new TransactionMessage({
      payerKey: new PublicKey(value.scope.owner),
      recentBlockhash: address(8),
      instructions: [],
    }).compileToLegacyMessage());
    substituted.signatures[0] = new Uint8Array(64).fill(9);
    const signature = transactionSignatureV1(substituted.signatures[0]!);
    const submitted = await markClientOperationSubmittedV1(storage, unsigned, signature, substituted.serialize());
    await expect(restoreSourceReadinessJournalV1(submitted)).rejects.toThrow(/substituted the saved unsigned message/);
  });
});
