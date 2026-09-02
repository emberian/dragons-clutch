import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import {
  CORE_STATE_BYTES_V1,
  LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
  LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
  WALLET_TERMINAL_INPUT_ADDRESSES_FORMAT_V1,
  WALLET_TERMINAL_INPUT_REQUEST_FORMAT_V1,
  WALLET_TERMINAL_INPUT_SNAPSHOT_FORMAT_V1,
  WALLET_TERMINAL_INPUT_WASM_BYTES_V1,
  WALLET_TERMINAL_INPUT_WASM_SHA256_V1,
} from './generated/walletTerminalInputWasmV1';
import {
  loadWalletTerminalInputWasmV1,
  parseWalletTerminalInputAddressesV1,
  requireWalletTerminalInputRoundNamesMarketV1,
} from './walletTerminalInputV1';

/**
 * THE LAST CLI COMMAND, compiled. Stage two reached the browser in `eed52c57`
 * and `RedeemFlow` still asks a reader to import the JSON that
 * `wallet-terminal-payout-input` emits. Its three pure phases were extracted
 * into a workspace crate and built for wasm32; this is the browser's half.
 *
 * Not wired to an acquisition yet, deliberately: the address book phase one
 * takes has no browser source. A boundary that compiles and is not yet called
 * is a clean state; a boundary half-wired to an acquisition is not.
 */

const artifact = () => new Uint8Array(readFileSync(
  fileURLToPath(new URL('./generated/walletTerminalInputWasm/wallet_terminal_input_bg.wasm', import.meta.url)),
));

describe('the payout input derivation reaches the browser as compiled Rust', () => {
  it('states every width from the contract that owns it, not from here', () => {
    // Each is emitted from its owner and pinned again inside the WASM crate
    // with `const _: () = assert!(...)`. If one moves, the Rust build fails
    // before this test can be wrong.
    expect(CORE_STATE_BYTES_V1).toBe(368);
    expect(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2).toBe(256);
    expect(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2).toBe(128);
    expect(WALLET_TERMINAL_INPUT_SNAPSHOT_FORMAT_V1).toBe('dclutch-wallet-terminal-payout-input-snapshot-v1');
    expect(WALLET_TERMINAL_INPUT_REQUEST_FORMAT_V1).toBe('dclutch-wallet-terminal-payout-input-request-v1');
  });

  it('refuses a blob whose bytes are not the generated artifact', async () => {
    // This module derives the input that becomes the transaction moving a
    // reader's settled proceeds. Unverified fetched bytes never execute.
    const wrong = new Uint8Array(WALLET_TERMINAL_INPUT_WASM_BYTES_V1).fill(9);
    await expect(loadWalletTerminalInputWasmV1(async () => new Response(wrong)))
      .rejects.toThrow(/do not match the generated Rust artifact identity/);
  });

  it('refuses a short blob before it hashes it', async () => {
    await expect(loadWalletTerminalInputWasmV1(async () => new Response(new Uint8Array(8))))
      .rejects.toThrow(/do not match the generated Rust artifact identity/);
  });

  it('refuses a blob the server would not serve', async () => {
    await expect(loadWalletTerminalInputWasmV1(async () => new Response(null, { status: 404 })))
      .rejects.toThrow(/fetch failed with HTTP 404/);
  });

  /**
   * THE POSITIVE CASE, and it is the one that makes the refusals mean
   * something: the committed artifact loads, and the widths it reports are the
   * ones its contracts state. A blob can match its digest and still have come
   * from a different tree, which is the drift a digest alone cannot catch.
   */
  it('loads the committed artifact and answers with its own contracts’ widths', async () => {
    const derivation = await loadWalletTerminalInputWasmV1(async () => new Response(artifact()));
    expect(derivation.core_state_bytes_v1()).toBe(CORE_STATE_BYTES_V1);
    expect(derivation.liability_basis_market_header_bytes_v2()).toBe(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2);
    expect(derivation.liability_basis_position_header_bytes_v2()).toBe(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2);
  });

  /** The derivation refuses a request in its own words, not in the browser's. */
  it('returns the derivation’s own refusal for a request that is not one', async () => {
    const derivation = await loadWalletTerminalInputWasmV1(async () => new Response(artifact()));
    expect(() => derivation.wallet_terminal_input_round_one_addresses_v1('{"format":"other"}'))
      .toThrow(/exact accepted JSON|payout input request format/);
  });

  it('refuses an address list that is not the exact accepted format', () => {
    expect(() => parseWalletTerminalInputAddressesV1('{"format":"other"}'))
      .toThrow(/payout input address list is not the exact accepted format/);
    expect(() => parseWalletTerminalInputAddressesV1('not json'))
      .toThrow(/payout input address list is not JSON/);
    expect(() => parseWalletTerminalInputAddressesV1(JSON.stringify({
      format: WALLET_TERMINAL_INPUT_ADDRESSES_FORMAT_V1, addresses: [],
    }))).toThrow(/payout input address list is empty/);
  });

  it('cross-checks the derivation’s round against the Market the caller asked about', () => {
    // Two independent sources: the request this client wrote, and the list the
    // derivation returned. A round that does not name the Market is not this
    // Market's round, whatever else it decoded to.
    const listed = parseWalletTerminalInputAddressesV1(JSON.stringify({
      format: WALLET_TERMINAL_INPUT_ADDRESSES_FORMAT_V1, addresses: ['market', 'aggregate'],
    }));
    expect(requireWalletTerminalInputRoundNamesMarketV1(listed, 'market')).toEqual(['market', 'aggregate']);
    expect(() => requireWalletTerminalInputRoundNamesMarketV1(listed, 'another'))
      .toThrow(/does not name the Market another it was asked about/);
  });

  it('pins the artifact identity so a regenerated derivation cannot slip in silently', () => {
    expect(WALLET_TERMINAL_INPUT_WASM_SHA256_V1).toMatch(/^[0-9a-f]{64}$/);
    expect(WALLET_TERMINAL_INPUT_WASM_BYTES_V1).toBeGreaterThan(100_000);
  });
});
