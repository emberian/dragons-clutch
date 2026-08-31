import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import SendStep from './SendStep';
import { type WalletPreparationState } from '@/lib/tradeFlowMachine';
import { assignRefusalV1 } from '@/lib/tradeFlowRefusals';

const render = (
  walletPreparation: WalletPreparationState,
  refusal: Parameters<typeof SendStep>[0]['refusal'] = null,
) => renderToStaticMarkup(<SendStep
  walletPreparation={walletPreparation}
  onSubmit={() => {}}
  refusal={refusal}
/>);

const OPERATOR_REQUIRED_V1: WalletPreparationState = Object.freeze({
  kind: 'operator-required' as const,
  payer: 'Payer1111111111111111111111111111111111111',
  reason: 'the authenticated route names another fee payer',
  takerTicket: '{"kind":"dclutch/direct-intent-ticket/v1"}',
  routeObservedSlot: '490712003',
  lastValidBlockHeight: '4001',
});

const WALLET_SIGNED_V1: WalletPreparationState = Object.freeze({
  kind: 'wallet-signed' as const,
  signature: 'Sig1111111111111111111111111111111111111111',
  signedWireBase64: 'AAAA',
  messageBase64: 'BBBB',
  wireBytes: 1_204,
  routeObservedSlot: '490712003',
  blockhashObservedSlot: '490712100',
  lastValidBlockHeight: '4001',
  lookupTable: 'Lut111111111111111111111111111111111111111',
  journal: {} as never,
  takerBefore: {} as never,
});

describe('step 7, the one send', () => {
  /**
   * The requirement the whole step exists for: sending is its own act, with
   * its own button, and that button exists in exactly one state. Offering it
   * anywhere else would invite the double-send the journal underneath exists
   * to make impossible.
   */
  it('offers a send control only where sending is the thing to do', () => {
    expect(render(WALLET_SIGNED_V1)).toContain('>Send it</button>');
    expect(render({ kind: 'idle' })).not.toContain('Send it');
    expect(render(OPERATOR_REQUIRED_V1)).not.toContain('Send it');
    // Submitted: in flight. No control could help, so none is drawn.
    expect(render({
      kind: 'submitted', journal: {} as never, signature: 'Sig111', takerBefore: null,
      confirmation: 'The exact signature is not finalized yet.',
    })).not.toContain('<button');
  });

  it('says a signed packet is saved and not yet sent, before it is sent', () => {
    const html = render(WALLET_SIGNED_V1);
    expect(html).toContain('Wallet signed · saved locally, not yet submitted');
    expect(html).toContain('nothing has been sent to RPC');
  });

  /**
   * `operator-required` IS A FIRST-CLASS OUTCOME, NOT AN ERROR.
   *
   * The trader did everything right; the route's payer is somebody else. They
   * are holding a real, portable, signed ticket, and dressing that as a
   * failure would tell them their signature was wasted when it is the exact
   * artifact the flow was for. It gets `flow-terminal` -- the same weight as
   * `executed` -- and it is never rendered through the refusal treatment.
   */
  it('gives operator-required the weight of a finished outcome, not a refusal', () => {
    const html = render(OPERATOR_REQUIRED_V1);
    expect(html).toContain('flow-terminal');
    expect(html).toContain('Your intent is signed. Nothing has executed.');
    expect(html).toContain('Payer1111111111111111111111111111111111111');
    expect(html).toContain('This page has not built, signed, or submitted a transaction.');
    // Not an alert, and not the amber refusal treatment.
    expect(html).not.toContain('flow-refusal');
    expect(html).not.toContain('role="alert"');
  });

  it('sends the reader to the explorer once it is finalized, and only then', () => {
    const executed = render({
      kind: 'executed', signature: 'Sig222', observedSlot: '490712009',
      after: { positionBalances: [1n, 0n], spendableCollateralAtoms: 5n } as never,
      changes: null,
    });
    expect(executed).toContain('Executed · finalized');
    expect(executed).toContain('flow-terminal');
    expect(executed).toContain('/explorer?view=transaction&amp;q=Sig222');
    expect(render(WALLET_SIGNED_V1)).not.toContain('/explorer?view=transaction');
  });

  it('keeps the never-send-twice promise standing in every state', () => {
    for (const state of [{ kind: 'idle' } as WalletPreparationState, WALLET_SIGNED_V1, OPERATOR_REQUIRED_V1]) {
      expect(render(state)).toContain('The signed packet is saved in this browser before its one send, so a reload picks it up rather than sending twice.');
    }
  });

  it('renders the refusal it owns, remedy first', () => {
    const refusal = assignRefusalV1('the signed packet expired at block height 4001; the chain can no longer include it', 7);
    const html = render({ kind: 'refused', reason: 'ignored' }, refusal);
    expect(html).toContain('Prepare and sign a new packet');
    expect(html).toContain('the chain can no longer include it');
  });
});
