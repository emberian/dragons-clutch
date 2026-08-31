import { type SignedDirectIntentV3 } from '@/lib/directInlineV3';
import {
  denominationUnitV1,
  exactTwinV1,
  formatClaimPriceV1,
  formatQuantityV1,
  type DenominationV1,
} from '@/lib/quantity';
import { shortAddressV1 } from '@/lib/marketDiscovery';
import { deadlineMomentPhraseV1, type SlotClockV1 } from '@/lib/slotClock';

/**
 * A ticket, as the thing it is rather than the thing it arrived in.
 *
 * A Direct ticket is twelve signed fields. The JSON is the transport, not the
 * artifact, and a 4096-byte blob in a textarea is a reader being handed the
 * envelope and asked to read the letter through it. On a valid parse the text
 * resolves into this: who is offering, what, at what, for how long, and what
 * it would cost the person reading.
 *
 * **The signature chip says WELL-FORMED and never `verified`.** What the
 * browser checked is shape -- 64 nonzero bytes that decode as lowercase hex.
 * Nothing here checked that the signature is the maker's, because nothing here
 * can: the chain re-derives the signing message and verifies it natively at
 * the Ed25519 program, and it is the only thing in this system that does. A
 * chip claiming otherwise would be borrowing an authority it does not have,
 * which is exactly the move the whole bearer-ticket design is safe without.
 */

/** The maker's signature, as the hex the drawer shows and the codec carries. */
function signatureHexV1(signature: Uint8Array): string {
  let hex = '';
  for (const byte of signature) hex += byte.toString(16).padStart(2, '0');
  return hex;
}

/**
 * What one fill of this ticket costs, at the ticket's own signed price.
 *
 * `fill * limitPrice / priceScale`, in exact BigInt, matching the protocol's
 * own `gross` derivation. This is a READING of the ticket, not a plan: the
 * planner is the thing that decides an admissible fill, and the receipt at
 * step 5 shows its numbers. This one answers "what am I looking at".
 */
export function ticketGrossAtomsV1(
  fillAtoms: bigint,
  limitPrice: bigint,
  priceScale: bigint,
): bigint {
  if (priceScale <= 0n) throw new Error('a ticket price needs one positive price scale to be a share of anything');
  return (fillAtoms * limitPrice) / priceScale;
}

export default function TicketCard({
  ticket,
  denomination,
  priceScale,
  outcomeLabel,
  clock,
  nowMs,
  action,
}: Readonly<{
  ticket: SignedDirectIntentV3;
  denomination: DenominationV1;
  priceScale: bigint;
  /** The registry's name for one outcome, falling back to its chain index. */
  outcomeLabel: (index: number) => string;
  /**
   * A measured slot clock, when one has been read. Absent, validity renders as
   * the exact slot it actually is rather than a wall-clock time nothing
   * measured -- an estimated deadline with no clock behind it is a guess
   * wearing a countdown's clothes.
   */
  clock: SlotClockV1 | null;
  nowMs: number | null;
  /** The board's "take this offer" control, when this card is an offer. */
  action?: React.ReactNode;
}>) {
  const { intent } = ticket;
  const unit = denominationUnitV1(denomination);
  const quantity = formatQuantityV1(intent.maximumFill, denomination);
  const price = formatClaimPriceV1(intent.limitPrice, priceScale);
  const gross = formatQuantityV1(
    ticketGrossAtomsV1(intent.maximumFill, intent.limitPrice, priceScale),
    denomination,
  );
  // side 0 is a maker SELLING, which is the reader BUYING. Stating only the
  // maker's side has been the panel's quiet trap: a reader scanning a list of
  // offers reads the verb nearest their own eye as theirs.
  const makerSells = intent.side === 0;
  const validity = clock === null || nowMs === null
    ? `Valid through slot ${intent.validThrough.toString()}`
    : `Valid ${deadlineMomentPhraseV1(clock, intent.validThrough.toString(), nowMs)} · to slot ${intent.validThrough.toString()}`;
  return <article className="ticket-card">
    <header>
      <strong title={ticket.maker}>{shortAddressV1(ticket.maker, 6)}</strong>
      <span>offers to {makerSells ? 'SELL' : 'BUY'}</span>
      <em className="ticket-chip">well-formed</em>
    </header>
    <p className="ticket-headline">
      {quantity.display} claims · {outcomeLabel(intent.outcome)}
    </p>
    <p className="ticket-terms">
      at {price.display} each · you would {makerSells ? 'pay' : 'receive'} {gross.display} {unit}
    </p>
    <ul className="ticket-facts">
      <li>{intent.lifecycle === 0 ? 'All or nothing' : 'Partial fills allowed'}</li>
      <li>{validity}</li>
      <li>Fee {intent.feeBasisPoints} bps each side</li>
    </ul>
    {action}
    <details className="ticket-fields">
      <summary>The exact signed fields</summary>
      <p className="direct-status">Every field below is covered by the maker&apos;s signature. Changing any one of them changes the signing message, and the chain refuses the trade rather than executing a different one.</p>
      <dl className="detail-facts">
        <div><dt>Maker</dt><dd><code>{ticket.maker}</code></dd></div>
        <div><dt>Market</dt><dd><code>{intent.market}</code></dd></div>
        <div><dt>Collateral account</dt><dd><code>{intent.collateralAccount}</code></dd></div>
        <div><dt>Generation</dt><dd>{intent.generation.toString()}</dd></div>
        <div><dt>Nonce</dt><dd>{intent.nonce.toString()}</dd></div>
        <div><dt>Side</dt><dd>{intent.side} · {makerSells ? 'sell' : 'buy'}</dd></div>
        <div><dt>Lifecycle</dt><dd>{intent.lifecycle} · {intent.lifecycle === 0 ? 'fill-or-kill' : 'immediate-or-cancel'}</dd></div>
        <div><dt>Outcome</dt><dd>{intent.outcome}</dd></div>
        <div><dt>Maximum fill</dt><dd>{exactTwinV1(quantity, 'claim')}</dd></div>
        <div><dt>Limit price</dt><dd>{price.fraction}</dd></div>
        <div><dt>Fee basis points</dt><dd>{intent.feeBasisPoints}</dd></div>
        <div><dt>Valid from</dt><dd>slot {intent.validFrom.toString()}</dd></div>
        <div><dt>Valid through</dt><dd>slot {intent.validThrough.toString()}</dd></div>
        <div><dt>Signature</dt><dd><code>{signatureHexV1(ticket.signature)}</code></dd></div>
      </dl>
      <p className="direct-status">This browser checked the signature&apos;s shape: 64 nonzero bytes, lowercase hex. Only the chain verifies that it is this maker&apos;s signature, and it does that when the trade executes.</p>
    </details>
  </article>;
}
