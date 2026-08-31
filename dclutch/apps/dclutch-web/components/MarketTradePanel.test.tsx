import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketTradePanel from './MarketTradePanel';
import {
  DIRECT_PACKET_BUDGET_EVIDENCE_V1,
  DIRECT_PRESTATE_WALL_V1,
  directPacketWallV1,
} from '@/lib/directTradeSpine';

describe('the market-detail trade panel', () => {
  const html = renderToStaticMarkup(<MarketTradePanel
    endpoint="http://127.0.0.1:8899"
    marketAddress="4fQNy8k7G7bZ9cak6pb2VnigV2F5fbhs7YnYFWQ2LQYH"
    coreProgramId=""
    registryProgramId={null}
    claimsProgramId={null}
    tradingProgramId={null}
    custodyProgramId={null}
    rentProgramId={null}
    liability={null}
  />);

  it('treats the named refusal as the product surface, not a disabled button', () => {
    expect(html).toContain('says why in one sentence');
    expect(html).toContain('never a greyed-out button with no reason');
  });

  it('carries the measured packet margin and the remaining prestate wall as exact facts', () => {
    expect(DIRECT_PACKET_BUDGET_EVIDENCE_V1).toEqual({
      wireBytes: 1_204, packetLimit: 1_232, marginBytes: 28, computeUnitLimit: 1_400_000,
    });
    expect(directPacketWallV1(DIRECT_PACKET_BUDGET_EVIDENCE_V1.wireBytes)).toBeNull();
    expect(directPacketWallV1(1_233)?.name).toBe('packet');
    expect(DIRECT_PRESTATE_WALL_V1.name).toBe('prestate');
  });

  it('starts from an honest empty state and links the advanced workbench', () => {
    expect(html).toContain('The chain has not been asked about trading this Market yet.');
    expect(html).toContain('Ask the chain about trading here');
    expect(html).toContain('Advanced: full route workbench');
  });

  it('never invents market-data metrics on a trading surface', () => {
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'TVL', 'APR', 'APY', '$', '24h', 'P&L']) {
      expect(html).not.toContain(forbidden);
    }
  });

  // This panel used to promise "There is no submit button here", and this test
  // pinned that sentence. Then submission shipped and the sentence stayed --
  // green, and untrue to every reader, on a public page. The assertions below
  // therefore pin the guarantees the panel actually keeps, and refuse the old
  // sentence by name so it cannot come back once it is no longer true.
  it('describes the submission it really performs, and the guarantees around it', () => {
    expect(html).not.toContain('There is no submit button here');
    expect(html).not.toContain('signed packet is never described as an executed trade');
    // What is still true, and is the part that mattered: signing is not
    // sending, sending happens once, and a signature is not a trade.
    expect(html).toContain('Signing sends nothing.');
    expect(html).toContain('it happens once');
    expect(html).toContain('rather than sending a second one');
    expect(html).toContain('Nothing is called a trade until the chain reports it finalized');
    expect(html).toContain('re-reads both sides of the trade from the chain');
    expect(html).toContain('If your wallet is paying');
    expect(html).toContain('If an operator is paying');
    expect(html).toContain('never your collateral account');
    expect(html).toContain('the programs on chain are what is true');
    expect(html).not.toContain('taker collateral account and Position derive under it');
    expect(html).not.toContain('Build, sign as payer, and submit');
  });
});
