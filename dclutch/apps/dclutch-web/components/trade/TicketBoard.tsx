'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';

import TicketCard from '@/components/trade/TicketCard';
import StepRefusal from '@/components/trade/StepRefusal';
import { type StepRefusalV1 } from '@/lib/tradeFlowRefusals';
import { screenBoardOffersV1, type BoardScreenContextV1 } from '@/lib/tradeFlowBoard';
import {
  configuredTicketBoardV1,
  listConfiguredBoardOffersV1,
  TICKET_BOARD_ABSENT_MESSAGE_V1,
  TICKET_BOARD_HONESTY_LINE_V1,
  TICKET_BOARD_UNREACHABLE_MESSAGE_V1,
  type TicketBoardListingV1,
} from '@/lib/ticketBoard';
import { type DenominationV1 } from '@/lib/quantity';
import { type SlotClockV1 } from '@/lib/slotClock';
import { type TicketState } from '@/lib/tradeFlowMachine';

/**
 * Step 3: who is on the other side.
 *
 * The step's contract is "produce one SignedDirectIntentV3, by any means", and
 * it is transport-blind on purpose. A board, a paste box, a URL fragment, a QR
 * code -- all feed the identical decoder, the identical card, and the identical
 * step 4. That is what makes the board a source plugged into a finished UI
 * rather than a redesign, and it is why the paste box costs nothing to keep.
 *
 * **The paste box is never removed.** It is the only path that works with no
 * relay at all, and it is the standing proof that the board is a convenience
 * rather than an authority. When no board is configured it is not a fallback
 * at all -- it is the step, and it opens by default.
 *
 * An absent board is the SUPPORTED DEFAULT, not a degraded deployment. A plain
 * checkout names no relay, and that reader is told what is true and handed the
 * box that works, never an error.
 */

/**
 * What a ticket looks like, for a reader whose paste just refused.
 *
 * Hand-written rather than lifted from `fixtures/direct-intent-ticket.json`,
 * because the fixture is a real signed vector and the thing a confused reader
 * needs is the SHAPE -- the field names and their types -- not 64 bytes of
 * somebody else's signature to mistake for a template.
 */
const TICKET_SHAPE_EXAMPLE_V1 = `{
  "kind": "dclutch/direct-intent-ticket/v1",
  "maker": "<the maker's base58 address>",
  "signature": "<128 lowercase hex characters>",
  "intent": {
    "side": 0,
    "lifecycle": 1,
    "outcome": 0,
    "market": "<this market's base58 address>",
    "generation": "1",
    "nonce": "1",
    "validFrom": "0",
    "validThrough": "4294967295",
    "maximumFill": "100000000",
    "limitPrice": "500000",
    "feeBasisPoints": 0,
    "collateralAccount": "<the maker's base58 collateral account>"
  }
}`;

/**
 * The board holds the RAW listing and the query it answered, never the
 * screened result.
 *
 * Screening is done during render instead, for two reasons that both bite. The
 * filters depend on the connected wallet and the route context, so screening
 * at fetch time would make the fetch depend on them -- and the context arrives
 * as a fresh object literal every render, which turns "refetch when the
 * context changes" into "refetch forever". It is also just better behaviour:
 * connecting a wallet hides your own offers immediately, with no round trip to
 * a relay that would return the identical bytes.
 */
type BoardStateV1 =
  | Readonly<{ kind: 'absent' }>
  | Readonly<{ kind: 'loading' }>
  | Readonly<{ kind: 'unreachable' }>
  | Readonly<{ kind: 'listed'; listing: TicketBoardListingV1; outcome: number | null }>;

export default function TicketBoard({
  marketAddress,
  outcome,
  outcomeLabel,
  screenContext,
  denomination,
  priceScale,
  clock,
  nowMs,
  ticketText,
  ticketState,
  onTicketText,
  refusal,
}: Readonly<{
  /** The ticket refusal this step owns, routed by the host. */
  refusal: StepRefusalV1 | null;
  marketAddress: string;
  outcome: number | null;
  outcomeLabel: (index: number) => string;
  screenContext: BoardScreenContextV1;
  denomination: DenominationV1;
  priceScale: bigint;
  clock: SlotClockV1 | null;
  nowMs: number | null;
  ticketText: string;
  ticketState: TicketState;
  onTicketText: (next: string) => void;
}>) {
  // The deployment's board is a build-time constant, so it is resolved once.
  // Rebuilding it every render would give `load` a new identity every render,
  // and an effect that depends on `load` would then never stop firing.
  const config = useMemo(() => configuredTicketBoardV1(), []);
  const [board, setBoard] = useState<BoardStateV1>(
    config === null ? { kind: 'absent' } : { kind: 'loading' },
  );

  const load = useCallback(async () => {
    if (config === null) return;
    try {
      const listing = await listConfiguredBoardOffersV1(config, {
        market: marketAddress,
        ...(outcome === null ? {} : { outcome }),
      });
      setBoard({ kind: 'listed', listing, outcome });
    } catch {
      // A relay that does not answer is an availability fact and nothing more.
      // Nothing about this market changed, and the paste box is untouched.
      setBoard({ kind: 'unreachable' });
    }
  }, [config, marketAddress, outcome]);

  // The same shape the rest of this app reads chain data with: the call is
  // deferred out of the effect body and guarded by a cancel flag, so a board
  // that answers after the reader has moved to another claim cannot land on
  // the step they are looking at now.
  useEffect(() => {
    let cancelled = false;
    queueMicrotask(() => {
      if (!cancelled) void load();
    });
    return () => { cancelled = true; };
  }, [load]);

  // Derived during render, never stored: a listing answered for a different
  // claim is not this step's answer, and saying so here costs no state.
  const answered = board.kind === 'listed' && board.outcome === outcome ? board.listing : null;
  const screen = answered === null
    ? null
    : screenBoardOffersV1(answered, { ...screenContext, finalizedSlot: answered.slotBasis });

  return <div className="ticket-board">
    {board.kind === 'absent' && <p className="direct-status">{TICKET_BOARD_ABSENT_MESSAGE_V1}</p>}
    {board.kind !== 'absent' && board.kind !== 'unreachable' && screen === null
      && <p className="direct-status" aria-live="polite">Asking the offer board what is on it…</p>}
    {board.kind === 'unreachable' && <div className="board-empty">
      <p className="direct-status">{TICKET_BOARD_UNREACHABLE_MESSAGE_V1}</p>
      <button type="button" className="secondary-action" onClick={() => { setBoard({ kind: 'loading' }); void load(); }}>Ask the board again</button>
    </div>}

    {screen !== null && answered !== null && <>
      {screen.offers.length === 0
        ? <div className="board-empty">
          <p className="direct-status">{screen.hidden.length === 0
            ? `No one is offering ${outcome === null ? 'anything on this market' : outcomeLabel(outcome)} right now.`
            : `${screen.hidden.length} offers here, none you can take right now.`}</p>
          <p className="direct-status">Nothing in this build authors an offer yet, so a ticket has to reach you from somewhere else. Paste it below.</p>
        </div>
        : <ul className="board-offers">
          {screen.offers.map((offer) => <li key={offer.digest}>
            <TicketCard
              ticket={offer.ticket}
              denomination={denomination}
              priceScale={priceScale}
              outcomeLabel={outcomeLabel}
              clock={clock}
              nowMs={nowMs}
              action={<button type="button" onClick={() => onTicketText(offer.text)}>Take this offer</button>}
            />
          </li>)}
        </ul>}

      {(screen.hidden.length > 0 || screen.droppedExpired > 0 || screen.refusedCount > 0) && <details className="board-hidden">
        <summary>{screen.hidden.length} offers hidden — why?</summary>
        <ul className="market-bindings">
          {screen.hidden.map((drop) => <li key={drop.offer.digest}>
            <span aria-hidden="true">×</span>
            <div><strong>{drop.reason}</strong><small>{drop.detail}</small></div>
          </li>)}
        </ul>
        {screen.droppedExpired > 0 && <p className="direct-status">The board itself swept {screen.droppedExpired} expired offers before answering.</p>}
        {screen.refusedCount > 0 && <p className="direct-status">{screen.refusedCount} entries on the board did not decode as tickets at all and were never shown.</p>}
        {answered.slotBasis !== null && <p className="direct-status">Validity was judged against the board&apos;s own observed slot {answered.slotBasis.toString()}.</p>}
      </details>}
    </>}

    <p className="board-honesty">{TICKET_BOARD_HONESTY_LINE_V1}</p>

    <details className="board-paste" open={config === null}>
      <summary>Paste a ticket instead</summary>
      <p className="direct-status">A trade here is two signed halves: yours and someone else&apos;s. There is no order book — the other half reaches you as a small ticket (dclutch/direct-intent-ticket/v1), passed along any way you like.</p>
      <label><span>Ticket JSON</span><textarea rows={5} spellCheck={false} value={ticketText} onChange={(event) => onTicketText(event.target.value)} /></label>
      {refusal !== null && <>
        <StepRefusal refusal={refusal} />
        <details className="ticket-shape">
          <summary>What a ticket looks like</summary>
          <pre className="trade-v3-bytes">{TICKET_SHAPE_EXAMPLE_V1}</pre>
        </details>
      </>}
    </details>

    {ticketState.kind === 'ready' && <TicketCard
      ticket={ticketState.ticket}
      denomination={denomination}
      priceScale={priceScale}
      outcomeLabel={outcomeLabel}
      clock={clock}
      nowMs={nowMs}
    />}
  </div>;
}
