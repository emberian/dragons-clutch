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
  inspectDirectSellerReadinessV1,
  type DirectParticipantCrossingAdmissionV1,
  type DirectParticipantReadinessV1,
} from '@/lib/directParticipant';
import {
  inspectDirectTradeSpineV1,
  type DirectTradeSpineV1,
} from '@/lib/directTradeSpine';
import {
  clearFinalizedClientOperationJournalV1,
  discardUnsignedClientOperationJournalV1,
  findClientOperationJournalV1,
  markClientOperationSubmittedV1,
  requireSubmittedSignatureMatchV1,
  submittedClientOperationWireV1,
  transactionSignatureV1,
  writeUnsignedClientOperationJournalV1,
  type ClientOperationJournalV1,
} from '@/lib/clientOperationJournal';
import {
  describeClaimChangeV1,
  directInlineJournalInputV1,
  directTradeBalanceChangesV1,
  directTradeFinalizedCompletionV1,
  type DirectTradeBalanceSnapshotV1,
} from '@/lib/directTradeJournal';
import { type MarketLiabilityV1 } from '@/lib/marketDiscovery';
import { publishedDirectRouteManifestV1 } from '@/lib/publishedRouteManifests';
import {
  requestWalletMessageSignatureV1,
  requestWalletTransactionSignatureV1,
  submitSignedTransactionV1,
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
 * workspace's format. This panel prepares, journals, wallet-signs, and — on a
 * cluster where mutation is admitted — submits the exact saved packet once,
 * then claims execution only from a finalized read-back of the connected
 * wallet's own Position. The journal discipline is redemption's, shared:
 * durable intent before key access, signature match on resume, never a
 * second send.
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
    takerBefore: DirectTradeBalanceSnapshotV1;
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
    journal: ClientOperationJournalV1;
    takerBefore: DirectTradeBalanceSnapshotV1;
  }>
  | Readonly<{
    kind: 'submitted';
    journal: ClientOperationJournalV1;
    signature: string;
    confirmation: string;
    takerBefore: DirectTradeBalanceSnapshotV1 | null;
  }>
  | Readonly<{
    kind: 'executed';
    signature: string;
    observedSlot: string;
    after: DirectTradeBalanceSnapshotV1;
    changes: ReturnType<typeof directTradeBalanceChangesV1> | null;
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

function browserStorage(): Storage {
  if (typeof window === 'undefined' || window.localStorage === undefined) throw new Error('this browser does not expose local recovery storage, so no wallet signature was requested');
  return window.localStorage;
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
  const publishedRoute = publishedDirectRouteManifestV1(marketAddress);
  const [routeText, setRouteText] = useState(publishedRoute ?? '');
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
    try {
      const admission = await rpc.assertMutationCluster();
      const saved = await findClientOperationJournalV1(
        browserStorage(),
        Object.freeze({ clusterGenesis: admission.genesisHash, market: marketAddress, owner: wallets.address }),
        'direct-inline-v3',
      );
      if (saved !== null && saved.phase === 'submitted') {
        setWalletPreparation({ kind: 'submitted', journal: saved, signature: saved.signature!, takerBefore: null, confirmation: 'A submitted Direct packet is saved for this exact chain, Market, and wallet. Resuming its signature; it is never sent twice.' });
        void pollDirectJournal(saved, null);
      }
    } catch {
      // Read-only cluster or no local storage: nothing could have been submitted from here.
    }
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
      // The seller is read as a SELLER, not as a second buyer: its collateral is
      // the Direct token account Trading derives and creates, and it has no
      // admission record to be missing. See `inspectDirectSellerReadinessV1`.
      const initialSeller = await inspectDirectSellerReadinessV1(client, participantRequest(ticketState.ticket.maker));
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
      const sellerParticipant = await inspectDirectSellerReadinessV1(client, participantRequest(ticketState.ticket.maker));
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
      if (takerParticipant.status !== 'ready') throw new Error('your participant accounts are not ready to trade, so a packet will not be prepared');
      const takerBefore = Object.freeze({
        positionBalances: takerParticipant.positionBalances,
        spendableCollateralAtoms: takerParticipant.spendableCollateralAtoms,
      });
      setWalletPreparation({ kind: 'wallet-preparable', preparation: prepared, takerTicket, takerBefore });
    } catch (error) {
      setWalletPreparation({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  async function signPreparedTransaction() {
    if (walletPreparation.kind !== 'wallet-preparable') return;
    const prepared = walletPreparation.preparation;
    const { takerTicket, takerBefore } = walletPreparation;
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
      const lookupTable = prepared.transactionPlan.transaction.message.addressTableLookups[0]?.accountKey.toBase58();
      if (lookupTable === undefined) throw new Error('prepared Direct packet omitted its authenticated lookup table');
      // Durable intent before key access: the exact packet is journaled in this
      // browser before the wallet is asked for a signature. A stale UNSIGNED
      // plan (no signature exists, nothing is ambiguous) is discarded; a
      // submitted-but-unresolved one refuses replacement inside the writer.
      const messageBytes = prepared.transactionPlan.transaction.message.serialize();
      const scope = Object.freeze({
        clusterGenesis: prepared.binding.genesisHash,
        market: marketAddress,
        owner: prepared.binding.connectedWallet,
      });
      const journalInput = await directInlineJournalInputV1(scope, takerTicket, {
        payer: prepared.payer,
        lookupTable,
        routeObservedSlot: prepared.binding.routeObservedSlot,
        blockhashObservedSlot: prepared.binding.blockhashObservedSlot.toString(),
        lastValidBlockHeight: prepared.binding.lastValidBlockHeight.toString(),
        messageBase64: base64(messageBytes),
      }, messageBytes);
      const existing = await findClientOperationJournalV1(browserStorage(), scope, 'direct-inline-v3');
      if (existing !== null && existing.phase === 'unsigned' && existing.operationDigest !== journalInput.operationDigest) {
        await discardUnsignedClientOperationJournalV1(browserStorage(), existing);
      }
      const journal = await writeUnsignedClientOperationJournalV1(browserStorage(), journalInput);
      setWalletPreparation({ kind: 'working', message: 'Saved the exact packet locally. Asking your wallet to sign it; nothing is submitted by signing.' });
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
      setWalletPreparation({
        kind: 'wallet-signed',
        signature: transactionSignatureV1(signed.transaction.signatures[0]!),
        signedWireBase64: base64(signed.wireBytes),
        messageBase64: base64(messageBytes),
        wireBytes: signed.wireBytes.length,
        routeObservedSlot: prepared.binding.routeObservedSlot,
        blockhashObservedSlot: prepared.binding.blockhashObservedSlot.toString(),
        lastValidBlockHeight: prepared.binding.lastValidBlockHeight.toString(),
        lookupTable,
        journal,
        takerBefore,
      });
    } catch (error) {
      setWalletPreparation({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  /** The participant read request, for reads that outlive one preparation closure. */
  function participantReadRequest(owner: string) {
    if (registryProgramId === null || claimsProgramId === null || tradingProgramId === null
        || custodyProgramId === null || rentProgramId === null) {
      throw new Error('this deployment does not name every program needed to authenticate participant accounts');
    }
    return Object.freeze({
      market: marketAddress,
      owner,
      coreProgram: coreProgramId,
      registryProgram: registryProgramId,
      claimsProgram: claimsProgramId,
      tradingProgram: tradingProgramId,
      custodyProgram: custodyProgramId,
      rentProgram: rentProgramId,
    });
  }

  /** Poll one submitted Direct signature to finalized truth, then show the Position change. */
  async function pollDirectJournal(
    journal: ClientOperationJournalV1,
    takerBefore: DirectTradeBalanceSnapshotV1 | null,
  ): Promise<void> {
    const signature = journal.signature;
    if (journal.phase !== 'submitted' || signature === null) throw new Error('Direct recovery requires one submitted signature');
    const client = new SolanaRpcClient(endpoint);
    for (let attempt = 0; attempt < 30; attempt += 1) {
      try {
        const status = (await client.signatureStatuses([signature]))[0];
        if (status?.known && status.succeeded === false) {
          setWalletPreparation({ kind: 'submitted', journal, signature, takerBefore, confirmation: `The chain reports an error (${status.errorText ?? 'unnamed chain error'}). The submitted record stays saved because it cannot be safely replayed or discarded.` });
          return;
        }
        if (directTradeFinalizedCompletionV1(status)) {
          const readiness = await inspectDirectParticipantReadinessV1(client, participantReadRequest(journal.owner));
          if (readiness.status !== 'ready') throw new Error(`the crossing finalized but your participant accounts read back ${readiness.status}: ${readiness.reason}`);
          const afterSnapshot = Object.freeze({
            positionBalances: readiness.positionBalances,
            spendableCollateralAtoms: readiness.spendableCollateralAtoms,
          });
          await clearFinalizedClientOperationJournalV1(browserStorage(), journal);
          setWalletPreparation({
            kind: 'executed',
            signature,
            observedSlot: readiness.observedSlot,
            after: afterSnapshot,
            changes: takerBefore === null ? null : directTradeBalanceChangesV1(takerBefore, afterSnapshot),
          });
          return;
        }
        setWalletPreparation({ kind: 'submitted', journal, signature, takerBefore, confirmation: 'The exact signature is not finalized yet. You can close this page; reloading resumes this signature and never submits it again.' });
      } catch (error) {
        setWalletPreparation({ kind: 'submitted', journal, signature, takerBefore, confirmation: `${errorMessage(error)} The submitted record stays saved; reloading only resumes its exact signature.` });
        return;
      }
      await new Promise((resolve) => setTimeout(resolve, 1_500));
    }
    setWalletPreparation({ kind: 'submitted', journal, signature, takerBefore, confirmation: 'Finalized completion is still unresolved. You can reload later; this exact signature stays saved and is never replayed.' });
  }

  async function submitDirectPacket() {
    if (walletPreparation.kind !== 'wallet-signed') return;
    const { journal, signature, signedWireBase64, takerBefore, lastValidBlockHeight } = walletPreparation;
    let submitted: ClientOperationJournalV1 | null = null;
    try {
      const client = new SolanaRpcClient(endpoint);
      const admission = await client.assertMutationCluster();
      if (admission.genesisHash !== journal.clusterGenesis) throw new Error('RPC genesis changed after the packet was signed; it must not be submitted here');
      const currentHeight = BigInt(await client.blockHeight());
      if (currentHeight > BigInt(lastValidBlockHeight)) {
        throw new Error(`the signed packet expired at block height ${lastValidBlockHeight}; the chain can no longer include it`);
      }
      const wireBytes = Uint8Array.from(atob(signedWireBase64), (character) => character.charCodeAt(0));
      submitted = await markClientOperationSubmittedV1(browserStorage(), journal, signature, wireBytes);
      setWalletPreparation({ kind: 'submitted', journal: submitted, signature, takerBefore, confirmation: 'Saved before submission; sending the exact signed packet…' });
      const returned = await submitSignedTransactionV1(client, submittedClientOperationWireV1(submitted));
      requireSubmittedSignatureMatchV1(signature, returned);
      await pollDirectJournal(submitted, takerBefore);
    } catch (error) {
      if (submitted !== null) setWalletPreparation({ kind: 'submitted', journal: submitted, signature, takerBefore, confirmation: `${errorMessage(error)} The submitted record stays saved; reloading never resubmits it.` });
      else setWalletPreparation({ kind: 'refused', reason: errorMessage(error) });
    }
  }

  const supplies = liability !== null && liability.status === 'bound' ? liability.supplyAtoms : null;

  return <section className="trade-v3-card">
    <header><span>06</span><div><h2>Trade this market</h2><p>Pick an outcome, choose how much, and take one signed offer at the price its maker set.</p></div></header>

    <div className="direct-actions">
      <button type="button" onClick={() => void inspect()}>Ask the chain about trading here</button>
      <Anchor className="secondary-action" href="/trade">Advanced: full route workbench →</Anchor>
    </div>
    <p className="direct-status" aria-live="polite">{spineStatus}</p>
    <p className="direct-status">Signing sends nothing. Sending is a separate step you take, and it happens once — reload part-way through and this page picks up the transaction you already sent rather than sending a second one.</p>
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
      <p className="direct-status">A trade here is two signed halves: yours and someone else&apos;s. There is no order book — the other half reaches you as a small ticket (dclutch/direct-intent-ticket/v1), passed along any way you like.</p>
      <label><span>Ticket JSON</span><textarea rows={5} spellCheck={false} value={ticketText} onChange={(event) => { setTicketText(event.target.value); invalidatePreview(); }} /></label>
      <div className="direct-form-grid">
        <label><span>My size · claim atoms (blank = take the ticket in full)</span><input inputMode="numeric" value={desired} onChange={(event) => { setDesired(event.target.value.trim()); invalidatePreview(); }} /></label>
      </div>
      <WalletDirectory directory={wallets} onConnected={invalidateWalletState} />

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
      {execution.kind === 'ready' && <p className="direct-status">Unsigned preview. Nothing is signed until you continue below.</p>}

      <h3 className="detail-subhead">What stands between this preview and a real trade</h3>
      {inspected.walls.length === 0
        ? <p className="direct-status">This bounded chain inspection found no Market-state wall. Signing still waits for the published route and completion verifier named above.</p>
        : <ul className="market-bindings">{inspected.walls.map((wall) => (
          <li key={wall.name} className="check-fail"><span aria-hidden="true">×</span><div><strong>{wall.name}</strong><small>{wall.detail}</small></div></li>
        ))}</ul>}

      <details className="trade-v3-bytes">
        <summary>Prepare the exact wallet handoff</summary>
        <p className="direct-status">Paste the route file the operator published for this market (a <code>dclutch-direct-hot-route-manifest-v3</code>).</p>
        {publishedRoute !== null && routeText === publishedRoute && <p className="direct-status">Pre-filled with the operator&apos;s published route for this market. You can replace it.</p>}
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
          <span>Wallet signed · saved locally, not yet submitted</span>
          <strong>{walletPreparation.signature}</strong>
          <p>{walletPreparation.wireBytes} bytes. Route slot {walletPreparation.routeObservedSlot}; blockhash slot {walletPreparation.blockhashObservedSlot}; expires at block height {walletPreparation.lastValidBlockHeight}. Frozen table {walletPreparation.lookupTable}. The exact packet is saved in this browser; nothing has been sent to RPC.</p>
          <label><span>Exact signed packet · base64</span><textarea readOnly rows={6} value={walletPreparation.signedWireBase64} /></label>
          <label><span>Exact v0 message · base64</span><textarea readOnly rows={5} value={walletPreparation.messageBase64} /></label>
          <div className="direct-actions"><button type="button" onClick={() => void submitDirectPacket()}>Submit this exact packet and watch it finalize</button></div>
          <p>Submitting sends this one saved packet once, then reads your Position back at finalized commitment. If this page closes mid-flight, reloading resumes the saved signature and never sends a second packet.</p>
        </div>}
        {walletPreparation.kind === 'submitted' && <div className="portfolio-claim">
          <span>Submitted · awaiting finalized truth</span>
          <strong>{walletPreparation.signature}</strong>
          <p aria-live="polite">{walletPreparation.confirmation}</p>
        </div>}
        {walletPreparation.kind === 'executed' && <div className="portfolio-claim">
          <span>Executed · finalized</span>
          <strong>{walletPreparation.signature}</strong>
          <p>Finalized, read back at slot {walletPreparation.observedSlot}. Your Position now holds:</p>
          <ul className="market-bindings">
            {walletPreparation.changes === null
              ? walletPreparation.after.positionBalances.map((balance, index) => <li key={index}>claim {index}: {balance.toString()} atoms</li>)
              : walletPreparation.changes.claims.map((change) => <li key={change.claimIndex}>{describeClaimChangeV1(change)}</li>)}
          </ul>
          {walletPreparation.changes !== null && <p>Spendable collateral: {walletPreparation.changes.spendableBefore.toString()} → {walletPreparation.changes.spendableAfter.toString()} atoms.{walletPreparation.changes.moved ? '' : ' Nothing moved — the finalized crossing changed no balance, and that is reported as exactly that.'}</p>}
        </div>}
        <p className="direct-status">The signed packet is saved in this browser before its one send, so a reload picks it up rather than sending twice.</p>
      </details>
    </>}
  </section>;
}
