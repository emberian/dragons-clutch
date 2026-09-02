import { describe, expect, it } from 'vitest';

import {
  checkedReleaseSetIdsV1,
  parseCheckedReleaseFragmentV1,
  stageCheckedReleaseV1,
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
    // THE SHIPPED CUT IS PENDING, and that is a live fact rather than a
    // placeholder: cohort-12's seven programs were closed on 2026-09-02 and
    // its market stopped resolving with them, so there is no market this
    // deployment can read or join until cohort-13 is founded and sealed.
    //
    // Pending is a first-class state here, not an empty one. Every surface
    // that reads the cut already has a pending face -- the front door says the
    // first markets are being set up, the launch rail says no market is open
    // yet, and both walk a reader to /markets rather than to a link that
    // returns account-not-found. The alternative was to leave the closed
    // cohort's address in place, which is exactly the defect the 2026-09-02 UX
    // walk found on the front door, and exactly how it got there: a lane
    // stopped without flipping a fixture.
    expect(PUBLIC_DEVNET_CUT_V1.market).toBeNull();
    // A pending cut may not name lifecycle activity at all -- the parser
    // refuses that shape -- so these are what the schema forces, not a choice.
    expect(PUBLIC_DEVNET_CUT_V1.activity.found).toBeNull();
    expect(PUBLIC_DEVNET_CUT_V1.activity.trade).toBeNull();
    expect(PUBLIC_DEVNET_CUT_V1.activity.resolve).toBeNull();
    expect(PUBLIC_DEVNET_CUT_V1.activity.redeem).toBeNull();
    expect(publicCutMarketHrefV1()).toBe('/markets');
    expect(publicCutExplorerHrefV1()).toBe('/explorer');
    // And it knows no checked release sets. Null, not the empty list: a cut
    // that names no Market describes no deployment, so it reports that nobody
    // consulted a record rather than that a record named none -- and the trade
    // spine raises no `release` wall from it.
    expect(PUBLIC_DEVNET_CUT_V1.checkedReleases).toEqual({});
    expect(checkedReleaseSetIdsV1()).toBeNull();
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

  /**
   * The row a person must never type. A sealing driver emits the triple and
   * this ingests it; the only judgement made here is whether the fragment is
   * about the set this cut's own Market selects.
   */
  describe('ingesting a sealing driver’s fragment', () => {
    const SELECTED = '797e83ac0522787898b24a963182b846f61f96c6968e4bfdbfbb8dc5bcf7e9a1';
    const OTHER = '6dcda322' + 'f'.repeat(56);
    // The sealing driver's own file, shaped exactly as it writes it: a map
    // keyed as the cut's own rows are, so ingestion is a copy and not a
    // transcription.
    // A live cut of this case's own, not the shipped one. Staging is about a
    // cut that NAMES a Market, and the shipped cut is pending between cohorts
    // -- coupling these to it meant they broke the hour cohort-12 closed, for
    // a reason that has nothing to do with what they check.
    const live = parsePublicDevnetCutV1({
      schema: 'dclutch-public-cut-v1',
      cluster: 'devnet',
      market: 'EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1',
      activity: { found: null, trade: null, resolve: null, redeem: null },
      checkedReleases: {},
    });
    const fragment = (releaseSetId: string) => parseCheckedReleaseFragmentV1({
      schema: 'dclutch-public-cut-checked-releases-fragment-v1',
      checkedReleases: { [releaseSetId]: { gateDigest: 'a'.repeat(64), sealedSet: 'b'.repeat(64) } },
    });

    it('stages a fragment for the set the cut’s Market selects', () => {
      const staged = stageCheckedReleaseV1(live, fragment(SELECTED), SELECTED);
      expect(staged.checkedReleases[SELECTED]).toEqual({ gateDigest: 'a'.repeat(64), sealedSet: 'b'.repeat(64) });
      expect(checkedReleaseSetIdsV1(staged)).toEqual([SELECTED]);
      // The shipped cut is untouched: staging returns a new cut and the caller
      // re-serializes it, so a refused stage cannot half-write a fixture.
      expect(live.checkedReleases).toEqual({});
    });

    it('refuses an unsealed plan’s empty map rather than staging nothing quietly', () => {
      // The producer emits an EMPTY map for an unsealed plan rather than
      // omitting the key -- the same "empty is the assertion" this cut makes.
      // Ingesting it must not read as a successful no-op.
      const empty = parseCheckedReleaseFragmentV1({
        schema: 'dclutch-public-cut-checked-releases-fragment-v1', checkedReleases: {},
      });
      expect(() => stageCheckedReleaseV1(live, empty, SELECTED)).toThrow(/seals nothing/);
    });

    it('refuses a fragment for a set this cut’s Market does not select, and names both', () => {
      expect(() => stageCheckedReleaseV1(live, fragment(OTHER), SELECTED))
        .toThrow(new RegExp(`${OTHER}[\\s\\S]*${SELECTED}`));
      // Staging it anyway would put a row in this site's deployment record
      // that turns the trade spine's `release` wall off for a market the
      // release was never checked against.
      expect(checkedReleaseSetIdsV1(live)).toEqual([]);
    });

    it('refuses a second, different release for a set it already names', () => {
      const staged = stageCheckedReleaseV1(live, fragment(SELECTED), SELECTED);
      expect(() => stageCheckedReleaseV1(staged, parseCheckedReleaseFragmentV1({
        schema: 'dclutch-public-cut-checked-releases-fragment-v1',
        checkedReleases: { [SELECTED]: { gateDigest: 'c'.repeat(64), sealedSet: 'd'.repeat(64) } },
      }), SELECTED)).toThrow(/already names a different checked release/);
      // Idempotent for the identical fragment: re-running the staging tool is
      // not a conflict.
      expect(stageCheckedReleaseV1(staged, fragment(SELECTED), SELECTED).checkedReleases[SELECTED])
        .toEqual({ gateDigest: 'a'.repeat(64), sealedSet: 'b'.repeat(64) });
    });

    it('refuses a fragment that is not one, rather than reading three fields out of it', () => {
      const row = { gateDigest: 'a'.repeat(64), sealedSet: 'b'.repeat(64) };
      for (const bad of [
        { schema: 'other', checkedReleases: { [SELECTED]: row } },
        { schema: 'dclutch-public-cut-checked-releases-fragment-v1', checkedReleases: { [SELECTED]: { gateDigest: 'a'.repeat(64) } } },
        { schema: 'dclutch-public-cut-checked-releases-fragment-v1', checkedReleases: { [SELECTED]: row }, extra: 1 },
        { schema: 'dclutch-public-cut-checked-releases-fragment-v1', checkedReleases: { [SELECTED.toUpperCase()]: row } },
      ]) expect(() => parseCheckedReleaseFragmentV1(bad)).toThrow();
    });
  });
});
