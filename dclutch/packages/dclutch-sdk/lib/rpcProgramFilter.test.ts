import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';
import { SolanaRpcClient } from './rpc';

const key = (seed: number) => new PublicKey(new Uint8Array(32).fill(seed)).toBase58();
const program = key(1); const address = key(2);
const account = { owner: program, executable: false, lamports: 1, space: 6, data: ['AQIDBAUG', 'base64'] };
function client(value: unknown, slot = 10, inspect?: (params: unknown[]) => void) {
  return new SolanaRpcClient('http://127.0.0.1:8899', async (_url, init) => {
    const request = JSON.parse(String(init?.body)); expect(request.method).toBe('getProgramAccounts'); inspect?.(request.params);
    return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result: { context: { slot }, value } }));
  });
}
const filters = { memcmp: [{ offset: 2, bytes: new Uint8Array([3, 4]) }] };

describe('native-authored bounded program inventory', () => {
  it('forwards native byte filters and verifies full and sliced responses', async () => {
    const full = await client([{ pubkey: address, account }], 10, (params) => {
      expect(params).toEqual([program, { commitment: 'finalized', encoding: 'base64', withContext: true,
        filters: [{ memcmp: { offset: 2, bytes: 'AwQ=', encoding: 'base64' } }], minContextSlot: 9 }]);
    }).programAccountsFiltered(program, filters, '9');
    expect(full.accounts[0]!.account!.data).toEqual(new Uint8Array([1, 2, 3, 4, 5, 6]));
    const sliced = await client([{ pubkey: address, account: { ...account, data: ['AwQ=', 'base64'] } }]).programAccountsFiltered(program, { ...filters, dataSize: 6, dataSlice: { offset: 2, length: 2 } });
    expect(sliced.accounts[0]!.account!.space).toBe(6);
    expect(sliced.accounts[0]!.account!.data).toEqual(new Uint8Array([3, 4]));
  });

  it('refuses wrong filter bytes, owners, widths and stale or duplicate responses exactly', async () => {
    await expect(client([{ pubkey: address, account: { ...account, data: ['AQIDAAUG', 'base64'] } }]).programAccountsFiltered(program, filters)).rejects.toThrow('filtered program scan returned bytes outside its requested filters');
    await expect(client([{ pubkey: address, account: { ...account, owner: key(3) } }]).programAccountsFiltered(program, filters)).rejects.toThrow('filtered program scan returned another owner or account width');
    await expect(client([{ pubkey: address, account }]).programAccountsFiltered(program, { ...filters, dataSize: 7 })).rejects.toThrow('filtered program scan returned another owner or account width');
    await expect(client([], 8).programAccountsFiltered(program, filters, '9')).rejects.toThrow('filtered program scan precedes the requested slot');
    await expect(client([{ pubkey: address, account }, { pubkey: address, account }]).programAccountsFiltered(program, filters)).rejects.toThrow('filtered program scan repeated or changed a canonical address');
  });

  it('refuses unbounded or unverifiable filters before requesting RPC', async () => {
    const rpc = new SolanaRpcClient('http://127.0.0.1:8899', async () => { throw new Error('unexpected network'); });
    await expect(rpc.programAccountsFiltered(program, { memcmp: [] })).rejects.toThrow('filtered program scan requires a width or 1..4 byte filters');
    await expect(rpc.programAccountsFiltered(program, { ...filters, dataSlice: { offset: 0, length: 2 } })).rejects.toThrow('filtered scan byte comparison must fit its returned data window');
    await expect(rpc.programAccountsFiltered(program, { memcmp: [{ offset: 0, bytes: new Uint8Array(129) }] })).rejects.toThrow('filtered scan byte comparison must fit its returned data window');
  });
});
