import type { OperatorSurfaceSnapshotV1 } from './operatorSurface';

export const CAPABILITY_STAGES = ['author', 'trade', 'resolve', 'claim'] as const;
export type CapabilityStage = (typeof CAPABILITY_STAGES)[number];
export type CapabilityFamily = 'Release' | 'Creation' | 'Direct' | 'Source' | 'Series' | 'General' | 'Dealer' | 'Claims';
export type CapabilityImplementation = 'browser-unsigned' | 'rust-unsigned' | 'awaiting-production';

export type CapabilityActionV1 = Readonly<{
  id: string;
  stage: CapabilityStage;
  family: CapabilityFamily;
  action: string;
  implementation: CapabilityImplementation;
  workspace: string | null;
  requiresMarket: boolean;
  exactBoundary: string;
}>;

export type CapabilityVerdictV1 = Readonly<{
  action: CapabilityActionV1;
  status: 'ready-to-preflight' | 'needs-chain' | 'needs-market' | 'rust-only' | 'unavailable';
  reason: string;
}>;

const action = (
  id: string,
  stage: CapabilityStage,
  family: CapabilityFamily,
  label: string,
  implementation: CapabilityImplementation,
  workspace: string | null,
  requiresMarket: boolean,
  exactBoundary: string,
): CapabilityActionV1 => Object.freeze({ id, stage, family, action: label, implementation, workspace, requiresMarket, exactBoundary });

export const CAPABILITY_ACTIONS_V1: ReadonlyArray<CapabilityActionV1> = Object.freeze([
  action('release.activate', 'author', 'Release', 'Activate checked multiprogram release', 'browser-unsigned', '/release', false, 'Registry activation, all six role artifacts, Loader V3 deployment observations, immutable policies, and a recent blockhash are reacquired by the release workspace.'),
  action('product.compile', 'author', 'Creation', 'Compile runtime-width Product result domain', 'browser-unsigned', '/product-v2', false, 'The compiler emits canonical Product, domain, portfolio, and content identities without claiming any account exists onchain.'),
  action('market.inspect', 'author', 'Creation', 'Inspect Realm and Market authority', 'browser-unsigned', '/create', false, 'Realm and Market are optional explicit coordinates; when supplied they must be non-executable, distinct, present at the same finalized floor, and owned by selected Core.'),
  action('market.found', 'author', 'Creation', 'Found common Core Market', 'browser-unsigned', '/found', false, 'The Found31 workspace independently authenticates runtime Product records, infrastructure profile and immutable Registry/Rent artifacts before constructing unsigned v0 bytes.'),
  action('source.create-fund', 'author', 'Source', 'Create resolution fund', 'awaiting-production', '/resolution', true, 'The production Core→Resolution action is reachable, but no chain-derived operator snapshot currently selects its authority and account frame. The browser will not construct from wire constants alone.'),

  action('direct.inline', 'trade', 'Direct', 'Fill categorical or graded inline intents', 'browser-unsigned', '/trade', true, 'The Direct V3 workspace reacquires Hot38, Product runtime width, ProgramSet selection, descriptor, lifecycle, strategy, TransitionVM, AccountProfile, Loader state, and checked release evidence.'),
  action('direct.register', 'trade', 'Direct', 'Create registered order', 'awaiting-production', null, true, 'The visible legacy registration encoder is intentionally excluded: successor maker-root ownership and production registration artifacts are not frozen.'),
  action('direct.cancel', 'trade', 'Direct', 'Cancel / expire / CancelThrough', 'awaiting-production', null, true, 'No browser action is exposed until successor replay roots, action artifacts, and terminal account profiles form one accepted production route.'),
  action('series.prepare', 'trade', 'Series', 'Prepare occurrence and ticket', 'rust-unsigned', '/operate', true, 'A chain-derived Rust Hot V3 builder exists; a generated browser ABI and production release bundle have not yet crossed the web boundary.'),
  action('general.consider', 'trade', 'General', 'Consider candidate / freeze selection', 'browser-unsigned', '/general', true, 'The General workspace derives the current candidate and selection lifecycle, action artifacts, exact PDA bumps, lookup table, and packet-safe unsigned transaction.'),
  action('dealer.liquidity', 'trade', 'Dealer', 'Activate pool / add or remove bounded liquidity', 'rust-unsigned', '/liquidity', true, 'The Rust operator derives Dealer equity and custody coordinates. Browser construction waits for the finalized production artifact bundle rather than inventing pool state.'),
  action('dealer.trade', 'trade', 'Dealer', 'Inventory-bounded immediate trade', 'rust-unsigned', '/liquidity', true, 'The Dealer successor kernel and operator are present; the browser still lacks a generated, release-selected transaction encoder.'),

  action('source.ready', 'resolve', 'Source', 'Verify resolution fund ready', 'awaiting-production', '/resolution', true, 'The production action is reachable, but the operator has no current authority-selection snapshot/report builder. A browser constructor would invent authority.'),
  action('source.provider', 'resolve', 'Source', 'Submit real provider evidence / reclaim', 'rust-unsigned', '/resolution', true, 'Real-provider 38-account submit and permissionless 18-account reclaim constructors exist in Rust; no browser copy of their ABI is accepted.'),
  action('source.admit-terminal', 'resolve', 'Source', 'Admit terminal resolution', 'rust-unsigned', '/resolution', true, 'The Rust operator checks the Product record digest, runtime-u32 selector, terminal certificate, and current resolution state before construction.'),
  action('source.close-fund', 'resolve', 'Source', 'Close resolution fund', 'rust-unsigned', '/resolution', true, 'The Rust operator requires the exact terminal and refund preconditions; browser transaction bytes remain unavailable until generated ABI parity lands.'),
  action('general.settle', 'resolve', 'General', 'Initialize / collect / materialize / distribute', 'browser-unsigned', '/general', true, 'The General workspace exposes only action artifacts whose current state/lifecycle recipes and child geometry can be fully reacquired and packet-checked.'),

  action('claims.conserve', 'claim', 'Claims', 'Split / merge conservative claims', 'browser-unsigned', '/economic', true, 'The economic workspace re-decodes release authority and exact conservative supply from current chain state before construction.'),
  action('claims.represent', 'claim', 'Claims', 'Materialize / dematerialize representation', 'browser-unsigned', '/economic', true, 'Native and represented supplies, custody effects, and exact integer payoff boundaries are derived from the selected Market and current accounts.'),
  action('claims.redeem', 'claim', 'Claims', 'Redeem terminal Rational / Bearer representation', 'rust-unsigned', '/redeem', true, 'The production terminal contract and Rust operator exist. Browser construction waits for the accepted Hot38 release bundle and generated account-profile encoder.'),
  action('series.close', 'claim', 'Series', 'Consume / expire ticket and close occurrence', 'rust-unsigned', '/operate', true, 'The Series V3 operator owns exact transaction planning, but the browser has no current production release manifest encoder.'),
  action('general.close', 'claim', 'General', 'Close settlement / General root', 'browser-unsigned', '/general', true, 'Close derives the successor terminal coordinate from expected revision plus one and recomputes all canonical state PDAs before unsigned construction.'),
  action('dealer.close', 'claim', 'Dealer', 'Reset ladder / close LP / retire pool', 'rust-unsigned', '/liquidity', true, 'Rust successor builders exist; the browser remains unavailable until exact production artifacts and account profile are generated and selected.'),
]);

export function evaluateCapabilityV1(
  actionDefinition: CapabilityActionV1,
  snapshot: OperatorSurfaceSnapshotV1 | null,
): CapabilityVerdictV1 {
  if (actionDefinition.implementation === 'awaiting-production') {
    return Object.freeze({ action: actionDefinition, status: 'unavailable', reason: actionDefinition.exactBoundary });
  }
  if (snapshot === null) {
    return Object.freeze({ action: actionDefinition, status: 'needs-chain', reason: 'Reacquire the selected role programs and optional Core state at one finalized floor first.' });
  }
  if (actionDefinition.requiresMarket && snapshot.market === null) {
    return Object.freeze({ action: actionDefinition, status: 'needs-market', reason: 'Select and authenticate one Core-owned Market at the same finalized observation floor first.' });
  }
  if (actionDefinition.implementation === 'rust-unsigned') {
    return Object.freeze({ action: actionDefinition, status: 'rust-only', reason: actionDefinition.exactBoundary });
  }
  return Object.freeze({ action: actionDefinition, status: 'ready-to-preflight', reason: actionDefinition.exactBoundary });
}

export function capabilityActionsForStageV1(stage: CapabilityStage): ReadonlyArray<CapabilityActionV1> {
  return CAPABILITY_ACTIONS_V1.filter((candidate) => candidate.stage === stage);
}
