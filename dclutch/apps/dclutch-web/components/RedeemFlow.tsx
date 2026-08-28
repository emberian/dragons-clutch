'use client';

import { useMemo, useState } from 'react';

import { type WalletDirectoryHandleV1 } from '@/components/WalletDirectory';
import {
  inspectClaimsCustodyReplayV1,
  type ClaimsCustodyReplayStateV1,
} from '@/lib/claimsCustodyReplay';
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
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'inspecting' }>
  | Readonly<{ kind: 'ready'; state: ClaimsCustodyReplayStateV1 }>
  | Readonly<{ kind: 'signing'; state: ClaimsCustodyReplayStateV1 }>
  | Readonly<{ kind: 'submitted'; state: ClaimsCustodyReplayStateV1; signature: string; confirmation: string }>
  | Readonly<{ kind: 'confirmed'; signature: string; confirmation: string; replayAddress: string; nextRevision: string }>
  | Readonly<{ kind: 'refused'; reason: string }>;

type PayoutFlow =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'preparing' }>
  | Readonly<{ kind: 'ready'; plan: PreparedWalletTerminalPayoutV3 }>
  | Readonly<{ kind: 'signing'; plan: PreparedWalletTerminalPayoutV3 }>
  | Readonly<{ kind: 'submitted'; plan: PreparedWalletTerminalPayoutV3; signature: string; confirmation: string }>
  | Readonly<{ kind: 'confirmed'; signature: string; observedSlot: string; payout: string }>
  | Readonly<{ kind: 'refused'; reason: string }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'the redemption step refused without a usable reason';
}

function retryableFinality(error: unknown): boolean {
  const message = errorMessage(error);
  return message.includes('not available at finalized commitment yet') || message.includes('finalized account floor has not reached');
}

export default function RedeemFlow({
  endpoint,
  marketAddress,
  positionAddress,
  claimIndex,
  availableQuantity,
  claimsProgramId,
  custodyProgramId,
  registryProgramId,
  directory,
}: Readonly<{
  endpoint: string;
  marketAddress: string;
  positionAddress: string;
  claimIndex: number;
  availableQuantity: string;
  claimsProgramId: string;
  custodyProgramId: string;
  registryProgramId: string;
  directory: WalletDirectoryHandleV1;
}>) {
  const client = useMemo(() => new SolanaRpcClient(endpoint), [endpoint]);
  const [replay, setReplay] = useState<ReplayFlow>({ kind: 'idle' });
  const [manifestText, setManifestText] = useState('');
  const [payout, setPayout] = useState<PayoutFlow>({ kind: 'idle' });
  const wallet = directory.address;
  const replayExists = (replay.kind === 'ready' && replay.state.status === 'exists') || replay.kind === 'confirmed';

  async function inspect() {
    setReplay({ kind: 'inspecting' });
    if (wallet === null) {
      setReplay({ kind: 'refused', reason: 'connect a browser wallet first: your wallet owns the claim balance and must authorize its payout' });
      return;
    }
    if (custodyProgramId === '' || registryProgramId === '') {
      setReplay({ kind: 'refused', reason: 'this deployment does not name all of the programs the payout needs' });
      return;
    }
    const state = await inspectClaimsCustodyReplayV1(client, {
      marketAddress, claimsProgramId, custodyProgramId, registryProgramId, payer: wallet,
    });
    setReplay({ kind: 'ready', state });
  }

  async function createReplay() {
    if (replay.kind !== 'ready' || replay.state.status !== 'creatable' || wallet === null) return;
    const plan = replay.state.plan;
    setReplay({ kind: 'signing', state: replay.state });
    try {
      const signed = await requestWalletTransactionSignatureV1(client, directory.handoff(endpoint), plan.transaction, wallet);
      if (!signed.complete) throw new Error('the wallet did not complete the one required signature');
      const signature = await submitSignedTransactionV1(client, signed.transaction);
      setReplay({ kind: 'submitted', state: replay.state, signature, confirmation: 'submitted; waiting for the payment record at finalized commitment' });
      for (let attempt = 0; attempt < 30; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 1_000));
        const [status] = await client.signatureStatuses([signature]);
        if (status !== undefined && status.known && status.succeeded === false) {
          setReplay({ kind: 'refused', reason: `the chain refused the submitted transaction: ${status.errorText ?? 'unnamed chain error'}` });
          return;
        }
        const confirmed = await inspectClaimsCustodyReplayV1(client, {
          marketAddress, claimsProgramId, custodyProgramId, registryProgramId, payer: wallet,
        });
        if (confirmed.status === 'exists') {
          const confirmation = status !== undefined && status.known && status.confirmationStatus !== null
            ? status.confirmationStatus
            : 'finalized account read';
          setReplay({ kind: 'confirmed', signature, confirmation, replayAddress: confirmed.replayAddress, nextRevision: confirmed.nextRevision });
          return;
        }
        setReplay({ kind: 'submitted', state: replay.state, signature, confirmation: 'submitted; the finalized payment record is still pending' });
      }
      setReplay({ kind: 'refused', reason: 'the transaction was submitted, but its payment record did not appear at finalized commitment within 30 seconds; inspect the signature before trying again' });
    } catch (error) {
      setReplay({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  async function preparePayout() {
    if (!replayExists || wallet === null) return;
    setPayout({ kind: 'preparing' });
    try {
      if (BigInt(availableQuantity) === 0n) throw new Error('this Position holds zero winning atoms, so there is nothing to redeem');
      const manifest = parseWalletTerminalPayoutManifestV3(manifestText);
      if (manifest.request.market !== marketAddress || manifest.request.position !== positionAddress
          || manifest.request.owner !== wallet || manifest.request.claimIndex !== claimIndex) {
        throw new Error('the payout plan names another Market, Position, owner, or winning claim');
      }
      if (BigInt(manifest.request.quantity) > BigInt(availableQuantity)) throw new Error('the payout plan tries to redeem more winning atoms than this Position holds');
      const plan = await prepareWalletTerminalPayoutV3(client, manifest, wallet);
      setPayout({ kind: 'ready', plan });
    } catch (error) {
      setPayout({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  async function signPayout() {
    if (payout.kind !== 'ready' || wallet === null) return;
    const plan = payout.plan;
    setPayout({ kind: 'signing', plan });
    try {
      const signed = await requestWalletTransactionSignatureV1(client, directory.handoff(endpoint), plan.transaction, wallet);
      if (!signed.complete) throw new Error('the wallet did not complete the one required signature');
      const signature = await submitSignedTransactionV1(client, signed.transaction);
      setPayout({ kind: 'submitted', plan, signature, confirmation: 'submitted; waiting for the exact finalized payout result' });
      for (let attempt = 0; attempt < 45; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 1_000));
        const [status] = await client.signatureStatuses([signature]);
        if (status !== undefined && status.known && status.succeeded === false) {
          setPayout({ kind: 'refused', reason: `the chain refused the payout: ${status.errorText ?? 'unnamed chain error'}` });
          return;
        }
        try {
          const confirmed = await finalizeWalletTerminalPayoutV3(client, signature, plan);
          setPayout({ kind: 'confirmed', signature, observedSlot: confirmed.observedSlot, payout: confirmed.payout });
          return;
        } catch (error) {
          if (!retryableFinality(error)) throw error;
        }
        setPayout({ kind: 'submitted', plan, signature, confirmation: 'submitted; the finalized receipt and account changes are still pending' });
      }
      setPayout({ kind: 'refused', reason: 'the payout was submitted, but its receipt and account changes were not jointly visible at finalized commitment within 45 seconds; inspect the signature before trying again' });
    } catch (error) {
      setPayout({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  const readyPlan = payout.kind === 'ready' || payout.kind === 'signing' || payout.kind === 'submitted' ? payout.plan : null;
  const summary = readyPlan === null ? null : walletTerminalPayoutSummaryV3(readyPlan.report);

  return <div className="redeem-flow">
    <h4 className="detail-subhead">Redeem</h4>
    <p className="direct-status">You redeem in two checked steps. First, this Market needs one reusable payment record for your claims. Then you review and sign a payout plan built from the current finalized Market state. The page rechecks the plan, its lookup table, the returned receipt, your changed claim balance, and both changed token balances.</p>
    {replay.kind === 'idle' && <div className="direct-actions"><button type="button" onClick={() => void inspect()}>Check redemption</button></div>}
    {replay.kind === 'inspecting' && <p className="direct-status" aria-live="polite">Checking the Market&apos;s finalized payment record…</p>}
    {replay.kind === 'refused' && <><p className="market-refusal">Refused: {replay.reason}</p><div className="direct-actions"><button type="button" className="secondary-action" onClick={() => void inspect()}>Check again</button></div></>}
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
    {replay.kind === 'submitted' && <p className="direct-status" aria-live="polite">Submitted as <code>{replay.signature}</code> · {replay.confirmation}…</p>}
    {replay.kind === 'confirmed' && <div className="portfolio-claim"><span>Payment record confirmed</span><strong>{replay.confirmation}</strong><p>Signature <code>{replay.signature}</code>. The record at <code>{replay.replayAddress}</code> is ready at revision {replay.nextRevision}.</p></div>}
    {replay.kind === 'ready' && replay.state.status === 'exists' && <p className="direct-status">Your payment record already exists at <code>{replay.state.replayAddress}</code> (revision {replay.state.nextRevision}), so no setup transaction is owed.</p>}

    {replayExists && <details className="trade-v3-bytes" open={payout.kind !== 'idle'}>
      <summary>Review and execute a payout plan</summary>
      <p className="direct-status">Paste the payout plan produced for this exact Position. Before your wallet opens, the page reads the current finalized accounts again, checks every plan field and the one exact lookup table, and refuses a stale or substituted plan.</p>
      <label><span>Payout plan JSON</span><textarea rows={7} spellCheck={false} value={manifestText} onChange={(event) => { setManifestText(event.target.value); setPayout({ kind: 'idle' }); }} /></label>
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
      {payout.kind === 'submitted' && <p className="direct-status" aria-live="polite">Submitted as <code>{payout.signature}</code> · {payout.confirmation}…</p>}
      {payout.kind === 'confirmed' && <div className="portfolio-claim"><span>Payout verified at finalized slot {payout.observedSlot}</span><strong>{payout.payout} collateral atoms</strong><p>Signature <code>{payout.signature}</code>. The returned receipt, your claim debit, the payment record, the Market&apos;s collateral balance, and your recipient balance all match the same exact payout.</p></div>}
    </details>}
  </div>;
}
