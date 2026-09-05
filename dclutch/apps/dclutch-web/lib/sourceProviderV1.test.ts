import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { SOURCE_PROVIDER_PLAN_FORMAT_V1, SOURCE_PROVIDER_SUBMIT_PLAN_FORMAT_V1 } from '@dclutch/sdk/generated/sourceProviderWasmV1';
import {
  parseSourceProviderReclaimPlanV1,
  parseSourceProviderSubmitPlanV1,
  sourceProviderReclaimPoststateCompletesV1,
} from './sourceProviderV1';

const address = (byte: number) => new PublicKey(new Uint8Array(32).fill(byte)).toBase58();

function planJson() {
  const completion = [address(1), address(2), address(3), address(4)];
  return JSON.stringify({
    format: SOURCE_PROVIDER_PLAN_FORMAT_V1,
    route: 'reclaim',
    observedSlot: '9',
    instruction: {
      program: address(5),
      accounts: Array.from({ length: 18 }, (_, index) => ({ address: address(20 + index), isSigner: index < 2, isWritable: index < 4 })),
      dataBase64: 'AQ==',
    },
    unsignedMessageBase64: 'AQ==',
    requiredSigners: [address(6), address(7)],
    wireBytes: 100,
    loadedAddresses: 0,
    lookupTables: [],
    lifecycle: completion[0],
    updateAuthority: completion[2],
    completion,
    expectedPoststates: completion.map((entry, index) => ({
      address: entry,
      owner: PublicKey.default.toBase58(),
      lamports: index === 3 ? '144' : '0',
      executable: false,
      dataBase64: '',
    })),
  });
}

function submitPlanJson() {
  const lifecycle = address(40);
  const update = address(41);
  return JSON.stringify({
    format: SOURCE_PROVIDER_SUBMIT_PLAN_FORMAT_V1,
    route: 'submit',
    observedSlot: '10',
    instruction: {
      program: address(42),
      accounts: Array.from({ length: 38 }, (_, index) => ({ address: address(50 + index), isSigner: index < 2, isWritable: index < 3 })),
      dataBase64: 'AQ==',
    },
    unsignedMessageBase64: 'AQ==',
    requiredSigners: [address(43), update],
    wireBytes: 900,
    loadedAddresses: 24,
    lookupTables: [address(44)],
    lifecycleTopUpLamports: '123',
    completion: [lifecycle, update],
    poststate: {
      lifecycle,
      updateAccount: update,
      updateAuthority: address(45),
      resolutionProgram: address(42),
      receiverProgram: address(46),
      submitRequestBase64: 'AQ==',
    },
  });
}

describe('Source provider browser contract', () => {
  it('strictly parses one 18-account, two-signer, four-poststate plan', () => {
    const plan = parseSourceProviderReclaimPlanV1(planJson());
    expect(plan.instruction.accounts).toHaveLength(18);
    expect(plan.requiredSigners).toHaveLength(2);
    expect(plan.expectedPoststates).toHaveLength(4);
    const changed = JSON.parse(planJson()) as Record<string, unknown>;
    changed.extra = true;
    expect(() => parseSourceProviderReclaimPlanV1(JSON.stringify(changed))).toThrow(/unknown fields/);
  });

  it('clears only when every projected finalized poststate matches exactly', async () => {
    const plan = parseSourceProviderReclaimPlanV1(planJson());
    const accounts = plan.expectedPoststates.map((expected, index) => ({
      address: expected.address,
      account: index < 3 ? null : {
        owner: expected.owner,
        lamports: expected.lamports,
        executable: expected.executable,
        data: new Uint8Array(),
      },
    }));
    const client = { multipleAccounts: async () => ({ slot: '10', accounts }) };
    await expect(sourceProviderReclaimPoststateCompletesV1(client as never, plan, '10')).resolves.toBe(true);
    const changed = accounts.map((entry, index) => index === 3 && entry.account !== null
      ? { ...entry, account: { ...entry.account, lamports: '145' } }
      : entry);
    await expect(sourceProviderReclaimPoststateCompletesV1({
      multipleAccounts: async () => ({ slot: '10', accounts: changed }),
    } as never, plan, '10')).resolves.toBe(false);
  });

  it('refuses account-order substitution even when the same accounts are returned', async () => {
    const plan = parseSourceProviderReclaimPlanV1(planJson());
    const reversed = [...plan.expectedPoststates].reverse().map((expected) => ({ address: expected.address, account: null }));
    await expect(sourceProviderReclaimPoststateCompletesV1({
      multipleAccounts: async () => ({ slot: '10', accounts: reversed }),
    } as never, plan, '10')).resolves.toBe(false);
  });

  it('strictly parses the table-backed 38-account submit geometry', () => {
    const plan = parseSourceProviderSubmitPlanV1(submitPlanJson());
    expect(plan.instruction.accounts).toHaveLength(38);
    expect(plan.requiredSigners).toHaveLength(2);
    expect(plan.lookupTables).toHaveLength(1);
    expect(plan.completion).toHaveLength(2);
    const changed = JSON.parse(submitPlanJson()) as Record<string, unknown>;
    changed.lookupTables = [];
    expect(() => parseSourceProviderSubmitPlanV1(JSON.stringify(changed))).toThrow(/exactly one frozen table/);
  });
});
