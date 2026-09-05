import { describe, expect, it } from 'vitest';

import {
  UNNAMED_COLLATERAL_UNIT_V1,
  claimPriceGlossV1,
  denominationUnitV1,
  formatClaimPriceV1,
  formatQuantityV1,
  parseClaimPriceV1,
  parseQuantityV1,
  unreadDenominationV1,
  type DenominationV1,
} from './quantity';

const MINT = 'So11111111111111111111111111111111111111112';

/** The devnet collateral: 6 decimals, no name anywhere. */
const SIX: DenominationV1 = Object.freeze({ decimals: 6, unit: null, mint: MINT });
const UNREAD: DenominationV1 = unreadDenominationV1(MINT);

describe('the units policy', () => {
  // The spec's pinned behaviour table, row for row. ember's screenshot showed
  // 500000000 labelled "issued atoms"; at the devnet collateral's 6 decimals
  // that is 500, and the site has had the function to say so all along.
  it('humanizes exactly, and keeps the exact integer beside every reading', () => {
    const fiveHundred = formatQuantityV1('500000000', SIX);
    expect(fiveHundred.display).toBe('500');
    expect(fiveHundred.atoms).toBe('500000000');
    expect(fiveHundred.humanized).toBe(true);
    expect(fiveHundred.title).toBe('500000000 atoms at 6 decimals');

    const grouped = formatQuantityV1('1250500000', SIX);
    expect(grouped.display).toBe('1,250.5');
    expect(grouped.atoms).toBe('1250500000');
    expect(grouped.title).toBe('1250500000 atoms at 6 decimals');

    const oneAtom = formatQuantityV1('1', SIX);
    expect(oneAtom.display).toBe('0.000001');
    expect(oneAtom.title).toBe('1 atom at 6 decimals');

    const zero = formatQuantityV1('0', SIX);
    expect(zero.display).toBe('0');
    expect(zero.title).toBe('0 atoms at 6 decimals');
  });

  // A null decimals is never treated as 0. Treating it as 0 would silently
  // multiply every quantity on the page by a million and read as confident.
  it('fails open to the truth when the mint precision was never read', () => {
    const unknown = formatQuantityV1('500000000', UNREAD);
    expect(unknown.display).toBe('500,000,000');
    expect(unknown.atoms).toBe('500000000');
    expect(unknown.humanized).toBe(false);
    expect(unknown.title).toBe('500000000 atoms — this mint’s display precision was not read');
  });

  it('stays exact past u64, because it is BigInt all the way down', () => {
    const wide = formatQuantityV1('12345678901234567890', SIX);
    expect(wide.display).toBe('12,345,678,901,234.56789');
    expect(wide.atoms).toBe('12345678901234567890');

    // The widest u64, at the widest decimals the mint byte admits.
    const u64Max = formatQuantityV1(18_446_744_073_709_551_615n, { decimals: 9, unit: null, mint: MINT });
    expect(u64Max.display).toBe('18,446,744,073.709551615');
    expect(u64Max.atoms).toBe('18446744073709551615');
  });

  // Grouping is applied to the INTEGER part after the split, so it can never
  // perturb the value and can never reach the fraction.
  it('groups the integer part only, and never the fraction', () => {
    expect(formatQuantityV1('1000000000000', SIX).display).toBe('1,000,000');
    expect(formatQuantityV1('1000000123456', SIX).display).toBe('1,000,000.123456');
    expect(formatQuantityV1('999999', SIX).display).toBe('0.999999');
    expect(formatQuantityV1('100', { decimals: 0, unit: null, mint: MINT }).display).toBe('100');
    expect(formatQuantityV1('1234', { decimals: 0, unit: null, mint: MINT }).display).toBe('1,234');
  });

  it('never invents a token symbol', () => {
    expect(denominationUnitV1(SIX)).toBe(UNNAMED_COLLATERAL_UNIT_V1);
    expect(denominationUnitV1(UNREAD)).toBe('collateral');
    expect(denominationUnitV1({ decimals: 6, unit: 'USDC', mint: MINT })).toBe('USDC');
  });
});

describe('reading a typed size back to atoms', () => {
  // The defect this closes: relabelling the size input to `claims` without
  // converting it would charge a reader who typed 500 for 500 atoms -- a
  // millionth of what the word above the box promised. The label and the
  // parse move together or neither moves.
  it('reads a display quantity as the atoms it stands for', () => {
    expect(parseQuantityV1('500', SIX)).toBe(500_000_000n);
    expect(parseQuantityV1('0.5', SIX)).toBe(500_000n);
    expect(parseQuantityV1('1250.5', SIX)).toBe(1_250_500_000n);
    expect(parseQuantityV1('0.000001', SIX)).toBe(1n);
    // Round-trips with the formatter, which is the property that matters.
    expect(parseQuantityV1(formatQuantityV1('1250500000', SIX).display, SIX)).toBe(1_250_500_000n);
  });

  // The site prints grouping separators, so a reader who copies a printed
  // value back into the box should not be punished for it.
  it('accepts the separators it prints', () => {
    expect(parseQuantityV1('1,250.5', SIX)).toBe(1_250_500_000n);
    expect(parseQuantityV1('12,345,678,901,234.56789', SIX)).toBe(12_345_678_901_234_567_890n);
  });

  it('refuses a size finer than the smallest tradeable unit, rather than rounding it', () => {
    expect(() => parseQuantityV1('0.0000001', SIX)).toThrow(/finer than one claim atom/);
  });

  it('keeps the bounds the panel has always enforced', () => {
    expect(() => parseQuantityV1('0', SIX)).toThrow(/positive amount of claims/);
    expect(() => parseQuantityV1('abc', SIX)).toThrow(/positive amount of claims/);
    expect(() => parseQuantityV1('-5', SIX)).toThrow(/positive amount of claims/);
    expect(() => parseQuantityV1('18446744073709551616', { decimals: 0, unit: null, mint: MINT }))
      .toThrow(/u64 amount width/);
    // The widest u64 is admitted, exactly.
    expect(parseQuantityV1('18446744073709551615', { decimals: 0, unit: null, mint: MINT }))
      .toBe(18_446_744_073_709_551_615n);
  });

  // With no display scale there is nothing to divide by, so a size here is
  // counted in whole atoms and says so.
  it('counts in whole atoms when the mint precision was never read', () => {
    expect(parseQuantityV1('500000000', UNREAD)).toBe(500_000_000n);
    expect(() => parseQuantityV1('0.5', UNREAD)).toThrow(/whole collateral atoms/);
  });
});

describe('the price scale, explained exactly once', () => {
  // previewDirectInlineV3 refuses executionPrice > priceScale, so the ratio is
  // always in (0, 1] -- the share of one full payout that one claim costs.
  it('reads a limitPrice as cents on the unit', () => {
    const thirtyFive = formatClaimPriceV1(350_000n, 1_000_000n);
    expect(thirtyFive.display).toBe('35¢');
    expect(thirtyFive.cents).toBe('35');
    expect(thirtyFive.exact).toBe(true);
    expect(thirtyFive.fraction).toBe('350000 / 1000000');
    expect(thirtyFive.title).toBe('35¢ on the unit — exactly 350000 / 1000000');

    // The upper bound of the proven range: one claim costing one whole payout.
    expect(formatClaimPriceV1(1_000_000n, 1_000_000n).display).toBe('100¢');
    // And the smallest step the scale admits, carried rather than rounded off.
    expect(formatClaimPriceV1(1n, 1_000_000n).display).toBe('0.0001¢');
  });

  it('marks a non-terminating ratio approximate rather than rounding it quietly', () => {
    const third = formatClaimPriceV1(1n, 3n);
    expect(third.exact).toBe(false);
    expect(third.display).toBe('≈33.3333¢');
    expect(third.fraction).toBe('1 / 3');
    expect(third.title).toContain('does not terminate');
  });

  it('refuses a price scale that is not a positive share of anything', () => {
    expect(() => formatClaimPriceV1(1n, 0n)).toThrow(/positive price scale/);
  });

  it('reads typed cents into the immutable exact price scale without rounding', () => {
    expect(parseClaimPriceV1('35', 1_000_000n)).toBe(350_000n);
    expect(parseClaimPriceV1('33.3333', 1_000_000n)).toBe(333_333n);
    expect(parseClaimPriceV1('0.0001', 1_000_000n)).toBe(1n);
    expect(parseClaimPriceV1('100', 1_000_000n)).toBe(1_000_000n);
  });

  it('refuses inexact ticks and values outside the proven price interval', () => {
    expect(() => parseClaimPriceV1('33.33333', 1_000_000n)).toThrow(/not exactly representable/);
    expect(() => parseClaimPriceV1('0', 1_000_000n)).toThrow(/more than 0/);
    expect(() => parseClaimPriceV1('100.0001', 1_000_000n)).toThrow(/no more than 100/);
    expect(() => parseClaimPriceV1('-1', 1_000_000n)).toThrow(/positive decimal/);
    expect(() => parseClaimPriceV1('35', 0n)).toThrow(/positive u64 price scale/);
  });

  // Cents on the unit and the market's implied percentage are the same figure;
  // the gloss teaches that identity once, from the live price.
  it('glosses the price in the reader own terms, without inventing a currency', () => {
    const gloss = claimPriceGlossV1(formatClaimPriceV1(350_000n, 1_000_000n), SIX);
    expect(gloss).toBe('Each claim pays 1 collateral if this outcome wins, nothing if it does not. So a price of 35¢ is this market saying about 35% likely.');
    // The trading surface forbids these outright; the gloss must not smuggle
    // one in through the back door.
    for (const forbidden of ['$', 'odds', 'probability', 'volume', 'TVL', 'APR', 'APY']) {
      expect(gloss).not.toContain(forbidden);
    }
  });
});
