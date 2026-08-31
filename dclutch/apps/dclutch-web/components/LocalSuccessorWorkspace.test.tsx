import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import LocalSuccessorWorkspace, { transactionTitle } from './LocalSuccessorWorkspace';
import checkpoint from '../fixtures/successor-checkpoint.json';

describe('local successor status presentation', () => {
  it('starts from a fixed finalized RPC profile and states the external boundaries', () => {
    const html = renderToStaticMarkup(<LocalSuccessorWorkspace />);
    expect(html).toContain('The local chain.');
    expect(html).toContain('http://127.0.0.1:20890/');
    expect(html).toContain('compared byte for byte against the hash-pinned checkpoint');
    expect(html).toContain('Read-only localhost profile · no wallet · no signing · no submission');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('mock');
    expect(html).not.toContain('Connect wallet');
  });

  it('does not narrate itself: no sentence describes the page, the run, or the runner', () => {
    const html = renderToStaticMarkup(<LocalSuccessorWorkspace />);
    for (const narration of ['This page ', 'The status page', 'is shown as such', 'does not pretend', 'untrusted projection', 'verbatim from']) {
      expect(html).not.toContain(narration);
    }
  });

  it('gives every checkpoint transaction a title in words, never the runner identifier', () => {
    const labels = (checkpoint as Readonly<{ expected_transactions: ReadonlyArray<Readonly<{ label: string }>> }>).expected_transactions;
    expect(labels.length).toBeGreaterThan(0);
    for (const { label } of labels) {
      const title = transactionTitle(label);
      expect(title).not.toBe(label);
      expect(title).not.toContain('_');
      expect(title[0]).toBe(title[0].toUpperCase());
    }
  });

  it('humanizes an identifier the checkpoint adds later rather than printing it raw', () => {
    expect(transactionTitle('resolution_some_future_route')).toBe('Resolution some future route');
  });
});
