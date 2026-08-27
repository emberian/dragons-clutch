import DClutchSemantics.StructuredV2

/-!
# Structured V2 concrete and hostile examples

Every example below is checked by the kernel.  The running instrument is a
two-coordinate Structured receipt over a shard layer with denominator `4` and
coefficients `[1, 3]`: one receipt atom denotes exactly `1/4` native claims of
coordinate `0` and `3/4` of coordinate `1`.

Structured V1 could not admit this recipe at all.  Its backing had to land on
whole native claims, so it required the least realization lot to equal the
Product denominator.  Structured V2 backs the same recipe exactly because the
shard layer already denominates a native claim into four transferable atoms.
-/

namespace DClutch.StructuredV2.Examples

open DClutch.StructuredV2

/-- Executable checked-arithmetic ceiling; a profile bound, not an ontology bound. -/
def scalarLimit : Nat := 2 ^ 64

/-- The running `1/4` and `3/4` two-coordinate basis. -/
def quarterThreeQuarterBasis : Basis := {
  termsId := 0x51
  marketId := 0x52
  productRecordId := 0x53
  resultDomainId := 0x54
  releaseSetId := 0x55
  shardTermsId := 0x56
  shardExposureId := 0x57
  receiptMintId := 0x58
  graphId := 0x59
  representationWidth := 2
  denominator := 4
  coefficients := [1, 3]
}

example : quarterThreeQuarterBasis.valid scalarLimit = true := by decide

/-- A basis backed by nothing at all is inadmissible. -/
def emptyBasis : Basis :=
  { quarterThreeQuarterBasis with coefficients := [0, 0] }

example : emptyBasis.valid scalarLimit = false := by decide

/-- A degenerate denominator is inadmissible: fractional denomination needs `D > 1`. -/
def wholeDenominatorBasis : Basis :=
  { quarterThreeQuarterBasis with denominator := 1 }

example : wholeDenominatorBasis.valid scalarLimit = false := by decide

/-- A width that disagrees with the coefficient vector is inadmissible. -/
def widthMismatchBasis : Basis :=
  { quarterThreeQuarterBasis with representationWidth := 3 }

example : widthMismatchBasis.valid scalarLimit = false := by decide

/-- A zero identity is inadmissible. -/
def zeroShardTermsBasis : Basis :=
  { quarterThreeQuarterBasis with shardTermsId := 0 }

example : zeroShardTermsBasis.valid scalarLimit = false := by decide

/-! ## Exact backing -/

/-- Ten receipt atoms require exactly `10 * [1, 3] = [10, 30]` shard atoms. -/
example : quarterThreeQuarterBasis.requiredCustody 10 = [10, 30] := by decide

/-- Zero supply requires exactly zero custody, which is what lets a node close. -/
example : quarterThreeQuarterBasis.requiredCustody 0 = [0, 0] := by decide

/-- A coordinate carrying a zero coefficient locks no shard atom. -/
example :
    ({ quarterThreeQuarterBasis with coefficients := [0, 3] } : Basis).requiredCustody 10 =
      [0, 30] := by decide

/-! ## Lifecycle frames -/

/-- Structured-owned persisted root at revision `7`. -/
def root : Root := {
  basisId := quarterThreeQuarterBasis.termsId
  marketId := quarterThreeQuarterBasis.marketId
  rentBeneficiaryId := 0x60
  revision := 7
}

/-- Open projection carrying `10` receipt atoms and no donated surplus. -/
def openProjection : Projection := {
  phase := .open
  basisId := quarterThreeQuarterBasis.termsId
  marketId := quarterThreeQuarterBasis.marketId
  shardTermsId := quarterThreeQuarterBasis.shardTermsId
  shardDenominator := quarterThreeQuarterBasis.denominator
  representationWidth := 2
  custody := { receiptSupply := 10, surplus := [0, 0] }
  revision := 7
}

/-- Terminal projection: coordinate `1` pays five collateral atoms per whole
native claim, coordinate `0` pays nothing. -/
def terminalProjection : Projection :=
  { openProjection with phase := .terminal [0, 5] }

/-- Canonical accepted frame builder. -/
def frame (projection : Projection) (holderReceipts : Nat) (command : Command) : Frame := {
  scalarLimit
  basis := quarterThreeQuarterBasis
  root
  projection
  holderReceipts
  command
}

/-! ## Accepted transitions -/

/-- Issue four more receipt atoms against the open Market. -/
example : accepts (frame openProjection 10 (.issue 4 7)) = true := by decide

/-- Issuing locks exactly `4 * [1, 3] = [4, 12]` shard atoms. -/
example :
    effect (frame openProjection 10 (.issue 4 7)) =
      some (.lockAndMint 4 [4, 12]) := by decide

/-- Unwrap three receipt atoms and release exactly `[3, 9]` shard atoms. -/
example : accepts (frame openProjection 10 (.unwrap 3 7)) = true := by decide

example :
    effect (frame openProjection 10 (.unwrap 3 7)) =
      some (.burnAndRelease 3 [3, 9]) := by decide

/-- Terminal redemption of all ten receipts.  Coordinate `0` releases ten shard
atoms, which divide into two whole native claims worth nothing and two atoms of
explicit change.  Coordinate `1` releases thirty shard atoms, which divide into
seven whole native claims at five collateral atoms each plus two atoms of
change.  Total settlement is exactly `35` collateral atoms. -/
example :
    effect (frame terminalProjection 10 (.terminalRedeem 10 7)) =
      some (.burnAndSettle 10 [10, 30] [
        { representationCoordinate := 0, releasedShards := 10, wholeClaims := 2,
          burnedShards := 8, changeShards := 2, payoutPerClaim := 0, collateralAtoms := 0 },
        { representationCoordinate := 1, releasedShards := 30, wholeClaims := 7,
          burnedShards := 28, changeShards := 2, payoutPerClaim := 5,
          collateralAtoms := 35 }]) := by decide

/-- The same settlement totals exactly the winner's payout; the losing
coordinate contributes zero even though it released whole native claims. -/
example :
    (terminalSettlement quarterThreeQuarterBasis [0, 5] 10).map totalCollateral =
      some 35 := by decide

/-- A sub-denominator release is explicit change, never a rounded-away credit:
redeeming three receipts yields no whole claim at coordinate `0` at all. -/
example :
    terminalSettlement quarterThreeQuarterBasis [0, 5] 3 = some [
      { representationCoordinate := 0, releasedShards := 3, wholeClaims := 0,
        burnedShards := 0, changeShards := 3, payoutPerClaim := 0, collateralAtoms := 0 },
      { representationCoordinate := 1, releasedShards := 9, wholeClaims := 2,
        burnedShards := 8, changeShards := 1, payoutPerClaim := 5,
        collateralAtoms := 10 }] := by decide

/-- Retirement of an empty node is accepted. -/
def retiredReadyProjection : Projection :=
  { terminalProjection with custody := { receiptSupply := 0, surplus := [0, 0] } }

example : accepts (frame retiredReadyProjection 0 (.retire 7)) = true := by decide

example :
    effect (frame retiredReadyProjection 0 (.retire 7)) =
      some (.closeToBeneficiary 0x60) := by decide

/-! ## Hostile refusals -/

/-- Substituted shard terms refuse: the receipt would be backed by a different
shard layer than the one its immutable basis selected. -/
example :
    accepts (frame { openProjection with shardTermsId := 0x99 } 10 (.issue 4 7)) = false := by
  decide

/-- A substituted shard denominator refuses. -/
example :
    accepts (frame { openProjection with shardDenominator := 8 } 10 (.issue 4 7)) = false := by
  decide

/-- A substituted Market refuses. -/
example :
    accepts (frame { openProjection with marketId := 0x99 } 10 (.issue 4 7)) = false := by decide

/-- A projection whose surplus vector has the wrong width refuses. -/
example :
    accepts (frame { openProjection with custody := { receiptSupply := 10, surplus := [0] } } 10
      (.issue 4 7)) = false := by decide

/-- A terminal payout vector of the wrong width refuses. -/
example :
    accepts (frame { openProjection with phase := .terminal [0, 5, 5] } 10
      (.terminalRedeem 4 7)) = false := by decide

/-- Stale replay refuses: the command's expected revision no longer matches. -/
example : accepts (frame openProjection 10 (.issue 4 6)) = false := by decide

/-- Double redemption refuses.  After redeeming three receipts the root advances
to revision `8` and the supply falls to `7`; presenting the identical command
again is rejected on its stale revision. -/
def redeemedRoot : Root := { root with revision := 8 }

def redeemedProjection : Projection :=
  { terminalProjection with
    custody := { receiptSupply := 7, surplus := [0, 0] }
    revision := 8 }

example :
    accepts { frame terminalProjection 10 (.terminalRedeem 3 7) with
      root := redeemedRoot
      projection := redeemedProjection } = false := by decide

/-- Redeeming more receipts than the observed supply refuses. -/
example : accepts (frame terminalProjection 20 (.terminalRedeem 11 7)) = false := by decide

/-- Redeeming more receipts than the actor holds refuses. -/
example : accepts (frame terminalProjection 2 (.terminalRedeem 4 7)) = false := by decide

/-- A zero-quantity action refuses. -/
example : accepts (frame openProjection 10 (.issue 0 7)) = false := by decide

/-- Issuing after terminal resolution refuses. -/
example : accepts (frame terminalProjection 10 (.issue 4 7)) = false := by decide

/-- Terminal redemption before resolution refuses. -/
example : accepts (frame openProjection 10 (.terminalRedeem 4 7)) = false := by decide

/-- Retiring a node with outstanding receipt supply refuses. -/
example : accepts (frame terminalProjection 0 (.retire 7)) = false := by decide

/-- Retiring a node holding donated surplus refuses: a Structured node closes
only when its observed custody is exactly empty. -/
example :
    accepts (frame
      { terminalProjection with custody := { receiptSupply := 0, surplus := [0, 5] } } 0
      (.retire 7)) = false := by decide

/-- Retiring before resolution refuses. -/
example :
    accepts (frame { openProjection with custody := { receiptSupply := 0, surplus := [0, 0] } } 0
      (.retire 7)) = false := by decide

/-- Overflow refuses rather than wrapping: the required custody product exceeds
the executable checked ceiling. -/
example :
    accepts { frame openProjection 10 (.issue (2 ^ 63) 7) with
      projection := { openProjection with custody := { receiptSupply := 0, surplus := [0, 0] } } } =
      false := by decide

/-! ## Graph refusals -/

example : canonicalGraph.length = 3 := by decide

example : (BackingEdge.mk .structuredReceipt .claimShard).admits = true := by decide

example : (BackingEdge.mk .structuredReceipt .structuredReceipt).admits = false := by decide

example : (BackingEdge.mk .claimShard .structuredReceipt).admits = false := by decide

example : (BackingEdge.mk .marketLiability .nativePosition).admits = false := by decide

end DClutch.StructuredV2.Examples
