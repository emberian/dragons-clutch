import {
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import {
  SOURCE_CLOSE_DETAIL_FORMAT_V1,
  SOURCE_CLOSE_PLAN_FORMAT_V1,
  SOURCE_CLOSE_SNAPSHOT_FORMAT_V1,
  SOURCE_CLOSE_VERIFY_FORMAT_V1,
} from './generated/sourceReadinessWasmV1';
import type { LatestBlockhashObservation, RpcAccount, SolanaRpcClient } from './rpc';
import { SOLANA_PACKET_BYTES_V1 } from './solanaLimits';
import {
  acquireSourceReadinessFrameV1,
  type SourceReadinessProgramsV1,
  type SourceReadinessWasmV1,
} from './sourceReadinessV1';

const CLOCK_SYSVAR = 'SysvarC1ock11111111111111111111111111111111';
const RENT_SYSVAR = 'SysvarRent111111111111111111111111111111111';
const SYSTEM_PROGRAM = SystemProgram.programId.toBase58();
const MAX_JSON = 64 * 1024 * 1024;

export type SourceCloseFundPlanV1 = Readonly<{
  format: typeof SOURCE_CLOSE_PLAN_FORMAT_V1;
  route: 'prepay' | 'close';
  observedSlot: string;
  instruction: Readonly<{
    program: string;
    accounts: ReadonlyArray<Readonly<{ address: string; isSigner: false; isWritable: boolean }>>;
    dataBase64: string;
  }> | null;
  prepay: Readonly<{ destination: string; lamports: string }> | null;
  accounts: Readonly<{ protocolWritable: ReadonlyArray<string>; completion: ReadonlyArray<string> }> | null;
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

export type SourceCloseFundAcquisitionV1 = Readonly<{
  plan: SourceCloseFundPlanV1;
  planJson: string;
  snapshotJson: string;
  observationAddresses: ReadonlyArray<string>;
}>;

export type SourceCloseFundTransactionV1 = Readonly<{
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  payer: string;
  route: 'prepay' | 'close';
  observedSlot: string;
  lastValidBlockHeight: string;
}>;

type CloseRpcV1 = Pick<SolanaRpcClient, 'finalizedSlot' | 'accountInfo' | 'multipleAccounts' | 'blockTime'>;

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
  for (let offset = 0; offset < bytes.length; offset += 8_192) binary += String.fromCharCode(...bytes.subarray(offset, offset + 8_192));
  return btoa(binary);
}

function base64(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length % 4 !== 0
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) throw new Error(`${label} is not canonical base64`);
  let bytes: Uint8Array;
  try { bytes = Uint8Array.from(atob(value), (character) => character.charCodeAt(0)); } catch { throw new Error(`${label} is not canonical base64`); }
  if (bytesBase64(bytes) !== value) throw new Error(`${label} is not canonical base64`);
  return value;
}

function pair(value: unknown, label: string): Readonly<{ raw: string; staging: string }> {
  const raw = object(value, ['raw', 'staging'], label);
  return Object.freeze({ raw: key(raw.raw, `${label}.raw`), staging: key(raw.staging, `${label}.staging`) });
}

function accountJson(address: string, account: RpcAccount | null): Readonly<Record<string, unknown>> {
  return account === null
    ? Object.freeze({ address, owner: SYSTEM_PROGRAM, lamports: '0', executable: false, dataBase64: '' })
    : Object.freeze({ address, owner: account.owner, lamports: account.lamports,
      executable: account.executable, dataBase64: bytesBase64(account.data) });
}

function observedAccount(source: ReadonlyArray<Record<string, unknown>>, address: string, label: string): Record<string, unknown> {
  const value = source.find((entry) => entry.address === address);
  if (value === undefined) throw new Error(`${label} was omitted from the staged Source observation`);
  return value;
}

/** Hostile-decode the Rust-owned receipt-prepay or direct-close plan. */
export function parseSourceCloseFundPlanV1(source: string): SourceCloseFundPlanV1 {
  const raw = object(json(source, 'Source close plan'),
    ['accounts', 'facts', 'format', 'geometry', 'instruction', 'observedSlot', 'prepay', 'route'], 'Source close plan');
  if (raw.format !== SOURCE_CLOSE_PLAN_FORMAT_V1 || !['prepay', 'close'].includes(String(raw.route))) throw new Error('Source close plan changed format or route');
  if (!plain(raw.facts)) throw new Error('Source close facts are not one object');
  const facts: Record<string, string> = {};
  for (const [name, value] of Object.entries(raw.facts)) {
    if (!/^[A-Za-z][A-Za-z0-9]{0,63}$/.test(name) || typeof value !== 'string' || value.length > 256) throw new Error('Source close fact is malformed');
    facts[name] = value;
  }
  const prepay = raw.prepay === null ? null : (() => {
    const value = object(raw.prepay, ['destination', 'lamports'], 'Source close prepay');
    return Object.freeze({ destination: key(value.destination, 'prepay destination'), lamports: u64(value.lamports, 'prepay lamports') });
  })();
  const instruction = raw.instruction === null ? null : (() => {
    const value = object(raw.instruction, ['accounts', 'dataBase64', 'program'], 'Source close instruction');
    if (!Array.isArray(value.accounts) || ![19, 21].includes(value.accounts.length)) throw new Error('Source close instruction changed its 19/21 account frame');
    const accounts = value.accounts.map((entry, index) => {
      const meta = object(entry, ['address', 'isSigner', 'isWritable'], `Source close meta ${index}`);
      if (meta.isSigner !== false || typeof meta.isWritable !== 'boolean') throw new Error(`Source close meta ${index} changed authority`);
      return Object.freeze({ address: key(meta.address, `Source close meta ${index}`), isSigner: false as const, isWritable: meta.isWritable });
    });
    return Object.freeze({ program: key(value.program, 'Source close program'), accounts: Object.freeze(accounts), dataBase64: base64(value.dataBase64, 'Source close data') });
  })();
  const accounts = raw.accounts === null ? null : (() => {
    const value = object(raw.accounts, ['completion', 'protocolWritable'], 'Source close account sets');
    if (!Array.isArray(value.completion) || !Array.isArray(value.protocolWritable)) throw new Error('Source close account sets changed shape');
    return Object.freeze({ completion: Object.freeze(value.completion.map((entry, index) => key(entry, `completion ${index}`))),
      protocolWritable: Object.freeze(value.protocolWritable.map((entry, index) => key(entry, `writable ${index}`))) });
  })();
  const geometry = raw.geometry === null ? null : (() => {
    const value = object(raw.geometry, ['protocolAccountCount', 'protocolDataLen', 'protocolSignerCount', 'protocolUniqueAccountCount',
      'protocolWritableCount', 'transactionInstructionCountWithoutComputeBudget', 'transactionLockCountWithoutPayer'], 'Source close geometry');
    return Object.freeze(Object.fromEntries(Object.entries(value).map(([name, entry]) => [name, count(entry, `geometry.${name}`)]))) as NonNullable<SourceCloseFundPlanV1['geometry']>;
  })();
  if ((raw.route === 'prepay') !== (prepay !== null) || (raw.route === 'close') !== (instruction !== null)
      || (raw.route === 'prepay') !== (accounts === null && geometry === null)
      || (geometry !== null && geometry.protocolSignerCount !== 0)) throw new Error('Source close route disagrees with executable fields');
  if (raw.route === 'close' && (accounts?.completion.length !== 4 || geometry === null
      || ![19, 21].includes(geometry.protocolAccountCount))) throw new Error('Source close completion or geometry changed');
  return Object.freeze({ format: SOURCE_CLOSE_PLAN_FORMAT_V1, route: raw.route as 'prepay' | 'close',
    observedSlot: u64(raw.observedSlot, 'observed slot'), instruction, prepay, accounts, geometry, facts: Object.freeze(facts) });
}

/** Reacquire the exact Retiring Source graph and select prepay or direct close. */
export async function acquireSourceCloseFundV1(
  client: CloseRpcV1,
  wasm: SourceReadinessWasmV1,
  market: string,
  programs: SourceReadinessProgramsV1,
): Promise<SourceCloseFundAcquisitionV1> {
  const marketAddress = key(market, 'Market');
  const staged = await acquireSourceReadinessFrameV1(client, wasm, marketAddress, programs);
  const initial = object(json(staged.snapshotJson, 'Source frame'), ['accounts', 'format', 'frame', 'observedSlot', 'unixTimestamp'], 'Source frame');
  if (!Array.isArray(initial.accounts)) throw new Error('Source frame accounts are not an array');
  const stagedAccounts = initial.accounts.map((entry, index) => object(entry,
    ['address', 'dataBase64', 'executable', 'lamports', 'owner'], `Source frame account ${index}`));
  const frame = object(initial.frame, ['activationCache', 'coordinates', 'coreProgram', 'coreProgramdata', 'registryProgram', 'resolutionProgram', 'resolutionProgramdata'], 'Source frame');
  const coordinates = object(frame.coordinates, ['activationReceipt', 'beneficiary', 'capabilityManifest', 'fundingLedger', 'market', 'recoveryPolicy', 'sourceMaterial', 'sourceState'], 'Source coordinates');
  const sourceState = key(coordinates.sourceState, 'Source state');
  const marketWire = observedAccount(stagedAccounts, marketAddress, 'Market');
  const sourceWire = observedAccount(stagedAccounts, sourceState, 'Source state');
  if (typeof marketWire.dataBase64 !== 'string' || typeof sourceWire.dataBase64 !== 'string') throw new Error('Source close staged bytes changed type');
  const detail = object(json(wasm.derive_source_close_detail_v1(JSON.stringify({
    format: SOURCE_CLOSE_DETAIL_FORMAT_V1,
    marketAddress,
    marketDataBase64: marketWire.dataBase64,
    resolutionProgram: programs.resolutionProgram,
    sourceStateAddress: sourceState,
    sourceStateDataBase64: sourceWire.dataBase64,
  })), 'Source close detail'), ['certificate', 'closureReceipt', 'closureSequence', 'terminalSequence'], 'Source close detail');
  const certificate = key(detail.certificate, 'terminal certificate');
  const closureReceipt = key(detail.closureReceipt, 'closure receipt');
  u64(detail.terminalSequence, 'terminal sequence');
  u64(detail.closureSequence, 'closure sequence');
  const sourceMaterial = pair(coordinates.sourceMaterial, 'Source material');
  const capabilityManifest = pair(coordinates.capabilityManifest, 'capability manifest');
  const recovery = coordinates.recoveryPolicy === null ? null : pair(coordinates.recoveryPolicy, 'recovery policy');
  const addresses = Object.freeze([
    marketAddress, key(frame.activationCache, 'activation cache'), key(frame.registryProgram, 'Registry program'),
    key(frame.coreProgram, 'Core program'), key(frame.coreProgramdata, 'Core ProgramData'),
    key(frame.resolutionProgram, 'Resolution program'), key(frame.resolutionProgramdata, 'Resolution ProgramData'),
    sourceMaterial.raw, sourceMaterial.staging, capabilityManifest.raw, capabilityManifest.staging,
    sourceState, key(coordinates.fundingLedger, 'funding ledger'), certificate, closureReceipt,
    key(coordinates.beneficiary, 'beneficiary'), CLOCK_SYSVAR, RENT_SYSVAR, SYSTEM_PROGRAM,
    ...(recovery === null ? [] : [recovery.raw, recovery.staging]),
  ]);
  if (![19, 21].includes(addresses.length) || new Set(addresses).size !== addresses.length) throw new Error('Source close frame changed width or aliases positions');
  const programdata = new Set([key(frame.coreProgramdata, 'Core ProgramData'), key(frame.resolutionProgramdata, 'Resolution ProgramData')]);
  const ordinaryAddresses = addresses.filter((address) => !programdata.has(address));
  const floor = await client.finalizedSlot();
  const [ordinary, coreProgramdata, resolutionProgramdata] = await Promise.all([
    client.multipleAccounts(ordinaryAddresses, floor),
    client.accountInfo(key(frame.coreProgramdata, 'Core ProgramData'), floor),
    client.accountInfo(key(frame.resolutionProgramdata, 'Resolution ProgramData'), floor),
  ]);
  if (ordinary.slot !== coreProgramdata.slot || ordinary.slot !== resolutionProgramdata.slot) throw new Error('finalized slot advanced during Source close ELF read');
  const timestamp = await client.blockTime(ordinary.slot);
  if (timestamp === null) throw new Error(`finalized Source close slot ${ordinary.slot} has no block time`);
  const accounts = new Map(ordinary.accounts.map((entry) => [entry.address, entry.account] as const));
  accounts.set(key(frame.coreProgramdata, 'Core ProgramData'), coreProgramdata.account);
  accounts.set(key(frame.resolutionProgramdata, 'Resolution ProgramData'), resolutionProgramdata.account);
  const snapshotJson = JSON.stringify({ format: SOURCE_CLOSE_SNAPSHOT_FORMAT_V1, observedSlot: ordinary.slot,
    unixTimestamp: timestamp, frame: { readiness: frame, certificate, closureReceipt },
    accounts: addresses.map((address) => accountJson(address, accounts.get(address) ?? null)) });
  const planJson = wasm.plan_source_close_fund_v1(snapshotJson);
  const plan = parseSourceCloseFundPlanV1(planJson);
  if (plan.observedSlot !== ordinary.slot || (plan.route === 'prepay' && plan.prepay?.destination !== closureReceipt)
      || (plan.route === 'close' && plan.accounts?.completion.join(',') !== [sourceState, coordinates.fundingLedger, closureReceipt, coordinates.beneficiary].join(','))) {
    throw new Error('Source close plan substituted its observation or completion frame');
  }
  return Object.freeze({ plan, planJson, snapshotJson, observationAddresses: addresses });
}

/** Compile only the Rust-selected prepay or signer-free direct close. */
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

export function buildSourceCloseFundTransactionV1(
  acquisition: SourceCloseFundAcquisitionV1,
  payerAddress: string,
  blockhash: PacketBlockhashV1,
): SourceCloseFundTransactionV1 {
  const payer = key(payerAddress, 'payer');
  const instructions = acquisition.plan.route === 'prepay'
    ? [SystemProgram.transfer({ fromPubkey: new PublicKey(payer), toPubkey: new PublicKey(acquisition.plan.prepay!.destination),
      lamports: BigInt(acquisition.plan.prepay!.lamports) })]
    : [new TransactionInstruction({ programId: new PublicKey(acquisition.plan.instruction!.program),
      keys: acquisition.plan.instruction!.accounts.map((meta) => ({ pubkey: new PublicKey(meta.address), isSigner: false, isWritable: meta.isWritable })),
      data: Buffer.from(Uint8Array.from(atob(acquisition.plan.instruction!.dataBase64), (character) => character.charCodeAt(0))) })];
  const transaction = new VersionedTransaction(new TransactionMessage({ payerKey: new PublicKey(payer),
    recentBlockhash: key(blockhash.blockhash, 'blockhash'), instructions }).compileToLegacyMessage());
  const wireBytes = transaction.serialize();
  if (transaction.signatures.length !== 1 || transaction.message.header.numRequiredSignatures !== 1
      || transaction.message.staticAccountKeys[0]?.toBase58() !== payer || wireBytes.length > SOLANA_PACKET_BYTES_V1) throw new Error('Source close packet changed sole-wallet authority or packet bound');
  return Object.freeze({ transaction, wireBytes, payer, route: acquisition.plan.route,
    observedSlot: acquisition.plan.observedSlot, lastValidBlockHeight: u64(blockhash.lastValidBlockHeight, 'last valid block height') });
}

/** Verify all finalized direct-close poststate, including the exact typed receipt. */
export async function verifySourceCloseFundFinalizedV1(
  client: CloseRpcV1,
  wasm: SourceReadinessWasmV1,
  acquisition: SourceCloseFundAcquisitionV1,
  resolutionProgram: string,
): Promise<Readonly<{ observedSlot: string; receipt: string }>> {
  if (acquisition.plan.route !== 'close' || acquisition.plan.accounts === null) throw new Error('receipt prepay has no close poststate');
  const [sourceState, fundingLedger, receipt, beneficiary] = acquisition.plan.accounts.completion;
  if (sourceState === undefined || fundingLedger === undefined || receipt === undefined || beneficiary === undefined) throw new Error('Source close plan omitted completion accounts');
  const floor = await client.finalizedSlot();
  const observation = await client.multipleAccounts([sourceState, fundingLedger, receipt, beneficiary, RENT_SYSVAR], floor);
  const found = (address: string) => observation.accounts.find((entry) => entry.address === address)?.account ?? null;
  if (found(sourceState) !== null || found(fundingLedger) !== null) throw new Error('Source state or funding ledger remains after close');
  const receiptAccount = found(receipt);
  const beneficiaryAccount = found(beneficiary);
  const rentAccount = found(RENT_SYSVAR);
  if (receiptAccount === null || beneficiaryAccount === null || rentAccount === null) throw new Error('Source close poststate omitted receipt, beneficiary, or Rent sysvar');
  if (beneficiaryAccount.executable) throw new Error('Source close beneficiary became executable');
  const expectedNames = ['market', 'generation', 'closureReceipt', 'sourceState', 'sourceMaterial', 'capabilityManifest',
    'terminalCertificate', 'beneficiary', 'selector', 'terminalSequence', 'sourceStateDigest', 'terminalCertificateDigest',
    'fundingSetDigest', 'sourceRefundLamports', 'ledgerRemainingNativePrincipal', 'ledgerRentLamports',
    'ledgerLamportSurplus', 'refundLamports', 'closedAt'] as const;
  const expected: Record<string, string> = {};
  for (const name of expectedNames) {
    const value = acquisition.plan.facts[name];
    if (value === undefined) throw new Error(`Source close Rust plan omitted ${name}`);
    expected[name] = value;
  }
  const timestamp = await client.blockTime(observation.slot);
  if (timestamp === null) throw new Error(`finalized Source close slot ${observation.slot} has no block time`);
  const verification = object(json(wasm.verify_source_close_receipt_v1(JSON.stringify({
    format: SOURCE_CLOSE_VERIFY_FORMAT_V1, observedSlot: observation.slot, unixTimestamp: timestamp,
    resolutionProgram: key(resolutionProgram, 'Resolution program'), receipt: accountJson(receipt, receiptAccount),
    rentSysvar: accountJson(RENT_SYSVAR, rentAccount), expected,
  })), 'Source close verification'), ['complete', 'format', 'observedSlot', 'receipt'], 'Source close verification');
  if (verification.format !== SOURCE_CLOSE_VERIFY_FORMAT_V1 || verification.complete !== true
      || verification.observedSlot !== observation.slot || verification.receipt !== receipt) throw new Error('Source close verifier substituted completion facts');
  return Object.freeze({ observedSlot: observation.slot, receipt });
}
