'use client';

import Link from 'next/link';
import { FormEvent, useState } from 'react';

import {
  compileProductV2,
  evaluateProductV2,
  parseProductKnots,
  parseProductTerms,
  productInteger,
  PRODUCT_V2_BYTES,
  type CompiledProductV2,
} from '@/lib/productV2';

function message(error: unknown): string { return error instanceof Error ? error.message : 'Product V2 operation failed without a usable refusal reason'; }
function base64(bytes: Uint8Array): string { let binary = ''; for (let offset = 0; offset < bytes.length; offset += 16_384) binary += String.fromCharCode(...bytes.slice(offset, offset + 16_384)); return btoa(binary); }

export default function ProductV2Studio() {
  const [productId, setProductId] = useState(''); const [domainId, setDomainId] = useState(''); const [unitId, setUnitId] = useState(''); const [payoutScale, setPayoutScale] = useState(''); const [knotDenominator, setKnotDenominator] = useState(''); const [knots, setKnots] = useState(''); const [terms, setTerms] = useState('');
  const [compiled, setCompiled] = useState<CompiledProductV2 | null>(null); const [compileStatus, setCompileStatus] = useState('No Product has been authored or compiled.');
  const [sampleNumerator, setSampleNumerator] = useState(''); const [sampleDenominator, setSampleDenominator] = useState(''); const [sample, setSample] = useState<string | null>(null);

  async function compile(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setCompiled(null); setSample(null); setCompileStatus('Checking exact integers, canonical term order, runtime width, and fixed-layout bytes…');
    try {
      const value = await compileProductV2({ productId: productInteger(productId, 'product scalar ID'), domainId: productInteger(domainId, 'domain scalar ID'), coordinateUnitId: productInteger(unitId, 'coordinate-unit scalar ID'), payoutScale: productInteger(payoutScale, 'payout scale'), knotDenominator: productInteger(knotDenominator, 'knot denominator'), knots: parseProductKnots(knots), terms: parseProductTerms(terms) });
      setCompiled(value); setCompileStatus(`Compiled ${value.input.knots.length} knots and ${value.input.terms.length} canonical terms into exactly ${value.bytes.length} bytes.`);
    } catch (error) { setCompileStatus(`Refused: ${message(error)}`); }
  }

  function evaluate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setSample(null); if (compiled === null) return;
    try { const numerator = productInteger(sampleNumerator, 'sample numerator'); const denominator = productInteger(sampleDenominator, 'sample denominator'); setSample(`${evaluateProductV2(compiled, numerator, denominator)} scaled payout atoms at ${numerator}/${denominator}`); } catch (error) { setSample(`Refused: ${message(error)}`); }
  }

  return <main className="product-shell direct-workspace product-v2-studio">
    <header className="product-nav"><Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link><nav><Link href="/direct">Direct</Link><Link href="/economic">Economic</Link><Link href="/general">General</Link><Link className="active" href="/product-v2">Product V2</Link><Link href="/release">Release</Link></nav><span className="preview-control"><i className="preview-dot" />exact rational</span></header>
    <section className="market-heading"><div><div className="market-kicker"><span>signed rational line</span><span>runtime width 2..16</span><span>one floor</span></div><h1>Author the payoff as data. Read back exactly what it denotes.</h1><p>Knots are signed i128 numerators over one positive u64 denominator. Terms are canonical constants, ramps, or tents with clamped tails. Coordinates remain exact rationals; the sole rounding boundary is the final floor into scaled payout atoms after interpolation.</p></div></section>
    <form className="direct-card" onSubmit={compile}><div className="direct-card-heading"><span>01</span><div><h2>Compile one canonical Product V2 record</h2><p>No market or deployment is implied. This first stage owns only exact semantic data and its content identity.</p></div></div>
      <div className="direct-form-grid"><label><span>Product scalar ID · nonzero u64</span><input required inputMode="numeric" value={productId} onChange={(event) => setProductId(event.target.value.trim())} /></label><label><span>Domain scalar ID · nonzero u64</span><input required inputMode="numeric" value={domainId} onChange={(event) => setDomainId(event.target.value.trim())} /></label><label><span>Coordinate-unit scalar ID · nonzero u64</span><input required inputMode="numeric" value={unitId} onChange={(event) => setUnitId(event.target.value.trim())} /></label><label><span>Payout scale · atoms per unit</span><input required inputMode="numeric" value={payoutScale} onChange={(event) => setPayoutScale(event.target.value.trim())} /></label><label><span>Common knot denominator · nonzero u64</span><input required inputMode="numeric" value={knotDenominator} onChange={(event) => setKnotDenominator(event.target.value.trim())} /></label></div>
      <div className="product-author-grid"><label><span>Strictly increasing signed knot numerators · one i128 per line</span><textarea required value={knots} onChange={(event) => setKnots(event.target.value)} spellCheck={false} /></label><label><span>Payoff terms · one canonical expression per line</span><textarea required value={terms} onChange={(event) => setTerms(event.target.value)} spellCheck={false} /><small>constant amplitude<br />ramp-up left-index right-index amplitude<br />ramp-down left-index right-index amplitude<br />tent left-index peak-index right-index amplitude</small></label></div>
      <button type="submit">Compile exact Product bytes</button><p className="direct-status" aria-live="polite">{compileStatus}</p>
      {compiled && <div className="direct-output product-compiled"><dl><div><dt>Canonical record identity</dt><dd>{compiled.digestHex}</dd></div><div><dt>Exact ABI</dt><dd>576 bytes · {compiled.input.knots.length} active knots · {compiled.input.terms.length} active terms</dd></div><div><dt>Conservative liability</dt><dd>{compiled.liabilityBound.toString()} scaled payout atoms</dd></div></dl><label><span>{PRODUCT_V2_BYTES}-byte Product V2 record · base64</span><textarea readOnly value={base64(compiled.bytes)} /></label><div className="product-region-grid">{compiled.regions.map((region) => <article className="registered-state-card" key={`${region.label}-${region.left}`}><span className="eyebrow">{region.label}</span><h3>{region.left} → {region.right}</h3><p>Exact rational coordinates; shape-specific endpoint clamp.</p></article>)}</div></div>}
    </form>
    {compiled && <form className="direct-card" onSubmit={evaluate}><div className="direct-card-heading"><span>02</span><div><h2>Evaluate without quantizing the coordinate</h2><p>The preview uses the compiled bytes&apos; exact rational semantics. Only the final nonnegative payout interpolation is floored.</p></div></div><div className="direct-form-grid"><label><span>Signed result numerator · i128</span><input required value={sampleNumerator} onChange={(event) => setSampleNumerator(event.target.value.trim())} /></label><label><span>Positive result denominator · u64</span><input required value={sampleDenominator} onChange={(event) => setSampleDenominator(event.target.value.trim())} /></label></div><button type="submit">Evaluate exact coordinate</button><p className="direct-status" aria-live="polite">{sample ?? 'No coordinate has been evaluated.'}</p></form>}
    {compiled && <section className="direct-card"><div className="direct-card-heading"><span>03</span><div><h2>On-chain liability admission is not offered here</h2><p>A third stage used to compose a 10-account evidence plus 28-account liability-admission transaction. It aimed at two crates no deployed program links, so it built a packet nothing on chain could execute; it was deleted on 2026-08-27. Live Product admission belongs to the Runtime V2 program over a different wire, and this browser has no encoder for it yet. Everything above is exact authored data and needs no chain to be true.</p></div></div></section>}
    <footer className="product-footer"><span>Static clients are untrusted projections</span><span>No private keys · no signing · no submission</span></footer>
  </main>;
}
