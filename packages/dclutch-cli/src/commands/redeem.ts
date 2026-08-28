/**
 * `dclutch redeem` projects one completed campaign through the read-only Rust
 * payout planners, admits its replay and ordered lookup table as separate
 * finalized prerequisites, then journals, signs, submits, and hostile-verifies
 * one exact wallet payout. Submitted ambiguity is preserved for read-only
 * recovery and is never signed or sent again.
 */
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { isAbsolute, join, resolve } from 'node:path';

import { inspectClaimsCustodyReplayV1 } from '@dclutch/sdk/claimsCustodyReplay';
import { inspectPortfolioV1 } from '@dclutch/sdk/portfolio';
import {
  parseWalletTerminalPayoutManifestV3,
  prepareWalletTerminalPayoutV3,
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
import {
  archivePayoutOperationJournalV1,
  authenticateCompletedCampaignEvidenceV1,
  finalizePayoutOperationV1,
  loadPayoutOperationJournalV1,
  markPayoutOperationSubmittedV1,
  parseWalletTerminalPayoutPlanInputV1,
  restorePayoutOperationJournalV1,
  signPayoutPlanV1,
  writeUnsignedPayoutOperationJournalV1,
  type WalletTerminalPayoutPlanInputV1,
} from '../payoutCompletion';

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

export type ProducedWalletTerminalPayoutInputV1 = Readonly<{
  input: WalletTerminalPayoutPlanInputV1;
  encoded: string;
}>;

/** Project one exact campaign dossier through the Rust read-only authority. */
export function produceWalletTerminalPayoutInputV1(
  context: CliContext,
  env: NodeJS.ProcessEnv,
  request: Readonly<{
    plan: string;
    evidence: string;
    market: string;
    owner: string;
    recipient: string;
    claimIndex: number;
    quantity: string;
  }>,
  devnetAcknowledgment: string,
  spawn: SuccessorSpawn = spawnSuccessor,
): ProducedWalletTerminalPayoutInputV1 {
  if (devnetAcknowledgment.length === 0) throw new Error('pass --i-mean-devnet <full devnet genesis hash>');
  const plan = isAbsolute(request.plan) ? request.plan : resolve(process.cwd(), request.plan);
  const evidence = isAbsolute(request.evidence) ? request.evidence : resolve(process.cwd(), request.evidence);
  authenticateCompletedCampaignEvidenceV1(readFileSync(plan), readFileSync(evidence));
  const binary = successorBinary(context, env);
  const args = [
    'wallet-terminal-payout-input',
    '--rpc-url', context.rpcUrl,
    '--i-mean-devnet', devnetAcknowledgment,
    '--plan', plan,
    '--evidence', evidence,
    '--market', request.market,
    '--owner', request.owner,
    '--recipient', request.recipient,
    '--claim-index', String(request.claimIndex),
    '--quantity', request.quantity,
  ] as const;
  const result = spawn(binary, args, { encoding: 'utf8', env });
  if (result.error !== undefined) throw new Error(`wallet payout input projector could not start: ${result.error.message}`);
  if (result.status !== 0) {
    const detail = boundedStderr(result.stderr);
    throw new Error(`wallet payout input projector exited ${result.status ?? `by signal ${result.signal ?? 'unknown'}`}${detail === '' ? '' : `: ${detail}`}`);
  }
  if (typeof result.stdout !== 'string') throw new Error('wallet payout input projector returned no text');
  const encoded = result.stdout.trim();
  if (encoded.length === 0 || encoded.length > 32_768) throw new Error('wallet payout input projector returned output outside the 1..32768 character bound');
  const input = parseWalletTerminalPayoutPlanInputV1(encoded);
  if (input.market !== request.market || input.owner !== request.owner || input.recipientOwner !== request.owner
      || input.recipient !== request.recipient || input.claimIndex !== request.claimIndex
      || input.quantity !== request.quantity) {
    throw new Error('wallet payout projected input substituted its Market, owner, recipient, winning claim, or quantity');
  }
  return Object.freeze({ input, encoded });
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
  recipient: string;
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
  if (manifest.request.recipient !== expected.recipient || manifest.route.recipient !== expected.recipient) {
    throw new Error('wallet payout manifest names another recipient token account');
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
  if (plan.payoutInput.recipient !== expected.recipient) {
    throw new Error('wallet payout ALT plan names another recipient token account');
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
  const owner = context.flags.payer;
  if (typeof owner !== 'string') throw new Error('pass --payer <Position owner address>');
  const recipient = context.flags.recipient;
  if (typeof recipient !== 'string') throw new Error('pass --recipient <collateral token account>');
  const journalFlag = context.flags['payout-journal'];
  if (typeof journalFlag !== 'string') throw new Error('pass --payout-journal <durable local journal path>');
  const journalPath = payoutInputPath(journalFlag);
  const devnetAcknowledgment = devnetGenesisAcknowledgment(context);
  const client = rpcClient(context);
  const progressIo: Io = context.json ? Object.freeze({ out: io.err, err: io.err }) : io;

  // A submitted payout is never signed or sent again. Recovery authenticates
  // the saved packet and finalized poststate before any key is consulted.
  const savedJournal = loadPayoutOperationJournalV1(journalPath);
  if (savedJournal !== null) {
    if (savedJournal.clusterGenesis !== devnetAcknowledgment
        || savedJournal.market !== marketAddress || savedJournal.owner !== owner) {
      throw new Error('the saved payout journal belongs to another cluster, Market, or owner');
    }
    if (savedJournal.phase === 'submitted') {
      await assertExactDevnetMutation(client, devnetAcknowledgment, 'redeem submitted-journal recovery');
      const restored = await restorePayoutOperationJournalV1(savedJournal);
      if (restored.manifest.request.recipient !== recipient) {
        throw new Error('the submitted payout journal belongs to another recipient; it remains preserved');
      }
      for (let attempt = 0; attempt < 60; attempt += 1) {
        try {
          const finalized = await finalizePayoutOperationV1(client, savedJournal, restored.plan);
          const archive = archivePayoutOperationJournalV1(journalPath, savedJournal, 'finalized');
          io.out(JSON.stringify({ status: 'finalized', ...finalized, journalArchive: archive }));
          return 0;
        } catch (error) {
          if (!(error instanceof Error) || !error.message.includes('not available at finalized commitment yet')) throw error;
          if (attempt === 59) {
            throw new Error('the submitted payout is still ambiguous after 60 finalized reads; the journal remains preserved and must not be resubmitted');
          }
          await new Promise((resolveWait) => setTimeout(resolveWait, 1_000));
        }
      }
    }
    if (context.flags['discard-unsigned-payout'] !== true) {
      throw new Error('an unsigned payout journal already exists; inspect it or pass --discard-unsigned-payout to archive it without signing');
    }
    const archive = archivePayoutOperationJournalV1(journalPath, savedJournal, 'discarded');
    progressIo.out(`archived the unsigned payout plan without signing: ${archive}`);
  }

  await assertExactDevnetMutation(client, devnetAcknowledgment, 'redeem preparation');

  // What is actually redeemable, before touching anything.
  const view = await inspectPortfolioV1(client, {
    coreProgramId: programId(context, 'core'),
    claimsProgramId: programId(context, 'claims'),
    registryProgramId: programId(context, 'registry'),
    owner,
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
    payer: owner,
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
    const keypair = loadKeypair(context, env);
    if (keypair.publicKey.toBase58() !== owner) throw new Error('the named replay signer is not --payer');
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
        payer: owner,
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
    progressIo.out('replay prerequisite is finalized; rerun redeem to prepare the payout without carrying this signature across phases');
    return 0;
  }

  const expected = Object.freeze({
    market: marketAddress,
    owner: view.owner,
    position: entry.position.address,
    recipient,
    winningClaim: entry.position.claim.winningClaim,
    availableQuantity: entry.position.claim.redeemableAtoms,
  });

  // Prefer one authenticated completed campaign dossier. Rust owns the flat
  // payout-input truth; this client only hostile-validates and binds its exact
  // projection. An already-projected input remains accepted for automation.
  const legacyInput = context.flags['payout-input'];
  const campaignPlan = context.flags.spec;
  const campaignEvidence = context.flags['payout-evidence'];
  if (typeof campaignPlan === 'string' && typeof legacyInput === 'string') {
    throw new Error('choose completed --spec/--payout-evidence input or --payout-input, not both');
  }
  if (typeof campaignPlan !== 'string' && typeof legacyInput !== 'string') {
    throw new Error('pass --spec <campaign plan> with --session <completed evidence>, or pass --payout-input <projected input>');
  }
  if (typeof campaignPlan === 'string' && typeof campaignEvidence !== 'string') {
    throw new Error('campaign payout projection requires --payout-evidence <completed evidence>');
  }
  const projectedDirectory = typeof campaignPlan === 'string'
    ? mkdtempSync(join(tmpdir(), 'dclutch-completed-payout-')) : null;
  let payoutInput = typeof legacyInput === 'string' ? legacyInput : '';
  try {
    if (projectedDirectory !== null) {
      const projected = produceWalletTerminalPayoutInputV1(context, env, {
        plan: campaignPlan as string,
        evidence: campaignEvidence as string,
        market: marketAddress,
        owner,
        recipient,
        claimIndex: entry.position.claim.winningClaim,
        quantity: entry.position.claim.redeemableAtoms,
      }, devnetAcknowledgment);
      payoutInput = join(projectedDirectory, 'payout-input.json');
      writeFileSync(payoutInput, `${projected.encoded}\n`, { encoding: 'utf8', flag: 'wx', mode: 0o600 });
    }

    // Step 2 is a separately finalized prerequisite. Its plan is written
    // before the first table transaction. A payout is never signed in the
    // same invocation that changes the table.
    const altPathFlag = context.flags['payout-alt-plan'];
    let manifest: WalletTerminalPayoutManifestV3;
    if (typeof altPathFlag === 'string') {
      const altPath = payoutInputPath(altPathFlag);
      const source = readFileSync(payoutInputPath(payoutInput));
      let altPlan: WalletTerminalPayoutAltPlanV1;
      let encoded: string;
      if (existsSync(altPath)) {
        encoded = readFileSync(altPath, 'utf8').trim();
        altPlan = parseWalletTerminalPayoutAltPlanV1(encoded, source);
        progressIo.out(`resuming checked payout lookup-table plan ${altPath}`);
      } else {
        const produced = produceWalletTerminalPayoutAltPlanV1(
          context,
          env,
          payoutInput,
          devnetAcknowledgment,
        );
        altPlan = produced.plan;
        encoded = produced.encoded;
        if (context.flags['dry-run'] !== true) {
          writeFileSync(altPath, `${encoded}\n`, { encoding: 'utf8', flag: 'wx', mode: 0o600 });
          progressIo.out(`saved the resumable payout lookup-table plan before submission: ${altPath}`);
        }
      }
      assertWalletTerminalPayoutAltMatchesPortfolioV1(altPlan, expected);
      const before = await finalizedAltObservation(client, altPlan);
      const next = nextWalletTerminalPayoutAltActionV1(altPlan, before);
      if (context.flags['dry-run'] === true) {
        progressIo.out(`dry run — payout lookup table ${altPlan.lookupTable}: next action ${next.kind}; nothing signed or submitted`);
        io.out(encoded);
        return 0;
      }
      if (next.kind !== 'ready') {
        const keypair = loadKeypair(context, env);
        if (keypair.publicKey.toBase58() !== owner) throw new Error('the named lookup-table signer is not --payer');
        const provisioned = await provisionAltFromRpc(
          client,
          altPlan,
          keypair,
          progressIo,
          devnetAcknowledgment,
        );
        const finalAccount = await client.accountInfo(altPlan.lookupTable, provisioned.finalizedSlot);
        if (finalAccount.account === null) throw new Error('finalized payout lookup table disappeared after provisioning');
        progressIo.out(
          `payout lookup table ready at ${altPlan.lookupTable}: ${altPlan.addresses.length} ordered addresses, `
          + `${finalAccount.account.lamports} lamports parked, ${provisioned.transactions} transaction(s) this run`,
        );
        progressIo.out('lookup-table prerequisite is finalized; rerun redeem to journal and sign the payout');
        return 0;
      }
      manifest = payoutManifestFromAltPlan(context, env, devnetAcknowledgment, altPlan);
    } else {
      if (projectedDirectory !== null) {
        throw new Error('campaign payout projection requires --payout-alt-plan <durable plan path>');
      }
      manifest = produceWalletTerminalPayoutManifestV3(context, env, payoutInput, devnetAcknowledgment);
    }
    assertWalletTerminalPayoutMatchesPortfolioV3(manifest, expected);

    if (context.flags['dry-run'] === true) {
      io.out(JSON.stringify({ status: 'ready', manifest, note: 'dry run — no payout journal, signature, or submission was created' }));
      return 0;
    }

    // The exact v0 packet and verifier prestate are durable before the key is
    // loaded. Its exact first signature is durable before raw submission.
    await assertExactDevnetMutation(client, devnetAcknowledgment, 'redeem payout preparation');
    const prepared = await prepareWalletTerminalPayoutV3(client, manifest, owner);
    if (prepared.requiredSigners.length !== 1 || prepared.requiredSigners[0] !== owner) {
      throw new Error('the payout transaction does not have the Position owner as its sole signer');
    }
    const unsignedJournal = writeUnsignedPayoutOperationJournalV1(
      journalPath,
      devnetAcknowledgment,
      manifest,
      prepared,
    );
    progressIo.out(`saved the exact unsigned payout plan before loading the signer: ${journalPath}`);
    await assertExactDevnetMutation(client, devnetAcknowledgment, 'redeem payout signature');
    const keypair = loadKeypair(context, env);
    if (keypair.publicKey.toBase58() !== owner) {
      throw new Error('the named payout signer is not --payer; the unsigned journal remains preserved');
    }
    const signed = signPayoutPlanV1(prepared, keypair);
    const submittedJournal = markPayoutOperationSubmittedV1(journalPath, unsignedJournal, signed.signature);
    await assertExactDevnetMutation(client, devnetAcknowledgment, 'redeem payout raw submission');
    const submittedSignature = await client.sendRawTransaction(signed.wireBytes);
    if (submittedSignature !== signed.signature) {
      throw new Error('the RPC returned another payout signature; the submitted journal remains preserved and must not be replayed');
    }
    for (let attempt = 0; attempt < 60; attempt += 1) {
      try {
        const finalized = await finalizePayoutOperationV1(client, submittedJournal, prepared);
        const archive = archivePayoutOperationJournalV1(journalPath, submittedJournal, 'finalized');
        io.out(JSON.stringify({ status: 'finalized', ...finalized, journalArchive: archive }));
        return 0;
      } catch (error) {
        if (!(error instanceof Error) || !error.message.includes('not available at finalized commitment yet')) throw error;
        if (attempt === 59) {
          throw new Error('the submitted payout is still ambiguous after 60 finalized reads; the journal remains preserved and must not be resubmitted');
        }
        await new Promise((resolveWait) => setTimeout(resolveWait, 1_000));
      }
    }
    throw new Error('unreachable payout finalization state');
  } finally {
    if (projectedDirectory !== null) rmSync(projectedDirectory, { recursive: true, force: true });
  }
}
