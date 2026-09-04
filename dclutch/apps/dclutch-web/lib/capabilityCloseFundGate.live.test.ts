import { describe, expect, it } from 'vitest';

import { capabilityActPhaseGatesV1, evaluateCapabilityV1, type CapabilityMarketSnapshotV1 } from './capabilityModel';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from './capabilitySurface';
import { decodeMarketCoreStateV2 } from './marketCoreV2';
import { DEVNET_DEPLOYMENT_V1 } from './deployments';
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
 * The coordinates are named rather than taken from the public cut, because the
 * cut follows whatever is featured and these two cases want one Market that
 * has LEFT Open and one that has not.
 *
 *   * cohort-14 market B, `docs/evidence/C16_REHEARSAL_2026_09_03.md` and
 *     `bd0182fbd`: Terminal + Consumed. Its cohort's programs were CLOSED on
 *     2026-09-04, which costs this test nothing -- a closed cohort keeps every
 *     account it wrote, and this reads the Market, not the code.
 *   * cohort-15's Open Market and its Core program,
 *     `docs/evidence/COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md`
 *     section 8: Open + Consumed, 368 bytes of `DCLTCOR3`.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1`. Two account reads.
 */

const COHORT14_MARKET_B = 'DUVcCGfjXzp1fBktTCjsAomgrn9S6sxSDziQHoyRiu8A';
const COHORT15_OPEN_MARKET = '3QytL1bBMtCvRoXWR5h7MgutRBZqtv7emUVubEo5a4T2';
const COHORT15_CORE_PROGRAM = '7hGerMC6Wj742FVTyiF9PhRnGSBzbee7TMZ6sUytsmFr';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

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
  live('refuses closing the fund of a Terminal Market, by the route that refuses it', async () => {
    const snapshot = await readMarket(COHORT14_MARKET_B, DEVNET_DEPLOYMENT_V1.programs.core);
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

  live('refuses the same act on an Open Market, and admits the one that Market does admit', async () => {
    // Two verdicts off ONE read, and they must disagree. A test that only ever
    // sees a refusal cannot tell a working gate from a gate that refuses
    // everything, and on this Market -- Open + Consumed, a live cohort --
    // `source.provider` is exactly the act whose own gate admits.
    const snapshot = await readMarket(COHORT15_OPEN_MARKET, COHORT15_CORE_PROGRAM);
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
