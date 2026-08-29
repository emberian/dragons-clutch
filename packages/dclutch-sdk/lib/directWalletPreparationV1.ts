import { PublicKey } from '@solana/web3.js';

import { type DirectHotRouteInspectionV3 } from './directHotChain';
import {
  compileDirectInlineTransactionV3,
  previewDirectInlineV3,
  type CompactIntentV2Input,
  type DirectInlineEconomicPreviewV3,
  type DirectInlineTransactionPlanV3,
  type SignedDirectIntentV3,
} from './directInlineV3';
import {
  requireAuthenticatedDirectMakerNonceV1,
  type AuthenticatedDirectMakerNonceV1,
} from './directMakerReplay';
import {
  admitDirectParticipantCrossingV1,
  type DirectParticipantCoordinatesV1,
  type DirectParticipantReadinessV1,
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
  sellerNonce: DirectWalletChainContextV1;
  takerNonce: DirectWalletChainContextV1;
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
  seller: DirectWalletParticipantBindingV1;
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
  sellerParticipant: DirectParticipantReadinessV1;
  takerParticipant: DirectParticipantReadinessV1;
  sellerNonce: AuthenticatedDirectMakerNonceV1;
  takerNonce: AuthenticatedDirectMakerNonceV1;
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
    ['seller nonce', context.sellerNonce],
    ['taker nonce', context.takerNonce],
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

  const sellerParticipant = requireReadyParticipant(input.sellerParticipant, 'seller');
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
  if (sellerParticipant.coordinates.aggregate !== takerParticipant.coordinates.aggregate
      || sellerParticipant.coordinates.custodyAuthority !== takerParticipant.coordinates.custodyAuthority
      || sellerParticipant.collateralMint !== takerParticipant.collateralMint
      || sellerParticipant.tokenProgram !== takerParticipant.tokenProgram) {
    throw new Error('seller and taker participant state disagree on aggregate, custody authority, Mint, or token program');
  }
  if (new Set([
    sellerParticipant.owner, takerParticipant.owner,
    sellerParticipant.coordinates.position, takerParticipant.coordinates.position,
    sellerParticipant.coordinates.admission, takerParticipant.coordinates.admission,
    sellerParticipant.coordinates.collateral, takerParticipant.coordinates.collateral,
  ]).size !== 8) {
    throw new Error('seller and taker participant authorities or owned accounts alias');
  }
  const sellerClaims = sellerParticipant.positionBalances[sellerIntent.outcome];
  if (sellerClaims === undefined || sellerClaims < crossingPlan.fill) {
    throw new Error('seller finalized Position does not cover the exact planned claim fill');
  }
  admitDirectParticipantCrossingV1(takerParticipant, crossingPlan);

  const sellerNonce = requireAuthenticatedDirectMakerNonceV1(input.sellerNonce, {
    tradingProgram: route.tradingProgram,
    market: route.market,
    generation: route.generation,
    maker: input.signedSeller.maker,
  });
  const takerNonce = requireAuthenticatedDirectMakerNonceV1(input.takerNonce, {
    tradingProgram: route.tradingProgram,
    market: route.market,
    generation: route.generation,
    maker: input.signedTaker.maker,
  });
  if (sellerNonce !== sellerIntent.nonce || takerNonce !== buyerIntent.nonce) {
    throw new Error('signed intent nonce is stale, future, or already consumed relative to finalized replay state');
  }
  if (input.sellerNonce.address === input.takerNonce.address) throw new Error('seller and taker replay coordinates alias');
  for (const [field, observation] of [['seller', input.sellerNonce], ['taker', input.takerNonce]] as const) {
    canonicalKey(observation.address, `${field} replay address`);
    const nonceSlot = exactSlotText(observation.observedSlot, `${field} nonce observation slot`);
    if (nonceSlot < routeSlot || nonceSlot > currentSlot) {
      throw new Error(`${field} nonce observation is outside the route-to-current finalized slot interval`);
    }
  }

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
    seller: participantBinding(sellerParticipant, input.sellerNonce),
    taker: participantBinding(takerParticipant, input.takerNonce),
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

  const transactionPlan = compileDirectInlineTransactionV3({
    route,
    seller: input.signedSeller,
    buyer: input.signedTaker,
    fill: crossingPlan.fill,
    executionPrice: crossingPlan.executionPrice,
    clockSlot: currentSlot,
  });
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
