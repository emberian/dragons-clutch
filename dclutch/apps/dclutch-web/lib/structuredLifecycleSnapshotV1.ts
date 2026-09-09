import { readAccountSnapshotV1, type AccountSnapshotRequestV1 } from '@dclutch/sdk/accountSnapshotV1';
import { type SolanaRpcClient } from '@dclutch/sdk/rpc';

export type { AccountSnapshotRequestV1 as StructuredAccountRequestV1, AccountSnapshotV1 as StructuredLifecycleSnapshotV1 } from '@dclutch/sdk/accountSnapshotV1';

/** Structured names the shared bounded snapshot transport; native code owns discovery. */
export function readStructuredLifecycleSnapshotV1(
  client: Pick<SolanaRpcClient, 'multipleAccounts' | 'multipleAccountDataSlices'>,
  requests: ReadonlyArray<AccountSnapshotRequestV1>, minimumSlot: string,
  requireCurrent: () => void,
) {
  return readAccountSnapshotV1(client, requests, minimumSlot, requireCurrent, 'Structured');
}
