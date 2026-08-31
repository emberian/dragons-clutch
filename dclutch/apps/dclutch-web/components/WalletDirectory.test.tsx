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
  return renderToStaticMarkup(<WalletDirectory directory={handle(state)} onConnected={() => undefined} />);
}

describe('browser wallet directory panel', () => {
  /**
   * Renegotiated 2026-08-31. This panel used to carry two standing paragraphs
   * -- what connecting does and does not do, and why an injection-only wallet
   * is not listed or probed -- plus a Wallet-Standard-flavoured status line
   * under every state. The panel is now a heading, the buttons, and one state
   * line. What is still pinned is that each state says the right SHORT thing
   * and never implies the app is at fault for a missing extension.
   */
  it('says there is no wallet, without blaming the app or naming the registry', () => {
    const html = render(discoverWalletsV1(null));
    expect(html).toContain('No browser wallet found.');
    expect(html).not.toContain('wallet-choice');
    expect(html).not.toContain('Wallet Standard');
  });

  it('tells an empty registry apart from a missing one, and says what to do', () => {
    const html = render(discoverWalletsV1(registry([])));
    expect(html).toContain('No wallet extension found. Install a Solana wallet to connect.');
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
    // A wallet that could not be listed is still disclosed, in an expander,
    // with its exact reason -- but the count no longer narrates the mechanism.
    expect(html).toContain('1 wallet could not be listed');
    expect(html).toContain('no solana: chain; it is not a Solana Wallet Standard wallet');
    expect(html).not.toContain('Disconnect');
  });

  it('carries no standing explanation of what connecting does', () => {
    const html = render(discoverWalletsV1(registry([])));
    for (const sermon of ['Connecting reads a public address only', 'is not silently probed for', 'Only wallets that register']) {
      expect(html).not.toContain(sermon);
    }
  });

  it('shows one connected identity as identity only, with a way to forget it', () => {
    const discovery = discoverWalletsV1(registry([wallet('Phantom')]));
    const connecting = walletConnectionTransitionV1(WALLET_CONNECTION_IDLE_V1, { kind: 'connect-requested', walletId: 'Phantom 1.0.0' });
    const html = render(discovery, walletConnectionTransitionV1(connecting, { kind: 'connect-succeeded', walletId: 'Phantom 1.0.0', address: ADDRESS, label: null }));
    expect(html).toContain('wallet-choice connected');
    expect(html).toContain(`Connected · ${ADDRESS}`);
    expect(html).toContain('Disconnect');
  });
});

describe('workspace wallet boundaries', () => {
  it('keeps the Wallet Standard directory on the release-gated Rational open route', () => {
    const html = renderToStaticMarkup(<RationalOpenPanel />);
    expect(html).toContain('No browser wallet found.');
    expect(html).not.toContain('Connect identity');
    expect(html).not.toContain('Connect payer');
  });

  it('keeps the read-only Direct preview free of wallet and transaction controls', () => {
    const html = renderToStaticMarkup(<DirectTradeWorkspace />);
    expect(html).not.toContain('wallet-directory');
    expect(html).toContain('This page has no wallet connection, signature request, packet download, or submission control.');
    expect(html).toContain('No wallet request · no packet builder · no submission path');
  });

  it('keeps every release-gated signing control disabled on the Rational open route', () => {
    const html = renderToStaticMarkup(<RationalOpenPanel />);
    expect(html).toContain('Wallet signing blocked by checked-release gate');
    expect(html).toContain('title="No checked positive common-Hot release is active."');
    expect(html).toContain('<button type="button" disabled="" title="No checked positive common-Hot release is active.">');
  });
});
