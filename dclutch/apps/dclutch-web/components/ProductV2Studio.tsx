'use client';

import PageShell from '@/components/PageShell';
import ConsoleHeader from '@/components/ConsoleHeader';
import { FormEvent, useMemo, useState } from 'react';

import { fromHex, hex } from '@dclutch/sdk/bytes';
import { useDeploymentFieldV1 } from '@/lib/deploymentStore';
import {
  evaluateProductPayoffV2WasmV1,
  loadProductPayoffV2WasmV1,
  payoutCurveKnotsV1,
  type PayoutCurveKnotV1,
  type ProductPayoffV2WasmV1,
} from '@/lib/productPayoffV2Evaluation';
import {
  compileProductV2,
  parseProductKnots,
  parseProductTerms,
  productInteger,
  PRODUCT_V2_BYTES,
  type CompiledProductV2,
} from '@dclutch/sdk/productV2';
import PayoutShape from '@/components/charts/PayoutShape';
import {
  DerivedProvenance,
  DerivedValue,
  Hex64Field,
  PubkeyField,
  U64Field,
} from '@/components/operator/OperatorFields';
import CommandRunbook from '@/components/operator/CommandRunbook';
import SplineProductArtifactInspector from '@/components/SplineProductArtifactInspector';

function message(error: unknown): string { return error instanceof Error ? error.message : 'Product V2 operation failed without a usable refusal reason'; }
function base64(bytes: Uint8Array): string { let binary = ''; for (let offset = 0; offset < bytes.length; offset += 16_384) binary += String.fromCharCode(...bytes.slice(offset, offset + 16_384)); return btoa(binary); }

export const SPLINE_PRODUCT_RUNBOOK_V1 = `dclutch-terminal \\
  --bootstrap-bin "$SUCCESSOR" product spline \\
  --input "$SPLINE_PRODUCT_INPUT" \\
  --output-dir "$PRODUCT_GRAPH"`;

/** The six record accounts, in the order `validate_frame` reads them. */
const RECORD_SLOTS_V1 = Object.freeze([
  Object.freeze({ slot: 'productRaw', label: 'Product raw account', schema: 'the pinned Product record schema' }),
  Object.freeze({ slot: 'productStaging', label: 'Product staging account', schema: 'the pinned Product record schema' }),
  Object.freeze({ slot: 'domainRaw', label: 'Result-domain raw account', schema: 'the pinned result-domain schema' }),
  Object.freeze({ slot: 'domainStaging', label: 'Result-domain staging account', schema: 'the pinned result-domain schema' }),
  Object.freeze({ slot: 'portfolioRaw', label: 'Portfolio raw account', schema: 'the pinned portfolio schema' }),
  Object.freeze({ slot: 'portfolioStaging', label: 'Portfolio staging account', schema: 'the pinned portfolio schema' }),
] as const);

const DIGEST_FOR_SLOT_V1: Readonly<Record<string, string>> = Object.freeze({
  productRaw: 'the Product record digest', productStaging: 'the Product record digest',
  domainRaw: 'the result-domain record digest', domainStaging: 'the result-domain record digest',
  portfolioRaw: 'the portfolio record digest', portfolioStaging: 'the portfolio record digest',
});

export default function ProductV2Studio() {
  const [productId, setProductId] = useState(''); const [domainId, setDomainId] = useState(''); const [unitId, setUnitId] = useState(''); const [payoutScale, setPayoutScale] = useState(''); const [knotDenominator, setKnotDenominator] = useState(''); const [knots, setKnots] = useState(''); const [terms, setTerms] = useState('');
  const [compiled, setCompiled] = useState<CompiledProductV2 | null>(null); const [compileStatus, setCompileStatus] = useState('No Product has been authored or compiled.');
  /**
   * The compiled payoff evaluator, and the curve it produced.
   *
   * Both used to be synchronous, because both went through `evaluateProductV2`
   * -- a TypeScript reimplementation of `ProductPayoffV2::evaluate_rational`,
   * which is the arithmetic the chain settles with. The Studio was showing a
   * payout the protocol had never been asked about. The boundary is loaded
   * once, on the first compile, and the curve is evaluated in ONE call across
   * it because a curve is its knots and nothing here is sampled.
   */
  const [payoffBoundary, setPayoffBoundary] = useState<ProductPayoffV2WasmV1 | null>(null);
  const [shapeKnots, setShapeKnots] = useState<ReadonlyArray<PayoutCurveKnotV1> | null>(null);
  const [sampleNumerator, setSampleNumerator] = useState(''); const [sampleDenominator, setSampleDenominator] = useState(''); const [sample, setSample] = useState<string | null>(null);

  /** Load the digest-pinned evaluator once, and keep it. */
  async function payoffEvaluator(): Promise<ProductPayoffV2WasmV1> {
    if (payoffBoundary !== null) return payoffBoundary;
    const loaded = await loadProductPayoffV2WasmV1();
    setPayoffBoundary(loaded);
    return loaded;
  }

  async function compile(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setCompiled(null); setSample(null); setShapeKnots(null); setCompileStatus('Compiling…');
    try {
      const value = await compileProductV2({ productId: productInteger(productId, 'product scalar ID'), domainId: productInteger(domainId, 'domain scalar ID'), coordinateUnitId: productInteger(unitId, 'coordinate-unit scalar ID'), payoutScale: productInteger(payoutScale, 'payout scale'), knotDenominator: productInteger(knotDenominator, 'knot denominator'), knots: parseProductKnots(knots), terms: parseProductTerms(terms) });
      setCompiled(value); setCompileStatus(`Compiled ${value.input.knots.length} knots and ${value.input.terms.length} canonical terms into exactly ${value.bytes.length} bytes.`);
      setShapeKnots(await payoutCurveKnotsV1(await payoffEvaluator(), value, base64));
    } catch (error) { setCompileStatus(`Refused: ${message(error)}`); }
  }

  async function evaluate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setSample(null); if (compiled === null) return;
    try {
      const numerator = productInteger(sampleNumerator, 'sample numerator'); const denominator = productInteger(sampleDenominator, 'sample denominator');
      const answer = await evaluateProductPayoffV2WasmV1(await payoffEvaluator(), base64(compiled.bytes), [{ numerator, denominator }]);
      setSample(`${answer.payouts[0]} scaled payout atoms at ${numerator}/${denominator}`);
    } catch (error) { setSample(`Refused: ${message(error)}`); }
  }

  return <PageShell className="product-shell direct-workspace product-v2-studio" header={<ConsoleHeader path="/product-v2" title="Product studio" purpose="Compile the current five-record spline graph through its Rust owner, then inspect exact low-level payoff semantics." />}>
    <section className="market-heading"><div><h1>Product studio.</h1><p>The current compiler accepts exact degree-2/3 spline semantics and emits every immutable record founding needs. Nothing is rounded until the named cumulative-floor boundary.</p></div></section>

    <section className="direct-card product-spline-current" id="spline-product">
      <div className="direct-card-heading"><span>Current</span><div><h2>Compile one admitted spline Product graph</h2><p>The public CLI delegates the whole graph to <code>compile_spline_product_records_v3</code>. The browser does not reproduce its basis identity, price-gate verification, record hashes, or PDAs.</p></div></div>
      <div className="operator-route-contract">
        <article><span>Input</span><strong>One canonical JSON document</strong><p>Semantic Product/source/unit/release identities, exact cut and portfolio fractions, degree, knots, failure partition, and the complete DCLTPGT1 price-gate certificate. The input is bounded and unknown or duplicate fields refuse.</p></article>
        <article><span>Authority</span><strong>Production Rust compilers and verifier</strong><p>ProductBasisV3 derives width and semantic identity; the production price gate verifies before its digest enters the graph. The Product compiler then emits and rejoins Product, domain, and portfolio records.</p></article>
        <article><span>Result</span><strong>Five records + one machine report</strong><p><code>product.bin</code>, <code>result-domain.bin</code>, <code>portfolio.bin</code>, <code>product-basis.bin</code>, and <code>price-gate.bin</code>, with exact sizes, schemas, SHA-256 identities, and raw/staging addresses.</p></article>
        <article><span>If refused</span><strong>Fix the named input or certificate</strong><p>A relative/noncanonical path, malformed integer or identity, knot-width mismatch, forged gate, or existing output directory refuses. Atomic output means a failed compilation leaves no partial graph.</p></article>
      </div>
      <CommandRunbook label="Key-free canonical compilation" command={SPLINE_PRODUCT_RUNBOOK_V1} />
      <p className="direct-status">This authors immutable files only. It reads no chain, asks for no wallet, signs nothing, and does not found a Market.</p>
      <SplineProductArtifactInspector />
    </section>

    <form className="direct-card" onSubmit={compile}><div className="direct-card-heading"><span>Low-level</span><div><h2>Compile and inspect one Product V2 payoff record</h2><p>This browser workbench evaluates one exact payoff record. It does not reproduce the five-record spline graph or price-gate authority above.</p><p><strong>Where the payoff bends, not where the outcome changes.</strong> The segments this step reports are derived here from the knots you type, and say how the payoff interpolates between them. The outcome partition is a different list of a different length: the operator writes it into its <code>ResultDomainV2</code> record, and <code>result-domain.bin</code> in the compiler handoff is where its actual cuts can be read.</p></div></div>
      <fieldset className="operator-act">
        <legend>The scalars this record is identified by</legend>
        <div className="operator-act-grid">
          <U64Field label="Product scalar ID · nonzero u64" value={productId} onChange={setProductId} noun="product scalar ID" min={1n} required />
          <U64Field label="Domain scalar ID · nonzero u64" value={domainId} onChange={setDomainId} noun="domain scalar ID" min={1n} required />
          <U64Field label="Coordinate-unit scalar ID · nonzero u64" value={unitId} onChange={setUnitId} noun="coordinate-unit scalar ID" min={1n} required />
          <U64Field label="Payout scale · atoms per unit" value={payoutScale} onChange={setPayoutScale} noun="payout atoms per unit" min={1n} required />
          <U64Field label="Common knot denominator · nonzero u64" value={knotDenominator} onChange={setKnotDenominator} noun="knot denominator" min={1n} required />
        </div>
      </fieldset>
      <div className="product-author-grid"><label><span>Strictly increasing signed knot numerators · one i128 per line</span><textarea required value={knots} onChange={(event) => setKnots(event.target.value)} spellCheck={false} /></label><label><span>Payoff terms · one canonical expression per line</span><textarea required value={terms} onChange={(event) => setTerms(event.target.value)} spellCheck={false} /><small>constant amplitude<br />ramp-up left-index right-index amplitude<br />ramp-down left-index right-index amplitude<br />tent left-index peak-index right-index amplitude</small></label></div>
      <button type="submit">Compile exact Product bytes</button><p className="direct-status" aria-live="polite">{compileStatus}</p>
      {compiled && <div className="direct-output product-compiled"><dl><div><dt>Canonical record identity</dt><dd>{compiled.digestHex}</dd></div><div><dt>Exact ABI</dt><dd>576 bytes · {compiled.input.knots.length} active knots · {compiled.input.terms.length} active terms</dd></div><div><dt>Conservative liability</dt><dd>{compiled.liabilityBound.toString()} scaled payout atoms</dd></div></dl><label><span>{PRODUCT_V2_BYTES}-byte Product V2 record · base64</span><textarea readOnly value={base64(compiled.bytes)} /></label><p className="direct-status">Payoff interpolation segments, derived from the knots above. Not the outcome partition.</p><div className="product-region-grid">{compiled.regions.map((region) => <article className="registered-state-card" key={`${region.label}-${region.left}`}><span className="eyebrow">{region.label}</span><h3>{region.left} → {region.right}</h3><p>Rational coordinates; shape-specific endpoint clamp.</p></article>)}</div>{/* FE-CHART mount: the compiled record drawn exactly — knot evaluations from the compiled Rust codec, not samples and not a second evaluator. */}<PayoutShape knots={shapeKnots ?? []} knotDenominator={compiled.input.knotDenominator.toString()} payoutScale={compiled.input.payoutScale.toString()} caption="What this payoff pays across its result domain: exact evaluations at every knot by the compiled Rust codec, straight lines between them, flat clamped tails beyond." emptyReason="The record compiled; its payout curve is still being evaluated by the compiled Rust codec." /></div>}
    </form>

    {compiled && <form className="direct-card" onSubmit={evaluate}><div className="direct-card-heading"><span>02</span><div><h2>Evaluate without quantizing the coordinate</h2><p>The compiled bytes’ exact rational semantics. Only the final nonnegative payout interpolation is floored.</p></div></div><div className="direct-form-grid"><label><span>Signed result numerator · i128</span><input required value={sampleNumerator} onChange={(event) => setSampleNumerator(event.target.value.trim())} spellCheck={false} /><small className="feed-forward">Signed, and wider than u64 — this one stays a plain field until the vocabulary carries an i128 type.</small></label><U64Field label="Positive result denominator · u64" value={sampleDenominator} onChange={setSampleDenominator} noun="result denominator" min={1n} required /></div><button type="submit">Evaluate exact coordinate</button><p className="direct-status" aria-live="polite">{sample ?? 'No coordinate has been evaluated.'}</p></form>}

    <footer className="product-footer"><span>No private keys · no signing · no submission</span></footer>
  </PageShell>;
}
