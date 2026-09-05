import { hex, sha256 } from '@dclutch/sdk/bytes';
import {
  CORE_STATE_BYTES_V1,
  LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
  LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
  WALLET_TERMINAL_INPUT_ADDRESSES_FORMAT_V1,
  WALLET_TERMINAL_INPUT_WASM_BYTES_V1,
  WALLET_TERMINAL_INPUT_WASM_SHA256_V1,
} from './generated/walletTerminalInputWasmV1';

/**
 * The compiled wallet-terminal payout INPUT derivation, and the browser's half.
 *
 * THE DEFECT THIS CLOSES. Stage two — the payout manifest — reached the browser
 * in `eed52c57`, and `RedeemFlow` still asks a reader to import the JSON that
 * `dclutch-local-successor-bootstrap wallet-terminal-payout-input` emits. That
 * command was the last one standing between a stranger and a redemption.
 *
 * WHAT THIS FILE DELIBERATELY DOES NOT DO is derive the input. A Claims
 * aggregate PDA, a Core routing authentication, nine raw-record addresses and a
 * parent-context pre-image reimplemented in TypeScript is the mirror this
 * application keeps convicting. The three pure phases were extracted into
 * `crates/dclutch-operator` — the binary kept its shell,
 * two file reads, an RPC and a cluster policy — and compiled to wasm32, so what
 * runs here is the same code the operator toolchain runs.
 *
 * AND IT IS CALLED NOW. `walletTerminalInputSnapshot.ts` derives the address
 * book instead of receiving one, so a browser with a deployment table and a
 * connected wallet reaches a redemption with no imported document at all.
 */

/** The six functions the compiled derivation exposes. */
export type WalletTerminalInputWasmV1 = Readonly<{
  wallet_terminal_input_round_one_addresses_v1(requestJson: string): string;
  wallet_terminal_input_book_round_two_addresses_v1(requestJson: string, roundOneJson: string): string;
  wallet_terminal_input_book_round_three_addresses_v1(requestJson: string, roundOneJson: string, roundTwoJson: string): string;
  derive_wallet_terminal_input_request_v1(requestJson: string, roundOneJson: string, roundTwoJson: string, roundThreeJson: string): string;
  wallet_terminal_input_frame_addresses_v1(requestJson: string, roundOneJson: string): string;
  build_wallet_terminal_payout_input_v1(requestJson: string, roundOneJson: string, roundTwoJson: string): string;
  associated_token_account_program_id_v1(): string;
  core_state_bytes_v1(): number;
  liability_basis_market_header_bytes_v2(): number;
  liability_basis_position_header_bytes_v2(): number;
}>;

/**
 * Hostile-decode one of the derivation's own address lists.
 *
 * The list decides which accounts a reader's settled proceeds are computed
 * from, so a substituted transport must not be able to hand back an empty or
 * ragged one and have it read as a frame. Nothing here writes down how LONG a
 * round is: that is the derivation's, and a client that pinned the number would
 * be the second place it is written.
 */
export function parseWalletTerminalInputAddressesV1(source: string): ReadonlyArray<string> {
  let parsed: unknown;
  try { parsed = JSON.parse(source); } catch { throw new Error('payout input address list is not JSON'); }
  if (parsed === null || typeof parsed !== 'object') throw new Error('payout input address list is not an object');
  const listed = parsed as Record<string, unknown>;
  if (listed.format !== WALLET_TERMINAL_INPUT_ADDRESSES_FORMAT_V1) {
    throw new Error('payout input address list is not the exact accepted format');
  }
  const addresses = listed.addresses;
  if (!Array.isArray(addresses) || addresses.length === 0) throw new Error('payout input address list is empty');
  return Object.freeze(addresses.map((entry, index) => {
    if (typeof entry !== 'string' || entry === '') throw new Error(`payout input address ${index} is absent`);
    return entry;
  }));
}

/**
 * A real two-source check: the derivation's list against the caller's own ask.
 *
 * The client already knows which Market it is redeeming against — it is in the
 * request it wrote — and the derivation names the accounts it authenticates. A
 * list that does not contain that Market is not this Market's round, whatever
 * else it decoded to.
 */
export function requireWalletTerminalInputRoundNamesMarketV1(
  addresses: ReadonlyArray<string>,
  market: string,
): ReadonlyArray<string> {
  if (!addresses.includes(market)) {
    throw new Error(`the payout input round does not name the Market ${market} it was asked about`);
  }
  return addresses;
}

/** Load the checked Rust derivation blob; unverified fetched bytes never execute. */
export async function loadWalletTerminalInputWasmV1(
  fetcher: typeof fetch = (input, init) => globalThis.fetch(input, init),
): Promise<WalletTerminalInputWasmV1> {
  const url = new URL('./generated/walletTerminalInputWasm/wallet_terminal_input_bg.wasm', import.meta.url);
  const response = await fetcher(url);
  if (!response.ok) throw new Error(`payout input derivation WASM fetch failed with HTTP ${response.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length !== WALLET_TERMINAL_INPUT_WASM_BYTES_V1
      || hex(await sha256(bytes)) !== WALLET_TERMINAL_INPUT_WASM_SHA256_V1) {
    throw new Error('payout input derivation WASM bytes do not match the generated Rust artifact identity');
  }
  const wasmModule = await import('./generated/walletTerminalInputWasm/wallet_terminal_input.js');
  await wasmModule.default({ module_or_path: bytes });
  // THE BLOB AGREED WITH ITS DIGEST AND MAY STILL DISAGREE WITH ITS OWNERS.
  // That is the one drift a digest alone cannot catch: emitted facts and
  // artifact from different trees. Each width below is stated by the contract
  // that owns it and pinned again inside the crate by a
  // `const _: () = assert!(...)`, so this asks the loaded module and compares.
  const widths: ReadonlyArray<Readonly<[string, number, number]>> = [
    ['Core Market state', wasmModule.core_state_bytes_v1(), CORE_STATE_BYTES_V1],
    ['Claims aggregate header', wasmModule.liability_basis_market_header_bytes_v2(), LIABILITY_BASIS_MARKET_HEADER_BYTES_V2],
    ['Claims Position header', wasmModule.liability_basis_position_header_bytes_v2(), LIABILITY_BASIS_POSITION_HEADER_BYTES_V2],
  ];
  for (const [role, reported, stated] of widths) {
    if (reported !== stated) {
      throw new Error(`payout input derivation reports a ${reported}-byte ${role} where its contract states ${stated}`);
    }
  }
  return Object.freeze({
    wallet_terminal_input_round_one_addresses_v1: wasmModule.wallet_terminal_input_round_one_addresses_v1,
    wallet_terminal_input_book_round_two_addresses_v1: wasmModule.wallet_terminal_input_book_round_two_addresses_v1,
    wallet_terminal_input_book_round_three_addresses_v1: wasmModule.wallet_terminal_input_book_round_three_addresses_v1,
    derive_wallet_terminal_input_request_v1: wasmModule.derive_wallet_terminal_input_request_v1,
    wallet_terminal_input_frame_addresses_v1: wasmModule.wallet_terminal_input_frame_addresses_v1,
    build_wallet_terminal_payout_input_v1: wasmModule.build_wallet_terminal_payout_input_v1,
    associated_token_account_program_id_v1: wasmModule.associated_token_account_program_id_v1,
    core_state_bytes_v1: wasmModule.core_state_bytes_v1,
    liability_basis_market_header_bytes_v2: wasmModule.liability_basis_market_header_bytes_v2,
    liability_basis_position_header_bytes_v2: wasmModule.liability_basis_position_header_bytes_v2,
  });
}
