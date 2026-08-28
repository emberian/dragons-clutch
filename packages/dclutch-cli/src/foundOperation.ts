/**
 * One public exterior for permanent-devnet Market founding plus the first
 * participant admission.
 *
 * This file owns no protocol formula and no transaction. The Rust successor
 * remains the sole author of the MarketRunInput, founding campaign, and User
 * Position admission. This exterior pins one operation to those three exact
 * producers, makes their handoffs durable, and never reads a key file itself.
 */
import { spawnSync } from 'node:child_process';
import {
  closeSync,
  existsSync,
  fstatSync,
  fsyncSync,
  linkSync,
  lstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeSync,
} from 'node:fs';
import { basename, dirname, isAbsolute, resolve } from 'node:path';
import { randomBytes } from 'node:crypto';
import { createHash } from 'node:crypto';

import { SOLANA_DEVNET_GENESIS_HASH_V1 } from '@dclutch/sdk/rpc';

import { decodeSession, type CliContext } from './context';
import type { Io } from './output';

const OPERATION_SCHEMA_V1 = 'dclutch-devnet-market-participant-operation-v1';
const JOURNAL_SCHEMA_V1 = 'dclutch-devnet-market-participant-journal-v1';
const CAMPAIGN_SCHEMA_V1 = 'dclutch-successor-campaign-report-v1';
const PARTICIPANT_SCHEMA_V1 = 'dclutch-devnet-user-position-admission-execution-v1';
const OPERATION_LOCK_SCHEMA_V1 = 'dclutch-devnet-market-participant-operation-lock-v1';
const MAX_OPERATION_BYTES = 128 * 1024;
const MAX_MARKET_INPUT_BYTES = 1024 * 1024;
const MAX_EVIDENCE_BYTES = 16 * 1024 * 1024;
const MAX_BINARY_BYTES = 256 * 1024 * 1024;
const MAX_CHILD_OUTPUT_BYTES = 16 * 1024 * 1024;
const MAX_JSON_DEPTH = 64;
const MAX_JSON_VALUES = 200_000;

const REQUIRED_CAMPAIGN_ROLES = Object.freeze([
  'core-upgrade-authority',
  'collateral-mint',
  'collateral-wallet',
  'founding-beneficiary',
  'founding-founder',
  'founding-projection-witness',
  'founding-source-funder',
  // This role does not sign, but pinning it makes the hostile request record
  // reproducible across a crash instead of silently drawing a new random key.
  'substituted-founder',
] as const);

const FLAGSHIP_MARKET_FLAGS = Object.freeze(new Set([
  '--registry-program-id',
  '--direct-fee-basis-points',
  '--direct-fee-recipient',
  '--price-update',
  '--window-start',
  '--window-width-seconds',
  '--max-age-seconds',
  '--cut-denominator',
  '--cuts',
  '--coefficients',
  '--product',
  '--coordinate-domain',
  '--feed',
  '--generation',
]));

const GRADUATION_MARKET_FLAGS = Object.freeze(new Set([
  '--registry-program-id',
  '--direct-fee-basis-points',
  '--direct-fee-recipient',
  '--relayer-attestation',
  '--pool',
  '--venue-deployment-slot',
  '--venue-upgrade-authority',
  '--venue-elf-sha256',
  '--window-start',
  '--window-end',
  '--max-age-seconds',
  '--venue-program',
  '--venue-programdata',
]));

const SHARED_CHILD_FLAGS = Object.freeze(new Set([
  '--plan', '--rpc-url', '--i-mean-devnet', '--market', '--evidence', '--through',
  '--execute', '--campaign-evidence', '--output', '--minimum-finalized-slot',
]));

type MarketKindV1 = 'flagship' | 'graduation';

type CampaignKeypairV1 = Readonly<{ role: string; path: string }>;
type ParticipantCollateralV1 = Readonly<{
  sourceOwner: string;
  sourceOwnerKeypair: string;
  sourceAccount: string;
  quantityAtoms: string;
}>;

type FoundOperationV1 = Readonly<{
  schema: typeof OPERATION_SCHEMA_V1;
  plan: string;
  market: Readonly<{
    kind: MarketKindV1;
    arguments: ReadonlyArray<string>;
    output: string;
  }>;
  campaign: Readonly<{
    evidence: string;
    keypairs: ReadonlyArray<CampaignKeypairV1>;
  }>;
  participant: Readonly<{
    output: string;
    positionOwner: string;
    positionOwnerKeypair: string;
    feePayer: string;
    feePayerKeypair: string;
    minimumFinalizedSlot: string;
    collateral: ParticipantCollateralV1 | null;
  }>;
}>;

type JournalPhaseV1 = 'planned' | 'market-authored' | 'market-prepared' | 'founding-complete' | 'participant-complete';

type JournalBodyV1 = Readonly<{
  schema: typeof JOURNAL_SCHEMA_V1;
  phase: JournalPhaseV1;
  sequence: number;
  authorizedMutation: boolean;
  operationSha256: string;
  successorSha256: string;
  planSha256: string;
  rpcUrl: string;
  devnetGenesis: typeof SOLANA_DEVNET_GENESIS_HASH_V1;
  marketInputSha256: string | null;
  marketInputBase64: string | null;
  campaignEvidenceSha256: string | null;
  participantEvidenceSha256: string | null;
}>;

type JournalV1 = JournalBodyV1 & Readonly<{ bodySha256: string }>;

type InvocationResultV1 = Readonly<{ status: number | null; stdout: Uint8Array; stderr: Uint8Array }>;

type OperationLeaseV1 = Readonly<{
  path: string;
  descriptor: number;
  device: bigint;
  inode: bigint;
}>;

export type FoundOperationDependenciesV1 = Readonly<{
  invoke: (binary: string, arguments_: ReadonlyArray<string>) => InvocationResultV1;
}>;

const DEFAULT_DEPENDENCIES: FoundOperationDependenciesV1 = Object.freeze({
  invoke(binary, arguments_) {
    const result = spawnSync(binary, [...arguments_], {
      encoding: 'buffer',
      maxBuffer: MAX_CHILD_OUTPUT_BYTES,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    if (result.error !== undefined) throw result.error;
    return Object.freeze({
      status: result.status,
      stdout: new Uint8Array(result.stdout ?? []),
      stderr: new Uint8Array(result.stderr ?? []),
    });
  },
});

function object(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`${label} must be one JSON object`);
  }
  return value as Record<string, unknown>;
}

/** Scan the original bytes before JSON.parse can erase duplicate keys. */
function exactJson(source: Uint8Array, maximum: number, label: string): unknown {
  if (source.length === 0 || source.length > maximum) {
    throw new Error(`${label} is outside its byte bound`);
  }
  let sourceText: string;
  try {
    sourceText = new TextDecoder('utf-8', { fatal: true }).decode(source);
  } catch {
    throw new Error(`${label} is not UTF-8 JSON`);
  }
  const roundTrip = new TextEncoder().encode(sourceText);
  if (roundTrip.length !== source.length || roundTrip.some((byte, index) => byte !== source[index])) {
    throw new Error(`${label} is not canonical UTF-8 JSON`);
  }

  let cursor = 0;
  let values = 0;
  const fail = (reason: string): never => { throw new Error(`${label} is not exact JSON: ${reason}`); };
  const whitespace = (): void => {
    while (cursor < sourceText.length && [' ', '\n', '\r', '\t'].includes(sourceText[cursor] ?? '')) cursor += 1;
  };
  const string = (): string => {
    if (sourceText[cursor] !== '"') return fail('expected one string');
    const start = cursor;
    cursor += 1;
    for (;;) {
      if (cursor >= sourceText.length) return fail('unterminated string');
      const character = sourceText[cursor] as string;
      if (character === '"') {
        cursor += 1;
        try { return JSON.parse(sourceText.slice(start, cursor)) as string; } catch { return fail('invalid string'); }
      }
      if (character.charCodeAt(0) < 0x20) return fail('unescaped control character');
      if (character === '\\') {
        cursor += 1;
        if (cursor >= sourceText.length) return fail('unterminated escape');
        const escape = sourceText[cursor] as string;
        if (escape === 'u') {
          if (!/^[0-9a-fA-F]{4}$/.test(sourceText.slice(cursor + 1, cursor + 5))) return fail('invalid Unicode escape');
          cursor += 5;
          continue;
        }
        if (!'"\\/bfnrt'.includes(escape)) return fail('invalid escape');
      }
      cursor += 1;
    }
  };
  const number = (): void => {
    if (sourceText[cursor] === '-') cursor += 1;
    if (sourceText[cursor] === '0') cursor += 1;
    else {
      if (!/[1-9]/.test(sourceText[cursor] ?? '')) return fail('invalid number');
      while (/[0-9]/.test(sourceText[cursor] ?? '')) cursor += 1;
    }
    if (sourceText[cursor] === '.') {
      cursor += 1;
      if (!/[0-9]/.test(sourceText[cursor] ?? '')) return fail('invalid fraction');
      while (/[0-9]/.test(sourceText[cursor] ?? '')) cursor += 1;
    }
    if (sourceText[cursor] === 'e' || sourceText[cursor] === 'E') {
      cursor += 1;
      if (sourceText[cursor] === '+' || sourceText[cursor] === '-') cursor += 1;
      if (!/[0-9]/.test(sourceText[cursor] ?? '')) return fail('invalid exponent');
      while (/[0-9]/.test(sourceText[cursor] ?? '')) cursor += 1;
    }
  };
  const value = (depth: number): void => {
    values += 1;
    if (values > MAX_JSON_VALUES) return fail(`tree exceeds ${MAX_JSON_VALUES} values`);
    if (depth > MAX_JSON_DEPTH) return fail(`nesting exceeds ${MAX_JSON_DEPTH}`);
    whitespace();
    const character = sourceText[cursor];
    if (character === '{') {
      cursor += 1;
      whitespace();
      const keys = new Set<string>();
      if (sourceText[cursor] === '}') { cursor += 1; return; }
      for (;;) {
        const key = string();
        if (keys.has(key)) return fail(`duplicate JSON object key ${JSON.stringify(key)}`);
        keys.add(key);
        whitespace();
        if (sourceText[cursor] !== ':') return fail('object key has no colon');
        cursor += 1;
        value(depth + 1);
        whitespace();
        if (sourceText[cursor] === '}') { cursor += 1; return; }
        if (sourceText[cursor] !== ',') return fail('object has no comma or closing brace');
        cursor += 1;
        whitespace();
      }
    }
    if (character === '[') {
      cursor += 1;
      whitespace();
      if (sourceText[cursor] === ']') { cursor += 1; return; }
      for (;;) {
        value(depth + 1);
        whitespace();
        if (sourceText[cursor] === ']') { cursor += 1; return; }
        if (sourceText[cursor] !== ',') return fail('array has no comma or closing bracket');
        cursor += 1;
      }
    }
    if (character === '"') { string(); return; }
    for (const literal of ['true', 'false', 'null']) {
      if (sourceText.startsWith(literal, cursor)) { cursor += literal.length; return; }
    }
    if (character === '-' || /[0-9]/.test(character ?? '')) { number(); return; }
    return fail('invalid value');
  };
  whitespace();
  value(0);
  whitespace();
  if (cursor !== sourceText.length) fail('trailing bytes or a second value');
  try { return JSON.parse(sourceText) as unknown; } catch { return fail('invalid value'); }
}

function exactKeys(value: Record<string, unknown>, expected: ReadonlyArray<string>, label: string): void {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
    throw new Error(`${label} fields must be exactly ${wanted.join(', ')}`);
  }
}

function text(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > 8_192 || value.includes('\0')) {
    throw new Error(`${label} must be one bounded nonempty string`);
  }
  return value;
}

function absolutePath(value: unknown, label: string): string {
  const path = text(value, label);
  if (!isAbsolute(path)) throw new Error(`${label} must be absolute`);
  return path;
}

function decimalU64(value: unknown, label: string, positive = false): string {
  const raw = text(value, label);
  if (!/^(0|[1-9][0-9]*)$/.test(raw)) throw new Error(`${label} must be a canonical decimal u64`);
  const parsed = BigInt(raw);
  if (parsed > 0xffff_ffff_ffff_ffffn || (positive && parsed === 0n)) {
    throw new Error(`${label} is outside its admitted u64 range`);
  }
  return raw;
}

function parseMarketArguments(kind: MarketKindV1, value: unknown): ReadonlyArray<string> {
  if (!Array.isArray(value) || value.length === 0 || value.length % 2 !== 0) {
    throw new Error('market.arguments must be nonempty flag/value pairs');
  }
  const allowed = kind === 'flagship' ? FLAGSHIP_MARKET_FLAGS : GRADUATION_MARKET_FLAGS;
  const seen = new Set<string>();
  const parsed: string[] = [];
  for (let index = 0; index < value.length; index += 2) {
    const flag = text(value[index], `market.arguments[${index}]`);
    const argument = text(value[index + 1], `market.arguments[${index + 1}]`);
    if (!flag.startsWith('--') || !allowed.has(flag) || SHARED_CHILD_FLAGS.has(flag)) {
      throw new Error(`${flag} is not an admitted ${kind} market argument`);
    }
    if (seen.has(flag)) throw new Error(`${flag} may appear only once in market.arguments`);
    seen.add(flag);
    parsed.push(flag, argument);
  }
  const required = kind === 'flagship'
    ? ['--registry-program-id', '--direct-fee-basis-points', '--direct-fee-recipient', '--price-update', '--window-start']
    : [
      '--registry-program-id', '--direct-fee-basis-points', '--direct-fee-recipient',
      '--relayer-attestation', '--pool', '--venue-deployment-slot',
      '--venue-upgrade-authority', '--venue-elf-sha256', '--window-start', '--window-end',
      '--max-age-seconds',
    ];
  for (const flag of required) if (!seen.has(flag)) throw new Error(`market.arguments omitted required ${flag}`);
  if (kind === 'flagship') {
    const priceIndex = parsed.indexOf('--price-update');
    absolutePath(parsed[priceIndex + 1], '--price-update');
  }
  return Object.freeze(parsed);
}

function parseOperation(source: Uint8Array): FoundOperationV1 {
  const decoded = exactJson(source, MAX_OPERATION_BYTES, 'found operation');
  const root = object(decoded, 'found operation');
  exactKeys(root, ['schema', 'plan', 'market', 'campaign', 'participant'], 'found operation');
  if (root.schema !== OPERATION_SCHEMA_V1) throw new Error(`found operation schema must be ${OPERATION_SCHEMA_V1}`);

  const market = object(root.market, 'market');
  exactKeys(market, ['kind', 'arguments', 'output'], 'market');
  if (market.kind !== 'flagship' && market.kind !== 'graduation') throw new Error('market.kind must be flagship or graduation');
  const marketKind: MarketKindV1 = market.kind;

  const campaign = object(root.campaign, 'campaign');
  exactKeys(campaign, ['evidence', 'keypairs'], 'campaign');
  if (!Array.isArray(campaign.keypairs)) throw new Error('campaign.keypairs must be one array');
  const keypairs = campaign.keypairs.map((entry, index) => {
    const row = object(entry, `campaign.keypairs[${index}]`);
    exactKeys(row, ['role', 'path'], `campaign.keypairs[${index}]`);
    return Object.freeze({ role: text(row.role, `campaign.keypairs[${index}].role`), path: absolutePath(row.path, `campaign.keypairs[${index}].path`) });
  });
  const roles = keypairs.map((row) => row.role).sort();
  const requiredRoles = [...REQUIRED_CAMPAIGN_ROLES].sort();
  if (roles.length !== requiredRoles.length || roles.some((role, index) => role !== requiredRoles[index])) {
    throw new Error(`campaign.keypairs roles must be exactly ${requiredRoles.join(', ')}`);
  }
  if (new Set(keypairs.map((row) => row.path)).size !== keypairs.length) {
    throw new Error('campaign keypair paths must be distinct');
  }

  const participant = object(root.participant, 'participant');
  exactKeys(participant, [
    'output', 'positionOwner', 'positionOwnerKeypair', 'feePayer', 'feePayerKeypair',
    'minimumFinalizedSlot', 'collateral',
  ], 'participant');
  let collateral: ParticipantCollateralV1 | null = null;
  if (participant.collateral !== null) {
    const row = object(participant.collateral, 'participant.collateral');
    exactKeys(row, ['sourceOwner', 'sourceOwnerKeypair', 'sourceAccount', 'quantityAtoms'], 'participant.collateral');
    collateral = Object.freeze({
      sourceOwner: text(row.sourceOwner, 'participant.collateral.sourceOwner'),
      sourceOwnerKeypair: absolutePath(row.sourceOwnerKeypair, 'participant.collateral.sourceOwnerKeypair'),
      sourceAccount: text(row.sourceAccount, 'participant.collateral.sourceAccount'),
      quantityAtoms: decimalU64(row.quantityAtoms, 'participant.collateral.quantityAtoms', true),
    });
  }

  const operation: FoundOperationV1 = Object.freeze({
    schema: OPERATION_SCHEMA_V1,
    plan: absolutePath(root.plan, 'plan'),
    market: Object.freeze({
      kind: marketKind,
      arguments: parseMarketArguments(marketKind, market.arguments),
      output: absolutePath(market.output, 'market.output'),
    }),
    campaign: Object.freeze({ evidence: absolutePath(campaign.evidence, 'campaign.evidence'), keypairs: Object.freeze(keypairs) }),
    participant: Object.freeze({
      output: absolutePath(participant.output, 'participant.output'),
      positionOwner: text(participant.positionOwner, 'participant.positionOwner'),
      positionOwnerKeypair: absolutePath(participant.positionOwnerKeypair, 'participant.positionOwnerKeypair'),
      feePayer: text(participant.feePayer, 'participant.feePayer'),
      feePayerKeypair: absolutePath(participant.feePayerKeypair, 'participant.feePayerKeypair'),
      minimumFinalizedSlot: decimalU64(participant.minimumFinalizedSlot, 'participant.minimumFinalizedSlot', true),
      collateral,
    }),
  });
  const outputs = [operation.market.output, operation.campaign.evidence, operation.participant.output];
  if (new Set(outputs).size !== outputs.length || outputs.includes(operation.plan)) {
    throw new Error('market, campaign, and participant outputs must be distinct and must not overwrite the plan');
  }
  return operation;
}

function sha256(source: Uint8Array): string {
  return createHash('sha256').update(source).digest('hex');
}

function canonicalJournal(body: JournalBodyV1): JournalV1 {
  const bodyBytes = Buffer.from(JSON.stringify(body));
  return Object.freeze({ ...body, bodySha256: sha256(bodyBytes) });
}

function journalBytes(journal: JournalV1): Uint8Array {
  return Buffer.from(`${JSON.stringify(journal, null, 2)}\n`);
}

function validateJournal(source: Uint8Array): JournalV1 {
  const root = object(exactJson(source, MAX_EVIDENCE_BYTES, 'found journal'), 'found journal');
  exactKeys(root, [
    'schema', 'phase', 'sequence', 'authorizedMutation', 'operationSha256', 'successorSha256',
    'planSha256', 'rpcUrl', 'devnetGenesis', 'marketInputSha256', 'marketInputBase64',
    'campaignEvidenceSha256', 'participantEvidenceSha256', 'bodySha256',
  ], 'found journal');
  if (root.schema !== JOURNAL_SCHEMA_V1) throw new Error(`found journal schema must be ${JOURNAL_SCHEMA_V1}`);
  const phase = root.phase;
  if (!['planned', 'market-authored', 'market-prepared', 'founding-complete', 'participant-complete'].includes(String(phase))) {
    throw new Error('found journal phase is unknown');
  }
  if (!Number.isSafeInteger(root.sequence) || Number(root.sequence) < 0 || Number(root.sequence) > 5) throw new Error('found journal sequence is invalid');
  if (typeof root.authorizedMutation !== 'boolean') throw new Error('found journal authorizedMutation is invalid');
  const nullableDigest = (value: unknown, label: string): string | null => value === null ? null : digest(value, label);
  const body: JournalBodyV1 = Object.freeze({
    schema: JOURNAL_SCHEMA_V1,
    phase: phase as JournalPhaseV1,
    sequence: Number(root.sequence),
    authorizedMutation: root.authorizedMutation,
    operationSha256: digest(root.operationSha256, 'operationSha256'),
    successorSha256: digest(root.successorSha256, 'successorSha256'),
    planSha256: digest(root.planSha256, 'planSha256'),
    rpcUrl: text(root.rpcUrl, 'rpcUrl'),
    devnetGenesis: root.devnetGenesis === SOLANA_DEVNET_GENESIS_HASH_V1
      ? SOLANA_DEVNET_GENESIS_HASH_V1
      : (() => { throw new Error('found journal does not name exact Solana devnet'); })(),
    marketInputSha256: nullableDigest(root.marketInputSha256, 'marketInputSha256'),
    marketInputBase64: root.marketInputBase64 === null ? null : canonicalBase64(root.marketInputBase64, 'marketInputBase64'),
    campaignEvidenceSha256: nullableDigest(root.campaignEvidenceSha256, 'campaignEvidenceSha256'),
    participantEvidenceSha256: nullableDigest(root.participantEvidenceSha256, 'participantEvidenceSha256'),
  });
  const expected = canonicalJournal(body);
  if (root.bodySha256 !== expected.bodySha256) throw new Error('found journal body digest disagrees with its exact fields');
  if ((body.marketInputSha256 === null) !== (body.marketInputBase64 === null)) throw new Error('found journal market input is partial');
  if (body.marketInputBase64 !== null && sha256(Buffer.from(body.marketInputBase64, 'base64')) !== body.marketInputSha256) {
    throw new Error('found journal market input digest disagrees with its saved bytes');
  }
  const phaseShape: Readonly<Record<JournalPhaseV1, Readonly<{
    minimumSequence: number;
    maximumSequence: number;
    market: boolean;
    campaign: boolean;
    participant: boolean;
  }>>> = Object.freeze({
    planned: Object.freeze({ minimumSequence: 0, maximumSequence: 0, market: false, campaign: false, participant: false }),
    'market-authored': Object.freeze({ minimumSequence: 1, maximumSequence: 1, market: true, campaign: false, participant: false }),
    'market-prepared': Object.freeze({ minimumSequence: 2, maximumSequence: 3, market: true, campaign: false, participant: false }),
    'founding-complete': Object.freeze({ minimumSequence: 3, maximumSequence: 4, market: true, campaign: true, participant: false }),
    'participant-complete': Object.freeze({ minimumSequence: 4, maximumSequence: 5, market: true, campaign: true, participant: true }),
  });
  const shape = phaseShape[body.phase];
  if (body.sequence < shape.minimumSequence || body.sequence > shape.maximumSequence
    || (body.marketInputSha256 !== null) !== shape.market
    || (body.campaignEvidenceSha256 !== null) !== shape.campaign
    || (body.participantEvidenceSha256 !== null) !== shape.participant
    || ((shape.campaign || shape.participant) && !body.authorizedMutation)) {
    throw new Error('found journal phase, sequence, authorization, and evidence fields disagree');
  }
  return expected;
}

function canonicalBase64(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.length === 0
    || value.length > Math.ceil(MAX_MARKET_INPUT_BYTES / 3) * 4
    || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error(`${label} must be bounded canonical base64`);
  }
  const decoded = Buffer.from(value, 'base64');
  if (decoded.length === 0 || decoded.length > MAX_MARKET_INPUT_BYTES || decoded.toString('base64') !== value) {
    throw new Error(`${label} must be bounded canonical base64`);
  }
  return value;
}

function digest(value: unknown, label: string): string {
  const raw = text(value, label);
  if (!/^[0-9a-f]{64}$/.test(raw)) throw new Error(`${label} must be lowercase SHA-256`);
  return raw;
}

function fsyncDirectory(path: string): void {
  const descriptor = openSync(path, 'r');
  try { fsyncSync(descriptor); } finally { closeSync(descriptor); }
}

function operationLockPath(journalPath: string): string {
  return `${journalPath}.lock`;
}

function operationLeaseLinkMatches(path: string, device: bigint, inode: bigint): boolean {
  try {
    const linked = lstatSync(path, { bigint: true });
    return linked.dev === device && linked.ino === inode;
  } catch {
    return false;
  }
}

function acquireOperationLease(operationPath: string, journalPath: string): OperationLeaseV1 {
  const path = operationLockPath(journalPath);
  const parent = dirname(path);
  mkdirSync(parent, { recursive: true });
  let descriptor: number;
  try {
    descriptor = openSync(path, 'wx', 0o600);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'EEXIST') {
      throw new Error(
        `found operation is locked at ${path}; locks are never removed automatically. `
        + 'Confirm that no process owns it before removing a stale lock manually',
      );
    }
    throw error;
  }
  let device: bigint | null = null;
  let inode: bigint | null = null;
  try {
    const metadata = fstatSync(descriptor, { bigint: true });
    device = metadata.dev;
    inode = metadata.ino;
    const owner = Buffer.from(`${JSON.stringify({
      schema: OPERATION_LOCK_SCHEMA_V1,
      pid: process.pid,
      operation: operationPath,
      journal: journalPath,
      createdAtUnixMs: Date.now(),
      stalePolicy: 'never-auto-remove; confirm no live owner, then remove manually',
    }, null, 2)}\n`);
    let offset = 0;
    while (offset < owner.length) offset += writeSync(descriptor, owner, offset, owner.length - offset);
    fsyncSync(descriptor);
    fsyncDirectory(parent);
    return Object.freeze({ path, descriptor, device: metadata.dev, inode: metadata.ino });
  } catch (error) {
    if (device !== null && inode !== null && operationLeaseLinkMatches(path, device, inode)) {
      try { unlinkSync(path); } catch { /* preserve the acquisition failure */ }
    }
    try { closeSync(descriptor); } catch { /* preserve the acquisition failure */ }
    throw error;
  }
}

function releaseOperationLease(lease: OperationLeaseV1): Error | null {
  let failure: Error | null = null;
  try {
    const held = fstatSync(lease.descriptor, { bigint: true });
    if (held.dev === lease.device && held.ino === lease.inode
      && operationLeaseLinkMatches(lease.path, lease.device, lease.inode)) {
      unlinkSync(lease.path);
      fsyncDirectory(dirname(lease.path));
    }
  } catch (error) {
    failure = error instanceof Error ? error : new Error(String(error));
  }
  try { closeSync(lease.descriptor); } catch (error) {
    failure ??= error instanceof Error ? error : new Error(String(error));
  }
  return failure;
}

function writeTemporary(path: string, source: Uint8Array): string {
  const parent = dirname(path);
  mkdirSync(parent, { recursive: true });
  const temporary = resolve(parent, `.${basename(path)}.dclutch-${process.pid}-${randomBytes(8).toString('hex')}.tmp`);
  const descriptor = openSync(temporary, 'wx', 0o600);
  try {
    let offset = 0;
    while (offset < source.length) offset += writeSync(descriptor, source, offset, source.length - offset);
    fsyncSync(descriptor);
  } finally {
    closeSync(descriptor);
  }
  return temporary;
}

function createDurable(path: string, source: Uint8Array): void {
  if (existsSync(path)) throw new Error(`${path} already exists`);
  const temporary = writeTemporary(path, source);
  try {
    linkSync(temporary, path);
    unlinkSync(temporary);
    fsyncDirectory(dirname(path));
  } catch (error) {
    try { unlinkSync(temporary); } catch { /* already removed */ }
    throw error;
  }
}

function replaceDurable(path: string, expected: Uint8Array, replacement: Uint8Array): void {
  const actual = readFileSync(path);
  if (!Buffer.from(actual).equals(Buffer.from(expected))) throw new Error(`${path} changed since it was authenticated`);
  const temporary = writeTemporary(path, replacement);
  try {
    renameSync(temporary, path);
    fsyncDirectory(dirname(path));
  } catch (error) {
    try { unlinkSync(temporary); } catch { /* renamed or already absent */ }
    throw error;
  }
}

function persistJournal(path: string, previous: Uint8Array | null, body: JournalBodyV1): { journal: JournalV1; bytes: Uint8Array } {
  const journal = canonicalJournal(body);
  const bytes = journalBytes(journal);
  if (previous === null) createDurable(path, bytes);
  else replaceDurable(path, previous, bytes);
  return { journal, bytes };
}

function transition(journal: JournalV1, patch: Partial<JournalBodyV1>): JournalBodyV1 {
  const { bodySha256, ...body } = journal;
  void bodySha256;
  return Object.freeze({ ...body, ...patch, sequence: journal.sequence + 1 });
}

function readBounded(path: string, maximum: number, label: string): Uint8Array {
  const stat = lstatSync(path);
  if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`${label} must be one ordinary file`);
  if (stat.size <= 0 || stat.size > maximum) throw new Error(`${label} is outside its byte bound`);
  return readFileSync(path);
}

function writeExactOutput(path: string, source: Uint8Array): void {
  if (existsSync(path)) {
    const existing = readFileSync(path);
    if (!Buffer.from(existing).equals(Buffer.from(source))) throw new Error(`${path} already contains different bytes`);
    return;
  }
  createDurable(path, source);
}

function invokeChecked(
  dependencies: FoundOperationDependenciesV1,
  binary: string,
  arguments_: ReadonlyArray<string>,
  label: string,
  io: Io,
): InvocationResultV1 {
  const result = dependencies.invoke(binary, arguments_);
  const stderr = Buffer.from(result.stderr).toString('utf8').trim();
  if (stderr !== '') io.err(stderr);
  if (result.status !== 0) throw new Error(`${label} exited ${result.status ?? 'by signal'}`);
  return result;
}

function parseJsonEvidence(source: Uint8Array, maximum: number, label: string): Record<string, unknown> {
  return object(exactJson(source, maximum, label), label);
}

function validateCampaignEvidence(source: Uint8Array, journal: JournalV1): Record<string, unknown> {
  const report = parseJsonEvidence(source, MAX_EVIDENCE_BYTES, 'campaign evidence');
  if (report.schema !== CAMPAIGN_SCHEMA_V1 || report.cluster !== 'devnet'
    || report.mode !== 'execute'
    || report.plan_sha256 !== journal.planSha256
    || report.market_sha256 !== journal.marketInputSha256) {
    throw new Error('campaign evidence does not join this exact devnet operation, plan, and Market input');
  }
  const execution = object(report.execution, 'campaign evidence execution');
  const market = object(execution.market, 'campaign evidence execution.market');
  const accounts = object(market.accounts, 'campaign evidence execution.market.accounts');
  const founding = object(accounts.founding_market, 'campaign founding_market evidence');
  text(founding.address, 'campaign founding Market address');
  if (execution.completed !== true) throw new Error('campaign evidence is not complete');
  return report;
}

function validateParticipantEvidence(source: Uint8Array, journal: JournalV1, operation: FoundOperationV1): Record<string, unknown> {
  const report = parseJsonEvidence(source, MAX_EVIDENCE_BYTES, 'participant evidence');
  const intent = object(report.intent, 'participant intent');
  if (report.schema !== PARTICIPANT_SCHEMA_V1 || report.cluster !== 'devnet'
    || intent.planSha256 !== journal.planSha256
    || intent.campaignEvidenceSha256 !== journal.campaignEvidenceSha256
    || report.phase !== 'finalized'
    || intent.positionOwner !== operation.participant.positionOwner
    || intent.feePayer !== operation.participant.feePayer
    || String(intent.minimumFinalizedSlot) !== operation.participant.minimumFinalizedSlot) {
    throw new Error('participant evidence does not finalize this exact devnet plan and campaign');
  }
  if (operation.participant.collateral !== null) {
    const collateral = object(report.collateral, 'participant collateral evidence');
    const collateralIntent = object(collateral.intent, 'participant collateral intent');
    if (collateral.phase !== 'finalized'
      || collateralIntent.sourceOwner !== operation.participant.collateral.sourceOwner
      || collateralIntent.sourceAccount !== operation.participant.collateral.sourceAccount
      || String(collateralIntent.quantityAtoms) !== operation.participant.collateral.quantityAtoms) {
      throw new Error('participant collateral preparation did not finalize the exact requested source and quantity');
    }
  } else if (report.collateral !== undefined) {
    throw new Error('participant evidence contains unrequested collateral preparation');
  }
  return report;
}

function marketCommand(kind: MarketKindV1): string {
  return kind === 'flagship' ? 'devnet-market' : 'graduation-market';
}

function campaignArguments(operation: FoundOperationV1, rpcUrl: string, execute: boolean): string[] {
  const arguments_ = [
    'campaign', '--rpc-url', rpcUrl, '--i-mean-devnet', SOLANA_DEVNET_GENESIS_HASH_V1,
    '--plan', operation.plan, '--market', operation.market.output,
    '--evidence', operation.campaign.evidence, '--through', 'founding',
  ];
  if (execute) arguments_.push('--execute');
  for (const row of operation.campaign.keypairs) arguments_.push(`--keypair-${row.role}`, row.path);
  return arguments_;
}

function participantArguments(operation: FoundOperationV1, rpcUrl: string, execute: boolean): string[] {
  const participant = operation.participant;
  const arguments_ = [
    'devnet-user-position-admission-v1', '--rpc-url', rpcUrl,
    '--i-mean-devnet', SOLANA_DEVNET_GENESIS_HASH_V1,
    '--plan', operation.plan, '--campaign-evidence', operation.campaign.evidence,
    '--position-owner', participant.positionOwner,
    '--position-owner-keypair', participant.positionOwnerKeypair,
    '--fee-payer', participant.feePayer,
    '--fee-payer-keypair', participant.feePayerKeypair,
    '--minimum-finalized-slot', participant.minimumFinalizedSlot,
    '--output', participant.output,
  ];
  if (execute) arguments_.push('--execute');
  if (participant.collateral !== null) {
    arguments_.push(
      '--collateral-source-owner', participant.collateral.sourceOwner,
      '--collateral-source-owner-keypair', participant.collateral.sourceOwnerKeypair,
      '--collateral-source-account', participant.collateral.sourceAccount,
      '--collateral-quantity-atoms', participant.collateral.quantityAtoms,
    );
  }
  return arguments_;
}

function readJournalEvidence(path: string, expectedSha256: string | null, label: string): Uint8Array {
  if (expectedSha256 === null) throw new Error(`found journal omitted its ${label} digest`);
  const source = readBounded(path, MAX_EVIDENCE_BYTES, label);
  if (sha256(source) !== expectedSha256) {
    throw new Error(`${label} changed after it was durably joined to the found journal`);
  }
  return source;
}

function authenticatePhaseArtifacts(journal: JournalV1, operation: FoundOperationV1): void {
  if (journal.phase === 'planned' || journal.phase === 'market-authored') return;
  const market = readBounded(operation.market.output, MAX_MARKET_INPUT_BYTES, 'Market input');
  const saved = Buffer.from(journal.marketInputBase64 ?? '', 'base64');
  if (sha256(market) !== journal.marketInputSha256 || !Buffer.from(market).equals(saved)) {
    throw new Error('Market input changed after it was durably joined to the found journal');
  }
  if (journal.phase === 'founding-complete' || journal.phase === 'participant-complete') {
    readJournalEvidence(operation.campaign.evidence, journal.campaignEvidenceSha256, 'campaign evidence');
  }
  if (journal.phase === 'participant-complete') {
    readJournalEvidence(operation.participant.output, journal.participantEvidenceSha256, 'participant evidence');
  }
}

function authenticateCampaignOwner(
  dependencies: FoundOperationDependenciesV1,
  binary: string,
  operation: FoundOperationV1,
  journal: JournalV1,
  io: Io,
): Record<string, unknown> {
  const before = readJournalEvidence(operation.campaign.evidence, journal.campaignEvidenceSha256, 'campaign evidence');
  invokeChecked(
    dependencies,
    binary,
    campaignArguments(operation, journal.rpcUrl, false),
    'founding campaign authentication',
    io,
  );
  const after = readJournalEvidence(operation.campaign.evidence, journal.campaignEvidenceSha256, 'campaign evidence');
  if (!Buffer.from(after).equals(Buffer.from(before))) {
    throw new Error('founding campaign authentication changed its finalized evidence');
  }
  return validateCampaignEvidence(after, journal);
}

function authenticateParticipantOwner(
  dependencies: FoundOperationDependenciesV1,
  binary: string,
  operation: FoundOperationV1,
  journal: JournalV1,
  io: Io,
): Record<string, unknown> {
  const before = readJournalEvidence(operation.participant.output, journal.participantEvidenceSha256, 'participant evidence');
  invokeChecked(
    dependencies,
    binary,
    participantArguments(operation, journal.rpcUrl, false),
    'participant admission authentication',
    io,
  );
  const after = readJournalEvidence(operation.participant.output, journal.participantEvidenceSha256, 'participant evidence');
  if (!Buffer.from(after).equals(Buffer.from(before))) {
    throw new Error('participant admission authentication changed its finalized evidence');
  }
  return validateParticipantEvidence(after, journal, operation);
}

function writeSession(path: string, rpcUrl: string, planSource: Uint8Array, campaign: Record<string, unknown>): void {
  const plan = JSON.parse(Buffer.from(planSource).toString('utf8')) as unknown;
  const decoded = decodeSession(plan);
  const execution = object(campaign.execution, 'campaign execution');
  const market = object(execution.market, 'campaign market');
  const accounts = object(market.accounts, 'campaign accounts');
  const founding = object(accounts.founding_market, 'campaign founding market');
  const session = Buffer.from(`${JSON.stringify({
    schema: 'dclutch-cli-session-v1',
    rpcUrl,
    programs: decoded.programs,
    markets: [text(founding.address, 'founding Market address')],
  }, null, 2)}\n`);
  writeExactOutput(path, session);
}

/**
 * Prepare or resume one permanent-devnet founding + participant operation.
 * Default is read-only Market-input preparation. `execute` is durably recorded
 * before either signer-owning child is invoked.
 */
function runFoundOperationUnderLeaseV1(
  context: CliContext,
  io: Io,
  binary: string,
  operationPath: string,
  journalPath: string,
  sessionOut: string | null,
  execute: boolean,
  dependencies: FoundOperationDependenciesV1 = DEFAULT_DEPENDENCIES,
): number {
  if (!isAbsolute(operationPath) || !isAbsolute(journalPath)) throw new Error('--found-operation and --found-journal must be absolute');
  if (typeof context.flags.rpc !== 'string') throw new Error('devnet founding requires an explicit --rpc URL');
  if (context.flags['i-mean-devnet'] !== SOLANA_DEVNET_GENESIS_HASH_V1) {
    throw new Error(`--i-mean-devnet must equal Solana devnet's full genesis hash ${SOLANA_DEVNET_GENESIS_HASH_V1}`);
  }
  const operationSource = readBounded(operationPath, MAX_OPERATION_BYTES, 'found operation');
  const operation = parseOperation(operationSource);
  const planSource = readBounded(operation.plan, MAX_EVIDENCE_BYTES, 'successor plan');
  const binarySource = readBounded(binary, MAX_BINARY_BYTES, 'successor binary');
  const expected = {
    operationSha256: sha256(operationSource),
    successorSha256: sha256(binarySource),
    planSha256: sha256(planSource),
  };
  if ([operationPath, journalPath, operationLockPath(journalPath), binary]
    .some((path) => [operation.market.output, operation.campaign.evidence, operation.participant.output].includes(path))) {
    throw new Error('operation outputs must not overwrite the operation, journal, operation lock, or successor binary');
  }

  let journalBytesBefore: Uint8Array;
  let journal: JournalV1;
  if (!existsSync(journalPath)) {
    const persisted = persistJournal(journalPath, null, Object.freeze({
      schema: JOURNAL_SCHEMA_V1,
      phase: 'planned',
      sequence: 0,
      authorizedMutation: false,
      ...expected,
      rpcUrl: context.rpcUrl,
      devnetGenesis: SOLANA_DEVNET_GENESIS_HASH_V1,
      marketInputSha256: null,
      marketInputBase64: null,
      campaignEvidenceSha256: null,
      participantEvidenceSha256: null,
    }));
    journal = persisted.journal;
    journalBytesBefore = persisted.bytes;
  } else {
    journalBytesBefore = readBounded(journalPath, MAX_EVIDENCE_BYTES, 'found journal');
    journal = validateJournal(journalBytesBefore);
    if (journal.operationSha256 !== expected.operationSha256 || journal.successorSha256 !== expected.successorSha256 || journal.planSha256 !== expected.planSha256 || journal.rpcUrl !== context.rpcUrl) {
      throw new Error('found journal belongs to another operation, successor binary, plan, or RPC origin');
    }
  }

  const persist = (body: JournalBodyV1): void => {
    const saved = persistJournal(journalPath, journalBytesBefore, body);
    journal = saved.journal;
    journalBytesBefore = saved.bytes;
  };

  if (journal.phase === 'planned') {
    const builderArguments = [
      marketCommand(operation.market.kind), ...operation.market.arguments,
      '--plan', operation.plan, '--rpc-url', context.rpcUrl,
      '--i-mean-devnet', SOLANA_DEVNET_GENESIS_HASH_V1,
    ];
    const result = invokeChecked(dependencies, binary, builderArguments, 'Market input producer', io);
    const market = result.stdout;
    parseJsonEvidence(market, MAX_MARKET_INPUT_BYTES, 'Market input');
    persist(transition(journal, {
      phase: 'market-authored',
      marketInputSha256: sha256(market),
      marketInputBase64: Buffer.from(market).toString('base64'),
    }));
  }

  if (journal.phase === 'market-authored') {
    const market = Buffer.from(journal.marketInputBase64 ?? '', 'base64');
    writeExactOutput(operation.market.output, market);
    persist(transition(journal, { phase: 'market-prepared' }));
  }

  let campaignAuthenticated = false;
  let participantAuthenticated = false;
  authenticatePhaseArtifacts(journal, operation);
  if (journal.phase === 'founding-complete' || journal.phase === 'participant-complete') {
    authenticateCampaignOwner(dependencies, binary, operation, journal, io);
    campaignAuthenticated = true;
  }
  if (journal.phase === 'participant-complete') {
    authenticateParticipantOwner(dependencies, binary, operation, journal, io);
    participantAuthenticated = true;
  }

  io.out(`market input prepared at ${operation.market.output} (${journal.marketInputSha256})`);
  if (!execute) {
    if (journal.phase === 'participant-complete') {
      const campaignSource = readJournalEvidence(
        operation.campaign.evidence,
        journal.campaignEvidenceSha256,
        'campaign evidence',
      );
      const participantSource = readJournalEvidence(
        operation.participant.output,
        journal.participantEvidenceSha256,
        'participant evidence',
      );
      const campaign = validateCampaignEvidence(campaignSource, journal);
      validateParticipantEvidence(participantSource, journal, operation);
      if (sessionOut !== null) {
        writeSession(
          isAbsolute(sessionOut) ? sessionOut : resolve(process.cwd(), sessionOut),
          context.rpcUrl,
          planSource,
          campaign,
        );
      }
      io.out(`founding complete: ${operation.campaign.evidence}`);
      io.out(`participant admission complete: ${operation.participant.output}`);
      io.out(`operation journal complete: ${journalPath}`);
      return 0;
    }
    io.out('read-only preparation complete; rerun the same operation and journal with --execute to found and admit the participant');
    return 0;
  }
  if (!journal.authorizedMutation) persist(transition(journal, { authorizedMutation: true }));

  if (journal.phase === 'market-prepared') {
    // The Rust campaign owns prior-report authentication and suffix recovery.
    // Always dispatch it: on a completed report it returns the exact preserved
    // bytes, while a partial report is resumed only from its authenticated
    // checkpoint. This exterior never substitutes a second report validator.
    authenticatePhaseArtifacts(journal, operation);
    invokeChecked(dependencies, binary, campaignArguments(operation, context.rpcUrl, true), 'founding campaign', io);
    const source = readBounded(operation.campaign.evidence, MAX_EVIDENCE_BYTES, 'campaign evidence');
    validateCampaignEvidence(source, journal);
    persist(transition(journal, { phase: 'founding-complete', campaignEvidenceSha256: sha256(source) }));
    campaignAuthenticated = true;
  }

  if (journal.phase === 'founding-complete') {
    // The participant producer owns its submitted/finalized report and live
    // poststate authentication. An existing output is deliberately passed
    // back to that producer rather than trusted as a static local projection.
    authenticatePhaseArtifacts(journal, operation);
    if (!campaignAuthenticated) {
      authenticateCampaignOwner(dependencies, binary, operation, journal, io);
      campaignAuthenticated = true;
    }
    invokeChecked(dependencies, binary, participantArguments(operation, context.rpcUrl, true), 'participant admission', io);
    const source = readBounded(operation.participant.output, MAX_EVIDENCE_BYTES, 'participant evidence');
    validateParticipantEvidence(source, journal, operation);
    persist(transition(journal, { phase: 'participant-complete', participantEvidenceSha256: sha256(source) }));
    participantAuthenticated = true;
  }

  if (journal.phase !== 'participant-complete') throw new Error(`found operation stopped at unexpected phase ${journal.phase}`);
  authenticatePhaseArtifacts(journal, operation);
  if (!campaignAuthenticated) authenticateCampaignOwner(dependencies, binary, operation, journal, io);
  if (!participantAuthenticated) authenticateParticipantOwner(dependencies, binary, operation, journal, io);
  // Re-read both after their canonical owners return. The complete journal is
  // never accepted from its TypeScript envelope or a pre-poll snapshot alone.
  const campaignSource = readJournalEvidence(operation.campaign.evidence, journal.campaignEvidenceSha256, 'campaign evidence');
  const participantSource = readJournalEvidence(operation.participant.output, journal.participantEvidenceSha256, 'participant evidence');
  const campaign = validateCampaignEvidence(campaignSource, journal);
  validateParticipantEvidence(participantSource, journal, operation);
  if (sessionOut !== null) writeSession(isAbsolute(sessionOut) ? sessionOut : resolve(process.cwd(), sessionOut), context.rpcUrl, planSource, campaign);
  io.out(`founding complete: ${operation.campaign.evidence}`);
  io.out(`participant admission complete: ${operation.participant.output}`);
  io.out(`operation journal complete: ${journalPath}`);
  return 0;
}

export function runFoundOperationV1(
  context: CliContext,
  io: Io,
  binary: string,
  operationPath: string,
  journalPath: string,
  sessionOut: string | null,
  execute: boolean,
  dependencies: FoundOperationDependenciesV1 = DEFAULT_DEPENDENCIES,
): number {
  if (!isAbsolute(operationPath) || !isAbsolute(journalPath)) {
    throw new Error('--found-operation and --found-journal must be absolute');
  }
  if (typeof context.flags.rpc !== 'string') throw new Error('devnet founding requires an explicit --rpc URL');
  if (context.flags['i-mean-devnet'] !== SOLANA_DEVNET_GENESIS_HASH_V1) {
    throw new Error(`--i-mean-devnet must equal Solana devnet's full genesis hash ${SOLANA_DEVNET_GENESIS_HASH_V1}`);
  }
  const lease = acquireOperationLease(operationPath, journalPath);
  let result: number | undefined;
  let failure: unknown;
  try {
    result = runFoundOperationUnderLeaseV1(
      context,
      io,
      binary,
      operationPath,
      journalPath,
      sessionOut,
      execute,
      dependencies,
    );
  } catch (error) {
    failure = error;
  }
  const releaseFailure = releaseOperationLease(lease);
  if (failure !== undefined) throw failure;
  if (releaseFailure !== null) throw releaseFailure;
  if (result === undefined) throw new Error('found operation returned no result');
  return result;
}

export const FOUND_OPERATION_SCHEMA_V1 = OPERATION_SCHEMA_V1;
