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
    expect(html).toContain('tells you exactly why in one sentence');
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

  it('keeps public Direct execution fail-closed until its canonical route and finalizer exist', () => {
    expect(html).toContain('does not accept a pasted route or ask for a signature');
    expect(html).toContain('A Claims Position holds claim balances');
    expect(html).toContain('never used as your collateral account');
    expect(html).toContain('authenticate your participant accounts');
    expect(html).toContain('calculate an unsigned crossing');
    expect(html).toContain('canonical public route manifest');
    expect(html).toContain('exact finalized receipt and poststate');
    expect(html).toContain('Browser data is an untrusted projection');
    expect(html).not.toContain('taker collateral account and Position derive under it');
    expect(html).not.toContain('Route manifest JSON');
    expect(html).not.toContain('Build, sign as payer, and submit');
  });
});
