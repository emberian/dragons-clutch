import DClutchSemantics.RationalRepresentationV2

/-!
# Executable exact-representation examples

These examples exercise the accepted explicit-remainder design and hostile DAG
substitutions.  They are theorem-regression evidence for the pure definitions,
not evidence for Token-2022, Record, CPI, SBF, or transaction rollback.
-/

namespace DClutch.RationalRepresentationV2.Examples

open DClutch.RationalRepresentationV2

def coordinate0 : StructuredCoordinate := {
  coefficient := 3
  nativeLocked := 3
  shardSupply := 30
  structuredCustody := 21
  explicitFreeShards := 9
}

def coordinate1 : StructuredCoordinate := {
  coefficient := 7
  nativeLocked := 6
  shardSupply := 60
  structuredCustody := 49
  explicitFreeShards := 11
}

example : coordinate0.exact 10 7 := by
  simp [StructuredCoordinate.exact, coordinate0]
example : coordinate1.exact 10 7 := by
  simp [StructuredCoordinate.exact, coordinate1]

example : (issueReceipts 1 coordinate0).exact 10 8 := by
  simp [StructuredCoordinate.exact, issueReceipts, coordinate0]
example : (issueReceipts 1 coordinate1).exact 10 8 := by
  simp [StructuredCoordinate.exact, issueReceipts, coordinate1]

example : (unwrapReceipts 2 coordinate0).exact 10 5 := by
  simp [StructuredCoordinate.exact, unwrapReceipts, coordinate0]
example : (unwrapReceipts 2 coordinate1).exact 10 5 := by
  simp [StructuredCoordinate.exact, unwrapReceipts, coordinate1]

/-- Nine shards do not silently round; joining one transferable shard produces
one exact native claim and no residual change. -/
example : coalesce 10 9 = some { inputShards := 9, nativeClaims := 0, changeShards := 9 } := by
  decide

example : coalesce 10 (9 + 1) =
    some { inputShards := 10, nativeClaims := 1, changeShards := 0 } := by
  decide

/-- Omitting explicit free shard supply is an invalid conservation claim. -/
def hiddenRounding : StructuredCoordinate := {
  coordinate0 with explicitFreeShards := 0
}

example : ¬ hiddenRounding.exact 10 7 := by
  simp [StructuredCoordinate.exact, hiddenRounding, coordinate0]

def native0 : Node := {
  contentId := 1
  rank := 0
  kind := .native 0
  edges := []
  exposure := ⟨[100, 0]⟩
}

def native1 : Node := {
  contentId := 2
  rank := 0
  kind := .native 1
  edges := []
  exposure := ⟨0 :: [100]⟩
}

def shard0 : Node := {
  contentId := 3
  rank := 1
  kind := .shard 10
  edges := [{
    childId := 1
    childRank := 0
    atomsPerParent := 1
    childExposure := ⟨[100, 0]⟩
  }]
  exposure := ⟨[10, 0]⟩
}

def shard1 : Node := {
  contentId := 4
  rank := 1
  kind := .shard 10
  edges := [{
    childId := 2
    childRank := 0
    atomsPerParent := 1
    childExposure := ⟨[0, 100]⟩
  }]
  exposure := ⟨[0, 10]⟩
}

def basket : Node := {
  contentId := 5
  rank := 2
  kind := .basket
  edges := [{
    childId := 3
    childRank := 1
    atomsPerParent := 3
    childExposure := ⟨[10, 0]⟩
  }, {
    childId := 4
    childRank := 1
    atomsPerParent := 7
    childExposure := ⟨[0, 10]⟩
  }]
  exposure := ⟨[30, 70]⟩
}

def acceptedGraph : Graph := {
  graphId := 6
  outcomeCount := 2
  scale := 100
  rootId := 5
  nodes := [native0, native1, shard0, shard1, basket]
}

example : acceptedGraph.valid = true := by native_decide
example : acceptedGraph.rootExposure? = some ⟨[30, 70]⟩ := by native_decide

def acceptedDescriptor : ImmutableDescriptor := {
  descriptorId := 7
  graphId := 6
  rootId := 5
  marketId := 8
  releaseSetId := 9
  receiptMint := 10
  tokenProgram := 11
  representationAuthority := 12
  denominator := 10
  coefficients := [3, 7]
}

example : acceptedDescriptor.validFor acceptedGraph = true := by native_decide

/-- A same-width coefficient vector is not substitutable for the finalized
descriptor's exact common-scale payoff. -/
def substitutedCoefficients : ImmutableDescriptor := {
  acceptedDescriptor with coefficients := [4, 6]
}

example : substitutedCoefficients.validFor acceptedGraph = false := by native_decide

/-- A descriptor cannot transplant its valid coefficient vector to another
same-width graph identity. -/
def substitutedGraphIdentity : ImmutableDescriptor := {
  acceptedDescriptor with graphId := 99
}

example : substitutedGraphIdentity.validFor acceptedGraph = false := by native_decide

/-- A self-reference authenticates an existing identity but cannot decrease
rank, so it is refused as a cycle. -/
def cyclicBasket : Node := {
  basket with
  edges := [{
    childId := 5
    childRank := 2
    atomsPerParent := 1
    childExposure := ⟨[30, 70]⟩
  }]
}

def cyclicGraph : Graph := {
  acceptedGraph with nodes := [native0, native1, shard0, shard1, cyclicBasket]
}

example : cyclicGraph.valid = false := by native_decide

/-- Child identities are a canonical set, not caller-selected evaluation
order. -/
def reversedBasket : Node := {
  basket with edges := basket.edges.reverse
}

def reversedGraph : Graph := {
  acceptedGraph with nodes := [native0, native1, shard0, shard1, reversedBasket]
}

example : reversedGraph.valid = false := by native_decide

/-- A root may not claim a forged native exposure even when every child record
exists. -/
def forgedBasket : Node := {
  basket with exposure := ⟨[31, 69]⟩
}

def forgedGraph : Graph := {
  acceptedGraph with nodes := [native0, native1, shard0, shard1, forgedBasket]
}

example : forgedGraph.valid = false := by native_decide

end DClutch.RationalRepresentationV2.Examples
