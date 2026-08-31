# Direct Hot: what AOT actually costs and actually saves

2026-08-31. Measurement lane. No protocol change.

AOT has been the standing long-term answer to "how much CU do our transactions
take", and the mass anatomy in `DIRECT_HOT_CU_VARIANCE_CENSUS_2026-08-30.md`
priced it at 466,615 CU, 33.9% of route mass, marked *never measured on this
route -- biggest unknown*. It had never been measured in either direction. This
document measures it in both, on real SBF ELFs, and reports what the number
does and does not license.

The short version, and it is not the answer the framing predicted:

- Replacing the interpreter with the ahead-of-time translation on the relation
  the live route runs saves **10,393 CU per invocation**. That is **29.5% of the
  35,274 CU fee-bearing overage** and **0.83% of the route floor**.
- The TransitionVM's share of the 466,615 CU "AOT lever" is **zero**. That
  bucket is a different interpreter, plus projection and marshalling that no
  transition AOT touches. The lever is misnamed.
- The AOT side **cannot be built for SBF today**. The crate does not compile for
  `target_os = "solana"`, so this measurement required a local two-line change
  that is deliberately not committed.

## 1. What AOT machinery actually exists

Before measuring, the ground truth, because the naming misleads in three ways.

**"AOT" is not a compiler.** `crates/dclutch-direct-aot-contract` and
`-v3-contract` are hand-written straight-line Rust re-implementations of the
relations. They never read the emitted program bytes: `DIRECT_PROGRAM_V2` sits
in the same crate but `execute_atomic` does not consult it. The two are held
together by differential tests over a fixed corpus, not by construction. Every
claim below about "the AOT path" is a claim about hand-written Rust asserted
equal to an emitted program, and that distinction is the whole of its risk.

**The deployed accelerator accelerates a superseded descriptor.**
`programs/dclutch-direct-aot-sbf` builds a real 26,832-byte ELF and evaluates
the Direct **V2** descriptor: 35 instructions, 856 bytes. It is not on the live
route; its only consumers are its own crate and program. WAVE.md already names
this "the Direct AOT inversion".

**The current translation has no ELF at all.**
`crates/dclutch-direct-aot-v3-contract` holds the translations the route would
need and nothing in the tree builds it into a `.so`. Section 6 shows it *cannot*
be built into one as committed.

Separately, `programs/dclutch-series-shadow-sbf` is a different Shadow-AOT
boundary for Series, blocked on a certificate its README states is
unconstructible under the shipped typing, shipping an empty fail-closed ELF
because `DCLUTCH_SERIES_SHADOW_GENERATED_INCLUDE` is never set. That is
SER-ACCEL's finding and a different route; the two must not be conflated.

## 2. Method

The comparison is only worth anything if nothing but the evaluator differs. So
one twin crate is built four times from identical source, selected by feature,
and only `evaluate_relation` changes:

| build | evaluator |
| --- | --- |
| `aot` | the hand-written straight-line translation |
| `interpreted` | `ProgramV3::decode` then the generic fold |
| `decode-only` | the same decode, then stop |
| `null` | the surrounding work, relation removed |

Bank construction, transition encode, digesting and return are shared verbatim,
so a difference between the ELFs is the evaluator and nothing else. The `null`
and `decode-only` builds exist so the headline can be decomposed rather than
quoted as a blend that flatters whichever side you prefer -- and section 4 shows
that decomposition changing the answer by a factor of 2.3.

Harness: `tools/gauntlet/aot-cu/`. CU from
`process_transaction_with_metadata(...).metadata.compute_units_consumed` under
`solana-program-test` with `prefer_bpf(true)`, against `cargo-build-sbf` ELFs --
no native or mock processor fallback.

**Faithfulness control.** The shipped `dclutch_direct_aot_sbf.so` is loaded
alongside the V2 twins and must agree with the AOT twin on every acknowledgement
byte for every seed. It does, and its CU tracks the twin within 1-2 CU
throughout. Its accepted cost of 2,650 CU reproduces
`DIRECT_FAMILY_CAMPAIGN_2026_08_27.md` exactly (2,650-2,658 accepted,
1,401-1,726 refused). That number was therefore *not* previously unmeasured;
what had never been measured is the comparison.

**Seeds: 32, deterministic, not sampled naively.** The relations are long
conjunctions -- `fill == maximum` under FOK, `nonce == next_nonce`, three equal
fee rates, an exact `fill * price / scale`. Independently sampled banks refuse
almost immediately. A first run of this harness produced 31 refusals out of 32
and would have priced the cheap early-exit path while claiming to price the
route. Seeds 1..24 are therefore admissible *by construction*; seeds 24..32 each
violate exactly one conjunct at a different depth, because refusal depth is
where an interpreter loses most and pricing only acceptances would flatter AOT.

Reported as floor plus tail, never a worst-seed sample.

## 3. The superseded descriptor (Direct V2, 35 instructions)

Included because it is the only relation with a shipped ELF, which is what makes
the twin frame trustworthy. All 32 seeds: acknowledgement bytes identical across
the shipped ELF, the AOT twin and the interpreted twin -- on-ELF refusal
equivalence, previously shown only on host.

| | floor | median | max | tail |
| --- | --- | --- | --- | --- |
| shipped accelerator, all seeds | 1,401 | 2,650 | 2,650 | +1,249 |
| twin AOT, all seeds | 1,402 | 2,652 | 2,652 | +1,250 |
| twin interpreted, all seeds | 5,938 | 10,308 | 10,310 | +4,372 |
| twin AOT, accepted (24) | 2,652 | 2,652 | 2,652 | **+0** |
| twin interpreted, accepted (24) | 10,308 | 10,308 | 10,310 | +2 |
| twin AOT, refused (8) | 1,402 | 1,447 | 1,926 | +524 |
| twin interpreted, refused (8) | 5,938 | 7,276 | 9,280 | +3,342 |

Accepted-path decomposition at the floor: shared frame 1,927 CU, AOT evaluator
**725**, interpreted evaluator **8,381**, saving **7,656** (11.6x). The AOT cost
is flat at 2,652 across all 24 admissible seeds -- tail +0, genuinely
key-independent -- while the interpreter varies by 2 CU.

This is a whole-decode figure and, per section 4, is not the shape the route
pays. It stands as a controlled comparison, not as a route claim.

## 4. The relation the route actually runs, and the correction that matters

The live route executes exactly **one** TransitionVM program per invocation:
`process_hot_execution_v3` → `execute_authenticated_hot_v3` →
`execute_interpreted_transition_v3` → `execute_fold_atomic`
(`programs/dclutch-trading-sbf/src/hot_v3.rs:3874`). The AOT accelerator arm at
`hot_v3.rs:2725` is not taken: the Direct strategy disposition is `Interpreted`,
pinned at `crates/dclutch-direct-codec/src/ordinary_artifacts_v3.rs:96`. The
second apparent call site at `hot_v3.rs:12605` is inside `#[cfg(test)]`. No
child role links the VM at all.

The fold does **not** walk all 70 emitted instructions. Per
`crates/dclutch-transition-vm/src/v3.rs:748-777` it runs prelude, then the item
range once per Product outcome, then epilogue: 66 + `tail_count` x 3 + 1. The
gate fixture's `tail_count` is the canonical three outcomes, so **76
dispatches**, growing +3 per additional outcome and not with fills or child
routes.

Measured at `N = 3`, 32 seeds, all agreeing on disposition and output-bank
digest between the two evaluators -- the first on-ELF equivalence evidence for
the current relation:

| | floor | median | max | tail |
| --- | --- | --- | --- | --- |
| AOT, all seeds | 14,733 | 16,543 | 16,551 | +1,818 |
| interpreted, all seeds | 28,884 | 40,417 | 40,433 | +11,549 |
| AOT, accepted (24) | 16,364 | 16,543 | 16,551 | +187 |
| interpreted, accepted (24) | 40,238 | 40,417 | 40,433 | +195 |
| AOT, refused (8) | 14,733 | 14,862 | 14,902 | +169 |
| interpreted, refused (8) | 28,884 | 31,513 | 32,481 | +3,597 |
| decode only, no fold | 28,760 | 28,939 | 28,947 | +187 |
| surrounding work, no relation | 15,279 | 15,458 | 15,466 | +187 |

Accepted-path decomposition at the floor:

| component | CU |
| --- | --- |
| shared bank build and transition encode | 15,279 |
| AOT evaluator alone | **1,085** |
| interpreted, decode plus fold | 24,959 |
| — of which full decode (shape + `validate_body`) | 13,481 |
| — of which the fold alone | **11,478** |

**The correction.** A naive reading takes the interpreted evaluator at 24,959
CU and claims a 23,874 CU saving. That is wrong, and it overstates AOT by a
factor of 2.3. The route calls `TransitionProgramV3::from_sealed`
(`hot_v3.rs:2250`), which skips `validate_body` -- the per-instruction sweep,
and by measurement the expensive 13,481 CU half of `decode` -- because the
write-once, permissionless seal instruction (`hot_v3/seal.rs:569`) already ran
it. The gate fixture stages that seal already written, so **none of the
1,252,751 CU floor is `validate_body`.**

What the route pays per invocation is the cheap constant shape decode plus the
fold. So the honest comparison is the fold against the AOT translation:

> **11,478 CU interpreted → 1,085 CU AOT. Route-relevant saving: 10,393 CU.**

That is **151 CU per dispatch** over 76 dispatches.

Two costs AOT does *not* remove, worth naming because they look like they should
be in the saving and are not: the program bytes are account-supplied and
SHA-256'd over all 1,712 bytes on every invocation (`hot_v3/seal.rs:783`)
whether or not the fold is interpreted; and the 15,279 CU of shared work above
is a harness convenience (encoding the program bytes locally), not a route cost,
which is why it is subtracted rather than reported.

`registered_fill_v4` (112 instructions) is a separate capability and is **not**
on this route, so no projection to it is made here.

## 5. What this does and does not license

Route anchors, verified at HEAD rather than taken on trust:
`TOP_LEVEL_KEY_INDEPENDENT_CU_V1 = 1_254_251` is asserted at
`programs/dclutch-trading-sbf/program-test/tests/direct_hot_top_level_margin_gate.rs:260`,
with the recorded floor 1,252,751 and the fee-bearing bound 1,435,274 exceeding
its ceiling by 35,274.

**The 33.9% lever does not exist as described.** Reading the code each census
bucket brackets:

| bucket | CU | what the bracketed code actually does |
| --- | --- | --- |
| effect projection | 164,289 | `dclutch_effect_kernel::v4::project_atomic_visiting` -- a **different interpreter**, over EffectProgram V4 bytes, 131 fixed effect operations. Not the TransitionVM. |
| register projection | 152,204 | AccountProfile, RequestProfile and lifecycle-policy projection -- other interpreters over other artifacts. |
| runtime observations | 95,173 | `try_borrow_data` over ~57 accounts plus key derivation. No program interpretation at all. |
| composition | 54,949 | child CPI frame construction. |

**The TransitionVM contributes zero CU to the 466,615.** Its fold sits inside a
different, unlisted bucket -- "lifecycle preplan, candidate, replan", 74,265 CU,
5.4% -- bracketed by `hot_cu_checkpoint!("request-lifecycle-preplan")` at
`hot_v3.rs:2723` and `hot_cu_checkpoint!("candidate")` at `:2778`, which also
contains both `prepare_lifecycle_v4` passes. The census's parenthetical
"(transition VM / effect kernel)" against effect projection is misleading. The
bucket that motivated this lane is the *neighbourhood* of a lever, not the
lever, and the census framing should be corrected.

Our own measurement is consistent with that: 11,478 CU of fold sits comfortably
inside the 74,265 CU bucket alongside the two preplan passes.

**Verdict on the fee-bearing gap: helpful, not sufficient.** 10,393 CU is 29.5%
of the 35,274 CU overage. It is a real, honestly measured saving on a relation
whose translation and differential tests already exist, and it is the largest
single identified saving for that gap. It does not close it alone, and anyone
planning around "AOT closes the fee-bearing gap" should stop.

**Verdict on the general margin: no.** 10,393 CU is 0.83% of the route floor. At
that rate a route-wide translation programme cannot pay for itself. The general
margin has to come from the other 99%, which this analysis says is effect-kernel
interpretation, profile projection, account marshalling and CPI composition.

**Where the real AOT lever is, if one is wanted.** The 164,289 CU effect
projection bucket *is* an interpreter -- the effect kernel, walking 131 fixed
operations over EffectProgram V4 bytes. It is 14x the TransitionVM fold and
nobody has measured what an ahead-of-time form of it would cost. If "AOT" is to
be the answer to the CU question, that is the thing to point it at, and this
lane's harness generalises to it directly. That is the single highest-value
follow-up here.

**The complexity price is one translation per relation.** The interpreter is
generic: one decoder serves every emitted descriptor and the Lean-emitted bytes
remain the authority. Each AOT translation is hand-written Rust that must be held
equal to its program by differential testing forever, and the current crate
already carries one recorded admission disagreement (P5g). ELF cost measured
here: V2 AOT +8,800 bytes over the null build against the interpreter's +14,872;
V3 AOT +10,760 against +19,216. A single relation is cheaper AOT; that inverts
as soon as several relations each carry a translation the one interpreter would
have served.

## 5a. Which U-014 columns this closes

U-014 asks for "exact Direct equivalence/certificate, Registry-bound
artifact/toolchain, refusal equivalence, rollback, CU, packet, and rent
comparison". This lane closes some columns and leaves the rest open; it does not
close U-014, and the shared index is deliberately not edited here because
several lanes hold that table.

| column | state after this lane |
| --- | --- |
| refusal equivalence | **closed on real ELFs** for both relations, 32 seeds each: identical acknowledgement bytes (V2), identical disposition and output-bank digest (V3). Previously host-only. |
| CU comparison | **closed** for V2 and for the InlineOrdinary V3 fold, with the route-relevant decomposition. Not attempted for `registered_fill_v4`, which is off-route and unbuildable. |
| rent | **partial**: ELF bytes measured (V2 AOT +8,800 / interpreted +14,872; V3 AOT +10,760 / interpreted +19,216 over the null build). Persistent account rent is zero for the stateless boundary, which holds no account. |
| packet | not advanced; the accelerator README's 584-byte request, 616-byte ack and 756-byte v0 transaction stand, unremeasured. |
| rollback | not measured. Both evaluators contract to leave output unchanged on refusal and host differential tests assert it; not tested on ELF here. |
| equivalence certificate | not advanced. Still blocked as the accelerator README describes. |
| Registry-bound artifact/toolchain | not advanced, and further away than assumed: section 6. |

## 6. Why the AOT column cannot be reproduced from a bare checkout

`dclutch-direct-aot-v3-contract` does not compile for `target_os = "solana"`.
Its `registered` module imports the V4 register schema unconditionally
(`src/registered.rs:3`), but that schema is published only off-chain:

```rust
// crates/dclutch-direct-codec/src/registered_fill_artifacts_v4.rs:39-40
#[cfg(not(target_os = "solana"))]
pub use crate::generated_registered_fill_v4::*;
```

`cargo build-sbf` therefore fails with 175 unresolved `FILL_SCALAR_*_V4` and
`FILL_IDENTITY_*_V4` names. A host `cargo check` passes, which is why this has
gone unnoticed: **the current Direct AOT translation has never been compiled
for the target it exists to run on.**

The whole of the fix, applied locally and deliberately not committed, since it
is a protocol-crate change and this is a measurement lane:

```rust
// crates/dclutch-direct-aot-v3-contract/src/lib.rs
#[cfg(not(target_os = "solana"))]
mod registered;
#[cfg(not(target_os = "solana"))]
pub use registered::execute_registered_ordinary_fill_atomic;
```

With those two lines all four V3 ELFs build. Section 4's numbers were produced
that way.

### Sized charter, if the 10,393 CU is judged worth having

1. **Gate or port the V4 schema.** Two lines gate `registered` off-chain, which
   unblocks `ordinary_v3` -- the on-route relation -- immediately but leaves
   `registered_fill_v4` permanently host-only. Publishing the V4 schema on-chain
   is the larger option. For *this* route the cheap fix is sufficient, and the
   decision only matters if the registered fill capability is later wanted on
   chain.
2. **Add an SBF program crate for the current translation**, mirroring
   `programs/dclutch-direct-aot-sbf` -- roughly 190 lines, that crate's exact
   shape, plus a registered refusal band.
3. **A build that fails loudly.** This defect survived because no CI job builds
   the crate for SBF. One `cargo build-sbf` line closes that class.
4. **Selection.** Per U-014 and the accelerator README, AOT-only execution stays
   unavailable until Registry owns descriptor/certificate/artifact admission.
   Nothing above changes that; it makes the artifact exist so the admission work
   has something to admit.
5. **Retire or re-point the deployed V2 accelerator**, which accelerates a
   descriptor the route no longer runs.

Weigh that against 10,393 CU, 0.83% of floor, before starting.

## 7. Limitations, stated plainly

- **The route floor was not re-measured.** The eight-ELF set in `target/deploy`
  is stale against HEAD (trading's ELF predates commits to
  `programs/dclutch-trading-sbf` and `crates/` made during this session), and
  rebuilding it immediately after the machine OOM'd, in a tree roughly ten lanes
  were committing to, would have produced a floor reflecting other lanes'
  half-landed work rather than HEAD. Anchors are cited from the in-code gate
  constant and prior evidence. Every conclusion here is a difference and is
  robust to the floor moving by a few thousand CU.
- **The evaluator was measured in isolation, not in the route.**
  `DIRECT_HOT_CU_VARIANCE_CENSUS` warns that changing a role's source redraws
  every bump depth, so an in-route A/B must be a host-side fixture switch
  against one ELF set. This measurement deliberately avoids that confound and
  therefore excludes whatever second-order bump-depth effects an in-route swap
  would have.
- **The fold's cost is bounded but not isolated inside the route.** 11,478 CU is
  measured standalone; within the route it sits in the merged 74,265 CU
  preplan/candidate/replan bucket. Splitting that bucket is a one-command job
  with the existing `hot-cu-profile` instrument (`hot_v3.rs:342-348`,
  `sol_log_compute_units`) by adding a checkpoint immediately before
  `hot_v3.rs:2765`. Not done here; it would confirm the standalone figure
  transfers.
- **`N = 3`.** Both columns scale with Product tail width (+3 dispatches per
  outcome). The ratio should be steadier than either absolute figure; untested.
- The AOT translations are hand-written and asserted equal by differential tests
  over a fixed corpus. Those tests say nothing about inputs outside it.

## Reproducing

`tools/gauntlet/aot-cu/README.md` carries the exact build and run commands for
both measurements, including the local patch the V3 AOT column requires.
