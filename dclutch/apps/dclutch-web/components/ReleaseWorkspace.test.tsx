import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ReleaseWorkspace from './ReleaseWorkspace';

describe('Registry release presentation', () => {
  it('exposes checked activation and reauthentication with an explicit external boundary', () => {
    const html = renderToStaticMarkup(<ReleaseWorkspace />);
    expect(html).toContain('Release activation.');
    expect(html).toContain('Load the checked build and derive the activation walk');
    expect(html).toContain('Reauthenticate one active role');
    expect(html).toContain('Inspect immutable protocol infrastructure');
    // Every artifact input names its producer and the file, offers a file
    // drop, and keeps paste as the labeled offline fallback (charter: no
    // paste box without provenance).
    expect(html).toContain('multiprogram.checked');
    expect(html).toContain('exactly 1,592 bytes');
    expect(html).toContain('evidence/core/checked.bin');
    expect(html).toContain('infrastructure.checked');
    expect(html).toContain('exactly 2,360 bytes');
    expect(html).toContain('Drop the file here, or click to choose it');
    expect(html).toContain('Offline fallback · paste the same file as base64');
    // Steps feed forward and say so: the wallet fills the payer, the plan
    // fills the cache, and the signing step names the plan it uses.
    expect(html).toContain('Connect a wallet in step 03 to fill this');
    expect(html).toContain('build a plan in step 02 to fill this');
    expect(html).toContain('Sign the walk with a browser wallet');
    // The un-gate is shut on a cold render and says so in the contract's terms.
    expect(html).toContain('closed');
    expect(html).toContain('No activation plan is green against this chain.');
    expect(html).toContain('Signing stays closed. It opens only when one activation plan is green against this chain');
    expect(html).not.toContain('This browser observed a chain whose finalized Registry records');
    // Un-gating signing must never introduce a submit path.
    expect(html).toContain('There is no submit path here, signed or unsigned.');
    expect(html).toContain('Wallet signing only behind a green plan · no submit path');
    expect(html).not.toContain('No wallet connector · no submit path');
    expect(html).toContain('No manifest or chain request has been made.');
    expect(html).toContain('No infrastructure snapshot has been reacquired.');
    expect(html).toContain('Registry program');
    // RL finding 3: the Registry program is an ownership boundary, never the
    // Core role's program, and the copy must not reintroduce the conflation.
    expect(html).not.toContain('Registry / Core program');
    // Activation admits one role per transaction; five separately signed
    // packets, never one 26-account instruction the chain refuses outright.
    expect(html).toContain('one ten-account action');
    expect(html).toContain('five separate packets, not one');
    expect(html).not.toContain('26-account');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('sample state');
  });
});
