'use client';

import { useState } from 'react';

import { type WalletDirectoryHandleV1 } from '@/components/WalletDirectory';
import {
  PLAIN_POSITION_PAYOUT_BLOCK_V1,
  inspectClaimsCustodyReplayV1,
  type ClaimsCustodyReplayStateV1,
} from '@/lib/claimsCustodyReplay';
import { SolanaRpcClient } from '@/lib/rpc';
import { requestWalletTransactionSignatureV1, submitSignedTransactionV1 } from '@/lib/walletHandoff';

/**
 * The redemption flow a resolved Market's winning Position opens.
 *
 * ADR-0008 §7.3 fixes the shape: every redemption plan OPENS with the
 * Claims-role Custody replay creation when that replay is absent — a
 * permissionless, prepaid, packet-bound legacy transaction any wallet can
 * sign. That step is fully executable here: built from the aggregate's own
 * persisted namespace, signed by the connected wallet, submitted through the
 * one RPC seam, and confirmed by polling the signature it produced.
 *
 * The payout instruction itself is NOT wallet-constructible yet, and this
 * surface says exactly why instead of pretending: see
 * `PLAIN_POSITION_PAYOUT_BLOCK_V1`.
 */

type FlowState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'inspecting' }>
  | Readonly<{ kind: 'ready'; state: ClaimsCustodyReplayStateV1 }>
  | Readonly<{ kind: 'signing'; state: ClaimsCustodyReplayStateV1 }>
  | Readonly<{ kind: 'submitted'; state: ClaimsCustodyReplayStateV1; signature: string; confirmation: string }>
  | Readonly<{ kind: 'confirmed'; signature: string; confirmation: string; replayAddress: string; nextRevision: string }>
  | Readonly<{ kind: 'refused'; reason: string }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'the redemption step refused without a usable reason';
}

export default function RedeemFlow({
  endpoint,
  marketAddress,
  claimsProgramId,
  custodyProgramId,
  registryProgramId,
  directory,
}: Readonly<{
  endpoint: string;
  marketAddress: string;
  claimsProgramId: string;
  custodyProgramId: string;
  registryProgramId: string;
  directory: WalletDirectoryHandleV1;
}>) {
  const [flow, setFlow] = useState<FlowState>({ kind: 'idle' });
  const wallet = directory.address;

  async function inspect() {
    setFlow({ kind: 'inspecting' });
    if (wallet === null) {
      setFlow({ kind: 'refused', reason: 'connect a browser wallet first: the replay creation is prepaid by, and refunds to, the wallet that signs it' });
      return;
    }
    if (custodyProgramId === '' || registryProgramId === '') {
      setFlow({ kind: 'refused', reason: 'the Custody and Registry programs are required: the replay lives under Custody and the caller authority is authenticated against the Registry activation cache' });
      return;
    }
    const state = await inspectClaimsCustodyReplayV1(new SolanaRpcClient(endpoint), {
      marketAddress,
      claimsProgramId,
      custodyProgramId,
      registryProgramId,
      payer: wallet,
    });
    setFlow({ kind: 'ready', state });
  }

  async function signAndSubmit() {
    if (flow.kind !== 'ready' || flow.state.status !== 'creatable' || wallet === null) return;
    const plan = flow.state.plan;
    setFlow({ kind: 'signing', state: flow.state });
    try {
      const signed = await requestWalletTransactionSignatureV1(directory.handoff(endpoint), plan.transaction, wallet);
      if (!signed.complete) throw new Error('the wallet did not complete the single required signature');
      const client = new SolanaRpcClient(endpoint);
      const signature = await submitSignedTransactionV1(client, signed.transaction);
      setFlow({ kind: 'submitted', state: flow.state, signature, confirmation: 'submitted; awaiting finalized status' });
      for (let attempt = 0; attempt < 30; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 1_000));
        const [status] = await client.signatureStatuses([signature]);
        if (status !== undefined && status.known) {
          if (status.succeeded === false) {
            setFlow({ kind: 'refused', reason: `the chain refused the submitted transaction: ${status.errorText ?? 'unnamed chain error'}` });
            return;
          }
          if (status.confirmationStatus === 'finalized' || status.confirmationStatus === 'confirmed') {
            const confirmed = await inspectClaimsCustodyReplayV1(client, {
              marketAddress, claimsProgramId, custodyProgramId, registryProgramId, payer: wallet,
            });
            if (confirmed.status === 'exists') {
              setFlow({ kind: 'confirmed', signature, confirmation: status.confirmationStatus, replayAddress: confirmed.replayAddress, nextRevision: confirmed.nextRevision });
            } else {
              setFlow({ kind: 'refused', reason: `the transaction ${status.confirmationStatus} but the replay did not decode afterwards: ${confirmed.status === 'refused' ? confirmed.reason : 'the derived address is still vacant'}` });
            }
            return;
          }
          setFlow({ kind: 'submitted', state: flow.state, signature, confirmation: status.confirmationStatus ?? 'processed' });
        }
      }
      setFlow({ kind: 'refused', reason: 'the transaction was submitted but no confirmation arrived within 30 seconds; check the signature on the activity surface' });
    } catch (error) {
      setFlow({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  return <div className="redeem-flow">
    <h4 className="detail-subhead">Redeem</h4>
    <p className="direct-status">Every payout plan opens with this Market&apos;s Claims-role Custody replay — the cursor a redemption replays against. Creating it is permissionless, prepaid by the connected wallet, refunded to the same wallet when the cursor closes, and fits one legacy packet by design.</p>
    {flow.kind === 'idle' && <div className="direct-actions">
      <button type="button" onClick={() => void inspect()}>Prepare redemption</button>
    </div>}
    {flow.kind === 'inspecting' && <p className="direct-status" aria-live="polite">Deriving the replay address and reading it at a finalized floor…</p>}
    {flow.kind === 'refused' && <>
      <p className="market-refusal">Refused: {flow.reason}</p>
      <div className="direct-actions"><button type="button" className="secondary-action" onClick={() => void inspect()}>Inspect again</button></div>
    </>}
    {(flow.kind === 'ready' || flow.kind === 'signing') && flow.state.status === 'refused' && <p className="market-refusal">Refused: {flow.state.reason}</p>}
    {(flow.kind === 'ready' || flow.kind === 'signing') && flow.state.status === 'exists' && <>
      <p className="direct-status">The Claims-role replay already exists at <code>{flow.state.replayAddress}</code> (next revision {flow.state.nextRevision}); no creation is owed.</p>
      <p className="market-capability-refusal"><span>payout leg</span>{PLAIN_POSITION_PAYOUT_BLOCK_V1}</p>
    </>}
    {(flow.kind === 'ready' || flow.kind === 'signing') && flow.state.status === 'creatable' && <>
      <dl className="market-card-facts">
        <div><dt>Replay to create</dt><dd title={flow.state.plan.replayAddress}>{flow.state.plan.replayAddress}</dd></div>
        <div><dt>Exact prepaid rent</dt><dd>{flow.state.plan.rentLamports} lamports · refunds to the connected wallet</dd></div>
        <div><dt>Transaction</dt><dd>{flow.state.plan.wireBytes.length} bytes · legacy · one signer</dd></div>
        <div><dt>Custody request digest</dt><dd title={flow.state.plan.custodyRequestDigestHex}>{flow.state.plan.custodyRequestDigestHex.slice(0, 16)}…</dd></div>
      </dl>
      <div className="direct-actions">
        <button type="button" disabled={flow.kind === 'signing'} onClick={() => void signAndSubmit()}>
          {flow.kind === 'signing' ? 'Awaiting wallet signature and submission…' : 'Sign with connected wallet and submit'}
        </button>
      </div>
      <p className="direct-status">{flow.state.note}</p>
    </>}
    {flow.kind === 'submitted' && <p className="direct-status" aria-live="polite">Submitted as <code>{flow.signature}</code> · {flow.confirmation}…</p>}
    {flow.kind === 'confirmed' && <>
      <div className="portfolio-claim">
        <span>Replay created and confirmed</span>
        <strong>{flow.confirmation}</strong>
        <p>Signature <code>{flow.signature}</code>. The Claims-role Custody replay now exists at <code>{flow.replayAddress}</code> with next revision {flow.nextRevision} — the exact cursor a payout replays against.</p>
      </div>
      <p className="market-capability-refusal"><span>payout leg</span>{PLAIN_POSITION_PAYOUT_BLOCK_V1}</p>
    </>}
  </div>;
}
