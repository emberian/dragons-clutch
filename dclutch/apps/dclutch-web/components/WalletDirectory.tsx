'use client';

import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react';

import {
  browserWalletRegistryV1,
  connectWalletIdentityV1,
  findAnnouncedWalletV1,
  projectAnnouncedWalletsV1,
  projectWalletConnectionV1,
  solanaChainForEndpointV1,
  subscribeWalletAccountsV1,
  walletConnectionTransitionV1,
  WALLET_CONNECTION_IDLE_V1,
  walletStandardHandoffV1,
  type DiscoveredWalletV1,
  type RefusedWalletV1,
  type WalletConnectionIntentV1,
  type WalletConnectionStateV1,
  type WalletHandoffV1,
  type WalletStandardRegistryV1,
} from '@/lib/walletStandard';

export type WalletConnectOutcomeV1 =
  | Readonly<{ status: 'connected'; address: string; label: string | null }>
  | Readonly<{ status: 'refused'; reason: string }>;

export type WalletDirectoryHandleV1 = Readonly<{
  state: WalletConnectionStateV1;
  wallets: ReadonlyArray<DiscoveredWalletV1>;
  refusals: ReadonlyArray<RefusedWalletV1>;
  address: string | null;
  connectedWalletId: string | null;
  connect(walletId: string): Promise<WalletConnectOutcomeV1>;
  forget(): void;
  /** The adapter `walletHandoff` rechecks before it will request a signature. */
  handoff(endpoint: string): WalletHandoffV1;
}>;

/**
 * `getWallets()` memoizes its answer on the first call and wires its DOM
 * handshake only when a window exists, so a single no-window call would leave
 * the registry permanently deaf. The no-window answer is therefore never
 * cached here.
 */
let cachedRegistry: WalletStandardRegistryV1 | null = null;
function liveRegistry(): WalletStandardRegistryV1 | null {
  if (cachedRegistry !== null) return cachedRegistry;
  cachedRegistry = browserWalletRegistryV1();
  return cachedRegistry;
}

function subscribeRegistry(onChange: () => void): () => void {
  const registry = liveRegistry();
  if (registry === null) return () => undefined;
  const offRegister = registry.on('register', onChange);
  const offUnregister = registry.on('unregister', onChange);
  return () => { offRegister(); offUnregister(); };
}

/** `get()` caches its array until registration changes, so this stays stable. */
function announcedSnapshot(): ReadonlyArray<unknown> | null {
  const registry = liveRegistry();
  if (registry === null) return null;
  try {
    return registry.get();
  } catch {
    return null;
  }
}

/** One stable reference, or React warns about an unbounded render loop. */
function serverSnapshot(): ReadonlyArray<unknown> | null {
  return null;
}

/**
 * Discover browser wallets through the Wallet Standard registry.
 *
 * The server render and hydration both report "no registry in this runtime",
 * so nothing about the user's installed extensions is asserted before the
 * browser answers. Connecting reads identity; requesting a signature is a
 * separate call this hook never makes.
 */
export function useWalletDirectoryV1(): WalletDirectoryHandleV1 {
  const announced = useSyncExternalStore(subscribeRegistry, announcedSnapshot, serverSnapshot);
  const discovery = useMemo(() => projectAnnouncedWalletsV1(announced), [announced]);
  const [intent, setIntent] = useState<WalletConnectionIntentV1>(WALLET_CONNECTION_IDLE_V1);
  const state = useMemo(() => projectWalletConnectionV1(discovery, intent), [discovery, intent]);

  const connect = useCallback(async (walletId: string): Promise<WalletConnectOutcomeV1> => {
    setIntent((current) => walletConnectionTransitionV1(current, { kind: 'connect-requested', walletId }));
    const candidate = findAnnouncedWalletV1(liveRegistry(), walletId);
    if (candidate === null) {
      const reason = 'the selected wallet is no longer registered in this browser';
      setIntent((current) => walletConnectionTransitionV1(current, { kind: 'connect-refused', walletId, reason }));
      return Object.freeze({ status: 'refused', reason });
    }
    try {
      const identity = await connectWalletIdentityV1(candidate);
      setIntent((current) => walletConnectionTransitionV1(current, { kind: 'connect-succeeded', walletId, address: identity.address, label: identity.label }));
      return Object.freeze({ status: 'connected', address: identity.address, label: identity.label });
    } catch (error) {
      const reason = error instanceof Error ? error.message : 'the wallet refused without a usable reason';
      setIntent((current) => walletConnectionTransitionV1(current, { kind: 'connect-refused', walletId, reason }));
      return Object.freeze({ status: 'refused', reason });
    }
  }, []);

  const forget = useCallback(() => setIntent((current) => walletConnectionTransitionV1(current, { kind: 'forgotten' })), []);

  // A connected wallet may switch or withdraw accounts on its own. Without its
  // own `standard:events` change stream the displayed identity would go stale.
  const connectedWalletId = state.kind === 'connected' ? state.walletId : null;
  useEffect(() => {
    if (connectedWalletId === null) return;
    const candidate = findAnnouncedWalletV1(liveRegistry(), connectedWalletId);
    if (candidate === null) return;
    return subscribeWalletAccountsV1(candidate, (account) => setIntent((current) => walletConnectionTransitionV1(current, {
      kind: 'account-changed',
      walletId: connectedWalletId,
      address: account?.address ?? null,
      label: account?.label ?? null,
    })));
  }, [connectedWalletId]);

  const handoff = useCallback((endpoint: string): WalletHandoffV1 => {
    if (state.kind !== 'connected') throw new Error('no browser wallet identity has been connected');
    const wallet = state.discovery.wallets.find((candidate) => candidate.id === state.walletId);
    const candidate = findAnnouncedWalletV1(liveRegistry(), state.walletId);
    if (wallet === undefined || candidate === null) throw new Error('the connected wallet is no longer registered in this browser');
    return walletStandardHandoffV1(candidate, state.address, solanaChainForEndpointV1(endpoint, wallet.solanaChains));
  }, [state]);

  return useMemo(() => Object.freeze({
    state,
    wallets: discovery.wallets,
    refusals: discovery.refusals,
    address: state.kind === 'connected' ? state.address : null,
    connectedWalletId,
    connect,
    forget,
    handoff,
  }), [state, discovery, connectedWalletId, connect, forget, handoff]);
}

export default function WalletDirectory({
  directory,
  purpose,
  onConnected,
}: Readonly<{
  directory: WalletDirectoryHandleV1;
  purpose: string;
  onConnected: (address: string) => void;
}>) {
  const { state, wallets, refusals } = directory;
  return <div className="wallet-directory">
    <span>Browser wallet · Wallet Standard · {purpose}</span>
    {wallets.length > 0 && <div className="wallet-directory-list">
      {wallets.map((wallet) => (
        <button
          key={wallet.id}
          type="button"
          className={`wallet-choice${directory.connectedWalletId === wallet.id ? ' connected' : ''}`}
          disabled={state.kind === 'connecting'}
          title={`${wallet.solanaChains.join(', ')} · ${wallet.canSignTransaction ? 'announces solana:signTransaction' : 'announces no transaction signing'}`}
          onClick={() => void directory.connect(wallet.id).then((outcome) => { if (outcome.status === 'connected') onConnected(outcome.address); })}
        >
          {wallet.icon !== null && <i className="wallet-mark" style={{ backgroundImage: `url("${wallet.icon}")` }} aria-hidden="true" />}
          {wallet.name}
        </button>
      ))}
      {directory.address !== null && <button type="button" className="wallet-choice" onClick={directory.forget}>Forget identity</button>}
    </div>}
    <p aria-live="polite">{state.message}</p>
    <p>Connecting reads a public address only. Requesting a signature is always a separate explicit action, and stays unavailable wherever a checked release does not recognize the outer.</p>
    <p>Only wallets that register through the Wallet Standard are listed. A wallet that exposes nothing but a legacy injected page provider is not discoverable here and is not silently probed for.</p>
    {refusals.length > 0 && <details>
      <summary>{refusals.length} announced registration{refusals.length === 1 ? '' : 's'} refused</summary>
      {refusals.map((refusal) => <p key={refusal.id}><strong>{refusal.name}</strong> · {refusal.reason}</p>)}
    </details>}
  </div>;
}
