'use client';

import PageShell from '@/components/PageShell';
import ConsoleHeader from '@/components/ConsoleHeader';
import { FormEvent, useState } from 'react';

import {
  decodeGeneralHotReceiptV3,
  decodeGeneralSuccessorPlanDocumentV5,
  generalPlanTemplateV5,
  inspectGeneralSuccessorPlanV5,
  reacquireGeneralSuccessorStatusV5,
  transactionBytesV5,
  type GeneralChainStatusV5,
  type GeneralBatchStatusV1,
  type GeneralCandidateStatusV1,
  type GeneralHotReceiptV3,
  type GeneralLifecycleStateV5,
  type GeneralLocalStateStatusV3,
  type GeneralOrderStatusV1,
  type GeneralPlanInspectionV5,
  type GeneralSelectionStatusV2,
  type GeneralSettlementStatusV2,
  type GeneralSuccessorActionV5,
  type GeneralVerifiedCandidateStatusV2,
  type GeneralVerifierStatusV2,
} from '@/lib/generalPlanV5';
import { SolanaRpcClient } from '@/lib/rpc';
import { useDeploymentFieldV1 } from '@/lib/deploymentStore';
import { GENERAL_HOT_FIXED_ACCOUNT_COUNT_V3 } from '@/lib/generated/generalSuccessorV5';

function message(error: unknown): string { return error instanceof Error ? error.message : 'General workflow failed without a usable refusal reason.'; }
function compact(value: string): string { return `${value.slice(0, 8)}…${value.slice(-8)}`; }
function quantities(value: ReadonlyArray<bigint>): string {
  if (value.length <= 8) return value.map(String).join(', ');
  return `${value.slice(0, 4).map(String).join(', ')}, …, ${value.slice(-2).map(String).join(', ')} (${value.length} outcomes)`;
}

const ACTION_READING: Readonly<Record<GeneralSuccessorActionV5, Readonly<{ label: string; effect: string; authority: string }>>> = Object.freeze({
  consider: { label: 'Compare one verified candidate', effect: 'Advance the open selection only if this candidate has the better canonical comparison key.', authority: 'Verified candidate evidence and the exact selection revision.' },
  freeze: { label: 'Freeze the current best valid submission', effect: 'Close selection around the best valid candidate already recorded in state.', authority: 'The open selection; no caller-supplied candidate identity.' },
  'initialize-settlement': { label: 'Create the settlement cursor', effect: 'Create the runtime-width cursor for the frozen candidate and begin collection.', authority: 'Frozen selection, authenticated candidate, Product N, and a funded vacant successor.' },
  collect: { label: 'Collect one execution row', effect: 'Move one admitted order’s exact portfolio and quote contribution into settlement.', authority: 'Candidate page, manifest ordinal, immutable order terms, Claims, and Custody.' },
  materialize: { label: 'Materialize complete sets once', effect: 'Perform the one exact mint, merge, or no-op required by the accumulated inventory.', authority: 'The complete collecting cursor and Product-defined claim domain.' },
  distribute: { label: 'Distribute one execution row', effect: 'Return the selected execution’s exact claim and quote outputs in manifest order.', authority: 'Materialized inventory, candidate page, Claims, and Custody receipts.' },
  close: { label: 'Close settlement', effect: 'Route the exact quote remainder, persist the terminal successor, and close the live cursor.', authority: 'The ready-to-close cursor and the next canonical terminal coordinate.' },
  'open-batch': { label: 'Open an order collection window', effect: 'Create one content-addressed Batch with immutable Product, generation, price-scale, and window terms.', authority: 'The General root revision, config bounds, current slot, and a funded vacant Batch PDA.' },
  'place-order': { label: 'Place and escrow one signed order', effect: 'Admit immutable portfolio terms and move their exact worst-case quote and claim reserves into order escrow.', authority: 'Maker signature, collecting Batch, signed terms, balances, and a funded vacant Order PDA.' },
  'cancel-order': { label: 'Cancel one live order', effect: 'Return that maker’s own full reserved collateral and mark the Order cancelled.', authority: 'Maker signature plus the exact collecting Batch and placed Order.' },
  'close-batch': { label: 'Finalize the order set', effect: 'Stop admissions and make the content-addressed Batch eligible for candidate settlement.', authority: 'The collecting Batch, root revision, and the permissionless-close window/fullness rule.' },
  'submit-candidate': { label: 'Submit one candidate', effect: 'Create the content-addressed candidate and its work escrow from immutable submission terms.', authority: 'Authenticated Batch, pages, solver terms, current slot, and funded vacant candidate state.' },
  'verify-candidate-row': { label: 'Verify one candidate row', effect: 'Advance Candidate and verifier state together; create the verified result only on the final valid row.', authority: 'Candidate, execution page, immutable Order, verifier cursor, and conditional result PDA.' },
  'release-order': { label: 'Release residual order escrow', effect: 'Permissionlessly return every observed residual atom to the Order owner after its settlement window.', authority: 'The placed Order’s immutable expiry and its observed Custody/Claims balances.' },
  'close-candidate': { label: 'Close candidate work state', effect: 'Pay the one cleanup crank, return unused verification funding and historical rent to the solver, then vacate the Candidate.', authority: 'The exact Candidate, its independently authenticated closed Batch, and either prior consideration or the Batch settlement deadline.' },
});

function download(bytes: Uint8Array, name: string): void {
  const blob = new Blob([new Uint8Array(bytes)], { type: 'application/octet-stream' });
  const href = URL.createObjectURL(blob); const anchor = document.createElement('a'); anchor.href = href; anchor.download = name; anchor.click(); URL.revokeObjectURL(href);
}

function SelectionStatus({ value }: Readonly<{ value: GeneralSelectionStatusV2 }>) {
  return <dl className="registered-facts"><div><dt>Selection phase / revision</dt><dd>{value.phase} / {value.revision.toString()}</dd></div><div><dt>Submitted candidates</dt><dd>{value.submittedCount}</dd></div><div><dt>Best valid submitted candidate</dt><dd>{compact(value.bestCandidateId)} · coordinate {value.bestCandidateCoordinate}</dd></div><div><dt>Comparison key</dt><dd>{value.bestFilledLots.toString()} filled lots · {value.bestQuoteSurplus.toString()} quote surplus</dd></div></dl>;
}

function SettlementStatus({ value }: Readonly<{ value: GeneralSettlementStatusV2 }>) {
  return <dl className="registered-facts"><div><dt>Settlement phase / revision</dt><dd>{value.phase} / {value.revision.toString()}</dd></div><div><dt>Progress</dt><dd>{value.nextOrder}/{value.orderCount} orders · N={value.outcomeCount}</dd></div><div><dt>Candidate</dt><dd>{compact(value.candidateId)}</dd></div><div><dt>Inventory</dt><dd>quote {value.quoteInventory.toString()} · claims [{quantities(value.inventory)}]</dd></div><div><dt>Complete sets / terminal</dt><dd>{value.completeSetQuantity.toString()} / {value.terminalCoordinate.toString()}</dd></div></dl>;
}

function BatchStatus({ value }: Readonly<{ value: GeneralBatchStatusV1 }>) {
  return <dl className="registered-facts"><div><dt>Batch phase / sequence</dt><dd>{value.phase} / {value.sequence.toString()}</dd></div><div><dt>Orders</dt><dd>{value.orderCount}/{value.maxOrders} admitted · {value.cancelledCount} cancelled</dd></div><div><dt>Window</dt><dd>collect before slot {value.collectionCloseSlot.toString()} · settle by {value.settlementCloseSlot.toString()}</dd></div><div><dt>Committed quote reserve</dt><dd>{value.committedQuoteReserve.toString()} atoms</dd></div><div><dt>Root revisions</dt><dd>opened {value.openedRootRevision.toString()} · closed {value.closedRootRevision.toString()}</dd></div></dl>;
}

function OrderStatus({ value }: Readonly<{ value: GeneralOrderStatusV1 }>) {
  return <dl className="registered-facts"><div><dt>Order phase / nonce</dt><dd>{value.phase} / {value.nonce.toString()}</dd></div><div><dt>Owner / Batch</dt><dd>{compact(value.owner)} / {compact(value.batchId)}</dd></div><div><dt>Fill bound</dt><dd>{value.maxLots.toString()} lots · at most {value.maxQuoteDebitPerLot.toString()} quote atoms per lot</dd></div><div><dt>Window</dt><dd>admitted slot {value.admittedSlot.toString()} · valid through {value.validUntilSlot.toString()} · released {value.releasedSlot.toString()}</dd></div><div><dt>Portfolio per lot</dt><dd>receive [{quantities(value.receivePerLot)}] · deliver [{quantities(value.deliverPerLot)}]</dd></div></dl>;
}

function CandidateStatus({ value }: Readonly<{ value: GeneralCandidateStatusV1 }>) {
  return <dl className="registered-facts"><div><dt>Candidate phase</dt><dd>{value.phase}</dd></div><div><dt>Verification work</dt><dd>{value.rowCount} rows across {value.pageCount} pages · page revision {value.pageRevision.toString()}</dd></div><div><dt>Work funding</dt><dd>{value.verificationRemaining.toString()} verification · {value.cleanupRemaining.toString()} cleanup lamports remain</dd></div><div><dt>Solver / Batch</dt><dd>{compact(value.solver)} / {compact(value.batchId)}</dd></div><div><dt>Verified result</dt><dd>{value.verifiedDigest === null ? 'Not complete — no digest or verified revision yet.' : `${compact(value.verifiedDigest)} · revision ${value.verifiedRevision.toString()}`}</dd></div></dl>;
}

function VerifierStatus({ value }: Readonly<{ value: GeneralVerifierStatusV2 }>) {
  return <dl className="registered-facts"><div><dt>Verifier phase / revision</dt><dd>{value.phase} / {value.revision.toString()}</dd></div><div><dt>Next exact row</dt><dd>page {value.nextPageIndex}/{value.pageCount} · row {value.nextRowIndex}</dd></div><div><dt>Candidate</dt><dd>{compact(value.candidateId)} · coordinate {value.candidateCoordinate}</dd></div><div><dt>Accepted execution</dt><dd>{value.orderCount} orders · {value.filledLots.toString()} filled lots · debit {value.quoteDebit.toString()} · credit {value.quoteCredit.toString()}</dd></div><div><dt>Simplex / claim movement</dt><dd>scale {value.priceScale.toString()} · prices [{quantities(value.prices)}] · in [{quantities(value.claimInputs)}] · out [{quantities(value.claimOutputs)}]</dd></div></dl>;
}

function VerifiedCandidateStatus({ value }: Readonly<{ value: GeneralVerifiedCandidateStatusV2 }>) {
  return <dl className="registered-facts"><div><dt>Terminal candidate / revision</dt><dd>{compact(value.candidateId)} / {value.revision.toString()}</dd></div><div><dt>Verified work</dt><dd>{value.pageCount} pages · {value.filledLots.toString()} filled lots</dd></div><div><dt>Quote result</dt><dd>debit {value.quoteDebit.toString()} · credit {value.quoteCredit.toString()}</dd></div><div><dt>Claim result</dt><dd>in [{quantities(value.claimInputs)}] · out [{quantities(value.claimOutputs)}]</dd></div></dl>;
}

function LocalStatus({ title, value }: Readonly<{ title: string; value: GeneralLocalStateStatusV3 | Readonly<{ status: 'vacant'; lamports: bigint }> | null }>) {
  if (value === null) return null;
  if (value.status === 'vacant') return <article className="registered-state-card"><span className="eyebrow">{title}</span><h3>Funded vacant System account</h3><p>{value.lamports.toString()} lamports · no state bytes</p></article>;
  return <article className="registered-state-card"><span className="eyebrow">{title} · lifecycle V3</span><h3>{value.status.kind}</h3><p>bump {value.bump} · rent principal {value.rentPrincipal.toString()} · beneficiary {compact(value.beneficiary)}</p>{value.status.kind === 'selection' ? <SelectionStatus value={value.status} /> : value.status.kind === 'settlement' ? <SettlementStatus value={value.status} /> : value.status.kind === 'batch' ? <BatchStatus value={value.status} /> : value.status.kind === 'order' ? <OrderStatus value={value.status} /> : value.status.kind === 'candidate' ? <CandidateStatus value={value.status} /> : <VerifierStatus value={value.status} />}</article>;
}

function ResultStatus({ value }: Readonly<{ value: GeneralChainStatusV5['conditionalResult'] }>) {
  if (value === null) return null;
  // `&& value.status === 'vacant'` looked like a check and was the opposite of
  // one. `status` exists on exactly one arm of this union and its only value
  // there is `'vacant'`, so the conjunct could never fail -- but it turned a
  // narrowing into a compound whose FALSE branch is "not vacant OR carries no
  // status", which is the whole union again. The verified-candidate render
  // below then received a value that might be the vacant arm. The `in` alone
  // is the discriminant, and dropping the tail is what narrows.
  if ('status' in value) return <article className="registered-state-card"><span className="eyebrow">conditional result</span><h3>Vacant until the final valid row</h3><p>{value.lamports.toString()} lamports are present, but no result bytes exist. A nonterminal Verify cannot create them.</p></article>;
  return <article className="registered-state-card"><span className="eyebrow">conditional result · raw V2</span><h3>Verified candidate</h3><p>This Trading-owned result exists only after the verifier completes every authenticated row.</p><VerifiedCandidateStatus value={value} /></article>;
}

function CandidateCloseStatus({ value }: Readonly<{ value: GeneralChainStatusV5['candidateClose'] }>) {
  if (value === null) return null;
  return <article className="registered-state-card"><span className="eyebrow">logical account 8 · independently authenticated evidence</span><h3>Closed Batch permits candidate cleanup</h3><p>{compact(value.closedBatchAccount)} · cranker {compact(value.cranker)} · solver {compact(value.solver)}</p><BatchStatus value={value.closedBatch} /></article>;
}

function LifecycleState({ role, value }: Readonly<{ role: string; value: GeneralLifecycleStateV5 | null }>) {
  if (value === null) return null;
  return <article className="registered-state-card"><span className="eyebrow">logical account {value.accountCoordinate} · {role}</span><h3>{value.isMaterialized ? 'Materialized in the plan snapshot' : 'Canonically addressed, currently vacant'}</h3><p>{compact(value.account)} · bump {value.bump}</p></article>;
}

function Receipt({ value }: Readonly<{ value: GeneralHotReceiptV3 }>) {
  return <div className="direct-output"><dl><div><dt>Committed request</dt><dd>{compact(value.requestDigest)}</dd></div><div><dt>Selected CapabilityProgram</dt><dd>{compact(value.selectedProgram)}</dd></div><div><dt>Root transition</dt><dd>{compact(value.rootPrestateDigest)} → {compact(value.rootPoststateDigest)}</dd></div><div><dt>Execution / child-receipt commitment</dt><dd>{value.executionDigest}</dd></div></dl><p className="direct-refusal"><strong>Receipt accepted.</strong> It matches the imported request, Market, generation, release set, root, and selected program.</p></div>;
}

export default function GeneralWorkspace() {
  const [endpoint, setEndpoint] = useDeploymentFieldV1((d) => d.endpoint);
  const [planText, setPlanText] = useState(''); const [inspection, setInspection] = useState<GeneralPlanInspectionV5 | null>(null); const [planStatus, setPlanStatus] = useState('No operator plan has been inspected.');
  const [chain, setChain] = useState<GeneralChainStatusV5 | null>(null); const [chainStatus, setChainStatus] = useState('No RPC request has been made.');
  const [receiptText, setReceiptText] = useState(''); const [receipt, setReceipt] = useState<GeneralHotReceiptV3 | null>(null); const [receiptStatus, setReceiptStatus] = useState('No commit-last receipt has been supplied.');

  async function inspect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setInspection(null); setChain(null); setReceipt(null); setPlanStatus('Checking the plan…');
    try {
      const value = await inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(planText)); setInspection(value);
      setPlanStatus(`Accepted ${value.request.action} plan · N=${value.plan.outcomeCount} · ${value.transaction.wireBytes}/1232 packet bytes · no signature present.`);
    } catch (error) { setPlanStatus(`Refused: ${message(error)}`); }
  }

  async function reacquire() {
    setChain(null); setChainStatus('Reading finalized state…');
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

  return <PageShell className="product-shell direct-workspace" header={<ConsoleHeader path="/general" title="General market operator" purpose="Understand and re-check one exact General action before any wallet sees it." />}>
    <section className="market-heading"><div><h1>General market operator.</h1><p>Inspect one exact action across order collection, candidate verification, settlement, and cleanup. The page explains what the packet can change, reacquires its authority from finalized state, and keeps the unsigned bytes available for an independent wallet handoff. Nothing is signed or submitted.</p></div></section>

    <form className="direct-card" onSubmit={inspect} aria-labelledby="general-plan">
      <div className="direct-card-heading"><span>01</span><div><h2 id="general-plan">Inspect one chain-derived operator plan</h2><p>First run <code>dclutch general plan --route /absolute/route.json --output /absolute/plan.json</code>. This page accepts only exact V5 fields, one measured heap declaration followed by one packet-bounded Hot instruction, a {GENERAL_HOT_FIXED_ACCOUNT_COUNT_V3}-coordinate Hot frame, CapabilityProgramV4/LifecycleV5 provenance, canonical lifecycle bumps, and action-specific DCE5 child-receipt order.</p></div></div>
      <label><span>General successor plan · JSON — exact operator handoff, not a form for inventing chain facts</span><textarea required rows={16} value={planText} onChange={(event) => setPlanText(event.target.value)} placeholder={generalPlanTemplateV5()} /></label>
      <button>Inspect exact unsigned plan</button><p className="direct-status" aria-live="polite">{planStatus}</p>
      {inspection && <div className="direct-output"><div className="registered-state-grid"><article className="registered-state-card"><span className="eyebrow">what this action does</span><h3>{ACTION_READING[inspection.request.action].label}</h3><p>{ACTION_READING[inspection.request.action].effect}</p></article><article className="registered-state-card"><span className="eyebrow">why it is authorized</span><h3>Reacquired, not trusted</h3><p>{ACTION_READING[inspection.request.action].authority}</p></article></div><dl><div><dt>Action / wire / revision</dt><dd>{inspection.request.action} · request {inspection.request.wire.toUpperCase()} · revision {inspection.request.expectedRevision.toString()}</dd></div><div><dt>Action subject</dt><dd>{inspection.request.subjectId === null ? 'None — Freeze must use the best valid submitted candidate already in Selection state.' : compact(inspection.request.subjectId)}</dd></div><div><dt>Manifest / source coordinates</dt><dd>manifest {inspection.request.manifestOrderIndex} · page {inspection.request.pageIndex} · execution {inspection.request.executionIndex}</dd></div><div><dt>Runtime width / invocations / heap</dt><dd>N={inspection.plan.outcomeCount} · {inspection.plan.admittedInvocationCount} accelerator invocation(s) · {inspection.plan.heapFrameBytes.toLocaleString()} bytes declared</dd></div><div><dt>Packet / ALT / signers</dt><dd>{inspection.transaction.wireBytes}/1232 bytes · {compact(inspection.plan.lookupTable)} · {inspection.plan.requiredSigners.map(compact).join(', ')}</dd></div><div><dt>Checked releases</dt><dd>Trading {compact(inspection.plan.tradingArtifactRelease)} · accelerator {compact(inspection.plan.generalArtifactRelease)} · manifest {compact(inspection.plan.checkedManifestDigest)}</dd></div><div><dt>Lifecycle frame</dt><dd>children begin at logical account {inspection.plan.lifecycle.childAccountStart}{inspection.plan.lifecycle.terminalCoordinate === null ? '' : ` · close successor coordinate ${inspection.plan.lifecycle.terminalCoordinate}`}</dd></div></dl>
        <div className="registered-state-grid"><LifecycleState role="primary state" value={inspection.plan.lifecycle.primary} /><LifecycleState role="secondary state" value={inspection.plan.lifecycle.secondary} /><LifecycleState role="conditional result" value={inspection.plan.lifecycle.conditionalResult} /></div>
        <div className="registered-state-grid">{inspection.plan.childRoutes.length === 0 ? <article className="registered-state-card"><span className="eyebrow">semantic-owner child routes</span><h3>None for this action</h3><p>The action still runs through authenticated Transition and state-last Effect artifacts; “no child route” does not mean “no state change.”</p></article> : inspection.plan.childRoutes.map((route) => <article className="registered-state-card" key={route.route}><span className="eyebrow">route {route.route} · {route.role}</span><h3>logical accounts {route.accountStart}…{route.accountStart + route.accountCount - 1}</h3><p>{route.receiptDependencies.length === 0 ? 'No prior receipt dependency.' : route.receiptDependencies.map((entry) => `${entry.producerRole} route ${entry.producerRoute} · ${entry.expectedReceiptBytes} bytes`).join(' · ')}</p></article>)}</div>
        <button type="button" onClick={() => download(transactionBytesV5(inspection), `dclutch-general-${inspection.request.action}-unsigned-v5.bin`)}>Download unsigned v0 packet</button><p className="direct-refusal"><strong>No signing or submission occurred.</strong> The checked blockhash will expire. Produce a fresh chain-derived handoff with <code>dclutch general plan --route /absolute/route.json --output /absolute/plan.json</code>, then inspect that exact output here.</p></div>}
    </form>

    <section className="direct-card" aria-labelledby="general-chain">
      <div className="direct-card-heading"><span>02</span><div><h2 id="general-chain">Reacquire exact chain status</h2><p>The packet’s sole lookup table, every dependency, program executability, and the action’s Trading-owned local state, each read at one finalized floor. No substitute is accepted for a named account.</p></div></div>
      <div className="direct-form-grid"><label><span>Finalized RPC endpoint</span><input type="url" value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label></div><button type="button" disabled={inspection === null} onClick={() => void reacquire()}>Reacquire plan dependencies</button><p className="direct-status" aria-live="polite">{chainStatus}</p>
      {chain && <><div className="trade-v3-evidence"><article><span>Dependencies</span><strong>{chain.dependencies.dependencies.length}</strong><small>missing 0 · non-executable programs 0</small></article><article><span>Observation slot</span><strong>{chain.observedSlot}</strong><small>minimum imported floor {inspection?.plan.observedSlot.toString()}</small></article><article><span>Action prestate</span><strong>{inspection?.request.action}</strong><small>revision {inspection?.request.expectedRevision.toString()}</small></article></div><div className="registered-state-grid"><LocalStatus title="primary state" value={chain.primary} /><LocalStatus title="secondary state" value={chain.secondary} /><ResultStatus value={chain.conditionalResult} /><CandidateCloseStatus value={chain.candidateClose} /></div></>}
    </section>

    <form className="direct-card" onSubmit={inspectReceipt} aria-labelledby="general-receipt">
      <div className="direct-card-heading"><span>03</span><div><h2 id="general-receipt">Verify the commit-last execution receipt</h2><p>Paste the 280-byte HotExecutionAckV3 the chain returns. It is accepted only if it joins the request digest, selected CapabilityProgram, Market generation, root prestate, and release set.</p></div></div>
      <label><span>Execution receipt · base64 — the 280-byte HotExecutionAckV3 the chain returns when the packet executes</span><textarea required value={receiptText} onChange={(event) => setReceiptText(event.target.value.trim())} /></label><button disabled={inspection === null}>Verify exact receipt</button><p className="direct-status" aria-live="polite">{receiptStatus}</p>{receipt && <Receipt value={receipt} />}
    </form>
  </PageShell>;
}
