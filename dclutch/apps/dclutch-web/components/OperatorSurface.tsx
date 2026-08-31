'use client';

import Anchor from '@/components/Anchor';
import ConsoleHeader from '@/components/ConsoleHeader';
import { FormEvent, useMemo, useState } from 'react';

import {
  LIVE_DEVNET_OPERATOR_PRESET_V1,
  OPERATOR_ROLES,
  acquireOperatorSurfaceV1,
  type OperatorDeploymentPresetV1,
  type OperatorCoordinatesV1,
  type OperatorSurfaceSnapshotV1,
} from '@/lib/operatorSurface';
import {
  CAPABILITY_ACTIONS_V1,
  type CapabilityActionV1,
  type CapabilityFamily,
} from '@/lib/capabilityModel';
import { SolanaRpcClient } from '@/lib/rpc';
import {
  acquireUnsignedTransactionDependenciesV1,
  inspectUnsignedTransactionV1,
  type UnsignedTransactionChainReportV1,
  type UnsignedTransactionInspectionV1,
} from '@/lib/walletHandoff';

import WalletDirectory, { useWalletDirectoryV1 } from './WalletDirectory';
import { useDeploymentFieldV1 } from '@/lib/deploymentStore';

type Discovery = Readonly<{ kind: 'idle' | 'loading' | 'error'; message: string }> | Readonly<{ kind: 'ready'; snapshot: OperatorSurfaceSnapshotV1 }>;
type Packet = Readonly<{ inspection: UnsignedTransactionInspectionV1; report: UnsignedTransactionChainReportV1 }>;

function reason(error: unknown): string { return error instanceof Error ? error.message : 'operation refused without a usable reason'; }
function compact(value: string): string { return value.length > 22 ? `${value.slice(0, 8)}…${value.slice(-7)}` : value; }
function familyGroups(): ReadonlyArray<Readonly<{ family: CapabilityFamily; actions: ReadonlyArray<CapabilityActionV1> }>> {
  const order: CapabilityFamily[] = ['Release', 'Creation', 'Direct', 'Source', 'Series', 'General', 'Dealer', 'Claims'];
  return order.map((family) => Object.freeze({ family, actions: CAPABILITY_ACTIONS_V1.filter((workflow) => workflow.family === family) }));
}

export default function OperatorSurface() {
  const [endpoint, setEndpoint] = useDeploymentFieldV1((d) => d.endpoint);
  const [coordinates, setCoordinates] = useState<Record<string, string>>(() => Object.fromEntries([...OPERATOR_ROLES, 'realm', 'market'].map((role) => [role, ''])));
  const [deploymentPreset, setDeploymentPreset] = useState<OperatorDeploymentPresetV1 | null>(null);
  const [discovery, setDiscovery] = useState<Discovery>({ kind: 'idle', message: 'No chain state has been read.' });
  const [unsignedText, setUnsignedText] = useState('');
  const [packet, setPacket] = useState<Packet | null>(null);
  const [packetStatus, setPacketStatus] = useState('Paste an unsigned transaction emitted by one accepted workflow.');
  const [wallet, setWallet] = useState('');
  const [walletStatus, setWalletStatus] = useState('Wallet identity is optional; signing and submission are absent from this surface.');
  const wallets = useWalletDirectoryV1();
  const groups = useMemo(() => familyGroups(), []);
  const counts = useMemo(() => ({
    constructible: CAPABILITY_ACTIONS_V1.filter((workflow) => workflow.implementation === 'browser-unsigned' || workflow.implementation === 'browser-wallet').length,
    request: CAPABILITY_ACTIONS_V1.filter((workflow) => workflow.implementation === 'rust-unsigned').length,
    blocked: CAPABILITY_ACTIONS_V1.filter((workflow) => workflow.implementation === 'awaiting-production').length,
  }), []);

  function updateCoordinate(role: string, value: string) {
    if ((OPERATOR_ROLES as ReadonlyArray<string>).includes(role)) setDeploymentPreset(null);
    setCoordinates((current) => ({ ...current, [role]: value.trim() }));
  }

  function updateEndpoint(value: string) {
    setDeploymentPreset(null);
    setEndpoint(value.trim());
  }

  function loadLiveDevnetPreset() {
    setEndpoint(LIVE_DEVNET_OPERATOR_PRESET_V1.endpoint);
    setCoordinates((current) => ({
      ...current,
      ...LIVE_DEVNET_OPERATOR_PRESET_V1.coordinates,
      market: '',
      realm: '',
    }));
    setDeploymentPreset(LIVE_DEVNET_OPERATOR_PRESET_V1);
    setDiscovery({ kind: 'idle', message: 'The checked devnet coordinates are filled in. No chain state has been read. Inspect them to require a finalized match before you rely on this deployment.' });
  }

  async function inspectDeployment(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setDiscovery({ kind: 'loading', message: deploymentPreset === null
      ? 'Reacquiring all six executable roles and the optional Market at one finalized floor…'
      : 'Reacquiring devnet identity, six exact Loader deployments, the release cache, and any address you supplied at one finalized floor…' });
    try {
      const client = new SolanaRpcClient(endpoint);
      const snapshot = await acquireOperatorSurfaceV1(client, coordinates as OperatorCoordinatesV1, deploymentPreset);
      setDiscovery({ kind: 'ready', snapshot });
    } catch (error) { setDiscovery({ kind: 'error', message: `Refused: ${reason(error)}` }); }
  }

  async function inspectPacket(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setPacket(null); setPacketStatus('Decoding the unsigned packet, resolving lookup tables, and reacquiring every message account…');
    try {
      const inspection = await inspectUnsignedTransactionV1(unsignedText);
      const report = await acquireUnsignedTransactionDependenciesV1(new SolanaRpcClient(endpoint), inspection);
      setPacket({ inspection, report });
      setPacketStatus(report.missing.length === 0 && report.nonExecutablePrograms.length === 0
        ? 'Unsigned packet and every current program/account dependency were reacquired. Nothing was signed or submitted.'
        : `Refused for execution handoff: ${report.missing.length} missing account(s), ${report.nonExecutablePrograms.length} non-executable program(s).`);
    } catch (error) { setPacketStatus(`Refused: ${reason(error)}`); }
  }

  function downloadPacket() {
    if (packet === null) return;
    const blob = new Blob([packet.inspection.bytes as BlobPart], { type: 'application/octet-stream' });
    const link = document.createElement('a');
    link.href = URL.createObjectURL(blob); link.download = `dclutch-unsigned-${packet.inspection.digestHex.slice(0, 16)}.bin`; link.click();
    URL.revokeObjectURL(link.href);
  }

  function adoptIdentity(address: string) {
    setWallet(address); setWalletStatus(`Connected · ${address}`);
  }

  return <main className="product-shell operator-shell">
    <ConsoleHeader path="/operate" title="Operations" purpose="See known constructors and missing seams. Every route still requires its own preflight." />
    <section className="operator-hero"><div><h1>Operations.</h1><p>You can load the checked devnet coordinates instead of typing six program addresses. The preset never supplies a Market, and loading it is not a chain observation. This page accepts it only after finalized RPC reads match devnet&apos;s identity and every exact Loader link, and it reads each role&apos;s deployment slot live from ProgramData rather than trusting the one it shipped with. Programs here are upgraded in place at permanent addresses, so a slot that has moved forward is reported, not refused. That match does not prove that a route is executable. Each route must still authenticate its own release, accounts, state, and packet before you use it.</p></div><div className="operator-counts"><article><strong>{counts.constructible}</strong><span>browser constructors</span></article><article><strong>{counts.request}</strong><span>operator-only constructors</span></article><article><strong>{counts.blocked}</strong><span>missing execution seams</span></article></div></section>

    <form className="operator-inspector" onSubmit={inspectDeployment}>
      <header><span>01</span><div><h2>Reacquire the multiprogram deployment</h2><p>Load the published devnet coordinates, or enter your own. The devnet preset must match live finalized Loader identity before this page calls it current, and its deployment slots are read from the chain rather than shipped. A custom set is only an input and receives no checked-deployment verdict.</p><div className="direct-actions"><button type="button" className="secondary-action" onClick={loadLiveDevnetPreset}>Use checked live-devnet preset</button><Anchor href="/release">Inspect the full route release →</Anchor></div></div></header>
      <div className="operator-coordinate-grid"><label className="wide"><span>Finalized RPC endpoint</span><input value={endpoint} onChange={(event) => updateEndpoint(event.target.value)} /></label>{OPERATOR_ROLES.map((role) => <label key={role}><span>{role} program</span><input required value={coordinates[role]} onChange={(event) => updateCoordinate(role, event.target.value)} /></label>)}<label><span>Realm (optional; never supplied by the preset)</span><input value={coordinates.realm} onChange={(event) => updateCoordinate('realm', event.target.value)} /></label><label><span>Market (optional; never supplied by the preset)</span><input value={coordinates.market} onChange={(event) => updateCoordinate('market', event.target.value)} /></label></div>
      <button type="submit" disabled={discovery.kind === 'loading'}>{discovery.kind === 'loading' ? 'Reading finalized state…' : 'Inspect chain-observed surface'}</button><p className="direct-status" aria-live="polite">{discovery.kind === 'ready' ? `${discovery.snapshot.deploymentPreset ? (discovery.snapshot.deploymentPreset.upgradedSinceRecord.length === 0 ? 'Checked devnet preset matched finalized chain state' : `Checked devnet preset: every Loader identity matched, and ${discovery.snapshot.deploymentPreset.upgradedSinceRecord.join(', ')} ${discovery.snapshot.deploymentPreset.upgradedSinceRecord.length === 1 ? 'has' : 'have'} been upgraded in place since this app was built`) : 'Custom coordinates observed'} at slot ${discovery.snapshot.observedSlot} · ${discovery.snapshot.roles.length} executable roles${discovery.snapshot.realm ? ` · Realm ${compact(discovery.snapshot.realm.address)}` : ''}${discovery.snapshot.market ? ` · Market ${compact(discovery.snapshot.market.address)}` : ''} · route-specific release preflight is still required` : discovery.message}</p>
      {discovery.kind === 'ready' && <div className="operator-role-grid">{discovery.snapshot.roles.map((role) => <article key={role.role}><span>{role.role}</span><strong>{compact(role.address)}</strong><small>{role.dataBytes} data bytes · owner {compact(role.owner)}</small></article>)}</div>}
    </form>

    <section className="operator-wave"><header><span>02</span><div><h2>Constructor readiness map</h2><p>“Browser unsigned” means a browser constructor exists but still needs its own release preflight. “Browser wallet” means the page can explicitly sign, submit, recover, and verify that named action. “Rust unsigned” means the constructor remains in the operator tooling. “Awaiting production” names a missing execution seam.</p></div></header><div className="operator-family-grid">{groups.map((group) => <article key={group.family}><h3>{group.family}</h3>{group.actions.map((workflow) => <div className="operator-action" key={workflow.id}><span className={`operator-status ${workflow.implementation}`}>{workflow.implementation.replaceAll('-', ' ')}</span><strong>{workflow.action}</strong><p>{workflow.exactBoundary}</p>{workflow.workspace && <Anchor href={workflow.workspace}>{workflow.implementation === 'browser-unsigned' ? 'Open exact preflight' : workflow.implementation === 'browser-wallet' ? 'Open wallet flow' : 'Inspect current boundary'} →</Anchor>}</div>)}</article>)}</div></section>

    <section className="operator-handoff"><header><span>03</span><div><h2>Inspect and export an unsigned transaction</h2><p>Paste bytes built by an accepted workspace. The console rejects signed or oversized packets, resolves active lookup tables, and reacquires every message account before export.</p></div></header><div className="operator-handoff-grid"><form onSubmit={inspectPacket}><label><span>Unsigned transaction · base64 — exported by another console&apos;s Export button, or by the operator tooling</span><textarea required value={unsignedText} onChange={(event) => setUnsignedText(event.target.value.trim())} /></label><button type="submit">Reacquire packet dependencies</button><p className="direct-status" aria-live="polite">{packetStatus}</p></form><aside><span className="panel-label">External identity boundary</span><h3>{wallet ? compact(wallet) : 'No wallet identity read'}</h3><WalletDirectory directory={wallets} onConnected={adoptIdentity} /><p>{walletStatus}</p>{packet && <><dl><div><dt>SHA-256</dt><dd>{packet.inspection.digestHex.slice(0, 24)}…</dd></div><div><dt>Wire / instructions</dt><dd>{packet.inspection.wireBytes} B / {packet.inspection.instructionCount}</dd></div><div><dt>Signatures required</dt><dd>{packet.inspection.requiredSignatures}</dd></div><div><dt>Resolved accounts</dt><dd>{packet.report.dependencies.length}</dd></div><div><dt>Lookup tables</dt><dd>{packet.inspection.lookupTables.length}</dd></div></dl><button type="button" disabled={packet.report.missing.length > 0 || packet.report.nonExecutablePrograms.length > 0} onClick={downloadPacket}>Download exact unsigned bytes</button></>}</aside></div></section>
  </main>;
}
