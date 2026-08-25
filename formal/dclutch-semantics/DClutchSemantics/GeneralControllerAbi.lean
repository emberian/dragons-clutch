import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import DClutchSemantics.GeneralClearing

/-!
# Fixed-layout General controller ABI

This is the physical profile for the data-defined General clearing semantics.
It fixes capacities, widths, tags, and cursor coordinates without introducing
an outcome-width-specific transition family. Candidate and page accounts carry
the authenticated certificate data; requests carry only action and optimistic
concurrency coordinates. A future Solana adapter remains outside this module.
-/

namespace DClutch.General.ControllerAbi

open DClutch DClutch.AbiSchema DClutch.General

def abiVersion : Nat := 1
def maxOutcomes : Nat := 16
def maxExecutionsPerPage : Nat := 32
def maxPagesPerCandidate : Nat := 64
def maxSelectionCriteria : Nat := 16

def candidateMagic : List UInt8 :=
  [0x44, 0x43, 0x47, 0x43, 0x41, 0x4e, 0x44, 0x31] -- `DCGCAND1`
def pageMagic : List UInt8 :=
  [0x44, 0x43, 0x47, 0x50, 0x41, 0x47, 0x45, 0x31] -- `DCGPAGE1`
def policyMagic : List UInt8 :=
  [0x44, 0x43, 0x47, 0x50, 0x4f, 0x4c, 0x59, 0x31] -- `DCGPOLY1`
def selectionMagic : List UInt8 :=
  [0x44, 0x43, 0x47, 0x53, 0x45, 0x4c, 0x43, 0x31] -- `DCGSELC1`
def settlementMagic : List UInt8 :=
  [0x44, 0x43, 0x47, 0x53, 0x45, 0x54, 0x54, 0x31] -- `DCGSETT1`
def requestMagic : List UInt8 :=
  [0x44, 0x43, 0x47, 0x52, 0x45, 0x51, 0x30, 0x31] -- `DCGREQ01`

inductive CandidateField where
  | magic | version | outcomeCount | reservedA | candidateId | productId
  | batchId | pageCount | reservedB | priceScale | prices
  deriving DecidableEq, Repr

def candidateSchema : List (FieldSpec CandidateField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.outcomeCount, .u8⟩,
  ⟨.reservedA, .reserved 5⟩, ⟨.candidateId, .bytes 32⟩,
  ⟨.productId, .bytes 32⟩, ⟨.batchId, .bytes 32⟩,
  ⟨.pageCount, .u32⟩, ⟨.reservedB, .reserved 4⟩,
  ⟨.priceScale, .u64⟩, ⟨.prices, .nested (maxOutcomes * 8)⟩
]

inductive ExecutionField where
  | orderId | ownerId | nonce | maxLots | maxQuoteDebitPerLot | lots
  | quoteDebit | quoteCredit | receivePerLot | deliverPerLot
  deriving DecidableEq, Repr

def executionSchema : List (FieldSpec ExecutionField) := [
  ⟨.orderId, .bytes 32⟩, ⟨.ownerId, .bytes 32⟩,
  ⟨.nonce, .u64⟩, ⟨.maxLots, .u64⟩,
  ⟨.maxQuoteDebitPerLot, .u64⟩, ⟨.lots, .u64⟩,
  ⟨.quoteDebit, .u64⟩, ⟨.quoteCredit, .u64⟩,
  ⟨.receivePerLot, .nested (maxOutcomes * 8)⟩,
  ⟨.deliverPerLot, .nested (maxOutcomes * 8)⟩
]

def executionBytes : Nat := schemaWidth executionSchema

inductive PageField where
  | magic | version | outcomeCount | executionCount | reservedA
  | candidateId | pageIndex | pageCount | reservedB | executions
  deriving DecidableEq, Repr

def pageSchema : List (FieldSpec PageField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.outcomeCount, .u8⟩,
  ⟨.executionCount, .u8⟩, ⟨.reservedA, .reserved 4⟩,
  ⟨.candidateId, .bytes 32⟩, ⟨.pageIndex, .u32⟩,
  ⟨.pageCount, .u32⟩, ⟨.reservedB, .reserved 8⟩,
  ⟨.executions, .nested (maxExecutionsPerPage * executionBytes)⟩
]

inductive PolicyField where
  | magic | version | criterionCount | reserved | policyId | criteria
  deriving DecidableEq, Repr

def policySchema : List (FieldSpec PolicyField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.criterionCount, .u8⟩,
  ⟨.reserved, .reserved 5⟩, ⟨.policyId, .bytes 32⟩,
  ⟨.criteria, .nested maxSelectionCriteria⟩
]

inductive SelectionField where
  | magic | version | closed | hasBest | reservedA | batchId | policyId
  | bestCandidateId | revision | reservedB
  deriving DecidableEq, Repr

def selectionSchema : List (FieldSpec SelectionField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.closed, .u8⟩,
  ⟨.hasBest, .u8⟩, ⟨.reservedA, .reserved 4⟩,
  ⟨.batchId, .bytes 32⟩, ⟨.policyId, .bytes 32⟩,
  ⟨.bestCandidateId, .bytes 32⟩, ⟨.revision, .u64⟩,
  ⟨.reservedB, .reserved 8⟩
]

inductive SettlementField where
  | magic | version | phase | outcomeCount | reserved | candidateId
  | pageCount | nextPage | revision | claimInventory
  | quoteInventory | quoteSurplusPaid
  deriving DecidableEq, Repr

def settlementSchema : List (FieldSpec SettlementField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.phase, .u8⟩,
  ⟨.outcomeCount, .u8⟩, ⟨.reserved, .reserved 4⟩,
  ⟨.candidateId, .bytes 32⟩, ⟨.pageCount, .u32⟩,
  ⟨.nextPage, .u32⟩, ⟨.revision, .u64⟩,
  ⟨.claimInventory, .nested (maxOutcomes * 8)⟩,
  ⟨.quoteInventory, .u64⟩, ⟨.quoteSurplusPaid, .u64⟩
]

inductive RequestField where
  | magic | version | action | reservedA | expectedRevision
  | candidateId | pageIndex | reservedB
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩,
  ⟨.reservedA, .reserved 5⟩, ⟨.expectedRevision, .u64⟩,
  ⟨.candidateId, .bytes 32⟩, ⟨.pageIndex, .u32⟩,
  ⟨.reservedB, .reserved 4⟩
]

def candidateLayout := specialize candidateSchema
def executionLayout := specialize executionSchema
def pageLayout := specialize pageSchema
def policyLayout := specialize policySchema
def selectionLayout := specialize selectionSchema
def settlementLayout := specialize settlementSchema
def requestLayout := specialize requestSchema

def candidateBytes := schemaWidth candidateSchema
def pageBytes := schemaWidth pageSchema
def policyBytes := schemaWidth policySchema
def selectionBytes := schemaWidth selectionSchema
def settlementBytes := schemaWidth settlementSchema
def requestBytes := schemaWidth requestSchema

theorem exact_physical_widths :
    candidateBytes = 256 ∧ executionBytes = 368 ∧ pageBytes = 11840 ∧
    policyBytes = 64 ∧ selectionBytes = 128 ∧ settlementBytes = 208 ∧
    requestBytes = 64 := by
  native_decide

theorem schemas_well_formed :
    WellFormed candidateSchema ∧ WellFormed executionSchema ∧
    WellFormed pageSchema ∧ WellFormed policySchema ∧ WellFormed selectionSchema ∧
    WellFormed settlementSchema ∧ WellFormed requestSchema := by
  simp [WellFormed, candidateSchema, executionSchema, pageSchema, policySchema,
    selectionSchema, settlementSchema, requestSchema, FieldKind.byteWidth,
    maxOutcomes, maxExecutionsPerPage, maxSelectionCriteria, executionBytes, schemaWidth]

theorem layouts_are_byte_disjoint :
    candidateLayout.Pairwise Before ∧ executionLayout.Pairwise Before ∧
    pageLayout.Pairwise Before ∧ policyLayout.Pairwise Before ∧
    selectionLayout.Pairwise Before ∧
    settlementLayout.Pairwise Before ∧ requestLayout.Pairwise Before := by
  exact ⟨specializeFrom_pairwise 0 _, specializeFrom_pairwise 0 _,
    specializeFrom_pairwise 0 _, specializeFrom_pairwise 0 _,
    specializeFrom_pairwise 0 _,
    specializeFrom_pairwise 0 _, specializeFrom_pairwise 0 _⟩

theorem candidate_coordinates_are_canonical : coordinates candidateLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.outcomeCount, 10, 1),
    (.reservedA, 11, 5), (.candidateId, 16, 32), (.productId, 48, 32),
    (.batchId, 80, 32), (.pageCount, 112, 4), (.reservedB, 116, 4),
    (.priceScale, 120, 8), (.prices, 128, 128)] := by native_decide

theorem execution_coordinates_are_canonical : coordinates executionLayout = [
    (.orderId, 0, 32), (.ownerId, 32, 32), (.nonce, 64, 8),
    (.maxLots, 72, 8), (.maxQuoteDebitPerLot, 80, 8), (.lots, 88, 8),
    (.quoteDebit, 96, 8), (.quoteCredit, 104, 8),
    (.receivePerLot, 112, 128), (.deliverPerLot, 240, 128)] := by native_decide

theorem page_coordinates_are_canonical : coordinates pageLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.outcomeCount, 10, 1),
    (.executionCount, 11, 1), (.reservedA, 12, 4), (.candidateId, 16, 32),
    (.pageIndex, 48, 4), (.pageCount, 52, 4), (.reservedB, 56, 8),
    (.executions, 64, 11776)] := by native_decide

theorem cursor_coordinates_are_canonical :
    coordinates selectionLayout = [
      (.magic, 0, 8), (.version, 8, 2), (.closed, 10, 1),
      (.hasBest, 11, 1), (.reservedA, 12, 4), (.batchId, 16, 32),
      (.policyId, 48, 32), (.bestCandidateId, 80, 32),
      (.revision, 112, 8), (.reservedB, 120, 8)] ∧
    coordinates settlementLayout = [
      (.magic, 0, 8), (.version, 8, 2), (.phase, 10, 1),
      (.outcomeCount, 11, 1), (.reserved, 12, 4), (.candidateId, 16, 32),
      (.pageCount, 48, 4), (.nextPage, 52, 4), (.revision, 56, 8),
      (.claimInventory, 64, 128), (.quoteInventory, 192, 8),
      (.quoteSurplusPaid, 200, 8)] := by native_decide

theorem request_coordinates_are_canonical : coordinates requestLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.action, 10, 1),
    (.reservedA, 11, 5), (.expectedRevision, 16, 8),
    (.candidateId, 24, 32), (.pageIndex, 56, 4),
    (.reservedB, 60, 4)] := by native_decide

theorem policy_coordinates_are_canonical : coordinates policyLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.criterionCount, 10, 1),
    (.reserved, 11, 5), (.policyId, 16, 32), (.criteria, 48, 16)] := by
  native_decide

def fieldOffset [DecidableEq α] (αLayout : List (PlacedField α)) (name : α) : Nat :=
  (coordinate? name αLayout).map Prod.fst |>.getD 0

inductive Action where
  | consider | freeze | initializeSettlement | collect | materialize | distribute | close
  deriving DecidableEq, Repr

def Action.tag : Action → UInt8
  | .consider => 0 | .freeze => 1 | .initializeSettlement => 2
  | .collect => 3 | .materialize => 4 | .distribute => 5 | .close => 6

def actionOfTag : UInt8 → Option Action
  | 0 => some .consider | 1 => some .freeze | 2 => some .initializeSettlement
  | 3 => some .collect | 4 => some .materialize | 5 => some .distribute
  | 6 => some .close | _ => none

inductive Phase where
  | collecting | materializing | distributing | readyToClose | terminal
  deriving DecidableEq, Repr

def Phase.tag : Phase → UInt8
  | .collecting => 0 | .materializing => 1 | .distributing => 2
  | .readyToClose => 3 | .terminal => 4

def criterionTag : SelectionCriterion → UInt8
  | .maximizeFilledLots => 0
  | .minimizeQuoteSurplus => 1
  | .minimizeCandidateId => 2

structure CandidateDataV1 where
  outcomeCount : Nat
  candidateId : Nat
  productId : Nat
  batchId : Nat
  pageCount : Nat
  priceScale : Nat
  prices : List Nat
  deriving DecidableEq, Repr

structure ExecutionDataV1 where
  orderId : Nat
  ownerId : Nat
  nonce : Nat
  maxLots : Nat
  maxQuoteDebitPerLot : Nat
  lots : Nat
  quoteDebit : Nat
  quoteCredit : Nat
  receivePerLot : List Nat
  deliverPerLot : List Nat
  deriving DecidableEq, Repr

structure PageDataV1 where
  outcomeCount : Nat
  candidateId : Nat
  pageIndex : Nat
  pageCount : Nat
  executions : List ExecutionDataV1
  deriving DecidableEq, Repr

structure SelectionPolicyDataV1 where
  policyId : Nat
  criteria : List SelectionCriterion
  deriving DecidableEq, Repr

structure SelectionCursorV1 where
  closed : Bool
  batchId : Nat
  policyId : Nat
  bestCandidateId : Option Nat
  revision : Nat
  deriving DecidableEq, Repr

structure SettlementCursorV1 where
  phase : Phase
  outcomeCount : Nat
  candidateId : Nat
  pageCount : Nat
  nextPage : Nat
  revision : Nat
  claimInventory : List Nat
  quoteInventory : Nat
  quoteSurplusPaid : Nat
  deriving DecidableEq, Repr

structure ControllerRequestV1 where
  action : Action
  expectedRevision : Nat
  candidateId : Nat
  pageIndex : Nat
  deriving DecidableEq, Repr

def encodeVector (values : List Nat) : List UInt8 :=
  (values.take maxOutcomes ++ List.replicate (maxOutcomes - values.length) 0).flatMap
    (Codec.encodeLE 8)

def encodeCandidate (value : CandidateDataV1) : List UInt8 :=
  candidateMagic ++ Codec.encodeLE 2 abiVersion ++ [UInt8.ofNat value.outcomeCount] ++
  List.replicate 5 0 ++ Codec.encodeLE 32 value.candidateId ++
  Codec.encodeLE 32 value.productId ++ Codec.encodeLE 32 value.batchId ++
  Codec.encodeLE 4 value.pageCount ++ List.replicate 4 0 ++
  Codec.encodeLE 8 value.priceScale ++ encodeVector value.prices

def encodeExecution (value : ExecutionDataV1) : List UInt8 :=
  Codec.encodeLE 32 value.orderId ++ Codec.encodeLE 32 value.ownerId ++
  Codec.encodeLE 8 value.nonce ++ Codec.encodeLE 8 value.maxLots ++
  Codec.encodeLE 8 value.maxQuoteDebitPerLot ++ Codec.encodeLE 8 value.lots ++
  Codec.encodeLE 8 value.quoteDebit ++ Codec.encodeLE 8 value.quoteCredit ++
  encodeVector value.receivePerLot ++ encodeVector value.deliverPerLot

def emptyExecution : List UInt8 := List.replicate executionBytes 0

def encodePage (value : PageDataV1) : List UInt8 :=
  pageMagic ++ Codec.encodeLE 2 abiVersion ++ [UInt8.ofNat value.outcomeCount,
    UInt8.ofNat value.executions.length] ++ List.replicate 4 0 ++
  Codec.encodeLE 32 value.candidateId ++ Codec.encodeLE 4 value.pageIndex ++
  Codec.encodeLE 4 value.pageCount ++ List.replicate 8 0 ++
  (value.executions.take maxExecutionsPerPage).flatMap encodeExecution ++
  (List.replicate (maxExecutionsPerPage - value.executions.length) emptyExecution).flatten

def encodePolicy (value : SelectionPolicyDataV1) : List UInt8 :=
  policyMagic ++ Codec.encodeLE 2 abiVersion ++ [UInt8.ofNat value.criteria.length] ++
  List.replicate 5 0 ++ Codec.encodeLE 32 value.policyId ++
  (value.criteria.take maxSelectionCriteria).map criterionTag ++
  List.replicate (maxSelectionCriteria - value.criteria.length) 0

def encodeSelection (value : SelectionCursorV1) : List UInt8 :=
  selectionMagic ++ Codec.encodeLE 2 abiVersion ++ [if value.closed then 1 else 0,
    if value.bestCandidateId.isSome then 1 else 0] ++ List.replicate 4 0 ++
  Codec.encodeLE 32 value.batchId ++ Codec.encodeLE 32 value.policyId ++
  Codec.encodeLE 32 (value.bestCandidateId.getD 0) ++
  Codec.encodeLE 8 value.revision ++ List.replicate 8 0

def encodeSettlement (value : SettlementCursorV1) : List UInt8 :=
  settlementMagic ++ Codec.encodeLE 2 abiVersion ++
  [value.phase.tag, UInt8.ofNat value.outcomeCount] ++ List.replicate 4 0 ++
  Codec.encodeLE 32 value.candidateId ++ Codec.encodeLE 4 value.pageCount ++
  Codec.encodeLE 4 value.nextPage ++ Codec.encodeLE 8 value.revision ++
  encodeVector value.claimInventory ++ Codec.encodeLE 8 value.quoteInventory ++
  Codec.encodeLE 8 value.quoteSurplusPaid

def encodeRequest (value : ControllerRequestV1) : List UInt8 :=
  requestMagic ++ Codec.encodeLE 2 abiVersion ++ [value.action.tag] ++
  List.replicate 5 0 ++ Codec.encodeLE 8 value.expectedRevision ++
  Codec.encodeLE 32 value.candidateId ++ Codec.encodeLE 4 value.pageIndex ++
  List.replicate 4 0

def exampleCandidate : CandidateDataV1 := {
  outcomeCount := 2, candidateId := 0x11, productId := 0x22, batchId := 0x33,
  pageCount := 1, priceScale := 100, prices := [40, 60]
}

def exampleExecution : ExecutionDataV1 := {
  orderId := 0x44, ownerId := 0x55, nonce := 7, maxLots := 9,
  maxQuoteDebitPerLot := 40, lots := 2, quoteDebit := 80, quoteCredit := 0,
  receivePerLot := [2, 0], deliverPerLot := [0, 0]
}

def examplePage : PageDataV1 := {
  outcomeCount := 2, candidateId := 0x11, pageIndex := 0, pageCount := 1,
  executions := [exampleExecution]
}

def examplePolicy : SelectionPolicyDataV1 := {
  policyId := 0x66
  criteria := [.maximizeFilledLots, .minimizeQuoteSurplus, .minimizeCandidateId]
}

def exampleSelection : SelectionCursorV1 := {
  closed := true, batchId := 0x33, policyId := 0x66,
  bestCandidateId := some 0x11, revision := 2
}

def exampleSettlement : SettlementCursorV1 := {
  phase := .collecting, outcomeCount := 2, candidateId := 0x11,
  pageCount := 1, nextPage := 0, revision := 3, claimInventory := [0, 0],
  quoteInventory := 0, quoteSurplusPaid := 0
}

def exampleRequest : ControllerRequestV1 := {
  action := .collect, expectedRevision := 3, candidateId := 0x11, pageIndex := 0
}

theorem example_encoding_lengths :
    (encodeCandidate exampleCandidate).length = candidateBytes ∧
    (encodeExecution exampleExecution).length = executionBytes ∧
    (encodePage examplePage).length = pageBytes ∧
    (encodePolicy examplePolicy).length = policyBytes ∧
    (encodeSelection exampleSelection).length = selectionBytes ∧
    (encodeSettlement exampleSettlement).length = settlementBytes ∧
    (encodeRequest exampleRequest).length = requestBytes := by native_decide

/-- Exact header projection into the unbounded semantic candidate. Page and
execution content remain in separately authenticated page accounts. -/
def CandidateDataV1.refines (physical : CandidateDataV1) (semantic : Candidate) : Prop :=
  physical.outcomeCount = semantic.outcomeCount ∧
  physical.candidateId = semantic.candidateId ∧ physical.productId = semantic.productId ∧
  physical.batchId = semantic.batchId ∧ physical.pageCount = semantic.pages.length ∧
  physical.priceScale = semantic.prices.scale ∧ physical.prices = semantic.prices.coordinates

/-- The policy record is an exact physical projection of interpreted semantic
criterion data; the cursor stores only `policyId`, so it cannot become a
second owner for the criterion sequence. -/
def SelectionPolicyDataV1.refines
    (physical : SelectionPolicyDataV1) (semantic : SelectionPolicy) : Prop :=
  physical.policyId = semantic.policyId ∧ physical.criteria = semantic.criteria

theorem example_policy_refines_interpreted_data :
    examplePolicy.refines {
      policyId := 0x66
      criteria := [.maximizeFilledLots, .minimizeQuoteSurplus, .minimizeCandidateId]
    } := by
  exact ⟨rfl, rfl⟩

/-- Cursor tags are a lossless fixed-layout projection of the semantic phase. -/
def phaseCursor : SettlementPhase → Phase × Nat
  | .collecting next => (.collecting, next)
  | .materializing => (.materializing, 0)
  | .distributing next => (.distributing, next)
  | .readyToClose => (.readyToClose, 0)
  | .terminal => (.terminal, 0)

theorem semantic_phase_projection_is_data_defined (phase : SettlementPhase) :
    phaseCursor phase = match phase with
      | .collecting next => (.collecting, next)
      | .materializing => (.materializing, 0)
      | .distributing next => (.distributing, next)
      | .readyToClose => (.readyToClose, 0)
      | .terminal => (.terminal, 0) := by cases phase <;> rfl

end DClutch.General.ControllerAbi
