# Observed-median resolution over an ensemble of declared sources

Design lane ENSEMBLE, 2026-09-04. Tree `/Users/ember/dev/dclutch`, read at
`a1bf4ddf0` through `60ae17272`; no program changed. The Lean owner is
`formal/dclutch-semantics/DClutchSemantics/EnsembleResolutionV1.lean` (this
commit): 31 theorems, 10 executable witnesses, zero `sorry`, `lake build`
green at v4.30.0 (143 jobs; the library's only `sorry`s are
`ScoringRuleV1.lean:761,766`, the SCORING-DEALER lane's). The compute and
lamport figures in §6 are DERIVED from cohort-15's measured transactions and
labelled provisional; nothing here was re-measured on an ELF. The companion
designs: BOND (a founder bond paid to holders on exhaustion) is what happens
after this mechanism fails; CONDITIONAL is orthogonal, at the Product layer.

**In one paragraph.** A market declares `k` sources under ONE window and a
quorum `q ≤ k`. Every source is captured independently inside the window by
whoever cares to, and each capture is a *fragment*: a kind-1 certificate,
authenticated by the source's own provider route exactly as today, written
into a per-member seat instead of the market's. After the window's closed
deadline, one permissionless fold reads the fragments, refuses fewer than `q`,
takes the MEDIAN of the readings on the material's one scale, and commits the
cell that median falls in through the selector every route already uses. Fewer
than `q` fragments hand the market to the funded recovery ladder (decision
0027), which is the fallback rung rather than a second mechanism. An attacker
or an outage has to take a majority of what was observed to move the cell;
nobody votes, nobody proposes, nobody posts a bond; and the single-source
market is the `k = q = 1` case, byte-identical to today because the two bytes
that declare an ensemble are the four reserved bytes `SourceMaterialV3` already
requires zero, and zero decodes to one source and one observation.

---

## 1. The spec shape

### 1.1 Two bytes on the material

`SourceMaterialV3` carries four reserved bytes at
`SOURCE_MATERIAL_V3_RESERVED_OFFSET` (`SourceMaterialV3Abi.lean:66`,
`⟨.reserved, .reserved 4⟩`; `source_material_v3.rs:23,113` requires them
zero). Two of them become

    members_minus_one : u8      k − 1
    quorum_minus_one  : u8      q − 1

so that the bytes every founded material already holds, `0, 0`, ARE `k = q = 1`
(`Spec.single`, `EnsembleResolutionV1.lean`) with no special case in the
decoder and no change to any content digest. `Spec.valid` is `1 ≤ q ≤ k ≤ 5`
(`single_is_valid`, `maxMembers_is_five`). The material is the owner because
it is where `recovery_policy` is declared and where the ensemble's members
live; the policy's five reserved header bytes were the alternative and were
rejected because a `k = 1` material has no policy at all.

### 1.2 The members are attempt slots; the rungs are the rest

A member needs exactly what a `RecoveryAttemptV2` already is: a source spec, a
provider release, a deadline, a funding allocation
(`source_recovery_policy_v2.rs:25-30`; Lean `Attempt`,
`SourceRecoveryPolicyV2Abi.lean:109`). So the ensemble's members are the
**leading `k − 1` attempt slots** of the market's `RecoveryPolicyV2`, and the
ladder's rungs are the slots after them:

    members spec policy = policy.attempts.take (k − 1)
    rungs   spec policy = policy.attempts.drop (k − 1)
    members ++ rungs = policy.attempts            -- members_and_rungs_partition_the_attempts

One list, one record, one funding ledger; the ladder's twelve theorems apply to
`rungs` verbatim because a `Ladder` is just a policy and a deadline
(`fallback`, `the_single_source_fallback_is_todays_ladder`). With
`RECOVERY_POLICY_MAX_ATTEMPTS_V2 = 4`:

| `k` | members (primary + slots) | rungs left for the ladder |
|---|---|---|
| 1 | the primary source | slots 0..3 — today's ladder, unchanged |
| 3 | primary, slot 0, slot 1 | slots 2, 3 |
| 5 | primary, slots 0..3 | none — fewer than `q` is the primary exhaustion into decision 0025's escrow |

(`the_members_fit_the_policy`: a valid spec never asks for a slot the policy
cannot hold.)

### 1.3 One window, and what makes a slot a member rather than a rung

A rung has its own deadline; a member does not. **A member's `deadline` MUST
equal the window's closed deadline `end + max_age`** — the same
`primary_deadline` the sponsored-push capture refuses past
(`sponsored_push_v1.rs:128-134`) — and founding refuses a policy whose leading
`k − 1` deadlines say otherwise (`membersShareTheWindow`). This is what keeps
`Policy.valid`'s `deadlinesIncreasing` meaningful across the boundary: every
member sits at the window's deadline, the first rung strictly after it, so the
ladder over the rungs is exactly as well-formed as today's.

Every member also shares the material's ONE `StatisticSpecV1`, hence one
`source_scale_exponent`. A member whose adapter publishes at another exponent
is refused at capture by `require_admitted_scale` → `ProviderScale = 0x801C`
(`OBSERVATION_SCALE_AUTHORITY.md`, "One selection site, one adapter rule,
three routes"). That is not a limitation the fold tolerates; it is the
precondition the fold is stated under (§5, hostile five).

### 1.4 Per-member funding, composed with the ladder's entries

Each member's `funding_allocation` names one funding-ledger row, exactly as a
rung's does (`funded.rs:179-239`, `plan_funding_release`). The difference is
what the row buys:

- **a rung's row pays a crank** — `FundingCompartment::Bounty`, released once,
  at the capability's own quote, to whoever turned it;
- **a member's row pays a capture** — the same compartment, released once, at
  the fold, to the captor whose fragment the fold consumed (the fragment's
  `refund_recipient`). A member never captured releases nothing, and its row
  stays `Active` until retirement (§4 (e)).

Whether the bounty is nonzero is ember's question 2 (§7). What is not optional
is the **fragment seat**: one prepaid, System-owned, zero-byte account per
member, at the certificate seat's own rent-exemption (312 B, **2,786,520
lamports** on cohort-15's cluster,
`COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md:366`), because a
member with no seat has nowhere to answer.

---

## 2. The capture protocol

### 2.1 A fragment is a certificate in a member seat

`ResolutionCertificateV2` already says everything a fragment must: kind `1`
(`ResolutionSuccess`), `market`, `generation`, `source_material`, the
`route` (provider release), `provider_evidence`, `attempt_index`,
`result_numerator` over `result_denominator = 1`, `selector`, `observed_at`
(`dclutch-resolution-codec/src/v2.rs:146`;
`validate_terminal_product`, `validate_shape`). A fragment is one of these
with `attempt_index = member` and `selector` = the member's own cell, in a seat
derived under a fragment domain:

    dclutch/ensemble-fragment/v1  ‖  source_state  ‖  [member]  ‖  terminal_sequence

The market's own terminal seat, `(RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
source_state, kind, terminal_sequence)` (`relay_transport_v1.rs:2171-2179`),
is untouched, so a `k = 1` market derives no fragment seat at all.

### 2.2 Each family's terminal route becomes a fragment route when `k > 1`

Three provider families commit a terminal today, each through its own route
and all through one contract transition,
`resolve_primary_from_authenticated_domain`
(`source_resolution_v2.rs:393`; callers `provider_v3.rs:317`,
`relay_v1.rs:362`, `sponsored_push_v1.rs:1207`). In an ensemble the SAME
route, with the SAME authentication, addressed to the member's seat, writes the
fragment and leaves the Source state on `Primary`:

| family | today (`k = 1`) | as a member (`k > 1`) |
|---|---|---|
| sponsored push (`sponsored_push_v1.rs`) | `Capture` writes a candidate and advances the head (103,810 CU); `Settle` after the deadline seals the head into the market's seat (140,902 / 146,902 CU) | `Capture` unchanged, with a member byte; `Settle` seals the head into the MEMBER's seat and does not move the Source state |
| Pyth v2 receiver (`provider_v3.rs`) | one transaction resolves the market | the same transaction writes the member's fragment |
| relayed mainnet state (`relay_v1.rs`, `ConsumeRecord`) | one transaction resolves the market | the same transaction writes the member's fragment |

The member byte rides the five bytes `SponsoredPushInstructionV1::decode`
requires zero at offsets 11..16 (`sponsored_push_v1.rs:165`,
`require_zero(bytes, 11, 5)`), so today's instruction bytes name member 0 —
the primary — and are byte-identical. `authenticate_sponsored_release`
(`:432`) authenticates the release against the primary source spec today; for
member `i > 0` it authenticates against `policy.attempts[i − 1]`'s
`source_spec_id` and `provider_release_id`, which is exactly the check the
recovery leg already states (`resolve_recovery_from_authenticated_domain`,
`source_resolution_v2.rs:748`, `LinkageMismatch`). **This is the owed
recovery-capture producer** (`LIVENESS_CENSUS_2026_08_29.md:96` R2, "the
honest recovery CAPTURE has a contract transition and no provider outer";
`GOAL.md:4738`): a capture route indexed by attempt slot is the same route
whether the slot is a member or the active rung.

The push family's head is already per-source: its PDA is seeded by the
sponsored release id (`sponsored_push_v1.rs:691-696`), so `k` push members
have `k` heads with no new seed, and a second capture of one source advances
that source's head rather than adding a fragment (§5, hostile two).

### 2.3 The fold

One new route, `EnsembleFold`, admissible strictly after the window's closed
deadline (the settle's own guard, `sponsored_push_v1.rs:860-863`), on a Source
state in `Primary`:

1. authenticate the market, material, window, statistic and Product runtime
   exactly as `process_settle` does (`:845-921`);
2. for each member `i < k`, derive the seat; a Resolution-owned 312-byte seat
   is decoded as a fragment and checked — kind `1`, this market, this
   generation, this material, `attempt_index = i`, `route` = the member's
   provider release, `observed_at` inside `[start, end + max_age]`; a
   System-owned zero-byte seat is a member that did not answer; anything else
   refuses `OutputState`. The fragment route's own write keeps the terminal
   write's all-zero conjunct (`relay_transport_v1.rs:2126-2130`), which is
   what makes a seat write-once;
3. count `n`; **`n < q` refuses `EnsembleQuorum`** (a new discriminant in
   band `0x8`, `programs/dclutch-resolution-proof-sbf/src/lib.rs:44`), because
   the admissible move is then the ladder's crank, not this route;
4. take the median of the `n` readings (§3) — five `i128`s at most, the
   rank-selection scan the scheduled-median evaluator already runs;
5. call `resolve_primary_from_authenticated_domain` with the median as the
   numerator, denominator `1`, the statistic's `source_scale_exponent` —
   unchanged, one call;
6. write the market's terminal certificate: kind `1`, `attempt_index = 0`,
   `schedule_index = 0`, `result_numerator` = the median, `selector` = its
   cell, `provider_evidence = hashv("dclutch/ensemble-evidence/v1", [n],
   fragment_0.provider_evidence, …)` over the consumed fragments in member
   order; and a fold receipt (`EnsembleFoldReceiptV1`, Lean-owned) naming the
   member bitmap, `n`, and each consumed fragment's digest;
7. release each consumed member's bounty to its fragment's captor (§1.4).

The terminal certificate's SHAPE is today's; the ensemble facts live in the
receipt and in `provider_evidence`. A reader holding the `n` fragments
recomputes the median and the cell from them, either way round
(`the_cell_of_the_median_is_the_median_of_the_cells`).

### 2.4 The fold and the crank are exclusive

After the deadline two routes could claim the same second: the fold and
`AdvanceRecovery` (`RelayActionV1::AdvanceRecovery = 7`, `crank_recovery_ladder`).
The crank's guard gains one conjunct — **`n < q`**, counted over the same
seats — and the fold's is `n ≥ q`, so from a closed window exactly one fires
(`the_fold_never_stalls`: its two arms are `n < q` and `q ≤ n`). A crank
against a market with a quorum refuses `EnsembleQuorumMet`. This is the
ensemble's copy of `a_closed_recovery_window_has_exactly_one_move`.

Two one-line changes make the ladder start at the first rung:
`crank_recovery_ladder`'s `SourceResolutionPhaseV1::Primary => 0_u8`
(`source_resolution_v2.rs:660`) becomes `=> k − 1`, and
`exhaust_after_primary_deadline`'s `material.recovery_policy().is_some()`
(`:527`) becomes "the policy has a rung". A `k = 5` market has no rungs and
exhausts on `Primary` like a no-policy market (`the_ladder_is_the_fallback`,
first arm).

---

## 3. The median rule, exactly

The fold is `SourceScheduledMedianV1.median?`
(`SourceScheduledMedianV1.lean`), not a second median: the scan tries each
reading and selects the one whose rank condition holds,

    below v ≤ ⌊n / 2⌋ < atMost v          (Selects, rank ⌊n/2⌋ zero-indexed)

where `below v` counts readings strictly less than `v` and `atMost v` counts
readings at most `v`.

- **Odd `n`.** The middle reading.
- **Even `n`.** The UPPER of the two middle readings: rank `n / 2` among `n`
  is the `(n/2 + 1)`-th smallest. The rank is inherited from the scan rather
  than chosen, so the tree has one median; the price of the even case is
  stated exactly in §4 (b) and the recommendation in §7 is an odd `k` and an
  odd `q`.
- **Ties.** Equal readings are one value at several ranks; `selects_unique`
  makes the answer one number whichever fragment carried it, and
  `median_permutation_invariant` makes it independent of member order.
- **Existence.** `median_some_of_nonempty` (new here, via `exists_selects`,
  quickselect's correctness stated over the scan's own predicate): every
  nonempty list has a median, so the fold is total on `n ≥ q ≥ 1`.

The readings are integers on ONE scale — the material's statistic puts every
member's observation on the result unit by one declared factor
(`4cd2b9cb5`), every certificate producer pins `result_denominator = 1`
(`provider_finalized_projection_v3.rs:632`) — and the cell is
`selectOrdinaryScaled ⟨median, 1⟩ scale`, the selector of `ProductRuntimeV2`
(`cellOf`). The fold touches the scale once, after the median.

---

## 4. The properties, with their proof status

**(a) `k = q = 1` is today.** `the_single_source_market_is_today`: one admitted
fragment folds to its own reading and `selectOrdinaryScaled ⟨reading, 1⟩
scale`, the value `resolve_primary_from_authenticated_domain` commits now; at
`Scale.identity` it is the pre-factor selector by the migration theorem
`selectOrdinaryScaled_identity`. At the byte level: the material's reserved
bytes decode `0, 0` to `Spec.single`; no fragment seat is derived; each
family's terminal route is unchanged; the push instruction's member byte is
one of the five bytes already required zero. `cohort15_market3_as_an_ensemble_of_one`
folds cohort-15 market 3's own reading (`10397222400` at `−8` against cuts
`10200, 10600` over `100`) to cell `1`, the cell the chain committed at
certificate offset 256. **Proven.**

**(b) Robustness, the exact bound.** Split the `n` folded fragments into honest
and manipulated with nothing assumed about the manipulated readings.

- `the_median_is_bracketed_by_an_honest_majority`: if `2·m < n`, some honest
  reading is at most the median and some honest reading is at least it. This
  is `SourceScheduledMedianV1.median_within_honest_range` with the odd-window
  hypothesis dropped — an odd `k` folds an even `n` the moment one source is
  dark, so the odd version is not enough. **Proven** for every `n`.
- `cellOf_mono` (from `selectOrdinaryFrom_mono`): the selector never falls as
  the reading rises. So the honest cells bracket the cell, and
  `an_attacker_below_the_bound_cannot_move_the_cell`: **if the manipulated
  fragments are fewer than half of what was observed and every honest reading
  falls in one cell, the fold decides that cell.** **Proven.**
- The bound is exact and the even case is asymmetric:
  `bracketed_below_by_at_most_half` holds at `2·m ≤ n`, the upper bracket only
  at `2·m < n`, and `exactly_half_can_move_the_cell_up_and_not_down` is the
  witness: two honest readings in cell `0`, two manipulated readings move the
  fold to cell `1` from above and cannot move it below the honest range from
  below. **Proven** (`native_decide`).
- In sources: `an_attacker_with_fewer_than_half_the_quorum_never_moves_the_cell`
  — a fold needs `n ≥ q`, so `2·m < q` is a strict minority of any fold that
  decides, whatever the honest outage pattern. With every honest source
  answering the same theorem holds at `k`. **Proven.** So the number of
  compromised sources that suffices is the smallest `m` with `2·m ≥ n`: with
  all `k` live, `⌈k/2⌉`; under the worst outage the fold accepts, `⌈q/2⌉`; and
  at exactly half only the upward move exists. The (k, q) table in §7 reads
  this off.

**(c) Manipulation cost.** A source operator controls the readings of the
sources it operates and nothing else: a fragment is written by the Resolution
program after the provider route authenticated the publication against the
member's pinned release (`12a9b13a5`), inside the window, on the declared
scale. There is no bonded proposal to out-bid and no vote to buy; the only
lever is a majority of the observed fragments, and (b) says fewer than that
moves nothing. The residual — an operator who controls fewer than the bound
can still WITHHOLD — is a liveness attack, and (d) says its worst outcome is
the ladder: `k − q + 1` withholding sources force the fallback rung (§7's
table), never a wrong cell. **Proof sketch; the theorem is (b).**

**(d) Liveness.** `fewer_than_the_quorum_engages_the_ladder` and
`a_quorum_always_decides`: a well-formed input under a positive quorum is
either fewer than `q` and the ladder, or at least `q` and decided —
`the_fold_never_stalls`, with no third arm. `the_ladder_is_the_fallback`: once
the window has closed, a market with rungs advances onto its first rung by
`Ladder.advance?` (the crank's transition) and a market with no rungs cannot
enter the ladder at all, which is the no-recovery market's own terminal.
Together: no fragment count and no rung count at which a market sits with a
closed window and nothing to do. **Proven.** The crank/fold exclusivity (§2.4)
is a route conjunct the build owes, stated here and not yet in Rust.

**(e) Conservation.** `k` seats and `k − 1` extra ledger rows are prepaid at
founding, and this is where each lamport ends:

| lamports | compartment / owner | law |
|---|---|---|
| a fragment seat, written | becomes the fragment account, Resolution-owned; closed at retirement to the Source state's `rentBeneficiary` by `close_to_beneficiary` (`relay_transport_v1.rs:2408`) as the terminal seat is | L6 rent conservation (`journey/src/ledger.rs:44`) |
| a fragment seat, never written | still System-owned and zero bytes; a `ReclaimMemberSeat` route, admissible only on a terminal Source state, moves it to the founding's recorded payer by a PDA-signed transfer. This retires the strand decision 0024 §2 item 4 records for the single seat too ("the certificate-seat prepay nothing reimburses") | L6, L7 |
| a member's bounty row, consumed | `FundingCompartment::Bounty`, released ONCE at the fold to the fragment's captor, at the row's own quote, by `release_in_place` (`funded.rs:218-226`) — the ladder's mechanism, not a second one | L7 lamport accounting, L8 per-class conservation |
| a member's bounty row, not consumed | stays `Active`; released with the ledger's remaining native principal at retirement (`authenticate_active_funding_ledger` → `remaining_native_lamports_total`, `resolution-core-v3-operator/src/lib.rs:3571`) | L7, L8 |
| a captor's float (push candidate 3,546,480 + head 2,938,512) | the captor's own, returned by `CloseCandidate` / `CloseHead` exactly as today | L6 |

"Each paid once" is structural: the fold runs once, because
`resolve_primary_from_authenticated_domain` refuses any phase but `Primary`
and the fold moves it to `Resolved`; a member's row is released only for a
consumed fragment; a member has one seat. **Law-level, not yet a theorem** —
the ledger census (`ledger.rs:1004-1012`) is the instrument, as for 0025.

---

## 5. The hostiles

Each names the refusing route, its discriminant, and the Lean witness.

1. **A fragment from a source not in the spec.** The fragment route refuses a
   member byte `≥ k` — `EnsembleMember`, new, band `0x8` — before deriving a
   seat; a seat for a member `≥ k` is not in the fold's frame at all.
   `a_fragment_from_a_source_not_in_the_spec_refuses`.
2. **Two fragments from one source.** One seat per member, and a seat is
   write-once: the write refuses `OutputState` unless every byte of the seat
   is zero (`relay_transport_v1.rs:2126-2130`), the conjunct the terminal
   write already carries. Note what does NOT guard it:
   `initialize_certificate_at_kind` accepts an already-owned well-formed seat
   (`:2184-2191`), because the terminal is guarded by the Source phase moving
   off `Primary`; a fragment moves no phase, so the seat's bytes are its whole
   guard and the fragment route must keep that conjunct. In the push family a
   second capture is a second candidate for the same head and advances it by
   the best-valid rule, never a second fragment.
   `two_fragments_from_one_source_refuse`.
3. **A fold with fewer than `q`.** `EnsembleQuorum`; the crank is the
   admissible move and the two are exclusive (§2.4).
   `a_fold_with_fewer_than_the_quorum_is_the_ladder`.
4. **A fragment outside the window.** Refused at capture — `ProviderFreshness`
   (`sponsored_push_v1.rs:134`) on the push leg, `DeadlineElapsed` on the
   recovery leg — and re-checked at the fold on each fragment's `observed_at`
   (`Fragment.admitted`). `a_fragment_outside_the_window_refuses` (past the
   deadline refuses; at the window's first second is admitted).
5. **A median over mixed scales.** Refused at capture — `ProviderScale =
   0x801C` when a member's adapter exponent disagrees with the material's one
   statistic — so the fold never sees two scales. What it would cost is the
   witness `a_median_over_mixed_scales_is_not_a_median`: cohort-15's cuts, three
   readings of `$103.74 / $103.75 / $103.80` fold to cell `1` on one scale, and
   to cell `0` when two of them arrive as cent mantissas: a reading not on the
   material's scale is not a smaller or larger price, it is a different number.

---

## 6. The price

### 6.1 Compute

Anchors, all cohort-15 (`COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md`),
devnet evidence: the push capture at **103,810** CU in-window (`:1392`); the
push settle at **140,902** (`:892`) and **146,902** (`:1729`) — and that
6,000-CU spread is exactly four `create_program_address` iterations at 1,500
(`DEALER_PARTIAL_REMOVE_COMPUTE_2026_09_02.md:55`), the bump-search luck of the
settle's PDAs. The ladder on real ELFs: advance 216,637 / exhaust 218,163 /
terminal 227,662 (`resolution_core_v3_lifecycle.rs:4086`).

| transaction | `k = 1` | `k = 3` | `k = 5` | status |
|---|---|---|---|---|
| capture, per member (push family) | 103,810 | 103,810 | 103,810 | measured; the member byte adds a comparison |
| fragment write, per push member (the settle addressed to the member seat) | — | ~140,902–146,902 | ~140,902–146,902 | derived: the same route, one seat derivation swapped for another |
| fold, upper bound: `settle(1) + k × 6,000` | — (no fold) | ≤ 158,902 / 164,902 | ≤ 170,902 / 176,902 | **provisional** |
| fold, per-fragment increment | — | 2,500–6,000 | 2,500–6,000 | derived: one seat derivation (1,500 × 1–3 iterations) + a 312-B fixed-layout decode and six equality checks |
| whole life, settle side, over `k + 1` transactions | 140,902 | ~580k–600k | ~875k–910k | derived |
| whole life, all routes | 244,712 | ~890k–915k | ~1.39M–1.43M | derived |

The fold's per-fragment cost is small because the fold parses NO publication:
each family's route did that when it wrote the fragment, under its own
authentication, and the fold trusts a Resolution-owned certificate the way the
terminal admission already does. The five-reading median is at most twenty-five
`i128` comparisons. **Lifting plan:** cohort-17 measures the fold at `k = 3`
and `k = 5` on the real ELF, which replaces the two provisional rows; the
frame grows by one seat per member (32 → 35 / 37 accounts), inside the lookup
table cohort-15 already uses.

### 6.2 Lamports

At cohort-15's funded rate, `(128 + len) × 6,333` (`COHORT15:2150`; the
cluster's Rent sysvar now reads 5,080 per byte-year, and decision 0030 fixes
the rate at funding time, so the figures below are what a market founded on
that cluster paid):

| item | bytes | lamports | who, and when it comes back |
|---|---|---|---|
| fragment seat, per extra member | 312 | **2,786,520** | founding; closed to the rent beneficiary at retirement, or reclaimed if never written |
| funding-ledger row, per extra member | +72 | 455,976 | founding; the ledger's rent, returned when the ledger closes |
| **prepay per extra member** | | **3,242,496** | |
| optional capture bounty, per member, at the funded-crank floor for a 312-B seat | | 2,786,520 | founding; paid to the captor at the fold or returned unspent |
| captor's float, per push capture (candidate 432 + head 336) | | 6,484,992 | the captor's; returned on close |
| transaction fee, per transaction at cohort-15's level | | 75,000 | spent |

The cost of robustness per market, founding-time prepay that comes back
minus fees that do not:

| | `k = 1` | `k = 3` | `k = 5` |
|---|---|---|---|
| seats + rows prepaid | 2,786,520 | 9,271,512 | 15,756,504 |
| of which additional to today | — | **6,484,992 (0.0065 SOL)** | **12,969,984 (0.0130 SOL)** |
| with a bounty per member | — | +8,359,560 | +13,932,600 |
| transactions (captures + fragment writes + fold) | 2 | 7 | 11 |
| fees at 75,000 | 150,000 | 525,000 | 825,000 |

Against cohort-15 market 3's whole life at **0.224581914 SOL** (`COHORT15:1356`),
`k = 5` with bounties is about **12%** more prepay, nearly all of it rent that
returns, and 675,000 lamports more in fees that do not.

---

## 7. The build

### 7.1 Reused, unchanged in meaning

- the ladder's attempt slots as the source list (`RecoveryPolicyV2`, its Lean
  and emitted twin), and its funding rows and `Bounty` release as the payment
  mechanism (`funded.rs:179-239`);
- the certificate as the fragment shape (`ResolutionCertificateV2`, its
  `validate_shape` / `validate_terminal_product`);
- every family's provider route as the fragment writer, and the push family's
  per-release head as the per-member head;
- `resolve_primary_from_authenticated_domain` as the one commit, with the
  median as its numerator;
- `SourceScheduledMedianV1`'s scan as the median, and
  `selectOrdinaryScaled` as the cell.

### 7.2 New, in order

1. **Lean and generated.** `SourceMaterialV3Abi`: the two ensemble bytes
   (`Spec`, emitted constants; the reserved-zero corpus row becomes the
   `Spec.single` row). `EnsembleFoldReceiptV1Abi` and its emitter. The Rust
   twins regenerated through `tools/genref/generate.sh --converge` by the
   convergence owner.
2. **Contract.** `SourceMaterialV3::ensemble()`; `RecoveryPolicyV2::members(k)`
   / `rungs(k)`; the founding conjunct `membersShareTheWindow`;
   `crank_recovery_ladder` entering `k − 1` from `Primary`
   (`source_resolution_v2.rs:660`) and the exhaustion refusal on "has a rung"
   (`:527`); `SourceResolutionStateV2::fold_ensemble_from_authenticated_domain`
   taking the `n` readings and calling the primary transition with their
   median. Hostiles named by discriminant.
3. **Resolution program.** Fragment mode on the three family routes (the
   member byte at push offset 11; the seat under
   `dclutch/ensemble-fragment/v1`); `EnsembleFold`; `ReclaimMemberSeat`; the
   crank's `n < q` conjunct; new codes `EnsembleMember`, `EnsembleQuorum`,
   `EnsembleQuorumMet` in band `0x8`, registered, `--check-unique` green;
   frameguard rows in the same commit.
4. **Operator and SDK.** A `devnet-ensemble-input-v1` producer for member
   captures (the push producer `devnet-sponsored-push-input-v1` with a member
   index); the fold's builder; the browser mirror
   (`packages/dclutch-sdk/lib/marketResolution.ts`,
   `ordinarySelectorV1.ts`) folding the `n` fragments and showing `k`, `q`,
   who answered and who did not — the disclosure half decision 0025 requires
   for the failure pathway applies to the ensemble's members too.
5. **Cohort-17.** Cohort-16 carries 0025/0027's founding changes and must not
   also carry a material-layout change. Cohort-17 founds one `k = 3, q = 2`
   SOL/USD market with the push feed, the Pyth v2 receiver and the relayed
   family as its three members and no rung; captures all three; measures the
   fold; then a second market with one member deliberately uncaptured, which
   must fold on two; then a `k = 3, q = 3` market with one member dark, which
   must crank onto a relayed rung. The harness campaign
   (`resolution_core_v3_lifecycle.rs`) gains the same three walks on real ELFs
   first.

### 7.3 The (k, q) table, and the two questions for ember

`m` is the number of manipulated fragments that suffices to move the cell (at
exactly half, upward only); "outages" is how many dark sources the fold still
decides through; "withholders" is how many colluding silent sources force the
ladder instead.

| `k` | `q` | `m`, all sources live | `m`, worst outage the fold accepts | outages tolerated | withholders to force the ladder |
|---|---|---|---|---|---|
| 1 | 1 | 1 | 1 | 0 | 1 |
| 3 | 2 | 2 | 1 | 1 | 2 |
| 3 | 3 | 2 | 2 | 0 | 1 |
| 5 | 3 | 3 | 2 | 2 | 3 |
| 5 | 4 | 3 | 2 | 1 | 2 |
| 5 | 5 | 3 | 3 | 0 | 1 |

**Question 1 — the flagship's `k` and `q`.** Recommended: **`k = 5, q = 3`**
where five independent releases exist for the statistic — it tolerates two
outages (cohort-13's Pyth receiver redeploy was one) while a single compromised
source never moves the cell, and two can only under a double outage — and
otherwise **`k = 3, q = 3` with a relayed first rung**, which makes any single
outage the ladder's problem rather than the fold's. Never an even `q`: the even
case's upward asymmetry is a real property, stated in §4 (b), not a rounding
choice. `k = 3, q = 2` is the cheap shape and its cost is exact — under one
outage a single source moves the cell.

**Question 2 — whose fee is the source operator's.** A push capture costs the
captor a 75,000-lamport fee and a returned float; a Pyth v2 pull costs whoever
posts the update its fee and the receiver's posting cost; the relay's cost is
the relayer's. Today nothing reimburses any of them and the honest capture is
unpaid (`work_paid: 0`, `sponsored_push_v1.rs:1261`). Option A keeps that: the
sponsor who wants a decided market captures `k` sources at its own expense.
Option B makes each member's row carry a bounty at the funded-crank floor
(rent-derived, never a literal, `FUNDED_CRANK_V1.md` §3), paid to the captor at
the fold — the same "cheap enough that a stranger with a stake turns it" that
ember's amendment to 0027 asked of the crank. Recommended: **B for a flagship,
A admissible for a cheap market**; the seat is mandatory either way.

---

## Evidence pointers

`formal/dclutch-semantics/DClutchSemantics/EnsembleResolutionV1.lean` (whole);
`SourceScheduledMedianV1.lean` (`median?`, `Selects`, `selects_unique`,
`median_permutation_invariant`, `median_within_honest_range`);
`ProductRuntimeV2.lean:184, 253` (`selectOrdinaryScaled_identity`,
`scaled_selection_in_one_cell`); `SourceResolutionStateV2Abi.lean` (the
ladder, twelve theorems); `SourceRecoveryPolicyV2Abi.lean:109, 132, 142`;
`SourceMaterialV3Abi.lean:61-74`;
`crates/dclutch-source-contract/src/source_resolution_v2.rs:393, 511-527,
619-700, 748`; `source_recovery_policy_v2.rs:25-30`;
`source_material_v3.rs:23, 113`;
`crates/dclutch-resolution-codec/src/v2.rs:146`;
`crates/dclutch-resolution-codec/src/sponsored_push_v1.rs:30-48, 117-124,
165, 178-207, 341-368, 533-563`;
`programs/dclutch-resolution-proof-sbf/src/sponsored_push_v1.rs:1-7,
120-134, 432, 691-696, 824-944, 1181-1236, 1248-1266`;
`programs/dclutch-resolution-proof-sbf/src/funded.rs:179-239, 354-470`;
`programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:2109-2130,
2161-2215, 2408`; `provider_v3.rs:317`; `relay_v1.rs:362`;
`crates/dclutch-relay-contract/src/instruction.rs:50-65`;
`crates/dclutch-capability-contract/src/funding.rs:426-441`,
`generated_abi.rs:81-82`;
`crates/dclutch-svm-harness/tests/resolution_core_v3_lifecycle.rs:4086`;
`tools/gauntlet/journey/src/ledger.rs:11-57, 1004-1012`;
`docs/decisions/0024` §2 items 4-5, `0025` §2-5, `0027` §2, §5-7, `0030`;
`docs/design/OBSERVATION_SCALE_AUTHORITY.md`, `FUNDED_CRANK_V1.md` §3,
`DEALER_PARTIAL_REMOVE_COMPUTE_2026_09_02.md:55`;
`docs/evidence/COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md:366,
892, 1362, 1392, 1729, 2150`; `LIVENESS_CENSUS_2026_08_29.md:96`;
`GOAL.md:4673-4678, 4738`; commits `4cd2b9cb5`, `485f5cb9f`, `12a9b13a5`,
`332b432e6`, `be8cac7b0`, `0b5e862ea`.
