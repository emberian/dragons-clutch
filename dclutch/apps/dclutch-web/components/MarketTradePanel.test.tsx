import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketTradePanel from './MarketTradePanel';
import { DIRECT_PACKET_WALL_V1, DIRECT_PRESTATE_WALL_V1 } from '@/lib/directTradeSpine';

describe('the market-detail trade panel', () => {
  const html = renderToStaticMarkup(<MarketTradePanel
    endpoint="http://127.0.0.1:8899"
    marketAddress="4fQNy8k7G7bZ9cak6pb2VnigV2F5fbhs7YnYFWQ2LQYH"
    coreProgramId=""
    registryProgramId={null}
    claimsProgramId={null}
    tradingProgramId={null}
    liability={null}
  />);

  it('treats the named refusal as the product surface, not a disabled button', () => {
    expect(html).toContain('tells you exactly why in one sentence');
    expect(html).toContain('never a greyed-out button with no reason');
  });

  it('carries the packet and prestate walls as exact named facts, never hedges', () => {
    // The walls render after inspection; their content is pinned at the source.
    expect(DIRECT_PACKET_WALL_V1.name).toBe('packet');
    expect(DIRECT_PRESTATE_WALL_V1.name).toBe('prestate');
    expect(DIRECT_PACKET_WALL_V1.detail).toContain('1,268 > 1,232');
    expect(DIRECT_PACKET_WALL_V1.detail).toContain('nothing your browser or wallet can do differently');
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
});
