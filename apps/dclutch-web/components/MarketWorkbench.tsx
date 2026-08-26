'use client';

import Link from 'next/link';
import { FormEvent, useMemo, useState } from 'react';

import {
  OPERATOR_ROLES,
  acquireOperatorSurfaceV1,
  type OperatorCoordinatesV1,
  type OperatorSurfaceSnapshotV1,
} from '@/lib/operatorSurface';
import {
  capabilityActionsForStageV1,
  evaluateCapabilityV1,
  type CapabilityStage,
} from '@/lib/capabilityModel';
import { SolanaRpcClient } from '@/lib/rpc';

type Stage = Readonly<{
  id: CapabilityStage;
  number: string;
  title: string;
  summary: string;
}>;

const STAGES: ReadonlyArray<Stage> = Object.freeze([
  { id: 'author', number: '01', title: 'Author & fund', summary: 'Compile exact Product bytes, authenticate releases, prepay physical creation, and found the Market.' },
  { id: 'trade', number: '02', title: 'Trade & provide liquidity', summary: 'Construct intent, candidate, Series, and inventory routes only from current state and accepted account frames.' },
  { id: 'resolve', number: '03', title: 'Resolve & settle', summary: 'Bind real provider evidence, execute failure/recovery policy, and stream conservative physical effects.' },
  { id: 'claim', number: '04', title: 'Claim & close', summary: 'Move exact liabilities between representations, redeem resolved value, then retire every quiescent child and root.' },
]);

type WorkbenchState = Readonly<{ kind: 'idle' | 'loading' | 'error'; message: string }> | Readonly<{ kind: 'ready'; snapshot: OperatorSurfaceSnapshotV1 }>;

function reason(error: unknown): string { return error instanceof Error ? error.message : 'chain acquisition refused without a usable reason'; }
function compact(value: string): string { return value.length > 22 ? `${value.slice(0, 8)}…${value.slice(-7)}` : value; }

export default function MarketWorkbench({ initialStage = 'author' }: Readonly<{ initialStage?: CapabilityStage }>) {
  const [stageId, setStageId] = useState<CapabilityStage>(initialStage);
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [coordinates, setCoordinates] = useState<Record<string, string>>(() => Object.fromEntries([...OPERATOR_ROLES, 'realm', 'market'].map((role) => [role, ''])));
  const [state, setState] = useState<WorkbenchState>({ kind: 'idle', message: 'No chain authority has been selected.' });
  const stage = STAGES.find((candidate) => candidate.id === stageId) ?? STAGES[0];
  const actions = useMemo(() => capabilityActionsForStageV1(stage.id), [stage]);

  function update(role: string, value: string) { setCoordinates((current) => ({ ...current, [role]: value.trim() })); }

  async function acquire(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setState({ kind: 'loading', message: 'Reacquiring executable roles and exact Market ownership from finalized local RPC…' });
    try {
      const snapshot = await acquireOperatorSurfaceV1(new SolanaRpcClient(endpoint), coordinates as OperatorCoordinatesV1);
      setState({ kind: 'ready', snapshot });
    } catch (error) { setState({ kind: 'error', message: `Refused: ${reason(error)}` }); }
  }

  const snapshot = state.kind === 'ready' ? state.snapshot : null;
  return <main className="product-shell workbench-shell">
    <header className="product-nav"><Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link><nav><Link href="/markets">Markets</Link><Link className={stageId === 'author' ? 'active' : ''} href="/create">Create</Link><Link className={stageId === 'trade' ? 'active' : ''} href="/liquidity">Trade</Link><Link className={stageId === 'resolve' ? 'active' : ''} href="/resolution">Resolve</Link><Link className={stageId === 'claim' ? 'active' : ''} href="/redeem">Redeem</Link><Link href="/operate">Operate</Link></nav><span className="preview-control"><i className="preview-dot" />chain workbench</span></header>
    <section className="workbench-heading"><div><p className="eyebrow">Market lifecycle workbench</p><h1>From exact terms<br />to terminal claims.</h1></div><p>No sample market, price, pool, provider, balance, or wallet authority appears here. Observe six program roles plus an optional Realm and Market; each concrete workspace must still prove its exact Registry, artifact, and Loader joins.</p></section>
    <nav className="workbench-stages" aria-label="Market lifecycle stages">{STAGES.map((candidate) => <button type="button" className={candidate.id === stageId ? 'active' : ''} onClick={() => setStageId(candidate.id)} key={candidate.id}><span>{candidate.number}</span><strong>{candidate.title}</strong><small>{candidate.summary}</small></button>)}</nav>

    <div className="workbench-grid"><form className="workbench-coordinates" onSubmit={acquire}><header><span>Chain observation</span><h2>Reacquire one execution surface</h2><p>These addresses are transport, not a checked release manifest. Realm and Market ownership must match selected Core.</p></header><label><span>Finalized RPC endpoint</span><input value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label><div>{OPERATOR_ROLES.map((role) => <label key={role}><span>{role}</span><input required value={coordinates[role]} onChange={(event) => update(role, event.target.value)} /></label>)}</div><label><span>Realm (optional)</span><input value={coordinates.realm} onChange={(event) => update('realm', event.target.value)} /></label><label><span>Market (optional during authoring)</span><input value={coordinates.market} onChange={(event) => update('market', event.target.value)} /></label><button type="submit" disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading finalized state…' : 'Observe this chain surface'}</button><p className="direct-status" aria-live="polite">{state.kind === 'ready' ? `Observed at slot ${state.snapshot.observedSlot}${state.snapshot.market ? ` · ${compact(state.snapshot.market.address)} · ${state.snapshot.market.dataBytes} bytes` : ' · no Market selected'}` : state.message}</p>{state.kind === 'ready' && <dl className="workbench-authority"><div><dt>Programs</dt><dd>{state.snapshot.roles.length} executable</dd></div><div><dt>Realm</dt><dd>{state.snapshot.realm?.header ?? (state.snapshot.realm ? 'Core-owned / unclassified' : 'not selected')}</dd></div><div><dt>Market</dt><dd>{state.snapshot.market?.header ?? (state.snapshot.market ? 'Core-owned / unclassified' : 'not selected')}</dd></div><div><dt>Release</dt><dd>unrecognized until route preflight</dd></div></dl>}</form>

      <section className="workbench-actions"><header><span>{stage.number} · current stage</span><h2>{stage.title}</h2><p>{stage.summary}</p></header><div>{actions.map((action) => {
        const verdict = evaluateCapabilityV1(action, snapshot);
        const accepted = verdict.status === 'ready-to-preflight' && action.workspace !== null;
        return <article className={accepted ? 'ready' : ''} key={action.id}><div><span className={`operator-status ${verdict.status}`}>{verdict.status.replaceAll('-', ' ')}</span><h3>{action.action}</h3></div><p>{verdict.reason}</p>{accepted && action.workspace ? <Link href={action.workspace}>Open exact preflight →</Link> : verdict.status === 'rust-only' && action.workspace ? <Link href={action.workspace}>Inspect current boundary →</Link> : <button type="button" disabled>Transaction unavailable</button>}</article>;
      })}</div><footer><strong>Transaction handoff</strong><span>Every accepted builder emits unsigned bytes. Inspect dependencies and download the exact packet in the <Link href="/operate">operator console →</Link></span></footer></section></div>
  </main>;
}
