import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import SignStep from './SignStep';
import { type WalletPreparationState } from '@/lib/tradeFlowMachine';
import { assignRefusalV1 } from '@/lib/tradeFlowRefusals';

const noop = () => {};
const PAYER = 'Payer1111111111111111111111111111111111111';
const BUYER = 'Buyer1111111111111111111111111111111111111';

const walletDirectory = (address: string | null) => ({
  state: { kind: 'idle', message: '' }, wallets: [], refusals: [], address,
  connectedWalletId: null, connect: async () => ({ status: 'refused', reason: 'unused' }),
  forget: noop, handoff: () => ({}),
}) as never;

const render = (
  walletPreparation: WalletPreparationState,
  overrides: Partial<Parameters<typeof SignStep>[0]> = {},
) => renderToStaticMarkup(<SignStep
  walletPreparation={walletPreparation}
  previewReady
  routeText=""
  publishedRoute={null}
  onRouteText={noop}
  onPrepare={noop}
  onSignPacket={noop}
  wallets={walletDirectory(null)}
  onWalletConnected={noop}
  refusal={null}
  {...overrides}
/>);

const PAYER_REQUIRED_V1: WalletPreparationState = Object.freeze({
  kind: 'payer-wallet-required' as const,
  preparation: {
    status: 'payer-wallet-required',
    payer: PAYER,
    reason: `Connect the authenticated route payer ${PAYER}`,
    binding: {
      taker: { owner: BUYER }, routeObservedSlot: '490712003',
      blockhashObservedSlot: 490712100n, lastValidBlockHeight: 4001n,
    },
    transactionPlan: {
      wireBytes: new Uint8Array(1204), loadedAddresses: 37,
      transaction: { message: { serialize: () => Uint8Array.from([1, 2, 3]), addressTableLookups: [{ accountKey: { toBase58: () => 'Lut111' } }] } },
    },
  } as never,
  takerTicket: '{"kind":"dclutch/direct-intent-ticket/v1"}',
  takerBefore: {} as never,
});

describe('step 6, the two signatures', () => {
  const idle = render({ kind: 'idle' });

  it('renders two signature rows and never one, before either has happened', () => {
    expect(idle).toContain('Your intent');
    expect(idle).toContain('The transaction');
    expect(idle).toContain('trade terms');
    expect(idle).toContain('sign transaction');
    expect(idle.split('signature-open').length - 1).toBe(2);
    expect(idle).not.toContain('signature-done');
  });

  /**
   * Row A's success state must say what the reader now HAS. They hold a real,
   * portable, signed ticket -- and if they close the tab here, that is still
   * true and nothing executed.
   */
  it('says what the first signature produced, on every path that reaches it', () => {
    const signed = render(PAYER_REQUIRED_V1);
    expect(signed).toContain('Offer signed. Ready to prepare the transaction.');
    expect(signed.split('signature-done').length - 1).toBe(1);
  });

  it('does not offer to sign again once the intent is signed', () => {
    expect(idle).toContain('Sign offer and prepare trade');
    expect(render(PAYER_REQUIRED_V1)).not.toContain('Sign offer and prepare trade');
  });

  it('keeps buyer and payer distinct, and enables the packet signature only for the route payer', () => {
    const waiting = render(PAYER_REQUIRED_V1);
    expect(waiting).toContain(`Buyer ${BUYER} has signed the offer`);
    expect(waiting).toContain(`route payer ${PAYER}`);
    expect(waiting).toContain('Portable signed buyer ticket');
    expect(waiting).toContain('disabled=""');
    const payerConnected = render(PAYER_REQUIRED_V1, { wallets: walletDirectory(PAYER) });
    expect(payerConnected).toContain('>Sign as the route payer</button>');
    expect(payerConnected).not.toContain('disabled=""');
  });

  it('says the packet request still does not submit, where the request is', () => {
    // Reached only in `wallet-preparable`, so the sentence is pinned against
    // the state that shows it rather than against the panel's idle shell.
    expect(idle).not.toContain('After signing, continue to Submit');
  });

  it('uses the operator’s published route in one line, without a textarea', () => {
    const published = render({ kind: 'idle' }, { routeText: 'ROUTE', publishedRoute: 'ROUTE' });
    expect(published).toContain('Using the operator&#x27;s published route for this market.');
    expect(published).toContain('change');
    expect(published).not.toContain('<textarea');
  });

  it('surfaces the route textarea as its own empty state when none is published', () => {
    expect(idle).toContain('<textarea');
    expect(idle).toContain('Checked Direct Hot route manifest');
  });

  /**
   * Not every refusal raised during preparation belongs to this step. The host
   * routes them; this step renders only what it owns, remedy first.
   */
  it('renders the refusal it owns with the remedy above the protocol’s words', () => {
    const refusal = assignRefusalV1('route manifest authenticates another Market or Trading program', 6);
    const html = render({ kind: 'refused', reason: 'ignored' }, { refusal });
    expect(html).toContain('Supply the route manifest for THIS market');
    expect(html).toContain('route manifest authenticates another Market or Trading program');
    expect(html.indexOf('Supply the route manifest for THIS market'))
      .toBeLessThan(html.indexOf('route manifest authenticates another Market'));
  });
});
