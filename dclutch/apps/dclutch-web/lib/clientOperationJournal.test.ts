import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { decodeBase58 } from './explorer/base58';
import {
  clearFinalizedClientOperationJournalV1,
  discardUnsignedClientOperationJournalV1,
  exactTransactionSignatureV1,
  findClientOperationJournalV1,
  markClientOperationSubmittedV1,
  requireSubmittedSignatureMatchV1,
  transactionSignatureV1,
  writeUnsignedClientOperationJournalV1,
  type ClientOperationJournalStorageV1,
  type ClientOperationScopeV1,
} from './clientOperationJournal';

class MemoryStorage implements ClientOperationJournalStorageV1 {
  readonly values = new Map<string, string>();
  get length() { return this.values.size; }
  key(index: number) { return [...this.values.keys()][index] ?? null; }
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, value); }
  removeItem(key: string) { this.values.delete(key); }
}

const address = (byte: number) => new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
const scope: ClientOperationScopeV1 = Object.freeze({
  clusterGenesis: address(1),
  market: address(2),
  owner: address(3),
});
const digest = (byte: number) => byte.toString(16).padStart(2, '0').repeat(32);

async function unsigned(storage: MemoryStorage) {
  return writeUnsignedClientOperationJournalV1(storage, {
    ...scope,
    operation: 'wallet-terminal-payout-v3',
    operationDigest: digest(4),
    intent: '{"request":"exact"}',
    plan: '{"prestate":"exact"}',
  });
}

describe('client operation crash journal', () => {
  it('keys and recovers one exact unsigned plan by cluster, Market, owner, operation, and digest', async () => {
    const storage = new MemoryStorage();
    const journal = await unsigned(storage);
    expect(storage.length).toBe(1);
    expect(storage.key(0)).toContain(`:${scope.clusterGenesis}:${scope.market}:${scope.owner}:${digest(4)}`);
    await expect(findClientOperationJournalV1(storage, scope, 'wallet-terminal-payout-v3')).resolves.toEqual(journal);
    for (const changed of [
      { ...scope, clusterGenesis: address(9) },
      { ...scope, market: address(9) },
      { ...scope, owner: address(9) },
    ]) await expect(findClientOperationJournalV1(storage, changed, 'wallet-terminal-payout-v3')).resolves.toBeNull();
    await expect(findClientOperationJournalV1(storage, scope, 'claims-replay-create-v1')).resolves.toBeNull();
  });

  it('derives the transaction id before submission and hostile-decodes it as exactly 64 canonical bytes', () => {
    const bytes = Uint8Array.from({ length: 64 }, (_, index) => index + 1);
    const signature = transactionSignatureV1(bytes);
    expect(decodeBase58(signature)).toEqual(bytes);
    expect(exactTransactionSignatureV1(signature)).toBe(signature);
    expect(() => exactTransactionSignatureV1(`0${signature}`)).toThrow(/canonical base58/);
    const leadingZero = Uint8Array.from({ length: 64 }, (_, index) => index);
    expect(decodeBase58(transactionSignatureV1(leadingZero))).toEqual(leadingZero);
    expect(() => transactionSignatureV1(new Uint8Array(64))).toThrow(/exact first Ed25519 signature/);
  });

  it('persists the signed packet id before submission and never overwrites or discards ambiguity', async () => {
    const storage = new MemoryStorage();
    const journal = await unsigned(storage);
    const signature = transactionSignatureV1(new Uint8Array(64).fill(7));
    const submitted = await markClientOperationSubmittedV1(storage, journal, signature);
    expect(submitted).toMatchObject({ phase: 'submitted', signature });
    await expect(markClientOperationSubmittedV1(storage, journal, transactionSignatureV1(new Uint8Array(64).fill(8))))
      .rejects.toThrow(/another transaction/);
    await expect(discardUnsignedClientOperationJournalV1(storage, submitted)).rejects.toThrow(/cannot be discarded/);
    await expect(writeUnsignedClientOperationJournalV1(storage, {
      ...scope, operation: 'wallet-terminal-payout-v3', operationDigest: digest(5), intent: 'replacement', plan: 'replacement',
    })).rejects.toThrow(/still unresolved/);
    expect(storage.length).toBe(1);
    expect(() => requireSubmittedSignatureMatchV1(signature, transactionSignatureV1(new Uint8Array(64).fill(8))))
      .toThrow(/not the exact signed packet id/);
  });

  it('fails before a wallet can be asked when recovery storage cannot persist the unsigned plan', async () => {
    const storage = new MemoryStorage();
    storage.setItem = () => { throw new Error('storage quota refused'); };
    await expect(unsigned(storage)).rejects.toThrow(/storage quota refused/);
    expect(storage.length).toBe(0);
  });

  it('allows explicit discard only while unsigned and finalized-verifier clearing afterward', async () => {
    const storage = new MemoryStorage();
    const first = await unsigned(storage);
    await discardUnsignedClientOperationJournalV1(storage, first);
    expect(storage.length).toBe(0);
    const second = await unsigned(storage);
    const submitted = await markClientOperationSubmittedV1(storage, second, transactionSignatureV1(new Uint8Array(64).fill(9)));
    await clearFinalizedClientOperationJournalV1(storage, submitted);
    expect(storage.length).toBe(0);
  });

  it('refuses plan, scope, signature, and duplicate-journal substitution instead of replaying it', async () => {
    const storage = new MemoryStorage();
    await unsigned(storage);
    const [key, source] = [...storage.values.entries()][0]!;
    const changedPlan = JSON.parse(source) as Record<string, unknown>;
    changedPlan.plan = '{"prestate":"substituted"}';
    storage.values.set(key, JSON.stringify(changedPlan));
    await expect(findClientOperationJournalV1(storage, scope, 'wallet-terminal-payout-v3')).rejects.toThrow(/plan bytes/);

    storage.values.set(key, source);
    const changedIntent = JSON.parse(source) as Record<string, unknown>;
    changedIntent.intent = '{"request":"substituted"}';
    storage.values.set(key, JSON.stringify(changedIntent));
    await expect(findClientOperationJournalV1(storage, scope, 'wallet-terminal-payout-v3')).rejects.toThrow(/intent bytes/);

    storage.values.set(key, source);
    const changedOwner = JSON.parse(source) as Record<string, unknown>;
    changedOwner.owner = address(8);
    storage.values.set(key, JSON.stringify(changedOwner));
    await expect(findClientOperationJournalV1(storage, scope, 'wallet-terminal-payout-v3')).rejects.toThrow(/storage key disagrees|substituted/);

    storage.values.set(key, source);
    const changedSignature = JSON.parse(source) as Record<string, unknown>;
    changedSignature.phase = 'submitted'; changedSignature.signature = 'not-base58';
    storage.values.set(key, JSON.stringify(changedSignature));
    await expect(findClientOperationJournalV1(storage, scope, 'wallet-terminal-payout-v3')).rejects.toThrow(/signature/);

    storage.values.set(key, source);
    storage.values.set(key.replace(digest(4), digest(6)), source.replaceAll(digest(4), digest(6)));
    await expect(findClientOperationJournalV1(storage, scope, 'wallet-terminal-payout-v3')).rejects.toThrow(/more than one/);
  });
});
