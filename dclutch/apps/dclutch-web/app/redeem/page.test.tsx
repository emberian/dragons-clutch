import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import RedeemPage from './page';

describe('/redeem public route', () => {
  const html = renderToStaticMarkup(<RedeemPage />);

  it('opens the connected-wallet Claims redemption journey', () => {
    expect(html).toContain('Your winning claims');
    // Two headlines are refused BY NAME here, and they failed the same way.
    // "Payout is not open yet" stopped being true the day redemption shipped in
    // this browser. "Nothing has resolved yet" replaced it and stopped being
    // true on 2026-09-02, when a market on this deployment resolved and was
    // paid -- because it is a CENSUS, and a census in a hero has a shelf life.
    // What stands here now is true whatever the markets are doing; which market
    // has answered is read from the chain, per position, further down.
    expect(html).not.toContain('Payout is not open yet');
    expect(html).not.toContain('Nothing has resolved yet');
    expect(html).not.toContain('no market on this deployment has reached an answer');
    expect(html).toContain('Cashed in here');
    expect(html).toContain('Connect your wallet');
    expect(html).toContain('What you can cash in');
  });

  it('does not route readers into the unrelated representation transfer console', () => {
    expect(html).not.toContain('Representation transfer');
    expect(html).not.toContain('Authenticate exact transfer route');
    expect(html).not.toContain('No automatic submission or hidden retry');
  });
});
