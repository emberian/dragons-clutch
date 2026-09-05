# Canonical Client Expectations V1

Adjudication of the five surfaces named by the canonical-generation mandate
(WAVE c2eb4f63): every client expectation is **DERIVED** from chain, or
**GENERATED** from the single author with a byte gate, or an irreducible
**ROOT** with release-aware selection. A hand-carried pin is a defect class,
not a style choice. This document is the disposition record plus implementing
briefs written to the zero-additional-research standard: an implementer needs
this file and the named anchors, nothing else. Line numbers are as of commit
54888c5e; every anchor is also quoted as grep-able text.

Provenance: PANEL-FIX's six commits (ebebbd4d, 5616aaae, 0cec8908, 2c07e1fe,
70fb9246, 54888c5e) convicted the first two instances of the class; this doc
generalizes the convictions into architecture.

The hierarchy, operationally:

- **DERIVE**: the value is read from a chain record the route already
  authenticates (a descriptor field, a manifest entry, a content digest). No
  constant exists client-side.
- **GENERATE**: the value is scraped from the single Rust/Lean author by a
  generator with a byte-for-byte `--check` gate, **plus** (new, S1) a
  route-binding gate proving the scraped file is the one the live route binds.
- **ROOT**: an irreducible identity (a release-set id, a PDA domain) selected
  release-awarely, refusing unknown vintages by naming the gap, never by
  accusing the record.

Twin topology: `apps/dclutch-web` is the author; `packages/dclutch-sdk`
absorbs byte-identical copies via
`node scripts/sync-from-web.mjs --copy --only <path>` (run from
`packages/dclutch-sdk`). Every edit below lands in the web tree first.

---

## Disposition table

| # | surface | verdict | size | status |
|---|---------|---------|------|--------|
| 1 | generator wrong-file pointer class | GENERATE is incomplete without a **route-binding gate**; add one to `generate-direct-inline-v3.mjs`, extend per audit | ~1 lane-day | **LANDED** for `abi:direct-v3`, both trees (0f0ac140) — all 11 route-bound authority constants gated, 5 red proofs; breadth sweep verdicts below |
| 2 | literal leftovers | sweep DONE: CONVICTED set → LITERALS lane (mechanical); SUSPECT no-twin-yet set → **new emitters**, briefed below | mechanical + ~1 lane-day of emitters | sweep table folded below; emitter briefs still open |
| 3 | six `DIRECT_INLINE_ORDINARY_*` program-id pins | **DERIVE**: drop all six equality conjuncts; content binding at fetch is the Rust semantics and already exists in the spine | ~½ lane-day | **LANDED** both trees (9fca18ae); live devnet re-confirmed both cohort-8 markets decode |
| 4 | dealer lifecycle PDA key (`dealerEquityChain.ts:362`) | **GENERATE from route authority**: V3 pin → `SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5`; derive impossible (V3-shape descriptor has no lifecycle-schema field) | 1-line + tests | **LANDED** both trees (b0d49e01); confirmation gap carried into the code comment, not closed |
| 5 | release-aware selection | land the **refusal-quality half** (vintage-naming refusals, zero new gates); ERA machine sized at 2–3 lane-days, blocked on choosing the release-set author | ~1 lane-day + sized remainder | refusal half **LANDED** both trees (028ba38d); ERA machine still sized-and-blocked |

**Landing notes (CANON-IMPL).**

- The twins had reconciled by the time this ran: `directHotChain.ts`,
  `dealerEquityChain.ts` and `directHotChain.test.ts` were byte-identical
  across trees, and `directTradeSpine.ts` differed only in one prose word
  (`this public page` vs `this public client`) — which is preserved. Every
  edit was applied per-file rather than sync-copied, so that difference
  survived.
- S4's follow-up audit resolved with nothing to do: after the fix, the only
  remaining non-test consumer of the V3 `LIFECYCLE_SCHEMA_RELEASE_ID` is
  `lib/explorer/derivations.ts:171`, which already labels it `'Lifecycle V3'` —
  the generation-naming treatment ebebbd4d asked for.
- S5's descriptor-error wrapping was written twice. The first attempt doctored
  record bytes in place and was caught by the spine's own content check
  (`finalizedRecordBody` hashes each record against its selected identity)
  before ever reaching the decoder. The fixtures now doctor artifacts BEFORE
  they are hashed. That failure is worth keeping in mind: in this client,
  mutating record bytes after the fixture is built tests the content gate, not
  the decoder.

---

## S1 — The wrong-file generator pointer class

**Conviction recap** (ebebbd4d): `abi:direct-v3`'s generator scraped
`crates/dclutch-effect-kernel/src/v3.rs SCHEMA_RELEASE_ID` and emitted it as
the (then-unversioned) effect schema while the live authenticator
(`crates/dclutch-direct-codec/src/artifacts_v4.rs`) binds
`v4.rs SCHEMA_RELEASE_ID_V4`. The `--check` byte gate stayed green: it proves
output freshness against whatever file the generator points at, never that
the pointed-at file is the route's author. The naming trap compounds it:
v3.rs's preimage reads `effect-program-v4-…`, the real V4 reads
`effect-program-v5-…`.

**Structural fix**: a generator's SOURCE binding must itself be checked
against the live route's binding. The gate reads the ROUTE file and verifies,
for each authority-selecting constant (schema release ids — anything whose
value picks a generation), that the route binds the same constant the
generator scrapes.

**Implementing brief** (author in `apps/dclutch-web/scripts/`, absorb to SDK):

1. New shared helper `route-binding.mjs`, pure functions over TEXT (no fs in
   the helper, so a vitest can feed doctored text):
   - `resolveUseBinding(routeText, aliasOrName)` → the `use`-path that binds
     the name (walk the use-tree; handle `X as Y` aliases and nested braces).
   - `requireRouteConjunct(routeText, anchorSnippet)` → throws unless the
     authentication conjunct exists (guards against the check silently dying
     when the route refactors).
   - `requireRouteBinding({routeText, name, expectedModulePath})` → throws
     unless `resolveUseBinding` resolves `name` to `expectedModulePath`.
2. Wire into `generate-direct-inline-v3.mjs` (both trees) immediately after
   the `sources` map, reading the route file once:
   - route: `crates/dclutch-direct-codec/src/artifacts_v4.rs`.
   - **effect**: the route's binding is, verbatim today,
     `v4::{ProgramV4 as EffectProgramV4, SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4}`
     inside `use dclutch_effect_kernel::{…}` (artifacts_v4.rs:26–30), used in
     the conjunct
     `descriptor.effect().schema().to_bytes() != EFFECT_SCHEMA_ID_V4`.
     Require: alias `EFFECT_SCHEMA_ID_V4` resolves to
     `dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4`, and the generator's
     `sources.effectV4` URL ends `crates/dclutch-effect-kernel/src/v4.rs`,
     and the emitted `EFFECT_SCHEMA_RELEASE_ID_V4` is scraped from that
     source under the constant name `SCHEMA_RELEASE_ID_V4`. If the route's
     use-statement ever moves to `v5::…`, the gate reds at generation AND at
     `--check` time.
   - **lifecycle**: the route's conjunct is
     `descriptor.lifecycle().schema().to_bytes() != SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5`
     (artifacts_v4.rs:205); the name is imported through
     `dclutch-capability-program-contract`'s v4 use-tree, which re-exports
     `lifecycle_v3::CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5`. Alias chains
     make path-resolution heavy, so use the **value-comparison fallback**:
     scrape the constant at the route-named source (follow the one re-export)
     AND at the generator's source
     (`crates/dclutch-account-profile-contract/src/lifecycle_v3.rs`,
     constant `CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5`) and require byte
     equality. A moved binding reds.
   - **general rule** for every authority-selecting constant in the emit list
     (the `SCHEMA_RELEASE_ID` family at generate-direct-inline-v3.mjs:350–377):
     either path-resolution or value-comparison against the route's named
     source. Layout scalars need no gate.
3. **Red proofs** (`scripts/routeBinding.test.ts`, both trees — vitest picks
   up `lib/` by default; check `vitest.config.ts` include globs and extend to
   `scripts/*.test.ts` if needed): (a) doctored route text whose effect
   import reads `v5::{… as EFFECT_SCHEMA_ID_V4}` → throws; (b) conjunct line
   deleted → throws; (c) the real current route text → passes. The generator
   imports the same functions, so the tested logic is the shipped logic.
4. **Breadth**: a generator-audit sweep (every `generate-*.mjs`, what each
   mirrors vs what the chain-facing route binds) was in flight at handoff.
   Append its verdict table to this doc; apply this same gate to any
   generator it convicts. Until then the gate above covers the one convicted
   generator plus its highest-risk sibling constants.

### Sweep results (CANON-IMPL, 2026-08-31)

**Headline: no second conviction.** No other generator emits a value the live
route disagrees with. `abi:direct-v3` was the only true wrong-file bind, and
it is fixed and gated. What the sweep did find is a different, milder shape —
generators whose emitted constants no route binds AT ALL — plus one whose
audit trail pointed at a non-binding file.

All 15 shared generators are byte-identical web↔SDK (`diff -q` clean). Four
are SDK-only (`aggregate-retirement-v1`, `direct-participant-v1`,
`relay-transport`, `resolution-certificate-v2`); `route-census` is web-only.

| generator | route checked | verdict |
|---|---|---|
| `generate-direct-inline-v3` | `dclutch-direct-codec/src/artifacts_v4.rs` | **CONVICTED → FIXED + GATED** (11 constants) |
| `generate-product-runtime-v2-admission` | `dclutch-product-runtime-v2-svm-reader/src/lib.rs` | **audit-trail defect → GATED** (see below) |
| `generate-core-found` | `core-sbf/src/found.rs` + svm-reader | CLEAN |
| `generate-dealer-equity-v3` | `trading-sbf/src/dealer/{v3_release,v4_scenario_release,v3_accelerator_accounts}.rs` | CLEAN (decoy noted) |
| `generate-generic-founding` | `trading-sbf/generic_market_founding_v1.rs:144`; `core-sbf/lib.rs:432` | CLEAN (best-structured of the set) |
| `generate-general-successor-v5` | `general-accelerator-sbf/src/lib.rs:61,334,337` | CLEAN (risk flagged below) |
| `generate-claims-custody-replay` | `claims-sbf/lib.rs:442`; `custody-sbf/lib.rs:290` | CLEAN |
| `generate-wallet-terminal-payout-v3` | `claims-sbf/lib.rs:411-413` | CLEAN |
| `generate-principal-capacity` | `core-sbf/found.rs:21,591,601` | CLEAN |
| `generate-protocol-infrastructure` | `core-sbf/infrastructure_v2.rs` | CLEAN |
| `generate-relay-transport` (SDK) | `resolution-proof-sbf/relay_transport_v1.rs:146` | CLEAN |
| `generate-resolution-certificate-v2` (SDK) | `claims-sbf/terminal_certificate_v3.rs:72` | CLEAN |
| `generate-aggregate-retirement-v1` (SDK) | `core-sbf/retire_v1.rs:556,838,1109` | CLEAN |
| `generate-direct-participant-v1` (SDK) | `claims-sbf/protocol_position_v2.rs`; `trading-sbf/direct_token_setup_v1.rs` | MIXED — one NO ROUTE |
| `generate-rational-terminal-hot-v3` | `claims-sbf/lib.rs:455-456` (child half only) | MIXED — hot half NO ROUTE |
| `generate-registered-direct` | none exists | **NO ROUTE** (whole module) |
| `generate-product-v2-payoff` | none exists | **NO ROUTE** |
| `generate-refusal-registry`, `generate-route-census`, `generate-successor-checkpoint`, `lean-emit` | n/a | no authority-selecting constants; nothing to gate |

**Fixed in this visit** — `generate-product-runtime-v2-admission`: the file a
reader would take for its route (`programs/dclutch-product-runtime-v2-sbf/src/lib.rs`)
binds NONE of the three schema IDs it emits; it delegates to
`authenticate_product_runtime_v2`. The real binder is
`crates/dclutch-product-runtime-v2-svm-reader/src/lib.rs:303,317,325`, where
each ID keys a Registry record against a digest. Values agree today, so this
was an audit-trail defect rather than a live bug — but "the pointer proves
nothing" is precisely the convicted class, so the gate now follows the real
binder, and the header says which file is the frame and which is the route.
Red-proved: pointing the gate at the frame file reds and names the true binder.

**Open findings, prioritized — none is a live wrong-value bug:**

1. `generate-registered-direct` — NO ROUTE, whole module. Its constants are
   `pub(crate)` and referenced only inside `dclutch-direct-codec` itself;
   nothing under `programs/` mentions `RegisteredControllerV1`/`RegisteredStateV1`.
   The crate exports instruction builders for instructions no linked program
   decodes. Same dead-ELF shape `generate-product-v2-payoff`'s own header
   describes deleting. **Disposition: delete or justify, do not gate.**
2. `generate-product-v2-payoff` — NO ROUTE, and the tree already says so:
   `ProductBasisV3Abi.lean:38` calls `DCLTPAY2` dormant. The live record is
   `DCLTPAY3`/`ProductBasisV3`, which this generator does not emit.
3. `generate-rational-terminal-hot-v3` — the hot half emits
   `RATIONAL_TERMINAL_HOT_*` which appears in zero files under `programs/`;
   the on-chain route binds `RepresentationRequestV2`. The child half is CLEAN.
4. `generate-direct-participant-v1:104` — emits a PDA seed **domain** scraped
   from a *private* const in an off-chain bootstrap tool
   (`tools/local-validator/bootstrap/…/user_position_admission.rs:85`). A seed
   domain picks an address; this one is authenticated by nothing on chain.
5. `generate-general-successor-v5` — three magics are hard-coded literals
   rather than scrapes (the Rust consts are private), though the generator does
   assert them in-file. Live risk: `general-adapter-contract/src/artifacts_v3.rs:399-410`
   already dispatches a V3 controller request, while this "V5" generator emits
   only the V2 shape. Not a wrong bind today (the accelerator is V2-only), but
   one route change from becoming one.
6. `generate-dealer-equity-v3` — correct, but a decoy
   `DEALER_CONFIG_SCHEMA_PREIMAGE_V2` lives in the same module the generator
   already reads for kind/root, while the routes bind the **V4** config. Worth
   gating on that basis alone.
7. `ADMISSION_RECEIPT_SCHEMA_PREIMAGE_V2`/`_ID_V2` have zero consumers
   anywhere — emitted but unbound.

**Gate-tooling gaps the sweep exposed, both closed here**: `followToDefinition`
originally followed only `use`/`pub use` trees, so two real chains reported
"neither defines nor re-exports" — which reads as a defect in the source
rather than a gap in the walker. It now also follows `include!`-stated
generated modules (`registry_v3.rs`'s `mod generated { include!(…) }`) and
same-crate module re-exports (`pub use principal_capacity_v1::{…}`, told from a
crate by the file's own `mod` declaration). Both have tests over the real
chains.

---

## S2 — The literal-leftover class

**Conviction recap** (70fb9246 + 54888c5e): `itemScalarStride` was demanded
as literal `0` in `validateDirectSignedRequestProfileV2` while
`DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3` sat emitted in
`crates/dclutch-direct-codec/src/generated_ordinary_v3.rs` beside two
siblings the same mirror already imported. The chain carried 2.

**Disposition rules**:

- CONVICTED (emitted twin exists): the generator emits the twin, the
  validator imports it beside its siblings, and the committed real-record
  vector gets the **flatten red proof** — substitute the old literal back and
  require the refusal to return (54888c5e's shape). Synthetic-fixture
  agreement is not evidence; the fixture must be built from the same emitted
  constants (70fb9246's closing move).
- BENIGN (structurally fixed by the codec primitive, or a foreign program's
  layout): leave, optionally comment.
- SUSPECT (twin plausible, not found): **author an emitter first** — never
  hand-add a TS twin, which would create the S1 class with an extra step.

**Sweep results** (full table:
`docs/evidence/LITERAL_SWEEP_CONVICTIONS_2026_08_31.md`). The CONVICTED set
is being fixed by the LITERALS Opus lane in parallel; headline items and the
one adjudication that was NOT mechanical:

- `directHotChain.ts:911` + `dealerEquityChain.ts:295` (both trees) pin the
  Core Market magic/version as `'DCLTCOR2'`/`2` beside imported
  `CORE_STATE_BYTES`, while the generated author says `DCLTCOR3`/`3`
  (`generated/coreFound.ts:35,:4`, bumped 2026-08-28 ff008fea).
  **Adjudicated mechanical, not vintage skew**, by two facts: PANEL-FIX's
  live spine test (0cec8908) passed `decodeMarketCoreStateV2` —
  DCLTCOR3/v3/368-relative — against devnet market21/22, so live markets ARE
  the current generation; and the DCLTCOR2 pins sit in the full-route
  functions that have never been live-driven (they need an operator route
  manifest; ember's browser refusal came from the spine's descriptor decode,
  upstream of :911). So the literals are standing refusals waiting for the
  first full-route live drive, and replacing them with the generated
  constants is safe. The decision procedure mattered: had live markets still
  been v2, the "fix" would have broken the panel — check the live vintage
  before replacing any generation-selecting literal.
- `fixtures/live-open-market.json` is itself a **v2-vintage capture**
  (DCLTCOR2, version 2, 352 bytes = a `SUPERSEDED_CORE_STATE_WIDTHS` entry):
  the repo already holds two vintages of real bytes — S5's ERA machine has
  its corpus started whether we build it or not.
- Stride-16 (`directInlineV3.ts:654`, `OPERATION_BYTES` twin), dealer 256
  (`dealerAccountProfileV3.ts:504`, `DEALER_LP_POSITION_BYTES_V3` twin),
  `rationalRetireReceiptV4.ts:76-82` shadow constants (one with the
  IDENTICAL name as its emitted twin), and `rationalOpenChainV4.ts:155-179`
  whole-literal decoders whose complete offset set is emitted in
  `generated/coreFound.ts` — all CONVICTED, all LITERALS-lane.
- **Twin divergence (process defect)**: the sweep found web and SDK copies of
  `directHotChain.ts`, `rationalRetireReceiptV4.ts`, and `localSuccessor.ts`
  DIFFERING between trees (web ahead ~15 lines in the first). The
  `twinIdentity.test.ts` gate did not red on this — successor should find
  out why (scope hole or stale ignore list) and close it; until then, run
  `node scripts/sync-from-web.mjs` (report mode) before ANY twin edit.

**SUSPECT class — emitter briefs (owned here, not by LITERALS)**:

1. **Resolution certificate V1** (`localSuccessor.ts:182`, whole decoder in
   literals: 312 bytes, version 1, reads at 16/240/248/…): the emitted V2
   module (`generated/resolutionCertificateV2.ts`, from
   `crates/dclutch-resolution-codec/src/generated_v2.rs:19-45`) exists
   **only in the SDK tree** — web's package.json has no
   `abi:resolution-certificate-v2` script, so web cannot import the twin at
   all. Brief: register the generator in `apps/dclutch-web/package.json`,
   emit the module into web's `lib/generated/`, and extend the emitter to
   also emit the **V1** generation (magic `DCSRCER1`; Lean author
   `formal/dclutch-semantics/DClutchSemantics/SourceResolutionAbi.lean:244-245`,
   `certificateSchema`/`certificateBytes`/`certificateMagic` scrapeable)
   under names that say the generation — the ebebbd4d treatment. V1 and V2
   offsets are numerically identical today, which is exactly why they must
   be named: nothing else will notice when they stop being. Reference
   implementation to converge on:
   `packages/dclutch-sdk/lib/resolutionCertificateV2.ts:58-86`. Also
   `:181`'s literal `1288` has a local twin (`ACTIVATION_CACHE_BYTES`,
   `releaseRegistry.ts:18`) — import it (LITERALS-grade, listed here only
   because it's the same visit).
2. **Capability seal** (`directHotChain.ts:843` region + PDA domain
   `'dclutch:capability-seal:v1'` hand-carried at `:127`):
   `crates/dclutch-capability-seal-contract` has **no generated file at
   all** — the whole contract is hand-authored constants. Brief: name the
   wire constants in the Rust contract (the `u16@12 == 6`, the `0x00ff`
   mask, the identity offsets 24/56/88/120), then scrape them in
   `generate-direct-inline-v3.mjs` like every other source — including the
   PDA domain via the existing `byteString` helper (the
   `CAPABILITY_ROOT_PDA_DOMAIN_V1` pattern at
   generate-direct-inline-v3.mjs:349). If the seal layout deserves a Lean
   author, that is a separate ruling; scraping named Rust constants is the
   minimum canonical rung.
3. **AccountProfile V2 header offsets** (`dealerAccountProfileV3.ts:415-422`
   reads at 12/14/16/18/20/22/24/26): no V2 header-offset constants are
   emitted anywhere, and the V1 names that share values carry different
   field semantics — emit V2-named offsets from the V2 author
   (`crates/dclutch-account-profile-contract/src/v2.rs`; note v2.rs:71's
   `AUTHENTICATED_ROUTE_ALIAS_ARTIFACT_PROFILE = 11` also needs a TS twin,
   used at `rationalOpenChainV4.ts:200`). Do NOT reuse V1 names for V2
   semantics.
4. **`DCRRGRP2` group record** (`rationalCapabilityChainV4.ts:73-85`): no
   twin in any generated module or `generated_*.rs`; locate the Rust author
   of the record (rational-representation family) and emit from it. Until
   the author is found this decoder is unverifiable against the canon —
   that absence is the finding.
5. **ProgramSet V1 selector header** (`dealerEquityChain.ts:164`:
   `u32@12 == 10`, `byte16 == 2`, `byte17 == 0` immediately after an
   imported `_HEADER_BYTES_V1` sibling): only HEADER/ENTRY byte-widths are
   emitted for V1 — emit the selector-shape constants from the V1 author.
6. Minor: `generalPlanV5.ts:345,409,428` use `u16(bytes,8) !== 2` beside
   `Abi.*_MAGIC_V2` where no `*_VERSION_V2` was emitted — add to that
   generator. `rationalOpenChainV4.ts:~231`'s bare account-index `5`
   exemption needs a named route-index constant from the claims frame
   author.

---

## S3 — The six `DIRECT_INLINE_ORDINARY_*` program-id pins

**Verdict: DERIVE-FROM-CHAIN. Drop all six equality conjuncts.**

**The Rust semantics (the authority)**: `authenticate_direct_artifacts_v4`
(`crates/dclutch-direct-codec/src/artifacts_v4.rs:166–260`) pins artifact
**schemas** to release-id constants and binds artifact **programs** only by
content: `require_content(descriptor.X().program().to_bytes(), artifacts.X)`
— the descriptor's named digest must hash the loaded bytes. No program-id
constant appears in any authentication conjunct. RequestProfile's schema is
also bound (V2-signed vs V1-unsigned dispatch in `decode_request_profile`,
artifacts_v4.rs:372–390), so every TS **schema** conjunct has a Rust twin;
only the six **program** pins have none.

**Why the tripwire option is rejected**: a generated tripwire needs an author
whose movement the gate can follow. The route has no program-id binding —
the six Rust constants (`ordinary_artifacts_v3.rs:62–76`,
`ordinary_bundle_v4.rs:67–81`) are the *publisher's encoder* output ids, not
route conjuncts. A client pinning them refuses every republication the chain
accepts: exactly the Talisman class (mirror disease with a fresher mirror).

**The client already has the Rust semantics at the fetch layer**: in
`inspectDirectInlineHotRouteV3` every one of the six artifacts is fetched via
`finalizedRecord(…, descriptor.X.schema, descriptor.X.program, …)`
(directHotChain.ts:970–995), which derives the Registry PDA from the
descriptor's own schema+digest, requires the manifest-supplied addresses to
be those PDAs, and requires `sha256(raw.data)` to equal the descriptor's
digest; the capability-seal list (directHotChain.ts:1050–1062) binds the same
six again. The read-only spine (`directTradeSpine.ts:194`) stops at the
descriptor by design — dropping the pins there removes a publisher mirror,
not an integrity check (integrity lives where bytes are loaded, in TS and on
chain alike).

**Exact edit** — `apps/dclutch-web/lib/directHotChain.ts:583–601` (then
absorb to `packages/dclutch-sdk/lib/directHotChain.ts`; the copies were
byte-identical at 54888c5e but the S2 sweep later found them diverged —
diff first, land on the reconciled base). Replace the seventeen-field `if`
with:

```ts
  // The schema conjuncts mirror the live Rust authenticator
  // (`authenticate_direct_artifacts_v4` in dclutch-direct-codec's
  // `artifacts_v4.rs`): each artifact's SCHEMA is release identity, required
  // exactly. The artifact PROGRAM fields are content digests, and Rust
  // deliberately does NOT pin them -- it requires each named digest to match
  // the loaded artifact bytes (`require_content`); this client does the same
  // where it fetches the records (`finalizedRecord` derives the Registry PDA
  // from the descriptor's own schema+digest and hashes the bytes). Pinning
  // the publisher's current program ids here refused every republication the
  // chain accepts -- the drift class that turned real readers away.
  //
  // A refusal names the one field that disagreed and both values, because a
  // seventeen-field conjunct that reports only its own name once cost a
  // manual chain diff to localize.
  const schemaConjuncts: ReadonlyArray<readonly [string, Uint8Array, Uint8Array]> = [
    ['successor kind', slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V4_KIND_OFFSET, 32), DIRECT_SUCCESSOR_KIND_ID_V3],
    ['config schema', slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V4_CONFIG_SCHEMA_OFFSET, 32), DirectAbi.DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1],
    ['request schema', slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V4_REQUEST_SCHEMA_OFFSET, 32), DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3],
    ['root schema', slice(bytes, DirectAbi.CAPABILITY_PROGRAM_V4_ROOT_SCHEMA_OFFSET, 32), DirectAbi.DIRECT_ROOT_SCHEMA_ID_V1],
    ['AccountProfile schema', accountProfile.schema, ACCOUNT_SCHEMA_RELEASE_ID],
    ['RequestProfile schema', requestProfile.schema, REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID],
    ['Lifecycle schema', lifecycle.schema, SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5],
    ['Strategy schema', strategy.schema, EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2],
    ['Transition schema', transition.schema, TRANSITION_SCHEMA_RELEASE_ID],
    ['Effect schema', effect.schema, EFFECT_SCHEMA_RELEASE_ID_V4],
  ];
  for (const [field, actual, release] of schemaConjuncts) {
    if (!same(actual, release)) {
      throw new Error(`selected CapabilityProgramV4 is not the schema-bound signed Direct InlineOrdinary bundle: its ${field} is ${hex(actual)} and this build decodes ${hex(release)}`);
    }
  }
  if (!same(derivationPolicy, lifecycle.program)) {
    throw new Error('selected CapabilityProgramV4 is not the schema-bound signed Direct InlineOrdinary bundle: its derivation policy is not its own Lifecycle program');
  }
```

(`hex` is already imported from `./bytes` at line 7. The message keeps the
`schema-bound` prefix so existing `/schema-bound/` matchers stay green. The
per-field naming is S5's refusal-quality rung landing in the same edit.)

Then: remove the six now-unused imports
(`DIRECT_INLINE_ORDINARY_{ACCOUNT_PROFILE_ID_V3,EFFECT_ID_V4,LIFECYCLE_ID_V5,REQUEST_PROFILE_ID_V3,STRATEGY_ID_V3,TRANSITION_ID_V3}`,
directHotChain.ts:25–30). The generated constants themselves STAY in
`lib/generated/directInlineV3.ts` — tests legitimately use them to build
fixtures (encoder side), and the dealer/registered encoders may too.

**Test flips** (`lib/directHotChain.test.ts`, both trees, the
`hostile-decodes the schema-bound V4 successor descriptor` case at ~360–385):

- `staleAccountProfile` (substituted account-profile **program** id,
  currently `.toThrow(/schema-bound/)`) **flips to acceptance**: assert it
  decodes and `decoded.accountProfile.program` equals the substituted
  identity — the fluidity proof that the Talisman refusal class is retired.
- `strategy` (substituted strategy **schema**) stays red; assert the message
  now names `Strategy schema` and both hex values.
- `parallelLifecycle` (derivationPolicy ≠ lifecycle.program) stays red with
  the derivation-policy message.
- The pin-equality expects at :368–370 remain true (fixture round-trip) —
  reframe the comment to say they assert the fixture, not the authenticator.

**Standing red proofs that must stay red**: `directDescriptorVector.test.ts`
(both trees) — cohort-8's real 600-byte record decodes; substituting the
superseded V3 effect schema refuses; the stride-flatten refusal returns.

**Gate**: `npm test` in `apps/dclutch-web` and `packages/dclutch-sdk`; the
live spine test (`DCLUTCH_LIVE_DEVNET=1`, directTradeSpine.live.test.ts)
still reaches both cohort-8 markets tradable.

---

## S4 — The dealer lifecycle PDA key

**Verdict: GENERATE from the route's emitted authority (V5). DERIVE is
impossible today** — and that impossibility is the root cause: the dealer
descriptor is the V3 `CapabilityProgram` shape
(`decodeDealerDescriptor`, dealerEquityChain.ts:189–221), which carries
schema+program **pairs** for requestProfile and strategy but only bare
content ids for accountProfile/lifecycle/effect. The neighbors that carry
their schema derive it from chain
(`descriptor.requestProfileSchema`, `descriptor.strategySchema` — both
already keyed that way at :358, :364); lifecycle couldn't, so a hand pin
appeared, and it pinned the wrong generation.

**The facts**:

- `dealerEquityChain.ts:362` (anchor text:
  `HotAbi.LIFECYCLE_SCHEMA_RELEASE_ID, descriptor.lifecycle, 'Dealer lifecycle'`)
  keys the Registry-record PDA derivation
  (`deriveFinalizedRecordAddressesV1(registry, schema, digest)` inside
  `finalizedRecord`) on lifecycle **V3**'s `SCHEMA_RELEASE_ID`
  (`crates/dclutch-account-profile-contract/src/lifecycle_v3.rs:29`).
- Qualified Rust binders of `lifecycle_v3::SCHEMA_RELEASE_ID` repo-wide:
  **zero** (grep-verified at 54888c5e).
- The dealer-family bundle author
  (`crates/dclutch-rational-lifecycle-hot-v3/src/selected_bundle_v6.rs:149`)
  registers `lifecycle: artifact(LIFECYCLE_SCHEMA_ID_V5, lifecycle_id…)`
  where `LIFECYCLE_SCHEMA_ID_V5 = CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5`,
  and validates with `StateLifecyclePolicyV5::decode_selected` (:204). A
  record published per this author lives at the **V5**-keyed PDA; the V3 key
  derives an address no publisher writes, so every real dealer route would
  refuse with `'Dealer lifecycle raw/staging addresses are not canonical
  Registry PDAs'`.
- The other five schema keys in `dealerEquityChain.ts` match
  selected_bundle_v6.rs:140–159's registrations exactly (accountProfile →
  `account_profile_contract::v2::SCHEMA_RELEASE_ID` = TS
  `ACCOUNT_SCHEMA_RELEASE_ID`; requestProfile + strategy → derived from the
  descriptor; transition → `transition_vm::v3::SCHEMA_RELEASE_ID` = TS
  `TRANSITION_SCHEMA_RELEASE_ID`; effect → `SCHEMA_RELEASE_ID_V4` = TS
  `EFFECT_SCHEMA_RELEASE_ID_V4`). **Lifecycle is the sole conviction.**

**Exact edit**: at :362 replace `HotAbi.LIFECYCLE_SCHEMA_RELEASE_ID` with
`HotAbi.SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5`, with a comment citing
selected_bundle_v6.rs:149 as the publishing authority and noting the V3 id
has zero route binders. Both trees (web author, SDK absorbs).

**Red proofs**:

1. Test: `SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5` and
   `LIFECYCLE_SCHEMA_RELEASE_ID` are unequal byte arrays — two independently
   emitted constants; reds if the generator ever collapses them (not
   vacuous: it compares two authors, not a value to itself).
2. Test: `deriveFinalizedRecordAddressesV1(registry, V5, digest)` ≠ the same
   under V3 for one fixed digest — proves the key is load-bearing.
3. S1's route-binding gate covers the authority side (route moves to V6 →
   generator gate reds).

**Confirmation gap, named (not guessed)**: no dealer market exists on chain,
so end-to-end confirmation is pending the first one. The confirming record
is **the dealer lifecycle Registry record found at
`PDA(registryProgram, SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5, descriptor.lifecycle)`**
— wire that assertion into a dealer live test
(`DCLUTCH_LIVE_DEVNET=1`, the dealerEquityChain twin of PANEL-FIX's
directHotArtifacts.live.test.ts) when a dealer market founds. If the record
is NOT at the V5 PDA, the divergence is the publisher's, and the fix moves
there — not back to a client pin.

**Follow-up in the same visit**: audit TS consumers of the V3
`LIFECYCLE_SCHEMA_RELEASE_ID`; if only the explorer names pre-cohort-8
records, rename the emission to say so (the effect-kernel treatment,
ebebbd4d: both generations emitted "under names that say which they are").

---

## S5 — Release-aware selection (the fluidity rung)

**Scope honestly split**: the refusal-quality half lands now; the full ERA
machine is sized and its blocker named.

**What exists already**: the chain's release identity reaches the client as
`market.identity.selectedReleaseSetId`
(`lib/marketCoreV2.ts`, `CORE_STATE_SELECTED_RELEASE_SET_OFFSET`), and the
house style for vintage-aware refusals exists in the same file
(`SUPERSEDED_CORE_STATE_WIDTHS` + `coreStateWidthGuardIsCoherent`: the
older-generation sentence, kept honest by a coherence test).

**Implementing brief (refusal-quality half)**:

1. New `lib/directDecodeVintage.ts` (web author, SDK absorbs):
   `DIRECT_DECODE_VINTAGE_V1` — a frozen object of **references** to the
   generated schema ids this build decodes (descriptor
   `CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID`, effect V4, lifecycle V5,
   account V2, request-profile V2, strategy V2, transition V3, all imported
   from `lib/generated/directInlineV3.ts`) — a view over the canon, **zero
   new values** — plus `describeDirectDecodeVintageV1()` returning one
   sentence with hex8 prefixes of each.
2. `directTradeSpine.ts` (~:191): the refusal
   `'the Direct entry selects a descriptor schema other than
   CapabilityProgramV4'` becomes self-describing:
   `this Market (release set ${market.identity.selectedReleaseSetId}) selects
   descriptor schema ${hex(selected.schema)}; this build decodes
   ${hex(CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID)} — if the Market's release
   is newer, this build predates it: ${describeDirectDecodeVintageV1()}`.
3. The spine wraps `decodeDirectDescriptorV4` errors (the S3 per-field
   messages) with the market's release-set id, so every vintage refusal
   names the chain's release identity and the exact disagreeing field with
   both values — replacing schema accusations for unknown vintages.
4. **The never-wrongly-refuse property is structural**: this half adds
   MESSAGES only — no new equality or membership gate anywhere — so a
   known-current release cannot acquire a new refusal path. Tests: cohort-8
   vector still decodes (existing); new tests assert the refusal text names
   field + both values + release-set id; a corrupt record (bad magic) still
   reads as corruption, never as vintage.

**Sized remainder — the ERA machine (est. 2–3 Opus lane-days), do NOT start
without resolving the blocker**: selection-by-id needs a client-side table of
known release sets (id → vintage name → decode-expectation bundle). The
blocker is choosing the **canonical in-repo author** for release-set ids —
candidates: the checked-release pipeline (`crates/dclutch-release-tool`)
emitting a per-cut manifest the generator mirrors, or the successor cache.
Hand-pinning today's id (cohort-8 `559f26e6…`) into TS would recreate the
defect class this document exists to end. Per-vintage decode bundles have
precedent (the explorer keeps effect V3 to name pre-cohort-8 records,
ebebbd4d); the machine generalizes that from one kept constant to a selected
bundle.

---

## Handoff protocol

- Edit web, absorb to SDK:
  `cd packages/dclutch-sdk && node scripts/sync-from-web.mjs --copy --only <path>`.
- Gates per landing: `npm test` in both trees; `npm run abi:direct-v3:verify`
  after any generator edit; every new gate red-proofed by doctoring its
  INPUT (route text, record bytes) — never by editing generated output.
- Recommended order: LITERALS lane's convicted set lands first (it edits the
  same `directHotChain.ts`/`dealerEquityChain.ts` regions' neighborhoods and
  reconciles the diverged twins), then S3 (code above), S4 (one line +
  tests), S1 gate, S5 refusal half, then the S2 emitter briefs.
- The generator-audit sweep (every `generate-*.mjs` vs what its route binds)
  was still in flight at handoff — append its verdict table to S1 and apply
  the route-binding gate to anything it convicts. The literal sweep's table
  is folded into S2 above.
