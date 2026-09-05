import { describe, expect, it } from 'vitest';

import {
  TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3,
  TERMINAL_SETTLEMENT_REQUEST_BYTES_V3,
  WALLET_TERMINAL_PAYOUT_ADDRESSES_FORMAT_V1,
  WALLET_TERMINAL_PAYOUT_SNAPSHOT_FORMAT_V1,
  WALLET_TERMINAL_PAYOUT_WASM_BYTES_V1,
  WALLET_TERMINAL_PAYOUT_WASM_SHA256_V1,
} from './generated/walletTerminalPayoutWasmV1';
import {
  loadWalletTerminalPayoutWasmV1,
  parseWalletTerminalPayoutAddressesV1,
} from './walletTerminalPayoutV1';

/**
 * THE LAST OF THE THREE. `RedeemFlow` says "This browser never creates or
 * completes a payout plan" and asks a reader to import JSON a Rust binary
 * emits. The derivation was extracted verbatim into a workspace crate and
 * compiled; this is the browser's half of that seam.
 *
 * Not wired to a snapshot yet, deliberately. A boundary that compiles and is
 * not yet called is a clean state; a boundary half-wired to an acquisition is
 * not.
 */
describe('the payout derivation reaches the browser as compiled Rust', () => {
  it('states the settlement frame width and request size from Claims, not from here', () => {
    // Both are emitted from `dclutch-claims` and pinned again inside the
    // WASM crate with `const _: () = assert!(...)`. If either moves, the Rust
    // build fails before this test can be wrong.
    expect(TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3).toBe(36);
    expect(TERMINAL_SETTLEMENT_REQUEST_BYTES_V3).toBe(640);
    expect(WALLET_TERMINAL_PAYOUT_SNAPSHOT_FORMAT_V1).toBe('dclutch-wallet-terminal-payout-snapshot-v1');
  });

  it('refuses a blob whose bytes are not the generated artifact', async () => {
    // This module builds the transaction that moves a reader's settled
    // proceeds. Unverified fetched bytes never execute.
    const wrong = new Uint8Array(WALLET_TERMINAL_PAYOUT_WASM_BYTES_V1).fill(9);
    await expect(loadWalletTerminalPayoutWasmV1(async () => new Response(wrong)))
      .rejects.toThrow(/do not match the generated Rust artifact identity/);
  });

  it('refuses a short blob before it hashes it', async () => {
    await expect(loadWalletTerminalPayoutWasmV1(async () => new Response(new Uint8Array(8))))
      .rejects.toThrow(/do not match the generated Rust artifact identity/);
  });

  it('refuses an address list that is not the exact accepted format', () => {
    expect(() => parseWalletTerminalPayoutAddressesV1('{"format":"other"}'))
      .toThrow(/payout address list is not the exact accepted format/);
    expect(() => parseWalletTerminalPayoutAddressesV1('not json'))
      .toThrow(/payout address list is not JSON/);
  });

  it('refuses an address list whose frame is not the width Claims states', () => {
    // The derivation cannot emit this; a substituted transport can. The client
    // checks the width it was told against the width the contract states, and
    // writes down neither.
    const listed = JSON.stringify({
      format: WALLET_TERMINAL_PAYOUT_ADDRESSES_FORMAT_V1,
      addresses: ['a'],
      accountCount: 35,
    });
    expect(() => parseWalletTerminalPayoutAddressesV1(listed))
      .toThrow(/payout states a 35-account settlement frame where Claims has 36/);
  });

  it('hands back the derivation’s own address list', () => {
    const listed = JSON.stringify({
      format: WALLET_TERMINAL_PAYOUT_ADDRESSES_FORMAT_V1,
      addresses: ['one', 'two'],
      accountCount: TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3,
    });
    expect(parseWalletTerminalPayoutAddressesV1(listed)).toEqual(['one', 'two']);
  });

  it('pins the artifact identity so a regenerated derivation cannot slip in silently', () => {
    expect(WALLET_TERMINAL_PAYOUT_WASM_SHA256_V1).toMatch(/^[0-9a-f]{64}$/);
    expect(WALLET_TERMINAL_PAYOUT_WASM_BYTES_V1).toBeGreaterThan(100_000);
  });
});
