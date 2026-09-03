import { describe, expect, it } from 'vitest';

import {
  capabilityActPhaseGatesV1,
  evaluateCapabilityV1,
  type CapabilityMarketSnapshotV1,
} from './capabilityModel';
import { routeOtherMachineGateV1, routePhaseGateV1 } from '@dclutch/sdk/generated/marketPhaseAdmissionV1';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from './capabilitySurface';
import { decodeMarketCoreStateV2 } from './marketCoreV2';
import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import { SolanaRpcClient } from './rpc';

/**
 * Cohort-14's second Market, which resolved and paid.
 *
 * Named as a coordinate rather than read from the public cut, because the cut
 * follows whatever is featured and this case wants a Market that has LEFT
 * `Open`. Its phase is still read from the chain and never assumed: what is
 * pinned below is the AGREEMENT between the decoded phase and the published
 * gate, so a later re-founding at a different phase changes which branch runs
 * and not whether the test passes.
 */
const COHORT14_MARKET_B = 'DUVcCGfjXzp1fBktTCjsAomgrn9S6sxSDziQHoyRiu8A';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

const standing = (id: string) => {
  const found = BROWSER_CAPABILITY_STANDINGS_V1.find((one) => one.action.id === id);
  if (found === undefined) throw new Error(`no standing for ${id}`);
  return found;
};

/**
 * The machines that are NOT the Market's phase, against a Market that exists.
 *
 * `capabilityPhaseGate.live.test.ts` runs the Market machine end to end against
 * a finalized read. This one is its other half, and it exists because the
 * routes that acquired a gate most recently did not acquire a MARKET one: the
 * Direct root's own lifecycle, the per-entry funding-ledger status, the Series
 * ticket phase. Those are separate discriminants in separate accounts, and the
 * one thing a reader must never do with them is conclude from the Market's
 * phase alone.
 *
 * So the two cases below are the two halves of that:
 *
 *   - a route whose Market gate this observation CAN answer, answered from the
 *     decoded phase, with the second machine still named beside the answer;
 *   - a route that has no Market gate at all and is nonetheless not ungated,
 *     which is the exact reading `no-phase-gate` exists to prevent.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1`. One account read per case.
 */
describe('live devnet gates over machines that are not the Market phase', () => {
  live('answers Direct token setup from the decoded phase and still names direct-root', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const observation = await client.accountInfo(COHORT14_MARKET_B);
    expect(observation.account, `no account at ${COHORT14_MARKET_B}`).not.toBeNull();
    expect(observation.account!.owner).toBe(DEVNET_DEPLOYMENT_V1.programs.core);

    const state = decodeMarketCoreStateV2(COHORT14_MARKET_B, observation.account!.data);
    const route = 'trading/direct_token_setup_v1::process_direct_token_setup_v1';

    // The Market half, which this observation can answer.
    const gate = routePhaseGateV1(route);
    expect(gate, `${route} lost its Market gate`).not.toBeNull();
    expect(gate!.phases).toEqual(['Open']);
    expect(gate!.phases.includes(state.phase)).toBe(state.phase === 'Open');

    // The half it cannot, and the reason this route is in BOTH tables: the
    // Direct root persists its own `Open`, in its own account, and a Market
    // that is `Open` says nothing about whether that root still admits makers.
    const other = routeOtherMachineGateV1(route);
    expect(other, `${route} lost its direct-root gate`).not.toBeNull();
    expect(other!.machines).toEqual(['direct-root']);
  }, 60_000);

  live('does not call an ungated-looking capability route ungated', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const observation = await client.accountInfo(COHORT14_MARKET_B);
    expect(observation.account).not.toBeNull();
    const state = decodeMarketCoreStateV2(COHORT14_MARKET_B, observation.account!.data);
    const snapshot: CapabilityMarketSnapshotV1 = {
      market: { address: COHORT14_MARKET_B, phase: state.phase, readiness: state.readiness },
    };

    // Closing a capability is exactly the act a Market in this phase is walking
    // toward, and until the funding ledger's per-entry status had a name the
    // census read NO gate for it -- which a consumer reads as "nothing refuses
    // this", one step from the READY TO PREFLIGHT this whole chain replaced.
    const route = 'core/capability::process#CloseCapability';
    expect(routePhaseGateV1(route), `${route} should have no MARKET gate`).toBeNull();
    const other = routeOtherMachineGateV1(route);
    expect(other, `${route} lost its funding-ledger gate`).not.toBeNull();
    expect(other!.machines).toEqual(['funding-ledger']);

    // The positive control, on the same observation, so "the table answers
    // nothing" and "this Market admits nothing" cannot read alike. Market B
    // settled, so redemption is admitted by its own gate at exactly the phase
    // just decoded.
    const redeem = standing('claims.redeem');
    const gates = capabilityActPhaseGatesV1(redeem.action);
    expect(gates).toHaveLength(1);
    const verdict = evaluateCapabilityV1(redeem, snapshot);
    const settled = state.phase === 'Terminal' || state.phase === 'Retiring';
    expect(verdict.phaseGate.verdict).toBe(settled ? 'admitted' : 'excluded');
    expect(verdict.status).toBe(settled ? 'ready-to-preflight' : 'wrong-phase');
  }, 60_000);
});
