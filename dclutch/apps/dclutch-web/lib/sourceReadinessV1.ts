import {
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { hex, sha256 } from '@dclutch/sdk/bytes';
import {
  SOURCE_READINESS_MARKET_FORMAT_V1,
  SOURCE_READINESS_PLAN_FORMAT_V1,
  SOURCE_READINESS_RECORDS_FORMAT_V1,
  SOURCE_READINESS_SNAPSHOT_FORMAT_V1,
  SOURCE_READINESS_SOURCE_FORMAT_V1,
  SOURCE_READINESS_WASM_BYTES_V1,
  SOURCE_READINESS_WASM_SHA256_V1,
} from '@dclutch/sdk/generated/sourceReadinessWasmV1';
import type {
  AccountInfoObservation,
  LatestBlockhashObservation,
  MultipleAccountObservation,
  RpcAccount,
  SolanaRpcClient,
} from '@dclutch/sdk/rpc';
import { SOLANA_PACKET_BYTES_V1 } from '@dclutch/sdk/solanaLimits';

const MAX_JSON_CHARACTERS = 64 * 1024 * 1024;
const MAX_OBSERVATION_ACCOUNTS = 20;
const SYSTEM_PROGRAM = SystemProgram.programId.toBase58();

export type SourceReadinessRouteV1 = 'create' | 'activate' | 'accept' | 'complete' | 'consumed-by-founding';

export type SourceReadinessWasmV1 = Readonly<{
  derive_source_readiness_base_v1(source: string): string;
  derive_source_readiness_recovery_v1(source: string): string;
  derive_source_readiness_detail_v1(source: string): string;
  plan_source_readiness_v1(source: string): string;
  derive_source_terminal_base_v1(source: string): string;
  derive_source_terminal_product_v1(source: string): string;
  derive_source_terminal_detail_v1(source: string): string;
  plan_source_terminal_v1(source: string): string;
  derive_source_close_detail_v1(source: string): string;
  plan_source_close_fund_v1(source: string): string;
  verify_source_close_receipt_v1(source: string): string;
}>;

/**
 * The four exports the READINESS route calls, named apart from the module.
 *
 * `SourceReadinessWasmV1` describes the whole eleven-export WASM boundary --
 * readiness, terminal, and close. This route uses four of them, and taking the
 * whole module as its parameter meant every caller and every test had to
 * present eleven exports to exercise four. Its test presented four and
 * annotated them as the module, which is a stub claiming to be a boundary it
 * does not cover; either the stub or the parameter had to become honest, and
 * the parameter is the half that also tells a reader what this route depends
 * on. The loader still returns the whole module and still satisfies this.
 */
export type SourceReadinessRouteWasmV1 = Pick<
  SourceReadinessWasmV1,
  'derive_source_readiness_base_v1'
  | 'derive_source_readiness_recovery_v1'
  | 'derive_source_readiness_detail_v1'
  | 'plan_source_readiness_v1'
>;

export type SourceReadinessAccountMetaV1 = Readonly<{
  address: string;
  isSigner: boolean;
  isWritable: boolean;
}>;

export type SourceReadinessInstructionV1 = Readonly<{
  program: string;
  accounts: ReadonlyArray<SourceReadinessAccountMetaV1>;
  dataBase64: string;
}>;

export type SourceReadinessPlanV1 = Readonly<{
  format: typeof SOURCE_READINESS_PLAN_FORMAT_V1;
  route: SourceReadinessRouteV1;
  observedSlot: string;
  instruction: SourceReadinessInstructionV1 | null;
  prepay: Readonly<{ destination: string; lamports: string }> | null;
  accounts: Readonly<{
    protocolWritable: ReadonlyArray<string>;
    completion: ReadonlyArray<string>;
  }> | null;
  geometry: Readonly<{
    protocolAccountCount: number;
    protocolUniqueAccountCount: number;
    protocolWritableCount: number;
    protocolSignerCount: number;
    protocolDataLen: number;
    transactionInstructionCountWithoutComputeBudget: number;
    transactionLockCountWithoutPayer: number;
  }> | null;
  facts: Readonly<Record<string, string>>;
}>;

export type SourceReadinessAcquisitionV1 = Readonly<{
  plan: SourceReadinessPlanV1;
  planJson: string;
  snapshotJson: string;
  observationAddresses: ReadonlyArray<string>;
}>;

export type SourceReadinessFrameAcquisitionV1 = Readonly<{
  snapshotJson: string;
  observationAddresses: ReadonlyArray<string>;
}>;

export type SourceReadinessTransactionV1 = Readonly<{
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  payer: string;
  route: 'create' | 'activate' | 'accept';
  observedSlot: string;
  lastValidBlockHeight: string;
}>;

export type SourceReadinessProgramsV1 = Readonly<{
  coreProgram: string;
  registryProgram: string;
  resolutionProgram: string;
}>;

type SourceReadinessRpcV1 = Pick<
  SolanaRpcClient,
  'finalizedSlot' | 'accountInfo' | 'multipleAccounts' | 'blockTime'
>;

type ReadinessFrameJsonV1 = Readonly<{
  coordinates: Readonly<{
    market: string;
    sourceMaterial: Readonly<{ raw: string; staging: string }>;
    capabilityManifest: Readonly<{ raw: string; staging: string }>;
    recoveryPolicy: Readonly<{ raw: string; staging: string }> | null;
    sourceState: string;
    fundingLedger: string;
    beneficiary: string;
    activationReceipt: string;
  }>;
  activationCache: string;
  registryProgram: string;
  coreProgram: string;
  coreProgramdata: string;
  resolutionProgram: string;
  resolutionProgramdata: string;
}>;

type ReadinessDetailJsonV1 = Readonly<{
  recoveryPolicy: string | null;
  recoveryPolicyStaging: string | null;
  fundingLedger: string;
  fundingEntryIndices: readonly [number, number, number];
  frame: ReadinessFrameJsonV1;
  addresses: ReadonlyArray<string>;
}>;

function plain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function exactKeys(value: Record<string, unknown>, fields: ReadonlyArray<string>, label: string): void {
  const observed = Object.keys(value).sort();
  const expected = [...fields].sort();
  if (observed.length !== expected.length || observed.some((field, index) => field !== expected[index])) {
    throw new Error(`${label} has missing or unknown fields`);
  }
}

function object(value: unknown, fields: ReadonlyArray<string>, label: string): Record<string, unknown> {
  if (!plain(value)) throw new Error(`${label} is not one object`);
  exactKeys(value, fields, label);
  return value;
}

function parseJson(source: string, label: string): unknown {
  if (typeof source !== 'string' || source.length === 0 || source.length > MAX_JSON_CHARACTERS) {
    throw new Error(`${label} is outside its bounded JSON size`);
  }
  try { return JSON.parse(source); } catch { throw new Error(`${label} is not JSON`); }
}

function key(value: unknown, field: string): string {
  if (typeof value !== 'string') throw new Error(`${field} is not text`);
  let canonical: string;
  try { canonical = new PublicKey(value).toBase58(); } catch { throw new Error(`${field} is not one Solana address`); }
  if (canonical !== value) throw new Error(`${field} is not canonical base58 text`);
  return canonical;
}

function unsigned(value: unknown, field: string): string {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} is not canonical unsigned decimal text`);
  const parsed = BigInt(value);
  if (parsed > 18_446_744_073_709_551_615n) throw new Error(`${field} exceeds u64`);
  return value;
}

function safeUnsigned(value: unknown, field: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new Error(`${field} is not a safe unsigned integer`);
  return value;
}

function canonicalBase64(value: unknown, field: string): string {
  if (typeof value !== 'string' || value.length % 4 !== 0
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error(`${field} is not canonical base64`);
  }
  let binary: string;
  try { binary = atob(value); } catch { throw new Error(`${field} is not canonical base64`); }
  if (bytesBase64(Uint8Array.from(binary, (character) => character.charCodeAt(0))) !== value) {
    throw new Error(`${field} is not canonical base64`);
  }
  return value;
}

function bytesBase64(bytes: Uint8Array): string {
  let output = '';
  for (let offset = 0; offset < bytes.length; offset += 8_192) {
    output += String.fromCharCode(...bytes.subarray(offset, offset + 8_192));
  }
  return btoa(output);
}

function base64Bytes(value: string): Uint8Array {
  return Uint8Array.from(atob(value), (character) => character.charCodeAt(0));
}

function recordCoordinates(value: unknown, field: string): Readonly<{ raw: string; staging: string }> {
  const raw = object(value, ['raw', 'staging'], field);
  return Object.freeze({ raw: key(raw.raw, `${field}.raw`), staging: key(raw.staging, `${field}.staging`) });
}

function parseBase(source: string): Readonly<{
  sourceMaterial: string;
  sourceMaterialStaging: string;
  capabilityManifest: string;
  capabilityManifestStaging: string;
}> {
  const raw = object(parseJson(source, 'Source readiness base coordinates'), [
    'activationCache', 'activationReceipt', 'beneficiary', 'capabilityManifest',
    'capabilityManifestStaging', 'coreProgramdata', 'generation', 'resolutionProgramdata',
    'sourceMaterial', 'sourceMaterialStaging', 'sourceState',
  ], 'Source readiness base coordinates');
  // Parse every field even though only the record pairs are needed for the
  // first read. This prevents a future Rust output extension from becoming an
  // unnoticed second client ABI.
  for (const field of ['activationCache', 'activationReceipt', 'beneficiary', 'capabilityManifest',
    'capabilityManifestStaging', 'coreProgramdata', 'resolutionProgramdata', 'sourceMaterial',
    'sourceMaterialStaging', 'sourceState'] as const) key(raw[field], `base.${field}`);
  unsigned(raw.generation, 'base.generation');
  return Object.freeze({
    sourceMaterial: raw.sourceMaterial as string,
    sourceMaterialStaging: raw.sourceMaterialStaging as string,
    capabilityManifest: raw.capabilityManifest as string,
    capabilityManifestStaging: raw.capabilityManifestStaging as string,
  });
}

function parseRecovery(source: string): Readonly<{ raw: string; staging: string } | null> {
  const raw = object(parseJson(source, 'Source readiness recovery coordinates'),
    ['recoveryPolicy', 'recoveryPolicyStaging'], 'Source readiness recovery coordinates');
  if (raw.recoveryPolicy === null && raw.recoveryPolicyStaging === null) return null;
  return Object.freeze({
    raw: key(raw.recoveryPolicy, 'recovery policy'),
    staging: key(raw.recoveryPolicyStaging, 'recovery policy staging'),
  });
}

function parseFrame(value: unknown): ReadinessFrameJsonV1 {
  const frame = object(value, [
    'activationCache', 'coordinates', 'coreProgram', 'coreProgramdata', 'registryProgram',
    'resolutionProgram', 'resolutionProgramdata',
  ], 'Source readiness frame');
  const coordinates = object(frame.coordinates, [
    'activationReceipt', 'beneficiary', 'capabilityManifest', 'fundingLedger', 'market',
    'recoveryPolicy', 'sourceMaterial', 'sourceState',
  ], 'Source readiness frame coordinates');
  const recovery = coordinates.recoveryPolicy === null
    ? null
    : recordCoordinates(coordinates.recoveryPolicy, 'frame recovery policy');
  return Object.freeze({
    coordinates: Object.freeze({
      market: key(coordinates.market, 'frame Market'),
      sourceMaterial: recordCoordinates(coordinates.sourceMaterial, 'frame Source material'),
      capabilityManifest: recordCoordinates(coordinates.capabilityManifest, 'frame capability manifest'),
      recoveryPolicy: recovery,
      sourceState: key(coordinates.sourceState, 'frame Source state'),
      fundingLedger: key(coordinates.fundingLedger, 'frame funding ledger'),
      beneficiary: key(coordinates.beneficiary, 'frame beneficiary'),
      activationReceipt: key(coordinates.activationReceipt, 'frame activation receipt'),
    }),
    activationCache: key(frame.activationCache, 'frame activation cache'),
    registryProgram: key(frame.registryProgram, 'frame Registry program'),
    coreProgram: key(frame.coreProgram, 'frame Core program'),
    coreProgramdata: key(frame.coreProgramdata, 'frame Core ProgramData'),
    resolutionProgram: key(frame.resolutionProgram, 'frame Resolution program'),
    resolutionProgramdata: key(frame.resolutionProgramdata, 'frame Resolution ProgramData'),
  });
}

function parseDetail(source: string): ReadinessDetailJsonV1 {
  const raw = object(parseJson(source, 'Source readiness detail coordinates'), [
    'addresses', 'frame', 'fundingEntryIndices', 'fundingLedger', 'recoveryPolicy',
    'recoveryPolicyStaging',
  ], 'Source readiness detail coordinates');
  if (!Array.isArray(raw.addresses) || raw.addresses.length < 18 || raw.addresses.length > MAX_OBSERVATION_ACCOUNTS) {
    throw new Error('Source readiness observation is outside its 18..20 account bound');
  }
  const addresses = Object.freeze(raw.addresses.map((address, index) => key(address, `observation address ${index}`)));
  if (new Set(addresses).size !== addresses.length) throw new Error('Source readiness observation repeats an address');
  if (!Array.isArray(raw.fundingEntryIndices) || raw.fundingEntryIndices.length !== 3) {
    throw new Error('Source readiness funding selection is not exactly three entries');
  }
  const indices = raw.fundingEntryIndices.map((entry, index) => safeUnsigned(entry, `funding entry index ${index}`));
  const recoveryPolicy = raw.recoveryPolicy === null ? null : key(raw.recoveryPolicy, 'detail recovery policy');
  const recoveryPolicyStaging = raw.recoveryPolicyStaging === null ? null : key(raw.recoveryPolicyStaging, 'detail recovery staging');
  if ((recoveryPolicy === null) !== (recoveryPolicyStaging === null)) throw new Error('Source readiness recovery pair is partial');
  return Object.freeze({
    recoveryPolicy,
    recoveryPolicyStaging,
    fundingLedger: key(raw.fundingLedger, 'detail funding ledger'),
    fundingEntryIndices: Object.freeze(indices) as unknown as readonly [number, number, number],
    frame: parseFrame(raw.frame),
    addresses,
  });
}

function parseInstruction(value: unknown): SourceReadinessInstructionV1 | null {
  if (value === null) return null;
  const raw = object(value, ['accounts', 'dataBase64', 'program'], 'Source readiness instruction');
  if (!Array.isArray(raw.accounts) || raw.accounts.length === 0 || raw.accounts.length > 32) {
    throw new Error('Source readiness instruction account frame is outside 1..32');
  }
  const accounts = raw.accounts.map((entry, index) => {
    const meta = object(entry, ['address', 'isSigner', 'isWritable'], `Source readiness meta ${index}`);
    if (typeof meta.isSigner !== 'boolean' || typeof meta.isWritable !== 'boolean') throw new Error(`Source readiness meta ${index} privileges are malformed`);
    return Object.freeze({ address: key(meta.address, `Source readiness meta ${index}`), isSigner: meta.isSigner, isWritable: meta.isWritable });
  });
  return Object.freeze({
    program: key(raw.program, 'Source readiness instruction program'),
    accounts: Object.freeze(accounts),
    dataBase64: canonicalBase64(raw.dataBase64, 'Source readiness instruction data'),
  });
}

export function parseSourceReadinessPlanV1(source: string): SourceReadinessPlanV1 {
  const raw = object(parseJson(source, 'Source readiness plan'), [
    'accounts', 'facts', 'format', 'geometry', 'instruction', 'observedSlot', 'prepay', 'route',
  ], 'Source readiness plan');
  if (raw.format !== SOURCE_READINESS_PLAN_FORMAT_V1) throw new Error('Source readiness plan has another format');
  if (!['create', 'activate', 'accept', 'complete', 'consumed-by-founding'].includes(String(raw.route))) {
    throw new Error('Source readiness plan has an unknown route');
  }
  const route = raw.route as SourceReadinessRouteV1;
  const instruction = parseInstruction(raw.instruction);
  const prepay = raw.prepay === null ? null : (() => {
    const value = object(raw.prepay, ['destination', 'lamports'], 'Source readiness prepay');
    return Object.freeze({ destination: key(value.destination, 'prepay destination'), lamports: unsigned(value.lamports, 'prepay lamports') });
  })();
  const accounts = raw.accounts === null ? null : (() => {
    const value = object(raw.accounts, ['completion', 'protocolWritable'], 'Source readiness account sets');
    if (!Array.isArray(value.protocolWritable) || !Array.isArray(value.completion)) throw new Error('Source readiness account sets are not arrays');
    return Object.freeze({
      protocolWritable: Object.freeze(value.protocolWritable.map((entry, index) => key(entry, `protocol writable ${index}`))),
      completion: Object.freeze(value.completion.map((entry, index) => key(entry, `completion account ${index}`))),
    });
  })();
  const geometry = raw.geometry === null ? null : (() => {
    const fields = ['protocolAccountCount', 'protocolDataLen', 'protocolSignerCount',
      'protocolUniqueAccountCount', 'protocolWritableCount',
      'transactionInstructionCountWithoutComputeBudget', 'transactionLockCountWithoutPayer'] as const;
    const value = object(raw.geometry, fields, 'Source readiness geometry');
    return Object.freeze(Object.fromEntries(fields.map((field) => [field, safeUnsigned(value[field], `geometry.${field}`)]))) as SourceReadinessPlanV1['geometry'];
  })();
  if (!plain(raw.facts)) throw new Error('Source readiness facts are not one object');
  const facts: Record<string, string> = {};
  for (const [name, value] of Object.entries(raw.facts)) {
    if (!/^[A-Za-z][A-Za-z0-9]{0,63}$/.test(name) || typeof value !== 'string' || value.length > 256) {
      throw new Error('Source readiness facts contain an unsupported field or value');
    }
    facts[name] = value;
  }
  const executable = route === 'create' || route === 'activate' || route === 'accept';
  if (executable !== (instruction !== null) || executable !== (accounts !== null) || executable !== (geometry !== null)) {
    throw new Error('Source readiness route disagrees with its executable plan fields');
  }
  if (!executable && prepay !== null) throw new Error('terminal Source readiness route carries a prepay');
  if (geometry !== null && geometry.protocolSignerCount !== 0) throw new Error('Source readiness protocol act unexpectedly requires a signer');
  if (instruction?.accounts.some((meta) => meta.isSigner) === true) throw new Error('Source readiness protocol metas unexpectedly require a signer');
  return Object.freeze({
    format: SOURCE_READINESS_PLAN_FORMAT_V1,
    route,
    observedSlot: unsigned(raw.observedSlot, 'plan observed slot'),
    instruction,
    prepay,
    accounts,
    geometry,
    facts: Object.freeze(facts),
  });
}

/** Load the checked Rust planner blob; unverified fetched bytes never execute. */
export async function loadSourceReadinessWasmV1(
  fetcher: typeof fetch = (input, init) => globalThis.fetch(input, init),
): Promise<SourceReadinessWasmV1> {
  const wasmModule = await import('./generated/sourceReadinessWasm/source_readiness.js');
  const url = new URL('./generated/sourceReadinessWasm/source_readiness_bg.wasm', import.meta.url);
  const response = await fetcher(url);
  if (!response.ok) throw new Error(`Source readiness WASM fetch failed with HTTP ${response.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length !== SOURCE_READINESS_WASM_BYTES_V1
      || hex(await sha256(bytes)) !== SOURCE_READINESS_WASM_SHA256_V1) {
    throw new Error('Source readiness WASM bytes do not match the generated Rust artifact identity');
  }
  await wasmModule.default({ module_or_path: bytes });
  return Object.freeze({
    derive_source_readiness_base_v1: wasmModule.derive_source_readiness_base_v1,
    derive_source_readiness_recovery_v1: wasmModule.derive_source_readiness_recovery_v1,
    derive_source_readiness_detail_v1: wasmModule.derive_source_readiness_detail_v1,
    plan_source_readiness_v1: wasmModule.plan_source_readiness_v1,
    derive_source_terminal_base_v1: wasmModule.derive_source_terminal_base_v1,
    derive_source_terminal_product_v1: wasmModule.derive_source_terminal_product_v1,
    derive_source_terminal_detail_v1: wasmModule.derive_source_terminal_detail_v1,
    plan_source_terminal_v1: wasmModule.plan_source_terminal_v1,
    derive_source_close_detail_v1: wasmModule.derive_source_close_detail_v1,
    plan_source_close_fund_v1: wasmModule.plan_source_close_fund_v1,
    verify_source_close_receipt_v1: wasmModule.verify_source_close_receipt_v1,
  });
}

function accountBase64(account: RpcAccount): string {
  return bytesBase64(account.data);
}

function requireAccount(observation: AccountInfoObservation, address: string, label: string): RpcAccount {
  if (observation.account === null) throw new Error(`${label} ${address} is absent at finalized slot ${observation.slot}`);
  return observation.account;
}

function accountByAddress(observation: MultipleAccountObservation, address: string, label: string): RpcAccount {
  const found = observation.accounts.find((entry) => entry.address === address);
  if (found === undefined || found.account === null) throw new Error(`${label} ${address} is absent at finalized slot ${observation.slot}`);
  return found.account;
}

function observedAccountJson(address: string, account: RpcAccount | null): Readonly<Record<string, unknown>> {
  return account === null
    ? Object.freeze({ address, owner: SYSTEM_PROGRAM, lamports: '0', executable: false, dataBase64: '' })
    : Object.freeze({ address, owner: account.owner, lamports: account.lamports, executable: account.executable, dataBase64: accountBase64(account) });
}

/**
 * Reacquire one exact finalized Source-readiness frame and ask the Rust owner
 * for the sole adjacent action. RPC stays outside the WASM module by design.
 */
export async function acquireSourceReadinessFrameV1(
  client: SourceReadinessRpcV1,
  wasm: SourceReadinessRouteWasmV1,
  market: string,
  programs: SourceReadinessProgramsV1,
): Promise<SourceReadinessFrameAcquisitionV1> {
  const marketAddress = key(market, 'Market');
  const coreProgram = key(programs.coreProgram, 'Core program');
  const registryProgram = key(programs.registryProgram, 'Registry program');
  const resolutionProgram = key(programs.resolutionProgram, 'Resolution program');
  const floor = unsigned(await client.finalizedSlot(), 'finalized floor');
  const marketObservation = await client.accountInfo(marketAddress, floor);
  const marketAccount = requireAccount(marketObservation, marketAddress, 'Market');
  const marketInput = JSON.stringify({
    format: SOURCE_READINESS_MARKET_FORMAT_V1,
    market: {
      address: marketAddress,
      owner: marketAccount.owner,
      executable: marketAccount.executable,
      dataBase64: accountBase64(marketAccount),
    },
    coreProgram,
    registryProgram,
    resolutionProgram,
  });
  const base = parseBase(wasm.derive_source_readiness_base_v1(marketInput));
  const firstRecords = await client.multipleAccounts([
    base.sourceMaterial,
    base.sourceMaterialStaging,
    base.capabilityManifest,
    base.capabilityManifestStaging,
  ], marketObservation.slot);
  const sourceMaterial = accountByAddress(firstRecords, base.sourceMaterial, 'Source material');
  const capabilityManifest = accountByAddress(firstRecords, base.capabilityManifest, 'capability manifest');
  const recovery = parseRecovery(wasm.derive_source_readiness_recovery_v1(JSON.stringify({
    format: SOURCE_READINESS_SOURCE_FORMAT_V1,
    marketDataBase64: accountBase64(marketAccount),
    registryProgram,
    sourceMaterialDataBase64: accountBase64(sourceMaterial),
  })));
  const recoveryObservation = recovery === null
    ? null
    : await client.multipleAccounts([recovery.raw, recovery.staging], firstRecords.slot);
  const recoveryAccount = recovery === null || recoveryObservation === null
    ? null
    : accountByAddress(recoveryObservation, recovery.raw, 'recovery policy');
  const detail = parseDetail(wasm.derive_source_readiness_detail_v1(JSON.stringify({
    format: SOURCE_READINESS_RECORDS_FORMAT_V1,
    marketAddress,
    marketOwner: marketAccount.owner,
    marketExecutable: marketAccount.executable,
    marketDataBase64: accountBase64(marketAccount),
    coreProgram,
    registryProgram,
    resolutionProgram,
    sourceMaterialDataBase64: accountBase64(sourceMaterial),
    capabilityManifestDataBase64: accountBase64(capabilityManifest),
    recoveryPolicyDataBase64: recoveryAccount === null ? null : accountBase64(recoveryAccount),
  })));
  const programdata = new Set([detail.frame.coreProgramdata, detail.frame.resolutionProgramdata]);
  const ordinaryAddresses = detail.addresses.filter((address) => !programdata.has(address));
  const exactFloor = unsigned(await client.finalizedSlot(), 'exact snapshot floor');
  const [ordinary, coreProgramdata, resolutionProgramdata] = await Promise.all([
    client.multipleAccounts(ordinaryAddresses, exactFloor),
    client.accountInfo(detail.frame.coreProgramdata, exactFloor),
    client.accountInfo(detail.frame.resolutionProgramdata, exactFloor),
  ]);
  if (ordinary.slot !== coreProgramdata.slot || ordinary.slot !== resolutionProgramdata.slot) {
    throw new Error('finalized slot advanced during the split ELF read; read the Source readiness frame again');
  }
  const timestamp = await client.blockTime(ordinary.slot);
  if (timestamp === null) throw new Error(`finalized slot ${ordinary.slot} has no authenticated block time; no readiness plan was built`);
  const accounts = new Map(ordinary.accounts.map((entry) => [entry.address, entry.account] as const));
  accounts.set(detail.frame.coreProgramdata, coreProgramdata.account);
  accounts.set(detail.frame.resolutionProgramdata, resolutionProgramdata.account);
  if (accounts.size !== detail.addresses.length) throw new Error('split Source readiness read omitted or repeated an account');
  const snapshotJson = JSON.stringify({
    format: SOURCE_READINESS_SNAPSHOT_FORMAT_V1,
    observedSlot: ordinary.slot,
    unixTimestamp: timestamp,
    frame: detail.frame,
    accounts: detail.addresses.map((address) => observedAccountJson(address, accounts.get(address) ?? null)),
  });
  return Object.freeze({ snapshotJson, observationAddresses: detail.addresses });
}


/** Reacquire a frame, then ask the Rust owner for its sole readiness route. */
export async function acquireSourceReadinessV1(
  client: SourceReadinessRpcV1,
  wasm: SourceReadinessRouteWasmV1,
  market: string,
  programs: SourceReadinessProgramsV1,
): Promise<SourceReadinessAcquisitionV1> {
  const frame = await acquireSourceReadinessFrameV1(client, wasm, market, programs);
  const planJson = wasm.plan_source_readiness_v1(frame.snapshotJson);
  const plan = parseSourceReadinessPlanV1(planJson);
  const snapshot = object(parseJson(frame.snapshotJson, 'Source readiness snapshot'),
    ['accounts', 'format', 'frame', 'observedSlot', 'unixTimestamp'], 'Source readiness snapshot');
  if (plan.observedSlot !== snapshot.observedSlot) throw new Error('Source readiness plan substituted its finalized observation slot');
  return Object.freeze({ plan, planJson, snapshotJson: frame.snapshotJson,
    observationAddresses: frame.observationAddresses });
}

/** Compile the exact Rust-owned instruction; the sole new authority is payer. */
export function buildSourceReadinessTransactionV1(
  acquisition: SourceReadinessAcquisitionV1,
  payerAddress: string,
  blockhash: LatestBlockhashObservation,
): SourceReadinessTransactionV1 {
  const { plan } = acquisition;
  if (plan.route !== 'create' && plan.route !== 'activate' && plan.route !== 'accept') {
    throw new Error(`Source readiness route ${plan.route} has no wallet act`);
  }
  if (plan.instruction === null || plan.geometry === null || plan.accounts === null) {
    throw new Error('executable Source readiness route omitted its checked instruction geometry');
  }
  const payer = key(payerAddress, 'transaction payer');
  if (plan.instruction.accounts.some((meta) => meta.isSigner)) throw new Error('Rust readiness instruction unexpectedly requests another signer');
  const protocol = new TransactionInstruction({
    programId: new PublicKey(plan.instruction.program),
    keys: plan.instruction.accounts.map((meta) => ({
      pubkey: new PublicKey(meta.address), isSigner: false, isWritable: meta.isWritable,
    })),
    data: Buffer.from(base64Bytes(plan.instruction.dataBase64)),
  });
  const instructions: TransactionInstruction[] = [];
  if (plan.prepay !== null && BigInt(plan.prepay.lamports) > 0n) {
    instructions.push(SystemProgram.transfer({
      fromPubkey: new PublicKey(payer),
      toPubkey: new PublicKey(plan.prepay.destination),
      lamports: BigInt(plan.prepay.lamports),
    }));
  }
  instructions.push(protocol);
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: new PublicKey(payer),
    recentBlockhash: key(blockhash.blockhash, 'recent blockhash'),
    instructions,
  }).compileToLegacyMessage());
  if (transaction.signatures.length !== 1 || transaction.message.header.numRequiredSignatures !== 1
      || transaction.message.staticAccountKeys[0]?.toBase58() !== payer) {
    throw new Error('Source readiness transaction did not compile to one sole-payer signature');
  }
  const wireBytes = transaction.serialize();
  if (wireBytes.length > SOLANA_PACKET_BYTES_V1) throw new Error(`Source readiness transaction is ${wireBytes.length} bytes, above Solana's packet bound`);
  if (instructions.length !== plan.geometry.transactionInstructionCountWithoutComputeBudget) {
    throw new Error('compiled Source readiness instruction count differs from Rust geometry');
  }
  return Object.freeze({
    transaction,
    wireBytes,
    payer,
    route: plan.route,
    observedSlot: plan.observedSlot,
    lastValidBlockHeight: unsigned(blockhash.lastValidBlockHeight, 'last valid block height'),
  });
}
