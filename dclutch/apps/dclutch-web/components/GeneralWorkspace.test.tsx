import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import GeneralWorkspace from './GeneralWorkspace';

describe('General successor workspace presentation', () => {
  it('renders the transaction-complete seven-action plan, status, and receipt workflow', () => {
    const html = renderToStaticMarkup(<GeneralWorkspace />);
    expect(html).toContain('General clearing, from candidate selection through terminal close.');
    expect(html).toContain('No RPC request has been made.');
    expect(html).toContain('Consider, Freeze, Initialize, Collect, Materialize, Distribute, or Close');
    expect(html).toContain('Inspect one chain-derived operator plan');
    expect(html).toContain('Reacquire exact chain status');
    expect(html).toContain('Verify the commit-last execution receipt');
    expect(html).toContain('download the unsigned v0 packet');
    expect(html).toContain('no signing or submission occurs');
    expect(html).not.toContain('settlement transaction remains unavailable');
    expect(html).not.toContain('pending Claims/Custody child wires');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('mock');
  });
});
