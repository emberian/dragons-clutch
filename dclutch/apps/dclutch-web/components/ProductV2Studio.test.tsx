import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ProductV2Studio from './ProductV2Studio';
import { classifySplineProductBundleFilesV1 } from './SplineProductArtifactInspector';

const SPLINE_BUNDLE_FILES = [
  'report.json',
  'product.bin',
  'result-domain.bin',
  'portfolio.bin',
  'product-basis.bin',
  'price-gate.bin',
] as const;

describe('Product V2 Studio presentation', () => {
  it('starts empty and exposes exact authoring, finalized authority, and an external signing boundary', () => {
    const html = renderToStaticMarkup(<ProductV2Studio />);
    expect(html).toContain('Product studio.');
    expect(html).toContain('Compile market payout records');
    expect(html).toContain('product spline');
    expect(html).toContain('DCLTPGT1 price-gate certificate');
    expect(html).toContain('product-basis.bin');
    expect(html).toContain('price-gate.bin');
    expect(html).toContain('--bootstrap-bin &quot;$SUCCESSOR&quot; product spline');
    expect(html).toContain('Compile with the CLI');
    expect(html).toContain('Use the compiled files in the market-opening workflow.');
    expect(html).toContain('Payouts use exact fractions and cumulative-floor rounding.');
    expect(html).toContain('Enter the payout terms, then compile the record.');
    expect(html).toContain('Payout design and preview');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('sample state');
    expect(html).not.toContain('value="1"');
    expect(html).not.toContain('Unsigned atomic v0 transaction');
    expect(html).toContain('Enter knots and payoff terms to create a Product V2 record and evaluate its payout.');
  });

  it('carries the compiler output into an inspectable Found39 handoff', () => {
    const html = renderToStaticMarkup(<ProductV2Studio />);
    expect(html).toContain('Inspect the compiler handoff');
    for (const file of ['report.json', 'product.bin', 'result-domain.bin', 'portfolio.bin', 'product-basis.bin', 'price-gate.bin']) {
      expect(html).toContain(file);
    }
    expect(html).toContain('Verify compiler handoff');
    expect(html).toContain('Compiler output · choose all six files');
    expect(html).toContain('<summary>Replace one file</summary>');
    expect(html).toContain('Each replacement must keep its exact compiler filename.');
    expect(html).toContain('Waiting for report.json and product.bin, result-domain.bin, portfolio.bin, product-basis.bin, price-gate.bin.');
    expect(html).toContain('inspect the outcome boundaries');
    expect(html).toContain('Load report.json and all five compiler files.');
  });

  it('classifies only one exact, complete compiler bundle', () => {
    const files = SPLINE_BUNDLE_FILES.map((name) => ({ name }));
    const bundle = classifySplineProductBundleFilesV1(files);
    expect(bundle.report.name).toBe('report.json');
    expect(bundle.resultDomain.name).toBe('result-domain.bin');
    expect(bundle.priceGate.name).toBe('price-gate.bin');
  });

  it('refuses incomplete, duplicate, and unrelated bundle selections before reading bytes', () => {
    expect(() => classifySplineProductBundleFilesV1(
      SPLINE_BUNDLE_FILES.slice(0, -1).map((name) => ({ name })),
    )).toThrow('missing price-gate.bin');
    expect(() => classifySplineProductBundleFilesV1([
      ...SPLINE_BUNDLE_FILES.map((name) => ({ name })),
      { name: 'product.bin' },
    ])).toThrow('duplicate file product.bin');
    expect(() => classifySplineProductBundleFilesV1([
      ...SPLINE_BUNDLE_FILES.map((name) => ({ name })),
      { name: 'notes.txt' },
    ])).toThrow('unexpected file notes.txt');
  });
});

describe('the studio does not present its own description as the partition', () => {
  const html = renderToStaticMarkup(<ProductV2Studio />);

  it('says the compiled regions are payoff segments, not the outcome partition', () => {
    // C-02's closing clause asks that the same artifacts the operator found be
    // explained and inspectable here. `compileProductV2` derives its regions in
    // TypeScript from the payoff KNOTS and labels them "interpolation segment
    // N"; the outcome partition is the operator's ResultDomainV2 CUTS, a
    // different list of a different length that only the chain holds. Rendering
    // the first where a reader looks for the second is the mirror this whole
    // page was supposed to stop being.
    expect(html).toContain('Knots define the payout curve.');
    expect(html).toContain('result-domain.bin');
  });

  it('points at the record that does carry the partition', () => {
    expect(html).toContain('ResultDomainV2');
  });
});
