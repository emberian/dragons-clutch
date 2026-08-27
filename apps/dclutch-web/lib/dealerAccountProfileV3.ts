import { PublicKey } from '@solana/web3.js';

import type { DirectHotAccountMetaV3 } from './directInlineV3';
import {
  CUSTODY_IDENTITY_BASE_V3,
  CUSTODY_IDENTITY_STRIDE_V3,
  CUSTODY_SCALAR_BASE_V3,
  CUSTODY_SCALAR_STRIDE_V3,
  DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3,
  DEALER_EQUITY_CUSTODY_CALLEE_ACCOUNT_COUNT_V3,
  DEALER_EQUITY_LOCAL_ACCOUNT_COUNT_V3,
  DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3,
  DEALER_LP_CLOSE_ACCOUNT_COUNT_V3,
  DEALER_LP_IDENTITY_COUNT_V3,
  DEALER_LP_OPEN_ACCOUNT_COUNT_V3,
  DEALER_LP_SCALAR_COUNT_V3,
  DEALER_LP_STATE_ACCOUNT_V3,
  DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4,
  DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
  DEALER_SCENARIO_CURRENT_SLOT_SCALAR_V4,
  DEALER_SCENARIO_CURRENT_TRADING_IDENTITY_V4,
  DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4,
  DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4,
  DEALER_SCENARIO_MAX_POSITION_COUNT_SCALAR_V4,
  DEALER_SCENARIO_OBLIGATION_IDENTITY_V4,
  DEALER_SCENARIO_PROFILE_FIXED_RULES_V4,
  DEALER_SCENARIO_PROFILE_SPANS_V4,
  DEALER_SCENARIO_PROFILE_SPAN_RULES_V4,
  DEALER_SCENARIO_SCRATCH_PAGE_COUNT_SCALAR_V4,
  DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE,
  DYNAMIC_FIXED_SPAN_COUNT_OFFSET,
  DYNAMIC_FIXED_SPAN_ENTRY_BYTES,
  DYNAMIC_FIXED_SPAN_ENTRY_COUNT_SCALAR_OFFSET,
  DYNAMIC_FIXED_SPAN_ENTRY_INSERTION_OFFSET,
  DYNAMIC_FIXED_SPAN_ENTRY_MAX_OFFSET,
  DYNAMIC_FIXED_SPAN_ENTRY_MIN_OFFSET,
  DYNAMIC_FIXED_SPAN_ENTRY_RULE_START_OFFSET,
  DYNAMIC_FIXED_SPAN_ENTRY_RULE_STRIDE_OFFSET,
  DYNAMIC_FIXED_SPAN_ENTRY_STEP_OFFSET,
  DYNAMIC_FIXED_SPAN_HEADER_BYTES,
  DYNAMIC_FIXED_SPAN_RESERVED_OFFSET,
  BASIS_WIDTH_OFFSET_V3,
  ACCOUNT_PROFILE_HEADER_BYTES_V2,
  LIFECYCLE_PRESTATE_ARTIFACT_PROFILE,
  ACCOUNT_PROFILE_MAGIC_V2,
  ACCOUNT_PROFILE_OPERATION_BYTES_V2,
  ACCOUNT_PROFILE_RULE_BYTES_V2,
  DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
  TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE,
  TRUSTED_ENVIRONMENT_KIND_OFFSET,
  TRUSTED_ENVIRONMENT_RESERVED_OFFSET,
  TRUSTED_ENVIRONMENT_SCALAR_OFFSET,
  TRUSTED_EXECUTING_PROGRAM_IDENTITY_OFFSET,
  TRUSTED_EXECUTING_PROGRAM_KIND_OFFSET,
  TRUSTED_EXECUTING_PROGRAM_RESERVED_OFFSET,
  ACCOUNT_PROFILE_VERSION_V2,
} from './generated/dealerEquityV3';

export type DealerAccountProfileRouteV3 =
  | Readonly<{ kind: 'equity'; selector: 1 | 2 | 3 | 4 | 5 | 6 }>
  | Readonly<{ kind: 'lp-open' }>
  | Readonly<{ kind: 'lp-close' }>
  | Readonly<{
    kind: 'scenario';
    spanCounts: readonly [number, number, number, number, number, number, number, number, number];
  }>;

type Rule = Readonly<{
  privileges: number;
  effectPermissions: number;
  aliasKind: number;
  prestate: number;
  aliasIndex: number;
  dataLength: number;
  dataItemStride: number;
}>;

type Span = Readonly<{
  insertion: number;
  countScalar: number;
  ruleStart: number;
  ruleStride: number;
  minimum: number;
  maximum: number;
  step: number;
}>;

type Profile = Readonly<{
  artifact: number;
  fixed: number;
  itemStride: number;
  fixedOperations: number;
  itemOperations: number;
  commonScalars: number;
  itemScalarStride: number;
  commonIdentities: number;
  itemIdentityStride: number;
  headerBytes: number;
  rules: readonly Rule[];
  spans: readonly Span[];
}>;

/// Sole writable Trading obligation, and the Custody callee appended after it.
/// The obligation is the last WRITE TARGET, not the last fixed coordinate.
const DEALER_SCENARIO_OBLIGATION_ACCOUNT_V4 =
  DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3;
const DEALER_SCENARIO_CUSTODY_PROGRAM_ACCOUNT_V4 = DEALER_SCENARIO_OBLIGATION_ACCOUNT_V4 + 1;

const EXPECTED_SCENARIO_SPANS = Object.freeze([
  [5, 7, 0, 14, 0, 14, 14],
  [5, 8, 14, 14, 0, 14, 14],
  [5, 9, 28, 14, 0, 14, 14],
  [5, 10, 42, 14, 0, 14, 14],
  [25, 0, 56, 1, 1, 2, 1],
  [25, 11, 57, 14, 0, 14, 14],
  [25, 12, 71, 14, 0, 14, 14],
  // Both trailing spans insert after the Custody callee coordinate at 26, not
  // after the obligation at 25.
  [27, 99, 85, 1, 0, 3, 1],
  [27, DEALER_SCENARIO_SCRATCH_PAGE_COUNT_SCALAR_V4, 86, 1, 6, 6, 1],
] as const);

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function u16(bytes: Uint8Array, offset: number): number {
  if (offset < 0 || offset + 2 > bytes.length) throw new Error('Dealer AccountProfile u16 is truncated');
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 2).getUint16(0, true);
}

function u32(bytes: Uint8Array, offset: number): number {
  if (offset < 0 || offset + 4 > bytes.length) throw new Error('Dealer AccountProfile u32 is truncated');
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
}

function checkedAdd(left: number, right: number, field: string): number {
  const value = left + right;
  if (!Number.isSafeInteger(value) || value < 0) throw new Error(`${field} overflows the browser integer domain`);
  return value;
}

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU32(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 4).setUint32(0, value, true);
}

function putRule(bytes: Uint8Array, offset: number, rule: Rule): void {
  bytes[offset] = rule.privileges;
  bytes[offset + 1] = rule.effectPermissions;
  bytes[offset + 2] = rule.aliasKind;
  bytes[offset + 3] = rule.prestate;
  putU16(bytes, offset + 4, rule.aliasIndex);
  putU32(bytes, offset + 8, rule.dataLength);
  putU32(bytes, offset + 12, rule.dataItemStride);
}

function putOperation(bytes: Uint8Array, offset: number, opcode: number, account: number, register: number, dataOffset = 0): void {
  bytes[offset] = opcode;
  putU16(bytes, offset + 2, account);
  putU16(bytes, offset + 6, register);
  putU32(bytes, offset + 8, dataOffset);
}

function rule(privileges: number, effectPermissions: number, dataLength: number, prestate = 0, aliasIndex: number | null = null, dataItemStride = 0): Rule {
  return Object.freeze({
    privileges,
    effectPermissions,
    aliasKind: aliasIndex === null ? 0 : 1,
    prestate,
    aliasIndex: aliasIndex ?? 0,
    dataLength,
    dataItemStride,
  });
}

function profileHeader(
  artifact: number,
  fixed: number,
  itemStride: number,
  fixedOperations: number,
  commonScalars: number,
  itemScalarStride: number,
  commonIdentities: number,
  itemIdentityStride: number,
  bytes: number,
): Uint8Array {
  const output = new Uint8Array(bytes);
  output.set(ACCOUNT_PROFILE_MAGIC_V2, 0);
  putU16(output, 8, ACCOUNT_PROFILE_VERSION_V2);
  putU16(output, 10, artifact);
  putU16(output, 12, fixed);
  putU16(output, 14, itemStride);
  putU16(output, 16, fixedOperations);
  putU16(output, 20, commonScalars);
  putU16(output, 22, itemScalarStride);
  putU16(output, 24, commonIdentities);
  putU16(output, 26, itemIdentityStride);
  return output;
}

function expectedEquityProfile(route: Extract<DealerAccountProfileRouteV3, { kind: 'equity' }>, lengths: readonly number[]): Uint8Array {
  const shape = equityShape(route.selector);
  const claimsStart = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3;
  const claimsCount = DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 + shape.positions;
  const laterStart = claimsStart + claimsCount;
  const localStart = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + shape.custody * DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3 + claimsCount;
  // The release-selected Custody program the Custody routes are invoked
  // through, appended past every route range. `CustodyFrameRoleV1` has no
  // `CustodyProgram` variant, so no Custody frame can carry its own callee and
  // the topology has to declare one coordinate for it.
  const custodyProgram = localStart + DEALER_EQUITY_LOCAL_ACCOUNT_COUNT_V3;
  const fixed = custodyProgram + DEALER_EQUITY_CUSTODY_CALLEE_ACCOUNT_COUNT_V3;
  if (lengths.length !== fixed) throw new Error('Dealer equity logical data-length vector has the wrong action/P width');
  const bytes = ACCOUNT_PROFILE_HEADER_BYTES_V2 + fixed * ACCOUNT_PROFILE_RULE_BYTES_V2 + 3 * ACCOUNT_PROFILE_OPERATION_BYTES_V2;
  const output = profileHeader(TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE, fixed, 0, 3, shape.scalars, 0, shape.identities, 0, bytes);
  putU16(output, TRUSTED_ENVIRONMENT_SCALAR_OFFSET, CUSTODY_SCALAR_BASE_V3 + shape.custody * CUSTODY_SCALAR_STRIDE_V3);
  output[TRUSTED_ENVIRONMENT_KIND_OFFSET] = 1;
  const custodyOffset = (coordinate: number): number | null => {
    if (coordinate >= DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 && coordinate < claimsStart) return coordinate - DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3;
    const laterEnd = laterStart + (shape.custody - 1) * DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3;
    return coordinate >= laterStart && coordinate < laterEnd ? (coordinate - laterStart) % DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3 : null;
  };
  for (let coordinate = 0; coordinate < fixed; coordinate += 1) {
    const offset = custodyOffset(coordinate);
    const claimsOffset = coordinate >= claimsStart && coordinate < claimsStart + claimsCount ? coordinate - claimsStart : null;
    const writable = coordinate === 0 || coordinate === localStart || coordinate === localStart + 1
      || (offset !== null && [8, 10, 11].includes(offset))
      || claimsOffset === 1 || (claimsOffset !== null && claimsOffset >= DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3);
    const executable = coordinate === custodyProgram
      || (offset !== null && [3, 4, 13].includes(offset))
      || (claimsOffset !== null && [13, 14, 16, 18].includes(claimsOffset));
    let alias: number | null = null;
    if (offset !== null && coordinate >= laterStart && [1, 2, 3, 4, 5, 6, 7, 9, 12, 13].includes(offset)) {
      alias = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + offset;
    } else if (claimsOffset !== null) {
      alias = new Map<number, number>([
        [2, 4], [4, 2], [8, 3], [11, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + 1],
        [13, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + 3], [14, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + 4],
        [15, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + 5],
      ]).get(claimsOffset) ?? null;
    }
    putRule(output, ACCOUNT_PROFILE_HEADER_BYTES_V2 + coordinate * ACCOUNT_PROFILE_RULE_BYTES_V2, rule(
      (writable ? 2 : 0) | (executable ? 4 : 0),
      coordinate === localStart || coordinate === localStart + 1 ? 4 : 0,
      lengths[coordinate] ?? -1,
      0,
      alias,
    ));
  }
  const operations = ACCOUNT_PROFILE_HEADER_BYTES_V2 + fixed * ACCOUNT_PROFILE_RULE_BYTES_V2;
  const tradingIdentity = CUSTODY_IDENTITY_BASE_V3 + 4;
  putOperation(output, operations, 2, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + 4, tradingIdentity);
  putOperation(output, operations + ACCOUNT_PROFILE_OPERATION_BYTES_V2, 1, localStart, tradingIdentity);
  putOperation(output, operations + 2 * ACCOUNT_PROFILE_OPERATION_BYTES_V2, 1, localStart + 1, tradingIdentity);
  return output;
}

function expectedLpProfile(route: Extract<DealerAccountProfileRouteV3, { kind: 'lp-open' | 'lp-close' }>, lengths: readonly number[]): Uint8Array {
  const open = route.kind === 'lp-open';
  const fixed = open ? DEALER_LP_OPEN_ACCOUNT_COUNT_V3 : DEALER_LP_CLOSE_ACCOUNT_COUNT_V3;
  const operationCount = open ? 14 : 13;
  if (lengths.length !== fixed || lengths[DEALER_LP_STATE_ACCOUNT_V3] !== 0 && lengths[DEALER_LP_STATE_ACCOUNT_V3] !== 256) {
    throw new Error('Dealer LP logical data-length vector has the wrong action/state width');
  }
  const output = profileHeader(
    LIFECYCLE_PRESTATE_ARTIFACT_PROFILE,
    fixed,
    0,
    operationCount,
    DEALER_LP_SCALAR_COUNT_V3,
    0,
    DEALER_LP_IDENTITY_COUNT_V3,
    0,
    ACCOUNT_PROFILE_HEADER_BYTES_V2 + fixed * ACCOUNT_PROFILE_RULE_BYTES_V2 + operationCount * ACCOUNT_PROFILE_OPERATION_BYTES_V2,
  );
  output[TRUSTED_ENVIRONMENT_KIND_OFFSET] = 1;
  const payer = open ? 7 : null;
  const credit = open ? 8 : 7;
  const system = open ? 9 : 8;
  for (let coordinate = 0; coordinate < fixed; coordinate += 1) {
    const state = coordinate === DEALER_LP_STATE_ACCOUNT_V3;
    const payerAccount = coordinate === payer;
    const writable = coordinate === 0 || state || payerAccount || coordinate === credit;
    const effect = (state || payerAccount ? 1 : 0) | (state || coordinate === credit ? 2 : 0) | (state ? 4 : 0);
    putRule(output, ACCOUNT_PROFILE_HEADER_BYTES_V2 + coordinate * ACCOUNT_PROFILE_RULE_BYTES_V2, rule(
      (payerAccount ? 1 : 0) | (writable ? 2 : 0) | (coordinate === system ? 4 : 0),
      effect,
      state ? 256 : (lengths[coordinate] ?? -1),
      state ? 1 : 0,
    ));
  }
  let cursor = ACCOUNT_PROFILE_HEADER_BYTES_V2 + fixed * ACCOUNT_PROFILE_RULE_BYTES_V2;
  const operation = (opcode: number, account: number, register: number, dataOffset = 0) => {
    putOperation(output, cursor, opcode, account, register, dataOffset);
    cursor += ACCOUNT_PROFILE_OPERATION_BYTES_V2;
  };
  operation(2, 5, 9);
  operation(5, 5, 6, 16);
  operation(2, 6, 10);
  operation(5, 6, 7, 16);
  operation(5, 6, 8, 216);
  operation(5, 6, 9, 232);
  operation(16, 6, 15, 240);
  for (const [dataOffset, destination] of [[24, 11], [56, 12], [88, 13], [120, 14], [152, 15], [184, 16]]) {
    operation(6, 6, destination ?? 0, dataOffset);
  }
  if (payer !== null) operation(1, payer, 17);
  return output;
}

function expectedScenarioProfile(lengths: readonly number[]): Uint8Array {
  if (lengths.length < DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3) throw new Error('Dealer scenario common data-length vector is truncated');
  const header = DYNAMIC_FIXED_SPAN_HEADER_BYTES + DEALER_SCENARIO_PROFILE_SPANS_V4 * DYNAMIC_FIXED_SPAN_ENTRY_BYTES;
  const output = profileHeader(
    DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE,
    DEALER_SCENARIO_PROFILE_FIXED_RULES_V4,
    DEALER_SCENARIO_PROFILE_SPAN_RULES_V4,
    3,
    DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
    DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4,
    DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4,
    DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4,
    header + (DEALER_SCENARIO_PROFILE_FIXED_RULES_V4 + DEALER_SCENARIO_PROFILE_SPAN_RULES_V4) * ACCOUNT_PROFILE_RULE_BYTES_V2 + 3 * ACCOUNT_PROFILE_OPERATION_BYTES_V2,
  );
  putU16(output, TRUSTED_ENVIRONMENT_SCALAR_OFFSET, DEALER_SCENARIO_CURRENT_SLOT_SCALAR_V4);
  output[TRUSTED_ENVIRONMENT_KIND_OFFSET] = 1;
  putU16(output, TRUSTED_EXECUTING_PROGRAM_IDENTITY_OFFSET, DEALER_SCENARIO_CURRENT_TRADING_IDENTITY_V4);
  output[TRUSTED_EXECUTING_PROGRAM_KIND_OFFSET] = 1;
  putU16(output, DYNAMIC_FIXED_SPAN_COUNT_OFFSET, DEALER_SCENARIO_PROFILE_SPANS_V4);
  EXPECTED_SCENARIO_SPANS.forEach((span, index) => {
    const offset = DYNAMIC_FIXED_SPAN_HEADER_BYTES + index * DYNAMIC_FIXED_SPAN_ENTRY_BYTES;
    [span[0], span[1], span[2], span[3]].forEach((value, field) => putU16(output, offset + field * 2, value));
    [span[4], span[5], span[6]].forEach((value, field) => putU32(output, offset + 8 + field * 4, value));
  });
  for (let coordinate = 0; coordinate < DEALER_SCENARIO_PROFILE_FIXED_RULES_V4; coordinate += 1) {
    let value = rule(coordinate === 0 ? 2 : 0, 0, coordinate < 5 ? (lengths[coordinate] ?? -1) : 0);
    const claims = coordinate - DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3;
    if (claims >= 0 && claims < DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3) {
      const privileges = claims === 0 ? 1 : claims === 1 ? 2 : [13, 14, 16, 18].includes(claims) ? 4 : 0;
      const alias = new Map<number, number>([[2, 4], [4, 2], [8, 3]]).get(claims) ?? null;
      value = rule(privileges, 0, 0, alias === null ? 5 : 4, alias);
    } else if (coordinate === DEALER_SCENARIO_OBLIGATION_ACCOUNT_V4) {
      value = rule(2, 4, 192, 0, null, 8);
    } else if (coordinate === DEALER_SCENARIO_CUSTODY_PROGRAM_ACCOUNT_V4) {
      // The release-selected Custody program the six Custody routes are invoked
      // through: readonly executable, no effect permission, no asserted width,
      // opaque prestate.
      value = rule(4, 0, 0, 5);
    }
    putRule(output, header + coordinate * ACCOUNT_PROFILE_RULE_BYTES_V2, value);
  }
  for (let coordinate = 0; coordinate < DEALER_SCENARIO_PROFILE_SPAN_RULES_V4; coordinate += 1) {
    let privileges = 0;
    for (const start of [0, 14, 28, 42, 57, 71]) {
      const local = coordinate - start;
      if (local >= 0 && local < 14) privileges = [8, 10, 11].includes(local) ? 2 : [3, 4, 13].includes(local) ? 4 : 0;
    }
    if (coordinate === 56) privileges = 2;
    putRule(output, header + (DEALER_SCENARIO_PROFILE_FIXED_RULES_V4 + coordinate) * ACCOUNT_PROFILE_RULE_BYTES_V2, rule(privileges, 0, 0, 5));
  }
  const operations = header + (DEALER_SCENARIO_PROFILE_FIXED_RULES_V4 + DEALER_SCENARIO_PROFILE_SPAN_RULES_V4) * ACCOUNT_PROFILE_RULE_BYTES_V2;
  putOperation(output, operations, 8, 2, DEALER_SCENARIO_MAX_POSITION_COUNT_SCALAR_V4, BASIS_WIDTH_OFFSET_V3);
  putOperation(output, operations + ACCOUNT_PROFILE_OPERATION_BYTES_V2, 1, 25, DEALER_SCENARIO_CURRENT_TRADING_IDENTITY_V4);
  putOperation(output, operations + 2 * ACCOUNT_PROFILE_OPERATION_BYTES_V2, 0, 25, DEALER_SCENARIO_OBLIGATION_IDENTITY_V4);
  return output;
}

function decodeRule(bytes: Uint8Array, offset: number, artifact: number, item: boolean, index: number, fixed: number): Rule {
  const privileges = bytes[offset] ?? 0xff;
  const effectPermissions = bytes[offset + 1] ?? 0xff;
  const aliasKind = bytes[offset + 2] ?? 0xff;
  const prestate = bytes[offset + 3] ?? 0xff;
  const aliasIndex = u16(bytes, offset + 4);
  if ((privileges & ~7) !== 0 || (effectPermissions & ~7) !== 0 || u16(bytes, offset + 6) !== 0) {
    throw new Error('Dealer AccountProfile rule has noncanonical permissions or reserved bytes');
  }
  const prestates = artifact === TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE
    ? [0]
    : artifact === LIFECYCLE_PRESTATE_ARTIFACT_PROFILE ? [0, 1] : [0, 1, 2, 4, 5];
  if (!prestates.includes(prestate)) throw new Error('Dealer AccountProfile rule has an unsupported prestate');
  if ((aliasKind === 0 && aliasIndex !== 0)
      || (aliasKind === 1 && (aliasIndex >= (item ? fixed : index)))
      || (aliasKind === 2 && (!item || aliasIndex >= index))
      || aliasKind > 2) {
    throw new Error('Dealer AccountProfile rule has a noncanonical alias');
  }
  if ((prestate === 1 && (aliasKind !== 0 || aliasIndex !== 0 || privileges !== 2))
      || (prestate === 4 && (aliasKind !== 1 || aliasIndex >= fixed || effectPermissions !== 0))
      || (prestate === 5 && (aliasKind !== 0 || aliasIndex !== 0 || effectPermissions !== 0))) {
    throw new Error('Dealer AccountProfile prestate does not match its alias/privilege shape');
  }
  return Object.freeze({
    privileges,
    effectPermissions,
    aliasKind,
    prestate,
    aliasIndex,
    dataLength: u32(bytes, offset + 8),
    dataItemStride: u32(bytes, offset + 12),
  });
}

function decodeProfile(bytes: Uint8Array): Profile {
  if (bytes.length < ACCOUNT_PROFILE_HEADER_BYTES_V2 || !same(bytes.slice(0, 8), ACCOUNT_PROFILE_MAGIC_V2) || u16(bytes, 8) !== ACCOUNT_PROFILE_VERSION_V2) {
    throw new Error('Dealer AccountProfile has the wrong V2 magic or version');
  }
  const artifact = u16(bytes, 10);
  if (!([TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE, LIFECYCLE_PRESTATE_ARTIFACT_PROFILE, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE] as number[]).includes(artifact)) {
    throw new Error('Dealer route selected a non-Dealer AccountProfile artifact');
  }
  const fixed = u16(bytes, 12);
  const itemStride = u16(bytes, 14);
  const fixedOperations = u16(bytes, 16);
  const itemOperations = u16(bytes, 18);
  const commonScalars = u16(bytes, 20);
  const itemScalarStride = u16(bytes, 22);
  const commonIdentities = u16(bytes, 24);
  const itemIdentityStride = u16(bytes, 26);
  if (fixed === 0 || (commonScalars === 0 && itemScalarStride === 0 && commonIdentities === 0 && itemIdentityStride === 0)
      || bytes[TRUSTED_ENVIRONMENT_KIND_OFFSET] !== 1
      || bytes[TRUSTED_ENVIRONMENT_RESERVED_OFFSET] !== 0
      || u16(bytes, TRUSTED_ENVIRONMENT_SCALAR_OFFSET) >= commonScalars) {
    throw new Error('Dealer AccountProfile has a noncanonical fixed/register/environment header');
  }
  let headerBytes: number = ACCOUNT_PROFILE_HEADER_BYTES_V2;
  const spans: Span[] = [];
  if (artifact === DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE) {
    if (bytes[TRUSTED_EXECUTING_PROGRAM_KIND_OFFSET] !== 1
        || bytes[TRUSTED_EXECUTING_PROGRAM_RESERVED_OFFSET] !== 0
        || u16(bytes, TRUSTED_EXECUTING_PROGRAM_IDENTITY_OFFSET) >= commonIdentities
        || bytes.slice(DYNAMIC_FIXED_SPAN_RESERVED_OFFSET, DYNAMIC_FIXED_SPAN_HEADER_BYTES).some((value) => value !== 0)) {
      throw new Error('Dealer Profile13 has a noncanonical trusted-program/span header');
    }
    const spanCount = u16(bytes, DYNAMIC_FIXED_SPAN_COUNT_OFFSET);
    headerBytes = checkedAdd(DYNAMIC_FIXED_SPAN_HEADER_BYTES, spanCount * DYNAMIC_FIXED_SPAN_ENTRY_BYTES, 'Dealer Profile13 header');
    for (let index = 0; index < spanCount; index += 1) {
      const offset = DYNAMIC_FIXED_SPAN_HEADER_BYTES + index * DYNAMIC_FIXED_SPAN_ENTRY_BYTES;
      const span = Object.freeze({
        insertion: u16(bytes, offset + DYNAMIC_FIXED_SPAN_ENTRY_INSERTION_OFFSET),
        countScalar: u16(bytes, offset + DYNAMIC_FIXED_SPAN_ENTRY_COUNT_SCALAR_OFFSET),
        ruleStart: u16(bytes, offset + DYNAMIC_FIXED_SPAN_ENTRY_RULE_START_OFFSET),
        ruleStride: u16(bytes, offset + DYNAMIC_FIXED_SPAN_ENTRY_RULE_STRIDE_OFFSET),
        minimum: u32(bytes, offset + DYNAMIC_FIXED_SPAN_ENTRY_MIN_OFFSET),
        maximum: u32(bytes, offset + DYNAMIC_FIXED_SPAN_ENTRY_MAX_OFFSET),
        step: u32(bytes, offset + DYNAMIC_FIXED_SPAN_ENTRY_STEP_OFFSET),
      });
      if (span.insertion > fixed || span.countScalar >= commonScalars || span.ruleStride === 0
          || span.ruleStart + span.ruleStride > itemStride || span.minimum > span.maximum || span.step === 0) {
        throw new Error('Dealer Profile13 span is outside its declared account/register geometry');
      }
      spans.push(span);
    }
  }
  const ruleCount = checkedAdd(fixed, itemStride, 'Dealer AccountProfile rule count');
  const operationCount = checkedAdd(fixedOperations, itemOperations, 'Dealer AccountProfile operation count');
  const expected = checkedAdd(headerBytes, checkedAdd(ruleCount * ACCOUNT_PROFILE_RULE_BYTES_V2, operationCount * ACCOUNT_PROFILE_OPERATION_BYTES_V2, 'Dealer AccountProfile body'), 'Dealer AccountProfile width');
  if (bytes.length !== expected) throw new Error('Dealer AccountProfile has a noncanonical exact byte width');
  const rules: Rule[] = [];
  for (let index = 0; index < ruleCount; index += 1) {
    rules.push(decodeRule(bytes, headerBytes + index * ACCOUNT_PROFILE_RULE_BYTES_V2, artifact, index >= fixed, index >= fixed ? index - fixed : index, fixed));
  }
  return Object.freeze({ artifact, fixed, itemStride, fixedOperations, itemOperations, commonScalars, itemScalarStride, commonIdentities, itemIdentityStride, headerBytes, rules, spans });
}

function equityShape(selector: number): Readonly<{ positions: number; custody: number; scalars: number; identities: number }> {
  if (!Number.isInteger(selector) || selector < 1 || selector > 6) throw new Error('Dealer equity selector is outside 1..6');
  const add = selector <= 3;
  const custody = add ? 2 : 3;
  return Object.freeze({
    positions: (selector - 1) % 3,
    custody,
    scalars: CUSTODY_SCALAR_BASE_V3 + custody * CUSTODY_SCALAR_STRIDE_V3 + 2,
    identities: CUSTODY_IDENTITY_BASE_V3 + custody * CUSTODY_IDENTITY_STRIDE_V3 + (add ? 1 : 0),
  });
}

function requireProfileShape(profile: Profile, route: DealerAccountProfileRouteV3): readonly number[] {
  if (route.kind === 'equity') {
    const shape = equityShape(route.selector);
    const fixed = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3
      + shape.custody * DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3
      + DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3 + shape.positions + DEALER_EQUITY_LOCAL_ACCOUNT_COUNT_V3
      + DEALER_EQUITY_CUSTODY_CALLEE_ACCOUNT_COUNT_V3;
    if (profile.artifact !== TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE || profile.fixed !== fixed || profile.itemStride !== 0
        || profile.fixedOperations !== 3 || profile.itemOperations !== 0 || profile.commonScalars !== shape.scalars
        || profile.itemScalarStride !== 0 || profile.commonIdentities !== shape.identities || profile.itemIdentityStride !== 0
        || profile.spans.length !== 0) {
      throw new Error('Dealer equity route did not select its exact Profile5 action/P geometry');
    }
    return [];
  }
  if (route.kind === 'lp-open' || route.kind === 'lp-close') {
    const open = route.kind === 'lp-open';
    if (profile.artifact !== LIFECYCLE_PRESTATE_ARTIFACT_PROFILE
        || profile.fixed !== (open ? DEALER_LP_OPEN_ACCOUNT_COUNT_V3 : DEALER_LP_CLOSE_ACCOUNT_COUNT_V3)
        || profile.itemStride !== 0 || profile.fixedOperations !== (open ? 14 : 13) || profile.itemOperations !== 0
        || profile.commonScalars !== DEALER_LP_SCALAR_COUNT_V3 || profile.itemScalarStride !== 0
        || profile.commonIdentities !== DEALER_LP_IDENTITY_COUNT_V3 || profile.itemIdentityStride !== 0
        || profile.rules[DEALER_LP_STATE_ACCOUNT_V3]?.prestate !== 1
        || profile.rules[DEALER_LP_STATE_ACCOUNT_V3]?.dataLength !== 256
        || profile.rules.some((rule, index) => index !== DEALER_LP_STATE_ACCOUNT_V3 && rule.prestate !== 0)) {
      throw new Error('Dealer LP route did not select its exact Profile6 Open/Close geometry');
    }
    return [];
  }
  if (profile.artifact !== DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
      || profile.fixed !== DEALER_SCENARIO_PROFILE_FIXED_RULES_V4
      || profile.itemStride !== DEALER_SCENARIO_PROFILE_SPAN_RULES_V4
      || profile.fixedOperations !== 3 || profile.itemOperations !== 0
      || profile.commonScalars !== DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4
      || profile.itemScalarStride !== DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4
      || profile.commonIdentities !== DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4
      || profile.itemIdentityStride !== DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4
      || profile.spans.length !== DEALER_SCENARIO_PROFILE_SPANS_V4) {
    throw new Error('Dealer scenario route did not select the exact Profile13 bank geometry');
  }
  profile.spans.forEach((span, index) => {
    const expected = EXPECTED_SCENARIO_SPANS[index];
    if (expected === undefined || [span.insertion, span.countScalar, span.ruleStart, span.ruleStride, span.minimum, span.maximum, span.step]
      .some((value, field) => value !== expected[field])) {
      throw new Error(`Dealer scenario Profile13 span ${index} differs from selector 9`);
    }
    const count = route.spanCounts[index];
    if (!Number.isInteger(count) || count < span.minimum || count > span.maximum || (count - span.minimum) % span.step !== 0) {
      throw new Error(`Dealer scenario span ${index} count is outside its protected range/congruence`);
    }
  });
  return route.spanCounts;
}

function dynamicRules(profile: Profile, spanCounts: readonly number[]): readonly Readonly<{ rule: Rule; representative: number }>[] {
  const output: Array<{ rule: Rule; representative: number }> = [];
  let baseCursor = 0;
  for (let index = 0; index < profile.spans.length; index += 1) {
    const span = profile.spans[index];
    if (span === undefined) throw new Error('Dealer Profile13 span is absent');
    while (baseCursor < span.insertion) {
      const rule = profile.rules[baseCursor];
      if (rule === undefined) throw new Error('Dealer Profile13 fixed rule is absent');
      output.push({ rule, representative: rule.aliasKind === 1 ? -1 - rule.aliasIndex : output.length });
      baseCursor += 1;
    }
    const count = spanCounts[index] ?? -1;
    for (let relative = 0; relative < count; relative += 1) {
      const rule = profile.rules[profile.fixed + span.ruleStart + (relative % span.ruleStride)];
      if (rule === undefined) throw new Error('Dealer Profile13 span rule is absent');
      const itemStart = output.length - (relative % span.ruleStride);
      const representative = rule.aliasKind === 0 ? output.length : rule.aliasKind === 1 ? -1 - rule.aliasIndex : itemStart + rule.aliasIndex;
      output.push({ rule, representative });
    }
  }
  while (baseCursor < profile.fixed) {
    const rule = profile.rules[baseCursor];
    if (rule === undefined) throw new Error('Dealer Profile13 fixed suffix rule is absent');
    output.push({ rule, representative: rule.aliasKind === 1 ? -1 - rule.aliasIndex : output.length });
    baseCursor += 1;
  }
  // Fixed aliases are encoded as negative base coordinates until every span
  // insertion is known. Resolve them to their expanded runtime coordinate.
  const baseRuntime = Array.from({ length: profile.fixed }, (_, base) => {
    let coordinate = base;
    profile.spans.forEach((span, index) => { if (span.insertion <= base) coordinate += spanCounts[index] ?? 0; });
    return coordinate;
  });
  return output.map(({ rule, representative }) => Object.freeze({
    rule,
    representative: representative < 0 ? (baseRuntime[-1 - representative] ?? -1) : representative,
  })).map((entry) => {
    if (entry.representative < 0 || entry.representative >= output.length) throw new Error('Dealer Profile13 alias representative is outside the expanded frame');
    return entry;
  });
}

function fixedRules(profile: Profile, tailCount: number): readonly Readonly<{ rule: Rule; representative: number }>[] {
  const output: Array<{ rule: Rule; representative: number }> = [];
  const count = profile.fixed + profile.itemStride * tailCount;
  for (let coordinate = 0; coordinate < count; coordinate += 1) {
    const fixed = coordinate < profile.fixed;
    const local = fixed ? coordinate : (coordinate - profile.fixed) % profile.itemStride;
    const itemStart = fixed ? 0 : coordinate - local;
    const rule = profile.rules[fixed ? local : profile.fixed + local];
    if (rule === undefined) throw new Error('Dealer AccountProfile rule expansion is truncated');
    const representative = rule.aliasKind === 0 ? coordinate : rule.aliasKind === 1 ? rule.aliasIndex : itemStart + rule.aliasIndex;
    output.push({ rule, representative });
  }
  return output;
}

function dataLengthMatches(rule: Rule, tailCount: number, actual: number): boolean {
  const exact = checkedAdd(rule.dataLength, rule.dataItemStride * tailCount, 'Dealer AccountProfile data width');
  if (rule.prestate === 0) return actual === exact;
  if (rule.prestate === 1) return actual === 0 || actual === exact;
  if (rule.prestate === 2) return actual >= exact && actual !== 0;
  return true;
}

/**
 * Validate the exact Dealer-specific Profile5, Profile6, or Profile13 physical
 * account frame. This deliberately does not share Direct's fixed+stride-N
 * validator: selector 9 owns protected dynamic spans, while LP Open/Close own
 * lifecycle vacancy/live alternatives.
 */
export function validateDealerAccountProfileV3(
  profileBytes: Uint8Array,
  route: DealerAccountProfileRouteV3,
  tailCount: number,
  accounts: ReadonlyArray<DirectHotAccountMetaV3>,
  accountData?: ReadonlyArray<Uint8Array>,
): void {
  if (!Number.isInteger(tailCount) || tailCount <= 0 || tailCount > 0xffff_ffff) {
    throw new Error('Dealer Product outcome count is outside runtime u32');
  }
  if (accountData === undefined || accountData.length !== accounts.length) {
    throw new Error('Dealer profile validation requires same-observation account data for every physical meta');
  }
  const profile = decodeProfile(profileBytes);
  const spanCounts = requireProfileShape(profile, route);
  const expected = route.kind === 'equity'
    ? expectedEquityProfile(route, accountData.map((data) => data.length))
    : route.kind === 'lp-open' || route.kind === 'lp-close'
      ? expectedLpProfile(route, accountData.map((data) => data.length))
      : expectedScenarioProfile(accountData.map((data) => data.length));
  if (!same(profileBytes, expected)) throw new Error(`Dealer ${route.kind} route selected a noncanonical account profile artifact`);
  const logical = profile.artifact === DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
    ? dynamicRules(profile, spanCounts)
    : fixedRules(profile, tailCount);
  const packed = profile.artifact === DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE;
  const physical = packed ? logical.filter((entry, coordinate) => entry.representative === coordinate) : logical;
  if (accounts.length !== physical.length || (accountData !== undefined && accountData.length !== accounts.length)) {
    throw new Error('Dealer physical runtime account width differs from its exact profile');
  }
  const seen = new Set<string>();
  physical.forEach((entry, ordinal) => {
    const coordinate = packed ? entry.representative : ordinal;
    let privileges = entry.rule.privileges | (packed && entry.rule.effectPermissions !== 0 ? 2 : 0);
    if (packed) logical.forEach((candidate) => {
      if (candidate.representative === coordinate) privileges |= candidate.rule.privileges;
    });
    const account = accounts[ordinal];
    if (account === undefined || account.isSigner !== ((privileges & 1) !== 0)
        || account.isWritable !== ((privileges & 2) !== 0) || account.executable !== ((privileges & 4) !== 0)) {
      throw new Error(`Dealer physical runtime account ${ordinal} differs from its unioned profile privileges`);
    }
    const canonical = new PublicKey(account.address).toBase58();
    if (canonical !== account.address) throw new Error('Dealer physical runtime account address is noncanonical');
    if (packed) {
      if (seen.has(canonical)) throw new Error('Dealer physical runtime account aliases another representative');
      seen.add(canonical);
    } else if (entry.representative !== ordinal) {
      const representative = accounts[entry.representative];
      const representativeData = accountData?.[entry.representative];
      if (representative === undefined || representative.address !== account.address
          || representative.isSigner !== account.isSigner || representative.isWritable !== account.isWritable
          || representative.executable !== account.executable
          || (accountData !== undefined && (representativeData === undefined || !same(representativeData, accountData[ordinal] ?? new Uint8Array())))) {
        throw new Error(`Dealer logical alias ${ordinal} differs from its canonical representative`);
      }
    }
    if (accountData !== undefined && !dataLengthMatches(entry.rule, tailCount, accountData[ordinal]?.length ?? -1)) {
      throw new Error(`Dealer physical runtime account ${ordinal} differs from its exact profile data geometry`);
    }
  });
}
