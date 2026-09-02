import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { GRADED_BASIS_RECORD_SCHEMA_ID_V3 } from './generated/coreFound';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { SolanaRpcClient } from './rpc';
import { loadUserPositionAdmissionWasmV1 } from './userPositionAdmissionV1';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

/**
 * The linked-basis record address, derived and then OCCUPIED.
 *
 * The regression this pins: the admission frame used to address this record
 * from the Claims aggregate's `basis_id`, which is the semantic LiabilityBasisV2
 * identity — it authenticates a basis body and cannot address one. On this
 * market that derivation lands on a vacant account. The digest that does
 * address it is named in exactly one place on chain,
 * `ProtocolPositionAdmissionEvidenceV2`, and the assertion here is not that
 * some address comes out but that the address holds the record.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1`; `DCLUTCH_LIVE_ENDPOINT` supplies a key from
 * the environment so none is written down.
 */
const COHORT_11 = Object.freeze({
  endpoint: process.env.DCLUTCH_LIVE_ENDPOINT ?? 'https://api.devnet.solana.com',
  registry: 'ADB72ar6ZSstXEg76Q1bPb5UY2EGmH6mrVfwr8K2fzom',
  // The founder's admission record on market `3rBfDBpa…`, a PDA of that
  // market's Claims aggregate and the founder wallet.
  admission: '3HyBinfqDZ9WBEdyUEfB6Mz3TSfSJBemT2dJHoVyRNRj',
  aggregate: '5wdhigoUUNDaQFjqBmVUTmyh5ihqjxUNV6sdaNt6izxE',
  // What the campaign report recorded for `linked_liability_basis_record`.
  linkedBasisRecord: 'HprHEBnudyLmbJkUSQ8US7B7tAvZ42Sc7XY7QCbTab9v',
});

describe('live devnet linked-basis record addressing', () => {
  live('derives an OCCUPIED record from the admission evidence, where basis_id derives a vacant one', async () => {
    const client = new SolanaRpcClient(COHORT_11.endpoint);
    const planner = await loadUserPositionAdmissionWasmV1(async () => new Response(
      readFileSync(new URL('./generated/userPositionAdmissionWasm/user_position_admission_bg.wasm', import.meta.url)),
    ));
    const floor = await client.finalizedSlot();
    const observed = await client.multipleAccounts([COHORT_11.admission, COHORT_11.aggregate], floor);
    const admission = observed.accounts[0]?.account;
    const aggregate = observed.accounts[1]?.account;
    expect(admission, 'the founder admission record').not.toBeNull();
    expect(aggregate, 'the Claims aggregate').not.toBeNull();

    let binary = '';
    for (const byte of admission!.data) binary += String.fromCharCode(byte);
    const digest = planner.linked_basis_record_digest_v1(btoa(binary));
    expect(digest).toMatch(/^[0-9a-f]{64}$/);

    const fromEvidence = deriveFinalizedRecordAddressesV1(
      COHORT_11.registry, GRADED_BASIS_RECORD_SCHEMA_ID_V3, Uint8Array.from(
        (digest.match(/../g) ?? []).map((pair) => Number.parseInt(pair, 16)),
      ),
    );
    expect(fromEvidence.record).toBe(COHORT_11.linkedBasisRecord);

    // The address the OLD derivation produced, and the point of the fix.
    const fromSemanticIdentity = deriveFinalizedRecordAddressesV1(
      COHORT_11.registry, GRADED_BASIS_RECORD_SCHEMA_ID_V3, aggregate!.data.slice(152, 184),
    );
    expect(fromSemanticIdentity.record).not.toBe(COHORT_11.linkedBasisRecord);

    const both = await client.multipleAccounts([fromEvidence.record, fromSemanticIdentity.record], floor);
    expect(both.accounts[0]?.account, 'the record the admission evidence addresses').not.toBeNull();
    expect(both.accounts[1]?.account, 'the record basis_id addresses').toBeNull();
  }, 120_000);
});
