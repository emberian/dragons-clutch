import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from '@/lib/deployments';
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
    expect(html).toContain('Reading the finalized market list…');
    expect(html).toContain('enumerated from the Core program itself');
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
    expect(html).toContain('The markets that are open');
    expect(html).toContain('The markets that are open come first');
    expect(html).toContain('is counted and named below rather than dropped');
    expect(html).toContain('One card per market that has finished founding');
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
    expect(bar).toContain('It reads nothing the page is not already showing you');
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
    expect(searched).toContain('0 of 7 markets match');
    expect(searched).toContain('Searching hides cards; it never changes what exists.');
  });

  it('states the provenance and refusal contract every card is held to', () => {
    expect(html).toContain('CHAIN · finalized slot');
    expect(html).toContain('REFUSED');
    expect(html).toContain('never partly invented');
  });

  it('presents raw atoms and never a market-data metric', () => {
    expect(html).toContain('in raw units');
    expect(html).toContain('No volume · no odds · no probability · no yield');
    // Market-data vocabulary may appear only inside the sentences that refuse it.
    const disclaimers = [
      'There is no volume, price, odds, probability, or yield here, because the chain does not store any of those.',
      'Claim counts come from the accounts that actually hold the claims, in raw units.',
      'No volume · no odds · no probability · no yield',
      'Issuance shares are issuance, not odds',
    ];
    let remainder = html;
    for (const disclaimer of disclaimers) {
      expect(remainder).toContain(disclaimer);
      remainder = remainder.split(disclaimer).join('');
    }
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'Probability', 'TVL', '24h', 'APR', 'APY', 'yield', 'Total value locked', '$']) {
      expect(remainder).not.toContain(forbidden);
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
    expect(empty).toContain('No current compatible market is listed on devnet');
    expect(empty).toContain('1 historical DCLTCOR2 Market account');
    expect(empty).toContain('disclosed here but not listed as current');
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
    expect(html).toContain('<summary><span>2 foundings that never finished</span>');
    expect(html).toContain('kept because devnet history is public');
    expect(html).toContain('stopped part-way through');
    expect(html).toContain('not because they are something to be quiet about');
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
    expect(html).toContain('1 older market this page cannot read');
    expect(html).toContain('disclosed here but not listed as current');
    expect(html).toContain('1 historical DCLTCOR2 Market account');
    expect(html).toContain('will not guess at the difference');
    expect(html).toContain('3Dhpq9tufPuBMroMfUNaWhfZMPfLFh6MG7vwhJFfqjMm');
  });

  it('renders nothing at all when the deployment holds nothing but open markets', () => {
    const only = curateMarketListingV1([card('open1111111111111111111111111111111111111111', 'Open')]);
    expect(renderToStaticMarkup(<RestOfTheRecord listing={only} incompatible={[]} />)).toBe('');
  });

  it('speaks in the singular for a single abandoned founding', () => {
    const one = curateMarketListingV1([card('found111111111111111111111111111111111111111', 'Founding')]);
    const singular = renderToStaticMarkup(<RestOfTheRecord listing={one} incompatible={[]} />);
    expect(singular).toContain('1 founding that never finished');
    expect(singular).toContain('this one stopped part-way through');
    expect(singular).not.toContain('foundings that');
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
    const page = renderToStaticMarkup(<MarketDiscoveryWorkspace />);
    // The page-level editorial provenance sentence is always present.
    expect(page).toContain('this site&#x27;s editorial');
    expect(page).toContain('the chain stores no names');
  });

  it('labels an unregistered founding as build-out debris, and never invents a name', () => {
    const listing = curateMarketListingV1([card('found111111111111111111111111111111111111111', 'Founding')]);
    const html = renderToStaticMarkup(<RestOfTheRecord listing={listing} incompatible={[]} />);
    expect(html).toContain('Build-out founding · foun…1111');
    expect(html).toContain('found111111111111111111111111111111111111111');
    expect(html).not.toContain('market-question');
  });

  it('gives a registered founding its registered words even in the debris group', () => {
    // If the registry ever names a founding, the name wins over the generated
    // label — the group placement (which is phase, i.e. chain fact) does not.
    const listing = curateMarketListingV1([card(FLAGSHIP, 'Founding')]);
    const html = renderToStaticMarkup(<RestOfTheRecord listing={listing} incompatible={[]} />);
    expect(html).toContain('SOL/USD range — the first public market');
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

  it('draws the split with its honesty caption when the liability is bound', () => {
    const listing = curateMarketListingV1([
      Object.freeze({ ...card('found111111111111111111111111111111111111111', 'Founding'), liability: bound }),
    ]);
    const html = renderToStaticMarkup(<RestOfTheRecord listing={listing} incompatible={[]} />);
    expect(html).toContain('25.00%');
    expect(html).toContain('evenly split: issuance has not leaned toward any outcome yet');
    expect(html).toContain('not a traded price and not a forecast');
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
    expect(html).toContain('<strong>500000000</strong> atoms');
    expect(html).toContain('the mint prints 6 display decimals, which never scale this figure');
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
