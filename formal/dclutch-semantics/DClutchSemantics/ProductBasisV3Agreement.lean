import DClutchSemantics.ProductBasisV3
import DClutchSemantics.LiabilityBasisV2

/-!
# Where the live evaluator and the V2 kernel provably agree, and where they cannot

"Prove the two evaluators agree" is ill-posed AT THE WIRE, and saying so
precisely is most of this file's value.

No byte string decodes on both. `ProductBasisV3` requires magic `DCLTPAY3` and
a length of at least 256 with the header's `record_bytes` equal to the buffer;
the kernel's spline request requires magic `DCLTLBV2` and a length of exactly
144. Disjoint magics AND disjoint lengths: the intersection of their domains is
empty. Nor does either implement the other's core notion -- the live evaluator
has no concept of degree at all, and the kernel has no shape enum, no per-term
amplitude and no failure-payout vector. Where a shared notion does exist the
types diverge (`i128`/`u64` against `i64`/`u32`) and, decisively, so do the
rounding rules: the live evaluator floors each term and hands the complement
`Q - sum(primary)`, while the kernel telescopes differences of consecutive
floors of a running sum. Those are different functions even on inputs both
could express.

So a blanket agreement theorem would have to be either false or vacuous. What
is neither is the fragment where a genuine DEFINITIONAL bridge already exists:
the capped ramp at degree 1. `LiabilityBasisV2.cappedRampComplementFloorBoundaryV2`
is *defined* as `ProductV2.interpolationFloor`, and the live evaluator's sole
rounding boundary is that same floor at the live wire's widths. On that
fragment the two agree exactly, and this file proves it.

Everything else is scoped as unreachable rather than asserted equal. That
distinction is the point: an unproved claim and a claim with no domain to be
stated over are different failures, and only the second one is honest here.
-/

namespace DClutch.ProductBasisV3Agreement

open DClutch

/-- **The bridge.** In the strict interior of a ramp -- the only place either
evaluator rounds at all -- the live evaluator's boundary and the V2 kernel's
capped-ramp apportionment boundary are the same number.

The live side takes `Nat` widths because the live wire's coordinates are
already reduced to a nonnegative elapsed and span by that point; the kernel
side takes `Int` because its defensive clamp is stated over the whole line. The
clamp is inert here, which is exactly what
`cappedRampComplementFloorBoundaryV2_interior` establishes. -/
theorem live_boundary_is_the_kernel_boundary_in_the_interior
    (scale : Nat) (elapsed width : Int)
    (positiveElapsed : 0 < elapsed) (interior : elapsed < width) :
    ProductBasisV3.interpolationFloor scale elapsed.toNat width.toNat
      = LiabilityBasisV2.cappedRampComplementFloorBoundaryV2 scale elapsed width := by
  rw [LiabilityBasisV2.cappedRampComplementFloorBoundaryV2_interior scale elapsed width
        positiveElapsed interior]
  obtain ⟨e, rfl⟩ : ∃ e : Nat, elapsed = (e : Int) :=
    ⟨elapsed.toNat, (Int.toNat_of_nonneg (by omega)).symm⟩
  obtain ⟨w, rfl⟩ : ∃ w : Nat, width = (w : Int) :=
    ⟨width.toNat, (Int.toNat_of_nonneg (by omega)).symm⟩
  simp only [ProductBasisV3.interpolationFloor, Int.toNat_natCast, ← Int.natCast_mul]
  rw [Nat.mul_comm e scale]
  rfl

/-- The agreement is not vacuous: the interior is reachable, and here is a
witness the corpus also carries. At scale 100, one tenth elapsed of a span of
ten, both sides answer 10. Without this, the theorem above could hold for want
of any instance satisfying its premises. -/
theorem the_bridge_has_an_instance :
    ProductBasisV3.interpolationFloor 100 1 10
      = LiabilityBasisV2.cappedRampComplementFloorBoundaryV2 100 1 10
    ∧ ProductBasisV3.interpolationFloor 100 1 10 = 10 := by
  constructor <;> native_decide

/-- And the bridge carries the rounding DIRECTION across, not merely the value.
Neither evaluator ever apportions a primary claim more than its exact rational
share, which is the property that makes flooring safe to pair with an exact
complement. -/
theorem the_shared_boundary_never_rounds_up
    (scale : Nat) (elapsed width : Int)
    (positiveElapsed : 0 < elapsed) (interior : elapsed < width) :
    (ProductBasisV3.interpolationFloor scale elapsed.toNat width.toNat : Int) * width
      ≤ (scale : Int) * elapsed := by
  rw [live_boundary_is_the_kernel_boundary_in_the_interior scale elapsed width
        positiveElapsed interior]
  exact LiabilityBasisV2.cappedRampComplementFloorBoundaryV2_never_rounds_up
    scale elapsed width positiveElapsed interior

/-! ## What is NOT proved here, stated so nobody mistakes silence for coverage

The theorems above cover the degree-1 capped ramp and nothing else. Each of the
following is outside the bridge for a structural reason rather than for want of
effort, and none of them should be given an agreement statement without first
giving it a domain:

* **Degree 2 and 3.** The live evaluator has no notion of degree, so there is
  no live side to the equation. This is what the wire half of
  `BASIS_ABI_UNIFICATION_V1` exists to change.
* **`Tent` and `Constant`.** The kernel has no shape enum; these have no
  kernel counterpart to agree with.
* **Failure payouts.** The kernel has no concept of one.
* **Widths above 10, or knots the kernel's `i64` cannot hold.** The live wire
  is strictly more capable, so the kernel is undefined there rather than
  different.
* **Whole-partition agreement at any width.** The rounding rules genuinely
  differ away from a single boundary: per-term floor plus exact complement is
  not cumulative-floor telescoping. Adopting the kernel's rule would move the
  money on every existing shaped basis, which is why the ruling keeps the live
  rule and ports the kernel's ALGORITHM under it rather than the reverse.
-/

end DClutch.ProductBasisV3Agreement
