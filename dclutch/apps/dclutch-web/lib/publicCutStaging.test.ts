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
    //
    // Pinned ONCE and reused. It used to be pinned twice, and when the cut
    // moved to the measured-volatility market the fixture changed and only
    // the fixture did -- so the literal below and the href literal beside it
    // disagreed with the shipped fixture and with each other.
    const MARKET = '8Xky2yx3wBmDRXeNfKSuJigqiWDtwSvGvB75BSW6tPxK';
    expect(PUBLIC_DEVNET_CUT_V1.market).toBe(MARKET);
    expect(PUBLIC_DEVNET_CUT_V1.activity.found).not.toBeNull();
    // It has now TRADED: the first public fill this protocol ever landed,
    // slot 490,907,340. Resolve and redeem are still genuinely null, and the
    // ladder says so rather than borrowing a signature from another step.
    expect(PUBLIC_DEVNET_CUT_V1.activity.trade).toBe(
      '4YQLY9tsRRVnxMJBcHdjGFZ6mVGY7nynjhnpUYyQX7EaSm9RufDrKit5GYqah88qcnHwtAzwaEBdFL4brcBRzPzX',
    );
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
