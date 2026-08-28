'use client';

import Anchor from '@/components/Anchor';
import { useMemo, useRef, useState } from 'react';

import WalletDirectory, { useWalletDirectoryV1 } from '@/components/WalletDirectory';
import { type SignedDirectIntentV3 } from '@/lib/directInlineV3';
import {
  inspectDirectMakerNonceV1,
  type AuthenticatedDirectMakerNonceV1,
} from '@/lib/directMakerReplay';
import {
  decodeDirectIntentTicketV1,
  planDirectCrossingV1,
  type DirectCrossingPlanV1,
} from '@/lib/directTicket';
import {
  inspectDirectTradeSpineV1,
  type DirectTradeSpineV1,
} from '@/lib/directTradeSpine';
import { deriveClaimsAggregateAddressV2, deriveClaimsPositionAddressV2 } from '@/lib/marketCoreV2';
import { type MarketLiabilityV1 } from '@/lib/marketDiscovery';
import { SolanaRpcClient } from '@/lib/rpc';

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
  | Readonly<{ kind: 'ready'; ticket: SignedDirectIntentV3 }>
  | Readonly<{ kind: 'crossed'; ticket: SignedDirectIntentV3; plan: DirectCrossingPlanV1 }>;

type TakerReplayState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'working' }>
  | Readonly<{ kind: 'ready'; observation: AuthenticatedDirectMakerNonceV1 }>;

type ExecutionState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'working'; message: string }>
  | Readonly<{ kind: 'refused'; reason: string }>;

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
  const [takerReplay, setTakerReplay] = useState<TakerReplayState>({ kind: 'idle' });
  const replayEpoch = useRef(0);
  const [execution, setExecution] = useState<ExecutionState>({ kind: 'idle' });

  const inspected = spine !== null && spine.status === 'inspected' ? spine : null;

  const ticketState: TicketState = useMemo(() => {
    if (inspected === null || ticketText.trim() === '') return Object.freeze({ kind: 'none' as const });
    if (wallets.address === null) return Object.freeze({ kind: 'refused' as const, reason: 'connect a browser wallet: the ticket is crossed against the connected identity' });
    if (claimsProgramId === null || claimsProgramId === '') return Object.freeze({ kind: 'refused' as const, reason: 'select the Claims program: the taker collateral account and Position derive under it' });
    try {
      const ticket = decodeDirectIntentTicketV1(ticketText.trim());
      if (takerReplay.kind !== 'ready') return Object.freeze({ kind: 'ready' as const, ticket });
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
        takerReplay: takerReplay.observation,
        // The taker's collateral coordinate on the intent wire; the executing
        // route re-authenticates it against the Realm, so a wrong account is a
        // chain refusal, never a silent substitution.
        takerCollateralAccount: takerPosition,
        desiredFill: desired.trim() === '' ? ticket.intent.maximumFill : BigInt(desired.trim()),
        clockSlot: BigInt(takerReplay.observation.observedSlot),
      });
      return Object.freeze({ kind: 'crossed' as const, ticket, plan });
    } catch (error) {
      return Object.freeze({ kind: 'refused' as const, reason: errorMessage(error) });
    }
  }, [inspected, ticketText, desired, wallets.address, claimsProgramId, marketAddress, takerReplay]);

  function invalidateTakerReplay(): void {
    replayEpoch.current += 1;
    setTakerReplay({ kind: 'idle' });
    setExecution({ kind: 'idle' });
  }

  async function inspect() {
    setSpineStatus('Reading the Market, its manifest, the Direct program set, descriptor, and config at one finalized floor…');
    replayEpoch.current += 1;
    setSpine(null); setTakerReplay({ kind: 'idle' }); setExecution({ kind: 'idle' });
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

  async function previewIntent() {
    if ((ticketState.kind !== 'ready' && ticketState.kind !== 'crossed') || wallets.address === null || inspected === null) return;
    if (tradingProgramId === null || tradingProgramId === '') {
      setExecution({ kind: 'refused', reason: 'select the Trading program before asking the chain for your next Direct nonce' });
      return;
    }
    const epoch = replayEpoch.current + 1;
    replayEpoch.current = epoch;
    setTakerReplay({ kind: 'working' });
    try {
      setExecution({ kind: 'working', message: 'Reading your canonical maker replay account at one finalized floor for this preview…' });
      const client = new SolanaRpcClient(endpoint);
      const replay = await inspectDirectMakerNonceV1(client, {
        tradingProgram: tradingProgramId,
        market: inspected.marketAddress,
        generation: BigInt(inspected.generation),
        maker: wallets.address,
      });
      if (replayEpoch.current !== epoch) return;
      if (claimsProgramId === null || claimsProgramId === '') throw new Error('select the Claims program before signing');
      setTakerReplay({ kind: 'ready', observation: replay });
      setExecution({
        kind: 'refused',
        reason: 'Direct signing is unavailable until this Market publishes one canonical public route manifest and an exact finalized completion verifier. Nothing was saved, signed, or submitted.',
      });
    } catch (error) {
      if (replayEpoch.current !== epoch) return;
      setTakerReplay({ kind: 'idle' });
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
    <p className="direct-status">This public page does not accept a pasted route or ask for a signature yet. Direct execution stays unavailable until the Market publishes one canonical public route manifest and the page can verify the exact finalized receipt and poststate. Browser data is an untrusted projection; the onchain programs refuse substitutions.</p>

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

      <h3 className="detail-subhead">The other side&apos;s ticket</h3>
      <p className="direct-status">A trade here is two signed halves: yours and someone else&apos;s. There is no order book to take from — the other half arrives as a small ticket (dclutch/direct-intent-ticket/v1) you can be handed any way you like. Pasting it is safe: nothing in it is believed until the chain itself checks the signature.</p>
      <label><span>Ticket JSON</span><textarea rows={5} spellCheck={false} value={ticketText} onChange={(event) => { setTicketText(event.target.value); invalidateTakerReplay(); }} /></label>
      <div className="direct-form-grid">
        <label><span>My size · claim atoms (blank = take the ticket in full)</span><input inputMode="numeric" value={desired} onChange={(event) => { setDesired(event.target.value.trim()); invalidateTakerReplay(); }} /></label>
      </div>
      <WalletDirectory directory={wallets} purpose="taker identity, intent and payer signatures" onConnected={invalidateTakerReplay} />

      {ticketState.kind === 'refused' && <p className="market-refusal">Ticket refused: {ticketState.reason}</p>}
      {(ticketState.kind === 'ready' || ticketState.kind === 'crossed') && <div className="direct-actions">
        <button type="button" disabled={takerReplay.kind === 'working'} onClick={() => void previewIntent()}>
          {takerReplay.kind === 'working'
            ? 'Reading my next nonce…'
            : 'Read my next nonce and preview'}
        </button>
      </div>}
      {takerReplay.kind === 'ready' && <p className="direct-status">A future signed intent would use nonce {takerReplay.observation.nextNonce.toString()}, read at finalized slot {takerReplay.observation.observedSlot}. This page has not asked your wallet to sign it.</p>}
      {ticketState.kind === 'crossed' && <>
        {outcome !== null && outcome !== ticketState.ticket.intent.outcome && <p className="market-refusal">The ticket is for claim {ticketState.ticket.intent.outcome}, not the selected claim {outcome}. Crossing follows the ticket; select claim {ticketState.ticket.intent.outcome} or find another ticket.</p>}
        <div className="trade-v3-preview">
          <div><span>{ticketState.plan.takerSide === 'buy' ? 'You buy' : 'You sell'}</span><strong>{ticketState.plan.fill.toString()}</strong></div>
          <div><span>Execution price</span><strong>{ticketState.plan.executionPrice.toString()}</strong></div>
          <div><span>{ticketState.plan.takerSide === 'buy' ? 'Exact debit' : 'Exact credit'}</span><strong>{ticketState.plan.takerSide === 'buy' ? ticketState.plan.preview.buyerCollateralDebit.toString() : ticketState.plan.preview.sellerNetCollateralCredit.toString()}</strong></div>
          <div><span>Fee each side</span><strong>{ticketState.plan.preview.sellerFee.toString()}</strong></div>
          <p>{ticketState.plan.note} Preview uses the finalized observation slot; the on-chain interpreters remain authoritative.</p>
        </div>
      </>}

      <h3 className="detail-subhead">What stands between this preview and a real trade</h3>
      {inspected.walls.length === 0
        ? <p className="direct-status">Nothing — the route below can execute this trade.</p>
        : <ul className="market-bindings">{inspected.walls.map((wall) => (
          <li key={wall.name} className="check-fail"><span aria-hidden="true">×</span><div><strong>{wall.name}</strong><small>{wall.detail}</small></div></li>
        ))}</ul>}

      <details className="trade-v3-bytes">
        <summary>Why Direct execution is not available here yet</summary>
        <p className="direct-status">This public page does not accept a pasted route or ask for a signature. It will execute only after this Market publishes one canonical public route manifest and the client can authenticate that exact manifest at finalized commitment, save the exact chain, Market, owner, operation digest, intent, plan, and signed transaction id before submission, and verify the exact finalized receipt and poststate after submission.</p>
        <p className="direct-status">Browser data is an untrusted projection. The onchain programs still refuse substituted accounts, nonces, instructions, and resource state.</p>
        {execution.kind === 'working' && <p className="direct-status" aria-live="polite">{execution.message}</p>}
        {execution.kind === 'refused' && <p className="market-refusal">Refused: {execution.reason}</p>}
      </details>
    </>}
  </section>;
}
