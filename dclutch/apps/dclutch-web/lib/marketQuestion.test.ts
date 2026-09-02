import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { sha256 } from './bytes';
import {
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
  SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
  WINDOW_SPEC_SCHEMA_ID_V1,
} from './generated/coreFound';
import {
  decodeWindowSpecV1,
  derivedOutcomeLabelsV1,
  derivedQuestionV1,
  derivedTitleV1,
  formatWindowInstantV1,
  inspectMarketQuestionV1,
} from './marketQuestion';
import { marketNarrativeV1 } from './marketRegistry';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { type SolanaRpcClient } from './rpc';

function put(bytes: Uint8Array, offset: number, value: Uint8Array): void { bytes.set(value, offset); }
function putU16(bytes: Uint8Array, offset: number, value: number): void { new DataView(bytes.buffer).setUint16(offset, value, true); }
function putU32(bytes: Uint8Array, offset: number, value: number): void { new DataView(bytes.buffer).setUint32(offset, value, true); }
function putU64(bytes: Uint8Array, offset: number, value: bigint): void { new DataView(bytes.buffer).setBigUint64(offset, value, true); }
function putI64(bytes: Uint8Array, offset: number, value: bigint): void { new DataView(bytes.buffer).setBigInt64(offset, value, true); }
function putI128(bytes: Uint8Array, offset: number, value: bigint): void {
  const view = new DataView(bytes.buffer);
  view.setBigUint64(offset, BigInt.asUintN(64, value), true);
  view.setBigInt64(offset + 8, BigInt.asIntN(64, value >> 64n), true);
}
function id(byte: number): Uint8Array { return new Uint8Array(32).fill(byte); }
function hex(bytes: Uint8Array): string { return [...bytes].map((byte) => byte.toString(16).padStart(2, '0')).join(''); }

/**
 * Cohort-12's actual shape: two cuts at 9,800 and 10,200 over a denominator of
 * 100, three ordinary cells and one explicit source-failure outcome. Built the
 * way the founding writes it so the decoders under test are the founding's.
 */
async function solUsdMarket(windowBounds: Readonly<{ start: bigint; end: bigint }> | null) {
  const cuts = [9_800n, 10_200n];
  const domain = new Uint8Array(240 + cuts.length * 16);
  put(domain, 0, new TextEncoder().encode('DCLTPRD2'));
  putU16(domain, 8, 2); putU16(domain, 10, 240); putU32(domain, 12, domain.length);
  putU32(domain, 16, cuts.length + 1); putU32(domain, 20, cuts.length);
  [1, 2, 3, 4, 5, 6].forEach((byte, index) => put(domain, 32 + index * 32, id(byte)));
  putU64(domain, 224, 100n);
  cuts.forEach((cut, index) => putI128(domain, 240 + index * 16, cut));
  const domainDigest = await sha256(domain);

  const outcomeCount = cuts.length + 2;
  const portfolio = new Uint8Array(208 + outcomeCount * 8);
  put(portfolio, 0, new TextEncoder().encode('DCLTPRF2'));
  putU16(portfolio, 8, 2); putU16(portfolio, 10, 208); putU32(portfolio, 12, portfolio.length);
  putU32(portfolio, 16, outcomeCount); portfolio[20] = 1;
  put(portfolio, 32, id(1)); put(portfolio, 64, domainDigest); put(portfolio, 96, id(7)); put(portfolio, 128, id(4)); put(portfolio, 160, id(5));
  putU64(portfolio, 192, 1n);
  [1n, 0n, 1n, 0n].forEach((coefficient, index) => putU64(portfolio, 208 + index * 8, coefficient));
  const portfolioDigest = await sha256(portfolio);

  const product = new Uint8Array(112);
  put(product, 0, new TextEncoder().encode('DCLTPRM2')); putU16(product, 8, 2);
  put(product, 16, id(1)); put(product, 48, domainDigest); put(product, 80, portfolioDigest);
  const productDigest = await sha256(product);

  const window = new Uint8Array(112);
  put(window, 0, new TextEncoder().encode('DCLTWIN1'));
  put(window, 16, id(2));
  if (windowBounds !== null) {
    putI64(window, 48, windowBounds.start);
    putI64(window, 56, windowBounds.end);
  }
  const windowDigest = await sha256(window);

  const source = new Uint8Array(240);
  put(source, 0, new TextEncoder().encode('DCLTSMV3')); putU16(source, 8, 3); source[11] = 2;
  put(source, 16, productDigest);
  put(source, 48, id(21)); put(source, 80, windowDigest); put(source, 112, id(4)); put(source, 176, id(6));
  put(source, 208, id(22));
  const sourceDigest = await sha256(source);

  const registry = new PublicKey(id(9)).toBase58();
  const addressFor = (schema: Uint8Array, digest: Uint8Array): string =>
    deriveFinalizedRecordAddressesV1(registry, schema, digest).record;
  const served = new Map<string, Uint8Array>([
    [addressFor(PRODUCT_RECORD_SCHEMA_ID_V2, productDigest), product],
    [addressFor(SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, sourceDigest), source],
    [addressFor(RESULT_DOMAIN_SCHEMA_ID_V2, domainDigest), domain],
    [addressFor(PORTFOLIO_SCHEMA_ID_V2, portfolioDigest), portfolio],
    [addressFor(WINDOW_SPEC_SCHEMA_ID_V1, windowDigest), window],
  ]);
  const observe = (addresses: ReadonlyArray<string>, length: number | null) => ({
    slot: '491885036',
    accounts: addresses.map((address) => ({
      address,
      account: served.has(address)
        ? {
            owner: registry,
            executable: false,
            lamports: '1000000',
            // `space` is the FULL data length whether or not a slice was asked
            // for, which is what devnet reports and what the chunk planner reads.
            space: served.get(address)!.length,
            data: length === null ? served.get(address)! : served.get(address)!.slice(0, length),
          }
        : null,
    })),
  });
  const client = {
    finalizedSlot: async () => '491885036',
    multipleAccountDataSlices: async (addresses: ReadonlyArray<string>, _offset: number, length: number) => observe(addresses, length),
    multipleAccounts: async (addresses: ReadonlyArray<string>) => observe(addresses, null),
  } as unknown as SolanaRpcClient;

  return { client, registry, productRecordId: hex(productDigest), resolutionPolicyId: hex(sourceDigest), served };
}

/**
 * The registry lagged every redeploy this project has done, and every field it
 * lagged on was a field the market's own records already carried. These are the
 * tests for reading them instead.
 */
describe('a market question, derived from the market', () => {
  it('reads the cuts, the denominator, the outcome width and the window off the chain', async () => {
    const start = 1_756_800_000n;
    const end = 1_756_886_400n;
    const fixture = await solUsdMarket({ start, end });
    const derived = await inspectMarketQuestionV1(fixture.client, {
      registryProgramId: fixture.registry,
      address: 'EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1',
      productRecordId: fixture.productRecordId,
      resolutionPolicyId: fixture.resolutionPolicyId,
    });
    expect(derived.cutDenominator).toBe(100n);
    expect(derived.cuts).toEqual([9_800n, 10_200n]);
    expect(derived.regionCount).toBe(3);
    // Four, and derived from the PORTFOLIO's width rather than from the cut
    // count: the failure outcome is a cell the product carries, not a cell
    // this module assumes.
    expect(derived.outcomeCount).toBe(4);
    expect(derived.window).toEqual({ startUnixSeconds: start, endUnixSeconds: end });
    expect(derived.windowRefusal).toBeNull();
  });

  it('renders a market with no registry row at all: real boundaries, no invented name', async () => {
    const fixture = await solUsdMarket({ start: 1_756_800_000n, end: 1_756_886_400n });
    const derived = await inspectMarketQuestionV1(fixture.client, {
      registryProgramId: fixture.registry,
      address: 'EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1',
      productRecordId: fixture.productRecordId,
      resolutionPolicyId: fixture.resolutionPolicyId,
    });

    // This is the walk's S3/R1 exactly: an unregistered market used to render
    // `Unnamed · EQnY…mGs1`, "Outcomes 4", no question and no settlement time,
    // and every one of those facts was already in the accounts it had read.
    const silent = marketNarrativeV1('EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1', 'Open', null, derived);
    expect(silent.titleSource).toBe('chain');
    expect(silent.title).not.toContain('Unnamed');
    expect(silent.title).toContain('98');
    expect(silent.title).toContain('102');
    expect(silent.question).toContain('below 98');
    expect(silent.question).toContain('98 – 102');
    expect(silent.outcomes).toEqual(['Below 98', '98 – 102', '102 and above', 'The source failed to report']);

    // The one thing the chain cannot say is the coordinate's common name, and
    // a row that supplies only that turns every derived string into the one a
    // reader wanted -- without the registry restating a single boundary.
    const named = marketNarrativeV1('EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1', 'Open', {
      title: null, question: null, coordinate: { label: 'SOL/USD', unitPrefix: '$' }, outcomes: null, resolution: null, story: null,
    }, derived);
    expect(named.title).toBe('SOL/USD — 3 ways past $98 and $102');
    expect(named.question).toContain('Where does SOL/USD finish');
    expect(named.outcomes).toEqual(['Below $98', '$98 – $102', '$102 and above', 'The source failed to report']);
    expect(named.questionSource).toBe('chain');
  });

  it('lets an editorial row override any derived string, and never the other way round', async () => {
    const fixture = await solUsdMarket({ start: 1n, end: 2n });
    const derived = await inspectMarketQuestionV1(fixture.client, {
      registryProgramId: fixture.registry,
      address: 'EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1',
      productRecordId: fixture.productRecordId,
      resolutionPolicyId: fixture.resolutionPolicyId,
    });
    const narrative = marketNarrativeV1('EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1', 'Open', {
      title: 'Which side of the week', question: 'Above or below?', coordinate: null,
      outcomes: ['A', 'B', 'C', 'D'], resolution: null, story: null,
    }, derived);
    expect(narrative.title).toBe('Which side of the week');
    expect(narrative.titleSource).toBe('registry');
    expect(narrative.outcomeSource).toBe('registry');
    // The numbers are untouched by the override: the registry supplies words
    // and only words.
    expect(derived.cuts).toEqual([9_800n, 10_200n]);
  });

  it('falls back to the address only when there is neither a row nor a read', () => {
    const nothing = marketNarrativeV1('EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1', 'Open', null, null);
    expect(nothing.title).toBe('Unnamed · EQnY…mGs1');
    expect(nothing.titleSource).toBe('address');
    expect(nothing.question).toBeNull();
    expect(nothing.outcomes).toBeNull();
  });

  it('keeps the market when only the window is unreadable, and says which half failed', async () => {
    const fixture = await solUsdMarket({ start: 9n, end: 1n });
    const derived = await inspectMarketQuestionV1(fixture.client, {
      registryProgramId: fixture.registry,
      address: 'EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1',
      productRecordId: fixture.productRecordId,
      resolutionPolicyId: fixture.resolutionPolicyId,
    });
    expect(derived.cuts).toEqual([9_800n, 10_200n]);
    expect(derived.window).toBeNull();
    expect(derived.windowRefusal).toContain('not ordered');
  });

  it('formats boundaries and instants exactly, never through a float', () => {
    const partition = { cuts: [9_800n, 10_200n], cutDenominator: 100n, regionCount: 3, outcomeCount: 4 };
    expect(derivedOutcomeLabelsV1(partition, { label: 'SOL/USD', unitPrefix: '$' })[1]).toBe('$98 – $102');
    // A denominator that is not a power of ten, and a cut that does not divide
    // evenly: the exact decimal, not the nearest double.
    expect(derivedOutcomeLabelsV1({ cuts: [1n], cutDenominator: 1_000_000n, regionCount: 2, outcomeCount: 3 }, null)[0])
      .toBe('Below 0.000001');
    expect(formatWindowInstantV1(1_756_886_400n)).toBe('2025-09-03 08:00 UTC');
    expect(derivedTitleV1({ cuts: [500n], cutDenominator: 10n, regionCount: 2, outcomeCount: 3 }, null))
      .toBe('An unnamed observable — above or below 50');
    expect(derivedQuestionV1({ cuts: [], cutDenominator: 1n, regionCount: 1, outcomeCount: 2 }, null))
      .toContain('Did the source report');
  });

  it('refuses a window record that is not a window', () => {
    const wrong = new Uint8Array(112);
    put(wrong, 0, new TextEncoder().encode('DCLTPRD2'));
    expect(() => decodeWindowSpecV1(wrong)).toThrow(/wrong exact ABI/);
    expect(() => decodeWindowSpecV1(new Uint8Array(111))).toThrow(/wrong exact ABI/);
  });
});
