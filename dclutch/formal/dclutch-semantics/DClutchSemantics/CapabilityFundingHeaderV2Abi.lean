import DClutchSemantics.AbiSchema

/-!
# Capability Funding Header V2 ABI

This header routes one to sixteen physical funding ledgers whose disjoint
subsets cover one to sixteen logical manifest entries.  The union mask may be
sparse over its sixteen-bit domain, but it is nonzero and its population count
is exactly the logical count.  A manifest-bound consumer separately constrains
the selected indices to that manifest's entry range.
-/

namespace DClutch.CapabilityFundingHeaderV2Abi

open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x46, 0x4c, 0x32]
def schemaVersion : Nat := 2
def maxPhysicalCount : Nat := 16
def maxLogicalCount : Nat := 16

inductive Field where
  | magic | version | physicalCount | logicalCount | selectedMask | reserved
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.physicalCount, .u8⟩,
  ⟨.logicalCount, .u8⟩,
  ⟨.selectedMask, .u16⟩,
  ⟨.reserved, .reserved 2⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

namespace Field

def rustName : Field → String
  | .magic => "CAPABILITY_FUNDING_MAGIC_OFFSET_V2"
  | .version => "CAPABILITY_FUNDING_VERSION_OFFSET_V2"
  | .physicalCount => "CAPABILITY_FUNDING_PHYSICAL_COUNT_OFFSET_V2"
  | .logicalCount => "CAPABILITY_FUNDING_LOGICAL_COUNT_OFFSET_V2"
  | .selectedMask => "CAPABILITY_FUNDING_SELECTED_MASK_OFFSET_V2"
  | .reserved => "CAPABILITY_FUNDING_RESERVED_OFFSET_V2"

end Field

structure Header where
  physicalCount : Nat
  logicalCount : Nat
  selectedMask : Nat
  deriving DecidableEq, Repr

def selectedBitCount (mask : Nat) : Nat :=
  (List.range maxLogicalCount).countP fun bit => mask.testBit bit

def valid (header : Header) : Bool :=
  header.physicalCount > 0 && header.physicalCount ≤ maxPhysicalCount &&
    header.physicalCount ≤ header.logicalCount &&
    header.logicalCount > 0 && header.logicalCount ≤ maxLogicalCount &&
    header.selectedMask > 0 && header.selectedMask < 2 ^ maxLogicalCount &&
    selectedBitCount header.selectedMask = header.logicalCount

def canonicalExample : Header := {
  physicalCount := 1
  logicalCount := 3
  selectedMask := 0b1000000000000101
}

theorem width_is_exact : bytes = 16 := by native_decide
theorem fields_unique : (schema.map fun field => field.name).Nodup := by native_decide
theorem fields_disjoint : layout.Pairwise Before := specializeFrom_pairwise 0 schema
theorem example_valid : valid canonicalExample := by native_decide
theorem zero_logical_count_refuses :
    !valid { canonicalExample with logicalCount := 0 } := by native_decide
theorem oversized_logical_count_refuses :
    !valid { canonicalExample with logicalCount := 17 } := by native_decide
theorem zero_selection_refuses :
    !valid { canonicalExample with selectedMask := 0 } := by native_decide
theorem out_of_range_selection_refuses :
    !valid { canonicalExample with selectedMask := 0b10000000000000000 } := by native_decide
theorem multiple_physical_accounts_refuse :
    !valid { canonicalExample with physicalCount := 17 } := by native_decide
theorem empty_physical_span_refuses :
    !valid { canonicalExample with physicalCount := 0 } := by native_decide
theorem incomplete_union_refuses :
    !valid { canonicalExample with selectedMask := 0b011 } := by native_decide
theorem too_many_physical_ledgers_refuse :
    !valid { canonicalExample with physicalCount := 4 } := by native_decide

end DClutch.CapabilityFundingHeaderV2Abi
