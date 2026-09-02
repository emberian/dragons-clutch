import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { type ClaimsCustodyReplayPlanV1, type ClaimsCustodyReplayStateV1 } from './claimsCustodyReplay';
import { CLIENT_OPERATION_JOURNAL_FORMAT_V1, type ClientOperationJournalV1 } from './clientOperationJournal';
import {
  authenticateClaimsReplayJournalV1,
  claimsReplayFinalizedCompletionV1,
  claimsReplayJournalInputV1,
  requireTerminalPayoutRouteScopeV1,
} from './redeemOperationJournal';
import { type SignatureStatusObservation } from './rpc';
import { type WalletTerminalPayoutManifestV3 } from './walletTerminalPayoutV3';

const address = (byte: number) => new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
const digest = (byte: number) => byte.toString(16).padStart(2, '0').repeat(32);

const request = Object.freeze({
  marketAddress: address(1), claimsProgramId: address(2), custodyProgramId: address(3),
  registryProgramId: address(4), payer: address(5),
});
const replayPlan = Object.freeze({
  marketAddress: request.marketAddress, aggregateAddress: address(6), replayAddress: address(7),
  callerAuthorityAddress: address(8), activationCacheAddress: address(9), claimsProgramDataAddress: address(10),
  realmRecordAddress: address(11), realmStagingAddress: address(12), payer: request.payer, rentLamports: '42',
  custodyRequestBytes: new Uint8Array([1, 2, 3]), custodyRequestDigestHex: digest(13),
  instructionData: new Uint8Array([4, 5]), requiredSigners: Object.freeze([request.payer]),
}) as unknown as ClaimsCustodyReplayPlanV1;

function journal(input: ReturnType<typeof claimsReplayJournalInputV1>): ClientOperationJournalV1 {
  return Object.freeze({
    format: CLIENT_OPERATION_JOURNAL_FORMAT_V1, ...input, intentDigest: digest(20), planDigest: digest(21),
    phase: 'unsigned', signature: null, signedWireBase64: null,
  });
}

describe('redeem operation recovery decisions', () => {
  it('authenticates every Claims replay route coordinate and refuses owner, address, or request substitution', () => {
    const input = claimsReplayJournalInputV1({ clusterGenesis: address(14), market: request.marketAddress, owner: request.payer }, request, replayPlan);
    const saved = journal(input);
    expect(() => authenticateClaimsReplayJournalV1(saved, request, replayPlan)).not.toThrow();
    expect(() => authenticateClaimsReplayJournalV1(saved, request, { ...replayPlan, replayAddress: address(15) })).toThrow(/differs/);
    expect(() => authenticateClaimsReplayJournalV1(saved, { ...request, custodyProgramId: address(16) }, replayPlan)).toThrow(/differs/);
    expect(() => claimsReplayJournalInputV1(saved, { ...request, payer: address(17) }, replayPlan)).toThrow(/scope/);
  });

  it('requires both the exact replay account and a successful finalized signature', () => {
    const exists: ClaimsCustodyReplayStateV1 = Object.freeze({ status: 'exists', replayAddress: address(7), nextRevision: '1', generation: '0', rentRefund: request.payer, note: 'exact', observedSlot: '9' });
    const creatable = Object.freeze({ status: 'creatable' as const, plan: replayPlan, note: 'absent', observedSlot: '9' });
    const status = (confirmationStatus: string | null, succeeded: boolean | null = true): SignatureStatusObservation => Object.freeze({
      signature: 'x', known: true, slot: '9', confirmationStatus, succeeded, errorText: succeeded === false ? 'failed' : null,
    });
    expect(claimsReplayFinalizedCompletionV1(status('finalized'), exists)).toBe(true);
    expect(claimsReplayFinalizedCompletionV1(status('confirmed'), exists)).toBe(false);
    expect(claimsReplayFinalizedCompletionV1(status('finalized', false), exists)).toBe(false);
    expect(claimsReplayFinalizedCompletionV1(status('finalized'), creatable)).toBe(false);
    expect(claimsReplayFinalizedCompletionV1(undefined, exists)).toBe(false);
  });

  it('refuses payout Market, Position, owner, claim, or operation substitution', () => {
    const expected = Object.freeze({ market: address(1), position: address(2), owner: address(3), claimIndex: 4 });
    const saved = Object.freeze({
      format: CLIENT_OPERATION_JOURNAL_FORMAT_V1, operation: 'wallet-terminal-payout-v3', clusterGenesis: address(9),
      market: expected.market, owner: expected.owner, operationDigest: digest(1), intentDigest: digest(2), planDigest: digest(3),
      intent: '{}', plan: '{}', phase: 'unsigned', signature: null, signedWireBase64: null,
    }) as ClientOperationJournalV1;
    const manifest = { request: expected } as unknown as WalletTerminalPayoutManifestV3;
    expect(() => requireTerminalPayoutRouteScopeV1(saved, manifest, expected)).not.toThrow();
    for (const changed of [
      { ...expected, market: address(8) }, { ...expected, position: address(8) },
      { ...expected, owner: address(8) }, { ...expected, claimIndex: 5 },
    ]) expect(() => requireTerminalPayoutRouteScopeV1(saved, { request: changed } as unknown as WalletTerminalPayoutManifestV3, expected)).toThrow(/substitutes/);
    expect(() => requireTerminalPayoutRouteScopeV1({ ...saved, operation: 'claims-replay-create-v1' }, manifest, expected)).toThrow(/substitutes/);
  });
});
