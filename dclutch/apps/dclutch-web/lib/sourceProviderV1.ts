import {
  Keypair,
  PublicKey,
  VersionedMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { hex, sha256 } from '@dclutch/sdk/bytes';
import {
  SOURCE_PROVIDER_COORDINATES_FORMAT_V1,
  SOURCE_PROVIDER_COORDINATES_INPUT_FORMAT_V1,
  SOURCE_PROVIDER_PLAN_FORMAT_V1,
  SOURCE_PROVIDER_PROGRAM_FORMAT_V1,
  SOURCE_PROVIDER_PROGRAM_INPUT_FORMAT_V1,
  SOURCE_PROVIDER_RECLAIM_INPUT_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_BASE_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_BASE_INPUT_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_FRESH_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_FRESH_INPUT_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_INPUT_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_LIFECYCLE_BYTES_V1,
  SOURCE_PROVIDER_SUBMIT_MATERIAL_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_MATERIAL_INPUT_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_PLAN_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_POSTSTATE_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_POSTSTATE_INPUT_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_PYTH_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_PYTH_INPUT_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_RECORD_FORMAT_V1,
  SOURCE_PROVIDER_SUBMIT_RECORD_INPUT_FORMAT_V1,
  SOURCE_PROVIDER_WASM_BYTES_V1,
  SOURCE_PROVIDER_PRICE_FORMAT_V1,
  SOURCE_PROVIDER_PRICE_INPUT_FORMAT_V1,
  SOURCE_PROVIDER_WASM_SHA256_V1,
} from '@dclutch/sdk/generated/sourceProviderWasmV1';
import type {
  AccountInfoObservation,
  MultipleAccountObservation,
  RpcAccount,
  SolanaRpcClient,
} from '@dclutch/sdk/rpc';
import { SOLANA_PACKET_BYTES_V1 } from '@dclutch/sdk/solanaLimits';

const MAX_JSON_CHARACTERS = 24 * 1024 * 1024;

export type SourceProviderWasmV1 = Readonly<{
  derive_source_provider_reclaim_coordinates_v1(source: string): string;
  derive_source_provider_programdata_v1(source: string): string;
  plan_source_provider_reclaim_v1(source: string): string;
  derive_source_provider_submit_base_v1(source: string): string;
  derive_source_provider_submit_material_v1(source: string): string;
  derive_source_provider_submit_provider_release_v1(source: string): string;
  derive_source_provider_submit_pyth_release_v1(source: string): string;
  derive_source_provider_submit_pyth_v1(source: string): string;
  read_source_provider_price_update_v1(source: string): string;
  derive_source_provider_submit_fresh_v1(source: string): string;
  plan_source_provider_submit_v1(source: string): string;
  verify_source_provider_submit_poststate_v1(source: string): string;
}>;

export type SourceProviderProgramsV1 = Readonly<{
  registryProgram: string;
  resolutionProgram: string;
}>;

export type SourceProviderSubmitProgramsV1 = SourceProviderProgramsV1 & Readonly<{
  coreProgram: string;
}>;

export type SourceProviderReclaimPlanV1 = Readonly<{
  format: typeof SOURCE_PROVIDER_PLAN_FORMAT_V1;
  route: 'reclaim';
  observedSlot: string;
  instruction: Readonly<{
    program: string;
    accounts: ReadonlyArray<Readonly<{ address: string; isSigner: boolean; isWritable: boolean }>>;
    dataBase64: string;
  }>;
  unsignedMessageBase64: string;
  requiredSigners: readonly [string, string];
  wireBytes: number;
  loadedAddresses: number;
  lookupTables: ReadonlyArray<string>;
  lifecycle: string;
  updateAuthority: string;
  completion: ReadonlyArray<string>;
  expectedPoststates: ReadonlyArray<Readonly<{
    address: string; owner: string; lamports: string; executable: boolean; dataBase64: string;
  }>>;
}>;

export type SourceProviderReclaimAcquisitionV1 = Readonly<{
  plan: SourceProviderReclaimPlanV1;
  planJson: string;
  inputJson: string;
  transaction: VersionedTransaction;
  resolver: Keypair;
  payer: string;
  market: string;
  lastValidBlockHeight: string;
  observationAddresses: ReadonlyArray<string>;
}>;

export type SourceProviderSubmitPlanV1 = Readonly<{
  format: typeof SOURCE_PROVIDER_SUBMIT_PLAN_FORMAT_V1;
  route: 'submit';
  observedSlot: string;
  instruction: Readonly<{
    program: string;
    accounts: ReadonlyArray<Readonly<{ address: string; isSigner: boolean; isWritable: boolean }>>;
    dataBase64: string;
  }>;
  unsignedMessageBase64: string;
  requiredSigners: readonly [string, string];
  wireBytes: number;
  loadedAddresses: number;
  lookupTables: readonly [string];
  lifecycleTopUpLamports: string;
  completion: readonly [string, string];
  poststate: Readonly<{
    lifecycle: string;
    updateAccount: string;
    updateAuthority: string;
    resolutionProgram: string;
    receiverProgram: string;
    submitRequestBase64: string;
  }>;
}>;

export type SourceProviderSubmitAcquisitionV1 = Readonly<{
  plan: SourceProviderSubmitPlanV1;
  planJson: string;
  inputJson: string;
  transaction: VersionedTransaction;
  update: Keypair;
  payer: string;
  market: string;
  lastValidBlockHeight: string;
  observationAddresses: ReadonlyArray<string>;
}>;

export type SourceProviderSubmitInputV1 = Readonly<{
  market: string;
  payer: string;
  encodedVaa: string;
  postUpdateBodyBase64: string;
  lookupTable: string;
  reclaimAfterUnixSeconds: string;
}>;

type SourceProviderRpcV1 = Pick<
  SolanaRpcClient,
  'finalizedSlot' | 'accountInfo' | 'multipleAccounts' | 'blockTime' | 'latestMutationBlockhash' | 'minimumBalanceForRentExemption'
>;

function plain(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function exactKeys(value: Record<string, unknown>, fields: ReadonlyArray<string>, label: string): void {
  const actual = Object.keys(value).sort();
  const expected = [...fields].sort();
  if (actual.length !== expected.length || actual.some((field, index) => field !== expected[index])) {
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
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new Error(`${field} is not one safe unsigned integer`);
  return value;
}

function bytesBase64(bytes: Uint8Array): string {
  let output = '';
  for (let offset = 0; offset < bytes.length; offset += 8_192) output += String.fromCharCode(...bytes.subarray(offset, offset + 8_192));
  return btoa(output);
}

function base64Bytes(value: unknown, field: string): Uint8Array {
  if (typeof value !== 'string' || value.length % 4 !== 0
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error(`${field} is not canonical base64`);
  }
  const bytes = Uint8Array.from(atob(value), (character) => character.charCodeAt(0));
  if (bytesBase64(bytes) !== value) throw new Error(`${field} is not canonical base64`);
  return bytes;
}

function accountJson(address: string, account: RpcAccount): Readonly<Record<string, unknown>> {
  return Object.freeze({
    address,
    owner: account.owner,
    lamports: account.lamports,
    executable: account.executable,
    dataBase64: bytesBase64(account.data),
  });
}

function accountOrVacantJson(address: string, account: RpcAccount | null): Readonly<Record<string, unknown>> {
  return account === null ? Object.freeze({
    address,
    owner: PublicKey.default.toBase58(),
    lamports: '0',
    executable: false,
    dataBase64: '',
  }) : accountJson(address, account);
}

function requireAccount(observation: AccountInfoObservation, address: string, label: string): RpcAccount {
  if (observation.account === null) throw new Error(`${label} ${address} is absent at finalized slot ${observation.slot}`);
  return observation.account;
}

function accountByAddress(observation: MultipleAccountObservation, address: string, label: string): RpcAccount {
  const found = observation.accounts.find((entry) => entry.address === address);
  if (found?.account === null || found === undefined) throw new Error(`${label} ${address} is absent at finalized slot ${observation.slot}`);
  return found.account;
}

function optionalAccountByAddress(observation: MultipleAccountObservation, address: string): RpcAccount | null {
  return observation.accounts.find((entry) => entry.address === address)?.account ?? null;
}

function parseCoordinates(source: string): Readonly<{
  lifecycle: string; market: string; sourceState: string; resolutionProgram: string; registryProgram: string; pythRelease: string;
  updateAccount: string; updateAuthority: string; refundRecipient: string; certificate: string;
  releaseSet: string; generation: string; terminalSequence: string;
}> {
  const raw = object(parseJson(source, 'Source provider coordinates'), [
    'certificate', 'format', 'generation', 'lifecycle', 'market', 'pythRelease', 'refundRecipient',
    'registryProgram', 'releaseSet', 'resolutionProgram', 'terminalSequence', 'updateAccount',
    'updateAuthority', 'sourceState',
  ], 'Source provider coordinates');
  if (raw.format !== SOURCE_PROVIDER_COORDINATES_FORMAT_V1) throw new Error('Source provider coordinates have another format');
  return Object.freeze({
    lifecycle: key(raw.lifecycle, 'provider lifecycle'),
    market: key(raw.market, 'Market'),
    sourceState: key(raw.sourceState, 'Source state'),
    resolutionProgram: key(raw.resolutionProgram, 'Resolution program'),
    registryProgram: key(raw.registryProgram, 'Registry program'),
    pythRelease: key(raw.pythRelease, 'Pyth release'),
    updateAccount: key(raw.updateAccount, 'Receiver update'),
    updateAuthority: key(raw.updateAuthority, 'update authority'),
    refundRecipient: key(raw.refundRecipient, 'refund recipient'),
    certificate: key(raw.certificate, 'terminal certificate'),
    releaseSet: key(raw.releaseSet, 'release set'),
    generation: unsigned(raw.generation, 'generation'),
    terminalSequence: unsigned(raw.terminalSequence, 'terminal sequence'),
  });
}

function parseProgram(source: string, expected: string): string {
  const raw = object(parseJson(source, 'Source provider Program'), ['format', 'program', 'programdata'], 'Source provider Program');
  if (raw.format !== SOURCE_PROVIDER_PROGRAM_FORMAT_V1 || key(raw.program, 'program') !== expected) throw new Error('Source provider Program output changed its producer');
  return key(raw.programdata, 'ProgramData');
}

function parseSubmitBase(source: string) {
  const raw = object(parseJson(source, 'Source provider submit base'), [
    'coreProgramdata', 'format', 'infrastructure', 'refundRecipient', 'resolutionProgramdata',
    'sourceMaterial', 'sourceState',
  ], 'Source provider submit base');
  if (raw.format !== SOURCE_PROVIDER_SUBMIT_BASE_FORMAT_V1) throw new Error('Source provider submit base has another format');
  return Object.freeze({
    sourceState: key(raw.sourceState, 'Source state'),
    sourceMaterial: key(raw.sourceMaterial, 'SourceMaterial'),
    refundRecipient: key(raw.refundRecipient, 'refund recipient'),
    infrastructure: key(raw.infrastructure, 'infrastructure'),
    coreProgramdata: key(raw.coreProgramdata, 'Core ProgramData'),
    resolutionProgramdata: key(raw.resolutionProgramdata, 'Resolution ProgramData'),
  });
}

function parseSubmitMaterial(source: string) {
  const raw = object(parseJson(source, 'Source provider submit material'), [
    'format', 'registryArtifact', 'registryArtifactStaging', 'sourceSpec', 'sourceSpecStaging',
    'window', 'windowStaging',
  ], 'Source provider submit material');
  if (raw.format !== SOURCE_PROVIDER_SUBMIT_MATERIAL_FORMAT_V1) throw new Error('Source provider submit material has another format');
  return Object.freeze({
    sourceSpec: key(raw.sourceSpec, 'SourceSpec'),
    sourceSpecStaging: key(raw.sourceSpecStaging, 'SourceSpec staging'),
    window: key(raw.window, 'WindowSpec'),
    windowStaging: key(raw.windowStaging, 'WindowSpec staging'),
    registryArtifact: key(raw.registryArtifact, 'Registry artifact'),
    registryArtifactStaging: key(raw.registryArtifactStaging, 'Registry artifact staging'),
  });
}

function parseSubmitRecord(source: string, label: string) {
  const raw = object(parseJson(source, label), ['format', 'raw', 'staging'], label);
  if (raw.format !== SOURCE_PROVIDER_SUBMIT_RECORD_FORMAT_V1) throw new Error(`${label} has another format`);
  return Object.freeze({ raw: key(raw.raw, `${label} raw`), staging: key(raw.staging, `${label} staging`) });
}

function parseSubmitPyth(source: string) {
  const raw = object(parseJson(source, 'Source provider submit Pyth'), [
    'format', 'guardianSet', 'receiverConfig', 'receiverProgram', 'receiverProgramdata',
    'routerProgram', 'routerProgramdata',
  ], 'Source provider submit Pyth');
  if (raw.format !== SOURCE_PROVIDER_SUBMIT_PYTH_FORMAT_V1) throw new Error('Source provider submit Pyth has another format');
  return Object.freeze({
    receiverProgram: key(raw.receiverProgram, 'Receiver program'),
    receiverProgramdata: key(raw.receiverProgramdata, 'Receiver ProgramData'),
    receiverConfig: key(raw.receiverConfig, 'Receiver Config'),
    routerProgram: key(raw.routerProgram, 'Router program'),
    routerProgramdata: key(raw.routerProgramdata, 'Router ProgramData'),
    guardianSet: key(raw.guardianSet, 'GuardianSet'),
  });
}

function parseSubmitFresh(source: string) {
  const raw = object(parseJson(source, 'Source provider submit fresh coordinates'), [
    'format', 'lifecycle', 'updateAuthority',
  ], 'Source provider submit fresh coordinates');
  if (raw.format !== SOURCE_PROVIDER_SUBMIT_FRESH_FORMAT_V1) throw new Error('Source provider submit fresh coordinates have another format');
  return Object.freeze({
    lifecycle: key(raw.lifecycle, 'provider lifecycle'),
    updateAuthority: key(raw.updateAuthority, 'update authority'),
  });
}

export function parseSourceProviderSubmitPlanV1(source: string): SourceProviderSubmitPlanV1 {
  const raw = object(parseJson(source, 'Source provider submit plan'), [
    'completion', 'format', 'instruction', 'lifecycleTopUpLamports', 'loadedAddresses',
    'lookupTables', 'observedSlot', 'poststate', 'requiredSigners', 'route',
    'unsignedMessageBase64', 'wireBytes',
  ], 'Source provider submit plan');
  if (raw.format !== SOURCE_PROVIDER_SUBMIT_PLAN_FORMAT_V1 || raw.route !== 'submit') throw new Error('Source provider submit plan has another format or route');
  const instruction = object(raw.instruction, ['accounts', 'dataBase64', 'program'], 'Source provider submit instruction');
  if (!Array.isArray(instruction.accounts) || instruction.accounts.length !== 38) throw new Error('Source provider submit frame is not exactly 38 accounts');
  const accounts = Object.freeze(instruction.accounts.map((entry, index) => {
    const meta = object(entry, ['address', 'isSigner', 'isWritable'], `Source provider submit account ${index}`);
    if (typeof meta.isSigner !== 'boolean' || typeof meta.isWritable !== 'boolean') throw new Error(`Source provider submit account ${index} privileges are malformed`);
    return Object.freeze({ address: key(meta.address, `Source provider submit account ${index}`), isSigner: meta.isSigner, isWritable: meta.isWritable });
  }));
  if (!Array.isArray(raw.requiredSigners) || raw.requiredSigners.length !== 2) throw new Error('Source provider submit does not have exactly two signers');
  const requiredSigners = raw.requiredSigners.map((entry, index) => key(entry, `submit signer ${index}`));
  if (!Array.isArray(raw.lookupTables) || raw.lookupTables.length !== 1) throw new Error('Source provider submit does not use exactly one frozen table');
  if (!Array.isArray(raw.completion) || raw.completion.length !== 2) throw new Error('Source provider submit completion set is not exact');
  const poststateRaw = object(raw.poststate, [
    'lifecycle', 'receiverProgram', 'resolutionProgram', 'submitRequestBase64', 'updateAccount', 'updateAuthority',
  ], 'Source provider submit poststate');
  const wireBytes = safeUnsigned(raw.wireBytes, 'submit wire bytes');
  if (wireBytes > SOLANA_PACKET_BYTES_V1) throw new Error('Source provider submit exceeds Solana packet size');
  return Object.freeze({
    format: SOURCE_PROVIDER_SUBMIT_PLAN_FORMAT_V1,
    route: 'submit',
    observedSlot: unsigned(raw.observedSlot, 'submit observed slot'),
    instruction: Object.freeze({
      program: key(instruction.program, 'submit instruction program'),
      accounts,
      dataBase64: bytesBase64(base64Bytes(instruction.dataBase64, 'submit instruction data')),
    }),
    unsignedMessageBase64: bytesBase64(base64Bytes(raw.unsignedMessageBase64, 'submit unsigned message')),
    requiredSigners: Object.freeze(requiredSigners) as readonly [string, string],
    wireBytes,
    loadedAddresses: safeUnsigned(raw.loadedAddresses, 'submit loaded addresses'),
    lookupTables: Object.freeze(raw.lookupTables.map((entry) => key(entry, 'submit lookup table'))) as readonly [string],
    lifecycleTopUpLamports: unsigned(raw.lifecycleTopUpLamports, 'lifecycle top-up'),
    completion: Object.freeze(raw.completion.map((entry, index) => key(entry, `submit completion ${index}`))) as readonly [string, string],
    poststate: Object.freeze({
      lifecycle: key(poststateRaw.lifecycle, 'poststate lifecycle'),
      updateAccount: key(poststateRaw.updateAccount, 'poststate update account'),
      updateAuthority: key(poststateRaw.updateAuthority, 'poststate update authority'),
      resolutionProgram: key(poststateRaw.resolutionProgram, 'poststate Resolution program'),
      receiverProgram: key(poststateRaw.receiverProgram, 'poststate Receiver program'),
      submitRequestBase64: bytesBase64(base64Bytes(poststateRaw.submitRequestBase64, 'poststate submit request')),
    }),
  });
}

export function parseSourceProviderReclaimPlanV1(source: string): SourceProviderReclaimPlanV1 {
  const raw = object(parseJson(source, 'Source provider plan'), [
    'completion', 'expectedPoststates', 'format', 'instruction', 'lifecycle', 'loadedAddresses', 'lookupTables',
    'observedSlot', 'requiredSigners', 'route', 'unsignedMessageBase64', 'updateAuthority', 'wireBytes',
  ], 'Source provider plan');
  if (raw.format !== SOURCE_PROVIDER_PLAN_FORMAT_V1 || raw.route !== 'reclaim') throw new Error('Source provider plan has another format or route');
  const instruction = object(raw.instruction, ['accounts', 'dataBase64', 'program'], 'Source provider instruction');
  if (!Array.isArray(instruction.accounts) || instruction.accounts.length !== 18) throw new Error('Source provider reclaim frame is not exactly 18 accounts');
  const accounts = Object.freeze(instruction.accounts.map((entry, index) => {
    const meta = object(entry, ['address', 'isSigner', 'isWritable'], `Source provider account ${index}`);
    if (typeof meta.isSigner !== 'boolean' || typeof meta.isWritable !== 'boolean') throw new Error(`Source provider account ${index} privileges are malformed`);
    return Object.freeze({ address: key(meta.address, `Source provider account ${index}`), isSigner: meta.isSigner, isWritable: meta.isWritable });
  }));
  if (!Array.isArray(raw.requiredSigners) || raw.requiredSigners.length !== 2) throw new Error('Source provider reclaim does not have exactly two signers');
  const requiredSigners = raw.requiredSigners.map((entry, index) => key(entry, `required signer ${index}`)) as [string, string];
  if (!Array.isArray(raw.lookupTables) || !Array.isArray(raw.completion) || raw.completion.length !== 4) throw new Error('Source provider routing or completion set is malformed');
  if (!Array.isArray(raw.expectedPoststates) || raw.expectedPoststates.length !== 4) throw new Error('Source provider expected poststate set is not exactly four accounts');
  const expectedPoststates = Object.freeze(raw.expectedPoststates.map((entry, index) => {
    const account = object(entry, ['address', 'dataBase64', 'executable', 'lamports', 'owner'], `expected poststate ${index}`);
    if (typeof account.executable !== 'boolean') throw new Error(`expected poststate ${index} executable flag is malformed`);
    return Object.freeze({
      address: key(account.address, `expected poststate ${index} address`),
      owner: key(account.owner, `expected poststate ${index} owner`),
      lamports: unsigned(account.lamports, `expected poststate ${index} lamports`),
      executable: account.executable,
      dataBase64: bytesBase64(base64Bytes(account.dataBase64, `expected poststate ${index} data`)),
    });
  }));
  const wireBytes = safeUnsigned(raw.wireBytes, 'wire bytes');
  if (wireBytes > SOLANA_PACKET_BYTES_V1) throw new Error('Source provider plan exceeds Solana packet size');
  return Object.freeze({
    format: SOURCE_PROVIDER_PLAN_FORMAT_V1,
    route: 'reclaim',
    observedSlot: unsigned(raw.observedSlot, 'observed slot'),
    instruction: Object.freeze({ program: key(instruction.program, 'instruction program'), accounts, dataBase64: bytesBase64(base64Bytes(instruction.dataBase64, 'instruction data')) }),
    unsignedMessageBase64: bytesBase64(base64Bytes(raw.unsignedMessageBase64, 'unsigned message')),
    requiredSigners: Object.freeze(requiredSigners) as readonly [string, string],
    wireBytes,
    loadedAddresses: safeUnsigned(raw.loadedAddresses, 'loaded addresses'),
    lookupTables: Object.freeze(raw.lookupTables.map((entry, index) => key(entry, `lookup table ${index}`))),
    lifecycle: key(raw.lifecycle, 'plan lifecycle'),
    updateAuthority: key(raw.updateAuthority, 'plan update authority'),
    completion: Object.freeze(raw.completion.map((entry, index) => key(entry, `completion account ${index}`))),
    expectedPoststates,
  });
}

/** Load only the generated blob whose Rust-derived digest and size match. */
export async function loadSourceProviderWasmV1(
  fetcher: typeof fetch = (input, init) => globalThis.fetch(input, init),
): Promise<SourceProviderWasmV1> {
  const wasmModule = await import('./generated/sourceProviderWasm/source_provider.js');
  const url = new URL('./generated/sourceProviderWasm/source_provider_bg.wasm', import.meta.url);
  const response = await fetcher(url);
  if (!response.ok) throw new Error(`Source provider WASM fetch failed with HTTP ${response.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length !== SOURCE_PROVIDER_WASM_BYTES_V1 || hex(await sha256(bytes)) !== SOURCE_PROVIDER_WASM_SHA256_V1) {
    throw new Error('Source provider WASM bytes do not match the generated Rust artifact identity');
  }
  await wasmModule.default({ module_or_path: bytes });
  return Object.freeze({
    derive_source_provider_reclaim_coordinates_v1: wasmModule.derive_source_provider_reclaim_coordinates_v1,
    derive_source_provider_programdata_v1: wasmModule.derive_source_provider_programdata_v1,
    plan_source_provider_reclaim_v1: wasmModule.plan_source_provider_reclaim_v1,
    derive_source_provider_submit_base_v1: wasmModule.derive_source_provider_submit_base_v1,
    derive_source_provider_submit_material_v1: wasmModule.derive_source_provider_submit_material_v1,
    derive_source_provider_submit_provider_release_v1: wasmModule.derive_source_provider_submit_provider_release_v1,
    derive_source_provider_submit_pyth_release_v1: wasmModule.derive_source_provider_submit_pyth_release_v1,
    derive_source_provider_submit_pyth_v1: wasmModule.derive_source_provider_submit_pyth_v1,
    read_source_provider_price_update_v1: wasmModule.read_source_provider_price_update_v1,
    derive_source_provider_submit_fresh_v1: wasmModule.derive_source_provider_submit_fresh_v1,
    plan_source_provider_submit_v1: wasmModule.plan_source_provider_submit_v1,
    verify_source_provider_submit_poststate_v1: wasmModule.verify_source_provider_submit_poststate_v1,
  });
}

/** Reacquire one complete provider submission and compile its exact two-signer message. */
export async function acquireSourceProviderSubmitV1(
  client: SourceProviderRpcV1,
  wasm: SourceProviderWasmV1,
  input: SourceProviderSubmitInputV1,
  programs: SourceProviderSubmitProgramsV1,
  update: Keypair = Keypair.generate(),
): Promise<SourceProviderSubmitAcquisitionV1> {
  const market = key(input.market, 'Market');
  const payer = key(input.payer, 'submitter wallet');
  const encodedVaa = key(input.encodedVaa, 'verified EncodedVaa');
  const lookupTable = key(input.lookupTable, 'provider lookup table');
  const coreProgram = key(programs.coreProgram, 'Core program');
  const registryProgram = key(programs.registryProgram, 'Registry program');
  const resolutionProgram = key(programs.resolutionProgram, 'Resolution program');
  const reclaimAfterUnixSeconds = unsigned(input.reclaimAfterUnixSeconds, 'reclaim-after timestamp');
  const postUpdateBodyBase64 = bytesBase64(base64Bytes(input.postUpdateBodyBase64, 'post-update body'));
  const firstFloor = await client.finalizedSlot();
  const firstMarket = await client.accountInfo(market, firstFloor);
  const firstMarketAccount = requireAccount(firstMarket, market, 'Market');
  const base = parseSubmitBase(wasm.derive_source_provider_submit_base_v1(JSON.stringify({
    format: SOURCE_PROVIDER_SUBMIT_BASE_INPUT_FORMAT_V1,
    market: accountJson(market, firstMarketAccount),
    coreProgram,
    registryProgram,
    resolutionProgram,
  })));
  const discovery = await client.multipleAccounts([
    market, base.sourceState, base.sourceMaterial, base.infrastructure,
    coreProgram, registryProgram, resolutionProgram,
  ], firstMarket.slot);
  const material = parseSubmitMaterial(wasm.derive_source_provider_submit_material_v1(JSON.stringify({
    format: SOURCE_PROVIDER_SUBMIT_MATERIAL_INPUT_FORMAT_V1,
    market: accountJson(market, accountByAddress(discovery, market, 'Market')),
    sourceMaterial: accountJson(base.sourceMaterial, accountByAddress(discovery, base.sourceMaterial, 'SourceMaterial')),
    infrastructure: accountJson(base.infrastructure, accountByAddress(discovery, base.infrastructure, 'infrastructure')),
  })));
  const sourceSpecObservation = await client.accountInfo(material.sourceSpec, discovery.slot);
  const sourceSpec = requireAccount(sourceSpecObservation, material.sourceSpec, 'SourceSpec');
  const providerRelease = parseSubmitRecord(wasm.derive_source_provider_submit_provider_release_v1(JSON.stringify({
    format: SOURCE_PROVIDER_SUBMIT_RECORD_INPUT_FORMAT_V1,
    registryProgram,
    record: accountJson(material.sourceSpec, sourceSpec),
  })), 'ProviderRelease coordinates');
  const providerObservation = await client.accountInfo(providerRelease.raw, sourceSpecObservation.slot);
  const provider = requireAccount(providerObservation, providerRelease.raw, 'ProviderRelease');
  const pythRelease = parseSubmitRecord(wasm.derive_source_provider_submit_pyth_release_v1(JSON.stringify({
    format: SOURCE_PROVIDER_SUBMIT_RECORD_INPUT_FORMAT_V1,
    registryProgram,
    record: accountJson(providerRelease.raw, provider),
  })), 'Pyth release coordinates');
  const pythDiscovery = await client.multipleAccounts([pythRelease.raw, encodedVaa], providerObservation.slot);
  const pyth = parseSubmitPyth(wasm.derive_source_provider_submit_pyth_v1(JSON.stringify({
    format: SOURCE_PROVIDER_SUBMIT_PYTH_INPUT_FORMAT_V1,
    registryProgram,
    pythRelease: accountJson(pythRelease.raw, accountByAddress(pythDiscovery, pythRelease.raw, 'Pyth release')),
    encodedVaa: accountJson(encodedVaa, accountByAddress(pythDiscovery, encodedVaa, 'verified EncodedVaa')),
  })));
  const fresh = parseSubmitFresh(wasm.derive_source_provider_submit_fresh_v1(JSON.stringify({
    format: SOURCE_PROVIDER_SUBMIT_FRESH_INPUT_FORMAT_V1,
    market,
    sourceState: base.sourceState,
    updateAccount: update.publicKey.toBase58(),
    resolutionProgram,
  })));
  const programHintObservation = await client.multipleAccounts([
    coreProgram, registryProgram, resolutionProgram, pyth.receiverProgram, pyth.routerProgram,
  ], pythDiscovery.slot);
  const deriveProgramdata = (address: string, account: RpcAccount) => parseProgram(wasm.derive_source_provider_programdata_v1(JSON.stringify({
    format: SOURCE_PROVIDER_PROGRAM_INPUT_FORMAT_V1,
    program: accountJson(address, account),
  })), address);
  const coreProgramdata = deriveProgramdata(coreProgram, accountByAddress(programHintObservation, coreProgram, 'Core program'));
  const registryProgramdata = deriveProgramdata(registryProgram, accountByAddress(programHintObservation, registryProgram, 'Registry program'));
  const resolutionProgramdata = deriveProgramdata(resolutionProgram, accountByAddress(programHintObservation, resolutionProgram, 'Resolution program'));
  const receiverProgramdata = deriveProgramdata(pyth.receiverProgram, accountByAddress(programHintObservation, pyth.receiverProgram, 'Receiver program'));
  const routerProgramdata = deriveProgramdata(pyth.routerProgram, accountByAddress(programHintObservation, pyth.routerProgram, 'Router program'));
  if (coreProgramdata !== base.coreProgramdata || resolutionProgramdata !== base.resolutionProgramdata
      || receiverProgramdata !== pyth.receiverProgramdata || routerProgramdata !== pyth.routerProgramdata) {
    throw new Error('provider program-to-ProgramData links differ from the selected Market or Pyth release');
  }
  const finalFloor = await client.finalizedSlot();
  const addresses = Object.freeze([
    market, base.sourceState, base.sourceMaterial, material.sourceSpec, providerRelease.raw,
    pythRelease.raw, material.window, encodedVaa, update.publicKey.toBase58(), fresh.lifecycle,
    base.infrastructure, coreProgram, coreProgramdata, registryProgram, registryProgramdata,
    resolutionProgram, resolutionProgramdata, material.registryArtifact,
    material.registryArtifactStaging, material.sourceSpecStaging, providerRelease.staging,
    pythRelease.staging, material.windowStaging, pyth.receiverProgram, receiverProgramdata,
    pyth.receiverConfig, pyth.routerProgram, routerProgramdata, pyth.guardianSet, lookupTable,
  ]);
  const [frame, blockhash, rent] = await Promise.all([
    client.multipleAccounts(addresses, finalFloor),
    client.latestMutationBlockhash(finalFloor),
    client.minimumBalanceForRentExemption(SOURCE_PROVIDER_SUBMIT_LIFECYCLE_BYTES_V1),
  ]);
  const exactMarket = accountByAddress(frame, market, 'Market');
  const exactBase = parseSubmitBase(wasm.derive_source_provider_submit_base_v1(JSON.stringify({
    format: SOURCE_PROVIDER_SUBMIT_BASE_INPUT_FORMAT_V1,
    market: accountJson(market, exactMarket), coreProgram, registryProgram, resolutionProgram,
  })));
  if (JSON.stringify(exactBase) !== JSON.stringify(base)) throw new Error('provider submit Market graph changed during exact reacquisition');
  const exactMaterial = parseSubmitMaterial(wasm.derive_source_provider_submit_material_v1(JSON.stringify({
    format: SOURCE_PROVIDER_SUBMIT_MATERIAL_INPUT_FORMAT_V1,
    market: accountJson(market, exactMarket),
    sourceMaterial: accountJson(base.sourceMaterial, accountByAddress(frame, base.sourceMaterial, 'SourceMaterial')),
    infrastructure: accountJson(base.infrastructure, accountByAddress(frame, base.infrastructure, 'infrastructure')),
  })));
  if (JSON.stringify(exactMaterial) !== JSON.stringify(material)) throw new Error('provider submit record graph changed during exact reacquisition');
  const exactCoreProgramdata = deriveProgramdata(coreProgram, accountByAddress(frame, coreProgram, 'Core program'));
  const exactRegistryProgramdata = deriveProgramdata(registryProgram, accountByAddress(frame, registryProgram, 'Registry program'));
  const exactResolutionProgramdata = deriveProgramdata(resolutionProgram, accountByAddress(frame, resolutionProgram, 'Resolution program'));
  const exactReceiverProgramdata = deriveProgramdata(pyth.receiverProgram, accountByAddress(frame, pyth.receiverProgram, 'Receiver program'));
  const exactRouterProgramdata = deriveProgramdata(pyth.routerProgram, accountByAddress(frame, pyth.routerProgram, 'Router program'));
  if (exactCoreProgramdata !== coreProgramdata || exactRegistryProgramdata !== registryProgramdata
      || exactResolutionProgramdata !== resolutionProgramdata || exactReceiverProgramdata !== receiverProgramdata
      || exactRouterProgramdata !== routerProgramdata) {
    throw new Error('provider program-to-ProgramData links changed during exact reacquisition');
  }
  const timestamp = await client.blockTime(frame.slot);
  if (timestamp === null) throw new Error(`finalized slot ${frame.slot} has no authenticated block time; no provider submit was planned`);
  const inputJson = JSON.stringify({
    format: SOURCE_PROVIDER_SUBMIT_INPUT_FORMAT_V1,
    observedSlot: frame.slot,
    unixTimestamp: timestamp,
    recentBlockhash: blockhash.blockhash,
    reclaimAfterUnixSeconds,
    postUpdateBodyBase64,
    lifecycleRentMinimum: rent.lamports,
    market: accountJson(market, exactMarket),
    sourceState: accountJson(base.sourceState, accountByAddress(frame, base.sourceState, 'Source state')),
    sourceMaterial: accountJson(base.sourceMaterial, accountByAddress(frame, base.sourceMaterial, 'SourceMaterial')),
    sourceSpec: accountJson(material.sourceSpec, accountByAddress(frame, material.sourceSpec, 'SourceSpec')),
    sourceProviderRelease: accountJson(providerRelease.raw, accountByAddress(frame, providerRelease.raw, 'ProviderRelease')),
    pythRelease: accountJson(pythRelease.raw, accountByAddress(frame, pythRelease.raw, 'Pyth release')),
    window: accountJson(material.window, accountByAddress(frame, material.window, 'WindowSpec')),
    encodedVaa: accountJson(encodedVaa, accountByAddress(frame, encodedVaa, 'verified EncodedVaa')),
    updatePrestate: accountOrVacantJson(update.publicKey.toBase58(), optionalAccountByAddress(frame, update.publicKey.toBase58())),
    lifecyclePrestate: accountOrVacantJson(fresh.lifecycle, optionalAccountByAddress(frame, fresh.lifecycle)),
    deployment: {
      submitter: payer,
      refundRecipient: base.refundRecipient,
      updateAccount: update.publicKey.toBase58(),
      infrastructure: base.infrastructure,
      registryProgramdata: exactRegistryProgramdata,
      registryArtifact: material.registryArtifact,
      registryArtifactStaging: material.registryArtifactStaging,
      coreProgramdata: exactCoreProgramdata,
      resolutionProgram,
      resolutionProgramdata: exactResolutionProgramdata,
      receiverConfig: pyth.receiverConfig,
      guardianSet: pyth.guardianSet,
      receiverProgram: pyth.receiverProgram,
    },
    lookupTable: accountJson(lookupTable, accountByAddress(frame, lookupTable, 'provider lookup table')),
  });
  const planJson = wasm.plan_source_provider_submit_v1(inputJson);
  const plan = parseSourceProviderSubmitPlanV1(planJson);
  if (plan.observedSlot !== frame.slot || plan.requiredSigners[0] !== payer
      || plan.requiredSigners[1] !== update.publicKey.toBase58()
      || plan.lookupTables[0] !== lookupTable || plan.poststate.lifecycle !== fresh.lifecycle
      || plan.poststate.updateAuthority !== fresh.updateAuthority) {
    throw new Error('Source provider submit plan changed its observation, authority, table, or fresh coordinates');
  }
  const transaction = new VersionedTransaction(VersionedMessage.deserialize(base64Bytes(plan.unsignedMessageBase64, 'submit unsigned message')));
  if (transaction.signatures.length !== 2
      || transaction.message.staticAccountKeys[0]?.toBase58() !== payer
      || transaction.message.staticAccountKeys[1]?.toBase58() !== update.publicKey.toBase58()
      || transaction.serialize().length !== plan.wireBytes) {
    throw new Error('browser reconstruction differs from the Rust-owned provider submit geometry');
  }
  return Object.freeze({
    plan, planJson, inputJson, transaction, update, payer, market,
    lastValidBlockHeight: blockhash.lastValidBlockHeight,
    observationAddresses: addresses,
  });
}

/** Reauthenticate the lifecycle/update poststate through the Rust/WASM owner. */
export async function sourceProviderSubmitPoststateCompletesV1(
  client: Pick<SolanaRpcClient, 'multipleAccounts'>,
  wasm: SourceProviderWasmV1,
  plan: SourceProviderSubmitPlanV1,
  minimumSlot: string,
): Promise<boolean> {
  const observation = await client.multipleAccounts(plan.completion, minimumSlot);
  const raw = object(parseJson(wasm.verify_source_provider_submit_poststate_v1(JSON.stringify({
    format: SOURCE_PROVIDER_SUBMIT_POSTSTATE_INPUT_FORMAT_V1,
    expectation: plan.poststate,
    lifecycle: observation.accounts[0]?.account === null || observation.accounts[0] === undefined
      ? null : accountJson(plan.completion[0], observation.accounts[0].account),
    update: observation.accounts[1]?.account === null || observation.accounts[1] === undefined
      ? null : accountJson(plan.completion[1], observation.accounts[1].account),
  })), 'Source provider submit poststate'), ['complete', 'format'], 'Source provider submit poststate');
  if (raw.format !== SOURCE_PROVIDER_SUBMIT_POSTSTATE_FORMAT_V1 || typeof raw.complete !== 'boolean') throw new Error('Source provider submit poststate verifier changed its format');
  return raw.complete;
}

/** Reacquire one consumed lifecycle and compile its exact permissionless reclaim. */
export async function acquireSourceProviderReclaimV1(
  client: SourceProviderRpcV1,
  wasm: SourceProviderWasmV1,
  lifecycleAddress: string,
  payerAddress: string,
  programs: SourceProviderProgramsV1,
  resolver: Keypair = Keypair.generate(),
): Promise<SourceProviderReclaimAcquisitionV1> {
  const lifecycle = key(lifecycleAddress, 'provider lifecycle');
  const payer = key(payerAddress, 'wallet payer');
  const registryProgram = key(programs.registryProgram, 'Registry program');
  const resolutionProgram = key(programs.resolutionProgram, 'Resolution program');
  const floor = await client.finalizedSlot();
  const firstLifecycle = await client.accountInfo(lifecycle, floor);
  const firstLifecycleAccount = requireAccount(firstLifecycle, lifecycle, 'provider lifecycle');
  const coordinates = parseCoordinates(wasm.derive_source_provider_reclaim_coordinates_v1(JSON.stringify({
    format: SOURCE_PROVIDER_COORDINATES_INPUT_FORMAT_V1,
    lifecycle: accountJson(lifecycle, firstLifecycleAccount),
  })));
  if (coordinates.registryProgram !== registryProgram || coordinates.resolutionProgram !== resolutionProgram) {
    throw new Error('provider lifecycle belongs to another configured Registry or Resolution deployment');
  }
  const programsObservation = await client.multipleAccounts([registryProgram, resolutionProgram], firstLifecycle.slot);
  const registryAccount = accountByAddress(programsObservation, registryProgram, 'Registry program');
  const resolutionAccount = accountByAddress(programsObservation, resolutionProgram, 'Resolution program');
  const deriveProgramdata = (address: string, account: RpcAccount) => parseProgram(wasm.derive_source_provider_programdata_v1(JSON.stringify({
    format: SOURCE_PROVIDER_PROGRAM_INPUT_FORMAT_V1,
    program: accountJson(address, account),
  })), address);
  const registryProgramdata = deriveProgramdata(registryProgram, registryAccount);
  const resolutionProgramdata = deriveProgramdata(resolutionProgram, resolutionAccount);
  const exactFloor = await client.finalizedSlot();
  const [ordinary, registryElf, resolutionElf, blockhash] = await Promise.all([
    client.multipleAccounts([
      lifecycle, coordinates.pythRelease, registryProgram, resolutionProgram,
      coordinates.updateAccount, coordinates.updateAuthority, coordinates.refundRecipient,
      coordinates.certificate,
    ], exactFloor),
    client.accountInfo(registryProgramdata, exactFloor),
    client.accountInfo(resolutionProgramdata, exactFloor),
    client.latestMutationBlockhash(exactFloor),
  ]);
  if (ordinary.slot !== registryElf.slot || ordinary.slot !== resolutionElf.slot) throw new Error('finalized slot advanced during the split provider release read; read reclaim again');
  requireAccount(registryElf, registryProgramdata, 'Registry ProgramData');
  requireAccount(resolutionElf, resolutionProgramdata, 'Resolution ProgramData');
  const exactLifecycle = accountByAddress(ordinary, lifecycle, 'provider lifecycle');
  const exactRegistry = accountByAddress(ordinary, registryProgram, 'Registry program');
  const exactResolution = accountByAddress(ordinary, resolutionProgram, 'Resolution program');
  if (deriveProgramdata(registryProgram, exactRegistry) !== registryProgramdata
      || deriveProgramdata(resolutionProgram, exactResolution) !== resolutionProgramdata) {
    throw new Error('program-to-ProgramData link changed during provider acquisition');
  }
  const exactCoordinates = parseCoordinates(wasm.derive_source_provider_reclaim_coordinates_v1(JSON.stringify({
    format: SOURCE_PROVIDER_COORDINATES_INPUT_FORMAT_V1,
    lifecycle: accountJson(lifecycle, exactLifecycle),
  })));
  if (JSON.stringify(exactCoordinates) !== JSON.stringify(coordinates)) throw new Error('provider lifecycle coordinates changed during exact reacquisition');
  const timestamp = await client.blockTime(ordinary.slot);
  if (timestamp === null) throw new Error(`finalized slot ${ordinary.slot} has no authenticated block time; no reclaim was planned`);
  const inputJson = JSON.stringify({
    format: SOURCE_PROVIDER_RECLAIM_INPUT_FORMAT_V1,
    observedSlot: ordinary.slot,
    unixTimestamp: timestamp,
    recentBlockhash: blockhash.blockhash,
    lifecycle: accountJson(lifecycle, exactLifecycle),
    pythRelease: accountJson(coordinates.pythRelease, accountByAddress(ordinary, coordinates.pythRelease, 'Pyth release')),
    update: accountJson(coordinates.updateAccount, accountByAddress(ordinary, coordinates.updateAccount, 'Receiver update')),
    updateAuthority: accountJson(coordinates.updateAuthority, accountByAddress(ordinary, coordinates.updateAuthority, 'update authority')),
    refundRecipient: accountJson(coordinates.refundRecipient, accountByAddress(ordinary, coordinates.refundRecipient, 'refund recipient')),
    certificate: accountJson(coordinates.certificate, accountByAddress(ordinary, coordinates.certificate, 'terminal certificate')),
    deployment: {
      payer,
      resolver: resolver.publicKey.toBase58(),
      registryProgramdata,
      resolutionProgram,
      resolutionProgramdata,
    },
    lookupTable: null,
  });
  const planJson = wasm.plan_source_provider_reclaim_v1(inputJson);
  const plan = parseSourceProviderReclaimPlanV1(planJson);
  if (plan.observedSlot !== ordinary.slot || plan.lifecycle !== lifecycle
      || plan.requiredSigners[0] !== payer || plan.requiredSigners[1] !== resolver.publicKey.toBase58()
      || plan.lookupTables.length !== 0 || plan.loadedAddresses !== 0) {
    throw new Error('Source provider reclaim plan changed its observation, authority, lifecycle, or inline routing');
  }
  const messageBytes = base64Bytes(plan.unsignedMessageBase64, 'unsigned message');
  const transaction = new VersionedTransaction(VersionedMessage.deserialize(messageBytes));
  if (transaction.signatures.length !== 2
      || transaction.message.staticAccountKeys[0]?.toBase58() !== payer
      || transaction.message.staticAccountKeys[1]?.toBase58() !== resolver.publicKey.toBase58()
      || transaction.serialize().length !== plan.wireBytes) {
    throw new Error('browser reconstruction differs from the Rust-owned provider message geometry');
  }
  return Object.freeze({
    plan,
    planJson,
    inputJson,
    transaction,
    resolver,
    payer,
    market: coordinates.market,
    lastValidBlockHeight: blockhash.lastValidBlockHeight,
    observationAddresses: Object.freeze([
      lifecycle, coordinates.pythRelease, registryProgram, registryProgramdata, resolutionProgram,
      resolutionProgramdata, coordinates.updateAccount, coordinates.updateAuthority,
      coordinates.refundRecipient, coordinates.certificate,
    ]),
  });
}

/** Compare one finalized reclaim read against the Rust owner's exact bytes. */
export async function sourceProviderReclaimPoststateCompletesV1(
  client: Pick<SolanaRpcClient, 'multipleAccounts'>,
  plan: SourceProviderReclaimPlanV1,
  minimumSlot: string,
): Promise<boolean> {
  const observation = await client.multipleAccounts(plan.completion, minimumSlot);
  return plan.expectedPoststates.every((expected, index) => {
    const entry = observation.accounts[index];
    if (entry?.address !== expected.address) return false;
    const account = entry.account;
    if (expected.lamports === '0' && expected.dataBase64 === '' && account === null) return true;
    return account !== null
      && account.owner === expected.owner
      && account.lamports === expected.lamports
      && account.executable === expected.executable
      && bytesBase64(account.data) === expected.dataBase64;
  });
}

/**
 * One sponsored price, read through the Source family's own decoder.
 *
 * Exact by construction and float-free: `price` and `exponent` are carried as
 * the account states them, and `decimal` is the two divided in integers. A
 * spot of 10003917148 at exponent -8 is the string `100.03917148` and never a
 * double that is nearly that.
 */
export type SponsoredPriceV1 = Readonly<{
  address: string;
  feedId: string;
  price: bigint;
  confidence: bigint;
  exponent: number;
  publishTimeUnixSeconds: bigint;
  postedSlot: string;
  /** The price as an exact decimal string, sign included. */
  decimal: string;
}>;

/** Format a scaled integer exactly, without touching floating point. */
function exactDecimalV1(value: bigint, exponent: number): string {
  if (exponent >= 0) return (value * 10n ** BigInt(exponent)).toString();
  const scale = 10n ** BigInt(-exponent);
  const negative = value < 0n;
  const magnitude = negative ? -value : value;
  const whole = magnitude / scale;
  const fraction = (magnitude % scale).toString().padStart(-exponent, '0').replace(/0+$/, '');
  return `${negative ? '-' : ''}${whole}${fraction === '' ? '' : `.${fraction}`}`;
}

/**
 * Read one sponsored `PriceUpdateV2` account.
 *
 * The receiver program is REQUIRED and is checked inside the WASM: a 134-byte
 * account carrying the right discriminator is not a price unless the program
 * that maintains it says so, and a browser that decoded one anyway would be
 * reading a shape rather than a fact.
 */
export async function readSponsoredPriceV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>,
  request: Readonly<{ priceUpdateAddress: string; receiverProgram: string }>,
  /** The transport that fetches the WASM. Node has no `fetch` for a file URL. */
  transport?: typeof fetch,
): Promise<SponsoredPriceV1> {
  const wasm = await loadSourceProviderWasmV1(transport);
  const floor = await client.finalizedSlot();
  const observation = await client.multipleAccounts([request.priceUpdateAddress], floor);
  const account = observation.accounts[0]?.account ?? null;
  if (account === null) throw new Error(`no sponsored price update exists at ${request.priceUpdateAddress}`);
  const raw = object(parseJson(wasm.read_source_provider_price_update_v1(JSON.stringify({
    format: SOURCE_PROVIDER_PRICE_INPUT_FORMAT_V1,
    receiverProgram: key(request.receiverProgram, 'Receiver program'),
    priceUpdate: accountJson(request.priceUpdateAddress, account),
  })), 'Source provider price'), [
    'address', 'confidence', 'exponent', 'feedId', 'format', 'postedSlot', 'price', 'publishTime',
  ], 'Source provider price');
  if (raw.format !== SOURCE_PROVIDER_PRICE_FORMAT_V1) throw new Error('Source provider price has another format');
  const exponent = Number(raw.exponent);
  if (!Number.isSafeInteger(exponent)) throw new Error('Source provider price exponent is not a whole number');
  const price = BigInt(String(raw.price));
  return Object.freeze({
    address: key(raw.address, 'price update'),
    feedId: String(raw.feedId),
    price,
    confidence: BigInt(String(raw.confidence)),
    exponent,
    publishTimeUnixSeconds: BigInt(String(raw.publishTime)),
    postedSlot: String(raw.postedSlot),
    decimal: exactDecimalV1(price, exponent),
  });
}
