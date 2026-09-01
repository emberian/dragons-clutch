import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { type WalletDirectoryHandleV1 } from '@/components/WalletDirectory';
import { type DenominationV1 } from '@/lib/quantity';

import MakerOfferComposer from './MakerOfferComposer';

const ADDRESS = '11111111111111111111111111111111';
const DENOMINATION: DenominationV1 = Object.freeze({ decimals: 6, unit: null, mint: ADDRESS });
const WALLETS = Object.freeze({
  state: Object.freeze({ kind: 'idle' }),
  wallets: Object.freeze([]),
  refusals: Object.freeze([]),
  address: null,
  connectedWalletId: null,
  connect: async () => Object.freeze({ status: 'refused', reason: 'not used' }),
  forget: () => undefined,
  handoff: () => null,
}) as unknown as WalletDirectoryHandleV1;

function render(): string {
  return renderToStaticMarkup(<MakerOfferComposer
    endpoint="http://127.0.0.1:8899"
    marketAddress={ADDRESS}
    coreProgramId={ADDRESS}
    registryProgramId={ADDRESS}
    claimsProgramId={ADDRESS}
    tradingProgramId={ADDRESS}
    custodyProgramId={ADDRESS}
    rentProgramId={ADDRESS}
    generation={BigInt(1)}
    feeBasisPoints={25}
    outcomeCount={2}
    outcome={1}
    outcomeLabel={(index) => index === 1 ? 'Rain' : 'Dry'}
    denomination={DENOMINATION}
    priceScale={BigInt(1_000_000)}
    clock={null}
    nowMs={null}
    wallets={WALLETS}
    boardConfig={null}
  />);
}

describe('the maker offer composer surface', () => {
  it('explains the exact non-transaction authority boundary before either act', () => {
    const html = render();
    expect(html).toContain('Make your own sell offer');
    expect(html).toContain('authoring an offer, not making a transaction');
    expect(html).toContain('Nothing is signed by checking');
    expect(html).toContain('Sign portable ticket');
    expect(html).not.toContain('Submit');
  });

  it('requires an explicit lifetime and explains both lifecycle choices', () => {
    const html = render();
    expect(html).toContain('Valid for how many slots');
    expect(html).toContain('No default lifetime is chosen for you');
    expect(html).toContain('Allow one smaller fill');
    expect(html).toContain('All or nothing');
    expect(html).toContain('no remainder rests onchain');
  });

  it('keeps relay-independent authoring visible when no board is configured', () => {
    const html = render();
    expect(html).toContain('Check these exact terms');
    expect(html).not.toContain('Post to configured relay');
  });
});
