import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketWorkbench, { workbenchRefusalFieldV1 } from './MarketWorkbench';

describe('market lifecycle workbench', () => {
  it('renders creation as unavailable until exact chain authority is selected', () => {
    const html = renderToStaticMarkup(<MarketWorkbench />);
    expect(html).toContain('Lifecycle readiness');
    expect(html).toContain('read-only map of where a market has got to');
    expect(html).toContain('does not create, trade, resolve, or redeem');
    expect(html).toContain('Author &amp; fund');
    expect(html).toContain('Compile an admitted degree-2/3 Product graph');
    expect(html).toContain('Found a current Market and first participant');
    expect(html).toContain('Admit another participant');
    expect(html).toContain('Reacquire the selected role programs');
    expect(html).toContain('Transaction unavailable');
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
    expect(html).toContain('Take and execute a Direct offer');
    expect(html).toContain('Wallet signs one detached message');
    expect(html).toContain('Transaction unavailable');
    expect(html).toContain('Inventory-bounded immediate trade');
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
