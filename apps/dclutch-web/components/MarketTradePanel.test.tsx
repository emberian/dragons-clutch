import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketTradePanel from './MarketTradePanel';

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
    expect(html).toContain('a named refusal is this protocol&#x27;s honest interface');
    expect(html).toContain('Walls between preview and execution · named, not hedged');
  });

  it('says why there is no order book instead of faking one', () => {
    expect(html).toContain('there is no order book to take from, by design');
    expect(html).toContain('dclutch/direct-intent-ticket/v1');
    expect(html).toContain('It is not trusted');
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
