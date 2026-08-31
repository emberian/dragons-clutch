import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import SignStep from './SignStep';
import { type WalletPreparationState } from '@/lib/tradeFlowMachine';
import { assignRefusalV1 } from '@/lib/tradeFlowRefusals';

const noop = () => {};

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
  refusal={null}
  {...overrides}
/>);

const OPERATOR_REQUIRED_V1: WalletPreparationState = Object.freeze({
  kind: 'operator-required' as const,
  payer: 'Payer1111111111111111111111111111111111111',
  reason: 'the route names another payer',
  takerTicket: '{"kind":"dclutch/direct-intent-ticket/v1"}',
  routeObservedSlot: '490712003',
  lastValidBlockHeight: '4001',
});

describe('step 6, the two signatures', () => {
  const idle = render({ kind: 'idle' });

  /**
   * THE RESUMPTION PROMISE, relocated.
   *
   * It used to sit at the top of the panel as the second of three
   * undifferentiated status paragraphs -- told to a reader who did not yet
   * know what signing or sending were, which is to say told to nobody. It now
   * renders in this step's header, one step away from being true, and it is
   * pinned HERE so the move cannot become a deletion.
   */
  it('carries the resumption promise, whole and unsplit', () => {
    expect(idle).toContain('Signing sends nothing.');
    expect(idle).toContain('it happens once');
    expect(idle).toContain('rather than sending a second one');
    // One sentence in one element. A `toContain` guard cannot survive half of
    // it being wrapped in a span for emphasis, and the guard is the point.
    expect(idle).toContain('Signing sends nothing. Sending is a separate step you take, and it happens once — reload part-way through and this page picks up the transaction you already sent rather than sending a second one.');
  });

  it('renders two signature rows and never one, before either has happened', () => {
    expect(idle).toContain('Your intent');
    expect(idle).toContain('The transaction');
    expect(idle).toContain('sign message');
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
    const signed = render(OPERATOR_REQUIRED_V1);
    expect(signed).toContain('Your intent is signed. Nothing has executed.');
    expect(signed.split('signature-done').length - 1).toBe(1);
  });

  it('does not offer to sign again once the intent is signed', () => {
    expect(idle).toContain('Sign my intent, then authenticate the packet');
    expect(render(OPERATOR_REQUIRED_V1)).not.toContain('Sign my intent, then authenticate the packet');
  });

  it('says the packet request still does not submit, where the request is', () => {
    // Reached only in `wallet-preparable`, so the sentence is pinned against
    // the state that shows it rather than against the panel's idle shell.
    expect(idle).not.toContain('This request still does not submit.');
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
