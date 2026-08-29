import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import example from '@/fixtures/simulator-status.example.json';
import { parseSimulatorStatusV1 } from '@/lib/simulatorStatus';

import PulseWorkspace from './PulseWorkspace';

// The flip: the same surface, handed a decoded artifact through the test
// seam. The fixture is a real-shaped status document, so the props contract
// cannot drift into agreeing only with a hand-built object.

const status = parseSimulatorStatusV1(example);

describe('the pulse surface, with a published run', () => {
  const html = renderToStaticMarkup(<PulseWorkspace preloaded={{ kind: 'loaded', status }} />);

  it('shows the counts the artifact carries, as exact values', () => {
    expect(html).toContain('>12</strong>');
    expect(html).toContain('>24</strong>');
    expect(html).toContain('>2</strong>');
    expect(html).not.toContain('>—</strong>');
  });

  it('names the cluster honestly — a local rehearsal is not devnet', () => {
    expect(html).toContain('a local rehearsal validator, not the public devnet');
  });

  it('shows the conservation verdict with its timestamp', () => {
    expect(html).toContain('conserved');
    expect(html).toContain('Checked at 2026-08-29T21:40:09+00:00');
  });

  it('renders every wallet with its exact balance, and an unread balance as unread', () => {
    expect(html).toContain('49442160 lamports');
    expect(html).toContain('balance unread this cycle');
  });

  it('links the market the trades name', () => {
    expect(html).toContain('/market?address=GtmpRvSL9y6RpqMth73VSdb9h1XRe7zqQZkhJkfgxKrA');
  });
});

describe('the pulse surface, after a halt', () => {
  const haltedRaw = JSON.parse(JSON.stringify(example)) as Record<string, unknown>;
  haltedRaw.halted = true;
  haltedRaw.halt_reason = 'conservation violated at cycle 12';
  (haltedRaw.last_reconciliation as Record<string, unknown>).ok = false;
  const halted = parseSimulatorStatusV1(haltedRaw);
  const html = renderToStaticMarkup(<PulseWorkspace preloaded={{ kind: 'loaded', status: halted }} />);

  it('leads with the halt instead of hiding it', () => {
    expect(html).toContain('Halted — loudly, on purpose');
    expect(html).toContain('The simulator halted itself: conservation violated at cycle 12');
  });

  it('shows the failed check as failed', () => {
    expect(html).toContain('violated');
    expect(html).toContain('status-chip fail');
  });
});
