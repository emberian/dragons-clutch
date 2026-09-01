/**
 * The Direct inline trade from a terminal.
 *
 * Direct settlement is bilateral: two signed compact intents (a maker on
 * each side) crossed at one execution price. The public CLI may authenticate
 * a route and sign an off-chain intent, but it does not yet own the complete
 * exterior evidence needed to mutate the chain. So the honest CLI shape is:
 *
 *   dclutch-terminal intent sell ... --out sell-intent.json    # off-chain handoff
 *   dclutch-terminal spine --market ...                       # read-only inspection
 *
 * `buy` and `sell` refuse before context, route, key, signature, or RPC access.
 * They stay closed until one accepted caller owns the durable exact-packet
 * journal, authenticated `HotExecutionAckV3`, and all ten writable poststates.
 */
import { readFileSync, writeFileSync } from 'node:fs';

import {
  encodeCompactIntentSigningMessageV2,
  type CompactIntentV2Input,
  type SignedDirectIntentV3,
} from '@dclutch/sdk/directInlineV3';
import { type DirectHotRouteInspectionV3 } from '@dclutch/sdk/directHotChain';
import { inspectDirectHotRouteManifestJsonV3 } from '@dclutch/sdk/directHotRouteManifest';
import {
  inspectDirectMakerNonceV1,
  type AuthenticatedDirectMakerNonceV1,
} from '@dclutch/sdk/directMakerReplay';
import {
  composeDirectSellOfferV1,
  sealDirectSellOfferV1,
  type ReadyDirectSellerV1,
} from '@dclutch/sdk/directOfferAuthoring';
import { inspectDirectSellerReadinessV1 } from '@dclutch/sdk/directParticipant';
import { encodeDirectIntentTicketV1 } from '@dclutch/sdk/directTicket';
import type { Keypair } from '@solana/web3.js';
import nacl from 'tweetnacl';

import { loadKeypair, programId, rpcClient, type CliContext } from '../context';
import { type Io } from '../output';

async function inspectRoute(context: CliContext, client = rpcClient(context)): Promise<DirectHotRouteInspectionV3> {
  const routePath = context.flags.route;
  if (typeof routePath !== 'string') throw new Error('pass --route <json> naming one checked Direct hot route manifest');
  return inspectDirectHotRouteManifestJsonV3(client, readFileSync(routePath));
}

// ------------------------------------------------------------------- intents

function flagBig(context: CliContext, name: string, fallback?: bigint): bigint {
  const value = context.flags[name];
  if (typeof value === 'string' && /^\d+$/.test(value)) return BigInt(value);
  if (fallback !== undefined) return fallback;
  throw new Error(`pass --${name} <unsigned integer>`);
}

function rawIntentLifecycleV1(context: CliContext): 0 | 1 {
  const value = context.flags.lifecycle;
  if (value === '0') return 0;
  if (value === '1') return 1;
  throw new Error('pass --lifecycle 0|1 for the low-level intent command');
}

export function parseOfferLifecycleV1(value: unknown): 0 | 1 {
  if (value === 'fok') return 0;
  if (value === 'ioc') return 1;
  throw new Error('pass --lifecycle fok|ioc (fok is all-or-nothing; ioc allows one smaller fill and leaves no resting remainder)');
}

function flagText(context: CliContext, name: string): string {
  const value = context.flags[name];
  if (typeof value !== 'string' || value.length === 0) throw new Error(`pass --${name}`);
  return value;
}

function buildIntent(
  context: CliContext,
  inspection: DirectHotRouteInspectionV3,
  side: 0 | 1,
  overrides: Readonly<{ maximumFill?: bigint; limitPrice?: bigint; collateral?: string; nonce?: bigint }> = {},
): CompactIntentV2Input {
  return Object.freeze({
    side,
    lifecycle: rawIntentLifecycleV1(context),
    outcome: Number(flagText(context, 'outcome')),
    market: inspection.route.market,
    generation: inspection.route.generation,
    nonce: overrides.nonce ?? flagBig(context, 'nonce'),
    validFrom: flagBig(context, 'valid-from'),
    validThrough: flagBig(context, 'valid-through'),
    maximumFill: overrides.maximumFill ?? flagBig(context, 'fill'),
    limitPrice: overrides.limitPrice ?? flagBig(context, 'price'),
    feeBasisPoints: inspection.route.feeBasisPoints,
    collateralAccount: overrides.collateral ?? flagText(context, 'collateral'),
  });
}

function signIntent(intent: CompactIntentV2Input, message: Uint8Array, maker: Keypair): SignedDirectIntentV3 {
  const signature = nacl.sign.detached(message, maker.secretKey);
  return Object.freeze({ maker: maker.publicKey.toBase58(), signature, intent });
}

// ------------------------------------------------------------------ commands

export async function intentCommand(context: CliContext, io: Io, sideText: string | undefined, env: NodeJS.ProcessEnv): Promise<number> {
  if (sideText !== 'sell' && sideText !== 'buy') throw new Error('usage: dclutch-terminal intent sell|buy --route <json> --outcome N --fill N --price N --collateral <address> --nonce N --valid-from SLOT --valid-through SLOT --lifecycle 0|1 --keypair <maker> --out <file>');
  const outPath = flagText(context, 'out');
  const inspection = await inspectRoute(context);
  const intent = buildIntent(context, inspection, sideText === 'sell' ? 0 : 1);
  // The canonical codec validates every width/address/interval before key
  // access. A malformed low-level intent never makes its signer file relevant.
  const message = encodeCompactIntentSigningMessageV2(intent);
  // Authentication and complete intent construction precede key access. A
  // hostile route or malformed intent must never make the signer file relevant.
  const maker = loadKeypair(context, env);
  const signed = signIntent(intent, message, maker);
  writeFileSync(outPath, `${encodeDirectIntentTicketV1(signed)}\n`);
  io.out(`${sideText} portable intent ticket signed by ${signed.maker} for market ${intent.market} — written to ${outPath}`);
  io.out(`  outcome ${intent.outcome}, max fill ${intent.maximumFill}, limit price ${intent.limitPrice}, valid slots ${intent.validFrom}..${intent.validThrough}`);
  return 0;
}

type DirectSellOfferRouteV1 = Parameters<typeof composeDirectSellOfferV1>[0]['route'];

export type DirectSellOfferObservationV1 = Readonly<{
  route: DirectSellOfferRouteV1;
  seller: ReadyDirectSellerV1;
  replay: AuthenticatedDirectMakerNonceV1;
}>;

/** Acquire all chain authority used by one offer before its signer is read. */
export async function observeDirectSellOfferV1(
  context: CliContext,
  maker: string,
): Promise<DirectSellOfferObservationV1> {
  const client = rpcClient(context);
  const inspection = await inspectRoute(context, client);
  const seller = await inspectDirectSellerReadinessV1(client, {
    market: inspection.route.market,
    owner: maker,
    coreProgram: programId(context, 'core'),
    registryProgram: programId(context, 'registry'),
    claimsProgram: programId(context, 'claims'),
    tradingProgram: programId(context, 'trading'),
    custodyProgram: programId(context, 'custody'),
    rentProgram: programId(context, 'rentCredit'),
  });
  if (seller.status !== 'ready') throw new Error(seller.reason);
  const replay = await inspectDirectMakerNonceV1(client, {
    tradingProgram: inspection.route.tradingProgram,
    market: inspection.route.market,
    generation: inspection.route.generation,
    maker,
  });
  return Object.freeze({
    route: Object.freeze({
      market: inspection.route.market,
      generation: inspection.route.generation,
      outcomeCount: inspection.route.outcomeCount,
      priceScale: inspection.route.priceScale,
      feeBasisPoints: inspection.route.feeBasisPoints,
      tradingProgram: inspection.route.tradingProgram,
    }),
    seller,
    replay,
  });
}

export type OfferCommandServicesV1 = Readonly<{
  observe: typeof observeDirectSellOfferV1;
  loadMaker: typeof loadKeypair;
  writeTicket: (path: string, text: string) => void;
}>;

const OFFER_COMMAND_SERVICES_V1: OfferCommandServicesV1 = Object.freeze({
  observe: observeDirectSellOfferV1,
  loadMaker: loadKeypair,
  writeTicket: (path, text) => writeFileSync(path, text),
});

/**
 * Author the participant-facing sell ticket without caller-invented nonce,
 * collateral destination, fee, generation, or start slot.
 *
 * Route authentication, seller readiness, replay acquisition and canonical
 * message construction all happen before the explicitly named keypair file is
 * opened. The command signs no transaction, submits nothing and needs no
 * relay; its sole output is the same portable ticket the web consumes.
 */
export async function offerCommand(
  context: CliContext,
  io: Io,
  sideText: string | undefined,
  env: NodeJS.ProcessEnv,
  services: OfferCommandServicesV1 = OFFER_COMMAND_SERVICES_V1,
): Promise<number> {
  if (sideText !== 'sell') {
    throw new Error('usage: dclutch-terminal offer sell --route <json> --maker <address> --outcome N --fill N --price N --duration-slots N --lifecycle fok|ioc --keypair <maker> --out <ticket.json>');
  }
  const outPath = flagText(context, 'out');
  const makerAddress = flagText(context, 'maker');
  const outcomeText = flagText(context, 'outcome');
  if (!/^(0|[1-9][0-9]*)$/.test(outcomeText)) throw new Error('pass --outcome <exact unsigned integer>');
  const outcome = Number(outcomeText);
  if (!Number.isSafeInteger(outcome)) throw new Error('--outcome exceeds the exact safe integer range');
  const maximumFill = flagBig(context, 'fill');
  const limitPrice = flagBig(context, 'price');
  const durationSlots = flagBig(context, 'duration-slots');
  const lifecycle = parseOfferLifecycleV1(context.flags.lifecycle);
  const observation = await services.observe(context, makerAddress);
  const draft = composeDirectSellOfferV1({
    route: observation.route,
    maker: makerAddress,
    seller: observation.seller,
    replay: observation.replay,
    outcome,
    maximumFill,
    limitPrice,
    lifecycle,
    durationSlots,
  });

  const maker = services.loadMaker(context, env);
  if (maker.publicKey.toBase58() !== makerAddress) {
    throw new Error('the explicitly named keypair is not the --maker whose Claims position and replay nonce were checked');
  }
  const signature = nacl.sign.detached(draft.signingMessage, maker.secretKey);
  const authored = sealDirectSellOfferV1(makerAddress, draft, signature);
  services.writeTicket(outPath, `${authored.text}\n`);
  io.out(`portable sell ticket authored for ${makerAddress} on market ${draft.intent.market} — written to ${outPath}`);
  io.out(`  outcome ${draft.intent.outcome}, max fill ${draft.intent.maximumFill}, limit price ${draft.intent.limitPrice}, nonce ${draft.intent.nonce}`);
  io.out(`  valid slots ${draft.intent.validFrom}..${draft.intent.validThrough}; no transaction signed or submitted; no relay required`);
  return 0;
}

export const DIRECT_TRADE_MUTATION_REFUSAL_V1 =
  'public Direct mutation is disabled before all context, route, key, signature, and RPC access: ' +
  'the CLI does not yet wire one durable exact-packet journal, an authenticated HotExecutionAckV3, ' +
  'and a finalizer for all ten writable poststates; use `dclutch-terminal spine` for read-only market ' +
  'inspection or `dclutch-terminal intent buy|sell` for an off-chain signed handoff — neither submits';

/**
 * Refuse the public mutation verbs at their dispatch boundary.
 *
 * This function deliberately accepts no context, environment, route, or I/O
 * object. Keeping those capabilities out of its type is the testable guarantee
 * that a `buy` or `sell` invocation cannot read a key or reach an RPC client.
 */
export async function tradeCommand(action: 'buy' | 'sell'): Promise<never> {
  throw new Error(`${action} refused: ${DIRECT_TRADE_MUTATION_REFUSAL_V1}`);
}
