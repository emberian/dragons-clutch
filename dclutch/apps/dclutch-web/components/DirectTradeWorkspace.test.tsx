import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import DirectTradeWorkspace from './DirectTradeWorkspace';
import { HOT_FIXED_ACCOUNT_COUNT_V3 } from '@dclutch/sdk/generated/directInlineV3';

describe('Direct V3 trade workbench', () => {
  it('presents a real chain-derived route and a read-only execution boundary', () => {
    const html = renderToStaticMarkup(<DirectTradeWorkspace />);
    expect(html).toContain('Direct trade');
    expect(html).toContain('Operator tool');
    expect(html).toContain('see the exact collateral arithmetic');
    expect(html).toContain('Route manifest · JSON');
    expect(html).toContain('infrastructure.checked');
    expect(html).toContain('2,360 bytes');
    expect(html).toContain('Review exact arithmetic');
    expect(html).toContain('Execution remains closed');
    // This console reads and never sends -- but it used to say "read-only
    // until the finalizer lands", which reads as "trading does not work",
    // while the market page's trade panel signs and submits. It now says
    // which page trades.
    expect(html).toContain('To place a trade, open the market on');
    expect(html).toContain('href="/markets"');
    expect(html).not.toContain('read-only until the finalizer lands');
    for (const forbidden of ['Connect identity', 'Sign this maker message', 'Sign as transaction payer', 'Download exact packet', 'Submit signed transaction']) {
      expect(html).not.toContain(forbidden);
    }
    expect(html).toContain('No chain state has been read.');
    expect(html).not.toContain('sample market');
    expect(html).not.toContain('mock balance');
  });

  it('scaffolds exactly the 39-row fixed frame the route acquirer requires', () => {
    // The scaffold shipped 38 rows for two days after decision 0005 added the
    // capability seal at coordinate 38: everyone pasting it got 'hot route
    // requires exactly 39 fixed accounts'. The count is imported, not restated.
    const html = renderToStaticMarkup(<DirectTradeWorkspace />);
    const scaffold = html.match(/&quot;fixedAccounts&quot;: \[[\s\S]*?\]/);
    expect(scaffold).not.toBeNull();
    const rows = (scaffold?.[0].match(/&quot;role&quot;/g) ?? []).length;
    expect(rows).toBe(HOT_FIXED_ACCOUNT_COUNT_V3);
    expect(html).toContain('Capability seal');
  });
});
