import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import GeneralWorkspace from './GeneralWorkspace';

describe('General successor workspace presentation', () => {
  it('renders release-authenticated outer routes and the unavailable child boundary before interaction', () => {
    const html = renderToStaticMarkup(<GeneralWorkspace />);
    expect(html).toContain('General clearing, with every physical boundary exposed.');
    expect(html).toContain('No RPC request has been made.');
    expect(html).toContain('release-authenticated unsigned outer transactions');
    expect(html).toContain('Candidates and candidate-wide quote aggregates');
    expect(html).toContain('Policy, verification, certificate, and settlement state');
    expect(html).toContain('Build an unsigned, release-authenticated action');
    expect(html).toContain('pending Claims/Custody child wires');
    expect(html).toContain('No exact candidate header is available.');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('mock');
  });
});
