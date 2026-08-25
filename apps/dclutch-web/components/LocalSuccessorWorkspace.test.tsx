import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import LocalSuccessorWorkspace from './LocalSuccessorWorkspace';

describe('local successor status presentation', () => {
  it('starts from a fixed finalized RPC profile and states the external boundaries', () => {
    const html = renderToStaticMarkup(<LocalSuccessorWorkspace />);
    expect(html).toContain('The local chain, with its evidence boundaries left intact.');
    expect(html).toContain('http://127.0.0.1:20890/');
    expect(html).toContain('Genesis-prepared records remain labeled as prepared inputs.');
    expect(html).toContain('Read-only localhost profile · no wallet · no signing · no submission');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('mock');
    expect(html).not.toContain('Connect wallet');
  });
});
