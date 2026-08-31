import { PublicKey } from '@solana/web3.js';

import {
  DEVNET_PROGRAM_EVIDENCE_V1,
  PROTOCOL_ROLES_V1,
  PROTOCOL_ROLE_MEANING_V1,
  type DeploymentV1,
  type ProtocolRoleV1,
} from '../deployments';
import { clusterNameV1 } from '../rpcDefault';
import { type ConnectionFacts, type SolanaRpcClient } from '../rpc';

/**
 * The explorer's landing content: THE PROTOCOL, not a form.
 *
 * The app knows its own deployment (`lib/deployments.ts`), so the first thing
 * the explorer shows is the seven role programs read LIVE off the active
 * cluster, plus the recent transactions the node's own per-address signature
 * history holds for them. Everything here is a finalized read or the node's
 * own history — the same provenance discipline as every other surface.
 */

export const PROTOCOL_ACTIVITY_PER_PROGRAM = 8;
export const PROTOCOL_ACTIVITY_MAX_ROWS = 12;

export type ProtocolProgramCardV1 = Readonly<{
  role: ProtocolRoleV1;
  address: string;
  meaning: string;
  status: 'live' | 'absent' | 'not-executable';
  owner: string | null;
  ownerLabel: string | null;
  lamports: string | null;
  /** DEPLOY-1's recorded deployment slot, when this deployment carries evidence. */
  /**
   * DEPLOY-1's ORIGINAL deployment slot, not the program's current one.
   *
   * These programs are mutable and upgraded in place at permanent addresses,
   * so the slot they sit at moves and this constant does not. It is kept
   * because a first-deployment slot is a historical fact that cannot go stale;
   * the card labels it as such. The live slot is read from ProgramData by the
   * /operate deployment inspector, which is the surface that already fetches
   * Loader headers.
   */
  deploymentSlot: string | null;
}>;

export type ProtocolActivityRowV1 = Readonly<{
  signature: string;
  slot: string;
  blockTime: string | null;
  succeeded: boolean;
  errorText: string | null;
  /** Which role programs' histories list this signature — the decoded-by-name part. */
  roles: ReadonlyArray<ProtocolRoleV1>;
}>;

export type ProtocolHomeV1 = Readonly<{
  facts: ConnectionFacts;
  /** The chain's own name for itself, from its genesis hash. */
  clusterName: string;
  /** Whether the chain's genesis hash matches what the manifest expects. */
  clusterCheck: 'match' | 'mismatch' | 'unpinned';
  observedSlot: string;
  cards: ReadonlyArray<ProtocolProgramCardV1>;
  activity: ReadonlyArray<ProtocolActivityRowV1>;
  /** Provenance of the activity list, or why it is shorter than expected. */
  activityNote: string;
}>;

const LOADER_LABELS: ReadonlyMap<string, string> = new Map([
  ['BPFLoaderUpgradeab1e11111111111111111111111', 'upgradeable loader'],
  ['BPFLoader2111111111111111111111111111111111', 'BPF loader v2'],
  ['NativeLoader1111111111111111111111111111111', 'native loader'],
]);

type ProtocolHomeRpc = Pick<SolanaRpcClient, 'probe' | 'multipleAccounts' | 'signaturesForAddress'>;

export async function inspectProtocolHomeV1(client: ProtocolHomeRpc, deployment: DeploymentV1): Promise<ProtocolHomeV1> {
  const facts = await client.probe();
  const clusterCheck = deployment.genesisHash === null
    ? 'unpinned'
    : facts.genesisHash === deployment.genesisHash ? 'match' : 'mismatch';

  const addresses = PROTOCOL_ROLES_V1.map((role) => deployment.programs[role]);
  const observation = await client.multipleAccounts(addresses);
  const cards = PROTOCOL_ROLES_V1.map((role, index): ProtocolProgramCardV1 => {
    const entry = observation.accounts[index];
    const account = entry.account;
    return Object.freeze({
      role,
      address: entry.address,
      meaning: PROTOCOL_ROLE_MEANING_V1[role],
      status: account === null ? 'absent' : account.executable ? 'live' : 'not-executable',
      owner: account?.owner ?? null,
      ownerLabel: account === null ? null : LOADER_LABELS.get(account.owner) ?? null,
      lamports: account?.lamports ?? null,
      deploymentSlot: deployment.cluster === 'devnet' ? DEVNET_PROGRAM_EVIDENCE_V1[role].deploymentSlot : null,
    });
  });

  const bySignature = new Map<string, { row: Omit<ProtocolActivityRowV1, 'roles'>; roles: ProtocolRoleV1[] }>();
  const refusedHistories: ProtocolRoleV1[] = [];
  for (const role of PROTOCOL_ROLES_V1) {
    try {
      const records = await client.signaturesForAddress(deployment.programs[role], PROTOCOL_ACTIVITY_PER_PROGRAM);
      for (const record of records) {
        const existing = bySignature.get(record.signature);
        if (existing === undefined) {
          bySignature.set(record.signature, {
            row: {
              signature: record.signature,
              slot: record.slot,
              blockTime: record.blockTime,
              succeeded: record.succeeded,
              errorText: record.errorText,
            },
            roles: [role],
          });
        } else if (!existing.roles.includes(role)) {
          existing.roles.push(role);
        }
      }
    } catch {
      refusedHistories.push(role);
    }
  }
  const activity = [...bySignature.values()]
    .sort((left, right) => {
      const bySlot = BigInt(right.row.slot) - BigInt(left.row.slot);
      if (bySlot !== 0n) return bySlot > 0n ? 1 : -1;
      return left.row.signature.localeCompare(right.row.signature);
    })
    .slice(0, PROTOCOL_ACTIVITY_MAX_ROWS)
    .map((entry) => Object.freeze({ ...entry.row, roles: Object.freeze([...entry.roles]) }));

  const activityNote = refusedHistories.length > 0
    ? `This node refused the signature history for ${refusedHistories.join(', ')}; only the programs it answered for appear here.`
    : activity.length === 0
      ? 'This node lists no signature history for any of the seven programs. Another node may answer differently.'
      : `Newest first, from this node’s own per-address signature history over the seven programs.`;

  return Object.freeze({
    facts,
    clusterName: clusterNameV1(facts.genesisHash),
    clusterCheck,
    observedSlot: observation.slot,
    cards: Object.freeze(cards),
    activity: Object.freeze(activity),
    activityNote,
  });
}

export type SearchClassificationV1 =
  | Readonly<{ kind: 'account'; address: string }>
  | Readonly<{ kind: 'transaction'; signature: string }>
  | Readonly<{ kind: 'refused'; reason: string }>;

/**
 * One search box, two shapes: a 32-byte base58 address opens the account
 * view, a 64-byte base58 signature opens the transaction view. Nothing else
 * is guessed at.
 */
export function classifySearchV1(text: string): SearchClassificationV1 {
  const candidate = text.trim();
  if (candidate === '') return Object.freeze({ kind: 'refused', reason: 'Paste an address or a transaction signature.' });
  try {
    const address = new PublicKey(candidate).toBase58();
    if (address === candidate) return Object.freeze({ kind: 'account', address });
  } catch {
    // not an address; a signature is the other honest shape
  }
  if (/^[1-9A-HJ-NP-Za-km-z]{64,88}$/.test(candidate)) {
    return Object.freeze({ kind: 'transaction', signature: candidate });
  }
  return Object.freeze({
    kind: 'refused',
    reason: 'That is neither one canonical base58 address nor one base58 transaction signature.',
  });
}
