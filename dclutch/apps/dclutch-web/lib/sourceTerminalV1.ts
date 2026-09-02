import {
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import {
  SOURCE_TERMINAL_BASE_FORMAT_V1,
  SOURCE_TERMINAL_DETAIL_FORMAT_V1,
  SOURCE_TERMINAL_PLAN_FORMAT_V1,
  SOURCE_TERMINAL_PRODUCT_FORMAT_V1,
  SOURCE_TERMINAL_SNAPSHOT_FORMAT_V1,
} from './generated/sourceReadinessWasmV1';
import type { LatestBlockhashObservation, RpcAccount, SolanaRpcClient } from './rpc';
import { SOLANA_PACKET_BYTES_V1 } from './solanaLimits';
import {
  acquireSourceReadinessV1,
  type SourceReadinessProgramsV1,
  type SourceReadinessWasmV1,
} from './sourceReadinessV1';

const SYSTEM_PROGRAM = '11111111111111111111111111111111';
const RENT_SYSVAR = 'SysvarRent111111111111111111111111111111111';
const MAX_JSON = 64 * 1024 * 1024;

export type SourceTerminalPlanV1 = Readonly<{
  format: typeof SOURCE_TERMINAL_PLAN_FORMAT_V1;
  route: 'admit' | 'complete';
  observedSlot: string;
  instruction: Readonly<{
    program: string;
    accounts: ReadonlyArray<Readonly<{ address: string; isSigner: boolean; isWritable: boolean }>>;
    dataBase64: string;
  }> | null;
  accounts: Readonly<{ protocolWritable: ReadonlyArray<string>; completion: ReadonlyArray<string> }>;
  geometry: Readonly<{
    protocolAccountCount: number;
    protocolUniqueAccountCount: number;
    protocolWritableCount: number;
    protocolSignerCount: number;
    protocolDataLen: number;
    transactionInstructionCountWithoutComputeBudget: number;
    transactionLockCountWithoutPayer: number;
  }>;
  facts: Readonly<Record<string, string>>;
}>;

export type SourceTerminalAcquisitionV1 = Readonly<{
  plan: SourceTerminalPlanV1;
  planJson: string;
  snapshotJson: string;
  observationAddresses: ReadonlyArray<string>;
}>;

export type SourceTerminalTransactionV1 = Readonly<{
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  payer: string;
  observedSlot: string;
  lastValidBlockHeight: string;
}>;

type TerminalRpcV1 = Pick<SolanaRpcClient,
  'finalizedSlot' | 'accountInfo' | 'multipleAccounts' | 'blockTime'>;

function plain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function object(value: unknown, fields: ReadonlyArray<string>, label: string): Record<string, unknown> {
  if (!plain(value)) throw new Error(`${label} is not one object`);
  const actual = Object.keys(value).sort();
  const expected = [...fields].sort();
  if (actual.length !== expected.length || actual.some((field, index) => field !== expected[index])) {
    throw new Error(`${label} has missing or unknown fields`);
  }
  return value;
}

function json(source: string, label: string): unknown {
  if (source.length === 0 || source.length > MAX_JSON) throw new Error(`${label} is outside its JSON bound`);
  try { return JSON.parse(source); } catch { throw new Error(`${label} is not JSON`); }
}

function key(value: unknown, label: string): string {
  if (typeof value !== 'string') throw new Error(`${label} is not text`);
  let canonical: string;
  try { canonical = new PublicKey(value).toBase58(); } catch { throw new Error(`${label} is not a Solana address`); }
  if (canonical !== value) throw new Error(`${label} is not canonical base58`);
  return value;
}

function u64(value: unknown, label: string): string {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value)
      || BigInt(value) > 18_446_744_073_709_551_615n) throw new Error(`${label} is not canonical u64`);
  return value;
}

function count(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new Error(`${label} is not a safe count`);
  return value;
}

function bytesBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += 8_192) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 8_192));
  }
  return btoa(binary);
}

function base64(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length % 4 !== 0
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error(`${label} is not canonical base64`);
  }
  let bytes: Uint8Array;
  try { bytes = Uint8Array.from(atob(value), (character) => character.charCodeAt(0)); } catch {
    throw new Error(`${label} is not canonical base64`);
  }
  if (bytesBase64(bytes) !== value) throw new Error(`${label} is not canonical base64`);
  return value;
}

function accountJson(address: string, account: RpcAccount | null): Readonly<Record<string, unknown>> {
  return account === null
    ? Object.freeze({ address, owner: SYSTEM_PROGRAM, lamports: '0', executable: false, dataBase64: '' })
    : Object.freeze({ address, owner: account.owner, lamports: account.lamports,
      executable: account.executable, dataBase64: bytesBase64(account.data) });
}

function account(entries: ReadonlyArray<Readonly<{ address: string; account: RpcAccount | null }>>, address: string, label: string): RpcAccount {
  const found = entries.find((entry) => entry.address === address)?.account;
  if (found === undefined || found === null) throw new Error(`${label} ${address} is absent`);
  return found;
}

function parsePair(value: unknown, label: string): Readonly<{ raw: string; staging: string }> {
  const pair = object(value, ['raw', 'staging'], label);
  return Object.freeze({ raw: key(pair.raw, `${label}.raw`), staging: key(pair.staging, `${label}.staging`) });
}

/** Hostile-decode the exact Rust terminal plan used by recovery and execution. */
export function parseSourceTerminalPlanV1(source: string): SourceTerminalPlanV1 {
  const raw = object(json(source, 'Source terminal plan'),
    ['accounts', 'facts', 'format', 'geometry', 'instruction', 'observedSlot', 'prepay', 'route'], 'Source terminal plan');
  if (raw.format !== SOURCE_TERMINAL_PLAN_FORMAT_V1 || !['admit', 'complete'].includes(String(raw.route)) || raw.prepay !== null) {
    throw new Error('Source terminal plan changed its format, route, or no-prepay contract');
  }
  const accountSets = object(raw.accounts, ['completion', 'protocolWritable'], 'Source terminal account sets');
  if (!Array.isArray(accountSets.completion) || !Array.isArray(accountSets.protocolWritable)) throw new Error('Source terminal account sets are not arrays');
  const accounts = Object.freeze({
    completion: Object.freeze(accountSets.completion.map((value, index) => key(value, `completion ${index}`))),
    protocolWritable: Object.freeze(accountSets.protocolWritable.map((value, index) => key(value, `writable ${index}`))),
  });
  const geometryRaw = object(raw.geometry, [
    'protocolAccountCount', 'protocolDataLen', 'protocolSignerCount', 'protocolUniqueAccountCount',
    'protocolWritableCount', 'transactionInstructionCountWithoutComputeBudget', 'transactionLockCountWithoutPayer',
  ], 'Source terminal geometry');
  const geometry = Object.freeze(Object.fromEntries(Object.keys(geometryRaw).map((field) => [field, count(geometryRaw[field], `geometry.${field}`)]))) as SourceTerminalPlanV1['geometry'];
  if (geometry.protocolSignerCount !== 0 || geometry.protocolAccountCount !== 22) throw new Error('Source terminal geometry changed');
  const instruction = raw.instruction === null ? null : (() => {
    const value = object(raw.instruction, ['accounts', 'dataBase64', 'program'], 'Source terminal instruction');
    if (!Array.isArray(value.accounts) || value.accounts.length !== 22) throw new Error('Source terminal instruction is not the exact 22-account frame');
    const metas = value.accounts.map((entry, index) => {
      const meta = object(entry, ['address', 'isSigner', 'isWritable'], `terminal meta ${index}`);
      if (typeof meta.isSigner !== 'boolean' || typeof meta.isWritable !== 'boolean' || meta.isSigner) throw new Error(`terminal meta ${index} changed authority`);
      return Object.freeze({ address: key(meta.address, `terminal meta ${index}`), isSigner: false, isWritable: meta.isWritable });
    });
    return Object.freeze({ program: key(value.program, 'terminal program'), accounts: Object.freeze(metas), dataBase64: base64(value.dataBase64, 'terminal data') });
  })();
  if ((raw.route === 'admit') !== (instruction !== null)) throw new Error('Source terminal route disagrees with its instruction');
  if (!plain(raw.facts)) throw new Error('Source terminal facts are not an object');
  const facts: Record<string, string> = {};
  for (const [name, value] of Object.entries(raw.facts)) {
    if (!/^[A-Za-z][A-Za-z0-9]{0,63}$/.test(name) || typeof value !== 'string' || value.length > 256) throw new Error('Source terminal fact is malformed');
    facts[name] = value;
  }
  return Object.freeze({ format: SOURCE_TERMINAL_PLAN_FORMAT_V1, route: raw.route as 'admit' | 'complete',
    observedSlot: u64(raw.observedSlot, 'observed slot'), instruction, accounts, geometry, facts: Object.freeze(facts) });
}

/** Reacquire and plan terminal admission entirely through the Rust owner. */
export async function acquireSourceTerminalV1(
  client: TerminalRpcV1,
  wasm: SourceReadinessWasmV1,
  market: string,
  programs: SourceReadinessProgramsV1,
): Promise<SourceTerminalAcquisitionV1> {
  const marketAddress = key(market, 'Market');
  const readiness = await acquireSourceReadinessV1(client, wasm, marketAddress, programs);
  if (readiness.plan.route !== 'consumed-by-founding') throw new Error(`Source terminal admission requires consumed readiness, not ${readiness.plan.route}`);
  const initial = object(json(readiness.snapshotJson, 'readiness snapshot'), ['accounts', 'format', 'frame', 'observedSlot', 'unixTimestamp'], 'readiness snapshot');
  if (!Array.isArray(initial.accounts)) throw new Error('readiness snapshot accounts are not an array');
  const initialAccounts = initial.accounts.map((value, index) => object(value,
    ['address', 'dataBase64', 'executable', 'lamports', 'owner'], `readiness account ${index}`));
  const marketWire = initialAccounts.find((value) => value.address === marketAddress);
  if (marketWire === undefined || typeof marketWire.dataBase64 !== 'string') throw new Error('readiness snapshot omitted Market bytes');
  const terminalBaseRaw = object(json(wasm.derive_source_terminal_base_v1(JSON.stringify({
    format: SOURCE_TERMINAL_BASE_FORMAT_V1,
    market: { address: marketAddress, owner: marketWire.owner, executable: marketWire.executable, dataBase64: marketWire.dataBase64 },
    coreProgram: programs.coreProgram, registryProgram: programs.registryProgram, resolutionProgram: programs.resolutionProgram,
  })), 'terminal base'), ['productRaw', 'productStaging', 'readiness'], 'terminal base');
  const productRaw = key(terminalBaseRaw.productRaw, 'Product raw');
  const productStaging = key(terminalBaseRaw.productStaging, 'Product staging');
  const firstProduct = await client.multipleAccounts([productRaw, productStaging], u64(initial.observedSlot, 'readiness slot'));
  const product = account(firstProduct.accounts, productRaw, 'Product');
  const terminalProduct = object(json(wasm.derive_source_terminal_product_v1(JSON.stringify({
    format: SOURCE_TERMINAL_PRODUCT_FORMAT_V1, marketAddress, marketDataBase64: marketWire.dataBase64,
    registryProgram: programs.registryProgram, productDataBase64: bytesBase64(product.data),
  })), 'terminal Product'), ['portfolioRaw', 'portfolioStaging', 'resultDomainRaw', 'resultDomainStaging'], 'terminal Product');
  const resultDomainRaw = key(terminalProduct.resultDomainRaw, 'ResultDomain raw');
  const resultDomainStaging = key(terminalProduct.resultDomainStaging, 'ResultDomain staging');
  const portfolioRaw = key(terminalProduct.portfolioRaw, 'Portfolio raw');
  const portfolioStaging = key(terminalProduct.portfolioStaging, 'Portfolio staging');
  const readinessFrame = object(initial.frame, ['activationCache', 'coordinates', 'coreProgram', 'coreProgramdata', 'registryProgram', 'resolutionProgram', 'resolutionProgramdata'], 'readiness frame');
  const readinessCoordinates = object(readinessFrame.coordinates, ['activationReceipt', 'beneficiary', 'capabilityManifest', 'fundingLedger', 'market', 'recoveryPolicy', 'sourceMaterial', 'sourceState'], 'readiness coordinates');
  const sourceState = key(readinessCoordinates.sourceState, 'Source state');
  const graph = await client.multipleAccounts([sourceState, resultDomainRaw, resultDomainStaging, portfolioRaw, portfolioStaging], firstProduct.slot);
  const source = account(graph.accounts, sourceState, 'Source state');
  const domain = account(graph.accounts, resultDomainRaw, 'ResultDomain');
  const terminalDetail = object(json(wasm.derive_source_terminal_detail_v1(JSON.stringify({
    format: SOURCE_TERMINAL_DETAIL_FORMAT_V1, marketAddress, marketDataBase64: marketWire.dataBase64,
    registryProgram: programs.registryProgram, resolutionProgram: programs.resolutionProgram,
    sourceStateAddress: sourceState, sourceStateDataBase64: bytesBase64(source.data),
    productDataBase64: bytesBase64(product.data), resultDomainDataBase64: bytesBase64(domain.data),
  })), 'terminal detail'), ['certificate', 'outcomeCount', 'portfolioRaw', 'portfolioStaging', 'resultDomainRaw', 'resultDomainStaging', 'terminalSequence'], 'terminal detail');
  if (key(terminalDetail.resultDomainRaw, 'detail ResultDomain') !== resultDomainRaw
      || key(terminalDetail.resultDomainStaging, 'detail ResultDomain staging') !== resultDomainStaging
      || key(terminalDetail.portfolioRaw, 'detail Portfolio') !== portfolioRaw
      || key(terminalDetail.portfolioStaging, 'detail Portfolio staging') !== portfolioStaging) throw new Error('Source terminal staged derivations disagree');
  const certificate = key(terminalDetail.certificate, 'terminal certificate');
  count(terminalDetail.outcomeCount, 'terminal outcome count');
  u64(terminalDetail.terminalSequence, 'terminal sequence');
  const sourceMaterial = parsePair(readinessCoordinates.sourceMaterial, 'Source material');
  const capabilityManifest = parsePair(readinessCoordinates.capabilityManifest, 'capability manifest');
  const addresses = Object.freeze([
    marketAddress, key(readinessFrame.activationCache, 'activation cache'), key(readinessFrame.registryProgram, 'Registry program'),
    key(readinessFrame.coreProgram, 'Core program'), key(readinessFrame.coreProgramdata, 'Core ProgramData'),
    key(readinessFrame.resolutionProgram, 'Resolution program'), key(readinessFrame.resolutionProgramdata, 'Resolution ProgramData'),
    sourceMaterial.raw, sourceMaterial.staging, capabilityManifest.raw, capabilityManifest.staging, sourceState,
    key(readinessCoordinates.fundingLedger, 'funding ledger'), certificate, RENT_SYSVAR, productRaw, productStaging,
    resultDomainRaw, resultDomainStaging, portfolioRaw, portfolioStaging,
  ]);
  if (new Set(addresses).size !== 21) throw new Error('Source terminal frame aliases semantic accounts');
  const floor = await client.finalizedSlot();
  const exact = await client.multipleAccounts(addresses, floor);
  const timestamp = await client.blockTime(exact.slot);
  if (timestamp === null) throw new Error(`finalized slot ${exact.slot} has no block time`);
  const snapshotJson = JSON.stringify({
    format: SOURCE_TERMINAL_SNAPSHOT_FORMAT_V1, observedSlot: exact.slot, unixTimestamp: timestamp,
    frame: { readiness: readinessFrame, certificate, productRaw, productStaging, resultDomainRaw, resultDomainStaging, portfolioRaw, portfolioStaging },
    accounts: addresses.map((address) => accountJson(address, exact.accounts.find((entry) => entry.address === address)?.account ?? null)),
  });
  const planJson = wasm.plan_source_terminal_v1(snapshotJson);
  const plan = parseSourceTerminalPlanV1(planJson);
  if (plan.observedSlot !== exact.slot || plan.accounts.completion.join(',') !== [marketAddress, sourceState, certificate].join(',')) {
    throw new Error('Source terminal plan substituted its observation or completion frame');
  }
  return Object.freeze({ plan, planJson, snapshotJson, observationAddresses: addresses });
}

/** Compile the Rust-owned admission with the wallet as its sole new authority. */
/**
 * The two fields a packet builder actually reads.
 *
 * `LatestBlockhashObservation` also carries the SLOT the blockhash was read
 * at, and a builder must not take a slot from there: the slot that belongs in
 * the packet is `acquisition.plan.observedSlot`, the authenticated floor the
 * Rust plan was derived at. Demanding the whole observation invited a second
 * slot next to that one, and every caller in this tree correctly declined to
 * supply it -- which is why five call sites did not typecheck.
 */
export type PacketBlockhashV1 = Pick<LatestBlockhashObservation, 'blockhash' | 'lastValidBlockHeight'>;

export function buildSourceTerminalTransactionV1(
  acquisition: SourceTerminalAcquisitionV1,
  payerAddress: string,
  blockhash: PacketBlockhashV1,
): SourceTerminalTransactionV1 {
  if (acquisition.plan.route !== 'admit' || acquisition.plan.instruction === null) throw new Error('completed Source terminal plan has no wallet act');
  const payer = key(payerAddress, 'payer');
  const protocol = new TransactionInstruction({
    programId: new PublicKey(acquisition.plan.instruction.program),
    keys: acquisition.plan.instruction.accounts.map((meta) => ({ pubkey: new PublicKey(meta.address), isSigner: false, isWritable: meta.isWritable })),
    data: Buffer.from(Uint8Array.from(atob(acquisition.plan.instruction.dataBase64), (character) => character.charCodeAt(0))),
  });
  const transaction = new VersionedTransaction(new TransactionMessage({ payerKey: new PublicKey(payer),
    recentBlockhash: key(blockhash.blockhash, 'blockhash'), instructions: [protocol] }).compileToLegacyMessage());
  const wireBytes = transaction.serialize();
  if (transaction.signatures.length !== 1 || transaction.message.header.numRequiredSignatures !== 1
      || transaction.message.staticAccountKeys[0]?.toBase58() !== payer || wireBytes.length > SOLANA_PACKET_BYTES_V1) {
    throw new Error('Source terminal packet changed sole-payer or packet geometry');
  }
  return Object.freeze({ transaction, wireBytes, payer, observedSlot: acquisition.plan.observedSlot,
    lastValidBlockHeight: u64(blockhash.lastValidBlockHeight, 'last valid block height') });
}
