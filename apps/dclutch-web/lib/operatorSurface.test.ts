import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { OPERATOR_ROLES, OPERATOR_WORKFLOWS, acquireOperatorSurfaceV1, type OperatorCoordinatesV1 } from './operatorSurface';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

function key(byte: number): string { return new PublicKey(new Uint8Array(32).fill(byte)).toBase58(); }
function account(owner: string, executable: boolean, dataBytes = 36): RpcAccount {
  return Object.freeze({ data: new Uint8Array(dataBytes), executable, lamports: '1', owner, space: dataBytes });
}

describe('unified chain-derived operator surface', () => {
  it('reacquires six distinct executable roles and exact Core-owned Market state', async () => {
    const coordinates = Object.fromEntries(OPERATOR_ROLES.map((role, index) => [role, key(index + 1)])) as Record<(typeof OPERATOR_ROLES)[number], string>;
    const market = key(30);
    const client = {
      finalizedSlot: async () => '41',
      multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
        slot: '42',
        accounts: Object.freeze(addresses.map((address, index) => Object.freeze({
          address,
          account: index < OPERATOR_ROLES.length ? account(key(90), true) : account(coordinates.core, false, 184),
        }))),
      }),
    } as unknown as SolanaRpcClient;
    const snapshot = await acquireOperatorSurfaceV1(client, { ...coordinates, market } as OperatorCoordinatesV1);
    expect(snapshot.observedSlot).toBe('42');
    expect(snapshot.roles).toHaveLength(6);
    expect(snapshot.roles.every((role) => role.executable)).toBe(true);
    expect(snapshot.market).toMatchObject({ address: market, owner: coordinates.core, dataBytes: 184 });
  });

  it('refuses aliased role authority before any RPC read', async () => {
    const coordinates = Object.fromEntries(OPERATOR_ROLES.map((role) => [role, key(1)])) as OperatorCoordinatesV1;
    const client = { finalizedSlot: async () => { throw new Error('must not read'); } } as unknown as SolanaRpcClient;
    await expect(acquireOperatorSurfaceV1(client, coordinates)).rejects.toThrow(/distinct/);
  });

  it('has an explicit executable boundary for every exposed workflow', () => {
    expect(OPERATOR_WORKFLOWS.length).toBeGreaterThanOrEqual(24);
    expect(OPERATOR_WORKFLOWS.every((workflow) => workflow.exactBoundary.length > 40)).toBe(true);
    expect(OPERATOR_WORKFLOWS.filter((workflow) => workflow.status === 'constructible').every((workflow) => workflow.route !== null)).toBe(true);
    expect(OPERATOR_WORKFLOWS.some((workflow) => workflow.family === 'Dealer' && workflow.status === 'awaiting-abi')).toBe(true);
    expect(OPERATOR_WORKFLOWS.find((workflow) => workflow.action === 'Found common Core Market')).toMatchObject({
      status: 'constructible',
      route: '/found',
      exactBoundary: expect.stringContaining('Found31 accounts'),
    });
  });
});
