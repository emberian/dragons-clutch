import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { compileProductV2 } from './productV2';
import {
  evaluateProductPayoffV2WasmV1,
  loadProductPayoffV2WasmV1,
  parseProductPayoffEvaluationV1,
  productPayoffEvaluationRequestV1,
} from './productPayoffV2Evaluation';

const wasmPath = fileURLToPath(new URL('./generated/productPayoffV2Wasm/product_payoff_v2_bg.wasm', import.meta.url));
const exact = () => new Uint8Array(readFileSync(wasmPath));
const load = () => loadProductPayoffV2WasmV1((async () => new Response(exact())) as unknown as typeof fetch);

function base64(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString('base64');
}

/** The Studio's own example, unchanged from the mirror's fixture. */
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

/**
 * The six values the deleted TypeScript mirror was pinned to.
 *
 * They are copied here VERBATIM from `productV2.test.ts` as it stood before the
 * removal, and that is the point: the case for deleting a second implementation
 * is not that the first one is prettier, it is that the surviving authority
 * reproduces every value the second one was ever held to. Two coordinates land
 * outside the knots, two inside a ramp, one on a knot, and one at a
 * denominator that is not the knot denominator — the case that made a
 * hand-written rational comparison worth having in the first place.
 */
const PINNED = [
  { numerator: -50n, denominator: 1n, payout: 3n },
  { numerator: -25n, denominator: 1n, payout: 78n },
  { numerator: 0n, denominator: 7n, payout: 153n },
  { numerator: 25n, denominator: 1n, payout: 128n },
  { numerator: 1n, denominator: 3n, payout: 152n },
  { numerator: 10_000n, denominator: 1n, payout: 103n },
] as const;

describe('the compiled Product V2 payoff evaluator', () => {
  it('reproduces every value the deleted TypeScript mirror was pinned to', async () => {
    const product = await fixture();
    const boundary = await load();
    const answer = await evaluateProductPayoffV2WasmV1(
      boundary,
      base64(product.bytes),
      PINNED.map(({ numerator, denominator }) => ({ numerator, denominator })),
    );
    expect(answer.payouts).toEqual(PINNED.map((pinned) => pinned.payout));
  }, 30_000);

  it('states the record scalars the codec decoded rather than the form that was typed', async () => {
    const product = await fixture();
    const boundary = await load();
    const answer = await evaluateProductPayoffV2WasmV1(boundary, base64(product.bytes), []);
    expect(answer.productId).toBe(41n);
    expect(answer.payoutScale).toBe(1_000_000n);
    expect(answer.knotDenominator).toBe(2n);
    expect(answer.knotCount).toBe(3);
    expect(answer.termCount).toBe(3);
    // The bound the TypeScript compiler computed and the bound the codec
    // decoded are the same number, read from two sides of the record.
    expect(answer.liabilityBound).toBe(product.liabilityBound);
  }, 30_000);

  it('refuses a zero denominator by the codec’s own name, not by a client guess', async () => {
    const product = await fixture();
    const boundary = await load();
    await expect(evaluateProductPayoffV2WasmV1(boundary, base64(product.bytes), [{ numerator: 0n, denominator: 0n }]))
      .rejects.toThrow(/ZeroCoordinateDenominator/);
  }, 30_000);

  it('refuses a record that is not the canonical width before evaluating anything', async () => {
    const boundary = await load();
    await expect(evaluateProductPayoffV2WasmV1(boundary, base64(new Uint8Array(10)), []))
      .rejects.toThrow(/10 bytes; the canonical record is 576/);
  }, 30_000);

  it('executes only the generated blob identity and refuses one changed byte', async () => {
    const bytes = exact();
    const changed = new Uint8Array(bytes);
    changed[changed.length - 1]! ^= 1;
    await expect(loadProductPayoffV2WasmV1((async () => new Response(changed)) as unknown as typeof fetch))
      .rejects.toThrow(/do not match the generated Rust artifact identity/);
  }, 30_000);

  it('refuses an answer that states another record width than the codec', () => {
    // A blob can match its digest and still come from a different tree. This is
    // the check that runs on every answer, not only at load.
    const forged = JSON.stringify({
      format: 'dclutch-product-payoff-v2-evaluation-v1',
      recordBytes: 512, magic: 'DCLTPAY2', version: 2, payouts: [],
    });
    expect(() => parseProductPayoffEvaluationV1(forged, 0)).toThrow(/512-byte record where the codec has 576/);
  });

  it('will not build a request above the coordinate bound the boundary declares', () => {
    const many = Array.from({ length: 4_097 }, () => ({ numerator: 0n, denominator: 1n }));
    expect(() => productPayoffEvaluationRequestV1('', many)).toThrow(/at most 4096 coordinates/);
  });
});
