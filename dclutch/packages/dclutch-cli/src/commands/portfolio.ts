/**
 * `dclutch portfolio [owner]` — the indexer-free position rollup: enumerate
 * markets, derive each Claims Position address locally, and report exact
 * balances and redeemability at one finalized floor. The owner defaults to
 * the named keypair's public key; an explicit address argument needs no key
 * at all (a portfolio is a public fact).
 */
import { enumerateCoreMarketAddressesV1 } from '@dclutch/sdk/marketDiscovery';
import { PORTFOLIO_MAX_MARKETS, inspectPortfolioV1 } from '@dclutch/sdk/portfolio';

import { loadKeypair, optionalProgramId, programId, rpcClient, type CliContext } from '../context';
import { block, type Io } from '../output';

export async function portfolio(context: CliContext, io: Io, ownerArgument: string | undefined, env: NodeJS.ProcessEnv): Promise<number> {
  const owner = ownerArgument ?? loadKeypair(context, env).publicKey.toBase58();
  const client = rpcClient(context);
  const coreProgramId = programId(context, 'core');
  const enumeration = await enumerateCoreMarketAddressesV1(client, coreProgramId);
  const marketAddresses = [...new Set([...enumeration.addresses, ...context.session.markets])].slice(0, PORTFOLIO_MAX_MARKETS);
  const view = await inspectPortfolioV1(client, {
    coreProgramId,
    registryProgramId: optionalProgramId(context, 'registry'),
    claimsProgramId: optionalProgramId(context, 'claims'),
    owner,
    marketAddresses,
  });
  if (context.json) {
    io.out(JSON.stringify(view, null, 2));
    return 0;
  }
  io.out(`portfolio of ${owner} at finalized slot ${view.floorSlot} across ${marketAddresses.length} market(s)`);
  for (const entry of view.entries) {
    io.out(`  market ${entry.marketAddress}${entry.market.status === 'refused' ? ` (market refused: ${entry.market.refusal})` : ''}`);
    const position = entry.position;
    if (position.status === 'held') {
      block(io, [
        ['position', position.address],
        ['balances', position.balances.join(' / ')],
        ['claim', position.claim.kind],
        ['note', position.claim.note],
      ]);
    } else if (position.status === 'absent') {
      block(io, [['position', `absent — ${position.note}`]]);
    } else {
      block(io, [['position', `refused — ${position.reason}`]]);
    }
  }
  if (view.entries.length === 0) io.out('  no markets to derive positions against');
  return 0;
}
