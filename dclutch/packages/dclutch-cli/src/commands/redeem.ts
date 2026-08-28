/**
 * `dclutch redeem --market <address> --payout-input <json>` — admit the
 * Claims-role replay, then ask the Rust authority for one exact wallet payout
 * manifest and independently bind its coordinates to the portfolio read.
 *
 * The successor subcommands are read-only. With `--payout-alt-plan`, this
 * command asks the Position owner to fund and sign only the standard lookup-
 * table create/extend transactions, then emits the seven-field payout
 * manifest. It does not submit the payout; the web flow owns that signature,
 * receipt, and finalized postcondition boundary.
 */
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { isAbsolute, join, resolve } from 'node:path';

import { inspectClaimsCustodyReplayV1 } from '@dclutch/sdk/claimsCustodyReplay';
import { inspectPortfolioV1 } from '@dclutch/sdk/portfolio';
import {
  parseWalletTerminalPayoutManifestV3,
  type WalletTerminalPayoutManifestV3,
} from '@dclutch/sdk/walletTerminalPayoutV3';

import { loadKeypair, programId, rpcClient, type CliContext } from '../context';
import { assertExactDevnetMutation, devnetGenesisAcknowledgment } from '../mutation';
import { block, type Io } from '../output';
import {
  nextWalletTerminalPayoutAltActionV1,
  observeWalletTerminalPayoutAltV1,
  parseWalletTerminalPayoutAltPlanV1,
  provisionWalletTerminalPayoutAltV1,
  type WalletTerminalPayoutAltPlanV1,
} from '../payoutAlt';
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

export type ProducedWalletTerminalPayoutAltPlanV1 = Readonly<{
  plan: WalletTerminalPayoutAltPlanV1;
  encoded: string;
  input: string;
}>;

/** Prepare the persisted first phase through the same read-only Rust authority. */
export function produceWalletTerminalPayoutAltPlanV1(
  context: CliContext,
  env: NodeJS.ProcessEnv,
  payoutInput: string,
  devnetAcknowledgment: string,
  spawn: SuccessorSpawn = spawnSuccessor,
): ProducedWalletTerminalPayoutAltPlanV1 {
  if (payoutInput.length === 0) throw new Error('pass --payout-input <wallet-terminal-payout input json>');
  if (devnetAcknowledgment.length === 0) throw new Error('pass --i-mean-devnet <full devnet genesis hash>');
  const input = isAbsolute(payoutInput) ? payoutInput : resolve(process.cwd(), payoutInput);
  const source = readFileSync(input);
  const binary = successorBinary(context, env);
  const args = [
    'wallet-terminal-payout-alt-plan',
    '--rpc-url', context.rpcUrl,
    '--i-mean-devnet', devnetAcknowledgment,
    '--input', input,
  ] as const;
  const result = spawn(binary, args, { encoding: 'utf8', env });
  if (result.error !== undefined) throw new Error(`wallet payout ALT producer could not start: ${result.error.message}`);
  if (result.status !== 0) {
    const detail = boundedStderr(result.stderr);
    throw new Error(`wallet payout ALT producer exited ${result.status ?? `by signal ${result.signal ?? 'unknown'}`}${detail === '' ? '' : `: ${detail}`}`);
  }
  if (typeof result.stdout !== 'string') throw new Error('wallet payout ALT producer returned no text');
  const encoded = result.stdout.trim();
  try {
    return Object.freeze({
      plan: parseWalletTerminalPayoutAltPlanV1(encoded, source),
      encoded,
      input,
    });
  } catch (error) {
    throw new Error(`wallet payout ALT producer returned a refused plan: ${error instanceof Error ? error.message : String(error)}`);
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

/** Bind the persisted transport spend before the wallet funds its table. */
export function assertWalletTerminalPayoutAltMatchesPortfolioV1(
  plan: WalletTerminalPayoutAltPlanV1,
  expected: RedeemablePortfolioCoordinateV3,
): void {
  if (plan.payoutInput.market !== expected.market
      || plan.payoutInput.owner !== expected.owner
      || plan.payoutInput.recipientOwner !== expected.owner) {
    throw new Error('wallet payout ALT plan names another Market or Position owner');
  }
  if (plan.payoutInput.claimIndex !== expected.winningClaim
      || plan.payoutInput.quantity !== expected.availableQuantity) {
    throw new Error('wallet payout ALT plan names another winning claim or quantity');
  }
}

function payoutInputPath(value: string): string {
  return isAbsolute(value) ? value : resolve(process.cwd(), value);
}

function payoutManifestFromAltPlan(
  context: CliContext,
  env: NodeJS.ProcessEnv,
  acknowledgment: string,
  plan: WalletTerminalPayoutAltPlanV1,
): WalletTerminalPayoutManifestV3 {
  const directory = mkdtempSync(join(tmpdir(), 'dclutch-payout-input-'));
  const path = join(directory, 'payout-input.json');
  try {
    writeFileSync(path, `${JSON.stringify(plan.payoutInput, null, 2)}\n`, { mode: 0o600 });
    return produceWalletTerminalPayoutManifestV3(context, env, path, acknowledgment);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

async function finalizedAltObservation(
  client: ReturnType<typeof rpcClient>,
  plan: WalletTerminalPayoutAltPlanV1,
) {
  const observed = await client.accountInfo(plan.lookupTable);
  return observeWalletTerminalPayoutAltV1(observed.slot, observed.account, plan.lookupTable);
}

async function provisionAltFromRpc(
  client: ReturnType<typeof rpcClient>,
  plan: WalletTerminalPayoutAltPlanV1,
  signer: ReturnType<typeof loadKeypair>,
  io: Io,
  acknowledgment: string,
) {
  return provisionWalletTerminalPayoutAltV1(plan, signer, {
    observe: () => finalizedAltObservation(client, plan),
    latestMutationBlockhash: async (minimumContextSlot) => {
      await assertExactDevnetMutation(client, acknowledgment, 'redeem lookup-table blockhash acquisition');
      return client.latestBlockhash(minimumContextSlot);
    },
    submit: async (wire) => (await submitAndConfirm(client, wire, io, acknowledgment)).succeeded,
    wait: () => new Promise((resolveWait) => setTimeout(resolveWait, 2_000)),
  });
}

export async function redeem(context: CliContext, io: Io, env: NodeJS.ProcessEnv): Promise<number> {
  const marketAddress = context.flags.market;
  if (typeof marketAddress !== 'string') throw new Error('pass --market <address>');
  // Require all local inputs before reading a signer or chain state. The
  // producer itself still runs only after the replay has been admitted.
  const payoutInput = context.flags['payout-input'];
  if (typeof payoutInput !== 'string') throw new Error('pass --payout-input <wallet-terminal-payout input json>');
  const devnetAcknowledgment = devnetGenesisAcknowledgment(context);
  const keypair = loadKeypair(context, env);
  const client = rpcClient(context);
  await assertExactDevnetMutation(client, devnetAcknowledgment, 'redeem preparation');
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
    await assertExactDevnetMutation(client, devnetAcknowledgment, 'redeem replay transaction signature');
    plan.transaction.sign([keypair]);
    const outcome = await submitAndConfirm(client, plan.transaction.serialize(), progressIo, devnetAcknowledgment);
    if (!outcome.succeeded) return 1;
    let finalizedReplay = false;
    for (let attempt = 0; attempt < 30; attempt += 1) {
      const observed = await inspectClaimsCustodyReplayV1(client, {
        marketAddress,
        claimsProgramId: programId(context, 'claims'),
        custodyProgramId: programId(context, 'custody'),
        registryProgramId: programId(context, 'registry'),
        payer: keypair.publicKey.toBase58(),
      });
      if (observed.status === 'exists') {
        finalizedReplay = true;
        progressIo.out(`replay finalized at ${observed.replayAddress} (next revision ${observed.nextRevision})`);
        break;
      }
      if (observed.status === 'refused') {
        throw new Error(`finalized replay readback refused: ${observed.reason}`);
      }
      await new Promise((resolveWait) => setTimeout(resolveWait, 2_000));
    }
    if (!finalizedReplay) {
      throw new Error('Claims-role Custody replay did not reach finalized readback within 60 seconds; do not resubmit it blindly');
    }
  }

  const expected = Object.freeze({
    market: marketAddress,
    owner: view.owner,
    position: entry.position.address,
    winningClaim: entry.position.claim.winningClaim,
    availableQuantity: entry.position.claim.redeemableAtoms,
  });

  // Step 2: provision or resume the request-specific ordered table when the
  // caller names a durable plan path. The plan is persisted before its first
  // transaction, so a process stop never strands an unresumable partial table.
  const altPathFlag = context.flags['payout-alt-plan'];
  let manifest: WalletTerminalPayoutManifestV3;
  if (typeof altPathFlag === 'string') {
    const altPath = payoutInputPath(altPathFlag);
    const source = readFileSync(payoutInputPath(payoutInput));
    let plan: WalletTerminalPayoutAltPlanV1;
    let encoded: string;
    if (existsSync(altPath)) {
      encoded = readFileSync(altPath, 'utf8').trim();
      plan = parseWalletTerminalPayoutAltPlanV1(encoded, source);
      progressIo.out(`resuming checked payout lookup-table plan ${altPath}`);
    } else {
      const produced = produceWalletTerminalPayoutAltPlanV1(
        context,
        env,
        payoutInput,
        devnetAcknowledgment,
      );
      plan = produced.plan;
      encoded = produced.encoded;
      if (context.flags['dry-run'] !== true) {
        writeFileSync(altPath, `${encoded}\n`, { encoding: 'utf8', flag: 'wx', mode: 0o600 });
        progressIo.out(`saved the resumable payout lookup-table plan before submission: ${altPath}`);
      }
    }
    assertWalletTerminalPayoutAltMatchesPortfolioV1(plan, expected);
    const before = await finalizedAltObservation(client, plan);
    const next = nextWalletTerminalPayoutAltActionV1(plan, before);
    if (context.flags['dry-run'] === true) {
      progressIo.out(`dry run — payout lookup table ${plan.lookupTable}: next action ${next.kind}; nothing signed or submitted`);
      io.out(encoded);
      return 0;
    }
    const provisioned = await provisionAltFromRpc(
      client,
      plan,
      keypair,
      progressIo,
      devnetAcknowledgment,
    );
    const finalAccount = await client.accountInfo(plan.lookupTable, provisioned.finalizedSlot);
    if (finalAccount.account === null) throw new Error('finalized payout lookup table disappeared after provisioning');
    progressIo.out(
      `payout lookup table ready at ${plan.lookupTable}: ${plan.addresses.length} ordered addresses, `
      + `${finalAccount.account.lamports} lamports parked, ${provisioned.transactions} transaction(s) this run`,
    );
    manifest = payoutManifestFromAltPlan(context, env, devnetAcknowledgment, plan);
  } else {
    // A previously provisioned table may still be named directly by the
    // phase-two input. Rust rereads and reauthenticates it before emitting.
    manifest = produceWalletTerminalPayoutManifestV3(context, env, payoutInput, devnetAcknowledgment);
  }
  assertWalletTerminalPayoutMatchesPortfolioV3(manifest, expected);

  if (!context.json) {
    progressIo.out('');
    progressIo.out('wallet payout manifest produced read-only and matched to this Position:');
  }
  io.out(JSON.stringify(manifest));
  return 0;
}
