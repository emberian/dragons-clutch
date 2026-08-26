'use client';

import Link from 'next/link';
import { FormEvent, useMemo, useState } from 'react';

import {
  OPERATOR_ROLES,
  OPERATOR_WORKFLOWS,
  acquireOperatorSurfaceV1,
  type OperatorCoordinatesV1,
  type OperatorSurfaceSnapshotV1,
  type OperatorWorkflowV1,
} from '@/lib/operatorSurface';
import { SolanaRpcClient } from '@/lib/rpc';

type StageId = 'author' | 'trade' | 'resolve' | 'claim';
type Stage = Readonly<{
  id: StageId;
  number: string;
  title: string;
  summary: string;
  actions: ReadonlyArray<string>;
}>;

const STAGES: ReadonlyArray<Stage> = Object.freeze([
  { id: 'author', number: '01', title: 'Author & fund', summary: 'Compile exact Product bytes, authenticate releases, prepay physical creation, and found the Market.', actions: ['Compile Product V2 result domain', 'Activate checked multiprogram release', 'Found physical economic projection', 'Found common Core Market', 'Create and fund resolution'] },
  { id: 'trade', number: '02', title: 'Trade & provide liquidity', summary: 'Construct intent, candidate, Series, and inventory routes only from current state and accepted account frames.', actions: ['Create registered order', 'Fill inline or registered intents', 'Prepare occurrence and ticket', 'Consider candidate / freeze selection', 'Initialize settlement', 'Activate custodied pool', 'Create LP / add / remove liquidity', 'Inventory-bounded immediate trade'] },
  { id: 'resolve', number: '03', title: 'Resolve & settle', summary: 'Bind real provider evidence, execute failure/recovery policy, and stream conservative physical effects.', actions: ['Resolve from real provider / failure path', 'Recover / archive / retire Source', 'Collect / materialize / distribute'] },
  { id: 'claim', number: '04', title: 'Claim & close', summary: 'Move exact liabilities between representations, redeem resolved value, then retire every quiescent child and root.', actions: ['Split / merge complete set', 'Materialize / dematerialize representation', 'Bearer mint / unwrap / redeem / retire', 'Cancel / expire / CancelThrough', 'Expire ticket / close occurrence / root', 'Close settlement / General root', 'Reset ladder / close LP / retire pool'] },
]);

type WorkbenchState = Readonly<{ kind: 'idle' | 'loading' | 'error'; message: string }> | Readonly<{ kind: 'ready'; snapshot: OperatorSurfaceSnapshotV1 }>;

function reason(error: unknown): string { return error instanceof Error ? error.message : 'chain acquisition refused without a usable reason'; }
function compact(value: string): string { return value.length > 22 ? `${value.slice(0, 8)}…${value.slice(-7)}` : value; }

function actionByName(action: string): OperatorWorkflowV1 {
  const workflow = OPERATOR_WORKFLOWS.find((candidate) => candidate.action === action);
  if (workflow === undefined) throw new Error(`workbench action ${action} has no operator owner`);
  return workflow;
}

function requiresMarket(workflow: OperatorWorkflowV1): boolean {
  return workflow.family !== 'Release' && workflow.family !== 'Creation';
}

export default function MarketWorkbench({ initialStage = 'author' }: Readonly<{ initialStage?: StageId }>) {
  const [stageId, setStageId] = useState<StageId>(initialStage);
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [coordinates, setCoordinates] = useState<Record<string, string>>(() => Object.fromEntries([...OPERATOR_ROLES, 'market'].map((role) => [role, ''])));
  const [state, setState] = useState<WorkbenchState>({ kind: 'idle', message: 'No chain authority has been selected.' });
  const stage = STAGES.find((candidate) => candidate.id === stageId) ?? STAGES[0];
  const actions = useMemo(() => stage.actions.map(actionByName), [stage]);

  function update(role: string, value: string) { setCoordinates((current) => ({ ...current, [role]: value.trim() })); }

  async function acquire(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setState({ kind: 'loading', message: 'Reacquiring executable roles and exact Market ownership from finalized local RPC…' });
    try {
      const snapshot = await acquireOperatorSurfaceV1(new SolanaRpcClient(endpoint), coordinates as OperatorCoordinatesV1);
      setState({ kind: 'ready', snapshot });
    } catch (error) { setState({ kind: 'error', message: `Refused: ${reason(error)}` }); }
  }

  const marketReady = state.kind === 'ready' && state.snapshot.market !== null;
  return <main className="product-shell workbench-shell">
    <header className="product-nav"><Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link><nav><Link className={stageId === 'author' ? 'active' : ''} href="/create">Create</Link><Link className={stageId === 'trade' ? 'active' : ''} href="/liquidity">Trade</Link><Link href="/operate">Operate</Link><Link href="/local">Local chain</Link></nav><span className="preview-control"><i className="preview-dot" />chain workbench</span></header>
    <section className="workbench-heading"><div><p className="eyebrow">Market lifecycle workbench</p><h1>From exact terms<br />to terminal claims.</h1></div><p>No sample market, price, pool, provider, balance, or wallet authority appears here. Select the six-program release and an optional live Market; the workbench exposes only accepted unsigned builders.</p></section>
    <nav className="workbench-stages" aria-label="Market lifecycle stages">{STAGES.map((candidate) => <button type="button" className={candidate.id === stageId ? 'active' : ''} onClick={() => setStageId(candidate.id)} key={candidate.id}><span>{candidate.number}</span><strong>{candidate.title}</strong><small>{candidate.summary}</small></button>)}</nav>

    <div className="workbench-grid"><form className="workbench-coordinates" onSubmit={acquire}><header><span>Chain authority</span><h2>Reacquire one exact execution surface</h2><p>The Registry is distinct from Core. Market ownership must match the selected Core program.</p></header><label><span>Finalized RPC endpoint</span><input value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label><div>{OPERATOR_ROLES.map((role) => <label key={role}><span>{role}</span><input required value={coordinates[role]} onChange={(event) => update(role, event.target.value)} /></label>)}</div><label><span>Market (optional during authoring)</span><input value={coordinates.market} onChange={(event) => update('market', event.target.value)} /></label><button type="submit" disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reading finalized state…' : 'Use this chain surface'}</button><p className="direct-status" aria-live="polite">{state.kind === 'ready' ? `Accepted at slot ${state.snapshot.observedSlot}${state.snapshot.market ? ` · ${compact(state.snapshot.market.address)} · ${state.snapshot.market.dataBytes} bytes` : ' · no Market selected'}` : state.message}</p>{state.kind === 'ready' && <dl className="workbench-authority"><div><dt>Programs</dt><dd>{state.snapshot.roles.length} executable</dd></div><div><dt>Market</dt><dd>{state.snapshot.market?.header ?? (state.snapshot.market ? 'unclassified accepted owner' : 'not selected')}</dd></div></dl>}</form>

      <section className="workbench-actions"><header><span>{stage.number} · current stage</span><h2>{stage.title}</h2><p>{stage.summary}</p></header><div>{actions.map((workflow) => {
        const chainMissing = state.kind !== 'ready';
        const marketMissing = requiresMarket(workflow) && !marketReady;
        const accepted = workflow.status === 'constructible' && workflow.route !== null && !chainMissing && !marketMissing;
        const liveReason = workflow.status === 'awaiting-abi' ? workflow.exactBoundary : chainMissing ? 'Reacquire the execution surface first.' : marketMissing ? 'Select and authenticate a Core-owned Market first.' : workflow.exactBoundary;
        return <article className={accepted ? 'ready' : ''} key={workflow.action}><div><span className={`operator-status ${accepted ? 'constructible' : workflow.status}`}>{accepted ? 'ready' : workflow.status}</span><h3>{workflow.action}</h3></div><p>{liveReason}</p>{accepted && workflow.route ? <Link href={workflow.route}>Open unsigned builder →</Link> : <button type="button" disabled>Transaction unavailable</button>}</article>;
      })}</div><footer><strong>Transaction handoff</strong><span>Every accepted builder emits unsigned bytes. Inspect dependencies and download the exact packet in the <Link href="/operate">operator console →</Link></span></footer></section></div>
  </main>;
}
