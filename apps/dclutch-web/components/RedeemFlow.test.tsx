import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { WALLET_CONNECTION_IDLE_V1 } from '@/lib/walletStandard';

import RedeemFlow from './RedeemFlow';
import { type WalletDirectoryHandleV1 } from './WalletDirectory';

const directory: WalletDirectoryHandleV1 = Object.freeze({
  state: WALLET_CONNECTION_IDLE_V1,
  wallets: Object.freeze([]),
  refusals: Object.freeze([]),
  address: null,
  connectedWalletId: null,
  async connect() { return Object.freeze({ status: 'refused' as const, reason: 'not used by this component test' }); },
  forget() { /* no connected identity */ },
  handoff() { throw new Error('no connected identity'); },
});

describe('wallet redemption flow', () => {
  const html = renderToStaticMarkup(<RedeemFlow
    endpoint="https://api.devnet.solana.com"
    marketAddress="gBxS1f6uyyGPuW5MzGBukidSb71jdsCb5fZaoSzULE5"
    positionAddress="k7FaK87WH8sR2tHfMX7hGivxiCrcHNTGkZLH5TbtQsS"
    claimIndex={1}
    availableQuantity="2"
    claimsProgramId="4vJ9JU1bJJE96FWSJKvHsmmF7ujPKAy5SKpjXkLc6R1Q"
    custodyProgramId="8qbHbw2BbbTHBW1sbeqakYXV5ZZGczXJG2ajNeN3WFe"
    registryProgramId="CktRuQ2mttgRG9XJNgMHDqZqQmM4j5EJQ3R2A4j3ZxY"
    directory={directory}
  />);

  it('states the complete receipt-and-resource verification boundary', () => {
    expect(html).toContain('two checked steps');
    expect(html).toContain('returned receipt');
    expect(html).toContain('your changed claim balance');
    expect(html).toContain('both changed token balances');
  });

  it('does not claim a payout before a connected wallet and checked plan exist', () => {
    expect(html).toContain('Check redemption');
    expect(html).not.toContain('Payout verified');
    expect(html).not.toContain('Redeem 2 winning atoms');
    expect(html).not.toContain('payout itself, is not wallet-ready');
  });

  it('explains crash recovery without treating browser storage as authority', () => {
    expect(html).toContain('saves the signed transaction id');
    expect(html).toContain('Reloading resumes only that exact signature');
    expect(html).toContain('it never submits it again');
    expect(html).toContain('Browser data is an untrusted projection');
  });

  it('exposes the Rust artifact handoff without claiming browser authorship', () => {
    expect(html).toContain('Rust payout plan file');
    expect(html).toContain('This browser never creates or completes a payout plan');
    expect(html).toContain('the checked Program and ProgramData generation');
    expect(html).toContain('remain disabled until the payment record above is verified');
  });
});
