import { acquireFinalizedAccountsInChunksV1 } from './coreFound';
import { WALLET_TERMINAL_PAYOUT_SNAPSHOT_FORMAT_V1 } from './generated/walletTerminalPayoutWasmV1';
import { type RpcAccount, type SolanaRpcClient } from './rpc';
import {
  parseWalletTerminalPayoutAddressesV1,
  type WalletTerminalPayoutWasmV1,
} from './walletTerminalPayoutV1';

/**
 * The finalized snapshot the compiled payout derivation authenticates.
 *
 * THE LAST UNIT between a stranger and redeeming. The derivation was extracted
 * verbatim, compiled to wasm32, digest-pinned and canaried; what it still
 * needed was its input.
 *
 * EVERY ADDRESS COMES FROM THE DERIVATION'S OWN LIST. This file asks the
 * boundary which accounts to observe and reads exactly those, in exactly that
 * order. It does not compute one of them. A client that derived these
 * addresses alongside the Rust would BE a second routing implementation, and
 * the two would drift the first time a seed or a coordinate moved — which is
 * the mirror hazard this application keeps convicting, prevented here at the
 * point where it would otherwise be introduced.
 *
 * EVERY READ IS FINALIZED, AT ONE FLOOR, taken once before anything is read. A
 * snapshot stitched from several observations authenticates a chain that
 * existed at no single moment.
 *
 * Nothing here decides anything. The derivation owns every check — which of
 * the thirty-six accounts may be vacant included — and this owns the reads.
 */

export type AcquiredPayoutSnapshotV1 = Readonly<{
  snapshotJson: string;
  observedSlot: string;
  addresses: ReadonlyArray<string>;
}>;

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

/** Read the derivation's own frame at one finalized floor. */
export async function acquireWalletTerminalPayoutSnapshotV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'blockTime' | 'multipleAccounts'>,
  planner: WalletTerminalPayoutWasmV1,
  inputJson: string,
): Promise<AcquiredPayoutSnapshotV1> {
  // ONE floor, taken before anything is read and passed to the read below.
  const floor = await client.finalizedSlot();
  const unixTimestamp = (await client.blockTime(floor)) ?? '0';

  // The derivation names its own accounts. Parsing rather than trusting is the
  // client's half: the list carries the settlement frame width, and this
  // refuses one that is not the width Claims states.
  const addresses = parseWalletTerminalPayoutAddressesV1(
    planner.wallet_terminal_payout_addresses_v1(inputJson),
  );

  const observed = await acquireFinalizedAccountsInChunksV1(client, addresses, floor);
  const byAddress = new Map<string, RpcAccount | null>(
    observed.accounts.map((entry) => [entry.address, entry.account]),
  );

  const accounts = addresses.map((address) => {
    const account = byAddress.get(address) ?? null;
    // A vacant account is a legitimate input — a payout's lookup table or a
    // record may or may not exist — so absence is carried, not refused. The
    // derivation decides which of the frame may be empty, with its own reason.
    if (account === null) return null;
    return {
      // Carried beside the key list on purpose: the boundary cross-checks the
      // two, because an observation paired with the wrong slot is the one
      // corruption a snapshot can suffer that still decodes and still
      // authenticates, against the wrong account.
      key: address,
      owner: account.owner,
      lamports: account.lamports,
      executable: account.executable,
      dataBase64: base64(account.data),
    };
  });

  return Object.freeze({
    snapshotJson: JSON.stringify({
      format: WALLET_TERMINAL_PAYOUT_SNAPSHOT_FORMAT_V1,
      slot: floor,
      unixTimestamp,
      keys: [...addresses],
      accounts,
    }),
    observedSlot: floor,
    addresses,
  });
}

/**
 * Acquire, then let the derivation build its own manifest.
 *
 * The JSON this returns is exactly the `dclutch-wallet-terminal-payout-v3`
 * artifact `RedeemFlow` already imports and checks, so everything downstream of
 * it — the account proofs, the wallet handoff, the journal, submission — is
 * unchanged. What changes is where the artifact comes from: the browser
 * derives it from finalized chain state instead of a reader importing what a
 * Rust binary computed elsewhere.
 */
export async function deriveWalletTerminalPayoutManifestV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'blockTime' | 'multipleAccounts'>,
  planner: WalletTerminalPayoutWasmV1,
  inputJson: string,
): Promise<string> {
  const acquired = await acquireWalletTerminalPayoutSnapshotV1(client, planner, inputJson);
  return planner.build_wallet_terminal_payout_manifest_v1(inputJson, acquired.snapshotJson);
}
