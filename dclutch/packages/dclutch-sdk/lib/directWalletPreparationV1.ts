import { PublicKey } from '@solana/web3.js';

import { type DirectHotRouteInspectionV3 } from './directHotChain';
import { mineDirectInlineHotBumpHintsV3 } from './directHotBumpHintsV1';
import {
  compileDirectInlineTransactionV3,
  previewDirectInlineV3,
  type CompactIntentV2Input,
  type DirectInlineEconomicPreviewV3,
  type DirectInlineTransactionPlanV3,
  type SignedDirectIntentV3,
} from './directInlineV3';
import {
  requireAuthenticatedDirectMakerNoncePairV1,
  requireAuthenticatedDirectMakerNonceV1,
  type AuthenticatedDirectMakerNoncePairV1,
  type AuthenticatedDirectMakerNonceV1,
} from './directMakerReplay';
import {
  admitDirectParticipantCrossingV1,
  deriveDirectSellerTokenAddressV1,
  type DirectParticipantCoordinatesV1,
  type DirectParticipantReadinessV1,
  type DirectSellerCollateralPrestateV1,
  type DirectSellerCoordinatesV1,
  type DirectSellerReadinessV1,
} from './directParticipant';
import { type DirectCrossingPlanV1 } from './directTicket';

const U64_MAX = 0xffff_ffff_ffff_ffffn;
const ED25519_SIGNATURE_BYTES = 64;

/** The hostile-decoded output of `decodeDirectIntentTicketV1`; no second ticket DTO exists. */
export type DirectTicketInspectionV1 = SignedDirectIntentV3;

export type DirectWalletChainContextV1 = Readonly<{
  rpcEndpoint: string;
  genesisHash: string;
}>;

export type DirectWalletPreparationContextV1 = Readonly<{
  route: DirectWalletChainContextV1;
  sellerParticipant: DirectWalletChainContextV1;
  takerParticipant: DirectWalletChainContextV1;
  noncePair: DirectWalletChainContextV1;
  planning: DirectWalletChainContextV1 & Readonly<{ connectedWallet: string }>;
  current: DirectWalletChainContextV1 & Readonly<{
    connectedWallet: string;
    finalizedSlot: bigint;
    blockHeight: bigint;
  }>;
}>;

export type DirectWalletParticipantBindingV1 = Readonly<{
  owner: string;
  participantObservedSlot: string;
  coordinates: DirectParticipantCoordinatesV1;
  positionRevision: bigint;
  nonceAddress: string;
  nonceObservedSlot: string;
  nonce: bigint;
}>;

/**
 * The seller half of one binding.
 *
 * It carries no admission coordinate because the seller route has none, and it
 * reports which prestate the seller's Direct token account was observed in, so
 * a caller can say out loud that the route's permissionless Trading token setup
 * still has to land before this trade can execute.
 */
export type DirectWalletSellerBindingV1 = Readonly<{
  owner: string;
  participantObservedSlot: string;
  coordinates: DirectSellerCoordinatesV1;
  positionRevision: bigint;
  collateralPrestate: DirectSellerCollateralPrestateV1;
  nonceAddress: string;
  nonceObservedSlot: string;
  nonce: bigint;
}>;

export type DirectWalletExecutionBindingV1 = Readonly<{
  rpcEndpoint: string;
  genesisHash: string;
  connectedWallet: string;
  market: string;
  generation: bigint;
  outcome: number;
  outcomeCount: number;
  priceScale: bigint;
  feeBasisPoints: number;
  fill: bigint;
  executionPrice: bigint;
  routeObservedSlot: string;
  blockhash: string;
  blockhashObservedSlot: bigint;
  lastValidBlockHeight: bigint;
  currentFinalizedSlot: bigint;
  currentBlockHeight: bigint;
  seller: DirectWalletSellerBindingV1;
  taker: DirectWalletParticipantBindingV1;
}>;

export type DirectWalletPreparationV1 =
  | Readonly<{
    status: 'wallet-preparable';
    payerBranch: 'wallet-pays';
    payer: string;
    binding: DirectWalletExecutionBindingV1;
    transactionPlan: DirectInlineTransactionPlanV3;
  }>
  | Readonly<{
    status: 'operator-required';
    payerBranch: 'operator-required';
    payer: string;
    binding: DirectWalletExecutionBindingV1;
    signedIntents: Readonly<{ seller: SignedDirectIntentV3; buyer: SignedDirectIntentV3 }>;
    reason: string;
  }>;

export type DirectWalletPreparationInputV1 = Readonly<{
  routeInspection: DirectHotRouteInspectionV3;
  ticketInspection: DirectTicketInspectionV1;
  crossingPlan: DirectCrossingPlanV1;
  sellerParticipant: DirectSellerReadinessV1;
  takerParticipant: DirectParticipantReadinessV1;
  noncePair: AuthenticatedDirectMakerNoncePairV1;
  signedSeller: SignedDirectIntentV3;
  signedTaker: SignedDirectIntentV3;
  context: DirectWalletPreparationContextV1;
}>;

function canonicalKey(value: string, field: string): string {
  let parsed: PublicKey;
  try {
    parsed = new PublicKey(value);
  } catch {
    throw new Error(`${field} is not one canonical Solana address`);
  }
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return value;
}

function exactU64(value: bigint, field: string): bigint {
  if (value < 0n || value > U64_MAX) throw new Error(`${field} is outside u64`);
  return value;
}

function exactSlotText(value: string, field: string): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} is not canonical unsigned decimal text`);
  return exactU64(BigInt(value), field);
}

function exactChainContext(context: DirectWalletChainContextV1, field: string): DirectWalletChainContextV1 {
  if (context.rpcEndpoint.length === 0 || context.rpcEndpoint.length > 2_048) {
    throw new Error(`${field} RPC endpoint is empty or above its explicit 2048-character bound`);
  }
  let endpoint: URL;
  try {
    endpoint = new URL(context.rpcEndpoint);
  } catch {
    throw new Error(`${field} RPC endpoint is not an absolute URL`);
  }
  if ((endpoint.protocol !== 'https:' && endpoint.protocol !== 'http:')
      || endpoint.username.length !== 0 || endpoint.password.length !== 0 || endpoint.hash.length !== 0) {
    throw new Error(`${field} RPC endpoint must be an HTTP(S) URL without credentials or a fragment`);
  }
  canonicalKey(context.genesisHash, `${field} genesis hash`);
  return context;
}

function sameBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function sameIntent(left: CompactIntentV2Input, right: CompactIntentV2Input): boolean {
  return left.side === right.side
    && left.lifecycle === right.lifecycle
    && left.outcome === right.outcome
    && left.market === right.market
    && left.generation === right.generation
    && left.nonce === right.nonce
    && left.validFrom === right.validFrom
    && left.validThrough === right.validThrough
    && left.maximumFill === right.maximumFill
    && left.limitPrice === right.limitPrice
    && left.feeBasisPoints === right.feeBasisPoints
    && left.collateralAccount === right.collateralAccount;
}

function sameSignedIntent(left: SignedDirectIntentV3, right: SignedDirectIntentV3): boolean {
  return left.maker === right.maker && sameBytes(left.signature, right.signature) && sameIntent(left.intent, right.intent);
}

function requireSignedIntent(value: SignedDirectIntentV3, field: string): void {
  canonicalKey(value.maker, `${field} maker`);
  canonicalKey(value.intent.market, `${field} Market`);
  canonicalKey(value.intent.collateralAccount, `${field} collateral account`);
  if (value.signature.length !== ED25519_SIGNATURE_BYTES || value.signature.every((byte) => byte === 0)) {
    throw new Error(`${field} requires one nonzero 64-byte detached signature`);
  }
}

function samePreview(left: DirectInlineEconomicPreviewV3, right: DirectInlineEconomicPreviewV3): boolean {
  return left.fill === right.fill
    && left.executionPrice === right.executionPrice
    && left.grossCollateral === right.grossCollateral
    && left.sellerFee === right.sellerFee
    && left.buyerFee === right.buyerFee
    && left.sellerNetCollateralCredit === right.sellerNetCollateralCredit
    && left.buyerCollateralDebit === right.buyerCollateralDebit
    && left.totalFeeTransfer === right.totalFeeTransfer;
}

function requireReadyParticipant(
  participant: DirectParticipantReadinessV1,
  field: string,
): Extract<DirectParticipantReadinessV1, { status: 'ready' }> {
  if (participant.status !== 'ready') throw new Error(`${field} participant state is ${participant.status}, not ready`);
  return participant;
}

function requireReadySeller(
  seller: DirectSellerReadinessV1,
): Extract<DirectSellerReadinessV1, { status: 'ready' }> {
  if (seller.status !== 'ready') throw new Error(`seller Direct state is ${seller.status}, not ready`);
  return seller;
}

function participantBinding(
  participant: Extract<DirectParticipantReadinessV1, { status: 'ready' }>,
  nonce: AuthenticatedDirectMakerNonceV1,
): DirectWalletParticipantBindingV1 {
  return Object.freeze({
    owner: participant.owner,
    participantObservedSlot: participant.observedSlot,
    coordinates: Object.freeze({ ...participant.coordinates }),
    positionRevision: participant.positionRevision,
    nonceAddress: nonce.address,
    nonceObservedSlot: nonce.observedSlot,
    nonce: nonce.nextNonce,
  });
}

function sellerBinding(
  seller: Extract<DirectSellerReadinessV1, { status: 'ready' }>,
  nonce: AuthenticatedDirectMakerNonceV1,
): DirectWalletSellerBindingV1 {
  return Object.freeze({
    owner: seller.owner,
    participantObservedSlot: seller.observedSlot,
    coordinates: Object.freeze({ ...seller.coordinates }),
    positionRevision: seller.positionRevision,
    collateralPrestate: seller.collateralPrestate,
    nonceAddress: nonce.address,
    nonceObservedSlot: nonce.observedSlot,
    nonce: nonce.nextNonce,
  });
}

const DIRECT_RUNTIME_SELLER_REPLAY_V1 = 0;
const DIRECT_RUNTIME_BUYER_REPLAY_V1 = 3;
const DIRECT_RUNTIME_CLAIMS_AGGREGATE_V1 = 7;
const DIRECT_RUNTIME_SELLER_POSITION_V1 = 23;
const DIRECT_RUNTIME_BUYER_POSITION_V1 = 24;
const DIRECT_RUNTIME_BUYER_COLLATERAL_V1 = 30;
const DIRECT_RUNTIME_SELLER_COLLATERAL_V1 = 31;

function requireRuntimeJoin(route: DirectHotRouteInspectionV3['route'], index: number, expected: string, field: string): void {
  const account = route.runtimeAccounts[index];
  if (account === undefined || account.address !== expected) {
    throw new Error(`authenticated Direct runtime substitutes the ${field} coordinate`);
  }
}

/**
 * Bind already-authenticated Direct observations to the wallet that is still
 * connected, then compile only when that wallet is the route's exact payer.
 * This function is pure caller-owned preparation: it never signs or submits.
 */
export function prepareDirectWalletTransactionV1(input: DirectWalletPreparationInputV1): DirectWalletPreparationV1 {
  const { routeInspection, crossingPlan, context } = input;
  const route = routeInspection.route;
  if (routeInspection.checkedOuter.status !== 'checked' || route.outerEvidence.status !== 'checked') {
    throw new Error('Direct wallet preparation requires an authenticated checked hot route');
  }
  if (routeInspection.checkedOuter.tradingArtifactRelease !== route.outerEvidence.tradingArtifactRelease
      || routeInspection.checkedOuter.checkedManifestDigest !== route.outerEvidence.checkedManifestDigest) {
    throw new Error('route inspection and executable route disagree on checked outer evidence');
  }

  const routeSlot = exactSlotText(routeInspection.observedSlot, 'route observation slot');
  if (routeSlot !== exactU64(route.observedSlot, 'route embedded observation slot')) {
    throw new Error('route inspection slot differs from its executable route slot');
  }
  canonicalKey(route.payer, 'route payer');
  canonicalKey(route.tradingProgram, 'Trading program');
  canonicalKey(route.market, 'route Market');
  canonicalKey(route.recentBlockhash, 'route recent blockhash');
  exactU64(route.blockhashObservedSlot, 'route blockhash observation slot');
  exactU64(route.lastValidBlockHeight, 'route last valid block height');
  if (route.blockhashObservedSlot < route.observedSlot) {
    throw new Error('route blockhash observation predates the authenticated route observation');
  }

  const currentContext = exactChainContext(context.current, 'current');
  const chainContexts: ReadonlyArray<readonly [string, DirectWalletChainContextV1]> = [
    ['route', context.route],
    ['seller participant', context.sellerParticipant],
    ['taker participant', context.takerParticipant],
    ['maker nonce pair', context.noncePair],
    ['planning', context.planning],
  ];
  for (const [field, candidate] of chainContexts) {
    exactChainContext(candidate, field);
    if (candidate.rpcEndpoint !== currentContext.rpcEndpoint) {
      throw new Error(`${field} observation came from another RPC endpoint`);
    }
    if (candidate.genesisHash !== currentContext.genesisHash) {
      throw new Error(`${field} observation came from another genesis hash`);
    }
  }
  const plannedWallet = canonicalKey(context.planning.connectedWallet, 'planning connected wallet');
  const connectedWallet = canonicalKey(context.current.connectedWallet, 'current connected wallet');
  if (plannedWallet !== connectedWallet) throw new Error('connected wallet changed after Direct crossing planning');
  const currentSlot = exactU64(context.current.finalizedSlot, 'current finalized slot');
  const currentBlockHeight = exactU64(context.current.blockHeight, 'current block height');
  if (currentSlot < route.blockhashObservedSlot) {
    throw new Error('current finalized slot predates the route blockhash observation');
  }
  if (currentBlockHeight > route.lastValidBlockHeight) {
    throw new Error(`route blockhash expired at block height ${route.lastValidBlockHeight}`);
  }

  requireSignedIntent(input.ticketInspection, 'ticket inspection');
  requireSignedIntent(input.signedSeller, 'signed seller intent');
  requireSignedIntent(input.signedTaker, 'signed taker intent');
  if (!sameSignedIntent(input.ticketInspection, crossingPlan.ticket)
      || !sameSignedIntent(input.ticketInspection, input.signedSeller)) {
    throw new Error('crossing plan or signed seller intent substitutes another portable ticket');
  }
  if (!sameIntent(crossingPlan.taker, input.signedTaker.intent)) {
    throw new Error('signed taker intent differs from the planned taker intent');
  }
  if (crossingPlan.takerAddress !== input.signedTaker.maker) {
    throw new Error('signed taker maker differs from the crossing-plan taker');
  }
  if (crossingPlan.takerSide !== 'buy' || input.signedSeller.intent.side !== 0 || input.signedTaker.intent.side !== 1) {
    throw new Error('wallet preparation V1 requires a portable seller ticket and the connected wallet as buyer');
  }
  if (connectedWallet !== crossingPlan.takerAddress || connectedWallet !== input.signedTaker.maker) {
    throw new Error('current connected wallet is not the exact taker owner');
  }
  if (input.signedSeller.maker === connectedWallet) throw new Error('seller and connected taker wallet must be distinct');

  const sellerIntent = input.signedSeller.intent;
  const buyerIntent = input.signedTaker.intent;
  if (sellerIntent.market !== route.market || buyerIntent.market !== route.market
      || sellerIntent.generation !== route.generation || buyerIntent.generation !== route.generation
      || sellerIntent.outcome !== buyerIntent.outcome || sellerIntent.outcome >= route.outcomeCount
      || crossingPlan.executionPrice !== sellerIntent.limitPrice
      || crossingPlan.executionPrice !== buyerIntent.limitPrice
      || crossingPlan.fill > sellerIntent.maximumFill
      || (sellerIntent.lifecycle === 0 && crossingPlan.fill !== sellerIntent.maximumFill)
      || crossingPlan.fill !== buyerIntent.maximumFill
      || sellerIntent.feeBasisPoints !== route.feeBasisPoints || buyerIntent.feeBasisPoints !== route.feeBasisPoints) {
    throw new Error('route, ticket, and taker intent disagree on exact Market, generation, outcome, price, fill, or fee');
  }

  // The two halves of a Direct crossing do not have the same shape on chain, so
  // they are not checked here as if they did.
  //
  // A BUYER spends collateral through Custody: it needs a Claims admission
  // record and an admission-created Token-2022 account delegated to this
  // Market's Custody authority, and `admitDirectParticipantCrossingV1` still
  // holds its spendable allowance against the exact planned debit.
  //
  // A SELLER spends CLAIMS. Its collateral is a DESTINATION, and the address is
  // one Trading owns: `direct_token_setup_v1::authenticate_semantics` derives
  // `find_program_address(DirectTokenAccountSeedsV1::new(market, generation,
  // position.owner, Seller), trading)` and CREATES it, permissionlessly, off one
  // precondition -- `authenticate_seller_position`, the canonical Claims
  // aggregate and the seller's Position under it. That route's frame has
  // twenty-three account indices and an admission record is not one of them, and
  // the account it creates via `initialize_account3` has no delegate at all, so
  // demanding a Custody-delegated participant account of the seller refused the
  // very account the chain builds. See `inspectDirectSellerReadinessV1`.
  const sellerParticipant = requireReadySeller(input.sellerParticipant);
  const takerParticipant = requireReadyParticipant(input.takerParticipant, 'taker');
  for (const [field, participant, signed] of [
    ['seller', sellerParticipant, input.signedSeller],
    ['taker', takerParticipant, input.signedTaker],
  ] as const) {
    canonicalKey(participant.owner, `${field} participant owner`);
    for (const [name, address] of Object.entries(participant.coordinates)) canonicalKey(address, `${field} ${name}`);
    const participantSlot = exactSlotText(participant.observedSlot, `${field} participant observation slot`);
    if (participantSlot < routeSlot || participantSlot > currentSlot) {
      throw new Error(`${field} participant observation is outside the route-to-current finalized slot interval`);
    }
    if (participant.market !== route.market || participant.generation !== route.generation
        || participant.owner !== signed.maker || participant.coordinates.collateral !== signed.intent.collateralAccount
        || participant.positionBalances.length !== route.outcomeCount) {
      throw new Error(`${field} participant substitutes another owner, Market, generation, collateral, or outcome width`);
    }
  }
  // Re-derive the seller's collateral here rather than believing the readiness
  // that reported it. The readiness is one authority; this route, the ticket the
  // seller signed, and Trading's own seeds are three more, and they have to name
  // one address. This is the join whose absence let a ticket authored with the
  // BUYER's `create_with_seed` derivation reach the producer and refuse there.
  if (sellerParticipant.coordinates.collateral !== deriveDirectSellerTokenAddressV1(
    route.tradingProgram, route.market, route.generation, input.signedSeller.maker,
  )) {
    throw new Error('seller collateral is not the Direct token account Trading derives for this Market, generation, and seller');
  }
  if (sellerParticipant.coordinates.aggregate !== takerParticipant.coordinates.aggregate
      || sellerParticipant.coordinates.custodyAuthority !== takerParticipant.coordinates.custodyAuthority
      || sellerParticipant.collateralMint !== takerParticipant.collateralMint
      || sellerParticipant.tokenProgram !== takerParticipant.tokenProgram) {
    throw new Error('seller and taker participant state disagree on aggregate, custody authority, Mint, or token program');
  }
  if (new Set([
    sellerParticipant.owner, takerParticipant.owner,
    sellerParticipant.coordinates.position, takerParticipant.coordinates.position,
    takerParticipant.coordinates.admission,
    sellerParticipant.coordinates.collateral, takerParticipant.coordinates.collateral,
  ]).size !== 7) {
    throw new Error('seller and taker participant authorities or owned accounts alias');
  }
  const sellerClaims = sellerParticipant.positionBalances[sellerIntent.outcome];
  if (sellerClaims === undefined || sellerClaims < crossingPlan.fill) {
    throw new Error('seller finalized Position does not cover the exact planned claim fill');
  }
  admitDirectParticipantCrossingV1(takerParticipant, crossingPlan);

  const noncePair = requireAuthenticatedDirectMakerNoncePairV1(input.noncePair);
  const sellerNonceObservation = noncePair[0];
  const takerNonceObservation = noncePair[1];
  const sellerNonce = requireAuthenticatedDirectMakerNonceV1(sellerNonceObservation, {
    tradingProgram: route.tradingProgram,
    market: route.market,
    generation: route.generation,
    maker: input.signedSeller.maker,
  });
  const takerNonce = requireAuthenticatedDirectMakerNonceV1(takerNonceObservation, {
    tradingProgram: route.tradingProgram,
    market: route.market,
    generation: route.generation,
    maker: input.signedTaker.maker,
  });
  if (sellerNonce !== sellerIntent.nonce || takerNonce !== buyerIntent.nonce) {
    throw new Error('signed intent nonce is stale, future, or already consumed relative to finalized replay state');
  }
  for (const [field, observation] of [['seller', sellerNonceObservation], ['taker', takerNonceObservation]] as const) {
    canonicalKey(observation.address, `${field} replay address`);
    const nonceSlot = exactSlotText(observation.observedSlot, `${field} nonce observation slot`);
    if (nonceSlot < routeSlot || nonceSlot > currentSlot) {
      throw new Error(`${field} nonce observation is outside the route-to-current finalized slot interval`);
    }
  }
  requireRuntimeJoin(route, DIRECT_RUNTIME_SELLER_REPLAY_V1, sellerNonceObservation.address, 'seller replay');
  requireRuntimeJoin(route, DIRECT_RUNTIME_BUYER_REPLAY_V1, takerNonceObservation.address, 'buyer replay');
  requireRuntimeJoin(route, DIRECT_RUNTIME_CLAIMS_AGGREGATE_V1, sellerParticipant.coordinates.aggregate, 'Claims aggregate');
  requireRuntimeJoin(route, DIRECT_RUNTIME_SELLER_POSITION_V1, sellerParticipant.coordinates.position, 'seller Position');
  requireRuntimeJoin(route, DIRECT_RUNTIME_BUYER_POSITION_V1, takerParticipant.coordinates.position, 'buyer Position');
  requireRuntimeJoin(route, DIRECT_RUNTIME_BUYER_COLLATERAL_V1, takerParticipant.coordinates.collateral, 'buyer collateral');
  requireRuntimeJoin(route, DIRECT_RUNTIME_SELLER_COLLATERAL_V1, sellerParticipant.coordinates.collateral, 'seller collateral');

  const preview = previewDirectInlineV3(
    route, input.signedSeller, input.signedTaker,
    crossingPlan.fill, crossingPlan.executionPrice, currentSlot,
  );
  if (!samePreview(preview, crossingPlan.preview)) {
    throw new Error('crossing plan economic preview differs from the exact current route and signed intents');
  }

  const transactionSigners = new Set<string>([route.payer]);
  for (const account of [...route.fixedAccounts, ...route.strategyAccounts, ...route.runtimeAccounts]) {
    if (account.isSigner) transactionSigners.add(account.address);
  }
  if (transactionSigners.size !== 1) throw new Error('authenticated Direct route names an unexpected transaction co-signer');

  const binding: DirectWalletExecutionBindingV1 = Object.freeze({
    rpcEndpoint: currentContext.rpcEndpoint,
    genesisHash: currentContext.genesisHash,
    connectedWallet,
    market: route.market,
    generation: route.generation,
    outcome: sellerIntent.outcome,
    outcomeCount: route.outcomeCount,
    priceScale: route.priceScale,
    feeBasisPoints: route.feeBasisPoints,
    fill: crossingPlan.fill,
    executionPrice: crossingPlan.executionPrice,
    routeObservedSlot: routeInspection.observedSlot,
    blockhash: route.recentBlockhash,
    blockhashObservedSlot: route.blockhashObservedSlot,
    lastValidBlockHeight: route.lastValidBlockHeight,
    currentFinalizedSlot: currentSlot,
    currentBlockHeight,
    seller: sellerBinding(sellerParticipant, sellerNonceObservation),
    taker: participantBinding(takerParticipant, takerNonceObservation),
  });

  if (route.payer !== connectedWallet) {
    return Object.freeze({
      status: 'operator-required',
      payerBranch: 'operator-required',
      payer: route.payer,
      binding,
      signedIntents: Object.freeze({ seller: input.signedSeller, buyer: input.signedTaker }),
      reason: `The authenticated route requires ${route.payer} to pay and sign the transaction; the connected taker wallet supplied only its signed Direct intent.`,
    });
  }

  // Mine the wire's eight reserved bump bytes here, from the finalized bodies
  // this same inspection already read and authenticated. Off chain each search
  // is free; on chain each rejected candidate costs the PROGRAM 1,500 CU at a
  // depth drawn from whose key is trading, so an unmined browser trade pays a
  // per-trader tax that a mined one does not. Every hint is reproduced against
  // the account Trading was handed and a wrong one refuses, so nothing here can
  // steer the execution -- see `mineDirectInlineHotBumpHintsV3`.
  //
  // The two child caller-authority slots stay zero, and that is the SAME gap
  // the Rust builder has: their seeds end in a digest over each child's
  // projected request, which only the selected Transition and Effect
  // interpreters produce. `build_direct_inline_hot_v4` takes those two bytes as
  // a parameter for exactly that reason. A zero slot searches, so this wire is
  // correct and six-eighths cheaper rather than correct and whole.
  //
  // The size of what closing that would cost, so nobody has to guess: a
  // TypeScript port of `project_direct_inline_ordinary_child_requests_v3` needs
  // TransitionVM V3, Effect kernel V2/V3/V4, AccountProfileV2 geometry and the
  // ordinary register projection -- 10,643 non-test Rust lines across four
  // crates, measured, each of which would gain a second authority. The two
  // slots are worth roughly a quarter of the block; that is the trade, and it
  // is not obviously worth taking.
  const bumpHints = mineDirectInlineHotBumpHintsV3({
    source: routeInspection.bumpHintSource,
    tradingProgram: route.tradingProgram,
    market: route.market,
    generation: route.generation,
    releaseSet: route.releaseSet,
    sellerMaker: input.signedSeller.maker,
    buyerMaker: input.signedTaker.maker,
    expectedLifecycleAccounts: [sellerNonceObservation.address, takerNonceObservation.address],
  });
  const transactionPlan = compileDirectInlineTransactionV3({
    route,
    seller: input.signedSeller,
    buyer: input.signedTaker,
    fill: crossingPlan.fill,
    executionPrice: crossingPlan.executionPrice,
    clockSlot: currentSlot,
    bumpHints,
  });
  if (transactionPlan.minedBumpHintSlots === 0) {
    throw new Error('mined Direct wallet wire carries an absent bump-hint block');
  }
  if (transactionPlan.requiredSigners.length !== 1 || transactionPlan.requiredSigners[0] !== connectedWallet) {
    throw new Error('compiled Direct transaction does not name the connected wallet as its sole exact payer');
  }
  return Object.freeze({
    status: 'wallet-preparable',
    payerBranch: 'wallet-pays',
    payer: connectedWallet,
    binding,
    transactionPlan,
  });
}
