'use client';

import Link from 'next/link';
import { FormEvent, useState } from 'react';

import { prepareCoreFoundV2, type CoreFoundInputV2, type CoreFoundPlanV2 } from '@/lib/coreFound';
import { CORE_FOUND_ACCOUNT_LABELS_V2 } from '@/lib/generated/coreFound';
import { SolanaRpcClient } from '@/lib/rpc';

type AddressField = Exclude<keyof CoreFoundInputV2, 'generation'>;
type AddressValues = Record<AddressField, string>;
type BuildState =
  | Readonly<{ kind: 'idle' | 'loading' | 'error'; message: string }>
  | Readonly<{ kind: 'ready'; plan: CoreFoundPlanV2; base64: string }>;

const ADDRESS_FIELDS: ReadonlyArray<Readonly<{ field: AddressField; label: string }>> = Object.freeze([
  { field: 'payer', label: 'Payer' },
  { field: 'registryProgram', label: 'Registry program' },
  { field: 'activationCache', label: 'Release activation cache' },
  { field: 'rentCredit', label: 'RentCredit' },
  { field: 'realmRecord', label: 'Realm raw record' },
  { field: 'productRecord', label: 'Product Runtime V2 raw' },
  { field: 'resultDomainRecord', label: 'Result domain raw' },
  { field: 'portfolioRecord', label: 'Portfolio raw' },
  { field: 'sourceMaterialRecord', label: 'SourceMaterialV2 raw' },
  { field: 'capabilityManifestRecord', label: 'Capability manifest raw' },
  { field: 'executionReleaseSetRecord', label: 'Execution release set raw' },
]);

function emptyAddresses(): AddressValues {
  return Object.fromEntries(ADDRESS_FIELDS.map(({ field }) => [field, ''])) as AddressValues;
}

function canonicalU64(value: string): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error('generation must be a canonical unsigned integer');
  const parsed = BigInt(value);
  if (parsed > 0xffff_ffff_ffff_ffffn) throw new Error('generation exceeds u64');
  return parsed;
}

function failure(error: unknown): string {
  return error instanceof Error ? error.message : 'construction failed without a usable refusal reason';
}

function encodeBase64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function compact(value: string): string {
  return value.length > 24 ? `${value.slice(0, 10)}…${value.slice(-9)}` : value;
}

export default function CoreFoundWorkspace() {
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [addresses, setAddresses] = useState<AddressValues>(emptyAddresses);
  const [generation, setGeneration] = useState('0');
  const [state, setState] = useState<BuildState>({
    kind: 'idle',
    message: 'No transaction has been constructed. Enter chain-derived record addresses to begin.',
  });

  function update(field: AddressField, value: string): void {
    setAddresses((current) => ({ ...current, [field]: value.trim() }));
  }

  async function construct(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    setState({ kind: 'loading', message: 'Reacquiring immutable releases, Product semantics, and all 31 accounts at finalized commitment…' });
    try {
      const plan = await prepareCoreFoundV2(new SolanaRpcClient(endpoint), {
        ...addresses,
        generation: canonicalU64(generation),
      });
      setState({ kind: 'ready', plan, base64: encodeBase64(plan.wireBytes) });
    } catch (error) {
      setState({ kind: 'error', message: `Refused: ${failure(error)}` });
    }
  }

  const ready = state.kind === 'ready' ? state : null;
  return <main className="product-shell direct-workspace found-workspace">
    <header className="product-nav"><Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link><nav><Link className="active" href="/found">Create</Link><Link href="/product-v2">Product</Link><Link href="/release">Release</Link><Link href="/workbench">Workbench</Link></nav><div className="preview-control"><span className="preview-dot" /> finalized RPC · unsigned only</div></header>

    <section className="market-heading found-heading"><div><div className="market-kicker"><span>Core Found · Runtime V2</span><span>Real 31-account frame</span></div><h1>Found one common<br />Core Market.</h1></div><p>This is not a market mockup. The builder derives the Market from canonical Registry records, verifies runtime-width Product semantics and immutable infrastructure, reacquires the complete instruction frame, then emits one unsigned v0 packet.</p></section>

    <section className="found-boundaries" aria-label="Construction boundaries">
      <article><span>01</span><strong>Select execution</strong><p>The activation cache must select immutable Core, Registry, and Rent artifacts whose Loader observations still match.</p></article>
      <article><span>02</span><strong>Join one semantic graph</strong><p>Product, domain, portfolio, Source, Realm, capabilities, and releases are decoded from finalized Registry bytes.</p></article>
      <article><span>03</span><strong>Reacquire &amp; compile</strong><p>All 31 roles are read again at a finalized floor before the exact 72-byte request and v0 message are exported.</p></article>
    </section>

    <form className="direct-card found-form" onSubmit={construct}>
      <header className="direct-card-heading"><span>01</span><div><h2>Chain authority and record coordinates</h2><p>No program, balance, Product, or release identity is supplied by the application. Every address below is treated as untrusted input and reauthenticated.</p></div></header>
      <div className="direct-form-grid found-control-grid"><label><span>Finalized RPC endpoint</span><input required value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label><label><span>Market generation</span><input required inputMode="numeric" value={generation} onChange={(event) => setGeneration(event.target.value.trim())} /></label></div>
      <div className="direct-form-grid found-record-grid">{ADDRESS_FIELDS.map(({ field, label }) => <label key={field}><span>{label}</span><input required spellCheck={false} value={addresses[field]} onChange={(event) => update(field, event.target.value)} /></label>)}</div>
      <button type="submit" disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reacquiring Found31 authority…' : 'Construct unsigned Found v0 transaction'}</button>
      <p className="direct-status" aria-live="polite">{state.kind === 'ready' ? `Accepted at finalized slot ${state.plan.observedSlot}. The transaction remains unsigned and unsubmitted.` : state.message}</p>
    </form>

    {ready === null ? <section className="direct-card found-empty"><div className="radar"><span /></div><div><p className="eyebrow">No inferred authority</p><h2>Construction stops at the first broken join.</h2><p>Missing records, stale ELF bytes, mutable infrastructure, same-width Product substitution, account aliases, insufficient rent, and packet overflow are refusals—not warnings. No signing or submission occurs in this UI.</p></div></section> : <>
      <section className="direct-card found-result">
        <header className="direct-card-heading"><span>02</span><div><h2>Unsigned transaction ready</h2><p>It has not been signed, funded, simulated, or submitted. The sole required signer is the chain-selected payer.</p></div></header>
        <div className="found-verdict"><span>{ready.plan.infrastructureRecognition.kind}</span><strong>{ready.plan.outcomeCount.toLocaleString()} outcomes · {ready.plan.wireBytes.length} / 1,232 bytes</strong><p>An internally consistent release is not an official dClutch release unless it matches a separately supplied checked manifest.</p></div>
        <dl className="found-facts"><div><dt>Derived Market</dt><dd>{ready.plan.market}</dd></div><div><dt>Product identity</dt><dd>{ready.plan.productId}</dd></div><div><dt>Product record digest</dt><dd>{ready.plan.productRecordDigest}</dd></div><div><dt>Execution release set</dt><dd>{ready.plan.executionReleaseSetId}</dd></div><div><dt>Infrastructure profile</dt><dd>{ready.plan.infrastructureProfile}</dd></div><div><dt>Core / Registry / Rent</dt><dd>{compact(ready.plan.coreProgram)} · {compact(ready.plan.registryProgram)} · {compact(ready.plan.rentProgram)}</dd></div><div><dt>Market rent top-up</dt><dd>{ready.plan.marketRentTopUp} lamports</dd></div><div><dt>Blockhash validity</dt><dd>through block height {ready.plan.lastValidBlockHeight}</dd></div></dl>
        <label><span>Unsigned v0 transaction · base64</span><textarea className="found-packet" readOnly value={ready.base64} /></label>
        <div className="found-export"><a download={`dclutch-found-${ready.plan.market}.tx`} href={`data:application/octet-stream;base64,${ready.base64}`}>Download unsigned packet</a><span>No signing or submission occurs in this UI.</span></div>
      </section>

      <section className="direct-card found-accounts">
        <header className="direct-card-heading"><span>03</span><div><h2>Exact account projection</h2><p>The order below is the instruction ABI. Only payer and the new Market are writable; only payer signs.</p></div></header>
        <ol>{ready.plan.accountAddresses.map((address, index) => <li key={address}><span>{index.toString().padStart(2, '0')}</span><strong>{CORE_FOUND_ACCOUNT_LABELS_V2[index]}</strong><code>{address}</code><small>{index === 0 ? 'writable · signer' : index === 1 ? 'writable' : 'read only'}</small></li>)}</ol>
      </section>
    </>}
  </main>;
}
