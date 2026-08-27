import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import DirectTradeWorkspace from './DirectTradeWorkspace';
import { HOT_FIXED_ACCOUNT_COUNT_V3 } from '@/lib/generated/directInlineV3';

describe('Direct V3 trade workbench', () => {
  it('presents a real chain-derived route and explicit transaction boundary', () => {
    const html = renderToStaticMarkup(<DirectTradeWorkspace />);
    expect(html).toContain('One signed price.');
    expect(html).toContain('ProgramSetV2 → CapabilityProgramV4');
    expect(html).toContain('Profile14/LifecycleV5');
    expect(html).toContain('Exact Hot39 + strategy/runtime-suffix + one canonical LUT route manifest');
    expect(html).toContain('2,280-byte checked V4-capable infrastructure');
    expect(html).toContain('runtime-u32 outcome coordinates');
    expect(html).toContain('222-byte Ed25519 evidence');
    expect(html).toContain('never duplicates them');
    expect(html).toContain('Build exact unsigned v0 transaction');
    expect(html).toContain('Sign as transaction payer');
    expect(html).toContain('Download exact packet');
    expect(html).toContain('Submission is deliberately outside this workbench');
    expect(html).not.toContain('Submit signed transaction');
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
