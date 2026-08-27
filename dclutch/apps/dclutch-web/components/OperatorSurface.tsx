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

type Discovery = Readonly<{ kind: 'idle' | 'loading' | 'error'; message: string }> | Readonly<{ kind: 'ready'; snapshot: OperatorSurfaceSnapshotV1 }>;
type Packet = Readonly<{ inspection: UnsignedTransactionInspectionV1; report: UnsignedTransactionChainReportV1 }>;

function reason(error: unknown): string { return error instanceof Error ? error.message : 'operation refused without a usable reason'; }
function compact(value: string): string { return value.length > 22 ? `${value.slice(0, 8)}…${value.slice(-7)}` : value; }
function familyGroups(): ReadonlyArray<Readonly<{ family: CapabilityFamily; actions: ReadonlyArray<CapabilityActionV1> }>> {
  const order: CapabilityFamily[] = ['Release', 'Creation', 'Direct', 'Source', 'Series', 'General', 'Dealer', 'Claims'];
  return order.map((family) => Object.freeze({ family, actions: CAPABILITY_ACTIONS_V1.filter((workflow) => workflow.family === family) }));
}

export default function OperatorSurface() {
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [coordinates, setCoordinates] = useState<Record<string, string>>(() => Object.fromEntries([...OPERATOR_ROLES, 'realm', 'market'].map((role) => [role, ''])));
  const [discovery, setDiscovery] = useState<Discovery>({ kind: 'idle', message: 'No chain state has been read.' });
  const [unsignedText, setUnsignedText] = useState('');
  const [packet, setPacket] = useState<Packet | null>(null);
  const [packetStatus, setPacketStatus] = useState('Paste an unsigned transaction emitted by one accepted workflow.');
  const [wallet, setWallet] = useState('');
  const [walletStatus, setWalletStatus] = useState('Wallet identity is optional; signing and submission are absent from this surface.');
  const wallets = useWalletDirectoryV1();
  const groups = useMemo(() => familyGroups(), []);
  const counts = useMemo(() => ({
    constructible: CAPABILITY_ACTIONS_V1.filter((workflow) => workflow.implementation === 'browser-unsigned').length,
    request: CAPABILITY_ACTIONS_V1.filter((workflow) => workflow.implementation === 'rust-unsigned').length,
    blocked: CAPABILITY_ACTIONS_V1.filter((workflow) => workflow.implementation === 'awaiting-production').length,
  }), []);

  function updateCoordinate(role: string, value: string) {
    setCoordinates((current) => ({ ...current, [role]: value.trim() }));
  }

  async function inspectDeployment(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setDiscovery({ kind: 'loading', message: 'Reacquiring all six executable roles and the optional Market at one finalized floor…' });
    try {
      const client = new SolanaRpcClient(endpoint);
      const snapshot = await acquireOperatorSurfaceV1(client, coordinates as OperatorCoordinatesV1);
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
    setWallet(address); setWalletStatus(`${address} · connected identity only; no signature requested`);
  }

  return <main className="product-shell operator-shell">
    <header className="product-nav"><Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link><nav><Link href="/trade">Direct</Link><Link href="/resolution">Resolution</Link><Link href="/redeem">Redeem</Link><Link href="/general">General</Link><Link href="/release">Release</Link><Link className="active" href="/operate">Operate</Link></nav><span className="preview-control"><i className="preview-dot" />unsigned only</span></header>
    <section className="operator-hero"><div><p className="eyebrow">One chain truth · every family</p><h1>Operate what exists.<br /><em>Refuse what does not.</em></h1><p>This console reacquires exact finalized programs, Market ownership, lookup tables, and transaction dependencies. It exports unsigned bytes. It never fills a missing ABI with frontend authority.</p></div><div className="operator-counts"><article><strong>{counts.constructible}</strong><span>browser-constructible actions</span></article><article><strong>{counts.request}</strong><span>exact request / Rust routes</span></article><article><strong>{counts.blocked}</strong><span>honestly waiting on ABI seams</span></article></div></section>

    <form className="operator-inspector" onSubmit={inspectDeployment}>
      <header><span>01</span><div><h2>Reacquire the multiprogram deployment</h2><p>All program identities are inputs until Registry activation and Market discovery can supply them. They must exist, be executable, and be distinct.</p></div></header>
      <div className="operator-coordinate-grid"><label className="wide"><span>Finalized RPC endpoint</span><input value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label>{OPERATOR_ROLES.map((role) => <label key={role}><span>{role} program</span><input required value={coordinates[role]} onChange={(event) => updateCoordinate(role, event.target.value)} /></label>)}<label><span>Realm (optional)</span><input value={coordinates.realm} onChange={(event) => updateCoordinate('realm', event.target.value)} /></label><label><span>Market (optional)</span><input value={coordinates.market} onChange={(event) => updateCoordinate('market', event.target.value)} /></label></div>
      <button type="submit" disabled={discovery.kind === 'loading'}>{discovery.kind === 'loading' ? 'Reading finalized state…' : 'Inspect chain-observed surface'}</button><p className="direct-status" aria-live="polite">{discovery.kind === 'ready' ? `Observed at slot ${discovery.snapshot.observedSlot} · ${discovery.snapshot.roles.length} executable roles${discovery.snapshot.realm ? ` · Realm ${compact(discovery.snapshot.realm.address)}` : ''}${discovery.snapshot.market ? ` · Market ${compact(discovery.snapshot.market.address)}` : ''} · unrecognized until route-specific release preflight` : discovery.message}</p>
      {discovery.kind === 'ready' && <div className="operator-role-grid">{discovery.snapshot.roles.map((role) => <article key={role.role}><span>{role.role}</span><strong>{compact(role.address)}</strong><small>{role.dataBytes} data bytes · owner {compact(role.owner)}</small></article>)}</div>}
    </form>

    <section className="operator-wave"><header><span>02</span><div><h2>The executable workflow wave</h2><p>“Browser unsigned” means an exact browser constructor exists but must still pass its own release preflight. “Rust unsigned” is production operator logic not yet duplicated into TypeScript. “Awaiting production” names a missing executable join.</p></div></header><div className="operator-family-grid">{groups.map((group) => <article key={group.family}><h3>{group.family}</h3>{group.actions.map((workflow) => <div className="operator-action" key={workflow.id}><span className={`operator-status ${workflow.implementation}`}>{workflow.implementation.replaceAll('-', ' ')}</span><strong>{workflow.action}</strong><p>{workflow.exactBoundary}</p>{workflow.workspace && <Link href={workflow.workspace}>{workflow.implementation === 'browser-unsigned' ? 'Open exact preflight' : 'Inspect current boundary'} →</Link>}</div>)}</article>)}</div></section>

    <section className="operator-handoff"><header><span>03</span><div><h2>Inspect and export an unsigned transaction</h2><p>Paste bytes built by an accepted workspace. The console rejects signed or oversized packets, resolves active lookup tables, and reacquires every message account before export.</p></div></header><div className="operator-handoff-grid"><form onSubmit={inspectPacket}><label><span>Unsigned versioned transaction · base64</span><textarea required value={unsignedText} onChange={(event) => setUnsignedText(event.target.value.trim())} /></label><button type="submit">Reacquire packet dependencies</button><p className="direct-status" aria-live="polite">{packetStatus}</p></form><aside><span className="panel-label">External identity boundary</span><h3>{wallet ? compact(wallet) : 'No wallet identity read'}</h3><WalletDirectory directory={wallets} purpose="identity only" onConnected={adoptIdentity} /><p>{walletStatus}</p>{packet && <><dl><div><dt>SHA-256</dt><dd>{packet.inspection.digestHex.slice(0, 24)}…</dd></div><div><dt>Wire / instructions</dt><dd>{packet.inspection.wireBytes} B / {packet.inspection.instructionCount}</dd></div><div><dt>Signatures required</dt><dd>{packet.inspection.requiredSignatures}</dd></div><div><dt>Resolved accounts</dt><dd>{packet.report.dependencies.length}</dd></div><div><dt>Lookup tables</dt><dd>{packet.inspection.lookupTables.length}</dd></div></dl><button type="button" disabled={packet.report.missing.length > 0 || packet.report.nonExecutablePrograms.length > 0} onClick={downloadPacket}>Download exact unsigned bytes</button></>}</aside></div></section>
  </main>;
}
