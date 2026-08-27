import Std.Tactic

/-!
# AccountProfile V2 Profile 13 zero-span canonicality

Profile 13 admits a fixed-topology artifact only through one exact empty-span
shape: the span count, repeated item-account stride, repeated item operations,
and span-rule template count are all zero.  This small assurance model mirrors
the safe Rust decoder/encoder boundary introduced by `bd57e39`; it does not
print or replace the executable AccountProfile interpreter.
-/

namespace DClutch.AccountProfileV2Profile13

def artifactProfile : Nat := 13

structure ZeroSpanShape where
  spanCount : Nat
  itemAccountStride : Nat
  itemOperationCount : Nat
  spanRuleCount : Nat
  deriving DecidableEq, Repr

def canonical (shape : ZeroSpanShape) : Bool :=
  shape.spanCount == 0 &&
  shape.itemAccountStride == 0 &&
  shape.itemOperationCount == 0 &&
  shape.spanRuleCount == 0

def fixedTopology : ZeroSpanShape := {
  spanCount := 0
  itemAccountStride := 0
  itemOperationCount := 0
  spanRuleCount := 0
}

def hostileZeroSpanCorpus : List ZeroSpanShape := [
  { fixedTopology with itemAccountStride := 1 },
  { fixedTopology with itemOperationCount := 1 },
  { fixedTopology with spanRuleCount := 1 },
  { fixedTopology with spanCount := 1 }
]

theorem fixed_topology_is_the_exact_zero_span_shape (shape : ZeroSpanShape) :
    canonical shape = true ↔ shape = fixedTopology := by
  rcases shape with ⟨spanCount, itemAccountStride, itemOperationCount, spanRuleCount⟩
  simp [canonical, fixedTopology, and_assoc]

theorem fixed_topology_is_canonical : canonical fixedTopology = true := by
  native_decide

theorem hostile_zero_span_corpus_refuses :
    hostileZeroSpanCorpus.all (fun hostile => canonical hostile = false) := by
  native_decide

end DClutch.AccountProfileV2Profile13
