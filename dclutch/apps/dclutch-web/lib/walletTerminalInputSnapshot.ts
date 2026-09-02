import { WALLET_TERMINAL_INPUT_REQUEST_FORMAT_V1, WALLET_TERMINAL_INPUT_SNAPSHOT_FORMAT_V1 } from './generated/walletTerminalInputWasmV1';
import { observedSnapshotJsonV1 } from './observedSnapshotV1';
import { type SolanaRpcClient } from './rpc';
import {
  parseWalletTerminalInputAddressesV1,
  requireWalletTerminalInputRoundNamesMarketV1,
  type WalletTerminalInputWasmV1,
} from './walletTerminalInputV1';

/**
 * PHASE ZERO AND STAGE ONE, acquired: the payout input, from chain alone.
 *
 * THE LAST IMPORT. Stage two derives the payout manifest in the browser, and
 * `RedeemFlow` still needed a reader to bring it the payout INPUT that stage
 * consumes — the output of a CLI command that reads two operator artifacts.
 * A browser has no campaign report, so as long as the address book had to be
 * SUPPLIED, no amount of derivation downstream made redemption reachable by a
 * stranger.
 *
 * SO THE BOOK IS DERIVED. Seven of its eleven rows have chain pointers, and
 * the four `terminal_composition_*` rows have none anywhere — they are not
 * stored facts about a market, they are COMPILED from it, so the derivation
 * recompiles them with the same function the founding published them with.
 *
 * FOUR ROUNDS, AT ONE FLOOR, and the shape is forced rather than chosen:
 *
 *  1. the Market, the Claims aggregate, and this wallet's admission record —
 *     all three addressable before any read;
 *  2. the realm, Product and product-basis records, addressed by digests round
 *     one produced (the basis digest comes from the ADMISSION record: the
 *     aggregate carries only the semantic basis identity, which authenticates
 *     a basis body and cannot address one);
 *  3. the result-domain and portfolio records, addressed by digests inside
 *     round two's BYTES, plus the price-gate certificate when the basis names
 *     one;
 *  4. the payout frame the routed input names.
 *
 * Every address in all four comes from the derivation's own list. Not one is
 * computed here.
 */

export type WalletTerminalInputRequestV1 = Readonly<{
  programs: Readonly<{ registry: string; core: string; claims: string; custody: string; resolution: string }>;
  market: string;
  owner: string;
  recipient: string;
  claimIndex: number;
  quantity?: string;
}>;

export type AcquiredPayoutInputV1 = Readonly<{
  inputJson: string;
  observedSlot: string;
  rounds: number;
}>;

type RoundClientV1 = Pick<SolanaRpcClient, 'finalizedSlot' | 'blockTime' | 'multipleAccounts'>;

/**
 * The caller's own ask, in the exact shape the boundary accepts.
 *
 * No `releaseSet` and no `routing`: the release set is the MARKET's choice and
 * the address book is derived, so a browser states neither. This is the
 * analogue of the CLI's argv, not of its artifacts.
 */
export function walletTerminalInputRequestJsonV1(request: WalletTerminalInputRequestV1): string {
  return JSON.stringify({
    format: WALLET_TERMINAL_INPUT_REQUEST_FORMAT_V1,
    programs: request.programs,
    request: {
      market: request.market,
      owner: request.owner,
      recipient: request.recipient,
      claimIndex: request.claimIndex,
      ...(request.quantity === undefined ? {} : { quantity: request.quantity }),
    },
  });
}

/**
 * Derive the payout input for one wallet against one resolved Market.
 *
 * Returns the exact `dclutch-wallet-terminal-payout-plan-input-v1` a reader
 * used to import, so everything downstream of it is unchanged. What changes is
 * where it comes from.
 */
export async function deriveWalletTerminalPayoutInputV1(
  client: RoundClientV1,
  derivation: WalletTerminalInputWasmV1,
  request: WalletTerminalInputRequestV1,
): Promise<AcquiredPayoutInputV1> {
  const requestJson = walletTerminalInputRequestJsonV1(request);

  // ONE floor, taken once, used by all four rounds. A book stitched from
  // several observations describes a chain that existed at no single moment.
  const floor = await client.finalizedSlot();
  const unixTimestamp = (await client.blockTime(floor)) ?? '0';
  const round = async (addressesJson: string) => {
    const addresses = parseWalletTerminalInputAddressesV1(addressesJson);
    return {
      addresses,
      json: await observedSnapshotJsonV1(client, addresses, floor, unixTimestamp, WALLET_TERMINAL_INPUT_SNAPSHOT_FORMAT_V1),
    };
  };

  const one = await round(derivation.wallet_terminal_input_round_one_addresses_v1(requestJson));
  // Two independent sources: the request this client wrote, and the list the
  // derivation returned.
  requireWalletTerminalInputRoundNamesMarketV1(one.addresses, request.market);
  const two = await round(derivation.wallet_terminal_input_book_round_two_addresses_v1(requestJson, one.json));
  const three = await round(
    derivation.wallet_terminal_input_book_round_three_addresses_v1(requestJson, one.json, two.json),
  );

  // The book, carried rather than reconstructed: what comes back is this same
  // request with `routing` filled in, so the routing shape is never written
  // down here.
  const derived = derivation.derive_wallet_terminal_input_request_v1(requestJson, one.json, two.json, three.json);

  const frame = await round(derivation.wallet_terminal_input_frame_addresses_v1(derived, one.json));
  return Object.freeze({
    inputJson: derivation.build_wallet_terminal_payout_input_v1(derived, one.json, frame.json),
    observedSlot: floor,
    rounds: 4,
  });
}
