'use client';

import PageShell from '@/components/PageShell';
import ConsoleHeader from '@/components/ConsoleHeader';
import { FormEvent, useMemo, useState } from 'react';

import { fromHex, hex } from '@/lib/bytes';
import { useDeploymentFieldV1 } from '@/lib/deploymentStore';
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
import PayoutShape, { payoutShapeKnotsFromCompiledProductV2 } from '@/components/charts/PayoutShape';
import {
  DerivedProvenance,
  DerivedValue,
  Hex64Field,
  PubkeyField,
  U64Field,
} from '@/components/operator/OperatorFields';
import { deriveProductV2AccountsV1, effectiveAccountV1 } from '@/components/operator/productV2Accounts';
import CommandRunbook from '@/components/operator/CommandRunbook';
import SplineProductArtifactInspector from '@/components/SplineProductArtifactInspector';

function message(error: unknown): string { return error instanceof Error ? error.message : 'Product V2 operation failed without a usable refusal reason'; }
function base64(bytes: Uint8Array): string { let binary = ''; for (let offset = 0; offset < bytes.length; offset += 16_384) binary += String.fromCharCode(...bytes.slice(offset, offset + 16_384)); return btoa(binary); }

type AdmissionPreflight = Readonly<{ receipt: string; bump: number; requestHex: string; accounts: ReadonlyArray<string> }>;

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
  const [sampleNumerator, setSampleNumerator] = useState(''); const [sampleDenominator, setSampleDenominator] = useState(''); const [sample, setSample] = useState<string | null>(null);
  const [admissionProgram, setAdmissionProgram] = useState(''); const [registry, setRegistry] = useDeploymentFieldV1((d) => d.programs.registry);
  const [deployedRegistry] = useDeploymentFieldV1((d) => d.programs.registry);
  const [recordDigests, setRecordDigests] = useState({ product: '', domain: '', portfolio: '' });
  const [accountOverrides, setAccountOverrides] = useState({ productRaw: '', productStaging: '', domainRaw: '', domainStaging: '', portfolioRaw: '', portfolioStaging: '' });
  const [admission, setAdmission] = useState<AdmissionPreflight | null>(null); const [admissionStatus, setAdmissionStatus] = useState('No admission request has been composed.');

  /**
   * OPERATOR_FORMS_V1 §3.2. These six were six `required` inputs, and each one
   * is `findProgramAddressSync([seed, pinned schema, digest], registry)` over
   * fields already on this form -- so the console was asking for an answer it
   * could compute, then refusing every mismatch. No chain read is involved.
   */
  const derivedAccounts = useMemo(
    () => deriveProductV2AccountsV1(registry, recordDigests),
    [registry, recordDigests],
  );

  function composeAdmission(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setAdmission(null);
    try {
      const account = (slot: keyof typeof accountOverrides) =>
        effectiveAccountV1(derivedAccounts?.[slot] ?? null, accountOverrides[slot]);
      const built = buildAdmissionInstructionV2({
        programId: admissionProgram, registry,
        productRaw: account('productRaw'), productStaging: account('productStaging'),
        resultDomainRaw: account('domainRaw'), resultDomainStaging: account('domainStaging'),
        portfolioRaw: account('portfolioRaw'), portfolioStaging: account('portfolioStaging'),
      }, {
        productDigest: fromHex(recordDigests.product, 'Product record digest'),
        resultDomainDigest: fromHex(recordDigests.domain, 'result-domain record digest'),
        portfolioDigest: fromHex(recordDigests.portfolio, 'portfolio record digest'),
      });
      setAdmission(Object.freeze({ receipt: built.receipt, bump: built.receiptBump, requestHex: hex(built.requestBytes), accounts: Object.freeze(built.instruction.keys.map((key) => `${key.pubkey.toBase58()}${key.isWritable ? ' · writable' : ''}`)) }));
      setAdmissionStatus('Composed the admission request and the account frame the adapter validates. Nothing was read from a chain, allocated, signed, or submitted.');
    } catch (error) { setAdmissionStatus(`Refused: ${message(error)}`); }
  }

  async function compile(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setCompiled(null); setSample(null); setCompileStatus('Compiling…');
    try {
      const value = await compileProductV2({ productId: productInteger(productId, 'product scalar ID'), domainId: productInteger(domainId, 'domain scalar ID'), coordinateUnitId: productInteger(unitId, 'coordinate-unit scalar ID'), payoutScale: productInteger(payoutScale, 'payout scale'), knotDenominator: productInteger(knotDenominator, 'knot denominator'), knots: parseProductKnots(knots), terms: parseProductTerms(terms) });
      setCompiled(value); setCompileStatus(`Compiled ${value.input.knots.length} knots and ${value.input.terms.length} canonical terms into exactly ${value.bytes.length} bytes.`);
    } catch (error) { setCompileStatus(`Refused: ${message(error)}`); }
  }

  function evaluate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setSample(null); if (compiled === null) return;
    try { const numerator = productInteger(sampleNumerator, 'sample numerator'); const denominator = productInteger(sampleDenominator, 'sample denominator'); setSample(`${evaluateProductV2(compiled, numerator, denominator)} scaled payout atoms at ${numerator}/${denominator}`); } catch (error) { setSample(`Refused: ${message(error)}`); }
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
      {compiled && <div className="direct-output product-compiled"><dl><div><dt>Canonical record identity</dt><dd>{compiled.digestHex}</dd></div><div><dt>Exact ABI</dt><dd>576 bytes · {compiled.input.knots.length} active knots · {compiled.input.terms.length} active terms</dd></div><div><dt>Conservative liability</dt><dd>{compiled.liabilityBound.toString()} scaled payout atoms</dd></div></dl><label><span>{PRODUCT_V2_BYTES}-byte Product V2 record · base64</span><textarea readOnly value={base64(compiled.bytes)} /></label><p className="direct-status">Payoff interpolation segments, derived from the knots above. Not the outcome partition.</p><div className="product-region-grid">{compiled.regions.map((region) => <article className="registered-state-card" key={`${region.label}-${region.left}`}><span className="eyebrow">{region.label}</span><h3>{region.left} → {region.right}</h3><p>Rational coordinates; shape-specific endpoint clamp.</p></article>)}</div>{/* FE-CHART mount: the compiled record drawn exactly — knot evaluations, not samples. */}<PayoutShape knots={payoutShapeKnotsFromCompiledProductV2(compiled)} knotDenominator={compiled.input.knotDenominator.toString()} payoutScale={compiled.input.payoutScale.toString()} caption="What this payoff pays across its result domain: exact evaluations at every knot, straight lines between them, flat clamped tails beyond." /></div>}
    </form>

    {compiled && <form className="direct-card" onSubmit={evaluate}><div className="direct-card-heading"><span>02</span><div><h2>Evaluate without quantizing the coordinate</h2><p>The compiled bytes’ exact rational semantics. Only the final nonnegative payout interpolation is floored.</p></div></div><div className="direct-form-grid"><label><span>Signed result numerator · i128</span><input required value={sampleNumerator} onChange={(event) => setSampleNumerator(event.target.value.trim())} spellCheck={false} /><small className="feed-forward">Signed, and wider than u64 — this one stays a plain field until the vocabulary carries an i128 type.</small></label><U64Field label="Positive result denominator · u64" value={sampleDenominator} onChange={setSampleDenominator} noun="result denominator" min={1n} required /></div><button type="submit">Evaluate exact coordinate</button><p className="direct-status" aria-live="polite">{sample ?? 'No coordinate has been evaluated.'}</p></form>}

    <form className="direct-card" onSubmit={composeAdmission} aria-labelledby="runtime-v2-admission"><div className="direct-card-heading"><span>03</span><div><h2 id="runtime-v2-admission">Compose one Runtime V2 admission request</h2><p>The content digests of three Registry-finalized records — the Product record, its result domain, and its portfolio — which the adapter authenticates for owner, PDA, hash, rent exemption, and staging vacancy. These are <strong>not</strong> the payoff identity compiled in step 01. Nothing is read from a chain and nothing is submitted.</p></div></div>

      <fieldset className="operator-act">
        <legend>The two programs this request names</legend>
        <div className="operator-act-grid">
          <PubkeyField label="Admission program · dclutch-product-runtime-v2-sbf" value={admissionProgram} onChange={setAdmissionProgram} required
            provenance="The deployed Product Runtime V2 adapter. It is not one of the seven protocol roles, so the deployment manifest cannot fill it — take it from your deployment plan." />
          <PubkeyField label="Registry program" value={registry} onChange={setRegistry} required
            identify={(address) => address === deployedRegistry ? 'the Registry of the deployment this browser is pointed at' : null}
            provenance={<DerivedProvenance derived={deployedRegistry === '' ? null : deployedRegistry} value={registry}
              source="the deployment this browser is pointed at"
              absent="Pick a cluster in the header to fill this, or paste the Registry program address." />} />
        </div>
      </fieldset>

      <fieldset className="operator-act">
        <legend>The three record digests</legend>
        <p>Each is the SHA-256 of a Registry-finalized record&rsquo;s account data — not the step 01 payoff identity. The six account addresses below are derived from these.</p>
        <div className="operator-act-grid">
          <Hex64Field label="Product record digest · 32 hex bytes" value={recordDigests.product} onChange={(next) => setRecordDigests({ ...recordDigests, product: next })} required />
          <Hex64Field label="Result-domain record digest" value={recordDigests.domain} onChange={(next) => setRecordDigests({ ...recordDigests, domain: next })} required />
          <Hex64Field label="Portfolio record digest" value={recordDigests.portfolio} onChange={(next) => setRecordDigests({ ...recordDigests, portfolio: next })} required />
        </div>
      </fieldset>

      <fieldset className="operator-act">
        <legend>The six record accounts, derived</legend>
        <p>Each address is <code>findProgramAddressSync</code> over the Registry program, a schema identity pinned in this build, and one digest above — the same arithmetic the adapter runs on chain. Nothing here is read from a chain.</p>
        <div className="operator-act-grid">
          {RECORD_SLOTS_V1.map((record) => <DerivedValue
            key={record.slot}
            label={record.label}
            value={derivedAccounts?.[record.slot] ?? null}
            derivation={`Derived from the Registry program, ${record.schema}, and ${DIGEST_FOR_SLOT_V1[record.slot]}.`}
          />)}
        </div>
        <details className="operator-override">
          <summary>Override a derived account</summary>
          <p>An operator is sometimes the person who knows a record moved. Anything set here replaces the derived address for that slot, and the request is composed from what you set. Leave a field empty to keep the derivation.</p>
          <div className="operator-act-grid">
            {RECORD_SLOTS_V1.map((record) => <PubkeyField
              key={record.slot}
              label={record.label}
              value={accountOverrides[record.slot]}
              onChange={(next) => setAccountOverrides({ ...accountOverrides, [record.slot]: next })}
              provenance={accountOverrides[record.slot] === '' ? 'Empty — the derived address above is what will be sent.' : 'Set — this replaces the derived address above.'}
            />)}
          </div>
        </details>
      </fieldset>

      <div className="registered-facts creation-boundary"><p><strong>One magic, two wires</strong> DCLTPRQ2 names both this live 112-byte admission request and a dead evaluator request of the same width. The dead one wrote 1 at byte 10; this decoder requires zero across bytes 10..16.</p><p><strong>Frame</strong> Exactly nine accounts, all distinct: a writable program-owned receipt, the executable Registry, six read-only record accounts, and the rent sysvar. A duplicate or a wrong count is refused.</p></div>
      <button type="submit">Compose exact admission request</button><p className="direct-status" aria-live="polite">{admissionStatus}</p>
      {admission && <div className="direct-output"><dl><div><dt>Derived receipt · bump</dt><dd>{admission.receipt} · {admission.bump}</dd></div><div><dt>Account frame</dt><dd>{admission.accounts.length} accounts</dd></div></dl><label><span>112-byte DCLTPRQ2 admission request · hex</span><textarea readOnly value={admission.requestHex} /></label><ol className="registered-refusals">{admission.accounts.map((entry, index) => <li key={entry}>{index}. {entry}</li>)}</ol><p className="direct-refusal">This is an instruction, not a transaction: no fee payer, no blockhash, no signature slot. The three records must be authenticated against the Registry before it is worth signing.</p></div>}
    </form>
    <footer className="product-footer"><span>No private keys · no signing · no submission</span></footer>
  </PageShell>;
}
