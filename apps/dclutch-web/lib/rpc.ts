import { PublicKey } from '@solana/web3.js';

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

export class SolanaRpcClient {
  readonly endpoint: string;
  private requestId = 0;

  constructor(endpoint: string, private readonly fetcher: typeof fetch = fetch) {
    const url = new URL(endpoint);
    if (url.protocol !== 'http:' && url.protocol !== 'https:') throw new Error('RPC endpoint must use http or https');
    this.endpoint = url.toString();
  }

  private async request(method: string, params: ReadonlyArray<unknown>): Promise<unknown> {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), RPC_TIMEOUT_MS);
    try {
      const response = await this.fetcher(this.endpoint, {
        method: 'POST',
        mode: 'cors',
        credentials: 'omit',
        cache: 'no-store',
        redirect: 'error',
        referrerPolicy: 'no-referrer',
        headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: ++this.requestId, method, params }),
        signal: controller.signal,
      });
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
