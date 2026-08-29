import { Keypair } from '@solana/web3.js';
import { describe, expect, it, vi } from 'vitest';

import {
  COLD_CLIENT_CHAIN_STEPS_V1,
  runColdClientJourneyV1,
  type ColdClientAdapterV1,
  type ColdClientChainStepV1,
  type ColdClientStepResultV1,
} from './coldClientJourney';

const address = (seed: number) => Keypair.fromSeed(new Uint8Array(32).fill(seed)).publicKey.toBase58();
const MARKET = address(20);
const WALLET = address(21);
const DEPLOYMENT = Object.freeze({
  cluster: 'devnet' as const,
  endpoint: 'https://api.devnet.solana.com',
  releaseSetId: 'ab'.repeat(32),
  programs: Object.freeze({
    registry: address(1), core: address(2), trading: address(3), claims: address(4),
    custody: address(5), resolution: address(6), rent: address(7),
  }),
});

function outcome(step: ColdClientChainStepV1): ColdClientStepResultV1 {
  const artifact = step === 'direct.preview-unsigned'
    ? { kind: 'unsigned-preview' as const, digest: '11'.repeat(32) }
    : step === 'redeem.prepare-unsigned'
      ? { kind: 'unsigned-transaction' as const, digest: '22'.repeat(32), byteLength: 900 }
      : undefined;
  return Object.freeze({
    step,
    status: 'ready' as const,
    reason: `${step} used injected finalized evidence.`,
    observedSlot: String(100 + COLD_CLIENT_CHAIN_STEPS_V1.indexOf(step)),
    addresses: step === 'market.discover' || step === 'market.inspect' ? [MARKET] : [],
    truths: [Object.freeze({
      subject: step === 'direct.inspect' ? 'Direct capability' : step,
      verdict: 'authenticated' as const,
      detail: 'The adapter reacquired this fact from finalized chain state.',
    })],
    artifact,
  });
}

function adapter(runStep = vi.fn(async (step: ColdClientChainStepV1) => outcome(step))): ColdClientAdapterV1 {
  return Object.freeze({
    coldState: vi.fn(async () => Object.freeze({ localStorageKeys: [], sessionStorageKeys: [], cacheKeys: [] })),
    loadBakedDeployment: vi.fn(async () => DEPLOYMENT),
    runStep,
  });
}

describe('cold-client public journey contract', () => {
  it('walks discovery through unsigned Direct, redeem preparation, and retirement inspection in one fixed order', async () => {
    const harness = adapter();
    const report = await runColdClientJourneyV1(harness, {
      deploymentKey: 'checked-devnet-v7',
      marketAddress: MARKET,
      walletAddress: WALLET,
      directTicket: '{"kind":"injected by the live driver"}',
      redeemPlan: '{"kind":"Rust-authored payout plan"}',
      transactionIds: ['injected-finalized-transaction-id'],
    });
    expect(harness.runStep).toHaveBeenCalledTimes(COLD_CLIENT_CHAIN_STEPS_V1.length);
    expect(vi.mocked(harness.runStep).mock.calls.map(([step]) => step)).toEqual(COLD_CLIENT_CHAIN_STEPS_V1);
    expect(report.selectedMarket).toBe(MARKET);
    expect(report.steps.find((step) => step.step === 'direct.inspect')?.truths).toContainEqual(expect.objectContaining({ subject: 'Direct capability', verdict: 'authenticated' }));
    expect(report.steps.find((step) => step.step === 'direct.preview-unsigned')?.artifact?.kind).toBe('unsigned-preview');
    expect(report.steps.find((step) => step.step === 'redeem.prepare-unsigned')?.artifact?.kind).toBe('unsigned-transaction');
    expect(report.steps.find((step) => step.step === 'retirement.inspect')).toMatchObject({ status: 'ready' });
    expect(report.injectedTransactionIds).toEqual(['injected-finalized-transaction-id']);
    expect(report).toMatchObject({ signingRequested: false, submissionRequested: false });
    expect(Object.keys(harness)).not.toContain('sign');
    expect(Object.keys(harness)).not.toContain('submit');
  });

  it('stops before deployment or RPC work when any client residue exists', async () => {
    const harness = adapter();
    vi.mocked(harness.coldState).mockResolvedValue({ localStorageKeys: ['old-recovery-journal'], sessionStorageKeys: [], cacheKeys: [] });
    await expect(runColdClientJourneyV1(harness, { deploymentKey: 'checked-devnet-v7' })).rejects.toThrow('state is not empty');
    expect(harness.loadBakedDeployment).not.toHaveBeenCalled();
    expect(harness.runStep).not.toHaveBeenCalled();
  });

  it('preserves explicit capability refusals and prevents downstream builders from claiming readiness', async () => {
    const runStep = vi.fn(async (step: ColdClientChainStepV1) => {
      if (step === 'direct.inspect') return Object.freeze({
        step, status: 'refused' as const, reason: 'This Market did not found Direct.',
        truths: [Object.freeze({ subject: 'Direct capability', verdict: 'refused' as const, detail: 'No Direct entry exists in the authenticated manifest.' })],
      });
      if (step === 'direct.preview-unsigned') return Object.freeze({ step, status: 'unavailable' as const, reason: 'No Direct capability exists to preview.' });
      return outcome(step);
    });
    const report = await runColdClientJourneyV1(adapter(runStep), {
      deploymentKey: 'checked-devnet-v7', marketAddress: MARKET, walletAddress: WALLET,
      directTicket: '{}', redeemPlan: '{}',
    });
    expect(report.steps.find((step) => step.step === 'direct.inspect')).toMatchObject({ status: 'refused' });
    expect(report.steps.find((step) => step.step === 'direct.preview-unsigned')).toMatchObject({ status: 'unavailable' });
  });

  it('refuses an unsigned preview that appears without all of its authenticated inputs', async () => {
    await expect(runColdClientJourneyV1(adapter(), {
      deploymentKey: 'checked-devnet-v7', marketAddress: MARKET, walletAddress: WALLET, redeemPlan: '{}',
    })).rejects.toThrow('Direct preview became ready without');
  });

  it('refuses a cold retirement result that claims readiness without an authenticated Market', async () => {
    const runStep = vi.fn(async (step: ColdClientChainStepV1) => {
      if (step === 'market.inspect') return Object.freeze({ step, status: 'refused' as const, reason: 'The Market bytes did not authenticate.' });
      if (step === 'participant.inspect' || step === 'direct.inspect' || step === 'direct.preview-unsigned'
          || step === 'resolution.inspect' || step === 'redeem.inspect' || step === 'redeem.prepare-unsigned') {
        return Object.freeze({ step, status: 'unavailable' as const, reason: 'No authenticated Market exists for this step.' });
      }
      return outcome(step);
    });
    await expect(runColdClientJourneyV1(adapter(runStep), {
      deploymentKey: 'checked-devnet-v7', marketAddress: MARKET, walletAddress: WALLET,
      directTicket: '{}', redeemPlan: '{}',
    })).rejects.toThrow('retirement.inspect became ready without an authenticated Market');
  });
});
