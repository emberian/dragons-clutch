import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketWorkbench from './MarketWorkbench';

describe('market lifecycle workbench', () => {
  it('renders creation as unavailable until exact chain authority is selected', () => {
    const html = renderToStaticMarkup(<MarketWorkbench />);
    expect(html).toContain('From exact terms');
    expect(html).toContain('Author &amp; fund');
    expect(html).toContain('Compile Product V2 result domain');
    expect(html).toContain('Found common Core Market');
    expect(html).toContain('Reacquire the execution surface first.');
    expect(html).toContain('Transaction unavailable');
    expect(html).not.toContain('Illustrative');
    expect(html).not.toContain('USDC');
  });

  it('opens the trade stage without synthetic pool or order state', () => {
    const html = renderToStaticMarkup(<MarketWorkbench initialStage="trade" />);
    expect(html).toContain('Trade &amp; provide liquidity');
    expect(html).toContain('Fill inline or registered intents');
    expect(html).toContain('Inventory-bounded immediate trade');
    expect(html).not.toContain('25,000');
    expect(html).not.toContain('Awaiting local chain');
  });
});
