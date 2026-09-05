import { WALLET_TERMINAL_PAYOUT_SNAPSHOT_FORMAT_V1 } from './generated/walletTerminalPayoutWasmV1';
import { observedSnapshotJsonV1 } from './observedSnapshotV1';
import { type SolanaRpcClient } from '@dclutch/sdk/rpc';
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

/** Read the derivation's own frame at one finalized floor. */
export async function acquireWalletTerminalPayoutSnapshotV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'blockTime' | 'multipleAccounts' | 'multipleAccountDataSlices'>,
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

  // ONE acquiring implementation, shared with stage one rather than copied.
  const snapshotJson = await observedSnapshotJsonV1(
    client,
    addresses,
    floor,
    unixTimestamp,
    WALLET_TERMINAL_PAYOUT_SNAPSHOT_FORMAT_V1,
  );

  return Object.freeze({ snapshotJson, observedSlot: floor, addresses });
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
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'blockTime' | 'multipleAccounts' | 'multipleAccountDataSlices'>,
  planner: WalletTerminalPayoutWasmV1,
  inputJson: string,
): Promise<string> {
  const acquired = await acquireWalletTerminalPayoutSnapshotV1(client, planner, inputJson);
  return planner.build_wallet_terminal_payout_manifest_v1(inputJson, acquired.snapshotJson);
}
