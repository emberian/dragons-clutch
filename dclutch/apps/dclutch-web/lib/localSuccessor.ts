import { PublicKey } from '@solana/web3.js';

import checkpointJson from '../fixtures/successor-checkpoint.json';
import { ascii, decodeBase64, hex, requireZero, sha256, slice, u16, u64 } from './bytes';
import {
  CAPABILITY_FUNDING_ALLOCATION_AMOUNT_OFFSET_V1,
  CAPABILITY_FUNDING_AMOUNTS_NATIVE_TOTAL_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_ACTIVATION_SLOT_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_BODY_RESERVED_BYTES_V1,
  CAPABILITY_FUNDING_STATE_BODY_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_BYTES_V1,
  CAPABILITY_FUNDING_STATE_ENTRY_INDEX_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_HEADER_RESERVED_BYTES_V1,
  CAPABILITY_FUNDING_STATE_HEADER_RESERVED_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_MAGIC_V1,
  CAPABILITY_FUNDING_STATE_MANIFEST_ID_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_RELEASED_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_REMAINING_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_SCHEMA_OFFSET_V1,
  CAPABILITY_FUNDING_STATE_SCHEMA_VERSION_V1,
  CAPABILITY_FUNDING_STATE_STATUS_OFFSET_V1,
  FUNDING_COMPARTMENTS_V1,
} from './generated/capabilityManifestV1';
import { SolanaRpcClient, type ConnectionFacts, type RpcAccount } from './rpc';

/** Offset of one remaining compartment's amount inside a `DCLTCFS1` account. */
function remainingAmount(compartment: (typeof FUNDING_COMPARTMENTS_V1)[number]['name']): number {
  const entry = FUNDING_COMPARTMENTS_V1.find((candidate) => candidate.name === compartment);
  if (entry === undefined) throw new Error(`${compartment} is not a canonical funding compartment`);
  return CAPABILITY_FUNDING_STATE_REMAINING_OFFSET_V1 + entry.offset + CAPABILITY_FUNDING_ALLOCATION_AMOUNT_OFFSET_V1;
}

const MAX_HISTORY = 64;
const MAX_RPC_BYTES = 2 * 1024 * 1024;
const TIMEOUT_MS = 12_000;
const SHA256 = /^[0-9a-f]{64}$/;

export type SuccessorOrigin = 'genesis-prepared' | 'genesis-prepared-refusal-sentinel' | 'genesis-prepared-then-transaction-mutated' | 'transaction-created';
type ExpectedAccount = Readonly<{ address: string; owner: string; lamports: number; executable: boolean; data_len: number; data_sha256: string; account_sha256: string; origin: SuccessorOrigin }>;
type ExpectedTransaction = Readonly<{ label: string; signature: string; slot: number; transaction_metadata_available: boolean; fee_lamports: number | null; compute_units_consumed: number | null; error: unknown }>;
type ProgramEvidence = Readonly<{ program_id: string; programdata_id: string; deployment_slot: number; upgrade_authority: null; upgrade_authority_effectively_disabled: boolean; elf_sha256: string }>;
type Scenario = Readonly<{ market: string; state: string; certificates: Readonly<Record<string, string>>; funding: Readonly<Record<string, string>>; hostile_certificate_preoccupied: boolean }>;

export type LocalSuccessorCheckpoint = Readonly<{
  schema: 'dclutch-web-local-successor-checkpoint-v1';
  provenance: Readonly<{ tool_commit: string; exact_source_commit: string; plan_sha256: string; evidence_sha256: string; profile_sha256: string }>;
  network: Readonly<{ rpc_url: string; genesis_hash: string; version: Readonly<{ 'solana-core': string; 'feature-set': number }> }>;
  evidence: Readonly<{ evidence_class: string; checked_production_release_claimed: false; captured_release_identity_claimed: false; genesis_fixture_boundary: ReadonlyArray<string>; rollback: Readonly<{ state_unchanged: boolean; certificate_unchanged: boolean; funding_unchanged: boolean; worker_unchanged: boolean; before: Readonly<Record<string, ExpectedAccount>>; after: Readonly<Record<string, ExpectedAccount>> }> }>;
  programs: Readonly<{ registry: ProgramEvidence; resolution: ProgramEvidence }>;
  scenarios: Readonly<{ primary: Scenario; lifecycle: Scenario; rollback: Scenario }>;
  expected_accounts: Readonly<Record<string, ExpectedAccount>>;
  expected_transactions: ReadonlyArray<ExpectedTransaction>;
  parser_fixtures: Readonly<{ accounts: Readonly<Record<string, Readonly<{ address: string; account: unknown }>>>; transactions: ReadonlyArray<unknown> }>;
}>;

export type ParsedSuccessorAccount = Readonly<{ kind: string; headline: string; facts: ReadonlyArray<Readonly<{ label: string; value: string }>> }>;
export type SuccessorAccountObservation = Readonly<{ name: string; expected: ExpectedAccount; observed: RpcAccount; digest: string; matches: boolean; parsed: ParsedSuccessorAccount; refusal: string | null }>;
export type SuccessorTransactionObservation = Readonly<{ label: string; signature: string; slot: string; computeUnits: string; outcome: 'success' | 'expected refusal'; rpcStatus: 'matched' | 'pruned' | 'mismatch'; detail: string }>;
export type LocalSuccessorSnapshot = Readonly<{
  facts: ConnectionFacts;
  observedSlot: string;
  accounts: ReadonlyArray<SuccessorAccountObservation>;
  transactions: ReadonlyArray<SuccessorTransactionObservation>;
  unexpectedProgramAccounts: ReadonlyArray<string>;
  missingProgramAccounts: ReadonlyArray<string>;
  exactAccounts: number;
  transactionCreatedAccounts: number;
  queryableTransactions: number;
  rollbackCurrent: boolean;
}>;

function plain(value: unknown): value is Record<string, unknown> { return value !== null && typeof value === 'object' && !Array.isArray(value); }
function text(value: unknown, field: string, maximum = 512): string { if (typeof value !== 'string' || value.trim() !== value || value.length === 0 || value.length > maximum) throw new Error(`${field} is not bounded canonical text`); return value; }
function uint(value: unknown, field: string): number { if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new Error(`${field} is not an exact unsigned integer`); return value; }
function key(value: unknown, field: string): string { const parsed = new PublicKey(text(value, field, 64)); if (parsed.toBase58() !== value) throw new Error(`${field} is not canonical base58 text`); return parsed.toBase58(); }
function digest(value: unknown, field: string): string { const parsed = text(value, field, 64); if (!SHA256.test(parsed)) throw new Error(`${field} is not lowercase SHA-256`); return parsed; }
function commit(value: unknown, field: string): string { const parsed = text(value, field, 40); if (!/^[0-9a-f]{40}$/.test(parsed)) throw new Error(`${field} is not a full lowercase git commit`); return parsed; }
function flag(value: unknown, field: string): boolean { if (typeof value !== 'boolean') throw new Error(`${field} is not Boolean`); return value; }
function same(left: unknown, right: unknown): boolean { return JSON.stringify(left) === JSON.stringify(right); }
function readI64(bytes: Uint8Array, offset: number): bigint { return new DataView(bytes.buffer, bytes.byteOffset + offset, 8).getBigInt64(0, true); }
function readI128(bytes: Uint8Array, offset: number): bigint { let value = 0n; for (let index = 15; index >= 0; index -= 1) value = (value << 8n) | BigInt(bytes[offset + index]); return value >= (1n << 127n) ? value - (1n << 128n) : value; }
function pubkey(bytes: Uint8Array, offset: number): string { return new PublicKey(slice(bytes, offset, 32)).toBase58(); }
function fact(label: string, value: string | number | bigint | boolean) { return Object.freeze({ label, value: String(value) }); }

function decodeExpectedAccount(value: unknown, field: string): ExpectedAccount {
  if (!plain(value)) throw new Error(`${field} is not an account expectation`);
  const origin = text(value.origin, `${field}.origin`) as SuccessorOrigin;
  if (!['genesis-prepared', 'genesis-prepared-refusal-sentinel', 'genesis-prepared-then-transaction-mutated', 'transaction-created'].includes(origin)) throw new Error(`${field}.origin is unknown`);
  return Object.freeze({ address: key(value.address, `${field}.address`), owner: key(value.owner, `${field}.owner`), lamports: uint(value.lamports, `${field}.lamports`), executable: flag(value.executable, `${field}.executable`), data_len: uint(value.data_len, `${field}.data_len`), data_sha256: digest(value.data_sha256, `${field}.data_sha256`), account_sha256: digest(value.account_sha256, `${field}.account_sha256`), origin });
}

export function decodeLocalSuccessorCheckpoint(value: unknown): LocalSuccessorCheckpoint {
  if (!plain(value) || value.schema !== 'dclutch-web-local-successor-checkpoint-v1' || !plain(value.provenance) || !plain(value.network) || !plain(value.evidence) || !plain(value.programs) || !plain(value.scenarios) || !plain(value.expected_accounts) || !Array.isArray(value.expected_transactions) || !plain(value.parser_fixtures)) throw new Error('local successor checkpoint has the wrong structural schema');
  // Loopback and an explicit port, and NOT a particular port. What this gate
  // is for is that a checkpoint can only ever point the browser at a validator
  // on this machine; 20890 was the launcher's historical base and is now one
  // admissible base among many, because N campaigns share one machine on
  // disjoint bases. Pinning the number here would refuse an honest checkpoint
  // captured from any of them.
  const url = new URL(text(value.network.rpc_url, 'checkpoint RPC')); if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || !/^[0-9]+$/.test(url.port)) throw new Error('local successor checkpoint is not a loopback explicit-port profile');
  const expectedAccounts = Object.freeze(Object.fromEntries(Object.entries(value.expected_accounts).map(([name, account]) => [name, decodeExpectedAccount(account, `expected account ${name}`)])));
  const programs = value.programs as Record<string, unknown>; const parseProgram = (name: string): ProgramEvidence => { const program = programs[name]; if (!plain(program)) throw new Error(`${name} program evidence is absent`); const deploymentSlot = uint(program.deployment_slot, `${name} deployment slot`); if (deploymentSlot !== 0 || program.upgrade_authority !== null || program.upgrade_authority_effectively_disabled !== true) throw new Error(`${name} is not the immutable slot-zero Loader profile`); return Object.freeze({ program_id: key(program.program_id, `${name} program`), programdata_id: key(program.programdata_id, `${name} ProgramData`), deployment_slot: deploymentSlot, upgrade_authority: null, upgrade_authority_effectively_disabled: true, elf_sha256: digest(program.elf_sha256, `${name} ELF`) }); };
  const parseScenario = (name: string): Scenario => { const scenario = (value.scenarios as Record<string, unknown>)[name]; if (!plain(scenario) || !plain(scenario.certificates) || !plain(scenario.funding)) throw new Error(`${name} scenario is malformed`); return Object.freeze({ market: key(scenario.market, `${name} market`), state: key(scenario.state, `${name} state`), certificates: Object.freeze(Object.fromEntries(Object.entries(scenario.certificates).map(([role, address]) => [role, key(address, `${name} ${role} certificate`)]))), funding: Object.freeze(Object.fromEntries(Object.entries(scenario.funding).map(([role, address]) => [role, key(address, `${name} ${role} funding`)]))), hostile_certificate_preoccupied: flag(scenario.hostile_certificate_preoccupied, `${name} occupied flag`) }); };
  const expectedTransactions = Object.freeze(value.expected_transactions.map((entry, index): ExpectedTransaction => {
    if (!plain(entry)) throw new Error(`transaction ${index} is malformed`);
    return Object.freeze({ label: text(entry.label, `transaction ${index} label`, 96), signature: text(entry.signature, `transaction ${index} signature`, 96), slot: uint(entry.slot, `transaction ${index} slot`), transaction_metadata_available: flag(entry.transaction_metadata_available, `transaction ${index} metadata flag`), fee_lamports: entry.fee_lamports === null ? null : uint(entry.fee_lamports, `transaction ${index} fee`), compute_units_consumed: entry.compute_units_consumed === null ? null : uint(entry.compute_units_consumed, `transaction ${index} CU`), error: entry.error });
  }));
  const evidence = value.evidence as Record<string, unknown>; const rollback = evidence.rollback; if (!plain(rollback) || !plain(rollback.before) || !plain(rollback.after) || !Array.isArray(evidence.genesis_fixture_boundary)) throw new Error('checkpoint rollback/genesis evidence is malformed');
  if (flag(evidence.checked_production_release_claimed, 'checked production release claim') !== false || flag(evidence.captured_release_identity_claimed, 'captured release identity claim') !== false) throw new Error('localhost checkpoint must not claim production or captured release identity');
  const parseRollbackSide = (side: Record<string, unknown>) => Object.freeze(Object.fromEntries(Object.entries(side).map(([name, account]) => [name, decodeExpectedAccount({ ...(account as object), origin: 'genesis-prepared-refusal-sentinel' }, `rollback ${name}`)])));
  const provenance = value.provenance as Record<string, unknown>; const network = value.network as Record<string, unknown>; const version = network.version;
  if (!plain(version) || !plain(value.parser_fixtures) || !plain(value.parser_fixtures.accounts) || !Array.isArray(value.parser_fixtures.transactions)) throw new Error('checkpoint network/parser fixture is malformed');
  return Object.freeze({ schema: value.schema, provenance: Object.freeze({ tool_commit: commit(provenance.tool_commit, 'tool commit'), exact_source_commit: commit(provenance.exact_source_commit, 'source commit'), plan_sha256: digest(provenance.plan_sha256, 'plan digest'), evidence_sha256: digest(provenance.evidence_sha256, 'evidence digest'), profile_sha256: digest(provenance.profile_sha256, 'profile digest') }), network: Object.freeze({ rpc_url: url.toString(), genesis_hash: text(network.genesis_hash, 'genesis hash', 96), version: Object.freeze({ 'solana-core': text(version['solana-core'], 'Solana version', 64), 'feature-set': uint(version['feature-set'], 'feature set') }) }), evidence: Object.freeze({ evidence_class: text(evidence.evidence_class, 'evidence class'), checked_production_release_claimed: false, captured_release_identity_claimed: false, genesis_fixture_boundary: Object.freeze(evidence.genesis_fixture_boundary.map((entry, index) => text(entry, `genesis boundary ${index}`, 320))), rollback: Object.freeze({ state_unchanged: flag(rollback.state_unchanged, 'rollback state'), certificate_unchanged: flag(rollback.certificate_unchanged, 'rollback certificate'), funding_unchanged: flag(rollback.funding_unchanged, 'rollback funding'), worker_unchanged: flag(rollback.worker_unchanged, 'rollback worker'), before: parseRollbackSide(rollback.before), after: parseRollbackSide(rollback.after) }) }), programs: Object.freeze({ registry: parseProgram('registry'), resolution: parseProgram('resolution') }), scenarios: Object.freeze({ primary: parseScenario('primary'), lifecycle: parseScenario('lifecycle'), rollback: parseScenario('rollback') }), expected_accounts: expectedAccounts, expected_transactions: expectedTransactions, parser_fixtures: Object.freeze({ accounts: value.parser_fixtures.accounts as LocalSuccessorCheckpoint['parser_fixtures']['accounts'], transactions: Object.freeze(value.parser_fixtures.transactions) }) });
}

export const LOCAL_SUCCESSOR_CHECKPOINT = decodeLocalSuccessorCheckpoint(checkpointJson as unknown);

function decodeLoader(name: string, bytes: Uint8Array, expected: ExpectedAccount, checkpoint: LocalSuccessorCheckpoint): ParsedSuccessorAccount | null {
  if (!name.startsWith('loader.')) return null;
  const programName = name.includes('registry') ? 'registry' : 'resolution'; const program = checkpoint.programs[programName];
  if (name.endsWith('.program')) { if (bytes.length !== 36 || new DataView(bytes.buffer, bytes.byteOffset, 4).getUint32(0, true) !== 2 || pubkey(bytes, 4) !== program.programdata_id) throw new Error('Loader Program linkage is not exact'); return Object.freeze({ kind: 'immutable Loader Program', headline: programName, facts: Object.freeze([fact('ProgramData', program.programdata_id), fact('executable', expected.executable)]) }); }
  if (bytes.length <= 45 || new DataView(bytes.buffer, bytes.byteOffset, 4).getUint32(0, true) !== 3 || u64(bytes, 4) !== 0n || bytes[12] !== 0) throw new Error('Loader ProgramData header is not immutable slot-zero V3'); requireZero(bytes, 13, 32, 'immutable ProgramData header'); return Object.freeze({ kind: 'immutable Loader ProgramData', headline: programName, facts: Object.freeze([fact('deployment slot', 0), fact('upgrade authority', 'none'), fact('ELF bytes', bytes.length - 45), fact('ELF SHA-256', program.elf_sha256)]) });
}

export function parseSuccessorAccount(name: string, account: RpcAccount, checkpoint = LOCAL_SUCCESSOR_CHECKPOINT): ParsedSuccessorAccount {
  const bytes = account.data; const loader = decodeLoader(name, bytes, checkpoint.expected_accounts[name], checkpoint); if (loader) return loader;
  if (name.endsWith('.occupied')) { if (bytes.length !== 312 || !bytes.every((byte) => byte === 0xa5)) throw new Error('hostile certificate sentinel is not the exact occupied pattern'); return Object.freeze({ kind: 'hostile preoccupied certificate', headline: 'deliberately malformed', facts: Object.freeze([fact('bytes', bytes.length), fact('purpose', 'late output-gate refusal')]) }); }
  const magic = bytes.length >= 8 ? ascii(bytes, 0, 8) : '';
  if (magic === 'DCLTACT1') { if (bytes.length !== 1288 || u16(bytes, 8) !== 1 || u16(bytes, 10) !== 1) throw new Error('activation cache has the wrong exact profile'); requireZero(bytes, 12, 4, 'activation cache'); return Object.freeze({ kind: 'Registry activation cache', headline: 'five checked roles', facts: Object.freeze([fact('release set', hex(slice(bytes, 16, 32))), fact('roles', 5), fact('origin', 'transaction-created')]) }); }
  if (magic === 'DCSRCER1') { if (bytes.length !== 312 || u16(bytes, 8) !== 1 || bytes[10] < 1 || bytes[10] > 4) throw new Error('Resolution certificate has the wrong exact layout'); requireZero(bytes, 11, 5, 'Resolution certificate header'); requireZero(bytes, 260, 4, 'Resolution certificate body'); const kinds = ['unknown', 'primary success', 'recovery advanced', 'exhausted', 'failure committed']; return Object.freeze({ kind: 'signed Resolution certificate', headline: kinds[bytes[10]], facts: Object.freeze([fact('market', pubkey(bytes, 16)), fact('generation', u64(bytes, 240)), fact('attempt / schedule', `${new DataView(bytes.buffer, bytes.byteOffset + 248, 4).getUint32(0, true)} / ${new DataView(bytes.buffer, bytes.byteOffset + 252, 4).getUint32(0, true)}`), fact('selector', new DataView(bytes.buffer, bytes.byteOffset + 256, 4).getUint32(0, true)), fact('work paid', u64(bytes, 264)), fact('funding remaining', u64(bytes, 272)), fact('result', `${readI128(bytes, 280)}/${u64(bytes, 296)}`), fact('observed at', u64(bytes, 304))]) }); }
  if (magic === 'DCLTSRS1') { if (bytes.length !== 224 || u16(bytes, 8) !== 1 || bytes[10] > 5 || bytes[12] > 3) throw new Error('Source resolution state has the wrong exact layout'); requireZero(bytes, 15, 1, 'Source state header'); requireZero(bytes, 208, 16, 'Source state tail'); const phases = ['primary', 'recovery', 'resolved', 'exhausted', 'failure committed', 'retired']; const routes = ['none', 'primary', 'recovery', 'failure']; return Object.freeze({ kind: 'Source resolution state', headline: phases[bytes[10]], facts: Object.freeze([fact('market', pubkey(bytes, 16)), fact('generation', u64(bytes, 48)), fact('active attempt', bytes[11]), fact('terminal route', routes[bytes[12]]), fact('selector', bytes[13]), fact('terminal sequence', u64(bytes, 184)), fact('resolved at', readI64(bytes, 192))]) }); }
  if (magic === CAPABILITY_FUNDING_STATE_MAGIC_V1) { const status = bytes[CAPABILITY_FUNDING_STATE_STATUS_OFFSET_V1]; if (bytes.length !== CAPABILITY_FUNDING_STATE_BYTES_V1 || u16(bytes, CAPABILITY_FUNDING_STATE_SCHEMA_OFFSET_V1) !== CAPABILITY_FUNDING_STATE_SCHEMA_VERSION_V1 || status > 1) throw new Error('capability funding state has the wrong exact layout'); requireZero(bytes, CAPABILITY_FUNDING_STATE_HEADER_RESERVED_OFFSET_V1, CAPABILITY_FUNDING_STATE_HEADER_RESERVED_BYTES_V1, 'funding state header'); requireZero(bytes, CAPABILITY_FUNDING_STATE_BODY_RESERVED_OFFSET_V1, CAPABILITY_FUNDING_STATE_BODY_RESERVED_BYTES_V1, 'funding state body'); return Object.freeze({ kind: 'typed capability funding', headline: status === 1 ? 'active' : 'pending', facts: Object.freeze([fact('manifest', hex(slice(bytes, CAPABILITY_FUNDING_STATE_MANIFEST_ID_OFFSET_V1, 32))), fact('entry', u16(bytes, CAPABILITY_FUNDING_STATE_ENTRY_INDEX_OFFSET_V1)), fact('activation slot', u64(bytes, CAPABILITY_FUNDING_STATE_ACTIVATION_SLOT_OFFSET_V1)), fact('remaining work', u64(bytes, remainingAmount('Work'))), fact('remaining bounty', u64(bytes, remainingAmount('Bounty'))), fact('remaining native total', u64(bytes, CAPABILITY_FUNDING_STATE_REMAINING_OFFSET_V1 + CAPABILITY_FUNDING_AMOUNTS_NATIVE_TOTAL_OFFSET_V1)), fact('released native total', u64(bytes, CAPABILITY_FUNDING_STATE_RELEASED_OFFSET_V1 + CAPABILITY_FUNDING_AMOUNTS_NATIVE_TOTAL_OFFSET_V1))]) }); }
  if (magic === 'DCLTCAT1') { if (bytes.length < 336 || u16(bytes, 8) !== 1 || bytes[11] !== 1 || bytes.length !== 320 + bytes[10] * 8 || ascii(bytes, 16, 8) !== 'DCLTROOT') throw new Error('categorical Market has the wrong exact profile'); return Object.freeze({ kind: 'categorical Market root', headline: `${bytes[10]} outcomes`, facts: Object.freeze([fact('generation', u64(bytes, 192)), fact('phase byte', bytes[200]), fact('origin', 'genesis-prepared')]) }); }
  if (/^DCLT[A-Z0-9]{4}$/.test(magic)) return Object.freeze({ kind: 'finalized semantic record', headline: magic, facts: Object.freeze([fact('bytes', bytes.length), fact('origin', 'genesis-prepared')]) });
  throw new Error(`unrecognized exact successor account magic ${hex(bytes.slice(0, Math.min(bytes.length, 8)))}`);
}

export function decodeCheckpointFixtureAccount(value: unknown): RpcAccount {
  if (!plain(value)) throw new Error('checkpoint fixture account is malformed'); const owner = key(value.owner, 'fixture owner'); const data = decodeBase64(value.data, 'fixture data'); const space = uint(value.space, 'fixture space'); if (space !== data.length) throw new Error('checkpoint fixture account space differs from data'); return Object.freeze({ owner, data, space, executable: flag(value.executable, 'fixture executable'), lamports: String(uint(value.lamports, 'fixture lamports')) });
}

async function boundedRpc(endpoint: string, method: string, params: ReadonlyArray<unknown>, fetcher: typeof fetch): Promise<unknown> {
  const controller = new AbortController(); const timeout = setTimeout(() => controller.abort(), TIMEOUT_MS);
  try { const response = await fetcher(endpoint, { method: 'POST', mode: 'cors', credentials: 'omit', redirect: 'error', cache: 'no-store', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }), signal: controller.signal }); if (!response.ok) throw new Error(`${method} returned HTTP ${response.status}`); const bytes = new Uint8Array(await response.arrayBuffer()); if (bytes.length > MAX_RPC_BYTES) throw new Error(`${method} response exceeds ${MAX_RPC_BYTES} bytes`); const payload: unknown = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)); if (!plain(payload) || payload.jsonrpc !== '2.0' || payload.error !== undefined || !('result' in payload)) throw new Error(`${method} returned an invalid JSON-RPC envelope`); return payload.result; } finally { clearTimeout(timeout); }
}

async function transactionHistory(endpoint: string, checkpoint: LocalSuccessorCheckpoint, fetcher: typeof fetch): Promise<ReadonlyArray<SuccessorTransactionObservation>> {
  const addressHistories = await Promise.all([checkpoint.programs.registry.program_id, checkpoint.programs.resolution.program_id].map(async (address) => boundedRpc(endpoint, 'getSignaturesForAddress', [address, { commitment: 'finalized', limit: MAX_HISTORY }], fetcher)));
  const liveSignatures = new Set<string>(); for (const raw of addressHistories) { if (!Array.isArray(raw) || raw.length > MAX_HISTORY) throw new Error('signature history is malformed or exceeds its bound'); for (const entry of raw) { if (!plain(entry)) throw new Error('signature history entry is malformed'); liveSignatures.add(text(entry.signature, 'history signature', 96)); } }
  const output: SuccessorTransactionObservation[] = [];
  for (const expected of checkpoint.expected_transactions) {
    const raw = await boundedRpc(endpoint, 'getTransaction', [expected.signature, { commitment: 'finalized', encoding: 'json', maxSupportedTransactionVersion: 0 }], fetcher);
    if (raw === null) { output.push(Object.freeze({ label: expected.label, signature: expected.signature, slot: String(expected.slot), computeUnits: String(expected.compute_units_consumed ?? 'unavailable'), outcome: expected.error === null ? 'success' : 'expected refusal', rpcStatus: 'pruned', detail: liveSignatures.has(expected.signature) ? 'signature remains indexed, but transaction body is unavailable' : 'transaction status has aged out of this local ledger; runner-captured evidence remains the only transaction record' })); continue; }
    if (!plain(raw) || !plain(raw.meta)) throw new Error(`transaction ${expected.label} is malformed`); const observedSlot = uint(raw.slot, `${expected.label} slot`); const observedCu = uint(raw.meta.computeUnitsConsumed, `${expected.label} CU`); const observedError = raw.meta.err ?? null; const matched = observedSlot === expected.slot && observedCu === expected.compute_units_consumed && same(observedError, expected.error);
    output.push(Object.freeze({ label: expected.label, signature: expected.signature, slot: String(observedSlot), computeUnits: String(observedCu), outcome: observedError === null ? 'success' : 'expected refusal', rpcStatus: matched ? 'matched' : 'mismatch', detail: matched ? 'finalized RPC transaction metadata matches the captured checkpoint' : 'finalized RPC metadata differs from the captured checkpoint' }));
  }
  return Object.freeze(output);
}

export async function discoverLocalSuccessor(client: SolanaRpcClient, checkpoint = LOCAL_SUCCESSOR_CHECKPOINT, fetcher: typeof fetch = fetch): Promise<LocalSuccessorSnapshot> {
  const endpoint = new URL(client.endpoint); if (endpoint.toString() !== checkpoint.network.rpc_url) throw new Error(`local successor profile requires ${checkpoint.network.rpc_url}`);
  const [facts, floor] = await Promise.all([client.probe(), client.finalizedSlot()]); if (facts.genesisHash !== checkpoint.network.genesis_hash || facts.solanaCore !== checkpoint.network.version['solana-core'] || facts.featureSet !== String(checkpoint.network.version['feature-set'])) throw new Error('RPC genesis or runtime version differs from the immutable successor checkpoint');
  const entries = Object.entries(checkpoint.expected_accounts); const reads = [] as Awaited<ReturnType<SolanaRpcClient['multipleAccounts']>>[]; for (let offset = 0; offset < entries.length; offset += 32) reads.push(await client.multipleAccounts(entries.slice(offset, offset + 32).map(([, account]) => account.address), floor));
  const observed = new Map(reads.flatMap((read) => read.accounts.map((entry) => [entry.address, entry.account] as const))); const accounts: SuccessorAccountObservation[] = [];
  for (const [name, expected] of entries) { const account = observed.get(expected.address); if (account === null || account === undefined) throw new Error(`${name} is absent at finalized commitment`); const observedDigest = hex(await sha256(account.data)); let parsed: ParsedSuccessorAccount; let refusal: string | null = null; try { parsed = parseSuccessorAccount(name, account, checkpoint); if (name.endsWith('.programdata')) { const program = checkpoint.programs[name.includes('registry') ? 'registry' : 'resolution']; if (hex(await sha256(account.data.slice(45))) !== program.elf_sha256) throw new Error('Loader ProgramData ELF digest differs from the pinned artifact'); } } catch (error) { refusal = error instanceof Error ? error.message : 'parser refused without a reason'; parsed = Object.freeze({ kind: 'refused account', headline: name, facts: Object.freeze([]) }); } const matches = account.owner === expected.owner && account.executable === expected.executable && account.space === expected.data_len && account.lamports === String(expected.lamports) && observedDigest === expected.data_sha256 && refusal === null; accounts.push(Object.freeze({ name, expected, observed: account, digest: observedDigest, matches, parsed, refusal: matches ? null : refusal ?? 'owner, executable flag, width, lamports, or data digest differs from the checkpoint' })); }
  const scans = await Promise.all([client.programHeaders(checkpoint.programs.registry.program_id), client.programHeaders(checkpoint.programs.resolution.program_id)]); const expectedOwned = new Set(entries.filter(([, account]) => account.owner === checkpoint.programs.registry.program_id || account.owner === checkpoint.programs.resolution.program_id).map(([, account]) => account.address)); const scanned = new Set(scans.flatMap((scan) => scan.accounts.map((account) => account.address))); const unexpected = [...scanned].filter((address) => !expectedOwned.has(address)).sort(); const missing = [...expectedOwned].filter((address) => !scanned.has(address)).sort();
  const transactions = await transactionHistory(checkpoint.network.rpc_url, checkpoint, fetcher); const rollback = checkpoint.evidence.rollback; const rollbackEqual = (['state', 'certificate', 'funding', 'worker'] as const).every((name) => rollback[`${name}_unchanged`] && rollback.before[name].account_sha256 === rollback.after[name].account_sha256); const currentRollback = accounts.find((entry) => entry.name === 'rollback.certificate.failure.occupied');
  return Object.freeze({ facts, observedSlot: reads.at(-1)?.slot ?? floor, accounts: Object.freeze(accounts), transactions, unexpectedProgramAccounts: Object.freeze(unexpected), missingProgramAccounts: Object.freeze(missing), exactAccounts: accounts.filter((account) => account.matches).length, transactionCreatedAccounts: accounts.filter((account) => account.matches && account.expected.origin === 'transaction-created').length, queryableTransactions: transactions.filter((transaction) => transaction.rpcStatus === 'matched').length, rollbackCurrent: rollbackEqual && currentRollback?.matches === true });
}
