import { describe, expect, it } from 'vitest';
import { generalOperationReceiptTestFixtureV5, generalOperationTestFixtureV5 } from '../fixtures/general-operation-test-v5';
import * as Abi from './generated/generalSuccessorV5';
import { hex, sha256 } from './bytes';
import { observeGeneralExecutionV5 } from './generalExecutionV5';
import { type SolanaRpcClient, type TransactionMetaObservation } from './rpc';

async function setup() {
  const fixture = await generalOperationTestFixtureV5(); fixture.transaction.sign([fixture.payer]);
  const { inspection } = fixture; const plan = inspection.plan;
  const ack = await generalOperationReceiptTestFixtureV5(fixture);
  const receipt = { signature: 'retained signature', slot: plan.observedSlot.toString(), succeeded: true, errorText: null, transactionBytes: fixture.transaction.serialize(), returnData: { programId: plan.tradingProgram, data: ack } } as unknown as TransactionMetaObservation;
  const client = { transaction: async () => receipt, multipleAccounts: async () => ({ slot: receipt.slot, accounts: [{ address: plan.root, account: fixture.root }, { address: plan.lifecycle.primary.account, account: null }] }) } as unknown as SolanaRpcClient;
  return { ...fixture, client, receipt, ack };
}
describe('General finalized receipt/root proof', () => {
  it('joins exact signed bytes, authenticated receipt and the observed root digest', async () => {
    const f = await setup();
    const observed = await observeGeneralExecutionV5(f.client, f.inspection, f.receipt.signature, f.transaction.serialize());
    expect(observed?.receipt.rootPoststateDigest).toBe(hex(await sha256(f.root.data)));
    expect(observed?.lifecycle).toEqual([{ address: f.inspection.plan.lifecycle.primary.account, state: 'absent' }]);
  });
  it('refuses another packet, receipt producer, request digest and changed root bytes exactly', async () => {
    const f = await setup();
    f.client.transaction = async () => ({ ...f.receipt, transactionBytes: new Uint8Array([1]) });
    await expect(observeGeneralExecutionV5(f.client, f.inspection, f.receipt.signature, f.transaction.serialize())).rejects.toThrow('General finalized transaction differs from the retained signed bytes');
    f.client.transaction = async () => ({ ...f.receipt, returnData: { programId: f.payer.publicKey.toBase58(), data: f.ack } });
    await expect(observeGeneralExecutionV5(f.client, f.inspection, f.receipt.signature, f.transaction.serialize())).rejects.toThrow('General finalized transaction omitted the Trading receipt');
    f.client.transaction = async () => f.receipt; f.ack[Abi.GENERAL_ACK_REQUEST_DIGEST_OFFSET_V3] ^= 1;
    await expect(observeGeneralExecutionV5(f.client, f.inspection, f.receipt.signature, f.transaction.serialize())).rejects.toThrow('General Hot receipt belongs to another request');
    f.ack[Abi.GENERAL_ACK_REQUEST_DIGEST_OFFSET_V3] ^= 1; f.root.data[0] ^= 1;
    await expect(observeGeneralExecutionV5(f.client, f.inspection, f.receipt.signature, f.transaction.serialize())).rejects.toThrow('General finalized root poststate is unavailable or has advanced');
  });
  it('keeps unavailable finalization distinct from a finalized runtime refusal', async () => {
    const f = await setup(); f.client.transaction = async () => null;
    await expect(observeGeneralExecutionV5(f.client, f.inspection, f.receipt.signature, f.transaction.serialize())).resolves.toBeNull();
    f.client.transaction = async () => ({ ...f.receipt, succeeded: false, errorText: 'ComputationalBudgetExceeded' });
    await expect(observeGeneralExecutionV5(f.client, f.inspection, f.receipt.signature, f.transaction.serialize())).rejects.toThrow('General finalized refusal: ComputationalBudgetExceeded');
  });
});
