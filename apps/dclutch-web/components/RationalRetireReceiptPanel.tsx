'use client';

import { FormEvent, useState } from 'react';

import {
  type RationalRetireReceiptCandidateV4,
  type RationalRetireReceiptInspectionV4,
  buildRationalRetireReceiptCandidateV4,
  compactRetireReceiptSummaryV4,
  inspectRationalRetireReceiptV4,
} from '@/lib/rationalRetireReceiptV4';
import { SolanaRpcClient } from '@/lib/rpc';
import { requestWalletTransactionSignatureV1, submitSignedTransactionV1, type WalletSignedTransactionV1 } from '@/lib/walletHandoff';

import Anchor from './Anchor';
import WalletDirectory, { useWalletDirectoryV1 } from './WalletDirectory';
import { useDeploymentFieldV1 } from '@/lib/deploymentStore';

type State = Readonly<{ kind: 'idle' | 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; message: string; inspection: RationalRetireReceiptInspectionV4 }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'operation refused without a usable reason';
}

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function short(value: string): string { return value.length <= 20 ? value : `${value.slice(0, 10)}…${value.slice(-8)}`; }

function fixedAddresses(text: string): string[] {
  const addresses = text.split(/\r?\n/).map((line) => line.trim()).filter((line) => line.length > 0);
  if (addresses.length !== 38) throw new Error(`Hot frame needs exactly 38 address lines; received ${addresses.length}`);
  return addresses;
}

export default function RationalRetireReceiptPanel() {
  const [endpoint, setEndpoint] = useDeploymentFieldV1((d) => d.endpoint);
  const [payer, setPayer] = useState('');
  const [lookupTable, setLookupTable] = useState('');
  const [fixed, setFixed] = useState('');
  const [walletStatus, setWalletStatus] = useState('No wallet identity has been requested.');
  const wallets = useWalletDirectoryV1();
  const [state, setState] = useState<State>({ kind: 'idle', message: 'No compact lifecycle state has been read.' });
  const [candidate, setCandidate] = useState<RationalRetireReceiptCandidateV4 | null>(null);
  const [buildStatus, setBuildStatus] = useState('Authenticate one finalized fixed38 route first.');
  const [signed, setSigned] = useState<WalletSignedTransactionV1 | null>(null);
  const [submittedSignature, setSubmittedSignature] = useState<string | null>(null);
  const inspection = state.kind === 'ready' ? state.inspection : null;
  const summary = inspection === null ? null : compactRetireReceiptSummaryV4(inspection);

  function adoptIdentity(address: string) {
    setPayer(address);
    setWalletStatus(`${address} · identity only; signing remains release-gated`);
  }

  async function inspect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setCandidate(null); setSigned(null); setSubmittedSignature(null);
    setState({ kind: 'loading', message: 'Reacquiring fixed38, SetV2/CapabilityV4, descriptor, Product, Claims, RentCreditV2, Token-2022, sparse vacancy, and ALT state…' });
    try {
      const next = await inspectRationalRetireReceiptV4(new SolanaRpcClient(endpoint), {
        payer, fixedAccounts: fixedAddresses(fixed), lookupTable,
      });
      setState({ kind: 'ready', inspection: next, message: `Exact representation K=${next.representationWidth}, terminal N=${next.resultOutcomeCount}, support S=${next.support.length} retirement candidate joined at finalized slot ${next.observedSlot}.` });
      setBuildStatus('Chain-derived family and child digests are ready. Acquire one fresh blockhash to compile the v0 candidate.');
    } catch (error) { setState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` }); }
  }

  async function build() {
    if (inspection === null) return;
    setCandidate(null); setSigned(null); setSubmittedSignature(null);
    try {
      const latest = await new SolanaRpcClient(endpoint).latestMutationBlockhash(inspection.observedSlot);
      const next = buildRationalRetireReceiptCandidateV4(inspection, latest.blockhash);
      setCandidate(next);
      setBuildStatus(`Unsigned v0 candidate: ${next.wireBytes.length} / 1232 bytes · ${next.accountLocks} / 64 unique account locks · ${next.loadedAddresses} ALT addresses · exact Claims frame 20+4×${next.supportCount}. You can now review, sign, and submit this checked route.`);
    } catch (error) { setBuildStatus(`Refused: ${errorMessage(error)}`); }
  }

  function download() {
    if (candidate === null) return;
    const blob = new Blob([candidate.wireBytes as BlobPart], { type: 'application/octet-stream' });
    const link = document.createElement('a'); link.href = URL.createObjectURL(blob);
    link.download = `dclutch-retire-receipt-v4-unsigned-k${candidate.supportCount}-${candidate.wireBytes.length}.bin`;
    link.click(); URL.revokeObjectURL(link.href);
  }

  async function sign() {
    if (candidate === null || candidate.executionStatus !== 'ready') return;
    try {
      const next = await requestWalletTransactionSignatureV1(new SolanaRpcClient(endpoint), wallets.handoff(endpoint), candidate.transaction, payer);
      setSigned(next); setSubmittedSignature(null);
      setWalletStatus(next.complete ? 'Your retirement transaction is fully signed and ready to submit.' : 'Your wallet added one signature; more signatures are required.');
    } catch (error) { setWalletStatus(`Refused: ${errorMessage(error)}`); }
  }

  async function submit() {
    if (signed === null || !signed.complete) return;
    try {
      setWalletStatus('Submitting your exact signed retirement transaction…');
      const signature = await submitSignedTransactionV1(new SolanaRpcClient(endpoint), signed.wireBytes);
      setSubmittedSignature(signature);
      setWalletStatus('Your retirement transaction was accepted for processing. Check its finalized result in the explorer.');
    } catch (error) { setWalletStatus(`Refused: ${errorMessage(error)}`); }
  }

  return <>
    <section className="trade-v3-card">
      <header><span>04</span><div><h2>Retire a zero-supply Structured receipt from descriptor truth</h2><p>Your wallet never supplies N, K, outcomes, coefficients, custody owners, Position addresses, or rent arithmetic. The browser derives them from finalized Product, descriptor, Claims, Token-2022, and lifecycle-scoped RentCreditV2 state.</p></div></header>
      <div className="trade-v3-evidence"><article><span>Family wire</span><strong>fixed 400</strong><small>DCRLHC04 · RetireReceipt only</small></article><article><span>Claims frame</span><strong>20 + 4S</strong><small>S is ordered nonzero support within representation K</small></article><article><span>Current lock limit</span><strong>S ≤ 3</strong><small>this one-shot route uses 50 + 4S unique locks; an ALT does not reduce that count</small></article><article><span>Execution</span><strong>checked release required</strong><small>signing appears only after the exact V4 route is authenticated</small></article></div>
    </section>

    <form className="trade-v3-card route-card" onSubmit={(event) => void inspect(event)}>
      <header><span>05</span><div><h2>Reacquire one exact compact route</h2><p>The 38 lines are account transport for the universal Hot frame, not caller-authored authority. Their root selection, ProgramSet, CapabilityV4, finalized artifacts, Product graph, and activated programs are hostile-decoded before any candidate exists.</p></div></header>
      <div className="direct-form-grid"><label><span>Finalized RPC endpoint</span><input type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label><label><span>Transaction payer</span><input required value={payer} onChange={(event) => setPayer(event.target.value.trim())} /></label><label><span>Canonical address lookup table</span><input required value={lookupTable} onChange={(event) => setLookupTable(event.target.value.trim())} /></label></div>
      <label><span>Hot fixed38 addresses · one canonical base58 address per line</span><textarea required rows={12} value={fixed} onChange={(event) => setFixed(event.target.value)} /></label>
      <WalletDirectory directory={wallets} purpose="payer identity" onConnected={adoptIdentity} />
      <div className="direct-actions"><button disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading finalized lifecycle…' : 'Authenticate compact RetireReceipt'}</button></div>
      <p className="direct-status">{walletStatus}</p><p className="direct-status" aria-live="polite">{state.message}</p>
      {inspection && summary && <div className="trade-v3-evidence"><article><span>Representation K / result N / support S</span><strong>{inspection.representationWidth} / {inspection.resultOutcomeCount} / {inspection.support.length}</strong><small>support outcomes {inspection.support.map((row) => row.outcome).join(', ')}</small></article><article><span>Claims frame</span><strong>{summary.frame}</strong><small>{inspection.claimsAccounts.length} exact child metas</small></article><article><span>Descriptor</span><strong>{summary.descriptorId.slice(0, 16)}…</strong><small>receipt {short(inspection.receiptMint)}</small></article><article><span>Rent credit</span><strong>{inspection.rentCreditBefore.toString()} lamports</strong><small>+ {inspection.receiptLamports.toString()} receipt lamports on close</small></article></div>}
    </form>

    <section className="trade-v3-card signing-card">
      <header><span>06</span><div><h2>Review, sign, and submit the checked packet</h2><p>The exact v0+ALT packet is available only after this page authenticates the compact V4 capability, every selected artifact, the active Trading release, and the lookup table at one finalized floor. Your wallet signs the same packet you review.</p></div></header>
      <button type="button" disabled={inspection === null} onClick={() => void build()}>Build exact unsigned v0 + ALT candidate</button><p className="direct-status" aria-live="polite">{buildStatus}</p>
      <div className="direct-actions"><button type="button" disabled={candidate === null || candidate.executionStatus !== 'ready'} onClick={() => void sign()}>Sign retirement transaction</button><button type="button" disabled={signed === null || !signed.complete} onClick={() => void submit()}>Submit fully signed retirement</button><button type="button" disabled={candidate === null} onClick={download}>Download unsigned candidate</button></div>
      {submittedSignature !== null && <p className="direct-status"><Anchor href={`/explorer?view=transaction&q=${encodeURIComponent(submittedSignature)}`}>Open your retirement transaction in the explorer →</Anchor></p>}
      {candidate && <div className="direct-output"><dl><div><dt>Packet</dt><dd>{candidate.wireBytes.length} / 1232 bytes · {candidate.loadedAddresses} ALT addresses</dd></div><div><dt>Unique account locks</dt><dd>{candidate.accountLocks} / 64</dd></div><div><dt>Account frame</dt><dd>{candidate.accountCount} metas before message de-duplication</dd></div><div><dt>Signer</dt><dd>{candidate.requiredSigners.join(', ')}</dd></div><div><dt>Execution status</dt><dd>checked route ready for wallet signature</dd></div></dl><details className="trade-v3-bytes"><summary>Exact compact wire</summary><dl><div><dt>400-byte family · base64</dt><dd>{base64(candidate.familyBytes)}</dd></div><div><dt>528-byte Hot data · base64</dt><dd>{base64(candidate.outerBytes)}</dd></div><div><dt>v0 packet · base64</dt><dd>{base64(candidate.wireBytes)}</dd></div></dl></details></div>}
    </section>
  </>;
}
