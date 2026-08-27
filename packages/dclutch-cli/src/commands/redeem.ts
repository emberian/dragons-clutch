/**
 * `dclutch redeem --market <address>` — as much of redemption as the chain
 * admits from a wallet, stated plainly.
 *
 * The standing precondition of every payout plan is the CLAIMS-role Custody
 * replay (ADR-0008): this command inspects it and, when it does not exist,
 * signs and submits the exact creation transaction the SDK derives. The
 * payout instruction itself for a plain Claims Position is a NAMED PROTOCOL
 * GAP — claims/terminal_settlement_v3 admits caller role Core or Trading
 * only — and this command says so with the SDK's own words rather than
 * pretending a flag would fix it.
 */
import { inspectClaimsCustodyReplayV1, PLAIN_POSITION_PAYOUT_BLOCK_V1 } from '@dclutch/sdk/claimsCustodyReplay';
import { inspectPortfolioV1 } from '@dclutch/sdk/portfolio';

import { loadKeypair, programId, rpcClient, type CliContext } from '../context';
import { block, type Io } from '../output';
import { submitAndConfirm } from '../submit';

export async function redeem(context: CliContext, io: Io, env: NodeJS.ProcessEnv): Promise<number> {
  const marketAddress = context.flags.market;
  if (typeof marketAddress !== 'string') throw new Error('pass --market <address>');
  const keypair = loadKeypair(context, env);
  const client = rpcClient(context);

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
    io.out(`nothing to redeem: position ${entry.position.status}${entry.position.status === 'refused' ? ` — ${entry.position.reason}` : ''}`);
    return 1;
  }
  block(io, [
    ['position', entry.position.address],
    ['balances', entry.position.balances.join(' / ')],
    ['claim', `${entry.position.claim.kind} — ${entry.position.claim.note}`],
  ]);
  if (entry.position.claim.kind !== 'redeemable') {
    io.out('the position is not redeemable at this floor; nothing was signed');
    return 1;
  }

  // Step 1, the one wallet-constructible mutation: the Claims-role replay.
  const state = await inspectClaimsCustodyReplayV1(client, {
    marketAddress,
    claimsProgramId: programId(context, 'claims'),
    custodyProgramId: programId(context, 'custody'),
    registryProgramId: programId(context, 'registry'),
    payer: keypair.publicKey.toBase58(),
  });
  if (state.status === 'refused') {
    io.err(`replay inspection refused: ${state.reason}`);
    return 1;
  }
  if (state.status === 'exists') {
    io.out(`Claims-role Custody replay already exists at ${state.replayAddress} (next revision ${state.nextRevision})`);
  } else {
    const plan = state.plan;
    io.out(`creating the Claims-role Custody replay at ${plan.replayAddress} (${plan.rentLamports} lamports rent, request digest ${plan.custodyRequestDigestHex.slice(0, 16)}…)`);
    if (context.flags['dry-run'] === true) {
      io.out('dry run — nothing signed or submitted');
      return 0;
    }
    plan.transaction.sign([keypair]);
    const outcome = await submitAndConfirm(client, plan.transaction.serialize(), io);
    if (!outcome.succeeded) return 1;
    io.out(`replay created — ${state.note}`);
  }

  // Step 2 is the protocol's, not this tool's. Say it in the SDK's words.
  io.out('');
  io.out(`payout: ${PLAIN_POSITION_PAYOUT_BLOCK_V1}`);
  return 0;
}
