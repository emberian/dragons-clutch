import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import {
  type MarketDiscoveryCardV1,
  type MarketDiscoveryV1,
  type MarketHoardV1,
} from '@/lib/marketDiscovery';

import LandingPulse, {
  collateralTileV1,
  emptyCurrentMarketPulseV1,
  partiallyReadPulseV1,
  readPulseV1,
} from './LandingPulse';
import NumberStrip from './NumberStrip';

const SCAN = Object.freeze({
  mode: 'program-scan' as const,
  note: 'test scan',
  scanSlot: '490435916',
  addresses: Object.freeze([]),
  scannedAccounts: 22,
  incompatibleMarketAccounts: Object.freeze([]),
});

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
  registryProgram: 'Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj',
  rentBeneficiary: 'Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj',
});

/** A vault this page authenticated, holding a named mint's raw atoms. */
function hoard(collateralMint: string, principalAtoms: string, mintDisplayDecimals: number | null): MarketHoardV1 {
  return Object.freeze({
    status: 'derived',
    observedSlot: SCAN.scanSlot,
    address: 'hoard111111111111111111111111111111111111111',
    custodyProgramId: '34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH',
    custodyContext: '22'.repeat(32),
    custodyAuthority: 'auth1111111111111111111111111111111111111111',
    collateralMint,
    tokenProgram: 'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb',
    principalAtoms,
    mintDisplayDecimals,
  });
}

function card(
  address: string,
  phase: 'Founding' | 'Open' | 'Terminal' | 'Retiring' | 'Retired',
  options: Readonly<{ hoard?: MarketHoardV1; terminal?: boolean }> = {},
): MarketDiscoveryCardV1 {
  return Object.freeze({
    status: 'decoded',
    address,
    provenance: Object.freeze({ kind: 'chain', observedSlot: SCAN.scanSlot }),
    observedSlot: SCAN.scanSlot,
    phase,
    readiness: phase === 'Open' ? 'Consumed' : 'Prepaid',
    generation: phase === 'Open' ? '2' : '1',
    outstandingCapabilities: '0',
    principalCapSets: '500000000',
    settlement: options.terminal === true
      ? Object.freeze({ status: 'terminal', label: 'terminal receipt', winner: 0 })
      : Object.freeze({ status: 'open', label: 'no terminal receipt' }),
    identity: IDENTITY,
    collateral: Object.freeze({ status: 'unread', realmContentId: IDENTITY.realmId, reason: 'unread' }),
    liability: Object.freeze({ status: 'unread', reason: 'unread' }),
    hoard: options.hoard ?? Object.freeze({ status: 'unread', reason: 'unread' }),
    capabilities: Object.freeze({ status: 'unread', manifestId: IDENTITY.capabilityManifestId, reason: 'unread' }),
    bindings: Object.freeze([]),
    refusal: null,
  });
}

function discovery(cards: ReadonlyArray<MarketDiscoveryCardV1>): MarketDiscoveryV1 {
  return Object.freeze({
    coreProgramId: 'HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N',
    registryProgramId: IDENTITY.registryProgram,
    claimsProgramId: '85hwTeQGabwFRs71Hafvngb1UmHb6dQoumBv3VV4epNN',
    custodyProgramId: '34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH',
    floorSlot: SCAN.scanSlot,
    enumeration: SCAN,
    cards: Object.freeze(cards),
    reason: `${cards.length} decoded`,
  });
}

/**
 * The devnet this was measured against: two markets opened, fourteen foundings
 * from the build-out left standing, six accounts from an older Core generation
 * the reader refuses, and each open market's vault holding half a billion atoms
 * of a DIFFERENT mint. Every number below is that chain's, not an invention.
 */
const MINT_A = '6odqARs4nxavriq36ynmbH8fTup824amzqpv2dfBki6C';
const MINT_B = '7rswmACUNP75FxSnt3YqDAYJKFamoZ9adGDiNc3u2Hqc';
const LIVE_SHAPE = discovery([
  card('open1111111111111111111111111111111111111111', 'Open', { hoard: hoard(MINT_A, '500000000', 6) }),
  ...Array.from({ length: 14 }, (_, index) => card(`found${index}${'1'.repeat(38 - String(index).length)}`, 'Founding')),
  card('open2222222222222222222222222222222222222222', 'Open', { hoard: hoard(MINT_B, '500000000', 6) }),
]);
const LIVE_SCAN = Object.freeze({
  ...SCAN,
  incompatibleMarketAccounts: Object.freeze(Array.from({ length: 6 }, (_, index) => Object.freeze({
    address: `legacy${index}${'1'.repeat(37 - String(index).length)}`,
    magic: 'DCLTCOR2',
    accountBytes: 352,
  }))),
});

describe('LandingPulse', () => {
  it('renders every count as unread while nothing has been read, never as zero', () => {
    const html = renderToStaticMarkup(<LandingPulse />);
    expect(html).toContain('Markets open');
    expect(html).toContain('Collateral locked up');
    expect(html).toContain('Markets resolved');
    expect(html.split('>—</strong>').length - 1).toBe(3);
    expect(html).toContain('Reading live from the chain…');
    expect(html).not.toContain('>0</strong>');
  });

  it('reports zero current listings without erasing incompatible historical Markets', () => {
    const state = emptyCurrentMarketPulseV1('Devnet', {
      mode: 'program-scan',
      note: 'test scan',
      scanSlot: '489269449',
      addresses: Object.freeze([]),
      scannedAccounts: 7,
      incompatibleMarketAccounts: Object.freeze([
        Object.freeze({ address: '3Dhpq9tufPuBMroMfUNaWhfZMPfLFh6MG7vwhJFfqjMm', magic: 'DCLTCOR2', accountBytes: 352 }),
        Object.freeze({ address: '8mQmwmQMwtUeW8SyzABrgM7W8wFb2UPpQMeavgcX87z', magic: 'DCLTCOR2', accountBytes: 352 }),
      ]),
    });
    expect(state.stats[0]).toMatchObject({ label: 'Markets open', value: '0' });
    // The facts are unchanged; who they are addressed to is not. A reader on
    // the landing page does not know what DCLTCOR2 or a 352-byte layout is,
    // and does not need to in order to understand that some older markets
    // exist and are not in the count.
    expect(state.provenance).toContain('holds no market this page can read');
    expect(state.provenance).toContain('2 older markets');
    expect(state.provenance).toContain('not counted above');
    expect(state.provenance).not.toContain('owns no Market');
    expect(state.provenance).not.toContain('DCLTCOR2');
  });
});

/**
 * The headline number.
 *
 * It used to be how many market accounts exist, which on a devnet the protocol
 * was BUILT on meant the debris of a build-out led the front page: "16 markets
 * listed / 2 open for trading". Both halves true; the emphasis exactly
 * backwards. These cases pin the emphasis, because that is the part a decoder
 * cannot check.
 */
describe('what the front page leads with', () => {
  it('counts only the markets that are open, never a founding attempt', () => {
    const state = readPulseV1('Devnet', LIVE_SCAN, LIVE_SHAPE);
    expect(state.stats[0]).toMatchObject({ label: 'Markets open', value: '2' });
    expect(state.stats[0].value).not.toBe('16');
    expect(state.stats[0].detail).toContain('founding is finished');
  });

  it('still owes the reader every market the headline left out', () => {
    const state = readPulseV1('Devnet', LIVE_SCAN, LIVE_SHAPE);
    expect(state.provenance).toContain('16 markets are listed on this deployment in all: 2 open');
    expect(state.provenance).toContain('14 are still in founding');
    expect(state.provenance).toContain('earlier attempts from the build-out');
    expect(state.provenance).toContain('devnet history is public');
    // The older generation is disclosed as its own fact, not folded into the
    // sixteen, because it was never in the sixteen.
    expect(state.provenance).toContain('6 more were written by an older version of the protocol');
    expect(state.provenance).toContain('slot 490435916');
  });

  it('says so plainly when nothing has opened yet', () => {
    const state = readPulseV1('Devnet', SCAN, discovery([card('found11111111111111111111111111111111111111', 'Founding')]));
    expect(state.stats[0]).toMatchObject({ value: '0' });
    expect(state.stats[0].detail).toBe('none yet — every market here is still being founded');
    expect(state.provenance).toContain('1 market is listed on this deployment in all: 0 open');
    expect(state.provenance).toContain('1 is still in founding');
  });

  /**
   * A market that is open and can never trade is not one you can do anything
   * with, so the headline figure does not carry it. Being left out of a count
   * is exactly how something goes quiet, so it is named in the same breath.
   */
  it('leaves a market that can never trade out of the headline and names it anyway', () => {
    const shut: MarketDiscoveryCardV1 = Object.freeze({
      ...card('shut1111111111111111111111111111111111111111', 'Open'),
      capabilities: Object.freeze({
        status: 'authenticated',
        manifestId: IDENTITY.capabilityManifestId,
        recordAddress: 'rec11111111111111111111111111111111111111111',
        observedSlot: SCAN.scanSlot,
        badges: Object.freeze([Object.freeze({
          index: 0,
          kindId: 'ab'.repeat(32),
          label: 'Direct successor',
          recognized: true,
          programSetId: 'cd'.repeat(32),
          configId: 'ef'.repeat(32),
          activation: 'deadline' as const,
          deadline: '490330281',
          dependencies: Object.freeze([]),
          funding: Object.freeze({
            compartments: Object.freeze([]),
            nativeLamportsTotal: BigInt(0),
            realmCollateralTotal: BigInt(0),
            realmCollateral: null,
          }),
        })]),
      }),
    });
    const state = readPulseV1('Devnet', SCAN, discovery([card('open1111111111111111111111111111111111111111', 'Open'), shut]));
    expect(state.stats[0]).toMatchObject({ label: 'Markets open', value: '1' });
    expect(state.stats[0].detail).toContain('1 more is open to read but can never trade');
    expect(state.provenance).toContain('2 markets are listed on this deployment in all: 1 open');
    expect(state.provenance).toContain('1 is open and readable but can never trade');
    expect(state.provenance).toContain('the window to switch trading on closed before it happened');

    // With nothing left that can trade, the strip says that instead of
    // reporting everything as still being founded.
    const onlyShut = readPulseV1('Devnet', SCAN, discovery([shut]));
    expect(onlyShut.stats[0]).toMatchObject({ value: '0' });
    expect(onlyShut.stats[0].detail).toContain('none you can trade');
    // It can still reach its answer, so the resolutions tile does not claim
    // there is nothing here to resolve.
    expect(onlyShut.stats[2].detail).toBe('none yet — a market reaches its answer when its own source reports, and not before');
  });

  it('names refused and settled markets in the sentence rather than dropping them', () => {
    const refused: MarketDiscoveryCardV1 = Object.freeze({
      status: 'refused',
      address: 'refuse11111111111111111111111111111111111111',
      provenance: Object.freeze({ kind: 'refused', reason: 'this account did not decode' }),
      observedSlot: SCAN.scanSlot,
      refusal: 'this account did not decode',
    });
    const state = readPulseV1('Devnet', SCAN, discovery([
      card('open1111111111111111111111111111111111111111', 'Open'),
      card('term1111111111111111111111111111111111111111', 'Terminal', { terminal: true }),
      refused,
    ]));
    expect(state.provenance).toContain('1 has passed its answer');
    expect(state.provenance).toContain('1 would not decode and carries its refusal instead of a figure');
    expect(state.stats[2]).toMatchObject({ label: 'Markets resolved', value: '1' });
    expect(state.stats[2].detail).toBe('markets that have reached their answer');
  });
});

/**
 * The collateral tile, which is the one that had a dash where it had a fact.
 * "2 different collateral tokens — their units do not add up" is true and it is
 * the wrong sentence: on this page a dash means WE COULD NOT READ IT, and both
 * totals had been read exactly.
 */
describe('collateral across more than one token', () => {
  it('shows one exact total per token instead of a dash', () => {
    const tile = collateralTileV1(LIVE_SHAPE);
    expect(tile.value).toBeNull();
    expect(tile.parts).toHaveLength(2);
    expect(tile.parts?.[0].value).toBe('500000000');
    expect(tile.parts?.[1].value).toBe('500000000');
    expect(tile.detail).toContain('units of different tokens are never added together');
    // No row anywhere is the two mints added together.
    expect(tile.parts?.map((part) => part.value)).not.toContain('1000000000');
  });

  it('labels each total with its own mint, its precision, and its vault count', () => {
    const tile = collateralTileV1(LIVE_SHAPE);
    expect(tile.parts?.[0].label).toBe('6odqA…Bki6C · 500 at 6 decimals · 1 vault');
    expect(tile.parts?.[1].label).toBe('7rswm…u2Hqc · 500 at 6 decimals · 1 vault');
  });

  it('adds vaults of the SAME token and shows one row for them', () => {
    const tile = collateralTileV1(discovery([
      card('open1111111111111111111111111111111111111111', 'Open', { hoard: hoard(MINT_A, '500000000', 6) }),
      card('open2222222222222222222222222222222222222222', 'Open', { hoard: hoard(MINT_A, '250000000', 6) }),
    ]));
    expect(tile.parts).toHaveLength(1);
    expect(tile.parts?.[0].value).toBe('750000000');
    expect(tile.parts?.[0].label).toBe('6odqA…Bki6C · 750 at 6 decimals · 2 vaults');
    expect(tile.detail).toBe('one collateral token, in raw units, across 2 vaults');
  });

  it('omits the precision rather than assuming one when the mint did not authenticate', () => {
    const tile = collateralTileV1(discovery([
      card('open1111111111111111111111111111111111111111', 'Open', { hoard: hoard(MINT_A, '500000000', null) }),
    ]));
    expect(tile.parts?.[0].label).toBe('6odqA…Bki6C · 1 vault');
    expect(tile.parts?.[0].value).toBe('500000000');
  });

  it('keeps the dash when there is genuinely no authenticated vault to total', () => {
    const tile = collateralTileV1(discovery([card('found11111111111111111111111111111111111111', 'Founding')]));
    expect(tile.value).toBeNull();
    expect(tile.parts).toBeUndefined();
    expect(tile.detail).toBe('no vault here could be authenticated, so no total is claimed');
  });

  it('renders the per-token rows as figures, not as one em dash', () => {
    const html = renderToStaticMarkup(<NumberStrip stats={[collateralTileV1(LIVE_SHAPE)]} provenance="test" />);
    expect(html).toContain('<ul class="viz-strip-parts">');
    expect(html.split('<strong>500000000</strong>').length - 1).toBe(2);
    // The em dash is the strip's UNREAD placeholder. It may still appear as
    // punctuation in a sentence; what it may not be any more is this tile's
    // value, because this tile's value was read.
    expect(html).not.toContain('<strong>—</strong>');
  });
});

describe('the resolutions count, which is a real zero', () => {
  it('keeps the zero and explains what would move it', () => {
    const state = readPulseV1('Devnet', LIVE_SCAN, LIVE_SHAPE);
    expect(state.stats[2]).toMatchObject({ label: 'Markets resolved', value: '0' });
    expect(state.stats[2].detail).toBe('none yet — a market reaches its answer when its own source reports, and not before');
    // No date, no "yesterday": a caption that ages is a caption that will lie.
    expect(state.stats[2].detail).not.toMatch(/yesterday|today|this week|2026/);
  });

  it('says the simpler true thing when there is nothing open to resolve', () => {
    const state = readPulseV1('Devnet', SCAN, discovery([card('found11111111111111111111111111111111111111', 'Founding')]));
    expect(state.stats[2].detail).toBe('none yet — no market is open to resolve');
  });
});

describe('a scan that answered and a join that did not', () => {
  const enumeration = Object.freeze({
    mode: 'program-scan' as const,
    note: 'test scan',
    scanSlot: '489905402',
    addresses: Object.freeze(['36CHzLdAujpE8c23ThGiKLNJLVndC2R5ogit1HHNFXFQ']),
    scannedAccounts: 16,
    incompatibleMarketAccounts: Object.freeze([]),
  });

  it('keeps the count it actually read instead of blanking the whole strip', () => {
    // The scan is one request; the join is roughly four per market. Against a
    // throttling public endpoint the second can fail after the first answered,
    // and the front page is the worst place to throw away a number we hold.
    const state = partiallyReadPulseV1('Devnet', enumeration, 'the endpoint is rate-limiting this browser (HTTP 429).');
    expect(state.provenance).toContain('holds 1 market');
    expect(state.provenance).toContain('Reading inside them did not finish');
    expect(state.provenance).toContain('rate-limiting');
    // The count survives; the headline does not claim it. How many are OPEN is
    // read INSIDE each market, and that is exactly the read that failed.
    expect(state.stats[0].value).toBeNull();
    expect(state.stats[0].detail).toBe('1 market is listed here; whether it is open is read inside it');
  });

  it('leaves the two it did not read as dashes, never as zeroes', () => {
    // A zero here would be a claim about collateral and resolutions that no
    // read supports. The page's own rule: a dash means we could not read it.
    const state = partiallyReadPulseV1('Devnet', enumeration, 'network down');
    expect(state.stats[1].value).toBeNull();
    expect(state.stats[2].value).toBeNull();
    expect(state.stats[1].detail).toBe('not read this time');
  });
});
