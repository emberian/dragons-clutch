import {
  claimPriceGlossV1,
  denominationUnitV1,
  exactTwinV1,
  formatClaimPriceV1,
  formatQuantityV1,
  type DenominationV1,
} from '@/lib/quantity';
import { type DirectParticipantCrossingAdmissionV1 } from '@/lib/directParticipant';
import { type DirectCrossingPlanV1 } from '@dclutch/sdk/directTicket';

/**
 * What exactly happens, as a receipt in sentence order.
 *
 * Four equal tiles are a good rendering of four equal facts, and these four
 * were never equal: what you buy, what it costs, what you end up holding, and
 * the evidence it was checked against are a sentence, and a grid asks a reader
 * to assemble that sentence themselves every time. So the decision reads as
 * prose and the evidence keeps its own line, mono and exact, at the bottom.
 *
 * **Nothing is lost.** The four tiles move into the drawer as the exact twin,
 * in raw atoms, which is the mechanism by which the units policy costs nothing:
 * humanized above, exact within one click, always both.
 *
 * The arithmetic is the protocol's, not this component's. `buyerCollateralDebit`
 * already includes `buyerFee` and `sellerNetCollateralCredit` is already net of
 * `sellerFee` -- so the receipt names those two fields rather than adding
 * numbers together and hoping the sum matches what the chain will do.
 */
export default function PreviewReceipt({
  plan,
  admission,
  replaySlot,
  denomination,
  priceScale,
  feeBasisPoints,
  outcomeLabel,
}: Readonly<{
  plan: DirectCrossingPlanV1;
  admission: DirectParticipantCrossingAdmissionV1;
  replaySlot: string;
  denomination: DenominationV1;
  priceScale: bigint;
  feeBasisPoints: number;
  outcomeLabel: (index: number) => string;
}>) {
  const unit = denominationUnitV1(denomination);
  const buying = plan.takerSide === 'buy';
  const fill = formatQuantityV1(plan.fill, denomination);
  const price = formatClaimPriceV1(plan.executionPrice, priceScale);
  const gross = formatQuantityV1(plan.preview.grossCollateral, denomination);
  const fee = formatQuantityV1(buying ? plan.preview.buyerFee : plan.preview.sellerFee, denomination);
  const settled = formatQuantityV1(
    buying ? plan.preview.buyerCollateralDebit : plan.preview.sellerNetCollateralCredit,
    denomination,
  );
  // One claim pays one unit of collateral if its outcome wins -- the same
  // scale, which is the fully-backed invariant showing through the units.
  const payout = formatQuantityV1(plan.fill, denomination);
  return <div className="preview-receipt">
    <p className="receipt-lead">
      You {buying ? 'buy' : 'sell'} <strong title={fill.title}>{fill.display}</strong> claims · <strong>{outcomeLabel(plan.taker.outcome)}</strong> — at <strong title={price.title}>{price.display}</strong> per claim
    </p>
    <p className="receipt-line">
      You {buying ? 'pay' : 'receive'} <strong title={settled.title}>{settled.display} {unit}</strong> — {gross.display} principal {buying ? 'plus' : 'less'} {fee.display} fee ({feeBasisPoints} bps)
    </p>
    <p className="receipt-line">{buying
      ? <>You will hold <strong>{fill.display}</strong> more claims. If this outcome wins, they pay <strong>{payout.display} {unit}</strong>.</>
      : <>You will hold <strong>{fill.display}</strong> fewer claims, and give up the <strong>{payout.display} {unit}</strong> they would have paid if this outcome won.</>}
    </p>
    <p className="receipt-evidence">Checked against your assets: {admission.requiredAtoms.toString()} required / {admission.availableAtoms.toString()} available, {admission.resource}, finalized through slot {replaySlot}.</p>
    <p className="direct-status">{claimPriceGlossV1(price, denomination)}</p>
    <details className="trade-v3-bytes">
      <summary>Exact numbers, in raw atoms</summary>
      <div className="trade-v3-evidence">
        <article><span>You {plan.takerSide}</span><strong title={fill.title}>{fill.display} claims</strong><small>{outcomeLabel(plan.taker.outcome)} at {price.display} each · {exactTwinV1(fill, 'claim')} at signed price {plan.executionPrice.toString()}</small></article>
        <article><span>Gross collateral</span><strong title={gross.title}>{gross.display} {unit}</strong><small>{exactTwinV1(gross, 'collateral')} · price scale {priceScale.toString()}</small></article>
        <article><span>Your fee</span><strong title={fee.title}>{fee.display} {unit}</strong><small>{exactTwinV1(fee, 'collateral')} · {feeBasisPoints} bps, rounded at the protocol boundary</small></article>
        <article><span>Asset check</span><strong>{admission.requiredAtoms.toString()} / {admission.availableAtoms.toString()}</strong><small>{admission.resource}, finalized through slot {replaySlot}</small></article>
      </div>
      <p className="direct-status">{plan.note}</p>
    </details>
    <p className="direct-status">Unsigned preview. Nothing is signed until you continue below.</p>
  </div>;
}
