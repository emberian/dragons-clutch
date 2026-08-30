'use client';

import Nav from '@/components/Nav';
import { FormEvent, useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';

import WalletDirectory, { useWalletDirectoryV1 } from '@/components/WalletDirectory';
import {
  ACTIVITY_MAX_MARKETS,
  activityHrefV1,
  activityLinkQueryV1,
  inspectActivityV1,
  type ActivityEntryV1,
  type ActivityV1,
} from '@/lib/activity';
import { PUBLIC_DEVNET_CUT_V1 } from '@/lib/publicCutStaging';
import { parseMarketAddressListV1, shortAddressV1 } from '@/lib/marketDiscovery';
import { parsePortfolioOwnerV1 } from '@/lib/portfolio';
import { SolanaRpcClient, type ConnectionFacts } from '@/lib/rpc';
import { useDeploymentFieldV1 } from '@/lib/deploymentStore';
import { clusterNameV1 } from '@/lib/rpcDefault';

type State =
  | Readonly<{ kind: 'idle' | 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; message: string; activity: ActivityV1; facts: ConnectionFacts; href: string }>;

function subscribeToLocation(onChange: () => void): () => void {
  window.addEventListener('popstate', onChange);
  return () => window.removeEventListener('popstate', onChange);
}

function readSearch(): string {
  return window.location.search;
}

function readServerSearch(): null {
  return null;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'the activity read refused without a usable reason';
}

function whenText(entry: ActivityEntryV1): string {
  if (entry.blockTime === null) return `slot ${entry.slot}`;
  const time = new Date(Number(entry.blockTime) * 1000);
  return `slot ${entry.slot} · ${time.toISOString().replace('T', ' ').slice(0, 19)} UTC (node-reported)`;
}

function ActivityRow({ entry }: Readonly<{ entry: ActivityEntryV1 }>) {
  return <article className={`portfolio-entry activity-row${entry.succeeded ? '' : ' refused'}`}>
    <div className="market-card-top">
      <code title={entry.signature}>{shortAddressV1(entry.signature, 10)}</code>
      <span className={`phase-chip${entry.succeeded ? ' phase-open' : ''}`}>{entry.succeeded ? 'succeeded' : 'failed'}</span>
      <small>{whenText(entry)}</small>
    </div>
    {entry.errorText !== null && <p className="market-refusal">Chain error: {entry.errorText}</p>}
    <dl className="market-card-facts">
      <div><dt>Touched because of</dt><dd>{entry.watchedAddresses.map((watched) => watched.meaning).join(' · ')}</dd></div>
      <div><dt>Programs invoked</dt><dd>{entry.programs.length === 0
        ? 'not decoded'
        : entry.programs.map((program) => program.label ?? shortAddressV1(program.address, 6)).join(' · ')}</dd></div>
      {entry.feeLamports !== null && <div><dt>Fee</dt><dd>{entry.feeLamports} lamports</dd></div>}
      {entry.ownerLamportDelta !== null && <div><dt>Owner lamport delta</dt><dd>{entry.ownerLamportDelta}</dd></div>}
    </dl>
    {entry.detail.status === 'refused' && <p className="market-capability-refusal"><span>detail refused</span>{entry.detail.reason}</p>}
  </article>;
}

export default function ActivityWorkspace() {
  const directory = useWalletDirectoryV1();
  const [endpoint, setEndpoint] = useDeploymentFieldV1((d) => d.endpoint);
  const [claimsProgram, setClaimsProgram] = useDeploymentFieldV1((d) => d.programs.claims);
  const [coreProgram, setCoreProgram] = useDeploymentFieldV1((d) => d.programs.core);
  const [tradingProgram, setTradingProgram] = useDeploymentFieldV1((d) => d.programs.trading);
  const search = useSyncExternalStore<string | null>(subscribeToLocation, readSearch, readServerSearch);
  const linked = useMemo(() => activityLinkQueryV1(search), [search]);
  const [ownerOverride, setOwnerOverride] = useState<string | null>(null);
  const [addressListOverride, setAddressListOverride] = useState<string | null>(null);
  const owner = ownerOverride ?? (linked.kind === 'ready' ? linked.owner : '');
  // A link's own market list wins, then the public cut's market if one is
  // named. Without that last fallback the launch page's "Read activity" call
  // to action lands a reader on a form asking for a Market address they have
  // no way to know -- at exactly the moment we most want them to succeed.
  const addressList = addressListOverride
    ?? (linked.kind === 'ready' ? linked.marketAddresses.join('\n') : (PUBLIC_DEVNET_CUT_V1.market ?? ''));
  const [state, setState] = useState<State>({ kind: 'idle', message: 'No signature history has been read.' });
  const activity = state.kind === 'ready' ? state.activity : null;

  const read = useCallback(async (nextOwner: string, marketAddresses: ReadonlyArray<string>) => {
    setState({ kind: 'loading', message: 'Reading this node’s finalized signature history for the owner and every derived Position address…' });
    try {
      const client = new SolanaRpcClient(endpoint);
      const facts = await client.probe();
      const programLabels: Record<string, string> = {};
      if (coreProgram !== '') programLabels[coreProgram] = 'Core (selected)';
      if (claimsProgram !== '') programLabels[claimsProgram] = 'Claims (selected)';
      if (tradingProgram !== '') programLabels[tradingProgram] = 'Trading (selected)';
      const next = await inspectActivityV1(client, {
        owner: parsePortfolioOwnerV1(nextOwner),
        claimsProgramId: claimsProgram === '' ? null : claimsProgram,
        marketAddresses,
        programLabels,
      });
      const href = activityHrefV1(next.owner, marketAddresses);
      window.history.replaceState(null, '', href);
      setState({ kind: 'ready', activity: next, facts, message: next.reason, href });
    } catch (error) {
      setState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` });
    }
  }, [claimsProgram, coreProgram, endpoint, tradingProgram]);

  const startedRef = useRef<string | null>(null);
  useEffect(() => {
    if (linked.kind === 'refused') {
      queueMicrotask(() => setState({ kind: 'refused', message: `Refused: ${linked.reason}` }));
      return;
    }
    if (linked.kind !== 'ready') return;
    const key = `${endpoint}\0${linked.owner}\0${linked.marketAddresses.join('\0')}`;
    if (startedRef.current === key) return;
    startedRef.current = key;
    let cancelled = false;
    queueMicrotask(() => {
      if (!cancelled) void read(linked.owner, linked.marketAddresses);
    });
    return () => { cancelled = true; };
  }, [endpoint, linked, read]);

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    void read(owner, parseMarketAddressListV1(addressList));
  }

  return <main className="product-shell trade-v3-shell">
    <Nav current="/activity" status="node history · not a protocol index" />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Activity · the node&apos;s history, honestly labeled</p>
        <h1>What this wallet did.<br /><em>As the node remembers it.</em></h1>
        <p>dClutch runs no indexer, so this surface reads the one history that exists without one: the RPC node&apos;s own per-address signature index, for the wallet you name and the Claims Position addresses derived from the Markets you name. Every row is a finalized transaction the node returned, decoded in this browser. A node configured without history honestly answers &quot;nothing&quot; for every address, and that answer is shown as the node&apos;s, never as yours.</p>
      </div>
      <aside>
        <span>Provenance</span>
        <strong>Node signature index</strong>
        <p>Not consensus state and not a protocol fact: two nodes can remember different histories. Amounts and programs below are decoded from the returned finalized bytes; nothing is aggregated or estimated.</p>
      </aside>
    </section>

    <form className="trade-v3-card route-card" onSubmit={submit}>
      <header><span>01</span><div><h2>Owner, Markets, and the node to ask</h2><p>The owner address is watched directly. Naming Markets additionally watches the Claims Position derived for each — the same derivation the portfolio uses — so trades and redemptions that touched the Position but not the wallet still appear.</p></div></header>
      <div className="direct-form-grid">
        <label><span>RPC endpoint</span><input type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label>
        <label><span>Owner address · wallet, pasted, or linked</span><input required value={owner} onChange={(event) => setOwnerOverride(event.target.value.trim())} /></label>
        <label><span>Claims program · required to derive Positions</span><input value={claimsProgram} onChange={(event) => setClaimsProgram(event.target.value.trim())} /></label>
        <label><span>Core program · label only</span><input value={coreProgram} onChange={(event) => setCoreProgram(event.target.value.trim())} /></label>
        <label><span>Trading program · label only</span><input value={tradingProgram} onChange={(event) => setTradingProgram(event.target.value.trim())} /></label>
      </div>
      <WalletDirectory directory={directory} purpose="read one owner identity" onConnected={(address) => setOwnerOverride(address)} />
      <label><span>Market addresses · one per line, up to {ACTIVITY_MAX_MARKETS}</span><textarea rows={4} value={addressList} onChange={(event) => setAddressListOverride(event.target.value)} /></label>
      <div className="direct-actions">
        <button disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading node history…' : 'Read activity'}</button>
      </div>
      <p className="direct-status" aria-live="polite">{state.message}</p>
      {state.kind === 'ready' && <div className="direct-actions"><a className="secondary-action" href={state.href}>Open this live view →</a><span className="direct-status">This link carries public addresses only. Opening it re-reads finalized node history; it does not preserve a snapshot.</span></div>}
    </form>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>Finalized transactions, newest first</h2><p>Each row names why it appears (which watched address it touched), the programs it invoked, and the owner&apos;s exact lamport movement. Claim-atom movements live on the portfolio surface, where the Position is decoded in full.</p></div></header>
      {activity === null && <p className="market-empty">No history has been read. Until an owner is named and the node answers, this surface stays empty rather than inventing an activity feed.</p>}
      {activity !== null && state.kind === 'ready' && <>
        <div className="trade-v3-evidence">
          <article><span>Owner</span><strong>{shortAddressV1(activity.owner, 6)}</strong><small>identity only; nothing is signed here</small></article>
          <article><span>Watched addresses</span><strong>{activity.watched.length}</strong><small>wallet + derived Positions</small></article>
          <article><span>Transactions</span><strong>{activity.entries.length}{activity.truncated ? '+' : ''}</strong><small>{activity.truncated ? 'truncated at the explicit browser bound' : 'complete node answer'}</small></article>
          <article><span>Endpoint</span><strong>{state.facts.solanaCore}</strong><small>{clusterNameV1(state.facts.genesisHash)} · genesis {shortAddressV1(state.facts.genesisHash, 6)}</small></article>
        </div>
        {activity.entries.length === 0
          ? <p className="market-empty">{activity.reason}</p>
          : <div className="market-card-grid">{activity.entries.map((entry) => <ActivityRow key={entry.signature} entry={entry} />)}</div>}
      </>}
    </section>

    <footer className="product-footer">
      <span>Node history · finalized bytes · explicit refusals</span>
      <span>No indexer · no synthesized events · raw lamports</span>
    </footer>
  </main>;
}
