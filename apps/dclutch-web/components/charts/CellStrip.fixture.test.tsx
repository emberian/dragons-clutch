import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { LIVE, liveRpcAccount } from '@/fixtures/liveOpenMarket';
import { sha256 } from '@/lib/bytes';
import { REALM_SCHEMA_RELEASE_ID_V1 } from '@/lib/generated/coreFound';
import { requiredBackingMeaningV1 } from '@/lib/marketDetail';
import { inspectMarketDiscoveryV1 } from '@/lib/marketDiscovery';
import { deriveFinalizedRecordAddressesV1 } from '@/lib/releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from '@/lib/rpc';

import CellStrip from './CellStrip';

/**
 * The strip against real chain bytes, not strings this test invented.
 *
 * `fixtures/live-open-market.json` is finalized account state copied verbatim
 * off a campaign validator. Running the same discovery join the Market detail
 * surface runs and feeding its liability into CellStrip exactly as the mount
 * does proves the props contract against the chain's own shapes — so the
 * chart cannot drift into agreeing only with its own unit tests.
 */

const SLOT = '99';

function client(accounts: ReadonlyMap<string, RpcAccount>): Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'> {
  return {
    finalizedSlot: async () => SLOT,
    multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
      slot: SLOT,
      accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null }))),
    }),
  };
}

describe('CellStrip over the live fixture', () => {
  it('renders the Claims aggregate the discovery join actually decodes', async () => {
    const accounts = new Map<string, RpcAccount>([
      [LIVE.market.address, liveRpcAccount(LIVE.market)],
      [LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate)],
    ]);
    const realm = deriveFinalizedRecordAddressesV1(LIVE.programs.registry, REALM_SCHEMA_RELEASE_ID_V1, await sha256(LIVE.realmRecord.data));
    accounts.set(realm.record, liveRpcAccount(LIVE.realmRecord));

    const discovery = await inspectMarketDiscoveryV1(client(accounts), {
      coreProgramId: LIVE.programs.core,
      registryProgramId: LIVE.programs.registry,
      claimsProgramId: LIVE.programs.claims,
      custodyProgramId: LIVE.programs.custody,
      addresses: [LIVE.market.address],
    });
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    if (card.liability.status !== 'bound') throw new Error(card.liability.reason);

    // Exactly the MarketDetailWorkspace mount, fed by the join.
    const html = renderToStaticMarkup(<CellStrip
      supplies={card.liability.supplyAtoms}
      winner={card.settlement.status === 'terminal' ? card.settlement.winner : null}
      requiredBackingAtoms={card.liability.requiredBackingAtoms}
      requiredBackingNote={requiredBackingMeaningV1(card.liability.requiredBackingBasis)}
      caption="Each cell is one claim; heights are issued claim atoms from the Claims aggregate."
    />);
    expect(html.split('viz-hit').length - 1).toBe(card.liability.claimCount);
    expect(html).toContain(`claim 0 · ${card.liability.supplyAtoms[0]} atoms`);
    expect(html).toContain(`required backing · ${card.liability.requiredBackingAtoms} atoms`);
    expect(html).toContain('largest claim supply');
  });
});
