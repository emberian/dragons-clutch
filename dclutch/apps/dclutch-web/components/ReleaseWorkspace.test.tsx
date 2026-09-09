import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ReleaseWorkspace from './ReleaseWorkspace';

describe('Registry release presentation', () => {
  it('exposes checked activation and reauthentication with an explicit external boundary', () => {
    const html = renderToStaticMarkup(<ReleaseWorkspace />);
    expect(html).toContain('Release activation.');
    expect(html).toContain('Load release files and prepare activation');
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
    expect(html).toContain('Or paste the file as base64');
    // Steps feed forward and say so: the wallet can fill the payer, while the
    // active deployment supplies the cache until a green plan derives it.
    expect(html).toContain('Connect a wallet in step 03, or paste a public address');
    expect(html).toContain('Activation-cache PDA');
    expect(html).toContain('Sign or export one role packet');
    // The un-gate is shut on a cold render and says so in the contract's terms.
    expect(html).toContain('closed');
    expect(html).toContain('No activation plan prepared.');
    expect(html).toContain('Prepare an activation plan and connect its fee-payer wallet to sign');
    expect(html).not.toContain('This browser observed a chain whose finalized Registry records');
    // Un-gating signing must never introduce a submit path.
    expect(html).toContain('Submit exported transactions through your client');
    expect(html).toContain('Release activation · sign and export');
    expect(html).not.toContain('No wallet connector · no submit path');
    expect(html).toContain('No manifest or chain request has been made.');
    expect(html).toContain('No infrastructure snapshot has been reacquired.');
    expect(html).toContain('Registry program');
    expect(html).toContain('Activate installed program releases');
    expect(html).toContain('Activation takes one transaction per program role');
    expect(html).toContain('Filled from the Devnet deployment');
    expect(html).toContain('Activation compute-unit limit');
    expect(html).toContain('Reauthentication compute-unit limit');
    expect(html).toContain('selects the trading cache slot and current deployed program');
    expect(html).toContain('operator-act');
    expect(html).not.toContain('current devnet Upgrade cycle');
    // RL finding 3: the Registry program is an ownership boundary, never the
    // Core role's program, and the copy must not reintroduce the conflation.
    expect(html).not.toContain('Registry / Core program');
    // Activation admits one role per transaction; five separately signed
    // packets, never one 26-account instruction the chain refuses outright.
    expect(html).toContain('one transaction per program role');
    expect(html).toContain('Connect the fee-payer wallet to sign');
    expect(html).not.toContain('26-account');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('sample state');
  });
});
