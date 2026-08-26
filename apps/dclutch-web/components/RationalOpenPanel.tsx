'use client';

import { FormEvent, useState } from 'react';

import {
  type RationalOpenCandidateV4,
  type RationalOpenChainInspectionV4,
  buildRationalOpenCandidateV4,
  inspectRationalOpenChainV4,
  rationalOpenChainSummaryV4,
} from '@/lib/rationalOpenChainV4';
import { type RationalOpenActionV3 } from '@/lib/rationalOpenHotV3';
import { SolanaRpcClient } from '@/lib/rpc';
import { requestReadonlyWalletIdentityV1 } from '@/lib/walletHandoff';

type State = Readonly<{ kind: 'idle' | 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; message: string; inspection: RationalOpenChainInspectionV4 }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'operation refused without a usable reason';
}

function rawU64(text: string): bigint {
  if (!/^[1-9][0-9]*$/.test(text)) throw new Error('raw quantity must be canonical positive decimal atoms');
  const value = BigInt(text);
  if (value > 18_446_744_073_709_551_615n) throw new Error('raw quantity exceeds u64::MAX');
  return value;
}

function outcome(text: string, action: RationalOpenActionV3): number | null {
  if (action === 'issue-structured' || action === 'unwrap-structured') return null;
  if (!/^(0|[1-9][0-9]*)$/.test(text)) throw new Error('selected outcome must be one canonical u32 index');
  const value = Number(text);
  if (!Number.isSafeInteger(value) || value > 0xffff_ffff) throw new Error('selected outcome exceeds u32::MAX');
  return value;
}

function fixedAddresses(text: string): string[] {
  const addresses = text.split(/\r?\n/).map((line) => line.trim()).filter((line) => line.length > 0);
  if (addresses.length !== 38) throw new Error(`Hot frame needs exactly 38 address lines; received ${addresses.length}`);
  return addresses;
}

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

export default function RationalOpenPanel() {
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [payer, setPayer] = useState('');
  const [actor, setActor] = useState('');
  const [descriptor, setDescriptor] = useState('');
  const [lookupTable, setLookupTable] = useState('');
  const [fixed, setFixed] = useState('');
  const [action, setAction] = useState<RationalOpenActionV3>('denominate');
  const [quantity, setQuantity] = useState('');
  const [selected, setSelected] = useState('0');
  const [walletStatus, setWalletStatus] = useState('No wallet identity has been requested.');
  const [state, setState] = useState<State>({ kind: 'idle', message: 'No Rational open route has been read.' });
  const [candidate, setCandidate] = useState<RationalOpenCandidateV4 | null>(null);
  const [buildStatus, setBuildStatus] = useState('Authenticate one finalized CapabilityV4 route first.');
  const inspection = state.kind === 'ready' ? state.inspection : null;
  const summary = inspection === null ? null : rationalOpenChainSummaryV4(inspection);
  const isStructured = action === 'issue-structured' || action === 'unwrap-structured';

  async function connectWallet() {
    try {
      const identity = await requestReadonlyWalletIdentityV1(window.solana);
      if (payer === '') setPayer(identity.address);
      if (actor === '') setActor(identity.address);
      setWalletStatus(`${identity.address} · identity only; no signing request`);
    } catch (error) { setWalletStatus(`Refused: ${errorMessage(error)}`); }
  }

  async function inspect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setCandidate(null);
    setState({ kind: 'loading', message: 'Reacquiring fixed38, SetV2/CapabilityV4, six artifacts, Product N, descriptor graph, Claims, Token-2022, replay rent, Profile11, and ALT…' });
    try {
      const next = await inspectRationalOpenChainV4(new SolanaRpcClient(endpoint), {
        action, payer, actor, descriptorId: descriptor, lookupTable, fixedAccounts: fixedAddresses(fixed),
        rawQuantity: rawU64(quantity), selectedOutcome: outcome(selected, action),
      });
      setState({ kind: 'ready', inspection: next, message: `Exact ${next.action} family joined at finalized slot ${next.observedSlot}; Product N=${next.outcomeCount}.` });
      setBuildStatus('The exact family/Claims specialization is ready for one fresh-blockhash packet-fit attempt.');
    } catch (error) { setState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` }); }
  }

  async function build() {
    if (inspection === null) return;
    setCandidate(null);
    try {
      const latest = await new SolanaRpcClient(endpoint).latestBlockhash(inspection.observedSlot);
      const next = buildRationalOpenCandidateV4(inspection, latest.blockhash);
      setCandidate(next);
      setBuildStatus(`Unsigned v0 candidate: ${next.wireBytes.length} / 1232 bytes · ${next.loadedAddresses} ALT addresses · Claims ${next.logicalClaimsAccounts} logical → ${next.physicalClaimsAccounts} physical.`);
    } catch (error) { setBuildStatus(`Refused: ${errorMessage(error)}`); }
  }

  function download() {
    if (candidate === null || inspection === null) return;
    const blob = new Blob([candidate.wireBytes as BlobPart], { type: 'application/octet-stream' });
    const link = document.createElement('a'); link.href = URL.createObjectURL(blob);
    link.download = `dclutch-${inspection.action}-v4-unsigned-n${inspection.outcomeCount}-${candidate.wireBytes.length}.bin`;
    link.click(); URL.revokeObjectURL(link.href);
  }

  return <>
    <section className="trade-v3-card">
      <header><span>04</span><div><h2>Open native shards or a Structured receipt from one Product graph</h2><p>Denominate, reconstitute, issue, and unwrap share one variable-width family. The browser derives N, coefficients, Mints, custody, Positions, replay revisions, and physical aliases from finalized chain state; users choose only an action and raw-atom quantity.</p></div></header>
      <div className="trade-v3-evidence"><article><span>Selected routes</span><strong>36 logical</strong><small>one Product outcome</small></article><article><span>Structured routes</span><strong>32 + 4N</strong><small>zero coefficients remain zero-delta rows</small></article><article><span>Economics</span><strong>raw u64</strong><small>display decimals never scale values</small></article><article><span>Execution</span><strong>release-gated</strong><small>compile/export; signing disabled</small></article></div>
    </section>

    <form className="trade-v3-card route-card" onSubmit={(event) => void inspect(event)}>
      <header><span>05</span><div><h2>Hostile-decode one exact CapabilityV4 open route</h2><p>The descriptor digest and action are discovery coordinates, not economic authority. SetV2 selects the 600-byte CapabilityV4; that record selects TokenBehaviorV2 and all six finalized artifacts before Product and Claims state can enter the request.</p></div></header>
      <div className="direct-form-grid"><label><span>Finalized RPC endpoint</span><input type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label><label><span>Transaction payer</span><input required value={payer} onChange={(event) => setPayer(event.target.value.trim())} /></label><label><span>Representation actor</span><input required value={actor} onChange={(event) => setActor(event.target.value.trim())} /></label><label><span>Action</span><select value={action} onChange={(event) => { setAction(event.target.value as RationalOpenActionV3); setCandidate(null); }}><option value="denominate">Denominate native claim</option><option value="reconstitute">Reconstitute native claim</option><option value="issue-structured">Issue Structured receipt</option><option value="unwrap-structured">Unwrap Structured receipt</option></select></label><label><span>Raw u64 quantity · atoms</span><input inputMode="numeric" required value={quantity} onChange={(event) => setQuantity(event.target.value.trim())} /></label>{!isStructured && <label><span>Selected Product outcome · zero based</span><input inputMode="numeric" required value={selected} onChange={(event) => setSelected(event.target.value.trim())} /></label>}<label><span>Representation descriptor digest · 64 hex</span><input required value={descriptor} onChange={(event) => setDescriptor(event.target.value.trim().toLowerCase())} /></label><label><span>Address lookup table</span><input required value={lookupTable} onChange={(event) => setLookupTable(event.target.value.trim())} /></label></div>
      <label><span>Hot fixed38 addresses · one canonical base58 address per line</span><textarea required rows={12} value={fixed} onChange={(event) => setFixed(event.target.value)} /></label>
      <div className="direct-actions"><button type="button" onClick={() => void connectWallet()}>Connect payer / actor identity</button><button disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading finalized open state…' : 'Authenticate exact open route'}</button></div>
      <p className="direct-status">{walletStatus}</p><p className="direct-status" aria-live="polite">{state.message}</p>
      {inspection && summary && <div className="trade-v3-evidence"><article><span>Action / Product width</span><strong>{inspection.action} · N={inspection.outcomeCount}</strong><small>{summary.quantity}</small></article><article><span>Claims geometry</span><strong>{summary.claims}</strong><small>Profile11 canonical representatives</small></article><article><span>Display metadata</span><strong>{summary.decimals}</strong><small>no exponentiation or rounding</small></article><article><span>Descriptor</span><strong>{summary.descriptor.slice(0, 16)}…</strong><small>Capability {summary.capability.slice(0, 16)}…</small></article></div>}
    </form>

    <section className="trade-v3-card signing-card">
      <header><span>06</span><div><h2>Attempt the real Solana packet bound, then stop at the release gate</h2><p>Every action is compiled into its exact Hot family and Claims child. v0+ALT construction either produces a packet at or below 1232 bytes or refuses with its actual encoding/size reason. A packet is evidence, not permission to sign an unattested outer.</p></div></header>
      <button type="button" disabled={inspection === null} onClick={() => void build()}>Build bounded unsigned v0 + ALT candidate</button><p className="direct-status" aria-live="polite">{buildStatus}</p>
      <div className="direct-actions"><button type="button" disabled title={candidate?.refusal ?? 'No checked positive common-Hot release is active.'}>Wallet signing blocked by checked-release gate</button><button type="button" disabled={candidate === null} onClick={download}>Download unsigned candidate</button></div>
      {candidate && inspection && <div className="direct-output"><dl><div><dt>Packet</dt><dd>{candidate.wireBytes.length} / 1232 bytes · {candidate.loadedAddresses} ALT addresses</dd></div><div><dt>Wallet signers</dt><dd>{candidate.requiredSigners.join(', ')}</dd></div><div><dt>Raw deltas</dt><dd>receipt {inspection.family.rawReceiptDelta.toString()} · shards [{inspection.family.rawShardDeltas.map((value) => value.toString()).join(', ')}]</dd></div><div><dt>Execution status</dt><dd>{candidate.refusal}</dd></div></dl><details className="trade-v3-bytes"><summary>Exact family and packet bytes</summary><dl><div><dt>Family · base64</dt><dd>{base64(inspection.family.familyBytes)}</dd></div><div><dt>Claims child · base64</dt><dd>{base64(inspection.family.childRequest)}</dd></div><div><dt>v0 packet · base64</dt><dd>{base64(candidate.wireBytes)}</dd></div></dl></details></div>}
    </section>
  </>;
}
