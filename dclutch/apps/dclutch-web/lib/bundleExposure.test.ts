import { describe, expect, it } from 'vitest';

import {
  BUNDLE_MINT_A_V1,
  BUNDLE_MINT_B_V1,
  BUNDLE_SLOT_V1,
  BUNDLE_TERMS_ONE_V1,
  BUNDLE_TERMS_TWO_V1,
  bundleEntryV1 as entry,
  bundleHexIdV1 as hexId,
  bundlePortfolioV1 as portfolio,
} from '../fixtures/bundlePortfolio';
import { currentCoreMarketV3, LIVE, liveRpcAccount } from '../fixtures/liveOpenMarket';
import { sha256 } from './bytes';
import { bundleExposureV1, claimBandV1 } from './bundleExposure';
import { REALM_SCHEMA_RELEASE_ID_V1 } from './generated/coreFound';
import { inspectPortfolioV1 } from './portfolio';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/**
 * The bundle bound, checked where a wrong answer would be a false statement
 * about someone's money.
 *
 * Two things are pinned harder than the rest. First, EXACTNESS: the arithmetic
 * runs on u64 atom counts that leave the double-precision integers behind, so a
 * suite that only ever tests small numbers would pass against a float
 * implementation. Second, DIRECTION: the ceiling is an upper bound and the
 * floor a lower one, and a netting release may only ever narrow the band. A
 * release that widened it, or a bound that moved the wrong way, is refused
 * rather than rendered.
 */

const SLOT = BUNDLE_SLOT_V1;
const MINT_A = BUNDLE_MINT_A_V1;
const MINT_B = BUNDLE_MINT_B_V1;
const OWNER = LIVE.founder;
const TERMS_ONE = BUNDLE_TERMS_ONE_V1;
const TERMS_TWO = BUNDLE_TERMS_TWO_V1;

describe('the band one position can pay', () => {
  it('is the smallest and largest balance, because the payout is their weighted average', () => {
    expect(claimBandV1(['10', '40', '25'])).toEqual({ floorAtoms: '10', ceilingAtoms: '40', swingAtoms: '30' });
  });

  it('collapses to a point when every claim is held in equal measure', () => {
    expect(claimBandV1(['7', '7', '7'])).toEqual({ floorAtoms: '7', ceilingAtoms: '7', swingAtoms: '0' });
  });

  it('stays exact past the last integer a double can name', () => {
    // 2^64-1 and 2^63+1 are both beyond Number.MAX_SAFE_INTEGER; a float
    // implementation reports the ceiling as 18446744073709552000 and the swing
    // as a number ending in 000. Nothing here may round in either direction.
    const band = claimBandV1(['9223372036854775809', '18446744073709551615']);
    expect(band.floorAtoms).toBe('9223372036854775809');
    expect(band.ceilingAtoms).toBe('18446744073709551615');
    expect(band.swingAtoms).toBe('9223372036854775806');
    expect(BigInt(band.floorAtoms) + BigInt(band.swingAtoms)).toBe(BigInt(band.ceilingAtoms));
  });

  it('refuses a balance that is not a canonical atom count instead of coercing it', () => {
    expect(() => claimBandV1(['10', '4.5'])).toThrow(/canonical unsigned decimal atom count/);
    expect(() => claimBandV1(['10', '-1'])).toThrow(/canonical unsigned decimal atom count/);
    expect(() => claimBandV1(['10', '007'])).toThrow(/canonical unsigned decimal atom count/);
    expect(() => claimBandV1([])).toThrow(/at least one balance/);
  });
});

describe('markets that share no terms', () => {
  const exposure = bundleExposureV1(portfolio([
    entry('MarketOne', ['10', '40', '25'], { terms: TERMS_ONE }),
    entry('MarketTwo', ['5', '5', '100'], { terms: TERMS_TWO }),
  ]));
  const [bundle] = exposure.bundles;

  it('sums the legs exactly, and says that sum is the answer rather than a caution', () => {
    expect(bundle.ceilingAtoms).toBe('140');
    expect(bundle.floorAtoms).toBe('15');
    expect(bundle.swingAtoms).toBe('125');
    expect(bundle.releaseAtoms).toBe('0');
    expect(bundle.clusters).toEqual([]);
    expect(bundle.sharedTerms).toBe(false);
  });

  it('states the one sentence no margined venue can state truthfully', () => {
    expect(bundle.netting).toContain('settle against different things');
    expect(bundle.netting).toContain('the sum of what each can pay alone');
    expect(bundle.netting).toContain('That sum is the true maximum, not a cautious one');
    expect(bundle.netting).toContain('that number assumes your Markets move together');
    expect(bundle.netting).toContain('will not put one into your arithmetic');
  });

  it('leaves the co-resolved figures equal to the plain ones, because nothing is locked', () => {
    expect(bundle.coResolvedCeilingAtoms).toBe(bundle.ceilingAtoms);
    expect(bundle.coResolvedFloorAtoms).toBe(bundle.floorAtoms);
  });
});

describe('markets that carry the identical terms identity', () => {
  const exposure = bundleExposureV1(portfolio([
    entry('MarketOne', ['10', '40', '25']),
    entry('MarketTwo', ['30', '5', '5']),
  ]));
  const [bundle] = exposure.bundles;
  const [cluster] = bundle.clusters;

  it('adds the balances claim by claim and narrows the band from both ends', () => {
    if (cluster.status !== 'locked') throw new Error(cluster.reason);
    // sums are [40, 45, 30]: joint ceiling 45 against a leg-sum of 70, joint
    // floor 30 against a leg-sum of 15.
    expect(cluster.jointCeilingAtoms).toBe('45');
    expect(cluster.jointFloorAtoms).toBe('30');
    expect(cluster.sumOfCeilingsAtoms).toBe('70');
    expect(cluster.sumOfFloorsAtoms).toBe('15');
    expect(cluster.ceilingReleaseAtoms).toBe('25');
    expect(cluster.floorReleaseAtoms).toBe('15');
  });

  it('keeps the headline on the sum and the release beside it, labelled as conditional', () => {
    expect(bundle.ceilingAtoms).toBe('70');
    expect(bundle.floorAtoms).toBe('15');
    expect(bundle.releaseAtoms).toBe('25');
    expect(bundle.coResolvedCeilingAtoms).toBe('45');
    expect(bundle.coResolvedFloorAtoms).toBe('30');
    if (cluster.status !== 'locked') throw new Error(cluster.reason);
    expect(cluster.note).toContain('walked to its own failure outcome on its own deadline');
    expect(cluster.note).toContain('the figures above the fold stay the sum');
  });

  it('names the release in the netting sentence without folding it into the bound', () => {
    expect(bundle.netting).toContain('1 group of these Markets settles against the same thing');
    expect(bundle.netting).toContain('25 atoms of the sum above can never be paid at once');
    expect(bundle.netting).toContain('nothing nets without a model');
  });
});

describe('what the netting refuses rather than approximates', () => {
  it('refuses two same-terms Markets whose Positions are different widths', () => {
    const exposure = bundleExposureV1(portfolio([
      entry('MarketOne', ['10', '40', '25']),
      entry('MarketTwo', ['30', '5']),
    ]));
    const [cluster] = exposure.bundles[0].clusters;
    if (cluster.status !== 'refused') throw new Error('a width mismatch must refuse');
    expect(cluster.reason).toContain('3 and 2 claims wide');
    expect(exposure.bundles[0].releaseAtoms).toBe('0');
    expect(exposure.bundles[0].ceilingAtoms).toBe('70');
  });

  it('refuses two same-terms Markets whose Positions name different liability bases', () => {
    const exposure = bundleExposureV1(portfolio([
      entry('MarketOne', ['10', '40', '25'], { basis: hexId(0x0b) }),
      entry('MarketTwo', ['30', '5', '5'], { basis: hexId(0x0c) }),
    ]));
    const [cluster] = exposure.bundles[0].clusters;
    if (cluster.status !== 'refused') throw new Error('a basis mismatch must refuse');
    expect(cluster.reason).toContain('the same claim index need not mean the same payout');
    expect(exposure.bundles[0].releaseAtoms).toBe('0');
  });

  it('never adds atoms of two collateral mints, and never claims one bundle spans them', () => {
    const exposure = bundleExposureV1(portfolio([
      entry('MarketOne', ['10', '40'], { mint: MINT_A }),
      entry('MarketTwo', ['1', '1000'], { mint: MINT_B, terms: TERMS_TWO }),
    ]));
    expect(exposure.bundles).toHaveLength(2);
    expect(exposure.bundles.map((bundle) => bundle.ceilingAtoms)).toEqual(['40', '1000']);
    expect(exposure.reason).toContain('atoms of different mints are different units and are never added');
  });

  it('excludes a Market that did not decode, and one whose Realm was never read, by name', () => {
    const exposure = bundleExposureV1(portfolio([
      entry('MarketOne', ['10', '40']),
      entry('MarketTwo', ['1', '1000'], { marketRefused: true }),
      entry('MarketThree', ['2', '9'], { realmUnread: true }),
    ]));
    expect(exposure.legCount).toBe(1);
    expect(exposure.bundles[0].ceilingAtoms).toBe('40');
    expect(exposure.excluded.map((item) => item.marketAddress)).toEqual(['MarketTwo', 'MarketThree']);
    expect(exposure.excluded[0].reason).toContain('did not decode at this finalized floor');
    expect(exposure.excluded[1].reason).toContain('the collateral mint these atoms are denominated in is unknown');
  });

  it('states the boundary it will not cross instead of estimating past it', () => {
    const exposure = bundleExposureV1(portfolio([entry('MarketOne', ['10', '40'])]));
    expect(exposure.boundary).toContain('the payoff basis records themselves, the knots and the degree');
    expect(exposure.boundary).toContain('It states no number it cannot derive from bytes it read');
    expect(exposure.bundles[0].netting).toContain('Netting is a question about two positions or more');
  });
});

describe('settlement can only ever narrow the band', () => {
  it('counts the settled legs and states the monotonicity as arithmetic, not a promise', () => {
    const none = bundleExposureV1(portfolio([entry('MarketOne', ['10', '40']), entry('MarketTwo', ['1', '9'], { terms: TERMS_TWO })]));
    expect(none.bundles[0].settledLegs).toBe(0);
    expect(none.bundles[0].settlement).toContain('None of these 2 Markets has settled yet');
    expect(none.bundles[0].settlement).toContain('nothing here was ever borrowed');

    const one = bundleExposureV1(portfolio([
      entry('MarketOne', ['10', '40'], { settled: true }),
      entry('MarketTwo', ['1', '9'], { terms: TERMS_TWO }),
    ]));
    expect(one.bundles[0].settledLegs).toBe(1);
    expect(one.bundles[0].settlement).toContain('1 of 2 Markets has settled');
    expect(one.bundles[0].settlement).toContain('the band only ever narrows');
  });
});

describe('the two directions this page may never get wrong', () => {
  const vectors: ReadonlyArray<ReadonlyArray<ReadonlyArray<string>>> = Object.freeze([
    [['0', '0', '0'], ['0', '0', '0']],
    [['1', '0', '0'], ['0', '1', '0']],
    [['18446744073709551615', '0'], ['0', '18446744073709551615']],
    [['4', '4', '4'], ['9', '2', '7']],
    [['1000000000000000003', '2', '999'], ['5', '1000000000000000001', '1']],
  ]);

  it('never reports a release that widens the band, at any magnitude', () => {
    for (const [left, right] of vectors) {
      const exposure = bundleExposureV1(portfolio([entry('MarketOne', left), entry('MarketTwo', right)]));
      const [bundle] = exposure.bundles;
      const [cluster] = bundle.clusters;
      if (cluster.status !== 'locked') throw new Error(cluster.reason);
      expect(BigInt(cluster.ceilingReleaseAtoms) >= 0n).toBe(true);
      expect(BigInt(cluster.floorReleaseAtoms) >= 0n).toBe(true);
      expect(BigInt(bundle.coResolvedCeilingAtoms) <= BigInt(bundle.ceilingAtoms)).toBe(true);
      expect(BigInt(bundle.coResolvedFloorAtoms) >= BigInt(bundle.floorAtoms)).toBe(true);
      expect(BigInt(bundle.coResolvedFloorAtoms) <= BigInt(bundle.coResolvedCeilingAtoms)).toBe(true);
    }
  });

  it('keeps every leg inside the bundle it is summed into', () => {
    for (const [left, right] of vectors) {
      const exposure = bundleExposureV1(portfolio([entry('MarketOne', left), entry('MarketTwo', right, { terms: TERMS_TWO })]));
      const [bundle] = exposure.bundles;
      const legCeilings = bundle.legs.reduce((sum, leg) => sum + BigInt(leg.ceilingAtoms), 0n);
      const legFloors = bundle.legs.reduce((sum, leg) => sum + BigInt(leg.floorAtoms), 0n);
      expect(bundle.ceilingAtoms).toBe(legCeilings.toString());
      expect(bundle.floorAtoms).toBe(legFloors.toString());
      for (const leg of bundle.legs) {
        expect(BigInt(leg.floorAtoms) <= BigInt(leg.ceilingAtoms)).toBe(true);
        expect(BigInt(leg.swingAtoms)).toBe(BigInt(leg.ceilingAtoms) - BigInt(leg.floorAtoms));
      }
    }
  });
});

describe('against the account bytes a live chain actually wrote', () => {
  const CORE = LIVE.programs.core;
  const REGISTRY = LIVE.programs.registry;
  const CLAIMS = LIVE.programs.claims;

  function client(accounts: ReadonlyMap<string, RpcAccount>): SolanaRpcClient {
    return {
      finalizedSlot: async () => SLOT,
      multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
        slot: SLOT,
        accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null }))),
      }),
    } as unknown as SolanaRpcClient;
  }

  async function chain(position?: Uint8Array): Promise<Map<string, RpcAccount>> {
    const accounts = new Map<string, RpcAccount>([
      [LIVE.market.address, liveRpcAccount(LIVE.market, { data: currentCoreMarketV3() })],
      [LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate)],
      [LIVE.founderPosition.address, liveRpcAccount(LIVE.founderPosition, { data: position ?? LIVE.founderPosition.data })],
    ]);
    const realm = deriveFinalizedRecordAddressesV1(REGISTRY, REALM_SCHEMA_RELEASE_ID_V1, await sha256(LIVE.realmRecord.data));
    accounts.set(realm.record, liveRpcAccount(LIVE.realmRecord));
    return accounts;
  }

  const request = { coreProgramId: CORE, claimsProgramId: CLAIMS, registryProgramId: REGISTRY, owner: OWNER, marketAddresses: [LIVE.market.address] };

  it('reads the founder complete set as a band that no outcome can move', async () => {
    const exposure = bundleExposureV1(await inspectPortfolioV1(client(await chain()), request));
    expect(exposure.legCount).toBe(1);
    expect(exposure.excluded).toEqual([]);
    const [bundle] = exposure.bundles;
    expect(bundle.floorAtoms).toBe('500000000');
    expect(bundle.ceilingAtoms).toBe('500000000');
    expect(bundle.swingAtoms).toBe('0');
    expect(bundle.headline).toContain('pays exactly 500000000 atoms whatever happens');
    expect(bundle.headline).toContain('this is collateral parked rather than a stance on anything');
  });

  it('opens the band the moment one claim balance falls below the others', async () => {
    const smaller = new Uint8Array(LIVE.founderPosition.data);
    new DataView(smaller.buffer).setBigUint64(128 + 2 * 8, 7n, true);
    const exposure = bundleExposureV1(await inspectPortfolioV1(client(await chain(smaller)), request));
    const [bundle] = exposure.bundles;
    expect(bundle.floorAtoms).toBe('7');
    expect(bundle.ceilingAtoms).toBe('500000000');
    expect(bundle.swingAtoms).toBe('499999993');
    expect(bundle.legs[0].marketAddress).toBe(LIVE.market.address);
    expect(bundle.legs[0].positionAddress).toBe(LIVE.founderPosition.address);
    expect(bundle.headline).toContain('at least 7 atoms and at most 500000000');
    expect(bundle.collateralMint.length).toBeGreaterThan(0);
  });
});
