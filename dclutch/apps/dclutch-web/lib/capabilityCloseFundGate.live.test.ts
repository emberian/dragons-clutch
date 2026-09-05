import { describe, expect, it } from 'vitest';

import { capabilityActPhaseGatesV1, evaluateCapabilityV1, type CapabilityMarketSnapshotV1 } from '@dclutch/sdk/capabilityModel';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from './capabilitySurface';
import { decodeMarketCoreStateV2 } from '@dclutch/sdk/marketCoreV2';
import { DEVNET_DEPLOYMENT_V1 } from '@dclutch/sdk/deployments';
import { PUBLIC_DEVNET_CUT_V1 } from '@dclutch/sdk/publicCutStaging';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';

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
 * NO MARKET LITERAL SURVIVES HERE, and each removal was paid for. The Open half
 * was a cohort-14 coordinate until cohort-15 landed, then a cohort-15
 * coordinate until cohort-16 landed, and both times it went red at the owner
 * check this file makes: a dead cohort's accounts are still readable, they are
 * just not owned by the program `DEVNET_DEPLOYMENT_V1` names. The address is
 * the cut's now, so a cohort boundary moves it and nothing here needs editing.
 *
 * BOTH CASES READ THE SAME ACCOUNT, ON PURPOSE, and they must still disagree.
 * A file that only ever sees a refusal cannot tell a working gate from a gate
 * that refuses everything, so the second case evaluates two acts against one
 * snapshot and asserts opposite verdicts: whichever phase the featured market
 * is in, exactly one of `source.close-fund` (Retiring+Consumed) and
 * `source.provider` (Open+Consumed) can admit it.
 *
 * WHAT COHORT-16 COSTS THIS FILE, stated rather than hidden: its featured
 * market is Open and cannot be activated at the deployed Direct release, so no
 * market on this cohort reaches Retiring and the ADMIT branch of
 * `source.close-fund` has no live subject. It is exercised offline instead --
 * `capabilityPhaseGate.test.ts`, "admits the one prestate close-fund declares"
 * -- which is a weaker evidence level than a real account and is named as one.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1`. Two account reads.
 */

/** The featured market, read out of the cut. Never a literal. */
const FEATURED_MARKET = PUBLIC_DEVNET_CUT_V1.market ?? '';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' && FEATURED_MARKET !== '' ? it : it.skip;

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
  live('refuses closing the fund of a Market outside Retiring+Consumed, by the route that refuses it', async () => {
    const snapshot = await readMarket(FEATURED_MARKET, DEVNET_DEPLOYMENT_V1.programs.core);
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

  live('admits exactly one of the two acts this Market’s phase can admit, and refuses the other', async () => {
    // Two verdicts off ONE read, and they must disagree. A test that only ever
    // sees a refusal cannot tell a working gate from a gate that refuses
    // everything. `source.close-fund` admits Retiring+Consumed and
    // `source.provider` admits Open+Consumed, so at most one of them can admit
    // any Market and the pair runs in both directions on one account whichever
    // phase the featured market is in.
    const snapshot = await readMarket(FEATURED_MARKET, DEVNET_DEPLOYMENT_V1.programs.core);
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
