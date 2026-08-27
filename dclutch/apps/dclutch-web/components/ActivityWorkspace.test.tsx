import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ActivityWorkspace from './ActivityWorkspace';

describe('Activity route', () => {
  const html = renderToStaticMarkup(<ActivityWorkspace />);

  it('names its provenance as the node history, never a protocol index', () => {
    expect(html).toContain('node history · not a protocol index');
    expect(html).toContain('the RPC node&#x27;s own per-address signature index');
    expect(html).toContain('Not consensus state and not a protocol fact');
    expect(html).toContain('two nodes can remember different histories');
  });

  it('derives Position addresses from named Markets exactly like the portfolio', () => {
    expect(html).toContain('the same derivation the portfolio uses');
    expect(html).toContain('Claims program · required to derive Positions');
    expect(html).toContain('Market addresses · one per line');
  });

  it('keeps the honest empty state instead of a placeholder feed', () => {
    expect(html).toContain('No signature history has been read.');
    expect(html).toContain('this surface stays empty rather than inventing an activity feed');
  });

  it('states that an empty node answer is the node speaking, not the chain', () => {
    expect(html).toContain('honestly answers &quot;nothing&quot; for every address');
  });

  it('makes the browser wallet optional and identity-only', () => {
    expect(html).toContain('Owner address · wallet or pasted');
    expect(html).toContain('Connecting reads a public address only');
  });

  it('presents lamports and refusals, never market-data metrics', () => {
    const remainder = html.replace(/<nav>[\s\S]*?<\/nav>/, '');
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'TVL', 'APR', 'APY', '$', 'P&L']) {
      expect(remainder).not.toContain(forbidden);
    }
    expect(remainder).toContain('lamport');
  });
});
