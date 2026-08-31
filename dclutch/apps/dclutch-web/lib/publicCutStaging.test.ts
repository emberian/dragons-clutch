import { describe, expect, it } from 'vitest';

import {
  PUBLIC_DEVNET_CUT_V1,
  parsePublicDevnetCutV1,
  publicCutExplorerHrefV1,
  publicCutMarketHrefV1,
} from './publicCutStaging';

describe('public devnet cut staging', () => {
  it('routes a pending cut to the walking surfaces, and the open cut to its Market', () => {
    // The pending face, pinned as a literal now that the published fixture
    // names a Market: a cut with no Market walks the reader to /markets.
    const pending = parsePublicDevnetCutV1({
      schema: 'dclutch-public-cut-v1',
      cluster: 'devnet',
      market: null,
      activity: { found: null, trade: null, resolve: null, redeem: null },
    });
    expect(publicCutMarketHrefV1(pending)).toBe('/markets');
    expect(publicCutExplorerHrefV1(pending)).toBe('/explorer');
    // The published cut itself: the market this deployment can actually
    // trade, and the founding transaction that created its Market record.
    expect(PUBLIC_DEVNET_CUT_V1.market).toBe('6WZXJ7jBPPA3eFZPc8hQmmNsf3R4zAZN4DRZzfhcV7a4');
    expect(PUBLIC_DEVNET_CUT_V1.activity.found).not.toBeNull();
    // Nothing has traded, resolved, or redeemed on it: the ladder says so
    // rather than borrowing a signature from another step.
    expect(PUBLIC_DEVNET_CUT_V1.activity.trade).toBeNull();
    expect(PUBLIC_DEVNET_CUT_V1.activity.resolve).toBeNull();
    expect(PUBLIC_DEVNET_CUT_V1.activity.redeem).toBeNull();
    // The featured market is registry-named, so its permalink is the exported
    // per-market page that carries its own title and share card.
    expect(publicCutMarketHrefV1()).toBe(
      '/markets/6WZXJ7jBPPA3eFZPc8hQmmNsf3R4zAZN4DRZzfhcV7a4',
    );
  });

  it('refuses activity without a Market and unknown manifest fields', () => {
    expect(() => parsePublicDevnetCutV1({ schema: 'dclutch-public-cut-v1', cluster: 'devnet', market: null, activity: { found: 'a'.repeat(64), trade: null, resolve: null, redeem: null } })).toThrow(/pending/);
    expect(() => parsePublicDevnetCutV1({ schema: 'dclutch-public-cut-v1', cluster: 'devnet', market: null, activity: { found: null, trade: null, resolve: null, redeem: null }, extra: true })).toThrow(/unknown/);
  });
});
