import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import EconomicWorkspace from './EconomicWorkspace';

describe('physical economic successor presentation', () => {
  it('exposes only finalized, unsigned, non-synthetic workflows before interaction', () => {
    const html = renderToStaticMarkup(<EconomicWorkspace />);
    expect(html).toContain('Conservative claims, tied to real collateral.');
    expect(html).toContain('Connect &amp; discover projections');
    expect(html).toContain('No RPC request has been made.');
    expect(html).toContain('Operate one founded projection');
    expect(html).toContain('Found one preallocated projection');
    expect(html).toContain('No exact preallocated vacant projection is available.');
    expect(html).toContain('No founded economic projection is available.');
    expect(html).toContain('never signs or submits');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('mock');
  });
});
