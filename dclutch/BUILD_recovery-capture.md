# BUILD — family `recovery-capture`

Branch `build/recovery-capture`, worktree off `main` at `f7c03e845`. Built by source
inspection; nothing here has met a chain. Written incrementally; the last section
records what is stubbed and what is owed.

## 0. The three rulings this build stands on (provisional; ember rules by reversal)

R1. **The refreshable publication is a lab minter, not a recapture.** The fixture's
guardian set IS the derivable dummy set (`secret_i = [i+1, 0..]`; checked against
`guardian-set-0.account.hex`: 19/19 addresses, 13/13 captured signatures recover under
`keccak(keccak(body))`, the single-leaf root is `keccak256(0x00 ‖ message)[..20]`). A
fresh VAA over a fresh `PriceFeedMessage` is signable offline at any instant. The shelf
life is a parameter of the tier (and of the producer's caller), never of the market shape.

R2. **Three of the material's four reserved bytes are the ensemble**: byte 12 `k−1`,
byte 13 `q−1`, byte 14 the rung count AFTER the members, byte 15 reserved. The rung byte
is REQUIRED zero for `k = 1` (a single-source policy is all rungs, as today), so every
founded material is byte-identical. It exists so the failure walk and the push family's
commit — whose 22-account frames carry no policy — can answer "does this ladder have a
rung" off the material alone (`SourceMaterialV3::ladder_has_rung`). The design's
"`exhaust_after_primary_deadline` becomes 'the policy has a rung'" is met without
growing those frames. Founding checks `attempt_count == (k−1) + rungs`.

R3. **Members are staggered by one second, not equal.** The design says every member's
deadline equals the window's, but `RecoveryPolicyV2::validate_shape` and Lean's
`deadlinesIncreasing` require strictly increasing deadlines over the whole list, so two
members at one deadline are an unencodable policy. Canonical spelling: member `m`
(1-based) sits at `window_deadline + m − 1`; a member's deadline is never read by any
route (a member is captured under the window's own clock rule), so the stagger is
informational and canonical. `RecoveryPolicyV2::validate_ensemble_membership` states it;
`EnsembleResolutionV1.membersShareTheWindow` is superseded by it (seam L-4 below).

R4. **The fragment's `receipt_account` carries the captor.** A fragment is a kind-1
certificate in a member seat; its `receipt_account` (self-referential and information-free
for a terminal) carries the execute's `resolver`, which is who the fold pays the member's
bounty to. Member zero (the primary source) has no attempt row and no bounty — today's
unpaid primary capture, byte-identical; the flagship founding can add one later (seam O-6).

R5. **Odd quorum is a founding refusal, not a decode refusal.** `EnsembleSpecV1::new`
admits any `1 ≤ q ≤ k ≤ 5` (the theorems are proven for all of them);
`EnsembleSpecV1::validate_foundable` refuses an even `q` with `EnsembleQuorumEven`, and
Core's founding conjunct plus the market compiler call it.

R6. **The reclaimed member seat returns to the Source state's `rent_beneficiary`**, not to
"the founding's recorded payer": one beneficiary for every seat's rent, one author.

## 1. What was built, file by file

### 1.1 The publication producer (deliverable 1, half)

- `tools/local-validator/bootstrap/successor/src/pyth_lab_publication.rs` (new, ~560 lines).
  `LabPublicationRequestV1 { publish_time, price, confidence, exponent, ema_price,
  ema_confidence, sequence }`, `LabPublicationRequestV1::at(publish_time, sequence)` with the
  capture's price facts; `mint_lab_publication_v1(request, write_authority) ->
  LabPublicationV1 { message, root, signed_vaa, post_update_body, projected_price_update }`;
  `post_update_data()` (Anchor tag + body); `recover_vaa_signers_v1` (the router's check,
  offline); `lab_guardian_secret_v1/address_v1`. Tests (6, run filtered, green): the
  fixture guardian set is the derivable set; the captured VAA recovers under the double
  keccak; the captured root is the leaf hash of the captured message; a mint at the
  captured instant reproduces the captured message AND `PostUpdate` data byte-for-byte and
  its fresh VAA recovers to guardians 0..13; determinism; a non-positive instant refuses.
- `tools/local-validator/bootstrap/successor/Cargo.toml`, `tools/gauntlet/ladder/Cargo.toml`:
  `libsecp256k1 = "=0.7.2"` (already in the workspace lock via the Solana SDK).
- `tools/local-validator/bootstrap/successor/src/main.rs`: `mod pyth_lab_publication;`.

### 1.2 Lean (deliverable 3, Lean-first)

- `formal/dclutch-semantics/DClutchSemantics/SourceMaterialV3Abi.lean`: fields
  `ensembleMembers | ensembleQuorum | ensembleRungs | reserved 1` at 12/13/14/15 (theorem
  `ensemble_bytes_sit_in_the_old_reserve`), `Material.ensembleValid`, `Material.valid`
  gains it, `Material.ensembleFoundable` (odd quorum), `Material.ladderHasRung`,
  `ensembleExample` (k=3,q=3,r=1), five new refusal-corpus rows (19 total), theorems
  `a_single_source_material_is_todays_bytes`, `an_even_quorum_is_not_foundable`,
  `a_single_source_policy_is_all_rungs`, `an_ensemble_without_rungs_exhausts_on_its_primary`.
  `lake build` green.
- `formal/dclutch-semantics/DClutchSemantics/EnsembleFoldReceiptV1Abi.lean` (new): the
  280-byte receipt (`DCLTEFR1`; memberCount, quorum, consumedCount, consumedBitmap, market,
  generation, terminalSequence, sourceMaterial, medianNumerator (16 bytes), selector, five
  fragment digests at stride 32 from 120); `receiptPdaDomain`, `fragmentPdaDomain`,
  `evidenceDomain`; `Receipt.valid` (bitmap population = count, digests agree with bitmap);
  two examples, 16 refusal rows, `consumed_count_is_the_bitmap_population`. `lake build` green.
- `formal/dclutch-semantics/DClutchSemantics/EnsembleResolutionV1.lean`: imports the
  material ABI; `Spec.ofMaterial`, `the_zero_bytes_are_the_single_source_spec`,
  `a_valid_material_declares_a_valid_spec`, `Spec.foundable` with the flagship (5,3), the
  fallback (3,3), the cheap shape (3,2) and (4,4) decided, `Spec.firstRungIndex`,
  `rungs_begin_at_the_first_rung_index`, `Spec.rungsAgree` with a worked example.
- Emitters: `EmitSourceMaterialV3AbiRust.lean` (adds `SOURCE_MATERIAL_V3_ENSEMBLE_MAX_MEMBERS`
  and the ensemble example), `EmitEnsembleFoldReceiptV1AbiRust.lean` (new). Both run through
  `lake env lean --run` and `rustfmt`; the outputs are the committed generated files.
- `formal/dclutch-semantics/DClutchSemantics.lean`: imports `EnsembleFoldReceiptV1Abi`.
- `crates/dclutch-source/check-generated.sh`: pins the four new material offsets and
  `ENSEMBLE_MAX_MEMBERS`; a ninth block guards the receipt emitter.

### 1.3 The contract crate `crates/dclutch-source`

- `src/generated_source_material_v3.rs` (regenerated), `src/generated_ensemble_fold_receipt_v1.rs` (new).
- `src/source_material_v3.rs`: `EnsembleSpecV1 { members, quorum }` (`SINGLE`, `new`,
  `first_rung_index`, `validate_foundable`, `declares_member`); the material gains
  `ensemble`, `ensemble_rungs`; `with_ensemble(spec, rungs)` (the constructors stay
  `const fn` and produce `SINGLE`, so no caller moves); decode/encode of the three bytes;
  `ensemble()`, `ensemble_rungs()`, `ladder_has_rung()`. Tests: the Lean example round-trips,
  every clause refuses by name.
- `src/source_recovery_policy_v2.rs`: `member_attempt(spec, member)`, `rung_count(spec)`,
  `validate_ensemble_membership(spec, rungs, window_deadline)` (R3).
- `src/source_resolution_v2.rs`: `crank_recovery_ladder(.., observed_fragments)` enters
  `first_rung_index` from `Primary`, refuses a rungless ladder (`InvalidRecoveryTransition`)
  and a met quorum (`EnsembleQuorumMet`); `exhaust_after_primary_deadline(..,
  observed_fragments)` refuses on `ladder_has_rung` and a met quorum;
  `next_crank_funding_config(material, ..)`; `fold_ensemble_from_authenticated_domain`
  (strictly after `end + max_age`, `k > 1`, `n ≥ q` else `EnsembleQuorumNotMet`, median by
  `exact_median_by`, commit through `resolve_primary_from_authenticated_domain`) →
  `EnsembleFoldV1 { decision, median, consumed }`.
- `src/ensemble_fold_receipt_v1.rs` (new): `EnsembleFoldReceiptV1` decode/encode over the
  generated layout with the bitmap/count/digest agreement; tests over the Lean corpus.
- `src/lib.rs`: modules, exports, `Error::{NonCanonicalEnsemble, EnsembleQuorumEven,
  EnsembleMemberOutOfRange, EnsembleWindowMismatch, EnsembleQuorumNotMet, EnsembleQuorumMet}`.

### 1.4 The relay wire and its frames (`crates/dclutch-source/src/relay/`)

- `instruction.rs` — two new actions on the `DCLTRIX1` family:
  `RelayActionV1::EnsembleFold = 8` (`EnsembleFoldInstructionV1`, **32 bytes**:
  generation @16, terminal sequence @24 — deliberately the crank's shape, so no
  caller can name a quorum, a member set or a reading and thereby choose the
  outcome) and `RelayActionV1::ReclaimMemberSeat = 9`
  (`ReclaimMemberSeatInstructionV1`, **40 bytes**: generation @16, terminal
  sequence @24, member byte @32, bytes 33..40 required zero).
  `ConsumeRecord` gains `source_index: u8` at **offset 42** (was reserved;
  `require_zero` narrows from `(42, 6)` to `(43, 5)`), builder
  `ConsumeRecordInstructionV1::on_source_index(u8)`, accessor `source_index()`.
  Zero — the primary — is what `new` still produces, so every consumption sent
  before an ensemble existed is byte-identical.
- `frame.rs` — three new `RelayAccountNameV1` variants (`EnsembleFoldReceipt`,
  `EnsembleFragmentSeat`, `EnsembleCaptor`); `ENSEMBLE_FOLD_FRAME_PREFIX_V1`
  (**25 fixed positions**: the failure walk's 22 plus the policy pair's members
  and the receipt seat; writable {0,4,5,6,21}) followed by a tail of `2k`
  (`ensemble_fold_tail_v1`: `k` read-only member seats in member order, then `k`
  writable captors in the same order); `RECLAIM_MEMBER_SEAT_FRAME_V1` (**exactly
  6**: worker, market-read, source-state-read, the seat, the beneficiary, System);
  `ensemble_seat_tail_v1` (the `k` read-only seats a crank or a failure walk
  carries so the crank/fold exclusivity is a fact of the frame);
  `validate_relay_frame_with_tail_v1(kind, accounts, tail_len, tail_role)` — exact
  prefix, tail privileges by role, no-alias across the WHOLE frame so a seat
  cannot be passed twice to answer twice.
- `resolution/mod.rs` — `EnsembleFragmentSeatSeedsV1` (`dclutch/ensemble-fragment/v1
  ‖ source_state ‖ [member] ‖ terminal_sequence`) and `EnsembleFoldReceiptSeatSeedsV1`
  (`dclutch/ensemble-fold-receipt/v1 ‖ source_state ‖ terminal_sequence`), both
  SDK-free with `seeds()` and `const _: () = assert!(domain.len() <= 32)`.

### 1.5 The Resolution program (`programs/dclutch-resolution-proof-sbf/`)

- `src/ensemble_v1.rs` (**new, 351 lines**) — the pure fold, failure-atomic, no
  mutation. `EnsembleFoldErrorV1 {Request, Source, Product, Fragment, Quorum,
  Transition, Funding, Arithmetic}`; `EnsembleFoldRequestV1`; `MemberSeatV1
  {Vacant, Written(ResolutionCertificateV2)}`; `AuthenticatedEnsembleSourceV1`
  (material + window + policy + the primary's provider release + the statistic's
  scale exponent); `EnsembleFoldPlanV1 {next_source, certificate, receipt,
  next_funding, funding_lamports_after, bounties}`; `admit_fragment` and
  `plan_ensemble_fold_v1`. The fold decodes each Resolution-owned member seat as a
  kind-1 fragment, refuses fewer than the quorum, takes the median through the
  crate's one scan (`scheduled_median_v1::exact_median_by`), commits through
  `resolve_primary_from_authenticated_domain`, writes the market's own terminal
  certificate with `attempt_index` zero and `provider_evidence` folded over the
  consumed fragments under `ENSEMBLE_EVIDENCE_DOMAIN_V1`, and releases each
  consumed member's bounty to the captor its fragment named.
- `src/funded.rs` — `FundedWalkErrorV1::QuorumMet`; `map_crank_error` (the one
  refusal that names the fold, kept apart from the deadline's);
  `MemberBountyReleaseV1 {member, entry_index, work_paid}`, `MemberBountyPlanV1`,
  `plan_member_bounty_releases_v1(escrow, policy, ensemble, consumed_bitmap)` —
  one pass over the one ledger, member `m` paid from the row its attempt's
  `funding_allocation` selects (configuration, never position), member zero (the
  primary) paid nothing as every primary capture has been;
  `selected_entry_for_config` (exactly one selected entry with that config, or a
  refusal). `process_funded_transition` and `plan_deadline_failure_v1` each gain a
  trailing `observed_fragments: u8`.
- `src/lib.rs` — `pub mod ensemble_v1;` and three codes appended to the 0x8 band
  pin: `EnsembleMember = 0x801E`, `EnsembleQuorum = 0x801F`,
  `EnsembleQuorumMet = 0x8020`.

### 1.6 The tiers (journey, ladder, successor)

- `tools/gauntlet/journey/src/provider.rs` (+~440 lines net) — `PublicationV1
  {signed_vaa, post_update_body, price_update_image, shelf_life_seconds}` and
  `PublicationV1::captured()`: the transport stopped being ABOUT one frozen
  August-2026 instant and takes the publication as a parameter, because a rung is
  entered only after the primary leg's grace expired and the pinned capture is
  stale by construction by then. `RungCaptureV1` (the three coordinates a rung
  substitutes: the alternative `SourceSpecV1`, its `PythAdapterConfigV1`, and the
  `RecoveryPolicyV2` that names them — the window, the statistic, the provider
  release and the whole Product graph stay the market's).
  `PYTH_TRANSPORT_RUNG_STAGE_V1` as a SEPARATE LABEL from the primary's, and
  `transport_stage_v1(rung)`. `refuse_capture_before_the_rung_v1`.
  `resolve_through_pyth` gains `(publication, rung, terminal_sequence)`.
- `tools/gauntlet/journey/src/journey.rs` — passes
  `&provider::PublicationV1::captured(), None, 1` (the primary leg, byte-identical
  to what it always sent); `ledger.track_market(addresses.founding_market)`.
- `tools/gauntlet/journey/src/resolution.rs` — `RecordPairV1::derive` and
  `vacant` raised to `pub(crate)` so the ladder can link the module.
- `tools/gauntlet/ladder/src/main.rs` — `#[path]` links for
  `pyth_lab_publication` (from the successor) and for the journey's `ledger`,
  `provider`, `resolution` and `stages`; `--publication-shelf-life-seconds I64`
  parsed into `LadderRequestV1`.
- `tools/gauntlet/ladder/src/ladder.rs` (+446 lines, written DURING the closeout
  sweep) — the walk that answers a rung: a publication minted at the CLUSTER's own
  block time (`DEFAULT_PUBLICATION_SHELF_LIFE_SECONDS_V1 = 1_200`, sequences 1 and
  2, `PROJECTION_WRITE_AUTHORITY_V1 = [0; 32]`), the market compiled against its
  projection so the window ENDS at the mint instant and both legs fall inside the
  hour the run occupies; the rung's own records (`rung_capture_v1`); the hostile
  that asks for a rung capture while the Source still stands on Primary and
  records the builder's refusal verbatim; the answered rung through the journey's
  own transport; `crate::resolution::admit_terminal`; a `ConservationLedgerV1`
  watching the crank worker and both crank certificates; the transcript gains
  `publications`, `capture`, `conservation` and `publication_shelf_life_seconds`.
  `publication_report_v1` RECOVERS the thirteen signatures rather than counting
  them. `tools/gauntlet/ladder/{run-ladder.sh,witnesses.json}` follow.
- `tools/local-validator/bootstrap/successor/src/market.rs` —
  `LocalMarketShapeV1.price_update_image: Option<Vec<u8>>` (`None` = the pinned
  capture, byte-for-byte what every caller compiled before the field existed);
  `demo_market_input_base_shaped` reads the feed identity, the exponent and the
  window's end off THAT image, so a market and the publication that answers it
  cannot be about two different instants.
- `tools/local-validator/bootstrap/successor/src/local_mutable.rs` —
  `market_shape_from_arguments_v1` carries the default through: a minted
  publication is a signed artifact, not a scalar a command line can hold.

### 1.6b The capture walk, in detail — and two findings that outlive this family

A child agent finished this after its parent died: the rung is not only reachable,
it is ANSWERED. **+471 lines in the journey's `provider.rs`, +447 in the ladder's
`ladder.rs`; both crates `cargo check` clean** (the child's own runs — CLOSEOUT's
red check was `dclutch-resolution-proof-sbf`, a different crate, §7).

- `PublicationV1 { signed_vaa, post_update_body, price_update_image,
  shelf_life_seconds }` with `DEFAULT_PUBLICATION_SHELF_LIFE_SECONDS_V1 = 1_200`.
- `RungCaptureV1` derives the rung's THREE record pairs by content identity — the
  alternative `SourceSpecV1`, its `PythAdapterConfigV1`, the `RecoveryPolicyV2`
  that names them — rather than taking addresses on trust.
- `resolve_through_pyth` takes the rung; `refuse_capture_before_the_rung_v1` is the
  hostile; the ladder tier gains `--walk capture` with its bindings and witnesses.

**FINDING 1 — the journey campaign did not compile at the branch point, and no
tier caught it.** `tools/gauntlet/journey/src/journey.rs:698` read
`ledger.track_market(addresses.market)` while `MarketAddressesV1`
(`tools/gauntlet/journey/src/stages.rs`) has no `market` field — its market is
`founding_market`. Verified against the merge-base `f7c03e845`: the line is there,
red. The child repaired it to `addresses.founding_market` as a side effect of
needing the module to link. **This is a fact about the tree, not about this
family**: a tier campaign sat un-compilable at HEAD and every gate stayed green,
which means nothing in CI compiles `tools/gauntlet/journey`. That gap is worth a
row of its own wherever the convergence tracks CI coverage.

**FINDING 2 — the pre-advance hostile cannot reach the conjunct it is named
after, and says so.** `refuse_capture_before_the_rung_v1` asks the operator to
build a rung capture while the Source still stands on `Primary`. The builder
refuses — but not on the phase-versus-ladder conjunct the hostile is about: the
transport decodes the LIFECYCLE before it reaches the ladder arm, and pre-advance
the lifecycle the request names is a vacancy, so that conjunct speaks first. The
honest shape, and the one the child built: **record the refusal verbatim** (never
assert which conjunct spoke) and add a **reachable mirror** in the capture stage
below, against a Source that really is standing on the rung. Describe it that way
in any summary — calling it "the pre-advance conjunct convicted" would be false.

### 1.7 The operator — NOT BUILT. No `dclutch-operator` file was touched.
### 1.8 The runbook and the SDK — NOT BUILT. No `steps.tsv` row, no TS decoder.

## 2. THE SEAMS (exact file:line, one-line change) — appended as each lands

- **C-1** `crates/dclutch-source/src/source_resolution_v2.rs` — callers of
  `crank_recovery_ladder` and `exhaust_after_primary_deadline` gain a trailing
  `observed_fragments: u8` (0 for a single-source frame):
  `programs/dclutch-resolution-proof-sbf/src/funded.rs:381` (`process_funded_transition`) and
  `:520` (`plan_deadline_failure_v1`); the harness and contract tests that call them.
- **C-2** `crates/dclutch-source/src/source_resolution_v2.rs:832` `next_crank_funding_config`
  now takes the material first: `programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:1150`
  becomes `source_state.next_crank_funding_config(walk_source.material, ladder.policy_id, ladder.policy)`.
- **L-1** `tools/gates/emission-coverage.md`: a row for
  `crates/dclutch-source/src/generated_ensemble_fold_receipt_v1.rs` ↔
  `EmitEnsembleFoldReceiptV1AbiRust.lean` under the `check-generated.sh` guard (regenerate
  with `tools/gate emission --write`).
- **L-2** `formal/dclutch-semantics/README.md` module census: `EnsembleFoldReceiptV1Abi`.
- **L-3** the SDK's generated-module list (`packages/dclutch-sdk/package.json` `abi:*`): see §1.8.
- **L-4** `formal/dclutch-semantics/DClutchSemantics/EnsembleResolutionV1.lean:671`
  `membersShareTheWindow` states equality; R3 supersedes it with the stagger — replace the
  body with `(List.range (members spec policy).length).all fun i => match (members spec
  policy)[i]? with | some a => a.deadline = window.deadline + i | none => false` (left as
  written so the existing theorems' names do not move; no theorem depends on its body).

- **P-1 (the missing outer, and the biggest seam in this family).**
  `programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:173` — the
  `match RelayInstructionV1::decode(...)` is non-exhaustive: `EnsembleFold(_)` and
  `ReclaimMemberSeat(_)` have no arm. `ensemble_v1.rs`'s own header names
  `crate::relay_transport_v1::process_ensemble_fold` as the physical outer that
  "owns the accounts, the seat derivations and the writes" — **that function does
  not exist**, and `plan_ensemble_fold_v1` therefore has no caller anywhere in the
  tree. Same for a `process_reclaim_member_seat`. Writing those two routes (frame
  validation through `validate_relay_frame_with_tail_v1`, the seat derivations
  through `EnsembleFragmentSeatSeedsV1` / `EnsembleFoldReceiptSeatSeedsV1`, the
  receipt write and the captor transfers) is the single largest piece of undone
  work in this branch.
- **P-2** `…/relay_transport_v1.rs:1156` —
  `.next_crank_funding_config(ladder.policy_id, ladder.policy)` becomes
  `.next_crank_funding_config(walk_source.material, ladder.policy_id, ladder.policy)`
  (this is seam C-2, now with the line confirmed by the compiler).
- **P-3** `…/relay_transport_v1.rs:1252` —
  `process_funded_transition(request, source_state, walk_source, ladder, escrow)`
  gains a sixth argument, the observed fragment count (`0` on a frame that carries
  no member seats).
- **P-4** `…/relay_transport_v1.rs:1498` — `plan_deadline_failure_v1(...)` gains a
  seventh argument, the same count.
- **P-5** `…/relay_transport_v1.rs:1525` — the `match error` over
  `FundedWalkErrorV1` gains `QuorumMet => ResolutionError::EnsembleQuorumMet`.
  `EnsembleMember` (0x801E) and `EnsembleQuorum` (0x801F) are declared and pinned
  but **nothing raises them yet**; the fold route (P-1) is where they belong.
- **T-1** `tools/gauntlet/ladder/Cargo.toml` — the tier now links four journey
  modules and one successor module by `#[path]`; whatever crates those pull
  (`dclutch-source`, `dclutch-product`, `dclutch-market`, `serde_json`,
  `libsecp256k1`) must be in its `[dependencies]`. Only `libsecp256k1` was added.
- **T-2** `tools/gauntlet/journey/src/provider.rs` — `resolve_through_pyth` gained
  three parameters. Every caller must pass them; the journey's own call is
  updated, and `tools/gauntlet/ladder/src/ladder.rs` is the second. Any THIRD
  caller (grep `resolve_through_pyth`) is a compile break.
- **T-3** `tools/local-validator/bootstrap/successor/src/market.rs` —
  `LocalMarketShapeV1` gained a field. Every struct literal that spells the shape
  out rather than `..Default::default()` breaks. `local_mutable.rs` is fixed; the
  rest of the successor's callers are not audited.
- **T-5** `tools/local-validator/bootstrap/successor/src/market.rs:3126` — the
  founding's evidence map publishes `recovery_policy_record` and the direct records,
  but **never the rung's own source records** (the alternative `SourceSpecV1` and its
  `PythAdapterConfigV1`). `RungCaptureV1` therefore derives all three pairs from the
  compiled market input by content identity. Either publish them beside
  `recovery_policy_record` here (three `accounts.insert` lines) or keep the
  derivation and say so — but a later reader that expects them in the evidence map
  will find a hole.
- **T-6** the ladder's conservation ledger declares `LamportClaimV1::inapplicable`
  at the crank boundary, so **L7 does not hold there**. The reason is real and not
  laziness: the crank's fee and bounty are paid by the WORKER keypair the tier
  opens, not by the campaign payer the ledger's boundary is drawn around. Either
  widen the watched set to make L7 exact at that boundary, or leave it inapplicable
  with this sentence attached — do not let it silently read as covered.
- **T-4** `tools/gauntlet/ladder/witnesses.json` and `run-ladder.sh` were rewritten
  for the capture walk; the transcript keys they read (`.capture`,
  `.conservation`, `.publications`) exist only on the capture walk and read their
  defaults on the exhaust walk.

## 3. Rulings — see §0, plus two made after the maker stopped

**R7 (the coordinator's, provisional; ember rules by reversal). The ladder tier
runs BOTH walks in one invocation** — the exhaust arm and the capture arm — rather
than flipping its default from `exhaust` to `capture`. The reason is a positive
control: the capture walk's witnesses hold VACUOUSLY on the exhaust walk and the
exhaust walk's `refund`/`failureWalk` witnesses hold vacuously on the capture walk,
and a witness that reads its default on the walk that did not run is an absent
signal with no way to tell "nothing fired" from "my instrument was disconnected".
Running both is the only version where each witness bites. Reversal cost: one
invocation covers one arm and the other arm's witnesses are permanently unread.

**R8. The rung's records are derived from the compiled market input, not read from
the evidence map** — because the founding never publishes them there (seam T-5).


## 4. Stubs

- No `todo!()` was written. The stub in this family is structural and named in
  P-1: **a complete pure fold with no physical outer.** `plan_ensemble_fold_v1`,
  `plan_member_bounty_releases_v1`, `validate_relay_frame_with_tail_v1`,
  `ensemble_fold_tail_v1`, `ensemble_seat_tail_v1`, `ENSEMBLE_FOLD_FRAME_PREFIX_V1`,
  `RECLAIM_MEMBER_SEAT_FRAME_V1`, `EnsembleFragmentSeatSeedsV1`,
  `EnsembleFoldReceiptSeatSeedsV1`, `EnsembleFoldInstructionV1` and
  `ReclaimMemberSeatInstructionV1` are each defined and **called by nothing**.
  This is the tree's own "producer-missing" shape with the halves swapped: here
  the producer is written and the route that would call it is not.
- `EnsembleMember` and `EnsembleQuorum` are pinned refusal codes with no raiser.
- The operator, the successor driver, the runbook rows and the SDK decoder for
  the receipt (§1.7, §1.8) were never started.

## 5. Tests written

- `pyth_lab_publication.rs` (6, the maker reports them run filtered and green):
  the fixture guardian set IS the derivable dummy set; the captured VAA recovers
  under the double keccak; the captured root is the leaf hash of the captured
  message; a mint at the captured instant reproduces the captured message AND the
  `PostUpdate` data byte-for-byte and its fresh VAA recovers to guardians 0..13;
  determinism; a non-positive instant refuses. Together these prove the lab can
  mint a publication the real router and receiver ELFs will accept, which is the
  only thing that makes a rung reachable inside a bounded run.
- `source_material_v3.rs`: the Lean ensemble example round-trips through the Rust
  decode/encode, and every clause refuses by name — proves the three reserved
  bytes carry the ensemble without moving a founded material's bytes.
- `ensemble_fold_receipt_v1.rs`: decode/encode over the Lean refusal corpus —
  proves the 280-byte receipt's bitmap, count and digests cannot disagree.
- Lean `lake build` green on `SourceMaterialV3Abi`, `EnsembleFoldReceiptV1Abi` and
  `EnsembleResolutionV1` (the maker's report; not re-run by CLOSEOUT).
- **Nothing tests the fold itself**, because nothing calls it (§4).

## 6. Frames and codes expected to move

- Codes: band 0x8 gains `EnsembleMember 0x801E`, `EnsembleQuorum 0x801F`,
  `EnsembleQuorumMet 0x8020` (§1.4). Nothing renumbered.
- Frames: every crate compiled into the Resolution link moves (`dclutch-source`,
  `dclutch-resolution-proof-sbf`); Core's founding conjunct moves the Core link. Carrier:
  cohort-17 for the two-source rows, cohort-18 for `found-ensemble` (a material-layout
  change; decision 0034 §4 keeps it off cohort-16).

## 7. STOPPED HERE

`cargo check -p dclutch-resolution-proof-sbf --offline` (run by CLOSEOUT in this
worktree, against the first sweep commit): **RED, 5 errors, all of them the seams
§2 predicted.** Verbatim, the first three:

```
error[E0061]: this method takes 3 arguments but 2 arguments were supplied
    --> programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:1156:10
     |
1156 |         .next_crank_funding_config(ladder.policy_id, ladder.policy)
     |          ^^^^^^^^^^^^^^^^^^^^^^^^^ ---------------- argument #1 of type `SourceMaterialV3` is missing

error[E0061]: this function takes 6 arguments but 5 arguments were supplied
    --> programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:1252:16
     |
1252 |     let plan = process_funded_transition(request, source_state, walk_source, ladder, escrow)
     |                ^^^^^^^^^^^^^^^^^^^^^^^^^---------------------------------------------------- argument #6 of type `u8` is missing

error[E0061]: this function takes 7 arguments but 6 arguments were supplied
    --> programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:1498:16
     |
1498 |       let plan = plan_deadline_failure_v1(
     |  ________________^^^^^^^^^^^^^^^^^^^^^^^^-
     | |_____- argument #7 of type `u8` is missing
```

(This result was reproduced in a target dir private to this worktree. The first
run shared a target dir with the sibling closeout families and cargo reused a
`dclutch-vm`/`dclutch-source` rmeta across worktrees — the units hash the same —
so **give every worktree its own `CARGO_TARGET_DIR`**; a shared one invents errors
about a crate that is not yours. The five errors below survived that correction and
are real.)

The remaining two are `E0004` non-exhaustive matches at `relay_transport_v1.rs:173`
(`EnsembleFold` / `ReclaimMemberSeat` uncovered) and `:1525`
(`FundedWalkErrorV1::QuorumMet` uncovered). `dclutch-source` itself compiled clean
on the way through, so the CONTRACT layer parses and only the program's outer is
behind. No check was run on the tiers.

The next three steps, concretely:

1. **Close the four call-site seams** P-2..P-5 above (four one-line edits in
   `relay_transport_v1.rs`) so the program compiles again. Pass `0` for
   `observed_fragments` from every existing frame — those frames carry no member
   seats, and the contract already refuses a rungless ladder by name.
2. **Write `process_ensemble_fold` and `process_reclaim_member_seat`** (seam P-1).
   Model them on `process_commit_deadline_failure` in the same file: authenticate
   the frame with `validate_relay_frame_with_tail_v1(EnsembleFold, accounts, 2*k,
   ensemble_fold_tail_v1(k, ·))` where `k` comes off the authenticated material,
   derive each seat with `EnsembleFragmentSeatSeedsV1`, read the seats into
   `[MemberSeatV1; 5]`, call `plan_ensemble_fold_v1`, then write the certificate,
   the receipt and the captor transfers. Map `EnsembleFoldErrorV1::{Fragment,
   Quorum}` onto the already-pinned `EnsembleMember` / `EnsembleQuorum`.
3. **Rebase onto main first, or expect churn.** This branch's merge-base is
   `f7c03e845` and `main` is **17 commits ahead** of it (`131e084da`). The tier
   files this family rewrote (`journey/src/provider.rs`,
   `ladder/src/{ladder,main}.rs`) are exactly the files main's newer commits also
   touch; resolving that before the program work is cheaper than after.

## 8. A note from CLOSEOUT: this worktree was still being written to

The first sweep commit landed at 21:40; `tools/gauntlet/ladder/src/{ladder,main}.rs`,
`run-ladder.sh` and `witnesses.json` were rewritten at 21:43 and committed
separately. The maker kept going through 21:54 (`provider.rs`, `ladder.rs`,
`ladder/main.rs`, `README.md`, `witnesses.json` again). If it resumes, expect more.
Read `git log build/recovery-capture` before assuming this document is complete,
and re-run the check before trusting the error list above.
