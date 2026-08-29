import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import PulseWorkspace from './PulseWorkspace';

// The shipped default: no artifact published. renderToStaticMarkup never runs
// effects, so this is exactly the prerendered HTML every visitor gets first —
// and the state a static host with no published pulse never leaves.

describe('the pulse surface, with nothing published', () => {
  const html = renderToStaticMarkup(<PulseWorkspace preloaded={{ kind: 'absent' }} />);

  it('says no simulator is running, in as many words', () => {
    expect(html).toContain('No simulator running');
    expect(html).toContain('nothing below is a zero');
  });

  it('shows dashes for every count and no zero anywhere', () => {
    expect(html.split('>—</strong>').length - 1).toBe(3);
    expect(html).not.toContain('>0</strong>');
  });

  it('never stands in sample data for the missing artifact', () => {
    for (const forbidden of ['mock', 'illustrative', 'Illustrative', 'sample data', 'placeholder', 'TPS', 'volume', 'Volume', '$', 'APY']) {
      expect(html).not.toContain(forbidden);
    }
  });

  it('explains what the robot is in plain terms', () => {
    expect(html).toContain('It sends the same transactions you would send');
    expect(html).toContain('it stops loudly and this page shows the stop');
  });

  it('keeps the ledger-check spot honest instead of green', () => {
    expect(html).toContain('No check has been read.');
    expect(html).not.toContain('conserved');
  });
});

describe('the pulse surface, before the read settles', () => {
  const html = renderToStaticMarkup(<PulseWorkspace />);

  it('says it is looking, and claims nothing else', () => {
    expect(html).toContain('Looking for a published pulse');
    expect(html.split('>—</strong>').length - 1).toBe(3);
    expect(html).not.toContain('>0</strong>');
  });
});
