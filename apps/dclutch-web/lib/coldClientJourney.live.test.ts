import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { makeColdClientChainAdapterV1 } from '@/lib/coldClientChainAdapter';
import {
  COLD_CLIENT_CHAIN_STEPS_V1,
  runColdClientJourneyV1,
  type ColdClientDeploymentV1,
} from '@/lib/coldClientJourney';

/**
 * The cold-client journey against a LIVE chain, through the real adapter.
 *
 * Opt-in: point DCLUTCH_JOURNEY_SESSION at a JSON session file naming the
 * endpoint, the seven programs, and (optionally) a Market, a wallet identity,
 * a signed counterparty ticket, and a Rust payout artifact path. The probe
 * runner writes one from a completed local lifecycle; a devnet session names
 * the public deployment instead. Without the variable this file skips —
 * it never invents a chain.
 *
 * Session schema (dclutch-web-journey-session-v1):
 * { schema, cluster: 'localnet'|'devnet', endpoint, releaseSetId,
 *   programs: {registry,core,trading,claims,custody,resolution,rent},
 *   market?, wallet?, directTicket?, redeemPlanPath? }
 */

type JourneySessionV1 = Readonly<{
  schema: 'dclutch-web-journey-session-v1';
  cluster: 'localnet' | 'devnet';
  endpoint: string;
  releaseSetId: string;
  programs: ColdClientDeploymentV1['programs'];
  market?: string;
  wallet?: string;
  directTicket?: string;
  redeemPlanPath?: string;
}>;

const sessionPath = process.env.DCLUTCH_JOURNEY_SESSION;
const describeLive = sessionPath === undefined ? describe.skip : describe;

function loadSession(path: string): JourneySessionV1 {
  const value = JSON.parse(readFileSync(path, 'utf8')) as JourneySessionV1;
  if (value.schema !== 'dclutch-web-journey-session-v1') throw new Error('journey session has another schema');
  if (value.cluster !== 'localnet' && value.cluster !== 'devnet') throw new Error('journey session names a cluster the journey refuses');
  return value;
}

describeLive('the cold-client journey against the live chain', () => {
  it('walks all nine public steps through the real reading surface', async () => {
    const session = loadSession(sessionPath!);
    const deployment: ColdClientDeploymentV1 = Object.freeze({
      cluster: session.cluster,
      endpoint: session.endpoint,
      releaseSetId: session.releaseSetId,
      programs: session.programs,
    });
    const adapter = makeColdClientChainAdapterV1({ deployments: Object.freeze({ [session.cluster]: deployment }) });
    const report = await runColdClientJourneyV1(adapter, Object.freeze({
      deploymentKey: session.cluster,
      marketAddress: session.market,
      walletAddress: session.wallet,
      directTicket: session.directTicket,
      redeemPlan: session.redeemPlanPath === undefined ? undefined : readFileSync(session.redeemPlanPath, 'utf8'),
    }));

    expect(report.schema).toBe('dclutch/cold-client-journey/v1');
    expect(report.signingRequested).toBe(false);
    expect(report.submissionRequested).toBe(false);
    expect(report.steps.map((step) => step.step)).toEqual([...COLD_CLIENT_CHAIN_STEPS_V1]);

    const byStep = new Map(report.steps.map((step) => [step.step, step]));
    // The pure reading spine must be READY against a real chain, wallet or not.
    for (const step of ['market.discover', 'market.inspect', 'direct.inspect', 'resolution.inspect', 'retirement.inspect'] as const) {
      expect(byStep.get(step)?.status, `${step}: ${byStep.get(step)?.reason}`).toBe('ready');
    }
    // Wallet-scoped steps are ready exactly when a wallet identity was injected.
    for (const step of ['participant.inspect', 'redeem.inspect'] as const) {
      const expected = session.wallet === undefined ? 'unavailable' : 'ready';
      expect(byStep.get(step)?.status, `${step}: ${byStep.get(step)?.reason}`).toBe(expected);
    }
    // Builder steps are ready exactly when their evidence inputs exist.
    expect(byStep.get('direct.preview-unsigned')?.status).toBe(
      session.wallet !== undefined && session.directTicket !== undefined ? 'ready' : 'unavailable',
    );
    expect(byStep.get('redeem.prepare-unsigned')?.status).toBe(
      session.wallet !== undefined && session.redeemPlanPath !== undefined ? 'ready' : 'unavailable',
    );

    // Human-readable yield, printed so a run leaves a table behind.
    for (const step of report.steps) {
      console.log(`${step.step.padEnd(24)} ${step.status.padEnd(12)} ${step.reason}`);
    }
  }, 120_000);
});
