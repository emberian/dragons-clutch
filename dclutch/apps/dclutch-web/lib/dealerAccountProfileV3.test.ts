import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { validateDealerAccountProfileV3 } from './dealerAccountProfileV3';
import type { DirectHotAccountMetaV3 } from './directInlineV3';
import {
  CUSTODY_IDENTITY_BASE_V3,
  CUSTODY_IDENTITY_STRIDE_V3,
  CUSTODY_SCALAR_BASE_V3,
  CUSTODY_SCALAR_STRIDE_V3,
  DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3,
  DEALER_EQUITY_LOCAL_ACCOUNT_COUNT_V3,
  DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3,
  ACCOUNT_PROFILE_HEADER_BYTES_V2,
  ACCOUNT_PROFILE_MAGIC_V2,
  ACCOUNT_PROFILE_OPERATION_BYTES_V2,
  ACCOUNT_PROFILE_RULE_BYTES_V2,
  DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3,
  TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE,
  TRUSTED_ENVIRONMENT_KIND_OFFSET,
  TRUSTED_ENVIRONMENT_SCALAR_OFFSET,
  ACCOUNT_PROFILE_VERSION_V2,
} from './generated/dealerEquityV3';

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putRule(bytes: Uint8Array, coordinate: number, privileges: number, effect: number, alias: number | null): void {
  const offset = ACCOUNT_PROFILE_HEADER_BYTES_V2 + coordinate * ACCOUNT_PROFILE_RULE_BYTES_V2;
  bytes[offset] = privileges;
  bytes[offset + 1] = effect;
  bytes[offset + 2] = alias === null ? 0 : 1;
  putU16(bytes, offset + 4, alias ?? 0);
}

function putOperation(bytes: Uint8Array, offset: number, opcode: number, account: number, register: number): void {
  bytes[offset] = opcode;
  putU16(bytes, offset + 2, account);
  putU16(bytes, offset + 6, register);
}

function canonicalContributeP0(): Readonly<{
  profile: Uint8Array;
  accounts: DirectHotAccountMetaV3[];
  data: Uint8Array[];
}> {
  const custody = 2;
  const claimsStart = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3;
  const laterStart = claimsStart + DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3;
  const localStart = DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + custody * DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3
    + DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3;
  const fixed = localStart + DEALER_EQUITY_LOCAL_ACCOUNT_COUNT_V3;
  const profile = new Uint8Array(ACCOUNT_PROFILE_HEADER_BYTES_V2 + fixed * ACCOUNT_PROFILE_RULE_BYTES_V2 + 3 * ACCOUNT_PROFILE_OPERATION_BYTES_V2);
  profile.set(ACCOUNT_PROFILE_MAGIC_V2);
  putU16(profile, 8, ACCOUNT_PROFILE_VERSION_V2);
  putU16(profile, 10, TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE);
  putU16(profile, 12, fixed);
  putU16(profile, 16, 3);
  putU16(profile, 20, CUSTODY_SCALAR_BASE_V3 + custody * CUSTODY_SCALAR_STRIDE_V3 + 2);
  putU16(profile, 24, CUSTODY_IDENTITY_BASE_V3 + custody * CUSTODY_IDENTITY_STRIDE_V3 + 1);
  putU16(profile, TRUSTED_ENVIRONMENT_SCALAR_OFFSET, CUSTODY_SCALAR_BASE_V3 + custody * CUSTODY_SCALAR_STRIDE_V3);
  profile[TRUSTED_ENVIRONMENT_KIND_OFFSET] = 1;

  const aliases = new Map<number, number>();
  for (const offset of [1, 2, 3, 4, 5, 6, 7, 9, 12, 13]) aliases.set(laterStart + offset, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + offset);
  for (const [claimsOffset, representative] of [
    [2, 4], [4, 2], [8, 3], [11, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + 1],
    [13, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + 3], [14, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + 4],
    [15, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + 5],
  ]) aliases.set(claimsStart + claimsOffset, representative);

  for (let coordinate = 0; coordinate < fixed; coordinate += 1) {
    const firstCustody = coordinate >= DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 && coordinate < claimsStart;
    const laterCustody = coordinate >= laterStart && coordinate < laterStart + DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3;
    const custodyOffset = firstCustody ? coordinate - DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3
      : laterCustody ? coordinate - laterStart : null;
    const claimsOffset = coordinate >= claimsStart && coordinate < laterStart ? coordinate - claimsStart : null;
    const writable = coordinate === 0 || coordinate >= localStart
      || (custodyOffset !== null && [8, 10, 11].includes(custodyOffset)) || claimsOffset === 1;
    const executable = (custodyOffset !== null && [3, 4, 13].includes(custodyOffset))
      || (claimsOffset !== null && [13, 14, 16, 18].includes(claimsOffset));
    putRule(profile, coordinate, (writable ? 2 : 0) | (executable ? 4 : 0), coordinate >= localStart ? 4 : 0, aliases.get(coordinate) ?? null);
  }
  const operations = ACCOUNT_PROFILE_HEADER_BYTES_V2 + fixed * ACCOUNT_PROFILE_RULE_BYTES_V2;
  const tradingIdentity = CUSTODY_IDENTITY_BASE_V3 + 4;
  putOperation(profile, operations, 2, DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + 4, tradingIdentity);
  putOperation(profile, operations + ACCOUNT_PROFILE_OPERATION_BYTES_V2, 1, localStart, tradingIdentity);
  putOperation(profile, operations + 2 * ACCOUNT_PROFILE_OPERATION_BYTES_V2, 1, localStart + 1, tradingIdentity);

  const accounts = Array.from({ length: fixed }, (_, coordinate): DirectHotAccountMetaV3 => {
    const representative = aliases.get(coordinate) ?? coordinate;
    const rule = ACCOUNT_PROFILE_HEADER_BYTES_V2 + coordinate * ACCOUNT_PROFILE_RULE_BYTES_V2;
    return Object.freeze({
      address: new PublicKey(new Uint8Array(32).fill(representative + 1)).toBase58(),
      isSigner: false,
      isWritable: (profile[rule] & 2) !== 0,
      executable: (profile[rule] & 4) !== 0,
    });
  });
  return Object.freeze({ profile, accounts, data: Array.from({ length: fixed }, () => new Uint8Array()) });
}

describe('Dealer-specific AccountProfile validation', () => {
  it('accepts the exact Profile5 selector-1 frame and refuses semantic/profile substitutions', () => {
    const fixture = canonicalContributeP0();
    expect(() => validateDealerAccountProfileV3(fixture.profile, { kind: 'equity', selector: 1 }, 4, fixture.accounts, fixture.data)).not.toThrow();

    const wrongSelector = () => validateDealerAccountProfileV3(fixture.profile, { kind: 'equity', selector: 2 }, 4, fixture.accounts, fixture.data);
    expect(wrongSelector).toThrow(/Profile5 action\/P geometry/);

    const weaker = fixture.profile.slice();
    weaker[ACCOUNT_PROFILE_HEADER_BYTES_V2] = 0;
    expect(() => validateDealerAccountProfileV3(weaker, { kind: 'equity', selector: 1 }, 4, fixture.accounts, fixture.data)).toThrow(/noncanonical account profile artifact/);

    const privilege = fixture.accounts.slice();
    privilege[0] = Object.freeze({ ...privilege[0]!, isWritable: false });
    expect(() => validateDealerAccountProfileV3(fixture.profile, { kind: 'equity', selector: 1 }, 4, privilege, fixture.data)).toThrow(/profile privileges/);

    const alias = fixture.accounts.slice();
    alias[claimsAliasCoordinate()] = Object.freeze({ ...alias[claimsAliasCoordinate()]!, address: new PublicKey(new Uint8Array(32).fill(0xee)).toBase58() });
    expect(() => validateDealerAccountProfileV3(fixture.profile, { kind: 'equity', selector: 1 }, 4, alias, fixture.data)).toThrow(/logical alias/);
  });
});

function claimsAliasCoordinate(): number {
  return DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3 + DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3 + 2;
}
