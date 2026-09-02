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
    coreProgramId="6JsGGCyDXfC7HmVpBZKUMkYCDnJTFhqBTPQCJgApnpDe"
    resolutionProgramId="9V1s7wcYqGZHtGx5jrCWWiVWMqDPFKSGZ2Hk8DYNmuKk"
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

  it('exposes the Rust artifact handoff and now completes it here', () => {
    // WAS: this pinned "This browser never creates or completes a payout
    // plan". That was honest while the derivation lived only in a binary. It
    // was extracted verbatim, compiled, and given its snapshot, so the
    // assertion moves with the behaviour rather than the sentence being
    // quietly deleted from under it.
    expect(html).toContain('Rust payout plan file');
    expect(html).not.toContain('This browser never creates or completes a payout plan');
    expect(html).toContain('This browser builds the whole payout itself');
    expect(html).toContain('the checked Program and ProgramData generation');
    expect(html).toContain('remain disabled until the payment record above is verified');
  });
});

describe('the browser derives the payout plan instead of only importing one', () => {
  const html = renderToStaticMarkup(<RedeemFlow
    endpoint="https://api.devnet.solana.com"
    marketAddress="gBxS1f6uyyGPuW5MzGBukidSb71jdsCb5fZaoSzULE5"
    positionAddress="k7FaK87WH8sR2tHfMX7hGivxiCrcHNTGkZLH5TbtQsS"
    claimIndex={1}
    availableQuantity="2"
    claimsProgramId="4vJ9JU1bJJE96FWSJKvHsmmF7ujPKAy5SKpjXkLc6R1Q"
    custodyProgramId="8qbHbw2BbbTHBW1sbeqakYXV5ZZGczXJG2ajNeN3WFe"
    registryProgramId="CktRuQ2mttgRG9XJNgMHDqZqQmM4j5EJQ3R2A4j3ZxY"
    coreProgramId="6JsGGCyDXfC7HmVpBZKUMkYCDnJTFhqBTPQCJgApnpDe"
    resolutionProgramId="9V1s7wcYqGZHtGx5jrCWWiVWMqDPFKSGZ2Hk8DYNmuKk"
    directory={directory}
  />);

  it('no longer claims the browser cannot create a payout plan', () => {
    // The sentence this unit was aimed at. It was true until the derivation
    // was extracted, compiled, and given its snapshot.
    expect(html).not.toContain('This browser never creates or completes a payout plan');
  });

  it('names the compiled derivation as the authority, and what it reads', () => {
    expect(html).toContain('compiled Rust');
    expect(html).toContain('finalized');
  });

  it('no longer sends the reader to the Rust producer for the payout input', () => {
    // WAS: this pinned that the page still names
    // `wallet-terminal-payout-input`, the CLI command a reader had to run to
    // get stage one's artifact. That was honest while stage one lived only in
    // a binary and while its address book could only come from a sealed
    // campaign report. The three phases were extracted, the book is DERIVED
    // from chain, and the sentence goes with the behaviour rather than being
    // quietly left standing.
    expect(html).not.toContain('wallet-terminal-payout-input');
    expect(html).not.toContain('successor bootstrap tool');
  });

  it('states that an empty box means the browser derives the input itself', () => {
    expect(html).toContain('empty means derive it here');
    expect(html).toContain('recompiles the four composition records that nothing on chain points at');
    expect(html).toContain('no operator document is needed at any step');
  });

  it('offers the destination as an override, not as a question', () => {
    // WAS: this pinned that the page ASKS for the recipient token account,
    // which was honest while nothing derived one. The default is now the
    // standard associated token account, derived under the program that
    // declares it, so the field is an override and the page says so.
    expect(html).toContain('empty means your associated token account');
    expect(html).toContain('Leave both boxes below empty and it needs nothing from you at all');
    expect(html).toContain('associated token account');
  });
});
