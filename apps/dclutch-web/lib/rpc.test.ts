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
});
