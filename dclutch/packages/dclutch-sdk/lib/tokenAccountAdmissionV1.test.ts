import { describe, expect, it } from 'vitest';

import {
  TOKEN_ACCOUNT_BYTES_V1,
  TOKEN_ACCOUNT_IMMUTABLE_OWNER_BYTES_V1,
  TOKEN_ACCOUNT_IMMUTABLE_OWNER_SUFFIX_V1,
} from './generated/walletTerminalPayoutV3';
import { admitBaseOrImmutableOwnerTokenAccountV1 } from './tokenAccountAdmissionV1';

/**
 * The hostile set is the Rust's own, case for case.
 *
 * `crates/dclutch-token-svm/src/state.rs`'s `immutable_owner_tests` names six
 * other extension types at the same 170-byte width, a Mint's account-type
 * discriminant carrying `ImmutableOwner`'s bytes, a right-typed entry at the
 * wrong width, and a truncated base. This file is that list, so a TypeScript
 * reader that admits something the chain refuses fails here rather than at a
 * wallet.
 */

/** `ImmutableOwner` is 7; each of these would change what a transfer MEANS. */
const OTHER_EXTENSIONS_V1 = Object.freeze([
  1, // TransferFeeConfig
  5, // NonTransferable
  11, // CpiGuard
  14, // TransferHookAccount
  6, // ImmutableOwner's neighbour, and not it
  8, // MemoTransfer
]);

/** The account-type discriminant of a Mint, where an Account's is 2. */
const MINT_ACCOUNT_TYPE_V1 = 1;

function base(amount: number): Uint8Array {
  const bytes = new Uint8Array(TOKEN_ACCOUNT_BYTES_V1);
  bytes.fill(3, 0, 32);
  bytes.fill(4, 32, 64);
  bytes[64] = amount;
  bytes[108] = 1;
  return bytes;
}

/** One base account with an arbitrary extension suffix appended. */
function suffixed(accountType: number, extensionType: number, valueWidth: number): Uint8Array {
  const bytes = new Uint8Array(TOKEN_ACCOUNT_BYTES_V1 + 5 + valueWidth);
  bytes.set(base(7), 0);
  bytes[TOKEN_ACCOUNT_BYTES_V1] = accountType;
  bytes[TOKEN_ACCOUNT_BYTES_V1 + 1] = extensionType & 0xff;
  bytes[TOKEN_ACCOUNT_BYTES_V1 + 2] = (extensionType >> 8) & 0xff;
  bytes[TOKEN_ACCOUNT_BYTES_V1 + 3] = valueWidth & 0xff;
  bytes[TOKEN_ACCOUNT_BYTES_V1 + 4] = (valueWidth >> 8) & 0xff;
  return bytes;
}

describe('the base-or-ImmutableOwner token account admission', () => {
  it('publishes the exact five bytes the ATA program wrote on devnet', () => {
    // `DsQSGKPbmJcZ89xts1Jgs1P5fprmX64fomqGFsQM1kmU`, the destination cohort-14
    // paid 500,000,000 atoms into on 2026-09-03, was read at 170 bytes with the
    // suffix `0207000000` before and after the transfer
    // (docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md, "THE PAYOUT,
    // into a 170-byte associated token account"). These constants are scraped
    // from the Rust's named constants and never typed, so this is the join
    // between what the generator derived and what the chain actually holds.
    expect(TOKEN_ACCOUNT_IMMUTABLE_OWNER_BYTES_V1).toBe(170);
    expect(TOKEN_ACCOUNT_IMMUTABLE_OWNER_BYTES_V1 - TOKEN_ACCOUNT_BYTES_V1).toBe(5);
    expect(Array.from(TOKEN_ACCOUNT_IMMUTABLE_OWNER_SUFFIX_V1, (byte) => byte.toString(16).padStart(2, '0')).join(''))
      .toBe('0207000000');
  });

  it('admits the base account unchanged and the ATA account down to its base', () => {
    const plain = base(9);
    expect(admitBaseOrImmutableOwnerTokenAccountV1(plain, 'recipient')).toBe(plain);

    const ata = suffixed(2, 7, 0);
    expect(ata).toHaveLength(TOKEN_ACCOUNT_IMMUTABLE_OWNER_BYTES_V1);
    const admitted = admitBaseOrImmutableOwnerTokenAccountV1(ata, 'recipient');
    expect(admitted).toHaveLength(TOKEN_ACCOUNT_BYTES_V1);
    // Read out of the same bytes by hand, so the admission is not its own
    // witness: the base fields survive the suffix.
    expect(Array.from(admitted)).toEqual(Array.from(ata.subarray(0, TOKEN_ACCOUNT_BYTES_V1)));
    expect(admitted[64]).toBe(7);
    expect(admitted[108]).toBe(1);
  });

  it('refuses every other extension at the same width, and the Mint discriminant', () => {
    for (const extension of OTHER_EXTENSIONS_V1) {
      const bytes = suffixed(2, extension, 0);
      expect(bytes).toHaveLength(TOKEN_ACCOUNT_IMMUTABLE_OWNER_BYTES_V1);
      expect(() => admitBaseOrImmutableOwnerTokenAccountV1(bytes, 'recipient'), `extension ${extension} must not be admitted`)
        .toThrow('not the exact empty ImmutableOwner entry');
    }
    expect(() => admitBaseOrImmutableOwnerTokenAccountV1(suffixed(MINT_ACCOUNT_TYPE_V1, 7, 0), 'recipient'))
      .toThrow('not the exact empty ImmutableOwner entry');
    // The right type at a declared length the four bytes of storage cannot
    // hold: the Rust's TLV walk calls that truncated, and the suffix comparison
    // refuses the same bytes.
    const declaredWide = suffixed(2, 7, 0);
    declaredWide[TOKEN_ACCOUNT_BYTES_V1 + 3] = 1;
    expect(() => admitBaseOrImmutableOwnerTokenAccountV1(declaredWide, 'recipient'))
      .toThrow('not the exact empty ImmutableOwner entry');
  });

  it('refuses every width but the two, and names both in the refusal', () => {
    for (const width of [0, 1, 82, 164, 166, 169, 171, 175]) {
      expect(() => admitBaseOrImmutableOwnerTokenAccountV1(new Uint8Array(width), 'Hoard'))
        .toThrow(`Hoard is ${width} bytes, neither the 165-byte base token account nor the 170-byte ImmutableOwner account`);
    }
    // A second entry after `ImmutableOwner`, which is the shape the Rust
    // refuses with a trailing-entry check and this one refuses by width.
    expect(() => admitBaseOrImmutableOwnerTokenAccountV1(suffixed(2, 7, 0 + 1), 'Hoard')).toThrow('171 bytes');
    // One byte of the base cut away with the suffix intact.
    expect(() => admitBaseOrImmutableOwnerTokenAccountV1(suffixed(2, 7, 0).subarray(1), 'Hoard')).toThrow('169 bytes');
  });
});
