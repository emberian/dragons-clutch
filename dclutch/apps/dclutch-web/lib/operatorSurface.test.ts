import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { CAPABILITY_ACTIONS_V1, evaluateCapabilityV1 } from './capabilityModel';
import { OPERATOR_ROLES, acquireOperatorSurfaceV1, type OperatorCoordinatesV1 } from './operatorSurface';
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
    expect(snapshot.realm).toBeNull();
    expect(snapshot.market).toMatchObject({ address: market, owner: coordinates.core, dataBytes: 184 });
  });

  it('refuses aliased role authority before any RPC read', async () => {
    const coordinates = Object.fromEntries(OPERATOR_ROLES.map((role) => [role, key(1)])) as OperatorCoordinatesV1;
    const client = { finalizedSlot: async () => { throw new Error('must not read'); } } as unknown as SolanaRpcClient;
    await expect(acquireOperatorSurfaceV1(client, coordinates)).rejects.toThrow(/distinct/);
  });

  it('has an explicit executable boundary for every exposed workflow', () => {
    expect(CAPABILITY_ACTIONS_V1.length).toBeGreaterThanOrEqual(20);
    expect(CAPABILITY_ACTIONS_V1.every((workflow) => workflow.exactBoundary.length > 40)).toBe(true);
    expect(CAPABILITY_ACTIONS_V1.filter((workflow) => workflow.implementation === 'browser-unsigned').every((workflow) => workflow.workspace !== null)).toBe(true);
    expect(CAPABILITY_ACTIONS_V1.some((workflow) => workflow.family === 'Direct' && workflow.implementation === 'awaiting-production')).toBe(true);
    expect(CAPABILITY_ACTIONS_V1.find((workflow) => workflow.action === 'Found common Core Market')).toMatchObject({
      implementation: 'browser-unsigned',
      workspace: '/found',
      exactBoundary: expect.stringContaining('Found31'),
    });
    const direct = CAPABILITY_ACTIONS_V1.find((workflow) => workflow.id === 'direct.inline');
    expect(direct && evaluateCapabilityV1(direct, null)).toMatchObject({ status: 'needs-chain' });
  });
});
