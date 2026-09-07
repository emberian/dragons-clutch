# MERGE NOTES — product-shapes

Lane `CONVERGE-PRODUCT-SHAPES`, branch `build/product-shapes`, rebased onto `main`
at `a43517586` (clean rebase, no conflicts: `main`'s ten changed files since the
branch point are disjoint from this branch's forty). Commits unsigned — 1Password
refused the rebase's first commit object, which per `~/.claude/CLAUDE.md` marks
work done while ember was away.

## 1. READY

**READY.**

Every crate this family touches is green under `cargo check --offline --all-targets`:
`dclutch-source`, `dclutch-market`, `dclutch-product-runtime-v2-operator`,
`dclutch-resolution-core-v3-operator`, `dclutch-core-sbf`,
`dclutch-resolution-proof-sbf`, `dclutch-local-successor-bootstrap`,
`dclutch-relayed-vertical-campaign`. `dclutch-source` and `dclutch-core-sbf` also emit **zero
warnings of their own**, which they did not when this lane opened (fifteen
dead-constant warnings in the first, one unused import in `parents_v1.rs`).

`cargo check --offline --workspace --all-targets` has exactly one failure, in two
crates this branch never touches and for a reason that has nothing to do with it:

```
error[E0433]: cannot find `tests` in `wire`
   --> crates/dclutch-wallet-terminal-input-wasm/src/lib.rs:716:69
note: found an item that was configured out
   --> crates/dclutch-operator/src/wallet_terminal_payout/wire.rs:1372:9
1371 | #[cfg(any(test, feature = "test-fixtures"))]
```

Five instances, one per call site. `dclutch-wallet-terminal-input-wasm` reads a
fixture module gated behind `dclutch-operator`'s `test-fixtures` feature, which a
bare `--workspace --all-targets` does not enable. Neither crate is in
`git diff main...HEAD --name-only`, so it is `main`'s and predates this lane.

One warning remains inside a crate this family owns and it is `main`'s:
`programs/dclutch-resolution-proof-sbf/src/provider_transport_v3.rs:34` imports
`SourceResolutionPhaseV1` and does not use it. That file is byte-identical to
`main` in `git diff main...HEAD`; left alone.

**Tests run, all green** (filtered, never a bare `-p` suite):

```
dclutch-source            --lib  relay::frame relay::decode parent_reference_v1   45 passed
dclutch-core-sbf          --lib  frame::                                           3 passed
dclutch-product-runtime-v2-operator --lib flagship_v1 child_v1                     7 passed
dclutch-product-runtime-v2-operator --test flagship_child                          3 passed
dclutch-product-runtime-v2-operator --test found                                  14 passed
dclutch-resolution-core-v3-operator --lib derived_settle                           2 passed
dclutch-local-successor-bootstrap        relayed::                                 7 passed
```

What is NOT proved: no `lake` run happened in this worktree (§5 says what that
costs), no SBF build, no program test, no devnet.

## 2. SHARED FILES — for the merge lane

Each of these is a file another `build/*` branch also touches, so this lane wrote
none of them. Each is one line.

| file | the change | why this lane did not make it |
| --- | --- | --- |
| `tools/gates/emission-coverage.md` | run `tools/gate emission --write`. **Measured, not predicted** — this lane ran it, read the diff and reverted it, and it is exactly three lines: `:4` becomes `**92 generated files from 86 emitters. 92 guarded (86 emitters), 0 unguarded (0 emitters).**`; `EmitParentReferenceV1Rust.lean` joins the front of the `crates/dclutch-source/check-generated.sh` guard row; and one generated-file row is added, `` | `crates/dclutch-source/src/generated_parent_reference_v1.rs` | `EmitParentReferenceV1Rust.lean` | ``. The gate's own summary already reads `generated=92 guarded=92 unguarded=0`, so **no unguarded file arrived** — it is `status=STOP` purely because the doc is stale. | `build/general-lifecycle` also edits it. |
| `tools/gauntlet/blocked.json` | one entry: `{"route": "resolution/derived_transport_v1::process_derived_settle_v1", "class": "status-report", "reason": "No campaign drives a derived settle yet and nothing structural is in the way: a child market must first be founded through the 49-account Found frame, which no runbook step does.", "owner": "CONVERGE-PRODUCT-SHAPES"}`. Without it the new route lands in the register's "NO stated reason at all" row, and `tools/genref/generate.mjs` refuses the file outright if the `class` is missing or unknown. | `build/series` also edits it. |
| `tools/cohort/steps.tsv` | two rows owed: `found-child` in the founding phase, `settle-derived` after both parents terminate. Not a gate failure today — it is *why* the `blocked.json` entry above must say `status-report` rather than claim a campaign. | `build/failure-arm`, `build/claims-split-merge` and `build/founder-bond` all edit it. |
| `tools/gates/frames-baseline.json` | accept a new baseline from two identical `cargo build-sbf` captures. This branch adds functions to two links: all of `parents_v1` plus a widened `FoundAccounts::parse` (Core), and all of `derived_v1` + `derived_transport_v1` plus a widened `process_instruction` and the two new `consume_*` helpers in `relay_transport_v1` (Resolution). `tools/gates/frames.py` refuses a function whose frames differ in EITHER direction, so this is a hard CI stop until it is re-captured. Watch `process_derived_settle_v1` against `bound_bytes: 4096` — it already boxes the source records (`derived_transport_v1.rs:231`) and each parent (`:371`) for exactly this reason. | uncontended, but it needs an SBF build, which this lane is forbidden and the disk could not take. |
| `packages/dclutch-sdk/**` | no SDK twin exists for either new magic (`parentReferenceV1.ts`/`.md`), and `packages/dclutch-sdk/lib/generated/coreFound.ts:11-18` declares only the 37/39 frames, so no SDK client can found a child. `npm run abi:found:verify` still passes because the new constants are additive and unread. | four other branches edit `packages/dclutch-sdk`; and emitting a TS frame table is only worth doing once the Rust builder is settled, which it now is. |

`docs/reference/refusals.md`, `routes.md` and `programs.md` are generated and
uncontended; see §6 for what this lane did about them.

## 3. What this lane completed, and what it ruled

### 3.1 The transport: the native venue gets onto a chain

This was the branch's stated blocker and the reason the two new observables
existed only on paper. `interpret_sealed_record_v1` took an `ArtifactReleaseV1`
unconditionally and **ignored it** on the `Native` arm, while
`relay_transport_v1.rs` demanded that release at consumption slots 19/20 — so a
Feature-program or sysvar row, which has no deployment to pin, had a decoder, a
Lean grammar, eight tests and no way onto a chain.

* `crates/dclutch-source/src/relay/frame.rs` — `CONSUME_RECORD_NATIVE_VENUE_FRAME_V1`
  (28 roles) is `CONSUME_RECORD_FRAME_V1` (30) without its venue-release pair,
  `RelayFrameKindV1::ConsumeRecordNativeVenue` selects it, and
  `consume_position_v1` moves one canonical position into whichever frame the
  venue kind fills. The two tables have one author because a test walks them
  against each other rather than trusting the sentence.
* `crates/dclutch-source/src/relay/decode.rs` — `pinned_venue_release` becomes
  `Option<ArtifactReleaseV1>`, and BOTH mismatches are named:
  `Error::VenueReleaseAbsent` (a `LoaderV3` row without one, whose cross-cluster
  ELF check IS its defense) and `Error::VenueReleaseNotPinnable` (a `Native` row
  carrying one). An ignored argument made a caller's belief that it had pinned a
  deployment unfalsifiable.
* `programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs` — the account
  count is the discriminant, exactly as Core's Found parser reads its three
  widths; every consumption index is written once in the canonical thirty-slot
  system and moved through `consume_slot`. After the adapter configuration
  decodes, the market's OWN decoding-rules row is asked whether the width the
  caller chose was the right one.
* `programs/dclutch-resolution-proof-sbf/src/lib.rs` — `RelayedVenueKind = 0x8027`.
  Deliberately **not** `AccountFrame`: the frame's count and privileges were
  exactly one of the two admissible shapes, and what disagreed was the shape and
  the row. The fix is the market's configuration, not the caller's account list.
* `relay_v1.rs` — `AuthenticatedVenueReleaseV1` pairs the release with the identity
  that named it, so no caller can present half the pair, and `authenticate_graph`
  takes the venue kind.

### 3.2 Provisional rulings (ember rules by reversal)

1. **A `Native` row's `SourceSpecV1.adapter_config_id` is the pinned ordered
   account set's identity** (`RelayedAdapterConfigV1::account_set_id`). On a
   `LoaderV3` row that slot names the venue's `ArtifactReleaseV1`; a native venue
   has no deployment, and the account set is the only thing it pins. This is not a
   placeholder: it binds the Source spec to the account set independently of the
   provider release, which today reaches it only through
   `provider_release.decoding_rules_id`. Two differently rooted commitments to one
   identity. Alternatives considered and rejected: a zero identity (`ContentId`
   refuses it), a new Lean-emitted sentinel (a new identity for an absence), and
   pointing it at the adapter configuration itself (the publisher would have to
   land the same body at the same address twice). **Reversal cost: one Lean
   constant and one line in `authenticate_graph`.**
2. **`emptyParent` gets its own Core code**, `ParentEmpty = 0x3027`. It was folded
   into `ParentBinding`, which made "this parent has no ordinary outcome" and "this
   parent's Product record moved" the same refusal. The five founding conjuncts of
   the Lean's `ProductShape.found?` / `ConditionalShape.found?` now map onto Core's
   band by name and TOTALLY. Caveat stated in the code: **no wire can reach it** —
   `ParentReferenceV1::decode` refuses a zero `ordinary_count` as `ZeroIdentifier`
   first — so on chain this arm proves the decoder already closed it, and the
   in-process callers of `admit_founding` (the child compiler) are what reach it.
   **Reversal cost: delete the variant and restore the `ParentBinding` arm.**
3. **The bootstrap publisher refuses a native-venue relayed market by name.**
   `relayed_market_input` (`tools/local-validator/.../relayed.rs`) states that
   `market.rs::authenticate_source_publication_v1` has a single
   `RelayedObservationRecord` arm which publishes the adapter configuration under
   `ARTIFACT_RELEASE_SCHEMA_ID_V1`, and that a row with no venue program has no
   such body. Debt, named as debt: founding the flagship's two parents on devnet
   needs that arm, and `market.rs` is a file three other branches are editing.
4. **The parent tail does not participate in the Found projection**, the same way
   the price gate does not. The projection selects the Market, generation, release
   set and lifecycle-credit PDA; the child's parenthood already reaches
   `MarketIdentity` through its own `SourceSpecV1`, whose `adapter_config_id` IS
   the reference record's digest, and Core's `authenticate_reference_record` proves
   that equality. Consequence worth knowing: the host does not check tail keys for
   distinctness against the canonical frame (it does not for the certificate pair
   either); Core's `require_distinct` is the only author of that.
5. **The flagship's metric is a flag, not a fork.** A child is blind to what its
   parents observe — `ParentFactsV1` carries a market, a generation, a Product
   digest, an ordinary count and a deadline, and nothing about a venue or a feed —
   so slot time through the relay's native row 3 and a Pyth price are the same
   child at the same width. `FlagshipMetricV1` therefore has both arms and neither
   is a stub.

Rulings 1–11 of `BUILD_PRODUCT-SHAPES.md` §3 are unchanged and unreversed. Its
ruling 10 — which feature, which slot, which metric, and the disclosure line —
**stays ember's**, and §3.3 below is what makes that answer three flags.

### 3.3 The flagship shape

`crates/dclutch-product-runtime-v2-operator/src/flagship_v1.rs`, new. Three
inputs and what each decides:

* `feature_gate` selects the decision parent's pinned account set (relay row 2's
  two positions). It moves no width and no cut.
* `activation_slot` `S` is the decision parent's ONE cut, at `S + 1` — row 2
  yields `activated_at` when activated and a sentinel above every slot when not,
  so `S + 1` separates "activated by `S`" (cell 0) from "not, including never"
  (cell 1). `FLAGSHIP_DECISION_ORDINARY_COUNT_V1 = 2` and it is not a parameter:
  a third ordinary cell would be one no observation can select.
* `metric` is the cuts that carve parent `B`, and only their COUNT reaches the
  child (ruling 5).

`flagship_slot_time_metric_v1()` is the first and only consumer of
`FLAGSHIP_SLOT_TIME_CUTS_MILLIS_V1 = &[390, 410]`, which the decoding rules emitted
and nothing read — the orphan `BUILD_PRODUCT-SHAPES.md` §2.5 named as "a neat
marker of where the flagship stops being built".

### 3.4 The orphans that closed

* `child_v1.rs` was unreachable: the operator's Found width dispatch had two arms
  where the program parses three. `ParentFoundTailObservationV2` is the twelve
  appended slots, `extend_with_parents` takes their privileges from the emitted
  `FOUND_PARENT_ACCOUNT_ROLES_V3` exactly as the price gate takes its own, and
  offering both refuses as `Error::PriceGateAndParentTail` — 51 is not a width.
  `tools/local-validator/.../market.rs::authenticate_found_snapshot_coordinates_v3`
  grew the same third arm plus a host-side mirror of `ParentSame`.
* `RelayedRowFactsV1::for_observable` grew its two real rows rather than a
  wildcard, `account_set_entries` is kind-aware (a native set is two entries) and
  asserts its width against the row's own `set_cardinality()`, and
  `venue_semantic_release` is `Option` because a native row has no deployment to
  name one.
* `tools/gauntlet/relayed-vertical/src/input.rs:43` hard-coded `[AccountSetEntryV1; 4]`
  in its own return type, making the shim a second author of a width the row
  already carries. It and its one caller (`vertical.rs:921`) now take the row's.

## 4. Tests added, and what each proves

**`crates/dclutch-source/src/relay/frame.rs`**

| test | proves |
| --- | --- |
| `the_native_consume_frame_is_the_canonical_one_without_its_venue_release` | the 28-slot frame IS the 30-slot frame minus positions 19/20, walked role by role, and `consume_position_v1`'s arithmetic is that walk. A slot added to either frame alone fails here. |
| `the_canonical_width_does_not_pass_as_a_native_consumption` | the widths are a real discriminant: a 30-account frame presented as a native consumption refuses on its count. |

**`crates/dclutch-source/src/relay/decode.rs`** (4). `interpret_sealed_record_v1` is
the whole relayed interpretation and **nothing in this repository tested it** —
every existing test in that file called a private `read_*` helper directly. These
four drive real sealed records built the way `record.rs` builds its own (create,
append per position, seal), with the account-set identity as ONE constant used
three times: the record's binding, the adapter configuration's, and the
`recomputed_account_set_id` argument.

| test | proves |
| --- | --- |
| `a_native_row_interprets_with_no_venue_release_and_reports_no_deployment` | **the positive control.** Three passes: the feature row activated (atoms `344_000_000`), the feature row dormant (atoms `FEATURE_NOT_ACTIVATED_SENTINEL_V1`), and the mean-slot-time row on the Lean's worked witness (atoms `400`). Each asserts exact observable, atoms, observed slot, observed unix time, and `venue_deployment() == None`. Its own control was run and reverted: pushing the clock past the age bound makes it fail `ObservationTooStale`, so the attested bytes really are being read. |
| `a_native_row_carrying_a_venue_release_refuses_rather_than_ignoring_it` | `Err(Error::VenueReleaseNotPinnable)`, both native rows. |
| `a_loader_v3_row_with_no_venue_release_refuses_by_name` | `Err(Error::VenueReleaseAbsent)` over a real four-position DBC set, so the refusal is reached PAST every earlier guard rather than short-circuited. `VenueReleaseAbsent` has exactly one production site, so the variant is attributable. |
| `a_native_row_whose_state_is_not_the_pinned_sysvar_refuses_on_its_owner` | the mean-slot-time row's address pin: correct owner on both entry and body, wrong key, and only the address check can fire — `Err(Error::ObservedOwnerMismatch)`. |

**`crates/dclutch-source/src/parent_reference_v1.rs`** — `the_emitted_corpus_refuses_row_by_row`
now asserts, per row, that the row is not the valid record BEFORE asserting that it
is refused. That assertion is what found §5's false theorem.

**`crates/dclutch-product-runtime-v2-operator/src/flagship_v1.rs`** (4)

| test | proves |
| --- | --- |
| `the_metric_is_a_flag_and_the_child_does_not_notice_which_one` | two different metrics, the same child at the same width and the same question — ruling 5, executed. |
| `the_worked_metric_carries_the_emitted_millisecond_cuts` | the cuts are the Lean's `[390, 410]`, read and not retyped. |
| `the_decision_cut_sits_one_slot_above_the_chosen_slot` | `S + 1`, and that `u64::MAX` (row 2's own not-activated sentinel) refuses. |
| `each_malformed_choice_refuses_by_its_own_name` | five malformed choices, five distinct named refusals, no shared "bad input". |

**`crates/dclutch-product-runtime-v2-operator/tests/flagship_child.rs`** (3, new file)

| test | proves |
| --- | --- |
| `the_flagship_compiles_from_three_flags_and_two_parent_facts` | the whole child compiles: reference bytes, the consecutive-cut domain `[1,2,3]`, the founder's branch bundle `[4,4,4,0,0]`, the refunding basis at width 5, and the window. |
| `the_reference_the_child_carries_is_the_record_core_will_admit` | the compiled `ParentReferenceV1` bytes decode to the same record and `admit_founding` to the same shape Core's `parents_v1.rs` will reach. |
| `the_chosen_slot_appears_once_as_the_decision_parents_cut` | the dormant-feature sentinel lands above the cut, in the not-activated cell. |

**`crates/dclutch-product-runtime-v2-operator/tests/found.rs`** (3)

| test | proves |
| --- | --- |
| `the_child_found_frame_is_the_canonical_frame_and_a_twelve_slot_parent_tail` | the tail starts exactly at `FOUND_ACCOUNT_COUNT_V3` with no gap, and the child roles table's first 37 rows are byte-identical to the canonical table's. |
| `a_child_founding_appends_twelve_parent_slots_at_the_privileges_the_frame_declares` | all 49 metas' `(writable, signer)` equal their emitted rows, each of the twelve keys is at its emitted coordinate, and the 37-account prefix is byte-equal to the canonical plan's. The builder cannot disagree with the program. |
| `a_price_gate_and_a_parent_tail_offered_together_refuse_as_no_market_at_all` | `Err(Error::PriceGateAndParentTail)`, exactly. |

**`programs/dclutch-core-sbf/src/frame.rs`** (2). `BUILD_PRODUCT-SHAPES.md` §5 said
`frame.rs` had **no tests at all** and that the whole on-chain surface had zero;
these are the on-chain parser's first, and they are the counterpart of the
operator-side pair above — the two ends of the same frame, each measured against
the same emitted table.

| test | proves |
| --- | --- |
| `the_found_parser_admits_three_widths_and_no_fourth` | 37, 39 and 49 parse and select the right extension; each of the twelve tail slots is the account at its own emitted coordinate; the three widths are pairwise distinct, which is what makes the length a discriminant; and **51 refuses `AccountFrame`** — ruling 3 enforced where it has to be. |
| `a_parent_tail_slot_that_signs_or_is_written_refuses` | all twelve slots × three privileges: a child founding acquires no authority over a parent's Market or its Registry records. |

**`tools/local-validator/bootstrap/successor/src/relayed.rs`** (2)

| test | proves |
| --- | --- |
| `every_table_row_builds_a_set_as_wide_as_its_own_row_states` | walks `RELAYED_OBSERVABLE_TABLE_V1`: each row's entry count is its own `set_cardinality()`, the state entry's width is its own `state_inline_bytes()`, and a native row's entries carry no Loader V3 owner. |
| `a_native_row_refuses_out_of_the_producer_naming_the_missing_publisher_arm` | the refusal names the row, the function and the arm — exact substrings, never `is_err()`. |

**`crates/dclutch-product-runtime-v2-operator/src/child_v1.rs`** — the three
pre-existing tests were rewritten to assert exact named refusals
(`Err(Error::FoundingBand)`, `Err(Error::InvalidRecord)`) instead of `is_err()`,
which is this tree's law and which they were breaking.

## 5. What a reviewer should distrust in `BUILD_PRODUCT-SHAPES.md`, because I checked

1. **§7's green is smaller than it reads, and it says so — but the four other
   crates were not "unchecked", they were RED.** Four one-line compile errors, one
   per crate: a receipt field read as a method (`child_v1.rs:308`), a private
   projection read as a field twice (`derived_transport_v1.rs:466`, `:497`), and a
   `ContentId` compared to a `[u8; 32]` (`parents_v1.rs:192`). The branch's own
   "presumably sound" was wrong in every one of the crates it applied to.

2. **§2.4 S9 does not break the build; §2.4 misses one that does.**
   `account_set_entries -> [AccountSetEntryV1; 4]` compiled fine at HEAD — no caller
   passes a native row — so it was a latent modelling gap, not a compile error. What
   the doc missed is a SECOND non-exhaustive match:
   `tools/local-validator/bootstrap/successor/src/market.rs:2046` on
   `SourceAccessProfile`, which the new `DerivedFromParents = 5` variant broke. Two
   exhaustive matches were red, not one, and only one of them is in the seam table.

3. **§5's "23 tests" had one that could not pass, and a Lean theorem that cannot
   hold.** `the_emitted_corpus_refuses_row_by_row` FAILED on corpus row 12. That row
   is `(encodeRecord productExample).set RecordField.settleBy.offset 0`, and the
   product example's `settle_by` is `0x6b49d200` — whose low byte is already zero.
   The row is the valid record byte for byte. So `record_refusal_corpus_refuses`,
   which claims every row is refused, **is false**, and `native_decide` cannot have
   proved it. The Lean now zeroes the whole field (`zeroField`), two new theorems say
   no corpus row is the valid record, and the emitted twin's row 12 was patched by
   hand to match. **This branch's Lean has never been built** (§7 admits it, and no
   `.lake` exists in this worktree); the merge lane must run
   `lake build DClutchSemantics.ParentReferenceV1Abi` before trusting any theorem
   name in `ParentReferenceV1Abi.lean` or `ConditionalMarketV1.lean`.

4. **§2 S6's census claim is right, but not for the reason given.** The doc says
   `tools/gauntlet/census/src/magics.rs:21`'s `DCLTDRV1` "reads as a historical note".
   Checked by value: `grep` for both the ASCII and the byte tuple across the whole
   tree finds `DCLTDRV1` in exactly three places — the census's prose comment, the
   Lean, and the emitted twin. `DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1` **no
   longer exists in the tree at all** (the dealer arm was folded into
   `dclutch-accelerator-sbf` on 2026-09-04). So the value is uniquely this branch's
   — and `magics.rs:21` now describes a constant that is gone, which will mislead
   the next reader.

5. **§2's seam count is now eight Core codes and ten Resolution codes, not seven
   and nine.** `ParentEmpty = 0x3027` and `RelayedVenueKind = 0x8027` are this
   lane's (§3.2 rulings 2 and 1). Anything the doc says about "sixteen rows" in
   `docs/reference/refusals.md` is now eighteen.

6. **§2.5's dead constant is no longer dead.** `FLAGSHIP_SLOT_TIME_CUTS_MILLIS_V1`
   has a consumer (§3.3).

7. **What the doc got RIGHT, checked, so nobody re-checks it.** §2.3's two
   blast-radius claims hold: `RelayedSetLayoutV1`'s `Option<u16>` fields and
   `RelayedObservationOutcomeV1::venue_deployment`'s `Option` return broke no
   compiling code, which a full `--workspace --all-targets` now says rather than
   a grep. §2.4's "no symbol on this branch is used but undefined" holds. §4's
   "nothing is marked" holds — no `todo!()`, `unimplemented!()`, `TODO`, `FIXME`
   or Lean `sorry` in the diff, and none was added by this lane.

8. **`tools/gauntlet/census/src/magics.rs:21` will mislead the next reader**, and
   it is not this branch's to fix. Its comment says "the dealer side is now
   `DCLTDRV1`" — `DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1` no longer exists
   anywhere in the tree (the dealer arm folded into `dclutch-accelerator-sbf` on
   2026-09-04), so the sentence names a constant that is gone while the value it
   names is now uniquely `DerivedSettleRequestV1`'s. The census gate itself is
   fine: 0 constant names bound to more than one magic.

## 6. The generated reference

`docs/reference/refusals.md`, `routes.md` and `programs.md` are uncontended, and
this lane attempted `tools/gate reference --check --converge`. See §6.1 for what
it found. Eighteen refusal rows are owed —
`0x3020`–`0x3027` after `0x301F SeriesMarketVacancy`, and `0x801E`–`0x8027` after
`0x801D SourceLadder` — plus one route row for
`resolution/derived_transport_v1::process_derived_settle_v1`, sorted between
`resolution/core_effect::process_direct_funding_close_v1` and
`resolution/pre_market_funding_abort_v1`, and a route-count bump for
`dclutch-resolution-proof-sbf` in `programs.md`. Do not hand-edit any of them:
`tools/gate reference --converge`, from a detached worktree at the merged HEAD,
and commit the reference and the SDK mirrors together.

### 6.1 Result — RAN, and committed

`tools/gate reference --converge` reached its fixpoint on pass 3 and its output
is committed (`2eb727d08`). Eighteen refusal rows and one route row are this
branch's, and the counts move: **148 → 149 routes, 352 → 370 refusal codes**;
`core` 32 → 40 codes, `resolution` 30 → 40, `resolution` 10 → 11 routes.

**Most of the churn is not this branch's, and that is the thing to know.** Devnet
witnesses move 42 → 47, the ProgramTest class 44 → 40 and blocked 36 → 35:
cohort-17's own evidence, which landed on `main` without the reference being
regenerated after it. So `main` was ALREADY off its fixpoint, and every sibling
converge branch that runs this gate will produce the same main-derived churn.

**Merge lane: do not resolve those eleven files hunk by hunk.** Take either side,
run `tools/gate reference --converge` ONCE on the merged tree, and commit what it
writes. `route-witnesses.md` will then say three unrecorded routes rather than
two until `tools/gauntlet/blocked.json` gets §2's entry.

The census inside that gate is also the answer to `BUILD_PRODUCT-SHAPES.md` S6:
**370 magics, 356 distinct values, 0 constant names bound to more than one
magic.** `DCLTPRF1` and `DCLTDRV1` collide with nothing.

### 6.2 Two gates that are red for reasons that are not this lane's

* `tools/gate fmt` — `crates/dclutch-operator/src/lib.rs` is rustfmt-dirty at
  `main` and is NOT in `tools/gates/fmt-baseline.txt`, which fails the gate on its
  own. Left alone deliberately: it is nobody's file in this lane, and the
  baseline's own note warns that formatting a `lib.rs` reflows whatever `mod`
  reaches. `tools/local-validator/.../market.rs` had the same defect (dirty at
  `main`, unbaselined) and IS fixed here, because this lane already edits it —
  one `assert_eq!` in its tests, reflowed.
* This lane also **shrank the fmt baseline by three**:
  `tools/local-validator/bootstrap/successor/src/relayed.rs`,
  `tools/gauntlet/relayed-vertical/src/input.rs` and `.../vertical.rs` are files
  this lane holds and are rustfmt-clean now, so their entries would have been
  stale — and the gate fails in that direction too. The section header said 62
  while the section held 57; it now says 54 and holds 54. `fmt-baseline.txt` is
  uncontended: no other `build/*` branch touches it.

## 7. Still owed, in the order that works

1. **The real-ELF program test.** A child founding through the 49-account Found
   frame against the compiled Core ELF, with parent `A` the feature-gate market and
   parent `B` the relayed slot-time market. It needs eight `cargo build-sbf`
   invocations (`programs/dclutch-core-sbf/run-open-market-program-test.sh`, 3–7 GB),
   which this lane was forbidden and the disk could not hold at the time. Write it
   as its own file under `programs/dclutch-core-sbf/tests/` — the runner globs that
   directory, so a first failure is then one red target and not five.

   **Everything short of the transaction is asserted and runs today**, from both
   ends of the frame: `tests/flagship_child.rs` compiles the child's four records
   from the three flags, `tests/found.rs` asserts all 49 metas against the emitted
   table, and `frame.rs`'s two new tests assert the on-chain parser's three widths
   and the tail's read-only discipline. What no test can reach without a bank is
   the SEEDING, and that is the whole of the remaining work:

   * the twelve tail accounts, at `FOUND_PARENT_REFERENCE_RAW_INDEX_V3 = 37`
     onward — the `ParentReferenceV1` raw record and its staging vacancy under the
     Registry, then per parent (`FOUND_PARENT_A_MARKET_INDEX_V3 = 39`,
     `FOUND_PARENT_B_MARKET_INDEX_V3 = 44`) a Core-owned `CoreState` at its own
     `MarketCoreStateSeedsV2` address plus that parent's Product record and result
     domain as raw+staging pairs;
   * each parent's `CoreState.phase` **Open or Terminal** — `Founding` and
     `Retiring`/`Retired` are what `CoreSbfError::ParentPhase` refuses;
   * each parent's `ResultDomainV2::region_count` equal to the reference's
     `ordinary_count` — 2 for the feature-gate parent, 3 for the slot-time parent —
     which `parents_v1.rs` proves rather than trusts;
   * the child's own `SourceSpecV1` with `access_profile = DerivedFromParents` and
     `adapter_config_id` equal to the reference record's digest, which is the
     equality `authenticate_reference_record` checks.

   `found_program_test.rs`'s existing `Record` / `Fixture` helpers build raw+staging
   Registry pairs already; the parents' `CoreState` accounts are the only genuinely
   new fixture.
2. **The frames baseline** (§2), after 1, because 1 is what settles which functions
   exist.
3. **The publisher's native-venue arm** (`market.rs::authenticate_source_publication_v1`),
   which is what unblocks founding the flagship's two parents on devnet, and then
   `relayed_market_input`'s refusal becomes a producer.
4. **The gauntlet campaign and the runbook rows** (§2), then the CU budget — in
   that order and no other, since `tools/gate budgets` refuses a campaign no
   bindings file names.
5. **The SDK twins** (§2), last, because emitting a TS frame table for a builder
   that was still moving would just have added an author.
