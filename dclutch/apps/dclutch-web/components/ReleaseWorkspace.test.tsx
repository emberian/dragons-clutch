import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ReleaseWorkspace from './ReleaseWorkspace';

describe('Registry release presentation', () => {
  it('exposes checked activation and reauthentication with an explicit external boundary', () => {
    const html = renderToStaticMarkup(<ReleaseWorkspace />);
    expect(html).toContain('Make executable authority inspectable.');
    expect(html).toContain('Activate a checked five-role release');
    expect(html).toContain('Reauthenticate one active role');
    expect(html).toContain('1,592-byte checked multiprogram');
    expect(html).toContain('No wallet connector · no submit path');
    expect(html).toContain('No manifest or chain request has been made.');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('sample state');
  });
});
