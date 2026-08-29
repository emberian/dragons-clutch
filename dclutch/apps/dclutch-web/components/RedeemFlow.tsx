'use client';

import { useState } from 'react';

import Anchor from '@/components/Anchor';
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
      setFlow({ kind: 'submitted', state: flow.state, signature, confirmation: 'submitted; awaiting the created replay at finalized commitment' });
      // Confirmation is the POSTCONDITION, not a status row: the replay account
      // decoding at its derived address at finalized commitment is what a
      // payout needs. The signature status is polled beside it because it can
      // answer sooner and can carry a chain refusal — but a node configured
      // without transaction history answers status `null` forever (measured on
      // a resumed local ledger), and the created account must still confirm.
      for (let attempt = 0; attempt < 30; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 1_000));
        const [status] = await client.signatureStatuses([signature]);
        if (status !== undefined && status.known && status.succeeded === false) {
          setFlow({ kind: 'refused', reason: `the chain refused the submitted transaction: ${status.errorText ?? 'unnamed chain error'}` });
          return;
        }
        const confirmed = await inspectClaimsCustodyReplayV1(client, {
          marketAddress, claimsProgramId, custodyProgramId, registryProgramId, payer: wallet,
        });
        if (confirmed.status === 'exists') {
          const confirmation = status !== undefined && status.known && status.confirmationStatus !== null
            ? status.confirmationStatus
            : 'finalized (postcondition read; this node serves no signature history)';
          setFlow({ kind: 'confirmed', signature, confirmation, replayAddress: confirmed.replayAddress, nextRevision: confirmed.nextRevision });
          return;
        }
        setFlow({ kind: 'submitted', state: flow.state, signature, confirmation: status !== undefined && status.known ? status.confirmationStatus ?? 'processed' : 'submitted; the derived replay address is still vacant at the finalized floor' });
      }
      setFlow({ kind: 'refused', reason: 'the transaction was submitted but the replay did not appear at finalized commitment within 30 seconds; check the signature on the activity surface' });
    } catch (error) {
      setFlow({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  return <div className="redeem-flow">
    <h4 className="detail-subhead">Redeem</h4>
    <p className="direct-status">Redeeming is two steps. Step one — creating this market&apos;s redemption cursor — works from your wallet today: it costs a storage deposit that comes back to you plus one network fee, and anyone may do it. Step two, the payout itself, is not wallet-ready yet, and this panel says exactly why below instead of hiding the button.</p>
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
    {flow.kind === 'submitted' && <p className="direct-status" aria-live="polite">Submitted as <code>{flow.signature}</code> · {flow.confirmation}… <Anchor href={`/explorer?view=transaction&q=${encodeURIComponent(flow.signature)}`}>Open your transaction in the explorer →</Anchor></p>}
    {flow.kind === 'confirmed' && <>
      <div className="portfolio-claim">
        <span>Replay created and confirmed</span>
        <strong>{flow.confirmation}</strong>
        <p>Signature <code>{flow.signature}</code>. The Claims-role Custody replay now exists at <code>{flow.replayAddress}</code> with next revision {flow.nextRevision} — the exact cursor a payout replays against. <Anchor href={`/explorer?view=transaction&q=${encodeURIComponent(flow.signature)}`}>Open your transaction in the explorer →</Anchor></p>
      </div>
      <p className="market-capability-refusal"><span>payout leg</span>{PLAIN_POSITION_PAYOUT_BLOCK_V1}</p>
    </>}
  </div>;
}
