import { describe, expect, it } from 'vitest';

import {
  aquariumLiveRefreshAllowedV1,
  aquariumLiveShownStateV1,
  type AquariumLivePanelStateV1,
} from './AquariumLiveChainPanel';
import { type AquariumLiveObservationV1, type AquariumLiveSelectionV1 } from '@/lib/aquariumLiveChain';

const FIRST = 'GtmpRvSL9y6RpqMth73VSdb9h1XRe7zqQZkhJkfgxKrA';
const SECOND = 'GyD95eyERwRfwj8fSFNhWjKF2eaDg5XcREidPKex65zY';

function selected(address: string): Extract<AquariumLiveSelectionV1, Readonly<{ kind: 'selected' }>> {
  return Object.freeze({ kind: 'selected', address, cohortNumber: 18, manifestSha256: 'a'.repeat(64), checkedReleaseSetIds: Object.freeze(['b'.repeat(64)]) });
}

function observation(address: string): AquariumLiveObservationV1 {
  return { address, detail: { address, floorSlot: '500', card: { status: 'decoded', phase: 'Open' } }, facts: { endpoint: 'https://api.devnet.solana.com', genesisHash: 'devnet', solanaCore: 'x', featureSet: null }, signatures: [], observedAt: '2026-09-07T19:00:00.000Z' } as unknown as AquariumLiveObservationV1;
}

describe('aquarium live-watch lifecycle', () => {
  it('drops prior-market facts when the snapshot selects another address and only permits visible non-overlapping reads', () => {
    const previous: AquariumLivePanelStateV1 = Object.freeze({ kind: 'ready', reason: null, observation: observation(FIRST) });
    expect(aquariumLiveShownStateV1(selected(SECOND), previous)).toEqual({ kind: 'loading', reason: null, observation: null });
    expect(aquariumLiveRefreshAllowedV1(false, false)).toBe(false);
    expect(aquariumLiveRefreshAllowedV1(true, true)).toBe(false);
    expect(aquariumLiveRefreshAllowedV1(true, false)).toBe(true);
  });
});
