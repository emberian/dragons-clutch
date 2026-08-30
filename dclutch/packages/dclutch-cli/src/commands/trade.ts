/**
 * The Direct inline trade from a terminal.
 *
 * Direct settlement is bilateral: two signed compact intents (a maker on
 * each side) crossed at one execution price. The public CLI may authenticate
 * a route and sign an off-chain intent, but it does not yet own the complete
 * exterior evidence needed to mutate the chain. So the honest CLI shape is:
 *
 *   dclutch intent sell ... --out sell-intent.json    # off-chain handoff
 *   dclutch spine --market ...                       # read-only inspection
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
import type { Keypair } from '@solana/web3.js';
import nacl from 'tweetnacl';

import { loadKeypair, rpcClient, type CliContext } from '../context';
import { type Io } from '../output';

async function inspectRoute(context: CliContext, client = rpcClient(context)): Promise<DirectHotRouteInspectionV3> {
  const routePath = context.flags.route;
  if (typeof routePath !== 'string') throw new Error('pass --route <json> naming one checked Direct hot route manifest');
  return inspectDirectHotRouteManifestJsonV3(client, readFileSync(routePath));
}

// ------------------------------------------------------------------- intents

type IntentFileV1 = Readonly<{
  schema: 'dclutch-direct-intent-v1';
  maker: string;
  signature: string;
  intent: Readonly<Record<string, string | number>>;
}>;

function intentToJson(signed: SignedDirectIntentV3): IntentFileV1 {
  const intent = signed.intent;
  return Object.freeze({
    schema: 'dclutch-direct-intent-v1' as const,
    maker: signed.maker,
    signature: Buffer.from(signed.signature).toString('hex'),
    intent: Object.freeze({
      side: intent.side,
      lifecycle: intent.lifecycle,
      outcome: intent.outcome,
      market: intent.market,
      generation: intent.generation.toString(),
      nonce: intent.nonce.toString(),
      validFrom: intent.validFrom.toString(),
      validThrough: intent.validThrough.toString(),
      maximumFill: intent.maximumFill.toString(),
      limitPrice: intent.limitPrice.toString(),
      feeBasisPoints: intent.feeBasisPoints,
      collateralAccount: intent.collateralAccount,
    }),
  });
}

function flagBig(context: CliContext, name: string, fallback?: bigint): bigint {
  const value = context.flags[name];
  if (typeof value === 'string' && /^\d+$/.test(value)) return BigInt(value);
  if (fallback !== undefined) return fallback;
  throw new Error(`pass --${name} <unsigned integer>`);
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
  const observed = BigInt(inspection.observedSlot);
  return Object.freeze({
    side,
    lifecycle: context.flags.lifecycle === '1' ? 1 : 0,
    outcome: Number(flagText(context, 'outcome')),
    market: inspection.route.market,
    generation: inspection.route.generation,
    nonce: overrides.nonce ?? flagBig(context, 'nonce', 1n),
    validFrom: flagBig(context, 'valid-from', observed),
    validThrough: flagBig(context, 'valid-through', observed + 150n),
    maximumFill: overrides.maximumFill ?? flagBig(context, 'fill'),
    limitPrice: overrides.limitPrice ?? flagBig(context, 'price'),
    feeBasisPoints: inspection.route.feeBasisPoints,
    collateralAccount: overrides.collateral ?? flagText(context, 'collateral'),
  });
}

function signIntent(intent: CompactIntentV2Input, maker: Keypair): SignedDirectIntentV3 {
  const message = encodeCompactIntentSigningMessageV2(intent);
  const signature = nacl.sign.detached(message, maker.secretKey);
  return Object.freeze({ maker: maker.publicKey.toBase58(), signature, intent });
}

// ------------------------------------------------------------------ commands

export async function intentCommand(context: CliContext, io: Io, sideText: string | undefined, env: NodeJS.ProcessEnv): Promise<number> {
  if (sideText !== 'sell' && sideText !== 'buy') throw new Error('usage: dclutch intent sell|buy --route <json> --outcome N --fill N --price N --collateral <address> --keypair <maker> --out <file>');
  const outPath = flagText(context, 'out');
  const inspection = await inspectRoute(context);
  const intent = buildIntent(context, inspection, sideText === 'sell' ? 0 : 1);
  // Authentication and complete intent construction precede key access. A
  // hostile route or malformed intent must never make the signer file relevant.
  const maker = loadKeypair(context, env);
  const signed = signIntent(intent, maker);
  writeFileSync(outPath, `${JSON.stringify(intentToJson(signed), null, 2)}\n`);
  io.out(`${sideText} intent signed by ${signed.maker} for market ${intent.market} — written to ${outPath}`);
  io.out(`  outcome ${intent.outcome}, max fill ${intent.maximumFill}, limit price ${intent.limitPrice}, valid slots ${intent.validFrom}..${intent.validThrough}`);
  return 0;
}

export const DIRECT_TRADE_MUTATION_REFUSAL_V1 =
  'public Direct mutation is disabled before all context, route, key, signature, and RPC access: ' +
  'the CLI does not yet wire one durable exact-packet journal, an authenticated HotExecutionAckV3, ' +
  'and a finalizer for all ten writable poststates; use `dclutch spine` for read-only market ' +
  'inspection or `dclutch intent buy|sell` for an off-chain signed handoff — neither submits';

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
