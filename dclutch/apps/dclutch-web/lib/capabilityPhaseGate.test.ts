import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import {
  CAPABILITY_ACTIONS_V1,
  capabilityActPhaseGatesV1,
  capabilityActUnobservableMachinesV1,
  capabilityActsWithNoPhaseGateV1,
  capabilityPhaseGateTextV1,
  capabilityRequiresMarketV1,
  evaluateCapabilityV1,
  type CapabilityMarketSnapshotV1,
} from '@dclutch/sdk/capabilityModel';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from './capabilitySurface';
import {
  ROUTE_PHASE_GATES_V1,
  ROUTES_GATED_ON_ANOTHER_MACHINE_V1,
  ROUTES_WITHOUT_A_STATE_MACHINE_V1,
  routeHasNoStateMachineV1,
  routeOtherMachineGateV1,
  routePhaseGateV1,
} from '@dclutch/sdk/generated/marketPhaseAdmissionV1';

/**
 * The cohort-12 SOL/USD market, and the phase the chain reports for it.
 *
 * `apps/dclutch-web/fixtures/market-registry.devnet.json` names this address;
 * the phase is what a finalized read of its Core state decodes to, and it is
 * the observation the UX walk was looking at when `/workbench` reported READY
 * TO PREFLIGHT for acts about it. `Open` implies `Consumed`: `open_market`
 * sets both in one transition (`crates/dclutch-market`), so no
 * Market is ever `Open + Ready`.
 */
const COHORT_12 = 'EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1';

const observed = (
  phase: 'Founding' | 'Open' | 'Terminal' | 'Retiring' | 'Retired' | null,
  readiness: 'Prepaid' | 'Ready' | 'Consumed' | null,
): CapabilityMarketSnapshotV1 => ({ market: { address: COHORT_12, phase, readiness } });

const standing = (id: string) => {
  const found = BROWSER_CAPABILITY_STANDINGS_V1.find((one) => one.action.id === id);
  if (found === undefined) throw new Error(`no standing for ${id}`);
  return found;
};

describe('every route an act declares is a route the census enumerates', () => {
  // The check that keeps the per-act declaration from being prose. A route id
  // nobody carries would otherwise read exactly like an act with no gate.
  const routesPage = readFileSync(new URL('../../../docs/reference/routes.md', import.meta.url), 'utf8');
  const enumerated = new Set(
    [...routesPage.matchAll(/^\| `([^`]+)` \| (?:entry|action) \|/gm)].map(([, route]) => route),
  );

  it('names at least the routes routes.md publishes', () => {
    expect(enumerated.size).toBeGreaterThan(100);
  });

  it.each(CAPABILITY_ACTIONS_V1.filter((act) => act.routes.length > 0).map((act) => [act.id, act.routes] as const))(
    '%s names only enumerated routes',
    (_id, routes) => {
      for (const route of routes) expect(enumerated.has(route)).toBe(true);
    },
  );

  it('every published gate belongs to an enumerated route', () => {
    for (const gate of ROUTE_PHASE_GATES_V1) expect(enumerated.has(gate.route)).toBe(true);
  });
});

describe('the phase gate refuses by name and never asserts readiness', () => {
  it('refuses an act on cohort-12 whose route admits only another prestate', () => {
    // `source.provider` drives `core/execute_provider_v3::process#ExecuteProvider`,
    // whose guard admits Open+Consumed alone. Cohort-12 IS Open+Consumed, so
    // the honest verdict there is admitted -- the positive control, without
    // which "not ready" would prove nothing about the instrument.
    const provider = standing('source.provider');
    expect(evaluateCapabilityV1(provider, observed('Open', 'Consumed'), [])).toMatchObject({
      status: 'ready-to-preflight',
      phaseGate: { verdict: 'admitted' },
    });
    // The same act, the same market, one phase earlier in its life: refused,
    // by name, before any account is read.
    const early = evaluateCapabilityV1(provider, observed('Founding', 'Prepaid'), []);
    expect(early.status).toBe('wrong-phase');
    expect(early.phaseGate.verdict).toBe('excluded');
    expect(early.phaseGate.excludedBy?.route).toBe('core/execute_provider_v3::process#ExecuteProvider');
    expect(early.reason).toContain('admits only Open+Consumed');
    expect(early.reason).toContain('this Market is Founding+Prepaid');
  });

  it('refuses a resolution act on a retired market', () => {
    const ready = standing('source.ready');
    const retired = evaluateCapabilityV1(ready, observed('Retired', 'Consumed'), []);
    expect(retired.status).toBe('wrong-phase');
    expect(retired.phaseGate.excludedBy?.route).toBe('core/resolution::process#VerifyFundReady');
  });

  it('the Founding-only route the census carries excludes cohort-12', () => {
    // No act in the catalogue drives `OpenMarket` today, so this is asserted
    // where the fact actually lives: the table says a Founding+Ready route
    // cannot be attempted against an Open+Consumed Market, which is what a
    // consumer that gains such an act will read.
    const openMarket = routePhaseGateV1('core/open_market::process#OpenMarket');
    expect(openMarket).not.toBeNull();
    expect(openMarket?.prestates).toEqual([['Founding', 'Ready']]);
    expect(openMarket?.phases.includes('Open')).toBe(false);
  });

  it('will not call an act ready when the Market did not decode', () => {
    const provider = standing('source.provider');
    const unread = evaluateCapabilityV1(provider, observed(null, null), []);
    expect(unread.status).toBe('needs-chain');
    expect(unread.phaseGate.verdict).toBe('unread');
  });

  it('an act with no published gate says so by name instead of implying admission', () => {
    // `market.found` declares a real route, and that route has no prestate:
    // the Market it founds does not exist yet. Read with no Market held, so
    // the subject rule below is not what is under test here.
    const found = standing('market.found');
    expect(found.action.routes).toEqual(['core/found::process#Found']);
    expect(capabilityActPhaseGatesV1(found.action)).toHaveLength(0);
    const verdict = evaluateCapabilityV1(found, { market: null }, []);
    expect(verdict.status).toBe('ready-to-preflight');
    expect(verdict.phaseGate.verdict).toBe('no-phase-gate');
  });

  it('says a Registry route has no state to gate on, not that it declares none', () => {
    // "No gate was read" and "there is no state to read" are different
    // answers, and only the second is final. The Registry persists no
    // lifecycle discriminant at all, which the census declares and the
    // reference prints as its own column value; a card that kept saying
    // "declares none" would invite a reader to wait for a gate that no
    // further naming will ever produce.
    //
    // No shipped act declares a Registry route today, so this exercises the
    // branch directly rather than through `evaluateCapabilityV1`. That is the
    // whole point of exercising it: a branch with no caller and no test is a
    // branch nobody has ever run.
    expect(ROUTES_WITHOUT_A_STATE_MACHINE_V1.length).toBeGreaterThan(0);
    const machineless = ROUTES_WITHOUT_A_STATE_MACHINE_V1[0]!;
    expect(routeHasNoStateMachineV1(machineless)).toBe(true);
    const gate = (routes: ReadonlyArray<string>) => capabilityPhaseGateTextV1({
      routes, gates: [], verdict: 'no-phase-gate' as const, excludedBy: null, unobservableMachines: [],
      machineGates: [], selectedGates: [],
    });
    expect(gate([machineless])).toContain('persists no lifecycle state to gate on');

    // The control, without which the assertion above passes on any text: a
    // route in a program that DOES persist a discriminant keeps saying that
    // its guard was not read, because that one may still be named later.
    const named = 'core/found::process#Found';
    expect(routeHasNoStateMachineV1(named)).toBe(false);
    expect(gate([named])).toContain('declares none');
    expect(gate([named, machineless])).toContain('declares none');
  });

  it('publishes no route as both gated and machineless', () => {
    for (const route of ROUTES_WITHOUT_A_STATE_MACHINE_V1) {
      expect(routePhaseGateV1(route)).toBeNull();
      expect(routeOtherMachineGateV1(route)).toBeNull();
    }
  });

  it('names exactly which acts carry no gate, so the coverage cannot be mistaken for total', () => {
    const ungated = capabilityActsWithNoPhaseGateV1();
    const gated = CAPABILITY_ACTIONS_V1.map((act) => act.id).filter((id) => !ungated.includes(id));
    expect(gated).toEqual([
      'source.create-fund',
      // The seventh, and the first that is not a Market prestate at all.
      // `direct.inline` declares `trading/hot_v3::process_hot_execution_v3`
      // together with four other acts, and the census reads NO gate on that
      // route -- the Direct root's `Open` set sits behind
      // `hot_v3::direct::prepare_direct_inline_hot_crosscheck_v3`, which
      // returns `Ok(None)` for every request that is not a Direct successor.
      // So the gate belongs to this act's declared FAMILY, and to none of the
      // other four; before the family was derived it belonged to nobody and this
      // card read READY TO PREFLIGHT with a root state nobody had read.
      'direct.inline',
      'source.ready',
      'source.provider',
      'source.admit-terminal',
      // Gained a gate by gaining a DECLARATION, not by anyone naming a new
      // guard: `resolution/core_effect::process_direct_funding_close_v1`
      // admits `Retiring+Consumed` and has for as long as the census has read
      // it, while this act declared nothing and so reported READY TO
      // PREFLIGHT on every Market in every other phase.
      // `capabilityRouteDerivation.test.ts` is where the declaration comes
      // from.
      'source.close-fund',
      'claims.redeem',
    ]);
    expect(ungated).toHaveLength(CAPABILITY_ACTIONS_V1.length - gated.length);
  });
});

/**
 * A machine this snapshot cannot observe is not an admission and not an absence.
 *
 * `ROUTES_GATED_ON_ANOTHER_MACHINE_V1` exists because Resolution's sponsored
 * routes are gated on the SOURCE resolution state as well as the Market's
 * phase, and a Market is `Open` for the whole span in which its Source moves
 * `Primary` to `Resolved` -- so the Market half admitting is not the answer.
 *
 * No act in the catalogue declares one of those routes yet, and that is
 * exactly why this is here: a field whose only writer is a literal is a field
 * nobody has run. These cases drive the path with a standing built over a real
 * route id from the generated table, so the mechanism is exercised before an
 * act needs it rather than after.
 */
describe('an act gated on a machine this observation cannot read', () => {
  // Named, not positional. This was `ROUTES_GATED_ON_ANOTHER_MACHINE_V1[0]`,
  // which asserted a fact about ALPHABETICAL ORDER while reading as a fact
  // about the Source machine: naming the Dealer checkpoint machine put two
  // `custody/` rows ahead of it and turned three assertions red without any
  // of them being about what changed. The route below is the one the
  // assertions actually describe, and it is looked up by name.
  const sourceGated = routeOtherMachineGateV1('resolution/process_capture#Capture');
  expect(ROUTES_GATED_ON_ANOTHER_MACHINE_V1.length).toBeGreaterThan(0);

  const overRoute = (route: string) => {
    const base = standing('source.provider');
    return { ...base, action: { ...base.action, routes: [route] } };
  };

  it('the generated table names at least one such route, with its machine', () => {
    expect(sourceGated).toBeDefined();
    expect(sourceGated!.machines).toContain('source');
    expect(routeOtherMachineGateV1(sourceGated!.route)).toEqual(sourceGated);
    expect(routeOtherMachineGateV1('core/execute_provider_v3::process#ExecuteProvider')).toBeNull();
  });

  it('says needs-chain and names the machine, against an observation its Market half admits', () => {
    // The Market half of `resolution/process_capture#Capture` is
    // `Open+Consumed`, which this observation IS. A reader that answered from
    // the Market alone would call it ready.
    const verdict = evaluateCapabilityV1(overRoute(sourceGated!.route), observed('Open', 'Consumed'), []);
    expect(verdict.status).toBe('needs-chain');
    expect(verdict.phaseGate.verdict).toBe('other-machine');
    expect(verdict.phaseGate.unobservableMachines).toEqual(['source']);
    expect(verdict.reason).toContain('source state machine');
  });

  it('is not counted as an act with no published gate', () => {
    // The census read a gate for it. Calling it ungated would be the same
    // false claim `no-phase-gate` exists one level down to prevent.
    expect(capabilityActUnobservableMachinesV1(overRoute(sourceGated!.route).action)).toEqual(['source']);
    expect(capabilityActUnobservableMachinesV1(standing('source.provider').action)).toEqual([]);
  });
});

/**
 * The other half of the UX walk's row O1, which the phase gates could not fix.
 *
 * A phase gate answers "may this Market be asked to do this now?". It has
 * nothing to say about an act whose Market does not exist yet, and
 * `core/found::process#Found` correctly publishes no prestate. What made that
 * card wrong was that `requiresMarket: boolean` could not tell "no Market" from
 * "creates a Market", so an act about a Market that does not exist rendered
 * READY TO PREFLIGHT beside an open one it can never touch.
 */
describe('an act that founds a Market is not about the Market on screen', () => {
  it('refuses ready by name against cohort-12, and says which Market it is holding', () => {
    const found = standing('market.found');
    expect(found.action.subject).toBe('new-market');
    const verdict = evaluateCapabilityV1(found, observed('Open', 'Consumed'), []);
    expect(verdict.status).toBe('not-this-market');
    expect(verdict.reason).toContain('founds a NEW Market');
    expect(verdict.reason).toContain(COHORT_12);
    expect(verdict.reason).toContain('Open');
    expect(verdict.reason).toContain('clear the Market coordinate');
  });

  it('is ready with no Market held, because founding one genuinely is', () => {
    // The positive control. Without it "not ready" would prove nothing: an act
    // that never reads ready is not a verdict, it is a wall.
    for (const id of ['market.found', 'market.inspect']) {
      expect(evaluateCapabilityV1(standing(id), { market: null }, [])).toMatchObject({
        status: 'ready-to-preflight',
      });
    }
  });

  it('refuses whatever phase the held Market is in, not only an open one', () => {
    // The subject is wrong at every phase. A rule keyed on `Open` would pass a
    // Founding Market through and report ready for founding a second one.
    for (const phase of ['Founding', 'Open', 'Terminal', 'Retiring', 'Retired'] as const) {
      expect(evaluateCapabilityV1(standing('market.found'), observed(phase, 'Consumed'), []).status)
        .toBe('not-this-market');
    }
    expect(evaluateCapabilityV1(standing('market.found'), observed(null, null), []).status)
      .toBe('not-this-market');
  });

  it('does not fire for an act that simply has no Market', () => {
    // Keyed on the subject, not on holding a Market at all: `release.activate`
    // is about a release set and stays ready with cohort-12 on screen.
    const release = standing('release.activate');
    expect(release.action.subject).toBe('no-market');
    expect(evaluateCapabilityV1(release, observed('Open', 'Consumed'), []).status)
      .toBe('ready-to-preflight');
  });

  it('pins which acts declare which subject, so a new act cannot pick one quietly', () => {
    const bySubject = (subject: string) =>
      CAPABILITY_ACTIONS_V1.filter((act) => act.subject === subject).map((act) => act.id);
    expect(bySubject('new-market')).toEqual(['market.inspect', 'market.found']);
    expect(bySubject('no-market')).toEqual(['release.activate', 'product.compile', 'direct.route']);
    expect(bySubject('observed-market')).toHaveLength(
      CAPABILITY_ACTIONS_V1.length - 2 - 3,
    );
    // `requiresMarket` is derived and has exactly one true case.
    for (const act of CAPABILITY_ACTIONS_V1) {
      expect(capabilityRequiresMarketV1(act)).toBe(act.subject === 'observed-market');
    }
  });
});
