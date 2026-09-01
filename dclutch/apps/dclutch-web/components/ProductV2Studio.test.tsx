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
    expect(html).toContain('Compile one admitted spline Product graph');
    expect(html).toContain('compile_spline_product_records_v3');
    expect(html).toContain('DCLTPGT1 price-gate certificate');
    expect(html).toContain('product-basis.bin');
    expect(html).toContain('price-gate.bin');
    expect(html).toContain('--bootstrap-bin &quot;$SUCCESSOR&quot; product spline');
    expect(html).toContain('Key-free canonical compilation');
    expect(html).toContain('does not found a Market');
    expect(html).toContain('Nothing is rounded until the named cumulative-floor boundary.');
    expect(html).toContain('No Product has been authored or compiled.');
    expect(html).toContain('No private keys · no signing · no submission');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('sample state');
    expect(html).not.toContain('value="1"');
    expect(html).not.toContain('Unsigned atomic v0 transaction');
    expect(html).toContain('does not reproduce the five-record spline graph or price-gate authority above');
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
    expect(html).toContain('It does not reimplement the spline compiler or price-gate theorem.');
    expect(html).toContain('Nothing is read from a chain.');
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

describe('Product V2 admission: what it asks for, and what it computes', () => {
  const html = renderToStaticMarkup(<ProductV2Studio />);

  it('derives the six record accounts instead of asking for them', () => {
    // OPERATOR_FORMS_V1 §3.2. These were six `required` inputs; each is a pure
    // function of the Registry program, a pinned schema, and a digest on this
    // same form. The console used to ask, then refuse every mismatch.
    for (const label of [
      'Product raw account', 'Product staging account',
      'Result-domain raw account', 'Result-domain staging account',
      'Portfolio raw account', 'Portfolio staging account',
    ]) {
      expect(html).toContain(label);
    }
    expect(html).toContain('Derived from the Registry program');
    expect(html).toContain('the pinned Product record schema');
    expect(html).toContain('the pinned result-domain schema');
    expect(html).toContain('the pinned portfolio schema');
  });

  it('says what it is waiting for rather than showing six blank boxes', () => {
    expect(html).toContain('waiting on the fields above');
  });

  it('keeps the derivation honest about not touching a chain', () => {
    expect(html).toContain('Nothing here is read from a chain.');
    expect(html).toContain('the same arithmetic the adapter runs on chain');
  });

  it('keeps every derived address overridable, one click down', () => {
    // Precision is the feature: the capability survives, it just stops being
    // the default. The drawer is real depth, not a hidden control.
    expect(html).toContain('<summary>Override a derived account</summary>');
    expect(html).toContain('Leave a field empty to keep the derivation.');
    expect(html).toContain('Empty — the derived address above is what will be sent.');
  });

  it('groups the fields under the act each one feeds', () => {
    expect(html).toContain('<legend>The two programs this request names</legend>');
    expect(html).toContain('<legend>The three record digests</legend>');
    expect(html).toContain('<legend>The six record accounts, derived</legend>');
  });

  it('fills the Registry from the deployment and says so, in the field', () => {
    // The DERIVE rule's first state. The default deployment carries a Registry,
    // so this field arrives filled, provenanced, and resolved to a named
    // account -- rather than as an empty box the operator retypes.
    expect(html).toContain('<strong>Filled from the deployment this browser is pointed at.</strong>');
    expect(html).toContain('You can paste a different value; this line will say so.');
    expect(html).toContain('the Registry of the deployment this browser is pointed at');
  });

  it('says why the adapter address cannot be filled the same way', () => {
    // The third state of the DERIVE rule: when nothing can be derived, the
    // field must still name where the value comes from.
    expect(html).toContain('It is not one of the seven protocol roles');
    expect(html).toContain('take it from your deployment plan');
  });

  it('still names every scalar the record is identified by', () => {
    for (const label of [
      'Product scalar ID · nonzero u64', 'Domain scalar ID · nonzero u64',
      'Coordinate-unit scalar ID · nonzero u64', 'Payout scale · atoms per unit',
      'Common knot denominator · nonzero u64',
    ]) {
      expect(html).toContain(label);
    }
  });

  it('does not offer evaluation before anything has been compiled', () => {
    // Step 02 is gated on a compiled record, so its fields are absent here.
    // The i128 coordinate field it carries is named debt -- the vocabulary has
    // no signed-wide type yet -- and that is asserted where step 02 renders,
    // not here, so this guard cannot pass for the wrong reason.
    expect(html).not.toContain('Signed result numerator');
    expect(html).not.toContain('Evaluate exact coordinate');
  });

  it('leaves the six account slots with no required input of their own', () => {
    // The conversion's load-bearing structural claim: the derived block is not
    // an input, and the override drawer's six inputs are optional. Six
    // `required` controls became zero, and no fact left the page.
    const overrideBlock = html.slice(html.indexOf('operator-override'));
    expect(overrideBlock).not.toContain('required=""');
  });
});
