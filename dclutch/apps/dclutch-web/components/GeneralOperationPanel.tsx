'use client';

import { useEffect, useRef, useState } from 'react';
import { type VersionedTransaction } from '@solana/web3.js';
import { type GeneralPlanInspectionV5 } from '@dclutch/sdk/generalPlanV5';
import { assertGeneralDeploymentV5, observeGeneralExecutionV5, previewGeneralExecutionV5, type GeneralExecutionPreviewV5 } from '@dclutch/sdk/generalExecutionV5';
import { useDeploymentV1 } from '@/lib/deploymentStore';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';
import WalletDirectory, { useWalletDirectoryV1 } from './WalletDirectory';
import { archiveGeneralOperationV5, retainGeneralOperationV5, restoreGeneralOperationV5, signGeneralOperationV5, submitGeneralOperationV5 } from '@/lib/generalOperationV5';
import { archiveFinalizedRefusedClientOperationV1 } from '@/lib/clientOperationRefusalV1';
import { discardUnsignedClientOperationJournalV1, findClientOperationJournalV1, submittedClientOperationWireV1, type ClientOperationJournalV1 } from '@/lib/clientOperationJournal';

type Props = Readonly<{ endpoint: string; nativePlan: string; inspection: GeneralPlanInspectionV5 | null }>;
function errorText(error: unknown): string { return error instanceof Error ? error.message : 'General operation refused'; }

export default function GeneralOperationPanel({ endpoint, nativePlan, inspection }: Props) {
  const directory = useWalletDirectoryV1();
  const deployment = useDeploymentV1();
  const selectionEpoch = useRef(0);
  useEffect(() => { selectionEpoch.current += 1; }, [endpoint, nativePlan, deployment.programs.trading]);
  const [journal, setJournal] = useState<ClientOperationJournalV1 | null>(null);
  const [preview, setPreview] = useState<GeneralExecutionPreviewV5 | null>(null);
  const [partial, setPartial] = useState<VersionedTransaction | null>(null);
  const [market, setMarket] = useState('');
  const [payer, setPayer] = useState('');
  const [status, setStatus] = useState('Review a plan to begin.');
  const [busy, setBusy] = useState(false);
  const [finalized, setFinalized] = useState<Awaited<ReturnType<typeof observeGeneralExecutionV5>>>(null);
  async function run(action: (requireCurrent: () => void) => Promise<void>) { const epoch = selectionEpoch.current; const requireCurrent = () => { if (selectionEpoch.current !== epoch) throw new Error('General selection changed during acquisition or wallet handoff; recover the retained operation before continuing'); }; setBusy(true); try { await action(requireCurrent); } catch (error) { setStatus(errorText(error)); } finally { setBusy(false); } }
  async function prepare(requireCurrent: () => void) {
    if (inspection === null) throw new Error('Inspect a native General plan first');
    const client = new SolanaRpcClient(endpoint); const admission = await client.assertMutationCluster();
    const value = await previewGeneralExecutionV5(client, inspection, deployment.programs.trading);
    requireCurrent();
    const saved = await retainGeneralOperationV5(localStorage, { clusterGenesis: admission.genesisHash, market: inspection.plan.market, owner: inspection.plan.payer }, nativePlan, value);
    setJournal(saved); setPreview(value); setPartial(null); setFinalized(null);
    setStatus('Plan saved. Review the transfers below and connect a required signer.');
  }
  async function recover() {
    const admission = await new SolanaRpcClient(endpoint).assertMutationCluster();
    const saved = await findClientOperationJournalV1(localStorage, { clusterGenesis: admission.genesisHash, market: market || inspection?.plan.market || '', owner: payer || inspection?.plan.payer || directory.address || '' }, 'general-v5');
    if (saved === null) throw new Error('No retained General operation for that chain, Market, and payer');
    const restored = await restoreGeneralOperationV5(saved);
    assertGeneralDeploymentV5(restored.inspection, deployment.programs.trading);
    setJournal(saved); setPreview(restored.preview); setPartial(null); setFinalized(null);
    setStatus(saved.phase === 'submitted' ? 'Saved transaction restored. Check confirmation before resending.' : 'Saved plan restored. Reconnect each signer to continue.');
  }
  async function sign(requireCurrent: () => void) {
    if (journal === null || directory.address === null) throw new Error('Retain an operation and connect a required signer');
    const signed = await signGeneralOperationV5(localStorage, new SolanaRpcClient(endpoint), journal, directory.handoff(endpoint), directory.address, deployment.programs.trading, partial ?? undefined, requireCurrent);
    requireCurrent();
    setPartial(signed.transaction); setStatus(signed.complete ? 'All signatures collected. Submit is a separate action.' : 'Signature added. Connect the next required signer.');
  }
  async function send(requireCurrent: () => void) {
    if (journal === null) throw new Error('No operation is retained');
    // The driver writes the signed journal BEFORE RPC. Recover it even if the send throws.
    try { setJournal(await submitGeneralOperationV5(localStorage, new SolanaRpcClient(endpoint), journal, deployment.programs.trading, partial ?? undefined, requireCurrent)); setStatus('Transaction sent. Check confirmation below.'); }
    finally { const saved = await findClientOperationJournalV1(localStorage, journal, 'general-v5'); if (saved !== null) setJournal(saved); }
  }
  async function poll() {
    if (journal?.phase !== 'submitted' || journal.signature === null) throw new Error('No signed packet has been retained');
    const client = new SolanaRpcClient(endpoint); const admission = await client.assertMutationCluster();
    if (admission.genesisHash !== journal.clusterGenesis) throw new Error('Selected endpoint is another chain');
    const restored = await restoreGeneralOperationV5(journal);
    assertGeneralDeploymentV5(restored.inspection, deployment.programs.trading);
    const result = await observeGeneralExecutionV5(client, restored.inspection, journal.signature, submittedClientOperationWireV1(journal));
    setFinalized(result); setStatus(result === null ? 'Not finalized yet. Check again shortly.' : 'Transaction finalized. Root verified. Updated account states are shown below.');
  }
  async function archive() {
    if (journal === null) throw new Error('No retained General operation');
    await archiveGeneralOperationV5(localStorage, new SolanaRpcClient(endpoint), journal, deployment.programs.trading);
    setJournal(null); setPartial(null); setPreview(null); setFinalized(null);
    setStatus('Transaction archived. Load a new plan for the next action.');
  }
  async function archiveRefusal() {
    if (journal === null) throw new Error('No retained General operation');
    const refusal = await archiveFinalizedRefusedClientOperationV1(localStorage, new SolanaRpcClient(endpoint), journal);
    setJournal(null); setPartial(null); setPreview(null); setFinalized(null);
    setStatus(`Finalized refusal archived with the exact packet: ${refusal.errorText}. Load a new plan to retry.`);
  }
  async function discard() { if (journal === null) return; await discardUnsignedClientOperationJournalV1(localStorage, journal); setJournal(null); setPartial(null); setPreview(null); setStatus('Saved plan discarded. Load a new plan to continue.'); }
  const complete = partial !== null && partial.signatures.every((signature) => signature.some((byte) => byte !== 0));
  return <section className="direct-card" aria-labelledby="general-execute">
    <h2 id="general-execute">Review and submit</h2>
    <p>Review the transfers, connect each required wallet, and submit. Restore a saved operation to continue after an interruption.</p>
    <WalletDirectory directory={directory} onConnected={() => undefined} purpose="sign this action after you review it" />
    <button type="button" disabled={busy || inspection === null || journal !== null} onClick={() => void run(prepare)}>Review and save plan</button>
    <details><summary>Restore a saved operation</summary><label>Market<input value={market} placeholder={inspection?.plan.market} onChange={(event) => setMarket(event.target.value.trim())} /></label><label>Original payer<input value={payer} placeholder={inspection?.plan.payer ?? directory.address ?? ''} onChange={(event) => setPayer(event.target.value.trim())} /></label><button type="button" disabled={busy} onClick={() => void run(recover)}>Restore saved operation</button></details>
    {journal && <dl><div><dt>Action</dt><dd>{JSON.parse(journal.intent).action}</dd></div><div><dt>Market / payer</dt><dd>{journal.market}<br />{journal.owner}</dd></div><div><dt>Status</dt><dd>{journal.phase} {journal.signature}</dd></div><div><dt>Required signers</dt><dd>{partial ? partial.message.staticAccountKeys.slice(0, partial.message.header.numRequiredSignatures).map((key, index) => `${key.toBase58()} (${partial.signatures[index]?.some((byte) => byte !== 0) ? 'signed' : 'needed'})`).join(', ') : 'Every required signer listed in the native plan; each wallet signs the same message.'}</dd></div></dl>}
    {preview && <><p>Simulation at slot {preview.observedSlot}; {preview.computeUnits ?? 'unreported'} CU. Amounts are integer atoms. Lamports include transaction fees and account rent.</p><div className="table-scroll"><table><thead><tr><th>Account / recipient</th><th>Lamports before → simulated after</th><th>Token atoms before → simulated after</th><th>Claim balances before → simulated after</th></tr></thead><tbody>{preview.accounts.map((entry) => <tr key={entry.address}><td>{entry.address}{entry.afterClaims && <><br />Claims owner: {entry.afterClaims.owner}</>}</td><td>{entry.beforeLamports} → {entry.afterLamports}</td><td>{entry.beforeToken?.amount ?? '—'} → {entry.afterToken?.amount ?? '—'}{(entry.afterToken ?? entry.beforeToken) && <><br />Mint: {(entry.afterToken ?? entry.beforeToken)?.mint}<br />Owner: {(entry.afterToken ?? entry.beforeToken)?.owner}</>}</td><td>{entry.beforeClaims?.balances.join(', ') ?? '—'} → {entry.afterClaims?.balances.join(', ') ?? '—'}</td></tr>)}</tbody></table></div></>}
    <div><button type="button" disabled={busy || journal?.phase !== 'unsigned' || directory.address === null || complete} onClick={() => void run(sign)}>Sign with this wallet</button><button type="button" disabled={busy || journal === null || (journal.phase === 'unsigned' && !complete) || finalized !== null} onClick={() => void run(send)}>{journal?.phase === 'submitted' ? 'Resend saved transaction' : 'Submit transaction'}</button><button type="button" disabled={busy || journal?.phase !== 'submitted'} onClick={() => void run(poll)}>Check confirmation</button><button type="button" disabled={busy || journal?.phase !== 'unsigned'} onClick={() => void run(discard)}>Discard saved plan</button></div>
    <button type="button" disabled={busy || finalized === null} onClick={() => void run(archive)}>Archive and continue</button>
    <button type="button" disabled={busy || journal?.phase !== 'submitted' || finalized !== null} onClick={() => void run(archiveRefusal)}>Archive rejected transaction</button>
    <p role="status">{status}</p>
    {finalized && <details open><summary>Transaction and account states</summary><pre>{JSON.stringify({ signature: finalized.transaction.signature, finalizedSlot: finalized.transaction.slot, observedSlot: finalized.observedSlot, rootPoststateDigest: finalized.receipt.rootPoststateDigest, lifecycle: finalized.lifecycle }, (_key, value: unknown) => typeof value === 'bigint' ? value.toString() : value, 2)}</pre></details>}
  </section>;
}
