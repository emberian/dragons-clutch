import { describe, expect, it } from 'vitest';

import { formatBasisPointsV1, issuedSupplySharesV1, SUPPLY_SHARE_MEANING_V1 } from './supplyShares';

describe('issued supply shares', () => {
  it('splits the flagship’s even vector into an exact even split', () => {
    const split = issuedSupplySharesV1(['500000000', '500000000', '500000000', '500000000']);
    expect(split).not.toBeNull();
    expect(split?.even).toBe(true);
    expect(split?.totalAtoms).toBe('2000000000');
    expect(split?.shares.map((share) => share.basisPoints)).toEqual([2500, 2500, 2500, 2500]);
  });

  it('always sums to exactly 100.00% under largest-remainder rounding', () => {
    const split = issuedSupplySharesV1(['1', '1', '1']);
    expect(split?.shares.map((share) => share.basisPoints)).toEqual([3334, 3333, 3333]);
    expect(split?.shares.reduce((sum, share) => sum + share.basisPoints, 0)).toBe(10_000);
    // Three equal single atoms are still an even split — 33/33/33 with the
    // remainder deterministically on the first index.
    expect(split?.even).toBe(true);

    const uneven = issuedSupplySharesV1(['7', '2', '1']);
    expect(uneven?.shares.reduce((sum, share) => sum + share.basisPoints, 0)).toBe(10_000);
    expect(uneven?.shares.map((share) => share.basisPoints)).toEqual([7000, 2000, 1000]);
    expect(uneven?.even).toBe(false);
  });

  it('never rounds a real cell to a phantom zero unless it truly is zero', () => {
    const split = issuedSupplySharesV1(['1', '999999999']);
    expect(split?.shares[0].basisPoints).toBeGreaterThanOrEqual(0);
    expect(split?.shares[0].atoms).toBe('1');
    // A truly zero supply is a zero share — a fact, not a rounding artifact.
    const withZero = issuedSupplySharesV1(['0', '100']);
    expect(withZero?.shares[0].basisPoints).toBe(0);
    expect(withZero?.even).toBe(false);
  });

  it('returns null — no invented uniform split — when nothing is issued', () => {
    expect(issuedSupplySharesV1([])).toBeNull();
    expect(issuedSupplySharesV1(['0', '0', '0'])).toBeNull();
  });

  it('refuses a negative supply outright', () => {
    expect(() => issuedSupplySharesV1(['-1', '2'])).toThrow('unsigned');
  });

  it('formats basis points exactly', () => {
    expect(formatBasisPointsV1(2500)).toBe('25.00%');
    expect(formatBasisPointsV1(3334)).toBe('33.34%');
    expect(formatBasisPointsV1(0)).toBe('0.00%');
    expect(formatBasisPointsV1(10_000)).toBe('100.00%');
    expect(() => formatBasisPointsV1(10_001)).toThrow();
    expect(() => formatBasisPointsV1(2.5)).toThrow();
  });

  it('carries the meaning sentence every share surface must render', () => {
    expect(SUPPLY_SHARE_MEANING_V1).toContain('not a traded price');
    expect(SUPPLY_SHARE_MEANING_V1).toContain('not a forecast');
    expect(SUPPLY_SHARE_MEANING_V1).toContain('even split');
  });
});
