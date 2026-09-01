import { PublicKey, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  markClientOperationSubmittedV1,
  transactionSignatureV1,
  writeUnsignedClientOperationJournalV1,
  type ClientOperationJournalStorageV1,
} from './clientOperationJournal';
import { SOURCE_TERMINAL_PLAN_FORMAT_V1 } from './generated/sourceReadinessWasmV1';
import { restoreSourceTerminalJournalV1, sourceTerminalJournalInputV1 } from './sourceTerminalOperationV1';
import { buildSourceTerminalTransactionV1, type SourceTerminalAcquisitionV1, type SourceTerminalPlanV1, type SourceTerminalTransactionV1 } from './sourceTerminalV1';

const address = (byte: number) => new PublicKey(new Uint8Array(32).fill(byte)).toBase58();

class MemoryStorage implements ClientOperationJournalStorageV1 {
  readonly values = new Map<string, string>();
  get length() { return this.values.size; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, value); }
  removeItem(key: string) { this.values.delete(key); }
}

function fixture(): Readonly<{
  scope: Readonly<{ clusterGenesis: string; market: string; owner: string }>;
  acquisition: SourceTerminalAcquisitionV1;
  transaction: SourceTerminalTransactionV1;
}> {
  const owner = address(1);
  const market = address(2);
  const certificate = address(3);
  const metas = Array.from({ length: 22 }, (_, index) => Object.freeze({
    address: index === 1 ? market : index === 13 ? certificate : address(index + 20),
    isSigner: false,
    isWritable: [1, 12, 13].includes(index),
  }));
  const plan: SourceTerminalPlanV1 = Object.freeze({
    format: SOURCE_TERMINAL_PLAN_FORMAT_V1, route: 'admit', observedSlot: '7',
    instruction: Object.freeze({ program: address(50), accounts: Object.freeze(metas), dataBase64: 'AQ==' }),
    accounts: Object.freeze({ protocolWritable: Object.freeze([market, metas[12]!.address, certificate]), completion: Object.freeze([market, address(6), certificate]) }),
    geometry: Object.freeze({ protocolAccountCount: 22, protocolUniqueAccountCount: 22,
      protocolWritableCount: 3, protocolSignerCount: 0, protocolDataLen: 1,
      transactionInstructionCountWithoutComputeBudget: 1, transactionLockCountWithoutPayer: 23 }),
    facts: Object.freeze({ terminal: 'false' }),
  });
  const planJson = JSON.stringify({ ...plan, prepay: null });
  const acquisition = Object.freeze({ plan, planJson, snapshotJson: '{}', observationAddresses: Object.freeze([]) });
  const transaction = buildSourceTerminalTransactionV1(acquisition, owner,
    { blockhash: address(5), lastValidBlockHeight: '20' });
  return Object.freeze({
    scope: Object.freeze({ clusterGenesis: address(7), market, owner }),
    acquisition,
    transaction,
  });
}

describe('Source terminal crash journal', () => {
  it('round-trips the exact Market, certificate, Rust plan, and sole-payer packet', async () => {
    const value = fixture();
    const input = await sourceTerminalJournalInputV1(value.scope, value.acquisition, value.transaction);
    const journal = await writeUnsignedClientOperationJournalV1(new MemoryStorage(), input);
    const restored = await restoreSourceTerminalJournalV1(journal);
    expect(restored.intent.certificate).toBe(address(3));
    expect(restored.transaction.serialize()).toEqual(value.transaction.wireBytes);

    const intent = JSON.parse(journal.intent) as Record<string, unknown>;
    intent.certificate = address(8);
    await expect(restoreSourceTerminalJournalV1({ ...journal, intent: JSON.stringify(intent) }))
      .rejects.toThrow(/substituted|digest mismatch/);
  });

  it('refuses a submitted signature over any other message', async () => {
    const value = fixture();
    const storage = new MemoryStorage();
    const unsigned = await writeUnsignedClientOperationJournalV1(storage,
      await sourceTerminalJournalInputV1(value.scope, value.acquisition, value.transaction));
    const substituted = new VersionedTransaction(new TransactionMessage({ payerKey: new PublicKey(value.scope.owner),
      recentBlockhash: address(9), instructions: [] }).compileToLegacyMessage());
    substituted.signatures[0] = new Uint8Array(64).fill(10);
    const submitted = await markClientOperationSubmittedV1(storage, unsigned,
      transactionSignatureV1(substituted.signatures[0]!), substituted.serialize());
    await expect(restoreSourceTerminalJournalV1(submitted)).rejects.toThrow(/substituted the saved message/);
  });
});
