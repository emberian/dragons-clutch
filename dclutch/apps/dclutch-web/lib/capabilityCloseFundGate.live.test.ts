import { describe, expect, it } from 'vitest';

import { capabilityActPhaseGatesV1, evaluateCapabilityV1, type CapabilityMarketSnapshotV1 } from './capabilityModel';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from './capabilitySurface';
import { decodeMarketCoreStateV2 } from './marketCoreV2';
import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import { PUBLIC_DEVNET_CUT_V1 } from './publicCutStaging';
import { SolanaRpcClient } from './rpc';

/**
 * The refusal `source.close-fund` gained by gaining a declaration, run live.
 *
 * The act's planner emits `DCLRFCQ1`, which the census's predicate table binds
 * to `resolution/core_effect::process_direct_funding_close_v1`, whose guard
 * admits `Retiring+Consumed` and nothing else. The act declared no route at
 * all until `capabilityRouteDerivation.test.ts`, so `/console` and
 * `/workbench` reported READY TO PREFLIGHT for closing a resolution fund
 * against any Market in any phase -- an act the chain refuses before an
 * account is read.
 *
 * Two Markets, two phases, one act, on two different cohorts' Core programs.
 * Neither is a phase this act admits, so both must refuse, and the assertion
 * is the AGREEMENT between the decoded phase and the verdict rather than a
 * cohort literal: a Market that reaches `Retiring` makes its case admit and
 * says so instead of failing.
 *
 * BOTH MARKETS ARE NOW ON ONE LIVE COHORT, and that is the repair rather than
 * a simplification. The Open half used to be cohort-15's Market beside a
 * cohort-14 Market for the phase that has left Open -- and this file asserts
 * that each account is owned by `DEVNET_DEPLOYMENT_V1.programs.core`, so the
 * cohort-14 coordinate went red at the owner check the morning cohort-15
 * landed. A dead cohort's accounts are readable; they are not owned by the
 * program this manifest names, and every case here reads through that manifest.
 *
 *   * the OPEN one is cohort-15's third Direct market, which took the first
 *     fee-bearing fill on a public chain and has not admitted its answer:
 *     Open + Consumed, 368 bytes of `DCLTCOR3`.
 *   * the one that has LEFT Open is the featured market, read out of the cut:
 *     settled, paid, and now Retiring + Consumed -- which is the ONE prestate
 *     this act admits, so the admit branch below is now exercised on a real
 *     account instead of only ever being described.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1`. Two account reads.
 */

const OPEN_MARKET = 'C9dLhWj7yi76RtQhhHV13gKuudAbV8qio8TZVEn3CjAT';
const SETTLED_MARKET = PUBLIC_DEVNET_CUT_V1.market ?? '';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' && SETTLED_MARKET !== '' ? it : it.skip;

const standing = (id: string) => {
  const found = BROWSER_CAPABILITY_STANDINGS_V1.find((one) => one.action.id === id);
  if (found === undefined) throw new Error(`no standing for ${id}`);
  return found;
};

async function readMarket(address: string, owner: string): Promise<CapabilityMarketSnapshotV1> {
  const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
  const observation = await client.accountInfo(address);
  expect(observation.account, `no account at ${address}`).not.toBeNull();
  expect(observation.account!.owner, `${address} is not owned by ${owner}`).toBe(owner);
  const state = decodeMarketCoreStateV2(address, observation.account!.data);
  return { market: { address, phase: state.phase, readiness: state.readiness } };
}

describe('live devnet close-fund gate', () => {
  live('refuses closing the fund of an Open Market, by the route that refuses it', async () => {
    const snapshot = await readMarket(OPEN_MARKET, DEVNET_DEPLOYMENT_V1.programs.core);
    const close = standing('source.close-fund');
    const gates = capabilityActPhaseGatesV1(close.action);
    expect(gates).toHaveLength(1);
    expect(gates[0]!.route).toBe('resolution/core_effect::process_direct_funding_close_v1');
    expect(gates[0]!.prestates).toEqual([['Retiring', 'Consumed']]);

    const verdict = evaluateCapabilityV1(close, snapshot, []);
    const phase = snapshot.market!.phase;
    if (phase === 'Retiring' && snapshot.market!.readiness === 'Consumed') {
      expect(verdict.status).toBe('ready-to-preflight');
      return;
    }
    expect(verdict.status).toBe('wrong-phase');
    expect(verdict.phaseGate.verdict).toBe('excluded');
    expect(verdict.phaseGate.excludedBy?.route).toBe('resolution/core_effect::process_direct_funding_close_v1');
    expect(verdict.reason).toContain('admits only Retiring+Consumed');
    expect(verdict.reason).toContain(phase!);
  }, 60_000);

  live('reaches the one prestate this act admits, and refuses the act that Market no longer admits', async () => {
    // Two verdicts off ONE read, and they must disagree. A test that only ever
    // sees a refusal cannot tell a working gate from a gate that refuses
    // everything. This Market is Retiring + Consumed, which is exactly what
    // `source.close-fund` admits and exactly what `source.provider` does not,
    // so the pair runs in both directions on one account.
    const snapshot = await readMarket(SETTLED_MARKET, DEVNET_DEPLOYMENT_V1.programs.core);
    const phase = snapshot.market!.phase;
    const readiness = snapshot.market!.readiness;

    const close = evaluateCapabilityV1(standing('source.close-fund'), snapshot, []);
    if (phase === 'Retiring' && readiness === 'Consumed') {
      expect(close.status).toBe('ready-to-preflight');
    } else {
      expect(close.status).toBe('wrong-phase');
      expect(close.phaseGate.excludedBy?.route).toBe('resolution/core_effect::process_direct_funding_close_v1');
    }

    const provider = evaluateCapabilityV1(standing('source.provider'), snapshot, []);
    const admits = phase === 'Open' && readiness === 'Consumed';
    expect(provider.phaseGate.verdict).toBe(admits ? 'admitted' : 'excluded');
    if (admits) expect(provider.status).toBe('ready-to-preflight');
  }, 60_000);
});
