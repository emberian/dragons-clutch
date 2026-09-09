import { Keypair, SystemProgram, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';
import { DealerSelectionGuardV1, findDealerOperationV1, restoreDealerPlanV1, retainDealerPlanV1, submitDealerOperationV1 } from './dealerLiquidityOperation';
import { discardUnsignedClientOperationJournalV1 } from './clientOperationJournal';

function storage() {
  const rows = new Map<string, string>();
  return { get length() { return rows.size; }, key: (n: number) => [...rows.keys()][n] ?? null,
    getItem: (k: string) => rows.get(k) ?? null, setItem: (k: string, v: string) => { rows.set(k, v); }, removeItem: (k: string) => { rows.delete(k); } };
}
function fixture() {
  const owner = Keypair.fromSeed(new Uint8Array(32).fill(31));
  const market = Keypair.fromSeed(new Uint8Array(32).fill(32)).publicKey;
  const transaction = new VersionedTransaction(new TransactionMessage({ payerKey: owner.publicKey,
    recentBlockhash: market.toBase58(), instructions: [SystemProgram.transfer({ fromPubkey: owner.publicKey, toPubkey: market, lamports: 1 })],
  }).compileToV0Message());
  return { owner, transaction, scope: { clusterGenesis: market.toBase58(), market: market.toBase58(), owner: owner.publicKey.toBase58() } };
}

describe('Dealer durable operation boundary', () => {
  it('invalidates an old async result immediately when selection changes', async () => {
    const guard = new DealerSelectionGuardV1();
    const prior = guard.current();
    let finish!: () => void;
    const pending = new Promise<void>((resolve) => { finish = resolve; }).then(() => guard.require(prior));
    guard.invalidate(); finish();
    await expect(pending).rejects.toThrow('Dealer selection changed while this operation was pending');
    expect(() => guard.require(guard.current())).not.toThrow();
  });

  it('retains ambiguity before a timed-out send and resumes the same packet after reload', async () => {
    const store = storage(); const { scope, owner, transaction } = fixture();
    const journal = await retainDealerPlanV1(store, scope, '{"intent":"add"}', '{"native":"plan"}', transaction);
    const signed = VersionedTransaction.deserialize(transaction.serialize()); signed.sign([owner]);
    await expect(submitDealerOperationV1(store, journal, signed, async () => {
      expect((await findDealerOperationV1(store, scope))?.phase).toBe('submitted');
      throw new Error('RPC response lost');
    })).rejects.toThrow('RPC response lost');
    const recovered = await findDealerOperationV1(store, scope);
    expect(recovered?.phase).toBe('submitted');
    expect(restoreDealerPlanV1(recovered!).transaction.message.serialize()).toEqual(transaction.message.serialize());
    await expect(discardUnsignedClientOperationJournalV1(store, recovered!)).rejects.toThrow('a submitted operation is ambiguous and cannot be discarded');
    await expect(retainDealerPlanV1(store, scope, 'new intent', 'new plan', transaction)).rejects.toThrow('still unresolved');
    let sends = 0;
    await expect(submitDealerOperationV1(store, recovered!, signed, async () => { sends += 1; return ''; })).rejects.toThrow('already submitted');
    expect(sends).toBe(0);
  });

  it('refuses a substituted message before persisting or sending', async () => {
    const store = storage(); const { scope, owner, transaction } = fixture();
    const journal = await retainDealerPlanV1(store, scope, 'input', 'plan', transaction);
    const changed = VersionedTransaction.deserialize(transaction.serialize());
    changed.message.recentBlockhash = owner.publicKey.toBase58(); changed.sign([owner]);
    let sends = 0;
    await expect(submitDealerOperationV1(store, journal, changed, async () => { sends += 1; return ''; })).rejects.toThrow('changed the reviewed unsigned message');
    expect(sends).toBe(0);
    expect((await findDealerOperationV1(store, scope))?.phase).toBe('unsigned');
  });
});
