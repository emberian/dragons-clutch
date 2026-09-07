import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import DClutchSemantics.ConditionalMarketV1
import Std.Tactic

/-!
# The parent reference, the derived settle request, and the derived family

`ConditionalMarketV1` owns the SEMANTICS of a child market: what a product
`A × B` and a conditional `B | A = a` are, how a child's selector is a function
of its parents' certificates, and which hostiles are refused by name.  This
module owns the WIRE of the same facts, and nothing else: it is the twin of
`ConditionalMarketV1.ParentRef` as bytes, the request the derived provider's
`Settle` route decodes, and the immutable identities the derived provider
family is pinned by.

Three layouts, each specialized from one schema so no offset has a second
author:

* `ParentReferenceV1` (`DCLTPRF1`, 200 bytes) — the content-addressed record a
  child founds against and names as its `SourceSpecV1.adapter_config_id`.  It
  binds each parent's Market, generation, Product-record digest and ordinary
  count, the condition (conditional only) and the child's `settle_by`.  A
  parent replaced after founding, or a Product record moved under it, fails
  this reference at settlement by the theorems of `ConditionalMarketV1`.
* `DerivedSettleRequestV1` (`DCLTDRV1`, 128 bytes) — the derived provider's
  one instruction: which market, which generation, which terminal sequence,
  and the three identities the frame's records must hash to.
* the Found frame EXTENSION is not here: it is a list of account slots and it
  lives in `CoreFoundFrameV3Abi` beside the price-gate extension it mirrors.

## Where the reference is named, and why it is the adapter-config slot

The design note (`MECHANISM_CONDITIONAL_MARKETS_2026_09_04.md` §2.1) named the
reference through `decoding_rules_id`.  That slot is `ProviderReleaseV1`'s,
and it names a layout TABLE shared by every market of a family — for the relay
it is the venue decoding-rules row.  A parent reference is per market, exactly
as `PythAdapterConfigV1` is per market, and that record is named by
`SourceSpecV1.adapter_config_id`.  So the reference lives where the per-market
adapter configuration lives, and the derived family's `ProviderReleaseV1` is a
constant with a constant `decoding_rules_id` (`derivedDecodingRulesId`: "the
joint index at exponent zero").  The note's sentence is corrected here rather
than in the note, which is a dated design record.

## What Lean does not do

Lean does not hash.  Every identity below that is a SHA-256 is held as its
preimage AND its digest, both data; the emitter prints both, and the Rust
twin's own test recomputes the digest from the preimage.  That is the pattern
`ProductAdmissionV2Abi` and `RelayedMainnetStateV1Abi` established.
-/

namespace DClutch.ParentReferenceV1Abi

open DClutch
open DClutch.AbiSchema

/-! ## Identities the derived family is pinned by -/

/-- `sha256("dclutch/parent-reference/v1")`: the Registry schema identity of a
finalized `ParentReferenceV1` record. -/
def recordSchemaPreimage : String := "dclutch/parent-reference/v1"
def recordSchemaId : List UInt8 := [
  0xd0, 0x27, 0x3b, 0xa2, 0x33, 0xca, 0x45, 0x95, 0xfd, 0x95, 0x7f, 0x5f, 0x16, 0x87, 0xbf, 0xeb,
  0xb5, 0xff, 0xf9, 0xe9, 0xa1, 0x2d, 0x9b, 0x59, 0x53, 0xc9, 0x76, 0x9b, 0x97, 0xdf, 0xd4, 0x22]

/-- `ProviderReleaseV1.provider_family_id` of the derived family. -/
def familyReleasePreimage : String := "dclutch/provider-family/derived-from-parents/v1"
def familyReleaseId : List UInt8 := [
  0x00, 0x3d, 0x69, 0xbe, 0x1c, 0xd5, 0x8e, 0xfd, 0x6d, 0x3d, 0xc5, 0x28, 0x00, 0xb3, 0x44, 0xec,
  0x38, 0xff, 0x84, 0x63, 0x29, 0x17, 0x45, 0x02, 0xcf, 0xe5, 0xb1, 0x97, 0xdd, 0x46, 0xf7, 0x81]

/-- The provider extension `SourceAccessProfile.DerivedFromParents` selects. -/
def extensionReleasePreimage : String := "dclutch/provider-extension/derived-from-parents/v1"
def extensionReleaseId : List UInt8 := [
  0x1d, 0xd0, 0x7b, 0x00, 0xcc, 0x22, 0x4c, 0x47, 0x8e, 0x73, 0xaf, 0x59, 0xd4, 0xaf, 0x35, 0x62,
  0x5e, 0x88, 0x49, 0x3e, 0xf0, 0xc3, 0x13, 0x0c, 0x48, 0x41, 0xde, 0xab, 0xed, 0x3a, 0x3d, 0x78]

/-- `ProviderReleaseV1.transport_profile_id`: the evidence is two on-cluster
certificate accounts, no signature, no relayer, no feed. -/
def transportProfilePreimage : String := "dclutch/transport-profile/parent-certificates/v1"
def transportProfileId : List UInt8 := [
  0x3b, 0x57, 0x53, 0xe3, 0x2b, 0xcc, 0x0a, 0x67, 0xfd, 0x9f, 0x34, 0x50, 0xab, 0x4c, 0xc4, 0x8f,
  0xfc, 0x6e, 0x27, 0x41, 0x4c, 0x79, 0xee, 0xdb, 0xeb, 0x95, 0x33, 0xff, 0x90, 0x44, 0xc6, 0x3c]

/-- `ProviderReleaseV1.decoding_rules_id`: one rule, "the joint index over
denominator one at exponent zero", constant for every child. -/
def decodingRulesPreimage : String := "dclutch/decoding-rules/derived-joint-index/v1"
def decodingRulesId : List UInt8 := [
  0x83, 0x64, 0x8b, 0xa8, 0xa0, 0xf6, 0x8d, 0xcd, 0xf2, 0x08, 0x39, 0x66, 0xf9, 0x78, 0xd1, 0x9e,
  0x51, 0x8d, 0xe4, 0xdd, 0xb0, 0x3c, 0x06, 0x47, 0xe9, 0xc5, 0xca, 0xd3, 0x57, 0xe6, 0xb8, 0x8f]

/-- Domain separating the derived provider's evidence identity
`H(domain ∥ cert_A ∥ cert_B)` from every other digest. -/
def evidenceDomainPreimage : String := "dclutch/derived-provider-evidence/v1"

theorem every_identity_is_an_identity :
    recordSchemaId.length = 32 ∧ familyReleaseId.length = 32 ∧
    extensionReleaseId.length = 32 ∧ transportProfileId.length = 32 ∧
    decodingRulesId.length = 32 := by native_decide

theorem the_identities_are_distinct :
    [recordSchemaId, familyReleaseId, extensionReleaseId, transportProfileId,
      decodingRulesId].eraseDups.length = 5 := by native_decide

/-- `SourceAccessProfile.DerivedFromParents`.  The four shipped profiles are
`1..4`; a child's spec selects the fifth.  This is the ONE author of the byte:
`crates/dclutch-source/src/lib.rs` pins its enum discriminant to the emitted
constant. -/
def derivedAccessProfile : Nat := 5

theorem the_access_profile_is_new : 4 < derivedAccessProfile ∧ derivedAccessProfile < 256 := by
  native_decide

/-- The two child shapes, as the record's `kind` byte. -/
inductive Kind where
  | product
  | conditional
  deriving DecidableEq, Repr

def Kind.byte : Kind → Nat
  | .product => 1
  | .conditional => 2

def Kind.ofByte? : Nat → Option Kind
  | 1 => some .product
  | 2 => some .conditional
  | _ => none

theorem kind_round_trips (kind : Kind) : Kind.ofByte? kind.byte = some kind := by
  cases kind <;> rfl

theorem a_zero_kind_is_refused : Kind.ofByte? 0 = none := rfl

/-! ## `ParentReferenceV1` -/

inductive RecordField where
  | magic | version | kind | reservedHeader
  | parentAMarket | parentAGeneration | parentARecord | parentAOrdinary | reservedA
  | parentBMarket | parentBGeneration | parentBRecord | parentBOrdinary | reservedB
  | condition | reservedCondition | settleBy | reservedTail
  deriving DecidableEq, Repr

def recordMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x50, 0x52, 0x46, 0x31]  -- DCLTPRF1
def recordVersion : Nat := 1

def recordSchema : List (FieldSpec RecordField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.kind, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩,
  ⟨.parentAMarket, .bytes 32⟩,
  ⟨.parentAGeneration, .u64⟩,
  ⟨.parentARecord, .bytes 32⟩,
  ⟨.parentAOrdinary, .u32⟩,
  ⟨.reservedA, .reserved 4⟩,
  ⟨.parentBMarket, .bytes 32⟩,
  ⟨.parentBGeneration, .u64⟩,
  ⟨.parentBRecord, .bytes 32⟩,
  ⟨.parentBOrdinary, .u32⟩,
  ⟨.reservedB, .reserved 4⟩,
  ⟨.condition, .u32⟩,
  ⟨.reservedCondition, .reserved 4⟩,
  ⟨.settleBy, .u64⟩,
  ⟨.reservedTail, .reserved 8⟩
]

def recordLayout : List (PlacedField RecordField) := specialize recordSchema
def recordBytes : Nat := schemaWidth recordSchema

namespace RecordField

def rustName : RecordField → String
  | .magic => "PARENT_REFERENCE_MAGIC_OFFSET_V1"
  | .version => "PARENT_REFERENCE_VERSION_OFFSET_V1"
  | .kind => "PARENT_REFERENCE_KIND_OFFSET_V1"
  | .reservedHeader => "PARENT_REFERENCE_RESERVED_HEADER_OFFSET_V1"
  | .parentAMarket => "PARENT_REFERENCE_PARENT_A_MARKET_OFFSET_V1"
  | .parentAGeneration => "PARENT_REFERENCE_PARENT_A_GENERATION_OFFSET_V1"
  | .parentARecord => "PARENT_REFERENCE_PARENT_A_RECORD_OFFSET_V1"
  | .parentAOrdinary => "PARENT_REFERENCE_PARENT_A_ORDINARY_OFFSET_V1"
  | .reservedA => "PARENT_REFERENCE_RESERVED_A_OFFSET_V1"
  | .parentBMarket => "PARENT_REFERENCE_PARENT_B_MARKET_OFFSET_V1"
  | .parentBGeneration => "PARENT_REFERENCE_PARENT_B_GENERATION_OFFSET_V1"
  | .parentBRecord => "PARENT_REFERENCE_PARENT_B_RECORD_OFFSET_V1"
  | .parentBOrdinary => "PARENT_REFERENCE_PARENT_B_ORDINARY_OFFSET_V1"
  | .reservedB => "PARENT_REFERENCE_RESERVED_B_OFFSET_V1"
  | .condition => "PARENT_REFERENCE_CONDITION_OFFSET_V1"
  | .reservedCondition => "PARENT_REFERENCE_RESERVED_CONDITION_OFFSET_V1"
  | .settleBy => "PARENT_REFERENCE_SETTLE_BY_OFFSET_V1"
  | .reservedTail => "PARENT_REFERENCE_RESERVED_TAIL_OFFSET_V1"

def offset (field : RecordField) : Nat :=
  (coordinate? field recordLayout).map (fun value => value.1) |>.getD 0

def width (field : RecordField) : Nat :=
  (coordinate? field recordLayout).map (fun value => value.2) |>.getD 0

end RecordField

theorem record_exact_width : recordBytes = 200 := by native_decide
theorem record_layout_disjoint : recordLayout.Pairwise Before :=
  specializeFrom_pairwise 0 recordSchema

/-- The two parents are one shape, one stride apart: every `parentB*` offset is
its `parentA*` offset plus eighty.  A decoder that reads a parent by index
reads one table twice rather than two tables once. -/
def parentStride : Nat := 80

theorem the_parents_are_one_stride_apart :
    RecordField.parentBMarket.offset = RecordField.parentAMarket.offset + parentStride ∧
    RecordField.parentBGeneration.offset = RecordField.parentAGeneration.offset + parentStride ∧
    RecordField.parentBRecord.offset = RecordField.parentARecord.offset + parentStride ∧
    RecordField.parentBOrdinary.offset = RecordField.parentAOrdinary.offset + parentStride := by
  native_decide

/-- The reference as a value: the wire twin of two `ConditionalMarketV1.ParentRef`s. -/
structure Record where
  kind : Kind
  parentA : ConditionalMarket.ParentRef
  parentB : ConditionalMarket.ParentRef
  condition : Nat
  settleBy : Nat
  deriving DecidableEq, Repr

def parentValid (ref : ConditionalMarket.ParentRef) : Bool :=
  ref.marketId != 0 && ref.marketId < 256 ^ 32 &&
  ref.generation != 0 && ref.generation < 256 ^ 8 &&
  ref.productRecordDigest != 0 && ref.productRecordDigest < 256 ^ 32 &&
  ref.ordinaryCount != 0 && ref.ordinaryCount < 256 ^ 4

/-- Canonical validity: what the decoder accepts.  The founding conjuncts
(`sameParent`, `conditionOnFailure`, `widthOverflow`, ...) are
`ConditionalMarketV1`'s and are NOT restated here: a record can be canonical
bytes and still be refused at founding by name. -/
def Record.valid (value : Record) : Bool :=
  parentValid value.parentA && parentValid value.parentB &&
  value.settleBy != 0 && value.settleBy < 256 ^ 8 &&
  value.condition < 256 ^ 4 &&
  (match value.kind with
    | .product => value.condition == 0
    | .conditional => true)

def encodeRecord (value : Record) : List UInt8 :=
  let parent (ref : ConditionalMarket.ParentRef) : List UInt8 :=
    Codec.encodeLE 32 ref.marketId ++ Codec.encodeLE 8 ref.generation ++
    Codec.encodeLE 32 ref.productRecordDigest ++ Codec.encodeLE 4 ref.ordinaryCount ++
    List.replicate 4 0
  recordMagic ++ Codec.encodeLE 2 recordVersion ++ [UInt8.ofNat value.kind.byte] ++
  List.replicate 5 0 ++ parent value.parentA ++ parent value.parentB ++
  Codec.encodeLE 4 value.condition ++ List.replicate 4 0 ++
  Codec.encodeLE 8 value.settleBy ++ List.replicate 8 0

theorem record_encoding_length (value : Record) : (encodeRecord value).length = recordBytes := by
  simp [encodeRecord, recordBytes, recordSchema, schemaWidth, recordMagic,
    Codec.encodeLE_length, FieldKind.byteWidth]

/-- Two width-3 parents, the shape decision 0029 item 7 ships; the product
child of `ConditionalMarketV1.Examples`. -/
def productExample : Record := {
  kind := .product
  parentA := ConditionalMarket.Examples.parentA
  parentB := ConditionalMarket.Examples.parentB
  condition := 0
  settleBy := 1_800_000_000
}

def conditionalExample : Record := { productExample with kind := .conditional, condition := 0 }

theorem product_example_valid : productExample.valid = true := by native_decide
theorem conditional_example_valid : conditionalExample.valid = true := by native_decide

def sliceNat (input : List UInt8) (offset width : Nat) : Nat :=
  Codec.decodeLE ((input.drop offset).take width)

def decodeParent (input : List UInt8) (base : Nat) : ConditionalMarket.ParentRef := {
  marketId := sliceNat input base 32
  generation := sliceNat input (base + 32) 8
  productRecordDigest := sliceNat input (base + 40) 32
  ordinaryCount := sliceNat input (base + 72) 4
}

def recordValidBytes (input : List UInt8) : Bool :=
  input.length = recordBytes && input.take 8 = recordMagic &&
  sliceNat input RecordField.version.offset 2 = recordVersion &&
  (input.drop RecordField.reservedHeader.offset).take 5 = List.replicate 5 0 &&
  (input.drop RecordField.reservedA.offset).take 4 = List.replicate 4 0 &&
  (input.drop RecordField.reservedB.offset).take 4 = List.replicate 4 0 &&
  (input.drop RecordField.reservedCondition.offset).take 4 = List.replicate 4 0 &&
  (input.drop RecordField.reservedTail.offset).take 8 = List.replicate 8 0 &&
  (match Kind.ofByte? (sliceNat input RecordField.kind.offset 1) with
    | none => false
    | some kind =>
        ({
          kind
          parentA := decodeParent input RecordField.parentAMarket.offset
          parentB := decodeParent input RecordField.parentBMarket.offset
          condition := sliceNat input RecordField.condition.offset 4
          settleBy := sliceNat input RecordField.settleBy.offset 8
        } : Record).valid)

theorem product_example_bytes_valid : recordValidBytes (encodeRecord productExample) = true := by
  native_decide

theorem conditional_example_bytes_valid :
    recordValidBytes (encodeRecord conditionalExample) = true := by native_decide

/-- Zero one whole field.

Almost every hostile below is one byte moved, which is the sharpest form: the
row differs from a valid record in exactly one coordinate.  `settleBy` cannot
be expressed that way.  Its example value is `0x6b49d200`, whose LOW byte is
already zero, so `set settleBy.offset 0` re-emits the valid record byte for
byte -- a corpus row that proves nothing and a `refuses` theorem that cannot
hold.  A deadline of zero is the hostile that row means, and saying it takes
the whole field. -/
def zeroField (bytes : List UInt8) (field : RecordField) : List UInt8 :=
  (List.range field.width).foldl (fun acc index => acc.set (field.offset + index) 0) bytes

/-- The hostile corpus: one field moved per row, each refused.  The Rust
decoder's own test walks the emitted twin of this list. -/
def recordRefusalCorpus : List (List UInt8) := [
  (encodeRecord productExample).set 0 0,
  (encodeRecord productExample).set RecordField.version.offset 2,
  (encodeRecord productExample).set RecordField.kind.offset 0,
  (encodeRecord productExample).set RecordField.kind.offset 3,
  (encodeRecord productExample).set RecordField.reservedHeader.offset 1,
  (encodeRecord productExample).set RecordField.parentAGeneration.offset 0,
  (encodeRecord productExample).set RecordField.parentAOrdinary.offset 0,
  (encodeRecord productExample).set RecordField.reservedA.offset 1,
  (encodeRecord productExample).set RecordField.parentBOrdinary.offset 0,
  (encodeRecord productExample).set RecordField.reservedB.offset 1,
  -- a product record carrying a condition
  (encodeRecord productExample).set RecordField.condition.offset 1,
  (encodeRecord productExample).set RecordField.reservedCondition.offset 1,
  zeroField (encodeRecord productExample) RecordField.settleBy,
  (encodeRecord productExample).set RecordField.reservedTail.offset 1
]

theorem record_refusal_corpus_refuses :
    recordRefusalCorpus.all fun candidate => !recordValidBytes candidate := by native_decide

/-- No corpus row IS the valid record.  A hostile that decodes to the example
is refused by nothing and proves nothing, and a mutation that lands on a byte
already holding the mutated value is invisible in both the emission and the
`refuses` theorem's statement. -/
theorem record_refusal_corpus_moves_every_row :
    recordRefusalCorpus.all fun candidate => candidate != encodeRecord productExample := by
  native_decide


/-- Every corpus row is exactly one record wide, so a refusal is a refusal of
the CONTENT and never of the length. -/
theorem record_refusal_corpus_is_record_wide :
    recordRefusalCorpus.all fun candidate => candidate.length == recordBytes := by
  native_decide

/-! ## The product and conditional shapes, read off a decoded record

The bytes name two parents and a kind; the child's width, cells and failure
coordinate are `ConditionalMarketV1`'s functions of them.  Stated here so the
emitted twin can be tested against the same numbers the semantics prove. -/

def Record.productShape (value : Record) : ConditionalMarket.ProductShape :=
  { parentA := value.parentA, parentB := value.parentB }

def Record.conditionalShape (value : Record) : ConditionalMarket.ConditionalShape :=
  { parentA := value.parentA, condition := value.condition, parentB := value.parentB }

/-- The child's ordinary count: `R_A · R_B` for a product, `R_B + 1` for a
conditional.  The child's `ResultDomainV2` has `ordinaryCount − 1` consecutive
integer cuts over denominator one and the child's refunding basis has
`payoutScale = ordinaryCount`. -/
def Record.ordinaryCount (value : Record) : Nat :=
  match value.kind with
  | .product => value.productShape.cells
  | .conditional => value.conditionalShape.ordinaryCount

def Record.width (value : Record) : Nat := value.ordinaryCount + 1

theorem example_widths :
    productExample.ordinaryCount = 4 ∧ productExample.width = 5 ∧
    conditionalExample.ordinaryCount = 3 ∧ conditionalExample.width = 4 := by native_decide

/-- The cuts of the child's domain are the integers `1 … n − 1`, so the joint
index `v ∈ [0, n)` selects region `v` (`ProductRuntimeV2.selectOrdinaryScaled`
counts the cuts at or below the coordinate). -/
def childCuts (ordinaryCount : Nat) : List Nat := (List.range (ordinaryCount - 1)).map (· + 1)

theorem child_cuts_are_consecutive_from_one :
    childCuts 4 = [1, 2, 3] ∧ childCuts 3 = [1, 2] ∧ childCuts 2 = [1] := by native_decide

theorem child_cuts_count : (childCuts 6).length = 5 := by native_decide

/-! ## `DerivedSettleRequestV1` -/

inductive RequestField where
  | magic | version | action | reserved | generation | terminalSequence
  | parentReference | sourceMaterial | sourceSpec
  deriving DecidableEq, Repr

def requestMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x44, 0x52, 0x56, 0x31]  -- DCLTDRV1
def requestVersion : Nat := 1
/-- The one action: settle the child from its parents' certificates. -/
def requestSettleAction : Nat := 1

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.action, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.generation, .u64⟩,
  ⟨.terminalSequence, .u64⟩,
  ⟨.parentReference, .bytes 32⟩,
  ⟨.sourceMaterial, .bytes 32⟩,
  ⟨.sourceSpec, .bytes 32⟩
]

def requestLayout : List (PlacedField RequestField) := specialize requestSchema
def requestBytes : Nat := schemaWidth requestSchema

namespace RequestField

def rustName : RequestField → String
  | .magic => "DERIVED_SETTLE_MAGIC_OFFSET_V1"
  | .version => "DERIVED_SETTLE_VERSION_OFFSET_V1"
  | .action => "DERIVED_SETTLE_ACTION_OFFSET_V1"
  | .reserved => "DERIVED_SETTLE_RESERVED_OFFSET_V1"
  | .generation => "DERIVED_SETTLE_GENERATION_OFFSET_V1"
  | .terminalSequence => "DERIVED_SETTLE_TERMINAL_SEQUENCE_OFFSET_V1"
  | .parentReference => "DERIVED_SETTLE_PARENT_REFERENCE_OFFSET_V1"
  | .sourceMaterial => "DERIVED_SETTLE_SOURCE_MATERIAL_OFFSET_V1"
  | .sourceSpec => "DERIVED_SETTLE_SOURCE_SPEC_OFFSET_V1"

def offset (field : RequestField) : Nat :=
  (coordinate? field requestLayout).map (fun value => value.1) |>.getD 0

end RequestField

theorem request_exact_width : requestBytes = 128 := by native_decide
theorem request_layout_disjoint : requestLayout.Pairwise Before :=
  specializeFrom_pairwise 0 requestSchema

structure Request where
  generation : Nat
  terminalSequence : Nat
  parentReference : Nat
  sourceMaterial : Nat
  sourceSpec : Nat
  deriving DecidableEq, Repr

def Request.valid (value : Request) : Bool :=
  value.generation != 0 && value.generation < 256 ^ 8 &&
  value.terminalSequence != 0 && value.terminalSequence < 256 ^ 8 &&
  value.parentReference != 0 && value.parentReference < 256 ^ 32 &&
  value.sourceMaterial != 0 && value.sourceMaterial < 256 ^ 32 &&
  value.sourceSpec != 0 && value.sourceSpec < 256 ^ 32

def encodeRequest (value : Request) : List UInt8 :=
  requestMagic ++ Codec.encodeLE 2 requestVersion ++ [UInt8.ofNat requestSettleAction] ++
  List.replicate 5 0 ++ Codec.encodeLE 8 value.generation ++
  Codec.encodeLE 8 value.terminalSequence ++ Codec.encodeLE 32 value.parentReference ++
  Codec.encodeLE 32 value.sourceMaterial ++ Codec.encodeLE 32 value.sourceSpec

theorem request_encoding_length (value : Request) :
    (encodeRequest value).length = requestBytes := by
  simp [encodeRequest, requestBytes, requestSchema, schemaWidth, requestMagic,
    Codec.encodeLE_length, FieldKind.byteWidth]

def requestExample : Request :=
  { generation := 2, terminalSequence := 1, parentReference := 0xA1, sourceMaterial := 0xB2,
    sourceSpec := 0xC3 }

theorem request_example_valid : requestExample.valid = true := by native_decide

def requestValidBytes (input : List UInt8) : Bool :=
  input.length = requestBytes && input.take 8 = requestMagic &&
  sliceNat input RequestField.version.offset 2 = requestVersion &&
  sliceNat input RequestField.action.offset 1 = requestSettleAction &&
  (input.drop RequestField.reserved.offset).take 5 = List.replicate 5 0 &&
  ({
    generation := sliceNat input RequestField.generation.offset 8
    terminalSequence := sliceNat input RequestField.terminalSequence.offset 8
    parentReference := sliceNat input RequestField.parentReference.offset 32
    sourceMaterial := sliceNat input RequestField.sourceMaterial.offset 32
    sourceSpec := sliceNat input RequestField.sourceSpec.offset 32
  } : Request).valid

def requestRefusalCorpus : List (List UInt8) := [
  (encodeRequest requestExample).set 0 0,
  (encodeRequest requestExample).set RequestField.version.offset 2,
  (encodeRequest requestExample).set RequestField.action.offset 0,
  (encodeRequest requestExample).set RequestField.action.offset 2,
  (encodeRequest requestExample).set RequestField.reserved.offset 1,
  (encodeRequest requestExample).set RequestField.generation.offset 0,
  (encodeRequest requestExample).set RequestField.terminalSequence.offset 0,
  (encodeRequest requestExample).set RequestField.parentReference.offset 0,
  (encodeRequest requestExample).set RequestField.sourceMaterial.offset 0,
  (encodeRequest requestExample).set RequestField.sourceSpec.offset 0
]

/-- No request corpus row IS the valid request, for the reason
`record_refusal_corpus_moves_every_row` states. -/
theorem request_refusal_corpus_moves_every_row :
    requestRefusalCorpus.all fun candidate => candidate != encodeRequest requestExample := by
  native_decide

theorem request_example_bytes_valid : requestValidBytes (encodeRequest requestExample) = true := by
  native_decide

theorem request_refusal_corpus_refuses :
    requestRefusalCorpus.all fun candidate => !requestValidBytes candidate := by native_decide

/-- The record and the request magics differ from each other and from the
relay's and the provider's, so one dispatcher cannot confuse them.  The
provider magic `DCLTPRQ3` and the relay magic `DCLTRIX1` are restated as bytes
here because this module must not import the modules that own them; the census
gate (`magic-collisions.json`) is the tree-wide check. -/
theorem the_magics_are_distinct :
    recordMagic ≠ requestMagic ∧
    requestMagic ≠ [0x44, 0x43, 0x4c, 0x54, 0x50, 0x52, 0x51, 0x33] ∧
    requestMagic ≠ [0x44, 0x43, 0x4c, 0x54, 0x52, 0x49, 0x58, 0x31] := by native_decide

/-! ## The derived settle frame

The account list the `Settle` route presents, in order, with privileges.  It is
the same kind of fixed layout `CoreFoundFrameV3Abi` owns for `Found`: a slot
list, projected to counts and named indices by the emitter.  Parent `B`'s
three accounts are a SUFFIX so the off-condition settlement of a conditional
child — which reads `A` and does not read `B` (`off_condition_ignores_B`) —
presents one admissible width fewer.  Both widths admit the Clock, Rent and
System program at the same three positions BEFORE the optional suffix, so
nothing moves. -/

structure Slot where
  field : String
  label : String
  writable : Bool
  signer : Bool
  deriving DecidableEq, Repr

private def ro (field label : String) : Slot :=
  { field, label, writable := false, signer := false }

def settleFrame : List Slot := [
  { field := "worker", label := "worker (fee payer)", writable := true, signer := true },
  ro "market" "child Market",
  ro "core_program" "Core program",
  ro "activation_cache" "activation cache",
  { field := "source_state", label := "child Source state", writable := true, signer := false },
  { field := "certificate", label := "child certificate seat", writable := true, signer := false },
  ro "material.raw" "Source material raw",
  ro "material.staging" "Source material staging",
  ro "source_spec.raw" "Source spec raw",
  ro "source_spec.staging" "Source spec staging",
  ro "provider_release.raw" "provider release raw",
  ro "provider_release.staging" "provider release staging",
  ro "window_spec.raw" "window spec raw",
  ro "window_spec.staging" "window spec staging",
  ro "statistic_spec.raw" "statistic spec raw",
  ro "statistic_spec.staging" "statistic spec staging",
  ro "parent_reference.raw" "parent reference raw",
  ro "parent_reference.staging" "parent reference staging",
  ro "product.raw" "Product raw",
  ro "product.staging" "Product staging",
  ro "result_domain.raw" "result domain raw",
  ro "result_domain.staging" "result domain staging",
  ro "portfolio.raw" "portfolio raw",
  ro "portfolio.staging" "portfolio staging",
  ro "clock" "Clock sysvar",
  ro "rent" "Rent sysvar",
  ro "system_program" "System program",
  ro "parent_a.market" "parent A Market",
  ro "parent_a.source_state" "parent A Source state",
  ro "parent_a.certificate" "parent A certificate"
]

def parentBSuffix : List Slot := [
  ro "parent_b.market" "parent B Market",
  ro "parent_b.source_state" "parent B Source state",
  ro "parent_b.certificate" "parent B certificate"
]

def settleFrameWithB : List Slot := settleFrame ++ parentBSuffix

def settleAccountCount : Nat := settleFrameWithB.length
def settleOffConditionAccountCount : Nat := settleFrame.length

def indexOf? (field : String) : Option Nat :=
  settleFrameWithB.findIdx? (fun slot => slot.field == field)

def marketIndex : Nat := (indexOf? "market").getD 0
def sourceStateIndex : Nat := (indexOf? "source_state").getD 0
def certificateIndex : Nat := (indexOf? "certificate").getD 0
def materialRawIndex : Nat := (indexOf? "material.raw").getD 0
def parentReferenceRawIndex : Nat := (indexOf? "parent_reference.raw").getD 0
def productRawIndex : Nat := (indexOf? "product.raw").getD 0
def clockIndex : Nat := (indexOf? "clock").getD 0
def rentIndex : Nat := (indexOf? "rent").getD 0
def systemIndex : Nat := (indexOf? "system_program").getD 0
def parentAMarketIndex : Nat := (indexOf? "parent_a.market").getD 0
def parentBMarketIndex : Nat := (indexOf? "parent_b.market").getD 0
/-- The three accounts one parent presents: Market, Source state, certificate. -/
def parentSlotCount : Nat := parentBSuffix.length

theorem settle_named_indices_are_positions :
    settleAccountCount = 33 ∧ settleOffConditionAccountCount = 30 ∧
    marketIndex = 1 ∧ sourceStateIndex = 4 ∧ certificateIndex = 5 ∧
    materialRawIndex = 6 ∧ parentReferenceRawIndex = 16 ∧ productRawIndex = 18 ∧
    clockIndex = 24 ∧ rentIndex = 25 ∧ systemIndex = 26 ∧
    parentAMarketIndex = 27 ∧ parentBMarketIndex = 30 ∧ parentSlotCount = 3 := by
  native_decide

theorem parent_b_is_a_suffix :
    settleFrameWithB.take settleOffConditionAccountCount = settleFrame ∧
    settleFrameWithB.drop settleOffConditionAccountCount = parentBSuffix ∧
    parentBMarketIndex = parentAMarketIndex + parentSlotCount := by native_decide

theorem the_worker_is_the_only_signer :
    settleFrameWithB.filter (fun slot => slot.signer) =
      [{ field := "worker", label := "worker (fee payer)", writable := true, signer := true }] := by
  native_decide

theorem only_the_child_outputs_are_writable :
    (settleFrameWithB.filter (fun slot => slot.writable)).map (fun slot => slot.field) =
      ["worker", "source_state", "certificate"] := by native_decide

/-- The parents are READ.  Neither parent's Market, Source state nor
certificate is writable, which is the physical half of `childPayoutVector_sum`:
no parent class can move at a child boundary because no parent account can. -/
theorem no_parent_account_is_writable :
    (settleFrameWithB.drop parentAMarketIndex).all (fun slot => !slot.writable) := by
  native_decide

theorem settle_slots_are_uniquely_named :
    (settleFrameWithB.map (fun slot => slot.field)).Nodup ∧
    (settleFrameWithB.map (fun slot => slot.label)).Nodup := by native_decide

/-- Under the 64-lock wall (`PACKET_LIMIT_2026_09_01.md:240`) with room: the
provider route carries 47 or 51 and this carries 33. -/
theorem the_frame_is_under_the_lock_wall : settleAccountCount ≤ 64 := by native_decide

end DClutch.ParentReferenceV1Abi
