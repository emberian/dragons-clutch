import { describe, expect, it } from 'vitest';

import {
  PRODUCT_V2_BYTES,
  compileProductV2,
  evaluateProductV2,
  parseProductKnots,
  parseProductTerms,
} from './productV2';

async function fixture() {
  return compileProductV2({
    productId: 41n,
    domainId: 42n,
    coordinateUnitId: 43n,
    payoutScale: 1_000_000n,
    knotDenominator: 2n,
    knots: [-100n, 0n, 100n],
    terms: [
      { shape: 'tent', left: 0, peak: 1, right: 2, amplitude: 50n },
      { shape: 'constant', left: 0, peak: 0, right: 0, amplitude: 3n },
      { shape: 'ramp-up', left: 0, peak: 0, right: 1, amplitude: 100n },
    ],
  });
}

describe('Product V2 exact signed-rational studio', () => {
  it('canonicalizes runtime-width terms into one exact 576-byte content identity', async () => {
    const product = await fixture();
    expect(product.bytes).toHaveLength(PRODUCT_V2_BYTES);
    expect(new TextDecoder().decode(product.bytes.slice(0, 8))).toBe('DCLTPAY2');
    expect(product.bytes[10]).toBe(3);
    expect(product.bytes[11]).toBe(3);
    expect(Array.from([product.bytes[320], product.bytes[336], product.bytes[352]])).toEqual([0, 1, 3]);
    expect(product.input.terms.map((term) => term.shape)).toEqual(['constant', 'ramp-up', 'tent']);
    expect(product.liabilityBound).toBe(153n);
    expect(product.digestHex).toMatch(/^[0-9a-f]{64}$/);
    expect(product.regions.map((region) => [region.left, region.right])).toEqual([
      ['−∞', '-100/2'], ['-100/2', '0/2'], ['0/2', '100/2'], ['100/2', '+∞'],
    ]);
  });

  it('keeps the coordinate rational and floors only each final interpolation contribution', async () => {
    const product = await fixture();
    expect(evaluateProductV2(product, -50n, 1n)).toBe(3n);
    expect(evaluateProductV2(product, -25n, 1n)).toBe(78n);
    expect(evaluateProductV2(product, 0n, 7n)).toBe(153n);
    expect(evaluateProductV2(product, 25n, 1n)).toBe(128n);
    expect(evaluateProductV2(product, 1n, 3n)).toBe(152n);
    expect(evaluateProductV2(product, 10_000n, 1n)).toBe(103n);
  });

  it('refuses noncanonical integers, partitions, terms, and arithmetic bounds', async () => {
    expect(() => parseProductKnots('01\n2')).toThrow('canonical');
    expect(() => parseProductTerms('ramp-up 0  1 50')).toThrow('canonical single spaces');
    await expect(compileProductV2({
      productId: 1n, domainId: 2n, coordinateUnitId: 3n, payoutScale: 4n, knotDenominator: 1n,
      knots: [0n, 0n], terms: [{ shape: 'constant', left: 0, peak: 0, right: 0, amplitude: 1n }],
    })).rejects.toThrow('strictly increasing');
    await expect(compileProductV2({
      productId: 1n, domainId: 2n, coordinateUnitId: 3n, payoutScale: 4n, knotDenominator: 1n,
      knots: [0n, 1n], terms: [
        { shape: 'ramp-up', left: 0, peak: 0, right: 1, amplitude: 1n },
        { shape: 'ramp-up', left: 0, peak: 0, right: 1, amplitude: 2n },
      ],
    })).rejects.toThrow('duplicate');
    const product = await fixture();
    expect(() => evaluateProductV2(product, 0n, 0n)).toThrow('denominator');
    expect(() => evaluateProductV2(product, 1n << 127n, 1n)).toThrow('i128');
  });

  // The two transaction tests that stood here were deleted with the surface they
  // covered: a 10-account evidence plus 28-account liability-admission pair aimed
  // at two crates no program links (`dclutch-product-payoff-v2-svm`,
  // `dclutch-product-admission-contract`), whose on-chain half
  // (`programs/dclutch-product-evidence-sbf`) was already banished. They asserted
  // the packet shape of a transaction no deployed program could execute.
});
