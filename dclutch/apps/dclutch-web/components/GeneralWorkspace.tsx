'use client';

import Link from 'next/link';
import { FormEvent, useState } from 'react';

import {
  decodeGeneralHotReceiptV3,
  decodeGeneralSuccessorPlanDocumentV5,
  generalPlanTemplateV5,
  inspectGeneralSuccessorPlanV5,
  reacquireGeneralSuccessorStatusV5,
  transactionBytesV5,
  type GeneralChainStatusV5,
  type GeneralHotReceiptV3,
  type GeneralLocalStateStatusV3,
  type GeneralPlanInspectionV5,
  type GeneralSelectionStatusV2,
  type GeneralSettlementStatusV2,
} from '@/lib/generalPlanV5';
import { SolanaRpcClient } from '@/lib/rpc';

function message(error: unknown): string { return error instanceof Error ? error.message : 'General workflow failed without a usable refusal reason.'; }
function compact(value: string): string { return `${value.slice(0, 8)}…${value.slice(-8)}`; }

function download(bytes: Uint8Array, name: string): void {
  const blob = new Blob([new Uint8Array(bytes)], { type: 'application/octet-stream' });
  const href = URL.createObjectURL(blob); const anchor = document.createElement('a'); anchor.href = href; anchor.download = name; anchor.click(); URL.revokeObjectURL(href);
}

function SelectionStatus({ value }: Readonly<{ value: GeneralSelectionStatusV2 }>) {
  return <dl className="registered-facts"><div><dt>Selection phase / revision</dt><dd>{value.phase} / {value.revision.toString()}</dd></div><div><dt>Submitted candidates</dt><dd>{value.submittedCount}</dd></div><div><dt>Best valid submitted candidate</dt><dd>{compact(value.bestCandidateId)} · coordinate {value.bestCandidateCoordinate}</dd></div><div><dt>Comparison key</dt><dd>{value.bestFilledLots.toString()} filled lots · {value.bestQuoteSurplus.toString()} quote surplus</dd></div></dl>;
}

function SettlementStatus({ value }: Readonly<{ value: GeneralSettlementStatusV2 }>) {
  return <dl className="registered-facts"><div><dt>Settlement phase / revision</dt><dd>{value.phase} / {value.revision.toString()}</dd></div><div><dt>Progress</dt><dd>{value.nextOrder}/{value.orderCount} orders · N={value.outcomeCount}</dd></div><div><dt>Candidate</dt><dd>{compact(value.candidateId)}</dd></div><div><dt>Inventory</dt><dd>quote {value.quoteInventory.toString()} · claims [{value.inventory.map(String).join(', ')}]</dd></div><div><dt>Complete sets / terminal</dt><dd>{value.completeSetQuantity.toString()} / {value.terminalCoordinate.toString()}</dd></div></dl>;
}

function LocalStatus({ title, value }: Readonly<{ title: string; value: GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }> | null }>) {
  if (value === null) return null;
  if (value.status === 'vacant') return <article className="registered-state-card"><span className="eyebrow">{title}</span><h3>Funded vacant System account</h3><p>{value.lamports.toString()} lamports · no state bytes</p></article>;
  return <article className="registered-state-card"><span className="eyebrow">{title} · lifecycle V3</span><h3>{value.status.kind}</h3><p>bump {value.bump} · rent principal {value.rentPrincipal.toString()} · beneficiary {compact(value.beneficiary)}</p>{value.status.kind === 'selection' ? <SelectionStatus value={value.status} /> : <SettlementStatus value={value.status} />}</article>;
}

function Receipt({ value }: Readonly<{ value: GeneralHotReceiptV3 }>) {
  return <div className="direct-output"><dl><div><dt>Committed request</dt><dd>{compact(value.requestDigest)}</dd></div><div><dt>Selected CapabilityProgram</dt><dd>{compact(value.selectedProgram)}</dd></div><div><dt>Root transition</dt><dd>{compact(value.rootPrestateDigest)} → {compact(value.rootPoststateDigest)}</dd></div><div><dt>Execution / child-receipt commitment</dt><dd>{value.executionDigest}</dd></div></dl><p className="direct-refusal"><strong>Commit-last receipt accepted.</strong> It matches the imported request, Market, generation, release set, root, and selected program.</p></div>;
}

export default function GeneralWorkspace() {
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [planText, setPlanText] = useState(''); const [inspection, setInspection] = useState<GeneralPlanInspectionV5 | null>(null); const [planStatus, setPlanStatus] = useState('No operator plan has been inspected.');
  const [chain, setChain] = useState<GeneralChainStatusV5 | null>(null); const [chainStatus, setChainStatus] = useState('No RPC request has been made.');
  const [receiptText, setReceiptText] = useState(''); const [receipt, setReceipt] = useState<GeneralHotReceiptV3 | null>(null); const [receiptStatus, setReceiptStatus] = useState('No commit-last receipt has been supplied.');

  async function inspect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setInspection(null); setChain(null); setReceipt(null); setPlanStatus('Hostile-decoding the V5 operator report and unsigned v0 packet…');
    try {
      const value = await inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(planText)); setInspection(value);
      setPlanStatus(`Accepted ${value.request.action} plan · N=${value.plan.outcomeCount} · ${value.transaction.wireBytes}/1232 packet bytes · no signature present.`);
    } catch (error) { setPlanStatus(`Refused: ${message(error)}`); }
  }

  async function reacquire() {
    setChain(null); setChainStatus('Reacquiring the exact LUT, packet dependencies, and lifecycle state at a finalized floor…');
    try {
      if (inspection === null) throw new Error('inspect one exact General operator plan first');
      const value = await reacquireGeneralSuccessorStatusV5(new SolanaRpcClient(endpoint), inspection); setChain(value);
      setChainStatus(`Reacquired ${value.dependencies.dependencies.length} exact dependencies and action prestate at slot ${value.observedSlot}.`);
    } catch (error) { setChainStatus(`Refused: ${message(error)}`); }
  }

  function inspectReceipt(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setReceipt(null);
    try {
      if (inspection === null) throw new Error('inspect the matching General operator plan first');
      const value = decodeGeneralHotReceiptV3(receiptText, inspection); setReceipt(value); setReceiptStatus('Commit-last Hot receipt joins the exact imported execution.');
    } catch (error) { setReceiptStatus(`Refused: ${message(error)}`); }
  }

  return <main className="product-shell direct-workspace">
    <header className="product-nav"><Link className="brand" href="/">dClutch</Link><nav><Link href="/direct">Direct</Link><Link href="/economic">Economic</Link><Link className="active" href="/general">General</Link><Link href="/explorer">Explorer</Link></nav><span className="preview-control"><i className="preview-dot" />offline until asked</span></header>
    <section className="market-heading"><div><div className="market-kicker"><span>seven successor actions</span><span>runtime-width</span><span>unsigned v0</span></div><h1>General clearing, from candidate selection through terminal close.</h1><p>Consume the canonical Rust operator’s finalized V5 plan for Consider, Freeze, Initialize, Collect, Materialize, Distribute, or Close. The browser independently checks the packet and request, can reacquire its exact chain dependencies, and never signs or submits.</p></div></section>

    <form className="direct-card" onSubmit={inspect} aria-labelledby="general-plan">
      <div className="direct-card-heading"><span>01</span><div><h2 id="general-plan">Inspect one chain-derived operator plan</h2><p>The plan is an untrusted projection. The browser requires exact V5 fields, one unsigned packet-bounded v0 instruction, Hot38, CapabilityProgramV4/LifecycleV5 provenance, canonical lifecycle bumps, and action-specific DCE5 child-receipt order. After acceptance, download the unsigned v0 packet; no signing or submission occurs.</p></div></div>
      <label><span>GeneralSuccessorTransactionPlanV0 · bounded JSON handoff</span><textarea required rows={16} value={planText} onChange={(event) => setPlanText(event.target.value)} placeholder={generalPlanTemplateV5()} /></label>
      <button>Inspect exact unsigned plan</button><p className="direct-status" aria-live="polite">{planStatus}</p>
      {inspection && <div className="direct-output"><dl><div><dt>Action / revision</dt><dd>{inspection.request.action} / {inspection.request.expectedRevision.toString()}</dd></div><div><dt>Best-valid-submitted candidate</dt><dd>{inspection.request.candidateId === null ? 'selection Freeze · candidate comes from frozen state' : compact(inspection.request.candidateId)}</dd></div><div><dt>Manifest / source coordinates</dt><dd>manifest {inspection.request.manifestOrderIndex} · page {inspection.request.pageIndex} · execution {inspection.request.executionIndex}</dd></div><div><dt>Runtime width / scratch</dt><dd>N={inspection.plan.outcomeCount} · {inspection.plan.scratchPageCount} authenticated bank page(s)</dd></div><div><dt>Packet / ALT / signers</dt><dd>{inspection.transaction.wireBytes}/1232 bytes · {compact(inspection.plan.lookupTable)} · {inspection.plan.requiredSigners.map(compact).join(', ')}</dd></div><div><dt>Checked releases</dt><dd>Trading {compact(inspection.plan.tradingArtifactRelease)} · accelerator {compact(inspection.plan.generalArtifactRelease)} · manifest {compact(inspection.plan.checkedManifestDigest)}</dd></div><div><dt>Lifecycle</dt><dd>primary {compact(inspection.plan.lifecycle.primaryState)} · bump {inspection.plan.lifecycle.primaryStateBump}{inspection.plan.lifecycle.terminalState ? ` · terminal ${compact(inspection.plan.lifecycle.terminalState)} @ ${inspection.plan.lifecycle.terminalCoordinate}` : ''}</dd></div></dl>
        <div className="registered-state-grid">{inspection.plan.childRoutes.length === 0 ? <article className="registered-state-card"><span className="eyebrow">effect routes</span><h3>No child CPI</h3><p>Selection is persisted through the generic state-last Effect path.</p></article> : inspection.plan.childRoutes.map((route) => <article className="registered-state-card" key={route.route}><span className="eyebrow">route {route.route} · {route.role}</span><h3>logical accounts {route.accountStart}…{route.accountStart + route.accountCount - 1}</h3><p>{route.receiptDependencies.length === 0 ? 'No prior receipt dependency.' : route.receiptDependencies.map((entry) => `${entry.producerRole} route ${entry.producerRoute} · ${entry.expectedReceiptBytes} bytes`).join(' · ')}</p></article>)}</div>
        <button type="button" onClick={() => download(transactionBytesV5(inspection), `dclutch-general-${inspection.request.action}-unsigned-v5.bin`)}>Download unsigned v0 packet</button><p className="direct-refusal"><strong>No signing or submission occurred.</strong> The checked blockhash will expire; re-run the Rust operator from one finalized snapshot before any later wallet handoff.</p></div>}
    </form>

    <section className="direct-card" aria-labelledby="general-chain">
      <div className="direct-card-heading"><span>02</span><div><h2 id="general-chain">Reacquire exact chain status</h2><p>The browser resolves the imported packet’s sole lookup table, rereads every dependency, checks program executability, and hostile-decodes the action’s Trading-owned local state. It does not scan for plausible substitutes.</p></div></div>
      <div className="direct-form-grid"><label><span>Finalized RPC endpoint</span><input type="url" value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label></div><button type="button" disabled={inspection === null} onClick={() => void reacquire()}>Reacquire plan dependencies</button><p className="direct-status" aria-live="polite">{chainStatus}</p>
      {chain && <><div className="trade-v3-evidence"><article><span>Dependencies</span><strong>{chain.dependencies.dependencies.length}</strong><small>missing 0 · non-executable programs 0</small></article><article><span>Observation slot</span><strong>{chain.observedSlot}</strong><small>minimum imported floor {inspection?.plan.observedSlot.toString()}</small></article><article><span>Action prestate</span><strong>{inspection?.request.action}</strong><small>revision {inspection?.request.expectedRevision.toString()}</small></article></div><div className="registered-state-grid"><LocalStatus title="primary state" value={chain.primary} /><LocalStatus title="terminal successor" value={chain.terminal} /></div></>}
    </section>

    <form className="direct-card" onSubmit={inspectReceipt} aria-labelledby="general-receipt">
      <div className="direct-card-heading"><span>03</span><div><h2 id="general-receipt">Verify the commit-last execution receipt</h2><p>Paste the exact 280-byte HotExecutionAckV3 returned by execution. The browser joins it to the request digest, selected CapabilityProgram, Market generation, root prestate, and release set before showing success.</p></div></div>
      <label><span>HotExecutionAckV3 · canonical base64</span><textarea required value={receiptText} onChange={(event) => setReceiptText(event.target.value.trim())} /></label><button disabled={inspection === null}>Verify exact receipt</button><p className="direct-status" aria-live="polite">{receiptStatus}</p>{receipt && <Receipt value={receipt} />}
    </form>
  </main>;
}
