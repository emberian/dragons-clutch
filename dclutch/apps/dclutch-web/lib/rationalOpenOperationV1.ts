import { PublicKey, VersionedTransaction } from '@solana/web3.js';

import { fromHex, hex, sha256 } from './bytes';
import {
  submittedClientOperationWireV1,
  type ClientOperationJournalV1,
  type ClientOperationScopeV1,
} from './clientOperationJournal';
import {
  type RationalOpenCandidateV4,
  type RationalOpenChainInspectionV4,
  type RationalOpenPoststateV4,
  type RationalOpenPoststateContextV4,
} from './rationalOpenChainV4';
import { type RationalOpenActionV3 } from './rationalOpenHotV3';

export const RATIONAL_OPEN_JOURNAL_INTENT_FORMAT_V1 = 'dclutch-rational-open-journal-intent-v1' as const;
export const RATIONAL_OPEN_JOURNAL_PLAN_FORMAT_V1 = 'dclutch-rational-open-journal-plan-v1' as const;

const MAX_U64 = 18_446_744_073_709_551_615n;
const MAX_OUTCOMES = 4_096;
const MAX_PACKET_BASE64 = 2_000;

type RationalOpenJournalIntentV1 = Readonly<{
  format: typeof RATIONAL_OPEN_JOURNAL_INTENT_FORMAT_V1;
  action: RationalOpenActionV3;
  market: string;
  owner: string;
  actor: string;
  observedSlot: string;
  selectedOutcome: number | null;
  rawQuantity: string;
  descriptorId: string;
  capabilityDigest: string;
  rootDigest: string;
  familyDigest: string;
  childDigest: string;
  outerDigest: string;
  lookupTable: string;
}>;

type RationalOpenJournalPlanV1 = Readonly<{
  format: typeof RATIONAL_OPEN_JOURNAL_PLAN_FORMAT_V1;
  lastValidBlockHeight: string;
  unsignedWireBase64: string;
  unsignedWireSha256: string;
  poststate: unknown;
}>;

export type RestoredRationalOpenJournalV1 = Readonly<{
  intent: RationalOpenJournalIntentV1;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  lastValidBlockHeight: string;
  poststate: RationalOpenPoststateV4;
}>;

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

function parseObject(source: string, fields: ReadonlyArray<string>, label: string): Record<string, unknown> {
  let value: unknown;
  try { value = JSON.parse(source); } catch { throw new Error(`${label} is not JSON`); }
  return object(value, fields, label);
}

function address(value: unknown, label: string): string {
  if (typeof value !== 'string') throw new Error(`${label} is not text`);
  let parsed: PublicKey;
  try { parsed = new PublicKey(value); } catch { throw new Error(`${label} is not one Solana address`); }
  if (parsed.toBase58() !== value) throw new Error(`${label} is not canonical base58`);
  return value;
}

function u64Text(value: unknown, label: string, positive = false): string {
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${label} is not canonical u64 text`);
  const parsed = BigInt(value);
  if (parsed > MAX_U64 || (positive && parsed === 0n)) throw new Error(`${label} is outside canonical u64`);
  return value;
}

function u64Value(value: unknown, label: string): bigint {
  return BigInt(u64Text(value, label));
}

function outcome(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
    throw new Error(`${label} is not canonical u32`);
  }
  return value;
}

function digest(value: unknown, label: string): string {
  if (typeof value !== 'string' || !/^[0-9a-f]{64}$/.test(value)) throw new Error(`${label} is not exact lowercase SHA-256 hex`);
  return value;
}

function identity(value: unknown, label: string): Uint8Array {
  const bytes = fromHex(digest(value, label), label);
  if (bytes.every((byte) => byte === 0)) throw new Error(`${label} is the reserved zero identity`);
  return bytes;
}

function base64(bytes: Uint8Array): string {
  let output = '';
  for (let offset = 0; offset < bytes.length; offset += 8_192) output += String.fromCharCode(...bytes.subarray(offset, offset + 8_192));
  return btoa(output);
}

function base64Bytes(value: unknown): Uint8Array {
  if (typeof value !== 'string' || value.length === 0 || value.length > MAX_PACKET_BASE64 || value.length % 4 !== 0
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error('Rational open journal packet is not bounded canonical base64');
  }
  const bytes = Uint8Array.from(atob(value), (character) => character.charCodeAt(0));
  if (base64(bytes) !== value) throw new Error('Rational open journal packet is not bounded canonical base64');
  return bytes;
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function action(value: unknown): RationalOpenActionV3 {
  if (value !== 'denominate' && value !== 'reconstitute' && value !== 'issue-structured' && value !== 'unwrap-structured') {
    throw new Error('Rational open journal names an unsupported action');
  }
  return value;
}

function selectedAction(value: RationalOpenActionV3): boolean {
  return value === 'denominate' || value === 'reconstitute';
}

function serializePoststate(poststate: RationalOpenPoststateV4): unknown {
  const context = poststate.context;
  return {
    context: {
      claimsProgram: context.claimsProgram,
      descriptorId: hex(context.descriptorId),
      actor: context.actor,
      representationAuthority: context.representationAuthority,
      aggregate: context.aggregate,
      market: context.market,
      releaseSet: hex(context.releaseSet),
      registry: context.registry,
      product: hex(context.product),
      realm: hex(context.realm),
      generation: context.generation.toString(),
      outcomes: context.outcomes,
      basis: hex(context.basis),
      custodyContext: hex(context.custodyContext),
    },
    replay: { address: poststate.replay.address, revision: poststate.replay.revision.toString() },
    aggregate: poststate.aggregate === null ? null : {
      address: poststate.aggregate.address,
      revision: poststate.aggregate.revision.toString(),
      balances: poststate.aggregate.balances.map((value) => value.toString()),
    },
    positions: poststate.positions.map((row) => ({
      address: row.address,
      owner: row.owner,
      revision: row.revision.toString(),
      balances: row.balances.map((value) => value.toString()),
    })),
    receipt: poststate.receipt === null ? null : {
      mint: poststate.receipt.mint,
      supply: poststate.receipt.supply.toString(),
      account: poststate.receipt.account,
      amount: poststate.receipt.amount.toString(),
    },
    assets: poststate.assets.map((row) => ({
      mint: row.mint,
      mintSupply: row.mintSupply.toString(),
      actorAccount: row.actorAccount,
      actorAmount: row.actorAmount.toString(),
      structuredAccount: row.structuredAccount,
      structuredAmount: row.structuredAmount.toString(),
    })),
  };
}

function balances(value: unknown, width: number, label: string): ReadonlyArray<bigint> {
  if (!Array.isArray(value) || value.length !== width) throw new Error(`${label} has another Claims vector width`);
  return Object.freeze(value.map((item, index) => u64Value(item, `${label} ${index}`)));
}

function parsePoststate(value: unknown, openAction: RationalOpenActionV3): RationalOpenPoststateV4 {
  const raw = object(value, ['aggregate', 'assets', 'context', 'positions', 'receipt', 'replay'], 'Rational open poststate');
  const rawContext = object(raw.context, [
    'actor', 'aggregate', 'basis', 'claimsProgram', 'custodyContext', 'descriptorId', 'generation', 'market',
    'outcomes', 'product', 'realm', 'registry', 'releaseSet', 'representationAuthority',
  ], 'Rational open poststate context');
  const outcomes = outcome(rawContext.outcomes, 'poststate outcome count');
  if (outcomes === 0 || outcomes > MAX_OUTCOMES) throw new Error('poststate outcome count is outside its bounded positive width');
  const context: RationalOpenPoststateContextV4 = Object.freeze({
    claimsProgram: address(rawContext.claimsProgram, 'poststate Claims program'),
    descriptorId: identity(rawContext.descriptorId, 'poststate descriptor'),
    actor: address(rawContext.actor, 'poststate actor'),
    representationAuthority: address(rawContext.representationAuthority, 'poststate representation authority'),
    aggregate: address(rawContext.aggregate, 'poststate aggregate'),
    market: address(rawContext.market, 'poststate Market'),
    releaseSet: identity(rawContext.releaseSet, 'poststate release set'),
    registry: address(rawContext.registry, 'poststate Registry'),
    product: identity(rawContext.product, 'poststate Product'),
    realm: identity(rawContext.realm, 'poststate Realm'),
    generation: u64Value(rawContext.generation, 'poststate generation'),
    outcomes,
    basis: identity(rawContext.basis, 'poststate semantic basis'),
    custodyContext: identity(rawContext.custodyContext, 'poststate custody context'),
  });
  const rawReplay = object(raw.replay, ['address', 'revision'], 'Rational open replay poststate');
  const replay = Object.freeze({
    address: address(rawReplay.address, 'poststate replay'),
    revision: u64Value(rawReplay.revision, 'poststate replay revision'),
  });
  const selected = selectedAction(openAction);
  let aggregate: RationalOpenPoststateV4['aggregate'] = null;
  if (raw.aggregate !== null) {
    const row = object(raw.aggregate, ['address', 'balances', 'revision'], 'Rational open aggregate poststate');
    aggregate = Object.freeze({
      address: address(row.address, 'poststate aggregate row'),
      revision: u64Value(row.revision, 'poststate aggregate revision'),
      balances: balances(row.balances, outcomes, 'poststate aggregate balances'),
    });
  }
  if (!Array.isArray(raw.positions) || raw.positions.length !== (selected ? 2 : 0)) throw new Error('Rational open position poststate has another action shape');
  const positions = Object.freeze(raw.positions.map((value, index) => {
    const row = object(value, ['address', 'balances', 'owner', 'revision'], `Rational open Position poststate ${index}`);
    return Object.freeze({
      address: address(row.address, `poststate Position ${index}`),
      owner: address(row.owner, `poststate Position owner ${index}`),
      revision: u64Value(row.revision, `poststate Position revision ${index}`),
      balances: balances(row.balances, outcomes, `poststate Position balances ${index}`),
    });
  }));
  let receipt: RationalOpenPoststateV4['receipt'] = null;
  if (raw.receipt !== null) {
    const row = object(raw.receipt, ['account', 'amount', 'mint', 'supply'], 'Rational open receipt poststate');
    receipt = Object.freeze({
      mint: address(row.mint, 'poststate receipt Mint'),
      supply: u64Value(row.supply, 'poststate receipt supply'),
      account: address(row.account, 'poststate receipt account'),
      amount: u64Value(row.amount, 'poststate receipt amount'),
    });
  }
  if (!Array.isArray(raw.assets) || raw.assets.length !== (selected ? 1 : outcomes)) throw new Error('Rational open asset poststate has another action width');
  const assets = Object.freeze(raw.assets.map((value, index) => {
    const row = object(value, ['actorAccount', 'actorAmount', 'mint', 'mintSupply', 'structuredAccount', 'structuredAmount'], `Rational open asset poststate ${index}`);
    return Object.freeze({
      mint: address(row.mint, `poststate shard Mint ${index}`),
      mintSupply: u64Value(row.mintSupply, `poststate shard supply ${index}`),
      actorAccount: address(row.actorAccount, `poststate actor shard account ${index}`),
      actorAmount: u64Value(row.actorAmount, `poststate actor shard amount ${index}`),
      structuredAccount: address(row.structuredAccount, `poststate Structured custody ${index}`),
      structuredAmount: u64Value(row.structuredAmount, `poststate Structured custody amount ${index}`),
    });
  }));
  if ((selected && (aggregate === null || receipt !== null)) || (!selected && (aggregate !== null || receipt === null))) {
    throw new Error('Rational open poststate Claims/receipt shape differs from its action');
  }
  return Object.freeze({ context, replay, aggregate, positions, receipt, assets });
}

/** Bind one exact single-wallet packet and its finalized atom ledger before signing. */
export async function rationalOpenJournalInputV1(
  scope: ClientOperationScopeV1,
  inspection: RationalOpenChainInspectionV4,
  candidate: RationalOpenCandidateV4,
  lastValidBlockHeight: string,
): Promise<ClientOperationScopeV1 & Readonly<{
  operation: 'rational-open-v1'; operationDigest: string; intent: string; plan: string;
}>> {
  if (scope.market !== inspection.market || scope.owner !== inspection.payer || inspection.payer !== inspection.actor
      || inspection.poststate.context.market !== inspection.market || inspection.poststate.context.actor !== inspection.actor
      || candidate.requiredSigners.length !== 1 || candidate.requiredSigners[0] !== scope.owner) {
    throw new Error('Rational open journal scope, Market, actor, payer, and wallet signer disagree');
  }
  const transaction = candidate.transaction;
  const wireBytes = transaction.serialize();
  const instruction = transaction.message.compiledInstructions[0];
  if (!same(wireBytes, candidate.wireBytes) || transaction.signatures.length !== 1
      || transaction.signatures[0]?.some((byte) => byte !== 0)
      || transaction.message.header.numRequiredSignatures !== 1
      || transaction.message.staticAccountKeys[0]?.toBase58() !== scope.owner
      || transaction.message.compiledInstructions.length !== 1 || instruction === undefined
      || !same(instruction.data, candidate.outerBytes)
      || transaction.message.addressTableLookups.length !== 1
      || transaction.message.addressTableLookups[0]?.accountKey.toBase58() !== inspection.lookupTable.key.toBase58()) {
    throw new Error('Rational open journal received another packet, signer set, outer, or lookup table');
  }
  const intent: RationalOpenJournalIntentV1 = Object.freeze({
    format: RATIONAL_OPEN_JOURNAL_INTENT_FORMAT_V1,
    action: inspection.action,
    market: inspection.market,
    owner: scope.owner,
    actor: inspection.actor,
    observedSlot: u64Text(inspection.observedSlot, 'observed slot'),
    selectedOutcome: inspection.selectedOutcome,
    rawQuantity: u64Text(inspection.rawQuantity.toString(), 'raw quantity', true),
    descriptorId: hex(inspection.descriptorId),
    capabilityDigest: hex(inspection.capabilityDigest),
    rootDigest: hex(inspection.rootDigest),
    familyDigest: hex(inspection.family.familyDigest),
    childDigest: hex(inspection.family.childDigest),
    outerDigest: hex(await sha256(candidate.outerBytes)),
    lookupTable: inspection.lookupTable.key.toBase58(),
  });
  const plan: RationalOpenJournalPlanV1 = Object.freeze({
    format: RATIONAL_OPEN_JOURNAL_PLAN_FORMAT_V1,
    lastValidBlockHeight: u64Text(lastValidBlockHeight, 'last valid block height'),
    unsignedWireBase64: base64(wireBytes),
    unsignedWireSha256: hex(await sha256(wireBytes)),
    poststate: serializePoststate(inspection.poststate),
  });
  // Exercise the hostile decoder before durable storage so typed but malformed
  // caller objects cannot create an unrecoverable journal.
  parsePoststate(plan.poststate, inspection.action);
  return Object.freeze({
    ...scope,
    operation: 'rational-open-v1',
    operationDigest: hex(await sha256(new TextEncoder().encode(JSON.stringify({ intent, plan })))),
    intent: JSON.stringify(intent),
    plan: JSON.stringify(plan),
  });
}

/** Hostile-decode one saved Rational open without replaying or submitting it. */
export async function restoreRationalOpenJournalV1(
  journal: ClientOperationJournalV1,
): Promise<RestoredRationalOpenJournalV1> {
  if (journal.operation !== 'rational-open-v1') throw new Error('journal is not a Rational open operation');
  const rawIntent = parseObject(journal.intent, [
    'action', 'actor', 'capabilityDigest', 'childDigest', 'descriptorId', 'familyDigest', 'format',
    'lookupTable', 'market', 'observedSlot', 'outerDigest', 'owner', 'rawQuantity', 'rootDigest', 'selectedOutcome',
  ], 'Rational open journal intent');
  const openAction = action(rawIntent.action);
  const selected = selectedAction(openAction);
  const selectedOutcome = rawIntent.selectedOutcome === null ? null : outcome(rawIntent.selectedOutcome, 'journal selected outcome');
  if (rawIntent.format !== RATIONAL_OPEN_JOURNAL_INTENT_FORMAT_V1 || rawIntent.market !== journal.market
      || rawIntent.owner !== journal.owner || (selected ? selectedOutcome === null : selectedOutcome !== null)) {
    throw new Error('Rational open journal intent substituted its scope or action shape');
  }
  const intent: RationalOpenJournalIntentV1 = Object.freeze({
    format: RATIONAL_OPEN_JOURNAL_INTENT_FORMAT_V1,
    action: openAction,
    market: address(rawIntent.market, 'journal Market'),
    owner: address(rawIntent.owner, 'journal owner'),
    actor: address(rawIntent.actor, 'journal actor'),
    observedSlot: u64Text(rawIntent.observedSlot, 'journal observed slot'),
    selectedOutcome,
    rawQuantity: u64Text(rawIntent.rawQuantity, 'journal raw quantity', true),
    descriptorId: digest(rawIntent.descriptorId, 'journal descriptor'),
    capabilityDigest: digest(rawIntent.capabilityDigest, 'journal capability'),
    rootDigest: digest(rawIntent.rootDigest, 'journal root'),
    familyDigest: digest(rawIntent.familyDigest, 'journal family'),
    childDigest: digest(rawIntent.childDigest, 'journal Claims child'),
    outerDigest: digest(rawIntent.outerDigest, 'journal outer'),
    lookupTable: address(rawIntent.lookupTable, 'journal lookup table'),
  });
  if (intent.actor !== intent.owner) throw new Error('Rational open journal is not one exact single-wallet operation');
  const rawPlan = parseObject(journal.plan,
    ['format', 'lastValidBlockHeight', 'poststate', 'unsignedWireBase64', 'unsignedWireSha256'], 'Rational open journal plan');
  if (rawPlan.format !== RATIONAL_OPEN_JOURNAL_PLAN_FORMAT_V1) throw new Error('Rational open journal plan has another format');
  const poststate = parsePoststate(rawPlan.poststate, openAction);
  if (poststate.context.market !== intent.market || poststate.context.actor !== intent.actor
      || hex(poststate.context.descriptorId) !== intent.descriptorId) {
    throw new Error('Rational open journal poststate substituted its Market, actor, or descriptor');
  }
  const wireBytes = base64Bytes(rawPlan.unsignedWireBase64);
  if (hex(await sha256(wireBytes)) !== digest(rawPlan.unsignedWireSha256, 'journal unsigned packet')) {
    throw new Error('Rational open journal unsigned packet digest differs');
  }
  let transaction: VersionedTransaction;
  try { transaction = VersionedTransaction.deserialize(wireBytes); } catch { throw new Error('Rational open saved packet is not one Solana transaction'); }
  const instruction = transaction.message.compiledInstructions[0];
  if (!same(transaction.serialize(), wireBytes) || transaction.signatures.length !== 1
      || transaction.signatures[0]?.some((byte) => byte !== 0)
      || transaction.message.header.numRequiredSignatures !== 1
      || transaction.message.staticAccountKeys[0]?.toBase58() !== intent.owner
      || transaction.message.compiledInstructions.length !== 1 || instruction === undefined
      || hex(await sha256(instruction.data)) !== intent.outerDigest
      || transaction.message.addressTableLookups.length !== 1
      || transaction.message.addressTableLookups[0]?.accountKey.toBase58() !== intent.lookupTable) {
    throw new Error('Rational open saved packet substituted its message, signer, outer, or lookup table');
  }
  if (journal.phase === 'submitted') {
    const signed = VersionedTransaction.deserialize(submittedClientOperationWireV1(journal));
    if (!same(signed.message.serialize(), transaction.message.serialize())) throw new Error('Rational open submitted packet substituted the exact unsigned message');
  }
  const operationDigest = hex(await sha256(new TextEncoder().encode(JSON.stringify({
    intent: JSON.parse(journal.intent), plan: JSON.parse(journal.plan),
  }))));
  if (operationDigest !== journal.operationDigest) throw new Error('Rational open journal operation digest changed');
  return Object.freeze({
    intent,
    transaction,
    wireBytes,
    lastValidBlockHeight: u64Text(rawPlan.lastValidBlockHeight, 'last valid block height'),
    poststate,
  });
}
