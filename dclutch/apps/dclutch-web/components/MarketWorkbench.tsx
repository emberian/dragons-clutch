'use client';

import Anchor from '@/components/Anchor';
import { FormEvent, useEffect, useMemo, useRef, useState } from 'react';

import {
  OPERATOR_ROLES,
  acquireOperatorSurfaceV1,
  type OperatorCoordinatesV1,
  type OperatorSurfaceSnapshotV1,
} from '@/lib/operatorSurface';
import {
  capabilityActionsForStageV1,
  capabilityActContractV1,
  capabilityWorkspaceV1,
  evaluateCapabilityV1,
  type CapabilityStage,
} from '@/lib/capabilityModel';
import ConsoleHeader from '@/components/ConsoleHeader';
import { smokeStoryEnabledV1 } from '@/lib/flags';
import { SolanaRpcClient } from '@/lib/rpc';
import { useDeploymentFieldV1, useDeploymentV1 } from '@/lib/deploymentStore';
import {
  DerivedProvenance,
  EndpointField,
  OperatorRefusal,
  PubkeyField,
} from '@/components/operator/OperatorFields';

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

type WorkbenchState =
  | Readonly<{ kind: 'idle'; message: string }>
  | Readonly<{ kind: 'loading' | 'error'; message: string; inputKey: string }>
  | Readonly<{ kind: 'ready'; snapshot: OperatorSurfaceSnapshotV1; inputKey: string }>;
type WorkbenchRefusalFieldV1 = 'endpoint' | (typeof OPERATOR_ROLES)[number] | 'realm' | 'market' | null;

function reason(error: unknown): string { return error instanceof Error ? error.message : 'chain acquisition refused without a usable reason'; }
function compact(value: string): string { return value.length > 22 ? `${value.slice(0, 8)}…${value.slice(-7)}` : value; }

/** Put a single-input refusal with that input; leave cross-field joins at act level. */
export function workbenchRefusalFieldV1(message: string): WorkbenchRefusalFieldV1 {
  const lower = message.toLowerCase();
  if (/endpoint|json-rpc|rpc |invalid url|http:|https:|genesis/.test(lower)) return 'endpoint';
  if (/realm or market|aliases|not owned by .*program|not owned by the selected/.test(lower)) return null;
  if (lower.includes('realm') && !lower.includes('realm and market')) return 'realm';
  if (lower.includes('market') && !lower.includes('realm and market')) return 'market';
  for (const role of OPERATOR_ROLES) {
    if (lower.includes(`${role} program`)) return role;
  }
  return null;
}

function refusalRemedy(field: Exclude<WorkbenchRefusalFieldV1, null>): string {
  if (field === 'endpoint') return 'Check the finalized RPC endpoint.';
  if (field === 'realm') return 'Check the optional Realm address, or clear it.';
  if (field === 'market') return 'Check the optional Market address, or clear it.';
  return `Check the ${field} program address.`;
}

export default function MarketWorkbench({ initialStage = 'author', surface = 'lifecycle' }: Readonly<{
  initialStage?: CapabilityStage;
  surface?: 'lifecycle' | 'resolution';
}>) {
  const deployment = useDeploymentV1();
  const [stageId, setStageId] = useState<CapabilityStage>(initialStage);
  const [endpoint, setEndpoint] = useDeploymentFieldV1((d) => d.endpoint);
  const [coordinates, setCoordinates] = useState<Record<string, string>>(() => Object.fromEntries([...OPERATOR_ROLES, 'realm', 'market'].map((role) => [role, ''])));
  const [state, setState] = useState<WorkbenchState>({ kind: 'idle', message: 'Deployment programs are filled from the selected cluster. No chain read yet.' });
  const stage = STAGES.find((candidate) => candidate.id === stageId) ?? STAGES[0];
  const actions = useMemo(() => capabilityActionsForStageV1(stage.id), [stage]);
  const effectiveCoordinates: OperatorCoordinatesV1 = Object.freeze({
    ...Object.fromEntries(OPERATOR_ROLES.map((role) => [role, coordinates[role] || deployment.programs[role]])),
    realm: coordinates.realm,
    market: coordinates.market,
  }) as OperatorCoordinatesV1;
  const inputKey = JSON.stringify([endpoint, deployment.cluster, ...OPERATOR_ROLES.map((role) => effectiveCoordinates[role]), effectiveCoordinates.realm, effectiveCoordinates.market]);
  const currentInputKey = useRef(inputKey);
  useEffect(() => { currentInputKey.current = inputKey; }, [inputKey]);

  function update(role: string, value: string) { setCoordinates((current) => ({ ...current, [role]: value.trim() })); }

  async function acquire(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const requestedInputKey = inputKey;
    setState({ kind: 'loading', message: 'Reacquiring executable roles and exact Market ownership from finalized local RPC…', inputKey: requestedInputKey });
    try {
      const snapshot = await acquireOperatorSurfaceV1(new SolanaRpcClient(endpoint), effectiveCoordinates);
      if (currentInputKey.current !== requestedInputKey) return;
      setState({ kind: 'ready', snapshot, inputKey: requestedInputKey });
    } catch (error) {
      if (currentInputKey.current === requestedInputKey) setState({ kind: 'error', message: `Refused: ${reason(error)}`, inputKey: requestedInputKey });
    }
  }

  const currentState: WorkbenchState = state.kind !== 'idle' && state.inputKey !== inputKey
    ? { kind: 'idle', message: 'Inputs changed. Reacquire this exact chain surface before opening a route.' }
    : state;
  const snapshot = currentState.kind === 'ready' ? currentState.snapshot : null;
  const resolutionSurface = surface === 'resolution';
  const refusalField = currentState.kind === 'error' ? workbenchRefusalFieldV1(currentState.message) : null;
  const refusalFor = (field: Exclude<WorkbenchRefusalFieldV1, null>) => refusalField === field
    ? <OperatorRefusal remedy={refusalRemedy(field)} detail={currentState.kind === 'error' ? currentState.message : ''} />
    : null;
  return <main className="product-shell workbench-shell">
    <ConsoleHeader
      path={resolutionSurface ? '/resolution' : '/workbench'}
      title={resolutionSurface ? 'Resolution readiness' : 'Lifecycle readiness'}
      purpose={resolutionSurface
        ? 'Read what the selected market still needs before a resolution route can begin preflight.'
        : 'Read which lifecycle routes can begin preflight against the chain you choose.'}
    />
    <section className="workbench-heading"><div><h1>{resolutionSurface ? <>Resolution<br />readiness.</> : <>The market<br />lifecycle.</>}</h1></div><p>{resolutionSurface
      ? 'This read-only map opens at Resolve & settle. It reads the selected chain and names missing preconditions; it cannot resolve a market.'
      : 'This is a read-only map of where a market has got to. It reads the chain and tells you what is still missing; it does not create, trade, resolve, or redeem. No sample market, price, pool, balance, or wallet authority appears here.'}</p></section>
    {smokeStoryEnabledV1() && <section className="trade-v3-card">
      <header><span>··</span><div><h2>Three markets, run in public</h2><p>A price market Pyth settles on its own, a devnet market about a real mainnet event, and one we abandon on purpose so you can finish it and collect the bounty.</p></div></header>
      <div className="direct-actions">
        <Anchor className="secondary-action" href="/smoke">Read the story →</Anchor>
        <Anchor className="secondary-action" href="/bounty">How the bounty works →</Anchor>
      </div>
    </section>}
    <nav className="workbench-stages" aria-label="Market lifecycle stages">{STAGES.map((candidate) => <button type="button" className={candidate.id === stageId ? 'active' : ''} onClick={() => setStageId(candidate.id)} key={candidate.id}><span>{candidate.number}</span><strong>{candidate.title}</strong><small>{candidate.summary}</small></button>)}</nav>

    <div className="workbench-grid"><form className="workbench-coordinates" onSubmit={acquire}><header><span>Chain observation</span><h2>Reacquire one execution surface</h2><p>{deployment.label} supplies the six program addresses. Realm and Market are the only optional coordinates this read needs.</p></header>
      <fieldset className="operator-act">
        <legend>The chain this read observes</legend>
        <div className="operator-field-slot"><EndpointField label="Finalized RPC endpoint" value={endpoint} onChange={setEndpoint}
          provenance={<DerivedProvenance derived={deployment.endpoint} value={endpoint} source="the cluster picked in the header" absent="Pick a cluster in the header, or paste an endpoint." />} />{refusalFor('endpoint')}</div>
        <details className="operator-override"><summary>Program overrides · {OPERATOR_ROLES.length} filled from {deployment.label}</summary><p>Use these only to inspect a deployment other than the one selected in the header. Every edit is still reacquired from the chain.</p><div className="operator-act-grid">{OPERATOR_ROLES.map((role) => <div className="operator-field-slot" key={role}><PubkeyField label={`${role} program`} required value={effectiveCoordinates[role]} onChange={(next) => update(role, next)}
          provenance={<DerivedProvenance derived={deployment.programs[role]} value={effectiveCoordinates[role]} source={`the ${deployment.label} deployment`} absent="Select a deployment in the header, or paste this program address." />} />{refusalFor(role)}</div>)}</div></details>
      </fieldset>
      <fieldset className="operator-act"><legend>Optional state coordinates</legend><div className="operator-act-grid">
        <div className="operator-field-slot"><PubkeyField label="Realm · optional" value={coordinates.realm} onChange={(next) => update('realm', next)} provenance="Add one only when the lifecycle decision depends on a Realm read." />{refusalFor('realm')}</div>
        <div className="operator-field-slot"><PubkeyField label="Market · optional during authoring" value={coordinates.market} onChange={(next) => update('market', next)} provenance="Add one to evaluate market-bound lifecycle actions; leave it empty while authoring records." />{refusalFor('market')}</div>
      </div></fieldset>
      <button type="submit" disabled={currentState.kind === 'loading'}>{currentState.kind === 'loading' ? 'Reading finalized state…' : 'Observe this chain surface'}</button><p className="direct-status" aria-live="polite">{currentState.kind === 'ready' ? `Observed at slot ${currentState.snapshot.observedSlot}${currentState.snapshot.market ? ` · ${compact(currentState.snapshot.market.address)} · ${currentState.snapshot.market.dataBytes} bytes` : ' · no Market selected'}` : currentState.kind === 'error' && refusalField !== null ? `Observation refused at ${refusalField}. Its remedy is beside that field.` : currentState.message}</p>{currentState.kind === 'error' && refusalField === null ? <OperatorRefusal remedy="Recheck the coordinates as one deployment." detail={currentState.message} /> : null}{currentState.kind === 'ready' && <dl className="workbench-authority"><div><dt>Programs</dt><dd>{currentState.snapshot.roles.length} executable</dd></div><div><dt>Realm</dt><dd>{currentState.snapshot.realm?.header ?? (currentState.snapshot.realm ? 'Core-owned / unclassified' : 'not selected')}</dd></div><div><dt>Market</dt><dd>{currentState.snapshot.market?.header ?? (currentState.snapshot.market ? 'Core-owned / unclassified' : 'not selected')}</dd></div><div><dt>Release</dt><dd>unrecognized until route preflight</dd></div></dl>}</form>

      <section className="workbench-actions"><header><span>{stage.number} · current stage</span><h2>{stage.title}</h2><p>{stage.summary}</p></header><div>{actions.map((action) => {
        const verdict = evaluateCapabilityV1(action, snapshot);
        const workspace = capabilityWorkspaceV1(action, snapshot);
        const contract = capabilityActContractV1(action);
        const accepted = verdict.status === 'ready-to-preflight' && workspace !== null;
        return <article className={accepted ? 'ready' : ''} key={action.id}><div><span className={`operator-status ${verdict.status}`}>{verdict.status.replaceAll('-', ' ')}</span><h3>{action.action}</h3></div><p>{verdict.reason}</p><dl className="operator-action-contract"><div><dt>Authority</dt><dd>{contract.authority}</dd></div><div><dt>Result</dt><dd>{contract.result}</dd></div></dl>{accepted && workspace !== null ? <Anchor href={workspace}>Open exact preflight →</Anchor> : verdict.status === 'rust-only' && workspace !== null ? <Anchor href={workspace}>Inspect current boundary →</Anchor> : <button type="button" disabled>{verdict.status === 'needs-market' ? 'Select and reacquire a Market' : 'Transaction unavailable'}</button>}</article>;
      })}</div><footer><strong>Action handoff</strong><span>Each action above names its authority and result. For an unsigned transaction, inspect dependencies and download the exact packet in the <Anchor href="/operate">operator console →</Anchor></span></footer></section></div>
  </main>;
}
