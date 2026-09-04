'use client';

import { useCallback, useEffect, useState } from 'react';

import Sparkline from '@/components/charts/Sparkline';
import { inspectDirectTradeSpineV1, type DirectTradeSpineV1 } from '@/lib/directTradeSpine';
import { formatWindowInstantV1 } from '@/lib/marketQuestion';
import {
  inspectMarketActivityV1,
  MARKET_ACTIVITY_PROVENANCE_V1,
  MARKET_ACTIVITY_TRANSACTIONS_V1,
  type MarketActivityRowV1,
  type MarketActivityV1,
  type MarketFillV1,
} from '@/lib/marketActivity';
import { outageDisclosureV1 } from '@/lib/marketDetail';
import { shortAddressV1 } from '@/lib/marketDiscovery';
import { checkedReleaseSetIdsV1 } from '@/lib/publicCutStaging';
import { denominationUnitV1, formatQuantityV1, type DenominationV1 } from '@/lib/quantity';
import { SolanaRpcClient } from '@/lib/rpc';

/**
 * WHAT HAS HAPPENED ON THIS MARKET, and it is the page's only past tense.
 *
 * Every other surface on the market page answers "what is true now": the phase,
 * the supply vector, the vault, the walls. None of them could say that a
 * crossing happened, who was on each side, what it crossed at, or where those
 * claims went — and on 2026-09-02 one did happen, and the page it happened on
 * had no word for it.
 *
 * Three sections, three provenances, each stated:
 *
 *   * THE CROSSINGS come out of the transactions themselves. Both parties sign
 *     an intent, the intents ride in the instruction, and `lib/marketActivity.ts`
 *     reads them back at the generated Direct V3 coordinates and prices them
 *     with `previewDirectInlineV3` — the same function the stepper previews an
 *     unsent fill with, so what a reader is shown about a crossing that
 *     happened is computed by the rule that admits one that has not.
 *   * WHERE THE CLAIMS SIT is read live from the Position accounts, ordered by
 *     what they hold. This is the leaderboard, and it only became one today:
 *     before a crossing every position holds complete sets and an ordering of
 *     them ranks nothing.
 *   * WHAT THE VENUE IS OWED is the maker replay's own `fee_owed`. A Direct
 *     fill leaves the fee as an obligation and a separate permissionless
 *     transaction settles it, so "the fee was charged" and "the fee was paid"
 *     are two different facts and only the replay knows the second.
 *
 * A SEPARATE READ, deliberately. It needs the Direct config's price scale and
 * fee rate, which is a chain read of its own, and a market whose activity will
 * not load must still render everything else on the page. So this owns its
 * refusal and says it here rather than taking the page down with it.
 */

type State =
  | Readonly<{ kind: 'idle' | 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; message: string; activity: MarketActivityV1; spine: Extract<DirectTradeSpineV1, Readonly<{ status: 'inspected' }>> }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'the activity read refused without a usable reason';
}

function instantV1(blockTime: string | null): string {
  return blockTime === null ? 'no block time' : formatWindowInstantV1(BigInt(blockTime));
}

function shortSignatureV1(signature: string): string {
  return `${signature.slice(0, 8)}…${signature.slice(-6)}`;
}

/** An exact ratio, printed as one. A price here is never a float. */
function priceV1(fill: MarketFillV1): string {
  if (fill.grossPerClaim === null) return '—';
  const { numerator, denominator } = fill.grossPerClaim;
  const exact = BigInt(numerator) % BigInt(denominator) === 0n
    ? (BigInt(numerator) / BigInt(denominator)).toString()
    : null;
  return exact === null ? `${numerator} ÷ ${denominator}` : exact;
}

/** What the census calls this act, or what it says it cannot call it. */
function actV1(row: MarketActivityRowV1): string {
  if (row.fill !== null) return 'Direct crossing · InlineOrdinary';
  if (row.route !== null) return row.route.routeId;
  return 'unnamed';
}

export function CrossingsTable({ fills, denomination, outcomes }: Readonly<{
  fills: ReadonlyArray<MarketFillV1>;
  denomination: DenominationV1 | null;
  outcomes: ReadonlyArray<string> | null;
}>) {
  return <div className="viz-table-scroll" tabIndex={0} role="region" aria-label="Crossings on this market">
    <table className="holders-table">
      <thead><tr>
        <th>When</th><th>Outcome</th><th>Claims</th><th>Price · collateral per claim</th>
        <th>Gross</th><th>Fee · each side</th><th>Seller</th><th>Buyer</th><th>Transaction</th>
      </tr></thead>
      <tbody>
        {fills.map((fill) => {
          const outcome = fill.terms.sellerIntent.outcome;
          const name = outcomes?.[outcome];
          return <tr key={fill.signature}>
            <td>{instantV1(fill.blockTime)}<br /><small>slot {fill.slot}</small></td>
            <td>claim {outcome}{name === undefined ? '' : ` · ${name}`}</td>
            <td>{fill.terms.fillAtoms.toString()}</td>
            <td>{priceV1(fill)}</td>
            <td>{fill.economics === null
              ? <span title={fill.economicsRefusal ?? undefined}>refused</span>
              : denomination === null
                ? fill.economics.grossCollateral.toString()
                : <>{formatQuantityV1(fill.economics.grossCollateral.toString(), denomination).display} {denominationUnitV1(denomination)}<br /><small>{fill.economics.grossCollateral.toString()} atoms</small></>}</td>
            <td>{fill.economics === null ? '—' : <>{fill.economics.sellerFee.toString()}<br /><small>{fill.economics.totalFeeTransfer.toString()} in total</small></>}</td>
            <td title={fill.terms.seller}>{shortAddressV1(fill.terms.seller, 5)}</td>
            <td title={fill.terms.buyer}>{shortAddressV1(fill.terms.buyer, 5)}</td>
            <td><code title={fill.signature}>{shortSignatureV1(fill.signature)}</code></td>
          </tr>;
        })}
      </tbody>
    </table>
  </div>;
}

export default function MarketActivity({ address, endpoint, programs, denomination, outcomes, supplyAtoms }: Readonly<{
  address: string;
  endpoint: string;
  /** The deployment's program ids, from the page's own deployment store. */
  programs: Readonly<{ core: string; registry: string; trading: string; claims: string }>;
  denomination: DenominationV1 | null;
  outcomes: ReadonlyArray<string> | null;
  /** The Claims aggregate's own supply vector, which the page already read. */
  supplyAtoms: ReadonlyArray<string> | null;
}>) {
  const [state, setState] = useState<State>({ kind: 'loading', message: 'Reading what has happened here…' });
  const { core, registry, trading, claims } = programs;

  const read = useCallback(async () => {
    setState({ kind: 'loading', message: 'Reading what has happened here…' });
    try {
      const client = new SolanaRpcClient(endpoint);
      const spine = await inspectDirectTradeSpineV1(client, {
        marketAddress: address,
        coreProgramId: core,
        registryProgramId: registry,
        tradingProgramId: trading,
        claimsProgramId: claims,
        checkedReleaseSetIds: checkedReleaseSetIdsV1(),
      });
      if (spine.status !== 'inspected') {
        setState({ kind: 'refused', message: `This market has no Direct venue for a crossing to happen at: ${spine.reason}` });
        return;
      }
      if (spine.aggregateAddress === null || spine.outcomeCount === null) {
        setState({ kind: 'refused', message: 'The Claims ledger for this market did not read, so its positions cannot be named.' });
        return;
      }
      const activity = await inspectMarketActivityV1(client, {
        marketAddress: address,
        tradingProgramId: trading,
        claimsProgramId: claims,
        aggregateAddress: spine.aggregateAddress,
        generation: BigInt(spine.generation),
        outcomeCount: spine.outcomeCount,
        priceScale: spine.priceScale,
        feeBasisPoints: spine.feeBasisPoints,
      });
      setState({ kind: 'ready', message: activity.reason, activity, spine });
    } catch (error) {
      setState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` });
    }
  }, [address, endpoint, core, registry, trading, claims]);

  // Read on mount and again whenever the market or the cluster changes. The
  // microtask defers it past the page's own first read, which shares this
  // endpoint's in-flight budget.
  useEffect(() => {
    let cancelled = false;
    queueMicrotask(() => {
      if (!cancelled) void read();
    });
    return () => { cancelled = true; };
  }, [read]);

  return <MarketActivityView
    state={state}
    denomination={denomination}
    outcomes={outcomes}
    supplyAtoms={supplyAtoms}
    onReread={() => { void read(); }}
  />;
}

/**
 * The rendering, split out so a test can drive it with a settled read.
 *
 * Exported for exactly that: the arrangement of this section is pinned by a
 * case that hands it a state, not by a screenshot.
 */
export function MarketActivityView({ state, denomination, outcomes, supplyAtoms, onReread }: Readonly<{
  state: State;
  denomination: DenominationV1 | null;
  outcomes: ReadonlyArray<string> | null;
  supplyAtoms: ReadonlyArray<string> | null;
  onReread?: () => void;
}>) {
  const activity = state.kind === 'ready' ? state.activity : null;
  const spine = state.kind === 'ready' ? state.spine : null;
  const fills = activity?.fills ?? [];
  const positions = activity?.positions ?? [];
  // The one thing a buyer cannot take a founder's word for, and it is not
  // written down anywhere: it is read off the supply vector this page already
  // holds and the Position accounts it just read. See `outageDisclosureV1`.
  const outage = supplyAtoms === null
    ? null
    : outageDisclosureV1({ outcomeCount: supplyAtoms.length, supplyAtoms, positions });

  return <section className="trade-v3-card" aria-label="What has happened on this market">
    <header>
      <span>02</span>
      <div>
        <h2>What has happened here</h2>
        <p>Read from this market&apos;s own transactions and accounts. There is no index behind this — the crossings are decoded out of the instructions that carried them.</p>
      </div>
      {onReread !== undefined && <div className="direct-actions">
        <button type="button" onClick={onReread} disabled={state.kind === 'loading'}>
          {state.kind === 'loading' ? 'Reading…' : 'Read it again'}
        </button>
      </div>}
    </header>

    <p className={state.kind === 'refused' ? 'market-refusal' : 'direct-status'} aria-live="polite">{state.message}</p>

    {activity !== null && <>
      {fills.length === 0
        ? <p className="market-empty">
          No crossing is in the {MARKET_ACTIVITY_TRANSACTIONS_V1} most recent transactions this node holds for this market.
          That is the node&apos;s answer over that window, not a claim that this market has never traded.
        </p>
        : <>
          <h3 className="detail-subhead">The crossings</h3>
          <CrossingsTable fills={fills} denomination={denomination} outcomes={outcomes} />
          <p className="slot-clock-note">
            Both sides sign a limit and the crossing happens between them, so gross, both fees and both net legs
            are recomputed here from the signed intents at the venue&apos;s immutable
            scale ({spine === null ? 'unread' : spine.priceScale.toString()}) and
            rate ({spine === null ? 'unread' : spine.feeBasisPoints} basis points a side)
            — by the same function that previews a fill nobody has sent.
          </p>
          {/* FE-CHART mount: the crossing history. ONE point is a point, and it
              is drawn as one: a line needs two, and manufacturing a second from
              the founding would be drawing a crossing that never happened. */}
          <Sparkline
            lines={[
              { label: 'claims crossed', values: fills.map((fill) => fill.terms.fillAtoms.toString()).reverse() },
              { label: 'gross collateral', values: fills.map((fill) => (fill.economics?.grossCollateral ?? 0n).toString()).reverse() },
            ]}
            xLabels={fills.map((fill) => instantV1(fill.blockTime)).reverse()}
            unit="atoms"
            caption="Every crossing this market has taken, oldest first."
            flatNote={fills.length === 1
              ? 'one crossing, drawn as one point — a line needs two, and this market has taken exactly one'
              : undefined}
            emptyReason="No crossing has been read."
          />
        </>}

      <h3 className="detail-subhead">Where the claims sit now</h3>
      {positions.length === 0
        ? <p className="market-empty">No Position on this market appears in the transactions read above.</p>
        : <>
          <div className="viz-table-scroll" tabIndex={0} role="region" aria-label="Positions on this market, largest holding first">
            <table className="holders-table">
              <thead><tr><th>#</th><th>Holder</th><th>Claims held, per outcome · raw u64</th><th>Total claims</th><th>Ledger revision</th></tr></thead>
              <tbody>
                {positions.map((position, index) => <tr key={position.address}>
                  <td>{index + 1}</td>
                  <td title={position.owner}>{shortAddressV1(position.owner, 5)}{position.level ? ' · complete sets only' : ''}</td>
                  <td>{position.balances.join(' · ')}</td>
                  <td>{position.totalClaims}</td>
                  <td>{position.revision}</td>
                </tr>)}
              </tbody>
            </table>
          </div>
          <p>
            Ordered by what each position holds, read live from the Position accounts themselves. A holder
            with the same count on every outcome holds complete sets and has taken no side.
          </p>
        </>}

      {outage !== null && <>
        <h3 className="detail-subhead">If the source never reports</h3>
        <p>{outage.headline}</p>
        <p className={outage.complete ? undefined : 'market-refusal'}>{outage.payee}</p>
        {outage.holders.length > 0 && <div className="viz-table-scroll" tabIndex={0} role="region" aria-label="Who holds this market's failure outcome">
          <table className="holders-table">
            <thead><tr><th>Holder of claim {outage.failureOutcome}</th><th>Claims held · raw u64</th><th>Share of the failure outcome</th></tr></thead>
            <tbody>
              {outage.holders.map((holder) => <tr key={holder.owner}>
                <td title={holder.owner}>{shortAddressV1(holder.owner, 5)}</td>
                <td>{holder.atoms}</td>
                <td>{holder.wholeColumn ? 'all of it' : `${holder.atoms} of ${outage.supplyAtoms}`}</td>
              </tr>)}
            </tbody>
          </table>
        </div>}
        <p className="slot-clock-note">
          Read from the Claims aggregate&apos;s supply vector and the Position accounts above, not from
          this market&apos;s written terms. {outage.complete
            ? `Those Positions account for every one of the ${outage.supplyAtoms} atoms on the failure outcome, so this is the whole answer.`
            : `They account for ${outage.accountedAtoms} of ${outage.supplyAtoms}; the rest sits in Positions this read did not reach.`}
        </p>
      </>}

      {activity.feeStandings.length > 0 && <>
        <h3 className="detail-subhead">What the venue is owed</h3>
        <div className="trade-v3-evidence">
          {activity.feeStandings.map((standing) => <article key={standing.maker}>
            <span title={standing.maker}>{shortAddressV1(standing.maker, 5)}</span>
            <strong>{standing.feeOwed === '0' ? 'settled' : `${standing.feeOwed} owed`}</strong>
            <small title={standing.replayAddress}>from its maker replay {shortAddressV1(standing.replayAddress, 5)} · next nonce {standing.nextNonce}</small>
          </article>)}
        </div>
        <p className="slot-clock-note">
          A Direct fill records the fee as an obligation on the payer&apos;s maker replay; a separate
          permissionless transaction settles it, and anyone may send that transaction. Zero owed here means
          it has been settled, read back from the replay itself.
        </p>
      </>}

      <details className="market-detail-drawer">
        <summary>Everything this node holds for this market</summary>
        <div className="market-detail-drawer-body">
          <div className="viz-table-scroll" tabIndex={0} role="region" aria-label="Transactions this node holds for this market">
            <table className="holders-table">
              <thead><tr><th>When</th><th>Slot</th><th>What it was</th><th>Fee · lamports</th><th>Transaction</th></tr></thead>
              <tbody>
                {activity.rows.map((row) => <tr key={row.signature}>
                  <td>{instantV1(row.blockTime)}</td>
                  <td>{row.slot}</td>
                  {/* Prose, not a value: what a transaction WAS is a sentence,
                      and it gets the sentence treatment rather than the dense
                      mono nowrap every other cell here correctly uses. */}
                  <td className="table-sentence">
                    {actV1(row)}
                    {row.unnamedReason === null ? null : <small>{row.unnamedReason}</small>}
                    {row.succeeded ? null : <small>refused: {row.errorText ?? 'no reason given'}</small>}
                  </td>
                  <td>{row.feeLamports}</td>
                  <td><code title={row.signature}>{shortSignatureV1(row.signature)}</code></td>
                </tr>)}
              </tbody>
            </table>
          </div>
          <p className="slot-clock-note">
            {MARKET_ACTIVITY_PROVENANCE_V1}
            {activity.signaturesNotRead === 0
              ? ''
              : ` ${activity.signaturesNotRead} older signature${activity.signaturesNotRead === 1 ? '' : 's'} the node listed are counted here and not read.`}
            {activity.transactionsRefused === 0
              ? ''
              : ` ${activity.transactionsRefused} the node would not return; each of those rows says so rather than reading as an act that did not happen.`}
            {' '}Read at finalized slot {activity.observedSlot}.
          </p>
        </div>
      </details>
    </>}

  </section>;
}
