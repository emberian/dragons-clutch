'use client';

import { useEffect, useMemo, useRef, useState } from 'react';

import Anchor from '@/components/Anchor';
import {
  observeAquariumLiveMarketV1,
  selectAquariumLiveMarketV1,
  type AquariumLiveObservationV1,
} from '@/lib/aquariumLiveChain';
import { type AquariumStatusV1 } from '@/lib/aquariumStatus';
import { checkedReleaseSetIdsV1 } from '@dclutch/sdk/publicCutStaging';
import { marketDetailHrefV1 } from '@dclutch/sdk/marketHref';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';
import { DEVNET_DEPLOYMENT_V1 } from '@dclutch/sdk/deployments';

/** Public-RPC work stays intentionally small: one Market and eight signatures every 20 seconds. */
export const AQUARIUM_LIVE_REFRESH_MS_V1 = 20_000;

export type AquariumLivePanelStateV1 =
  | Readonly<{ kind: 'unavailable' | 'error'; reason: string; observation: AquariumLiveObservationV1 | null }>
  | Readonly<{ kind: 'loading' | 'ready' | 'stale'; reason: string | null; observation: AquariumLiveObservationV1 | null }>;

/** A hidden tab and an in-flight read both suppress another public-RPC call. */
export function aquariumLiveRefreshAllowedV1(visible: boolean, inFlight: boolean): boolean {
  return visible && !inFlight;
}

/** Never show facts for address A while the snapshot has selected address B. */
export function aquariumLiveShownStateV1(
  selection: ReturnType<typeof selectAquariumLiveMarketV1>,
  state: AquariumLivePanelStateV1,
): AquariumLivePanelStateV1 {
  if (selection.kind === 'unavailable') return Object.freeze({ kind: 'unavailable', reason: selection.reason, observation: null });
  if (state.observation !== null && state.observation.address !== selection.address) {
    return Object.freeze({ kind: 'loading', reason: null, observation: null });
  }
  return state;
}

function short(value: string): string {
  return `${value.slice(0, 10)}…${value.slice(-8)}`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'the live read refused without a usable reason';
}

/**
 * A fresh, bounded public-chain read beside the aquarium's static report.
 * It observes a Market and node history; it never claims that the worker is
 * still running.
 */
export default function AquariumLiveChainPanel({ status }: Readonly<{ status: AquariumStatusV1 | null }>) {
  const selection = useMemo(() => selectAquariumLiveMarketV1(status, checkedReleaseSetIdsV1()), [status]);
  const [state, setState] = useState<AquariumLivePanelStateV1>(() => selection.kind === 'unavailable'
    ? Object.freeze({ kind: 'unavailable', reason: selection.reason, observation: null })
    : Object.freeze({ kind: 'loading', reason: null, observation: null }));
  const lastKnown = useRef<AquariumLiveObservationV1 | null>(null);

  useEffect(() => {
    if (selection.kind === 'unavailable') {
      lastKnown.current = null;
      return undefined;
    }
    // A newly selected snapshot address must never inherit facts from the
    // previous selected Market while its own first read is in flight.
    lastKnown.current = null;
    let disposed = false;
    let inFlight = false;
    let controller: AbortController | null = null;
    const client = new SolanaRpcClient(DEVNET_DEPLOYMENT_V1.endpoint);

    const refresh = async (): Promise<void> => {
      if (disposed || !aquariumLiveRefreshAllowedV1(document.visibilityState === 'visible', inFlight)) return;
      inFlight = true;
      controller = new AbortController();
      const prior = lastKnown.current;
      setState({ kind: 'loading', reason: null, observation: prior });
      try {
        const observation = await observeAquariumLiveMarketV1(client, selection, { signal: controller.signal });
        if (!disposed && !controller.signal.aborted) {
          lastKnown.current = observation;
          setState({ kind: 'ready', reason: null, observation });
        }
      } catch (error) {
        if (!disposed && !controller.signal.aborted) {
          const reason = errorMessage(error);
          setState(prior === null
            ? { kind: 'error', reason, observation: null }
            : { kind: 'stale', reason, observation: prior });
        }
      } finally {
        inFlight = false;
        controller = null;
      }
    };
    const onVisibility = (): void => {
      if (document.visibilityState === 'visible') void refresh();
      // The SDK transport has no signal hook; abort only prevents its late
      // result from entering React state while this page is hidden.
      else controller?.abort();
    };
    void refresh();
    const timer = window.setInterval(() => { void refresh(); }, AQUARIUM_LIVE_REFRESH_MS_V1);
    document.addEventListener('visibilitychange', onVisibility);
    return () => {
      disposed = true;
      controller?.abort();
      window.clearInterval(timer);
      document.removeEventListener('visibilitychange', onVisibility);
    };
  }, [selection]);

  // The cohort gate is derived from the incoming snapshot, so render it
  // directly instead of synchronously setting state from this effect.
  const shownState = aquariumLiveShownStateV1(selection, state);
  const observation = shownState.observation;
  const decoded = observation?.detail.card.status === 'decoded' ? observation.detail.card : null;
  return <section className="trade-v3-card">
    <header><span>02</span><div><h2>Live devnet market read</h2><p>One snapshot-listed market, read at finalized commitment and checked against its release set. Recent signatures are the node&apos;s index; they are not all trades, and this read cannot show whether the aquarium worker is running.</p></div></header>
    {shownState.kind === 'unavailable' && <p className="market-empty">{shownState.reason}</p>}
    {shownState.kind === 'error' && <p className="market-refusal">Live read unavailable: {shownState.reason}</p>}
    {observation !== null && decoded !== null && <>
      <div className="trade-v3-evidence">
        <article><span>Snapshot-listed market</span><strong><Anchor href={marketDetailHrefV1(observation.address)}>{short(observation.address)}</Anchor></strong><small>open its account detail and check joining there</small></article>
        <article><span>Finalized observation</span><strong>slot {observation.detail.floorSlot}</strong><small>read {observation.observedAt}</small></article>
        <article><span>Market phase now</span><strong>{decoded.phase}</strong><small>{shownState.kind === 'loading' ? 'refreshing this bounded read' : shownState.kind === 'stale' ? 'last known chain facts' : 'current finalized read'}</small></article>
      </div>
      {shownState.kind === 'stale' && <p className="market-refusal">The refresh failed, so these are the last known chain facts: {shownState.reason}</p>}
      <h3 className="detail-subhead">Recent finalized signatures</h3>
      {observation.signatures.length === 0
        ? <p className="market-empty">This node lists no recent finalized signatures for this Market.</p>
        : <div className="viz-table-scroll" tabIndex={0} role="region" aria-label="Recent finalized signatures for the selected Market">
          <table className="holders-table"><thead><tr><th>Signature</th><th>Slot</th><th>Time</th><th>Result</th></tr></thead><tbody>
            {observation.signatures.map((entry) => <tr key={entry.signature}>
              <td><Anchor href={`/explorer?view=transaction&q=${encodeURIComponent(entry.signature)}`} title={entry.signature}>{short(entry.signature)}</Anchor></td>
              <td>{entry.slot}</td><td>{entry.blockTime ?? 'not supplied by this node'}</td><td>{entry.succeeded ? 'succeeded' : 'failed'}</td>
            </tr>)}
          </tbody></table>
        </div>}
      <p className="slot-clock-note">At most eight rows, newest first, from this node&apos;s finalized per-address signature index. A signature can be any transaction that touched this Market.</p>
    </>}
    {shownState.kind === 'loading' && observation === null && <p className="direct-status">Reading one checked Market at finalized commitment…</p>}
  </section>;
}
