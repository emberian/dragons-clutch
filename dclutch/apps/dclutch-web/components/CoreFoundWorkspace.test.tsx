import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import CoreFoundWorkspace from './CoreFoundWorkspace';

describe('Core Found workspace', () => {
  it('leads with the current journaled campaign and keeps legacy Found37 diagnostic-only', () => {
    const html = renderToStaticMarkup(<CoreFoundWorkspace />);
    expect(html).toContain('Found a market');
    expect(html).toContain('Found one current devnet Market');
    expect(html).toContain('dclutch-devnet-market-participant-operation-v1');
    expect(html).toContain('Preview first; execute explicitly');
    expect(html).toContain('Market + first participant + session');
    expect(html).toContain('Rerun the same operation and journal');
    expect(html).toContain('Show preview and execute commands');
    expect(html).toContain('found-operation');
    expect(html).toContain('found-journal');
    expect(html).toContain('--session-out');
    expect(html).toContain('--execute');
    expect(html).toContain('Open the legacy Found37 packet inspector');
    expect(html).toContain('cannot perform the current atomic opening');
    expect(html).toContain('Construct unsigned lifecycle + Found transactions');
    expect(html).toContain('Immutable rent refund wallet');
    expect(html).toContain('No transaction has been constructed');
    expect(html).toContain('No signing or submission here.');
    expect(html).toContain('Product Runtime V2 raw');
    expect(html).toContain('SourceMaterialV3 raw');
    expect(html).toContain('Linked basis raw');
    expect(html).not.toContain('Execution release set raw');
    expect(html).not.toContain('Illustrative');
    expect(html).not.toContain('sample balance');
    expect(html.indexOf('Found one current devnet Market')).toBeLessThan(html.indexOf('Open the legacy Found37 packet inspector'));
  });
});

describe('Core Found workspace: what each field is, and where its value comes from', () => {
  const html = renderToStaticMarkup(<CoreFoundWorkspace />);

  it('gives every address a sentence saying where it comes from', () => {
    // OPERATOR_FORMS_V1 §0 -- `ArtifactInput`'s rule, generalised. Fourteen
    // addresses used to arrive with a label and nothing else.
    expect(html).toContain('The wallet that funds both packets and signs them elsewhere.');
    expect(html).toContain('Embedded once in the Market-bound RentCredit and immutable afterwards');
    expect(html).toContain('at the Product record’s bytes 48..80');
    expect(html).toContain('at its bytes 208..240');
    expect(html).toContain('Its dependency graph must terminate; a cycle is refused.');
  });

  it('is honest that the linked basis is never joined to the graph', () => {
    // The audit's finding: it is the one record of the ten that is
    // rent- and PDA-authenticated and then never decoded.
    expect(html).toContain('none of its bytes are joined to the semantic graph');
  });

  it('names the five records another record already answers, as named debt', () => {
    // §3.2: derivable, but only with a chain read this console does not make
    // before submit. Recorded rather than smuggled in under a forms pass.
    const derivable = html.split('Derivable from that record once this console reads it').length - 1;
    expect(derivable).toBe(5);
  });

  it('fills the deployment fields and says so, rather than filling them silently', () => {
    // These two were already pre-filled before this pass -- with no line
    // anywhere saying they had been.
    expect(html).toContain('<strong>Filled from the deployment this browser is pointed at.</strong>');
    expect(html).toContain('Both arrive filled from the cluster picked in the header.');
  });

  it('groups the sixteen fields under the four questions they answer', () => {
    expect(html).toContain('<legend>The chain this founds against</legend>');
    expect(html).toContain('<legend>Who pays, and who is refunded</legend>');
    expect(html).toContain('<legend>The deployment this founds against</legend>');
    expect(html).toContain('<legend>The ten finalized records this market is built from</legend>');
  });

  it('keeps all fourteen addresses reachable — grouping, not simplification', () => {
    for (const label of [
      'Payer', 'Immutable rent refund wallet', 'Registry program', 'Release activation cache',
      'Realm raw record', 'Product Runtime V2 raw', 'Result domain raw', 'Portfolio raw',
      'Linked basis raw', 'SourceMaterialV3 raw', 'Source spec raw',
      'Source capacity profile raw', 'Manipulation floor raw', 'Capability manifest raw',
    ]) {
      expect(html).toContain(label);
    }
    expect(html).toContain('Finalized RPC endpoint');
    expect(html).toContain('Market generation');
  });

  it('no longer shows an empty form a single shared refusal slot', () => {
    // Before: one `aria-live` line served all sixteen fields. The slot still
    // exists for refusals nothing owns; what changed is that a routed refusal
    // renders at its field instead.
    expect(html).toContain('No transaction has been constructed');
    expect(html).not.toContain('role="alert"');
  });
});
