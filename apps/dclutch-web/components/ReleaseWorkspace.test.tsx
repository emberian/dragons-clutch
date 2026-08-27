import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ReleaseWorkspace from './ReleaseWorkspace';

describe('Registry release presentation', () => {
  it('exposes checked activation and reauthentication with an explicit external boundary', () => {
    const html = renderToStaticMarkup(<ReleaseWorkspace />);
    expect(html).toContain('Make executable authority inspectable.');
    expect(html).toContain('Activate a checked five-role release');
    expect(html).toContain('Reauthenticate one active role');
    expect(html).toContain('Inspect immutable protocol infrastructure');
    expect(html).toContain('1,592-byte checked multiprogram');
    expect(html).toContain('2,280-byte checked infrastructure manifest');
    expect(html).toContain('No wallet connector · no submit path');
    expect(html).toContain('No manifest or chain request has been made.');
    expect(html).toContain('No infrastructure snapshot has been reacquired.');
    expect(html).toContain('Registry program');
    // RL finding 3: the Registry program is an ownership boundary, never the
    // Core role's program, and the copy must not reintroduce the conflation.
    expect(html).not.toContain('Registry / Core program');
    // Activation admits one role per transaction; five separately signed
    // packets, never one 26-account instruction the chain refuses outright.
    expect(html).toContain('one exact ten-account action per role');
    expect(html).toContain('five separately signed packets, not one');
    expect(html).not.toContain('26-account');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('sample state');
  });
});
