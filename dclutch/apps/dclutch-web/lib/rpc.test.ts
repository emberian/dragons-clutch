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

  /**
   * Every other case in this file INJECTS a fetcher, so the default path — the
   * only one the product uses — was never executed here. It was broken:
   * `this.fetcher(...)` called the ambient `fetch` with the client as receiver,
   * which Chromium refuses outright. Measured against a live localhost
   * validator on 2026-08-27: every read surface in the app answered
   * `Refused: Failed to execute 'fetch' on 'Window': Illegal invocation`.
   *
   * The stub below enforces the browser's rule inside a runtime that does not,
   * so the regression fails here instead of only on a chain.
   */
  it('calls the ambient fetch with a receiver a browser accepts', async () => {
    const original = globalThis.fetch;
    const seen: unknown[] = [];
    globalThis.fetch = function browserFetch(this: unknown, _input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
      seen.push(this);
      if (this !== undefined && this !== globalThis) {
        throw new TypeError("Failed to execute 'fetch' on 'Window': Illegal invocation");
      }
      const request = JSON.parse(String(init?.body)) as { method: string };
      if (request.method === 'getVersion') return Promise.resolve(response({ 'solana-core': '4.0.2' }));
      return Promise.resolve(response('EtWTRABZaYq6iMfeYKouRu166VU2xqa1'));
    } as typeof fetch;
    try {
      await expect(new SolanaRpcClient('http://127.0.0.1:20890/').probe()).resolves.toMatchObject({ solanaCore: '4.0.2' });
      expect(seen.length).toBe(2);
      for (const receiver of seen) expect(receiver === undefined || receiver === globalThis).toBe(true);
    } finally {
      globalThis.fetch = original;
    }
  });

  it('reads the node signature history with exact bounds and canonical text', async () => {
    const signature = '5'.repeat(88);
    const fetcher: typeof fetch = async (_input, init) => {
      const request = JSON.parse(String(init?.body)) as { method: string; params: unknown[] };
      expect(request.method).toBe('getSignaturesForAddress');
      expect(request.params[1]).toEqual({ commitment: 'finalized', limit: 3 });
      return response([
        { signature, slot: 90, err: null, blockTime: 1790000000, memo: null },
        { signature: '4'.repeat(88), slot: 88, err: { InstructionError: [1, 'Custom'] }, blockTime: null },
      ]);
    };
    const client = new SolanaRpcClient('http://127.0.0.1:8899', fetcher);
    const records = await client.signaturesForAddress('11111111111111111111111111111111', 3);
    expect(records).toHaveLength(2);
    expect(records[0]).toMatchObject({ signature, slot: '90', succeeded: true, errorText: null, blockTime: '1790000000' });
    expect(records[1]).toMatchObject({ slot: '88', succeeded: false });
    expect(records[1].errorText).toContain('InstructionError');
    await expect(client.signaturesForAddress('11111111111111111111111111111111', 0)).rejects.toThrow('1..50');
  });

  it('polls signature statuses one-for-one including unknown signatures', async () => {
    const known = '6'.repeat(88);
    const unknown = '7'.repeat(88);
    const fetcher: typeof fetch = async (_input, init) => {
      const request = JSON.parse(String(init?.body)) as { method: string; params: unknown[] };
      expect(request.method).toBe('getSignatureStatuses');
      expect(request.params[1]).toEqual({ searchTransactionHistory: true });
      return response({ context: { slot: 99 }, value: [{ slot: 91, confirmationStatus: 'finalized', err: null }, null] });
    };
    const statuses = await new SolanaRpcClient('http://127.0.0.1:8899', fetcher).signatureStatuses([known, unknown]);
    expect(statuses[0]).toEqual({ signature: known, known: true, slot: '91', confirmationStatus: 'finalized', succeeded: true, errorText: null });
    expect(statuses[1]).toEqual({ signature: unknown, known: false, slot: null, confirmationStatus: null, succeeded: null, errorText: null });
  });

  it('reads one finalized transaction as exact bytes, balances, and logs', async () => {
    const signature = '8'.repeat(88);
    const bytes = Uint8Array.from([1, 2, 3, 4]);
    let binary = '';
    for (const byte of bytes) binary += String.fromCharCode(byte);
    const fetcher: typeof fetch = async (_input, init) => {
      const request = JSON.parse(String(init?.body)) as { method: string; params: unknown[] };
      expect(request.method).toBe('getTransaction');
      expect(request.params[1]).toEqual({ commitment: 'finalized', encoding: 'base64', maxSupportedTransactionVersion: 0 });
      return response({
        slot: 92,
        blockTime: 1790000101,
        transaction: [btoa(binary), 'base64'],
        meta: { err: null, fee: 5000, preBalances: [10, 20], postBalances: [5, 25], logMessages: ['Program log: ok'] },
      });
    };
    const observation = await new SolanaRpcClient('http://127.0.0.1:8899', fetcher).transaction(signature);
    expect(observation).toMatchObject({
      signature, slot: '92', blockTime: '1790000101', succeeded: true, feeLamports: '5000',
      preBalances: ['10', '20'], postBalances: ['5', '25'], logMessages: ['Program log: ok'],
    });
    expect(Array.from(observation?.transactionBytes ?? [])).toEqual([1, 2, 3, 4]);
    // Bytes that are not one canonical versioned transaction decode no account list.
    expect(observation?.accountAddresses).toEqual([]);
  });

  it('reports an unserved transaction as null rather than inventing one', async () => {
    const fetcher: typeof fetch = async () => response(null);
    await expect(new SolanaRpcClient('http://127.0.0.1:8899', fetcher).transaction('9'.repeat(88))).resolves.toBeNull();
  });
});
