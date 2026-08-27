'use client';

import Link from 'next/link';
import { useMemo, useState } from 'react';

import WalletDirectory, { useWalletDirectoryV1 } from '@/components/WalletDirectory';
import {
  compileDirectInlineTransactionV3,
  encodeCompactIntentSigningMessageV2,
  type SignedDirectIntentV3,
} from '@/lib/directInlineV3';
import { inspectDirectHotRouteV3, type DirectHotRouteManifestV3, type DirectHotRouteCoordinateV3 } from '@/lib/directHotChain';
import {
  decodeDirectIntentTicketV1,
  planDirectCrossingV1,
  type DirectCrossingPlanV1,
} from '@/lib/directTicket';
import {
  inspectDirectTradeSpineV1,
  type DirectTradeSpineV1,
} from '@/lib/directTradeSpine';
import {
  decodeClaimsPositionV2,
  deriveClaimsAggregateAddressV2,
  deriveClaimsPositionAddressV2,
} from '@/lib/marketCoreV2';
import { type MarketLiabilityV1 } from '@/lib/marketDiscovery';
import { SolanaRpcClient } from '@/lib/rpc';
import { requestWalletMessageSignatureV1, requestWalletTransactionSignatureV1, submitSignedTransactionV1 } from '@/lib/walletHandoff';

/**
 * The trader's face of one Market: pick an outcome, size it, cross one
 * counterparty ticket, sign, submit, and see the confirmed Position — with
 * every wall between those steps named in the chain's own words.
 *
 * There is no order book here, deliberately: the recovered product brief
 * forbids rendering one, and the protocol settles two SIGNED intents rather
 * than resting orders. The counterparty's half arrives as a portable ticket;
 * this panel's own half is signed by the connected wallet. Everything
 * quantitative below — price scale, fee, debit, credit — is chain-read or
 * derived by the same builders the byte-level tests pin. Preview arithmetic
 * needs no route; EXECUTION needs the operator-published route manifest and
 * lookup table, which the advanced drawer accepts in exactly the /trade
 * workspace's format.
 */

type TicketState =
  | Readonly<{ kind: 'none' }>
  | Readonly<{ kind: 'refused'; reason: string }>
  | Readonly<{ kind: 'crossed'; ticket: SignedDirectIntentV3; plan: DirectCrossingPlanV1 }>;

type ExecutionState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'working'; message: string }>
  | Readonly<{ kind: 'refused'; reason: string }>
  | Readonly<{
    kind: 'confirmed';
    signature: string;
    confirmation: string;
    positionAddress: string;
    balances: ReadonlyArray<string>;
  }>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'the step refused without a usable reason';
}

function coordinate(value: unknown, field: string): DirectHotRouteCoordinateV3 {
  if (value === null || typeof value !== 'object') throw new Error(`${field} is not an account coordinate`);
  const candidate = value as Record<string, unknown>;
  if (typeof candidate.address !== 'string' || typeof candidate.isSigner !== 'boolean' || typeof candidate.isWritable !== 'boolean') {
    throw new Error(`${field} must name address, isSigner, and isWritable`);
  }
  return Object.freeze({ address: candidate.address, isSigner: candidate.isSigner, isWritable: candidate.isWritable });
}

function parseRouteManifest(text: string): DirectHotRouteManifestV3 {
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
    checkedInfrastructure: null,
  });
}

export default function MarketTradePanel({
  endpoint,
  marketAddress,
  coreProgramId,
  registryProgramId,
  claimsProgramId,
  tradingProgramId,
  liability,
}: Readonly<{
  endpoint: string;
  marketAddress: string;
  coreProgramId: string;
  registryProgramId: string | null;
  claimsProgramId: string | null;
  tradingProgramId: string | null;
  liability: MarketLiabilityV1 | null;
}>) {
  const wallets = useWalletDirectoryV1();
  const [spine, setSpine] = useState<DirectTradeSpineV1 | null>(null);
  const [spineStatus, setSpineStatus] = useState('The chain has not been asked about trading this Market yet.');
  const [outcome, setOutcome] = useState<number | null>(null);
  const [desired, setDesired] = useState('');
  const [ticketText, setTicketText] = useState('');
  const [signature, setSignature] = useState<Uint8Array | null>(null);
  const [routeText, setRouteText] = useState('');
  const [execution, setExecution] = useState<ExecutionState>({ kind: 'idle' });

  const inspected = spine !== null && spine.status === 'inspected' ? spine : null;

  const ticketState: TicketState = useMemo(() => {
    if (inspected === null || ticketText.trim() === '') return Object.freeze({ kind: 'none' as const });
    if (wallets.address === null) return Object.freeze({ kind: 'refused' as const, reason: 'connect a browser wallet: the ticket is crossed against the connected identity' });
    if (claimsProgramId === null || claimsProgramId === '') return Object.freeze({ kind: 'refused' as const, reason: 'select the Claims program: the taker collateral account and Position derive under it' });
    try {
      const ticket = decodeDirectIntentTicketV1(ticketText.trim());
      const aggregate = deriveClaimsAggregateAddressV2(claimsProgramId, marketAddress);
      const takerPosition = deriveClaimsPositionAddressV2(claimsProgramId, aggregate, wallets.address);
      const plan = planDirectCrossingV1({
        route: {
          market: inspected.marketAddress,
          generation: BigInt(inspected.generation),
          outcomeCount: inspected.outcomeCount ?? Number.MAX_SAFE_INTEGER,
          priceScale: inspected.priceScale,
          feeBasisPoints: inspected.feeBasisPoints,
        },
        ticket,
        takerAddress: wallets.address,
        // The taker's collateral coordinate on the intent wire; the executing
        // route re-authenticates it against the Realm, so a wrong account is a
        // chain refusal, never a silent substitution.
        takerCollateralAccount: takerPosition,
        desiredFill: desired.trim() === '' ? ticket.intent.maximumFill : BigInt(desired.trim()),
        clockSlot: BigInt(inspected.observedSlot),
      });
      return Object.freeze({ kind: 'crossed' as const, ticket, plan });
    } catch (error) {
      return Object.freeze({ kind: 'refused' as const, reason: errorMessage(error) });
    }
  }, [inspected, ticketText, desired, wallets.address, claimsProgramId, marketAddress]);

  async function inspect() {
    setSpineStatus('Reading the Market, its manifest, the Direct program set, descriptor, and config at one finalized floor…');
    setSpine(null); setSignature(null); setExecution({ kind: 'idle' });
    if (registryProgramId === null || registryProgramId === '') {
      setSpine(Object.freeze({ status: 'refused' as const, reason: 'the Registry program is required: the Direct capability lives in Registry-finalized records' }));
      setSpineStatus('Refused before any read.');
      return;
    }
    const next = await inspectDirectTradeSpineV1(new SolanaRpcClient(endpoint), {
      marketAddress,
      coreProgramId,
      registryProgramId,
      tradingProgramId,
      claimsProgramId,
      owner: wallets.address,
    });
    setSpine(next);
    setSpineStatus(next.status === 'inspected' ? next.reason : `Refused: ${next.reason}`);
  }

  async function signIntent() {
    if (ticketState.kind !== 'crossed' || wallets.address === null) return;
    try {
      const message = encodeCompactIntentSigningMessageV2(ticketState.plan.taker);
      const signed = await requestWalletMessageSignatureV1(wallets.handoff(endpoint), wallets.address, message);
      setSignature(signed);
    } catch (error) {
      setExecution({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  async function execute() {
    if (ticketState.kind !== 'crossed' || signature === null || wallets.address === null || inspected === null) return;
    try {
      setExecution({ kind: 'working', message: 'Reacquiring and joining the full 39-account route at one finalized floor…' });
      const client = new SolanaRpcClient(endpoint);
      const manifest = parseRouteManifest(routeText);
      const inspection = await inspectDirectHotRouteV3(client, manifest);
      const mine: SignedDirectIntentV3 = Object.freeze({ maker: wallets.address, signature, intent: ticketState.plan.taker });
      const seller = ticketState.ticket.intent.side === 0 ? ticketState.ticket : mine;
      const buyer = ticketState.ticket.intent.side === 1 ? ticketState.ticket : mine;
      setExecution({ kind: 'working', message: 'Compiling the exact two-instruction v0 packet from the joined route…' });
      const plan = compileDirectInlineTransactionV3({
        route: inspection.route,
        seller,
        buyer,
        fill: ticketState.plan.fill,
        executionPrice: ticketState.plan.executionPrice,
        clockSlot: BigInt(inspection.observedSlot),
      });
      setExecution({ kind: 'working', message: `Unsigned ${plan.wireBytes.length}-byte packet compiled; requesting the payer signature…` });
      const signed = await requestWalletTransactionSignatureV1(wallets.handoff(endpoint), plan.transaction, inspection.route.payer);
      if (!signed.complete) throw new Error('the wallet did not complete the payer signature');
      setExecution({ kind: 'working', message: 'Submitting the signed packet through the one RPC seam…' });
      const submitted = await submitSignedTransactionV1(client, signed.transaction);
      for (let attempt = 0; attempt < 30; attempt += 1) {
        await new Promise((resolve) => setTimeout(resolve, 1_000));
        const [status] = await client.signatureStatuses([submitted]);
        if (status !== undefined && status.known) {
          if (status.succeeded === false) {
            setExecution({ kind: 'refused', reason: `the chain refused the fill: ${status.errorText ?? 'unnamed chain error'}` });
            return;
          }
          if (status.confirmationStatus === 'finalized' || status.confirmationStatus === 'confirmed') {
            const aggregate = deriveClaimsAggregateAddressV2(claimsProgramId ?? '', marketAddress);
            const positionAddress = deriveClaimsPositionAddressV2(claimsProgramId ?? '', aggregate, wallets.address);
            const observation = await client.multipleAccounts([positionAddress]);
            const positionAccount = observation.accounts[0]?.account ?? null;
            const balances = positionAccount === null ? [] : decodeClaimsPositionV2(positionAddress, positionAccount.data).balances;
            setExecution({ kind: 'confirmed', signature: submitted, confirmation: status.confirmationStatus, positionAddress, balances });
            return;
          }
          setExecution({ kind: 'working', message: `Submitted as ${submitted} · ${status.confirmationStatus ?? 'processed'}…` });
        }
      }
      setExecution({ kind: 'refused', reason: 'submitted, but no confirmation arrived within 30 seconds; check the signature on the activity surface' });
    } catch (error) {
      setExecution({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  const supplies = liability !== null && liability.status === 'bound' ? liability.supplyAtoms : null;

  return <section className="trade-v3-card">
    <header><span>05</span><div><h2>Trade this Market</h2><p>Pick an outcome, size it, and cross one counterparty ticket at the maker&apos;s signed price. Every number below is chain-read (the immutable price scale and fee) or computed by the same builders the byte-level tests pin. What cannot execute yet says exactly why, in the chain&apos;s own vocabulary — a named refusal is this protocol&apos;s honest interface.</p></div></header>

    <div className="direct-actions">
      <button type="button" onClick={() => void inspect()}>Ask the chain about trading here</button>
      <Link className="secondary-action" href="/trade">Advanced: full route workbench →</Link>
    </div>
    <p className="direct-status" aria-live="polite">{spineStatus}</p>

    {spine !== null && spine.status === 'refused' && <p className="market-refusal">Refused: {spine.reason}</p>}
    {inspected !== null && <>
      <div className="trade-v3-evidence">
        <article><span>Immutable price scale</span><strong>{inspected.priceScale.toString()}</strong><small>from the Direct config record</small></article>
        <article><span>Fee</span><strong>{inspected.feeBasisPoints} bps each side</strong><small>immutable, founded with the Market</small></article>
        <article><span>Phase</span><strong>{inspected.phase}</strong><small>generation {inspected.generation}</small></article>
        <article><span>Activation</span><strong>{inspected.rootExists === null ? 'not checked' : inspected.rootExists ? 'root standing' : 'never activated'}</strong><small>{inspected.rootAddress === null ? 'select a Trading program to check' : `capability root ${inspected.rootAddress.slice(0, 8)}…`}</small></article>
      </div>

      {inspected.outcomeCount !== null && <>
        <h3 className="detail-subhead">Outcome · pick the claim to trade</h3>
        <ol className="outcome-vector">
          {Array.from({ length: inspected.outcomeCount }, (_, index) => (
            <li key={index} className={outcome === index ? 'winning-outcome' : ''}>
              <button type="button" className="secondary-action" onClick={() => setOutcome(index)}>claim {index}</button>
              {supplies !== null && <strong>{supplies[index] ?? '0'}</strong>}
              {supplies !== null && <small>issued atoms</small>}
            </li>
          ))}
        </ol>
      </>}

      <h3 className="detail-subhead">Counterparty ticket · the other signed half</h3>
      <p className="direct-status">A Direct fill settles two signed intents from two distinct makers; there is no order book to take from, by design. Paste the counterparty&apos;s portable ticket (dclutch/direct-intent-ticket/v1). It is not trusted: the builder re-derives the signing message, and the chain verifies the signature natively.</p>
      <label><span>Ticket JSON</span><textarea rows={5} spellCheck={false} value={ticketText} onChange={(event) => { setTicketText(event.target.value); setSignature(null); }} /></label>
      <div className="direct-form-grid">
        <label><span>My size · claim atoms (blank = take the ticket in full)</span><input inputMode="numeric" value={desired} onChange={(event) => { setDesired(event.target.value.trim()); setSignature(null); }} /></label>
      </div>
      <WalletDirectory directory={wallets} purpose="taker identity, intent and payer signatures" onConnected={() => setSignature(null)} />

      {ticketState.kind === 'refused' && <p className="market-refusal">Ticket refused: {ticketState.reason}</p>}
      {ticketState.kind === 'crossed' && <>
        {outcome !== null && outcome !== ticketState.ticket.intent.outcome && <p className="market-refusal">The ticket is for claim {ticketState.ticket.intent.outcome}, not the selected claim {outcome}. Crossing follows the ticket; select claim {ticketState.ticket.intent.outcome} or find another ticket.</p>}
        <div className="trade-v3-preview">
          <div><span>{ticketState.plan.takerSide === 'buy' ? 'You buy' : 'You sell'}</span><strong>{ticketState.plan.fill.toString()}</strong></div>
          <div><span>Execution price</span><strong>{ticketState.plan.executionPrice.toString()}</strong></div>
          <div><span>{ticketState.plan.takerSide === 'buy' ? 'Exact debit' : 'Exact credit'}</span><strong>{ticketState.plan.takerSide === 'buy' ? ticketState.plan.preview.buyerCollateralDebit.toString() : ticketState.plan.preview.sellerNetCollateralCredit.toString()}</strong></div>
          <div><span>Fee each side</span><strong>{ticketState.plan.preview.sellerFee.toString()}</strong></div>
          <p>{ticketState.plan.note} Preview uses the finalized observation slot; the on-chain interpreters remain authoritative.</p>
        </div>
        <div className="direct-actions">
          <button type="button" onClick={() => void signIntent()}>{signature === null ? 'Sign my intent with the connected wallet' : 'Intent signed · sign again to replace'}</button>
        </div>
      </>}

      <h3 className="detail-subhead">Walls between preview and execution · named, not hedged</h3>
      {inspected.walls.length === 0
        ? <p className="direct-status">No walls: the route below can execute this crossing.</p>
        : <ul className="market-bindings">{inspected.walls.map((wall) => (
          <li key={wall.name} className="check-fail"><span aria-hidden="true">×</span><div><strong>{wall.name}</strong><small>{wall.detail}</small></div></li>
        ))}</ul>}

      <details className="trade-v3-bytes">
        <summary>Execute · requires the operator-published route manifest (advanced)</summary>
        <p className="direct-status">Execution reacquires the full 39-account Hot route and its one canonical lookup table, compiles the exact two-instruction v0 packet, asks the wallet for the single payer signature, submits through the one RPC seam, and confirms. The manifest format is the /trade workbench&apos;s. Every refusal along the way is the chain&apos;s, with its reason.</p>
        <label><span>Route manifest JSON</span><textarea rows={6} spellCheck={false} value={routeText} onChange={(event) => setRouteText(event.target.value)} /></label>
        <div className="direct-actions">
          <button type="button" disabled={ticketState.kind !== 'crossed' || signature === null || execution.kind === 'working'} onClick={() => void execute()}>
            {execution.kind === 'working' ? 'Executing…' : 'Build, sign as payer, and submit'}
          </button>
        </div>
        {execution.kind === 'working' && <p className="direct-status" aria-live="polite">{execution.message}</p>}
        {execution.kind === 'refused' && <p className="market-refusal">Refused: {execution.reason}</p>}
        {execution.kind === 'confirmed' && <div className="portfolio-claim">
          <span>Fill confirmed · {execution.confirmation}</span>
          <strong>{execution.signature.slice(0, 20)}…</strong>
          <p>Position {execution.positionAddress} now holds [{execution.balances.join(' · ')}] raw claim atoms, read back finalized after the fill.</p>
        </div>}
      </details>
    </>}
  </section>;
}
