import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import RedeemPage from './page';

describe('/redeem public route', () => {
  const html = renderToStaticMarkup(<RedeemPage />);

  it('opens the connected-wallet Claims redemption journey', () => {
    expect(html).toContain('Redeem your winning claims');
    expect(html).toContain('Connect your wallet');
    expect(html).toContain('Your redeemable Positions');
  });

  it('does not route readers into the unrelated representation transfer console', () => {
    expect(html).not.toContain('Representation transfer');
    expect(html).not.toContain('Authenticate exact transfer route');
    expect(html).not.toContain('No automatic submission or hidden retry');
  });
});
