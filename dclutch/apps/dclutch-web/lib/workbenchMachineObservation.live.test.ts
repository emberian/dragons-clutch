import { describe, expect, it } from 'vitest';

import {
  acquireMachineObservationsV1,
  MARKET_DERIVABLE_MACHINES_V1,
  type MachineObservationV1,
} from '@dclutch/sdk/stateMachines';
import { routeSelectedGatesV1 } from '@dclutch/sdk/generated/marketPhaseAdmissionV1';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from './capabilitySurface';
import { evaluateCapabilityV1, type CapabilityMarketSnapshotV1 } from './capabilityModel';
import { decodeMarketCoreStateV2 } from './marketCoreV2';
import { SolanaRpcClient } from './rpc';

/**
 * `/workbench`'s own acquisition, against the chain that wrote the accounts.
 *
 * WHAT THIS IS THE OTHER HALF OF. `capabilityMachineGate.live.test.ts` reads a
 * Market and shows that a route's OTHER machine is named and unanswered; the
 * card for `direct.inline` then said `needs-chain` on every Market ever
 * selected. The missing thing was never the decoder. It was the ADDRESS: the
 * Direct root was treated as a coordinate arriving from a route manifest, and
 * it is not one -- it is a forward projection of the Market's own generation
 * and the capability manifest its header commits to. So this reads what
 * `/workbench` reads, from the same coordinates a person types into it, and
 * asserts the card that comes out.
 *
 * THE THREE MARKETS ARE THREE ANSWERS, and they are named rather than taken
 * from the public cut because the cut follows whatever is featured and these
 * cases want Direct roots that DISAGREE. All three carry the Direct successor
 * kind in their manifests; what differs is what has happened to the root:
 *
 *   * `C9dLhWj7…` — activated, its root live;
 *   * `3QytL1bB…` — activated and since driven through
 *     `direct_begin_retiring_v1` (cohort-15 evidence D7), so its root has left
 *     the state the inline crosscheck admits while the account still exists;
 *   * `4phPvYy8…` — FOUNDED AND NEVER ACTIVATED. Its manifest names the same
 *     Direct entry and no account exists at the derived address, which is the
 *     hostile: a card here must read `needs-chain` naming direct-root and must
 *     never report a state, least of all the one its sibling holds.
 *
 * EVERY ASSERTION IS AN AGREEMENT, never a state literal. What is pinned is
 * that the verdict the card prints is the one the census's own published set
 * says about the state that decoded -- so a root that advances changes which
 * branch runs and not whether this passes, and a set widened to admit
 * everything fails the disjointness rather than passing.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1`. Four account reads per market.
 */

const COHORT15 = Object.freeze({
  core: '7hGerMC6Wj742FVTyiF9PhRnGSBzbee7TMZ6sUytsmFr',
  trading: '3gBSSjYwSC4phutpGKRkMhrnCDVzHu6kfQ3L4jLf2UmG',
  resolution: '24AkUjtXg61La45u7KTge8u4dKpVqkzirmzycVyckFgn',
});

/** A Market whose Direct capability is activated and whose root is live. */
const ACTIVATED_MARKET = 'C9dLhWj7yi76RtQhhHV13gKuudAbV8qio8TZVEn3CjAT';
/** A Market driven through `direct_begin_retiring_v1`; its root still exists. */
const RETIRED_ROOT_MARKET = '3QytL1bBMtCvRoXWR5h7MgutRBZqtv7emUVubEo5a4T2';
/** A Market founded with the same Direct entry and never activated. */
const UNACTIVATED_MARKET = '4phPvYy8tTNPujxSfa2TnsUgfr14gFg9irG52q5pmSPS';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

const DIRECT_INLINE = (() => {
  const found = BROWSER_CAPABILITY_STANDINGS_V1.find((one) => one.action.id === 'direct.inline');
  if (found === undefined) throw new Error('no standing for direct.inline');
  return found;
})();

/**
 * The set the Direct family's classifier admits, from the generated census.
 *
 * Read here rather than written down: the whole point of the assertions below
 * is that they compare a decoded state against what the CENSUS publishes, and
 * a copy of that set in this file would make the comparison circular.
 */
const DIRECT_ROOT_SET = (() => {
  const gates = routeSelectedGatesV1('trading/hot_v3::process_hot_execution_v3')
    .filter((gate) => gate.machine === 'direct-root');
  if (gates.length !== 1) throw new Error(`expected one direct-root selected gate, found ${gates.length}`);
  return gates[0]!.states;
})();

const client = () => new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? 'https://api.devnet.solana.com');

/** Exactly what `/workbench` does: read the Market, then acquire from it. */
async function observe(market: string): Promise<Readonly<{
  snapshot: CapabilityMarketSnapshotV1;
  machines: ReadonlyArray<MachineObservationV1>;
}>> {
  const rpc = client();
  const observation = await rpc.accountInfo(market);
  expect(observation.account, `no account at ${market}`).not.toBeNull();
  expect(observation.account!.owner, `${market} is not cohort-15 Core-owned`).toBe(COHORT15.core);
  const state = decodeMarketCoreStateV2(market, observation.account!.data);
  const machines = await acquireMachineObservationsV1(
    rpc,
    {
      address: market,
      generation: BigInt(state.identity.generation),
      capabilityManifestId: state.identity.capabilityManifestId,
      registryProgram: state.identity.registryProgram,
    },
    COHORT15.resolution,
    COHORT15.trading,
  );
  return Object.freeze({
    snapshot: Object.freeze({
      market: Object.freeze({ address: market, phase: state.phase, readiness: state.readiness }),
    }),
    machines,
  });
}

const rootOf = (machines: ReadonlyArray<MachineObservationV1>): MachineObservationV1 => {
  const found = machines.find((one) => one.machine === 'direct-root');
  if (found === undefined) throw new Error('the acquisition named no direct-root observation');
  return found;
};

describe('live devnet /workbench machine acquisition', () => {
  /**
   * The activated Market: a root read, and a card that says so.
   *
   * The account was never named to this test. Its address came out of the
   * Market's own header through the manifest that header commits to, and the
   * only reason a state decodes at all is that the derivation is right.
   */
  live('derives an activated Market’s Direct root and answers the card from it', async () => {
    const { snapshot, machines } = await observe(ACTIVATED_MARKET);
    const root = rootOf(machines);
    expect(root.refusal, `direct-root refused: ${root.refusal}`).toBeNull();
    expect(root.present, `no account at the derived Direct root for ${ACTIVATED_MARKET}`).toBe(true);
    expect(root.state).not.toBeNull();

    const verdict = evaluateCapabilityV1(DIRECT_INLINE, snapshot, machines);
    const admitted = DIRECT_ROOT_SET.includes(root.state!);
    expect(verdict.status).toBe(admitted ? 'ready-to-preflight' : 'wrong-phase');
    // Whichever branch ran, the card is answered BY THE MACHINE and no longer
    // by an absence: a selected gate with a state on it, not `needs-chain`.
    const [selected] = verdict.phaseGate.selectedGates;
    expect(selected!.machine).toBe('direct-root');
    expect(selected!.observed).toBe(root.state);
    expect(selected!.verdict).toBe(admitted ? 'admitted' : 'excluded');
    expect(verdict.phaseGate.unobservableMachines).toEqual([]);
  }, 90_000);

  /**
   * The same derivation on a Market whose root has left that set.
   *
   * The control the activated case cannot give on its own: two roots of one
   * cohort, read the same way, that do not agree. A decoder returning a
   * constant, or a derivation that quietly named one account for both, fails
   * here rather than passing twice.
   */
  live('refuses the card by name when the derived root is past the admitted set', async () => {
    const activated = rootOf((await observe(ACTIVATED_MARKET)).machines);
    const { snapshot, machines } = await observe(RETIRED_ROOT_MARKET);
    const root = rootOf(machines);
    expect(root.present, `no account at the derived Direct root for ${RETIRED_ROOT_MARKET}`).toBe(true);
    expect(root.state).not.toBeNull();
    expect(root.state).not.toBe(activated.state);

    const verdict = evaluateCapabilityV1(DIRECT_INLINE, snapshot, machines);
    const admitted = DIRECT_ROOT_SET.includes(root.state!);
    expect(admitted).toBe(false);
    expect(verdict.status).toBe('wrong-phase');
    // The refusal names the machine, the set and what was read -- the state
    // taken from the decoder, so a set widened to admit everything would fail
    // the line above rather than pass this one.
    expect(verdict.reason).toContain('direct-root');
    expect(verdict.reason).toContain(root.state!);
  }, 120_000);

  /**
   * THE HOSTILE. A Market founded and never activated.
   *
   * Its manifest carries the same Direct entry as its activated sibling, so
   * the address is derived and the read is made; nothing is there. The card
   * must say the root is unread and must never report a state -- and the
   * positive control is that the SAME code path, on the sibling above, does
   * report one.
   */
  live('reports a founded, never-activated Market’s root as unobserved and never as a state', async () => {
    // THE POSITIVE CONTROL, first and on the same code path. "Nothing is
    // there" and "this reader stopped reading" log identically, and only the
    // second is a defect -- so the sibling's root is read here too, and an
    // acquisition that had quietly stopped looking would fail on this line
    // rather than pass the absence below.
    expect(rootOf((await observe(ACTIVATED_MARKET)).machines).present).toBe(true);

    const { snapshot, machines } = await observe(UNACTIVATED_MARKET);
    const root = rootOf(machines);
    expect(root.present).toBe(false);
    expect(root.state).toBeNull();
    expect(root.refusal).toBeNull();

    const verdict = evaluateCapabilityV1(DIRECT_INLINE, snapshot, machines);
    expect(verdict.status).toBe('needs-chain');
    expect(verdict.phaseGate.unobservableMachines).toEqual(['direct-root']);
    const [selected] = verdict.phaseGate.selectedGates;
    expect(selected!.verdict).toBe('unobserved');
    expect(selected!.observed).toBeNull();
    expect(selected!.reason).toContain('does not exist');
    for (const state of DIRECT_ROOT_SET) expect(verdict.reason).not.toContain(state);
  }, 90_000);

  /**
   * The funding ledger, derived beside the root off the same manifest entry.
   *
   * It answers no card today -- no act on the board declares a route the
   * census gates on `funding-ledger` -- and it is read anyway, because the
   * manifest read that names the root has already bought the address and a
   * machine a surface CAN observe and does not is the gap this lane closed.
   */
  live('derives the Trading funding ledger of the same entry, and its slot decodes', async () => {
    const { machines } = await observe(ACTIVATED_MARKET);
    expect([...machines.map((one) => one.machine)].sort())
      .toEqual([...MARKET_DERIVABLE_MACHINES_V1].sort());
    const ledger = machines.find((one) => one.machine === 'funding-ledger')!;
    expect(ledger.refusal, `funding-ledger refused: ${ledger.refusal}`).toBeNull();
    expect(ledger.present, 'no account at the derived Trading funding ledger').toBe(true);
    expect(ledger.state).not.toBeNull();

    // The unactivated sibling has no ledger either, and the two answers come
    // from one code path: absence here is read, not assumed.
    const unactivated = (await observe(UNACTIVATED_MARKET)).machines
      .find((one) => one.machine === 'funding-ledger')!;
    expect(unactivated.present).toBe(false);
    expect(unactivated.state).toBeNull();
  }, 120_000);
});
