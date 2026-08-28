import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketWorkbench from './MarketWorkbench';

describe('market lifecycle workbench', () => {
  it('renders creation as unavailable until exact chain authority is selected', () => {
    const html = renderToStaticMarkup(<MarketWorkbench />);
    expect(html).toContain('Lifecycle readiness');
    expect(html).toContain('read-only lifecycle readiness map');
    expect(html).toContain('does not create, trade, resolve, or redeem');
    expect(html).toContain('Author &amp; fund');
    expect(html).toContain('Compile runtime-width Product result domain');
    expect(html).toContain('Prepare the current founding campaign');
    expect(html).toContain('Reacquire the selected role programs');
    expect(html).toContain('Transaction unavailable');
    expect(html).not.toContain('Illustrative');
    expect(html).not.toContain('USDC');
  });

  it('opens the trade stage without synthetic pool or order state', () => {
    const html = renderToStaticMarkup(<MarketWorkbench initialStage="trade" />);
    expect(html).toContain('Trade &amp; provide liquidity');
    expect(html).toContain('Inspect a Direct route and its arithmetic');
    expect(html).toContain('Transaction unavailable');
    expect(html).toContain('Inventory-bounded immediate trade');
    expect(html).not.toContain('25,000');
    expect(html).not.toContain('Awaiting local chain');
  });
});
