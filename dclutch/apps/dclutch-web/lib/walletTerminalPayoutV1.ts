import { hex, sha256 } from './bytes';
import {
  TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3,
  WALLET_TERMINAL_PAYOUT_ADDRESSES_FORMAT_V1,
  WALLET_TERMINAL_PAYOUT_WASM_BYTES_V1,
  WALLET_TERMINAL_PAYOUT_WASM_SHA256_V1,
} from './generated/walletTerminalPayoutWasmV1';

/**
 * The compiled wallet-terminal payout derivation, and the browser's half of it.
 *
 * THE DEFECT THIS CLOSES. `RedeemFlow` says "This browser never creates or
 * completes a payout plan" and hands the reader a `dclutch-local-successor-
 * bootstrap wallet-terminal-payout-input` command. Redemption is the last of
 * the three capabilities in "the browser can sign but cannot originate".
 *
 * WHAT THIS FILE DELIBERATELY DOES NOT DO is derive the payout. A
 * thirty-six-account settlement frame, a lookup table geometry, and an
 * authenticated report reimplemented in TypeScript is the mirror this
 * application keeps convicting. The derivation was extracted VERBATIM into
 * `crates/dclutch-wallet-terminal-payout-operator` — the binary kept only its
 * shell — and compiled to wasm32, so what runs here is the same code the
 * operator toolchain runs.
 *
 * NOT YET CALLED. The snapshot acquisition is its own unit and is not built.
 * A boundary that compiles and is not yet wired is a clean state; a boundary
 * half-wired to an acquisition is not.
 */

/** The three functions the compiled derivation exposes. */
export type WalletTerminalPayoutWasmV1 = Readonly<{
  wallet_terminal_payout_addresses_v1(inputJson: string): string;
  build_wallet_terminal_payout_manifest_v1(inputJson: string, snapshotJson: string): string;
  terminal_settlement_account_count_v3(): number;
  terminal_settlement_request_bytes_v3(): number;
  terminal_settlement_candidate_domain_v3(): string;
}>;

/**
 * Hostile-decode the derivation's own address list.
 *
 * The width check is not defensive noise. `TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3`
 * is emitted from Claims and pinned again inside the WASM crate by a
 * `const _: () = assert!(...)`, so the derivation cannot emit another width —
 * but a substituted transport can, and this list decides which accounts a
 * reader's settled proceeds are computed from. The client checks the number it
 * was told against the number Claims states, and writes down neither.
 */
export function parseWalletTerminalPayoutAddressesV1(source: string): ReadonlyArray<string> {
  let parsed: unknown;
  try { parsed = JSON.parse(source); } catch { throw new Error('payout address list is not JSON'); }
  if (parsed === null || typeof parsed !== 'object') throw new Error('payout address list is not an object');
  const listed = parsed as Record<string, unknown>;
  if (listed.format !== WALLET_TERMINAL_PAYOUT_ADDRESSES_FORMAT_V1) {
    throw new Error('payout address list is not the exact accepted format');
  }
  if (listed.accountCount !== TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3) {
    throw new Error(`payout states a ${String(listed.accountCount)}-account settlement frame where Claims has ${TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3}`);
  }
  const addresses = listed.addresses;
  if (!Array.isArray(addresses) || addresses.length === 0) throw new Error('payout address list is empty');
  return Object.freeze(addresses.map((entry, index) => {
    if (typeof entry !== 'string' || entry === '') throw new Error(`payout address ${index} is absent`);
    return entry;
  }));
}

/** Load the checked Rust derivation blob; unverified fetched bytes never execute. */
export async function loadWalletTerminalPayoutWasmV1(
  fetcher: typeof fetch = (input, init) => globalThis.fetch(input, init),
): Promise<WalletTerminalPayoutWasmV1> {
  const url = new URL('./generated/walletTerminalPayoutWasm/wallet_terminal_payout_bg.wasm', import.meta.url);
  const response = await fetcher(url);
  if (!response.ok) throw new Error(`payout derivation WASM fetch failed with HTTP ${response.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length !== WALLET_TERMINAL_PAYOUT_WASM_BYTES_V1
      || hex(await sha256(bytes)) !== WALLET_TERMINAL_PAYOUT_WASM_SHA256_V1) {
    throw new Error('payout derivation WASM bytes do not match the generated Rust artifact identity');
  }
  const wasmModule = await import('./generated/walletTerminalPayoutWasm/wallet_terminal_payout.js');
  await wasmModule.default({ module_or_path: bytes });
  const width = wasmModule.terminal_settlement_account_count_v3();
  if (width !== TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3) {
    // The blob agreed with its digest and still disagrees with Claims. That can
    // only mean the emitted facts and the artifact came from different trees,
    // which is exactly the drift the canary exists for and the one thing a
    // digest alone cannot catch.
    throw new Error(`payout derivation reports a ${width}-account settlement frame where Claims has ${TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3}`);
  }
  return Object.freeze({
    wallet_terminal_payout_addresses_v1: wasmModule.wallet_terminal_payout_addresses_v1,
    build_wallet_terminal_payout_manifest_v1: wasmModule.build_wallet_terminal_payout_manifest_v1,
    terminal_settlement_account_count_v3: wasmModule.terminal_settlement_account_count_v3,
    terminal_settlement_request_bytes_v3: wasmModule.terminal_settlement_request_bytes_v3,
    terminal_settlement_candidate_domain_v3: wasmModule.terminal_settlement_candidate_domain_v3,
  });
}
