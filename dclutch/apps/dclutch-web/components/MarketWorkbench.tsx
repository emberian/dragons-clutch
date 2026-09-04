'use client';

import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import { FormEvent, useEffect, useMemo, useRef, useState } from 'react';

import {
  OPERATOR_ROLES,
  acquireOperatorSurfaceV1,
  type OperatorCoordinatesV1,
  type OperatorSurfaceSnapshotV1,
} from '@/lib/operatorSurface';
import {
  capabilityActContractV1,
  capabilityPhaseGateTextV1,
  evaluateCapabilityV1,
  machineTextV1,
  type CapabilityStage,
} from '@/lib/capabilityModel';
import {
  acquireMachineObservationsV1,
  type MachineObservationV1,
} from '@dclutch/sdk/stateMachines';
import { CORE_STATE_GENERATION_OFFSET } from '@dclutch/sdk/generated/coreFound';
import { u64 } from '@dclutch/sdk/bytes';
import { browserCapabilityStandingsForStageV1, capabilityWorkspaceV1 } from '@/lib/capabilitySurface';
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

/**
 * What the machines this observation read are in, for the status line.
 *
 * Said beside the Market's own slot because a Source resolution state is a
 * DIFFERENT machine at the same floor, and a reader who sees only the Market's
 * phase has been told half of what was read. A machine whose account is absent
 * says so; a machine whose bytes were refused says that instead, because those
 * are different facts and only the second is a defect.
 */
export function machineObservationTextV1(machines: ReadonlyArray<MachineObservationV1>): string {
  if (machines.length === 0) return '';
  return machines.map((machine) => {
    if (machine.state !== null) return ` · ${machine.machine} ${machine.state}`;
    return machine.present ? ` · ${machine.machine} refused` : ` · no ${machine.machine} account`;
  }).join('');
}

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
  | Readonly<{
      kind: 'ready';
      snapshot: OperatorSurfaceSnapshotV1;
      /**
       * The state machines this observation could read, decoded.
       *
       * Empty is not "none apply": it is "this reader observed none", which is
       * what `evaluateCapabilityV1` turns into `needs-chain` with the machine
       * named. Only the Source state has an address a Market determines, so
       * this holds at most one entry today and says so on the surface rather
       * than leaving a reader to infer it.
       */
      machines: ReadonlyArray<MachineObservationV1>;
      inputKey: string;
    }>;
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
  const actions = useMemo(() => browserCapabilityStandingsForStageV1(stage.id), [stage]);
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
      const client = new SolanaRpcClient(endpoint);
      const snapshot = await acquireOperatorSurfaceV1(client, effectiveCoordinates);
      if (currentInputKey.current !== requestedInputKey) return;
      // The Source state is the one machine whose account a Market determines,
      // so it is read at the same floor rather than left `needs-chain`. A
      // refusal here is recorded as a refusal and never dropped: an observation
      // that quietly became empty is indistinguishable from one that was never
      // attempted, which is the reading this whole surface exists to remove.
      let machines: ReadonlyArray<MachineObservationV1> = [];
      if (snapshot.market !== null) {
        const market = snapshot.market.address;
        try {
          const core = await client.accountInfo(market);
          if (core.account === null) throw new Error('the Market vanished between reads');
          machines = await acquireMachineObservationsV1(
            client,
            { address: market, generation: u64(core.account.data, CORE_STATE_GENERATION_OFFSET) },
            effectiveCoordinates.resolution,
          );
        } catch (error) {
          machines = [{ machine: 'source', present: true, state: null, refusal: reason(error) }];
        }
      }
      if (currentInputKey.current !== requestedInputKey) return;
      setState({ kind: 'ready', snapshot, machines, inputKey: requestedInputKey });
    } catch (error) {
      if (currentInputKey.current === requestedInputKey) setState({ kind: 'error', message: `Refused: ${reason(error)}`, inputKey: requestedInputKey });
    }
  }

  const currentState: WorkbenchState = state.kind !== 'idle' && state.inputKey !== inputKey
    ? { kind: 'idle', message: 'Inputs changed. Reacquire this exact chain surface before opening a route.' }
    : state;
  const snapshot = currentState.kind === 'ready' ? currentState.snapshot : null;
  const machines: ReadonlyArray<MachineObservationV1> = currentState.kind === 'ready' ? currentState.machines : [];
  const resolutionSurface = surface === 'resolution';
  const refusalField = currentState.kind === 'error' ? workbenchRefusalFieldV1(currentState.message) : null;
  const refusalFor = (field: Exclude<WorkbenchRefusalFieldV1, null>) => refusalField === field
    ? <OperatorRefusal remedy={refusalRemedy(field)} detail={currentState.kind === 'error' ? currentState.message : ''} />
    : null;
  return <PageShell className="product-shell workbench-shell" header={<ConsoleHeader
      path={resolutionSurface ? '/resolution' : '/workbench'}
      title={resolutionSurface ? 'Resolution readiness' : 'Lifecycle readiness'}
      purpose={resolutionSurface
        ? 'Read what the selected market still needs before a resolution route can begin preflight.'
        : 'Read which lifecycle routes can begin preflight against the chain you choose.'}
    />}>
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
      <button type="submit" disabled={currentState.kind === 'loading'}>{currentState.kind === 'loading' ? 'Reading finalized state…' : 'Observe this chain surface'}</button><p className="direct-status" aria-live="polite">{currentState.kind === 'ready' ? `Observed at slot ${currentState.snapshot.observedSlot}${currentState.snapshot.market ? ` · ${compact(currentState.snapshot.market.address)} · ${currentState.snapshot.market.dataBytes} bytes` : ' · no Market selected'}${machineObservationTextV1(currentState.machines)}` : currentState.kind === 'error' && refusalField !== null ? `Observation refused at ${refusalField}. Its remedy is beside that field.` : currentState.message}</p>{currentState.kind === 'error' && refusalField === null ? <OperatorRefusal remedy="Recheck the coordinates as one deployment." detail={currentState.message} /> : null}{currentState.kind === 'ready' && <dl className="workbench-authority"><div><dt>Programs</dt><dd>{currentState.snapshot.roles.length} executable</dd></div><div><dt>Realm</dt><dd>{currentState.snapshot.realm?.header ?? (currentState.snapshot.realm ? 'Core-owned / unclassified' : 'not selected')}</dd></div><div><dt>Market</dt><dd>{currentState.snapshot.market?.header ?? (currentState.snapshot.market ? 'Core-owned / unclassified' : 'not selected')}</dd></div><div><dt>Release</dt><dd>unrecognized until route preflight</dd></div></dl>}</form>

      <section className="workbench-actions"><header><span>{stage.number} · current stage</span><h2>{stage.title}</h2><p>{stage.summary}</p></header><div>{actions.map((standing) => {
        const verdict = evaluateCapabilityV1(standing, snapshot, machines);
        // Derived, never typed: the clauses come from the census's own sets and
        // the decoded state, so a card cannot say a machine admitted anything
        // the table does not.
        const machineClauses = machineTextV1(verdict.phaseGate);
        const workspace = capabilityWorkspaceV1(standing.action, snapshot);
        const contract = capabilityActContractV1(standing);
        const accepted = verdict.status === 'ready-to-preflight' && workspace !== null;
        // No disabled button anywhere on this surface. A control that says no
        // and cannot say why is the flat-console failure in miniature; where an
        // act cannot be opened, the card says what is missing and links to the
        // page that answers it, which is always reachable.
        return <article className={accepted ? 'ready' : ''} key={standing.action.id}><div><span className={`operator-status ${verdict.status}`}>{verdict.status.replaceAll('-', ' ')}</span><h3>{standing.action.action}</h3></div><p>{verdict.reason}</p><dl className="operator-action-contract"><div><dt>Where it runs</dt><dd>{contract.venue}</dd></div><div><dt>What it promises</dt><dd>{contract.guarantee}</dd></div><div><dt>Phase gate</dt><dd>{capabilityPhaseGateTextV1(verdict.phaseGate)}</dd></div>{machineClauses.length > 0 ? <div><dt>Machine gate</dt><dd>{machineClauses.join('; ')}</dd></div> : null}</dl>{standing.walls.map((held) => <p className="operator-action-wall" key={held.citation}><strong>Known wall</strong> {held.statement} <small>({held.citation})</small></p>)}{accepted && workspace !== null
          ? <Anchor href={workspace}>Open exact preflight →</Anchor>
          : verdict.status === 'not-this-market' && workspace !== null
            ? <Anchor href={workspace}>Open it for a new Market →</Anchor>
            : workspace !== null
            ? <Anchor href={workspace}>Inspect current boundary →</Anchor>
            : verdict.status === 'needs-market'
              ? <Anchor href="/markets">Choose a Market, then reacquire →</Anchor>
              : <Anchor href="/operate">Read this act’s boundary →</Anchor>}</article>;
      })}</div><footer><strong>Action handoff</strong><span>Each act above names where it runs and what it promises. For an unsigned transaction, inspect dependencies and download the exact packet in the <Anchor href="/operate">operator console →</Anchor></span></footer></section></div>
  </PageShell>;
}
