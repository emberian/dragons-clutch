import { describe, expect, it } from 'vitest';

import {
  USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1,
  USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1,
  USER_POSITION_ADMISSION_PLAN_FORMAT_V1,
  USER_POSITION_ADMISSION_SNAPSHOT_FORMAT_V1,
  USER_POSITION_ADMISSION_WASM_BYTES_V1,
  USER_POSITION_ADMISSION_WASM_SHA256_V1,
} from './generated/userPositionAdmissionWasmV1';
import {
  loadUserPositionAdmissionWasmV1,
  parseUserPositionAdmissionPlanV1,
} from './userPositionAdmissionV1';

/**
 * THE DEFECT THIS CLOSES. `JoinPanel` states that admission "needs the
 * position owner's signature over a frame the browser cannot yet assemble
 * byte-exactly", and hands the reader a CLI command instead. You cannot trade
 * in a market you cannot join, so that one sentence is why maker/taker trade
 * was present-but-unreachable for a stranger holding only a wallet.
 *
 * The answer is not a TypeScript frame builder. Twenty-seven accounts,
 * per-coordinate privileges, two rent deficits and a predicted Claims receipt
 * reimplemented by hand is the mirror this application keeps convicting —
 * `evaluateProductV2` is already one. So the Rust planner is compiled, and
 * these tests hold the browser to consuming it rather than re-deriving it.
 */
describe('the admission planner reaches the browser as compiled Rust', () => {
  it('states the frame width and selector from the contract, not from here', () => {
    // Both are emitted from `dclutch-claims::position_admission` and
    // pinned again inside the WASM crate with `const _: () = assert!(...)`.
    // If either moves, the Rust build fails before this test can be wrong.
    expect(USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1).toBe(27);
    expect(USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1).toBeLessThan(USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1);
    expect(USER_POSITION_ADMISSION_SNAPSHOT_FORMAT_V1).toBe('dclutch-user-position-admission-snapshot-v1');
    expect(USER_POSITION_ADMISSION_PLAN_FORMAT_V1).toBe('dclutch-user-position-admission-plan-v1');
  });

  it('refuses a blob whose bytes are not the generated artifact', async () => {
    // Unverified fetched bytes never execute. The digest and length both come
    // from the generator, so a substituted planner is refused before any of it
    // runs — which matters more here than anywhere: this module builds the
    // transaction a wallet is about to sign.
    const wrong = new Uint8Array(USER_POSITION_ADMISSION_WASM_BYTES_V1).fill(7);
    await expect(loadUserPositionAdmissionWasmV1(async () => new Response(wrong)))
      .rejects.toThrow(/do not match the generated Rust artifact identity/);
  });

  it('refuses a short blob before it hashes it', async () => {
    await expect(loadUserPositionAdmissionWasmV1(async () => new Response(new Uint8Array(4))))
      .rejects.toThrow(/do not match the generated Rust artifact identity/);
  });

  it('refuses a plan that is not the exact accepted format', () => {
    expect(() => parseUserPositionAdmissionPlanV1('{"format":"something-else"}'))
      .toThrow(/admission plan is not the exact accepted format/);
    expect(() => parseUserPositionAdmissionPlanV1('not json'))
      .toThrow(/admission plan is not JSON/);
  });

  it('refuses a plan whose frame is not the contract width', () => {
    // The planner cannot emit this; a substituted or truncated transport can.
    // The client checks the width it was told rather than trusting the blob.
    const plan = JSON.stringify({
      format: USER_POSITION_ADMISSION_PLAN_FORMAT_V1,
      instructions: [],
      requiredSigner: 'x',
      accountCount: 26,
      ownerAccountIndex: USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1,
    });
    expect(() => parseUserPositionAdmissionPlanV1(plan))
      .toThrow(/admission plan states a 26-account frame where the contract has 27/);
  });

  it('accepts the planner-shaped plan and hands back its instructions', () => {
    const plan = JSON.stringify({
      format: USER_POSITION_ADMISSION_PLAN_FORMAT_V1,
      instructions: [{ programId: 'Trading', accounts: [{ pubkey: 'a', isSigner: true, isWritable: false }], dataBase64: 'AA==' }],
      requiredSigner: 'owner',
      observedSlot: '900',
      position: 'p',
      admission: 'a',
      positionTopUpLamports: '10',
      admissionTopUpLamports: '0',
      accountCount: USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1,
      ownerAccountIndex: USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1,
    });
    const parsed = parseUserPositionAdmissionPlanV1(plan);
    expect(parsed.instructions.length).toBe(1);
    expect(parsed.requiredSigner).toBe('owner');
    expect(parsed.observedSlot).toBe('900');
    expect(parsed.positionTopUpLamports).toBe('10');
  });

  it('pins the artifact identity so a regenerated planner cannot slip in silently', () => {
    expect(USER_POSITION_ADMISSION_WASM_SHA256_V1).toMatch(/^[0-9a-f]{64}$/);
    expect(USER_POSITION_ADMISSION_WASM_BYTES_V1).toBeGreaterThan(100_000);
  });
});
