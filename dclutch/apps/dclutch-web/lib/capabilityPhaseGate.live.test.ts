import { describe, expect, it } from 'vitest';

import {
  capabilityActPhaseGatesV1,
  evaluateCapabilityV1,
  type CapabilityMarketSnapshotV1,
} from '@dclutch/sdk/capabilityModel';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from './capabilitySurface';
import { decodeMarketCoreStateV2 } from '@dclutch/sdk/marketCoreV2';
import { DEVNET_DEPLOYMENT_V1 } from '@dclutch/sdk/deployments';
import { PUBLIC_DEVNET_CUT_V1 } from '@dclutch/sdk/publicCutStaging';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';

const featured = PUBLIC_DEVNET_CUT_V1.market;
const live = process.env.DCLUTCH_LIVE_DEVNET === '1' && featured !== null ? it : it.skip;

const standing = (id: string) => {
  const found = BROWSER_CAPABILITY_STANDINGS_V1.find((one) => one.action.id === id);
  if (found === undefined) throw new Error(`no standing for ${id}`);
  return found;
};

/**
 * The phase gate, run once against a Market that actually exists.
 *
 * Every other case in `capabilityPhaseGate.test.ts` constructs the observation
 * it then judges, which is the right way to cover fifteen prestates and the
 * wrong way to learn whether the chain of custody survives contact with a
 * node. This one reads the featured devnet Market's Core account, decodes its
 * phase with the browser's own decoder, and hands THAT to the same evaluator
 * the workbench calls -- so the guard's constant, the census that read it, the
 * reference page, the generated table and the verdict are exercised end to end
 * against a finalized read rather than a literal.
 *
 * WHY REDEMPTION IS THE CASE. `claims.redeem` declares
 * `claims/terminal_settlement_v3::process`, which was ungated until Claims
 * named its guards: the route admits `Terminal` and `Retiring` and nothing
 * else. The featured Market is Open and trading, so the card for it on
 * `/workbench` said READY TO PREFLIGHT for an act the chain refuses on sight,
 * and it is the first act outside Resolution whose refusal this surface can
 * publish. What is pinned is the AGREEMENT between the decoded phase and the
 * verdict, never the cohort's own literals -- a later cohort in `Terminal`
 * makes this case admit, and it says so rather than failing.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1`. One account read.
 */
describe('live devnet capability phase gate', () => {
  live('judges the featured Market’s redemption card from its own decoded phase', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const observation = await client.accountInfo(featured!);
    expect(observation.account, `no account at ${featured}`).not.toBeNull();
    expect(observation.account!.owner).toBe(DEVNET_DEPLOYMENT_V1.programs.core);

    const state = decodeMarketCoreStateV2(featured!, observation.account!.data);
    const snapshot: CapabilityMarketSnapshotV1 = {
      market: { address: featured!, phase: state.phase, readiness: state.readiness },
    };

    const redeem = standing('claims.redeem');
    const gates = capabilityActPhaseGatesV1(redeem.action);
    expect(gates).toHaveLength(1);
    expect(gates[0]!.route).toBe('claims/terminal_settlement_v3::process');
    expect(gates[0]!.phases).toEqual(['Terminal', 'Retiring']);

    const verdict = evaluateCapabilityV1(redeem, snapshot, []);
    const settled = state.phase === 'Terminal' || state.phase === 'Retiring';
    if (settled) {
      expect(verdict.phaseGate.verdict).toBe('admitted');
      expect(verdict.status).toBe('ready-to-preflight');
      return;
    }
    // The case this test was written for, and the one the featured Market is
    // in: an unsettled Market refuses redemption, by name, with the route that
    // refused it and both phases said out loud.
    expect(verdict.status).toBe('wrong-phase');
    expect(verdict.phaseGate.verdict).toBe('excluded');
    expect(verdict.phaseGate.excludedBy?.route).toBe('claims/terminal_settlement_v3::process');
    expect(verdict.reason).toContain('Terminal or Retiring');
    expect(verdict.reason).toContain(state.phase!);
  }, 60_000);

  live('does not refuse an act whose own gate the same observation admits', async () => {
    // The positive control. A test that only ever sees a refusal cannot tell a
    // working gate from a gate that refuses everything, and "nothing was
    // admitted" and "the evaluator is broken" would read identically.
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const observation = await client.accountInfo(featured!);
    expect(observation.account).not.toBeNull();
    const state = decodeMarketCoreStateV2(featured!, observation.account!.data);
    const snapshot: CapabilityMarketSnapshotV1 = {
      market: { address: featured!, phase: state.phase, readiness: state.readiness },
    };

    const provider = standing('source.provider');
    const gates = capabilityActPhaseGatesV1(provider.action);
    expect(gates).toHaveLength(1);
    expect(gates[0]!.prestates).toEqual([['Open', 'Consumed']]);

    const verdict = evaluateCapabilityV1(provider, snapshot, []);
    const live_market = state.phase === 'Open' && state.readiness === 'Consumed';
    expect(verdict.phaseGate.verdict).toBe(live_market ? 'admitted' : 'excluded');
    if (live_market) expect(verdict.status).toBe('ready-to-preflight');
  }, 60_000);
});
