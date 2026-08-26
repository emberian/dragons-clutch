import DClutchSemantics.ProductRuntimeV2

/-! Lean-owned physical layout for runtime-tail Product V2 records. -/

namespace DClutch.ProductRuntimeV2Abi

def schemaVersion : Nat := 2

def domainMagic : String := "DCLTPRD2"
def domainHeaderBytes : Nat := 240
def domainRegionCountOffset : Nat := 16
def domainCutCountOffset : Nat := 20
def domainProductIdOffset : Nat := 32
def domainCoordinateDomainIdOffset : Nat := 64
def domainResultUnitIdOffset : Nat := 96
def domainLiabilityBasisIdOffset : Nat := 128
def domainRepresentationReleaseIdOffset : Nat := 160
def domainMappingReleaseIdOffset : Nat := 192
def domainCutDenominatorOffset : Nat := 224
def domainCutBytes : Nat := 16

def portfolioMagic : String := "DCLTPRF2"
def portfolioHeaderBytes : Nat := 208
def portfolioCoefficientCountOffset : Nat := 16
def portfolioRoundingOffset : Nat := 20
def portfolioProductIdOffset : Nat := 32
def portfolioResultDomainIdOffset : Nat := 64
def portfolioClaimBasisIdOffset : Nat := 96
def portfolioLiabilityBasisIdOffset : Nat := 128
def portfolioRepresentationReleaseIdOffset : Nat := 160
def portfolioDenominatorOffset : Nat := 192
def portfolioCoefficientBytes : Nat := 8
def representationFloorTag : Nat := 1

def domainRecordBytes (cutCount : Nat) : Nat :=
  domainHeaderBytes + cutCount * domainCutBytes

def portfolioRecordBytes (coefficientCount : Nat) : Nat :=
  portfolioHeaderBytes + coefficientCount * portfolioCoefficientBytes

example : domainRecordBytes 0 = 240 := by decide
example : domainRecordBytes 300 = 5040 := by decide
example : portfolioRecordBytes 301 = 2616 := by decide

end DClutch.ProductRuntimeV2Abi
