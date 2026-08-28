/**
 * The Direct inline trade from a terminal.
 *
 * Direct settlement is bilateral: two signed compact intents (a maker on
 * each side) crossed at one execution price, compiled into the exact
 * chain-derived v0 transaction and paid for by the route's fixed payer. So
 * the honest CLI shape is intent handoff:
 *
 *   dclutch intent --side sell ... --out sell-intent.json     # maker signs
 *   dclutch buy --take sell-intent.json ...                   # taker crosses
 *
 * `buy` takes a sell intent, `sell` takes a buy intent; either can also
 * self-match with `--counter-keypair` (both sides signed locally), which is
 * how a local bench exercises the loop. Every economic quantity is previewed
 * exactly — one fill×price÷scale boundary — before anything is signed.
 */
import { readFileSync, writeFileSync } from 'node:fs';

import {
  compileDirectInlineTransactionV3,
  encodeCompactIntentSigningMessageV2,
  previewDirectInlineV3,
  type CompactIntentV2Input,
  type SignedDirectIntentV3,
} from '@dclutch/sdk/directInlineV3';
import { type DirectHotRouteInspectionV3 } from '@dclutch/sdk/directHotChain';
import { inspectDirectHotRouteManifestJsonV3 } from '@dclutch/sdk/directHotRouteManifest';
import type { Keypair } from '@solana/web3.js';
import type { SolanaRpcClient } from '@dclutch/sdk/rpc';
import nacl from 'tweetnacl';

import { loadKeypair, rpcClient, type CliContext } from '../context';
import { assertExactDevnetMutation, devnetGenesisAcknowledgment } from '../mutation';
import { block, type Io } from '../output';
import { submitAndConfirm } from '../submit';

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

export function intentFromJson(value: unknown): SignedDirectIntentV3 {
  if (typeof value !== 'object' || value === null) throw new Error('intent file must be one JSON object');
  const input = value as Record<string, unknown>;
  if (input.schema !== 'dclutch-direct-intent-v1' || typeof input.maker !== 'string' || typeof input.signature !== 'string'
      || typeof input.intent !== 'object' || input.intent === null) {
    throw new Error('intent file is not a dclutch-direct-intent-v1 document');
  }
  const raw = input.intent as Record<string, unknown>;
  const num = (field: string): number => {
    const value = raw[field];
    if (typeof value !== 'number' || !Number.isInteger(value)) throw new Error(`intent ${field} must be an integer`);
    return value;
  };
  const big = (field: string): bigint => {
    const value = raw[field];
    if (typeof value !== 'string' || !/^\d+$/.test(value)) throw new Error(`intent ${field} must be an unsigned decimal string`);
    return BigInt(value);
  };
  const text = (field: string): string => {
    const value = raw[field];
    if (typeof value !== 'string') throw new Error(`intent ${field} must be text`);
    return value;
  };
  const side = num('side');
  const lifecycle = num('lifecycle');
  if (side !== 0 && side !== 1) throw new Error('intent side must be 0 (sell) or 1 (buy)');
  if (lifecycle !== 0 && lifecycle !== 1) throw new Error('intent lifecycle must be 0 or 1');
  if (!/^[0-9a-fA-F]{128}$/.test(input.signature)) throw new Error('intent signature must be 64 hex bytes');
  return Object.freeze({
    maker: input.maker,
    signature: Uint8Array.from(Buffer.from(input.signature, 'hex')),
    intent: Object.freeze({
      side, lifecycle,
      outcome: num('outcome'),
      market: text('market'),
      generation: big('generation'),
      nonce: big('nonce'),
      validFrom: big('validFrom'),
      validThrough: big('validThrough'),
      maximumFill: big('maximumFill'),
      limitPrice: big('limitPrice'),
      feeBasisPoints: num('feeBasisPoints'),
      collateralAccount: text('collateralAccount'),
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
  const maker = loadKeypair(context, env);
  const inspection = await inspectRoute(context);
  const intent = buildIntent(context, inspection, sideText === 'sell' ? 0 : 1);
  const signed = signIntent(intent, maker);
  writeFileSync(outPath, `${JSON.stringify(intentToJson(signed), null, 2)}\n`);
  io.out(`${sideText} intent signed by ${signed.maker} for market ${intent.market} — written to ${outPath}`);
  io.out(`  outcome ${intent.outcome}, max fill ${intent.maximumFill}, limit price ${intent.limitPrice}, valid slots ${intent.validFrom}..${intent.validThrough}`);
  return 0;
}

export async function tradeCommand(context: CliContext, io: Io, action: 'buy' | 'sell', env: NodeJS.ProcessEnv): Promise<number> {
  const acknowledgment = devnetGenesisAcknowledgment(context);
  const client: SolanaRpcClient = rpcClient(context);
  await assertExactDevnetMutation(client, acknowledgment, `${action} route preparation`);
  const inspection = await inspectRoute(context, client);
  if (inspection.route.outerEvidence.status !== 'checked') {
    io.err(`route is internally consistent but unrecognized: ${inspection.route.outerEvidence.status === 'unavailable' ? inspection.route.outerEvidence.reason : ''}`);
  }
  const taker = loadKeypair(context, env);
  const fill = flagBig(context, 'fill');
  const executionPrice = flagBig(context, 'price');

  // The counterparty: a handed-off signed intent, or a locally signed one
  // when the caller explicitly provides the counterparty's keypair.
  const mySide: 0 | 1 = action === 'buy' ? 1 : 0;
  let counterparty: SignedDirectIntentV3;
  const takePath = context.flags.take;
  if (typeof takePath === 'string') {
    counterparty = intentFromJson(JSON.parse(readFileSync(takePath, 'utf8')));
    if (counterparty.intent.side !== (1 - mySide)) throw new Error(`--take intent is side ${counterparty.intent.side}; a ${action} needs the opposite side`);
  } else if (typeof context.flags['counter-keypair'] === 'string') {
    const counterKeypair = loadKeypair(context, env, 'counter-keypair');
    const counterIntent = buildIntent(context, inspection, (1 - mySide) as 0 | 1, {
      collateral: flagText(context, 'counter-collateral'),
      nonce: flagBig(context, 'counter-nonce', 1n),
    });
    await assertExactDevnetMutation(client, acknowledgment, `${action} counterparty intent signature`);
    counterparty = signIntent(counterIntent, counterKeypair);
  } else {
    throw new Error(`a ${action} crosses a signed counterparty intent: pass --take <intent.json>, or --counter-keypair + --counter-collateral to self-match locally`);
  }

  const myIntent = buildIntent(context, inspection, mySide);
  await assertExactDevnetMutation(client, acknowledgment, `${action} taker intent signature`);
  const mine = signIntent(myIntent, taker);
  const seller = mySide === 0 ? mine : counterparty;
  const buyer = mySide === 1 ? mine : counterparty;
  const clockSlot = BigInt(inspection.observedSlot);

  const preview = previewDirectInlineV3(inspection.route, seller, buyer, fill, executionPrice, clockSlot);
  block(io, [
    ['gross collateral', preview.grossCollateral.toString()],
    ['seller credit', preview.sellerNetCollateralCredit.toString()],
    ['buyer debit', preview.buyerCollateralDebit.toString()],
    ['total fees', preview.totalFeeTransfer.toString()],
  ]);

  const plan = compileDirectInlineTransactionV3({ route: inspection.route, seller, buyer, fill, executionPrice, clockSlot });
  io.out(`compiled ${plan.wireBytes.length}/1232-byte v0 transaction, ${plan.loadedAddresses} lookup addresses, signers: ${plan.requiredSigners.join(', ')}`);
  if (context.flags['dry-run'] === true) {
    io.out('dry run — nothing signed or submitted');
    return 0;
  }

  const payer = typeof context.flags.payer === 'string' ? loadKeypair(context, env, 'payer') : taker;
  if (payer.publicKey.toBase58() !== inspection.route.payer) {
    throw new Error(`the route's fixed payer is ${inspection.route.payer}; the ${typeof context.flags.payer === 'string' ? '--payer' : '--keypair'} key is ${payer.publicKey.toBase58()} — pass --payer with the payer keypair`);
  }
  await assertExactDevnetMutation(client, acknowledgment, `${action} transaction signature`);
  plan.transaction.sign([payer]);
  const outcome = await submitAndConfirm(client, plan.transaction.serialize(), io, acknowledgment);
  return outcome.succeeded ? 0 : 1;
}
