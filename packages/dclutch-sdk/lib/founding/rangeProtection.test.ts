import { describe, expect, it } from 'vitest';

import manifestVector from '../../fixtures/founding/campaign-manifest-vector.json';
import {
  composeRangeProtectionV1,
  formatTicksV1,
  rangeProtectionBackingV1,
} from './rangeProtection';

const campaign = (manifestVector as Readonly<{
  marketInput: Readonly<{ cut_denominator: number; cuts: ReadonlyArray<string>; portfolio_denominator: number; coefficients: ReadonlyArray<number>; initial_collateral_atoms: number }>;
}>).marketInput;

describe('range protection against the campaign that founded a real Market', () => {
  it('composes exactly the cuts, denominator and coefficients the campaign published', () => {
    // demo_market_input's SOL/USD band: 120.00 to 180.00 at a denominator of
    // 100. If this composition and the campaign's recorded input disagree, one
    // of the two is not building the product it says it is.
    const product = composeRangeProtectionV1({
      coordinateLabel: 'SOL/USD',
      cutDenominator: BigInt(campaign.cut_denominator),
      lowerEdgeTicks: BigInt(campaign.cuts[0]),
      upperEdgeTicks: BigInt(campaign.cuts[1]),
    });
    expect(product.cuts.map(String)).toEqual([...campaign.cuts]);
    expect(product.cutDenominator).toBe(BigInt(campaign.cut_denominator));
    expect(product.portfolioDenominator).toBe(BigInt(campaign.portfolio_denominator));
    expect(product.coefficients.map(Number)).toEqual([...campaign.coefficients]);
  });

  it('labels the three regions and the explicit failure outcome', () => {
    const product = composeRangeProtectionV1({
      coordinateLabel: 'SOL/USD',
      cutDenominator: 100n,
      lowerEdgeTicks: 12_000n,
      upperEdgeTicks: 18_000n,
    });
    expect(product.outcomes.map((outcome) => outcome.label)).toEqual([
      'SOL/USD < 120',
      '120 ≤ SOL/USD < 180',
      'SOL/USD ≥ 180',
      'Resolution failure',
    ]);
    expect(product.outcomes.map((outcome) => outcome.kind)).toEqual(['below-band', 'inside-band', 'above-band', 'resolution-failure']);
    expect(product.failureOutcomeIndex).toBe(3);
  });

  it('pays in both tails and nothing inside the band or on failure', () => {
    const product = composeRangeProtectionV1({
      coordinateLabel: 'ETH/USD',
      cutDenominator: 100n,
      lowerEdgeTicks: 350_000n,
      upperEdgeTicks: 400_000n,
    });
    expect(product.outcomes.filter((outcome) => outcome.coefficient > 0n).map((outcome) => outcome.kind)).toEqual(['below-band', 'above-band']);
    expect(product.outcomes.filter((outcome) => outcome.coefficient === 0n).map((outcome) => outcome.kind)).toEqual(['inside-band', 'resolution-failure']);
  });

  it('keeps outcomes exactly regions + 1, the width the Found decoder re-derives', () => {
    const product = composeRangeProtectionV1({ coordinateLabel: 'X', cutDenominator: 1n, lowerEdgeTicks: 1n, upperEdgeTicks: 2n });
    expect(product.regions).toBe(product.cuts.length + 1);
    expect(product.outcomeCount).toBe(product.regions + 1);
    expect(product.coefficients.length).toBe(product.outcomeCount);
    expect(product.outcomes.length).toBe(product.outcomeCount);
  });
});

describe('range protection refuses a band that is not a partition', () => {
  it('refuses edges that meet or cross, because cuts must strictly increase', () => {
    for (const [low, high] of [[100n, 100n], [200n, 100n]] as const) {
      expect(() => composeRangeProtectionV1({ coordinateLabel: 'X', cutDenominator: 100n, lowerEdgeTicks: low, upperEdgeTicks: high }))
        .toThrow(/strictly below/);
    }
  });

  it('refuses a nonpositive or oversized denominator', () => {
    expect(() => composeRangeProtectionV1({ coordinateLabel: 'X', cutDenominator: 0n, lowerEdgeTicks: 1n, upperEdgeTicks: 2n })).toThrow(/positive u64/);
    expect(() => composeRangeProtectionV1({ coordinateLabel: 'X', cutDenominator: 1n << 64n, lowerEdgeTicks: 1n, upperEdgeTicks: 2n })).toThrow(/positive u64/);
  });

  it('admits negative cuts, which the result domain encodes as i128', () => {
    const product = composeRangeProtectionV1({ coordinateLabel: 'basis', cutDenominator: 100n, lowerEdgeTicks: -500n, upperEdgeTicks: 500n });
    expect(product.cuts.map(String)).toEqual(['-500', '500']);
    expect(product.outcomes[0].label).toBe('basis < -5');
  });
});

describe('formatting ticks exactly', () => {
  it('never uses floating point, at any magnitude', () => {
    expect(formatTicksV1(12_000n, 100n)).toBe('120');
    expect(formatTicksV1(12_345n, 100n)).toBe('123.45');
    expect(formatTicksV1(12_305n, 100n)).toBe('123.05');
    expect(formatTicksV1(-12_345n, 100n)).toBe('-123.45');
    // Well past Number.MAX_SAFE_INTEGER, where a float formatter would lie.
    expect(formatTicksV1(9_007_199_254_740_993_00n, 100n)).toBe('9007199254740993');
    expect(formatTicksV1(1n, 100_000_000n)).toBe('0.00000001');
  });

  it('refuses a nonpositive denominator', () => {
    expect(() => formatTicksV1(1n, 0n)).toThrow(/must be positive/);
  });
});

describe('what a founding principal buys', () => {
  it('mints one complete set per atom and backs the largest outcome supply', () => {
    const product = composeRangeProtectionV1({ coordinateLabel: 'SOL/USD', cutDenominator: 100n, lowerEdgeTicks: 12_000n, upperEdgeTicks: 18_000n });
    const backing = rangeProtectionBackingV1(product, BigInt(campaign.initial_collateral_atoms));
    expect(backing.completeSets).toBe(1_000_000_000n);
    expect(backing.perOutcomeSupplyAtoms).toEqual([1_000_000_000n, 1_000_000_000n, 1_000_000_000n, 1_000_000_000n]);
    expect(backing.requiredBackingWhileUnresolvedAtoms).toBe(1_000_000_000n);
    expect(backing.payingOutcomes).toEqual([0, 2]);
  });

  it('refuses a zero or oversized principal', () => {
    const product = composeRangeProtectionV1({ coordinateLabel: 'X', cutDenominator: 1n, lowerEdgeTicks: 1n, upperEdgeTicks: 2n });
    expect(() => rangeProtectionBackingV1(product, 0n)).toThrow(/nonzero u64/);
    expect(() => rangeProtectionBackingV1(product, 1n << 64n)).toThrow(/nonzero u64/);
  });
});
