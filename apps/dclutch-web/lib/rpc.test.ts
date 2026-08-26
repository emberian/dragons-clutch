import { describe, expect, it } from 'vitest';

import { SolanaRpcClient } from './rpc';

function response(result: unknown): Response {
  return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result }), {
    headers: { 'content-type': 'application/json' },
    status: 200,
  });
}

describe('bounded finalized RPC client', () => {
  it('probes only the selected real endpoint surface', async () => {
    const fetcher: typeof fetch = async (_input, init) => {
      const request = JSON.parse(String(init?.body)) as { method: string };
      if (request.method === 'getVersion') return response({ 'solana-core': '4.2.1', 'feature-set': 123 });
      if (request.method === 'getGenesisHash') return response('EtWTRABZaYq6iMfeYKouRu166VU2xqa1');
      throw new Error(`unexpected method ${request.method}`);
    };
    const client = new SolanaRpcClient('http://127.0.0.1:8899', fetcher);
    await expect(client.probe()).resolves.toEqual({
      endpoint: 'http://127.0.0.1:8899/',
      genesisHash: 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1',
      solanaCore: '4.2.1',
      featureSet: '123',
    });
  });

  it('refuses non-HTTP transports and inexact numeric account facts', async () => {
    expect(() => new SolanaRpcClient('ws://127.0.0.1:8900')).toThrow('http or https');
    const fetcher: typeof fetch = async () => response({
      context: { slot: 4 },
      value: {
        data: ['', 'base64'],
        executable: false,
        lamports: Number.MAX_SAFE_INTEGER + 1,
        owner: '11111111111111111111111111111111',
        space: 0,
      },
    });
    await expect(new SolanaRpcClient('http://127.0.0.1:8899', fetcher).accountInfo('11111111111111111111111111111111')).rejects.toThrow('exact safe unsigned');
  });

  it('acquires a finalized recent blockhash above the selected snapshot floor', async () => {
    const blockhash = '11111111111111111111111111111111';
    const fetcher: typeof fetch = async (_input, init) => {
      const request = JSON.parse(String(init?.body)) as { method: string; params: unknown[] };
      expect(request.method).toBe('getLatestBlockhash');
      expect(request.params).toEqual([{ commitment: 'finalized', minContextSlot: 44 }]);
      return response({ context: { slot: 45 }, value: { blockhash, lastValidBlockHeight: 72 } });
    };
    await expect(new SolanaRpcClient('http://127.0.0.1:8899', fetcher).latestBlockhash('44')).resolves.toEqual({
      slot: '45', blockhash, lastValidBlockHeight: '72',
    });
  });

  it('reports the finalized rent obligation for an exact account width', async () => {
    const fetcher: typeof fetch = async (_input, init) => {
      const request = JSON.parse(String(init?.body)) as { method: string; params: unknown[] };
      expect(request.method).toBe('getMinimumBalanceForRentExemption');
      expect(request.params).toEqual([232, { commitment: 'finalized' }]);
      return response(2_503_680);
    };
    await expect(new SolanaRpcClient('http://127.0.0.1:8899', fetcher).minimumBalanceForRentExemption(232)).resolves.toEqual({
      dataLength: 232, lamports: '2503680',
    });
  });

  it('acquires distinct accounts in one finalized RPC context', async () => {
    const addresses = ['11111111111111111111111111111111', 'SysvarC1ock11111111111111111111111111111111'];
    const fetcher: typeof fetch = async (_input, init) => {
      const request = JSON.parse(String(init?.body)) as { method: string; params: unknown[] };
      expect(request.method).toBe('getMultipleAccounts');
      expect(request.params).toEqual([addresses, { commitment: 'finalized', encoding: 'base64', minContextSlot: 44 }]);
      return response({ context: { slot: 45 }, value: [null, { data: ['', 'base64'], executable: false, lamports: 1, owner: addresses[0], space: 0 }] });
    };
    await expect(new SolanaRpcClient('http://127.0.0.1:8899', fetcher).multipleAccounts(addresses, '44')).resolves.toMatchObject({
      slot: '45', accounts: [{ address: addresses[0], account: null }, { address: addresses[1], account: { lamports: '1' } }],
    });
  });

  it('submits only one bounded caller-signed packet with preflight enabled', async () => {
    const signature = '2'.repeat(88);
    const fetcher: typeof fetch = async (_input, init) => {
      const request = JSON.parse(String(init?.body)) as { method: string; params: unknown[] };
      expect(request.method).toBe('sendTransaction');
      expect(request.params).toEqual([btoa(String.fromCharCode(1, 2, 3)), {
        encoding: 'base64', skipPreflight: false, preflightCommitment: 'confirmed', maxRetries: 3,
      }]);
      return response(signature);
    };
    await expect(new SolanaRpcClient('http://127.0.0.1:8899', fetcher).sendRawTransaction(Uint8Array.from([1, 2, 3]))).resolves.toBe(signature);
    await expect(new SolanaRpcClient('http://127.0.0.1:8899', fetcher).sendRawTransaction(new Uint8Array(1_233))).rejects.toThrow(/1..1232/);
  });
});
