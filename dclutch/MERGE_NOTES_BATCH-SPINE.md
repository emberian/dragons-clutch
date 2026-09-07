# MERGE NOTES — `batch-spine`

Branch `build/batch-spine`, measured at `42d91e048` (this document is the commit above it), rebased onto `main` at `a43517586`
(clean rebase, no conflicts; the branch's three original commits kept, three
convergence commits on top).

## 1. READY

Green, with the whole cascade closed and one deployment gate named in §6.

| what | result |
| --- | --- |
| `cargo check -p dclutch-trading -p dclutch-operator --offline --tests` | green, 0 errors. Every warning is main's (`wallet_terminal_payout/wire.rs`, `programs/dclutch-trading-sbf/`), none in a file this branch touches. |
| `cargo test -p dclutch-trading --offline --lib` | **599 passed, 0 failed.** At the branch's old head this was 17 failures deep, not the one the handoff named. |
| `cargo test -p dclutch-direct-hot-program-test-support --offline --lib` | 21 passed, 0 failed. |
| `cargo test -p dclutch-operator --offline --lib direct_` | 42 passed, 0 failed. |
| `sh crates/dclutch-trading/check-successor-generated.sh` | exit 0 — five emitted files byte-identical to their emitters, `generated_intent_v3.rs` now among them. |
| `lake build DClutchSemantics.DirectIntentV3Codec DirectRfqV1 DirectOrdinaryV3` | exit 0, no warnings. The family's 101 theorems have now actually been elaborated. |
| SDK `vitest run` (whole suite) | **900 passed, 20 skipped, 0 failed** (94 files). |
| SDK `tsc --noEmit`, `eslint lib scripts index.ts` | both exit 0. |
| `tools/gate emission --verify` | `generated=92 guarded=92 unguarded=0` — the new file is guarded. The census DOC is stale by three lines; see §2. |
| `tools/gate reference --check` | `abi/directInlineV3.md` is fresh at this head. Three other pages are stale and that staleness is **main's**; see §2. |

## 2. SHARED FILES — for the merge lane

**S4 · `tools/gates/emission-coverage.md`** — also touched by `build/general-lifecycle`,
so left alone here. After both land, run `tools/gate emission --write`. The delta
this branch alone produces is exactly three lines, all additive, measured at
`5e1ba5ca0`:

- `:4` `**91 generated files from 85 emitters. 91 guarded (85 emitters), 0 unguarded (0 emitters).**` → `92 … 86 … 92 … 86 …`
- `:55` the `crates/dclutch-trading/check-successor-generated.sh` row gains `EmitDirectIntentV3Rust.lean` (alphabetically after `EmitDirectIntentV2Rust.lean`)
- `:154` a new row `| crates/dclutch-trading/src/generated_intent_v3.rs | EmitDirectIntentV3Rust.lean |` after the `generated_intent_v2.rs` row

The gate must still read `unguarded=0` afterwards. If it does not, a generated
file arrived with nothing checking it.

**`tools/cohort/steps.tsv:71`** — touched by four other build branches
(`failure-arm`, `series`, `claims-split-merge`, `founder-bond`), so left alone.
The `fill` row authors both tickets with `--limit-price 1000000`, so floor = cap
and the equal split is the identity: the row is safe, **by accident**. Add to its
comment: *both limits must stay equal — the execution price is now the floored
equal split of the two, and a row with asymmetric limits will refuse at
`SCALAR_DERIVED_PRICE_V3`.*

**`docs/reference/{README.md,routes.md,route-witnesses.md}`** — stale at this
head and stale on `main` for a reason that is not this branch: cohort-17's
witnesses (`111 with a witness` not 110, `undriven: 31` not 32, cohort-17 rows on
two claims routes). Regenerating them here would have put the identical diff on
twelve branches. Whoever lands first should run `tools/gate reference --converge`
once for main's own drift. `docs/reference/abi/directInlineV3.md` **is** committed
here, because it is this branch's own fact.

## 3. What this lane completed, and what it ruled

### 3.1 The seam the handoff did not find: five pinned identities, not one

The doc's §2.1 said `cargo test -p dclutch-trading` was red at one line. It was
red at **seventeen tests**, because the three new scalar slots moved the common
bank 68 → 71, and five hand-pinned content identities are functions of that width
or of the program bytes. All five moved here, each recomputed from the emitters
and each proved by the test that pins it:

| identity | file |
| --- | --- |
| `DIRECT_INLINE_ORDINARY_TRANSITION_ID_V3` | `crates/dclutch-trading/src/ordinary_artifacts_v3.rs:67` |
| `DIRECT_INLINE_ORDINARY_STRATEGY_ID_V3` | `…/ordinary_artifacts_v3.rs:72` |
| `DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_ID_V3` | `…/ordinary_artifacts_v3.rs:62` |
| `DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_ID_V3` | `…/ordinary_bundle_v4.rs:65` |
| `DIRECT_INLINE_ORDINARY_EFFECT_ID_V4` | `…/ordinary_bundle_v4.rs:75` |
| `DIRECT_HOT_FIXTURE_DESCRIPTOR_ID_V5` | `programs/dclutch-trading-sbf/program-test/direct-hot/src/lib.rs:49` |
| `DIRECT_HOT_FIXTURE_PROGRAM_SET_ID_V5` | `…/direct-hot/src/lib.rs:54` |

`ordinary_bundle_v4.rs:702-720` already carries a long comment saying this
cascade has happened twice and that no emitter authors these digests. It has now
happened a third time, and for a reason the comment does not name — the *scalar
bank*, not `dclutch_market::STATE_BYTES`. Its message names two of the seven
downstream pins; it should name all of them. Not changed here, because the
comment is a shared reader and the list is longer than one lane's edit.

Three hand-written `68` literals were also replaced by
`DIRECT_ORDINARY_COMMON_SCALARS_V3` (`ordinary_account_artifacts_v3.rs:1197`,
`ordinary_effect_artifacts_v3.rs:879`, `direct-hot/src/lib.rs:487`) — the number
had four authors and now has one.

### 3.2 Seams applied

| seam | state |
| --- | --- |
| S1 | **applied, and not as the doc described it.** The `1_784` literal was a *second author* of a width that `the_emitted_program_carries_its_own_derived_width_and_geometry` already derives from header + instruction count. It is gone, replaced by the price rule read off the admitted frame. The enclosing test is renamed `exact_program_admits_the_derived_price_ioc_and_routes_the_seller_leg_alone`: "price improved" is the concept the equal split abolishes. The fixture needed no re-deriving — 40/60 at 50 is already the split. |
| S2 | **applied.** `npm run abi:direct-v3` regenerated `packages/dclutch-sdk/lib/generated/directInlineV3.ts`; the generator independently reproduced all six moved digests from the Rust sources. `--check` exits 0. |
| S3 | **applied**, `abi/directInlineV3.md` only (§2). |
| S4 | **handed on** (§2). |
| S5a | **applied** at `crates/dclutch-operator/src/direct_inline_v3.rs:1739-1752`, mirroring the chain's conjunct order exactly: the two interval checks and the `≤ price_scale` bound stay, `execution_price != derived_price` is added, and the sum is `checked_add` so an overflow refuses the way `checkedAddInto` does. |
| S5b | **applied** at `packages/dclutch-sdk/lib/directInlineV3.ts:743-752`, same shape, with its own message. |
| S6 | **applied and RUN.** The parallel stanza pins `COMPACT_INTENT_BYTES_V3 = 172` and `COMPACT_INTENT_COUNTERPARTY_OFFSET_V3 = 140` before the byte compare, so a schema edit that dropped the counterparty is accused rather than reported as an anonymous byte difference. `tools/gates/emission.py` picked the guard up with no list to edit, as the doc predicted. |
| S7 | **applied**, in the exact form the doc gave. |
| S8 | **applied.** `crates/dclutch-trading/src/intent_v3.rs`, 404 lines, with `bearer`, `admits` and the `terms` projection to V2, plus four tests. |
| S9 | **applied**, and the root file's "every file under `DClutchSemantics/`, alphabetically" claim is now verified against the directory listing, not asserted. |

### 3.3 Ruled provisionally

**R-A — cohort-8's published RequestProfile vector is a SUPERSEDED release's
record, not a stale pin.** `packages/dclutch-sdk/fixtures/direct-descriptor-v4.devnet.json`
holds the record devnet cohort-8 actually published, and
`directDescriptorVector.test.ts` asserted the browser accepts it. It cannot: the
record states a common scalar width of 68 and this client reads 71. Under
decision 0012 an upgrade is a re-found, so this is the correct outcome, not a
defect. The test now asserts the strongest available statement — the record is
refused on **that one field** and accepted on **every other** (the width is
patched in a copy and the validator passes) — and the fixture carries an `OWED`
naming what replaces it: a vector read at finalized commitment from the first
cohort that carries the derived price. Reversal cost: the assertion, once such a
cohort exists.

**R-B — the common-scalar width gets its own refusal.**
`validateDirectSignedRequestProfileV2` (`packages/dclutch-sdk/lib/directHotChain.ts:761`)
folded eight conjuncts into one sentence, and the width is the only one of the
eight that a *release* moves rather than a corruption; folded in, a superseded
record read as a corrupt one. Split out with the observed and expected widths in
the message. This is decision 0007's "a refusal names its conjunct" applied to a
client-side accusation. Reversal cost: one `if`.

**R-C — the derived price can be one the scale cannot represent, and then the
pair simply cannot trade.** Under the interval rule a matcher could step off an
awkward midpoint to a representable price; it no longer can, and
`fill × derivedPrice % priceScale != 0` refuses on chain (`mulDivExact`) and in
both previews. Asserted, not left in prose, at
`packages/dclutch-sdk/lib/directInlineV3.test.ts` (limits 400,001 / 600,002).
This is a real narrowing of what can trade and it belongs in whatever record
carries ruling 1.

**Not ruled, deliberately**: the `execution_price` parameter of
`build_direct_inline_hot_v4` / `compileDirectInlineTransactionV3` is now
redundant with the two limits — it can only ever hold one value. Deleting it is
correct under one-author-per-fact and it is a public signature change across the
operator, the SDK, the local validator and the browser. Left for a lane that can
sweep all four.

## 4. Tests added, and what each proves

| test | proves |
| --- | --- |
| `intent_v3::tests::the_lean_owned_named_and_bearer_tickets_round_trip` | the hand-written Rust codec and the Lean emitter agree byte for byte on both example tickets and the signed preimage. The three `#[cfg(test)]` fixtures the doc found orphaned are now the assertions. |
| `intent_v3::tests::the_counterparty_is_the_only_field_the_terms_projection_forgets` | `terms()` is a projection: the named and bearer tickets encode to different V3 bytes and identical V2 terms. The Rust mirror of `terms_agree` / `tickets_differ`. |
| `intent_v3::tests::a_named_ticket_admits_its_taker_alone_and_a_bearer_ticket_admits_anyone` | `counterpartyAdmits`, all three branches. |
| `intent_v3::tests::a_v2_ticket_a_v2_domain_and_a_dirty_reserved_span_all_refuse` | five named refusals: a V2 ticket by length, a V2 ticket widened to 172 by magic, version 2 by version, both reserved spans, and a V2 signature domain. Each `assert_eq!` on the exact `Error` variant, none an `is_err()`. |
| `ordinary_v3::tests::only_the_equal_split_is_admitted_and_the_split_rounds_down` | the rule on the **executed program**: the fixture at the split admits (the positive control), the two interval endpoints the old rule accepted now refuse with `Error::CheckFailed` and leave the output bank untouched, and a floor of 40 against a cap of 61 derives 50. The off-split prices are chosen to keep gross an exact division, so the refusal cannot be the neighbouring `mulDivExact`. |
| `direct_inline_v3::tests::the_preview_admits_only_the_equal_split_the_chain_derives` | the same rule in the operator, same positive-control shape. `preview_economics` had **no test at all** before this. |
| `directInlineV3.test.ts` "previews one named collateral rounding boundary exactly" (extended) | the rule in the browser: three off-split prices refuse by name, an odd sum derives the lower price, and an unrepresentable split refuses at the exact division (R-C). |
| `directDescriptorVector.test.ts` (rewritten, +1 test) | R-A: the published record is refused on one named field and accepted on every other; the stride red-proof now runs on the width-repaired copy, which proves the two conjuncts are separately accused. |

## 5. What a reviewer should distrust in `BUILD_BATCH-SPINE.md`

I checked its load-bearing claims. Most held; these did not.

1. **"§6 Does it parse: `cargo test -p dclutch-trading` is red at `ordinary_v3.rs:528`"** — true and badly incomplete. It was red in **17 tests across five modules**, and sixteen of them were the pinned-identity cascade of §3.1, which the doc does not mention anywhere. A lane that fixed only S1 would have found the next sixteen.
2. **"S1: its fixture prices need re-deriving, not just the width"** — false. The fixture is seller 40 / buyer 60 at 50, which *is* the equal split; nothing needed re-deriving. What needed changing was the test's *name*.
3. **"§2.5 `tools/gauntlet/CU_BUDGETS.json`: no row is mechanically red"** — not re-checked here (no SBF build), and the program grew 73 → 77 instructions. Treat as unverified.
4. **"§2.5 `docs/reference/routes.md`: no hand row"** — true, but the doc does not say that `docs/reference` is stale on main for an unrelated reason, which a lane running the gate will hit first and may mistake for its own.
5. **§2.4's name trap (`SignedDirectIntentV3` carries a `CompactIntentV2`)** — verified true, and it stays true: `intent_v3.rs` adds `CompactIntentV3` with no producer on the envelope. Nothing signs or sends a V3 ticket yet, by design.

Everything else in §1, §2.2, §2.3 and §3 checked out against the tree.

## 6. What is still absent, and the one thing that gates a deploy

**Deliberately not built** (the doc's §2.4, and the brief's instruction not to
build the joint-clearing family's neighbour twice):

- the SBF RFQ route in `programs/dclutch-trading-sbf/`, and the ten refusal codes
  `Rfq.crossed`'s nine conjuncts plus `counterpartyAdmits` are owed
  (`docs/reference/refusals.md` has no row and no `TradingSbfError` variant);
- a TS `CompactIntentV3` encoder and its
  `packages/dclutch-sdk/scripts/generate-direct-inline-v3.mjs` rows (`:69` sources
  map, `:411` emit list) — a wire with no Rust author should not get four more;
- `tools/gates/wire-vector-pins.tsv`, which pins two-sided Rust↔TS fixtures and
  has nothing to pin until that encoder exists;
- **`ClearingPriceV1`**: `DirectRfqV1.lean:37,141` names it in prose as the
  persistence surface for the derived price, and it is defined on
  `build/joint-clearing` (`DClutchSemantics/ClearingPriceV1Abi.lean`). Whichever
  of the two lands second inherits the obligation to make that reference real.
  `Rfq.priceVector` is exactly the vector that fact would record for a batch of
  two, so it should become one author, not a second.

**The deploy gate, which nothing in the handoff names.** This branch changes the
emitted ordinary transition and therefore every artifact identity a live Direct
market pins. Cohort-8's published RequestProfile no longer validates (R-A) and
cohort-17's will not either. Under decision 0012 an upgrade is a re-found, so
this is not a migration — it is a **re-release**: landing this branch obliges the
next cohort to redeploy the Direct set and re-found, and no market founded before
it can execute an ordinary fill under the new client. That should be stated in
the ledger entry that lands it, and it is the reason to land `batch-spine` early
in a cohort cycle rather than late.

**Two gates this lane could not run**, both needing `cargo-build-sbf`, which the
convergence brief forbids:

- `tools/gate sbfcontracts` — `intent_v3.rs` is declared `pub mod intent_v3;`
  unconditionally, like `intent_v2`, and uses only the same `no_std` primitives
  its V2 peer does, so it should compile for `target_os = "solana"`. Should is
  not measured; run it.
- `tools/gate programs` and `tools/gate suites` — the Direct hot link's ELF and
  its program-tests. The pinned identities in §3.1 are what those tests compare
  against, and all of them moved.

**Frames**: `tools/gates/frames-baseline.json` is untouched. The ordinary
transition grew 96 bytes and the Direct hot link's frames were measured against
the shorter program; `tools/gate frames --at <commit> --capture` is owed by
whoever runs the next SBF build.
