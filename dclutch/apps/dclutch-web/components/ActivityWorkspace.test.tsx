import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ActivityWorkspace from './ActivityWorkspace';

describe('Activity route', () => {
  const html = renderToStaticMarkup(<ActivityWorkspace />);

  it('names its provenance as the node history, never a protocol index', () => {
    expect(html).toContain('node history');
    expect(html).toContain('As the node remembers it');
    // Renegotiated 2026-08-31: "Not consensus state and not a protocol fact"
    // plus the paragraph on how the node's per-address signature index works
    // are deleted. The one thing a reader acts on is that two nodes can
    // disagree, and that is what the aside says now.
    expect(html).toContain('Two nodes can remember different histories.');
  });

  it('derives Position addresses from named Markets exactly like the portfolio', () => {
    expect(html).toContain('the same derivation the portfolio uses');
    expect(html).toContain('Claims program · required to derive Positions');
    expect(html).toContain('Market addresses · one per line');
  });

  it('accepts static-host-safe links without calling them snapshots', () => {
    expect(html).toContain('Owner address · wallet, pasted, or linked');
    expect(html).not.toContain('activity snapshot');
  });

  it('keeps the honest empty state instead of a placeholder feed', () => {
    expect(html).toContain('No signature history has been read.');
    expect(html).toContain('Nothing read yet.');
  });

  it('states that an empty node answer is the node speaking, not the chain', () => {
    expect(html).toContain('answers &quot;nothing&quot; for every address');
  });

  it('makes the browser wallet optional and identity-only', () => {
    expect(html).toContain('Owner address · wallet, pasted, or linked');
    // Renegotiated 2026-08-31: the standing "connecting reads a public address
    // only" paragraph is deleted from the wallet panel everywhere it appeared.
    expect(html).not.toContain('Connecting reads a public address only');
  });

  it('presents lamports and refusals, never market-data metrics', () => {
    const remainder = html.replace(/<nav>[\s\S]*?<\/nav>/, '');
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'TVL', 'APR', 'APY', '$', 'P&L']) {
      expect(remainder).not.toContain(forbidden);
    }
    expect(remainder).toContain('lamport');
  });
});
