import { describe, expect, it } from 'vitest';

import { LOCAL_SUCCESSOR_CHECKPOINT, discoverLocalSuccessor } from './localSuccessor';
import { SolanaRpcClient } from './rpc';

const live = process.env.DCLUTCH_LIVE_SUCCESSOR_RPC === '1' ? it : it.skip;

describe('live immutable successor validator', () => {
  live('reacquires the complete finalized checkpoint without unexpected program state', async () => {
    const snapshot = await discoverLocalSuccessor(new SolanaRpcClient(LOCAL_SUCCESSOR_CHECKPOINT.network.rpc_url));
    expect(snapshot.facts.genesisHash).toBe(LOCAL_SUCCESSOR_CHECKPOINT.network.genesis_hash);
    expect(snapshot.exactAccounts).toBe(33);
    expect(snapshot.transactionCreatedAccounts).toBe(7);
    expect(snapshot.missingProgramAccounts).toEqual([]);
    expect(snapshot.unexpectedProgramAccounts).toEqual([]);
    expect(snapshot.transactions).toHaveLength(9);
    expect(snapshot.transactions.every((transaction) => transaction.rpcStatus !== 'mismatch')).toBe(true);
    expect(snapshot.rollbackCurrent).toBe(true);
  });
});
