import { describe, expect, it } from 'vitest';

import {
  checkedReleaseSetIdsV1,
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
      checkedReleases: {},
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

    // No execution release set on this deployment has a checked release, and
    // that is a STATED emptiness rather than a missing field: cohort-12 is a
    // full redeploy, and a full-redeploy cohort cannot produce the
    // authenticated permanent checked deployment set the release demands. The
    // trade spine reads this list and raises its `release` wall from it, so an
    // empty map here is what tells a trader the fill waits.
    expect(PUBLIC_DEVNET_CUT_V1.checkedReleases).toEqual({});
    expect(checkedReleaseSetIdsV1()).toEqual([]);
    // Null is the OTHER answer and means nobody consulted a record; a cut with
    // no Market describes no deployment and must not claim to know.
    expect(checkedReleaseSetIdsV1(pending)).toBeNull();
  });

  it('parses a checked release row, and refuses one that is not a pair of digests', () => {
    const releaseSet = 'a'.repeat(64);
    const cut = parsePublicDevnetCutV1({
      schema: 'dclutch-public-cut-v1', cluster: 'devnet', market: null,
      activity: { found: null, trade: null, resolve: null, redeem: null },
      checkedReleases: { [releaseSet]: { gateDigest: 'b'.repeat(64), sealedSet: 'c'.repeat(64) } },
    });
    expect(cut.checkedReleases[releaseSet]).toEqual({ gateDigest: 'b'.repeat(64), sealedSet: 'c'.repeat(64) });
    for (const bad of [
      { [releaseSet]: { gateDigest: 'b'.repeat(64) } },
      { [releaseSet]: { gateDigest: 'B'.repeat(64), sealedSet: 'c'.repeat(64) } },
      { 'not-a-digest': { gateDigest: 'b'.repeat(64), sealedSet: 'c'.repeat(64) } },
    ]) {
      expect(() => parsePublicDevnetCutV1({
        schema: 'dclutch-public-cut-v1', cluster: 'devnet', market: null,
        activity: { found: null, trade: null, resolve: null, redeem: null },
        checkedReleases: bad,
      })).toThrow();
    }
  });

  it('refuses activity without a Market and unknown manifest fields', () => {
    expect(() => parsePublicDevnetCutV1({ schema: 'dclutch-public-cut-v1', cluster: 'devnet', market: null, activity: { found: 'a'.repeat(64), trade: null, resolve: null, redeem: null }, checkedReleases: {} })).toThrow(/pending/);
    expect(() => parsePublicDevnetCutV1({ schema: 'dclutch-public-cut-v1', cluster: 'devnet', market: null, activity: { found: null, trade: null, resolve: null, redeem: null }, checkedReleases: {}, extra: true })).toThrow(/unknown/);
    // The new field is required, not optional: a cut that omits it has not
    // been asked the question, and defaulting it to {} would answer "none have
    // a checked release" on its behalf.
    expect(() => parsePublicDevnetCutV1({ schema: 'dclutch-public-cut-v1', cluster: 'devnet', market: null, activity: { found: null, trade: null, resolve: null, redeem: null } })).toThrow(/missing or unknown/);
  });
});
