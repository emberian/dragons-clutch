import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import DirectTradeWorkspace from './DirectTradeWorkspace';

describe('Direct V3 trade workbench', () => {
  it('presents a real chain-derived route and explicit transaction boundary', () => {
    const html = renderToStaticMarkup(<DirectTradeWorkspace />);
    expect(html).toContain('One signed price.');
    expect(html).toContain('Exact Hot38 + strategy/runtime-suffix route manifest');
    expect(html).toContain('2,280-byte checked infrastructure');
    expect(html).toContain('runtime-u32 outcome coordinates');
    expect(html).toContain('Build exact unsigned v0 transaction');
    expect(html).toContain('Sign as transaction payer');
    expect(html).toContain('Download exact packet');
    expect(html).toContain('Submission is deliberately outside this workbench');
    expect(html).not.toContain('Submit signed transaction');
    expect(html).toContain('No chain state has been read.');
    expect(html).not.toContain('sample market');
    expect(html).not.toContain('mock balance');
  });
});
