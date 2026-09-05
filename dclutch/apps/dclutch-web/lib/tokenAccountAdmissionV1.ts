import {
  TOKEN_ACCOUNT_BYTES_V1,
  TOKEN_ACCOUNT_IMMUTABLE_OWNER_BYTES_V1,
  TOKEN_ACCOUNT_IMMUTABLE_OWNER_SUFFIX_V1,
} from './generated/walletTerminalPayoutV3';

/**
 * The one width admission a browser applies to a Token-2022 account.
 *
 * THE CONTRADICTION THIS RESOLVES, restated on the client side.
 * `TokenAccount::parse` (crates/dclutch-custody/src/token_svm/state.rs) refuses every
 * extension suffix by design, and under Token-2022 the Associated Token
 * Account program ALWAYS appends `ImmutableOwner` — it is not optional and no
 * caller chooses it — so a stranger's wallet's own token account is 170 bytes.
 * The chain resolved this with a second function,
 * `TokenAccount::parse_base_or_immutable_owner`, which the runtime profile
 * (`token_svm/profile.rs`), the operator (`wallet_terminal_payout_v3.rs`) and
 * the conservation ledger (`tools/gauntlet/journey/src/ledger.rs`) all share.
 *
 * TypeScript had no such function. `decodeToken2022BehaviorAccountV2` and
 * `walletTerminalPayoutV3.ts`'s `validateToken` each required exactly 165
 * bytes, so the browser could not read the destination cohort-14 exists to be
 * able to pay: `DsQSGKPbmJcZ89xts1Jgs1P5fprmX64fomqGFsQM1kmU`, 170 bytes, paid
 * 500,000,000 atoms on 2026-09-03. Two readers refusing the same account for
 * the same reason is one missing function, so this is that function, and both
 * of them call it.
 *
 * WHY THIS ONE EXTENSION AND NO OTHER, mirrored from the Rust rather than
 * re-argued: `ImmutableOwner` says the token program will refuse
 * `SetAuthority(AccountOwner)`, which STRENGTHENS every check made against a
 * destination — its mint, its owner, its initialized state — because the owner
 * that was authenticated cannot afterwards change. A transfer hook, a transfer
 * fee, a confidential balance or a CPI guard each change what a transfer
 * MEANS. They stay refused.
 *
 * WHY BYTE EQUALITY IS THE WHOLE CHECK. Rust walks the TLV: account type `2`,
 * one entry whose type is `ImmutableOwner` at length zero, and nothing after
 * it. At exactly 170 bytes there are four bytes of extension storage, which is
 * one header and no value, so an entry declaring any nonzero length is
 * truncated and a second entry does not fit. Every walk that can succeed at
 * that width succeeds on exactly the five bytes
 * `TOKEN_ACCOUNT_IMMUTABLE_OWNER_SUFFIX_V1` holds, and both constants are
 * emitted from the Rust's own named constants by
 * `scripts/generate-wallet-terminal-payout-v3.mjs` — including the six other
 * extensions the Rust's hostile test names, each of which differs from this
 * suffix in the two type bytes.
 */
export function admitBaseOrImmutableOwnerTokenAccountV1(bytes: Uint8Array, field: string): Uint8Array {
  if (bytes.length === TOKEN_ACCOUNT_BYTES_V1) return bytes;
  if (bytes.length !== TOKEN_ACCOUNT_IMMUTABLE_OWNER_BYTES_V1) {
    throw new Error(`${field} is ${bytes.length} bytes, neither the ${TOKEN_ACCOUNT_BYTES_V1}-byte base token account nor the ${TOKEN_ACCOUNT_IMMUTABLE_OWNER_BYTES_V1}-byte ImmutableOwner account the ATA program writes`);
  }
  for (const [index, byte] of TOKEN_ACCOUNT_IMMUTABLE_OWNER_SUFFIX_V1.entries()) {
    if (bytes[TOKEN_ACCOUNT_BYTES_V1 + index] !== byte) {
      throw new Error(`${field} carries a Token-2022 extension suffix that is not the exact empty ImmutableOwner entry`);
    }
  }
  // Borrowed, exactly as the Rust borrows it: every base-layout offset below
  // this point reads the 165 bytes and nothing sees the suffix.
  return bytes.subarray(0, TOKEN_ACCOUNT_BYTES_V1);
}
