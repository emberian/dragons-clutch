/**
 * `dclutch spine --market <address>` — is this market tradable through the
 * Direct path right now, and if not, exactly which walls stand?
 *
 * The spine derivation reads everything from the market alone: the manifest
 * entry, the immutable program set, price scale and fee, the root prestate,
 * and (given `--keypair` or an owner argument) the caller's own trading
 * prestate. Each missing precondition is a named wall, not a generic error.
 * This is read-only inspection; it does not enable the disabled public
 * `buy`/`sell` mutation commands.
 */
import { inspectDirectTradeSpineV1 } from '@dclutch/sdk/directTradeSpine';

import { bindDeploymentIdentity, loadKeypair, optionalProgramId, programId, rpcClient, type CliContext } from '../context';
import { block, type Io } from '../output';

export async function spine(context: CliContext, io: Io, ownerArgument: string | undefined, env: NodeJS.ProcessEnv): Promise<number> {
  const marketAddress = context.flags.market;
  if (typeof marketAddress !== 'string') throw new Error('pass --market <address>');
  let owner: string | null = ownerArgument ?? null;
  if (owner === null) {
    try {
      owner = loadKeypair(context, env).publicKey.toBase58();
    } catch {
      owner = null; // ownerless inspection is still a complete market view
    }
  }
  const client = rpcClient(context);
  await bindDeploymentIdentity(context, client, 'spine');
  const view = await inspectDirectTradeSpineV1(client, {
    marketAddress,
    coreProgramId: programId(context, 'core'),
    registryProgramId: programId(context, 'registry'),
    tradingProgramId: optionalProgramId(context, 'trading'),
    claimsProgramId: optionalProgramId(context, 'claims'),
    owner,
  });
  if (context.json) {
    io.out(JSON.stringify(view, (_key, value) => (typeof value === 'bigint' ? value.toString() : value), 2));
    return 0;
  }
  if (view.status === 'refused') {
    io.err(`spine refused: ${view.reason}`);
    return 1;
  }
  io.out(`market ${view.marketAddress} at finalized slot ${view.observedSlot}`);
  block(io, [
    ['phase', String(view.phase)],
    ['generation', view.generation],
    ['price scale', view.priceScale.toString()],
    ['fee', `${view.feeBasisPoints} bps to ${view.feeRecipient}`],
    ['root prestate', view.rootExists === null ? 'not derivable' : view.rootExists ? `standing at ${view.rootAddress}` : `absent (${view.rootAddress})`],
    ['your position', owner === null ? 'no owner given' : view.positionExists === null ? 'not derivable' : view.positionExists ? `standing at ${view.positionAddress}` : `absent (${view.positionAddress})`],
    ['tradable', view.tradable ? 'yes' : 'no'],
  ]);
  if (view.walls.length > 0) {
    io.out('  walls:');
    for (const wall of view.walls) io.out(`    ${wall.name}: ${wall.detail}`);
  }
  io.out(`  ${view.reason}`);
  return view.tradable ? 0 : 1;
}
