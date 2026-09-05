import { describe, expect, it } from 'vitest';

import { ROUTE_SELECTED_GATES_V1 } from '@dclutch/sdk/generated/marketPhaseAdmissionV1';
import { decodeDirectRootStateV1, machineObservationV1 } from '@dclutch/sdk/stateMachines';
import { CAPABILITY_ROOT_HEADER_BYTES_V1 } from '@dclutch/sdk/generated/directInlineV3';
import {
  HOT_FAMILY_CLASSIFIERS_V1,
  evaluateCapabilityV1,
  selectedTextV1,
  type CapabilityMarketSnapshotV1,
} from '@dclutch/sdk/capabilityModel';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from './capabilitySurface';
import { decodeMarketCoreStateV2 } from '@dclutch/sdk/marketCoreV2';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';

/**
 * The first verdict this browser derives from a machine that is not the Market.
 *
 * Until the families were declared, every act on
 * `trading/hot_v3::process_hot_execution_v3` answered from the Market phase
 * alone, because the census reads no gate on that route: the Direct root's
 * `Open` set sits behind `hot_v3::direct::prepare_direct_inline_hot_crosscheck_v3`,
 * which declines every request that is not a Direct successor. So the gate
 * existed, the decoder existed, the account existed, and no card asked.
 *
 * This is the ask, against the chain. Two finalized reads: cohort-15's founded
 * Market, for the phase half, and the activation root whose 24-byte Direct
 * tail follows the composite capability-root header. The coordinates come from
 * `docs/evidence/COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md`
 * sections 3 and 8 rather than from the public cut, which follows whatever is
 * featured.
 *
 * Every assertion is the AGREEMENT between the decoded root state and the set
 * the census published, never a state literal: a root that begins retiring
 * changes which branch runs and not whether this passes. The four other acts
 * on the same route are evaluated against the SAME observation, which is the
 * control -- an arm that answered everybody would refuse them too.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1`. Two account reads.
 */

/** Cohort-15's Trading program, which owns the activation root. */
const COHORT15_TRADING_PROGRAM = '3gBSSjYwSC4phutpGKRkMhrnCDVzHu6kfQ3L4jLf2UmG';
/** Cohort-15's Core program, which owns the founded Market. */
const COHORT15_CORE_PROGRAM = '7hGerMC6Wj742FVTyiF9PhRnGSBzbee7TMZ6sUytsmFr';
/** The founded Open Market, section 8. */
const COHORT15_OPEN_MARKET = '3QytL1bBMtCvRoXWR5h7MgutRBZqtv7emUVubEo5a4T2';
/** The activation root section 8 names; the Direct family's tail follows the header. */
const COHORT15_ACTIVATION_ROOT = 'FUJ9pNukWRb5658ysiDP5gz9gF3Hx8c4cU2mUrvELAWG';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

const client = () => new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? 'https://api.devnet.solana.com');

const standing = (id: string) => {
  const found = BROWSER_CAPABILITY_STANDINGS_V1.find((one) => one.action.id === id);
  if (found === undefined) throw new Error(`no standing for ${id}`);
  return found;
};

async function read(address: string, owner: string): Promise<Uint8Array> {
  const observation = await client().accountInfo(address);
  expect(observation.account, `no account at ${address}`).not.toBeNull();
  expect(observation.account!.owner, `${address} is not owned by ${owner}`).toBe(owner);
  return observation.account!.data;
}

describe('live devnet gate behind a classifier', () => {
  live('answers direct.inline from cohort-15’s Direct root, and asks nobody else', async () => {
    const classifier = HOT_FAMILY_CLASSIFIERS_V1.find((entry) => entry.family === 'Direct');
    expect(classifier, 'no Direct classifier is bound').toBeDefined();
    const gate = ROUTE_SELECTED_GATES_V1.find((entry) => entry.selectedBy === classifier!.classifier);
    expect(gate, `the census publishes no gate behind ${classifier!.classifier}`).toBeDefined();

    const market = decodeMarketCoreStateV2(
      COHORT15_OPEN_MARKET, await read(COHORT15_OPEN_MARKET, COHORT15_CORE_PROGRAM),
    );
    const snapshot: CapabilityMarketSnapshotV1 = {
      market: { address: COHORT15_OPEN_MARKET, phase: market.phase, readiness: market.readiness },
    };

    const account = await read(COHORT15_ACTIVATION_ROOT, COHORT15_TRADING_PROGRAM);
    const decode = decodeDirectRootStateV1(account.subarray(CAPABILITY_ROOT_HEADER_BYTES_V1));
    expect(decode.status, decode.status === 'refused' ? decode.reason : '').toBe('decoded');
    if (decode.status !== 'decoded') return;
    const observations = [machineObservationV1(decode)];

    // The verdict, derived: machine, family, classifier, observed state.
    const verdict = evaluateCapabilityV1(standing('direct.inline'), snapshot, observations);
    expect(verdict.phaseGate.selectedGates).toHaveLength(1);
    const answered = verdict.phaseGate.selectedGates[0]!;
    expect(answered.machine).toBe(gate!.machine);
    expect(answered.family).toBe('Direct');
    expect(answered.selectedBy).toBe(gate!.selectedBy);
    expect(answered.observed).toBe(decode.state);
    const admits = gate!.states.includes(decode.state);
    expect(answered.verdict).toBe(admits ? 'admitted' : 'excluded');
    expect(verdict.status).toBe(admits ? 'ready-to-preflight' : 'wrong-phase');
    expect(answered.reason).toContain(gate!.machine);
    expect(answered.reason).toContain(decode.state);
    // The card's own clause names the classifier and the family it belongs to.
    const clause = selectedTextV1(verdict.phaseGate).join('; ');
    expect(clause).toContain(gate!.selectedBy);
    expect(clause).toContain('Direct');

    // The control, on the identical observation. Four acts declare the same
    // route and none is a Direct successor, so the crosscheck declines before
    // it reads anything and the root is not their gate.
    for (const id of ['general.consider', 'general.settle', 'general.close', 'dealer.liquidity']) {
      const other = evaluateCapabilityV1(standing(id), snapshot, observations);
      expect(other.standing.action.routes).toContain(gate!.route);
      expect(other.phaseGate.selectedGates, `${id} was asked about ${gate!.machine}`).toEqual([]);
      expect(other.reason).not.toContain(gate!.machine);
    }
  }, 60_000);
});
