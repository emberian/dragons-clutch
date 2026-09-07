import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import DClutchSemantics.EnsembleResolutionV1
import Std.Tactic

/-!
# The ensemble fold's receipt

One fold decides an ensemble market (`EnsembleResolutionV1.fold`), and the
terminal certificate it writes keeps today's shape: kind `1`, `attempt_index`
`0`, the median as `result_numerator`, its cell as `selector`, and
`provider_evidence` folded over the consumed fragments in member order. What
the certificate cannot say is WHICH members answered, and the fragment seats
that could say it are closed at retirement. This record is the durable answer:
the member count, the quorum, the consumed bitmap and count, the median and its
cell, and the digest of every consumed fragment, written once by the fold into
its own seat beside the certificate.

A reader holding this receipt and the fold's evidence rule recomputes
`provider_evidence`; a reader holding the fragments recomputes the median
(`the_cell_of_the_median_is_the_median_of_the_cells`). The receipt is a
projection of facts the fold already committed and never a second authority
for any of them, which is why its own validity is stated as consistency
between its fields rather than as anything about a market.
-/

namespace DClutch.EnsembleFoldReceiptV1Abi

open DClutch
open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x45, 0x46, 0x52, 0x31] -- `DCLTEFR1`
def schemaVersion : Nat := 1

/-- The seat the fold writes this receipt into, under the Source state and the
terminal sequence: `dclutch/ensemble-fold-receipt/v1 ‖ source_state ‖ seq`. -/
def receiptPdaDomain : List UInt8 := "dclutch/ensemble-fold-receipt/v1".toUTF8.toList
/-- The seat one member's fragment is written into:
`dclutch/ensemble-fragment/v1 ‖ source_state ‖ [member] ‖ seq`. -/
def fragmentPdaDomain : List UInt8 := "dclutch/ensemble-fragment/v1".toUTF8.toList
/-- The domain the fold's `provider_evidence` is hashed under, over the consumed
count and every consumed fragment's evidence in member order. -/
def evidenceDomain : List UInt8 := "dclutch/ensemble-evidence/v1".toUTF8.toList

/-- The greatest `k`, from the one place it is defined. -/
def maxMembers : Nat := EnsembleResolutionV1.maxMembers

inductive Field where
  | magic | version | memberCount | quorum | consumedCount | consumedBitmap
  | reservedHeader | market | generation | terminalSequence | sourceMaterial
  | medianNumerator | selector | reservedSelector
  | fragmentDigest0 | fragmentDigest1 | fragmentDigest2 | fragmentDigest3 | fragmentDigest4
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.memberCount, .u8⟩,
  ⟨.quorum, .u8⟩,
  ⟨.consumedCount, .u8⟩,
  ⟨.consumedBitmap, .u8⟩,
  ⟨.reservedHeader, .reserved 2⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.terminalSequence, .u64⟩,
  ⟨.sourceMaterial, .bytes 32⟩,
  ⟨.medianNumerator, .bytes 16⟩,
  ⟨.selector, .u32⟩,
  ⟨.reservedSelector, .reserved 4⟩,
  ⟨.fragmentDigest0, .bytes 32⟩,
  ⟨.fragmentDigest1, .bytes 32⟩,
  ⟨.fragmentDigest2, .bytes 32⟩,
  ⟨.fragmentDigest3, .bytes 32⟩,
  ⟨.fragmentDigest4, .bytes 32⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

namespace Field

def rustName : Field → String
  | .magic => "ENSEMBLE_FOLD_RECEIPT_V1_MAGIC_OFFSET"
  | .version => "ENSEMBLE_FOLD_RECEIPT_V1_VERSION_OFFSET"
  | .memberCount => "ENSEMBLE_FOLD_RECEIPT_V1_MEMBER_COUNT_OFFSET"
  | .quorum => "ENSEMBLE_FOLD_RECEIPT_V1_QUORUM_OFFSET"
  | .consumedCount => "ENSEMBLE_FOLD_RECEIPT_V1_CONSUMED_COUNT_OFFSET"
  | .consumedBitmap => "ENSEMBLE_FOLD_RECEIPT_V1_CONSUMED_BITMAP_OFFSET"
  | .reservedHeader => "ENSEMBLE_FOLD_RECEIPT_V1_RESERVED_HEADER_OFFSET"
  | .market => "ENSEMBLE_FOLD_RECEIPT_V1_MARKET_OFFSET"
  | .generation => "ENSEMBLE_FOLD_RECEIPT_V1_GENERATION_OFFSET"
  | .terminalSequence => "ENSEMBLE_FOLD_RECEIPT_V1_TERMINAL_SEQUENCE_OFFSET"
  | .sourceMaterial => "ENSEMBLE_FOLD_RECEIPT_V1_SOURCE_MATERIAL_OFFSET"
  | .medianNumerator => "ENSEMBLE_FOLD_RECEIPT_V1_MEDIAN_NUMERATOR_OFFSET"
  | .selector => "ENSEMBLE_FOLD_RECEIPT_V1_SELECTOR_OFFSET"
  | .reservedSelector => "ENSEMBLE_FOLD_RECEIPT_V1_RESERVED_SELECTOR_OFFSET"
  | .fragmentDigest0 => "ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_0_OFFSET"
  | .fragmentDigest1 => "ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_1_OFFSET"
  | .fragmentDigest2 => "ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_2_OFFSET"
  | .fragmentDigest3 => "ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_3_OFFSET"
  | .fragmentDigest4 => "ENSEMBLE_FOLD_RECEIPT_V1_FRAGMENT_DIGEST_4_OFFSET"

def offset (field : Field) : Nat :=
  (coordinate? field layout).map (fun value => value.1) |>.getD 0

end Field

theorem exact_width : bytes = 280 := by native_decide

theorem schema_well_formed : WellFormed schema := by
  simp [WellFormed, schema, FieldKind.byteWidth]

theorem layout_is_byte_disjoint : layout.Pairwise Before :=
  specializeFrom_pairwise 0 schema

/-- The fragment digests are a fixed stride from the first, so a reader indexes
them by member rather than by name. -/
theorem fragment_digests_are_one_stride :
    Field.fragmentDigest1.offset = Field.fragmentDigest0.offset + 32 ∧
      Field.fragmentDigest4.offset = Field.fragmentDigest0.offset + 4 * 32 := by
  native_decide

/-- Whether member `member` is set in a consumed bitmap. -/
def consumed (bitmap member : Nat) : Bool := (bitmap / 2 ^ member) % 2 = 1

/-- The number of members a bitmap marks consumed, over the eight bits a byte
can hold. Total by construction: a fold over `List.range`. -/
def bitsSet (bitmap : Nat) : Nat :=
  (List.range 8).foldl (fun count bit => count + (bitmap / 2 ^ bit) % 2) 0

structure Receipt where
  memberCount : Nat
  quorum : Nat
  consumedCount : Nat
  consumedBitmap : Nat
  market : Nat
  generation : Nat
  terminalSequence : Nat
  sourceMaterial : Nat
  medianNumerator : Nat
  selector : Nat
  /-- Exactly `maxMembers` entries; a member that did not answer carries zero. -/
  fragmentDigests : List Nat
  deriving DecidableEq, Repr

def fitsId (value : Nat) : Bool := value < 256 ^ 32

/-- A consumed member's digest is nonzero and an unconsumed member's is zero:
the bitmap and the digests are one fact stated twice, and the record is
canonical only when they agree. -/
def digestsAgree (bitmap : Nat) (digests : List Nat) : Bool :=
  (List.range maxMembers).all fun member =>
    match digests[member]? with
    | some digest => fitsId digest && (consumed bitmap member == (digest != 0))
    | none => false

def Receipt.valid (value : Receipt) : Bool :=
  2 ≤ value.memberCount && value.memberCount ≤ maxMembers &&
  1 ≤ value.quorum && value.quorum ≤ value.memberCount &&
  value.quorum ≤ value.consumedCount && value.consumedCount ≤ value.memberCount &&
  value.consumedBitmap < 2 ^ value.memberCount &&
  bitsSet value.consumedBitmap = value.consumedCount &&
  value.market != 0 && fitsId value.market &&
  value.generation != 0 && value.generation < 256 ^ 8 &&
  value.terminalSequence != 0 && value.terminalSequence < 256 ^ 8 &&
  value.sourceMaterial != 0 && fitsId value.sourceMaterial &&
  value.medianNumerator < 256 ^ 16 &&
  value.selector < 256 &&
  value.fragmentDigests.length = maxMembers &&
  digestsAgree value.consumedBitmap value.fragmentDigests

def encode (value : Receipt) : List UInt8 :=
  magic ++ Codec.encodeLE 2 schemaVersion ++
  [UInt8.ofNat value.memberCount] ++ [UInt8.ofNat value.quorum] ++
  [UInt8.ofNat value.consumedCount] ++ [UInt8.ofNat value.consumedBitmap] ++
  List.replicate 2 0 ++
  Codec.encodeLE 32 value.market ++
  Codec.encodeLE 8 value.generation ++
  Codec.encodeLE 8 value.terminalSequence ++
  Codec.encodeLE 32 value.sourceMaterial ++
  Codec.encodeLE 16 value.medianNumerator ++
  Codec.encodeLE 4 value.selector ++
  List.replicate 4 0 ++
  ((value.fragmentDigests.take maxMembers).flatMap (Codec.encodeLE 32)) ++
  List.replicate ((maxMembers - value.fragmentDigests.length) * 32) 0

/-- Three of three members answered `k = 3, q = 3`; the median is the middle
reading and it fell in cell one. -/
def exampleReceipt : Receipt := {
  memberCount := 3
  quorum := 3
  consumedCount := 3
  consumedBitmap := 7
  market := 11
  generation := 7
  terminalSequence := 1
  sourceMaterial := 12
  medianNumerator := 10375000000
  selector := 1
  fragmentDigests := [21, 22, 23, 0, 0]
}

/-- Two of three answered under `q = 2`: member one was dark. -/
def partialReceipt : Receipt := {
  exampleReceipt with
  quorum := 2
  consumedCount := 2
  consumedBitmap := 5
  fragmentDigests := [21, 0, 23, 0, 0]
}

theorem example_valid : exampleReceipt.valid = true := by native_decide
theorem partial_valid : partialReceipt.valid = true := by native_decide
theorem example_encoding_length : (encode exampleReceipt).length = bytes := by native_decide
theorem partial_encoding_length : (encode partialReceipt).length = bytes := by native_decide

def sliceNat (input : List UInt8) (offset width : Nat) : Nat :=
  Codec.decodeLE ((input.drop offset).take width)

def decodedDigests (input : List UInt8) : List Nat :=
  (List.range maxMembers).map fun member =>
    sliceNat input (Field.fragmentDigest0.offset + member * 32) 32

def validBytes (input : List UInt8) : Bool :=
  let receipt : Receipt := {
    memberCount := sliceNat input Field.memberCount.offset 1
    quorum := sliceNat input Field.quorum.offset 1
    consumedCount := sliceNat input Field.consumedCount.offset 1
    consumedBitmap := sliceNat input Field.consumedBitmap.offset 1
    market := sliceNat input Field.market.offset 32
    generation := sliceNat input Field.generation.offset 8
    terminalSequence := sliceNat input Field.terminalSequence.offset 8
    sourceMaterial := sliceNat input Field.sourceMaterial.offset 32
    medianNumerator := sliceNat input Field.medianNumerator.offset 16
    selector := sliceNat input Field.selector.offset 4
    fragmentDigests := decodedDigests input
  }
  input.length = bytes && input.take 8 = magic &&
  sliceNat input Field.version.offset 2 = schemaVersion &&
  (input.drop Field.reservedHeader.offset).take 2 = List.replicate 2 0 &&
  (input.drop Field.reservedSelector.offset).take 4 = List.replicate 4 0 &&
  receipt.valid

def refusalCorpus : List (List UInt8) := [
  (encode exampleReceipt).set 0 0,
  (encode exampleReceipt).set Field.version.offset 2,
  -- `k = 1` is not an ensemble and writes no receipt.
  (encode exampleReceipt).set Field.memberCount.offset 1,
  -- `k = 6` exceeds the policy's capacity plus the primary.
  (encode exampleReceipt).set Field.memberCount.offset 6,
  -- a quorum of zero decides on nothing.
  (encode exampleReceipt).set Field.quorum.offset 0,
  -- fewer consumed than the quorum: that fold was the ladder's, not this one.
  (encode exampleReceipt).set Field.consumedCount.offset 2,
  -- a bitmap naming a member the count does not.
  (encode exampleReceipt).set Field.consumedBitmap.offset 15,
  -- a bitmap and a digest disagreeing about member two.
  (encode exampleReceipt).set Field.consumedBitmap.offset 3,
  (encode exampleReceipt).set Field.reservedHeader.offset 1,
  (encode exampleReceipt).set Field.market.offset 0,
  (encode exampleReceipt).set Field.generation.offset 0,
  (encode exampleReceipt).set Field.terminalSequence.offset 0,
  (encode exampleReceipt).set Field.sourceMaterial.offset 0,
  (encode exampleReceipt).set Field.reservedSelector.offset 1,
  -- a digest for a member the bitmap says did not answer.
  (encode partialReceipt).set Field.fragmentDigest1.offset 9,
  -- a consumed member with a zero digest.
  (encode exampleReceipt).set Field.fragmentDigest0.offset 0
]

theorem example_bytes_accepted : validBytes (encode exampleReceipt) = true := by native_decide
theorem partial_bytes_accepted : validBytes (encode partialReceipt) = true := by native_decide
theorem generated_refusal_corpus_refuses :
    refusalCorpus.all fun candidate => !validBytes candidate := by native_decide

/-- The consumed count is the bitmap's population: a valid receipt cannot say
"three answered" over a bitmap naming two. -/
theorem consumed_count_is_the_bitmap_population (value : Receipt) (valid : value.valid = true) :
    bitsSet value.consumedBitmap = value.consumedCount := by
  simp only [Receipt.valid, Bool.and_eq_true, decide_eq_true_eq] at valid
  omega

end DClutch.EnsembleFoldReceiptV1Abi
