import { describe, expect, it } from 'vitest';

import {
  PUBLIC_DEVNET_CUT_V1,
  parsePublicDevnetCutV1,
  publicCutExplorerHrefV1,
  publicCutMarketHrefV1,
} from './publicCutStaging';

describe('public devnet cut staging', () => {
  it('keeps the published cut honestly pending until one manifest update names a Market', () => {
    expect(PUBLIC_DEVNET_CUT_V1.market).toBeNull();
    expect(publicCutMarketHrefV1()).toBe('/markets');
    expect(publicCutExplorerHrefV1()).toBe('/explorer');
  });

  it('refuses activity without a Market and unknown manifest fields', () => {
    expect(() => parsePublicDevnetCutV1({ schema: 'dclutch-public-cut-v1', cluster: 'devnet', market: null, activity: { found: 'a'.repeat(64), trade: null, resolve: null, redeem: null } })).toThrow(/pending/);
    expect(() => parsePublicDevnetCutV1({ schema: 'dclutch-public-cut-v1', cluster: 'devnet', market: null, activity: { found: null, trade: null, resolve: null, redeem: null }, extra: true })).toThrow(/unknown/);
  });
});
