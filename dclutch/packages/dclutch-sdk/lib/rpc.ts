import { PublicKey, VersionedTransaction } from '@solana/web3.js';

import { decodeBase64 } from './bytes';
import {
  AccountProjection,
  classifyHeader,
  crossCheckBindings,
  decodeCoreAccount,
  FullAccountObservation,
  verifyLocalBindings,
} from './decoders';

const MAX_RPC_RESPONSE_BYTES = 4 * 1024 * 1024;
const MAX_PROGRAM_ACCOUNTS = 256;
const MAX_REACQUIRED_ACCOUNTS = 128;
const MAX_MULTIPLE_ACCOUNTS = 32;
const SOLANA_PACKET_BYTES = 1_232;
const RPC_TIMEOUT_MS = 15_000;

export type RpcAccount = Readonly<{
  data: Uint8Array;
  executable: boolean;
  lamports: string;
  owner: string;
  space: number;
}>;

export type AccountInfoObservation = Readonly<{
  slot: string;
  account: RpcAccount | null;
}>;

export type MultipleAccountObservation = Readonly<{
  slot: string;
  accounts: ReadonlyArray<Readonly<{ address: string; account: RpcAccount | null }>>;
}>;

type HeaderObservation = Readonly<{
  address: string;
  account: RpcAccount;
}>;

export type ConnectionFacts = Readonly<{
  endpoint: string;
  genesisHash: string;
  solanaCore: string;
  featureSet: string | null;
}>;

export type LatestBlockhashObservation = Readonly<{
  slot: string;
  blockhash: string;
  lastValidBlockHeight: string;
}>;

export type RentExemptionObservation = Readonly<{
  dataLength: number;
  lamports: string;
}>;

export type SignatureRecordObservation = Readonly<{
  signature: string;
  slot: string;
  succeeded: boolean;
  errorText: string | null;
  blockTime: string | null;
  memo: string | null;
}>;

export type SignatureStatusObservation = Readonly<{
  signature: string;
  known: boolean;
  slot: string | null;
  confirmationStatus: string | null;
  succeeded: boolean | null;
  errorText: string | null;
}>;

export type TransactionMetaObservation = Readonly<{
  signature: string;
  slot: string;
  blockTime: string | null;
  succeeded: boolean;
  errorText: string | null;
  feeLamports: string;
  accountAddresses: ReadonlyArray<string>;
  preBalances: ReadonlyArray<string>;
  postBalances: ReadonlyArray<string>;
  logMessages: ReadonlyArray<string>;
  transactionBytes: Uint8Array;
}>;

export type ProgramSnapshot = Readonly<{
  programId: string;
  scanSlot: string;
  totalAccounts: string;
  decodedAccounts: string;
  refusedAccounts: string;
  projections: ReadonlyArray<AccountProjection>;
}>;

function exactUnsigned(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new Error(`${field} is not an exact safe unsigned integer`);
  return value;
}

function exactText(value: unknown, field: string, maximum = 512): string {
  if (typeof value !== 'string' || value.trim() !== value || value.length === 0 || value.length > maximum) throw new Error(`${field} is not bounded canonical text`);
  return value;
}

function plain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

async function boundedJson(response: Response, maximumBytes = MAX_RPC_RESPONSE_BYTES): Promise<unknown> {
  if (!response.ok) throw new Error(`RPC HTTP status ${response.status}`);
  const declared = response.headers.get('content-length');
  if (declared !== null && exactUnsigned(Number(declared), 'RPC Content-Length') > maximumBytes) throw new Error('RPC response exceeds the browser byte bound');
  const reader = response.body?.getReader();
  if (!reader) {
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (bytes.byteLength > maximumBytes) throw new Error('RPC response exceeds the browser byte bound');
    return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
  }
  const chunks: Uint8Array[] = [];
  let length = 0;
  for (;;) {
    const next = await reader.read();
    if (next.done) break;
    length += next.value.byteLength;
    if (length > maximumBytes) {
      await reader.cancel();
      throw new Error('RPC response exceeds the browser byte bound');
    }
    chunks.push(next.value);
  }
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
}

function parseAccount(value: unknown, field: string): RpcAccount {
  if (!plain(value)) throw new Error(`${field} is not an RPC account object`);
  const owner = exactText(value.owner, `${field}.owner`, 64);
  new PublicKey(owner);
  if (typeof value.executable !== 'boolean') throw new Error(`${field}.executable is not boolean`);
  const lamports = exactUnsigned(value.lamports, `${field}.lamports`);
  const data = decodeBase64(value.data, `${field}.data`);
  const space = value.space === undefined ? data.length : exactUnsigned(value.space, `${field}.space`);
  return Object.freeze({ data, executable: value.executable, lamports: String(lamports), owner, space });
}

/**
 * The ambient `fetch`, called the way a browser requires.
 *
 * `fetch` is a `Window` method and Chromium enforces its receiver: storing the
 * bare function on an instance and calling `this.fetcher(...)` invokes it with
 * the client as `this` and every request dies with
 * `Failed to execute 'fetch' on 'Window': Illegal invocation`. Node, jsdom and
 * every injected test double are lenient about the receiver, so the defect is
 * invisible to unit tests and fatal in the product. Read the ambient binding at
 * call time rather than capturing it at construction, so a test that replaces
 * `globalThis.fetch` afterwards is still the function that runs.
 */
const ambientFetch: typeof fetch = (input, init) => globalThis.fetch(input, init);

/**
 * The one place browser and node `fetch` typings disagree on an option this
 * client uses: undici's `RequestInit` (the node types) omits `cache`, which
 * browsers honor and this client relies on to keep an RPC read from being
 * served stale. Widen locally so the same literal typechecks under both the
 * DOM lib and the node lib; node's fetch ignores the property at runtime.
 */
type PortableRequestInit = RequestInit & { cache?: 'no-store' };

export class SolanaRpcClient {
  readonly endpoint: string;
  private requestId = 0;

  constructor(endpoint: string, private readonly fetcher: typeof fetch = ambientFetch) {
    const url = new URL(endpoint);
    if (url.protocol !== 'http:' && url.protocol !== 'https:') throw new Error('RPC endpoint must use http or https');
    this.endpoint = url.toString();
  }

  private async request(method: string, params: ReadonlyArray<unknown>): Promise<unknown> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), RPC_TIMEOUT_MS);
    try {
      const init: PortableRequestInit = {
        method: 'POST',
        mode: 'cors',
        credentials: 'omit',
        cache: 'no-store',
        redirect: 'error',
        referrerPolicy: 'no-referrer',
        headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: ++this.requestId, method, params }),
        signal: controller.signal,
      };
      const response = await this.fetcher(this.endpoint, init);
      const payload = await boundedJson(response);
      if (!plain(payload) || payload.jsonrpc !== '2.0') throw new Error(`${method} returned an invalid JSON-RPC envelope`);
      if (payload.error !== undefined) {
        const message = plain(payload.error) && typeof payload.error.message === 'string' ? payload.error.message.slice(0, 240) : 'unknown RPC refusal';
        throw new Error(`${method} refused: ${message}`);
      }
      if (!('result' in payload)) throw new Error(`${method} omitted its result`);
      return payload.result;
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') throw new Error(`${method} timed out after ${RPC_TIMEOUT_MS / 1000} seconds`);
      throw error;
    } finally {
      clearTimeout(timeout);
    }
  }

  async probe(): Promise<ConnectionFacts> {
    const [versionRaw, genesisRaw] = await Promise.all([this.request('getVersion', []), this.request('getGenesisHash', [])]);
    if (!plain(versionRaw)) throw new Error('getVersion returned an invalid result');
    return Object.freeze({
      endpoint: this.endpoint,
      genesisHash: exactText(genesisRaw, 'genesis hash', 96),
      solanaCore: exactText(versionRaw['solana-core'], 'solana-core version', 64),
      featureSet: versionRaw['feature-set'] === undefined ? null : String(exactUnsigned(versionRaw['feature-set'], 'feature set')),
    });
  }

  async programHeaders(programId: string): Promise<Readonly<{ slot: string; accounts: ReadonlyArray<HeaderObservation> }>> {
    new PublicKey(programId);
    const raw = await this.request('getProgramAccounts', [programId, {
      commitment: 'finalized',
      encoding: 'base64',
      withContext: true,
      dataSlice: { offset: 0, length: 16 },
    }]);
    if (!plain(raw) || !plain(raw.context) || !Array.isArray(raw.value)) throw new Error('getProgramAccounts did not return a finalized context and account array');
    if (raw.value.length > MAX_PROGRAM_ACCOUNTS) throw new Error(`program scan found ${raw.value.length} accounts, above the explicit ${MAX_PROGRAM_ACCOUNTS}-account browser bound`);
    const accounts = raw.value.map((entry, index) => {
      if (!plain(entry)) throw new Error(`program account ${index} is malformed`);
      const address = exactText(entry.pubkey, `program account ${index} address`, 64);
      new PublicKey(address);
      return Object.freeze({ address, account: parseAccount(entry.account, `program account ${index}`) });
    });
    if (new Set(accounts.map((account) => account.address)).size !== accounts.length) throw new Error('program scan repeated an account address');
    return Object.freeze({ slot: String(exactUnsigned(raw.context.slot, 'program scan slot')), accounts: Object.freeze(accounts) });
  }

  async accountInfo(address: string, minimumContextSlot?: string): Promise<AccountInfoObservation> {
    new PublicKey(address);
    const configuration: Record<string, unknown> = { commitment: 'finalized', encoding: 'base64' };
    if (minimumContextSlot !== undefined) configuration.minContextSlot = exactUnsigned(Number(minimumContextSlot), 'minimum context slot');
    const raw = await this.request('getAccountInfo', [address, configuration]);
    if (!plain(raw) || !plain(raw.context) || !('value' in raw)) throw new Error('getAccountInfo did not return a finalized context');
    return Object.freeze({
      slot: String(exactUnsigned(raw.context.slot, 'account observation slot')),
      account: raw.value === null ? null : parseAccount(raw.value, 'account observation'),
    });
  }

  async multipleAccounts(addresses: ReadonlyArray<string>, minimumContextSlot?: string): Promise<MultipleAccountObservation> {
    if (addresses.length === 0 || addresses.length > MAX_MULTIPLE_ACCOUNTS) throw new Error(`getMultipleAccounts requires 1..${MAX_MULTIPLE_ACCOUNTS} exact addresses`);
    const canonical = addresses.map((address) => new PublicKey(address).toBase58());
    if (canonical.some((address, index) => address !== addresses[index])) throw new Error('getMultipleAccounts addresses must be canonical base58 text');
    if (new Set(canonical).size !== canonical.length) throw new Error('getMultipleAccounts addresses must be distinct');
    const configuration: Record<string, unknown> = { commitment: 'finalized', encoding: 'base64' };
    if (minimumContextSlot !== undefined) configuration.minContextSlot = exactUnsigned(Number(minimumContextSlot), 'minimum context slot');
    const raw = await this.request('getMultipleAccounts', [canonical, configuration]);
    if (!plain(raw) || !plain(raw.context) || !Array.isArray(raw.value) || raw.value.length !== canonical.length) throw new Error('getMultipleAccounts did not return one finalized value per address');
    const slot = String(exactUnsigned(raw.context.slot, 'multiple-account observation slot'));
    // Bound the narrowed array to a const: property narrowing does not survive
    // into the map callback, and the guard above is the only authority for it.
    const values: ReadonlyArray<unknown> = raw.value;
    return Object.freeze({
      slot,
      accounts: Object.freeze(canonical.map((address, index) => Object.freeze({ address, account: values[index] === null ? null : parseAccount(values[index], `multiple account ${index}`) }))),
    });
  }

  async finalizedSlot(): Promise<string> {
    return String(exactUnsigned(await this.request('getSlot', [{ commitment: 'finalized' }]), 'finalized slot'));
  }

  async latestBlockhash(minimumContextSlot?: string): Promise<LatestBlockhashObservation> {
    const configuration: Record<string, unknown> = { commitment: 'finalized' };
    if (minimumContextSlot !== undefined) configuration.minContextSlot = exactUnsigned(Number(minimumContextSlot), 'minimum context slot');
    const raw = await this.request('getLatestBlockhash', [configuration]);
    if (!plain(raw) || !plain(raw.context) || !plain(raw.value)) throw new Error('getLatestBlockhash did not return a finalized context and value');
    const blockhash = exactText(raw.value.blockhash, 'recent blockhash', 64);
    new PublicKey(blockhash);
    return Object.freeze({
      slot: String(exactUnsigned(raw.context.slot, 'blockhash context slot')),
      blockhash,
      lastValidBlockHeight: String(exactUnsigned(raw.value.lastValidBlockHeight, 'last valid block height')),
    });
  }

  async minimumBalanceForRentExemption(dataLength: number): Promise<RentExemptionObservation> {
    if (!Number.isSafeInteger(dataLength) || dataLength < 0 || dataLength > 10_485_760) throw new Error('rent data length is outside the bounded account profile');
    const lamports = exactUnsigned(await this.request('getMinimumBalanceForRentExemption', [dataLength, { commitment: 'finalized' }]), 'rent-exempt lamports');
    return Object.freeze({ dataLength, lamports: String(lamports) });
  }

  /**
   * The RPC node's own signature index for one address, newest first.
   *
   * This is the node's transaction history, not a protocol index: a node
   * configured without history answers with an empty list, and nothing here
   * pretends otherwise. Callers state that provenance on the surface.
   */
  async signaturesForAddress(address: string, limit: number): Promise<ReadonlyArray<SignatureRecordObservation>> {
    new PublicKey(address);
    if (!Number.isInteger(limit) || limit < 1 || limit > 50) throw new Error('signature listing limit must be 1..50');
    const raw = await this.request('getSignaturesForAddress', [address, { commitment: 'finalized', limit }]);
    if (!Array.isArray(raw)) throw new Error('getSignaturesForAddress did not return an array');
    return Object.freeze(raw.map((entry, index) => {
      if (!plain(entry)) throw new Error(`signature record ${index} is malformed`);
      const signature = exactText(entry.signature, `signature record ${index}`, 96);
      if (!/^[1-9A-HJ-NP-Za-km-z]{64,88}$/.test(signature)) throw new Error(`signature record ${index} is not canonical base58 text`);
      return Object.freeze({
        signature,
        slot: String(exactUnsigned(entry.slot, `signature record ${index} slot`)),
        succeeded: entry.err === null || entry.err === undefined,
        errorText: entry.err === null || entry.err === undefined ? null : JSON.stringify(entry.err).slice(0, 240),
        blockTime: typeof entry.blockTime === 'number' && Number.isSafeInteger(entry.blockTime) ? String(entry.blockTime) : null,
        memo: typeof entry.memo === 'string' ? entry.memo.slice(0, 240) : null,
      });
    }));
  }

  /** Poll the status of explicitly named signatures, including node history. */
  async signatureStatuses(signatures: ReadonlyArray<string>): Promise<ReadonlyArray<SignatureStatusObservation>> {
    if (signatures.length === 0 || signatures.length > 16) throw new Error('signature status polling requires 1..16 exact signatures');
    for (const signature of signatures) {
      if (!/^[1-9A-HJ-NP-Za-km-z]{64,88}$/.test(signature)) throw new Error('signature status polling requires canonical base58 signatures');
    }
    const raw = await this.request('getSignatureStatuses', [signatures, { searchTransactionHistory: true }]);
    if (!plain(raw) || !Array.isArray(raw.value) || raw.value.length !== signatures.length) {
      throw new Error('getSignatureStatuses did not return one status per signature');
    }
    // Same const-binding as getMultipleAccounts: the closure cannot see the
    // Array.isArray narrowing on a property access.
    const statuses: ReadonlyArray<unknown> = raw.value;
    return Object.freeze(signatures.map((signature, index) => {
      const entry = statuses[index];
      if (entry === null || entry === undefined) {
        return Object.freeze({ signature, known: false, slot: null, confirmationStatus: null, succeeded: null, errorText: null });
      }
      if (!plain(entry)) throw new Error(`signature status ${index} is malformed`);
      return Object.freeze({
        signature,
        known: true,
        slot: String(exactUnsigned(entry.slot, `signature status ${index} slot`)),
        confirmationStatus: typeof entry.confirmationStatus === 'string' ? entry.confirmationStatus.slice(0, 32) : null,
        succeeded: entry.err === null || entry.err === undefined,
        errorText: entry.err === null || entry.err === undefined ? null : JSON.stringify(entry.err).slice(0, 240),
      });
    }));
  }

  /** Read one finalized transaction with its balance movements and logs. */
  async transaction(signature: string): Promise<TransactionMetaObservation | null> {
    if (!/^[1-9A-HJ-NP-Za-km-z]{64,88}$/.test(signature)) throw new Error('transaction read requires one canonical base58 signature');
    const raw = await this.request('getTransaction', [signature, {
      commitment: 'finalized',
      encoding: 'base64',
      maxSupportedTransactionVersion: 0,
    }]);
    if (raw === null) return null;
    if (!plain(raw) || !plain(raw.meta) || !Array.isArray(raw.transaction) || typeof raw.transaction[0] !== 'string') {
      throw new Error('getTransaction did not return base64 transaction bytes and meta');
    }
    const meta = raw.meta;
    const bytes = decodeBase64(raw.transaction, 'finalized transaction');
    if (!Array.isArray(meta.preBalances) || !Array.isArray(meta.postBalances) || meta.preBalances.length !== meta.postBalances.length) {
      throw new Error('getTransaction meta balances are malformed');
    }
    let accountAddresses: string[] = [];
    try {
      const decoded = VersionedTransaction.deserialize(bytes);
      const staticKeys = decoded.message.staticAccountKeys.map((accountKey) => accountKey.toBase58());
      const loaded = plain(meta.loadedAddresses) ? meta.loadedAddresses : {};
      const writable = Array.isArray(loaded.writable) ? loaded.writable.map((entry) => exactText(entry, 'loaded writable address', 64)) : [];
      const readonly = Array.isArray(loaded.readonly) ? loaded.readonly.map((entry) => exactText(entry, 'loaded readonly address', 64)) : [];
      accountAddresses = [...staticKeys, ...writable, ...readonly];
    } catch {
      accountAddresses = [];
    }
    return Object.freeze({
      signature,
      slot: String(exactUnsigned(raw.slot, 'transaction slot')),
      blockTime: typeof raw.blockTime === 'number' && Number.isSafeInteger(raw.blockTime) ? String(raw.blockTime) : null,
      succeeded: meta.err === null || meta.err === undefined,
      errorText: meta.err === null || meta.err === undefined ? null : JSON.stringify(meta.err).slice(0, 240),
      feeLamports: String(exactUnsigned(meta.fee, 'transaction fee')),
      accountAddresses: Object.freeze(accountAddresses),
      preBalances: Object.freeze(meta.preBalances.map((value, index) => String(exactUnsigned(value, `pre-balance ${index}`)))),
      postBalances: Object.freeze(meta.postBalances.map((value, index) => String(exactUnsigned(value, `post-balance ${index}`)))),
      logMessages: Object.freeze(Array.isArray(meta.logMessages)
        ? meta.logMessages.slice(0, 64).map((entry, index) => exactText(entry, `log message ${index}`, 512))
        : []),
      transactionBytes: bytes,
    });
  }

  /**
   * Submit one caller-signed packet after an explicit user action.
   *
   * This method never signs, mutates, retries in a loop, or skips preflight.
   */
  async sendRawTransaction(bytes: Uint8Array): Promise<string> {
    if (!(bytes instanceof Uint8Array) || bytes.length === 0 || bytes.length > SOLANA_PACKET_BYTES) {
      throw new Error(`signed transaction must contain 1..${SOLANA_PACKET_BYTES} bytes`);
    }
    let binary = '';
    for (const byte of bytes) binary += String.fromCharCode(byte);
    const result = exactText(await this.request('sendTransaction', [btoa(binary), {
      encoding: 'base64',
      skipPreflight: false,
      preflightCommitment: 'confirmed',
      maxRetries: 3,
    }]), 'transaction signature', 96);
    if (result.length < 64 || !/^[1-9A-HJ-NP-Za-km-z]+$/.test(result)) {
      throw new Error('sendTransaction returned a noncanonical base58 signature');
    }
    return result;
  }
}

async function concurrentMap<T, U>(values: ReadonlyArray<T>, limit: number, mapper: (value: T) => Promise<U>): Promise<U[]> {
  const output = new Array<U>(values.length);
  let next = 0;
  async function worker(): Promise<void> {
    for (;;) {
      const index = next++;
      if (index >= values.length) return;
      output[index] = await mapper(values[index]);
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, values.length) }, () => worker()));
  return output;
}

export async function scanProgram(client: SolanaRpcClient, programId: string): Promise<ProgramSnapshot> {
  const canonicalProgramId = new PublicKey(programId).toBase58();
  if (canonicalProgramId !== programId) throw new Error('program ID must be canonical base58 text');
  const scan = await client.programHeaders(canonicalProgramId);
  const recognized = scan.accounts.filter((entry) => classifyHeader(entry.account.data) !== null);
  if (recognized.length > MAX_REACQUIRED_ACCOUNTS) throw new Error(`scan found ${recognized.length} recognized accounts, above the explicit ${MAX_REACQUIRED_ACCOUNTS}-account reacquisition bound`);

  const recognizedAddresses = new Set(recognized.map((entry) => entry.address));
  const unknown: AccountProjection[] = scan.accounts.filter((entry) => !recognizedAddresses.has(entry.address)).map((entry) => decodeCoreAccount(Object.freeze({
    address: entry.address,
    owner: entry.account.owner,
    executable: entry.account.executable,
    lamports: entry.account.lamports,
    observedSlot: scan.slot,
    data: entry.account.data,
  }), canonicalProgramId));

  const reacquired = await concurrentMap(recognized, 4, async (entry): Promise<AccountProjection> => {
    const full = await client.accountInfo(entry.address, scan.slot);
    if (full.account === null) {
      return Object.freeze({ status: 'refused', kind: classifyHeader(entry.account.data) ?? 'Unknown', address: entry.address, lamports: entry.account.lamports, observedSlot: full.slot, reason: 'account disappeared after the finalized header scan', header: Array.from(entry.account.data, (byte) => byte.toString(16).padStart(2, '0')).join('') });
    }
    const observation: FullAccountObservation = Object.freeze({
      address: entry.address,
      owner: full.account.owner,
      executable: full.account.executable,
      lamports: full.account.lamports,
      observedSlot: full.slot,
      data: full.account.data,
    });
    const projection = decodeCoreAccount(observation, canonicalProgramId);
    return projection.status === 'decoded' ? verifyLocalBindings(projection, canonicalProgramId) : projection;
  });
  const projections = crossCheckBindings([...reacquired, ...unknown].sort((left, right) => left.address.localeCompare(right.address)));
  const decoded = projections.filter((projection) => projection.status === 'decoded').length;
  return Object.freeze({
    programId: canonicalProgramId,
    scanSlot: scan.slot,
    totalAccounts: String(scan.accounts.length),
    decodedAccounts: String(decoded),
    refusedAccounts: String(projections.length - decoded),
    projections: Object.freeze(projections),
  });
}
