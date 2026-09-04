import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  DEVNET_DEPLOYMENT_V1,
  DEVNET_PROGRAM_EVIDENCE_V1,
  PROTOCOL_ROLES_V1,
  type ProtocolRoleV1,
} from './deployments';
import {
  LOADER_STATE_PROGRAM_DATA_V1,
  LOADER_STATE_PROGRAM_V1,
  PROGRAM_DATA_HEADER_BYTES_V1,
  UPGRADEABLE_LOADER_V1,
  describeDeploymentLivenessV1,
  readDeploymentLivenessV1,
} from './deploymentLiveness';
import { PUBLIC_DEVNET_CUT_V1, checkedReleaseSetIdsV1 } from './publicCutStaging';
import type { AccountInfoObservation, MultipleAccountObservation, RpcAccount } from './rpc';

import featuredMarket from '../fixtures/cohort15-featured-market.devnet.json';

/**
 * THE SHAPE `solana program close` LEAVES BEHIND, built by hand.
 *
 * Every case below is the same deployment read by a stub cluster, and the only
 * thing that varies is the one fact the two published defects turned on: does
 * the account that HOLDS THE CODE answer. The Program stubs are identical in
 * every case, because they are identical on a real closed cohort -- 36 bytes,
 * state tag 2, executable, loader-owned, still naming a ProgramData address
 * that no longer exists. A gate that reads only those cannot tell these cases
 * apart, and for two cohorts none of ours could.
 *
 * The market bytes are the REAL featured Market's, off devnet and unmodified
 * (`fixtures/cohort15-featured-market.devnet.json`). A synthetic 368-byte
 * DCLTCOR3 would mean forging the six nonzero identities, the phase/receipt
 * agreement and the principal cap that `decodeMarketCoreStateV2` refuses
 * without -- which is to say, forging the invariants the decoder exists to
 * check, and then testing the gate against a record no program wrote. Only
 * byte 208 is moved, and only in the case that is about byte 208.
 */

const MARKET_RELEASE_SET_OFFSET = 208;

function stub(programData: string): RpcAccount {
  const data = new Uint8Array(36);
  new DataView(data.buffer).setUint32(0, LOADER_STATE_PROGRAM_V1, true);
  data.set(new PublicKey(programData).toBytes(), 4);
  return Object.freeze({ data, executable: true, lamports: '1141440', owner: UPGRADEABLE_LOADER_V1, space: 36 });
}

function programDataHeader(deploymentSlot: bigint): RpcAccount {
  const data = new Uint8Array(PROGRAM_DATA_HEADER_BYTES_V1);
  const view = new DataView(data.buffer);
  view.setUint32(0, LOADER_STATE_PROGRAM_DATA_V1, true);
  view.setBigUint64(4, deploymentSlot, true);
  return Object.freeze({ data, executable: false, lamports: '8000000', owner: UPGRADEABLE_LOADER_V1, space: 1_193_400 });
}

function hexBytes(text: string): Uint8Array {
  return Uint8Array.from(text.match(/../g)!.map((pair) => Number.parseInt(pair, 16)));
}

/** The captured Market, with the selected release set optionally moved. */
function marketBytes(releaseSetIdHex: string | null): Uint8Array {
  const data = hexBytes(featuredMarket.dataHex);
  if (releaseSetIdHex !== null) data.set(hexBytes(releaseSetIdHex), MARKET_RELEASE_SET_OFFSET);
  return data;
}

type StubReads = Readonly<{
  /** null for a role means its ProgramData account is vacant. */
  vacant?: ReadonlyArray<ProtocolRoleV1>;
  marketOwner?: string;
  marketReleaseSetId?: string;
  marketAbsent?: boolean;
}>;

function stubClient(reads: StubReads) {
  const releaseSetId = reads.marketReleaseSetId ?? null;
  return {
    async multipleAccounts(addresses: ReadonlyArray<string>): Promise<MultipleAccountObservation> {
      return Object.freeze({
        slot: '492944410',
        accounts: Object.freeze(addresses.map((address, index) => Object.freeze({
          address,
          account: stub(DEVNET_PROGRAM_EVIDENCE_V1[PROTOCOL_ROLES_V1[index]].programData),
        }))),
      });
    },
    async multipleAccountDataSlices(addresses: ReadonlyArray<string>): Promise<MultipleAccountObservation> {
      return Object.freeze({
        slot: '492944410',
        accounts: Object.freeze(addresses.map((address, index) => Object.freeze({
          address,
          account: (reads.vacant ?? []).includes(PROTOCOL_ROLES_V1[index]) ? null : programDataHeader(492_745_516n + BigInt(index)),
        }))),
      });
    },
    async accountInfo(_address: string): Promise<AccountInfoObservation> {
      if (reads.marketAbsent === true) return Object.freeze({ slot: '492944410', account: null });
      return Object.freeze({
        slot: '492944410',
        account: Object.freeze({
          data: marketBytes(releaseSetId),
          executable: false,
          lamports: featuredMarket.lamports,
          owner: reads.marketOwner ?? DEVNET_DEPLOYMENT_V1.programs.core,
          space: featuredMarket.space,
        }),
      });
    },
  };
}

const shipped = {
  deployment: DEVNET_DEPLOYMENT_V1,
  evidence: DEVNET_PROGRAM_EVIDENCE_V1,
  market: PUBLIC_DEVNET_CUT_V1.market,
  checkedReleaseSetIds: checkedReleaseSetIdsV1(PUBLIC_DEVNET_CUT_V1),
} as const;

describe('the deployment liveness gate', () => {
  it('reads seven live programs and the release set the featured market itself carries', async () => {
    const liveness = await readDeploymentLivenessV1(stubClient({}), shipped);
    expect(liveness.status).toBe('alive');
    if (liveness.status !== 'alive') return;
    expect(liveness.roles).toHaveLength(PROTOCOL_ROLES_V1.length);
    expect(liveness.roles.every((row) => row.live)).toBe(true);
    expect(liveness.roles[0].deploymentSlot).toBe('492745516');
    // NOT a literal, and not the fixture's word either: the release set is read
    // out of the captured Market's own bytes, and the cut is what says it was
    // checked. The two meeting is the assertion.
    expect(checkedReleaseSetIdsV1(PUBLIC_DEVNET_CUT_V1)).toContain(liveness.marketReleaseSetId);
    expect(describeDeploymentLivenessV1(liveness)).toContain('ALIVE');
  });

  it('CALLS A CLOSED COHORT CLOSED, on stubs that answer every other question correctly', async () => {
    // This is the exact reading taken off devnet on 2026-09-02 and again on
    // 2026-09-04: all seven Program accounts alive and every ProgramData gone.
    const liveness = await readDeploymentLivenessV1(stubClient({ vacant: PROTOCOL_ROLES_V1 }), shipped);
    expect(liveness.status).toBe('closed');
    if (liveness.status !== 'closed') return;
    expect(liveness.closedRoles).toEqual([...PROTOCOL_ROLES_V1]);
    expect(liveness.roles.every((row) => row.live)).toBe(false);
    expect(liveness.reason).toContain('CLOSED');
    expect(describeDeploymentLivenessV1(liveness)).toContain('ProgramData VACANT');
  });

  it('refuses on ONE vacant role, because a partial close is still a dead deployment', async () => {
    const liveness = await readDeploymentLivenessV1(stubClient({ vacant: ['core'] }), shipped);
    expect(liveness.status).toBe('closed');
    if (liveness.status !== 'closed') return;
    expect(liveness.closedRoles).toEqual(['core']);
    expect(liveness.reason).toContain('core');
  });

  it('names a featured market that belongs to another cohort by its owner', async () => {
    const liveness = await readDeploymentLivenessV1(stubClient({ marketOwner: '9JW1qqJVeFo9ZRvzzVzNvqrwzt7QvyHpGafTJmj2hBFB' }), shipped);
    expect(liveness.status).toBe('refused');
    if (liveness.status !== 'refused') return;
    expect(liveness.reason).toContain('another cohort');
  });

  it('REFUSES A CHECKED-RELEASE TABLE ABOUT A SET THE MARKET DOES NOT SELECT', async () => {
    // The reverse conjunct. `stageCheckedReleaseV1` proves this once, from an
    // argument a human passes; a cut that outlives a cohort keeps saying a
    // release was checked for a market that no longer selects it.
    const liveness = await readDeploymentLivenessV1(stubClient({ marketReleaseSetId: 'ab'.repeat(32) }), shipped);
    expect(liveness.status).toBe('refused');
    if (liveness.status !== 'refused') return;
    expect(liveness.reason).toContain('a release this market does not select');
  });

  it('refuses an absent featured market rather than reporting on the programs alone', async () => {
    const liveness = await readDeploymentLivenessV1(stubClient({ marketAbsent: true }), shipped);
    expect(liveness.status).toBe('refused');
    if (liveness.status !== 'refused') return;
    expect(liveness.reason).toContain('does not exist');
  });

  it('refuses when the manifest records a ProgramData the Program stub does not name', async () => {
    const liveness = await readDeploymentLivenessV1(stubClient({}), {
      ...shipped,
      evidence: Object.freeze({
        ...DEVNET_PROGRAM_EVIDENCE_V1,
        core: Object.freeze({ ...DEVNET_PROGRAM_EVIDENCE_V1.core, programData: '11111111111111111111111111111111' }),
      }),
    });
    expect(liveness.status).toBe('refused');
    if (liveness.status !== 'refused') return;
    expect(liveness.reason).toContain('the manifest records 11111111111111111111111111111111');
  });
});
