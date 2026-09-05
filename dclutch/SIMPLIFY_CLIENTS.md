# SIMPLIFY-CLIENTS

Branch `simplify/clients`, cut from `main` at `330bbfaba`. Seven commits. Every
number below was measured in this worktree at its HEAD; the `wc -l` counts are
over `.ts/.tsx/.mjs/.js` excluding `node_modules`.

## Before -> after

| | before | after |
| --- | --- | --- |
| `apps/dclutch-web` | 547 files / 107,883 lines | 348 files / 61,750 lines |
| `packages/dclutch-sdk` | 257 files / 65,860 lines | 255 files / 64,931 lines |
| `packages/dclutch-cli` | 37 files / 9,935 lines | 37 files / 9,755 lines |
| paths present in both trees | 202 (167 byte-identical, 35 drifted) | 3, all deliberate (two per-tree gates, one per-tree baseline) |
| twin machinery | `tools/twins/classification.mjs` (7 classes, 38 exceptions), `sync-from-web.mjs`, `twinIdentity.test.ts`, `vendoredPairIdentity.test.ts` | none |
| generator scripts | 40 in the web (19 of them copies of SDK scripts) + 34 in the SDK | 16 in the web (7 wasm, capability surface, SBF runtime, coverage/OG/simulator tooling) + 31 in the SDK, one census script in `tools/abi-coverage/` |
| `abi:*:verify` gates | 28 web + 25 SDK (twenty of them checking the same module twice) | 10 web + 32 SDK, each module checked once by the tree that owns it |
| SDK hand-mirror census | 29 magic rows, 25 seed-domain rows, 433 literal offsets in 20 files | 5 magic rows, 5 domain rows, 433 offsets in 20 files |
| web hand-mirror census | 26 magic rows, 12 domain rows, 17 offset files | 0 magic rows, 1 domain row (a journey schema tag), 0 offset files |
| explorer coverage | 59/68 records, 54/65 instructions rendered (census read the web's stale copy) | 60/68 records, 56/67 instructions, 0 unrendered (census reads the SDK's generated tree) |
| submission transports | 3 (web `rpc.ts` fork, SDK `walletHandoff` on a client the SDK could not construct, CLI `rpcSubmission.ts`) | 1 (`SolanaRpcClient.sendRawTransaction`), with `walletHandoff` and the CLI as its two journaled callers |

## The ruling this branch makes, and its defence

The SDK's export map refused `sendRawTransaction` from its public RPC client
and routed `walletHandoff` and `directInlineV3` to read-only facades
(commits `39cb2628e`, `4968dc067`, `7c2de1e50` of 2026-08-28: one-line titles,
no decision record, no rationale in the tree). Nothing outside this repository
consumes the package (`private: true`). The only consumer of that refusal was
the web app, which answered it with an 826-line private fork of `rpc.ts`, two
shims reaching past the export map by relative path, and the twin arrangement
the classification table existed to describe. The CLI answered it with a third
transport. This branch **reverses the refusal**: submission is one bounded
primitive (one packet, preflight on, genesis rechecked, no loop) that every
caller owns a journal around, the dispatcher stays an ECMAScript private slot,
and filesystem-style deep imports stay refused (`publicSurface.test.ts` keeps
exactly those two properties). Reversible on request; the facades are two
20-line files to restore, but the fork they caused would come back with them.

## Files deleted, with the control

- **167 byte-identical web copies of SDK modules, 18 fixtures, 19 generator
  scripts** (`dcaba4770`). Control: 157 importers rewritten to
  `@dclutch/sdk/<module>`; a resolver sweep found zero unresolvable specifiers;
  `tsc` clean in both trees; 347/347 across the 30 web suites that exercise the
  rewritten imports, standings, explorer and fixtures.
- **17 re-export shims, `slotClock` shim (its chain-reading half moved into the
  SDK beside the arithmetic it re-exported), the twin machinery.** Control: no
  path is shared by the two trees except the three named above.
- **The retired Direct trio** (`directTransaction.ts`, `directCodec.ts`,
  `registeredDirect.ts`, generated `registeredDirect.ts`), both trees
  (`997d5dd0e`). Control: imported by nothing but their own tests since
  2026-08-27 (grep of every non-Rust consumer); the explorer's five Registered
  Direct arms were their last reader and decode records no program or crate
  writes (`DCLTRGI1` and the registered create/fill/terminal/retire requests are
  declared only by `DirectLifecycleAbi.lean`; the controller-proof program was
  banished on 2026-08-27). `EmitRegisteredDirectTs.lean` now prints a module
  nothing consumes -- the Lean maker's to retire.
- **The `/local` checkpoint surface**: page, workspace, nav entries, 
  `localSuccessor.ts`, its 911-line fixture and generator, both trees. Control:
  the fixture's provenance is hardcoded to two commits of the old successor
  era, its records are a superseded generation (`DCLTSRS1`), and the generator
  refuses any RPC but a loopback validator of that era -- it can never match a
  running successor again. The CLI's `successorBinary` (which locates the
  bootstrap binary, not the checkpoint) stays.
- **`rationalOpenOperationV1.ts`** (413 lines) and its test. Control: its five
  exports are reached by no file; `RationalOpenPanel` calls the chain modules
  directly.
- **The web's `rpc.ts` and `rpc.test.ts`**: merged into the SDK's as a union
  (SDK's private `#request` slot and `programAccountsOfExactWidth`; web's
  `blockTime`, `signatureStatus`, inner-instruction and compute-unit
  observations, `sendRawTransaction`). Control: 94/94 across the eight SDK
  suites touched, including the three submission cases carried over.
- **The CLI's own JSON-RPC transport** (139 -> 55 lines). Control: CLI `tsc`
  clean, transport/redeem/payout/mutation suites green.
- **Stale explorer exemption** for `principalCapacityV1.ts` (the module emits
  its magic now).

## Files moved

- Web -> SDK: `activity.ts` (+test), one `lookupTable.ts` comment,
  `rpcConcurrency.test.ts`, `rpcHttpFailure.test.ts` (tests of SDK modules that
  lived only in the web); `quantity`, `marketFiltering`, `marketDenomination`,
  `openerTerms`, `rpcSubscribe` (+tests) -- the pure protocol modules a
  dependency census (transitive imports against react/next/window/document/
  storage/`NEXT_PUBLIC`/components) found among the web's remaining `lib/`.
- The four "BACKLOG" files where the SDK was already ahead (`directTicket`,
  `directTradeSpine`, two tests) keep the SDK's copy.
- `apps/dclutch-web/lib/ticketBoard.ts` -> `deploymentTicketBoard.ts`: the app's
  `NEXT_PUBLIC_*` board configuration, renamed so no file shares a name with a
  different file in the other tree.
- `abi-coverage.mjs` -> `tools/abi-coverage/abi-coverage.mjs`, run from either
  package; each tree keeps its own baseline.

## Generators, before -> after

- Before: 40 scripts in the web, 34 in the SDK, nineteen of them byte copies,
  two source-* wasm generators writing their facts module into both trees, and
  no generator for the constants the client spelled by hand.
- After: the SDK owns every Rust/Lean-derived module (31 scripts, 32 verifies;
  seven of the Lean-backed ones go through the one `lean-emit.mjs` runner); the
  web keeps its seven wasm generators (the operators maker is folding the eight
  wasm crates into one `dclutch-wasm`, built not committed -- their wrapper
  module names are unchanged here), the capability surface, the SBF runtime
  table, and its coverage/OG/simulator tooling.
- New: `scripts/generate-protocol-constants.mjs` -- one table of
  `[export, Rust file, Rust constant, form]` rows, read straight out of the
  crate that owns each constant, emitting `lib/generated/protocolConstantsV1.ts`
  under `abi:protocol-constants:verify`. 23 constants; fifteen SDK files now
  import them. This is the "one script that reads the emitted Rust and writes a
  module from a table" the brief asked for, scoped to the two literal classes
  it can take wholesale.
- Not unified: the ~21 bespoke Rust-scrape generators. The Lean maker is making
  one emitter produce both Rust and TS for the records that have two; folding
  the scrapers into a table now would be a second framework beside that one.
  Generated TS module names are unchanged so that work lands on the same paths.

## The instruments learned to see the SDK

- `generate-capability-surface.mjs` surveys every module a route reaches in
  either tree, names SDK modules by specifier (`@dclutch/sdk/coreFound`) and
  pairs SDK generated modules with the SDK's verifiers (30 -> 36 authorities
  credited). The act anchors in `capabilityModel.ts` name their real homes.
- `explorer-coverage.mjs` reads both generated trees; the route census comes
  from the SDK. Ten records and two instructions a web-only survey had never
  seen became visible: the two Trading founding routes and the rational
  lifecycle request are rendered (from their handlers' docs and the generated
  offsets); eight records are exempted by name with owner and fix.
- `abi-coverage` baselines, `capabilityAccess` rehearsal pins,
  `abiCoverage.test.ts` floor/witness, genref's coupled-emitter list and
  converge trees, the reference reader, the public-cut exporter, the CI web
  tier's liveness loop and abi note, `emission-guard/COVERAGE.md` (94/94
  guarded): all follow the flip in the same commits.

## Deliberately left, and why

- **The explorer stays in the web** (`lib/explorer/*`, ~5k lines): the map's
  section 1.3 keeps it there; it is a rendering model over the SDK's decoders.
- **The wasm-coupled web modules** (source readiness/provider/terminal/close,
  user-position admission, wallet-terminal input/payout, partition quality,
  product payoff evaluation; ~4k lines) stay until the operators maker lands the
  one `dclutch-wasm`; moving them now would move them twice.
- **The app's journals, evidence surface and config** (`clientOperationJournal`,
  `directTradeJournal`, `redeemOperationJournal`, `capabilitySurface`,
  `coldClientJourney`, `deploymentStore`, `flags`, `rpcDefault`,
  `deploymentTicketBoard`, `walletStandard`) are the app's own state.
- **433 literal byte offsets in 20 SDK files** (`releaseRegistry.ts` 51,
  `rationalRetireReceiptV4.ts` 57, `dealerEquityChain.ts` 51,
  `directHotChain.ts` 45, `coreFound.ts` 40, ...). Their Rust owners declare
  layouts in encode/decode bodies, not as named constants, and no Lean schema
  exists for the Registry family; converting them is Lean-emitter work (one
  emitter per record), not a scrape.
- **Five magics and five domains** the census still counts: `DCLTSMV3`,
  `DCLRNTL2`, `DCLTDMR1`, `DCLTPAY3` are declared by no Rust or Lean source at
  all -- the client is their only author, which is the hazard the ratchet names;
  `DCLTCPS1` appears only inline in Trading; the four domains are Lean schema
  preimages (`RequestProfileAbi`, `ProductBasisV3Abi`,
  `ProductRepresentationExposureV3Abi`) whose honest home is a Lean emission.
- **The 41 skipped tests** are `.live.test.ts` probes gated on
  `DCLUTCH_LIVE_DEVNET`; the CI web tier runs the liveness one. Not dead.
- **The seven wasm verifies** build Rust and were not run in this worktree.
- **`docs/reference/abi/*`** will change on the next `--converge` (SDK-only
  modules gain pages, the web's wasm wrappers lose them); the convergence owner
  runs it, lanes do not.
- **Design notes** that instruct `sync-from-web.mjs` or name `localSuccessor.ts`
  (`CANONICAL_CLIENT_EXPECTATIONS_V1.md`, `DEVNET_DEMO_DEPLOY.md`,
  `OPERATOR_FORMS_V1.md`) are dated history; the docs maker's column.

## Found on the way, not fixed here

- `DCLTARF1` is declared by two different records (`ARTIFACT_RELEASE_MAGIC_V1`
  in registry-contract, `AGGREGATE_RETIREMENT_FINISH_MAGIC_V1` in
  market-core-codec) and `DCLTRIX1` by three crates -- the magic-names gate's
  to refuse.
- The web's committed `capabilitySurfaceV1.ts` was already stale against the
  crate list at `main` (`dclutch-market::protocol_parameters` missing); it is
  regenerated here.
- `explorer-coverage.mjs` and the capability surface had both gone blind to
  anything under the SDK's generated tree; both are fixed here, and the
  exemptions they surfaced are named with owners.

## Controls run

`tsc` clean and lint 0 errors in web, SDK and CLI at HEAD. Suites: SDK 94 + 9
+ 56 + 156, web 347 + 152 + 82, CLI 17 + 22 -- every suite that imports a
touched module, never the whole suite. `abi:capability-surface:verify`,
`abi:sbf-runtime:verify`, `abi:protocol-constants:verify`, both `abi:coverage`
runs, `emission_guard.py --write` (94/94) green.
