import type { OperatorSurfaceSnapshotV1 } from './operatorSurface';
import { marketDetailHrefV1 } from './marketHref';

export const CAPABILITY_STAGES = ['author', 'trade', 'resolve', 'claim'] as const;
export type CapabilityStage = (typeof CAPABILITY_STAGES)[number];
export type CapabilityFamily = 'Release' | 'Creation' | 'Direct' | 'Source' | 'Series' | 'General' | 'Dealer' | 'Claims';
export type CapabilityImplementation = 'browser-unsigned' | 'browser-message' | 'browser-wallet' | 'operator-artifact' | 'operator-campaign' | 'rust-unsigned' | 'awaiting-production';
export type CapabilityWorkspaceV1 = string | 'market-detail';

export type CapabilityActionV1 = Readonly<{
  id: string;
  stage: CapabilityStage;
  family: CapabilityFamily;
  action: string;
  implementation: CapabilityImplementation;
  workspace: CapabilityWorkspaceV1 | null;
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
  workspace: CapabilityWorkspaceV1 | null,
  requiresMarket: boolean,
  exactBoundary: string,
): CapabilityActionV1 => Object.freeze({ id, stage, family, action: label, implementation, workspace, requiresMarket, exactBoundary });

export const CAPABILITY_ACTIONS_V1: ReadonlyArray<CapabilityActionV1> = Object.freeze([
  action('release.activate', 'author', 'Release', 'Activate checked multiprogram release', 'browser-unsigned', '/release', false, 'Registry activation, all six role artifacts, Loader V3 deployment observations, immutable policies, and a recent blockhash are reacquired by the release workspace.'),
  action('product.compile', 'author', 'Creation', 'Compile an admitted degree-2/3 Product graph', 'operator-artifact', '/product-v2#spline-product', false, 'The key-free CLI delegates canonical JSON to the production Rust spline-basis, price-gate, and Product compilers. It atomically emits the five immutable record files plus a machine report binding their schemas, hashes, semantic basis identity, and raw/staging addresses; it does not read a chain, sign, submit, or found a Market.'),
  action('market.inspect', 'author', 'Creation', 'Inspect Realm and Market authority', 'browser-unsigned', '/create', false, 'Realm and Market are optional explicit coordinates; when supplied they must be non-executable, distinct, present at the same finalized floor, and owned by selected Core.'),
  action('market.found', 'author', 'Creation', 'Found a current Market and first participant', 'operator-campaign', '/found#current-founding', false, 'The key-owning Rust children author the finalized prestates, compact atomic opening, participant admission, reports, and recovery checkpoints. The CLI records explicit devnet execution authority before they can read a key and resumes only the same operation journal.'),
  action('market.join', 'author', 'Creation', 'Admit another participant', 'operator-campaign', 'market-detail', true, 'The selected Market page first authenticates the connected wallet’s participant accounts. The CLI then asks the Rust admission child to plan or execute against the caller-named founding plan and evidence, writes a durable report, and derives the admitted identity only from the caller-named key file.'),
  action('source.create-fund', 'author', 'Source', 'Create resolution fund', 'browser-wallet', '/resolution', true, 'Create or activate the Market’s exact Source funding state. The checked Rust/WASM owner derives the current release, record, ledger, authority, and instruction frame; the browser acquires one finalized observation, saves the sole-payer packet before wallet consent and submission, sends once, and verifies the adjacent finalized route.'),

  action('direct.route', 'trade', 'Direct', 'Export an authenticated Direct route', 'operator-artifact', '/operate#direct-route', false, 'The key-free CLI asks the Rust successor to bind the live activated releases, caller-pinned checked-release files, frozen lookup-table journal, and finalized Direct planning snapshot. It emits a portable route plus machine report; it cannot sign or submit.'),
  action('direct.author', 'trade', 'Direct', 'Author a portable sell offer', 'browser-message', 'market-detail', true, 'The selected Market page reacquires the seller’s exact Claims Position and canonical maker nonce, binds an explicit size, exact price tick, fill rule, and lifetime, then requests one detached message signature. It preserves the canonical ticket without a relay; no transaction is created or submitted and no claims move.'),
  action('direct.inline', 'trade', 'Direct', 'Take and execute a Direct offer', 'browser-wallet', 'market-detail', true, 'The selected Market page authenticates the current Direct release and both participants, previews exact integer effects, persists intent and signed packet before submission, resumes only the saved transaction id, and verifies finalized economic poststates. Each wallet request is an explicit act.'),
  action('direct.register', 'trade', 'Direct', 'Create registered order', 'awaiting-production', null, true, 'The visible legacy registration encoder is intentionally excluded: successor maker-root ownership and production registration artifacts are not frozen.'),
  action('direct.cancel', 'trade', 'Direct', 'Cancel / expire / CancelThrough', 'awaiting-production', null, true, 'No browser action is exposed until successor replay roots, action artifacts, and terminal account profiles form one accepted production route.'),
  action('series.prepare', 'trade', 'Series', 'Prepare occurrence and ticket', 'rust-unsigned', '/operate', true, 'A chain-derived Rust Hot V3 builder exists; a generated browser ABI and production release bundle have not yet crossed the web boundary.'),
  action('general.consider', 'trade', 'General', 'Consider candidate / freeze selection', 'browser-unsigned', '/general', true, 'The General workspace derives the current candidate and selection lifecycle, action artifacts, exact PDA bumps, lookup table, and packet-safe unsigned transaction.'),
  action('dealer.liquidity', 'trade', 'Dealer', 'Activate pool / add or remove bounded liquidity', 'rust-unsigned', '/liquidity', true, 'The Rust operator derives Dealer equity and custody coordinates. Browser construction waits for the finalized production artifact bundle rather than inventing pool state.'),
  action('dealer.trade', 'trade', 'Dealer', 'Inventory-bounded immediate trade', 'rust-unsigned', '/liquidity', true, 'The Dealer successor kernel and operator are present; the browser still lacks a generated, release-selected transaction encoder.'),

  action('source.ready', 'resolve', 'Source', 'Verify resolution fund ready', 'browser-wallet', '/resolution', true, 'Reauthenticate the active Market-bound funding set and have Core accept it as ready. The checked Rust/WASM owner selects the permissionless VerifyFundReady instruction from exact finalized state; the browser saves the sole-payer packet, requests one explicit wallet act, submits once, and clears recovery only after the Ready poststate selects the terminal route.'),
  action('source.provider', 'resolve', 'Source', 'Submit real provider evidence / reclaim', 'browser-wallet', '/resolution', true, 'Submit reacquires the current Market-to-provider release graph and verified EncodedVaa, then the Rust/WASM owner constructs one atomic lifecycle prepay and evidence transaction for the wallet and a fresh operation-scoped update signer. Reclaim derives its exact consumed lifecycle frame, uses a fresh readonly resolver beside the fee-payer wallet, and clears recovery only after Rust verifies the terminal accounts.'),
  action('source.admit-terminal', 'resolve', 'Source', 'Admit terminal resolution', 'browser-wallet', '/resolution', true, 'Reacquire the terminal Source, runtime-width Product graph, certificate, release deployments, and exact three-entry funding subset, then have Core name that certificate. The Rust/WASM owner derives all 22 protocol accounts and the caller-authority request; the browser saves the sole-payer packet, submits once, and clears recovery only after Rust reauthenticates the exact Terminal receipt.'),
  action('source.close-fund', 'resolve', 'Source', 'Close resolution fund', 'browser-wallet', '/resolution', true, 'Reacquire the Retiring Source, durably prepay its exact closure-receipt rent when needed, then wallet-submit the signer-free V7 direct close and verify the finalized typed receipt.'),
  action('general.settle', 'resolve', 'General', 'Initialize / collect / materialize / distribute', 'browser-unsigned', '/general', true, 'The General workspace exposes only action artifacts whose current state/lifecycle recipes and child geometry can be fully reacquired and packet-checked.'),

  action('claims.conserve', 'claim', 'Claims', 'Split / merge conservative claims', 'awaiting-production', null, true, 'The browser workspace that claimed this action encoded the schema-1 DCLTECO1 economic projection, whose only program was banished; the live successor speaks DCLTEMK2/DCLTEPS2 through dclutch-economic-slice-kernel and has no generated browser ABI. Nothing is constructed from a wire no deployed program reads.'),
  action('claims.represent', 'claim', 'Claims', 'Materialize / dematerialize representation', 'awaiting-production', null, true, 'Representation conversion was reachable only through the same banished schema-1 projection. Native and represented supplies now live in Claims/Custody state, and no browser encoder for that route has been generated from its owning crate.'),
  action('claims.redeem', 'claim', 'Claims', 'Redeem a terminal Claims Position', 'browser-wallet', '/redeem', true, 'The page derives the connected wallet’s Position from the live Market set. After you supply the Rust-authored payout plan for that exact Position, it reacquires the route, creates the wallet replay account when needed, persists the plan and signed packet before submission, resumes only the saved transaction id, and clears recovery state only after finalized payout poststate verifies.'),
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
  if (actionDefinition.implementation === 'operator-artifact'
      || (actionDefinition.implementation === 'operator-campaign' && !actionDefinition.requiresMarket)) {
    return Object.freeze({ action: actionDefinition, status: 'rust-only', reason: actionDefinition.exactBoundary });
  }
  if (snapshot === null) {
    return Object.freeze({ action: actionDefinition, status: 'needs-chain', reason: 'Reacquire the selected role programs and optional Core state at one finalized floor first.' });
  }
  if (actionDefinition.requiresMarket && snapshot.market === null) {
    return Object.freeze({ action: actionDefinition, status: 'needs-market', reason: 'Select and authenticate one Core-owned Market at the same finalized observation floor first.' });
  }
  if (actionDefinition.implementation === 'rust-unsigned' || actionDefinition.implementation === 'operator-campaign') {
    return Object.freeze({ action: actionDefinition, status: 'rust-only', reason: actionDefinition.exactBoundary });
  }
  return Object.freeze({ action: actionDefinition, status: 'ready-to-preflight', reason: actionDefinition.exactBoundary });
}

export type CapabilityActContractV1 = Readonly<{
  authority: string;
  result: string;
}>;

/** What one implementation class asks for and what it can produce. */
export function capabilityActContractV1(actionDefinition: CapabilityActionV1): CapabilityActContractV1 {
  switch (actionDefinition.implementation) {
    case 'browser-unsigned':
      return Object.freeze({ authority: 'No key access in this act.', result: 'Browser produces checked unsigned transaction bytes.' });
    case 'browser-message':
      return Object.freeze({ authority: 'Wallet signs one detached message; no transaction.', result: 'Browser produces a portable signed artifact.' });
    case 'browser-wallet':
      return Object.freeze({ authority: 'Wallet transaction authority is requested explicitly.', result: 'Browser submits once, resumes by transaction id, and verifies finalized poststate.' });
    case 'operator-artifact':
      return Object.freeze({ authority: 'No key or wallet access; finalized devnet reads only.', result: 'Operator CLI produces a pinned portable artifact and machine report.' });
    case 'operator-campaign':
      return Object.freeze({ authority: 'Explicit devnet execution; only caller-named Rust child key files.', result: 'Operator CLI resumes a durable campaign and verifies its machine reports.' });
    case 'rust-unsigned':
      return Object.freeze({ authority: 'No browser key access.', result: 'Rust operator tooling produces the checked unsigned transaction.' });
    case 'awaiting-production':
      return Object.freeze({ authority: 'No authority request is exposed.', result: 'No executable artifact is produced until the named seam exists.' });
  }
}

/** Resolve a market-bound workspace only from the Market actually reacquired. */
export function capabilityWorkspaceV1(
  actionDefinition: CapabilityActionV1,
  snapshot: OperatorSurfaceSnapshotV1 | null,
): string | null {
  if (actionDefinition.workspace === null) return null;
  if (actionDefinition.workspace !== 'market-detail') return actionDefinition.workspace;
  return snapshot?.market === null || snapshot?.market === undefined
    ? null
    : marketDetailHrefV1(snapshot.market.address);
}

export function capabilityActionsForStageV1(stage: CapabilityStage): ReadonlyArray<CapabilityActionV1> {
  return CAPABILITY_ACTIONS_V1.filter((candidate) => candidate.stage === stage);
}
