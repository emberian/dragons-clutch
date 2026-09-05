import { describe, expect, it } from 'vitest';

import vector from '@dclutch/sdk/fixtures/state-machines.devnet.json';
import {
  ROUTES_GATED_ON_ANOTHER_MACHINE_V1,
  routeMachineStatesV1,
} from '@dclutch/sdk/generated/marketPhaseAdmissionV1';
import { STATE_MACHINE_RECORDS_V1 } from '@dclutch/sdk/generated/stateMachinesV1';
import {
  absentMachineObservationV1,
  decodeMachineStateV1,
  machineGateCoverageV1,
  machineGateSentenceV1,
  machineObservationV1,
  type MachineObservationV1,
  type StateMachineV1,
} from '@dclutch/sdk/stateMachines';
import {
  capabilityActMachineGatesV1,
  capabilityPhaseGateTextV1,
  evaluateCapabilityV1,
  machineTextV1,
  type CapabilityMarketSnapshotV1,
} from '@dclutch/sdk/capabilityModel';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from './capabilitySurface';

/**
 * The other-machine verdict, answered from a decoded machine.
 *
 * WHAT THIS REPLACES. `evaluateCapabilityV1` used to compute
 * `unobservableMachines` from an act's DECLARED ROUTES alone, so an act gated
 * on a Direct root reported `needs-chain` whether or not the caller held that
 * root's bytes — and no caller could hold them, because no client surface
 * decoded one. The answer was therefore fixed at "I cannot say" for every
 * observation forever, which is indistinguishable on a card from "the account
 * is not there". Both halves are now answerable, and this file is where the
 * three outcomes are held apart: admitted, refused by the machine's name, and
 * genuinely unread.
 *
 * WHY THE STANDINGS ARE SYNTHETIC. Because the intersection is empty, and that
 * is a measured fact rather than an oversight — the first case below states it.
 * No act in the catalogue declares a route the census gates on another machine,
 * so driving this path through a shipped act is impossible today. A mechanism
 * whose only exercise waits for an act that does not exist is a mechanism
 * nobody has run, which is the defect `capabilityPhaseGate.test.ts` already
 * names one level up. So the standings here are built over REAL route ids from
 * the generated table, and the observations over REAL bytes read off cohort-15.
 */

const COHORT_15_OPEN_MARKET = '3QytL1bBMtCvRoXWR5h7MgutRBZqtv7emUVubEo5a4T2';

const observed = (
  phase: 'Founding' | 'Open' | 'Terminal' | 'Retiring' | 'Retired' | null,
  readiness: 'Prepaid' | 'Ready' | 'Consumed' | null,
): CapabilityMarketSnapshotV1 => ({ market: { address: COHORT_15_OPEN_MARKET, phase, readiness } });

const standing = (id: string) => {
  const found = BROWSER_CAPABILITY_STANDINGS_V1.find((one) => one.action.id === id);
  if (found === undefined) throw new Error(`no standing for ${id}`);
  return found;
};

/** A standing over one real census route, so no literal route id is typed. */
const overRoute = (route: string) => {
  const base = standing('source.provider');
  return { ...base, action: { ...base.action, routes: [route] } };
};

const hex = (value: string): Uint8Array =>
  Uint8Array.from((value.match(/../g) ?? []).map((byte) => Number.parseInt(byte, 16)));

/** One machine observation built from bytes this repository read off chain. */
function chainObservation(machine: StateMachineV1, address: string): MachineObservationV1 {
  const record = vector.records.find((row) => row.machine === machine && row.address === address);
  if (record === undefined) throw new Error(`no ${machine} record for ${address} in the devnet vector`);
  return machineObservationV1(decodeMachineStateV1(machine, hex(record.recordHex)));
}

const DIRECT_ROOT = 'FUJ9pNukWRb5658ysiDP5gz9gF3Hx8c4cU2mUrvELAWG';
const SOURCE_PRIMARY = '5QoawNdiiBtggmeFs81UsejxC5XwWayfPFsswN1redBr';
const SOURCE_RESOLVED = 'JAz42gc4tRTKFEWVzELAHe5tvYUG3SXkQJFVtWrRa5ka';

describe('the intersection of declared routes and machine-gated routes', () => {
  /**
   * Stated as a checked number rather than as prose.
   *
   * `/console` prints this same coverage, computed on every render from the
   * same two tables, so the page and this assertion cannot disagree. When an
   * act finally declares such a route the count moves and this case says so
   * instead of quietly continuing to pass.
   */
  it('is empty, over 32 gated routes and 12 declared ones', () => {
    const coverage = machineGateCoverageV1(BROWSER_CAPABILITY_STANDINGS_V1.map((one) => one.action));
    expect(coverage.gatedRoutes).toBe(ROUTES_GATED_ON_ANOTHER_MACHINE_V1.length);
    expect(coverage.declaredRoutes).toBeGreaterThan(0);
    expect(coverage.intersection).toEqual([]);
    // Every machine the census gates on is decodable, which is the change:
    // before this lane the figure was zero of six.
    expect(coverage.decodable).toEqual(coverage.machines);
    expect(coverage.machines).toEqual([
      'direct-root', 'funding-ledger', 'projected-custody', 'series-ticket', 'source',
    ]);
    // Every act's machine gate list is therefore empty, and that is the
    // consequence rather than a second assumption.
    for (const one of BROWSER_CAPABILITY_STANDINGS_V1) {
      expect(capabilityActMachineGatesV1(one.action, []), one.action.id).toEqual([]);
    }
  });

  it('says so in the sentence the console prints', () => {
    const sentence = machineGateSentenceV1(
      machineGateCoverageV1(BROWSER_CAPABILITY_STANDINGS_V1.map((one) => one.action)),
    );
    expect(sentence).toContain('None of the');
    // Scoped to what this coverage measures. It read "no card here is yet
    // answered by a machine" while a route gate was the only kind there was;
    // a gate behind a family's classifier is in neither table this counts and
    // IS answered on a card (`capabilitySelectedGate.test.ts`), so the
    // unqualified claim would now contradict the sentence printed beside it.
    expect(sentence).toContain('no card here is yet answered by a gate the route itself carries');
    // The control: a coverage whose intersection is NOT empty reads differently,
    // so the sentence is not one string with a number in it.
    const withOne = machineGateCoverageV1([{ routes: [ROUTES_GATED_ON_ANOTHER_MACHINE_V1[0]!.route] }]);
    expect(machineGateSentenceV1(withOne)).not.toContain('None of the');
    expect(machineGateSentenceV1(withOne)).toContain(ROUTES_GATED_ON_ANOTHER_MACHINE_V1[0]!.route);
  });
});

describe('a machine gate answered from a decoded observation', () => {
  /**
   * The hostile, per machine: an act gated on a state the machine is not in
   * refuses BY THE MACHINE'S NAME, on bytes read off the chain.
   */
  it('refuses a Direct maker close against a root that is Open', () => {
    const route = 'trading/direct_close_maker_v1::process_direct_close_maker_v1';
    expect(routeMachineStatesV1(route, 'direct-root')).toEqual(['Retiring']);
    const root = chainObservation('direct-root', DIRECT_ROOT);
    const verdict = evaluateCapabilityV1(overRoute(route), observed('Open', 'Consumed'), [root]);

    expect(verdict.status).toBe('wrong-phase');
    expect(verdict.phaseGate.verdict).toBe('excluded');
    expect(verdict.phaseGate.unobservableMachines).toEqual([]);
    expect(verdict.reason).toContain('direct-root Retiring');
    expect(verdict.reason).toContain(root.state!);
    expect(verdict.reason).toContain('before any account is read');
    expect(capabilityPhaseGateTextV1(verdict.phaseGate)).toContain('admits only direct-root Retiring');
  });

  it('admits a Direct token setup against the same root, at the same observation', () => {
    // The positive control the refusal above needs: a gate that refused
    // everything would pass that case, and this one shares its observation.
    const route = 'trading/direct_token_setup_v1::process_direct_token_setup_v1';
    expect(routeMachineStatesV1(route, 'direct-root')).toEqual(['Open']);
    const root = chainObservation('direct-root', DIRECT_ROOT);
    const verdict = evaluateCapabilityV1(overRoute(route), observed('Open', 'Consumed'), [root]);
    expect(verdict.status).toBe('ready-to-preflight');
    expect(verdict.phaseGate.verdict).toBe('admitted');
    expect(machineTextV1(verdict.phaseGate)).toEqual([`direct-root ${root.state} admitted against Open`]);
  });

  /**
   * The half-answer this whole chain replaced, in its machine form.
   *
   * `resolution/process_capture#Capture` is gated `market: Open+Consumed` AND
   * `source: Primary`. The observation below is `Open+Consumed`, so a reader
   * that answered from the Market alone would call it ready — and on the
   * resolved Source it must refuse instead.
   */
  it('refuses a capture on a resolved Source whose Market half admits', () => {
    const route = 'resolution/process_capture#Capture';
    const resolved = chainObservation('source', SOURCE_RESOLVED);
    const primary = chainObservation('source', SOURCE_PRIMARY);
    expect(resolved.state).not.toBe(primary.state);

    const refused = evaluateCapabilityV1(overRoute(route), observed('Open', 'Consumed'), [resolved]);
    expect(refused.status).toBe('wrong-phase');
    expect(refused.reason).toContain('source Primary');

    const admitted = evaluateCapabilityV1(overRoute(route), observed('Open', 'Consumed'), [primary]);
    expect(admitted.status).toBe('ready-to-preflight');
    expect(admitted.phaseGate.verdict).toBe('admitted');
  });

  /**
   * `needs-chain` survives, narrowed to what it always claimed to mean.
   *
   * Never read, read and absent, and read with the WRONG machine in hand are
   * three ways to have no observation of this one, and none of them may admit.
   */
  it('still says needs-chain, by machine name, when that machine was not observed', () => {
    const route = 'resolution/process_capture#Capture';
    const cases: ReadonlyArray<ReadonlyArray<MachineObservationV1>> = [
      [],
      [absentMachineObservationV1('source')],
      [chainObservation('direct-root', DIRECT_ROOT)],
    ];
    for (const machines of cases) {
      const verdict = evaluateCapabilityV1(overRoute(route), observed('Open', 'Consumed'), machines);
      expect(verdict.status).toBe('needs-chain');
      expect(verdict.phaseGate.verdict).toBe('other-machine');
      expect(verdict.phaseGate.unobservableMachines).toEqual(['source']);
      expect(capabilityPhaseGateTextV1(verdict.phaseGate)).toContain('source');
    }
  });

  /**
   * A machine refusal outranks an unread sibling.
   *
   * `core/series_consume::process` is gated on two machines. With one excluded
   * and the other unread the answer is the refusal: an act whose projection is
   * in the wrong phase cannot become attemptable by reading a ticket.
   */
  it('publishes the refusal when one machine excludes and another is unread', () => {
    const route = 'core/series_consume::process';
    const gates = routeMachineStatesV1(route, 'projected-custody');
    expect(gates).not.toBeNull();
    const wrong = STATE_MACHINE_RECORDS_V1.find((record) => record.machine === 'projected-custody')!
      .states.map((state) => state.state).find((state) => !gates!.includes(state))!;
    const observation: MachineObservationV1 = { machine: 'projected-custody', present: true, state: wrong, refusal: null };
    const verdict = evaluateCapabilityV1(overRoute(route), observed('Open', 'Consumed'), [observation]);
    expect(verdict.status).toBe('wrong-phase');
    expect(verdict.reason).toContain('projected-custody');
    expect(verdict.reason).toContain(wrong);
    // The unread sibling is still reported rather than dropped.
    expect(verdict.phaseGate.unobservableMachines).toEqual(['series-ticket']);
  });

  /**
   * The hostile, for EVERY machine the census gates on rather than the three
   * that happen to have a live account.
   *
   * A per-machine claim proven on one machine is a claim about that machine.
   * This walks the generated table, picks for each machine a route that gates
   * on it and a state of that machine the route does NOT admit, and requires
   * the refusal to name the machine, its set and the observed state. A machine
   * whose every state is admitted by every one of its routes is skipped and
   * counted, so the case cannot pass by silently walking nothing.
   */
  it('refuses by name for every machine the census gates a route on', () => {
    const covered: string[] = [];
    const unrefutable: string[] = [];
    for (const record of STATE_MACHINE_RECORDS_V1) {
      const entry = ROUTES_GATED_ON_ANOTHER_MACHINE_V1.find((row) => row.machines.includes(record.machine));
      if (entry === undefined) continue;
      const admitted = routeMachineStatesV1(entry.route, record.machine)!;
      const wrong = record.states.map((state) => state.state).find((state) => !admitted.includes(state));
      if (wrong === undefined) { unrefutable.push(record.machine); continue; }
      const observation: MachineObservationV1 = {
        machine: record.machine, present: true, state: wrong, refusal: null,
      };
      const verdict = evaluateCapabilityV1(overRoute(entry.route), observed('Open', 'Consumed'), [observation]);
      expect(verdict.status, `${record.machine} on ${entry.route}`).toBe('wrong-phase');
      expect(verdict.reason).toContain(`${record.machine} ${admitted.join(' or ')}`);
      expect(verdict.reason).toContain(wrong);
      // The positive control on the same route: a state it DOES admit stops
      // refusing BY THIS MACHINE. Not "stops refusing" -- several of these
      // routes carry a Market gate as well, and `direct_begin_retiring_v1`
      // wants `market: Retiring` against an observation that is Open, so the
      // whole verdict is still `wrong-phase` for a reason that is not this
      // machine's. A control that asserted the status would be asserting the
      // Market half by accident.
      const admits = evaluateCapabilityV1(overRoute(entry.route), observed('Open', 'Consumed'), [
        { machine: record.machine, present: true, state: admitted[0]!, refusal: null },
      ]);
      expect(
        admits.phaseGate.machineGates.filter((gate) => gate.verdict === 'excluded'),
        `${record.machine} on ${entry.route}`,
      ).toEqual([]);
      covered.push(record.machine);
    }
    // Every machine that carries a route gate is covered or explicitly named.
    const gated = machineGateCoverageV1(BROWSER_CAPABILITY_STANDINGS_V1.map((one) => one.action)).machines;
    expect([...covered, ...unrefutable].sort()).toEqual([...gated].sort());
    expect(unrefutable, 'a machine every route admits entirely cannot be refuted').toEqual([]);
  });

  it('leaves every act with no machine gate exactly as it was', () => {
    // The regression this parameter could have caused: an act with no machine
    // gate must answer identically whatever is passed.
    const withNothing = evaluateCapabilityV1(standing('source.provider'), observed('Open', 'Consumed'), []);
    const withPlenty = evaluateCapabilityV1(standing('source.provider'), observed('Open', 'Consumed'), [
      chainObservation('direct-root', DIRECT_ROOT),
      chainObservation('source', SOURCE_RESOLVED),
    ]);
    expect(withPlenty).toEqual(withNothing);
    expect(withNothing.status).toBe('ready-to-preflight');
  });
});
