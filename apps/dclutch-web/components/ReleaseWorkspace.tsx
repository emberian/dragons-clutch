'use client';

import Link from 'next/link';
import { FormEvent, useState } from 'react';

import {
  REGISTRY_MAX_COMPUTE_UNITS,
  REGISTRY_ROLES,
  decodeManifestBase64,
  prepareRegistryActivation,
  prepareRegistryReauthentication,
  type RegistryActivationModeV1,
  type RegistryActivationPlanV1,
  type RegistryReauthenticationPlanV1,
  type RegistryRole,
} from '@/lib/releaseRegistry';
import {
  CHECKED_INFRASTRUCTURE_BYTES_V1,
  inspectProtocolInfrastructureV1,
  type ProtocolInfrastructureInspectionV1,
} from '@/lib/infrastructure';
import { SolanaRpcClient } from '@/lib/rpc';

const LOCAL_RPC = 'http://127.0.0.1:8899';

function message(error: unknown): string { return error instanceof Error ? error.message : 'unknown release refusal'; }
function compact(value: string): string { return value.length > 20 ? `${value.slice(0, 9)}…${value.slice(-7)}` : value; }
function base64(bytes: Uint8Array): string {
  let binary = ''; for (let offset = 0; offset < bytes.length; offset += 16_384) binary += String.fromCharCode(...bytes.slice(offset, offset + 16_384)); return btoa(binary);
}
function blankManifests(): Record<RegistryRole, string> { return Object.fromEntries(REGISTRY_ROLES.map((role) => [role, ''])) as Record<RegistryRole, string>; }

const ACTIVATION_MODE_TEXT: Readonly<Record<RegistryActivationModeV1, string>> = {
  absent: 'no cache account exists yet — the first transaction creates it',
  partial: 'the cache exists and holds a strict subset of its five roles',
  complete: 'the cache holds all five roles and is readable',
};

function ActivationResult({ plan }: Readonly<{ plan: RegistryActivationPlanV1 }>) {
  return <div className="direct-output release-output" data-testid="activation-result">
    <dl>
      <div><dt>Finalized observation</dt><dd>slot {plan.observedSlot}</dd></div>
      <div><dt>Observed cache state</dt><dd>{plan.mode} · {ACTIVATION_MODE_TEXT[plan.mode]} · {plan.cacheRentDebitLamports} lamports rent debit</dd></div>
      <div><dt>Roles already admitted</dt><dd>{plan.activatedRoles.length === 0 ? 'none' : plan.activatedRoles.join(', ')} · {plan.remainingRoles.length} remaining</dd></div>
      <div><dt>Execution release set</dt><dd>{plan.evidence.releaseSet.id}</dd></div>
      <div><dt>Checked evidence</dt><dd>{plan.evidence.checkedId}</dd></div>
      <div><dt>Activation cache</dt><dd>{plan.cache}</dd></div>
      <div><dt>Walk shape</dt><dd>{plan.packets.length} unsigned v0 packets · one role each · 10-account Registry frame</dd></div>
      <div><dt>Compute limit</dt><dd>{plan.computeUnitLimit.toLocaleString()} CU per transaction</dd></div>
      <div><dt>Total ELF bytes hashed</dt><dd>{plan.totalElfBytesHashed.toLocaleString()} across the whole walk, never in one transaction</dd></div>
      <div><dt>External signer</dt><dd>{plan.packets[0]?.requiredSigners.join(', ') ?? plan.payer}</dd></div>
    </dl>
    <p className="direct-status">Activation admits one role per transaction. Whole-ELF hashing costs about one compute unit per two bytes, so a single five-role instruction exceeds the chain maximum outright; the Registry program accepts exactly ten accounts and one named role, and refuses any other frame before reading a byte.</p>
    <div className="registered-state-grid release-role-grid">
      {plan.packets.map((packet) => <article className="registered-state-card" key={packet.role} data-testid={`activation-packet-${packet.role}`}>
        <span className="eyebrow">{packet.role} role · {packet.alreadyActivated ? 'already admitted' : 'not yet admitted'}</span><h3>{compact(packet.addresses.program)}</h3>
        <p>artifact {compact(plan.evidence.releaseSet.roles[packet.role].artifactReleaseId)} · semantic {compact(plan.evidence.artifacts[packet.role].semanticReleaseId)}</p>
        <p>ProgramData {compact(packet.addresses.programData)} · slot {plan.evidence.artifacts[packet.role].deploymentSlot.toString()}</p>
        <p>{packet.wireBytes.length} / 1232 bytes · {packet.elfBytesHashed.toLocaleString()} ELF bytes hashed by this transaction</p>
        <label><span>{packet.role} unsigned v0 transaction · base64</span><textarea readOnly value={base64(packet.wireBytes)} /></label>
      </article>)}
    </div>
    {plan.mode === 'complete'
      ? <p className="direct-status">Every role is already admitted. Re-sending any packet is idempotent on chain and still pays that role&apos;s full ELF hash, so a cheapest walk-up sends none of them.</p>
      : <p className="direct-status">Send the {plan.remainingRoles.length} packet{plan.remainingRoles.length === 1 ? '' : 's'} whose role is not yet admitted, in any order. Each is separately signed and separately submitted.</p>}
    <p className="direct-refusal"><strong>No signing, submission, deployment, or account mutation occurred.</strong> The payer signature remains an external boundary and the finalized blockhash will expire.</p>
  </div>;
}

function ReauthenticationResult({ plan }: Readonly<{ plan: RegistryReauthenticationPlanV1 }>) {
  return <div className="direct-output release-output" data-testid="reauth-result"><dl>
    <div><dt>Role / observation</dt><dd>{plan.role} · slot {plan.observedSlot}</dd></div>
    <div><dt>Release set</dt><dd>{plan.releaseSetId}</dd></div>
    <div><dt>Artifact release</dt><dd>{plan.artifactReleaseId}</dd></div>
    <div><dt>Current executable</dt><dd>{plan.artifact.program}</dd></div>
    <div><dt>ProgramData / deployment</dt><dd>{plan.artifact.programData} · slot {plan.artifact.deploymentSlot.toString()}</dd></div>
    <div><dt>Unsigned v0 packet</dt><dd>{plan.wireBytes.length} / 1232 bytes · 3-account Registry frame</dd></div>
    <div><dt>External signer</dt><dd>{plan.requiredSigners.join(', ')}</dd></div>
  </dl><label><span>Unsigned v0 transaction · base64</span><textarea readOnly value={base64(plan.wireBytes)} /></label>
    <p className="direct-refusal"><strong>No signing or submission occurred.</strong> Reauthentication is read-only onchain; the fee-payer signature remains external.</p></div>;
}

function InfrastructureResult({ report }: Readonly<{ report: ProtocolInfrastructureInspectionV1 }>) {
  const recognized = report.recognition.kind === 'supplied-manifest-match';
  return <div className="direct-output release-output" data-testid="infrastructure-result">
    <dl>
      <div><dt>Finalized observation</dt><dd>slot {report.observedSlot}</dd></div>
      <div><dt>Recognition</dt><dd>{recognized ? 'supplied manifest matches' : 'internally consistent / unrecognized'}</dd></div>
      <div><dt>Execution release set</dt><dd>{report.executionReleaseSetId}</dd></div>
      <div><dt>Core-owned profile</dt><dd>{report.profilePda}</dd></div>
      <div><dt>Profile SHA-256</dt><dd>{report.profileDigest}</dd></div>
      {recognized && <div><dt>Checked infrastructure</dt><dd>{report.recognition.checkedInfrastructureId}</dd></div>}
    </dl>
    <div className="registered-state-grid release-role-grid">
      {(['core', 'registry', 'rent'] as const).map((role) => <article className="registered-state-card" key={role}>
        <span className="eyebrow">{role} · immutable</span><h3>{compact(report[role].program)}</h3>
        <p>artifact {compact(report[role].artifactReleaseId)} · semantic {compact(report[role].semanticReleaseId)}</p>
        <p>ProgramData {compact(report[role].programData)} · slot {report[role].deploymentSlot}</p>
        <p>ELF {compact(report[role].elfDigest)}</p>
      </article>)}
    </div>
    <p className="direct-refusal"><strong>{recognized ? 'Recognized only by the exact manifest supplied in this inspection.' : 'No checked manifest was supplied, so this chain is not recognized.'}</strong> Internal consistency is not an official-deployment claim.</p>
  </div>;
}

export default function ReleaseWorkspace() {
  const [endpoint, setEndpoint] = useState(LOCAL_RPC); const [registry, setRegistry] = useState(''); const [payer, setPayer] = useState('');
  const [multiprogram, setMultiprogram] = useState(''); const [manifests, setManifests] = useState(blankManifests);
  const [activationCompute, setActivationCompute] = useState(String(REGISTRY_MAX_COMPUTE_UNITS)); const [activationStatus, setActivationStatus] = useState('No manifest or chain request has been made.'); const [activation, setActivation] = useState<RegistryActivationPlanV1 | null>(null);
  const [cache, setCache] = useState(''); const [role, setRole] = useState<RegistryRole>('trading'); const [reauthCompute, setReauthCompute] = useState('80000'); const [reauthStatus, setReauthStatus] = useState('No cache has been reacquired.'); const [reauth, setReauth] = useState<RegistryReauthenticationPlanV1 | null>(null);
  const [infrastructureManifest, setInfrastructureManifest] = useState(''); const [infrastructureStatus, setInfrastructureStatus] = useState('No infrastructure snapshot has been reacquired.'); const [infrastructure, setInfrastructure] = useState<ProtocolInfrastructureInspectionV1 | null>(null);

  async function buildActivation(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setActivation(null); setActivationStatus('Decoding all six evidence files and reacquiring the exact finalized record, Loader, runtime, cache, rent, and payer state…');
    try {
      const checkedReleases = Object.fromEntries(REGISTRY_ROLES.map((name) => [name, decodeManifestBase64(manifests[name], `${name} checked release`)])) as Record<RegistryRole, Uint8Array>;
      const plan = await prepareRegistryActivation(new SolanaRpcClient(endpoint), { registryProgram: registry, payer, multiprogram: decodeManifestBase64(multiprogram, 'checked multiprogram'), checkedReleases, computeUnitLimit: Number(activationCompute) });
      setActivation(plan); setCache(plan.cache); setActivationStatus(`Ready: ${plan.remainingRoles.length} of ${plan.packets.length} roles remain to admit (observed cache: ${plan.mode}). Every packet is unsigned and none has been submitted.`);
    } catch (error) { setActivationStatus(`Refused: ${message(error)}`); }
  }

  async function buildReauthentication(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setReauth(null); setReauthStatus('Reacquiring the finalized cache and its selected current Loader-v3 deployment…');
    try {
      const plan = await prepareRegistryReauthentication(new SolanaRpcClient(endpoint), { registryProgram: registry, payer, cache, role, computeUnitLimit: Number(reauthCompute) });
      setReauth(plan); setReauthStatus(`Ready: ${role} reauthentication is ${plan.wireBytes.length} bytes. It is unsigned and has not been submitted.`);
    } catch (error) { setReauthStatus(`Refused: ${message(error)}`); }
  }

  async function inspectInfrastructure(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setInfrastructure(null); setInfrastructureStatus('Reacquiring the activation cache, deriving Core and its immutable profile, then authenticating current Registry/Rent records and Loader state…');
    try {
      const checkedManifest = infrastructureManifest.length === 0 ? undefined : decodeManifestBase64(infrastructureManifest, 'checked infrastructure');
      const report = await inspectProtocolInfrastructureV1(new SolanaRpcClient(endpoint), { registryProgram: registry, activationCache: cache, checkedManifest });
      setInfrastructure(report);
      setInfrastructureStatus(report.recognition.kind === 'supplied-manifest-match' ? 'Recognized: every observed infrastructure fact matches the supplied checked manifest.' : 'Internally consistent / unrecognized: no checked manifest was supplied.');
    } catch (error) { setInfrastructureStatus(`Refused: ${message(error)}`); }
  }

  return <main className="product-shell direct-workspace release-workspace">
    <header className="product-nav"><Link className="brand" href="/">dClutch</Link><nav><Link href="/direct">Direct</Link><Link href="/economic">Economic</Link><Link href="/general">General</Link><Link className="active" href="/release">Release</Link><Link href="/explorer">Explorer</Link></nav><span className="preview-control"><i className="preview-dot" />finalized only</span></header>
    <section className="market-heading"><div><div className="market-kicker"><span>five exact roles</span><span>Loader v3</span><span>unsigned packets</span></div><h1>Make executable authority inspectable.</h1><p>Join complete checked-build evidence to finalized Registry records and the code actually loaded by Solana. Construct activation or role reauthentication only when every content identity, PDA, Loader link, account digest, deployment slot, authority, and packet boundary agrees.</p></div></section>
    <section className="direct-card"><div className="direct-card-heading"><span>01</span><div><h2>Shared local boundary</h2><p>The workspace defaults to the local validator. It never reads a wallet; the payer is an explicit public key whose signature remains outside this application.</p></div></div><div className="direct-form-grid"><label><span>Finalized RPC endpoint</span><input value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label><label><span>Registry program</span><input required value={registry} onChange={(event) => setRegistry(event.target.value.trim())} /></label><label><span>External fee payer public key</span><input required value={payer} onChange={(event) => setPayer(event.target.value.trim())} /></label></div></section>
    <form className="direct-card" onSubmit={buildActivation}><div className="direct-card-heading"><span>02</span><div><h2>Activate a checked five-role release</h2><p>Supply the canonical fixed-width multiprogram evidence and all five complete checked-release manifests. The frontend rebuilds the compact artifact authorities, derives finalized record/staging/cache PDAs, and checks current Program and ProgramData bytes before constructing one exact ten-account action per role. Activation admits one role per transaction, so this produces five separately signed packets, not one.</p></div></div>
      <label><span>1,592-byte checked multiprogram · base64</span><textarea required value={multiprogram} onChange={(event) => setMultiprogram(event.target.value.trim())} /></label>
      <div className="release-manifest-grid">{REGISTRY_ROLES.map((name) => <label key={name}><span>{name} complete checked release · base64</span><textarea required value={manifests[name]} onChange={(event) => setManifests((current) => ({ ...current, [name]: event.target.value.trim() }))} /></label>)}</div>
      <div className="direct-form-grid"><label><span>Activation compute-unit limit</span><input inputMode="numeric" value={activationCompute} onChange={(event) => setActivationCompute(event.target.value.trim())} /></label></div>
      <button type="submit">Reacquire finalized authority &amp; build activation</button><p className="direct-status" aria-live="polite">{activationStatus}</p>{activation && <ActivationResult plan={activation} />}
    </form>
    <form className="direct-card" onSubmit={buildReauthentication}><div className="direct-card-heading"><span>03</span><div><h2>Reauthenticate one active role</h2><p>The Registry-owned cache selects the role and artifact. The workspace reacquires that cache, the Registry executable, and the role&apos;s current Program/ProgramData before constructing the exact read-only three-account action.</p></div></div>
      <div className="direct-form-grid"><label><span>Activation cache PDA</span><input required value={cache} onChange={(event) => setCache(event.target.value.trim())} /></label><label><span>Execution role</span><select value={role} onChange={(event) => setRole(event.target.value as RegistryRole)}>{REGISTRY_ROLES.map((name) => <option value={name} key={name}>{name}</option>)}</select></label><label><span>Reauthentication compute-unit limit</span><input inputMode="numeric" value={reauthCompute} onChange={(event) => setReauthCompute(event.target.value.trim())} /></label></div>
      <button type="submit">Reacquire current deployment &amp; build reauthentication</button><p className="direct-status" aria-live="polite">{reauthStatus}</p>{reauth && <ReauthenticationResult plan={reauth} />}
    </form>
    <form className="direct-card" onSubmit={inspectInfrastructure}><div className="direct-card-heading"><span>04</span><div><h2>Inspect immutable protocol infrastructure</h2><p>The activation cache selects Core. Core derives one immutable 144-byte profile selecting exact Registry and Rent programs and artifact releases. This read-only pass reacquires all three current Loader deployments and refuses mutable, stale, substituted, or partially joined state.</p></div></div>
      <div className="direct-form-grid"><label><span>Market-selected activation cache PDA</span><input required value={cache} onChange={(event) => setCache(event.target.value.trim())} /></label></div>
      <label><span>Optional {CHECKED_INFRASTRUCTURE_BYTES_V1.toLocaleString()}-byte checked infrastructure manifest · base64</span><textarea value={infrastructureManifest} onChange={(event) => setInfrastructureManifest(event.target.value.trim())} /></label>
      <button type="submit">Reacquire &amp; inspect immutable chain</button><p className="direct-status" aria-live="polite">{infrastructureStatus}</p>{infrastructure && <InfrastructureResult report={infrastructure} />}
    </form>
    <footer className="product-footer"><span>Chain state and checked files are hostile inputs</span><span>No wallet connector · no submit path</span></footer>
  </main>;
}
