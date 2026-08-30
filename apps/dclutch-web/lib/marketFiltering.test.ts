import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from '@/lib/deployments';
import { type MarketDiscoveryCardV1, type MarketLiabilityV1 } from '@/lib/marketDiscovery';
import {
  filterMarketCardsV1,
  MARKET_SORT_CHOICES_V1,
  noMatchSentenceV1,
  sortMarketCardsV1,
  totalIssuedAtomsV1,
} from '@/lib/marketFiltering';

/**
 * Narrowing a listing is where a read-only page most easily starts lying: a
 * search that matches a hidden field, a sort that ranks an unread market as a
 * zero, a count that quietly reports the filtered number as the real one.
 * These pin against all three.
 */

const IDENTITY = Object.freeze({
  realmId: 'bb'.repeat(32),
  productRecordId: 'cc'.repeat(32),
  productInstanceId: 'dd'.repeat(32),
  resolutionPolicyId: 'ee'.repeat(32),
  capabilityManifestId: 'ff'.repeat(32),
  selectedReleaseSetId: '11'.repeat(32),
  registryProgram: DEVNET_DEPLOYMENT_V1.programs.registry,
  rentBeneficiary: DEVNET_DEPLOYMENT_V1.programs.rent,
});

const UNREAD: MarketLiabilityV1 = Object.freeze({ status: 'unread', reason: 'no Claims program was selected' });

function bound(supplyAtoms: ReadonlyArray<string>): MarketLiabilityV1 {
  return Object.freeze({
    status: 'bound',
    address: 'Ff'.repeat(22),
    claimCount: String(supplyAtoms.length),
    supplyAtoms: Object.freeze(supplyAtoms),
    requiredBackingAtoms: '0',
    revision: '1',
    observedSlot: '490435916',
  }) as unknown as MarketLiabilityV1;
}

function card(
  address: string,
  phase: 'Founding' | 'Open' | 'Terminal' | 'Retiring' | 'Retired',
  liability: MarketLiabilityV1 = UNREAD,
): MarketDiscoveryCardV1 {
  return Object.freeze({
    status: 'decoded',
    address,
    provenance: Object.freeze({ kind: 'chain', observedSlot: '490435916' }),
    observedSlot: '490435916',
    phase,
    readiness: phase === 'Open' ? 'Consumed' : 'Prepaid',
    generation: phase === 'Open' ? '2' : '1',
    outstandingCapabilities: '0',
    principalCapSets: '500000000',
    settlement: Object.freeze({ status: 'open', label: 'no terminal receipt' }),
    identity: IDENTITY,
    collateral: Object.freeze({ status: 'unread', realmContentId: IDENTITY.realmId, reason: 'no Registry program was selected' }),
    liability,
    hoard: Object.freeze({ status: 'unread', reason: 'no Custody program was selected' }),
    capabilities: Object.freeze({ status: 'unread', manifestId: IDENTITY.capabilityManifestId, reason: 'no Registry program was selected' }),
    bindings: Object.freeze([]),
    refusal: null,
  }) as unknown as MarketDiscoveryCardV1;
}

const TRADEABLE = '9JwhTHyxGhaoVsvSyT9VsJxV7PoQcPcjyhMLuJtY38Uq';
const FIRST_PUBLIC = '7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC';
const ORPHAN = 'CasyDFowGxqREDW5iWvKRgSMCgk5HnLQjnjegvRsSNPM';
const UNNAMED = 'zzzz111111111111111111111111111111111111111z';

describe('searching a listing', () => {
  const cards = [card(TRADEABLE, 'Open'), card(FIRST_PUBLIC, 'Terminal'), card(ORPHAN, 'Founding'), card(UNNAMED, 'Open')];

  it('returns everything for an empty or whitespace query, never nothing', () => {
    expect(filterMarketCardsV1(cards, '')).toHaveLength(4);
    expect(filterMarketCardsV1(cards, '   ')).toHaveLength(4);
  });

  it('matches the name this site gives a market', () => {
    const found = filterMarketCardsV1(cards, 'orphan');
    expect(found).toHaveLength(1);
    expect(found[0].address).toBe(ORPHAN);
  });

  it('matches an address a reader pasted back in, and is not case-sensitive', () => {
    expect(filterMarketCardsV1(cards, TRADEABLE)).toHaveLength(1);
    expect(filterMarketCardsV1(cards, TRADEABLE.toLowerCase())).toHaveLength(1);
  });

  it('matches the phase word printed on the card', () => {
    const open = filterMarketCardsV1(cards, 'open');
    expect(open.map((entry) => entry.address).sort()).toEqual([TRADEABLE, UNNAMED].sort());
  });

  it('narrows with every added term rather than widening', () => {
    expect(filterMarketCardsV1(cards, 'sol/usd').length).toBe(3);
    expect(filterMarketCardsV1(cards, 'sol/usd orphan').length).toBe(1);
    expect(filterMarketCardsV1(cards, 'sol/usd orphan nonsense').length).toBe(0);
  });

  /**
   * The rule that keeps a search honest: a card may only ever appear for a
   * reason its own text explains. Matching a field the page does not print
   * would produce a result the reader cannot account for.
   */
  it('never matches something the card does not show', () => {
    expect(filterMarketCardsV1(cards, IDENTITY.realmId)).toHaveLength(0);
    expect(filterMarketCardsV1(cards, 'Prepaid'.toLowerCase())).toHaveLength(0);
  });

  it('leaves the cards it keeps in the order they arrived', () => {
    const kept = filterMarketCardsV1(cards, 'sol/usd').map((entry) => entry.address);
    expect(kept).toEqual([TRADEABLE, FIRST_PUBLIC, ORPHAN]);
  });
});

describe('ordering a listing', () => {
  const unread = card(UNNAMED, 'Open');
  const small = card(FIRST_PUBLIC, 'Open', bound(['1', '2']));
  const large = card(TRADEABLE, 'Open', bound(['500000000', '500000000']));
  const cards = [unread, small, large];

  it('leaves the chain’s own order alone when that is what was chosen', () => {
    expect(sortMarketCardsV1(cards, 'enumerated')).toBe(cards);
  });

  it('sorts by name, and puts a market with no name last', () => {
    const ordered = sortMarketCardsV1(cards, 'name').map((entry) => entry.address);
    expect(ordered[2]).toBe(UNNAMED);
    // "the first market that can trade" sorts before "the first public market".
    expect(ordered.slice(0, 2)).toEqual([TRADEABLE, FIRST_PUBLIC]);
  });

  /**
   * The one that matters. An unread Claims aggregate has an UNKNOWN issuance.
   * Treating it as zero would rank it beneath a market that genuinely issued
   * nothing, which is a comparison nobody is entitled to make.
   */
  it('ranks by issued claims and never treats an unread market as a zero', () => {
    expect(totalIssuedAtomsV1(unread)).toBeNull();
    expect(totalIssuedAtomsV1(small)).toBe(3n);
    expect(totalIssuedAtomsV1(large)).toBe(1000000000n);
    const ordered = sortMarketCardsV1(cards, 'issued').map((entry) => entry.address);
    expect(ordered).toEqual([TRADEABLE, FIRST_PUBLIC, UNNAMED]);
  });

  it('breaks every tie with the incoming order, so a list never reshuffles itself', () => {
    const a = card(FIRST_PUBLIC, 'Open', bound(['7']));
    const b = card(TRADEABLE, 'Open', bound(['7']));
    expect(sortMarketCardsV1([a, b], 'issued').map((entry) => entry.address)).toEqual([FIRST_PUBLIC, TRADEABLE]);
    expect(sortMarketCardsV1([b, a], 'issued').map((entry) => entry.address)).toEqual([TRADEABLE, FIRST_PUBLIC]);
  });

  it('offers only orderings that say what they do and do not claim', () => {
    expect(MARKET_SORT_CHOICES_V1).toHaveLength(3);
    for (const choice of MARKET_SORT_CHOICES_V1) {
      expect(choice.label.length).toBeGreaterThan(0);
      expect(choice.meaning.length).toBeGreaterThan(20);
    }
    expect(MARKET_SORT_CHOICES_V1[2].meaning).toContain('not a measure of interest');
  });
});

describe('what a search says when it finds nothing', () => {
  it('reports what is still listed, so hiding is never read as removing', () => {
    const sentence = noMatchSentenceV1('zebra', 7);
    expect(sentence).toContain('zebra');
    expect(sentence).toContain('All 7 of these markets are still listed');
  });
});
