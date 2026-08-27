'use client';

import Link from 'next/link';
import { FormEvent, useState } from 'react';

import { fromHex, hex } from '@/lib/bytes';
import {
  compileProductV2,
  evaluateProductV2,
  parseProductKnots,
  parseProductTerms,
  productInteger,
  PRODUCT_V2_BYTES,
  type CompiledProductV2,
} from '@/lib/productV2';
import { buildAdmissionInstructionV2 } from '@/lib/productRuntimeV2Admission';

function message(error: unknown): string { return error instanceof Error ? error.message : 'Product V2 operation failed without a usable refusal reason'; }
function base64(bytes: Uint8Array): string { let binary = ''; for (let offset = 0; offset < bytes.length; offset += 16_384) binary += String.fromCharCode(...bytes.slice(offset, offset + 16_384)); return btoa(binary); }

type AdmissionPreflight = Readonly<{ receipt: string; bump: number; requestHex: string; accounts: ReadonlyArray<string> }>;

export default function ProductV2Studio() {
  const [productId, setProductId] = useState(''); const [domainId, setDomainId] = useState(''); const [unitId, setUnitId] = useState(''); const [payoutScale, setPayoutScale] = useState(''); const [knotDenominator, setKnotDenominator] = useState(''); const [knots, setKnots] = useState(''); const [terms, setTerms] = useState('');
  const [compiled, setCompiled] = useState<CompiledProductV2 | null>(null); const [compileStatus, setCompileStatus] = useState('No Product has been authored or compiled.');
  const [sampleNumerator, setSampleNumerator] = useState(''); const [sampleDenominator, setSampleDenominator] = useState(''); const [sample, setSample] = useState<string | null>(null);
  const [admissionProgram, setAdmissionProgram] = useState(''); const [registry, setRegistry] = useState('');
  const [recordDigests, setRecordDigests] = useState({ product: '', domain: '', portfolio: '' });
  const [recordAccounts, setRecordAccounts] = useState({ productRaw: '', productStaging: '', domainRaw: '', domainStaging: '', portfolioRaw: '', portfolioStaging: '' });
  const [admission, setAdmission] = useState<AdmissionPreflight | null>(null); const [admissionStatus, setAdmissionStatus] = useState('No admission request has been composed.');

  function composeAdmission(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setAdmission(null);
    try {
      const built = buildAdmissionInstructionV2({
        programId: admissionProgram, registry,
        productRaw: recordAccounts.productRaw, productStaging: recordAccounts.productStaging,
        resultDomainRaw: recordAccounts.domainRaw, resultDomainStaging: recordAccounts.domainStaging,
        portfolioRaw: recordAccounts.portfolioRaw, portfolioStaging: recordAccounts.portfolioStaging,
      }, {
        productDigest: fromHex(recordDigests.product, 'Product record digest'),
        resultDomainDigest: fromHex(recordDigests.domain, 'result-domain record digest'),
        portfolioDigest: fromHex(recordDigests.portfolio, 'portfolio record digest'),
      });
      setAdmission(Object.freeze({ receipt: built.receipt, bump: built.receiptBump, requestHex: hex(built.requestBytes), accounts: Object.freeze(built.instruction.keys.map((key) => `${key.pubkey.toBase58()}${key.isWritable ? ' · writable' : ''}`)) }));
      setAdmissionStatus('Composed the exact admission request and the account frame the adapter validates. Nothing was read from a chain, allocated, signed, or submitted.');
    } catch (error) { setAdmissionStatus(`Refused: ${message(error)}`); }
  }

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
    <header className="product-nav"><Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link><nav><Link href="/direct">Direct</Link><Link href="/general">General</Link><Link className="active" href="/product-v2">Product V2</Link><Link href="/release">Release</Link></nav><span className="preview-control"><i className="preview-dot" />exact rational</span></header>
    <section className="market-heading"><div><div className="market-kicker"><span>signed rational line</span><span>runtime width 2..16</span><span>one floor</span></div><h1>Author the payoff as data. Read back exactly what it denotes.</h1><p>Knots are signed i128 numerators over one positive u64 denominator. Terms are canonical constants, ramps, or tents with clamped tails. Coordinates remain exact rationals; the sole rounding boundary is the final floor into scaled payout atoms after interpolation.</p></div></section>
    <form className="direct-card" onSubmit={compile}><div className="direct-card-heading"><span>01</span><div><h2>Compile one canonical Product V2 record</h2><p>No market or deployment is implied. This first stage owns only exact semantic data and its content identity.</p></div></div>
      <div className="direct-form-grid"><label><span>Product scalar ID · nonzero u64</span><input required inputMode="numeric" value={productId} onChange={(event) => setProductId(event.target.value.trim())} /></label><label><span>Domain scalar ID · nonzero u64</span><input required inputMode="numeric" value={domainId} onChange={(event) => setDomainId(event.target.value.trim())} /></label><label><span>Coordinate-unit scalar ID · nonzero u64</span><input required inputMode="numeric" value={unitId} onChange={(event) => setUnitId(event.target.value.trim())} /></label><label><span>Payout scale · atoms per unit</span><input required inputMode="numeric" value={payoutScale} onChange={(event) => setPayoutScale(event.target.value.trim())} /></label><label><span>Common knot denominator · nonzero u64</span><input required inputMode="numeric" value={knotDenominator} onChange={(event) => setKnotDenominator(event.target.value.trim())} /></label></div>
      <div className="product-author-grid"><label><span>Strictly increasing signed knot numerators · one i128 per line</span><textarea required value={knots} onChange={(event) => setKnots(event.target.value)} spellCheck={false} /></label><label><span>Payoff terms · one canonical expression per line</span><textarea required value={terms} onChange={(event) => setTerms(event.target.value)} spellCheck={false} /><small>constant amplitude<br />ramp-up left-index right-index amplitude<br />ramp-down left-index right-index amplitude<br />tent left-index peak-index right-index amplitude</small></label></div>
      <button type="submit">Compile exact Product bytes</button><p className="direct-status" aria-live="polite">{compileStatus}</p>
      {compiled && <div className="direct-output product-compiled"><dl><div><dt>Canonical record identity</dt><dd>{compiled.digestHex}</dd></div><div><dt>Exact ABI</dt><dd>576 bytes · {compiled.input.knots.length} active knots · {compiled.input.terms.length} active terms</dd></div><div><dt>Conservative liability</dt><dd>{compiled.liabilityBound.toString()} scaled payout atoms</dd></div></dl><label><span>{PRODUCT_V2_BYTES}-byte Product V2 record · base64</span><textarea readOnly value={base64(compiled.bytes)} /></label><div className="product-region-grid">{compiled.regions.map((region) => <article className="registered-state-card" key={`${region.label}-${region.left}`}><span className="eyebrow">{region.label}</span><h3>{region.left} → {region.right}</h3><p>Exact rational coordinates; shape-specific endpoint clamp.</p></article>)}</div></div>}
    </form>
    {compiled && <form className="direct-card" onSubmit={evaluate}><div className="direct-card-heading"><span>02</span><div><h2>Evaluate without quantizing the coordinate</h2><p>The preview uses the compiled bytes&apos; exact rational semantics. Only the final nonnegative payout interpolation is floored.</p></div></div><div className="direct-form-grid"><label><span>Signed result numerator · i128</span><input required value={sampleNumerator} onChange={(event) => setSampleNumerator(event.target.value.trim())} /></label><label><span>Positive result denominator · u64</span><input required value={sampleDenominator} onChange={(event) => setSampleDenominator(event.target.value.trim())} /></label></div><button type="submit">Evaluate exact coordinate</button><p className="direct-status" aria-live="polite">{sample ?? 'No coordinate has been evaluated.'}</p></form>}
    <form className="direct-card" onSubmit={composeAdmission} aria-labelledby="runtime-v2-admission"><div className="direct-card-heading"><span>03</span><div><h2 id="runtime-v2-admission">Compose one Runtime V2 admission request</h2><p>These digests are <strong>not</strong> the payoff identity above. They are the content digests of three Registry-finalized records — the Product record, its result domain, and its portfolio — which the deployed adapter authenticates for owner, PDA, hash, rent exemption and staging vacancy at its own boundary. This stage composes the request and derives the receipt address the program itself recomputes. It reads no chain and submits nothing.</p></div></div>
      <div className="direct-form-grid"><label><span>Admission program · dclutch-product-runtime-v2-sbf</span><input required value={admissionProgram} onChange={(event) => setAdmissionProgram(event.target.value.trim())} /></label><label><span>Registry program</span><input required value={registry} onChange={(event) => setRegistry(event.target.value.trim())} /></label></div>
      <div className="direct-form-grid"><label><span>Product record digest · 32 hex bytes</span><input required value={recordDigests.product} onChange={(event) => setRecordDigests({ ...recordDigests, product: event.target.value.trim() })} /></label><label><span>Result-domain record digest</span><input required value={recordDigests.domain} onChange={(event) => setRecordDigests({ ...recordDigests, domain: event.target.value.trim() })} /></label><label><span>Portfolio record digest</span><input required value={recordDigests.portfolio} onChange={(event) => setRecordDigests({ ...recordDigests, portfolio: event.target.value.trim() })} /></label></div>
      <div className="direct-form-grid"><label><span>Product raw account</span><input required value={recordAccounts.productRaw} onChange={(event) => setRecordAccounts({ ...recordAccounts, productRaw: event.target.value.trim() })} /></label><label><span>Product staging account</span><input required value={recordAccounts.productStaging} onChange={(event) => setRecordAccounts({ ...recordAccounts, productStaging: event.target.value.trim() })} /></label><label><span>Result-domain raw account</span><input required value={recordAccounts.domainRaw} onChange={(event) => setRecordAccounts({ ...recordAccounts, domainRaw: event.target.value.trim() })} /></label><label><span>Result-domain staging account</span><input required value={recordAccounts.domainStaging} onChange={(event) => setRecordAccounts({ ...recordAccounts, domainStaging: event.target.value.trim() })} /></label><label><span>Portfolio raw account</span><input required value={recordAccounts.portfolioRaw} onChange={(event) => setRecordAccounts({ ...recordAccounts, portfolioRaw: event.target.value.trim() })} /></label><label><span>Portfolio staging account</span><input required value={recordAccounts.portfolioStaging} onChange={(event) => setRecordAccounts({ ...recordAccounts, portfolioStaging: event.target.value.trim() })} /></label></div>
      <div className="registered-facts creation-boundary"><p><strong>One magic, two wires</strong> DCLTPRQ2 names both this live 112-byte admission request and a dead evaluator request of the same width. The dead one wrote 1 at byte 10; this decoder requires zero across bytes 10..16. Every coordinate here is generated from the live crate, never restated.</p><p><strong>Frame</strong> Exactly nine accounts, all distinct: a writable program-owned receipt, the executable Registry, six read-only record accounts, and the rent sysvar. A duplicate or a wrong count is refused here rather than on chain.</p></div>
      <button type="submit">Compose exact admission request</button><p className="direct-status" aria-live="polite">{admissionStatus}</p>
      {admission && <div className="direct-output"><dl><div><dt>Derived receipt · bump</dt><dd>{admission.receipt} · {admission.bump}</dd></div><div><dt>Account frame</dt><dd>{admission.accounts.length} accounts</dd></div></dl><label><span>112-byte DCLTPRQ2 admission request · hex</span><textarea readOnly value={admission.requestHex} /></label><ol className="registered-refusals">{admission.accounts.map((entry, index) => <li key={entry}>{index}. {entry}</li>)}</ol><p className="direct-refusal">This is an instruction, not a transaction: no fee payer, no blockhash, no signature slot. A separate boundary must authenticate the three records against the Registry before any of this is worth signing.</p></div>}
    </form>
    <footer className="product-footer"><span>Static clients are untrusted projections</span><span>No private keys · no signing · no submission</span></footer>
  </main>;
}
