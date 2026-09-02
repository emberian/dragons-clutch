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
