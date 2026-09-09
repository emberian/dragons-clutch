import { Keypair, SystemProgram, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';
import { discardUnsignedClientOperationJournalV1, transactionSignatureV1 } from './clientOperationJournal';
import { type TransactionMetaObservation, type SignatureStatusObservation } from '@dclutch/sdk/rpc';
import { structuredLifecycleIntentV1 } from './structuredLifecycleModelV1';
import {
  finalizeStructuredLifecycleStepV1, findStructuredLifecycleStepV1, restoreStructuredLifecycleStepV1,
  retainStructuredLifecycleStepV1, signStructuredLifecycleStepV1, submitStructuredLifecycleStepV1,
} from './structuredLifecycleOperationV1';

function storage() {
  const entries = new Map<string, string>();
  return { get length() { return entries.size; }, key: (index: number) => [...entries.keys()][index] ?? null,
    getItem: (key: string) => entries.get(key) ?? null, setItem: (key: string, value: string) => { entries.set(key, value); },
    removeItem: (key: string) => { entries.delete(key); } };
}
function fixture() {
  const payer = Keypair.fromSeed(new Uint8Array(32).fill(41));
  const market = Keypair.fromSeed(new Uint8Array(32).fill(42)).publicKey;
  const intent = structuredLifecycleIntentV1({ market: market.toBase58(), payer: payer.publicKey.toBase58(), action: 'activate-receipt', coordinate: '' });
  // Transport-only fixture; this transfer does not claim native lifecycle parity.
  const transaction = new VersionedTransaction(new TransactionMessage({ payerKey: payer.publicKey,
    recentBlockhash: market.toBase58(), instructions: [SystemProgram.transfer({ fromPubkey: payer.publicKey, toPubkey: market, lamports: 1 })],
  }).compileToV0Message());
  return { payer, intent, transaction, scope: { clusterGenesis: market.toBase58(), market: market.toBase58(), owner: payer.publicKey.toBase58() } };
}
const checked = async () => {};

describe('Structured per-step durable wallet boundary', () => {
  it('requires durable storage and current native review before opening the wallet', async () => {
    const { scope, intent, transaction } = fixture();
    const store = storage();
    await expect(retainStructuredLifecycleStepV1({ ...store, setItem: () => { throw new Error('storage unavailable'); } }, scope, intent, 'input', 'plan', transaction)).rejects.toThrow('storage unavailable');
    const journal = await retainStructuredLifecycleStepV1(store, scope, intent, 'input', 'plan', transaction);
    let walletCalls = 0;
    await expect(signStructuredLifecycleStepV1(store, journal, async () => { throw new Error('native prestate changed'); }, async () => { walletCalls += 1; })).rejects.toThrow('native prestate changed');
    expect(walletCalls).toBe(0);
    await expect(signStructuredLifecycleStepV1(store, journal, async () => { await discardUnsignedClientOperationJournalV1(store, journal); }, async () => { walletCalls += 1; })).rejects.toThrow('Structured unsigned review changed during verification');
    expect(walletCalls).toBe(0);
  });

  it('retains a timed-out submission and refuses a second send or replacement step', async () => {
    const { scope, intent, payer, transaction } = fixture(); const store = storage();
    const journal = await retainStructuredLifecycleStepV1(store, scope, intent, 'input', 'plan', transaction);
    const signed = VersionedTransaction.deserialize(transaction.serialize()); signed.sign([payer]);
    await expect(submitStructuredLifecycleStepV1(store, journal, signed, checked, async () => {
      expect((await findStructuredLifecycleStepV1(store, scope))?.phase).toBe('submitted');
      throw new Error('network response lost');
    })).rejects.toThrow('network response lost');
    const resumed = (await findStructuredLifecycleStepV1(store, scope))!;
    expect((await restoreStructuredLifecycleStepV1(resumed)).transaction.message.serialize()).toEqual(transaction.message.serialize());
    let sends = 0;
    // Even a stale unsigned UI object cannot repeat the saved submission.
    await expect(submitStructuredLifecycleStepV1(store, journal, signed, checked, async () => { sends += 1; return resumed.signature!; })).rejects.toThrow('Structured step already submitted; recover its retained signature');
    expect(sends).toBe(0);
    await expect(retainStructuredLifecycleStepV1(store, scope, intent, 'next input', 'next plan', transaction)).rejects.toThrow('still unresolved');
    await expect(discardUnsignedClientOperationJournalV1(store, resumed)).rejects.toThrow('a submitted operation is ambiguous and cannot be discarded');
  });

  it('refuses a changed signed message and keeps the unsigned review', async () => {
    const { scope, intent, payer, transaction } = fixture(); const store = storage();
    const journal = await retainStructuredLifecycleStepV1(store, scope, intent, 'input', 'plan', transaction);
    const changed = VersionedTransaction.deserialize(transaction.serialize());
    changed.message.recentBlockhash = payer.publicKey.toBase58(); changed.sign([payer]);
    let sends = 0;
    await expect(submitStructuredLifecycleStepV1(store, journal, changed, checked, async () => { sends += 1; return ''; })).rejects.toThrow('Structured signed packet changed the reviewed message');
    expect(sends).toBe(0);
    expect((await findStructuredLifecycleStepV1(store, scope))?.phase).toBe('unsigned');
  });

  it('binds retained intent to the chain scope and actual fee payer', async () => {
    const { scope, intent, transaction } = fixture(); const store = storage();
    await expect(retainStructuredLifecycleStepV1(store, { ...scope, owner: scope.market }, intent, 'input', 'plan', transaction)).rejects.toThrow('Structured journal differs from the selected Market or payer');
    const wrongPayer = { ...intent, payer: scope.market };
    await expect(retainStructuredLifecycleStepV1(store, { ...scope, owner: scope.market }, wrongPayer, 'input', 'plan', transaction)).rejects.toThrow('Structured review requires one unsigned payer packet');
  });

  it('retains recovery until the exact packet and native finalized poststate both match', async () => {
    const { scope, intent, payer, transaction } = fixture(); const store = storage();
    const review = await retainStructuredLifecycleStepV1(store, scope, intent, 'input', 'plan', transaction);
    const signed = VersionedTransaction.deserialize(transaction.serialize()); signed.sign([payer]);
    const signature = transactionSignatureV1(signed.signatures[0]!);
    const journal = await submitStructuredLifecycleStepV1(store, review, signed, checked, async () => signature);
    let status: SignatureStatusObservation = { signature, known: true, slot: '50', confirmationStatus: 'confirmed', succeeded: true, errorText: null };
    let landed: TransactionMetaObservation = { signature, slot: '50', blockTime: null, succeeded: true, errorText: null, error: null,
      feeLamports: '5000', computeUnits: '100', accountAddresses: [], preBalances: [], postBalances: [],
      logMessages: [], innerInstructions: [], returnData: null, transactionBytes: signed.serialize() };
    const client = { assertMutationCluster: async () => ({ endpoint: 'http://localhost:8899', genesisHash: scope.clusterGenesis, kind: 'loopback-local-validator' as const }),
      signatureStatuses: async () => [status], transaction: async () => landed };
    let poststateCalls = 0;
    const verify = async () => { poststateCalls += 1; };
    await expect(finalizeStructuredLifecycleStepV1(store, journal, client, verify)).rejects.toThrow('Structured step is not finalized; its signed packet remains retained');
    expect(poststateCalls).toBe(0);
    status = { ...status, confirmationStatus: 'finalized' };
    landed = { ...landed, transactionBytes: transaction.serialize() };
    await expect(finalizeStructuredLifecycleStepV1(store, journal, client, verify)).rejects.toThrow('Structured finalized transaction differs from the retained signed packet');
    expect(poststateCalls).toBe(0);
    landed = { ...landed, transactionBytes: signed.serialize() };
    await expect(finalizeStructuredLifecycleStepV1(store, journal, client, async () => { throw new Error('native Position poststate differs'); })).rejects.toThrow('native Position poststate differs');
    expect((await findStructuredLifecycleStepV1(store, scope))?.phase).toBe('submitted');
    expect(await finalizeStructuredLifecycleStepV1(store, journal, client, verify)).toEqual({ signature, slot: '50' });
    expect(poststateCalls).toBe(1);
    expect(await findStructuredLifecycleStepV1(store, scope)).toBeNull();
  });
});
