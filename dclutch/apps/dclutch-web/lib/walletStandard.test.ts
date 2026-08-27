import { PublicKey, TransactionInstruction, TransactionMessage, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { requestWalletTransactionSignatureV1 } from './walletHandoff';
import {
  browserWalletRegistryV1,
  connectWalletIdentityV1,
  findAnnouncedWalletV1,
  describeWalletV1,
  discoverWalletsV1,
  projectWalletConnectionV1,
  solanaChainForEndpointV1,
  subscribeWalletAccountsV1,
  WALLET_CONNECTION_IDLE_V1,
  walletConnectionTransitionV1,
  walletStandardHandoffV1,
  type WalletStandardRegistryV1,
} from './walletStandard';

function key(byte: number): string { return new PublicKey(new Uint8Array(32).fill(byte)).toBase58(); }

type MockAccount = Readonly<{ address: string; label?: string; chains: string[]; features: string[] }>;

function account(byte: number, chains = ['solana:localnet']): MockAccount {
  return { address: key(byte), chains, features: ['solana:signTransaction', 'solana:signMessage'] };
}

function mockWallet(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    version: '1.0.0',
    name: 'Mock Wallet',
    icon: 'data:image/svg+xml;base64,PHN2Zy8+',
    chains: ['solana:localnet', 'solana:devnet'],
    accounts: [account(11)],
    features: {
      'standard:connect': { version: '1.0.0', connect: async () => ({ accounts: [account(11)] }) },
      'standard:events': { version: '1.0.0', on: () => () => undefined },
      'solana:signTransaction': { version: '1.0.0', supportedTransactionVersions: [0], signTransaction: async () => [] },
      'solana:signMessage': { version: '1.0.0', signMessage: async () => [] },
    },
    ...overrides,
  };
}

function registry(wallets: ReadonlyArray<unknown>): WalletStandardRegistryV1 {
  return Object.freeze({ get: () => wallets, on: () => () => undefined });
}

function unsignedFixture(payer: string): VersionedTransaction {
  const instruction = new TransactionInstruction({
    programId: new PublicKey(key(70)),
    keys: [{ pubkey: new PublicKey(key(71)), isSigner: false, isWritable: true }],
    data: Buffer.from([1, 2, 3]),
  });
  const message = new TransactionMessage({ payerKey: new PublicKey(payer), recentBlockhash: key(73), instructions: [instruction] }).compileToV0Message();
  return new VersionedTransaction(message);
}

describe('Wallet Standard discovery', () => {
  it('projects a conforming Solana wallet and its explicit feature capabilities', () => {
    const projection = describeWalletV1(mockWallet());
    expect(projection).toMatchObject({ name: 'Mock Wallet', canConnect: true, canSignTransaction: true, canSignMessage: true });
    if (!('solanaChains' in projection)) throw new Error('expected a discovered wallet');
    expect(projection.solanaChains).toEqual(['solana:devnet', 'solana:localnet']);
    expect(projection.icon).toBe('data:image/svg+xml;base64,PHN2Zy8+');
    expect(projection.accounts[0].address).toBe(key(11));
  });

  it('refuses nonconforming registrations with their exact reason instead of guessing', () => {
    const cases: ReadonlyArray<Readonly<[unknown, RegExp]>> = [
      ['not-an-object', /not an object/],
      [mockWallet({ version: '2.0.0' }), /Wallet Standard version 2\.0\.0/],
      [mockWallet({ chains: ['bip122:000000000019d6689c085ae165831e93'] }), /no solana: chain/],
      [mockWallet({ features: { 'standard:events': {} } }), /standard:connect/],
      [mockWallet({ accounts: [{ address: 'not-base58', chains: ['solana:localnet'], features: [] }] }), /canonical Solana address/],
      [mockWallet({ name: '' }), /1\.\.64 characters/],
    ];
    for (const [candidate, pattern] of cases) {
      const projection = describeWalletV1(candidate);
      if (!('reason' in projection)) throw new Error('expected a refusal');
      expect(projection.reason).toMatch(pattern);
    }
  });

  it('drops a nonconforming icon without refusing the wallet', () => {
    const projection = describeWalletV1(mockWallet({ icon: 'https://example.invalid/logo.png' }));
    if (!('icon' in projection)) throw new Error('expected a discovered wallet');
    expect(projection.icon).toBeNull();
  });

  it('reports an honest listing for an absent registry, an empty registry, and refusals', () => {
    const absent = discoverWalletsV1(null);
    expect(absent.registryPresent).toBe(false);
    expect(absent.wallets).toEqual([]);
    expect(absent.reason).toMatch(/No Wallet Standard registry/);

    const empty = discoverWalletsV1(registry([]));
    expect(empty.registryPresent).toBe(true);
    expect(empty.reason).toMatch(/no browser wallet has registered/);

    // Talisman registers a real Solana Wallet Standard wallet (v3.0.0, 2025-09-03)
    // and injects no window.solana, so it is discovered exactly like Phantom.
    const mixed = discoverWalletsV1(registry([
      mockWallet({ name: 'Talisman' }),
      mockWallet({ name: 'Substrate Only', chains: ['polkadot:91b171bb158e2d3848fa23a9f1c25182'] }),
    ]));
    expect(mixed.wallets).toHaveLength(1);
    expect(mixed.wallets[0].name).toBe('Talisman');
    expect(mixed.refusals).toHaveLength(1);
    expect(mixed.refusals[0].name).toBe('Substrate Only');
    expect(mixed.reason).toMatch(/1 conforming Solana wallet announced; 1 registration refused/);
  });

  it('names a wallet by printable name and version, and finds it again in the live registry', () => {
    const live = registry([mockWallet({ name: 'Other Wallet' }), mockWallet()]);
    const discovery = discoverWalletsV1(live);
    const mock = discovery.wallets.find((wallet) => wallet.name === 'Mock Wallet');
    if (mock === undefined) throw new Error('expected the mock wallet to be listed');
    expect(mock.id).toBe('Mock Wallet 1.0.0');
    expect(findAnnouncedWalletV1(live, mock.id)).toBe(live.get()[1]);
    expect(findAnnouncedWalletV1(live, 'Ghost 1.0.0')).toBeNull();
    expect(findAnnouncedWalletV1(null, mock.id)).toBeNull();
  });

  it('returns no registry under SSR or a non-browser test runtime', () => {
    expect(typeof window).toBe('undefined');
    expect(browserWalletRegistryV1()).toBeNull();
  });
});

describe('Wallet Standard connect state machine', () => {
  const discovery = discoverWalletsV1(registry([mockWallet()]));
  const walletId = discovery.wallets[0].id;
  const idle = WALLET_CONNECTION_IDLE_V1;

  function connectedIntent() {
    const connecting = walletConnectionTransitionV1(idle, { kind: 'connect-requested', walletId });
    return walletConnectionTransitionV1(connecting, { kind: 'connect-succeeded', walletId, address: key(11), label: null });
  }

  it('projects an idle intent into the state its registry justifies', () => {
    expect(projectWalletConnectionV1(discoverWalletsV1(null), idle).kind).toBe('unsupported');
    expect(projectWalletConnectionV1(discoverWalletsV1(registry([])), idle).kind).toBe('empty');
    expect(projectWalletConnectionV1(discovery, idle).kind).toBe('discovered');
  });

  it('reaches connected only through an explicit request for that exact wallet', () => {
    const connecting = walletConnectionTransitionV1(idle, { kind: 'connect-requested', walletId });
    expect(connecting.kind).toBe('connecting');
    expect(projectWalletConnectionV1(discovery, connecting).message).toMatch(/no signature is requested/);
    const connected = walletConnectionTransitionV1(connecting, { kind: 'connect-succeeded', walletId, address: key(11), label: null });
    expect(connected).toMatchObject({ kind: 'connected', address: key(11), switched: false });
    expect(projectWalletConnectionV1(discovery, connected).message).toMatch(/identity only; no signature requested/);
  });

  it('refuses an answer this surface did not request and an unlisted wallet', () => {
    expect(walletConnectionTransitionV1(idle, { kind: 'connect-succeeded', walletId, address: key(11), label: null })).toMatchObject({ kind: 'refused' });
    const ghost = walletConnectionTransitionV1(idle, { kind: 'connect-requested', walletId: 'Ghost 1.0.0' });
    expect(projectWalletConnectionV1(discovery, ghost)).toMatchObject({ kind: 'refused' });
    expect(projectWalletConnectionV1(discovery, ghost).message).toMatch(/not in the current registry listing/);
    const connecting = walletConnectionTransitionV1(idle, { kind: 'connect-requested', walletId });
    expect(walletConnectionTransitionV1(connecting, { kind: 'connect-succeeded', walletId: 'Other 1.0.0', address: key(11), label: null })).toMatchObject({ kind: 'refused' });
    const rejected = walletConnectionTransitionV1(connecting, { kind: 'connect-refused', walletId, reason: 'user rejected' });
    expect(projectWalletConnectionV1(discovery, rejected).message).toMatch(/Refused: user rejected/);
  });

  it('drops a connected identity when its wallet unregisters, and forgets on request', () => {
    const connected = connectedIntent();
    const gone = projectWalletConnectionV1(discoverWalletsV1(registry([])), connected);
    expect(gone).toMatchObject({ kind: 'refused' });
    expect(gone.message).toMatch(/unregistered itself/);
    expect(projectWalletConnectionV1(discovery, walletConnectionTransitionV1(connected, { kind: 'forgotten' })).kind).toBe('discovered');
  });
});

describe('Wallet Standard identity and handoff', () => {
  it('requests identity only and never a signature', async () => {
    let connectCalls = 0;
    let signCalls = 0;
    const wallet = mockWallet({
      features: {
        'standard:connect': { version: '1.0.0', connect: async () => { connectCalls += 1; return { accounts: [account(11)] }; } },
        'solana:signTransaction': { version: '1.0.0', signTransaction: async () => { signCalls += 1; return []; } },
      },
    });
    const identity = await connectWalletIdentityV1(wallet);
    expect(identity.address).toBe(key(11));
    expect(identity.chains).toEqual(['solana:localnet']);
    expect(connectCalls).toBe(1);
    expect(signCalls).toBe(0);
  });

  it('refuses a wallet that connects without authorizing a Solana account', async () => {
    await expect(connectWalletIdentityV1(mockWallet({
      accounts: [],
      features: { 'standard:connect': { version: '1.0.0', connect: async () => ({ accounts: [] }) } },
    }))).rejects.toThrow(/without authorizing one account/);
    await expect(connectWalletIdentityV1(mockWallet({
      features: { 'standard:connect': { version: '1.0.0', connect: async () => ({ accounts: [account(11, ['eip155:1'])] }) } },
    }))).rejects.toThrow(/no solana: chain/);
  });

  it('selects the chain an endpoint justifies and refuses when the wallet has none', () => {
    expect(solanaChainForEndpointV1('http://127.0.0.1:8899', ['solana:mainnet', 'solana:localnet'])).toBe('solana:localnet');
    expect(solanaChainForEndpointV1('http://127.0.0.1:8899', ['solana:devnet'])).toBe('solana:devnet');
    expect(solanaChainForEndpointV1('https://rpc.example.com', ['solana:devnet', 'solana:mainnet'])).toBe('solana:mainnet');
    expect(() => solanaChainForEndpointV1('https://rpc.example.com', ['eip155:1'])).toThrow(/announces none of/);
  });

  it('forwards one explicit signTransaction request that walletHandoff still rechecks', async () => {
    const payer = key(11);
    let signCalls = 0;
    const wallet = mockWallet({
      features: {
        'standard:connect': { version: '1.0.0', connect: async () => ({ accounts: [account(11)] }) },
        'solana:signTransaction': {
          version: '1.0.0',
          signTransaction: async (input: Readonly<{ transaction: Uint8Array; chain: string }>) => {
            signCalls += 1;
            expect(input.chain).toBe('solana:localnet');
            const decoded = VersionedTransaction.deserialize(input.transaction);
            decoded.signatures[0] = new Uint8Array(64).fill(9);
            return [{ signedTransaction: decoded.serialize() }];
          },
        },
      },
    });
    const handoff = walletStandardHandoffV1(wallet, payer, 'solana:localnet');
    expect(signCalls).toBe(0);
    const signed = await requestWalletTransactionSignatureV1(handoff, unsignedFixture(payer), payer);
    expect(signCalls).toBe(1);
    expect(signed.signer).toBe(payer);
    expect(signed.complete).toBe(true);
  });

  it('lets walletHandoff reject a wallet that rewrites the message it was handed', async () => {
    const payer = key(11);
    const wallet = mockWallet({
      features: {
        'standard:connect': { version: '1.0.0', connect: async () => ({ accounts: [account(11)] }) },
        'solana:signTransaction': {
          version: '1.0.0',
          signTransaction: async () => {
            const rewritten = unsignedFixture(payer);
            rewritten.message.recentBlockhash = key(99);
            rewritten.signatures[0] = new Uint8Array(64).fill(9);
            return [{ signedTransaction: rewritten.serialize() }];
          },
        },
      },
    });
    const handoff = walletStandardHandoffV1(wallet, payer, 'solana:localnet');
    await expect(requestWalletTransactionSignatureV1(handoff, unsignedFixture(payer), payer)).rejects.toThrow(/rewrote/);
  });

  it('exposes no sign method a wallet did not announce, and refuses an unauthorized address', async () => {
    const readOnly = walletStandardHandoffV1(mockWallet({
      features: { 'standard:connect': { version: '1.0.0', connect: async () => ({ accounts: [account(11)] }) } },
    }), key(11), 'solana:localnet');
    expect(readOnly.signTransaction).toBeUndefined();
    expect(readOnly.signMessage).toBeUndefined();
    const other = walletStandardHandoffV1(mockWallet(), key(12), 'solana:localnet');
    await expect(other.connect()).rejects.toThrow(/has not authorized/);
  });
});

describe('Wallet Standard account changes', () => {
  const discovery = discoverWalletsV1(registry([mockWallet()]));
  const walletId = discovery.wallets[0].id;

  function connected() {
    const connecting = walletConnectionTransitionV1(WALLET_CONNECTION_IDLE_V1, { kind: 'connect-requested', walletId });
    return walletConnectionTransitionV1(connecting, { kind: 'connect-succeeded', walletId, address: key(11), label: null });
  }

  it('follows a wallet account switch and drops identity when every account is withdrawn', () => {
    const switched = walletConnectionTransitionV1(connected(), { kind: 'account-changed', walletId, address: key(12), label: 'second' });
    expect(switched).toMatchObject({ kind: 'connected', address: key(12), label: 'second', switched: true });
    expect(projectWalletConnectionV1(discovery, switched).message).toMatch(/switched accounts/);
    const withdrawn = walletConnectionTransitionV1(connected(), { kind: 'account-changed', walletId, address: null, label: null });
    expect(withdrawn).toMatchObject({ kind: 'refused' });
    expect(projectWalletConnectionV1(discovery, withdrawn).message).toMatch(/withdrew every authorized account/);
    expect(walletConnectionTransitionV1(connected(), { kind: 'account-changed', walletId: 'Other 1.0.0', address: null, label: null }).kind).toBe('connected');
  });

  it('subscribes to a wallet standard:events change stream and unsubscribes cleanly', () => {
    let handler: ((properties: unknown) => void) | null = null;
    let off = 0;
    const wallet = mockWallet({
      features: {
        'standard:connect': { version: '1.0.0', connect: async () => ({ accounts: [account(11)] }) },
        'standard:events': {
          version: '1.0.0',
          on: (_event: string, listener: (properties: unknown) => void) => { handler = listener; return () => { off += 1; }; },
        },
      },
    });
    const seen: Array<string | null> = [];
    const unsubscribe = subscribeWalletAccountsV1(wallet, (next) => seen.push(next?.address ?? null));
    if (handler === null) throw new Error('expected a change listener');
    const notify = handler as (properties: unknown) => void;
    notify({ chains: ['solana:devnet'] });
    notify({ accounts: [account(12)] });
    notify({ accounts: [] });
    notify({ accounts: [{ address: 'not-base58', chains: [], features: [] }] });
    expect(seen).toEqual([key(12), null, null]);
    unsubscribe();
    expect(off).toBe(1);
  });

  it('reports no subscription for a wallet that announces no standard:events', () => {
    const unsubscribe = subscribeWalletAccountsV1(mockWallet({ features: { 'standard:connect': { version: '1.0.0', connect: async () => ({ accounts: [] }) } } }), () => undefined);
    expect(() => unsubscribe()).not.toThrow();
  });

  it('refuses an account whose raw public key does not encode its own address', () => {
    const mismatched = describeWalletV1(mockWallet({
      accounts: [{ address: key(11), publicKey: new Uint8Array(32).fill(12), chains: ['solana:localnet'], features: [] }],
    }));
    if (!('reason' in mismatched)) throw new Error('expected a refusal');
    expect(mismatched.reason).toMatch(/does not encode its own address/);
    const agreeing = describeWalletV1(mockWallet({
      accounts: [{ address: key(11), publicKey: new Uint8Array(32).fill(11), chains: ['solana:localnet'], features: [] }],
    }));
    expect('accounts' in agreeing).toBe(true);
  });
});
