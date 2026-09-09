import { describe, expect, it, vi } from 'vitest';
import { type TransactionMetaObservation, type SolanaRpcClient } from '@dclutch/sdk/rpc';
import { generalOperationReceiptTestFixtureV5, generalOperationTestFixtureV5 } from '@dclutch/sdk/fixtures/general-operation-test-v5';
import { archiveGeneralOperationV5, retainGeneralOperationV5, restoreGeneralOperationV5, signGeneralOperationV5, submitGeneralOperationV5 } from './generalOperationV5';
import { findClientOperationJournalV1, submittedClientOperationWireV1, transactionSignatureV1 } from './clientOperationJournal';

function storage() { const entries = new Map<string, string>(); return { get length() { return entries.size; }, key: (index: number) => [...entries.keys()][index] ?? null, getItem: (key: string) => entries.get(key) ?? null, setItem: (key: string, value: string) => { entries.set(key, value); }, removeItem: (key: string) => { entries.delete(key); } }; }
async function setup() {
  const fixture = await generalOperationTestFixtureV5(); const store = storage();
  const scope = { clusterGenesis: '11111111111111111111111111111111', market: fixture.inspection.plan.market, owner: fixture.payer.publicKey.toBase58() };
  const journal = await retainGeneralOperationV5(store, scope, fixture.nativePlan, fixture.preview);
  const client = { assertMutationCluster: async () => ({ genesisHash: scope.clusterGenesis }), multipleAccounts: async () => ({ slot: fixture.preview.observedSlot, accounts: [{ address: fixture.inspection.plan.root, account: fixture.root }] }) } as unknown as SolanaRpcClient;
  return { ...fixture, store, scope, journal, client };
}
describe('durable General native-plan operation', () => {
  it('recovers exact unsigned intent after interruption before wallet access', async () => {
    const f = await setup(); const recovered = await findClientOperationJournalV1(f.store, f.scope, 'general-v5');
    expect(recovered).not.toBeNull();
    const restored = await restoreGeneralOperationV5(recovered!);
    expect(restored.nativePlan).toBe(f.nativePlan); expect(restored.transaction.serialize()).toEqual(f.transaction.serialize());
  });
  it('requires persisted intent and current reviewed inputs before prompting a wallet', async () => {
    const f = await setup(); const signTransaction = vi.fn(); const wallet = { publicKey: f.payer.publicKey, connect: async () => undefined, signTransaction };
    f.client.multipleAccounts = async () => ({ slot: f.preview.observedSlot, accounts: [{ address: f.inspection.plan.root, account: { ...f.root, lamports: '99' } }] });
    await expect(signGeneralOperationV5(f.store, f.client, f.journal, wallet, f.scope.owner, f.inspection.plan.tradingProgram)).rejects.toThrow('General reviewed input changed'); expect(signTransaction).not.toHaveBeenCalled();
    f.store.removeItem(f.store.key(0)!);
    await expect(signGeneralOperationV5(f.store, f.client, f.journal, wallet, f.scope.owner, f.inspection.plan.tradingProgram)).rejects.toThrow('General unsigned journal disappeared'); expect(signTransaction).not.toHaveBeenCalled();
  });
  it('refuses an imported program outside the selected deployment before wallet access', async () => {
    const f = await setup(); const signTransaction = vi.fn();
    const wallet = { publicKey: f.payer.publicKey, connect: async () => undefined, signTransaction };
    await expect(signGeneralOperationV5(f.store, f.client, f.journal, wallet, f.scope.owner, f.scope.owner)).rejects.toThrow('General native plan differs from the selected Trading deployment');
    expect(signTransaction).not.toHaveBeenCalled();
  });
  it('uses the connected wallet for the exact message and refuses a rewritten message', async () => {
    const f = await setup();
    const wallet = { publicKey: f.payer.publicKey, connect: async () => undefined, signTransaction: async (transaction: typeof f.transaction) => { transaction.sign([f.payer]); return transaction; } };
    const signed = await signGeneralOperationV5(f.store, f.client, f.journal, wallet, f.scope.owner, f.inspection.plan.tradingProgram);
    expect(signed.complete).toBe(true); expect(signed.transaction.message.serialize()).toEqual(f.transaction.message.serialize());
    wallet.signTransaction = async (transaction) => { transaction.message.recentBlockhash = f.scope.owner; transaction.sign([f.payer]); return transaction; };
    await expect(signGeneralOperationV5(f.store, f.client, f.journal, wallet, f.scope.owner, f.inspection.plan.tradingProgram)).rejects.toThrow('wallet rewrote the transaction message');
  });
  it('stops at the selection guard before wallet access after asynchronous acquisition', async () => {
    const f = await setup(); const signTransaction = vi.fn();
    const wallet = { publicKey: f.payer.publicKey, connect: async () => undefined, signTransaction };
    await expect(signGeneralOperationV5(f.store, f.client, f.journal, wallet, f.scope.owner, f.inspection.plan.tradingProgram, undefined, () => { throw new Error('selection changed'); })).rejects.toThrow('selection changed');
    expect(signTransaction).not.toHaveBeenCalled();
  });
  it('persists signed bytes before an ambiguous send and resumes only those same bytes', async () => {
    const f = await setup(); f.transaction.sign([f.payer]); const sent: Uint8Array[] = [];
    f.client.sendRawTransaction = async (wire) => { const retained = await findClientOperationJournalV1(f.store, f.scope, 'general-v5'); expect(retained?.phase).toBe('submitted'); expect(submittedClientOperationWireV1(retained!)).toEqual(wire); sent.push(wire); throw new Error('transport interrupted'); };
    await expect(submitGeneralOperationV5(f.store, f.client, f.journal, f.inspection.plan.tradingProgram, f.transaction)).rejects.toThrow('transport interrupted');
    const retained = (await findClientOperationJournalV1(f.store, f.scope, 'general-v5'))!;
    f.client.sendRawTransaction = async (wire) => { sent.push(wire); return transactionSignatureV1(f.transaction.signatures[0]!); };
    await submitGeneralOperationV5(f.store, f.client, retained, f.inspection.plan.tradingProgram);
    expect(sent).toHaveLength(2); expect(sent[1]).toEqual(sent[0]);
  });
  it('refuses edited plan/preview and substituted signed messages exactly', async () => {
    const f = await setup();
    await expect(retainGeneralOperationV5(storage(), f.scope, f.nativePlan, { ...f.preview, messageDigest: '00'.repeat(32) })).rejects.toThrow('General preview belongs to another native message');
    const altered = f.transaction; altered.message.recentBlockhash = f.scope.owner; altered.sign([f.payer]);
    await expect(submitGeneralOperationV5(f.store, f.client, f.journal, f.inspection.plan.tradingProgram, altered)).rejects.toThrow('General signed transaction differs from the retained native plan');
    await expect(restoreGeneralOperationV5({ ...f.journal, intent: f.journal.intent.replace('open-batch', 'close-batch') })).rejects.toThrow('General journal substituted scope');
  });
  it('keeps the signed journal until a verified receipt archive has been durably written', async () => {
    const f = await setup(); f.transaction.sign([f.payer]);
    const signature = transactionSignatureV1(f.transaction.signatures[0]!);
    f.client.sendRawTransaction = async () => signature;
    const retained = await submitGeneralOperationV5(f.store, f.client, f.journal, f.inspection.plan.tradingProgram, f.transaction);
    f.client.transaction = async () => null;
    await expect(archiveGeneralOperationV5(f.store, f.client, retained, f.inspection.plan.tradingProgram)).rejects.toThrow('General archive requires finalized execution and matching root poststate');
    const ack = await generalOperationReceiptTestFixtureV5(f);
    f.client.transaction = async () => ({ signature, slot: f.preview.observedSlot, succeeded: true, transactionBytes: f.transaction.serialize(), returnData: { programId: f.inspection.plan.tradingProgram, data: ack } }) as TransactionMetaObservation;
    const write = f.store.setItem;
    f.store.setItem = (key, value) => { if (key.startsWith('dclutch.general-finalized-root.')) throw new Error('archive storage full'); write(key, value); };
    await expect(archiveGeneralOperationV5(f.store, f.client, retained, f.inspection.plan.tradingProgram)).rejects.toThrow('archive storage full');
    expect((await findClientOperationJournalV1(f.store, f.scope, 'general-v5'))?.phase).toBe('submitted');
    f.store.setItem = write;
    await archiveGeneralOperationV5(f.store, f.client, retained, f.inspection.plan.tradingProgram);
    expect(await findClientOperationJournalV1(f.store, f.scope, 'general-v5')).toBeNull();
    const archive = JSON.parse(f.store.getItem(f.store.key(0)!)!);
    expect(archive.journal.signedWireBase64).toBe(retained.signedWireBase64);
    expect(archive.receipt.rootPoststateDigest).toBe(f.inspection.plan.rootPrestateDigest);
  });

});
