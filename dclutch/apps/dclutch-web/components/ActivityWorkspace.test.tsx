import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ActivityWorkspace from './ActivityWorkspace';

describe('Activity route', () => {
  const html = renderToStaticMarkup(<ActivityWorkspace />);

  it('shows wallet activity and its selected history source', () => {
    expect(html).toContain('node history');
    expect(html).toContain('Wallet activity.');
    expect(html).toContain('History is limited to the selected RPC provider’s records.');
  });

  it('derives Position addresses from named Markets exactly like the portfolio', () => {
    expect(html).toContain('include trades and payouts involving its claim accounts');
    expect(html).toContain('Claims program · required to derive Positions');
    expect(html).toContain('Market addresses · one per line');
  });

  it('accepts static-host-safe links without calling them snapshots', () => {
    expect(html).toContain('Owner address · wallet, pasted, or linked');
    expect(html).not.toContain('activity snapshot');
  });

  it('shows the initial state before a history read', () => {
    expect(html).toContain('No signature history has been read.');
    expect(html).toContain('Nothing read yet.');
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
