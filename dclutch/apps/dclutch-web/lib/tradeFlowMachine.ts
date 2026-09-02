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
  directInlineJournalInputV1,
  directTradeBalanceChangesV1,
  directTradeFinalizedCompletionV1,
  type DirectTradeBalanceSnapshotV1,
} from '@/lib/directTradeJournal';
import { parseQuantityV1, type DenominationV1 } from '@/lib/quantity';
import {
  requestWalletMessageSignatureV1,
  requestWalletTransactionSignatureV1,
  submitSignedTransactionV1,
} from '@/lib/walletHandoff';
import { type WalletDirectoryHandleV1 } from '@/components/WalletDirectory';
import { inspectDirectHotRouteManifestJsonV3 } from '@dclutch/sdk/directHotRouteManifest';
import {
  inspectDirectMakerNoncePairV1,
  inspectDirectMakerNonceV1,
} from '@dclutch/sdk/directMakerReplay';
// THE SUBMITTING CLIENT, DELIBERATELY NOT THE PACKAGE'S.
//
// `@dclutch/sdk/rpc` ships a read-only client and `lib/publicSurface.test.ts`
// enforces that: `sendRawTransaction` is on its forbidden list, asserted absent
// from the root, the subpath, the prototype and an instance, and a synthetic
// outside consumer is typechecked to prove calling it does not compile. The
// package is a reader; submission belongs to a surface that owns a durable
// journal. This file is one of those surfaces -- it writes the operation
// journal before it sends and never resubmits on reload -- and the browser's
// own `lib/rpc.ts` is the client that can send, which is exactly why that twin
// is on the deliberate-divergence list rather than being absorbed.
//
// It was importing the reader anyway, so `submitSignedTransactionV1` was
// handed a client structurally incapable of submitting and the whole Direct
// trade could be signed and never sent. The only sign was a type error nobody
// owned, and adding the method to the package instead would have broken the
// invariant rather than the habit.
import { SolanaRpcClient } from '@/lib/rpc';
import { planDirectCrossingV1, type DirectCrossingPlanV1 } from '@dclutch/sdk/directTicket';
import {
  prepareDirectWalletTransactionV1,
  type DirectWalletChainContextV1,
  type DirectWalletPreparationV1,
} from '@dclutch/sdk/directWalletPreparationV1';

/**
 * The Direct trade flow, as a machine.
 *
 * This module holds the orchestration that used to live inside
 * MarketTradePanel, and it holds it UNCHANGED. That is the point: the eight
 * functions below implement durable intent before key access, signature match
 * on resume, never a second send, and five separate re-checks that the chain
 * under the flow is still the chain the flow started on. That discipline is
 * the product, it has been audited where it stands, and a redesign of the
 * panel is not a licence to rewrite it. What moved is the code's ADDRESS, not
 * its behaviour -- the rendering that surrounds it is free to change, and the
 * conditions in here are not.
 *
 * The factory shape is what makes the move exact. React rebuilds a component's
 * closures on every render, so a function that captured `participant` from one
 * render captured that render's value; calling createDirectTradeFlowMachineV1
 * once per render reproduces that capture semantics precisely, and each body
 * keeps reading the same free names it always read.
 */
export type TicketState =
  | Readonly<{ kind: 'none' }>
  | Readonly<{ kind: 'refused'; reason: string }>
  | Readonly<{ kind: 'ready'; ticket: SignedDirectIntentV3 }>;

export type ExecutionState =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'working'; message: string }>
  | Readonly<{ kind: 'refused'; reason: string }>
  | Readonly<{
    kind: 'ready';
    plan: DirectCrossingPlanV1;
    admission: DirectParticipantCrossingAdmissionV1;
    replaySlot: string;
  }>;

export type WalletPreparationState =
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
function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'the step refused without a usable reason';
}

export function base64(bytes: Uint8Array): string {
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
/**
 * The fill this trader asked for, in exact atoms.
 *
 * Blank still means "take the ticket in full", which is why the fallback is
 * the ticket's own maximumFill and why the input's placeholder says so. A
 * typed size is read in the DISPLAY denomination and converted, because the
 * label above it now says `claims` -- see parseQuantityV1 for why the two must
 * move together.
 */
function desiredFillAtomsV1(text: string, fallback: bigint, denomination: DenominationV1): bigint {
  if (text === '') return fallback;
  return parseQuantityV1(text, denomination);
}

/**
 * The exact twin of a humanized quantity, rendered VISIBLY rather than hidden
 * behind a hover: touch devices have no hover, and a humanized number must
 * never be the only number on screen. When the mint's precision was never
 * read the display is ALREADY the grouped raw integer, so this labels it as
 * atoms rather than restating it.
 */

/**
 * The ticket the flow is holding, decoded from whatever transport delivered
 * it. Pure: the component still owns the memo and its dependency list, so the
 * recomputation points are unchanged.
 */
export function directTicketStateV1(input: Readonly<{
  inspected: Extract<DirectTradeSpineV1, { status: 'inspected' }> | null;
  ticketText: string;
  wallets: WalletDirectoryHandleV1;
  claimsProgramId: string | null;
}>): TicketState {
  const { inspected, ticketText, wallets, claimsProgramId } = input;
  if (inspected === null || ticketText.trim() === '') return Object.freeze({ kind: 'none' as const });
  if (wallets.address === null) return Object.freeze({ kind: 'refused' as const, reason: 'connect a browser wallet: the ticket is crossed against the connected identity' });
  if (claimsProgramId === null || claimsProgramId === '') return Object.freeze({ kind: 'refused' as const, reason: 'select the Claims program before checking the participant admission evidence' });
  try {
    const ticket = decodeDirectIntentTicketV1(ticketText.trim());
    return Object.freeze({ kind: 'ready' as const, ticket });
  } catch (error) {
    return Object.freeze({ kind: 'refused' as const, reason: errorMessage(error) });
  }
}

/**
 * Everything the machine's functions close over: the deployment's program
 * set, the connected wallet, this render's state, and the setters that move
 * it. Rebuilt each render, exactly as the component's own closures were.
 */
export type DirectTradeFlowContextV1 = Readonly<{
  endpoint: string;
  marketAddress: string;
  coreProgramId: string;
  registryProgramId: string | null;
  claimsProgramId: string | null;
  tradingProgramId: string | null;
  custodyProgramId: string | null;
  rentProgramId: string | null;
  denomination: DenominationV1;
  wallets: WalletDirectoryHandleV1;
  inspected: Extract<DirectTradeSpineV1, { status: 'inspected' }> | null;
  participant: DirectParticipantReadinessV1 | null;
  outcome: number | null;
  desired: string;
  routeText: string;
  ticketState: TicketState;
  execution: ExecutionState;
  walletPreparation: WalletPreparationState;
  setSpine: (next: DirectTradeSpineV1 | null) => void;
  setSpineStatus: (next: string) => void;
  setParticipant: (next: DirectParticipantReadinessV1 | null) => void;
  setParticipantStatus: (next: string) => void;
  setExecution: (next: ExecutionState) => void;
  setWalletPreparation: (next: WalletPreparationState) => void;
}>;

/** The flow's callable surface. Every member is lifted, not reimplemented. */
export type DirectTradeFlowMachineV1 = Readonly<{
  invalidatePreview: () => void;
  invalidateWalletState: () => void;
  inspect: () => Promise<void>;
  previewIntent: () => Promise<void>;
  prepareWalletIntent: () => Promise<void>;
  signPreparedTransaction: () => Promise<void>;
  pollDirectJournal: (journal: ClientOperationJournalV1, takerBefore: DirectTradeBalanceSnapshotV1 | null) => Promise<void>;
  submitDirectPacket: () => Promise<void>;
}>;

export function createDirectTradeFlowMachineV1(context: DirectTradeFlowContextV1): DirectTradeFlowMachineV1 {
  const {
    endpoint, marketAddress, coreProgramId, registryProgramId, claimsProgramId,
    tradingProgramId, custodyProgramId, rentProgramId, denomination, wallets,
    inspected, participant, outcome, desired, routeText,
    ticketState, execution, walletPreparation,
    setSpine, setSpineStatus, setParticipant, setParticipantStatus,
    setExecution, setWalletPreparation,
  } = context;

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
        desiredFill: desiredFillAtomsV1(desired, ticketState.ticket.intent.maximumFill, denomination),
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
        desiredFill: desiredFillAtomsV1(desired, ticketState.ticket.intent.maximumFill, denomination),
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

  return Object.freeze({
    invalidatePreview,
    invalidateWalletState,
    inspect,
    previewIntent,
    prepareWalletIntent,
    signPreparedTransaction,
    pollDirectJournal,
    submitDirectPacket,
  });
}
