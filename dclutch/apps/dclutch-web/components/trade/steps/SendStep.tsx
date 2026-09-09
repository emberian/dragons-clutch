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
 * A route with a distinct payer remains in the signing step until that payer
 * signs the exact compiled packet. This step therefore has one job for every
 * route: submit a complete saved packet, once.
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

    {walletPreparation.kind === 'wallet-signed' && <div className="portfolio-claim">
      <span>Wallet signed · saved locally, not yet submitted</span>
      <strong>{walletPreparation.signature}</strong>
      <p>{walletPreparation.wireBytes} bytes. Route slot {walletPreparation.routeObservedSlot}; blockhash slot {walletPreparation.blockhashObservedSlot}; expires at block height {walletPreparation.lastValidBlockHeight}. Frozen table {walletPreparation.lookupTable}. Saved in this browser and ready to submit.</p>
      <div className="direct-actions"><button type="button" onClick={onSubmit}>Send it</button></div>
      <p className="direct-status">Submit the signed transaction, then check your updated claim balances.</p>
      <details className="trade-v3-bytes">
        <summary>Transaction data</summary>
        <label><span>Signed transaction · base64</span><textarea readOnly rows={6} value={walletPreparation.signedWireBase64} /></label>
        <label><span>Transaction message · base64</span><textarea readOnly rows={5} value={walletPreparation.messageBase64} /></label>
      </details>
    </div>}

    {walletPreparation.kind === 'submitted' && <div className="portfolio-claim">
      <span>Submitted · awaiting confirmation</span>
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
      {walletPreparation.changes !== null && <p>Spendable collateral: {walletPreparation.changes.spendableBefore.toString()} → {walletPreparation.changes.spendableAfter.toString()} atoms.{walletPreparation.changes.moved ? '' : ' Balances unchanged.'}</p>}
      <Anchor className="secondary-action" href={`/explorer?view=transaction&q=${encodeURIComponent(walletPreparation.signature)}`}>See it in the explorer →</Anchor>
    </div>}

    <p className="direct-status">You can reload this page to resume checking this transaction.</p>
  </>;
}
