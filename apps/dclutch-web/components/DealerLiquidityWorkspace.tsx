'use client';

import Link from 'next/link';
import { FormEvent, useState } from 'react';

import {
  type DirectHotRouteCoordinateV3,
} from '@/lib/directHotChain';
import {
  type DealerEquityRouteInspectionV3,
  type DealerEquityRouteManifestV3,
  inspectDealerEquityRouteV3,
} from '@/lib/dealerEquityChain';
import { type DealerEquityTransactionPlanV3, compileDealerEquityTransactionV3 } from '@/lib/dealerEquityV3';
import { CHECKED_INFRASTRUCTURE_BYTES_V1 } from '@/lib/infrastructure';
import { SolanaRpcClient } from '@/lib/rpc';
import {
  type WalletSignedTransactionV1,
  requestReadonlyWalletIdentityV1,
  requestWalletTransactionSignatureV1,
} from '@/lib/walletHandoff';

const FIXED_ROLES = Object.freeze([
  'Market', 'Dealer root', 'Manifest raw', 'Manifest staging', 'ProgramSet raw', 'ProgramSet staging',
  'Descriptor raw', 'Descriptor staging', 'Config raw', 'Config staging', 'AccountProfile raw', 'AccountProfile staging',
  'RequestProfile raw', 'RequestProfile staging', 'Transition raw', 'Transition staging', 'Effect raw', 'Effect staging',
  'Lifecycle raw', 'Lifecycle staging', 'Strategy raw', 'Strategy staging', 'Activation cache', 'Core program',
  'Core ProgramData', 'Trading program', 'Trading ProgramData', 'Registry program', 'Rent sysvar', 'Instructions sysvar',
  'Product raw', 'Product staging', 'result domain raw', 'result domain staging', 'portfolio raw', 'portfolio staging',
  'Product basis raw', 'Product basis staging',
]);

type RouteState = Readonly<{ kind: 'idle' | 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; message: string; inspection: DealerEquityRouteInspectionV3 }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'operation refused without a usable reason';
}

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function decodeBase64(text: string, field: string, exactBytes?: number): Uint8Array {
  if (text.trim() !== text || text.length === 0 || text.length % 4 !== 0 || !/^[A-Za-z0-9+/]*={0,2}$/.test(text)) {
    throw new Error(`${field} must be canonical base64 text`);
  }
  let binary: string;
  try { binary = atob(text); } catch { throw new Error(`${field} is not valid base64`); }
  if (exactBytes !== undefined && binary.length !== exactBytes) throw new Error(`${field} must decode to exactly ${exactBytes} bytes`);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

function coordinate(value: unknown, field: string): DirectHotRouteCoordinateV3 {
  if (value === null || typeof value !== 'object') throw new Error(`${field} is not an account coordinate`);
  const candidate = value as Record<string, unknown>;
  if (typeof candidate.address !== 'string' || typeof candidate.isSigner !== 'boolean' || typeof candidate.isWritable !== 'boolean') {
    throw new Error(`${field} must name address, isSigner, and isWritable`);
  }
  return Object.freeze({ address: candidate.address, isSigner: candidate.isSigner, isWritable: candidate.isWritable });
}

function parseManifest(text: string, checkedInfrastructure: Uint8Array | null): DealerEquityRouteManifestV3 {
  let value: unknown;
  try { value = JSON.parse(text); } catch { throw new Error('Dealer route manifest is not valid JSON'); }
  if (value === null || typeof value !== 'object') throw new Error('Dealer route manifest must be one object');
  const input = value as Record<string, unknown>;
  if (typeof input.payer !== 'string' || !Array.isArray(input.fixedAccounts) || !Array.isArray(input.strategyAccounts)
      || !Array.isArray(input.runtimeAccounts) || !Array.isArray(input.lookupTables)
      || input.lookupTables.some((entry) => typeof entry !== 'string')) throw new Error('Dealer route manifest has the wrong exact field types');
  return Object.freeze({
    payer: input.payer,
    fixedAccounts: Object.freeze(input.fixedAccounts.map((entry, index) => coordinate(entry, `fixed account ${index}`))),
    strategyAccounts: Object.freeze(input.strategyAccounts.map((entry, index) => coordinate(entry, `strategy account ${index}`))),
    runtimeAccounts: Object.freeze(input.runtimeAccounts.map((entry, index) => coordinate(entry, `runtime account ${index}`))),
    lookupTables: Object.freeze(input.lookupTables as string[]), checkedInfrastructure,
  });
}

function scaffold(): string {
  return JSON.stringify({
    payer: '',
    fixedAccounts: FIXED_ROLES.map((role, index) => ({ role, address: '', isSigner: false, isWritable: index === 1 })),
    strategyAccounts: [], runtimeAccounts: [], lookupTables: [],
  }, null, 2);
}

function short(value: string): string {
  return value.length <= 18 ? value : `${value.slice(0, 9)}…${value.slice(-7)}`;
}

export default function DealerLiquidityWorkspace() {
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [routeText, setRouteText] = useState(scaffold);
  const [requestText, setRequestText] = useState('');
  const [infrastructureText, setInfrastructureText] = useState('');
  const [routeState, setRouteState] = useState<RouteState>({ kind: 'idle', message: 'No Dealer request or chain state has been read.' });
  const [plan, setPlan] = useState<DealerEquityTransactionPlanV3 | null>(null);
  const [buildStatus, setBuildStatus] = useState('Authenticate one exact action-selected route before transaction construction.');
  const [wallet, setWallet] = useState('');
  const [walletStatus, setWalletStatus] = useState('No wallet identity has been requested.');
  const [signed, setSigned] = useState<WalletSignedTransactionV1 | null>(null);
  const inspection = routeState.kind === 'ready' ? routeState.inspection : null;

  async function inspect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setPlan(null); setSigned(null); setRouteState({ kind: 'loading', message: 'Reacquiring Dealer state and every selected artifact at finalized commitment…' });
    try {
      const request = decodeBase64(requestText, 'Dealer request');
      const checked = infrastructureText === '' ? null : decodeBase64(infrastructureText, 'checked infrastructure', CHECKED_INFRASTRUCTURE_BYTES_V1);
      const next = await inspectDealerEquityRouteV3(new SolanaRpcClient(endpoint), parseManifest(routeText, checked), request);
      setRouteState({
        kind: 'ready', inspection: next,
        message: next.checkedOuter.status === 'checked'
          ? `Recognized executable Dealer ${next.request.action}/P${next.request.signedPositionCount} at finalized slot ${next.observedSlot}.`
          : `State is internally joined but execution remains unavailable: ${next.checkedOuter.reason}`,
      });
    } catch (error) { setRouteState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` }); }
  }

  async function build() {
    if (inspection === null) return;
    setPlan(null); setSigned(null);
    try {
      const next = await compileDealerEquityTransactionV3(inspection.route, inspection.request.bytes);
      setPlan(next);
      setBuildStatus(`Unsigned ${next.request.action}/P${next.request.signedPositionCount} v0 transaction: ${next.wireBytes.length} / 1232 bytes · ${next.loadedAddresses} ALT addresses · ${next.accountCount} Hot accounts.`);
    } catch (error) { setBuildStatus(`Refused: ${errorMessage(error)}`); }
  }

  async function connectWallet() {
    try {
      const identity = await requestReadonlyWalletIdentityV1(window.solana);
      setWallet(identity.address); setWalletStatus(`${identity.address} · no signature requested yet`);
    } catch (error) { setWalletStatus(`Refused: ${errorMessage(error)}`); }
  }

  async function signTransaction() {
    if (plan === null || inspection === null) return;
    try {
      const next = await requestWalletTransactionSignatureV1(window.solana, plan.transaction, inspection.route.payer);
      setSigned(next); setWalletStatus(next.complete ? 'Fee-payer signature complete. Nothing has been submitted.' : 'Wallet added its authorized signature; more signatures remain.');
    } catch (error) { setWalletStatus(`Refused: ${errorMessage(error)}`); }
  }

  function downloadPacket() {
    if (plan === null) return;
    const wire = signed?.wireBytes ?? plan.wireBytes;
    const blob = new Blob([wire as BlobPart], { type: 'application/octet-stream' });
    const link = document.createElement('a');
    link.href = URL.createObjectURL(blob);
    link.download = `dclutch-dealer-${plan.request.action}-p${plan.request.signedPositionCount}-${signed === null ? 'unsigned' : 'wallet-signed'}-${wire.length}.bin`;
    link.click(); URL.revokeObjectURL(link.href);
  }

  return <main className="product-shell trade-v3-shell">
    <header className="product-nav"><Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link><nav><Link href="/create">Create</Link><Link href="/trade">Trade</Link><Link className="active" href="/liquidity">Liquidity</Link><Link href="/release">Release</Link></nav><span className="preview-control"><i className="preview-dot" />local / user-selected RPC</span></header>

    <section className="trade-v3-hero"><div><p className="eyebrow">Dealer · junior equity · six executable shapes</p><h1>Liquidity is a residual.<br /><em>Backed by current custody.</em></h1><p>Contribute or redeem scenario-solvent Dealer equity through the exact action/P-selected Hot route. The browser treats every pasted address and request byte as hostile until finalized Core, Registry, Product, LP, obligation, artifact, and deployment state rejoin.</p></div><aside><span>Executable successor</span><strong>{inspection?.checkedOuter.status === 'checked' ? `${inspection.request.action} · P${inspection.request.signedPositionCount}` : 'fail closed'}</strong><p>Selectors 1–6 only. LP open/close and scenario trading remain hidden until their production outer routes are complete.</p></aside></section>

    <form className="trade-v3-card route-card" onSubmit={(event) => void inspect(event)}>
      <header><span>01</span><div><h2>Authenticate one canonical Dealer request and route</h2><p>The request must come from the chain-derived Dealer equity constructor. The route map carries coordinates only; it cannot author Market, LP, obligation, Product width, release, revision, or authority.</p></div></header>
      <div className="trade-v3-route-grid"><label><span>Finalized RPC endpoint</span><input type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label><label><span>{CHECKED_INFRASTRUCTURE_BYTES_V1.toLocaleString()}-byte checked infrastructure · base64</span><textarea value={infrastructureText} onChange={(event) => setInfrastructureText(event.target.value.trim())} /></label><label><span>Canonical Dealer equity request · base64</span><textarea required spellCheck={false} value={requestText} onChange={(event) => setRequestText(event.target.value.trim())} /></label><label className="route-json"><span>Exact Hot38 + admitted-AOT + runtime route manifest · JSON</span><textarea required spellCheck={false} value={routeText} onChange={(event) => setRouteText(event.target.value)} /></label></div>
      <button disabled={routeState.kind === 'loading'}>{routeState.kind === 'loading' ? 'Reading finalized Dealer route…' : 'Authenticate Dealer route'}</button><p className="direct-status" aria-live="polite">{routeState.message}</p>
      {inspection && <div className="trade-v3-evidence"><article><span>Action / Claims frame</span><strong>{inspection.request.action} · P{inspection.request.signedPositionCount}</strong><small>selector {inspection.request.selector}</small></article><article><span>Outcome width / shares</span><strong>{inspection.request.width} / {inspection.request.shares.toString()}</strong><small>Product + request joined</small></article><article><span>LP owner</span><strong>{short(inspection.request.lpOwner)}</strong><small>canonical LP PDA</small></article><article><span>Descriptor → strategy</span><strong>{inspection.selectedProgramDigest.slice(0, 8)}… → {inspection.strategyDigest.slice(0, 8)}…</strong><small>admitted AOT</small></article></div>}
    </form>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>Construct the exact unsigned liquidity transaction</h2><p>One immutable Hot envelope carries the complete Dealer request. Runtime accounts are checked against the selected AccountProfile; lookup tables are reacquired and packet size is measured after v0 compilation.</p></div></header>
      <button type="button" disabled={inspection === null} onClick={() => void build()}>Build exact unsigned v0 transaction</button><p className="direct-status" aria-live="polite">{buildStatus}</p>
      {inspection && <div className="direct-output"><dl><div><dt>Market / root</dt><dd>{inspection.request.market}<br />{inspection.request.childRoot}</dd></div><div><dt>Obligation / LP revision</dt><dd>{inspection.request.obligationRevision.toString()} / {inspection.request.lpRevision.toString()}</dd></div><div><dt>Collateral / shares</dt><dd>{inspection.request.collateral.toString()} / {inspection.request.shares.toString()}</dd></div><div><dt>Request expiry</dt><dd>slot {inspection.request.expiresAt.toString()} · observed {inspection.observedSlot}</dd></div></dl></div>}
    </section>

    <section className="trade-v3-card signing-card">
      <header><span>03</span><div><h2>Wallet handoff and packet export</h2><p>Connecting reads identity only. Signing is an explicit separate action, the wallet may not rewrite the message, and submission remains outside this workbench.</p></div></header>
      <div className="signing-grid"><article><span>Wallet identity</span><strong>{wallet || 'not connected'}</strong><button type="button" onClick={() => void connectWallet()}>Connect identity</button><p>{walletStatus}</p></article><article><span>Unsigned / signed packet</span><strong>{plan ? `${plan.wireBytes.length} bytes · ${plan.loadedAddresses} ALT` : 'no transaction built'}</strong><button type="button" disabled={plan === null} onClick={() => void signTransaction()}>Sign as transaction payer</button><button type="button" disabled={plan === null} onClick={downloadPacket}>Download exact packet</button><p>{signed ? `${signed.complete ? 'Complete' : 'Partial'} signature set · ${signed.wireBytes.length} bytes. Export it to an external submitter.` : 'No transaction signature requested.'}</p></article></div>
      {plan && <details className="trade-v3-bytes"><summary>Exact transaction material</summary><dl><div><dt>Required signer</dt><dd>{plan.requiredSigners.join(', ')}</dd></div><div><dt>Wire bytes · base64</dt><dd>{base64(signed?.wireBytes ?? plan.wireBytes)}</dd></div><div><dt>Request bytes · base64</dt><dd>{base64(plan.request.bytes)}</dd></div></dl></details>}
    </section>

    <footer className="product-footer"><span>Six real equity routes · no synthetic liquidity state</span><span>No automatic wallet request · no submission path</span></footer>
  </main>;
}
