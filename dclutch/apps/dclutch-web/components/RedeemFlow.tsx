'use client';

import { useCallback, useEffect, useMemo, useState, type ChangeEvent } from 'react';

import { type WalletDirectoryHandleV1 } from '@/components/WalletDirectory';
import { inspectClaimsCustodyReplayV1, type ClaimsCustodyReplayRequestV1, type ClaimsCustodyReplayStateV1 } from '@dclutch/sdk/claimsCustodyReplay';
import {
  clearFinalizedClientOperationJournalV1,
  discardUnsignedClientOperationJournalV1,
  findClientOperationJournalV1,
  markClientOperationSubmittedV1,
  requireSubmittedSignatureMatchV1,
  submittedClientOperationWireV1,
  transactionSignatureV1,
  writeUnsignedClientOperationJournalV1,
  type ClientOperationJournalV1,
  type ClientOperationScopeV1,
} from '@/lib/clientOperationJournal';
import {
  authenticateClaimsReplayJournalV1,
  authenticateUnsignedTerminalPayoutJournalV1,
  claimsReplayJournalInputV1,
  claimsReplayFinalizedCompletionV1,
  requireTerminalPayoutRouteScopeV1,
  restoreTerminalPayoutJournalV1,
  terminalPayoutJournalInputV1,
} from '@/lib/redeemOperationJournal';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';
import {
  finalizeWalletTerminalPayoutV3,
  importRustWalletTerminalPayoutArtifactV3,
  prepareCheckedLiveDevnetWalletTerminalPayoutV3,
  walletTerminalPayoutSummaryV3,
  type PreparedWalletTerminalPayoutV3,
} from '@dclutch/sdk/walletTerminalPayoutV3';
import { requestWalletTransactionSignatureV1, submitSignedTransactionV1 } from '@dclutch/sdk/walletHandoff';
import { WALLET_TERMINAL_PAYOUT_INPUT_FORMAT_V1 } from '@/lib/generated/walletTerminalPayoutWasmV1';
import { deriveWalletTerminalPayoutManifestV1 } from '@/lib/walletTerminalPayoutSnapshot';
import { loadWalletTerminalPayoutWasmV1 } from '@/lib/walletTerminalPayoutV1';
import { deriveWalletTerminalPayoutInputV1 } from '@/lib/walletTerminalInputSnapshot';
import { loadWalletTerminalInputWasmV1 } from '@/lib/walletTerminalInputV1';

type ReplayFlow =
  | Readonly<{ kind: 'idle' | 'inspecting' }>
  | Readonly<{ kind: 'ready'; state: ClaimsCustodyReplayStateV1; journal: ClientOperationJournalV1 | null }>
  | Readonly<{ kind: 'signing'; state: ClaimsCustodyReplayStateV1; journal: ClientOperationJournalV1 }>
  | Readonly<{ kind: 'submitted'; journal: ClientOperationJournalV1; signature: string; confirmation: string }>
  | Readonly<{ kind: 'confirmed'; signature: string | null; replayAddress: string; nextRevision: string }>
  | Readonly<{ kind: 'refused'; reason: string; journal: ClientOperationJournalV1 | null }>;

type PayoutFlow =
  | Readonly<{ kind: 'idle' | 'preparing' }>
  | Readonly<{ kind: 'ready' | 'signing'; plan: PreparedWalletTerminalPayoutV3; journal: ClientOperationJournalV1 }>
  | Readonly<{ kind: 'submitted'; plan: PreparedWalletTerminalPayoutV3; journal: ClientOperationJournalV1; signature: string; confirmation: string }>
  | Readonly<{ kind: 'confirmed'; signature: string; observedSlot: string; payout: string }>
  | Readonly<{ kind: 'refused'; reason: string; journal: ClientOperationJournalV1 | null }>;

const errorMessage = (error: unknown) => error instanceof Error ? error.message : 'the redemption step refused without a usable reason';
const retryableFinality = (error: unknown) => {
  const message = errorMessage(error);
  return message.includes('not available at finalized commitment yet') || message.includes('finalized account floor has not reached');
};
const pause = () => new Promise<void>((resolve) => setTimeout(resolve, 1_000));

function browserStorage(): Storage {
  if (typeof window === 'undefined' || window.localStorage === undefined) throw new Error('this browser does not expose local recovery storage, so no wallet signature was requested');
  return window.localStorage;
}

/**
 * Is this the payout INPUT rather than a completed manifest?
 *
 * The format name is emitted from the operator crate, so the browser
 * recognises the artifact without writing its name down. A reader who pastes
 * the earlier artifact gets it completed here rather than being sent away to
 * run a second command.
 */
function isPayoutInputV1(text: string): boolean {
  try {
    const parsed: unknown = JSON.parse(text);
    return parsed !== null && typeof parsed === 'object'
      && (parsed as Record<string, unknown>).format === WALLET_TERMINAL_PAYOUT_INPUT_FORMAT_V1;
  } catch { return false; }
}

export default function RedeemFlow({ endpoint, marketAddress, positionAddress, claimIndex, availableQuantity, claimsProgramId, custodyProgramId, registryProgramId, coreProgramId, resolutionProgramId, directory }: Readonly<{
  endpoint: string; marketAddress: string; positionAddress: string; claimIndex: number; availableQuantity: string;
  claimsProgramId: string; custodyProgramId: string; registryProgramId: string; coreProgramId: string; resolutionProgramId: string; directory: WalletDirectoryHandleV1;
}>) {
  const client = useMemo(() => new SolanaRpcClient(endpoint), [endpoint]);
  const [replay, setReplay] = useState<ReplayFlow>({ kind: 'idle' });
  const [manifestText, setManifestText] = useState('');
  const [manifestSource, setManifestSource] = useState('');
  // The one coordinate the protocol has never derived: which token account the
  // proceeds are paid into. The CLI takes it as `--recipient`, and every other
  // browser surface that moves collateral asks for it the same way -- JoinPanel
  // and the Direct workspace both do. It is a wallet coordinate, not a
  // document.
  const [recipient, setRecipient] = useState('');
  const [payout, setPayout] = useState<PayoutFlow>({ kind: 'idle' });
  const [recovery, setRecovery] = useState('');
  const wallet = directory.address;
  const replayExists = (replay.kind === 'ready' && replay.state.status === 'exists') || replay.kind === 'confirmed';

  const replayRequest = useCallback((owner: string): ClaimsCustodyReplayRequestV1 => Object.freeze({
    marketAddress,
    claimsProgramId,
    custodyProgramId,
    registryProgramId,
    payer: owner,
  }), [marketAddress, claimsProgramId, custodyProgramId, registryProgramId]);
  const operationScope = useCallback(async (owner: string): Promise<ClientOperationScopeV1> => {
    const facts = await client.probe();
    return Object.freeze({ clusterGenesis: facts.genesisHash, market: marketAddress, owner });
  }, [client, marketAddress]);

  const pollReplayJournal = useCallback(async (journal: ClientOperationJournalV1, owner: string, alive: () => boolean = () => true): Promise<void> => {
    const signature = journal.signature;
    if (journal.phase !== 'submitted' || signature === null) throw new Error('replay recovery requires one submitted signature');
    for (let attempt = 0; attempt < 30 && alive(); attempt += 1) {
      try {
        const [status, state] = await Promise.all([
          client.signatureStatuses([signature]).then((statuses) => statuses[0]),
          inspectClaimsCustodyReplayV1(client, replayRequest(owner)),
        ]);
        if (!alive()) return;
        if (status?.known && status.succeeded === false) {
          setReplay({ kind: 'submitted', journal, signature, confirmation: `The chain reports an error (${status.errorText ?? 'unnamed chain error'}). The transaction is saved. Check its signature before taking further action.` }); return;
        }
        if (claimsReplayFinalizedCompletionV1(status, state)) {
          await clearFinalizedClientOperationJournalV1(browserStorage(), journal);
          if (alive()) setReplay({ kind: 'confirmed', signature, replayAddress: state.replayAddress, nextRevision: state.nextRevision });
          return;
        }
        setReplay({ kind: 'submitted', journal, signature, confirmation: 'Payment-record confirmation is pending. Reload later to check this transaction.' });
      } catch (error) {
        if (alive()) setReplay({ kind: 'submitted', journal, signature, confirmation: `${errorMessage(error)} The transaction is saved. Reload to check its status.` });
        return;
      }
      await pause();
    }
    if (alive()) setReplay({ kind: 'submitted', journal, signature, confirmation: 'Confirmation is pending. Reload later to check this transaction.' });
  }, [client, replayRequest]);

  const pollPayoutJournal = useCallback(async (journal: ClientOperationJournalV1, plan: PreparedWalletTerminalPayoutV3, alive: () => boolean = () => true): Promise<void> => {
    const signature = journal.signature;
    if (journal.phase !== 'submitted' || signature === null) throw new Error('payout recovery requires one submitted signature');
    for (let attempt = 0; attempt < 45 && alive(); attempt += 1) {
      try {
        const finalized = await finalizeWalletTerminalPayoutV3(
          client,
          signature,
          plan,
          submittedClientOperationWireV1(journal),
        );
        await clearFinalizedClientOperationJournalV1(browserStorage(), journal);
        if (alive()) setPayout({ kind: 'confirmed', signature, observedSlot: finalized.observedSlot, payout: finalized.payout });
        return;
      } catch (error) {
        if (!retryableFinality(error)) {
          if (alive()) setPayout({ kind: 'submitted', plan, journal, signature, confirmation: `${errorMessage(error)} The payout is saved. Reload to check its status.` });
          return;
        }
      }
      if (alive()) setPayout({ kind: 'submitted', plan, journal, signature, confirmation: 'Payout confirmation is pending. Reload later to check this transaction.' });
      await pause();
    }
    if (alive()) setPayout({ kind: 'submitted', plan, journal, signature, confirmation: 'Confirmation is pending. Reload later to check this transaction.' });
  }, [client]);

  useEffect(() => {
    let current = true;
    if (wallet === null || custodyProgramId === '' || registryProgramId === '') return () => { current = false; };
    void (async () => {
      setRecovery('Checking for a saved redemption…');
      let scope: ClientOperationScopeV1;
      try { scope = await operationScope(wallet); } catch (error) { if (current) setRecovery(`Recovery refused: ${errorMessage(error)}`); return; }
      let replayJournal: ClientOperationJournalV1 | null; let payoutJournal: ClientOperationJournalV1 | null;
      try {
        [replayJournal, payoutJournal] = await Promise.all([
          findClientOperationJournalV1(browserStorage(), scope, 'claims-replay-create-v1'),
          findClientOperationJournalV1(browserStorage(), scope, 'wallet-terminal-payout-v3'),
        ]);
      } catch (error) { if (current) setRecovery(`Recovery refused: ${errorMessage(error)}`); return; }
      if (!current) return;
      setRecovery(replayJournal === null && payoutJournal === null ? 'No redemption is waiting to resume.' : 'Checking your saved redemption…');

      if (replayJournal !== null) {
        if (replayJournal.phase === 'submitted') {
          setReplay({ kind: 'submitted', journal: replayJournal, signature: replayJournal.signature!, confirmation: 'Checking your saved setup transaction…' });
          void pollReplayJournal(replayJournal, wallet, () => current);
        } else {
          const state = await inspectClaimsCustodyReplayV1(client, replayRequest(wallet));
          if (!current) return;
          if (state.status === 'exists') {
            await clearFinalizedClientOperationJournalV1(browserStorage(), replayJournal);
            if (current) setReplay({ kind: 'confirmed', signature: null, replayAddress: state.replayAddress, nextRevision: state.nextRevision });
          } else if (state.status === 'creatable') {
            try { authenticateClaimsReplayJournalV1(replayJournal, replayRequest(wallet), state.plan); setReplay({ kind: 'ready', state, journal: replayJournal }); }
            catch (error) { setReplay({ kind: 'refused', reason: errorMessage(error), journal: replayJournal }); }
          } else setReplay({ kind: 'refused', reason: state.reason, journal: replayJournal });
        }
      }

      if (payoutJournal !== null) {
        try {
          const restored = await restoreTerminalPayoutJournalV1(payoutJournal);
          requireTerminalPayoutRouteScopeV1(payoutJournal, restored.manifest, { market: marketAddress, position: positionAddress, owner: wallet, claimIndex });
          if (!current) return;
          setManifestText(payoutJournal.intent);
          if (payoutJournal.phase === 'submitted') {
            setPayout({ kind: 'submitted', plan: restored.plan, journal: payoutJournal, signature: payoutJournal.signature!, confirmation: 'Checking your saved payout transaction…' });
            void pollPayoutJournal(payoutJournal, restored.plan, () => current);
          } else {
            const fresh = await prepareCheckedLiveDevnetWalletTerminalPayoutV3(client, restored.manifest, wallet);
            await authenticateUnsignedTerminalPayoutJournalV1(payoutJournal, restored.manifest, fresh);
            if (current) setPayout({ kind: 'ready', plan: fresh, journal: payoutJournal });
          }
        } catch (error) { if (current) setPayout({ kind: 'refused', reason: errorMessage(error), journal: payoutJournal }); }
      }
    })();
    return () => { current = false; };
  }, [client, wallet, marketAddress, positionAddress, claimIndex, claimsProgramId, custodyProgramId, registryProgramId, operationScope, pollPayoutJournal, pollReplayJournal, replayRequest]);

  async function inspect() {
    setReplay({ kind: 'inspecting' });
    if (wallet === null) { setReplay({ kind: 'refused', reason: 'connect a browser wallet first: your wallet owns the claim balance and must authorize its payout', journal: null }); return; }
    if (custodyProgramId === '' || registryProgramId === '' || coreProgramId === '' || resolutionProgramId === '') { setReplay({ kind: 'refused', reason: 'this deployment does not name all of the programs the payout needs', journal: null }); return; }
    try {
      const request = replayRequest(wallet); const state = await inspectClaimsCustodyReplayV1(client, request);
      if (state.status !== 'creatable') { setReplay({ kind: 'ready', state, journal: null }); return; }
      const scope = await operationScope(wallet);
      const journal = await writeUnsignedClientOperationJournalV1(browserStorage(), claimsReplayJournalInputV1(scope, request, state.plan));
      setReplay({ kind: 'ready', state, journal });
    } catch (error) { setReplay({ kind: 'refused', reason: errorMessage(error), journal: null }); }
  }

  async function createReplay() {
    if (replay.kind !== 'ready' || replay.state.status !== 'creatable' || replay.journal === null || wallet === null) return;
    const state = replay.state; const plan = state.plan; const unsignedJournal = replay.journal;
    setReplay({ kind: 'signing', state, journal: unsignedJournal });
    let submittedJournal: ClientOperationJournalV1 | null = null;
    try {
      const signed = await requestWalletTransactionSignatureV1(client, directory.handoff(endpoint), plan.transaction, wallet);
      if (!signed.complete) throw new Error('the wallet did not complete the one required signature');
      const transactionId = transactionSignatureV1(signed.transaction.signatures[0]!);
      submittedJournal = await markClientOperationSubmittedV1(
        browserStorage(),
        unsignedJournal,
        transactionId,
        signed.wireBytes,
      );
      setReplay({ kind: 'submitted', journal: submittedJournal, signature: transactionId, confirmation: 'Submitting your signed transaction…' });
      const returned = await submitSignedTransactionV1(client, submittedClientOperationWireV1(submittedJournal));
      requireSubmittedSignatureMatchV1(transactionId, returned);
      await pollReplayJournal(submittedJournal, wallet);
    } catch (error) {
      if (submittedJournal !== null) setReplay({ kind: 'submitted', journal: submittedJournal, signature: submittedJournal.signature!, confirmation: `${errorMessage(error)} The transaction is saved. Reload to check its status.` });
      else setReplay({ kind: 'refused', reason: errorMessage(error), journal: unsignedJournal });
    }
  }

  async function preparePayout() {
    if (!replayExists || wallet === null) return;
    setPayout({ kind: 'preparing' });
    try {
      if (BigInt(availableQuantity) === 0n) throw new Error('this Position holds zero winning atoms, so there is nothing to redeem');
      // NOTHING IS IMPORTED. With an empty box the payout input is DERIVED
      // here, from this deployment's five program ids and this Market: four
      // finalized rounds that name the eleven-row address book the CLI used to
      // project out of a sealed campaign report, and recompile the four
      // composition records that no account on chain points at.
      const inputText = manifestText.trim() === ''
        ? (await deriveWalletTerminalPayoutInputV1(client, await loadWalletTerminalInputWasmV1(), {
            programs: { registry: registryProgramId, core: coreProgramId, claims: claimsProgramId, custody: custodyProgramId, resolution: resolutionProgramId },
            market: marketAddress,
            owner: wallet,
            // Empty means the conventional destination, filled in by the
            // derivation beside the address book. Naming one overrides it.
            recipient: recipient.trim() === '' ? undefined : recipient.trim(),
            claimIndex,
          })).inputJson
        : manifestText;
      // A payout INPUT is completed here by the compiled derivation; an
      // already-complete manifest is taken as it always was. Two artifacts at
      // different stages of ONE authority, not two authorities: whichever
      // arrives, what reaches the checks below is the same
      // `dclutch-wallet-terminal-payout-v3` proved against finalized devnet by
      // the same code.
      const manifest = importRustWalletTerminalPayoutArtifactV3(
        isPayoutInputV1(inputText)
          ? await deriveWalletTerminalPayoutManifestV1(client, await loadWalletTerminalPayoutWasmV1(), inputText)
          : inputText,
      );
      if (manifest.request.market !== marketAddress || manifest.request.position !== positionAddress || manifest.request.owner !== wallet || manifest.request.claimIndex !== claimIndex) throw new Error('the payout plan names another Market, Position, owner, or winning claim');
      if (BigInt(manifest.request.quantity) > BigInt(availableQuantity)) throw new Error('the payout plan tries to redeem more winning atoms than this Position holds');
      const plan = await prepareCheckedLiveDevnetWalletTerminalPayoutV3(client, manifest, wallet); const scope = await operationScope(wallet);
      const journal = await writeUnsignedClientOperationJournalV1(browserStorage(), terminalPayoutJournalInputV1(scope, manifest, plan));
      setPayout({ kind: 'ready', plan, journal });
    } catch (error) { setPayout({ kind: 'refused', reason: errorMessage(error), journal: null }); }
  }

  async function importPayoutFile(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = '';
    if (file === undefined) return;
    setPayout({ kind: 'preparing' });
    try {
      if (file.size === 0 || file.size > 32_768) throw new Error('the payout plan file must contain 1..32768 bytes');
      const text = await file.text();
      const manifest = importRustWalletTerminalPayoutArtifactV3(text);
      setManifestText(JSON.stringify(manifest, null, 2));
      setManifestSource(`${file.name} · lookup table ${manifest.lookupTable}`);
      setPayout({ kind: 'idle' });
    } catch (error) {
      setManifestSource('');
      setPayout({ kind: 'refused', reason: errorMessage(error), journal: null });
    }
  }

  async function signPayout() {
    if (payout.kind !== 'ready' || wallet === null) return;
    const plan = payout.plan; const unsignedJournal = payout.journal;
    setPayout({ kind: 'signing', plan, journal: unsignedJournal });
    let submittedJournal: ClientOperationJournalV1 | null = null;
    try {
      const signed = await requestWalletTransactionSignatureV1(client, directory.handoff(endpoint), plan.transaction, wallet);
      if (!signed.complete) throw new Error('the wallet did not complete the one required signature');
      const transactionId = transactionSignatureV1(signed.transaction.signatures[0]!);
      submittedJournal = await markClientOperationSubmittedV1(
        browserStorage(),
        unsignedJournal,
        transactionId,
        signed.wireBytes,
      );
      setPayout({ kind: 'submitted', plan, journal: submittedJournal, signature: transactionId, confirmation: 'Submitting your signed transaction…' });
      const returned = await submitSignedTransactionV1(client, submittedClientOperationWireV1(submittedJournal));
      requireSubmittedSignatureMatchV1(transactionId, returned);
      await pollPayoutJournal(submittedJournal, plan);
    } catch (error) {
      if (submittedJournal !== null) setPayout({ kind: 'submitted', plan, journal: submittedJournal, signature: submittedJournal.signature!, confirmation: `${errorMessage(error)} The transaction is saved. Reload to check its status.` });
      else setPayout({ kind: 'refused', reason: errorMessage(error), journal: unsignedJournal });
    }
  }

  const discardReplay = async (journal: ClientOperationJournalV1) => {
    try { await discardUnsignedClientOperationJournalV1(browserStorage(), journal); setReplay({ kind: 'idle' }); }
    catch (error) { setReplay({ kind: 'refused', reason: errorMessage(error), journal }); }
  };
  const discardPayout = async (journal: ClientOperationJournalV1) => {
    try { await discardUnsignedClientOperationJournalV1(browserStorage(), journal); setPayout({ kind: 'idle' }); }
    catch (error) { setPayout({ kind: 'refused', reason: errorMessage(error), journal }); }
  };

  const readyPlan = payout.kind === 'ready' || payout.kind === 'signing' || payout.kind === 'submitted' ? payout.plan : null;
  const summary = readyPlan === null ? null : walletTerminalPayoutSummaryV3(readyPlan.report);
  const replayUnsigned = (replay.kind === 'ready' || replay.kind === 'signing' || replay.kind === 'refused') && replay.journal?.phase === 'unsigned' ? replay.journal : null;
  const payoutUnsigned = (payout.kind === 'ready' || payout.kind === 'signing' || payout.kind === 'refused') && payout.journal?.phase === 'unsigned' ? payout.journal : null;

  return <div className="redeem-flow">
    <h4 className="detail-subhead">Redeem</h4>
    <p className="direct-status">Set up the market’s payment record if needed, then review and sign your payout. Redemption burns your paying claims and sends collateral to your token account.</p>
    <p className="direct-status">Your progress is saved in this browser. If interrupted, reload to check the same transaction.</p>
    {recovery !== '' && <p className="direct-status" aria-live="polite">{recovery}</p>}
    {replay.kind === 'idle' && <div className="direct-actions"><button type="button" onClick={() => void inspect()}>Check redemption</button></div>}
    {replay.kind === 'inspecting' && <p className="direct-status" aria-live="polite">Checking the Market&apos;s finalized payment record…</p>}
    {replay.kind === 'refused' && <p className="market-refusal">Refused: {replay.reason}</p>}
    {(replay.kind === 'ready' || replay.kind === 'signing') && replay.state.status === 'refused' && <p className="market-refusal">Refused: {replay.state.reason}</p>}
    {(replay.kind === 'ready' || replay.kind === 'signing') && replay.state.status === 'creatable' && <>
      <dl className="market-card-facts">
        <div><dt>Payment record</dt><dd title={replay.state.plan.replayAddress}>{replay.state.plan.replayAddress}</dd></div>
        <div><dt>Refundable storage deposit</dt><dd>{replay.state.plan.rentLamports} lamports</dd></div>
        <div><dt>Transaction</dt><dd>{replay.state.plan.wireBytes.length} bytes · one signer</dd></div>
      </dl>
      <div className="direct-actions"><button type="button" disabled={replay.kind === 'signing'} onClick={() => void createReplay()}>{replay.kind === 'signing' ? 'Waiting for your wallet…' : 'Create payment record'}</button></div>
      <p className="direct-status">The storage deposit returns to the same wallet when the record can be closed.</p>
    </>}
    {replayUnsigned !== null && <div className="direct-actions"><button type="button" className="secondary-action" onClick={() => void discardReplay(replayUnsigned)}>Discard this unsigned saved plan</button></div>}
    {replay.kind === 'submitted' && <p className="direct-status" aria-live="polite">Submitted as <code>{replay.signature}</code>. {replay.confirmation}</p>}
    {replay.kind === 'confirmed' && <div className="portfolio-claim"><span>Payment record ready</span><strong>revision {replay.nextRevision}</strong><p>{replay.signature === null ? 'Setup is complete.' : <>Signature <code>{replay.signature}</code> is finalized.</>} The record at <code>{replay.replayAddress}</code> is ready.</p></div>}
    {replay.kind === 'ready' && replay.state.status === 'exists' && <p className="direct-status">The payment record already exists at <code>{replay.state.replayAddress}</code> (revision {replay.state.nextRevision}), so you can proceed to the payout.</p>}

    <details className="trade-v3-bytes" open={payout.kind !== 'idle'}>
      <summary>Review and execute a payout plan</summary>
      <p className="direct-status">Leave the recipient blank to receive collateral in your wallet’s associated token account. You can also choose another token account or import a payout plan. Review the amount and destination before signing.</p>
      <label><span>Collateral token account for your proceeds — optional; empty means your associated token account</span><input value={recipient} spellCheck={false} disabled={payoutUnsigned !== null || payout.kind === 'submitted'} onChange={(event) => { setRecipient(event.target.value); setPayout({ kind: 'idle' }); }} /></label>
      <label><span>Payout plan file — optional</span><input type="file" accept="application/json,.json" disabled={payoutUnsigned !== null || payout.kind === 'submitted'} onChange={(event) => void importPayoutFile(event)} /></label>
      {manifestSource !== '' && <p className="direct-status" aria-live="polite">Imported {manifestSource}.</p>}
      <label><span>Payout plan JSON — optional; empty means derive it here</span><textarea rows={7} spellCheck={false} disabled={payoutUnsigned !== null || payout.kind === 'submitted'} value={manifestText} onChange={(event) => { setManifestText(event.target.value); setPayout({ kind: 'idle' }); }} /></label>
      {!replayExists && <p className="direct-status">Complete payment-record setup above before checking the payout plan.</p>}
      <div className="direct-actions"><button type="button" disabled={!replayExists || payout.kind === 'preparing' || payout.kind === 'signing' || payout.kind === 'submitted'} onClick={() => void preparePayout()}>{payout.kind === 'preparing' ? 'Checking payout plan…' : 'Check payout plan'}</button></div>
      {payout.kind === 'refused' && <p className="market-refusal">Refused: {payout.reason}</p>}
      {summary !== null && <>
        <dl className="market-card-facts">
          <div><dt>Winning atoms burned</dt><dd>{readyPlan?.report.request.quantity}</dd></div>
          <div><dt>Collateral atoms paid</dt><dd>{summary.payout}</dd></div>
          <div><dt>Transaction</dt><dd>{readyPlan?.wireBytes.length} bytes · v0 · one signer</dd></div>
          <div><dt>Checked release</dt><dd title={readyPlan?.report.request.releaseSet}>{readyPlan?.report.request.releaseSet.slice(0, 16)}…</dd></div>
          <div><dt>Lookup table</dt><dd title={readyPlan?.lookupTable}>{readyPlan?.lookupTable.slice(0, 16)}…</dd></div>
          <div><dt>Request digest</dt><dd title={summary.requestDigest}>{summary.requestDigest.slice(0, 16)}…</dd></div>
        </dl>
        {(payout.kind === 'ready' || payout.kind === 'signing') && <div className="direct-actions"><button type="button" disabled={payout.kind === 'signing'} onClick={() => void signPayout()}>{payout.kind === 'signing' ? 'Waiting for your wallet…' : `Redeem ${readyPlan?.report.request.quantity} winning atoms`}</button></div>}
      </>}
      {payoutUnsigned !== null && <div className="direct-actions"><button type="button" className="secondary-action" onClick={() => void discardPayout(payoutUnsigned)}>Discard this unsigned saved plan</button></div>}
      {payout.kind === 'submitted' && <p className="direct-status" aria-live="polite">Submitted as <code>{payout.signature}</code>. {payout.confirmation}</p>}
      {payout.kind === 'confirmed' && <div className="portfolio-claim"><span>Payout verified at finalized slot {payout.observedSlot}</span><strong>{payout.payout} collateral atoms</strong><p>Signature <code>{payout.signature}</code>. Your claims were redeemed and the collateral was paid to your recipient account.</p></div>}
    </details>
  </div>;
}
