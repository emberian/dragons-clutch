import { describe, expect, it, vi } from 'vitest';

import { SOLANA_DEVNET_GENESIS_HASH_V1, type MutationClusterAdmissionV1 } from '@dclutch/sdk/rpc';

import { resolveContext } from '../src/context';
import {
  assertExactDevnetMutation,
  devnetGenesisAcknowledgment,
  latestExactDevnetBlockhash,
} from '../src/mutation';

const DEVNET = SOLANA_DEVNET_GENESIS_HASH_V1;
const MAINNET = '5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d';
const LOCAL = '7VEhQrEhpF1R8H3qY4sHvY4UFAYYuVAE72SFPFsjnGFc';

function admission(genesisHash: string, kind: MutationClusterAdmissionV1['kind']): MutationClusterAdmissionV1 {
  return Object.freeze({ endpoint: 'http://127.0.0.1:8899/', genesisHash, kind });
}

describe('exact devnet mutation admission', () => {
  it('requires the full known devnet hash before consulting the endpoint', async () => {
    const assertMutationCluster = vi.fn(async () => admission(DEVNET, 'devnet'));
    await expect(assertExactDevnetMutation({ assertMutationCluster }, LOCAL, 'buy signature')).rejects.toThrow(/did not acknowledge.*exact genesis hash/);
    expect(assertMutationCluster).not.toHaveBeenCalled();

    const missing = resolveContext({}, {});
    expect(() => devnetGenesisAcknowledgment(missing)).toThrow(/pass --i-mean-devnet/);
    const wrong = resolveContext({ 'i-mean-devnet': LOCAL }, {});
    expect(() => devnetGenesisAcknowledgment(wrong)).toThrow(/must equal Solana devnet's full genesis hash/);
  });

  it('preserves the SDK mainnet refusal and never admits the mutation boundary', async () => {
    const assertMutationCluster = vi.fn(async (): Promise<MutationClusterAdmissionV1> => {
      throw new Error(`mutation refused: the endpoint reports Solana mainnet-beta genesis ${MAINNET}`);
    });
    await expect(assertExactDevnetMutation({ assertMutationCluster }, DEVNET, 'sell signature')).rejects.toThrow(/mainnet-beta/);
    expect(assertMutationCluster).toHaveBeenCalledOnce();
  });

  it('refuses the SDK local-validator allowance for the public trader CLI', async () => {
    const assertMutationCluster = vi.fn(async () => admission(LOCAL, 'loopback-local-validator'));
    await expect(assertExactDevnetMutation({ assertMutationCluster }, DEVNET, 'walk signature')).rejects.toThrow(/no longer reports the exact acknowledged devnet genesis/);
  });

  it('reacquires identity and closes a stale devnet-to-wrong-genesis substitution', async () => {
    const observations = [admission(DEVNET, 'devnet'), admission(LOCAL, 'loopback-local-validator')];
    const assertMutationCluster = vi.fn(async () => {
      const next = observations.shift();
      if (next === undefined) throw new Error('unexpected admission call');
      return next;
    });
    await expect(assertExactDevnetMutation({ assertMutationCluster }, DEVNET, 'trade preparation')).resolves.toMatchObject({ genesisHash: DEVNET });
    await expect(assertExactDevnetMutation({ assertMutationCluster }, DEVNET, 'trade signature')).rejects.toThrow(/no longer reports the exact acknowledged devnet genesis/);
    expect(assertMutationCluster).toHaveBeenCalledTimes(2);
  });

  it('does not request a blockhash after an exact-genesis refusal', async () => {
    const latestMutationBlockhash = vi.fn(async () => Object.freeze({ slot: '9', blockhash: LOCAL, lastValidBlockHeight: '10' }));
    const client = {
      assertMutationCluster: vi.fn(async () => admission(LOCAL, 'loopback-local-validator')),
      latestMutationBlockhash,
    };
    await expect(latestExactDevnetBlockhash(client, DEVNET, 'walk blockhash')).rejects.toThrow(/exact acknowledged devnet genesis/);
    expect(latestMutationBlockhash).not.toHaveBeenCalled();
  });
});
