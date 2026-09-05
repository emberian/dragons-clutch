import { describe, expect, it } from 'vitest';

import { decodeBase58 } from './explorer/base58';
import {
  CORE_PHASE_OPEN_TAG,
  CORE_READINESS_PREPAID_TAG,
  CORE_STATE_BYTES,
  CORE_STATE_CAPABILITY_MANIFEST_OFFSET,
  CORE_STATE_GENERATION_OFFSET,
  CORE_STATE_IDENTITY_REALM_OFFSET,
  CORE_STATE_MAGIC,
  CORE_STATE_MARKET_ID_OFFSET,
  CORE_STATE_OUTSTANDING_CAPABILITIES_OFFSET,
  CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET,
  CORE_STATE_PHASE_OFFSET,
  CORE_STATE_PRODUCT_ID_OFFSET,
  CORE_STATE_PRODUCT_RECORD_OFFSET,
  CORE_STATE_READINESS_OFFSET,
  CORE_STATE_REGISTRY_PROGRAM_OFFSET,
  CORE_STATE_RENT_BENEFICIARY_OFFSET,
  CORE_STATE_RESOLUTION_POLICY_OFFSET,
  CORE_STATE_SELECTED_RELEASE_SET_OFFSET,
  CORE_STATE_VERSION_OFFSET,
  CORE_VERSION,
} from '@dclutch/sdk/generated/coreFound';
import { USER_POSITION_ADMISSION_SNAPSHOT_FORMAT_V1 } from './generated/userPositionAdmissionWasmV1';
import {
  ADMISSION_SNAPSHOT_ACCOUNT_FIELDS_V1,
  acquireUserPositionAdmissionSnapshotV1,
} from './userPositionAdmissionSnapshot';
import { type SolanaRpcClient } from '@dclutch/sdk/rpc';

/**
 * THE LAST UNIT BETWEEN A WALLET AND A TRADE.
 *
 * The planner is compiled and digest-pinned; what it authenticates is a
 * twenty-five-account finalized snapshot. Every address in it is DERIVED here
 * — from the Market's own Core state, from the Claims aggregate, from content
 * digests under the Registry's record PDA, and from the Loader — because a
 * snapshot a stranger pastes is a snapshot a stranger can be silently wrong
 * about, which is the same rule that took `/found` from fourteen pasted
 * addresses to ten.
 *
 * And every read is finalized at ONE floor. A snapshot stitched from several
 * observations, or from confirmed-but-not-finalized state, authenticates
 * against a chain that may not be the one that lands.
 */

const MARKET = 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG';
const OWNER = 'BPFLoaderUpgradeab1e11111111111111111111111';
const REGISTRY = '11111111111111111111111111111115';

/** A current-width Open Market, built from the generated constants. */
function coreState(): Uint8Array {
  const bytes = new Uint8Array(CORE_STATE_BYTES);
  const view = new DataView(bytes.buffer);
  bytes.set(CORE_STATE_MAGIC, 0);
  view.setUint16(CORE_STATE_VERSION_OFFSET, CORE_VERSION, true);
  bytes[CORE_STATE_PHASE_OFFSET] = CORE_PHASE_OPEN_TAG;
  bytes[CORE_STATE_READINESS_OFFSET] = CORE_READINESS_PREPAID_TAG;
  bytes.set(new Uint8Array(32).fill(2), CORE_STATE_MARKET_ID_OFFSET);
  bytes.set(new Uint8Array(32).fill(3), CORE_STATE_IDENTITY_REALM_OFFSET);
  bytes.set(new Uint8Array(32).fill(4), CORE_STATE_PRODUCT_RECORD_OFFSET);
  bytes.set(new Uint8Array(32).fill(10), CORE_STATE_PRODUCT_ID_OFFSET);
  bytes.set(new Uint8Array(32).fill(11), CORE_STATE_RESOLUTION_POLICY_OFFSET);
  bytes.set(new Uint8Array(32).fill(12), CORE_STATE_CAPABILITY_MANIFEST_OFFSET);
  bytes.set(new Uint8Array(32).fill(5), CORE_STATE_SELECTED_RELEASE_SET_OFFSET);
  bytes.set(decodeBase58(REGISTRY), CORE_STATE_REGISTRY_PROGRAM_OFFSET);
  view.setBigUint64(CORE_STATE_GENERATION_OFFSET, 7n, true);
  view.setBigUint64(CORE_STATE_OUTSTANDING_CAPABILITIES_OFFSET, 1n, true);
  view.setBigUint64(CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET, 1n, true);
  bytes.set(new Uint8Array(32).fill(13), CORE_STATE_RENT_BENEFICIARY_OFFSET);
  return bytes;
}

function productRecord(): Uint8Array {
  const bytes = new Uint8Array(112);
  bytes.set(new TextEncoder().encode('DCLTPRM2'), 0);
  new DataView(bytes.buffer).setUint16(8, 2, true);
  bytes.set(new Uint8Array(32).fill(1), 16);  // product identity
  bytes.set(new Uint8Array(32).fill(8), 48);  // result domain digest
  bytes.set(new Uint8Array(32).fill(9), 80);  // portfolio digest
  return bytes;
}

function aggregate(): Uint8Array {
  // WAS: this wrote a value at offset 152 and called it "linked basis digest".
  // Offset 152 is `basis_id`, the SEMANTIC LiabilityBasisV2 identity, and the
  // fixture made the module's belief true by construction — so the test could
  // never notice that the address it derived is vacant on a real market. The
  // aggregate is still read and still checked for presence; it just no longer
  // pretends to name a record.
  return new Uint8Array(280);
}

/**
 * The compiled planner's decoder, faked to the one call this module makes.
 *
 * The real one decodes `ProtocolPositionAdmissionV2`; a fake that returns a
 * fixed digest is enough to test the WIRING, and the live test is what proves
 * the digest is the right one.
 */
const derivation = Object.freeze({
  linked_basis_record_digest_v1: () => 'de'.repeat(32),
});

/** Serves any address; records what was asked, and at which floor. */
function client(asked: string[][], floors: (string | undefined)[]): SolanaRpcClient {
  const serve = (address: string) => ({
    address,
    account: {
      owner: '11111111111111111111111111111111',
      executable: false,
      lamports: '1000000',
      space: 0,
      data: address === MARKET ? coreState() : new Uint8Array(8),
    },
  });
  return {
    finalizedSlot: async () => '900',
    probe: async () => ({ genesisHash: MARKET, solanaCore: '2.0.0', endpoint: 'http://x', featureSetHash: '' }),
    blockTime: async () => '1790000000',
    ...SIZING_ROUND_V1,
    multipleAccounts: async (addresses: ReadonlyArray<string>, floor?: string) => {
      asked.push([...addresses]);
      floors.push(floor);
      return {
        slot: '900',
        accounts: addresses.map((address) => {
          const served = serve(address);
          if (asked.length === 2) {
            // The second read is the Product record, the Claims aggregate, and
            // this owner's admission record.
            const at = addresses.indexOf(address);
            served.account.data = at === 0 ? productRecord() : at === 1 ? aggregate() : new Uint8Array(512);
          }
          return served;
        }),
      };
    },
  } as unknown as SolanaRpcClient;
}

/**
 * The sizing round every client double now needs, and nothing more.
 *
 * `acquireFinalizedAccountsInChunksV1` learns each address's data length before
 * it splits, so a double that answers only `multipleAccounts` is no longer a
 * node. It is deliberately separate from the body reads each double records:
 * those count the ROUNDS this derivation makes, and a size decides only how one
 * round is split. A vacant answer plans every address at zero bytes, which is
 * the one chunk these fixtures were always read in.
 */
const SIZING_ROUND_V1 = {
  multipleAccountDataSlices: async (addresses: ReadonlyArray<string>) => ({
    slot: '900',
    accounts: addresses.map((address) => ({ address, account: null })),
  }),
};

const request = Object.freeze({
  market: MARKET,
  owner: OWNER,
  coreProgramId: '11111111111111111111111111111112',
  claimsProgramId: '11111111111111111111111111111113',
  tradingProgramId: '11111111111111111111111111111114',
  registryProgramId: REGISTRY,
  rentProgramId: '11111111111111111111111111111116',
  activationCache: '11111111111111111111111111111117',
});

describe('the linked-basis record is addressed by the digest that addresses it', () => {
  /**
   * A wallet with no admission record is TOLD, not guessed at.
   *
   * The record digest is named on chain in exactly one account, and a wallet
   * only has that account once it has been admitted. Before this, the module
   * derived an address from the aggregate's semantic `basis_id` — an address
   * nothing lives at on a real market — and the planner failed decoding empty
   * bytes. Refusing by name, with the remedy, is the honest shape.
   */
  it('refuses a first admission that was not told the linked-basis digest', async () => {
    const vacantAdmission = {
      finalizedSlot: async () => '900',
      probe: async () => ({ genesisHash: MARKET, solanaCore: '2.0.0', endpoint: 'http://x', featureSetHash: '' }),
      blockTime: async () => '1790000000',
      ...SIZING_ROUND_V1,
      multipleAccounts: async (addresses: ReadonlyArray<string>) => ({
        slot: '900',
        accounts: addresses.map((address, index) => ({
          address,
          // The third account of the second read is the admission record, and
          // this wallet has none.
          account: addresses.length === 3 && index === 2 ? null : {
            owner: '11111111111111111111111111111111', executable: false, lamports: '1000000', space: 0,
            data: address === MARKET ? coreState() : addresses.length === 3 && index === 0 ? productRecord() : aggregate(),
          },
        })),
      }),
    } as unknown as SolanaRpcClient;
    await expect(acquireUserPositionAdmissionSnapshotV1(vacantAdmission, request, derivation))
      .rejects.toThrow(/has no admission record .* yet, and the linked-basis record digest is named on chain only inside an admission record/);
  });

  it('accepts a first admission that was told, and refuses a malformed digest', async () => {
    const vacantAdmission = {
      finalizedSlot: async () => '900',
      probe: async () => ({ genesisHash: MARKET, solanaCore: '2.0.0', endpoint: 'http://x', featureSetHash: '' }),
      blockTime: async () => '1790000000',
      ...SIZING_ROUND_V1,
      multipleAccounts: async (addresses: ReadonlyArray<string>) => ({
        slot: '900',
        accounts: addresses.map((address, index) => ({
          address,
          account: addresses.length === 3 && index === 2 ? null : {
            owner: '11111111111111111111111111111111', executable: false, lamports: '1000000', space: 0,
            data: address === MARKET ? coreState() : addresses.length === 3 && index === 0 ? productRecord() : aggregate(),
          },
        })),
      }),
    } as unknown as SolanaRpcClient;
    const told = Object.freeze({ ...request, linkedBasisRecordDigest: 'ab'.repeat(32) });
    await expect(acquireUserPositionAdmissionSnapshotV1(vacantAdmission, told, derivation)).resolves.toBeDefined();
    const nonsense = Object.freeze({ ...request, linkedBasisRecordDigest: 'NOT-HEX' });
    await expect(acquireUserPositionAdmissionSnapshotV1(vacantAdmission, nonsense, derivation))
      .rejects.toThrow(/linked-basis record digest is named on chain only inside an admission record/);
  });
});

describe('the admission snapshot is derived and finalized', () => {
  it('carries exactly the twenty-five accounts the planner authenticates', async () => {
    const asked: string[][] = [];
    const floors: (string | undefined)[] = [];
    const acquired = await acquireUserPositionAdmissionSnapshotV1(client(asked, floors), request, derivation);
    const snapshot = JSON.parse(acquired.snapshotJson) as Record<string, unknown>;
    expect(snapshot.format).toBe(USER_POSITION_ADMISSION_SNAPSHOT_FORMAT_V1);
    // Field-for-field with the Rust snapshot. A missing one is an input the
    // planner authenticates and this transport silently dropped.
    expect(ADMISSION_SNAPSHOT_ACCOUNT_FIELDS_V1.length).toBe(25);
    for (const field of ADMISSION_SNAPSHOT_ACCOUNT_FIELDS_V1) {
      expect(snapshot[field], `${field} is absent from the snapshot`).toBeDefined();
    }
    expect(Object.keys(snapshot).sort()).toEqual([...ADMISSION_SNAPSHOT_ACCOUNT_FIELDS_V1, 'format', 'genesisHash'].sort());
  });

  it('reads every account at one finalized floor, never several', async () => {
    // The defect this refuses: a snapshot stitched from several observations
    // authenticates a chain that never existed at any single moment.
    const asked: string[][] = [];
    const floors: (string | undefined)[] = [];
    await acquireUserPositionAdmissionSnapshotV1(client(asked, floors), request, derivation);
    expect(floors.length).toBeGreaterThan(1);
    for (const floor of floors) expect(floor).toBe('900');
  });

  it('derives every address rather than accepting one', async () => {
    const asked: string[][] = [];
    const floors: (string | undefined)[] = [];
    const acquired = await acquireUserPositionAdmissionSnapshotV1(client(asked, floors), request, derivation);
    // The request carries a Market, an owner, and this deployment's programs.
    // Everything else in the frame is computed.
    expect(Object.keys(request).length).toBe(8);
    const addresses = asked[asked.length - 1];
    expect(addresses.length).toBe(25);
    expect(new Set(addresses).size).toBe(25);
    expect(addresses).toContain(MARKET);
    expect(addresses).toContain(acquired.derived.position);
    expect(addresses).toContain(acquired.derived.admission);
    expect(addresses).toContain(acquired.derived.claimsAggregate);
    expect(addresses).toContain(acquired.derived.rentCredit);
  });

  it('carries the genesis hash as the planner reads it, not as base58', async () => {
    const acquired = await acquireUserPositionAdmissionSnapshotV1(client([], []), request, derivation);
    const snapshot = JSON.parse(acquired.snapshotJson) as Readonly<{ genesisHash: string }>;
    const bytes = Uint8Array.from(atob(snapshot.genesisHash), (one) => one.charCodeAt(0));
    expect(bytes.length).toBe(32);
    expect([...bytes]).toEqual([...decodeBase58(MARKET)]);
  });

  it('refuses a Market whose Core state is not the exact ABI', async () => {
    const broken = {
      finalizedSlot: async () => '900',
      probe: async () => ({ genesisHash: MARKET, solanaCore: '2.0.0', endpoint: 'http://x', featureSetHash: '' }),
      blockTime: async () => '1790000000',
      ...SIZING_ROUND_V1,
      multipleAccounts: async (addresses: ReadonlyArray<string>) => ({
        slot: '900',
        accounts: addresses.map((address) => ({
          address,
          account: { owner: '11111111111111111111111111111111', executable: false, lamports: '1', space: 0, data: new Uint8Array(360) },
        })),
      }),
    } as unknown as SolanaRpcClient;
    await expect(acquireUserPositionAdmissionSnapshotV1(broken, request, derivation))
      .rejects.toThrow(/Core Market state has the wrong exact ABI/);
  });

  it('refuses an absent Market rather than snapshotting a hole', async () => {
    const empty = {
      finalizedSlot: async () => '900',
      probe: async () => ({ genesisHash: MARKET, solanaCore: '2.0.0', endpoint: 'http://x', featureSetHash: '' }),
      blockTime: async () => '1790000000',
      ...SIZING_ROUND_V1,
      multipleAccounts: async (addresses: ReadonlyArray<string>) => ({
        slot: '900',
        accounts: addresses.map((address) => ({ address, account: null })),
      }),
    } as unknown as SolanaRpcClient;
    await expect(acquireUserPositionAdmissionSnapshotV1(empty, request, derivation))
      .rejects.toThrow(/Core Market .* is absent at finalized commitment/);
  });
});
