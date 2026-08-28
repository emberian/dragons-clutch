'use client';

import ConsoleHeader from '@/components/ConsoleHeader';
import { FormEvent, useMemo, useState } from 'react';

import {
  type DirectHotRouteCoordinateV3,
  type DirectHotRouteInspectionV3,
  type DirectHotRouteManifestV3,
  inspectDirectHotRouteV3,
} from '@/lib/directHotChain';
import {
  type CompactIntentV2Input,
  previewDirectInlineV3,
} from '@/lib/directInlineV3';
import { CHECKED_INFRASTRUCTURE_BYTES_V1 } from '@/lib/infrastructure';
import { SolanaRpcClient } from '@/lib/rpc';

import { useDeploymentFieldV1 } from '@/lib/deploymentStore';

const MAX_U64 = 18_446_744_073_709_551_615n;
const FIXED_ROLES = Object.freeze([
  'Market', 'Direct root', 'Manifest raw', 'Manifest staging', 'ProgramSet raw', 'ProgramSet staging',
  'Descriptor raw', 'Descriptor staging', 'Config raw', 'Config staging', 'AccountProfile raw', 'AccountProfile staging',
  'RequestProfile raw', 'RequestProfile staging', 'Transition raw', 'Transition staging', 'Effect raw', 'Effect staging',
  'Lifecycle raw', 'Lifecycle staging', 'Strategy raw', 'Strategy staging', 'Activation cache', 'Core program',
  'Core ProgramData', 'Trading program', 'Trading ProgramData', 'Registry program', 'Rent sysvar', 'Instructions sysvar',
  'Product raw', 'Product staging', 'result domain raw', 'result domain staging', 'portfolio raw', 'portfolio staging',
  'Product basis raw', 'Product basis staging', 'Capability seal',
]);

type ParticipantFields = Readonly<{
  maker: string;
  collateral: string;
  nonce: string;
  maximumFill: string;
  limitPrice: string;
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

const EMPTY_PARTICIPANT: ParticipantFields = Object.freeze({ maker: '', collateral: '', nonce: '0', maximumFill: '', limitPrice: '' });
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

export default function DirectTradeWorkspace() {
  const [endpoint, setEndpoint] = useDeploymentFieldV1((d) => d.endpoint);
  const [routeText, setRouteText] = useState(manifestScaffold);
  const [infrastructureText, setInfrastructureText] = useState('');
  const [routeState, setRouteState] = useState<RouteState>({ kind: 'idle', message: 'No chain state has been read.' });
  const [form, setForm] = useState<FormFields>(INITIAL_FORM);
  const [tradeStatus, setTradeStatus] = useState('Acquire an exact action-selected route before reviewing a fill.');

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
  }

  function updateField<K extends keyof Omit<FormFields, 'seller' | 'buyer'>>(field: K, value: FormFields[K]) {
    setForm((current) => ({ ...current, [field]: value }));
  }

  async function inspectRoute(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setRouteState({ kind: 'loading', message: 'Reacquiring the exact route at one finalized floor…' });
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

  function reviewFill(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (inspection === null) return;
    try {
      const seller = Object.freeze({ intent: compactIntent(form.seller, 0, form, inspection) });
      const buyer = Object.freeze({ intent: compactIntent(form.buyer, 1, form, inspection) });
      const executionPrice = unsigned(form.executionPrice, 'execution price');
      const midpoint = (seller.intent.limitPrice + buyer.intent.limitPrice) / 2n;
      const deviation = executionPrice >= midpoint ? executionPrice - midpoint : midpoint - executionPrice;
      const reviewBps = midpoint === 0n ? 0n : deviation * 10_000n / midpoint;
      if (reviewBps > unsigned(form.reviewSlippageBps, 'review slippage', 10_000n)) throw new Error(`execution differs from the signed-limit midpoint by ${reviewBps} bps, above the user review tolerance`);
      const next = previewDirectInlineV3(inspection.route, seller, buyer, unsigned(form.fill, 'fill'), executionPrice, BigInt(inspection.observedSlot));
      setTradeStatus(`Read-only arithmetic accepted at finalized slot ${inspection.observedSlot}: buyer debit ${next.buyerCollateralDebit}, seller credit ${next.sellerNetCollateralCredit}, total fee transfer ${next.totalFeeTransfer}. No intent, signature, packet, or transaction was created.`);
    } catch (error) { setTradeStatus(`Refused: ${errorMessage(error)}`); }
  }

  return <main className="product-shell trade-v3-shell">
    <ConsoleHeader path="/trade" title="Direct trade" purpose="Check a route against live chain state and review one fill without signing or building a transaction." />

    <section className="trade-v3-hero"><div><h1>Direct<br /><em>trade.</em></h1><p>You name a Market and a possible fill; the console reads its programs, route, and release evidence from the chain, then shows the exact collateral arithmetic. It does not create an intent, ask for a signature, build a packet, or submit a transaction.</p></div><aside><span>Execution boundary</span><strong>read-only until the finalizer lands</strong><p>Direct execution reopens only with a durable exact-packet journal, an authenticated Trading acknowledgement, and all ten writable poststates. Static account lists are never authority.</p></aside></section>

    <form className="trade-v3-card route-card" onSubmit={inspectRoute}>
      <header><span>01</span><div><h2>Acquire the action-selected route</h2><p>The address map is transport, not truth. Every account is reacquired and joined against the current Registry/Core/Trading state.</p></div></header>
      <div className="trade-v3-route-grid"><label><span>Finalized RPC endpoint</span><input type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label><label><span>Checked infrastructure manifest · base64, optional — <code>infrastructure.checked</code> from the release pipeline, {CHECKED_INFRASTRUCTURE_BYTES_V1.toLocaleString()} bytes</span><textarea value={infrastructureText} onChange={(event) => setInfrastructureText(event.target.value.trim())} /></label><label className="route-json"><span>Route manifest · JSON — every address in it is reacquired from the chain before use; this page pre-fills the exact scaffold</span><textarea required spellCheck={false} value={routeText} onChange={(event) => setRouteText(event.target.value)} /></label></div>
      <button disabled={routeState.kind === 'loading'}>{routeState.kind === 'loading' ? 'Reading finalized route…' : 'Authenticate route'}</button><p className="direct-status" aria-live="polite">{routeState.message}</p>
      {inspection && <div className="trade-v3-evidence"><article><span>Outcome width</span><strong>{inspection.route.outcomeCount.toLocaleString()}</strong><small>Product-derived u32</small></article><article><span>Price / fee</span><strong>{inspection.route.priceScale.toString()} / {inspection.route.feeBasisPoints} bps</strong><small>immutable Direct config</small></article><article><span>Program selector</span><strong>{inspection.selectedProgramDigest.slice(0, 16)}…</strong><small>SetV2 · CapabilityV4 · action 1</small></article><article><span>Strategy → VM</span><strong>{inspection.strategyDigest.slice(0, 8)}… → {inspection.transitionDigest.slice(0, 8)}…</strong><small>interpreted V2 / V3</small></article></div>}
    </form>

    <form className="trade-v3-card" onSubmit={reviewFill}>
      <header><span>02</span><div><h2>Review one possible atomic fill</h2><p>The preview uses the Market&apos;s authenticated outcome width, price scale, fee rate, and finalized observation slot. It creates no signed intent or transaction material.</p></div></header>
      {inspection === null ? <><p className="trade-v3-refusal">Acquire a binding-clean route first. No Market, width, price scale, fee, revision, or authority is fabricated here.</p><button disabled>Review exact arithmetic</button></> : <>
        <div className="trade-v3-common"><label><span>Outcome index · below {inspection.route.outcomeCount.toLocaleString()}</span><input inputMode="numeric" value={form.outcome} onChange={(event) => updateField('outcome', event.target.value)} /></label><label><span>Lifecycle</span><select value={form.lifecycle} onChange={(event) => updateField('lifecycle', event.target.value as '0' | '1')}><option value="0">Fill or kill</option><option value="1">Immediate or cancel</option></select></label><label><span>Valid from slot</span><input inputMode="numeric" value={form.validFrom} onChange={(event) => updateField('validFrom', event.target.value)} /></label><label><span>Valid through slot</span><input inputMode="numeric" value={form.validThrough} onChange={(event) => updateField('validThrough', event.target.value)} /></label></div>
        <div className="trade-v3-participants">{(['seller', 'buyer'] as const).map((side) => <section key={side}><div className="participant-title"><span>{side === 'seller' ? 'SELL' : 'BUY'}</span><h3>{side} terms</h3></div><label><span>Maker Ed25519 public key</span><input required value={form[side].maker} onChange={(event) => updateParticipant(side, 'maker', event.target.value)} /></label><label><span>Collateral token account</span><input required value={form[side].collateral} onChange={(event) => updateParticipant(side, 'collateral', event.target.value)} /></label><div className="paired-fields"><label><span>Gap-free nonce</span><input inputMode="numeric" value={form[side].nonce} onChange={(event) => updateParticipant(side, 'nonce', event.target.value)} /></label><label><span>Maximum fill</span><input inputMode="numeric" value={form[side].maximumFill} onChange={(event) => updateParticipant(side, 'maximumFill', event.target.value)} /></label></div><label><span>{side === 'seller' ? 'Minimum' : 'Maximum'} price · scale {inspection.route.priceScale.toString()}</span><input inputMode="numeric" value={form[side].limitPrice} onChange={(event) => updateParticipant(side, 'limitPrice', event.target.value)} /></label></section>)}</div>
        <div className="trade-v3-execution"><label><span>Fill quantity</span><input inputMode="numeric" value={form.fill} onChange={(event) => updateField('fill', event.target.value)} /></label><label><span>Execution price</span><input inputMode="numeric" value={form.executionPrice} onChange={(event) => updateField('executionPrice', event.target.value)} /></label><label><span>Review tolerance vs midpoint · bps</span><input inputMode="numeric" value={form.reviewSlippageBps} onChange={(event) => updateField('reviewSlippageBps', event.target.value)} /></label><button>Review exact arithmetic</button></div>
        {preview && <div className="trade-v3-preview"><div><span>Gross collateral</span><strong>{preview.grossCollateral.toString()}</strong></div><div><span>Seller credit</span><strong>{preview.sellerNetCollateralCredit.toString()}</strong></div><div><span>Buyer debit</span><strong>{preview.buyerCollateralDebit.toString()}</strong></div><div><span>Total fees</span><strong>{preview.totalFeeTransfer.toString()}</strong></div><p>Preview uses the finalized observation slot and one exact fill×price ÷ scale boundary. Onchain Clock and interpreter execution remain authoritative.</p></div>}
        <p className="direct-status" aria-live="polite">{tradeStatus}</p>
      </>}
    </form>

    <section className="trade-v3-card signing-card">
      <header><span>03</span><div><h2>Execution remains closed</h2><p>This page has no wallet connection, signature request, packet download, or submission control.</p></div></header>
      <p className="trade-v3-refusal">A compiled packet without its durable caller would be an unsafe half-feature. Direct execution will reopen here only after the caller persists the exact signed bytes before one send, authenticates Trading&apos;s final acknowledgement, and verifies all ten ordered writable poststates from finalized history.</p>
    </section>

    <footer className="product-footer"><span>Chain state and checked release evidence are authoritative.</span><span>No wallet request · no packet builder · no submission path</span></footer>
  </main>;
}
