import { PublicKey, VersionedTransaction } from '@solana/web3.js';
import {
  SolanaSignAndSendTransaction,
  SolanaSignIn,
  SolanaSignMessage,
  SolanaSignTransaction,
} from '@solana/wallet-standard-features';
import { getWallets } from '@wallet-standard/app';
import { StandardConnect, StandardDisconnect, StandardEvents } from '@wallet-standard/features';

/**
 * Wallet Standard discovery for browser-installed Solana wallets.
 *
 * Every wallet a browser extension registers is an untrusted projection: this
 * module hostile-decodes the announced object before it is listed, and refuses
 * a nonconforming registration with its exact reason instead of guessing a
 * layout. Connecting reads identity only. Signing is never initiated here; the
 * exported handoff adapter only exposes the wallet's own explicit sign features
 * to `walletHandoff`, which independently rechecks signer and message bytes.
 */

export const WALLET_STANDARD_VERSION = '1.0.0';

/**
 * Feature and chain identifiers are owned by the standard's own packages, not
 * restated here, so a spec revision cannot drift silently past this browser.
 */
export const STANDARD_CONNECT: string = StandardConnect;
export const STANDARD_DISCONNECT: string = StandardDisconnect;
export const STANDARD_EVENTS: string = StandardEvents;
export const SOLANA_SIGN_TRANSACTION: string = SolanaSignTransaction;
export const SOLANA_SIGN_AND_SEND_TRANSACTION: string = SolanaSignAndSendTransaction;
export const SOLANA_SIGN_MESSAGE: string = SolanaSignMessage;
export const SOLANA_SIGN_IN: string = SolanaSignIn;

export const SOLANA_CHAINS = Object.freeze([
  'solana:mainnet',
  'solana:devnet',
  'solana:testnet',
  'solana:localnet',
] as const);
export type SolanaChainV1 = (typeof SOLANA_CHAINS)[number];

const MAX_LISTED_WALLETS = 32;
const MAX_LISTED_ACCOUNTS = 16;
const MAX_NAME_BYTES = 64;
const MAX_ICON_BYTES = 128 * 1024;
const ICON_PREFIX = /^data:image\/(svg\+xml|webp|png|gif);base64,[A-Za-z0-9+/]+={0,2}$/;

/** The subset of `@wallet-standard/app`'s `getWallets()` handle this app uses. */
export type WalletStandardRegistryV1 = Readonly<{
  get(): ReadonlyArray<unknown>;
  on(event: 'register' | 'unregister', listener: (...wallets: ReadonlyArray<unknown>) => void): () => void;
}>;

export type WalletStandardAccountV1 = Readonly<{
  address: string;
  label: string | null;
  chains: ReadonlyArray<string>;
  features: ReadonlyArray<string>;
}>;

export type DiscoveredWalletV1 = Readonly<{
  id: string;
  name: string;
  version: string;
  icon: string | null;
  chains: ReadonlyArray<string>;
  features: ReadonlyArray<string>;
  solanaChains: ReadonlyArray<SolanaChainV1>;
  accounts: ReadonlyArray<WalletStandardAccountV1>;
  canConnect: boolean;
  canDisconnect: boolean;
  canSignTransaction: boolean;
  canSignMessage: boolean;
  announcesSignAndSend: boolean;
}>;

export type RefusedWalletV1 = Readonly<{ id: string; name: string; reason: string }>;

export type WalletDiscoveryV1 = Readonly<{
  registryPresent: boolean;
  wallets: ReadonlyArray<DiscoveredWalletV1>;
  refusals: ReadonlyArray<RefusedWalletV1>;
  reason: string;
}>;

function plain(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function boundedName(value: unknown): string {
  if (typeof value !== 'string') throw new Error('wallet name is not text');
  const name = value.trim();
  if (name.length === 0 || name.length > MAX_NAME_BYTES) throw new Error(`wallet name must be 1..${MAX_NAME_BYTES} characters`);
  if (/[\u0000-\u001f\u007f]/.test(name)) throw new Error('wallet name carries control characters');
  return name;
}

function boundedIcon(value: unknown): string | null {
  if (typeof value !== 'string' || value.length === 0 || value.length > MAX_ICON_BYTES) return null;
  return ICON_PREFIX.test(value) ? value : null;
}

function identifiers(value: unknown, field: string, maximum: number): ReadonlyArray<string> {
  if (!Array.isArray(value)) throw new Error(`${field} is not an array`);
  if (value.length > maximum) throw new Error(`${field} announces more than ${maximum} entries`);
  const entries = value.map((entry) => {
    if (typeof entry !== 'string' || entry.length === 0 || entry.length > 128) throw new Error(`${field} contains a non-identifier entry`);
    return entry;
  });
  if (new Set(entries).size !== entries.length) throw new Error(`${field} repeats an identifier`);
  return Object.freeze(entries);
}

function canonicalAddress(value: unknown, field: string): string {
  if (typeof value !== 'string') throw new Error(`${field} is not text`);
  let canonical: string;
  try {
    canonical = new PublicKey(value).toBase58();
  } catch {
    throw new Error(`${field} is not one canonical Solana address`);
  }
  if (canonical !== value) throw new Error(`${field} is not canonical base58 text`);
  return canonical;
}

function decodeAccount(value: unknown, index: number): WalletStandardAccountV1 {
  if (!plain(value)) throw new Error(`wallet account ${index} is not an object`);
  const label = typeof value.label === 'string' && value.label.length > 0 && value.label.length <= MAX_NAME_BYTES ? value.label : null;
  const address = canonicalAddress(value.address, `wallet account ${index} address`);
  // A Wallet Standard account carries both an address and its raw public key.
  // They are two statements of one identity, so they must agree.
  if (value.publicKey !== undefined) {
    const raw = value.publicKey;
    if (!(raw instanceof Uint8Array) || raw.length !== 32) throw new Error(`wallet account ${index} public key is not 32 raw bytes`);
    if (new PublicKey(raw).toBase58() !== address) throw new Error(`wallet account ${index} public key does not encode its own address`);
  }
  return Object.freeze({
    address,
    label,
    chains: identifiers(value.chains, `wallet account ${index} chains`, 32),
    features: identifiers(value.features, `wallet account ${index} features`, 32),
  });
}

/** Project one announced Wallet Standard registration, or explain the refusal. */
export function describeWalletV1(candidate: unknown): DiscoveredWalletV1 | RefusedWalletV1 {
  let name = 'unnamed registration';
  try {
    if (!plain(candidate)) throw new Error('registration is not an object');
    name = boundedName(candidate.name);
    if (candidate.version !== WALLET_STANDARD_VERSION) {
      throw new Error(`Wallet Standard version ${String(candidate.version)} is not the supported ${WALLET_STANDARD_VERSION}`);
    }
    const chains = identifiers(candidate.chains, 'wallet chains', 64);
    if (!plain(candidate.features)) throw new Error('wallet features is not an object');
    const features = Object.freeze(Object.keys(candidate.features).sort());
    const rawAccounts = Array.isArray(candidate.accounts) ? candidate.accounts : [];
    if (rawAccounts.length > MAX_LISTED_ACCOUNTS) throw new Error(`wallet announces more than ${MAX_LISTED_ACCOUNTS} accounts`);
    const accounts = Object.freeze(rawAccounts.map((account, index) => decodeAccount(account, index)));
    const solanaChains = Object.freeze(SOLANA_CHAINS.filter((chain) => chains.includes(chain)));
    if (solanaChains.length === 0) throw new Error('wallet announces no solana: chain; it is not a Solana Wallet Standard wallet');
    if (!features.includes(STANDARD_CONNECT)) throw new Error(`wallet does not expose ${STANDARD_CONNECT}`);
    if (typeof (candidate.features as Record<string, unknown>)[STANDARD_CONNECT] !== 'object') throw new Error(`${STANDARD_CONNECT} is not a feature object`);
    return Object.freeze({
      id: `${name} ${String(candidate.version)}`,
      name,
      version: WALLET_STANDARD_VERSION,
      icon: boundedIcon(candidate.icon),
      chains,
      features,
      solanaChains,
      accounts,
      canConnect: true,
      canDisconnect: features.includes(STANDARD_DISCONNECT),
      canSignTransaction: features.includes(SOLANA_SIGN_TRANSACTION),
      canSignMessage: features.includes(SOLANA_SIGN_MESSAGE),
      announcesSignAndSend: features.includes(SOLANA_SIGN_AND_SEND_TRANSACTION),
    });
  } catch (error) {
    return Object.freeze({ id: `${name} refused`, name, reason: error instanceof Error ? error.message : 'registration refused without a usable reason' });
  }
}

function isDiscovered(value: DiscoveredWalletV1 | RefusedWalletV1): value is DiscoveredWalletV1 {
  return 'canConnect' in value;
}

const NO_REGISTRY: WalletDiscoveryV1 = Object.freeze({
  registryPresent: false,
  wallets: Object.freeze([]),
  refusals: Object.freeze([]),
  reason: 'No Wallet Standard registry exists in this runtime. Browser wallet discovery is unavailable here.',
});

/**
 * Project one announced registration list into an honest listing.
 *
 * `null` means no registry exists in this runtime at all, which is a different
 * statement from a registry that answered with nothing.
 */
export function projectAnnouncedWalletsV1(announced: ReadonlyArray<unknown> | null): WalletDiscoveryV1 {
  if (announced === null) return NO_REGISTRY;
  if (!Array.isArray(announced)) {
    return Object.freeze({
      registryPresent: true,
      wallets: Object.freeze([]),
      refusals: Object.freeze([]),
      reason: 'The Wallet Standard registry did not return an array of registrations.',
    });
  }
  const bounded = announced.slice(0, MAX_LISTED_WALLETS);
  const projections = bounded.map((entry) => describeWalletV1(entry));
  const wallets = Object.freeze(projections.filter(isDiscovered));
  const refusals = Object.freeze(projections.filter((entry): entry is RefusedWalletV1 => !isDiscovered(entry)));
  const overflow = announced.length - bounded.length;
  const reason = wallets.length > 0
    ? `${wallets.length} conforming Solana wallet${wallets.length === 1 ? '' : 's'} announced${refusals.length > 0 ? `; ${refusals.length} registration${refusals.length === 1 ? '' : 's'} refused` : ''}${overflow > 0 ? `; ${overflow} beyond the ${MAX_LISTED_WALLETS}-wallet listing bound were not read` : ''}.`
    : announced.length === 0
      ? 'A Wallet Standard registry is present but no browser wallet has registered. Install a conforming Solana wallet extension.'
      : `No announced registration conforms to the Solana Wallet Standard; ${refusals.length} were refused.`;
  return Object.freeze({ registryPresent: true, wallets, refusals, reason });
}

/** Read one registry and project it. Enumeration failure is stated, not hidden. */
export function discoverWalletsV1(registry: WalletStandardRegistryV1 | null): WalletDiscoveryV1 {
  if (registry === null) return NO_REGISTRY;
  try {
    return projectAnnouncedWalletsV1(registry.get());
  } catch (error) {
    return Object.freeze({
      registryPresent: true,
      wallets: Object.freeze([]),
      refusals: Object.freeze([]),
      reason: `The Wallet Standard registry refused enumeration: ${error instanceof Error ? error.message : 'no usable reason'}`,
    });
  }
}

/**
 * Subscribe to one wallet's own `standard:events` change notifications.
 *
 * A wallet may switch or withdraw accounts after connecting; without this the
 * displayed identity would silently go stale. The listener is told the wallet's
 * current first authorized Solana account, or `null` when it has none.
 */
export function subscribeWalletAccountsV1(
  candidate: unknown,
  listener: (account: Readonly<{ address: string; label: string | null }> | null) => void,
): () => void {
  if (!plain(candidate) || !plain(candidate.features)) return () => undefined;
  const events = (candidate.features as FeatureBag)[STANDARD_EVENTS];
  if (!plain(events) || typeof events.on !== 'function') return () => undefined;
  const on = events.on.bind(events) as (event: string, handler: (properties: unknown) => void) => unknown;
  const off = on('change', (properties: unknown) => {
    if (!plain(properties) || !('accounts' in properties)) return;
    const accounts = Array.isArray(properties.accounts) ? properties.accounts : [];
    if (accounts.length === 0) { listener(null); return; }
    try {
      const account = decodeAccount(accounts[0], 0);
      listener(Object.freeze({ address: account.address, label: account.label }));
    } catch {
      listener(null);
    }
  });
  return typeof off === 'function' ? (off as () => void) : () => undefined;
}

/**
 * Find the announced registration behind one listed wallet id.
 *
 * The listing is a projection; the object a connect request is sent to is
 * always re-read from the registry, so a wallet that unregistered between the
 * listing and the click is a refusal rather than a stale handle.
 */
export function findAnnouncedWalletV1(registry: WalletStandardRegistryV1 | null, walletId: string): unknown {
  if (registry === null) return null;
  let announced: ReadonlyArray<unknown>;
  try {
    announced = registry.get();
  } catch {
    return null;
  }
  if (!Array.isArray(announced)) return null;
  for (const candidate of announced.slice(0, MAX_LISTED_WALLETS)) {
    const projection = describeWalletV1(candidate);
    if (isDiscovered(projection) && projection.id === walletId) return candidate;
  }
  return null;
}

/**
 * Obtain the browser's Wallet Standard registry.
 *
 * `getWallets()` owns the `wallet-standard:app-ready` / `register-wallet`
 * handshake so a wallet is registered whether it loads before or after this
 * app. It answers even with no `window`, so SSR and the test runtime are
 * reported as "no registry in this runtime" rather than as an empty browser.
 */
export function browserWalletRegistryV1(): WalletStandardRegistryV1 | null {
  if (typeof window === 'undefined') return null;
  return getWallets();
}

export type WalletConnectionStateV1 =
  | Readonly<{ kind: 'unsupported'; discovery: WalletDiscoveryV1; message: string }>
  | Readonly<{ kind: 'empty'; discovery: WalletDiscoveryV1; message: string }>
  | Readonly<{ kind: 'discovered'; discovery: WalletDiscoveryV1; message: string }>
  | Readonly<{ kind: 'connecting'; discovery: WalletDiscoveryV1; walletId: string; message: string }>
  | Readonly<{ kind: 'connected'; discovery: WalletDiscoveryV1; walletId: string; address: string; label: string | null; message: string }>
  | Readonly<{ kind: 'refused'; discovery: WalletDiscoveryV1; walletId: string | null; message: string }>;

/**
 * What this surface has asked one wallet for, independent of what is currently
 * registered. Intent never contains a signature or any authority: identity is
 * the only thing it can hold.
 */
export type WalletConnectionIntentV1 =
  | Readonly<{ kind: 'idle' }>
  | Readonly<{ kind: 'connecting'; walletId: string }>
  | Readonly<{ kind: 'connected'; walletId: string; address: string; label: string | null; switched: boolean }>
  | Readonly<{ kind: 'refused'; walletId: string | null; reason: string }>;

export const WALLET_CONNECTION_IDLE_V1: WalletConnectionIntentV1 = Object.freeze({ kind: 'idle' });

export type WalletConnectionEventV1 =
  | Readonly<{ kind: 'connect-requested'; walletId: string }>
  | Readonly<{ kind: 'connect-succeeded'; walletId: string; address: string; label: string | null }>
  | Readonly<{ kind: 'connect-refused'; walletId: string; reason: string }>
  | Readonly<{ kind: 'account-changed'; walletId: string; address: string | null; label: string | null }>
  | Readonly<{ kind: 'forgotten' }>;

/**
 * One pure transition of the connect intent.
 *
 * A wallet cannot answer a request this surface did not make, and no transition
 * reaches a signing state.
 */
export function walletConnectionTransitionV1(intent: WalletConnectionIntentV1, event: WalletConnectionEventV1): WalletConnectionIntentV1 {
  if (event.kind === 'forgotten') return WALLET_CONNECTION_IDLE_V1;
  if (event.kind === 'connect-requested') return Object.freeze({ kind: 'connecting', walletId: event.walletId });
  if (event.kind === 'account-changed') {
    if (intent.kind !== 'connected' || intent.walletId !== event.walletId) return intent;
    if (event.address === null) {
      return Object.freeze({ kind: 'refused', walletId: intent.walletId, reason: 'the wallet withdrew every authorized account' });
    }
    if (event.address === intent.address) return intent;
    return Object.freeze({ kind: 'connected', walletId: intent.walletId, address: event.address, label: event.label, switched: true });
  }
  if (intent.kind !== 'connecting' || intent.walletId !== event.walletId) {
    return Object.freeze({ kind: 'refused', walletId: event.walletId, reason: 'a wallet answered an identity request this surface did not make' });
  }
  if (event.kind === 'connect-refused') return Object.freeze({ kind: 'refused', walletId: event.walletId, reason: event.reason });
  return Object.freeze({ kind: 'connected', walletId: event.walletId, address: event.address, label: event.label, switched: false });
}

function walletName(discovery: WalletDiscoveryV1, walletId: string): string {
  return discovery.wallets.find((wallet) => wallet.id === walletId)?.name ?? walletId;
}

function restState(discovery: WalletDiscoveryV1): WalletConnectionStateV1 {
  if (!discovery.registryPresent) return Object.freeze({ kind: 'unsupported', discovery, message: discovery.reason });
  if (discovery.wallets.length === 0) return Object.freeze({ kind: 'empty', discovery, message: discovery.reason });
  return Object.freeze({ kind: 'discovered', discovery, message: discovery.reason });
}

/**
 * What one discovery listing and one connect intent mean together.
 *
 * This is a projection, not stored state, so a registry that changes under a
 * connected identity cannot leave a stale address on screen: a wallet that has
 * unregistered is reported as a refusal on the next render.
 */
export function projectWalletConnectionV1(discovery: WalletDiscoveryV1, intent: WalletConnectionIntentV1): WalletConnectionStateV1 {
  if (intent.kind === 'idle') return restState(discovery);
  if (intent.kind === 'refused') {
    return Object.freeze({ kind: 'refused', discovery, walletId: intent.walletId, message: `Refused: ${intent.reason}` });
  }
  const registered = discovery.wallets.some((wallet) => wallet.id === intent.walletId);
  if (intent.kind === 'connecting') {
    if (!registered) return Object.freeze({ kind: 'refused', discovery, walletId: intent.walletId, message: 'Refused: the selected wallet is not in the current registry listing.' });
    return Object.freeze({ kind: 'connecting', discovery, walletId: intent.walletId, message: `Requesting identity from ${walletName(discovery, intent.walletId)}; no signature is requested.` });
  }
  if (!registered) {
    return Object.freeze({ kind: 'refused', discovery, walletId: intent.walletId, message: `Refused: ${walletName(discovery, intent.walletId)} unregistered itself; the connected identity was dropped.` });
  }
  return Object.freeze({
    kind: 'connected',
    discovery,
    walletId: intent.walletId,
    address: intent.address,
    label: intent.label,
    message: intent.switched
      ? `${intent.address} · identity only; the wallet switched accounts`
      : `${intent.address} · identity only; no signature requested`,
  });
}

type FeatureBag = Readonly<Record<string, unknown>>;

function features(wallet: unknown, field: string): FeatureBag {
  if (!plain(wallet) || !plain(wallet.features)) throw new Error(`${field} exposes no Wallet Standard feature set`);
  return wallet.features as FeatureBag;
}

function feature(wallet: unknown, name: string): Record<string, unknown> {
  const bag = features(wallet, 'wallet');
  const entry = bag[name];
  if (!plain(entry)) throw new Error(`wallet does not expose ${name}`);
  return entry;
}

function method(wallet: unknown, name: string, key: string): (...args: ReadonlyArray<unknown>) => Promise<unknown> {
  const entry = feature(wallet, name);
  const callable = entry[key];
  if (typeof callable !== 'function') throw new Error(`${name} does not expose a callable ${key}`);
  return callable.bind(entry) as (...args: ReadonlyArray<unknown>) => Promise<unknown>;
}

/**
 * Request identity from one wallet. `silent` asks the wallet to answer only
 * from an already authorized session; nothing here ever requests a signature.
 */
export async function connectWalletIdentityV1(
  candidate: unknown,
  options: Readonly<{ silent?: boolean }> = {},
): Promise<Readonly<{ address: string; label: string | null; chains: ReadonlyArray<string> }>> {
  const projection = describeWalletV1(candidate);
  if (!isDiscovered(projection)) throw new Error(projection.reason);
  const connect = method(candidate, STANDARD_CONNECT, 'connect');
  const answer = await connect(options.silent === true ? { silent: true } : {});
  const announced = plain(answer) && Array.isArray(answer.accounts)
    ? answer.accounts
    : (plain(candidate) && Array.isArray(candidate.accounts) ? candidate.accounts : []);
  if (announced.length === 0) throw new Error('wallet connected without authorizing one account');
  const account = decodeAccount(announced[0], 0);
  const solana = SOLANA_CHAINS.filter((chain) => account.chains.includes(chain));
  if (solana.length === 0) throw new Error('authorized account announces no solana: chain');
  return Object.freeze({ address: account.address, label: account.label, chains: Object.freeze(solana) });
}

/** Pick the wallet chain that matches a finalized RPC endpoint, or refuse. */
export function solanaChainForEndpointV1(endpoint: string, available: ReadonlyArray<string>): SolanaChainV1 {
  let host: string;
  try {
    host = new URL(endpoint).hostname;
  } catch {
    throw new Error('RPC endpoint is not one absolute http or https URL');
  }
  const loopback = host === 'localhost' || host === '127.0.0.1' || host === '::1' || host === '0.0.0.0';
  const preference: ReadonlyArray<SolanaChainV1> = loopback
    ? ['solana:localnet', 'solana:devnet', 'solana:testnet', 'solana:mainnet']
    : ['solana:mainnet', 'solana:devnet', 'solana:testnet', 'solana:localnet'];
  const chain = preference.find((candidate) => available.includes(candidate));
  if (chain === undefined) throw new Error(`wallet announces none of ${SOLANA_CHAINS.join(', ')} for endpoint ${endpoint}`);
  return chain;
}

export type WalletHandoffV1 = Readonly<{
  publicKey: Readonly<{ toBase58(): string }>;
  connect(): Promise<void>;
  signTransaction?(transaction: VersionedTransaction): Promise<VersionedTransaction>;
  signMessage?(message: Uint8Array): Promise<Uint8Array>;
}>;

function accountHandle(candidate: unknown, address: string): unknown {
  if (!plain(candidate) || !Array.isArray(candidate.accounts)) throw new Error('wallet exposes no authorized account list');
  const account = candidate.accounts.find((entry) => plain(entry) && entry.address === address);
  if (account === undefined) throw new Error(`wallet has not authorized ${address}`);
  return account;
}

function firstResult(value: unknown, field: string): Record<string, unknown> {
  if (!Array.isArray(value) || value.length !== 1 || !plain(value[0])) throw new Error(`${field} did not return exactly one output`);
  return value[0];
}

function exactBytes(value: unknown, field: string): Uint8Array {
  if (!(value instanceof Uint8Array) || value.length === 0) throw new Error(`${field} is not exact bytes`);
  return new Uint8Array(value);
}

/**
 * Adapt one Wallet Standard wallet to the injected shape `walletHandoff`
 * already hostile-checks. This adapter adds no authority: it forwards a
 * user-initiated request and re-serializes the wallet's answer so
 * `walletHandoff` can reject a rewritten message or an unexpected signature.
 */
export function walletStandardHandoffV1(candidate: unknown, address: string, chain: SolanaChainV1): WalletHandoffV1 {
  const canonical = canonicalAddress(address, 'handoff address');
  const bag = features(candidate, 'wallet');
  const handoff: {
    publicKey: Readonly<{ toBase58(): string }>;
    connect(): Promise<void>;
    signTransaction?(transaction: VersionedTransaction): Promise<VersionedTransaction>;
    signMessage?(message: Uint8Array): Promise<Uint8Array>;
  } = {
    publicKey: Object.freeze({ toBase58: () => canonical }),
    connect: async () => { accountHandle(candidate, canonical); },
  };
  if (plain(bag[SOLANA_SIGN_TRANSACTION])) {
    handoff.signTransaction = async (transaction: VersionedTransaction): Promise<VersionedTransaction> => {
      const account = accountHandle(candidate, canonical);
      const sign = method(candidate, SOLANA_SIGN_TRANSACTION, 'signTransaction');
      const output = firstResult(await sign({ account, transaction: transaction.serialize(), chain }), SOLANA_SIGN_TRANSACTION);
      return VersionedTransaction.deserialize(exactBytes(output.signedTransaction, `${SOLANA_SIGN_TRANSACTION} signedTransaction`));
    };
  }
  if (plain(bag[SOLANA_SIGN_MESSAGE])) {
    handoff.signMessage = async (message: Uint8Array): Promise<Uint8Array> => {
      const account = accountHandle(candidate, canonical);
      const sign = method(candidate, SOLANA_SIGN_MESSAGE, 'signMessage');
      const output = firstResult(await sign({ account, message: new Uint8Array(message) }), SOLANA_SIGN_MESSAGE);
      return exactBytes(output.signature, `${SOLANA_SIGN_MESSAGE} signature`);
    };
  }
  return Object.freeze(handoff);
}
