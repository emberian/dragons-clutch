/**
 * `dclutch redeem --market <address> --payout-input <json>` — admit the
 * Claims-role replay, then ask the Rust authority for one exact wallet payout
 * manifest and independently bind its coordinates to the portfolio read.
 *
 * The successor subcommand is read-only: it reads the named cluster and emits
 * the seven-field manifest. This command does not submit the payout. Its JSON
 * output is the exact manifest accepted by the wallet payout SDK and web flow.
 */
import { spawnSync } from 'node:child_process';
import { isAbsolute, resolve } from 'node:path';

import { inspectClaimsCustodyReplayV1 } from '@dclutch/sdk/claimsCustodyReplay';
import { inspectPortfolioV1 } from '@dclutch/sdk/portfolio';
import {
  parseWalletTerminalPayoutManifestV3,
  type WalletTerminalPayoutManifestV3,
} from '@dclutch/sdk/walletTerminalPayoutV3';

import { loadKeypair, programId, rpcClient, type CliContext } from '../context';
import { block, type Io } from '../output';
import { submitAndConfirm } from '../submit';
import { successorBinary } from '../successor';

export type SuccessorSpawnResult = Readonly<{
  status: number | null;
  signal: string | null;
  stdout: string | null;
  stderr: string | null;
  error?: Error;
}>;

export type SuccessorSpawn = (
  binary: string,
  args: ReadonlyArray<string>,
  options: Readonly<{ encoding: 'utf8'; env: NodeJS.ProcessEnv }>,
) => SuccessorSpawnResult;

const spawnSuccessor: SuccessorSpawn = (binary, args, options) => spawnSync(binary, [...args], options);

function boundedStderr(value: string | null): string {
  const text = value?.trim() ?? '';
  return text.length > 4_096 ? `${text.slice(0, 4_096)}…` : text;
}

/** Invoke the Rust payout authority in its read-only manifest-producing mode. */
export function produceWalletTerminalPayoutManifestV3(
  context: CliContext,
  env: NodeJS.ProcessEnv,
  payoutInput: string,
  devnetAcknowledgment: string,
  spawn: SuccessorSpawn = spawnSuccessor,
): WalletTerminalPayoutManifestV3 {
  if (payoutInput.length === 0) throw new Error('pass --payout-input <wallet-terminal-payout input json>');
  if (devnetAcknowledgment.length === 0) throw new Error('pass --i-mean-devnet <full devnet genesis hash>');
  const input = isAbsolute(payoutInput) ? payoutInput : resolve(process.cwd(), payoutInput);
  const binary = successorBinary(context, env);
  const args = [
    'wallet-terminal-payout-plan',
    '--rpc-url', context.rpcUrl,
    '--i-mean-devnet', devnetAcknowledgment,
    '--input', input,
  ] as const;
  const result = spawn(binary, args, { encoding: 'utf8', env });
  if (result.error !== undefined) throw new Error(`wallet payout producer could not start: ${result.error.message}`);
  if (result.status !== 0) {
    const detail = boundedStderr(result.stderr);
    throw new Error(`wallet payout producer exited ${result.status ?? `by signal ${result.signal ?? 'unknown'}`}${detail === '' ? '' : `: ${detail}`}`);
  }
  if (typeof result.stdout !== 'string') throw new Error('wallet payout producer returned no text');
  if (result.stdout.length === 0 || result.stdout.length > 32_768) throw new Error('wallet payout producer returned output outside the 1..32768 character bound');
  try {
    return parseWalletTerminalPayoutManifestV3(result.stdout.trim());
  } catch (error) {
    throw new Error(`wallet payout producer returned a refused manifest: ${error instanceof Error ? error.message : String(error)}`);
  }
}

export type RedeemablePortfolioCoordinateV3 = Readonly<{
  market: string;
  owner: string;
  position: string;
  winningClaim: number;
  availableQuantity: string;
}>;

/** Refuse a validly shaped producer result if it names a different live holding. */
export function assertWalletTerminalPayoutMatchesPortfolioV3(
  manifest: WalletTerminalPayoutManifestV3,
  expected: RedeemablePortfolioCoordinateV3,
): void {
  if (manifest.request.market !== expected.market || manifest.route.market !== expected.market) {
    throw new Error('wallet payout manifest names another Market');
  }
  if (manifest.request.owner !== expected.owner || manifest.request.recipientOwner !== expected.owner) {
    throw new Error('wallet payout manifest names another Position or recipient owner');
  }
  if (manifest.request.position !== expected.position || manifest.route.position !== expected.position) {
    throw new Error('wallet payout manifest names another Claims Position');
  }
  if (manifest.request.claimIndex !== expected.winningClaim) {
    throw new Error('wallet payout manifest names another winning claim');
  }
  if (manifest.request.quantity !== expected.availableQuantity) {
    throw new Error('wallet payout manifest does not debit the full available winning-claim quantity');
  }
}

export async function redeem(context: CliContext, io: Io, env: NodeJS.ProcessEnv): Promise<number> {
  const marketAddress = context.flags.market;
  if (typeof marketAddress !== 'string') throw new Error('pass --market <address>');
  // Require all local inputs before reading a signer or chain state. The
  // producer itself still runs only after the replay has been admitted.
  const payoutInput = context.flags['payout-input'];
  if (typeof payoutInput !== 'string') throw new Error('pass --payout-input <wallet-terminal-payout input json>');
  const devnetAcknowledgment = context.flags['i-mean-devnet'];
  if (typeof devnetAcknowledgment !== 'string') throw new Error('pass --i-mean-devnet <full devnet genesis hash>');
  const keypair = loadKeypair(context, env);
  const client = rpcClient(context);
  const progressIo: Io = context.json ? Object.freeze({ out: io.err, err: io.err }) : io;

  // What is actually redeemable, before touching anything.
  const view = await inspectPortfolioV1(client, {
    coreProgramId: programId(context, 'core'),
    claimsProgramId: programId(context, 'claims'),
    registryProgramId: programId(context, 'registry'),
    owner: keypair.publicKey.toBase58(),
    marketAddresses: [marketAddress],
  });
  const entry = view.entries[0];
  if (entry === undefined) throw new Error('the market produced no portfolio entry');
  if (entry.position.status !== 'held') {
    progressIo.out(`nothing to redeem: position ${entry.position.status}${entry.position.status === 'refused' ? ` — ${entry.position.reason}` : ''}`);
    return 1;
  }
  block(progressIo, [
    ['position', entry.position.address],
    ['balances', entry.position.balances.join(' / ')],
    ['claim', `${entry.position.claim.kind} — ${entry.position.claim.note}`],
  ]);
  if (entry.position.claim.kind !== 'redeemable') {
    progressIo.out('the position is not redeemable at this floor; nothing was signed');
    return 1;
  }

  // Step 1: admit the Claims-role replay the payout frame requires.
  const state = await inspectClaimsCustodyReplayV1(client, {
    marketAddress,
    claimsProgramId: programId(context, 'claims'),
    custodyProgramId: programId(context, 'custody'),
    registryProgramId: programId(context, 'registry'),
    payer: keypair.publicKey.toBase58(),
  });
  if (state.status === 'refused') {
    progressIo.err(`replay inspection refused: ${state.reason}`);
    return 1;
  }
  if (state.status === 'exists') {
    progressIo.out(`Claims-role Custody replay already exists at ${state.replayAddress} (next revision ${state.nextRevision})`);
  } else {
    const plan = state.plan;
    progressIo.out(`creating the Claims-role Custody replay at ${plan.replayAddress} (${plan.rentLamports} lamports rent, request digest ${plan.custodyRequestDigestHex.slice(0, 16)}…)`);
    if (context.flags['dry-run'] === true) {
      progressIo.out('dry run — nothing signed or submitted');
      return 0;
    }
    plan.transaction.sign([keypair]);
    const outcome = await submitAndConfirm(client, plan.transaction.serialize(), progressIo);
    if (!outcome.succeeded) return 1;
    progressIo.out(`replay created — ${state.note}`);
  }

  // Step 2: run the read-only Rust producer and bind its output to step 0.
  const manifest = produceWalletTerminalPayoutManifestV3(context, env, payoutInput, devnetAcknowledgment);
  assertWalletTerminalPayoutMatchesPortfolioV3(manifest, {
    market: marketAddress,
    owner: view.owner,
    position: entry.position.address,
    winningClaim: entry.position.claim.winningClaim,
    availableQuantity: entry.position.claim.redeemableAtoms,
  });

  if (!context.json) {
    progressIo.out('');
    progressIo.out('wallet payout manifest produced read-only and matched to this Position:');
  }
  io.out(JSON.stringify(manifest));
  return 0;
}
