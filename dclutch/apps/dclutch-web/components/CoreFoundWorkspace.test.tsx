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
    // The payer's sentence moved from a static line to the derived-provenance
    // line, because the page reads it from the wallet now instead of asking.
    expect(html).toContain('Connect a wallet above to fill this, or paste the payer address.');
    expect(html).toContain('Embedded once in the Market-bound RentCredit and immutable afterwards');
    // These two used to pin a byte range the browser had written down in its
    // own words. The coordinates now come from `lib/generated/coreFound.ts`,
    // so the sentence names the RECORD the value is read out of and the
    // number is nowhere in this component to drift.
    expect(html).toContain('Read it out of the Product record above rather than finding it');
    expect(html).toContain('Read it out of the SourceMaterialV3 record above rather than finding it');
    expect(html).toContain('Its dependency graph must terminate; a cycle is refused.');
    // The one place a byte range could come back: nowhere on this page.
    expect(html).not.toContain('bytes 48..80');
    expect(html).not.toContain('bytes 208..240');
  });

  it('is honest that the linked basis is never joined to the graph', () => {
    // The audit's finding: it is the one record of the ten that is
    // rent- and PDA-authenticated and then never decoded.
    expect(html).toContain('none of its bytes are joined to the semantic graph');
  });

  it('derives four of the five records another record already answers', () => {
    // WAS: five addresses carrying "Derivable from that record once this
    // console reads it; today it is typed and then checked" -- named debt,
    // recorded rather than paid. Four are now read out of the parent record
    // that names them, so the console asks for ten addresses instead of
    // fourteen and four fewer places exist for a reader to be silently wrong.
    const stillTyped = html.split('Derivable from that record once this console reads it').length - 1;
    expect(stillTyped, 'only the capacity profile should still be named debt').toBe(1);
    // The one that remains says exactly why, and names the Rust that owes the
    // constant: an unexplained leftover is how four become five again.
    expect(html).toContain('SourceSpecV1 writes that coordinate as a bare number with no named constant');
    // Four fields, each pointing at the parent it is read out of.
    const product = html.split('Read it out of the Product record above').length - 1;
    const source = html.split('Read it out of the SourceMaterialV3 record above').length - 1;
    expect(product).toBe(2);
    expect(source).toBe(2);
  });

  it('offers the derivation as an act, and never pretends it has already run', () => {
    // A field that says "derived" before any chain read is the same claim as
    // a status somebody typed. Until the button runs, the four say what they
    // are and that they are waiting.
    expect(html).toContain('Read the four dependent records');
    expect(html).toContain('No dependent record has been read.');
    // The post-read provenance line, which names a finalized slot, must not
    // appear before a read has produced one.
    expect(html).not.toContain('at finalized slot');
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

describe('the payer is read from the wallet, not asked for', () => {
  const html = renderToStaticMarkup(<CoreFoundWorkspace />);

  it('offers to read the payer from a connected wallet', () => {
    // OPERATOR_FORMS_V1 §0 again: "if a console asks you to paste something and
    // you don't know where it comes from, that's a bug in the console." The
    // payer is the one address on this form whose answer is sitting in the
    // reader's own browser — asking them to transcribe their own public key is
    // the purest case of the rule.
    expect(html).toContain('Connect a wallet to fill the payer');
    expect(html).toContain('Connecting reads your address');
  });

  it('is explicit that connecting still signs nothing here', () => {
    // This console's whole contract is that it exports unsigned bytes and asks
    // for no key. Reading an address is not signing, and the page has to say
    // which one it is doing.
    expect(html).toContain('Nothing is signed on this page');
  });

  it('still lets the refund wallet differ from the payer', () => {
    // It is immutable once embedded in the Market-bound RentCredit, so
    // defaulting it silently to the payer would decide something permanent for
    // a reader who never looked at the field.
    expect(html).toContain('Often the payer, and it does not have to be');
  });
});
