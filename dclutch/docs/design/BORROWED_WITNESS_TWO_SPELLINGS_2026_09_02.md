# A borrowed witness has two spellings and one validator

*2026-09-02. Written by the Dealer lane after the post-trade partial equity
Remove cleared its heap wall at `43106855a` and refused `Custom(16421)` =
`0x4025 BorrowedWitnessRoute` at the wall behind it.*

## The refusal, localized to its operands

`hot_v3::require_borrowed_witness_coverage_v3` refused, and the diagnostic it
now prints on the refusing path says which of its three sites and with what:

```text
Program log: dclutch-hot:borrowed-witness
Program log: 0x3, 0x0, 0x4, 0x1, 0x0
```

Site 3: **borrower count 0**, route count 4, successor range count 1, tail
count 0. The function requires exactly one route whose `borrows_witness()` bit
is set. The effect has none, and it has one successor range.

## Two spellings, both first-party, both in this tree

The equity artifacts are built by two encoders that say "there is a borrowed
witness here" in two different ways, and the Trading validator knows one.

* `dealer/v3_artifacts.rs:502` — the V3 effect validator **requires** route 1
  to carry `borrows_witness()`.
* `dealer/v4_equity_release.rs:654-666` — the V4 encoder's own test asserts the
  opposite for every base route, in its own words: *"Effect V4 is the sole
  owner of every borrowed request range."* V4 carries the witness as
  `borrowed_range_count_for_route(1) == 1` and a `borrowed_range(0)` equal to
  the signed-delta suffix.

`require_borrowed_witness_coverage_v3` is the V3 rule. On a V4 effect it counts
zero borrowers and refuses, whatever the successor ranges say — and it has
already validated those ranges four lines earlier, through
`SuccessorProgramV4::validate_request_coverage`.

## Why nothing reached it until now

`dealer/v3_artifacts.rs:269-274`: when `signed_position_count == 0` the equity
request profile is emitted as **`RequestProfileV1`** and the function returns.
Only a non-zero signed-position count reaches
`encode_request_profile_v3_atomic` and the `BorrowedWitnessPolicyV3` beneath
it.

And `require_borrowed_witness_coverage_v3` returns `Ok` immediately unless the
request profile is `RequestProfileKindV3::Borrowed`. So:

| campaign step | signed positions | request profile | reaches the rule |
| --- | --- | --- | --- |
| LP Open, equity Add | 0 | V1 unsigned | no |
| post-trade partial equity Remove | 2 | V3 borrowed | **yes** |

Every equity action the campaign had ever executed carried zero signed
positions, so the first action that produced a signed-delta suffix was the
first to be checked at all. This is the producer-missing shape read from the
other end: the V4 producer exists and is exercised, the V3 validator exists and
is exercised, and the ONE combination that puts them in the same instruction
had never been run.

## The decision that is owed, and what it must not become

The rule wants a real property: **the witness the request profile declared is
consumed exactly once, by a route the policy admits, as that route's whole
request.** V3 expresses "consumed here" as a route bit; V4 expresses it as a
range bound to a route. The rule needs to hold over both spellings without
becoming a disjunction that either one satisfies alone — a route with a bit and
no range, or a range on a route the policy does not admit, must still refuse.

Three things a repair owes:

1. **The V4 arm stated positively**: exactly one route with exactly one
   borrowed range, that range equal to the profile's declared witness, that
   route's role equal to the policy's consumer role, `Once`, and carrying no
   request bytes of its own.
2. **Hostiles per conjunct**, on the real ELF, each naming
   `BorrowedWitnessRoute` or `BorrowedWitnessBytes` exactly — the two codes
   split out of `Content` at `43106855a` for this boundary. A bare `is_err()`
   here would pass on whatever refuses first.
3. **A statement of which spelling is canonical going forward.** Two spellings
   for one fact is how they stop agreeing; V4 already claims to be "the sole
   owner of every borrowed request range", and if that is true then
   `v3_artifacts.rs:502`'s requirement is the one that should be retired, in
   the same convergence cycle, rather than left as a second authority.

Not made here, because a validator that is loosened to make an integration test
pass is the thing this tree's own instructions forbid, and because the fix is
worth more than the wall: the campaign is 30/1 with this as its only failure.

## The ruling: V4 owns the borrow, and it was never a choice

*Appended by the Dealer lane, 2026-09-02, after building it and measuring it.*

### The section above named the wrong second author

`v3_artifacts.rs:502` is not a second author of the shipped fact. It validates
a DIFFERENT ARTIFACT: `v4_equity_release.rs:339-365` encodes a legacy V3 effect
locally, with `encode_dealer_equity_effect_program_v3`, and feeds it to
`authenticate_dealer_equity_artifacts_v3` as a geometry cross-check. Those bytes
are never sealed, never referenced by a descriptor, and never reach a
validator on chain. Requiring route 1 to carry `borrows_witness()` is correct
*of that artifact*.

The artifact that ships is `encode_dealer_equity_effect_base_for_v4`
(`v3_hot_artifact.rs:532`). Both come from one encoder,
`encode_dealer_equity_effect_program_with_claims_witness_v3`, differing in one
boolean, and that encoder's own doc comment states the ownership positively
already:

> The successor differs in one fact only: its Claims route does not borrow a V3
> witness because Effect V4 owns the complete SignedDelta suffix as an
> authenticated borrowed range.

The second author was in the RUNTIME, not in the release.

### Three landed rules force V4, and the third forbids the alternative outright

1. `dclutch-effect-kernel/src/v4.rs:803` — `validate_range_table` refuses a V4
   program **any** of whose base routes carries `borrows_witness()`, for every
   artifact, whatever its range count. A V4 successor spelling the borrow the
   V3 way is not merely discouraged; it is unrepresentable through
   `EffectProgramV4::decode`. `require_borrowed_witness_coverage_v3` requiring
   the bit under V4 was requiring a shape the kernel refuses to encode.
2. `dclutch-effect-kernel/src/v4.rs:600` — `validate_request_coverage` under
   `DisjointExactCoverage`. With signed positions the family request is header
   followed by the signed-delta suffix, so a Borrowed profile with zero ranges
   leaves `cursor != family_request_len`. The range must exist.
3. `hot_v3.rs::child_receipt_provenance_v4` already refused a route carrying
   both spellings, and `execute_core_route_v3` already read the V4 range in
   preference to the V3 witness, calling the latter "the zero-range legacy
   compatibility branch" in `BorrowedRouteRangesV4::family_request`'s own doc.

So the ruling is not a preference between two live spellings. **The V3 route bit
is retired on every V4 artifact, by the kernel, and the runtime's job is to say
so where the kernel's sweep cannot reach.**

### Where the kernel's sweep cannot reach, and why the rule restates it

`EffectProgramV4::from_sealed` takes `decode_shape` alone — decision 0005 argues
the artifact digest was already joined to the descriptor, so the table sweeps
"cannot have changed". `decode_sealed_effect_v4` is the SHIPPED Hot path. A
sealed record carrying the retired bit therefore reaches the route walk with
`validate_range_table` never having run over it, which is why
`require_borrowed_witness_coverage_v3` refuses `borrows_witness()` by name
(`BorrowedWitnessRoute`, site 4) instead of assuming the kernel got there first.

### Four readers of one fact, and three of them were V3-only

"What does this route borrow?" was answered independently in five places. Two
already read the V4 range; three read only the bit, and each was a wall behind
the last, found by running the route rather than by reading:

| reader | before | refused with |
| --- | --- | --- |
| `hot_v3::child_receipt_provenance_v4` | both, V4 preferred | -- |
| `core_composition_v3::prepare` | both, V4 preferred | -- |
| `hot_v3::require_borrowed_witness_coverage_v3` | V3 bit only | `0x4025` site 3, borrower count 0 |
| `claims_composition_v3::invocation_request` | V3 witness only | `0x4003` in the child walk, empty child request |
| `dclutch-claims-svm::composition_v3::claims_request` | V3 witness only | `0x4003` before `pf-enter`, empty child request |

The last two are one function apart and were reached in that order, each one
becoming visible only once the one before it stopped refusing. The composition
decoder's guards -- five `borrowed_witness.is_some()` conjuncts asserting "this
request kind does not borrow" -- now read a single resolved
`Option<&[u8]>` rather than the bit, so the count of readers went from five to
three rather than from five to seven.

### Measured

Real ELFs, own worktree, own target dir, frame diagnostics 0 on all six builds.
Campaign 30 passed / 1 failed, and the packet census of all 286 transactions --
route, legacy bytes, v0 bytes and unique locks -- is **byte-identical to HEAD's**.
No family's shape moved.

The post-trade partial equity Remove now:

* passes the borrowed-witness rule (the `dclutch-hot:borrowed-witness` log is
  gone from the run entirely);
* decodes its Claims composition, so the SignedDelta plan is the child request;
* **executes its first Custody route for real** -- 114,637 CU, a Token CPI
  inside it, a receipt returned;
* **reaches its Claims child** and dies there on COMPUTE, not on content.

Heap peak on the profile ELF: **58,568 of 65,536**, unchanged from `43106855a`
-- the range walk allocates nothing.

### The next wall, named exactly: the 1.4M transaction ceiling

`Program failed to complete`. Trading consumed **1,399,692 of 1,399,700**; the
Claims child was entered with **94,426 CU** and exhausted the meter at 94,418.
The same SignedDelta Claims child costs **175,259 to 188,786 CU** at its nine
other invocations in this same run, so the shortfall at that point alone is
about 81,000.

The measured decomposition of the transaction, in order:

| phase | CU |
| --- | ---: |
| Trading, entry to the accelerator CPI | 506,450 |
| accelerator child | 422,757 |
| Trading, accelerator return to Custody route 0 | 219,730 |
| Custody route 0 (with its Token CPI) | 114,637 |
| Trading, Custody return to Claims route 1 | 41,700 |
| Claims route 1 | 94,426 given, >=175,259 needed |

Routes 2 and 3 -- two more Custody legs -- and the whole commit phase are still
unreached. At the observed per-leg cost that is a further ~313,000 CU before
the commit, so the action as composed needs on the order of **1.9M CU against a
1,400,000 ceiling**. This is not a budget that can be raised: 1,400,000 is the
chain maximum, and `set_compute_max_units` in `program-test` is not an
instrument for measuring past it -- raising it makes the bank ignore the
transaction's ComputeBudget instructions entirely, including the heap request,
and the run then faults writing at `0x30000ff68` with a 32 KiB grant.

### What is still owed

* **The CU wall above.** The equity Remove with two signed positions does not
  fit in one transaction. Nothing in this series makes it fit.
* **The legacy twin.** `finalize_dealer_equity_descriptor_v3` has zero callers;
  `encode_dealer_equity_effect_program_v3` and the `legacy_claims_witness`
  boolean exist only to feed `authenticate_dealer_equity_artifacts_v3`, whose
  route-1 requirement is the retired spelling. That authenticator is currently
  the only route-geometry check the equity release makes, applied to a twin
  that differs from the shipped base in exactly the bit under dispute. Pointing
  it at the shipped base -- with route 1 required to carry the V4 range rather
  than the bit -- retires the twin, the boolean and the second encoder in one
  host-side commit that owes no frame rows.
* **`hot_v3.rs:11267-11268`**, found not fixed and older than this series: the
  Claims preflight arm binds `composition` and `selected` and uses neither,
  while the comment above them describes three conjuncts being distinguished.
  Only `invocation_index != 0` remains.
