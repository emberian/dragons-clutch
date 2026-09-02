import { describe, expect, it } from 'vitest';

import { DEVNET_PROGRAM_EVIDENCE_V1, DEVNET_DEPLOYMENT_V1, LOCAL_DEPLOYMENT_V1, PROTOCOL_ROLES_V1 } from '../deployments';
import { type MultipleAccountObservation, type RpcAccount, type SignatureRecordObservation } from '../rpc';
import { classifySearchV1, inspectProtocolHomeV1 } from './protocolHome';

const FACTS = Object.freeze({
  endpoint: 'https://api.devnet.solana.com/',
  genesisHash: 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG',
  solanaCore: '2.3.9',
  featureSet: '1',
});

function programAccount(executable: boolean): RpcAccount {
  return Object.freeze({
    data: new Uint8Array(36),
    executable,
    lamports: '1141440',
    owner: 'BPFLoaderUpgradeab1e11111111111111111111111',
    space: 36,
  });
}

function signature(fill: number): string {
  return '4'.repeat(87) + String((fill % 9) + 1);
}

function record(fill: number, slot: string, succeeded = true): SignatureRecordObservation {
  return Object.freeze({
    signature: signature(fill),
    slot,
    succeeded,
    errorText: succeeded ? null : '{"InstructionError":[0,{"Custom":1}]}',
    blockTime: '1790000000',
    memo: null,
  });
}

function fakeClient(overrides?: Readonly<{
  absent?: ReadonlyArray<string>;
  histories?: ReadonlyMap<string, ReadonlyArray<SignatureRecordObservation>>;
  refuseHistory?: ReadonlyArray<string>;
}>) {
  return {
    probe: async () => FACTS,
    multipleAccounts: async (addresses: ReadonlyArray<string>): Promise<MultipleAccountObservation> => Object.freeze({
      slot: '489200000',
      accounts: Object.freeze(addresses.map((address) => Object.freeze({
        address,
        account: overrides?.absent?.includes(address) ? null : programAccount(true),
      }))),
    }),
    signaturesForAddress: async (address: string): Promise<ReadonlyArray<SignatureRecordObservation>> => {
      if (overrides?.refuseHistory?.includes(address)) throw new Error('getSignaturesForAddress refused: long-term storage disabled');
      return overrides?.histories?.get(address) ?? Object.freeze([]);
    },
  };
}

describe('the explorer protocol home', () => {
  it('reads all seven programs live at one observation and names the cluster from its genesis', async () => {
    const home = await inspectProtocolHomeV1(fakeClient(), DEVNET_DEPLOYMENT_V1);
    expect(home.cards).toHaveLength(7);
    expect(home.cards.map((card) => card.role)).toEqual([...PROTOCOL_ROLES_V1]);
    expect(home.cards.every((card) => card.status === 'live')).toBe(true);
    expect(home.cards.every((card) => card.ownerLabel === 'upgradeable loader')).toBe(true);
    expect(home.clusterName).toBe('devnet');
    expect(home.clusterCheck).toBe('match');
    expect(home.observedSlot).toBe('489200000');
    // Devnet cards carry the SHIPPED evidence table's recorded slots. Read from
    // the record rather than restated: a literal here pinned cohort-8's slot and
    // aged out the day devnet redeployed, and the claim worth making is that the
    // card reports the manifest's slot, not that the manifest holds one number.
    expect(home.cards.find((card) => card.role === 'core')?.deploymentSlot)
      .toBe(DEVNET_PROGRAM_EVIDENCE_V1.core.deploymentSlot);
  });

  it('reports an absent program as absent, never as an empty success', async () => {
    const home = await inspectProtocolHomeV1(
      fakeClient({ absent: [DEVNET_DEPLOYMENT_V1.programs.trading] }),
      DEVNET_DEPLOYMENT_V1,
    );
    expect(home.cards.find((card) => card.role === 'trading')?.status).toBe('absent');
    expect(home.cards.filter((card) => card.status === 'live')).toHaveLength(6);
  });

  it('merges per-program histories by signature, newest first, naming every touching role', async () => {
    const histories = new Map([
      [DEVNET_DEPLOYMENT_V1.programs.core, [record(1, '100'), record(2, '300')]],
      [DEVNET_DEPLOYMENT_V1.programs.claims, [record(2, '300'), record(3, '200', false)]],
    ]);
    const home = await inspectProtocolHomeV1(fakeClient({ histories }), DEVNET_DEPLOYMENT_V1);
    expect(home.activity.map((row) => row.slot)).toEqual(['300', '200', '100']);
    // Roles arrive in the canonical seven-role order the merge walks.
    expect(home.activity[0].roles).toEqual(['claims', 'core']);
    expect(home.activity[1].succeeded).toBe(false);
    expect(home.activityNote).toContain('this node’s own per-address signature history');
  });

  it('says when a node refused a history rather than presenting a shorter list as complete', async () => {
    const home = await inspectProtocolHomeV1(
      fakeClient({ refuseHistory: [DEVNET_DEPLOYMENT_V1.programs.registry] }),
      DEVNET_DEPLOYMENT_V1,
    );
    expect(home.activityNote).toContain('refused the signature history for registry');
  });

  it('marks the cluster check unpinned for a deployment with no expected genesis, and mismatch for the wrong one', async () => {
    const local = await inspectProtocolHomeV1(fakeClient(), LOCAL_DEPLOYMENT_V1);
    expect(local.clusterCheck).toBe('unpinned');
    const wrong = await inspectProtocolHomeV1(
      { ...fakeClient(), probe: async () => Object.freeze({ ...FACTS, genesisHash: '5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d' }) },
      DEVNET_DEPLOYMENT_V1,
    );
    expect(wrong.clusterCheck).toBe('mismatch');
    expect(wrong.clusterName).toBe('mainnet-beta');
  });
});

describe('search classification', () => {
  it('classifies one canonical address as an account', () => {
    expect(classifySearchV1(` ${DEVNET_DEPLOYMENT_V1.programs.core} `)).toEqual({ kind: 'account', address: DEVNET_DEPLOYMENT_V1.programs.core });
  });

  it('classifies one 64-byte base58 signature as a transaction', () => {
    expect(classifySearchV1(signature(5))).toEqual({ kind: 'transaction', signature: signature(5) });
  });

  it('refuses everything else with the two honest shapes named', () => {
    const refused = classifySearchV1('what is a market');
    expect(refused.kind).toBe('refused');
    const empty = classifySearchV1('   ');
    expect(empty.kind).toBe('refused');
  });
});
