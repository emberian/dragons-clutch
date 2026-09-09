'use client';

import { useState } from 'react';

import StepRefusal from '@/components/trade/StepRefusal';
import WalletDirectory, { type WalletDirectoryHandleV1 } from '@/components/WalletDirectory';
import { type StepRefusalV1 } from '@/lib/tradeFlowRefusals';
import { base64, type WalletPreparationState } from '@/lib/tradeFlowMachine';

/**
 * Step 6: two signatures, and neither one sends.
 *
 * The spec refuses to collapse these two acts and so does this component,
 * because they are genuinely different things happening to genuinely different
 * bytes. Signature A is a detached Ed25519 message -- your half of the trade --
 * and what it produces is a TICKET: portable, yours, and worth something even
 * if you walk away right here. Signature B is the v0 packet carrying both
 * halves, and what it produces is a transaction that is still, at that moment,
 * sitting in this browser having been sent nowhere.
 *
 * A single "sign" button spanning both would be shorter and would lie about
 * what the reader is agreeing to twice. The two-row progress below is the
 * whole point: it says what you have after each one.
 *
 * The resumption promise lives in this step's header rather than at the top of
 * the panel. It used to sit above everything as the second of three
 * undifferentiated status paragraphs -- told to a reader who did not yet know
 * what signing or sending were, and so told to nobody. Here it is one step away
 * from being true.
 */

/** Has signature A happened? Every state past preparation implies it has. */
function intentSignedV1(state: WalletPreparationState): boolean {
  return state.kind === 'payer-wallet-required' || state.kind === 'wallet-preparable'
    || state.kind === 'wallet-signed' || state.kind === 'submitted' || state.kind === 'executed';
}

/** Has signature B happened? Only these three states carry a signed packet. */
function packetSignedV1(state: WalletPreparationState): boolean {
  return state.kind === 'wallet-signed' || state.kind === 'submitted' || state.kind === 'executed';
}

export default function SignStep({
  walletPreparation,
  previewReady,
  routeText,
  publishedRoute,
  onRouteText,
  onPrepare,
  onSignPacket,
  wallets,
  onWalletConnected,
  refusal,
}: Readonly<{
  walletPreparation: WalletPreparationState;
  previewReady: boolean;
  routeText: string;
  publishedRoute: string | null;
  onRouteText: (next: string) => void;
  onPrepare: () => void;
  onSignPacket: () => void;
  wallets: WalletDirectoryHandleV1;
  onWalletConnected: (address: string) => void;
  /**
   * The refusal this step OWNS, routed by the host. Not every refusal the
   * preparation raised belongs here -- a buy-side ticket refuses during
   * preparation but is step 3's problem, and showing it at step 6 would tell
   * a reader to fix the route when the route is fine.
   */
  refusal: StepRefusalV1 | null;
}>) {
  const usingPublished = publishedRoute !== null && routeText === publishedRoute;
  const [editingRoute, setEditingRoute] = useState(false);
  const showRouteInput = !usingPublished || editingRoute;
  const intentDone = intentSignedV1(walletPreparation);
  const packetDone = packetSignedV1(walletPreparation);

  return <>
    <p className="direct-status">Review and sign the transaction. You will submit it in the next step.</p>

    {usingPublished
      ? <p className="direct-status">Using the operator&apos;s published route for this market. <button type="button" className="secondary-action" onClick={() => setEditingRoute(true)}>change</button></p>
      : null}
    {showRouteInput && <>
      <p className="direct-status">Paste the route file the operator published for this market (a <code>dclutch-direct-hot-route-manifest-v3</code>).</p>
      <label><span>Checked Direct Hot route manifest · JSON</span><textarea rows={7} spellCheck={false} value={routeText} onChange={(event) => onRouteText(event.target.value)} /></label>
    </>}

    <ol className="signature-rows">
      <li className={intentDone ? 'signature-done' : 'signature-open'}>
        <span aria-hidden="true">A</span>
        <div>
          <strong>Your intent</strong>
          <small>Sign the message containing your trade terms to create a portable offer.</small>
          {intentDone && <p className="signature-standing">Offer signed. Ready to prepare the transaction.</p>}
        </div>
      </li>
      <li className={packetDone ? 'signature-done' : 'signature-open'}>
        <span aria-hidden="true">B</span>
        <div>
          <strong>The transaction</strong>
          <small>Use your wallet’s &ldquo;sign transaction&rdquo; action to approve the matched trade.</small>
          {packetDone && <p className="signature-standing">Transaction signed. Ready to submit.</p>}
        </div>
      </li>
    </ol>

    {walletPreparation.kind === 'working' && <p className="direct-status" aria-live="polite">{walletPreparation.message}</p>}
    {refusal !== null && <StepRefusal refusal={refusal} />}

    {!intentDone && <div className="direct-actions">
      <button
        type="button"
        disabled={!previewReady || walletPreparation.kind === 'working'}
        onClick={onPrepare}
      >Sign offer and prepare trade</button>
    </div>}

    {(walletPreparation.kind === 'wallet-preparable' || walletPreparation.kind === 'payer-wallet-required') && <div className="portfolio-claim">
      <span>{walletPreparation.kind === 'payer-wallet-required' ? 'Buyer intent signed · route payer still needed' : 'Ready for transaction signature'}</span>
      <strong>{walletPreparation.preparation.transactionPlan.wireBytes.length} bytes · {walletPreparation.preparation.transactionPlan.loadedAddresses} LUT addresses · 61 unique keys</strong>
      <p>Route slot {walletPreparation.preparation.binding.routeObservedSlot}; blockhash slot {walletPreparation.preparation.binding.blockhashObservedSlot.toString()}; expires at block height {walletPreparation.preparation.binding.lastValidBlockHeight.toString()}. Frozen table {walletPreparation.preparation.transactionPlan.transaction.message.addressTableLookups[0]?.accountKey.toBase58()}.</p>
      {walletPreparation.kind === 'payer-wallet-required' && <>
        <p>{walletPreparation.preparation.reason} Buyer {walletPreparation.preparation.binding.taker.owner} has signed the offer. Connect the payer to sign the transaction.</p>
        <label><span>Portable signed buyer ticket</span><textarea readOnly rows={6} value={walletPreparation.takerTicket} /></label>
        <WalletDirectory directory={wallets} onConnected={onWalletConnected} purpose={`connect route payer ${walletPreparation.preparation.payer} for the transaction signature`} />
      </>}
      <div className="direct-actions"><button type="button" disabled={wallets.address !== walletPreparation.preparation.payer} onClick={onSignPacket}>{walletPreparation.kind === 'payer-wallet-required' ? 'Sign as the route payer' : 'Sign this packet'}</button></div>
      <p className="direct-status">After signing, continue to Submit to send the transaction.</p>
      <details className="trade-v3-bytes">
        <summary>Transaction data</summary>
        <label><span>Unsigned transaction message · base64</span><textarea readOnly rows={5} value={base64(walletPreparation.preparation.transactionPlan.transaction.message.serialize())} /></label>
      </details>
    </div>}
  </>;
}
