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
    // Submitted: in flight. No control could help, so none is drawn.
    expect(render({
      kind: 'submitted', journal: {} as never, signature: 'Sig111', takerBefore: null,
      confirmation: 'The exact signature is not finalized yet.',
    })).not.toContain('<button');
  });

  it('says a signed packet is saved and not yet sent, before it is sent', () => {
    const html = render(WALLET_SIGNED_V1);
    expect(html).toContain('Wallet signed · saved locally, not yet submitted');
    expect(html).toContain('Saved in this browser and ready to submit.');
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

  it('shows how to resume transaction status', () => {
    for (const state of [{ kind: 'idle' } as WalletPreparationState, WALLET_SIGNED_V1]) {
      expect(render(state)).toContain('You can reload this page to resume checking this transaction.');
    }
  });

  it('renders the refusal it owns, remedy first', () => {
    const refusal = assignRefusalV1('the signed packet expired at block height 4001; the chain can no longer include it', 7);
    const html = render({ kind: 'refused', reason: 'ignored' }, refusal);
    expect(html).toContain('Prepare and sign a new packet');
    expect(html).toContain('the chain can no longer include it');
  });
});
