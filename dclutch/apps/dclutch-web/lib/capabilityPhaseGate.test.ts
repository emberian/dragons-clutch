import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import {
  CAPABILITY_ACTIONS_V1,
  capabilityActPhaseGatesV1,
  capabilityActsWithNoPhaseGateV1,
  evaluateCapabilityV1,
  type CapabilityMarketSnapshotV1,
} from './capabilityModel';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from './capabilitySurface';
import { ROUTE_PHASE_GATES_V1, routePhaseGateV1 } from '@dclutch/sdk/generated/marketPhaseAdmissionV1';

/**
 * The cohort-12 SOL/USD market, and the phase the chain reports for it.
 *
 * `apps/dclutch-web/fixtures/market-registry.devnet.json` names this address;
 * the phase is what a finalized read of its Core state decodes to, and it is
 * the observation the UX walk was looking at when `/workbench` reported READY
 * TO PREFLIGHT for acts about it. `Open` implies `Consumed`: `open_market`
 * sets both in one transition (`crates/dclutch-market-core-codec`), so no
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
    expect(evaluateCapabilityV1(provider, observed('Open', 'Consumed'))).toMatchObject({
      status: 'ready-to-preflight',
      phaseGate: { verdict: 'admitted' },
    });
    // The same act, the same market, one phase earlier in its life: refused,
    // by name, before any account is read.
    const early = evaluateCapabilityV1(provider, observed('Founding', 'Prepaid'));
    expect(early.status).toBe('wrong-phase');
    expect(early.phaseGate.verdict).toBe('excluded');
    expect(early.phaseGate.excludedBy?.route).toBe('core/execute_provider_v3::process#ExecuteProvider');
    expect(early.reason).toContain('admits only Open+Consumed');
    expect(early.reason).toContain('this Market is Founding+Prepaid');
  });

  it('refuses a resolution act on a retired market', () => {
    const ready = standing('source.ready');
    const retired = evaluateCapabilityV1(ready, observed('Retired', 'Consumed'));
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
    const unread = evaluateCapabilityV1(provider, observed(null, null));
    expect(unread.status).toBe('needs-chain');
    expect(unread.phaseGate.verdict).toBe('unread');
  });

  it('an act with no published gate says so by name instead of implying admission', () => {
    // `market.found` declares a real route, and that route has no prestate:
    // the Market it founds does not exist yet. The verdict must not present
    // that as a checked admission.
    const found = standing('market.found');
    expect(found.action.routes).toEqual(['core/found::process#Found']);
    expect(capabilityActPhaseGatesV1(found.action)).toHaveLength(0);
    const verdict = evaluateCapabilityV1(found, observed('Open', 'Consumed'));
    expect(verdict.status).toBe('ready-to-preflight');
    expect(verdict.phaseGate.verdict).toBe('no-phase-gate');
  });

  it('names exactly which acts carry no gate, so the coverage cannot be mistaken for total', () => {
    const ungated = capabilityActsWithNoPhaseGateV1();
    const gated = CAPABILITY_ACTIONS_V1.map((act) => act.id).filter((id) => !ungated.includes(id));
    expect(gated).toEqual([
      'source.create-fund',
      'source.ready',
      'source.provider',
      'source.admit-terminal',
    ]);
    expect(ungated).toHaveLength(CAPABILITY_ACTIONS_V1.length - gated.length);
  });
});
