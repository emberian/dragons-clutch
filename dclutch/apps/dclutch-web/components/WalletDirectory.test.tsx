import { PublicKey } from '@solana/web3.js';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import {
  discoverWalletsV1,
  projectWalletConnectionV1,
  WALLET_CONNECTION_IDLE_V1,
  walletConnectionTransitionV1,
  type WalletConnectionStateV1,
  type WalletDiscoveryV1,
  type WalletStandardRegistryV1,
} from '@/lib/walletStandard';
import DirectTradeWorkspace from './DirectTradeWorkspace';
import RationalOpenPanel from './RationalOpenPanel';
import WalletDirectory, { type WalletDirectoryHandleV1 } from './WalletDirectory';

const ADDRESS = new PublicKey(new Uint8Array(32).fill(11)).toBase58();

function wallet(name: string, overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    version: '1.0.0',
    name,
    chains: ['solana:localnet'],
    accounts: [{ address: ADDRESS, chains: ['solana:localnet'], features: [] }],
    features: { 'standard:connect': { version: '1.0.0', connect: async () => ({ accounts: [] }) } },
    ...overrides,
  };
}

function registry(wallets: ReadonlyArray<unknown>): WalletStandardRegistryV1 {
  return Object.freeze({ get: () => wallets, on: () => () => undefined });
}

function handle(state: WalletConnectionStateV1): WalletDirectoryHandleV1 {
  return Object.freeze({
    state,
    wallets: state.discovery.wallets,
    refusals: state.discovery.refusals,
    address: state.kind === 'connected' ? state.address : null,
    connectedWalletId: state.kind === 'connected' ? state.walletId : null,
    connect: async () => Object.freeze({ status: 'refused' as const, reason: 'not reachable from a static render' }),
    forget: () => undefined,
    handoff: () => { throw new Error('not reachable from a static render'); },
  });
}

function render(discovery: WalletDiscoveryV1, intent = WALLET_CONNECTION_IDLE_V1): string {
  const state = projectWalletConnectionV1(discovery, intent);
  return renderToStaticMarkup(<WalletDirectory directory={handle(state)} purpose="payer identity" onConnected={() => undefined} />);
}

describe('browser wallet directory panel', () => {
  it('states honestly that no registry exists under SSR or a wallet-less browser', () => {
    const html = render(discoverWalletsV1(null));
    expect(html).toContain('No Wallet Standard registry exists in this runtime');
    expect(html).toContain('Connecting reads a public address only.');
    expect(html).not.toContain('wallet-choice');
  });

  it('says a registry is present but empty rather than implying a wallet is missing from the app', () => {
    const html = render(discoverWalletsV1(registry([])));
    expect(html).toContain('no browser wallet has registered');
    expect(html).toContain('Install a conforming Solana wallet extension');
  });

  it('lists every conforming wallet and discloses each refused registration', () => {
    // Talisman registers a real Solana Wallet Standard wallet and injects no
    // window.solana, so it lists alongside Phantom rather than being refused.
    const discovery = discoverWalletsV1(registry([
      wallet('Phantom'),
      wallet('Solflare'),
      wallet('Talisman'),
      wallet('Substrate Only', { chains: ['polkadot:91b171bb158e2d3848fa23a9f1c25182'] }),
    ]));
    const html = render(discovery);
    expect(html).toContain('Phantom');
    expect(html).toContain('Solflare');
    expect(html).toContain('Talisman');
    expect(html).toContain('1 announced registration refused');
    expect(html).toContain('no solana: chain; it is not a Solana Wallet Standard wallet');
    expect(html).not.toContain('Forget identity');
  });

  it('says plainly that a legacy injection-only wallet is not discoverable here', () => {
    const html = render(discoverWalletsV1(registry([])));
    expect(html).toContain('Only wallets that register through the Wallet Standard are listed');
    expect(html).toContain('is not silently probed for');
  });

  it('shows one connected identity as identity only, with a way to forget it', () => {
    const discovery = discoverWalletsV1(registry([wallet('Phantom')]));
    const connecting = walletConnectionTransitionV1(WALLET_CONNECTION_IDLE_V1, { kind: 'connect-requested', walletId: 'Phantom 1.0.0' });
    const html = render(discovery, walletConnectionTransitionV1(connecting, { kind: 'connect-succeeded', walletId: 'Phantom 1.0.0', address: ADDRESS, label: null }));
    expect(html).toContain('wallet-choice connected');
    expect(html).toContain('identity only; no signature requested');
    expect(html).toContain('Forget identity');
  });
});

describe('workspaces reach wallets through the directory', () => {
  it('replaces the bespoke connect button on identity-only and signing surfaces alike', () => {
    for (const html of [renderToStaticMarkup(<RationalOpenPanel />), renderToStaticMarkup(<DirectTradeWorkspace />)]) {
      expect(html).toContain('Browser wallet · Wallet Standard');
      expect(html).toContain('No Wallet Standard registry exists in this runtime');
      expect(html).not.toContain('Connect identity');
      expect(html).not.toContain('Connect payer');
    }
  });

  it('keeps every release-gated signing control disabled on the Rational open route', () => {
    const html = renderToStaticMarkup(<RationalOpenPanel />);
    expect(html).toContain('Wallet signing blocked by checked-release gate');
    expect(html).toContain('title="No checked positive common-Hot release is active."');
    expect(html).toContain('<button type="button" disabled="" title="No checked positive common-Hot release is active.">');
  });
});
