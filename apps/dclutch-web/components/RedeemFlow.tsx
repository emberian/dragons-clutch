'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';

import { type WalletDirectoryHandleV1 } from '@/components/WalletDirectory';
import { inspectClaimsCustodyReplayV1, type ClaimsCustodyReplayRequestV1, type ClaimsCustodyReplayStateV1 } from '@/lib/claimsCustodyReplay';
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
import { SolanaRpcClient } from '@/lib/rpc';
import {
  finalizeWalletTerminalPayoutV3,
  parseWalletTerminalPayoutManifestV3,
  prepareWalletTerminalPayoutV3,
  walletTerminalPayoutSummaryV3,
  type PreparedWalletTerminalPayoutV3,
} from '@/lib/walletTerminalPayoutV3';
import { requestWalletTransactionSignatureV1, submitSignedTransactionV1 } from '@/lib/walletHandoff';

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

export default function RedeemFlow({ endpoint, marketAddress, positionAddress, claimIndex, availableQuantity, claimsProgramId, custodyProgramId, registryProgramId, directory }: Readonly<{
  endpoint: string; marketAddress: string; positionAddress: string; claimIndex: number; availableQuantity: string;
  claimsProgramId: string; custodyProgramId: string; registryProgramId: string; directory: WalletDirectoryHandleV1;
}>) {
  const client = useMemo(() => new SolanaRpcClient(endpoint), [endpoint]);
  const [replay, setReplay] = useState<ReplayFlow>({ kind: 'idle' });
  const [manifestText, setManifestText] = useState('');
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
          setReplay({ kind: 'submitted', journal, signature, confirmation: `The chain reports an error (${status.errorText ?? 'unnamed chain error'}). This submitted record stays saved because it cannot be safely replayed or discarded.` }); return;
        }
        if (claimsReplayFinalizedCompletionV1(status, state)) {
          await clearFinalizedClientOperationJournalV1(browserStorage(), journal);
          if (alive()) setReplay({ kind: 'confirmed', signature, replayAddress: state.replayAddress, nextRevision: state.nextRevision });
          return;
        }
        setReplay({ kind: 'submitted', journal, signature, confirmation: 'The exact signature or payment record is not finalized yet. You can close this page; reloading resumes this signature and never submits it again.' });
      } catch (error) {
        if (alive()) setReplay({ kind: 'submitted', journal, signature, confirmation: `${errorMessage(error)} The submitted record stays saved; reloading only resumes its exact signature.` });
        return;
      }
      await pause();
    }
    if (alive()) setReplay({ kind: 'submitted', journal, signature, confirmation: 'Finalized completion is still unresolved. You can reload later; this exact signature stays saved and is never replayed.' });
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
          if (alive()) setPayout({ kind: 'submitted', plan, journal, signature, confirmation: `${errorMessage(error)} The submitted record stays saved because this page will not replay or discard an ambiguous payout.` });
          return;
        }
      }
      if (alive()) setPayout({ kind: 'submitted', plan, journal, signature, confirmation: 'The exact finalized receipt and five account changes are still pending. You can close this page; reloading resumes this signature without submitting again.' });
      await pause();
    }
    if (alive()) setPayout({ kind: 'submitted', plan, journal, signature, confirmation: 'Finalized completion is still unresolved. You can reload later; this exact signature stays saved and is never replayed.' });
  }, [client]);

  useEffect(() => {
    let current = true;
    if (wallet === null || custodyProgramId === '' || registryProgramId === '') return () => { current = false; };
    void (async () => {
      setRecovery('Checking this browser for an exact saved redemption operation…');
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
      setRecovery(replayJournal === null && payoutJournal === null ? 'No saved redemption operation exists for this exact chain, Market, and wallet.' : 'A saved operation was found and is being authenticated against finalized chain state.');

      if (replayJournal !== null) {
        if (replayJournal.phase === 'submitted') {
          setReplay({ kind: 'submitted', journal: replayJournal, signature: replayJournal.signature!, confirmation: 'Resuming the exact saved signature at finalized commitment…' });
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
            setPayout({ kind: 'submitted', plan: restored.plan, journal: payoutJournal, signature: payoutJournal.signature!, confirmation: 'Resuming the exact saved signature and finalized verifier…' });
            void pollPayoutJournal(payoutJournal, restored.plan, () => current);
          } else {
            const fresh = await prepareWalletTerminalPayoutV3(client, restored.manifest, wallet);
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
    if (custodyProgramId === '' || registryProgramId === '') { setReplay({ kind: 'refused', reason: 'this deployment does not name all of the programs the payout needs', journal: null }); return; }
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
      setReplay({ kind: 'submitted', journal: submittedJournal, signature: transactionId, confirmation: 'Saved before submission; sending the exact signed packet…' });
      const returned = await submitSignedTransactionV1(client, submittedClientOperationWireV1(submittedJournal));
      requireSubmittedSignatureMatchV1(transactionId, returned);
      await pollReplayJournal(submittedJournal, wallet);
    } catch (error) {
      if (submittedJournal !== null) setReplay({ kind: 'submitted', journal: submittedJournal, signature: submittedJournal.signature!, confirmation: `${errorMessage(error)} The submitted record stays saved; reloading never resubmits it.` });
      else setReplay({ kind: 'refused', reason: errorMessage(error), journal: unsignedJournal });
    }
  }

  async function preparePayout() {
    if (!replayExists || wallet === null) return;
    setPayout({ kind: 'preparing' });
    try {
      if (BigInt(availableQuantity) === 0n) throw new Error('this Position holds zero winning atoms, so there is nothing to redeem');
      const manifest = parseWalletTerminalPayoutManifestV3(manifestText);
      if (manifest.request.market !== marketAddress || manifest.request.position !== positionAddress || manifest.request.owner !== wallet || manifest.request.claimIndex !== claimIndex) throw new Error('the payout plan names another Market, Position, owner, or winning claim');
      if (BigInt(manifest.request.quantity) > BigInt(availableQuantity)) throw new Error('the payout plan tries to redeem more winning atoms than this Position holds');
      const plan = await prepareWalletTerminalPayoutV3(client, manifest, wallet); const scope = await operationScope(wallet);
      const journal = await writeUnsignedClientOperationJournalV1(browserStorage(), terminalPayoutJournalInputV1(scope, manifest, plan));
      setPayout({ kind: 'ready', plan, journal });
    } catch (error) { setPayout({ kind: 'refused', reason: errorMessage(error), journal: null }); }
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
      setPayout({ kind: 'submitted', plan, journal: submittedJournal, signature: transactionId, confirmation: 'Saved before submission; sending the exact signed packet…' });
      const returned = await submitSignedTransactionV1(client, submittedClientOperationWireV1(submittedJournal));
      requireSubmittedSignatureMatchV1(transactionId, returned);
      await pollPayoutJournal(submittedJournal, plan);
    } catch (error) {
      if (submittedJournal !== null) setPayout({ kind: 'submitted', plan, journal: submittedJournal, signature: submittedJournal.signature!, confirmation: `${errorMessage(error)} The submitted record stays saved; reloading never resubmits it.` });
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
    <p className="direct-status">You redeem in two checked steps. First, this Market needs one reusable payment record for your claims. Then you review and sign a payout plan built from the current finalized Market state. The page rechecks the plan, its lookup table, the returned receipt, your changed claim balance, and both changed token balances.</p>
    <p className="direct-status">Before your wallet signs, this page saves the exact chain, Market, owner, operation digest, intent, and plan in this browser. Before submission it saves the signed transaction id. Reloading resumes only that exact signature; it never submits it again. Browser data is an untrusted projection, and the onchain programs refuse substitutions.</p>
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
    {replay.kind === 'confirmed' && <div className="portfolio-claim"><span>Payment record verified at finalized commitment</span><strong>revision {replay.nextRevision}</strong><p>{replay.signature === null ? 'The exact payment record is complete.' : <>Signature <code>{replay.signature}</code> is finalized.</>} The record at <code>{replay.replayAddress}</code> is ready.</p></div>}
    {replay.kind === 'ready' && replay.state.status === 'exists' && <p className="direct-status">Your payment record already exists at <code>{replay.state.replayAddress}</code> (revision {replay.state.nextRevision}), so no setup transaction is owed.</p>}

    {replayExists && <details className="trade-v3-bytes" open={payout.kind !== 'idle'}>
      <summary>Review and execute a payout plan</summary>
      <p className="direct-status">Paste the payout plan produced for this exact Position. Before your wallet opens, the page reads the current finalized accounts again, checks every plan field and the one exact lookup table, and refuses a stale or substituted plan.</p>
      <label><span>Payout plan JSON</span><textarea rows={7} spellCheck={false} disabled={payoutUnsigned !== null || payout.kind === 'submitted'} value={manifestText} onChange={(event) => { setManifestText(event.target.value); setPayout({ kind: 'idle' }); }} /></label>
      <div className="direct-actions"><button type="button" disabled={payout.kind === 'preparing' || payout.kind === 'signing' || payout.kind === 'submitted'} onClick={() => void preparePayout()}>{payout.kind === 'preparing' ? 'Checking payout plan…' : 'Check payout plan'}</button></div>
      {payout.kind === 'refused' && <p className="market-refusal">Refused: {payout.reason}</p>}
      {summary !== null && <>
        <dl className="market-card-facts">
          <div><dt>Winning atoms burned</dt><dd>{readyPlan?.report.request.quantity}</dd></div>
          <div><dt>Collateral atoms paid</dt><dd>{summary.payout}</dd></div>
          <div><dt>Transaction</dt><dd>{readyPlan?.wireBytes.length} bytes · v0 · one signer</dd></div>
          <div><dt>Request digest</dt><dd title={summary.requestDigest}>{summary.requestDigest.slice(0, 16)}…</dd></div>
        </dl>
        {(payout.kind === 'ready' || payout.kind === 'signing') && <div className="direct-actions"><button type="button" disabled={payout.kind === 'signing'} onClick={() => void signPayout()}>{payout.kind === 'signing' ? 'Waiting for your wallet…' : `Redeem ${readyPlan?.report.request.quantity} winning atoms`}</button></div>}
      </>}
      {payoutUnsigned !== null && <div className="direct-actions"><button type="button" className="secondary-action" onClick={() => void discardPayout(payoutUnsigned)}>Discard this unsigned saved plan</button></div>}
      {payout.kind === 'submitted' && <p className="direct-status" aria-live="polite">Submitted as <code>{payout.signature}</code>. {payout.confirmation}</p>}
      {payout.kind === 'confirmed' && <div className="portfolio-claim"><span>Payout verified at finalized slot {payout.observedSlot}</span><strong>{payout.payout} collateral atoms</strong><p>Signature <code>{payout.signature}</code>. The returned receipt, your claim debit, the payment record, the Market&apos;s collateral balance, and your recipient balance all match the same exact payout.</p></div>}
    </details>}
  </div>;
}
