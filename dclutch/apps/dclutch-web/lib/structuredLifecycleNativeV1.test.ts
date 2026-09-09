import { PublicKey, SystemProgram } from '@solana/web3.js';
import { describe, expect, it, vi } from 'vitest';
import { compileStructuredLifecycleStepV1, discoverStructuredLifecycleV1, reconstructStructuredLifecycleReviewV1, structuredBase64V1, type StructuredLifecycleNativeV1, type StructuredNativeInputV1, type StructuredNativePlanV1 } from './structuredLifecycleNativeV1';
import { type StructuredLifecycleIntentV1 } from './structuredLifecycleModelV1';
import { type SolanaRpcClient } from '@dclutch/sdk/rpc';

const key = (n: number) => new PublicKey(new Uint8Array(32).fill(n)).toBase58();
const intent: StructuredLifecycleIntentV1 = { market: key(1), payer: key(2), action: 'activate-receipt', coordinate: null, selectedCapability: null, representationDescriptor: null, expectedPosition: null };
const output = (result: unknown) => JSON.stringify({ format: 'dclutch-structured-lifecycle-planning-v1', result });
const absent = (address: string) => ({ address, account: null });
const account = { owner: key(6), lamports: '9007199254740993', executable: false, space: 2, data: new Uint8Array([7, 8]) };
function fixture() {
  const client = {
    assertMutationCluster: vi.fn(async () => ({ genesisHash: key(20), endpoint: 'http://localhost:8899', kind: 'loopback-local-validator' as const })),
    finalizedSlot: vi.fn(async () => '40'),
    multipleAccounts: vi.fn(async (addresses: string[]) => ({ slot: '42', accounts: addresses.map((address) => address === key(3) ? { address, account } : absent(address)) })),
    multipleAccountDataSlices: vi.fn(async (addresses: string[]) => ({ slot: '42', accounts: addresses.map((address) => ({ address, account: { ...account, space: 100 } })) })),
    programAccountsFiltered: vi.fn(async () => ({ slot: '41', accounts: [{ address: key(3), account }] })),
  };
  return client;
}
describe('Structured native account acquisition', () => {
  it('reacquires the entire point corpus after native filtered discovery and preserves absence and header windows', async () => {
    const client = fixture(); const inputs: StructuredNativeInputV1[] = [];
    const native: StructuredLifecycleNativeV1 = {
      plan_structured_lifecycle_v1: (text) => {
        const input = JSON.parse(text) as StructuredNativeInputV1; inputs.push(input);
        if (inputs.length === 1) return output({ kind: 'discover', requests: [{ address: key(1), dataSlice: null }, { address: key(4), dataSlice: { offset: 0, length: 2 } }], scans: [] });
        if (inputs.length === 2) return output({ kind: 'discover', requests: [{ address: key(3), dataSlice: null }], scans: [{ program: key(6), dataSize: null, memcmp: [{ offset: 0, bytesBase64: structuredBase64V1(new Uint8Array([7])) }], dataSlice: null }] });
        return output({ kind: 'select', capabilities: [] });
      }, verify_structured_lifecycle_poststates_v1: () => { throw new Error('unused'); },
    };
    const result = await discoverStructuredLifecycleV1(client as unknown as SolanaRpcClient, native, intent, new Uint8Array([90]));
    expect(result.clusterGenesis).toBe(key(20)); expect(inputs[2]!.snapshot.slot).toBe('42');
    expect(client.multipleAccounts.mock.calls.map((row) => row[0])).toEqual([[key(1)], [key(1), key(3)]]);
    expect(inputs[2]!.snapshot.accounts[0]!.value).toBeNull();
    expect(inputs[2]!.snapshot.accounts[1]).toEqual({ request: { address: key(4), dataSlice: { offset: 0, length: 2 } }, value: { owner: key(6), lamports: '9007199254740993', executable: false, space: '100', dataBase64: 'Bwg=' } });
    expect(inputs[2]!.snapshot.scans[0]!.addresses).toEqual([key(3)]);
    expect(client.programAccountsFiltered).toHaveBeenCalledWith(key(6), { memcmp: [{ offset: 0, bytes: new Uint8Array([7]) }] }, '42');
  });
  it('invalidates an in-flight discovery before a stale native result can become a review', async () => {
    const client = fixture(); let current = true;
    client.finalizedSlot.mockImplementation(async () => { current = false; return '40'; });
    const native = { plan_structured_lifecycle_v1: vi.fn(), verify_structured_lifecycle_poststates_v1: vi.fn() };
    await expect(discoverStructuredLifecycleV1(client as unknown as SolanaRpcClient, native, intent, new Uint8Array(), () => { if (!current) throw new Error('Selection changed'); })).rejects.toThrow('Selection changed');
    expect(native.plan_structured_lifecycle_v1).not.toHaveBeenCalled();
  });
  it('requires discovery progress and refuses conflicting native account windows', async () => {
    const client = fixture();
    const native = { plan_structured_lifecycle_v1: () => output({ kind: 'discover', requests: [], scans: [] }), verify_structured_lifecycle_poststates_v1: vi.fn() };
    await expect(discoverStructuredLifecycleV1(client as unknown as SolanaRpcClient, native, intent, new Uint8Array())).rejects.toThrow('Structured native discovery made no progress');
    let round = 0; native.plan_structured_lifecycle_v1 = () => output({ kind: 'discover', requests: [{ address: key(4), dataSlice: { offset: round++, length: 2 } }], scans: [] });
    await expect(discoverStructuredLifecycleV1(client as unknown as SolanaRpcClient, native, intent, new Uint8Array())).rejects.toThrow('Structured native discovery requested conflicting account windows');
  });
});

describe('Structured native packet reconstruction', () => {
  it('refuses a changed instruction packet even when the native plan and payer still match', () => {
    const instruction = SystemProgram.transfer({ fromPubkey: new PublicKey(intent.payer), toPubkey: new PublicKey(key(5)), lamports: 1 });
    const plan: StructuredNativePlanV1 = { intent, stepId: '11'.repeat(32), stepKind: 'fund-rent', selectedCapability: '22'.repeat(32), finalizedSlot: '42', instructions: [{ programId: instruction.programId.toBase58(), accounts: instruction.keys.map((meta) => ({ address: meta.pubkey.toBase58(), isSigner: meta.isSigner, isWritable: meta.isWritable })), dataBase64: structuredBase64V1(instruction.data) }], requiredWalletSigners: [intent.payer], preview: { receiptMint: key(7), coordinate: null, position: null, preparationLamports: '1', returnedRentLamports: '0', rentRecipient: null, receiptSupplyBefore: '0', receiptSupplyAfter: '0' }, expectedPoststates: [] };
    const input: StructuredNativeInputV1 = { format: 'dclutch-structured-lifecycle-input-v1', intent, checkedInfrastructureBase64: '', snapshot: { slot: '42', accounts: [], scans: [] } };
    const native = { plan_structured_lifecycle_v1: () => output({ kind: 'ready', plan }), verify_structured_lifecycle_poststates_v1: vi.fn() };
    const transaction = compileStructuredLifecycleStepV1(plan, key(30), []);
    reconstructStructuredLifecycleReviewV1(native, input, plan, transaction, []);
    transaction.message.compiledInstructions[0]!.data[4] = 2;
    expect(() => reconstructStructuredLifecycleReviewV1(native, input, plan, transaction, [])).toThrow('Structured saved packet differs from native instructions');
  });
});
