import { describe, expect, it, vi } from 'vitest';

import example from '@/fixtures/aquarium-status-v1.example.json';
import { parseAquariumStatusV1 } from '@/lib/aquariumStatus';
import {
  AQUARIUM_LIVE_SIGNATURE_LIMIT_V1,
  observeAquariumLiveMarketV1,
  selectAquariumLiveMarketV1,
  type AquariumLiveSelectionV1,
} from './aquariumLiveChain';
import { type MarketDetailV1 } from '@dclutch/sdk/marketDetail';
import { SOLANA_DEVNET_GENESIS_HASH_V1, type SolanaRpcClient } from '@dclutch/sdk/rpc';

const ADDRESS = 'GtmpRvSL9y6RpqMth73VSdb9h1XRe7zqQZkhJkfgxKrA';
const RELEASE = 'a'.repeat(64);

const selection: Extract<AquariumLiveSelectionV1, Readonly<{ kind: 'selected' }>> = Object.freeze({
  kind: 'selected', address: ADDRESS, cohortNumber: 18, manifestSha256: 'b'.repeat(64), checkedReleaseSetIds: Object.freeze([RELEASE]),
});

function detail(release = RELEASE): MarketDetailV1 {
  return {
    address: ADDRESS,
    floorSlot: '500',
    reason: 'one market read',
    card: { status: 'decoded', phase: 'Open', identity: { selectedReleaseSetId: release } },
  } as unknown as MarketDetailV1;
}

function rpc() {
  return {
    probe: vi.fn(async () => Object.freeze({ endpoint: 'https://api.devnet.solana.com', genesisHash: SOLANA_DEVNET_GENESIS_HASH_V1, solanaCore: '2.3.9', featureSet: null })),
    signaturesForAddress: vi.fn(async () => Object.freeze(Array.from({ length: 12 }, (_, index) => Object.freeze({
      signature: `${'4'.repeat(87)}${(index % 9) + 1}`,
      slot: String(800 - index), succeeded: index !== 1, errorText: index === 1 ? 'refused' : null, blockTime: null, memo: null,
    })))),
  } as unknown as Pick<SolanaRpcClient, 'probe' | 'signaturesForAddress' | 'finalizedSlot' | 'multipleAccounts'>;
}

describe('aquarium live-chain observation', () => {
  it('keeps the unpublished cohort-18 placeholder off public RPC', () => {
    const selected = selectAquariumLiveMarketV1(parseAquariumStatusV1(example), [RELEASE]);
    expect(selected).toEqual({ kind: 'unavailable', reason: 'The checked cohort-18 inventory has not been published yet.' });
  });

  it('reads one checked market and asks the node for no more than eight finalized index rows', async () => {
    const client = rpc();
    const readMarket = vi.fn(async () => detail());
    const observed = await observeAquariumLiveMarketV1(client, selection, {
      readMarket,
      now: () => new Date('2026-09-07T19:00:00Z'),
    });
    expect(readMarket).toHaveBeenCalledOnce();
    expect((client.signaturesForAddress as ReturnType<typeof vi.fn>)).toHaveBeenCalledWith(ADDRESS, AQUARIUM_LIVE_SIGNATURE_LIMIT_V1);
    expect(observed.detail.floorSlot).toBe('500');
    expect(observed.observedAt).toBe('2026-09-07T19:00:00.000Z');
    expect(observed.signatures).toHaveLength(AQUARIUM_LIVE_SIGNATURE_LIMIT_V1);
  });

  it('does not present an unchecked market as live or request its signature history', async () => {
    const client = rpc();
    await expect(observeAquariumLiveMarketV1(client, selection, { readMarket: async () => detail('c'.repeat(64)) }))
      .rejects.toThrow('does not name a checked release set');
    expect(client.signaturesForAddress).not.toHaveBeenCalled();
  });
});
