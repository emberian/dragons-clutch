import { acquireFinalizedAccountsInChunksV1 } from './coreFound';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/**
 * ONE finalized observation, in the shape every compiled derivation here reads.
 *
 * Both wallet-terminal boundaries take the same snapshot wire — a format name,
 * a slot, the addresses the derivation asked for, and one observation per
 * address carrying the address it is OF. The Rust side already shares a single
 * decoder for it; this is the acquiring half, shared for the same reason. Two
 * copies of "read these addresses and pair each observation with its key" drift
 * exactly where it matters least visibly.
 *
 * Nothing here decides anything. The derivation names the addresses and owns
 * every check — which accounts may be vacant included; this owns the reads.
 */

export type ObservedRoundClientV1 = Pick<SolanaRpcClient, 'multipleAccounts' | 'multipleAccountDataSlices'>;

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

/**
 * Read exactly these addresses, in this order, at this floor.
 *
 * A vacant account is carried as vacant rather than refused: the derivation
 * decides which of a frame may be empty, with its own reason.
 */
export async function observedSnapshotJsonV1(
  client: ObservedRoundClientV1,
  addresses: ReadonlyArray<string>,
  floor: string,
  unixTimestamp: string,
  format: string,
): Promise<string> {
  const observed = await acquireFinalizedAccountsInChunksV1(client, addresses, floor);
  const byAddress = new Map<string, RpcAccount | null>(
    observed.accounts.map((entry) => [entry.address, entry.account]),
  );
  const accounts = addresses.map((address) => {
    const account = byAddress.get(address) ?? null;
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
  return JSON.stringify({ format, slot: floor, unixTimestamp, keys: [...addresses], accounts });
}
