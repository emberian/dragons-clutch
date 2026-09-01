import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketWorkbench, { workbenchRefusalFieldV1 } from './MarketWorkbench';

describe('market lifecycle workbench', () => {
  it('says what each authoring act needs before it can begin, and never greys a control', () => {
    const html = renderToStaticMarkup(<MarketWorkbench />);
    expect(html).toContain('Lifecycle readiness');
    expect(html).toContain('read-only map of where a market has got to');
    expect(html).toContain('does not create, trade, resolve, or redeem');
    expect(html).toContain('Author &amp; fund');
    expect(html).toContain('Compile a Product record and its admission request');
    expect(html).toContain('Found a Market and admit its first participant');
    expect(html).toContain('Admit another participant');
    expect(html).toContain('Read the selected programs, and any Market you name, at one finalized floor first');
    expect(html).toContain('Where it runs');
    expect(html).toContain('What it promises');
    // No disabled control anywhere: every card that cannot be opened links to
    // the page that answers why, which is always reachable.
    expect(html).not.toContain('disabled');
    expect(html.toLowerCase()).not.toContain('transaction unavailable');
    expect(html.toLowerCase()).not.toContain('greyed');
    expect(html).toContain('Devnet supplies the six program addresses');
    expect(html).toContain('Program overrides · 6 filled from Devnet');
    expect(html).toContain('Filled from the Devnet deployment');
    expect(html).toContain('Optional state coordinates');
    expect(html).not.toContain('Illustrative');
    expect(html).not.toContain('USDC');
  });

  it('routes single-field read refusals without guessing at cross-field joins', () => {
    expect(workbenchRefusalFieldV1('Refused: trading program is not executable')).toBe('trading');
    expect(workbenchRefusalFieldV1('Refused: Realm is not owned by the selected Core program')).toBeNull();
    expect(workbenchRefusalFieldV1('Refused: Invalid URL')).toBe('endpoint');
    expect(workbenchRefusalFieldV1('Refused: multiprogram roles must have distinct executable program identities')).toBeNull();
    expect(workbenchRefusalFieldV1('Refused: Realm and Market must have distinct state identities')).toBeNull();
    expect(workbenchRefusalFieldV1('Refused: Realm or Market aliases an executable program role')).toBeNull();
  });

  it('opens the trade stage without synthetic pool or order state', () => {
    const html = renderToStaticMarkup(<MarketWorkbench initialStage="trade" />);
    expect(html).toContain('Trade &amp; provide liquidity');
    expect(html).toContain('Author a portable sell offer');
    expect(html).toContain('Take and execute a signed offer');
    expect(html).toContain('This browser \u00b7 one detached message signature');
    expect(html).toContain('Take an inventory-bounded immediate trade');
    // An act with no venue names its wall here too, in the same words and
    // with the same citation the census uses.
    expect(html).toContain('Known wall');
    expect(html).toContain('crates/dclutch-dealer-scenario-kernel');
    expect(html).not.toContain('25,000');
    expect(html).not.toContain('Awaiting local chain');
  });

  it('names the resolution route honestly and keeps it read-only', () => {
    const html = renderToStaticMarkup(<MarketWorkbench surface="resolution" initialStage="resolve" />);
    expect(html).toContain('Resolution readiness');
    expect(html).toContain('before a resolution route can begin preflight');
    expect(html).toContain('opens at Resolve &amp; settle');
    expect(html).toContain('it cannot resolve a market');
    expect(html).not.toContain('<strong>Lifecycle readiness</strong>');
  });
});
