'use client';

import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import ArtifactInput from '@/components/ArtifactInput';
import ConsoleHeader from '@/components/ConsoleHeader';
import { Button } from '@/components/ui/button';
import { FormEvent, useEffect, useMemo, useRef, useState } from 'react';

import {
  liveDevnetOperatorPresetV1,
  OPERATOR_ROLES,
  acquireOperatorSurfaceV1,
  type OperatorDeploymentPresetV1,
  type OperatorCoordinatesV1,
  type OperatorSurfaceSnapshotV1,
} from '@dclutch/sdk/operatorSurface';
import {
  capabilityActContractV1,
  type CapabilityFamily,
  type CapabilityStandingV1,
} from '@dclutch/sdk/capabilityModel';
import { browserActPrerequisitesV1, BROWSER_CAPABILITY_STANDINGS_V1, capabilityWorkspaceV1 } from '@/lib/capabilitySurface';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';
import {
  acquireUnsignedTransactionDependenciesV1,
  inspectUnsignedTransactionV1,
  type UnsignedTransactionChainReportV1,
  type UnsignedTransactionInspectionV1,
} from '@dclutch/sdk/walletHandoff';
import CommandRunbook from '@/components/operator/CommandRunbook';

import { useDeploymentFieldV1 } from '@/lib/deploymentStore';

type Discovery = Readonly<{ kind: 'idle' | 'loading' | 'error'; message: string }> | Readonly<{ kind: 'ready'; snapshot: OperatorSurfaceSnapshotV1 }>;
type Packet = Readonly<{
  inspection: UnsignedTransactionInspectionV1;
  report: UnsignedTransactionChainReportV1 | null;
  endpoint: string;
  sourceText: string;
}>;
export type PacketExportStateV1 = Readonly<{
  endpoint: string;
  sourceText: string;
  report: Pick<UnsignedTransactionChainReportV1, 'missing' | 'nonExecutablePrograms'> | null;
}>;

export const DIRECT_ROUTE_RUNBOOK_V1 = `dclutch-terminal --rpc "$DEVNET_RPC" \\
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \\
  --bootstrap-bin "$SUCCESSOR" route release-set \\
  --plan "$PLAN" --expected-plan-sha256 "$PLAN_SHA256" \\
  --core-checked "$CORE_CHECKED" --expected-core-checked-sha256 "$CORE_SHA256" \\
  --claims-checked "$CLAIMS_CHECKED" --expected-claims-checked-sha256 "$CLAIMS_SHA256" \\
  --trading-checked "$TRADING_CHECKED" --expected-trading-checked-sha256 "$TRADING_SHA256" \\
  --resolution-checked "$RESOLUTION_CHECKED" --expected-resolution-checked-sha256 "$RESOLUTION_SHA256" \\
  --custody-checked "$CUSTODY_CHECKED" --expected-custody-checked-sha256 "$CUSTODY_SHA256" \\
  --output "$CHECKED_EXECUTION_RELEASE"

dclutch-terminal --rpc "$DEVNET_RPC" \\
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \\
  --bootstrap-bin "$SUCCESSOR" route direct \\
  --session "$DIRECT_SESSION" \\
  --checked-execution-release "$CHECKED_EXECUTION_RELEASE" \\
  --expected-checked-execution-release-sha256 "$CHECKED_EXECUTION_SHA256" \\
  --registry-checked "$REGISTRY_CHECKED" --expected-registry-checked-sha256 "$REGISTRY_SHA256" \\
  --rent-checked "$RENT_CHECKED" --expected-rent-checked-sha256 "$RENT_SHA256" \\
  --output "$DIRECT_ROUTE"`;

function reason(error: unknown): string { return error instanceof Error ? error.message : 'operation refused without a usable reason'; }
function compact(value: string): string { return value.length > 22 ? `${value.slice(0, 8)}…${value.slice(-7)}` : value; }
export function packetExportReadyV1(packet: PacketExportStateV1 | null, endpoint: string, sourceText: string): boolean {
  return packet !== null
    && packet.endpoint === endpoint
    && packet.sourceText === sourceText
    && packet.report !== null
    && packet.report.missing.length === 0
    && packet.report.nonExecutablePrograms.length === 0;
}
function familyGroups(): ReadonlyArray<Readonly<{ family: CapabilityFamily; actions: ReadonlyArray<CapabilityStandingV1> }>> {
  const order: CapabilityFamily[] = ['Release', 'Creation', 'Direct', 'Source', 'Series', 'General', 'Dealer', 'Claims'];
  return order.map((family) => Object.freeze({
    family,
    actions: BROWSER_CAPABILITY_STANDINGS_V1.filter((standing) => standing.action.family === family),
  }));
}

export default function OperatorSurface() {
  const [endpoint, setEndpoint] = useDeploymentFieldV1((d) => d.endpoint);
  const [coordinates, setCoordinates] = useState<Record<string, string>>(() => Object.fromEntries([...OPERATOR_ROLES, 'realm', 'market'].map((role) => [role, ''])));
  const [deploymentPreset, setDeploymentPreset] = useState<OperatorDeploymentPresetV1 | null>(null);
  const [discovery, setDiscovery] = useState<Discovery>({ kind: 'idle', message: 'No chain state has been read.' });
  const [unsignedText, setUnsignedText] = useState('');
  const [packet, setPacket] = useState<Packet | null>(null);
  const [packetStatus, setPacketStatus] = useState('No unsigned packet has been inspected.');
  const [dependencyStatus, setDependencyStatus] = useState('Inspect a packet before reading its dependencies.');
  const packetOperation = useRef(0);
  const currentEndpoint = useRef(endpoint);
  const currentUnsignedText = useRef(unsignedText);
  useEffect(() => { currentEndpoint.current = endpoint; }, [endpoint]);
  useEffect(() => { currentUnsignedText.current = unsignedText; }, [unsignedText]);
  const groups = useMemo(() => familyGroups(), []);
  // Counted from the derived standings, so these three numbers move when the
  // browser moves and never because someone retyped a row.
  const counts = useMemo(() => ({
    constructible: BROWSER_CAPABILITY_STANDINGS_V1.filter((standing) => standing.venue === 'browser').length,
    request: BROWSER_CAPABILITY_STANDINGS_V1.filter((standing) => standing.venue === 'operator-cli').length,
    blocked: BROWSER_CAPABILITY_STANDINGS_V1.filter((standing) => standing.venue === 'no-venue').length,
  }), []);

  function updateCoordinate(role: string, value: string) {
    if ((OPERATOR_ROLES as ReadonlyArray<string>).includes(role)) setDeploymentPreset(null);
    setCoordinates((current) => ({ ...current, [role]: value.trim() }));
  }

  function updateEndpoint(value: string) {
    packetOperation.current += 1;
    setDeploymentPreset(null);
    const next = value.trim();
    currentEndpoint.current = next;
    setEndpoint(next);
    setPacket(null);
    setPacketStatus('Endpoint changed. Inspect the unsigned packet again.');
    setDependencyStatus('Inspect the packet against this endpoint before exporting it.');
  }

  function updateUnsignedText(value: string) {
    packetOperation.current += 1;
    currentUnsignedText.current = value;
    setUnsignedText(value);
    setPacket(null);
    setPacketStatus('Packet bytes changed. Inspect this exact artifact.');
    setDependencyStatus('Inspect the packet before reading its dependencies.');
  }

  function loadLiveDevnetPreset() {
    setEndpoint(liveDevnetOperatorPresetV1().endpoint);
    setCoordinates((current) => ({
      ...current,
      ...liveDevnetOperatorPresetV1().coordinates,
      market: '',
      realm: '',
    }));
    setDeploymentPreset(liveDevnetOperatorPresetV1());
    setDiscovery({ kind: 'idle', message: 'The checked devnet coordinates are filled in. No chain state has been read yet.' });
  }

  async function inspectDeployment(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setDiscovery({ kind: 'loading', message: 'Reading finalized state…' });
    try {
      const client = new SolanaRpcClient(endpoint);
      const snapshot = await acquireOperatorSurfaceV1(client, coordinates as OperatorCoordinatesV1, deploymentPreset);
      setDiscovery({ kind: 'ready', snapshot });
    } catch (error) { setDiscovery({ kind: 'error', message: `Refused: ${reason(error)}` }); }
  }

  async function inspectPacket(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const operation = ++packetOperation.current;
    const sourceText = unsignedText;
    const sourceEndpoint = endpoint;
    setPacket(null); setPacketStatus('Inspecting packet bytes…'); setDependencyStatus('Inspect a packet before reading its dependencies.');
    try {
      const inspection = await inspectUnsignedTransactionV1(sourceText);
      if (packetOperation.current !== operation
          || currentEndpoint.current !== sourceEndpoint
          || currentUnsignedText.current !== sourceText) return;
      setPacket({ inspection, report: null, endpoint: sourceEndpoint, sourceText });
      setPacketStatus(`Unsigned packet inspected · ${inspection.wireBytes} bytes · ${inspection.instructionCount} instruction(s) · ${inspection.requiredSignatures} signature(s) required.`);
    } catch (error) {
      if (packetOperation.current === operation) setPacketStatus(`Refused: ${reason(error)}`);
    }
  }

  async function reacquirePacketDependencies() {
    if (packet === null) return;
    const operation = ++packetOperation.current;
    const inspected = packet;
    setPacket({ ...inspected, report: null });
    setDependencyStatus('Reading every packet dependency at one finalized floor…');
    try {
      const report = await acquireUnsignedTransactionDependenciesV1(new SolanaRpcClient(inspected.endpoint), inspected.inspection);
      if (packetOperation.current !== operation
          || currentEndpoint.current !== inspected.endpoint
          || currentUnsignedText.current !== inspected.sourceText) return;
      setPacket({ ...inspected, report });
      setDependencyStatus(report.missing.length === 0 && report.nonExecutablePrograms.length === 0
        ? `Ready to export · ${report.dependencies.length} accounts checked.`
        : `Refused for export: ${report.missing.length} missing account(s), ${report.nonExecutablePrograms.length} non-executable program(s).`);
    } catch (error) {
      if (packetOperation.current !== operation) return;
      setPacket({ ...inspected, report: null }); setDependencyStatus(`Refused: ${reason(error)}`);
    }
  }

  function downloadPacket() {
    if (!packetExportReadyV1(packet, endpoint, unsignedText) || packet === null) return;
    const blob = new Blob([packet.inspection.bytes as BlobPart], { type: 'application/octet-stream' });
    const link = document.createElement('a');
    link.href = URL.createObjectURL(blob); link.download = `dclutch-unsigned-${packet.inspection.digestHex.slice(0, 16)}.bin`; link.click();
    URL.revokeObjectURL(link.href);
  }

  return <PageShell className="product-shell operator-shell" header={<ConsoleHeader path="/operate" title="Operations" purpose="Inspect a deployment, find an action, or prepare a transaction." />}>
    <section className="operator-hero"><div><h1>Operations.</h1><p>Load the devnet program addresses or enter a custom deployment. Choose an action below to open its wallet flow or CLI instructions.</p></div><div className="operator-counts"><article><strong>{counts.constructible}</strong><span>browser actions</span></article><article><strong>{counts.request}</strong><span>CLI actions</span></article><article><strong>{counts.blocked}</strong><span>unavailable actions</span></article></div></section>

    <form className="operator-inspector" onSubmit={inspectDeployment}>
      <header><span>01</span><div><h2>Inspect a deployment</h2><p>Choose the devnet preset or enter program addresses and an RPC endpoint.</p><div className="direct-actions"><button type="button" className="secondary-action" onClick={loadLiveDevnetPreset}>Use devnet preset</button><Anchor href="/release">View release details →</Anchor></div></div></header>
      <div className="operator-coordinate-grid"><label className="wide"><span>Finalized RPC endpoint</span><input value={endpoint} onChange={(event) => updateEndpoint(event.target.value)} /></label>{OPERATOR_ROLES.map((role) => <label key={role}><span>{role} program</span><input required value={coordinates[role]} onChange={(event) => updateCoordinate(role, event.target.value)} /></label>)}<label><span>Realm (optional)</span><input value={coordinates.realm} onChange={(event) => updateCoordinate('realm', event.target.value)} /></label><label><span>Market (optional)</span><input value={coordinates.market} onChange={(event) => updateCoordinate('market', event.target.value)} /></label></div>
      <button type="submit" disabled={discovery.kind === 'loading'}>{discovery.kind === 'loading' ? 'Reading finalized state…' : 'Inspect deployment'}</button><p className="direct-status" aria-live="polite">{discovery.kind === 'ready' ? `${discovery.snapshot.deploymentPreset ? (discovery.snapshot.deploymentPreset.upgradedSinceRecord.length === 0 ? 'Checked devnet preset matched finalized chain state' : `Checked devnet preset: every Loader identity matched, and ${discovery.snapshot.deploymentPreset.upgradedSinceRecord.join(', ')} ${discovery.snapshot.deploymentPreset.upgradedSinceRecord.length === 1 ? 'has' : 'have'} been upgraded in place since this app was built`) : 'Custom coordinates observed'} at slot ${discovery.snapshot.observedSlot} · ${discovery.snapshot.roles.length} executable roles${discovery.snapshot.realm ? ` · Realm ${compact(discovery.snapshot.realm.address)}` : ''}${discovery.snapshot.market ? ` · Market ${compact(discovery.snapshot.market.address)}` : ''}` : discovery.message}</p>
      {discovery.kind === 'ready' && <div className="operator-role-grid">{discovery.snapshot.roles.map((role) => <article key={role.role}><span>{role.role}</span><strong>{compact(role.address)}</strong><small>{role.dataBytes} data bytes · owner {compact(role.owner)}</small></article>)}</div>}
    </form>

    <section className="operator-route-runbook" id="direct-route"><header><span>02</span><div><h2>Export the portable Direct route</h2><p>Create the route file a maker or taker loads into a trading client.</p></div></header><div className="operator-route-contract"><article><span>Inputs</span><strong>Checked releases + frozen Direct session</strong><p>Provide the release reports and their SHA-256 hashes, plus a Direct session with a frozen lookup table.</p></article><article><span>Network</span><strong>Solana devnet</strong><p>The command checks the deployment and session accounts before exporting.</p></article><article><span>Result</span><strong>One route + one report</strong><p>The route JSON contains the accounts, lookup table and deployment information needed by the trading client.</p></article><article><span>If refused</span><strong>Resolve the reported error</strong><p>Freeze the session lookup table if requested. For a release mismatch, use reports matching the selected deployment.</p></article></div><details><summary>Show the exact CLI invocation</summary><p>Set each shell variable to an absolute path or the digest from that artifact’s machine report. Both output paths must be new.</p><CommandRunbook label="Read-only route export" command={DIRECT_ROUTE_RUNBOOK_V1} /></details></section>

    <section className="operator-wave"><header><span>03</span><div><h2>Protocol actions</h2><p>Find an action by protocol family. Each entry lists its requirements and links to the available tool.</p></div></header><div className="operator-family-grid">{groups.map((group) => <article key={group.family}><h3>{group.family}</h3>{group.actions.map((standing) => {
      const contract = capabilityActContractV1(standing);
      const workspace = capabilityWorkspaceV1(standing.action, discovery.kind === 'ready' ? discovery.snapshot : null);
      const needed = browserActPrerequisitesV1(standing);
      return <div className="operator-action" key={standing.action.id}><span className={`operator-status ${standing.venue}`}>{contract.venue}</span><strong>{standing.action.action}</strong><p>{standing.action.guarantee}</p>{needed.length === 0 ? null : <p className="operator-action-need"><strong>Before you start</strong> {needed.map((entry) => entry.statement).join('; and ')}.</p>}{standing.walls.map((held) => <p className="operator-action-wall" key={held.citation}><strong>Unavailable:</strong> {held.statement} <small>({held.citation})</small></p>)}{standing.unverifiedAbis.map((module) => <p className="operator-action-wall" key={module}><strong>Build check missing:</strong> {module} is generated and no <code>abi:*:verify</code> script checks it.</p>)}{workspace !== null
        ? <Anchor href={workspace}>{standing.venue === 'operator-cli' ? 'Open CLI instructions' : standing.authority === 'none' ? 'Check requirements' : standing.authority === 'wallet-message' ? 'Open offer authoring' : 'Open wallet flow'} →</Anchor>
        : standing.action.workspace === 'market-detail' && <small className="operator-action-remedy">Enter a Market above to open its actions.</small>}</div>;
    })}</article>)}</div></section>

    <section className="operator-handoff"><header><span>04</span><div><h2>Inspect and export a transaction</h2><p>Load an unsigned transaction, check its accounts on the selected network, then download it.</p></div></header><div className="operator-handoff-grid"><form onSubmit={inspectPacket}>
      <span className="panel-label">01 · inspect bytes</span>
      <ArtifactInput label="Unsigned transaction" provenance="Load a transaction file from the console or CLI, or paste its base64 text." value={unsignedText} onChange={updateUnsignedText} required />
      <Button type="submit">Inspect unsigned packet</Button><p className="direct-status" aria-live="polite">{packetStatus}</p>
    </form><aside><span className="panel-label">02 · check accounts</span><h3>{packet === null ? 'No packet inspected' : `${packet.inspection.digestHex.slice(0, 12)}…${packet.inspection.digestHex.slice(-8)}`}</h3>
      {packet === null
        ? <p>Load a transaction to see its accounts, programs, lookup tables and required signatures.</p>
        : <dl><div><dt>SHA-256</dt><dd className="mono-value">{packet.inspection.digestHex}</dd></div><div><dt>Wire / instructions</dt><dd>{packet.inspection.wireBytes} B / {packet.inspection.instructionCount}</dd></div><div><dt>Signatures required</dt><dd>{packet.inspection.requiredSignatures}</dd></div><div><dt>Lookup tables</dt><dd>{packet.inspection.lookupTables.length}</dd></div>{packet.report && <><div><dt>Resolved accounts</dt><dd>{packet.report.dependencies.length}</dd></div><div><dt>Missing / non-executable</dt><dd>{packet.report.missing.length} / {packet.report.nonExecutablePrograms.length}</dd></div></>}</dl>}
      <Button type="button" variant="outline" disabled={packet === null} onClick={() => void reacquirePacketDependencies()}>Check transaction accounts</Button><p className="direct-status" aria-live="polite">{dependencyStatus}</p>
      <Button type="button" disabled={!packetExportReadyV1(packet, endpoint, unsignedText)} onClick={downloadPacket}>Download unsigned transaction</Button>
      <p>Sign the downloaded transaction in your wallet client.</p>
    </aside></div></section>
  </PageShell>;
}
