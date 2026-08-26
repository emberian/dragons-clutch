import { PublicKey } from '@solana/web3.js';

import { classifyHeader } from './decoders';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

export const OPERATOR_ROLES = ['registry', 'core', 'trading', 'claims', 'custody', 'resolution'] as const;
export type OperatorRole = (typeof OPERATOR_ROLES)[number];

export type OperatorCoordinatesV1 = Readonly<Record<OperatorRole, string>> & Readonly<{
  market?: string;
  realm?: string;
}>;

export type OperatorRoleObservationV1 = Readonly<{
  role: OperatorRole;
  address: string;
  owner: string;
  executable: boolean;
  dataBytes: number;
}>;

export type OperatorSurfaceSnapshotV1 = Readonly<{
  observedSlot: string;
  roles: ReadonlyArray<OperatorRoleObservationV1>;
  realm: null | Readonly<{
    address: string;
    owner: string;
    dataBytes: number;
    header: string | null;
  }>;
  market: null | Readonly<{
    address: string;
    owner: string;
    dataBytes: number;
    header: string | null;
  }>;
}>;

function canonicalKey(value: string, field: string): string {
  const key = new PublicKey(value);
  if (key.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return value;
}

function requireAccount(account: RpcAccount | null, field: string): RpcAccount {
  if (account === null) throw new Error(`${field} is absent at the finalized observation floor`);
  return account;
}

export async function acquireOperatorSurfaceV1(
  client: SolanaRpcClient,
  coordinates: OperatorCoordinatesV1,
): Promise<OperatorSurfaceSnapshotV1> {
  const roleAddresses = OPERATOR_ROLES.map((role) => canonicalKey(coordinates[role], `${role} program`));
  if (new Set(roleAddresses).size !== roleAddresses.length) throw new Error('multiprogram roles must have distinct executable program identities');
  const market = coordinates.market === undefined || coordinates.market === ''
    ? null
    : canonicalKey(coordinates.market, 'Market');
  const realm = coordinates.realm === undefined || coordinates.realm === ''
    ? null
    : canonicalKey(coordinates.realm, 'Realm');
  const stateAddresses = [realm, market].filter((address): address is string => address !== null);
  if (stateAddresses.some((address) => roleAddresses.includes(address))) throw new Error('Realm or Market aliases an executable program role');
  if (new Set(stateAddresses).size !== stateAddresses.length) throw new Error('Realm and Market must have distinct state identities');
  const floor = await client.finalizedSlot();
  const addresses = [...roleAddresses, ...stateAddresses];
  const observation = await client.multipleAccounts(addresses, floor);
  const roles = OPERATOR_ROLES.map((role, index) => {
    const account = requireAccount(observation.accounts[index].account, `${role} program`);
    if (!account.executable) throw new Error(`${role} program is not executable`);
    return Object.freeze({
      role,
      address: roleAddresses[index],
      owner: account.owner,
      executable: account.executable,
      dataBytes: account.data.length,
    });
  });
  let nextStateIndex = roleAddresses.length;
  let realmObservation: OperatorSurfaceSnapshotV1['realm'] = null;
  if (realm !== null) {
    const account = requireAccount(observation.accounts[nextStateIndex].account, 'Realm');
    nextStateIndex += 1;
    if (account.executable) throw new Error('Realm is executable');
    if (account.owner !== coordinates.core) throw new Error('Realm is not owned by the selected Core program');
    realmObservation = Object.freeze({
      address: realm,
      owner: account.owner,
      dataBytes: account.data.length,
      header: classifyHeader(account.data),
    });
  }
  let marketObservation: OperatorSurfaceSnapshotV1['market'] = null;
  if (market !== null) {
    const account = requireAccount(observation.accounts[nextStateIndex].account, 'Market');
    if (account.executable) throw new Error('Market is executable');
    if (account.owner !== coordinates.core) throw new Error('Market is not owned by the selected Core program');
    marketObservation = Object.freeze({
      address: market,
      owner: account.owner,
      dataBytes: account.data.length,
      header: classifyHeader(account.data),
    });
  }
  return Object.freeze({ observedSlot: observation.slot, roles: Object.freeze(roles), realm: realmObservation, market: marketObservation });
}
