import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from '@/lib/deployments';
import { type CapabilityFundingQuoteV1 } from '@/lib/capabilityManifest';
import {
  curateMarketListingV1,
  type MarketDiscoveryCardV1,
  type MarketHoardV1,
} from '@/lib/marketDiscovery';

import MarketFilterBar from './MarketFilterBar';
import MarketDiscoveryWorkspace, { EmptyMarkets, RestOfTheRecord } from './MarketDiscoveryWorkspace';

/**
 * The product inversion this surface carries: /markets lands on CONTENT. The
 * deployment manifest supplies the endpoint and the Core authority, the list
 * auto-loads, and there is no infrastructure form anywhere on the page. These
 * tests pin the inversion so the ask-the-visitor pattern cannot creep back.
 *
 * The page has since grown one control — a search, and an ordering beside it.
 * That did not weaken the inversion and was not allowed to: see the long note
 * on 'may grow a control that narrows what is here' below, which states in
 * words what is still forbidden and why a search is not an instance of it.
 */
describe('Market discovery route', () => {
  const html = renderToStaticMarkup(<MarketDiscoveryWorkspace />);

  it('lands on the market list of the baked deployment, loading with zero typing', () => {
    expect(html).toContain('Markets on Devnet');
    expect(html).toContain('Reading the market list…');
    // "current-compatible" is our word for "a market this build can read".
    expect(html).not.toContain('current-compatible');
    // The one button is a refresh, disabled while the auto-load is in flight.
    expect(html).toContain('>Reading…</button>');
  });

  /**
   * The page used to open on one flat grid in enumeration order, which put a
   * founding somebody abandoned mid-run between the two markets a reader came
   * to see. Every card in it was true. The ARRANGEMENT was the lie, and no
   * decoder can catch that kind, so it is pinned here instead.
   */
  it('leads with the markets that are open, and says where the rest went', () => {
    expect(html).toContain('Markets you can trade');
    expect(html).toContain('Markets you can trade come first');
    // Renegotiated 2026-08-31. The section used to carry a blurb explaining
    // that we list straight from the Core program with no index in between and
    // never partly invent a card. That is a promise about US, and it is gone.
    // The arrangement claim is the only one this test was ever really making.
    expect(html).not.toContain('no index in between');
    expect(html).not.toContain('never partly invented');
  });

  it('asks the visitor for NO endpoint and NO program address', () => {
    expect(html).not.toContain('Finalized RPC endpoint');
    expect(html).not.toContain('Core program</span>');
    expect(html).not.toContain('Registry program · optional');
    expect(html).not.toContain('Known Market addresses');
    expect(html).not.toContain('<textarea');
    // The page a reader lands on carries no control whatsoever. The list is
    // already loading; there is nothing to fill in and nothing to submit.
    expect(html).not.toContain('<input');
  });

  /**
   * THE GUARD ABOVE, RENEGOTIATED — read this before adding a control.
   *
   * `not.toContain('<input')` was, for a while, this suite's whole statement
   * of the inversion. It was a proxy, and a good one, for the thing actually
   * being protected: this page must never make a reader go and FIND a piece
   * of infrastructure before it will show them what it already knows.
   *
   * A search box is an `<input>` and violates none of that. So the rule is
   * now stated as what it always meant, and it is checked instead of counted:
   *
   *   FORBIDDEN, unchanged — a control that asks for infrastructure. An RPC
   *   endpoint, a program or Market address to paste, a keypair, a registry
   *   ID. Also unchanged: no `<textarea>` (the paste box for "Known Market
   *   addresses" was one), and no signing or submitting anywhere near a
   *   discovery surface. The single place "bring your own infrastructure"
   *   lives is the cluster picker in the nav, and that is deliberate.
   *
   *   ALLOWED — a control that narrows or reorders what is ALREADY on the
   *   page. It asks for nothing the reader does not already have, and the
   *   page is complete before anyone touches it.
   *
   * If a future control cannot be described by that second paragraph, it does
   * not belong here, and neither does a weakening of this test.
   */
  it('may grow a control that narrows what is here, and none that asks for infrastructure', () => {
    const bar = renderToStaticMarkup(
      <MarketFilterBar query="" onQuery={() => {}} order="enumerated" onOrder={() => {}} shown={2} total={2} />,
    );
    const inputs = bar.match(/<input\b[^>]*>/g) ?? [];
    expect(inputs).toHaveLength(1);
    expect(inputs[0]).toContain('type="search"');
    expect(bar).toContain('Search these markets');
    expect(bar).not.toContain('<textarea');

    // It asks for nothing a reader would have to leave the page to obtain,
    // and it says so about itself.
    // Renegotiated 2026-08-31: the bar carried two explanatory sentences under
    // the controls -- what a search reads, and what the chosen order means.
    // A search box and an order dropdown do not need either.
    expect(bar).not.toContain('It reads nothing');
    expect(bar).not.toContain('Nothing is ranked');
    for (const infrastructure of ['endpoint', 'Endpoint', 'RPC', 'keypair', 'private key', 'http', 'program address', 'Paste']) {
      expect(bar).not.toContain(infrastructure);
    }
    for (const submission of ['Sign', 'Submit', 'Connect']) {
      expect(bar).not.toContain(submission);
    }
  });

  it('reports what the deployment holds separately from what a search is showing', () => {
    const searched = renderToStaticMarkup(
      <MarketFilterBar query="nothing matches this" onQuery={() => {}} order="enumerated" onOrder={() => {}} shown={0} total={7} />,
    );
    expect(searched).toContain('0 of 7 match');
    // "Searching hides cards; it never changes what exists" is deleted; the
    // count says it. The invariant it described is pinned in this file by the
    // filtering tests themselves, which assert what a query may and may not
    // remove from the listing.
    expect(searched).not.toContain('never changes what exists');
  });

  /**
   * Renegotiated 2026-08-31. This block used to pin the hero aside that
   * narrated our provenance contract at the reader ("Every panel says where
   * its own numbers came from... never a blank, and never a zero"). The aside
   * is deleted. Provenance is now carried by the nav status line and by the
   * per-card chip, which is a label, not a sentence — so what is pinned here
   * is that the PAGE says none of it in prose.
   */
  it('carries no provenance sermon in the page chrome', () => {
    for (const sermon of ['never a blank', 'never partly invented', 'read live from the chain', 'no index in between']) {
      expect(html).not.toContain(sermon);
    }
  });

  /**
   * Renegotiated 2026-08-31, and it got STRICTER rather than looser.
   *
   * This used to be subtract-the-disclaimers-then-forbid: four sentences whose
   * whole job was to say what the page is not ("There is no volume, price,
   * odds, probability, or yield here...", "No volume · no odds · no
   * probability · no yield", "Issuance shares are issuance, not odds") were
   * exempted from the scan and then everything else was checked. Those
   * sentences are deleted -- a labelled number needs no interpretation guard,
   * and reading four refusals before the first market was the single most
   * exhausting thing on the page. With nothing left to exempt, the forbidden
   * scan now runs over the WHOLE document, which is what it should always have
   * been.
   */
  it('never shows a market-data metric, anywhere on the page', () => {
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'Probability', 'TVL', '24h', 'APR', 'APY', 'yield', 'Total value locked', '$']) {
      expect(html).not.toContain(forbidden);
    }
  });

  it('never exposes a signing or submission control on a discovery surface', () => {
    expect(html).not.toContain('Sign');
    expect(html).not.toContain('Submit');
    expect(html).not.toContain('Connect identity');
  });

  it('renders historical incompatible accounts without listing them as current markets', () => {
    const legacyAddress = '3Dhpq9tufPuBMroMfUNaWhfZMPfLFh6MG7vwhJFfqjMm';
    const empty = renderToStaticMarkup(<EmptyMarkets
      deployment={DEVNET_DEPLOYMENT_V1}
      enumeration={{
        mode: 'program-scan',
        note: 'test scan',
        scanSlot: '489269449',
        addresses: Object.freeze([]),
        scannedAccounts: 2,
        incompatibleMarketAccounts: Object.freeze([
          Object.freeze({ address: legacyAddress, magic: 'DCLTCOR2', accountBytes: 352 }),
        ]),
      }}
    />);
    expect(empty).toContain('No market on devnet yet');
    expect(empty).toContain('Made by an older version of the protocol');
    expect(empty).toContain('not listed as current');
    expect(empty).toContain(legacyAddress);
    expect(empty).toContain(`/explorer?view=account&amp;q=${legacyAddress}`);
    expect(empty).not.toContain('No markets on devnet');
  });
});

/**
 * The curated listing, rendered.
 *
 * `curateMarketListingV1` is pinned in the SDK; what these pin is the PAGE:
 * that each group keeps a label and a count a reader meets before expanding
 * anything, and that a founding which never finished cannot end up rendered
 * beside the markets that are open.
 */
const IDENTITY = Object.freeze({
  schemaMagic: 'DCLTCOR3',
  schemaVersion: 3,
  accountBytes: 360,
  marketId: 'aa'.repeat(32),
  realmId: 'bb'.repeat(32),
  productRecordId: 'cc'.repeat(32),
  productInstanceId: 'dd'.repeat(32),
  resolutionPolicyId: 'ee'.repeat(32),
  capabilityManifestId: 'ff'.repeat(32),
  selectedReleaseSetId: '11'.repeat(32),
  registryProgram: DEVNET_DEPLOYMENT_V1.programs.registry,
  rentBeneficiary: DEVNET_DEPLOYMENT_V1.programs.rent,
});

function card(
  address: string,
  phase: 'Founding' | 'Open' | 'Terminal' | 'Retiring' | 'Retired',
  hoard: MarketHoardV1 = { status: 'unread', reason: 'no Custody program was selected' },
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
    liability: Object.freeze({ status: 'unread', reason: 'no Claims program was selected' }),
    hoard,
    capabilities: Object.freeze({ status: 'unread', manifestId: IDENTITY.capabilityManifestId, reason: 'no Registry program was selected' }),
    bindings: Object.freeze([]),
    refusal: null,
  });
}

describe('the rest of the record', () => {
  const listing = curateMarketListingV1([
    card('found111111111111111111111111111111111111111', 'Founding'),
    card('open1111111111111111111111111111111111111111', 'Open'),
    card('found222222222222222222222222222222222222222', 'Founding'),
  ]);
  const legacy = Object.freeze([
    Object.freeze({ address: '3Dhpq9tufPuBMroMfUNaWhfZMPfLFh6MG7vwhJFfqjMm', magic: 'DCLTCOR2', accountBytes: 352 }),
  ]);
  const html = renderToStaticMarkup(<RestOfTheRecord listing={listing} incompatible={legacy} />);

  it('labels the abandoned foundings with their count and their reason, before anything is expanded', () => {
    // The summary is what a reader who never clicks walks away with, so the
    // count and the framing both have to live in it.
    expect(html).toContain('<summary><span>2 markets that were never finished</span>');
    expect(html).toContain('setup stopped part-way');
    expect(html).toContain('stopped part-way through');
    expect(html).toContain('There is nothing to trade against them');
  });

  it('collapses the group without dropping a single account from it', () => {
    // <details> with no `open` attribute: collapsed, and fully in the markup.
    expect(html).toContain('<details class="listing-group">');
    expect(html).not.toContain('<details class="listing-group" open');
    expect(html).toContain('found111111111111111111111111111111111111111');
    expect(html).toContain('found222222222222222222222222222222222222222');
  });

  it('never renders an open market into the aside', () => {
    expect(html).not.toContain('open1111111111111111111111111111111111111111');
  });

  it('gives the older-generation accounts a labelled row of their own', () => {
    expect(html).toContain('1 older market this build cannot read');
    expect(html).toContain('not listed as current');
    expect(html).toContain('Made by an older version of the protocol');
    // Renegotiated 2026-08-31: "It will not guess at the difference, so it
    // declines to read them rather than show you a field it made up" is a
    // promise about our decoder's manners. Deleted; the byte counts say it.
    expect(html).toContain('352 bytes where this build expects 360');
    expect(html).toContain('3Dhpq9tufPuBMroMfUNaWhfZMPfLFh6MG7vwhJFfqjMm');
  });

  it('renders nothing at all when the deployment holds nothing but open markets', () => {
    const only = curateMarketListingV1([card('open1111111111111111111111111111111111111111', 'Open')]);
    expect(renderToStaticMarkup(<RestOfTheRecord listing={only} incompatible={[]} />)).toBe('');
  });

  it('speaks in the singular for a single abandoned founding', () => {
    const one = curateMarketListingV1([card('found111111111111111111111111111111111111111', 'Founding')]);
    const singular = renderToStaticMarkup(<RestOfTheRecord listing={one} incompatible={[]} />);
    expect(singular).toContain('1 market that was never finished');
    expect(singular).toContain('this one stopped part-way');
    expect(singular).not.toContain('markets that were');
  });
});

/**
 * The bucket for a market that is `Open` and can never trade.
 *
 * This group exists because the site had exactly one untrue thing on it: two
 * devnet markets whose trading can never be switched on were filed under "the
 * markets that are open", which is where a reader looks for something they can
 * act on. The phase is not wrong and is not edited — the card still prints
 * `Open` — so the correction is a bucket, and what these pin is that a reader
 * meets the plain fact before expanding anything, and that such a market can
 * never be rendered among the open ones.
 */
describe('markets that can never trade', () => {
  const NO_FUNDING: CapabilityFundingQuoteV1 = Object.freeze({
    compartments: Object.freeze([]),
    nativeLamportsTotal: BigInt(0),
    realmCollateralTotal: BigInt(0),
    realmCollateral: null,
  });

  /** An `Open` card whose only capability could last be activated at `deadline`. */
  function shutCard(address: string, deadline = '490330281'): MarketDiscoveryCardV1 {
    return Object.freeze({
      ...card(address, 'Open'),
      capabilities: Object.freeze({
        status: 'authenticated' as const,
        manifestId: IDENTITY.capabilityManifestId,
        recordAddress: 'rec11111111111111111111111111111111111111111',
        observedSlot: '490435916',
        badges: Object.freeze([Object.freeze({
          index: 0,
          kindId: 'ab'.repeat(32),
          label: 'Direct successor',
          recognized: true,
          programSetId: 'cd'.repeat(32),
          configId: 'ef'.repeat(32),
          activation: 'deadline' as const,
          deadline,
          dependencies: Object.freeze([]),
          funding: NO_FUNDING,
        })]),
      }),
    });
  }

  const listing = curateMarketListingV1([
    shutCard('7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC'),
    shutCard('CasyDFowGxqREDW5iWvKRgSMCgk5HnLQjnjegvRsSNPM'),
    card('open1111111111111111111111111111111111111111', 'Open'),
  ]);
  const html = renderToStaticMarkup(<RestOfTheRecord listing={listing} incompatible={[]} />);

  it('names the group and its count before anything is expanded', () => {
    expect(html).toContain('<summary><span>2 markets that can never trade</span>');
    expect(html).toContain('trading can no longer be switched on');
  });

  it('says what happened in words a stranger can act on, without protocol vocabulary', () => {
    expect(html).toContain('Trading has to be switched on within a set window');
    expect(html).toContain('the window closed first');
    expect(html).toContain('Nothing can turn it on now');
    // Renegotiated 2026-08-31: "they stay readable for good", "It is here to
    // be read, not traded" and "every figure below is read live from it" were
    // reassurance about the page, not facts about the market. Deleted.
    expect(html).not.toContain('readable for good');

    // The prose this group adds is checked on its own. The card's chain-fact
    // rows around it keep the protocol's vocabulary, as they must — those are
    // quotations of what the account says, not an explanation of it.
    const prose = [...html.matchAll(/<p class="(?:market-empty|market-never-trades-note)">(.*?)<\/p>/g)].map((match) => match[1]);
    expect(prose.length).toBeGreaterThanOrEqual(2);
    for (const jargon of ['capabilit', 'manifest', 'PDA', 'activation', 'program', 'protocol', 'we ', 'our ']) {
      for (const sentence of prose) expect(sentence).not.toContain(jargon);
    }
  });

  it('marks each card, and still prints the phase the chain actually says', () => {
    expect(html).toContain('never trades');
    expect(html).toContain('Trading can never be switched on.');
    expect(html).toContain('The window closed at slot 490330281.');
    // The phase chip is chain fact and is not edited into something softer.
    expect(html).toContain('>Open</span>');
  });

  it('keeps a market that can still trade out of the group entirely', () => {
    expect(html).not.toContain('open1111111111111111111111111111111111111111');
    expect(listing.open.map((entry) => entry.address)).toEqual(['open1111111111111111111111111111111111111111']);
  });

  it('speaks in the singular for one such market', () => {
    const one = curateMarketListingV1([shutCard('7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC')]);
    const singular = renderToStaticMarkup(<RestOfTheRecord listing={one} incompatible={[]} />);
    expect(singular).toContain('1 market that can never trade');
    expect(singular).toContain('on this one the window closed');
  });

  /**
   * The boundary this bucket must not cross. `(DCLTCOR3, version 3, 360 bytes)`
   * is the width every Market on the cluster is written at, and this reader
   * cannot decode it. Such an account is disclosed as one it cannot read — it
   * is NOT filed as a market that can never trade, which would be the page
   * inventing a verdict about trading out of a failed read.
   */
  it('does not absorb an account it simply could not decode', () => {
    const undecodable: MarketDiscoveryCardV1 = Object.freeze({
      status: 'refused',
      address: '9JwhTHyxGhaoVsvSyT9VsJxV7PoQcPcjyhMLuJtY38Uq',
      provenance: Object.freeze({ kind: 'refused', reason: 'Core Market state is 360 bytes; the exact current width is 368.' }),
      observedSlot: '490435916',
      refusal: 'Core Market state is 360 bytes; the exact current width is 368.',
    });
    const listing = curateMarketListingV1([undecodable]);
    expect(listing.untradeable).toEqual([]);
    expect(listing.open).toEqual([]);
    expect(listing.unreadable.map((entry) => entry.address)).toEqual([undecodable.address]);

    const rendered = renderToStaticMarkup(<RestOfTheRecord
      listing={listing}
      incompatible={[Object.freeze({ address: '7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC', magic: 'DCLTCOR3', accountBytes: 360 })]}
    />);
    expect(rendered).toContain('1 account we could not read');
    expect(rendered).toContain('Core Market state is 360 bytes; the exact current width is 368.');
    expect(rendered).toContain('1 older market this build cannot read');
    expect(rendered).toContain('not listed as current');
    // Neither of them is dressed in the never-trades state, and the page does
    // not claim anywhere that they cannot trade.
    expect(rendered).not.toContain('never trades');
    expect(rendered).not.toContain('can never trade');
  });

  it('leaves a market whose manifest was never read among the open ones', () => {
    // An unread manifest is a read that did not happen. It is not evidence of a
    // shut window, and it never becomes a claim that a market is finished.
    const unread = curateMarketListingV1([card('open1111111111111111111111111111111111111111', 'Open')]);
    expect(unread.untradeable).toEqual([]);
    expect(renderToStaticMarkup(<RestOfTheRecord listing={unread} incompatible={[]} />)).toBe('');
  });
});

/**
 * The editorial layer: the chain stores no market names, so names come from
 * the shipped registry and say so. What these pin: a registered market shows
 * its name AND its question AND its address; an unregistered founding is
 * labelled as build-out debris rather than given an invented name; and the
 * page carries the whose-words-these-are sentence.
 */
describe('market names on cards', () => {
  const FLAGSHIP = '7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC';

  it('shows the registered name, question, and still the address, on a registered market', () => {
    const listing = curateMarketListingV1([card(FLAGSHIP, 'Open')]);
    const html = renderToStaticMarkup(<RestOfTheRecord listing={listing} incompatible={[]} />);
    // Open cards render in the main grid, not the aside — use a founding-side
    // proxy: render the card directly through the curated open group instead.
    expect(html).toBe('');
    // Renegotiated 2026-08-31. There used to be a page-level sentence saying
    // the name and question are ours and the chain stores no names. It is
    // deleted: a title on a card does not need a note explaining who typed it.
    // What is pinned now is that the page does not re-acquire one.
    const page = renderToStaticMarkup(<MarketDiscoveryWorkspace />);
    expect(page).not.toContain('the chain stores no names');
    expect(page).not.toContain('editorial');
  });

  it('labels an unregistered founding as build-out debris, and never invents a name', () => {
    const listing = curateMarketListingV1([card('found111111111111111111111111111111111111111', 'Founding')]);
    const html = renderToStaticMarkup(<RestOfTheRecord listing={listing} incompatible={[]} />);
    expect(html).toContain('Unfinished · foun…1111');
    expect(html).toContain('found111111111111111111111111111111111111111');
    expect(html).not.toContain('market-question');
  });

  it('gives a registered founding its registered words even in the debris group', () => {
    // If the registry ever names a founding, the name wins over the generated
    // label — the group placement (which is phase, i.e. chain fact) does not.
    const listing = curateMarketListingV1([card(FLAGSHIP, 'Founding')]);
    const html = renderToStaticMarkup(<RestOfTheRecord listing={listing} incompatible={[]} />);
    expect(html).toContain('SOL/USD range — first public market');
    expect(html).toContain('Where does the SOL/USD price finish this market&#x27;s window');
    expect(html).toContain(FLAGSHIP);
  });
});

/**
 * The issuance split on a card: drawn only from a bound liability, labelled
 * with the SDK's what-this-is-not sentence, and even splits explain
 * themselves. The card keeps the exact supply row as its value twin.
 */
describe('the issuance split on cards', () => {
  const bound = Object.freeze({
    status: 'bound' as const,
    observedSlot: '490435916',
    aggregateAddress: 'agg11111111111111111111111111111111111111111',
    claimsProgramId: DEVNET_DEPLOYMENT_V1.programs.claims,
    claimCount: 4,
    revision: '4',
    generation: '2',
    liabilityBasisId: '33'.repeat(32),
    custodyContext: '44'.repeat(32),
    supplyAtoms: Object.freeze(['500000000', '500000000', '500000000', '500000000']),
    requiredBackingAtoms: '500000000',
    requiredBackingBasis: 'maximum-claim-supply' as const,
  });

  it('draws the split, labelled, with its exact-value twin intact', () => {
    const listing = curateMarketListingV1([
      Object.freeze({ ...card('found111111111111111111111111111111111111111', 'Founding'), liability: bound }),
    ]);
    const html = renderToStaticMarkup(<RestOfTheRecord listing={listing} incompatible={[]} />);
    expect(html).toContain('25.00%');
    // Renegotiated 2026-08-31: the strip used to carry a two-sentence caption
    // saying the split is not a traded price and not a forecast, plus a
    // readout appendix explaining what an even split means. A labelled
    // percentage bar needs neither. The label is the whole contract.
    expect(html).toContain('Claims issued per outcome');
    expect(html).not.toContain('forecast');
    expect(html).not.toContain('has not leaned');
    // The exact-value twin stays: the raw supply row is still on the card.
    expect(html).toContain('500000000 · 500000000 · 500000000 · 500000000');
  });

  it('draws no split at all when the liability was not read', () => {
    const listing = curateMarketListingV1([card('found111111111111111111111111111111111111111', 'Founding')]);
    const html = renderToStaticMarkup(<RestOfTheRecord listing={listing} incompatible={[]} />);
    expect(html).not.toContain('%');
    expect(html).not.toContain('viz-figure');
  });
});

describe('a Hoard the page could authenticate', () => {
  const derived = Object.freeze({
    status: 'derived' as const,
    observedSlot: '490435916',
    address: 'hoard111111111111111111111111111111111111111',
    custodyProgramId: DEVNET_DEPLOYMENT_V1.programs.custody,
    custodyContext: '22'.repeat(32),
    custodyAuthority: 'auth1111111111111111111111111111111111111111',
    collateralMint: '6odqARs4nxavriq36ynmbH8fTup824amzqpv2dfBki6C',
    tokenProgram: 'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb',
    principalAtoms: '500000000',
    mintDisplayDecimals: 6,
  });

  it('prints the mint precision as the display convention it is, beside an unscaled figure', () => {
    const listing = curateMarketListingV1([card('found111111111111111111111111111111111111111', 'Founding', derived)]);
    const html = renderToStaticMarkup(<RestOfTheRecord listing={listing} incompatible={[]} />);
    // Renegotiated 2026-08-31: the sentence explaining that the mint's 6
    // display decimals never scale this figure is deleted. The misreading it
    // guarded against is closed by LABELLING the unit on the value instead --
    // "500000000 atoms" -- which is what the number is, not a note about it.
    expect(html).toContain('<strong>500000000</strong> atoms');
    expect(html).not.toContain('display decimals');
    // The scaled figure is a landing-strip convenience and has no business
    // standing in for the quantity on a card.
    expect(html).not.toContain('>500<');
  });

  it('says nothing about precision when the mint did not authenticate', () => {
    const listing = curateMarketListingV1([
      card('found111111111111111111111111111111111111111', 'Founding', { ...derived, mintDisplayDecimals: null }),
    ]);
    const html = renderToStaticMarkup(<RestOfTheRecord listing={listing} incompatible={[]} />);
    expect(html).toContain('<strong>500000000</strong> atoms');
    expect(html).not.toContain('display decimals');
  });
});
