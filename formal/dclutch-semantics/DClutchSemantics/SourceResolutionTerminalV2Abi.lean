import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# Runtime-width Source terminal wire V2

The primary request carries only optimistic generation and provider-release
coordinates. Product authority comes from the authenticated `SourceMaterialV2`
graph root. The certificate binds that exact Product-record content digest and
stores the full `u32` selector; terminal admission joins it to an independently
authenticated Product outcome count.
-/

namespace DClutch.SourceResolutionTerminalV2Abi

open DClutch
open DClutch.AbiSchema

/-! ## Primary request -/

inductive RequestField where
  | magic | version | action | reserved | expectedGeneration | expectedProviderRelease
  deriving DecidableEq, Repr

def requestMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x41, 0x50, 0x56, 0x32]
def requestVersion : Nat := 2
def requestAction : Nat := 1

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.action, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.expectedGeneration, .u64⟩,
  ⟨.expectedProviderRelease, .bytes 32⟩
]

def requestLayout : List (PlacedField RequestField) := specialize requestSchema
def requestBytes : Nat := schemaWidth requestSchema

namespace RequestField

def rustName : RequestField → String
  | .magic => "ACCEPT_PYTH_V2_MAGIC_OFFSET"
  | .version => "ACCEPT_PYTH_V2_VERSION_OFFSET"
  | .action => "ACCEPT_PYTH_V2_ACTION_OFFSET"
  | .reserved => "ACCEPT_PYTH_V2_RESERVED_OFFSET"
  | .expectedGeneration => "ACCEPT_PYTH_V2_EXPECTED_GENERATION_OFFSET"
  | .expectedProviderRelease => "ACCEPT_PYTH_V2_EXPECTED_PROVIDER_RELEASE_OFFSET"

def offset (field : RequestField) : Nat :=
  (coordinate? field requestLayout).map (fun value => value.1) |>.getD 0

end RequestField

theorem request_exact_width : requestBytes = 56 := by native_decide
theorem request_layout_disjoint : requestLayout.Pairwise Before :=
  specializeFrom_pairwise 0 requestSchema

structure Request where
  expectedGeneration : Nat
  expectedProviderRelease : Nat
  deriving DecidableEq, Repr

def Request.valid (value : Request) : Bool :=
  value.expectedGeneration != 0 && value.expectedGeneration < 256 ^ 8 &&
  value.expectedProviderRelease != 0 && value.expectedProviderRelease < 256 ^ 32

def encodeRequest (value : Request) : List UInt8 :=
  requestMagic ++ Codec.encodeLE 2 requestVersion ++ [UInt8.ofNat requestAction] ++
  List.replicate 5 0 ++ Codec.encodeLE 8 value.expectedGeneration ++
  Codec.encodeLE 32 value.expectedProviderRelease

theorem request_encoding_length (value : Request) :
    (encodeRequest value).length = requestBytes := by
  simp [encodeRequest, requestBytes, requestSchema, schemaWidth, requestMagic,
    Codec.encodeLE_length, FieldKind.byteWidth]

def requestExample : Request := { expectedGeneration := 9, expectedProviderRelease := 1 }
theorem request_example_valid : requestExample.valid = true := by native_decide

def sliceNat (input : List UInt8) (offset width : Nat) : Nat :=
  Codec.decodeLE ((input.drop offset).take width)

def requestValidBytes (input : List UInt8) : Bool :=
  input.length = requestBytes && input.take 8 = requestMagic &&
  sliceNat input RequestField.version.offset 2 = requestVersion &&
  sliceNat input RequestField.action.offset 1 = requestAction &&
  (input.drop RequestField.reserved.offset).take 5 = List.replicate 5 0 &&
  ({
    expectedGeneration := sliceNat input RequestField.expectedGeneration.offset 8
    expectedProviderRelease := sliceNat input RequestField.expectedProviderRelease.offset 32
  } : Request).valid

def requestRefusalCorpus : List (List UInt8) := [
  (encodeRequest requestExample).set 0 0,
  (encodeRequest requestExample).set RequestField.version.offset 3,
  (encodeRequest requestExample).set RequestField.action.offset 2,
  (encodeRequest requestExample).set RequestField.reserved.offset 1,
  (encodeRequest requestExample).set RequestField.expectedGeneration.offset 0,
  (encodeRequest requestExample).set RequestField.expectedProviderRelease.offset 0
]

theorem request_example_bytes_valid : requestValidBytes (encodeRequest requestExample) = true := by
  native_decide

theorem request_refusal_corpus_refuses :
    requestRefusalCorpus.all fun candidate => !requestValidBytes candidate := by native_decide

/-! ## Terminal certificate -/

inductive CertificateField where
  | magic | version | kind | reservedHeader
  | market | route | sourceMaterial | productRecord | providerEvidence
  | fundingAllocation | receiptAccount
  | generation | attemptIndex | scheduleIndex | selector | reservedBody
  | workPaid | fundingRemaining | resultNumerator | resultDenominator | observedAt
  deriving DecidableEq, Repr

def certificateMagic : List UInt8 :=
  [0x44, 0x43, 0x53, 0x52, 0x43, 0x45, 0x52, 0x32]
def certificateVersion : Nat := 2
def certificateSuccessKind : Nat := 1
def certificateRecoveryAdvancedKind : Nat := 2
def certificateExhaustedKind : Nat := 3
def certificateFailureKind : Nat := 4

def certificateSchema : List (FieldSpec CertificateField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.kind, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.route, .bytes 32⟩,
  ⟨.sourceMaterial, .bytes 32⟩,
  ⟨.productRecord, .bytes 32⟩,
  ⟨.providerEvidence, .bytes 32⟩,
  ⟨.fundingAllocation, .bytes 32⟩,
  ⟨.receiptAccount, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.attemptIndex, .u32⟩,
  ⟨.scheduleIndex, .u32⟩,
  ⟨.selector, .u32⟩,
  ⟨.reservedBody, .reserved 4⟩,
  ⟨.workPaid, .u64⟩,
  ⟨.fundingRemaining, .u64⟩,
  ⟨.resultNumerator, .bytes 16⟩,
  ⟨.resultDenominator, .u64⟩,
  ⟨.observedAt, .u64⟩
]

def certificateLayout : List (PlacedField CertificateField) := specialize certificateSchema
def certificateBytes : Nat := schemaWidth certificateSchema

namespace CertificateField

def rustName : CertificateField → String
  | .magic => "CERTIFICATE_V2_MAGIC_OFFSET"
  | .version => "CERTIFICATE_V2_VERSION_OFFSET"
  | .kind => "CERTIFICATE_V2_KIND_OFFSET"
  | .reservedHeader => "CERTIFICATE_V2_RESERVED_HEADER_OFFSET"
  | .market => "CERTIFICATE_V2_MARKET_OFFSET"
  | .route => "CERTIFICATE_V2_ROUTE_OFFSET"
  | .sourceMaterial => "CERTIFICATE_V2_SOURCE_MATERIAL_OFFSET"
  | .productRecord => "CERTIFICATE_V2_PRODUCT_RECORD_OFFSET"
  | .providerEvidence => "CERTIFICATE_V2_PROVIDER_EVIDENCE_OFFSET"
  | .fundingAllocation => "CERTIFICATE_V2_FUNDING_ALLOCATION_OFFSET"
  | .receiptAccount => "CERTIFICATE_V2_RECEIPT_ACCOUNT_OFFSET"
  | .generation => "CERTIFICATE_V2_GENERATION_OFFSET"
  | .attemptIndex => "CERTIFICATE_V2_ATTEMPT_INDEX_OFFSET"
  | .scheduleIndex => "CERTIFICATE_V2_SCHEDULE_INDEX_OFFSET"
  | .selector => "CERTIFICATE_V2_SELECTOR_OFFSET"
  | .reservedBody => "CERTIFICATE_V2_RESERVED_BODY_OFFSET"
  | .workPaid => "CERTIFICATE_V2_WORK_PAID_OFFSET"
  | .fundingRemaining => "CERTIFICATE_V2_FUNDING_REMAINING_OFFSET"
  | .resultNumerator => "CERTIFICATE_V2_RESULT_NUMERATOR_OFFSET"
  | .resultDenominator => "CERTIFICATE_V2_RESULT_DENOMINATOR_OFFSET"
  | .observedAt => "CERTIFICATE_V2_OBSERVED_AT_OFFSET"

def offset (field : CertificateField) : Nat :=
  (coordinate? field certificateLayout).map (fun value => value.1) |>.getD 0

end CertificateField

theorem certificate_exact_width : certificateBytes = 312 := by native_decide
theorem certificate_layout_disjoint : certificateLayout.Pairwise Before :=
  specializeFrom_pairwise 0 certificateSchema

structure Certificate where
  kind : Nat
  market : Nat
  route : Nat
  sourceMaterial : Nat
  productRecord : Nat
  providerEvidence : Nat
  fundingAllocation : Nat
  receiptAccount : Nat
  generation : Nat
  attemptIndex : Nat
  scheduleIndex : Nat
  selector : Nat
  workPaid : Nat
  fundingRemaining : Nat
  resultNumerator : Nat
  resultDenominator : Nat
  observedAt : Nat
  deriving DecidableEq, Repr

def Certificate.valid (value : Certificate) : Bool :=
  value.kind ≥ 1 && value.kind ≤ 4 &&
  value.market != 0 && value.market < 256 ^ 32 &&
  value.sourceMaterial != 0 && value.sourceMaterial < 256 ^ 32 &&
  value.productRecord != 0 && value.productRecord < 256 ^ 32 &&
  value.receiptAccount != 0 && value.receiptAccount < 256 ^ 32 &&
  value.generation != 0 && value.generation < 256 ^ 8 &&
  value.selector < 256 ^ 4 && value.attemptIndex < 256 ^ 4 &&
  value.scheduleIndex < 256 ^ 4 && value.workPaid < 256 ^ 8 &&
  value.fundingRemaining < 256 ^ 8 && value.resultNumerator < 256 ^ 16 &&
  value.resultDenominator < 256 ^ 8 && value.observedAt < 256 ^ 8 &&
  match value.kind with
  | 1 => value.route != 0 && value.providerEvidence != 0 &&
      value.resultDenominator != 0 && value.observedAt != 0
  | 2 | 3 => value.route != 0 && value.providerEvidence = 0 &&
      value.fundingAllocation != 0 && value.selector = 0 && value.workPaid != 0 &&
      value.resultNumerator = 0 && value.resultDenominator = 0 && value.observedAt != 0
  | 4 => value.route = 0 && value.providerEvidence = 0 &&
      value.fundingAllocation != 0 && value.workPaid != 0 &&
      value.scheduleIndex = 0 && value.resultNumerator = 0 &&
      value.resultDenominator = 0 && value.observedAt = 0
  | _ => false

def encodeCertificate (value : Certificate) : List UInt8 :=
  certificateMagic ++ Codec.encodeLE 2 certificateVersion ++ [UInt8.ofNat value.kind] ++
  List.replicate 5 0 ++ Codec.encodeLE 32 value.market ++ Codec.encodeLE 32 value.route ++
  Codec.encodeLE 32 value.sourceMaterial ++ Codec.encodeLE 32 value.productRecord ++
  Codec.encodeLE 32 value.providerEvidence ++ Codec.encodeLE 32 value.fundingAllocation ++
  Codec.encodeLE 32 value.receiptAccount ++ Codec.encodeLE 8 value.generation ++
  Codec.encodeLE 4 value.attemptIndex ++ Codec.encodeLE 4 value.scheduleIndex ++
  Codec.encodeLE 4 value.selector ++ List.replicate 4 0 ++
  Codec.encodeLE 8 value.workPaid ++ Codec.encodeLE 8 value.fundingRemaining ++
  Codec.encodeLE 16 value.resultNumerator ++ Codec.encodeLE 8 value.resultDenominator ++
  Codec.encodeLE 8 value.observedAt

theorem certificate_encoding_length (value : Certificate) :
    (encodeCertificate value).length = certificateBytes := by
  simp [encodeCertificate, certificateBytes, certificateSchema, schemaWidth,
    certificateMagic, Codec.encodeLE_length, FieldKind.byteWidth]

def wideSuccess : Certificate := {
  kind := 1, market := 1, route := 2, sourceMaterial := 3, productRecord := 4
  providerEvidence := 5, fundingAllocation := 0, receiptAccount := 6
  generation := 9, attemptIndex := 0, scheduleIndex := 0, selector := 257
  workPaid := 0, fundingRemaining := 0, resultNumerator := 7
  resultDenominator := 1, observedAt := 100
}

def wideFailure : Certificate := {
  wideSuccess with
    kind := 4
    route := 0
    providerEvidence := 0
    fundingAllocation := 7
    selector := 257
    workPaid := 1
    resultNumerator := 0
    resultDenominator := 0
    observedAt := 0
}

def terminalJoin (certificate : Certificate) (authenticatedProductRecord outcomeCount : Nat) : Bool :=
  certificate.valid && certificate.productRecord = authenticatedProductRecord &&
  outcomeCount ≥ 2 && outcomeCount < 256 ^ 4 &&
  match certificate.kind with
  | 1 => certificate.selector < outcomeCount - 1
  | 4 => certificate.selector + 1 = outcomeCount
  | _ => false

theorem wide_success_is_native_u32 : terminalJoin wideSuccess 4 259 = true := by native_decide
theorem wide_failure_is_exact_last_cell : terminalJoin wideFailure 4 258 = true := by native_decide
theorem substituted_product_refuses : terminalJoin wideSuccess 5 259 = false := by native_decide
theorem selector_equal_to_success_width_refuses : terminalJoin wideSuccess 4 258 = false := by
  native_decide

def certificateRefusalCorpus : List (List UInt8) := [
  (encodeCertificate wideSuccess).set 0 0,
  (encodeCertificate wideSuccess).set CertificateField.version.offset 3,
  (encodeCertificate wideSuccess).set CertificateField.kind.offset 9,
  (encodeCertificate wideSuccess).set CertificateField.reservedHeader.offset 1,
  (encodeCertificate wideSuccess).set CertificateField.market.offset 0,
  (encodeCertificate wideSuccess).set CertificateField.sourceMaterial.offset 0,
  (encodeCertificate wideSuccess).set CertificateField.productRecord.offset 0,
  (encodeCertificate wideSuccess).set CertificateField.providerEvidence.offset 0,
  (encodeCertificate wideSuccess).set CertificateField.receiptAccount.offset 0,
  (encodeCertificate wideSuccess).set CertificateField.generation.offset 0,
  (encodeCertificate wideSuccess).set CertificateField.reservedBody.offset 1,
  (encodeCertificate wideSuccess).set CertificateField.resultDenominator.offset 0,
  (encodeCertificate wideSuccess).set CertificateField.observedAt.offset 0
]

def certificateValidBytes (input : List UInt8) : Bool :=
  input.length = certificateBytes && input.take 8 = certificateMagic &&
  sliceNat input CertificateField.version.offset 2 = certificateVersion &&
  (input.drop CertificateField.reservedHeader.offset).take 5 = List.replicate 5 0 &&
  (input.drop CertificateField.reservedBody.offset).take 4 = List.replicate 4 0 &&
  ({
    kind := sliceNat input CertificateField.kind.offset 1
    market := sliceNat input CertificateField.market.offset 32
    route := sliceNat input CertificateField.route.offset 32
    sourceMaterial := sliceNat input CertificateField.sourceMaterial.offset 32
    productRecord := sliceNat input CertificateField.productRecord.offset 32
    providerEvidence := sliceNat input CertificateField.providerEvidence.offset 32
    fundingAllocation := sliceNat input CertificateField.fundingAllocation.offset 32
    receiptAccount := sliceNat input CertificateField.receiptAccount.offset 32
    generation := sliceNat input CertificateField.generation.offset 8
    attemptIndex := sliceNat input CertificateField.attemptIndex.offset 4
    scheduleIndex := sliceNat input CertificateField.scheduleIndex.offset 4
    selector := sliceNat input CertificateField.selector.offset 4
    workPaid := sliceNat input CertificateField.workPaid.offset 8
    fundingRemaining := sliceNat input CertificateField.fundingRemaining.offset 8
    resultNumerator := sliceNat input CertificateField.resultNumerator.offset 16
    resultDenominator := sliceNat input CertificateField.resultDenominator.offset 8
    observedAt := sliceNat input CertificateField.observedAt.offset 8
  } : Certificate).valid

theorem certificate_examples_valid :
    certificateValidBytes (encodeCertificate wideSuccess) = true ∧
    certificateValidBytes (encodeCertificate wideFailure) = true := by native_decide

theorem certificate_refusal_corpus_refuses :
    certificateRefusalCorpus.all fun candidate => !certificateValidBytes candidate := by
  native_decide

/-! ## Source closure receipt -/

inductive ClosureField where
  | magic | version | kind | reservedHeader
  | market | sourceState | sourceMaterial | capabilityManifest
  | terminalCertificate | receiptAccount | beneficiary
  | sourceStateDigest | terminalCertificateDigest | fundingSetDigest
  | generation | terminalSequence | fundingCount | selector
  | refundLamports | closedAt | reservedBody
  deriving DecidableEq, Repr

def closureMagic : List UInt8 :=
  [0x44, 0x43, 0x53, 0x52, 0x43, 0x4c, 0x53, 0x32]
def closureVersion : Nat := 2
def closureKind : Nat := 1
def closureFundingCount : Nat := 3

def closureSchema : List (FieldSpec ClosureField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.kind, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.sourceState, .bytes 32⟩,
  ⟨.sourceMaterial, .bytes 32⟩,
  ⟨.capabilityManifest, .bytes 32⟩,
  ⟨.terminalCertificate, .bytes 32⟩,
  ⟨.receiptAccount, .bytes 32⟩,
  ⟨.beneficiary, .bytes 32⟩,
  ⟨.sourceStateDigest, .bytes 32⟩,
  ⟨.terminalCertificateDigest, .bytes 32⟩,
  ⟨.fundingSetDigest, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.terminalSequence, .u64⟩,
  ⟨.fundingCount, .u32⟩,
  ⟨.selector, .u32⟩,
  ⟨.refundLamports, .u64⟩,
  ⟨.closedAt, .u64⟩,
  ⟨.reservedBody, .reserved 8⟩
]

def closureLayout : List (PlacedField ClosureField) := specialize closureSchema
def closureBytes : Nat := schemaWidth closureSchema

namespace ClosureField

def rustName : ClosureField → String
  | .magic => "CLOSURE_V2_MAGIC_OFFSET"
  | .version => "CLOSURE_V2_VERSION_OFFSET"
  | .kind => "CLOSURE_V2_KIND_OFFSET"
  | .reservedHeader => "CLOSURE_V2_RESERVED_HEADER_OFFSET"
  | .market => "CLOSURE_V2_MARKET_OFFSET"
  | .sourceState => "CLOSURE_V2_SOURCE_STATE_OFFSET"
  | .sourceMaterial => "CLOSURE_V2_SOURCE_MATERIAL_OFFSET"
  | .capabilityManifest => "CLOSURE_V2_CAPABILITY_MANIFEST_OFFSET"
  | .terminalCertificate => "CLOSURE_V2_TERMINAL_CERTIFICATE_OFFSET"
  | .receiptAccount => "CLOSURE_V2_RECEIPT_ACCOUNT_OFFSET"
  | .beneficiary => "CLOSURE_V2_BENEFICIARY_OFFSET"
  | .sourceStateDigest => "CLOSURE_V2_SOURCE_STATE_DIGEST_OFFSET"
  | .terminalCertificateDigest => "CLOSURE_V2_TERMINAL_CERTIFICATE_DIGEST_OFFSET"
  | .fundingSetDigest => "CLOSURE_V2_FUNDING_SET_DIGEST_OFFSET"
  | .generation => "CLOSURE_V2_GENERATION_OFFSET"
  | .terminalSequence => "CLOSURE_V2_TERMINAL_SEQUENCE_OFFSET"
  | .fundingCount => "CLOSURE_V2_FUNDING_COUNT_OFFSET"
  | .selector => "CLOSURE_V2_SELECTOR_OFFSET"
  | .refundLamports => "CLOSURE_V2_REFUND_LAMPORTS_OFFSET"
  | .closedAt => "CLOSURE_V2_CLOSED_AT_OFFSET"
  | .reservedBody => "CLOSURE_V2_RESERVED_BODY_OFFSET"

def offset (field : ClosureField) : Nat :=
  (coordinate? field closureLayout).map (fun value => value.1) |>.getD 0

end ClosureField

theorem closure_exact_width : closureBytes = 384 := by native_decide
theorem closure_layout_disjoint : closureLayout.Pairwise Before :=
  specializeFrom_pairwise 0 closureSchema

structure Closure where
  market : Nat
  sourceState : Nat
  sourceMaterial : Nat
  capabilityManifest : Nat
  terminalCertificate : Nat
  receiptAccount : Nat
  beneficiary : Nat
  sourceStateDigest : Nat
  terminalCertificateDigest : Nat
  fundingSetDigest : Nat
  generation : Nat
  terminalSequence : Nat
  selector : Nat
  refundLamports : Nat
  closedAt : Nat
  deriving DecidableEq, Repr

def Closure.valid (value : Closure) : Bool :=
  value.market != 0 && value.market < 256 ^ 32 &&
  value.sourceState != 0 && value.sourceState < 256 ^ 32 &&
  value.sourceMaterial != 0 && value.sourceMaterial < 256 ^ 32 &&
  value.capabilityManifest != 0 && value.capabilityManifest < 256 ^ 32 &&
  value.terminalCertificate != 0 && value.terminalCertificate < 256 ^ 32 &&
  value.receiptAccount != 0 && value.receiptAccount < 256 ^ 32 &&
  value.beneficiary != 0 && value.beneficiary < 256 ^ 32 &&
  value.sourceStateDigest != 0 && value.sourceStateDigest < 256 ^ 32 &&
  value.terminalCertificateDigest != 0 && value.terminalCertificateDigest < 256 ^ 32 &&
  value.fundingSetDigest != 0 && value.fundingSetDigest < 256 ^ 32 &&
  value.generation != 0 && value.generation < 256 ^ 8 &&
  value.terminalSequence != 0 && value.terminalSequence < 256 ^ 8 &&
  value.selector < 256 ^ 4 &&
  value.refundLamports != 0 && value.refundLamports < 256 ^ 8 &&
  value.closedAt != 0 && value.closedAt < 256 ^ 8

def encodeClosure (value : Closure) : List UInt8 :=
  closureMagic ++ Codec.encodeLE 2 closureVersion ++ [UInt8.ofNat closureKind] ++
  List.replicate 5 0 ++ Codec.encodeLE 32 value.market ++
  Codec.encodeLE 32 value.sourceState ++ Codec.encodeLE 32 value.sourceMaterial ++
  Codec.encodeLE 32 value.capabilityManifest ++
  Codec.encodeLE 32 value.terminalCertificate ++ Codec.encodeLE 32 value.receiptAccount ++
  Codec.encodeLE 32 value.beneficiary ++ Codec.encodeLE 32 value.sourceStateDigest ++
  Codec.encodeLE 32 value.terminalCertificateDigest ++
  Codec.encodeLE 32 value.fundingSetDigest ++ Codec.encodeLE 8 value.generation ++
  Codec.encodeLE 8 value.terminalSequence ++ Codec.encodeLE 4 closureFundingCount ++
  Codec.encodeLE 4 value.selector ++ Codec.encodeLE 8 value.refundLamports ++
  Codec.encodeLE 8 value.closedAt ++ List.replicate 8 0

theorem closure_encoding_length (value : Closure) :
    (encodeClosure value).length = closureBytes := by
  simp [encodeClosure, closureBytes, closureSchema, schemaWidth, closureMagic,
    Codec.encodeLE_length, FieldKind.byteWidth]

def wideClosure : Closure := {
  market := 1, sourceState := 2, sourceMaterial := 3, capabilityManifest := 4
  terminalCertificate := 5, receiptAccount := 6, beneficiary := 7
  sourceStateDigest := 8, terminalCertificateDigest := 9, fundingSetDigest := 10
  generation := 11, terminalSequence := 12, selector := 257
  refundLamports := 13, closedAt := 14
}

def closureValidBytes (input : List UInt8) : Bool :=
  input.length = closureBytes && input.take 8 = closureMagic &&
  sliceNat input ClosureField.version.offset 2 = closureVersion &&
  sliceNat input ClosureField.kind.offset 1 = closureKind &&
  (input.drop ClosureField.reservedHeader.offset).take 5 = List.replicate 5 0 &&
  sliceNat input ClosureField.fundingCount.offset 4 = closureFundingCount &&
  (input.drop ClosureField.reservedBody.offset).take 8 = List.replicate 8 0 &&
  ({
    market := sliceNat input ClosureField.market.offset 32
    sourceState := sliceNat input ClosureField.sourceState.offset 32
    sourceMaterial := sliceNat input ClosureField.sourceMaterial.offset 32
    capabilityManifest := sliceNat input ClosureField.capabilityManifest.offset 32
    terminalCertificate := sliceNat input ClosureField.terminalCertificate.offset 32
    receiptAccount := sliceNat input ClosureField.receiptAccount.offset 32
    beneficiary := sliceNat input ClosureField.beneficiary.offset 32
    sourceStateDigest := sliceNat input ClosureField.sourceStateDigest.offset 32
    terminalCertificateDigest := sliceNat input ClosureField.terminalCertificateDigest.offset 32
    fundingSetDigest := sliceNat input ClosureField.fundingSetDigest.offset 32
    generation := sliceNat input ClosureField.generation.offset 8
    terminalSequence := sliceNat input ClosureField.terminalSequence.offset 8
    selector := sliceNat input ClosureField.selector.offset 4
    refundLamports := sliceNat input ClosureField.refundLamports.offset 8
    closedAt := sliceNat input ClosureField.closedAt.offset 8
  } : Closure).valid

def closureRefusalCorpus : List (List UInt8) := [
  (encodeClosure wideClosure).set 0 0,
  (encodeClosure wideClosure).set ClosureField.version.offset 3,
  (encodeClosure wideClosure).set ClosureField.kind.offset 2,
  (encodeClosure wideClosure).set ClosureField.reservedHeader.offset 1,
  (encodeClosure wideClosure).set ClosureField.market.offset 0,
  (encodeClosure wideClosure).set ClosureField.sourceState.offset 0,
  (encodeClosure wideClosure).set ClosureField.sourceMaterial.offset 0,
  (encodeClosure wideClosure).set ClosureField.capabilityManifest.offset 0,
  (encodeClosure wideClosure).set ClosureField.terminalCertificate.offset 0,
  (encodeClosure wideClosure).set ClosureField.receiptAccount.offset 0,
  (encodeClosure wideClosure).set ClosureField.beneficiary.offset 0,
  (encodeClosure wideClosure).set ClosureField.sourceStateDigest.offset 0,
  (encodeClosure wideClosure).set ClosureField.terminalCertificateDigest.offset 0,
  (encodeClosure wideClosure).set ClosureField.fundingSetDigest.offset 0,
  (encodeClosure wideClosure).set ClosureField.generation.offset 0,
  (encodeClosure wideClosure).set ClosureField.terminalSequence.offset 0,
  (encodeClosure wideClosure).set ClosureField.fundingCount.offset 2,
  (encodeClosure wideClosure).set ClosureField.refundLamports.offset 0,
  (encodeClosure wideClosure).set ClosureField.closedAt.offset 0,
  (encodeClosure wideClosure).set ClosureField.reservedBody.offset 1
]

theorem wide_closure_preserves_selector_257 :
    closureValidBytes (encodeClosure wideClosure) = true := by native_decide

theorem closure_refusal_corpus_refuses :
    closureRefusalCorpus.all fun candidate => !closureValidBytes candidate := by native_decide

end DClutch.SourceResolutionTerminalV2Abi
