import { PublicKey } from '@solana/web3.js';
import { MAX_MULTIPLE_ACCOUNTS, type RpcAccount, type SolanaRpcClient } from './rpc';

export type AccountSnapshotRequestV1 = Readonly<{
  address: string;
  dataSlice: Readonly<{ offset: number; length: number }> | null;
}>;
export type AccountSnapshotV1 = Readonly<{
  slot: string;
  accounts: ReadonlyArray<AccountSnapshotRequestV1 & Readonly<{ account: RpcAccount | null }>>;
}>;

/** Transport profile, provisional: raise only with a measured supported native route. */
const MAX_DISCOVERY_ACCOUNTS = 256;

/**
 * Fetch native-selected addresses without deriving protocol accounts in JavaScript.
 * Absent accounts remain absent. Sliced Loader evidence is labelled explicitly.
 * Every row must belong to one finalized slot, including across the RPC key limit.
 */
export async function readAccountSnapshotV1(
  client: Pick<SolanaRpcClient, 'multipleAccounts' | 'multipleAccountDataSlices'>,
  requests: ReadonlyArray<AccountSnapshotRequestV1>, minimumSlot: string,
  requireCurrent: () => void = () => {},
  label = 'Account',
): Promise<AccountSnapshotV1> {
  if (!/^(0|[1-9][0-9]*)$/.test(minimumSlot)) throw new Error(`${label} snapshot floor is not a canonical slot`);
  if (requests.length === 0 || requests.length > MAX_DISCOVERY_ACCOUNTS) throw new Error(`${label} discovery exceeds its provisional 1..256-account transport profile`);
  const seen = new Set<string>();
  for (const request of requests) {
    if (new PublicKey(request.address).toBase58() !== request.address || seen.has(request.address)) throw new Error(`${label} discovery requires distinct canonical account addresses`);
    seen.add(request.address);
    const slice = request.dataSlice;
    if (slice !== null && (!Number.isSafeInteger(slice.offset) || slice.offset < 0 || !Number.isSafeInteger(slice.length) || slice.length < 1)) throw new Error(`${label} native discovery requested an invalid data slice`);
  }
  // Keep different native slice requests in different RPC calls.
  const groups = new Map<string, AccountSnapshotRequestV1[]>();
  for (const request of requests) {
    const key = JSON.stringify(request.dataSlice);
    groups.set(key, [...(groups.get(key) ?? []), request]);
  }
  const chunks: AccountSnapshotRequestV1[][] = [];
  for (const group of groups.values()) {
    for (let start = 0; start < group.length; start += MAX_MULTIPLE_ACCOUNTS) chunks.push(group.slice(start, start + MAX_MULTIPLE_ACCOUNTS));
  }
  for (let attempt = 0; attempt < 3; attempt += 1) {
    requireCurrent();
    const rows = await Promise.all(chunks.map(async (chunk) => {
      requireCurrent();
      const addresses = chunk.map((row) => row.address);
      const slice = chunk[0]!.dataSlice;
      const observed = slice === null ? await client.multipleAccounts(addresses, minimumSlot)
        : await client.multipleAccountDataSlices(addresses, slice.offset, slice.length, minimumSlot);
      requireCurrent();
      if (observed.accounts.length !== chunk.length || observed.accounts.some((row, index) => row.address !== chunk[index]!.address)) throw new Error(`${label} RPC snapshot changed the requested account order`);
      return { slot: observed.slot, accounts: observed.accounts.map((row, index) => ({ ...chunk[index]!, account: row.account })) };
    }));
    const slot = rows[0]!.slot;
    if (rows.every((row) => row.slot === slot && BigInt(row.slot) >= BigInt(minimumSlot))) {
      const byAddress = new Map(rows.flatMap((row) => row.accounts).map((row) => [row.address, row]));
      return { slot, accounts: requests.map((request) => byAddress.get(request.address)!) };
    }
  }
  throw new Error(`${label} finalized account reads did not converge on one slot after three attempts`);
}
