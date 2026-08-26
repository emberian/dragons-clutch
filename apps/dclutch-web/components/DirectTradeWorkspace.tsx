'use client';

import Link from 'next/link';
import { FormEvent, useMemo, useState } from 'react';

import {
  type DirectHotRouteCoordinateV3,
  type DirectHotRouteInspectionV3,
  type DirectHotRouteManifestV3,
  inspectDirectHotRouteV3,
} from '@/lib/directHotChain';
import {
  type CompactIntentV2Input,
  type DirectInlineTransactionPlanV3,
  type SignedDirectIntentV3,
  compileDirectInlineTransactionV3,
  encodeCompactIntentSigningMessageV2,
  previewDirectInlineV3,
} from '@/lib/directInlineV3';
import { CHECKED_INFRASTRUCTURE_BYTES_V1 } from '@/lib/infrastructure';
import { SolanaRpcClient } from '@/lib/rpc';
import {
  type WalletSignedTransactionV1,
  requestReadonlyWalletIdentityV1,
  requestWalletMessageSignatureV1,
  requestWalletTransactionSignatureV1,
} from '@/lib/walletHandoff';

const MAX_U64 = 18_446_744_073_709_551_615n;
const FIXED_ROLES = Object.freeze([
  'Market', 'Direct root', 'Manifest raw', 'Manifest staging', 'ProgramSet raw', 'ProgramSet staging',
  'Descriptor raw', 'Descriptor staging', 'Config raw', 'Config staging', 'AccountProfile raw', 'AccountProfile staging',
  'RequestProfile raw', 'RequestProfile staging', 'Transition raw', 'Transition staging', 'Effect raw', 'Effect staging',
  'Lifecycle raw', 'Lifecycle staging', 'Strategy raw', 'Strategy staging', 'Activation cache', 'Core program',
  'Core ProgramData', 'Trading program', 'Trading ProgramData', 'Registry program', 'Rent sysvar', 'Instructions sysvar',
  'Product raw', 'Product staging', 'result domain raw', 'result domain staging', 'portfolio raw', 'portfolio staging',
  'Product basis raw', 'Product basis staging',
]);

type ParticipantFields = Readonly<{
  maker: string;
  collateral: string;
  nonce: string;
  maximumFill: string;
  limitPrice: string;
  signature: string;
}>;

type FormFields = Readonly<{
  outcome: string;
  lifecycle: '0' | '1';
  validFrom: string;
  validThrough: string;
  fill: string;
  executionPrice: string;
  reviewSlippageBps: string;
  seller: ParticipantFields;
  buyer: ParticipantFields;
}>;

type RouteState = Readonly<{ kind: 'idle' | 'loading' | 'refused'; message: string }>
  | Readonly<{ kind: 'ready'; message: string; inspection: DirectHotRouteInspectionV3 }>;

const EMPTY_PARTICIPANT: ParticipantFields = Object.freeze({ maker: '', collateral: '', nonce: '0', maximumFill: '', limitPrice: '', signature: '' });
const INITIAL_FORM: FormFields = Object.freeze({
  outcome: '0', lifecycle: '0', validFrom: '0', validThrough: String(MAX_U64), fill: '', executionPrice: '', reviewSlippageBps: '100',
  seller: EMPTY_PARTICIPANT, buyer: EMPTY_PARTICIPANT,
});

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'operation refused without a usable reason';
}

function unsigned(value: string, field: string, maximum = MAX_U64): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} must be one canonical unsigned integer`);
  const parsed = BigInt(value);
  if (parsed > maximum) throw new Error(`${field} exceeds its exact width`);
  return parsed;
}

function uint32(value: string, field: string): number {
  return Number(unsigned(value, field, 0xffff_ffffn));
}

function signature(text: string, field: string): Uint8Array {
  if (!/^[0-9a-f]{128}$/.test(text) || /^0+$/.test(text)) throw new Error(`${field} must be one nonzero 64-byte lowercase-hex Ed25519 signature`);
  return Uint8Array.from(text.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function decodeCheckedInfrastructure(text: string): Uint8Array | null {
  if (text.length === 0) return null;
  if (text.trim() !== text || text.length % 4 !== 0 || !/^[A-Za-z0-9+/]*={0,2}$/.test(text)) throw new Error('checked infrastructure must be canonical base64 text');
  let binary: string;
  try { binary = atob(text); } catch { throw new Error('checked infrastructure is not valid base64'); }
  if (binary.length !== CHECKED_INFRASTRUCTURE_BYTES_V1) throw new Error(`checked infrastructure must decode to exactly ${CHECKED_INFRASTRUCTURE_BYTES_V1} bytes`);
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

function parseRouteManifest(text: string, checkedInfrastructure: Uint8Array | null): DirectHotRouteManifestV3 {
  let value: unknown;
  try { value = JSON.parse(text); } catch { throw new Error('route manifest is not valid JSON'); }
  if (value === null || typeof value !== 'object') throw new Error('route manifest must be one object');
  const input = value as Record<string, unknown>;
  if (typeof input.payer !== 'string' || !Array.isArray(input.fixedAccounts) || !Array.isArray(input.strategyAccounts)
      || !Array.isArray(input.runtimeAccounts) || !Array.isArray(input.lookupTables)
      || input.lookupTables.some((entry) => typeof entry !== 'string')) {
    throw new Error('route manifest has the wrong exact field types');
  }
  return Object.freeze({
    payer: input.payer,
    fixedAccounts: Object.freeze(input.fixedAccounts.map((entry, index) => coordinate(entry, `fixed account ${index}`))),
    strategyAccounts: Object.freeze(input.strategyAccounts.map((entry, index) => coordinate(entry, `strategy account ${index}`))),
    runtimeAccounts: Object.freeze(input.runtimeAccounts.map((entry, index) => coordinate(entry, `runtime account ${index}`))),
    lookupTables: Object.freeze(input.lookupTables as string[]),
    checkedInfrastructure,
  });
}

function manifestScaffold(): string {
  return JSON.stringify({
    payer: '',
    fixedAccounts: FIXED_ROLES.map((role, index) => ({ role, address: '', isSigner: false, isWritable: index === 1 })),
    strategyAccounts: [],
    runtimeAccounts: [],
    lookupTables: [],
  }, null, 2);
}

function compactIntent(
  participant: ParticipantFields,
  side: 0 | 1,
  form: FormFields,
  inspection: DirectHotRouteInspectionV3,
): CompactIntentV2Input {
  return Object.freeze({
    side,
    lifecycle: Number(form.lifecycle) as 0 | 1,
    outcome: uint32(form.outcome, 'outcome'),
    market: inspection.route.market,
    generation: inspection.route.generation,
    nonce: unsigned(participant.nonce, `${side === 0 ? 'seller' : 'buyer'} nonce`),
    validFrom: unsigned(form.validFrom, 'valid-from slot'),
    validThrough: unsigned(form.validThrough, 'valid-through slot'),
    maximumFill: unsigned(participant.maximumFill, `${side === 0 ? 'seller' : 'buyer'} maximum fill`),
    limitPrice: unsigned(participant.limitPrice, `${side === 0 ? 'seller' : 'buyer'} limit price`),
    feeBasisPoints: inspection.route.feeBasisPoints,
    collateralAccount: participant.collateral,
  });
}

function signedIntent(
  participant: ParticipantFields,
  side: 0 | 1,
  form: FormFields,
  inspection: DirectHotRouteInspectionV3,
): SignedDirectIntentV3 {
  return Object.freeze({
    maker: participant.maker,
    signature: signature(participant.signature, `${side === 0 ? 'seller' : 'buyer'} signature`),
    intent: compactIntent(participant, side, form, inspection),
  });
}

export default function DirectTradeWorkspace() {
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [routeText, setRouteText] = useState(manifestScaffold);
  const [infrastructureText, setInfrastructureText] = useState('');
  const [routeState, setRouteState] = useState<RouteState>({ kind: 'idle', message: 'No chain state has been read.' });
  const [form, setForm] = useState<FormFields>(INITIAL_FORM);
  const [tradeStatus, setTradeStatus] = useState('Acquire an exact action-selected route before building a transaction.');
  const [plan, setPlan] = useState<DirectInlineTransactionPlanV3 | null>(null);
  const [wallet, setWallet] = useState('');
  const [walletStatus, setWalletStatus] = useState('No wallet identity has been requested.');
  const [signed, setSigned] = useState<WalletSignedTransactionV1 | null>(null);

  const inspection = routeState.kind === 'ready' ? routeState.inspection : null;
  const preview = useMemo(() => {
    if (inspection === null) return null;
    try {
      const seller = Object.freeze({ intent: compactIntent(form.seller, 0, form, inspection) });
      const buyer = Object.freeze({ intent: compactIntent(form.buyer, 1, form, inspection) });
      return previewDirectInlineV3(inspection.route, seller, buyer, unsigned(form.fill, 'fill'), unsigned(form.executionPrice, 'execution price'), BigInt(inspection.observedSlot));
    } catch { return null; }
  }, [form, inspection]);

  function updateParticipant(side: 'seller' | 'buyer', field: keyof ParticipantFields, value: string) {
    setForm((current) => ({ ...current, [side]: { ...current[side], [field]: value.trim() } }));
    setPlan(null); setSigned(null);
  }

  function updateField<K extends keyof Omit<FormFields, 'seller' | 'buyer'>>(field: K, value: FormFields[K]) {
    setForm((current) => ({ ...current, [field]: value }));
    setPlan(null); setSigned(null);
  }

  async function inspectRoute(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setRouteState({ kind: 'loading', message: 'Reacquiring the exact route at one finalized floor…' }); setPlan(null); setSigned(null);
    try {
      const manifest = parseRouteManifest(routeText, decodeCheckedInfrastructure(infrastructureText));
      const next = await inspectDirectHotRouteV3(new SolanaRpcClient(endpoint), manifest);
      setRouteState({
        kind: 'ready', inspection: next,
        message: next.checkedOuter.status === 'checked'
          ? `Recognized immutable Direct hot release at finalized slot ${next.observedSlot}.`
          : `Internally consistent but unrecognized: ${next.checkedOuter.reason}`,
      });
      setForm((current) => ({ ...current, validFrom: next.observedSlot, validThrough: String(BigInt(next.observedSlot) + 150n) }));
    } catch (error) { setRouteState({ kind: 'refused', message: `Refused: ${errorMessage(error)}` }); }
  }

  function buildTransaction(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setPlan(null); setSigned(null);
    if (inspection === null) return;
    try {
      const seller = signedIntent(form.seller, 0, form, inspection);
      const buyer = signedIntent(form.buyer, 1, form, inspection);
      const executionPrice = unsigned(form.executionPrice, 'execution price');
      const midpoint = (seller.intent.limitPrice + buyer.intent.limitPrice) / 2n;
      const deviation = executionPrice >= midpoint ? executionPrice - midpoint : midpoint - executionPrice;
      const reviewBps = midpoint === 0n ? 0n : deviation * 10_000n / midpoint;
      if (reviewBps > unsigned(form.reviewSlippageBps, 'review slippage', 10_000n)) throw new Error(`execution differs from the signed-limit midpoint by ${reviewBps} bps, above the user review tolerance`);
      const next = compileDirectInlineTransactionV3({
        route: inspection.route, seller, buyer,
        fill: unsigned(form.fill, 'fill'), executionPrice, clockSlot: BigInt(inspection.observedSlot),
      });
      setPlan(next);
      setTradeStatus(`Unsigned v0 transaction constructed from the finalized route: ${next.wireBytes.length} / 1232 bytes, ${next.loadedAddresses} ALT addresses.`);
    } catch (error) { setTradeStatus(`Refused: ${errorMessage(error)}`); }
  }

  async function connectWallet() {
    try {
      const identity = await requestReadonlyWalletIdentityV1(window.solana);
      setWallet(identity.address); setWalletStatus(`${identity.address} · no signature requested yet`);
    } catch (error) { setWalletStatus(`Refused: ${errorMessage(error)}`); }
  }

  async function signMaker(side: 'seller' | 'buyer') {
    if (inspection === null) return;
    try {
      const intent = compactIntent(form[side], side === 'seller' ? 0 : 1, form, inspection);
      const next = await requestWalletMessageSignatureV1(window.solana, form[side].maker, encodeCompactIntentSigningMessageV2(intent));
      updateParticipant(side, 'signature', hex(next));
      setWalletStatus(`${side} intent signed by the connected maker after explicit request.`);
    } catch (error) { setWalletStatus(`Refused: ${errorMessage(error)}`); }
  }

  async function signTransaction() {
    if (plan === null || inspection === null) return;
    try {
      const next = await requestWalletTransactionSignatureV1(window.solana, plan.transaction, inspection.route.payer);
      setSigned(next); setWalletStatus(next.complete ? 'Transaction payer signature complete. Nothing has been submitted.' : 'Wallet added one authorized signature; more signatures remain.');
    } catch (error) { setWalletStatus(`Refused: ${errorMessage(error)}`); }
  }

  function downloadPacket() {
    if (plan === null) return;
    const wire = signed?.wireBytes ?? plan.wireBytes;
    const blob = new Blob([wire as BlobPart], { type: 'application/octet-stream' });
    const link = document.createElement('a');
    link.href = URL.createObjectURL(blob);
    link.download = `dclutch-direct-${signed === null ? 'unsigned' : 'wallet-signed'}-${wire.length}.bin`;
    link.click();
    URL.revokeObjectURL(link.href);
  }

  return <main className="product-shell trade-v3-shell">
    <header className="product-nav"><Link className="brand" href="/"><span className="brand-mark">dC</span><span>dClutch</span></Link><nav><Link href="/create">Create</Link><Link className="active" href="/trade">Trade</Link><Link href="/liquidity">Liquidity</Link><Link href="/release">Release</Link></nav><span className="preview-control"><i className="preview-dot" />local / user-selected RPC</span></header>

    <section className="trade-v3-hero"><div><p className="eyebrow">Direct · inline ordinary · runtime width</p><h1>One signed price.<br /><em>One checked route.</em></h1><p>This workbench reads the current Market, Product width, action-selected ProgramSet, finalized interpreters, Loader state, and checked release evidence before it will construct the adjacent native-Ed25519 and Trading instructions.</p></div><aside><span>Execution boundary</span><strong>{inspection?.checkedOuter.status === 'checked' ? 'recognized release' : 'fail closed'}</strong><p>No checked manifest, no executable transaction. Static account lists are never authority.</p></aside></section>

    <form className="trade-v3-card route-card" onSubmit={inspectRoute}>
      <header><span>01</span><div><h2>Acquire the action-selected route</h2><p>The address map is transport, not truth. Every account is reacquired and joined against the current Registry/Core/Trading state.</p></div></header>
      <div className="trade-v3-route-grid"><label><span>Finalized RPC endpoint</span><input type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label><label><span>Optional {CHECKED_INFRASTRUCTURE_BYTES_V1.toLocaleString()}-byte checked infrastructure · base64</span><textarea value={infrastructureText} onChange={(event) => setInfrastructureText(event.target.value.trim())} /></label><label className="route-json"><span>Exact Hot38 + strategy/runtime-suffix route manifest · JSON</span><textarea required spellCheck={false} value={routeText} onChange={(event) => setRouteText(event.target.value)} /></label></div>
      <button disabled={routeState.kind === 'loading'}>{routeState.kind === 'loading' ? 'Reading finalized route…' : 'Authenticate route'}</button><p className="direct-status" aria-live="polite">{routeState.message}</p>
      {inspection && <div className="trade-v3-evidence"><article><span>Outcome width</span><strong>{inspection.route.outcomeCount.toLocaleString()}</strong><small>Product-derived u32</small></article><article><span>Price / fee</span><strong>{inspection.route.priceScale.toString()} / {inspection.route.feeBasisPoints} bps</strong><small>immutable Direct config</small></article><article><span>Program selector</span><strong>{inspection.selectedProgramDigest.slice(0, 16)}…</strong><small>InlineOrdinary action 1</small></article><article><span>Strategy → VM</span><strong>{inspection.strategyDigest.slice(0, 8)}… → {inspection.transitionDigest.slice(0, 8)}…</strong><small>interpreted V2 / V3</small></article></div>}
    </form>

    <form className="trade-v3-card" onSubmit={buildTransaction}>
      <header><span>02</span><div><h2>Price and authorize one atomic fill</h2><p>Both 172-byte messages carry runtime-u32 outcome coordinates. Detached signatures are verified by the adjacent native Ed25519 instruction; the transaction payer is separate.</p></div></header>
      {inspection === null ? <><p className="trade-v3-refusal">Acquire a binding-clean route first. No Market, width, price scale, fee, revision, or authority is fabricated here.</p><button disabled>Build exact unsigned v0 transaction</button></> : <>
        <div className="trade-v3-common"><label><span>Outcome index · below {inspection.route.outcomeCount.toLocaleString()}</span><input inputMode="numeric" value={form.outcome} onChange={(event) => updateField('outcome', event.target.value)} /></label><label><span>Lifecycle</span><select value={form.lifecycle} onChange={(event) => updateField('lifecycle', event.target.value as '0' | '1')}><option value="0">Fill or kill</option><option value="1">Immediate or cancel</option></select></label><label><span>Valid from slot</span><input inputMode="numeric" value={form.validFrom} onChange={(event) => updateField('validFrom', event.target.value)} /></label><label><span>Valid through slot</span><input inputMode="numeric" value={form.validThrough} onChange={(event) => updateField('validThrough', event.target.value)} /></label></div>
        <div className="trade-v3-participants">{(['seller', 'buyer'] as const).map((side) => <section key={side}><div className="participant-title"><span>{side === 'seller' ? 'SELL' : 'BUY'}</span><h3>{side} intent</h3></div><label><span>Maker Ed25519 public key</span><input required value={form[side].maker} onChange={(event) => updateParticipant(side, 'maker', event.target.value)} /></label><label><span>Collateral token account</span><input required value={form[side].collateral} onChange={(event) => updateParticipant(side, 'collateral', event.target.value)} /></label><div className="paired-fields"><label><span>Gap-free nonce</span><input inputMode="numeric" value={form[side].nonce} onChange={(event) => updateParticipant(side, 'nonce', event.target.value)} /></label><label><span>Maximum fill</span><input inputMode="numeric" value={form[side].maximumFill} onChange={(event) => updateParticipant(side, 'maximumFill', event.target.value)} /></label></div><label><span>{side === 'seller' ? 'Minimum' : 'Maximum'} signed price · scale {inspection.route.priceScale.toString()}</span><input inputMode="numeric" value={form[side].limitPrice} onChange={(event) => updateParticipant(side, 'limitPrice', event.target.value)} /></label><label><span>Detached 64-byte signature · lowercase hex</span><textarea value={form[side].signature} onChange={(event) => updateParticipant(side, 'signature', event.target.value)} /></label><button type="button" className="secondary-action" onClick={() => void signMaker(side)}>Sign this maker message with connected wallet</button></section>)}</div>
        <div className="trade-v3-execution"><label><span>Fill quantity</span><input inputMode="numeric" value={form.fill} onChange={(event) => updateField('fill', event.target.value)} /></label><label><span>Execution price</span><input inputMode="numeric" value={form.executionPrice} onChange={(event) => updateField('executionPrice', event.target.value)} /></label><label><span>Review tolerance vs signed midpoint · bps</span><input inputMode="numeric" value={form.reviewSlippageBps} onChange={(event) => updateField('reviewSlippageBps', event.target.value)} /></label><button>Build exact unsigned v0 transaction</button></div>
        {preview && <div className="trade-v3-preview"><div><span>Gross collateral</span><strong>{preview.grossCollateral.toString()}</strong></div><div><span>Seller credit</span><strong>{preview.sellerNetCollateralCredit.toString()}</strong></div><div><span>Buyer debit</span><strong>{preview.buyerCollateralDebit.toString()}</strong></div><div><span>Total fees</span><strong>{preview.totalFeeTransfer.toString()}</strong></div><p>Preview uses the finalized observation slot and one exact fill×price ÷ scale boundary. Onchain Clock and interpreter execution remain authoritative.</p></div>}
        <p className="direct-status" aria-live="polite">{tradeStatus}</p>
      </>}
    </form>

    <section className="trade-v3-card signing-card">
      <header><span>03</span><div><h2>Wallet handoff and exact packet export</h2><p>Connecting reads only identity. Maker and transaction signing are separate explicit wallet requests. Submission is deliberately outside this workbench.</p></div></header>
      <div className="signing-grid"><article><span>Wallet identity</span><strong>{wallet || 'not connected'}</strong><button type="button" onClick={() => void connectWallet()}>Connect identity</button><p>{walletStatus}</p></article><article><span>Unsigned / signed packet</span><strong>{plan ? `${plan.wireBytes.length} bytes · ${plan.loadedAddresses} ALT` : 'no transaction built'}</strong><button type="button" disabled={plan === null} onClick={() => void signTransaction()}>Sign as transaction payer</button><button type="button" disabled={plan === null} onClick={downloadPacket}>Download exact packet</button><p>{signed ? `${signed.complete ? 'Complete' : 'Partial'} signature set · ${signed.wireBytes.length} bytes. Export it for an external submitter.` : 'No transaction signature requested.'}</p></article></div>
      {plan && <details className="trade-v3-bytes"><summary>Exact transaction material</summary><dl><div><dt>Required signer</dt><dd>{plan.requiredSigners.join(', ')}</dd></div><div><dt>Wire bytes</dt><dd>{signed ? base64(signed.wireBytes) : base64(plan.wireBytes)}</dd></div><div><dt>Request bytes</dt><dd>{hex(plan.requestBytes)}</dd></div></dl></details>}
    </section>

    <footer className="product-footer"><span>Chain state and checked release evidence are authoritative.</span><span>No automatic wallet request · no submission path</span></footer>
  </main>;
}
