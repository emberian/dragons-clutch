'use client';

import Anchor from '@/components/Anchor';
import { useMemo, useState } from 'react';

import WalletDirectory, { useWalletDirectoryV1 } from '@/components/WalletDirectory';
import {
  encodeCompactIntentSigningMessageV2,
  type SignedDirectIntentV3,
} from '@/lib/directInlineV3';
import { decodeDirectIntentTicketV1, encodeDirectIntentTicketV1 } from '@/lib/directTicket';
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
import { transactionSignatureV1 } from '@/lib/clientOperationJournal';
import { type MarketLiabilityV1 } from '@/lib/marketDiscovery';
import {
  requestWalletMessageSignatureV1,
  requestWalletTransactionSignatureV1,
} from '@/lib/walletHandoff';
import { inspectDirectHotRouteManifestJsonV3 } from '@dclutch/sdk/directHotRouteManifest';
import {
  inspectDirectMakerNoncePairV1,
  inspectDirectMakerNonceV1,
} from '@dclutch/sdk/directMakerReplay';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';
import { planDirectCrossingV1, type DirectCrossingPlanV1 } from '@dclutch/sdk/directTicket';
import {
  prepareDirectWalletTransactionV1,
  type DirectWalletChainContextV1,
  type DirectWalletPreparationV1,
} from '@dclutch/sdk/directWalletPreparationV1';

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
 * workspace's format. This panel may prepare and wallet-sign exact bytes, but
 * deliberately stops before any submission or execution claim.
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

type WalletPreparationState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'working'; message: string }>
  | Readonly<{ kind: 'refused'; reason: string }>
  | Readonly<{
    kind: 'wallet-preparable';
    preparation: Extract<DirectWalletPreparationV1, { status: 'wallet-preparable' }>;
    takerTicket: string;
  }>
  | Readonly<{
    kind: 'operator-required';
    payer: string;
    reason: string;
    takerTicket: string;
    routeObservedSlot: string;
    lastValidBlockHeight: string;
  }>
  | Readonly<{
    kind: 'wallet-signed';
    signature: string;
    signedWireBase64: string;
    messageBase64: string;
    wireBytes: number;
    routeObservedSlot: string;
    blockhashObservedSlot: string;
    lastValidBlockHeight: string;
    lookupTable: string;
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

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += 8_192) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 8_192));
  }
  return btoa(binary);
}

function chainContext(
  admission: Readonly<{ endpoint: string; genesisHash: string }>,
): DirectWalletChainContextV1 {
  return Object.freeze({ rpcEndpoint: admission.endpoint, genesisHash: admission.genesisHash });
}

function sameChain(
  left: Readonly<{ endpoint: string; genesisHash: string }>,
  right: Readonly<{ endpoint: string; genesisHash: string }>,
): void {
  if (left.endpoint !== right.endpoint || left.genesisHash !== right.genesisHash) {
    throw new Error('RPC endpoint or genesis changed while the Direct route was being authenticated');
  }
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
  const [routeText, setRouteText] = useState('');
  const [execution, setExecution] = useState<ExecutionState>({ kind: 'idle' });
  const [walletPreparation, setWalletPreparation] = useState<WalletPreparationState>({ kind: 'idle' });

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
    setWalletPreparation({ kind: 'idle' });
  }

  function invalidateWalletState(): void {
    setParticipant(null);
    setParticipantStatus('Your wallet changed. Ask the chain again before previewing a crossing.');
    invalidatePreview();
  }

  async function inspect() {
    setSpineStatus('Reading the Market, its manifest, the Direct program set, descriptor, and config at one finalized floor…');
    setSpine(null); setParticipant(null); setExecution({ kind: 'idle' }); setWalletPreparation({ kind: 'idle' });
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

  async function prepareWalletIntent() {
    if (ticketState.kind !== 'ready' || execution.kind !== 'ready' || wallets.address === null) return;
    if (ticketState.ticket.intent.side !== 0) {
      setWalletPreparation({
        kind: 'refused',
        reason: 'Wallet preparation V1 accepts a portable sell ticket and your connected wallet as buyer. This buy ticket remains a valid read-only preview, but this caller will not silently reverse its participant roles.',
      });
      return;
    }
    if (routeText.trim() === '') {
      setWalletPreparation({ kind: 'refused', reason: 'Paste the operator-published Direct Hot route manifest before asking your wallet to sign.' });
      return;
    }
    if (registryProgramId === null || claimsProgramId === null || tradingProgramId === null
        || custodyProgramId === null || rentProgramId === null) {
      setWalletPreparation({ kind: 'refused', reason: 'This deployment does not name every program needed to authenticate both participants.' });
      return;
    }
    const connectedWallet = wallets.address;
    const client = new SolanaRpcClient(endpoint);
    const participantRequest = (owner: string) => Object.freeze({
      market: marketAddress,
      owner,
      coreProgram: coreProgramId,
      registryProgram: registryProgramId,
      claimsProgram: claimsProgramId,
      tradingProgram: tradingProgramId,
      custodyProgram: custodyProgramId,
      rentProgram: rentProgramId,
    });
    try {
      setWalletPreparation({ kind: 'working', message: 'Authenticating the route, both participants, and both replay nonces before asking for your intent signature…' });
      const planningAdmission = await client.assertMutationCluster();
      const initialRoute = await inspectDirectHotRouteManifestJsonV3(client, routeText);
      const initialRouteAdmission = await client.assertMutationCluster();
      sameChain(planningAdmission, initialRouteAdmission);
      if (initialRoute.route.market !== marketAddress || initialRoute.route.tradingProgram !== tradingProgramId) {
        throw new Error('route manifest authenticates another Market or Trading program');
      }
      const initialSeller = await inspectDirectParticipantReadinessV1(client, participantRequest(ticketState.ticket.maker));
      const initialTaker = await inspectDirectParticipantReadinessV1(client, participantRequest(connectedWallet));
      if (initialSeller.status !== 'ready' || initialTaker.status !== 'ready') {
        throw new Error(`both participants must be ready before signing: seller is ${initialSeller.status}; you are ${initialTaker.status}`);
      }
      const initialNonces = await inspectDirectMakerNoncePairV1(client, [
        {
          tradingProgram: initialRoute.route.tradingProgram,
          market: initialRoute.route.market,
          generation: initialRoute.route.generation,
          maker: ticketState.ticket.maker,
        },
        {
          tradingProgram: initialRoute.route.tradingProgram,
          market: initialRoute.route.market,
          generation: initialRoute.route.generation,
          maker: connectedWallet,
        },
      ]);
      const crossingPlan = planDirectCrossingV1({
        route: initialRoute.route,
        ticket: ticketState.ticket,
        takerAddress: connectedWallet,
        takerReplay: initialNonces[1],
        takerCollateralAccount: initialTaker.coordinates.collateral,
        desiredFill: positiveU64(desired, ticketState.ticket.intent.maximumFill),
        clockSlot: BigInt(initialNonces[1].observedSlot),
      });
      if (initialSeller.positionBalances[crossingPlan.taker.outcome] === undefined
          || initialSeller.positionBalances[crossingPlan.taker.outcome]! < crossingPlan.fill) {
        throw new Error('the ticket seller’s finalized Position does not cover this fill');
      }
      admitDirectParticipantCrossingV1(initialTaker, crossingPlan);
      setWalletPreparation({ kind: 'working', message: 'The chain checks passed. Your wallet will now sign only your detached Direct intent; this is not a transaction or submission.' });
      const takerSignature = await requestWalletMessageSignatureV1(
        client,
        wallets.handoff(endpoint),
        connectedWallet,
        encodeCompactIntentSigningMessageV2(crossingPlan.taker),
      );
      const signedTaker: SignedDirectIntentV3 = Object.freeze({
        maker: connectedWallet,
        signature: takerSignature,
        intent: crossingPlan.taker,
      });

      setWalletPreparation({ kind: 'working', message: 'Intent signed. Reacquiring the route, both participants, and both nonces before compiling anything…' });
      const routeBefore = await client.assertMutationCluster();
      const routeInspection = await inspectDirectHotRouteManifestJsonV3(client, routeText);
      const routeAfter = await client.assertMutationCluster();
      sameChain(routeBefore, routeAfter);
      sameChain(planningAdmission, routeAfter);
      const sellerContextAdmission = await client.assertMutationCluster();
      const sellerParticipant = await inspectDirectParticipantReadinessV1(client, participantRequest(ticketState.ticket.maker));
      const takerContextAdmission = await client.assertMutationCluster();
      const takerParticipant = await inspectDirectParticipantReadinessV1(client, participantRequest(connectedWallet));
      const nonceContextAdmission = await client.assertMutationCluster();
      const noncePair = await inspectDirectMakerNoncePairV1(client, [
        {
          tradingProgram: routeInspection.route.tradingProgram,
          market: routeInspection.route.market,
          generation: routeInspection.route.generation,
          maker: ticketState.ticket.maker,
        },
        {
          tradingProgram: routeInspection.route.tradingProgram,
          market: routeInspection.route.market,
          generation: routeInspection.route.generation,
          maker: connectedWallet,
        },
      ]);
      const currentAdmission = await client.assertMutationCluster();
      for (const admission of [sellerContextAdmission, takerContextAdmission, nonceContextAdmission, currentAdmission]) {
        sameChain(planningAdmission, admission);
      }
      const currentFinalizedSlot = BigInt(await client.finalizedSlot());
      const currentBlockHeight = BigInt(await client.blockHeight(currentFinalizedSlot.toString()));
      const prepared = prepareDirectWalletTransactionV1({
        routeInspection,
        ticketInspection: ticketState.ticket,
        crossingPlan,
        sellerParticipant,
        takerParticipant,
        noncePair,
        signedSeller: ticketState.ticket,
        signedTaker,
        context: Object.freeze({
          route: chainContext(routeAfter),
          sellerParticipant: chainContext(sellerContextAdmission),
          takerParticipant: chainContext(takerContextAdmission),
          noncePair: chainContext(nonceContextAdmission),
          planning: Object.freeze({ ...chainContext(planningAdmission), connectedWallet }),
          current: Object.freeze({
            ...chainContext(currentAdmission),
            connectedWallet,
            finalizedSlot: currentFinalizedSlot,
            blockHeight: currentBlockHeight,
          }),
        }),
      });
      const takerTicket = encodeDirectIntentTicketV1(signedTaker);
      if (prepared.status === 'operator-required') {
        setWalletPreparation({
          kind: 'operator-required',
          payer: prepared.payer,
          reason: prepared.reason,
          takerTicket,
          routeObservedSlot: prepared.binding.routeObservedSlot,
          lastValidBlockHeight: prepared.binding.lastValidBlockHeight.toString(),
        });
        return;
      }
      setWalletPreparation({ kind: 'wallet-preparable', preparation: prepared, takerTicket });
    } catch (error) {
      setWalletPreparation({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  async function signPreparedTransaction() {
    if (walletPreparation.kind !== 'wallet-preparable') return;
    const prepared = walletPreparation.preparation;
    try {
      if (wallets.address !== prepared.binding.connectedWallet) throw new Error('connected wallet changed after Direct preparation');
      const client = new SolanaRpcClient(endpoint);
      const admission = await client.assertMutationCluster();
      if (admission.endpoint !== prepared.binding.rpcEndpoint || admission.genesisHash !== prepared.binding.genesisHash) {
        throw new Error('RPC endpoint or genesis changed after Direct preparation');
      }
      const blockHeight = BigInt(await client.blockHeight(prepared.binding.currentFinalizedSlot.toString()));
      if (blockHeight > prepared.binding.lastValidBlockHeight) {
        throw new Error(`prepared Direct blockhash expired at block height ${prepared.binding.lastValidBlockHeight}`);
      }
      setWalletPreparation({ kind: 'working', message: 'Asking your wallet to sign the exact 1,159-byte v0 packet. The page will not submit it.' });
      const signed = await requestWalletTransactionSignatureV1(
        client,
        wallets.handoff(endpoint),
        prepared.transactionPlan.transaction,
        prepared.payer,
      );
      if (!signed.complete) throw new Error('wallet did not complete the sole required payer signature');
      const after = await client.assertMutationCluster();
      if (after.endpoint !== prepared.binding.rpcEndpoint || after.genesisHash !== prepared.binding.genesisHash) {
        throw new Error('RPC endpoint or genesis changed while the wallet signed');
      }
      const afterHeight = BigInt(await client.blockHeight(prepared.binding.currentFinalizedSlot.toString()));
      if (afterHeight > prepared.binding.lastValidBlockHeight) {
        throw new Error(`signed Direct packet expired at block height ${prepared.binding.lastValidBlockHeight}; it must not be submitted`);
      }
      const lookupTable = prepared.transactionPlan.transaction.message.addressTableLookups[0]?.accountKey.toBase58();
      if (lookupTable === undefined) throw new Error('prepared Direct packet omitted its authenticated lookup table');
      setWalletPreparation({
        kind: 'wallet-signed',
        signature: transactionSignatureV1(signed.transaction.signatures[0]!),
        signedWireBase64: base64(signed.wireBytes),
        messageBase64: base64(signed.transaction.message.serialize()),
        wireBytes: signed.wireBytes.length,
        routeObservedSlot: prepared.binding.routeObservedSlot,
        blockhashObservedSlot: prepared.binding.blockhashObservedSlot.toString(),
        lastValidBlockHeight: prepared.binding.lastValidBlockHeight.toString(),
        lookupTable,
      });
    } catch (error) {
      setWalletPreparation({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  const supplies = liability !== null && liability.status === 'bound' ? liability.supplyAtoms : null;

  return <section className="trade-v3-card">
    <header><span>06</span><div><h2>Trade this Market</h2><p>Pick an outcome, size it, and cross one signed offer at the price its maker signed. Every number you see is read off the chain or computed by the exact code the chain runs. When something cannot happen yet, this panel tells you exactly why in one sentence — never a greyed-out button with no reason.</p></div></header>

    <div className="direct-actions">
      <button type="button" onClick={() => void inspect()}>Ask the chain about trading here</button>
      <Anchor className="secondary-action" href="/trade">Advanced: full route workbench →</Anchor>
    </div>
    <p className="direct-status" aria-live="polite">{spineStatus}</p>
    <p className="direct-status">This page can authenticate both participants, the checked Hot route, its frozen lookup table, and both replay nonces. If the connected wallet is the route payer, it can prepare and ask your wallet to sign the exact v0 packet. If an operator is the payer, it names that next actor instead. There is no submit button here, and a signed packet is never described as an executed trade. A Claims Position holds claim balances; it is never used as your collateral account. Browser data is an untrusted projection; the onchain programs remain authoritative.</p>
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
      {execution.kind === 'ready' && <p className="direct-status">This is an unsigned preview, not a submitted trade. Your wallet has not signed this intent unless you explicitly continue below.</p>}

      <h3 className="detail-subhead">What stands between this preview and a real trade</h3>
      {inspected.walls.length === 0
        ? <p className="direct-status">This bounded chain inspection found no Market-state wall. Signing still waits for the published route and completion verifier named above.</p>
        : <ul className="market-bindings">{inspected.walls.map((wall) => (
          <li key={wall.name} className="check-fail"><span aria-hidden="true">×</span><div><strong>{wall.name}</strong><small>{wall.detail}</small></div></li>
        ))}</ul>}

      <details className="trade-v3-bytes">
        <summary>Prepare the exact wallet handoff</summary>
        <p className="direct-status">Paste the operator-published <code>dclutch-direct-hot-route-manifest-v3</code>. The reader hostile-decodes the bounded JSON, reacquires the 39 named accounts plus the frozen lookup table, authenticates its checked release and capability seal, and then rechecks both participants and both nonces after your detached intent signature.</p>
        <label><span>Checked Direct Hot route manifest · JSON</span><textarea rows={7} spellCheck={false} value={routeText} onChange={(event) => { setRouteText(event.target.value); setWalletPreparation({ kind: 'idle' }); }} /></label>
        <div className="direct-actions">
          <button
            type="button"
            disabled={execution.kind !== 'ready' || walletPreparation.kind === 'working'}
            onClick={() => void prepareWalletIntent()}
          >Sign my intent, then authenticate the packet</button>
        </div>
        {walletPreparation.kind === 'working' && <p className="direct-status" aria-live="polite">{walletPreparation.message}</p>}
        {walletPreparation.kind === 'refused' && <p className="market-refusal">Refused: {walletPreparation.reason}</p>}
        {walletPreparation.kind === 'operator-required' && <div className="portfolio-claim">
          <span>Your intent is signed. Nothing has executed.</span>
          <strong>Route payer {walletPreparation.payer}</strong>
          <p>{walletPreparation.reason} The authenticated route was observed at slot {walletPreparation.routeObservedSlot}; its blockhash expires at block height {walletPreparation.lastValidBlockHeight}. Give the exact signed taker ticket below to that payer. This page has not built, signed, or submitted a transaction.</p>
          <label><span>Your signed taker ticket</span><textarea readOnly rows={7} value={walletPreparation.takerTicket} /></label>
        </div>}
        {walletPreparation.kind === 'wallet-preparable' && <div className="portfolio-claim">
          <span>Wallet-preparable · not signed as a transaction</span>
          <strong>{walletPreparation.preparation.transactionPlan.wireBytes.length} bytes · {walletPreparation.preparation.transactionPlan.loadedAddresses} LUT addresses · 61 unique keys</strong>
          <p>Route slot {walletPreparation.preparation.binding.routeObservedSlot}; blockhash slot {walletPreparation.preparation.binding.blockhashObservedSlot.toString()}; expires at block height {walletPreparation.preparation.binding.lastValidBlockHeight.toString()}. Frozen table {walletPreparation.preparation.transactionPlan.transaction.message.addressTableLookups[0]?.accountKey.toBase58()}.</p>
          <label><span>Exact unsigned v0 message · base64</span><textarea readOnly rows={5} value={base64(walletPreparation.preparation.transactionPlan.transaction.message.serialize())} /></label>
          <div className="direct-actions"><button type="button" onClick={() => void signPreparedTransaction()}>Ask my wallet to sign this exact packet</button></div>
          <p>This request still does not submit. Your wallet must preserve the exact message bytes; any rewrite is refused.</p>
        </div>}
        {walletPreparation.kind === 'wallet-signed' && <div className="portfolio-claim">
          <span>Wallet signed · prepared, not submitted</span>
          <strong>{walletPreparation.signature}</strong>
          <p>{walletPreparation.wireBytes} bytes. Route slot {walletPreparation.routeObservedSlot}; blockhash slot {walletPreparation.blockhashObservedSlot}; expires at block height {walletPreparation.lastValidBlockHeight}. Frozen table {walletPreparation.lookupTable}. Nothing has been sent to RPC.</p>
          <label><span>Exact signed packet · base64</span><textarea readOnly rows={6} value={walletPreparation.signedWireBase64} /></label>
          <label><span>Exact v0 message · base64</span><textarea readOnly rows={5} value={walletPreparation.messageBase64} /></label>
        </div>}
        <p className="direct-status">The next protocol step after a prepared packet is an explicitly authorized durable submit/finalization caller. This page does not pretend that boundary has occurred.</p>
      </details>
    </>}
  </section>;
}
