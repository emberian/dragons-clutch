/**
 * The founding ladder, stated honestly.
 *
 * Founding is not one transaction. Between "an operator has chosen a Product"
 * and "a Market is Open" there are five kinds of work — a collateral Mint, a
 * dozen finalized Registry records, a lifecycle RentCredit, a projected-Custody
 * prestate, and finally the atomic DCLTGMF1 outer — and they are not equally
 * reachable from a browser. Some have a browser builder today. Some are
 * derivations that only exist inside Rust kernels, and porting them by hand
 * would be re-deriving an authority rather than reading one.
 *
 * This module is the inventory. It exists so the review step of the wizard can
 * show the ladder as it really is, with each rung labelled by what actually
 * builds it, instead of showing a "Create market" button that is a lie about
 * five sixths of the work. `reason` is not UI filler: for every rung that is
 * tooling-only it names the specific thing that would have to exist first.
 */

export type FoundingRungStatusV1 =
  /** A builder in `apps/dclutch-web` emits this transaction's exact bytes. */
  | 'browser-builder'
  /**
   * The browser assembles the instruction, but a prerequisite input to it is
   * produced by Rust today. The frame is honest; the coordinates are borrowed.
   */
  | 'browser-frame-borrowed-coordinates'
  /** Only the Rust reference client builds this. The reason says why. */
  | 'tooling-only';

export type FoundingRungV1 = Readonly<{
  id: string;
  title: string;
  /** What lands on chain, in the operator's terms. */
  effect: string;
  /** How many transactions this rung costs, when that is knowable up front. */
  transactions: string;
  status: FoundingRungStatusV1;
  /** The builder that emits it, browser-side or Rust-side. */
  builder: string;
  /** Why it is where it is. For tooling-only rungs, what would move it. */
  reason: string;
  /** Whether this rung rides a v0 address lookup table. */
  lookupTable: boolean;
}>;

/**
 * The ladder in execution order, as `execute_found_market` and
 * `execute_projected_custody_bootstrap` run it.
 */
export const FOUNDING_LADDER_V1: ReadonlyArray<FoundingRungV1> = Object.freeze([
  Object.freeze({
    id: 'collateral',
    title: 'Collateral Mint and funded wallet',
    effect: 'A Token-2022 Mint at the chosen display precision, and a wallet holding the founding principal in raw atoms.',
    transactions: '2 (InitializeMint2 + InitializeAccount3/MintTo)',
    status: 'tooling-only',
    builder: 'market.rs · create_real_collateral',
    reason: 'The browser has no Token-2022 instruction builder. This is the one rung whose port is small and mechanical — three fixed-layout instructions — and it is the obvious next piece of browser founding work.',
    lookupTable: false,
  }),
  Object.freeze({
    id: 'records',
    title: 'Publish the semantic record graph',
    effect: 'Realm, Product, result domain, portfolio, Source spec, window spec, statistic spec, provider release, Pyth adapter config, recovery policy, Source material, capability manifest, and the linked liability basis, each finalized at the Registry PDA derived from the hash of its own body.',
    transactions: '3 per record (Begin, Append, Finalize), for thirteen records',
    status: 'tooling-only',
    builder: 'market.rs · publish_market_records over dclutch-product-runtime-v2-operator::publication',
    reason: 'Each body is compiled by a first-party Rust encoder — ProductCompilerV2, SourceSpecV1, WindowSpecV1, StatisticSpecV1, RecoveryPolicyV2, SourceMaterialV2, the liability-basis V3 linker. A finalized record lives at an address derived from the hash of its body, so an encoder that is one byte off names a record nobody can publish. Porting thirteen encoders by hand is exactly the re-derivation the one-authority discipline forbids; the honest path is an emitter per encoder, not a transcription.',
    lookupTable: false,
  }),
  Object.freeze({
    id: 'rent-credit',
    title: 'Market-scoped lifecycle RentCredit',
    effect: 'The immutable rent-refund beneficiary for this Market and generation, created before anything that will owe rent to it.',
    transactions: '1',
    status: 'browser-builder',
    builder: 'lib/coreFound.ts · compileLifecycleRentCreateTransactionV2',
    reason: 'Fully built in the browser from the generated RentV2 ABI, including the PDA and bump. The payer is the sole signer.',
    lookupTable: false,
  }),
  Object.freeze({
    id: 'found31',
    title: 'Found31 — the Market at Founding',
    effect: 'A Core Market account in phase Founding, its identity derived from the seven finalized record digests, the Registry program and the generation.',
    transactions: '1 (v0, 31 accounts, routed through a lookup table)',
    status: 'browser-builder',
    builder: 'lib/coreFound.ts · prepareCoreFoundV2 / compileCoreFoundTransactionV2',
    reason: 'The whole 31-account projection, every record authentication, and the exact rent debit are derived in the browser from finalized RPC. It does not fit a packet inline: with the ComputeBudget limit it cannot execute without, the message is 1,242 bytes against a 1,232-byte bound, so it rides the routing table below.',
    lookupTable: true,
  }),
  Object.freeze({
    id: 'custody-bootstrap',
    title: 'DCLTPCB1 — the projected-Custody prestate',
    effect: 'The Hoard vault, the founding source vault and its replay, the projected-Custody replay, and one FundingState per capability manifest entry, each prefunded to its rent plus that entry’s quoted native total.',
    transactions: '1 (v0 on a 256 KiB heap frame, 79 + tail accounts), plus its lookup table and prefunding',
    status: 'tooling-only',
    builder: 'market.rs · build_projected_custody_bootstrap_v1',
    reason: 'A 79-account frame whose coordinates are Custody PDAs derived from seeds the browser does not yet emit, and whose FundingState plan comes from plan_one_funding_state_v1 inside the Trading program. Reachable from a browser only after the Custody seed set gets an emitter.',
    lookupTable: true,
  }),
  Object.freeze({
    id: 'founding-requests',
    title: 'The four readonly founding requests',
    effect: 'The Found artifact, the terminal Lock, the Realize request, and the Claims FoundingV5 request, each published as a finalized Registry record whose address is the hash of its body.',
    transactions: '3 per request, for the two the founding derives',
    status: 'browser-frame-borrowed-coordinates',
    builder: 'lib/founding/genericFoundingRequest.ts (Found request) · market.rs · derive_founding_outer_v1 (the other three)',
    reason: 'The 400-byte GenericFoundingRequestV1 is built and byte-verified in the browser against the first-party Rust encoder. The Lock, Realize and Claims requests are not: their bodies are receipts produced by running the Custody kernel’s own lock_hoard_and_close_source and realize_and_close transitions over the exact prestate DCLTPCB1 left on chain. Those transitions are the authority for what the receipts say, and a browser re-implementation of them would be a second authority, not a client.',
    lookupTable: false,
  }),
  Object.freeze({
    id: 'prefunding',
    title: 'Prefund the five program-allocated accounts',
    effect: 'The Market, the one-shot Core permit, and the Claims aggregate, founder Position and admission, each moved to exactly its rent minimum.',
    transactions: '1 (five System transfers)',
    status: 'tooling-only',
    builder: 'market.rs · execute_generic_market_founding',
    reason: 'Nothing in the protocol funds these five: Core and Claims allocate and assign, never transfer, so each must already hold its rent. The transfers themselves are trivial; the three Claims rents are re-derived byte-exactly inside the permit’s committed Claims request, so the amounts come from the same derivation the requests do.',
    lookupTable: false,
  }),
  Object.freeze({
    id: 'routing-table',
    title: 'The DCLTGMF1 routing table',
    effect: 'An address lookup table holding every non-signer key in the founding frame, extended over several transactions and usable only strictly after the slot that last extended it.',
    transactions: '1 create + 1 per twenty addresses',
    status: 'browser-builder',
    builder: 'lib/founding/lookupTable.ts · planLookupTableV1 / lookupTableAccountV1',
    reason: 'Built in the browser, in the canonical sorted order a table actually stores, and then read back off the chain at finalized commitment and compared against the plan before anything routes through it. That read-back is the whole point: the vertical lane paid three validator runs for a client that compiled indexes against the list it built the plan from rather than against the table itself, and got a permuted account frame refused three layers away.',
    lookupTable: false,
  }),
  Object.freeze({
    id: 'dcltgmf1',
    title: 'DCLTGMF1 — Lock, Found, Realize, Claims, Open',
    effect: 'One rollback domain. The Market is created by the Found stage and Opened by the last, so this single transaction is the whole distance from the projected-Custody prestate to a live Market with a Claims aggregate, a founder Position, and a Hoard holding the collateral.',
    transactions: '1 (v0 on a 256 KiB heap frame, 135 + funding_count accounts, ~1.21M CU measured)',
    status: 'browser-frame-borrowed-coordinates',
    builder: 'lib/founding/genericMarketFounding.ts',
    reason: 'The outer instruction is assembled in the browser: eight bytes of data and the full stage-ordered frame, with the writability union and both invariants the reference client asserts on itself. What it cannot do alone is supply the coordinates — the four request records and the Custody PDAs come from the rungs above. The frame is the browser’s; the prestate is not.',
    lookupTable: true,
  }),
] as const);

export type FoundingLadderSummaryV1 = Readonly<{
  rungs: number;
  browserBuilders: number;
  browserFrames: number;
  toolingOnly: number;
  /** True only when every rung has a browser builder. It does not, today. */
  browserComplete: boolean;
}>;

export function summarizeFoundingLadderV1(ladder: ReadonlyArray<FoundingRungV1> = FOUNDING_LADDER_V1): FoundingLadderSummaryV1 {
  const count = (status: FoundingRungStatusV1) => ladder.filter((rung) => rung.status === status).length;
  const browserBuilders = count('browser-builder');
  return Object.freeze({
    rungs: ladder.length,
    browserBuilders,
    browserFrames: count('browser-frame-borrowed-coordinates'),
    toolingOnly: count('tooling-only'),
    browserComplete: browserBuilders === ladder.length,
  });
}

/**
 * The rungs a browser can drive against a chain that already holds the rest.
 *
 * This is the honest shape of browser founding today: on a validator whose
 * record graph and collateral Mint already exist, the two Core rungs are
 * reachable from a wallet, at a generation nothing has used.
 */
export function browserDrivableRungsV1(ladder: ReadonlyArray<FoundingRungV1> = FOUNDING_LADDER_V1): ReadonlyArray<FoundingRungV1> {
  return Object.freeze(ladder.filter((rung) => rung.status === 'browser-builder'));
}
