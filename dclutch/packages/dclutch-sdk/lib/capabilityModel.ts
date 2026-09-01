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

export type CapabilityActionV1 = Readonly<{
  id: string;
  stage: CapabilityStage;
  family: CapabilityFamily;
  /** The outcome, in the reader's terms. Cards lead with this. */
  action: string;
  workspace: CapabilityWorkspaceV1 | null;
  requiresMarket: boolean;
  anchors: CapabilityAnchorsV1;
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
  requiresMarket: boolean,
  actionAnchors: CapabilityAnchorsV1,
  guarantee: string,
  walls: ReadonlyArray<CapabilityWallV1> = [],
): CapabilityActionV1 => Object.freeze({
  id, stage, family, action: label, workspace, requiresMarket, anchors: actionAnchors, guarantee, walls: Object.freeze([...walls]),
});

export const CAPABILITY_ACTIONS_V1: ReadonlyArray<CapabilityActionV1> = Object.freeze([
  action('release.activate', 'author', 'Release', 'Activate a checked multiprogram release', '/release', false,
    anchors('components/ReleaseWorkspace.tsx', 'lib/releaseRegistry.ts'),
    'Each role packet is signed on its own and leaves as a file; this page never sends one.'),
  action('product.compile', 'author', 'Creation', 'Compile a Product record and its admission request', '/product-v2', false,
    anchors('components/ProductV2Studio.tsx', 'lib/productV2.ts', 'components/ProductV2Studio.tsx'),
    'Nothing is read from a chain and nothing is signed: the output is a record and an instruction, not a transaction.'),
  action('market.inspect', 'author', 'Creation', 'Check a founding against the chain before you commit to it', '/create', false,
    anchors('components/CreateMarketWizard.tsx', 'lib/coreFound.ts'),
    'Finalized reads only. The wizard reports what the founding would cost and refuse; it exports no packet.'),
  action('market.found', 'author', 'Creation', 'Found a Market and admit its first participant', '/found', false,
    anchors('components/CoreFoundWorkspace.tsx', 'lib/coreFound.ts', 'components/CoreFoundWorkspace.tsx'),
    'The browser exports unsigned bytes and asks for no key; the published campaign records devnet authorization before any child may sign.'),
  action('market.join', 'author', 'Creation', 'Admit another participant', 'market-detail', true,
    anchors('components/JoinPanel.tsx', 'lib/userPositionAdmissionOperation.ts'),
    'The compiled Rust planner derives all 27 accounts from one finalized observation; the exact packet is saved before your wallet sees it, sent once, and cleared only after the chain confirms it, so reloading resumes and never resubmits.'),
  action('source.create-fund', 'author', 'Source', 'Create the resolution fund', '/resolution', true,
    anchors('components/ResolutionWorkspace.tsx', 'lib/sourceReadinessV1.ts'),
    'The exact packet is saved before your wallet sees it, sent once, and cleared only after the finalized poststate is read back; reloading resumes and never resubmits.'),

  action('direct.route', 'trade', 'Direct', 'Export a portable Direct route', '/operate', false,
    anchors('components/OperatorSurface.tsx', null, 'components/OperatorSurface.tsx'),
    'The published command holds no key and can neither sign nor send: it reads finalized state and writes two files.'),
  action('direct.author', 'trade', 'Direct', 'Author a portable sell offer', 'market-detail', true,
    anchors('components/trade/MakerOfferComposer.tsx', 'lib/directOfferAuthoring.ts'),
    'One detached message signature. No transaction is built and no claims move; the ticket is yours to keep or hand on.'),
  action('direct.inline', 'trade', 'Direct', 'Take and execute a signed offer', 'market-detail', true,
    anchors('lib/tradeFlowMachine.ts', 'lib/directInlineV3.ts'),
    'Two separate signatures, neither of which sends. The signed packet is saved before submission, sent once, and reconciled against finalized balances.',
    [wall('Trading runs to 1,330,239 of 1,399,700 CU and dies ProgramFailedToComplete; the fill can exhaust its budget on chain.', 'GOAL.md')]),
  action('direct.register', 'trade', 'Direct', 'Create a registered resting order', null, true,
    NO_ANCHORS,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('No route renders a control for it and no browser module builds its transaction; the registered-order wire lives in the Rust codec.', 'crates/dclutch-direct-codec')]),
  action('direct.cancel', 'trade', 'Direct', 'Cancel, expire, or cancel through a resting order', null, true,
    NO_ANCHORS,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('No route renders a control for it and no browser module builds its transaction; successor replay roots and terminal account profiles are not one accepted route.', 'crates/dclutch-direct-codec')]),
  action('series.prepare', 'trade', 'Series', 'Prepare an occurrence and its ticket', null, true,
    NO_ANCHORS,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('A Template with nonzero close rent describes a root activation may not fund and Close can never open; the activation funding seam is jointly unsatisfiable.', 'WAVE.md')]),
  action('general.consider', 'trade', 'General', 'Check a candidate plan and export its exact packet', '/general', true,
    anchors('components/GeneralWorkspace.tsx', 'lib/generalPlanV5.ts'),
    'The plan is authored elsewhere; this page authenticates it against finalized state and hands back the same bytes. No key is asked for.'),
  action('dealer.liquidity', 'trade', 'Dealer', 'Contribute or redeem dealer equity', '/liquidity', true,
    anchors('components/DealerLiquidityWorkspace.tsx', 'lib/dealerEquityV3.ts'),
    'The packet is signed here and downloaded; this page submits nothing, so an external submitter is the only thing that can send it.',
    [wall('Exactly one selector can satisfy validate_selection: derivation_policy is pinned per descriptor to its own lifecycle digest and per root to a single manifest entry.', 'WAVE.md')]),
  action('dealer.trade', 'trade', 'Dealer', 'Take an inventory-bounded immediate trade', null, true,
    NO_ANCHORS,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('No route renders a control for it and no browser module builds its transaction; the Dealer scenario kernel owns it.', 'crates/dclutch-dealer-scenario-kernel')]),

  action('source.ready', 'resolve', 'Source', 'Have Core accept the fund as ready', '/resolution', true,
    anchors('components/ResolutionWorkspace.tsx', 'lib/sourceReadinessV1.ts'),
    'The exact packet is saved before your wallet sees it, sent once, and cleared only after the Ready poststate selects the terminal route.'),
  action('source.provider', 'resolve', 'Source', 'Submit provider evidence, or reclaim it', '/resolution', true,
    anchors('components/ResolutionWorkspace.tsx', 'lib/sourceProviderV1.ts'),
    'Two signatures on one immutable message — a fresh operation signer and your wallet — saved before submission and verified against the terminal accounts.'),
  action('source.admit-terminal', 'resolve', 'Source', 'Admit the terminal resolution', '/resolution', true,
    anchors('components/ResolutionWorkspace.tsx', 'lib/sourceTerminalV1.ts'),
    'The signed record is saved before one submission and kept until the finalized Terminal receipt is read back; a reload resumes the same signature.'),
  action('source.close-fund', 'resolve', 'Source', 'Close the resolution fund', '/resolution', true,
    anchors('components/ResolutionWorkspace.tsx', 'lib/sourceCloseFundV1.ts'),
    'Prepay and close are separate signed acts; each is saved before submission and confirmed against the finalized typed receipt.'),
  action('general.settle', 'resolve', 'General', 'Check a settlement plan and export its exact packet', '/general', true,
    anchors('components/GeneralWorkspace.tsx', 'lib/generalPlanV5.ts'),
    'The plan is authored elsewhere; this page authenticates it against finalized state and hands back the same bytes. No key is asked for.'),

  action('claims.conserve', 'claim', 'Claims', 'Split or merge conservative claims', null, true,
    NO_ANCHORS,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('No route renders a control for it and no browser module builds its transaction; the conservation contract owns the wire.', 'crates/dclutch-claims-conservation-contract')]),
  action('claims.represent', 'claim', 'Claims', 'Materialize or dematerialize a representation', null, true,
    NO_ANCHORS,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('No route renders a control for it and no browser module builds its transaction; the representation codec owns the wire.', 'crates/dclutch-claims-representation-codec'),
     wall('The structured representation campaign stands at a named ATA-derivation wall.', 'GOAL.md')]),
  action('claims.replay', 'claim', 'Claims', 'Create the replay account redemption requires', '/redeem', true,
    anchors('components/RedeemFlow.tsx', 'lib/claimsCustodyReplay.ts'),
    'One signature for the account the chain demands before payout; saved before submission, sent once, and confirmed finalized before the payout step opens.'),
  action('claims.redeem', 'claim', 'Claims', 'Redeem a terminal Claims Position', '/redeem', true,
    anchors('components/RedeemFlow.tsx', 'lib/walletTerminalPayoutV3.ts'),
    'The payout plan and the signed packet are both saved before one submission; recovery resumes the saved signature and never sends a second.'),
  action('series.close', 'claim', 'Series', 'Consume or expire a ticket and close the occurrence', null, true,
    NO_ANCHORS,
    'No signature is requested, because nothing here can build this transaction.',
    [wall('No route renders a control for it and no browser module builds its transaction; the Series V3 kernel owns the exact planning.', 'crates/dclutch-series-v3-kernel'),
     wall('Expire refused on a real ELF at custom code 16387 with the permit account absent.', 'docs/evidence/SERIES_PERMIT_EXPIRY_HOT_WALL_2026_08_31.json')]),
  action('general.close', 'claim', 'General', 'Check a close plan and export its exact packet', '/general', true,
    anchors('components/GeneralWorkspace.tsx', 'lib/generalPlanV5.ts'),
    'The plan is authored elsewhere; this page authenticates it against finalized state and hands back the same bytes. No key is asked for.'),
  action('dealer.close', 'claim', 'Dealer', 'Reset the ladder, close an LP, or retire the pool', null, true,
    NO_ANCHORS,
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

export type CapabilityVerdictV1 = Readonly<{
  standing: CapabilityStandingV1;
  status: 'ready-to-preflight' | 'needs-chain' | 'needs-market' | 'operator-only' | 'no-venue';
  reason: string;
}>;

/**
 * One chain-observed snapshot, described only by the field this decision uses.
 *
 * Structural rather than imported so the two trees' snapshot types can differ
 * without either one silently changing a status.
 */
export type CapabilityMarketSnapshotV1 = Readonly<{ market: Readonly<{ address: string }> | null }>;

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
    });
  }
  if (standing.venue === 'operator-cli') {
    return Object.freeze({ standing, status: 'operator-only', reason: capabilityVenueTextV1(standing) });
  }
  if (snapshot === null) {
    return Object.freeze({ standing, status: 'needs-chain', reason: 'Read the selected programs, and any Market you name, at one finalized floor first.' });
  }
  if (standing.action.requiresMarket && snapshot.market === null) {
    return Object.freeze({ standing, status: 'needs-market', reason: 'Name one Core-owned Market and read it at the same finalized floor first.' });
  }
  return Object.freeze({ standing, status: 'ready-to-preflight', reason: standing.action.guarantee });
}

export function capabilityActionsForStageV1(stage: CapabilityStage): ReadonlyArray<CapabilityActionV1> {
  return CAPABILITY_ACTIONS_V1.filter((candidate) => candidate.stage === stage);
}
