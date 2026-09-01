import {
  CAPABILITY_ACTIONS_V1,
  capabilityStandingV1,
  type CapabilityActionV1,
  type CapabilityClientSurfaceV1,
  type CapabilityStage,
  type CapabilityStandingV1,
} from '@dclutch/sdk/capabilityModel';
import {
  CLIENT_MODULE_SURFACES_V1,
  CLIENT_ROUTES_V1,
  GENERATED_ABI_AUTHORITIES_V1,
  OPERATOR_RUNBOOKS_V1,
} from './generated/capabilitySurfaceV1';
import { marketDetailHrefV1 } from './marketHref';

/**
 * This browser's own evidence, handed to the SDK's capability rules.
 *
 * `lib/generated/capabilitySurfaceV1.ts` is emitted from the application's
 * import graph and byte-gated by `abi:capability-surface:verify`, so what a
 * card says about an act moves only when the act's code moves. This module is
 * the one place the two halves meet: the SDK cannot know what this application
 * routes, and this application does not get to decide what a route means.
 */
export const BROWSER_CAPABILITY_SURFACE_V1: CapabilityClientSurfaceV1 = Object.freeze({
  routes: CLIENT_ROUTES_V1,
  // Modules name their generated authorities by index; the pairing is emitted
  // once rather than repeated under every module that reaches it.
  modules: Object.freeze(CLIENT_MODULE_SURFACES_V1.map((entry) => Object.freeze({
    module: entry.module,
    routes: entry.routes,
    authority: entry.authority,
    submits: entry.submits,
    generatedAbis: Object.freeze(entry.generatedAbis.map((index) => GENERATED_ABI_AUTHORITIES_V1[index])),
  }))),
  runbooks: OPERATOR_RUNBOOKS_V1,
});

/** What one act can actually do in this browser. */
export function browserCapabilityStandingV1(actionDefinition: CapabilityActionV1): CapabilityStandingV1 {
  return capabilityStandingV1(actionDefinition, BROWSER_CAPABILITY_SURFACE_V1);
}

/** Every act's standing, in catalogue order. */
export const BROWSER_CAPABILITY_STANDINGS_V1: ReadonlyArray<CapabilityStandingV1> = Object.freeze(
  CAPABILITY_ACTIONS_V1.map(browserCapabilityStandingV1),
);

/** The acts of one lifecycle stage, with their standing. */
export function browserCapabilityStandingsForStageV1(stage: CapabilityStage): ReadonlyArray<CapabilityStandingV1> {
  return BROWSER_CAPABILITY_STANDINGS_V1.filter((candidate) => candidate.action.stage === stage);
}

/**
 * The link that opens one act, resolved against the Market actually read.
 *
 * A market-bound act has no address of its own: until one Market has been
 * reacquired there is no page to send a reader to, and inventing one would be
 * the same class of claim as inventing a status. This lives in the browser
 * rather than the SDK because the permalink depends on this site's published
 * market registry.
 */
export function capabilityWorkspaceV1(
  actionDefinition: CapabilityActionV1,
  snapshot: Readonly<{ market: Readonly<{ address: string }> | null }> | null,
): string | null {
  if (actionDefinition.workspace === null) return null;
  if (actionDefinition.workspace !== 'market-detail') return actionDefinition.workspace;
  const market = snapshot?.market ?? null;
  return market === null ? null : marketDetailHrefV1(market.address);
}
