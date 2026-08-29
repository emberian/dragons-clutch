'use client';

import Anchor from '@/components/Anchor';
import { useMemo, useState } from 'react';

import WalletDirectory, { useWalletDirectoryV1 } from '@/components/WalletDirectory';
import { type SignedDirectIntentV3 } from '@/lib/directInlineV3';
import { decodeDirectIntentTicketV1 } from '@/lib/directTicket';
import {
  admitDirectParticipantCrossingV1,
  inspectDirectParticipantReadinessV1,
  type DirectParticipantCrossingAdmissionV1,
  type DirectParticipantReadinessV1,
} from '@/lib/directParticipant';
import {
  inspectDirectTradeSpineV1,
  type DirectTradeSpineV1,
} from '@/lib/directTradeSpine';
import { type MarketLiabilityV1 } from '@/lib/marketDiscovery';
import { SolanaRpcClient } from '@/lib/rpc';
import { inspectDirectMakerNonceV1 } from '@dclutch/sdk/directMakerReplay';
import { planDirectCrossingV1, type DirectCrossingPlanV1 } from '@dclutch/sdk/directTicket';

/**
 * The trader's face of one Market: pick an outcome, size it, cross one
 * counterparty ticket, and see every wall between a preview and execution.
 *
 * There is no order book here, deliberately: the recovered product brief
 * forbids rendering one, and the protocol settles two SIGNED intents rather
 * than resting orders. The counterparty's half arrives as a portable ticket;
 * this panel's own half is signed by the connected wallet. Everything
 * quantitative below — price scale, fee, debit, credit — is chain-read or
 * derived by the same builders the byte-level tests pin. Preview arithmetic
 * needs no route; EXECUTION needs the operator-published route manifest and
 * lookup table, which the advanced drawer accepts in exactly the /trade
 * workspace's format. This panel deliberately stops before signing until a
 * canonical public manifest and exact finalized completion verifier exist.
 */

type TicketState =
  | Readonly<{ kind: 'none' }>
  | Readonly<{ kind: 'refused'; reason: string }>
  | Readonly<{ kind: 'ready'; ticket: SignedDirectIntentV3 }>;

type ExecutionState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'working'; message: string }>
  | Readonly<{ kind: 'refused'; reason: string }>
  | Readonly<{
    kind: 'ready';
    plan: DirectCrossingPlanV1;
    admission: DirectParticipantCrossingAdmissionV1;
    replaySlot: string;
  }>;

function positiveU64(text: string, fallback: bigint): bigint {
  if (text === '') return fallback;
  if (!/^[1-9][0-9]*$/.test(text)) throw new Error('your size must be one positive whole number of claim atoms');
  const value = BigInt(text);
  if (value > 0xffff_ffff_ffff_ffffn) throw new Error('your size exceeds the protocol’s u64 amount width');
  return value;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'the step refused without a usable reason';
}

export default function MarketTradePanel({
  endpoint,
  marketAddress,
  coreProgramId,
  registryProgramId,
  claimsProgramId,
  tradingProgramId,
  custodyProgramId,
  rentProgramId,
  liability,
}: Readonly<{
  endpoint: string;
  marketAddress: string;
  coreProgramId: string;
  registryProgramId: string | null;
  claimsProgramId: string | null;
  tradingProgramId: string | null;
  custodyProgramId: string | null;
  rentProgramId: string | null;
  liability: MarketLiabilityV1 | null;
}>) {
  const wallets = useWalletDirectoryV1();
  const [spine, setSpine] = useState<DirectTradeSpineV1 | null>(null);
  const [spineStatus, setSpineStatus] = useState('The chain has not been asked about trading this Market yet.');
  const [participant, setParticipant] = useState<DirectParticipantReadinessV1 | null>(null);
  const [participantStatus, setParticipantStatus] = useState('Connect your wallet, then ask the chain to check your Position, admission, and collateral account.');
  const [outcome, setOutcome] = useState<number | null>(null);
  const [desired, setDesired] = useState('');
  const [ticketText, setTicketText] = useState('');
  const [execution, setExecution] = useState<ExecutionState>({ kind: 'idle' });

  const inspected = spine !== null && spine.status === 'inspected' ? spine : null;

  const ticketState: TicketState = useMemo(() => {
    if (inspected === null || ticketText.trim() === '') return Object.freeze({ kind: 'none' as const });
    if (wallets.address === null) return Object.freeze({ kind: 'refused' as const, reason: 'connect a browser wallet: the ticket is crossed against the connected identity' });
    if (claimsProgramId === null || claimsProgramId === '') return Object.freeze({ kind: 'refused' as const, reason: 'select the Claims program before checking the participant admission evidence' });
    try {
      const ticket = decodeDirectIntentTicketV1(ticketText.trim());
      return Object.freeze({ kind: 'ready' as const, ticket });
    } catch (error) {
      return Object.freeze({ kind: 'refused' as const, reason: errorMessage(error) });
    }
  }, [inspected, ticketText, wallets.address, claimsProgramId]);

  function invalidatePreview(): void {
    setExecution({ kind: 'idle' });
  }

  function invalidateWalletState(): void {
    setParticipant(null);
    setParticipantStatus('Your wallet changed. Ask the chain again before previewing a crossing.');
    invalidatePreview();
  }

  async function inspect() {
    setSpineStatus('Reading the Market, its manifest, the Direct program set, descriptor, and config at one finalized floor…');
    setSpine(null); setParticipant(null); setExecution({ kind: 'idle' });
    if (registryProgramId === null || registryProgramId === '') {
      setSpine(Object.freeze({ status: 'refused' as const, reason: 'the Registry program is required: the Direct capability lives in Registry-finalized records' }));
      setSpineStatus('Refused before any read.');
      return;
    }
    const rpc = new SolanaRpcClient(endpoint);
    const next = await inspectDirectTradeSpineV1(rpc, {
      marketAddress,
      coreProgramId,
      registryProgramId,
      tradingProgramId,
      claimsProgramId,
      owner: wallets.address,
    });
    setSpine(next);
    setSpineStatus(next.status === 'inspected' ? next.reason : `Refused: ${next.reason}`);
    if (wallets.address === null) {
      setParticipantStatus('Connect your wallet, then ask again so the chain can check the accounts that belong to you.');
      return;
    }
    if (registryProgramId === null || claimsProgramId === null || tradingProgramId === null
        || custodyProgramId === null || rentProgramId === null) {
      setParticipantStatus('This deployment does not name every program needed to authenticate your participant accounts.');
      return;
    }
    const nextParticipant = await inspectDirectParticipantReadinessV1(rpc, {
      market: marketAddress,
      owner: wallets.address,
      coreProgram: coreProgramId,
      registryProgram: registryProgramId,
      claimsProgram: claimsProgramId,
      tradingProgram: tradingProgramId,
      custodyProgram: custodyProgramId,
      rentProgram: rentProgramId,
    });
    setParticipant(nextParticipant);
    setParticipantStatus(nextParticipant.status === 'refused' ? `Refused: ${nextParticipant.reason}` : nextParticipant.reason);
  }

  async function previewIntent() {
    if (ticketState.kind !== 'ready' || wallets.address === null || inspected === null) return;
    if (participant === null || participant.status !== 'ready') {
      setExecution({ kind: 'refused', reason: participant?.reason ?? 'Ask the chain to authenticate your participant accounts before previewing a crossing.' });
      return;
    }
    if (tradingProgramId === null || inspected.outcomeCount === null) {
      setExecution({ kind: 'refused', reason: 'This Market does not expose the Trading program and Product width needed for an exact crossing.' });
      return;
    }
    if (outcome === null) {
      setExecution({ kind: 'refused', reason: 'Pick the claim you intend to trade before previewing the ticket.' });
      return;
    }
    if (outcome !== ticketState.ticket.intent.outcome) {
      setExecution({ kind: 'refused', reason: `You picked claim ${outcome}, but this ticket is signed for claim ${ticketState.ticket.intent.outcome}.` });
      return;
    }
    setExecution({ kind: 'working', message: 'Reading your finalized replay nonce and checking the crossing against your current assets…' });
    try {
      const rpc = new SolanaRpcClient(endpoint);
      const replay = await inspectDirectMakerNonceV1(rpc, {
        tradingProgram: tradingProgramId,
        market: marketAddress,
        generation: BigInt(inspected.generation),
        maker: wallets.address,
      });
      const plan = planDirectCrossingV1({
        route: {
          tradingProgram: tradingProgramId,
          market: marketAddress,
          generation: BigInt(inspected.generation),
          outcomeCount: inspected.outcomeCount,
          priceScale: inspected.priceScale,
          feeBasisPoints: inspected.feeBasisPoints,
        },
        ticket: ticketState.ticket,
        takerAddress: wallets.address,
        takerReplay: replay,
        takerCollateralAccount: participant.coordinates.collateral,
        desiredFill: positiveU64(desired, ticketState.ticket.intent.maximumFill),
        clockSlot: BigInt(replay.observedSlot),
      });
      const admission = admitDirectParticipantCrossingV1(participant, plan);
      setExecution({ kind: 'ready', plan, admission, replaySlot: replay.observedSlot });
    } catch (error) {
      setExecution({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  const supplies = liability !== null && liability.status === 'bound' ? liability.supplyAtoms : null;

  return <section className="trade-v3-card">
    <header><span>05</span><div><h2>Trade this Market</h2><p>Pick an outcome, size it, and cross one signed offer at the price its maker signed. Every number you see is read off the chain or computed by the exact code the chain runs. When something cannot happen yet, this panel tells you exactly why in one sentence — never a greyed-out button with no reason.</p></div></header>

    <div className="direct-actions">
      <button type="button" onClick={() => void inspect()}>Ask the chain about trading here</button>
      <Anchor className="secondary-action" href="/trade">Advanced: full route workbench →</Anchor>
    </div>
    <p className="direct-status" aria-live="polite">{spineStatus}</p>
    <p className="direct-status">This page can authenticate your participant accounts and calculate an unsigned crossing from a portable ticket. It does not accept a pasted route or ask for a signature, and it does not submit a trade yet. Execution stays unavailable until this Market publishes one canonical public route manifest and the page can verify the exact finalized receipt and poststate. A Claims Position holds claim balances; it is never used as your collateral account. Browser data is an untrusted projection; the onchain programs remain authoritative.</p>
    <p className="direct-status" aria-live="polite">{participantStatus}</p>

    {spine !== null && spine.status === 'refused' && <p className="market-refusal">Refused: {spine.reason}</p>}
    {inspected !== null && <>
      <div className="trade-v3-evidence">
        <article><span>Immutable price scale</span><strong>{inspected.priceScale.toString()}</strong><small>from the Direct config record</small></article>
        <article><span>Fee</span><strong>{inspected.feeBasisPoints} bps each side</strong><small>immutable, founded with the Market</small></article>
        <article><span>Phase</span><strong>{inspected.phase}</strong><small>generation {inspected.generation}</small></article>
        <article><span>Activation</span><strong>{inspected.rootExists === null ? 'not checked' : inspected.rootExists ? 'root standing' : 'never activated'}</strong><small>{inspected.rootAddress === null ? 'select a Trading program to check' : `capability root ${inspected.rootAddress.slice(0, 8)}…`}</small></article>
      </div>

      {participant !== null && participant.status === 'ready' && <div className="trade-v3-evidence">
        <article><span>Your claim balance</span><strong>{outcome === null ? 'pick a claim' : (participant.positionBalances[outcome] ?? 0n).toString()}</strong><small>finalized Position revision {participant.positionRevision.toString()}</small></article>
        <article><span>Your collateral</span><strong>{participant.collateralAtoms.toString()}</strong><small>{participant.spendableCollateralAtoms.toString()} atoms currently delegated</small></article>
        <article><span>Your Position</span><strong>{participant.coordinates.position.slice(0, 8)}…</strong><small>derived from this Market and your wallet</small></article>
        <article><span>Your collateral account</span><strong>{participant.coordinates.collateral.slice(0, 8)}…</strong><small>derived from this Market, wallet, and release</small></article>
      </div>}
      {participant !== null && participant.status === 'incomplete' && <p className="market-refusal">Not ready: {participant.reason}</p>}
      {participant !== null && participant.status === 'refused' && <p className="market-refusal">Participant state refused: {participant.reason}</p>}

      {inspected.outcomeCount !== null && <>
        <h3 className="detail-subhead">Outcome · pick the claim to trade</h3>
        <ol className="outcome-vector">
          {Array.from({ length: inspected.outcomeCount }, (_, index) => (
            <li key={index} className={outcome === index ? 'winning-outcome' : ''}>
              <button type="button" className="secondary-action" onClick={() => { setOutcome(index); invalidatePreview(); }}>claim {index}</button>
              {supplies !== null && <strong>{supplies[index] ?? '0'}</strong>}
              {supplies !== null && <small>issued atoms</small>}
            </li>
          ))}
        </ol>
      </>}

      <h3 className="detail-subhead">The other side&apos;s ticket</h3>
      <p className="direct-status">A trade here is two signed halves: yours and someone else&apos;s. There is no order book to take from — the other half arrives as a small ticket (dclutch/direct-intent-ticket/v1) you can be handed any way you like. Pasting it is safe: nothing in it is believed until the chain itself checks the signature.</p>
      <label><span>Ticket JSON</span><textarea rows={5} spellCheck={false} value={ticketText} onChange={(event) => { setTicketText(event.target.value); invalidatePreview(); }} /></label>
      <div className="direct-form-grid">
        <label><span>My size · claim atoms (blank = take the ticket in full)</span><input inputMode="numeric" value={desired} onChange={(event) => { setDesired(event.target.value.trim()); invalidatePreview(); }} /></label>
      </div>
      <WalletDirectory directory={wallets} purpose="taker identity, intent and payer signatures" onConnected={invalidateWalletState} />

      {ticketState.kind === 'refused' && <p className="market-refusal">Ticket refused: {ticketState.reason}</p>}
      {ticketState.kind === 'ready' && <div className="direct-actions">
        <button type="button" onClick={() => void previewIntent()}>Preview this exact crossing</button>
      </div>}

      {execution.kind === 'ready' && <div className="trade-v3-evidence">
        <article><span>You {execution.plan.takerSide}</span><strong>{execution.plan.fill.toString()} claim atoms</strong><small>claim {execution.plan.taker.outcome} at signed price {execution.plan.executionPrice.toString()}</small></article>
        <article><span>Gross collateral</span><strong>{execution.plan.preview.grossCollateral.toString()}</strong><small>price scale {inspected.priceScale.toString()}</small></article>
        <article><span>Your fee</span><strong>{(execution.plan.takerSide === 'buy' ? execution.plan.preview.buyerFee : execution.plan.preview.sellerFee).toString()}</strong><small>{inspected.feeBasisPoints} bps, rounded at the protocol boundary</small></article>
        <article><span>Asset check</span><strong>{execution.admission.requiredAtoms.toString()} / {execution.admission.availableAtoms.toString()}</strong><small>{execution.admission.resource}, finalized through slot {execution.replaySlot}</small></article>
      </div>}
      {execution.kind === 'ready' && <p className="direct-status">This is an unsigned preview, not a submitted trade. The page has not asked your wallet to sign anything.</p>}

      <h3 className="detail-subhead">What stands between this preview and a real trade</h3>
      {inspected.walls.length === 0
        ? <p className="direct-status">This bounded chain inspection found no Market-state wall. Signing still waits for the published route and completion verifier named above.</p>
        : <ul className="market-bindings">{inspected.walls.map((wall) => (
          <li key={wall.name} className="check-fail"><span aria-hidden="true">×</span><div><strong>{wall.name}</strong><small>{wall.detail}</small></div></li>
        ))}</ul>}

      <details className="trade-v3-bytes">
        <summary>Why Direct execution is not available here yet</summary>
        <p className="direct-status">This public page does not accept a pasted route or ask for a signature. It will execute only after the participant-admission semantic owner supplies the distinct chain-derived Token-2022 account, this Market publishes one canonical public route manifest, and the client can authenticate that exact manifest at finalized commitment, save the exact chain, Market, owner, operation digest, intent, plan, and signed transaction id before submission, and verify the exact finalized receipt and poststate after submission.</p>
        <p className="direct-status">Browser data is an untrusted projection. The onchain programs still refuse substituted accounts, nonces, instructions, and resource state.</p>
        {execution.kind === 'working' && <p className="direct-status" aria-live="polite">{execution.message}</p>}
        {execution.kind === 'refused' && <p className="market-refusal">Refused: {execution.reason}</p>}
      </details>
    </>}
  </section>;
}
