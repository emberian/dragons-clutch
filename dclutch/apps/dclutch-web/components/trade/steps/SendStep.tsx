'use client';

import Anchor from '@/components/Anchor';
import StepRefusal from '@/components/trade/StepRefusal';
import { describeClaimChangeV1 } from '@/lib/directTradeJournal';
import { type StepRefusalV1 } from '@/lib/tradeFlowRefusals';
import { type WalletPreparationState } from '@/lib/tradeFlowMachine';

/**
 * Step 7: once, and only once.
 *
 * Sending is its own act with its own button, and the button only exists in
 * the one state where pressing it means anything. Every other state here shows
 * no control at all, because no control would help: a submitted packet is
 * already in flight and cannot be helped by a second press, and offering one
 * would be inviting the exact double-send the journal underneath exists to
 * make impossible.
 *
 * **`operator-required` is a first-class outcome, not an error.** The trader
 * did everything right; the route's payer is somebody else. It gets the same
 * visual weight as `executed`, because it is a finished piece of work that
 * produced a real, portable, signed artifact -- and rendering it as a failure
 * would tell a reader they had wasted their signature when they are holding
 * the thing the whole flow was for.
 */
export default function SendStep({
  walletPreparation,
  onSubmit,
  refusal,
}: Readonly<{
  walletPreparation: WalletPreparationState;
  onSubmit: () => void;
  /** The refusal this step owns, routed by the host. */
  refusal: StepRefusalV1 | null;
}>) {
  return <>
    {refusal !== null && <StepRefusal refusal={refusal} />}

    {walletPreparation.kind === 'operator-required' && <div className="portfolio-claim flow-terminal">
      <span>Your intent is signed. Nothing has executed.</span>
      <strong>Route payer {walletPreparation.payer}</strong>
      <p>{walletPreparation.reason} The authenticated route was observed at slot {walletPreparation.routeObservedSlot}; its blockhash expires at block height {walletPreparation.lastValidBlockHeight}. Give the exact signed taker ticket below to that payer. This page has not built, signed, or submitted a transaction.</p>
      <label><span>Your signed taker ticket</span><textarea readOnly rows={7} value={walletPreparation.takerTicket} /></label>
    </div>}

    {walletPreparation.kind === 'wallet-signed' && <div className="portfolio-claim">
      <span>Wallet signed · saved locally, not yet submitted</span>
      <strong>{walletPreparation.signature}</strong>
      <p>{walletPreparation.wireBytes} bytes. Route slot {walletPreparation.routeObservedSlot}; blockhash slot {walletPreparation.blockhashObservedSlot}; expires at block height {walletPreparation.lastValidBlockHeight}. Frozen table {walletPreparation.lookupTable}. The exact packet is saved in this browser; nothing has been sent to RPC.</p>
      <div className="direct-actions"><button type="button" onClick={onSubmit}>Send it</button></div>
      <p className="direct-status">Sending submits this one saved packet once, then reads your Position back at finalized commitment. If this page closes mid-flight, reloading resumes the saved signature and never sends a second packet.</p>
      <details className="trade-v3-bytes">
        <summary>The exact bytes that will be sent</summary>
        <label><span>Exact signed packet · base64</span><textarea readOnly rows={6} value={walletPreparation.signedWireBase64} /></label>
        <label><span>Exact v0 message · base64</span><textarea readOnly rows={5} value={walletPreparation.messageBase64} /></label>
      </details>
    </div>}

    {walletPreparation.kind === 'submitted' && <div className="portfolio-claim">
      <span>Submitted · awaiting finalized truth</span>
      <strong>{walletPreparation.signature}</strong>
      <p aria-live="polite">{walletPreparation.confirmation}</p>
    </div>}

    {walletPreparation.kind === 'executed' && <div className="portfolio-claim flow-terminal">
      <span>Executed · finalized</span>
      <strong>{walletPreparation.signature}</strong>
      <p>Finalized, read back at slot {walletPreparation.observedSlot}. Your Position now holds:</p>
      <ul className="market-bindings">
        {walletPreparation.changes === null
          ? walletPreparation.after.positionBalances.map((balance, index) => <li key={index}>claim {index}: {balance.toString()} atoms</li>)
          : walletPreparation.changes.claims.map((change) => <li key={change.claimIndex}>{describeClaimChangeV1(change)}</li>)}
      </ul>
      {walletPreparation.changes !== null && <p>Spendable collateral: {walletPreparation.changes.spendableBefore.toString()} → {walletPreparation.changes.spendableAfter.toString()} atoms.{walletPreparation.changes.moved ? '' : ' Nothing moved — the finalized crossing changed no balance, and that is reported as exactly that.'}</p>}
      <Anchor className="secondary-action" href={`/explorer?view=transaction&q=${encodeURIComponent(walletPreparation.signature)}`}>See it in the explorer →</Anchor>
    </div>}

    <p className="direct-status">The signed packet is saved in this browser before its one send, so a reload picks it up rather than sending twice.</p>
  </>;
}
