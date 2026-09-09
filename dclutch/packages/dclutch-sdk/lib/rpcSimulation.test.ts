import { PublicKey, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';
import { SolanaRpcClient } from './rpc';
const address = new PublicKey(new Uint8Array(32).fill(7)).toBase58();
const packet = new VersionedTransaction(new TransactionMessage({ payerKey: new PublicKey(address), recentBlockhash: address, instructions: [] }).compileToV0Message()).serialize();
function client(value: unknown, slot = 10, inspect?: (params: unknown[]) => void) {
  return new SolanaRpcClient('http://127.0.0.1:8899', async (_input, init) => {
    const request = JSON.parse(String(init?.body)); expect(request.method).toBe('simulateTransaction'); inspect?.(request.params);
    return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result: { context: { slot }, value } }));
  });
}
describe('bounded unchanged-packet simulation', () => {
  it('retains exact bytes, blockhash, account order, return data and native CU', async () => {
    const value = await client({ err: null, unitsConsumed: 42, logs: [' native '], returnData: { programId: address, data: ['AQ==', 'base64'] }, accounts: [null] }, 10, (params) => {
      expect(params).toEqual([Buffer.from(packet).toString('base64'), { encoding: 'base64', commitment: 'finalized', sigVerify: false, replaceRecentBlockhash: false, accounts: { encoding: 'base64', addresses: [address] }, minContextSlot: 9 }]);
    }).simulateTransaction(packet, [address], '9');
    expect(value).toMatchObject({ succeeded: true, slot: '10', computeUnits: '42', accounts: [{ address, account: null }], logMessages: [' native '], returnData: { programId: address, data: new Uint8Array([1]) } });
  });
  it('retains a runtime refusal separately from absent poststates', async () => {
    const err = 'ComputationalBudgetExceeded';
    await expect(client({ err, unitsConsumed: 1_400_000, accounts: null }).simulateTransaction(packet, [address])).resolves.toMatchObject({ succeeded: false, error: err, computeUnits: '1400000' });
  });
  it('refuses missing execution status, mismatched poststate count and stale context exactly', async () => {
    await expect(client({ accounts: [null] }).simulateTransaction(packet, [address])).rejects.toThrow('simulation omitted its execution result');
    await expect(client({ err: null, accounts: [] }).simulateTransaction(packet, [address])).rejects.toThrow('simulation did not return one poststate per requested account');
    await expect(client({ err: null, accounts: [null] }, 8).simulateTransaction(packet, [address], '9')).rejects.toThrow('simulation context precedes the requested floor');
  });
  it('refuses duplicate accounts and oversized packets before network access', async () => {
    const rpc = new SolanaRpcClient('http://127.0.0.1:8899', async () => { throw new Error('unexpected network'); });
    await expect(rpc.simulateTransaction(packet, [address, address])).rejects.toThrow('simulation requires 1..32 distinct account addresses');
    await expect(rpc.simulateTransaction(new Uint8Array(1233), [address])).rejects.toThrow('simulation packet exceeds the Solana packet bound');
  });
});
