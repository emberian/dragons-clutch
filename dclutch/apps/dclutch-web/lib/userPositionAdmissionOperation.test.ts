import { ComputeBudgetProgram, PublicKey, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT_V1 } from '@dclutch/sdk/generated/genericFoundingV1';
import { SOLANA_PACKET_BYTES_V1 } from '@dclutch/sdk/solanaLimits';
import { type DirectParticipantReadinessV1 } from '@dclutch/sdk/directParticipant';
import { type SignatureStatusObservation } from '@dclutch/sdk/rpc';
import { USER_POSITION_ADMISSION_PLAN_FORMAT_V1, USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1, USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1 } from './generated/userPositionAdmissionWasmV1';
import { compileUserPositionAdmissionTransactionV1, requireFinalizedAdmissionPoststateV1 } from './userPositionAdmissionOperation';
import { parseUserPositionAdmissionPlanV1 } from './userPositionAdmissionV1';

const BLOCKHASH = 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG';

/** A distinct, valid, deterministic account key per frame coordinate. */
function distinctKey(index: number): string {
  const bytes = new Uint8Array(32);
  new DataView(bytes.buffer).setUint32(0, index + 1, true);
  return new PublicKey(bytes).toBase58();
}
const OWNER = '11111111111111111111111111111112';

function plan(accountCount = 6): string {
  return JSON.stringify({
    format: USER_POSITION_ADMISSION_PLAN_FORMAT_V1,
    accountCount: USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1,
    ownerAccountIndex: USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1,
    requiredSigner: OWNER,
    observedSlot: '900',
    position: '11111111111111111111111111111113',
    admission: '11111111111111111111111111111114',
    positionTopUpLamports: '2000',
    admissionTopUpLamports: '0',
    instructions: [{
      programId: '11111111111111111111111111111115',
      dataBase64: 'AAAA',
      accounts: Array.from({ length: accountCount }, (_, index) => ({
        // Distinct, valid, deterministic keys.
        pubkey: index === 0 ? OWNER : distinctKey(index),
        isSigner: index === 0,
        isWritable: index < 2,
      })),
    }],
  });
}

/**
 * The planner answers with instructions; a wallet signs a TRANSACTION. This is
 * the one step between them, and the only judgement it makes is refusing a
 * packet that cannot fly — named, with both numbers, rather than handed to a
 * wallet to fail opaquely at submission.
 */
describe('compiling the admission transaction', () => {
  it('compiles the planner instructions with the owner as sole signer', () => {
    const compiled = compileUserPositionAdmissionTransactionV1(
      parseUserPositionAdmissionPlanV1(plan()), { payer: OWNER, recentBlockhash: BLOCKHASH });
    expect(compiled.requiredSigners).toEqual([OWNER]);
    expect(compiled.wireBytes.length).toBeGreaterThan(0);
    expect(compiled.wireBytes.length).toBeLessThanOrEqual(SOLANA_PACKET_BYTES_V1);
    const decoded = VersionedTransaction.deserialize(compiled.wireBytes);
    expect(decoded.message.compiledInstructions).toHaveLength(compiled.plan.instructions.length + 1);
    const allowance = decoded.message.compiledInstructions[0]!;
    expect(decoded.message.staticAccountKeys[allowance.programIdIndex]?.toBase58())
      .toBe(ComputeBudgetProgram.programId.toBase58());
    expect(Array.from(allowance.data)).toEqual(Array.from(ComputeBudgetProgram.setComputeUnitLimit({
      units: LOCAL_PROTOCOL_COMPUTE_UNIT_LIMIT_V1,
    }).data));
  });

  it('refuses a payer who is not the planner’s required signer', () => {
    // The planner names exactly one signer. A transaction paid by anyone else
    // is a different transaction than the one that was authenticated.
    expect(() => compileUserPositionAdmissionTransactionV1(
      parseUserPositionAdmissionPlanV1(plan()), { payer: '11111111111111111111111111111119', recentBlockhash: BLOCKHASH }))
      .toThrow(/admission payer is not the planner’s required signer/);
  });

  it('refuses an oversized packet by name, with both numbers', () => {
    // A frame that does not fit is a real outcome here: the outer is 27
    // accounts before the two funding transfers. Saying so beats a wallet
    // failing at submission with nothing a reader can act on.
    let failed = '';
    try {
      compileUserPositionAdmissionTransactionV1(
        parseUserPositionAdmissionPlanV1(plan(120)), { payer: OWNER, recentBlockhash: BLOCKHASH });
    } catch (error) { failed = error instanceof Error ? error.message : ''; }
    expect(failed).toMatch(/admission transaction does not fit Solana’s 1,232-byte packet bound: the frame reached \d+ distinct accounts/);
  });
});

describe('admission completion evidence', () => {
  const prepared = Object.freeze({
    derived: Object.freeze({ position: distinctKey(31), admission: distinctKey(32), claimsAggregate: distinctKey(33), rentCredit: distinctKey(34), generation: '7' }),
  });
  const ready: DirectParticipantReadinessV1 = Object.freeze({
    status: 'ready' as const,
    observedSlot: '901', market: distinctKey(35), generation: 7n, owner: OWNER,
    coordinates: Object.freeze({ aggregate: distinctKey(33), position: prepared.derived.position, admission: prepared.derived.admission, collateral: distinctKey(36), custodyAuthority: distinctKey(37) }),
    collateralMint: distinctKey(38), tokenProgram: distinctKey(39), positionRevision: 0n,
    positionBalances: Object.freeze([0n, 0n]), collateralAtoms: 0n, delegatedCollateralAtoms: 0n,
    spendableCollateralAtoms: 0n, reason: 'authenticated',
  });
  const status = (confirmationStatus: string | null): SignatureStatusObservation => Object.freeze({
    signature: 'signature', known: true, slot: '901', confirmationStatus, succeeded: true, errorText: null,
  });

  it('requires a successful finalized signature and the exact derived accounts', () => {
    expect(() => requireFinalizedAdmissionPoststateV1(status('confirmed'), ready, prepared))
      .toThrow(/not a successful finalized transaction/);
    expect(requireFinalizedAdmissionPoststateV1(status('finalized'), ready, prepared).observedSlot).toBe('901');
  });

  it('does not clear a journal when the finalized read names another admission', () => {
    const substituted: DirectParticipantReadinessV1 = Object.freeze({
      ...ready,
      coordinates: Object.freeze({ ...ready.coordinates, admission: distinctKey(40) }),
    });
    expect(() => requireFinalizedAdmissionPoststateV1(status('finalized'), substituted, prepared))
      .toThrow(/different Position or admission coordinates/);
  });
});
