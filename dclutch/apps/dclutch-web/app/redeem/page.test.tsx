import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import RedeemPage from './page';

describe('/redeem public route', () => {
  const html = renderToStaticMarkup(<RedeemPage />);

  it('opens the connected-wallet Claims redemption journey', () => {
    expect(html).toContain('Your winning claims');
    // The old headline said "Payout is not open yet", which stopped being true
    // the day redemption shipped in this browser. Refused by name so it cannot
    // come back: what is true is a fact about the markets, not about the page.
    expect(html).not.toContain('Payout is not open yet');
    expect(html).toContain('Nothing has resolved yet');
    expect(html).toContain('Connect your wallet');
    expect(html).toContain('What you can cash in');
  });

  it('does not route readers into the unrelated representation transfer console', () => {
    expect(html).not.toContain('Representation transfer');
    expect(html).not.toContain('Authenticate exact transfer route');
    expect(html).not.toContain('No automatic submission or hidden retry');
  });
});
