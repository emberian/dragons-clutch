import { describe, expect, it } from 'vitest';

import { enumerateCoreMarketAddressesV1, inspectMarketDiscoveryV1 } from './marketDiscovery';
import { SolanaRpcClient } from './rpc';

/**
 * What one cold discovery read actually costs a public endpoint.
 *
 * The unit pin beside this file fixes the batch arithmetic against a fabricated
 * chain. This one asks the deployed devnet substrate the same question through
 * the real client, because the arithmetic was never the doubtful part -- the
 * doubtful part was whether a browser opening the live site fits inside a free
 * endpoint's burst allowance, measured at roughly eight heavy reads.
 *
 * Opt-in on DCLUTCH_LIVE_DEVNET=1; without it this skips rather than spending
 * someone else's rate limit during an ordinary suite run.
 */

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

const DEVNET_ENDPOINT = 'https://api.devnet.solana.com';
const DEVNET_PROGRAMS = Object.freeze({
  registry: 'Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj',
  custody: '34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH',
  claims: '85hwTeQGabwFRs71Hafvngb1UmHb6dQoumBv3VV4epNN',
  core: 'HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N',
});

/** The burst allowance PUB measured against this endpoint on 2026-08-29. */
const MEASURED_BURST_ALLOWANCE = 8;

describe('the devnet read budget of one discovery listing', () => {
  live('reads every deployed Market inside the endpoint\'s measured burst allowance', async () => {
    const client = new SolanaRpcClient(DEVNET_ENDPOINT);
    const widths: number[] = [];
    // Delegation, not a Proxy: the client reads its endpoint out of a private
    // field, and a Proxy receiver is not an instance that may touch one.
    const counted = {
      finalizedSlot: () => client.finalizedSlot(),
      programHeaders: (program: string) => client.programHeaders(program),
      multipleAccounts: (addresses: ReadonlyArray<string>, floor?: string) => {
        widths.push(addresses.length);
        return client.multipleAccounts(addresses, floor);
      },
    };

    const enumeration = await enumerateCoreMarketAddressesV1(counted, DEVNET_PROGRAMS.core);
    if (enumeration.mode !== 'program-scan') throw new Error(`devnet enumeration refused: ${enumeration.reason}`);
    const discovery = await inspectMarketDiscoveryV1(counted, {
      coreProgramId: DEVNET_PROGRAMS.core,
      registryProgramId: DEVNET_PROGRAMS.registry,
      claimsProgramId: DEVNET_PROGRAMS.claims,
      custodyProgramId: DEVNET_PROGRAMS.custody,
      addresses: enumeration.addresses,
      enumeration,
    });

    // The listing completed: reaching this line at all is the finding, because
    // the same read refused partway through with a 429 before the join was
    // staged into batched rounds.
    expect(discovery.cards).toHaveLength(enumeration.addresses.length);
    expect(widths.length).toBeLessThanOrEqual(MEASURED_BURST_ALLOWANCE);
    expect(widths.every((width) => width >= 1 && width <= 32)).toBe(true);
    console.log(`devnet discovery: ${enumeration.addresses.length} Markets, getMultipleAccounts widths ${JSON.stringify(widths)} (${widths.length} calls) plus one getProgramAccounts and one getSlot`);
  }, 120_000);
});
