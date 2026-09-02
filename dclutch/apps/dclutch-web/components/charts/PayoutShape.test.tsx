import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import {
  loadProductPayoffV2WasmV1,
  payoutCurveKnotsV1,
  type PayoutCurveKnotV1,
} from '@/lib/productPayoffV2Evaluation';
import { compileProductV2, parseProductKnots, parseProductTerms, type CompiledProductV2 } from '@/lib/productV2';

import PayoutShape from './PayoutShape';

const wasmPath = fileURLToPath(new URL('../../lib/generated/productPayoffV2Wasm/product_payoff_v2_bg.wasm', import.meta.url));

/**
 * The knots the chart draws, evaluated by the compiled Rust codec.
 *
 * This used to be a synchronous call into `evaluateProductV2`, a TypeScript
 * reimplementation of the payoff living in the chart module itself. The values
 * asserted below are unchanged, which is the point: the curve is the same
 * curve, drawn by the authority that settles it.
 */
async function knotsOf(compiled: CompiledProductV2): Promise<ReadonlyArray<PayoutCurveKnotV1>> {
  const boundary = await loadProductPayoffV2WasmV1(
    (async () => new Response(new Uint8Array(readFileSync(wasmPath)))) as unknown as typeof fetch,
  );
  return payoutCurveKnotsV1(boundary, compiled, (bytes) => Buffer.from(bytes).toString('base64'));
}

async function tentProduct() {
  return compileProductV2({
    productId: 7n,
    domainId: 9n,
    coordinateUnitId: 3n,
    payoutScale: 1_000_000n,
    knotDenominator: 1n,
    knots: parseProductKnots('0\n50\n100'),
    terms: parseProductTerms('tent 0 1 2 1000000'),
  });
}

describe('PayoutShape', () => {
  it('derives exact knot values from a compiled Product V2 — the control polygon, not samples', async () => {
    const knots = await knotsOf(await tentProduct());
    expect(knots.map((knot) => [knot.numerator, knot.payoutAtoms])).toEqual([
      ['0', '0'],
      ['50', '1000000'],
      ['100', '0'],
    ]);
  });

  it('renders the curve with one marker per knot, the payout-scale law line, and the exact table twin', async () => {
    const compiled = await tentProduct();
    const html = renderToStaticMarkup(<PayoutShape
      knots={await knotsOf(compiled)}
      knotDenominator="1"
      payoutScale="1000000"
      caption="What this payoff pays across its result domain, exact at every knot."
    />);
    expect(html.split('<circle').length - 1).toBe(3);
    expect(html).toContain('<polyline');
    expect(html).toContain('payout scale · 1000000 atoms');
    expect(html).toContain('pays 1000000 of 1000000 scaled payout atoms at 50/1');
    // The clamped tails are stated as facts, not left to inference.
    expect(html).toContain('below 0/1');
    expect(html).toContain('above 100/1');
    expect(html).toContain('Exact numbers');
  });

  it('projects i128-range knot numerators without losing the plot', () => {
    const big = '170141183460469231731687303715884105727';
    const html = renderToStaticMarkup(<PayoutShape
      knots={[
        { numerator: '-170141183460469231731687303715884105728', payoutAtoms: '0' },
        { numerator: big, payoutAtoms: '5' },
      ]}
      knotDenominator="1000000000"
      payoutScale="5"
      caption="Full-domain ramp."
    />);
    expect(html).toContain(`pays 5 of 5 scaled payout atoms at ${big}/1000000000`);
  });

  it('carries the your-position note into the readout when a position is named', async () => {
    const compiled = await tentProduct();
    const html = renderToStaticMarkup(<PayoutShape
      knots={await knotsOf(compiled)}
      knotDenominator="1"
      payoutScale="1000000"
      caption="Shaded for a held position."
      position={{ atoms: '250', note: 'you hold 250 claim atoms against this shape' }}
    />);
    expect(html).toContain('you hold 250 claim atoms against this shape');
  });

  it('says why in one plain sentence when the shape has fewer than two knots', () => {
    const html = renderToStaticMarkup(<PayoutShape
      knots={[]}
      knotDenominator="1"
      payoutScale="1"
      caption="unused"
      emptyReason="No payoff record has been compiled yet, so there is no shape to draw."
    />);
    expect(html).toContain('No payoff record has been compiled yet, so there is no shape to draw.');
    expect(html).not.toContain('<svg');
  });
});
