/**
 * The executable capability model. One catalogue; no status anyone can type.
 *
 * WHAT WAS WRONG. Every row of this file used to carry an `implementation:`
 * string -- `'browser-wallet'`, `'rust-unsigned'`, `'awaiting-production'` --
 * and `/console`, `/operate` and `/workbench` rendered their whole story from
 * it. Nothing connected that string to code. The test that guarded it asserted
 * the string back (`expect(...).toMatchObject({ implementation: 'browser-wallet' })`),
 * so a lane could change what the browser claimed by editing two lines, and a
 * lane that changed what the browser DID changed nothing at all. The board was
 * wrong in both directions at once: it said the browser could not construct a
 * Dealer liquidity transaction while `/liquidity` was building and signing one,
 * and it said `/release` produced unsigned bytes while that page had been
 * asking for a wallet transaction signature.
 *
 * WHAT IS HERE INSTEAD. An act names ANCHORS -- the module that owns it, the
 * module that builds its bytes, the page that publishes its exact command --
 * and `capabilityStandingV1` derives venue, authority and status from a
 * `CapabilityClientSurfaceV1` read off the application's own import graph
 * (`apps/dclutch-web/lib/generated/capabilitySurfaceV1.ts`, emitted and
 * byte-gated like every other generated module). There is no field to set. To
 * make an act say it reaches a wallet you must give it an owner that reaches
 * the wallet, from a route a reader can open.
 *
 * WHERE THE LINE IS. This module owns the SEMANTICS of an act: what it is
 * called, which family and stage it belongs to, what safety it guarantees, and
 * the rules that turn evidence into a status. It cannot own the evidence, which
 * is a fact about one application's routes -- so the surface is injected, and
 * an SDK consumer that is not this browser supplies its own.
 *
 * WHAT THIS MODEL DOES NOT COVER, and no status model can. It decides whether
 * an act can be performed and what performing it asks of you. It says nothing
 * about whether the NUMBERS an act sits above mean what they look like. Those
 * are a second axis — conserved versus merely declared — and the protocol has
 * both: a registered sell's `reserved_claims` is a ceiling on what that record
 * may ever fill and moves nothing, while the claims themselves are conserved at
 * the fill, where `claim_custody_debit` either moves them or rolls the whole
 * transaction back. A perfectly honest venue line above a total of declared
 * reservations is still a lie. `apps/dclutch-web/lib/reservationVocabulary.test.ts`
 * is that axis's guard; this file is not.
 *
 * WALLS. An act with no venue does not get a cheerful label. It carries a WALL:
 * what stops it, in the words of the thing that stops it, and a citation --
 * a commit or a path in this repository -- that a reader and a test can both
 * resolve. `lib/capabilityEvidence.test.ts` refuses a wall whose citation does
 * not exist and refuses the vocabulary of a roadmap.
 */

import {
  type MarketPhaseV1,
  type MarketReadinessV1,
  type RoutePhaseGateV1,
  routeHasNoStateMachineV1,
  routeOtherMachineGateV1,
  routePhaseGateV1,
} from './generated/marketPhaseAdmissionV1';

export const CAPABILITY_STAGES = ['author', 'trade', 'resolve', 'claim'] as const;
export type CapabilityStage = (typeof CAPABILITY_STAGES)[number];
export type CapabilityFamily = 'Release' | 'Creation' | 'Direct' | 'Source' | 'Series' | 'General' | 'Dealer' | 'Claims';

/** The strongest wallet request one module's transitive closure can make. */
export type CapabilityClientAuthorityV1 = 'none' | 'wallet-message' | 'wallet-transaction';

/** One generated decode authority, and the script that byte-checks it. */
export type CapabilityGeneratedAbiV1 = Readonly<{ module: string; verify: string | null }>;

/** What one client module can do, read off an import graph rather than stated. */
export type CapabilityClientModuleV1 = Readonly<{
  module: string;
  routes: ReadonlyArray<string>;
  authority: CapabilityClientAuthorityV1;
  submits: boolean;
  generatedAbis: ReadonlyArray<CapabilityGeneratedAbiV1>;
}>;

/** One page that publishes an exact CLI command a reader is meant to run. */
export type CapabilityRunbookV1 = Readonly<{
  module: string;
  commands: number;
  routes: ReadonlyArray<string>;
  namesExecutionAuthority: boolean;
}>;

/**
 * The evidence a client application supplies about itself.
 *
 * Structural on purpose: the browser hands over its generated surface, and any
 * other consumer of this SDK describes its own. Nothing here is authored by
 * hand in the same file as the act it decides.
 */
export type CapabilityClientSurfaceV1 = Readonly<{
  routes: ReadonlyArray<string>;
  modules: ReadonlyArray<CapabilityClientModuleV1>;
  runbooks: ReadonlyArray<CapabilityRunbookV1>;
}>;

/**
 * What stops an act, and where that is written down.
 *
 * `citation` is a commit hash or a repository-relative path. A wall is the
 * honest rendering of absent evidence; it is never a promise.
 */
export type CapabilityWallV1 = Readonly<{ statement: string; citation: string }>;

/**
 * The three places a status can come from, and nothing else.
 *
 * `owner` is the module whose closure carries the act's authority -- the
 * component for a workspace, the flow machine for a trade. `builder` is the
 * module that produces this act's exact bytes and must lie inside the owner's
 * reach. `runbook` is the page that publishes the exact command, when the act
 * runs outside the browser.
 */
export type CapabilityAnchorsV1 = Readonly<{
  owner: string | null;
  builder: string | null;
  runbook: string | null;
}>;

export type CapabilityWorkspaceV1 = string | 'market-detail';

/**
 * WHICH Market an act is about. Three answers, and the third is the one whose
 * absence was a defect.
 *
 * This replaced a `requiresMarket: boolean`, which could only say whether an
 * act needed the observed Market and so collapsed "this act has nothing to do
 * with any Market" together with "this act CREATES a Market". Both read as
 * `false`, so `/workbench` observing the open cohort-12 market reported READY
 * TO PREFLIGHT for "Found a Market and admit its first participant" -- true of
 * founding a market, and false of every market on that screen. Measured
 * 2026-09-02 in the UX walk (row O1) and misdiagnosed at the time as a missing
 * phase gate; the phase gates landed and this card did not move, because the
 * defect was never the phase. It was that the act's SUBJECT was unwritten.
 *
 * `requiresMarket` is now derived, by [`capabilityRequiresMarketV1`]. There is
 * no boolean to set inconsistently with the subject.
 */
export type CapabilityMarketSubjectV1 =
  /** Acts on the Market the reader has selected and read. */
  | 'observed-market'
  /** Creates a Market, so no Market on screen is its subject. */
  | 'new-market'
  /** Has no Market subject at all: a release, a Product record, a route file. */
  | 'no-market';

export type CapabilityActionV1 = Readonly<{
  id: string;
  stage: CapabilityStage;
  family: CapabilityFamily;
  /** The outcome, in the reader's terms. Cards lead with this. */
  action: string;
  workspace: CapabilityWorkspaceV1 | null;
  /** Which Market this act is about. See [`CapabilityMarketSubjectV1`]. */
  subject: CapabilityMarketSubjectV1;
  anchors: CapabilityAnchorsV1;
  /**
   * Census route ids this act's submit path reaches.
   *
   * REQUIRED, and empty is a real answer: an act cannot be added to this
   * catalogue without someone deciding what it drives, which is the only
   * structure that keeps a per-act phase declaration from being partial by
   * accident. Every id is checked against the census's published route list by
   * `apps/dclutch-web/lib/capabilityPhaseGate.test.ts`, so a name that no
   * route carries is red rather than silently ungated.
   *
   * THAT CHECK IS NOT ENOUGH ON ITS OWN, and for a while it was the only one.
   * A route id that EXISTS passes it whatever the act's builder emits — so an
   * act declaring a route its transaction never reaches, and an act reaching a
   * route it declares nothing about, both read as correct. The second is the
   * dangerous direction: an undeclared route is an unread phase gate, and an
   * unread gate renders as READY TO PREFLIGHT. Two acts were in exactly that
   * state (`dealer.liquidity` had been building a `DCLTHOT3` Trading
   * instruction since `/liquidity` shipped, and `release.activate` a
   * `DCLTRIX1` Registry one) and a third, `source.close-fund`, declared
   * nothing while its planner emits `DCLRFCQ1`, whose guard admits
   * `Retiring+Consumed` alone.
   *
   * So the second check is `apps/dclutch-web/lib/capabilityRouteDerivation.test.ts`:
   * for every act whose builder AUTHORS its instruction bytes, it runs the
   * builder against a fixture and puts the compiled instruction through
   * `routeSelector.ts`, which reads the census's own selector tables — the
   * route id comes out of (program, leading eight bytes), and the declaration
   * must contain every route those bytes select. What that check cannot cover
   * is stated there rather than assumed: Core dispatches on a decoded `Action`
   * variant that no leading-byte view can name, and nine acts do not author
   * their bytes at all — a Rust planner does, and their declarations are cited
   * to it.
   *
   * An empty list means NO ROUTE IS ESTABLISHED, never "no phase constrains
   * it". Most acts here are authoring acts with no Market to consult; some
   * reach routes in the programs whose guards are still written inline and
   * which the census therefore reads no constant for. Both print as
   * `no phase gate`, and a consumer must not read either as admission.
   */
  routes: ReadonlyArray<string>;
  /** One compact safety/recovery sentence: signing, finality, recovery, submission. */
  guarantee: string;
  walls: ReadonlyArray<CapabilityWallV1>;
}>;

const NO_ANCHORS: CapabilityAnchorsV1 = Object.freeze({ owner: null, builder: null, runbook: null });

const anchors = (owner: string | null, builder: string | null, runbook: string | null = null): CapabilityAnchorsV1 =>
  Object.freeze({ owner, builder, runbook });

const wall = (statement: string, citation: string): CapabilityWallV1 => Object.freeze({ statement, citation });

const action = (
  id: string,
  stage: CapabilityStage,
  family: CapabilityFamily,
  label: string,
  workspace: CapabilityWorkspaceV1 | null,
  subject: CapabilityMarketSubjectV1,
  actionAnchors: CapabilityAnchorsV1,
  routes: ReadonlyArray<string>,
  guarantee: string,
  walls: ReadonlyArray<CapabilityWallV1> = [],
): CapabilityActionV1 => Object.freeze({
  id, stage, family, action: label, workspace, subject, anchors: actionAnchors,
  routes: Object.freeze([...routes]), guarantee, walls: Object.freeze([...walls]),
});

/**
 * Whether this act needs the reader to have named and read a Market.
 *
 * Derived from the subject, never stored: only an act ON the observed Market
 * needs one. An act that founds a Market needs the coordinate EMPTY, which is
 * a different requirement and is stated where it is enforced, in
 * [`evaluateCapabilityV1`].
 */
export function capabilityRequiresMarketV1(actionDefinition: CapabilityActionV1): boolean {
  return actionDefinition.subject === 'observed-market';
}

/** No census route is established for this act. Written out, never defaulted. */
const NO_ROUTE: ReadonlyArray<string> = Object.freeze([]);

export const CAPABILITY_ACTIONS_V1: ReadonlyArray<CapabilityActionV1> = Object.freeze([
  action('release.activate', 'author', 'Release', 'Activate a checked multiprogram release', '/release', 'no-market',
    anchors('components/ReleaseWorkspace.tsx', 'lib/releaseRegistry.ts'),
    ['registry/record_v1::dispatch'],
    'Each role packet is signed on its own and leaves as a file; this page never sends one.'),
  action('product.compile', 'author', 'Creation', 'Compile a Product record and its admission request', '/product-v2', 'no-market',
    anchors('components/ProductV2Studio.tsx', 'lib/productV2.ts', 'components/ProductV2Studio.tsx'),
    NO_ROUTE,
    'Nothing is read from a chain and nothing is signed: the output is a record and an instruction, not a transaction.'),
  action('market.inspect', 'author', 'Creation', 'Check a founding against the chain before you commit to it', '/create', 'new-market',
    anchors('components/CreateMarketWizard.tsx', 'lib/coreFound.ts'),
    NO_ROUTE,
    'Finalized reads only. The wizard reports what the founding would cost and refuse; it exports no packet.'),
  action('market.found', 'author', 'Creation', 'Found a Market and admit its first participant', '/found', 'new-market',
    anchors('components/CoreFoundWorkspace.tsx', 'lib/coreFound.ts', 'components/CoreFoundWorkspace.tsx'),
    ['core/found::process#Found'],
    'The browser exports unsigned bytes and asks for no key; the published campaign records devnet authorization before any child may sign.'),
  action('market.join', 'author', 'Creation', 'Admit another participant', 'market-detail', 'observed-market',
    anchors('components/JoinPanel.tsx', 'lib/userPositionAdmissionOperation.ts'),
    ['trading/user_position_admission_v1::process_user_position_admission_v1',
     'trading/user_position_admission_v1::process_user_position_admission_v1#Admit'],
    'The compiled Rust planner derives all 27 accounts from one finalized observation; the exact packet is saved before your wallet sees it, sent once, and cleared only after the chain confirms it, so reloading resumes and never resubmits.'),
  action('source.create-fund', 'author', 'Source', 'Create the resolution fund', '/resolution', 'observed-market',
    anchors('components/ResolutionWorkspace.tsx', 'lib/sourceReadinessV1.ts'),
    ['core/resolution::process#CreateFund'],
    'The exact packet is saved before your wallet sees it, sent once, and cleared only after the finalized poststate is read back; reloading resumes and never resubmits.'),

  action('direct.route', 'trade', 'Direct', 'Export a portable Direct route', '/operate', 'no-market',
    anchors('components/OperatorSurface.tsx', null, 'components/OperatorSurface.tsx'),
    NO_ROUTE,
    'The published command holds no key and can neither sign nor send: it reads finalized state and writes two files.'),
  action('direct.author', 'trade', 'Direct', 'Author a portable sell offer', 'market-detail', 'observed-market',
    anchors('components/trade/MakerOfferComposer.tsx', 'lib/directOfferAuthoring.ts'),
    NO_ROUTE,
    'One detached message signature. No transaction is built and no claims move; the ticket is yours to keep or hand on.'),
  action('direct.inline', 'trade', 'Direct', 'Take and execute a signed offer', 'market-detail', 'observed-market',
    anchors('lib/tradeFlowMachine.ts', 'lib/directInlineV3.ts'),
    ['trading/hot_v3::process_hot_execution_v3'],
    'Two separate signatures, neither of which sends. The signed packet is saved before submission, sent once, and reconciled against finalized balances.',
    [wall('Trading runs to 1,330,239 of 1,399,700 CU and dies ProgramFailedToComplete; the fill can exhaust its budget on chain.', 'GOAL.md')]),
  action('direct.register', 'trade', 'Direct', 'Create a registered resting order', null, 'observed-market',
    NO_ANCHORS,
    NO_ROUTE,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('No route renders a control for it and no browser module builds its transaction; the registered-order wire lives in the Rust codec.', 'crates/dclutch-direct-codec')]),
  action('direct.cancel', 'trade', 'Direct', 'Cancel, expire, or cancel through a resting order', null, 'observed-market',
    NO_ANCHORS,
    NO_ROUTE,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('No route renders a control for it and no browser module builds its transaction; successor replay roots and terminal account profiles are not one accepted route.', 'crates/dclutch-direct-codec')]),
  action('series.prepare', 'trade', 'Series', 'Prepare an occurrence and its ticket', null, 'observed-market',
    NO_ANCHORS,
    NO_ROUTE,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('A Template with nonzero close rent describes a root activation may not fund and Close can never open; the activation funding seam is jointly unsatisfiable.', 'WAVE.md')]),
  action('general.consider', 'trade', 'General', 'Check a candidate plan and export its exact packet', '/general', 'observed-market',
    anchors('components/GeneralWorkspace.tsx', 'lib/generalPlanV5.ts'),
    ['trading/hot_v3::process_hot_execution_v3'],
    'The plan is authored elsewhere; this page authenticates it against finalized state and hands back the same bytes. No key is asked for.'),
  action('dealer.liquidity', 'trade', 'Dealer', 'Contribute or redeem dealer equity', '/liquidity', 'observed-market',
    anchors('components/DealerLiquidityWorkspace.tsx', 'lib/dealerEquityV3.ts'),
    ['trading/hot_v3::process_hot_execution_v3'],
    'The packet is signed here and downloaded; this page submits nothing, so an external submitter is the only thing that can send it.',
    [wall('Exactly one selector can satisfy validate_selection: derivation_policy is pinned per descriptor to its own lifecycle digest and per root to a single manifest entry.', 'WAVE.md')]),
  action('dealer.trade', 'trade', 'Dealer', 'Take an inventory-bounded immediate trade', null, 'observed-market',
    NO_ANCHORS,
    NO_ROUTE,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('No route renders a control for it and no browser module builds its transaction; the Dealer scenario kernel owns it.', 'crates/dclutch-dealer-scenario-kernel')]),

  action('source.ready', 'resolve', 'Source', 'Have Core accept the fund as ready', '/resolution', 'observed-market',
    anchors('components/ResolutionWorkspace.tsx', 'lib/sourceReadinessV1.ts'),
    ['core/resolution::process#VerifyFundReady'],
    'The exact packet is saved before your wallet sees it, sent once, and cleared only after the Ready poststate selects the terminal route.'),
  action('source.provider', 'resolve', 'Source', 'Submit provider evidence, or reclaim it', '/resolution', 'observed-market',
    anchors('components/ResolutionWorkspace.tsx', 'lib/sourceProviderV1.ts'),
    ['core/execute_provider_v3::process#ExecuteProvider'],
    'Two signatures on one immutable message — a fresh operation signer and your wallet — saved before submission and verified against the terminal accounts.'),
  action('source.admit-terminal', 'resolve', 'Source', 'Admit the terminal resolution', '/resolution', 'observed-market',
    anchors('components/ResolutionWorkspace.tsx', 'lib/sourceTerminalV1.ts'),
    ['core/resolution::process#AdmitTerminal'],
    'The signed record is saved before one submission and kept until the finalized Terminal receipt is read back; a reload resumes the same signature.'),
  action('source.close-fund', 'resolve', 'Source', 'Close the resolution fund', '/resolution', 'observed-market',
    anchors('components/ResolutionWorkspace.tsx', 'lib/sourceCloseFundV1.ts'),
    ['resolution/core_effect::process_direct_funding_close_v1'],
    'Prepay and close are separate signed acts; each is saved before submission and confirmed against the finalized typed receipt.'),
  action('general.settle', 'resolve', 'General', 'Check a settlement plan and export its exact packet', '/general', 'observed-market',
    anchors('components/GeneralWorkspace.tsx', 'lib/generalPlanV5.ts'),
    ['trading/hot_v3::process_hot_execution_v3'],
    'The plan is authored elsewhere; this page authenticates it against finalized state and hands back the same bytes. No key is asked for.'),

  action('claims.conserve', 'claim', 'Claims', 'Split or merge conservative claims', null, 'observed-market',
    NO_ANCHORS,
    NO_ROUTE,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('No route renders a control for it and no browser module builds its transaction; the conservation contract owns the wire.', 'crates/dclutch-claims-conservation-contract'),
     wall('The wire is absent on chain, not merely unbuilt here: the contract declares magic DCLCNS01 and the Claims program has no handler that dispatches it, which the operator planner records in its own words.', 'crates/dclutch-operator/src/claims_conservation_v1.rs')]),
  action('claims.represent', 'claim', 'Claims', 'Denominate or reconstitute a rational representation', null, 'observed-market',
    NO_ANCHORS,
    NO_ROUTE,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('No route renders a control for it and no browser module builds its transaction; route DCRRPRQ2 and its Denominate/Reconstitute actions own the wire.', 'crates/dclutch-rational-representation-v2-contract'),
     wall('The structured representation campaign stands at a named ATA-derivation wall.', 'GOAL.md')]),
  action('claims.replay', 'claim', 'Claims', 'Create the replay account redemption requires', '/redeem', 'observed-market',
    anchors('components/RedeemFlow.tsx', 'lib/claimsCustodyReplay.ts'),
    ['claims/custody_replay_v1::process'],
    'One signature for the account the chain demands before payout; saved before submission, sent once, and confirmed finalized before the payout step opens.'),
  action('claims.redeem', 'claim', 'Claims', 'Redeem a terminal Claims Position', '/redeem', 'observed-market',
    anchors('components/RedeemFlow.tsx', 'lib/walletTerminalPayoutV3.ts'),
    ['claims/terminal_settlement_v3::process'],
    'The payout plan and the signed packet are both saved before one submission; recovery resumes the saved signature and never sends a second.'),
  action('series.close', 'claim', 'Series', 'Consume or expire a ticket and close the occurrence', null, 'observed-market',
    NO_ANCHORS,
    NO_ROUTE,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('No route renders a control for it and no browser module builds its transaction; the Series V3 kernel owns the exact planning.', 'crates/dclutch-series-v3-kernel'),
     wall('Expire refused on a real ELF at custom code 16387 with the permit account absent.', 'docs/evidence/SERIES_PERMIT_EXPIRY_HOT_WALL_2026_08_31.json')]),
  action('general.close', 'claim', 'General', 'Check a close plan and export its exact packet', '/general', 'observed-market',
    anchors('components/GeneralWorkspace.tsx', 'lib/generalPlanV5.ts'),
    ['trading/hot_v3::process_hot_execution_v3'],
    'The plan is authored elsewhere; this page authenticates it against finalized state and hands back the same bytes. No key is asked for.'),
  action('dealer.close', 'claim', 'Dealer', 'Reset the ladder, close an LP, or retire the pool', null, 'observed-market',
    NO_ANCHORS,
    NO_ROUTE,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('No route renders a control for it and no browser module builds its transaction; the Dealer codec owns the wire.', 'crates/dclutch-dealer-codec')]),
]);

/** Where an act runs. Derived; there is no field that sets it. */
export type CapabilityVenueV1 = 'browser' | 'operator-cli' | 'no-venue';

/** What an act asks a person for. Derived from the wallet request it reaches. */
export type CapabilityAuthorityV1 = 'none' | 'wallet-message' | 'wallet-transaction' | 'operator-key';

/**
 * What one act can actually do, and the evidence it stands on.
 *
 * Everything below `action` is computed. `routes` empty means no reader can
 * reach it, whatever else is true of the code.
 */
export type CapabilityStandingV1 = Readonly<{
  action: CapabilityActionV1;
  venue: CapabilityVenueV1;
  authority: CapabilityAuthorityV1;
  /** Whether the browser sends the packet itself. */
  submits: boolean;
  /** Routes a reader can open to perform it. */
  routes: ReadonlyArray<string>;
  /** Generated decode authorities behind it. */
  generatedAbis: ReadonlyArray<CapabilityGeneratedAbiV1>;
  /** Generated modules it depends on that no `abi:*:verify` script checks. */
  unverifiedAbis: ReadonlyArray<string>;
  /** Whether the browser publishes an exact command for it. */
  publishesCommand: boolean;
  walls: ReadonlyArray<CapabilityWallV1>;
}>;

function moduleOf(surface: CapabilityClientSurfaceV1, path: string | null): CapabilityClientModuleV1 | null {
  if (path === null) return null;
  return surface.modules.find((candidate) => candidate.module === path) ?? null;
}

/**
 * Derive one act's standing from the client's own evidence.
 *
 * The rules, in the order they decide:
 *
 *   1. An owner nothing routes is not a capability. `RationalRepresentation`
 *      is a complete signing workspace with no page rendering it, and calling
 *      that available would be exactly the claim this model exists to refuse.
 *   2. An act with no builder is not a browser act, however reachable its page
 *      is; and a builder the surface knows must lie inside the owner's reach,
 *      so an act may not borrow the authority of a workspace that never calls
 *      its constructor.
 *   3. Authority is whatever wallet request the owner's closure actually
 *      reaches, and submission is whether the one submission primitive is
 *      reachable. A browser that signs and hands you a file is not a browser
 *      that sends.
 *   4. A published command with an execution flag is operator-key authority.
 *   5. Anything left has no venue, and says why in its walls.
 */
export function capabilityStandingV1(
  actionDefinition: CapabilityActionV1,
  surface: CapabilityClientSurfaceV1,
): CapabilityStandingV1 {
  const owner = moduleOf(surface, actionDefinition.anchors.owner);
  const builder = moduleOf(surface, actionDefinition.anchors.builder);
  const runbook = actionDefinition.anchors.runbook === null
    ? null
    : surface.runbooks.find((candidate) => candidate.module === actionDefinition.anchors.runbook) ?? null;

  const routed = owner !== null && owner.routes.length > 0;
  // The builder is evidence only when the surface knows it; a constructor with
  // no wallet reach and no generated dependency is not surveyed, and its
  // absence from the survey is not a reason to refuse the owner's own facts.
  const builderReaches = builder === null || builder.routes.some((route) => owner?.routes.includes(route) === true);

  // No builder, no browser act. `/market` renders a Join panel and `/operate`
  // renders a route export, and both pages are reachable, decode generated
  // records, and look for all the world like workspaces -- but neither has a
  // module that builds its transaction, so neither is something the browser
  // can do. Route reachability alone is the route-magic mistake in another
  // costume.
  const browser = routed && actionDefinition.anchors.builder !== null && builderReaches;
  const authority: CapabilityAuthorityV1 = browser && owner !== null
    ? owner.authority
    : runbook !== null && runbook.namesExecutionAuthority ? 'operator-key' : 'none';
  const venue: CapabilityVenueV1 = browser ? 'browser' : runbook !== null ? 'operator-cli' : 'no-venue';

  const generatedAbis = browser && owner !== null ? owner.generatedAbis : [];
  return Object.freeze({
    action: actionDefinition,
    venue,
    authority,
    submits: browser && owner !== null ? owner.submits : false,
    routes: Object.freeze(browser && owner !== null ? [...owner.routes] : runbook !== null ? [...runbook.routes] : []),
    generatedAbis: Object.freeze([...generatedAbis]),
    unverifiedAbis: Object.freeze(generatedAbis.filter((entry) => entry.verify === null).map((entry) => entry.module)),
    publishesCommand: runbook !== null,
    walls: actionDefinition.walls,
  });
}

/** Every act's standing, in catalogue order. */
export function capabilityStandingsV1(surface: CapabilityClientSurfaceV1): ReadonlyArray<CapabilityStandingV1> {
  return Object.freeze(CAPABILITY_ACTIONS_V1.map((candidate) => capabilityStandingV1(candidate, surface)));
}

/**
 * The second line of a card: venue and authority, in one phrase.
 *
 * Outcome first (`action.action`), venue and authority second (here), one
 * safety guarantee third (`action.guarantee`). Nothing in this function is a
 * protocol lecture and nothing apologises for the architecture.
 */
export function capabilityVenueTextV1(standing: CapabilityStandingV1): string {
  if (standing.venue === 'no-venue') return 'Nothing here can build it yet';
  if (standing.venue === 'operator-cli') {
    return standing.authority === 'operator-key'
      ? 'Published command · your own key, after an explicit authorization'
      : 'Published command · reads only, no key';
  }
  switch (standing.authority) {
    case 'wallet-transaction':
      return standing.submits
        ? 'This browser · one wallet signature, sent from here'
        : 'This browser · one wallet signature, exported as a file';
    case 'wallet-message':
      return 'This browser · one detached message signature';
    default:
      return 'This browser · no key, no signature';
  }
}

export type CapabilityActContractV1 = Readonly<{ venue: string; guarantee: string }>;

/** The two lines under one outcome. */
export function capabilityActContractV1(standing: CapabilityStandingV1): CapabilityActContractV1 {
  return Object.freeze({ venue: capabilityVenueTextV1(standing), guarantee: standing.action.guarantee });
}

/**
 * What the published phase gates say about this act, against this observation.
 *
 * `no-phase-gate` is the honest degradation and is reported by name rather
 * than folded into `admitted`: it means no constant was read for any route
 * this act declares, which covers an act with no established route, an
 * authoring act with no Market to consult, a route whose guard is still
 * written inline in one of the seven programs the census reads no constant
 * for, and a route whose admissibility is over a state machine that is not
 * the Market's -- an activation, a Dealer root, a ticket, a funding ledger --
 * none of which the Market phase can answer for. None of those is an
 * admission, and a surface that renders them as one is repeating the defect
 * this field exists to close.
 */
export type CapabilityPhaseGateV1 = Readonly<{
  /** Census route ids the act declares, whether gated or not. */
  routes: ReadonlyArray<string>;
  /** Those with a published gate. */
  gates: ReadonlyArray<RoutePhaseGateV1>;
  verdict: 'admitted' | 'excluded' | 'unread' | 'no-phase-gate' | 'other-machine';
  /** The gate that excluded the observation, when one did. */
  excludedBy: RoutePhaseGateV1 | null;
  /**
   * State machines gating this act that this snapshot cannot observe.
   *
   * A Source resolution state, a Dealer root's lifecycle, a Series ticket:
   * none of them is the Market's phase, and none of them can be answered by
   * one. A route gated only on such a machine is NOT ungated, so an act
   * driving it reports `other-machine` and the verdict says `needs-chain` --
   * never `ready-to-preflight`, and never `no-phase-gate`, which would claim
   * the census read nothing when it read something this reader cannot use.
   */
  unobservableMachines: ReadonlyArray<string>;
}>;

export type CapabilityVerdictV1 = Readonly<{
  standing: CapabilityStandingV1;
  status:
    | 'ready-to-preflight'
    | 'wrong-phase'
    | 'not-this-market'
    | 'needs-chain'
    | 'needs-market'
    | 'operator-only'
    | 'no-venue';
  reason: string;
  phaseGate: CapabilityPhaseGateV1;
}>;

/**
 * One chain-observed snapshot, described only by the field this decision uses.
 *
 * Structural rather than imported so the two trees' snapshot types can differ
 * without either one silently changing a status.
 */
export type CapabilityMarketSnapshotV1 = Readonly<{
  market: Readonly<{
    address: string;
    /**
     * The Market's Core phase, decoded from the same finalized bytes the
     * address was read at, or `null` when the account did not decode.
     *
     * Not optional. An optional field is one a caller forgets, and a verdict
     * that silently falls back to "ready" when it is missing is exactly the
     * verdict this whole chain replaced.
     */
    phase: MarketPhaseV1 | null;
    /** Its Resolution Fund readiness, which several gates constrain jointly. */
    readiness: MarketReadinessV1 | null;
  }> | null;
}>;

/**
 * THE PHASE THIS VERDICT CONSULTS, and exactly how far it reaches.
 *
 * `ready-to-preflight` used to be asserted from a market's EXISTENCE alone, so
 * `/workbench` observing the open cohort-12 market reported READY TO PREFLIGHT
 * for acts that market refuses on sight (measured 2026-09-02, UX walk row O1;
 * routed to its authors by `2b0046fb`).
 *
 * The repair ran in the only order that could not produce a hand mapping. The
 * guards were named where they are enforced -- ten inline
 * `state.phase != Phase::Open` conditions in Core became ten
 * `MarketAdmissionV1` constants beside the code that checks them (`315f1931`).
 * The route census learned to read those constants structurally out of the
 * Rust AST and to carry them per route (`7d24a851`). `routes.md` renders them,
 * and `lib/generated/marketPhaseAdmissionV1.ts` mirrors that page. Nothing in
 * this file states a phase; it looks one up, and a name it looks up that no
 * route carries is a red test rather than a silent miss.
 *
 * WHAT IT REACHES TODAY. Seventy-two of 162 routes carry a named gate, and
 * fifteen of the twenty-seven acts below declare a route at all -- up from
 * nine, because the declarations stopped being typed and started being read
 * off what each builder emits (see `routes` below). SIX acts therefore have a
 * phase gate: `source.create-fund`, `source.ready`, `source.provider`,
 * `source.admit-terminal`, `claims.redeem`, whose
 * `claims/terminal_settlement_v3::process` admits `Terminal` and `Retiring`
 * and refuses every other phase -- so a redemption card beside a Founding or
 * an Open Market says WRONG PHASE instead of READY TO PREFLIGHT -- and now
 * `source.close-fund`, whose `DCLRFCQ1` route admits `Retiring+Consumed`
 * alone and which had been reporting READY TO PREFLIGHT on every Market in
 * every other phase. The rest report `no-phase-gate` BY NAME in the verdict,
 * which is the whole difference from the state this replaced: a reader can
 * tell a checked admission from an unchecked one, which is the thing a partial
 * mapping alone cannot give them.
 *
 * THE CARD THE UX WALK COMPLAINED ABOUT IS A DIFFERENT DEFECT, and it is
 * closed here rather than by a phase. `market.found` declares
 * `core/found::process#Found`, which HAS no prestate: the Market does not
 * exist yet, so no phase gate could ever have moved that card. What was wrong
 * was that its SUBJECT was unwritten -- a `requiresMarket: boolean` cannot say
 * "creates one" -- so a card about a Market that does not exist rendered
 * READY TO PREFLIGHT beside an open Market it can never touch. The subject is
 * now declared (`CapabilityMarketSubjectV1`), and an act whose subject is a
 * Market it creates refuses `ready` by name while an observation holds one.
 *
 * WHAT IT STILL DOES NOT REACH, and why, because the two reasons are
 * different and only one of them is unfinished work.
 *
 * Some acts declare a route the census reads no gate for because the guard is
 * still inline -- `claims.replay` on `claims/custody_replay_v1::process`, and
 * every act in the seven programs that have named nothing yet.
 *
 * The rest are not waiting on a name at all: their admissibility is over a
 * DIFFERENT STATE MACHINE, and no Market phase can answer for it.
 * `direct.inline` drives `trading/hot_v3::process_hot_execution_v3`, whose own
 * discriminant guard is a Series ticket's `TicketPhaseV3` and whose Market
 * reading comes through the activation cache; `dealer.liquidity` and
 * `dealer.close` are over the Dealer root's own lifecycle phase;
 * `series.prepare` and `series.close` are over a ticket's. Naming the Market's
 * guards could never have reached them, and counting them as "ungated pending
 * Core's vocabulary" hid that.
 *
 * The SECOND MACHINE now exists and is the shape the rest will take. The
 * Source resolution state has its own `SourceAdmissionV1` over its own wire
 * tags, the census reads it with a machine parameter, `routes.md` writes every
 * set as `machine: states`, and this module's generated table carries
 * `ROUTES_GATED_ON_ANOTHER_MACHINE_V1` -- the routes whose gate this snapshot
 * has no field for. An act driving one of those reports `needs-chain` with the
 * machine NAMED, never `ready-to-preflight` and never `no-phase-gate`: the
 * census read a gate, and this reader cannot use it. The check runs BEFORE the
 * Market gates and even when they would admit, because a route gated on
 * `market: Open+Consumed` and `source: Primary` passes both or neither, and a
 * Market is `Open` for the whole span in which its Source moves `Primary` to
 * `Resolved` -- reporting the Market half as an admission is exactly the
 * half-answer this chain replaced.
 *
 * WHAT AN ADMISSION IS NOT. Every gate is a NECESSARY condition. An act whose
 * prestate is excluded cannot succeed, and that refutation is publishable. An
 * act whose prestate is admitted still has every account, release, request and
 * child-acknowledgement check ahead of it, and `ready-to-preflight` has never
 * meant more than "you can try this now".
 */

/**
 * Whether one gate admits the observed prestate.
 *
 * A gate that names pairs is checked against the pair when readiness was read,
 * and against the phase projection when it was not -- the weaker test, which
 * can only fail to refuse and never wrongly refuse.
 */
function gateAdmitsV1(
  gate: RoutePhaseGateV1,
  phase: MarketPhaseV1,
  readiness: MarketReadinessV1 | null,
): boolean {
  if (gate.prestates.length > 0 && readiness !== null) {
    return gate.prestates.some(([admitted, required]) => admitted === phase && required === readiness);
  }
  return gate.phases.includes(phase);
}

/** How one gate reads out loud, for a refusal a person has to act on. */
function gateTextV1(gate: RoutePhaseGateV1): string {
  return gate.prestates.length > 0
    ? gate.prestates.map(([phase, readiness]) => `${phase}+${readiness}`).join(' or ')
    : gate.phases.join(' or ');
}

const NO_GATE_READ: CapabilityPhaseGateV1 = Object.freeze({
  routes: Object.freeze([]), gates: Object.freeze([]), verdict: 'no-phase-gate', excludedBy: null,
  unobservableMachines: Object.freeze([]),
});

/**
 * The machines gating this act that this snapshot has no field for.
 *
 * The snapshot carries the Core Market's phase and readiness and nothing else,
 * so every other machine the census publishes is unobservable from here. That
 * is a fact about this snapshot, not about the act, and it is why the answer
 * is `needs-chain` rather than a refusal: the act may well be admissible, and
 * this reader cannot say.
 */
export function capabilityActUnobservableMachinesV1(act: CapabilityActionV1): ReadonlyArray<string> {
  const machines: string[] = [];
  for (const route of act.routes) {
    const entry = routeOtherMachineGateV1(route);
    if (entry === null) continue;
    for (const machine of entry.machines) if (!machines.includes(machine)) machines.push(machine);
  }
  return Object.freeze(machines);
}

/** The published gates for one act, resolved from the census-derived table. */
export function capabilityActPhaseGatesV1(act: CapabilityActionV1): ReadonlyArray<RoutePhaseGateV1> {
  return Object.freeze(act.routes.map(routePhaseGateV1).filter((gate): gate is RoutePhaseGateV1 => gate !== null));
}

/**
 * The phase gate in one line, for a card that has to say what it checked.
 *
 * `no published gate` is said out loud rather than left blank. A blank cell
 * reads as "checked and fine", which is the reading this whole chain exists to
 * take away from a surface that has not checked anything.
 */
export function capabilityPhaseGateTextV1(gate: CapabilityPhaseGateV1): string {
  switch (gate.verdict) {
    case 'admitted':
      return `admitted at ${gate.gates.map(gateTextV1).join('; ')}`;
    case 'excluded':
      return `admits only ${gate.excludedBy === null ? 'another prestate' : gateTextV1(gate.excludedBy)}`;
    case 'unread':
      return 'the Market did not decode at this observation';
    case 'other-machine':
      return `gated on the ${gate.unobservableMachines.join(' and ')} state machine, which this observation does not read`;
    case 'no-phase-gate': {
      if (gate.routes.length === 0) return 'no published gate; no census route is established for this act';
      // "No gate was read" and "there is no state to read" are different
      // answers, and only the second is final. A route in a program that
      // persists no lifecycle discriminant will never acquire a gate, so a
      // card that keeps saying "declares none" invites a reader to wait for
      // one.
      return gate.routes.every(routeHasNoStateMachineV1)
        ? `no published gate; ${gate.routes.join(', ')} runs in a program that persists no lifecycle state to gate on`
        : `no published gate; ${gate.routes.join(', ')} declares none`;
    }
  }
}

/**
 * Every act with no published phase gate, by name.
 *
 * An act gated on a machine this snapshot cannot observe is NOT in this list:
 * the census read a gate for it, and calling it ungated would be the same
 * false claim `no-phase-gate` exists to prevent one level down.
 */
export function capabilityActsWithNoPhaseGateV1(): ReadonlyArray<string> {
  return Object.freeze(CAPABILITY_ACTIONS_V1
    .filter((act) => capabilityActPhaseGatesV1(act).length === 0
      && capabilityActUnobservableMachinesV1(act).length === 0)
    .map((act) => act.id));
}

/** What a reader can do about this act right now, given what has been read. */
export function evaluateCapabilityV1(
  standing: CapabilityStandingV1,
  snapshot: CapabilityMarketSnapshotV1 | null,
): CapabilityVerdictV1 {
  if (standing.venue === 'no-venue') {
    return Object.freeze({
      standing,
      status: 'no-venue',
      reason: standing.walls[0]?.statement ?? 'No route, command, or constructor reaches this act.',
      phaseGate: NO_GATE_READ,
    });
  }
  if (standing.venue === 'operator-cli') {
    return Object.freeze({ standing, status: 'operator-only', reason: capabilityVenueTextV1(standing), phaseGate: NO_GATE_READ });
  }
  if (snapshot === null) {
    return Object.freeze({ standing, status: 'needs-chain', reason: 'Read the selected programs, and any Market you name, at one finalized floor first.', phaseGate: NO_GATE_READ });
  }
  if (capabilityRequiresMarketV1(standing.action) && snapshot.market === null) {
    return Object.freeze({ standing, status: 'needs-market', reason: 'Name one Core-owned Market and read it at the same finalized floor first.', phaseGate: NO_GATE_READ });
  }
  // An act that CREATES a Market is never about the Market on screen, whatever
  // phase that one is in. Saying so is not pedantry: `ready-to-preflight`
  // beside an open Market reads as ready FOR IT, and this is the one status
  // where that reading is unrecoverably wrong -- the act cannot touch it at
  // all. The phase is reported anyway, because the reason a reader needs is
  // which Market they are holding, not that they are holding one.
  if (standing.action.subject === 'new-market' && snapshot.market !== null) {
    const held = snapshot.market.phase === null
      ? snapshot.market.address
      : `${snapshot.market.address}, ${snapshot.market.phase}`;
    return Object.freeze({
      standing,
      status: 'not-this-market',
      reason: `This act founds a NEW Market. The one this observation holds (${held}) is not its subject and nothing here can act on it; clear the Market coordinate to preflight a founding.`,
      phaseGate: Object.freeze({
        routes: standing.action.routes,
        gates: capabilityActPhaseGatesV1(standing.action),
        verdict: 'no-phase-gate',
        excludedBy: null,
        unobservableMachines: capabilityActUnobservableMachinesV1(standing.action),
      }),
    });
  }

  const gates = capabilityActPhaseGatesV1(standing.action);
  const routes = standing.action.routes;
  const unobservableMachines = capabilityActUnobservableMachinesV1(standing.action);
  // A machine this snapshot cannot observe is checked BEFORE the Market gates,
  // and it is checked even when the Market gates would admit. A route gated on
  // `market: Open+Consumed` and `source: Primary` passes both or neither, and
  // reporting the Market half as an admission is exactly the half-answer this
  // whole chain replaced.
  if (unobservableMachines.length > 0) {
    return Object.freeze({
      standing,
      status: 'needs-chain',
      reason: `This act is also gated on the ${unobservableMachines.join(' and ')} state machine, which this observation does not read. Read that state at the same finalized floor before calling this act attemptable.`,
      phaseGate: Object.freeze({ routes, gates, verdict: 'other-machine', excludedBy: null, unobservableMachines }),
    });
  }
  if (gates.length === 0) {
    return Object.freeze({
      standing,
      status: 'ready-to-preflight',
      reason: standing.action.guarantee,
      phaseGate: Object.freeze({ routes, gates, verdict: 'no-phase-gate', excludedBy: null, unobservableMachines }),
    });
  }
  // A gated act is about a Market by construction, so a gate with no Market
  // read is unread rather than admitted.
  const market = snapshot.market;
  if (market === null || market.phase === null) {
    return Object.freeze({
      standing,
      status: 'needs-chain',
      reason: `This act is admitted only at ${gateTextV1(gates[0])}, and the Market's Core phase was not decoded at this observation. Read the Market again at one finalized floor.`,
      phaseGate: Object.freeze({ routes, gates, verdict: 'unread', excludedBy: null, unobservableMachines }),
    });
  }
  // Gates are conjunctive: every one on the path admits, so one refusal is the
  // whole refusal.
  const excludedBy = gates.find((gate) => !gateAdmitsV1(gate, market.phase as MarketPhaseV1, market.readiness)) ?? null;
  if (excludedBy !== null) {
    const observed = market.readiness === null ? market.phase : `${market.phase}+${market.readiness}`;
    return Object.freeze({
      standing,
      status: 'wrong-phase',
      reason: `\`${excludedBy.route}\` admits only ${gateTextV1(excludedBy)}; this Market is ${observed}. The chain refuses this act before any account is read.`,
      phaseGate: Object.freeze({ routes, gates, verdict: 'excluded', excludedBy, unobservableMachines }),
    });
  }
  return Object.freeze({
    standing,
    status: 'ready-to-preflight',
    reason: standing.action.guarantee,
    phaseGate: Object.freeze({ routes, gates, verdict: 'admitted', excludedBy: null, unobservableMachines }),
  });
}

export function capabilityActionsForStageV1(stage: CapabilityStage): ReadonlyArray<CapabilityActionV1> {
  return CAPABILITY_ACTIONS_V1.filter((candidate) => candidate.stage === stage);
}
