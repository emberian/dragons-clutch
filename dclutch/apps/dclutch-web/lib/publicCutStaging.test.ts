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
    // read and join today.
    //
    // Pinned ONCE and reused. It used to be pinned twice, and when the cut
    // moved to the measured-volatility market the fixture changed and only
    // the fixture did -- so the literal below and the href literal beside it
    // disagreed with the shipped fixture and with each other.
    const MARKET = 'EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1';
    expect(PUBLIC_DEVNET_CUT_V1.market).toBe(MARKET);
    // Every lifecycle signature is null, and that is the honest state rather
    // than an oversight: cohort-12's Found rides an address lookup table, so
    // the chain cannot be asked for it by the Market's address, and no fill
    // has executed on this cohort. A signature appears here when one has been
    // read back, never because a step is expected to have happened.
    expect(PUBLIC_DEVNET_CUT_V1.activity.found).toBeNull();
    expect(PUBLIC_DEVNET_CUT_V1.activity.trade).toBeNull();
    expect(PUBLIC_DEVNET_CUT_V1.activity.resolve).toBeNull();
    expect(PUBLIC_DEVNET_CUT_V1.activity.redeem).toBeNull();
    // The featured market is registry-named, so its permalink is the exported
    // per-market page that carries its own title and share card.
    expect(publicCutMarketHrefV1()).toBe(`/markets/${MARKET}`);
  });

  it('refuses activity without a Market and unknown manifest fields', () => {
    expect(() => parsePublicDevnetCutV1({ schema: 'dclutch-public-cut-v1', cluster: 'devnet', market: null, activity: { found: 'a'.repeat(64), trade: null, resolve: null, redeem: null } })).toThrow(/pending/);
    expect(() => parsePublicDevnetCutV1({ schema: 'dclutch-public-cut-v1', cluster: 'devnet', market: null, activity: { found: null, trade: null, resolve: null, redeem: null }, extra: true })).toThrow(/unknown/);
  });
});
