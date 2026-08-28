import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { compileProductV2, parseProductKnots, parseProductTerms } from '@/lib/productV2';

import PayoutShape, { payoutShapeKnotsFromCompiledProductV2 } from './PayoutShape';

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
    const knots = payoutShapeKnotsFromCompiledProductV2(await tentProduct());
    expect(knots.map((knot) => [knot.numerator, knot.payoutAtoms])).toEqual([
      ['0', '0'],
      ['50', '1000000'],
      ['100', '0'],
    ]);
  });

  it('renders the curve with one marker per knot, the payout-scale law line, and the exact table twin', async () => {
    const compiled = await tentProduct();
    const html = renderToStaticMarkup(<PayoutShape
      knots={payoutShapeKnotsFromCompiledProductV2(compiled)}
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
    expect(html).toContain('Exact payout at every knot');
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
      knots={payoutShapeKnotsFromCompiledProductV2(compiled)}
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
