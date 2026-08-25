import DClutchSemantics.SourceResolutionAbi

/-!
# Physical Source Resolution controller ABI

The physical Resolution-role specialization admits one already-posted, fully
verified Pyth terminal observation and the bounded funded liveness transitions
which advance ordered recovery or commit Product-owned failure.  Requests
carry only optimistic concurrency coordinates.  Market, Source material,
Product result-domain, capability funding, provider release, observation, and
Clock truth remain in authenticated accounts and are not copied into requests
as caller-selected policy.

The successful output is exactly the 312-byte certificate schema already
owned by `SourceResolutionAbi`.  This module adds no second receipt layout; it
only exposes the existing cursor-derived coordinates to the Rust generator.
-/

namespace DClutch.SourceResolution.ControllerAbi

open DClutch DClutch.AbiSchema
open DClutch.SourceResolution
open DClutch.SourceResolution.Abi

def requestMagic : List UInt8 :=
  [0x44, 0x43, 0x53, 0x52, 0x50, 0x59, 0x54, 0x31] -- `DCSRPYT1`

def requestVersion : Nat := 1
def acceptPythAction : UInt8 := 0

/-- A primary Source has transition sequence zero by canonical state shape, so
its one terminal success certificate is constructible before execution at the
immediate successor sequence.  Clock slot remains observation evidence only. -/
def primaryCertificateSequence (state : State) : Nat :=
  state.transitionSequence + 1

theorem primary_certificate_sequence_is_first
    {state : State} (hsequence : state.transitionSequence = 0) :
    primaryCertificateSequence state = 1 := by
  simp [primaryCertificateSequence, hsequence]

#guard primaryCertificateSequence exampleState == 1

inductive RequestField where
  | magic | version | action | reserved | expectedGeneration
  | expectedResultDomainId | expectedProviderReleaseId
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.action, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.expectedGeneration, .u64⟩,
  ⟨.expectedResultDomainId, .bytes 32⟩,
  ⟨.expectedProviderReleaseId, .bytes 32⟩
]

def requestLayout : List (PlacedField RequestField) :=
  DClutch.AbiSchema.specialize requestSchema
def requestBytes : Nat := schemaWidth requestSchema

namespace RequestField

def all : List RequestField := [
  .magic, .version, .action, .reserved, .expectedGeneration,
  .expectedResultDomainId, .expectedProviderReleaseId
]

def coordinate (field : RequestField) : Nat × Nat :=
  (coordinate? field requestLayout).getD (0, 0)

def offset (field : RequestField) : Nat := (coordinate field).1
def width (field : RequestField) : Nat := (coordinate field).2

def rustName : RequestField → String
  | .magic => "REQUEST_MAGIC_OFFSET"
  | .version => "REQUEST_VERSION_OFFSET"
  | .action => "REQUEST_ACTION_OFFSET"
  | .reserved => "REQUEST_RESERVED_OFFSET"
  | .expectedGeneration => "REQUEST_EXPECTED_GENERATION_OFFSET"
  | .expectedResultDomainId => "REQUEST_EXPECTED_RESULT_DOMAIN_ID_OFFSET"
  | .expectedProviderReleaseId => "REQUEST_EXPECTED_PROVIDER_RELEASE_ID_OFFSET"

theorem all_fields_are_schema_order :
    requestSchema.map (fun field => field.name) = all := by
  native_decide

theorem rust_names_are_unique : (all.map rustName).Nodup := by
  native_decide

end RequestField

theorem request_width : requestBytes = 88 := by native_decide

theorem request_well_formed : WellFormed requestSchema := by
  constructor
  · native_decide
  · intro field member
    simp [requestSchema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem request_fields_disjoint : requestLayout.Pairwise Before := by
  exact specializeFrom_pairwise 0 requestSchema

theorem request_coordinates_are_canonical : coordinates requestLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.action, 10, 1),
    (.reserved, 11, 5), (.expectedGeneration, 16, 8),
    (.expectedResultDomainId, 24, 32),
    (.expectedProviderReleaseId, 56, 32)
  ] := by
  native_decide

structure AcceptPythRequestV1 where
  expectedGeneration : Nat
  expectedResultDomainId : Nat
  expectedProviderReleaseId : Nat
  deriving DecidableEq, Repr

def RequestEncodable (request : AcceptPythRequestV1) : Prop :=
  request.expectedGeneration < 256 ^ 8 ∧
  request.expectedResultDomainId < 256 ^ 32 ∧
  request.expectedProviderReleaseId < 256 ^ 32

def encodeRequest (request : AcceptPythRequestV1) : List UInt8 :=
  requestMagic ++ Codec.encodeLE 2 requestVersion ++ [acceptPythAction] ++
  List.replicate 5 0 ++ Codec.encodeLE 8 request.expectedGeneration ++
  Codec.encodeLE 32 request.expectedResultDomainId ++
  Codec.encodeLE 32 request.expectedProviderReleaseId

def decodeRequest (input : List UInt8) : Option AcceptPythRequestV1 := do
  if input.length != requestBytes then none else
  if input.take (RequestField.offset .version) != requestMagic then none else
  if Codec.decodeLE ((input.drop (RequestField.offset .version)).take 2) !=
      requestVersion then none else
  if input[RequestField.offset .action]? != some acceptPythAction then none else
  if (input.drop (RequestField.offset .reserved)).take
      (RequestField.width .reserved) != List.replicate 5 0 then none else
  some {
    expectedGeneration := Codec.decodeLE
      ((input.drop (RequestField.offset .expectedGeneration)).take 8)
    expectedResultDomainId := Codec.decodeLE
      ((input.drop (RequestField.offset .expectedResultDomainId)).take 32)
    expectedProviderReleaseId := Codec.decodeLE
      ((input.drop (RequestField.offset .expectedProviderReleaseId)).take 32)
  }

def exampleRequest : AcceptPythRequestV1 := {
  expectedGeneration := 7
  expectedResultDomainId := 0x1122
  expectedProviderReleaseId := 0x3344
}

theorem encode_request_length (request : AcceptPythRequestV1) :
    (encodeRequest request).length = requestBytes := by
  simp [encodeRequest, requestMagic, requestBytes, requestSchema,
    Codec.encodeLE_length]
  native_decide

theorem example_request_round_trip :
    decodeRequest (encodeRequest exampleRequest) = some exampleRequest := by
  native_decide

theorem hostile_request_examples_refuse :
    decodeRequest [] = none ∧
    decodeRequest (encodeRequest exampleRequest |>.drop 1) = none ∧
    decodeRequest (List.set (encodeRequest exampleRequest)
      (RequestField.offset .magic) 0) = none ∧
    decodeRequest (List.set (encodeRequest exampleRequest)
      (RequestField.offset .version) 2) = none ∧
    decodeRequest (List.set (encodeRequest exampleRequest)
      (RequestField.offset .action) 1) = none ∧
    decodeRequest (List.set (encodeRequest exampleRequest)
      (RequestField.offset .reserved) 1) = none := by
  native_decide

/-! ## Funded liveness request -/

def fundedRequestMagic : List UInt8 :=
  [0x44, 0x43, 0x53, 0x52, 0x46, 0x4e, 0x44, 0x33] -- `DCSRFND3`

def fundedRequestVersion : Nat := 3

inductive FundedAction where
  | failNext | exhaust | commitFailure
  deriving DecidableEq, Repr

def FundedAction.tag : FundedAction → UInt8
  | .failNext => 1
  | .exhaust => 2
  | .commitFailure => 3

def decodeFundedAction : UInt8 → Option FundedAction
  | 1 => some .failNext
  | 2 => some .exhaust
  | 3 => some .commitFailure
  | _ => none

inductive FundedRequestField where
  | magic | version | action | reservedHeader | expectedGeneration
  | expectedRecoveryIndex | reservedBody | expectedResultDomainId
  | expectedFundingAllocationId
  deriving DecidableEq, Repr

def fundedRequestSchema : List (FieldSpec FundedRequestField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.action, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩,
  ⟨.expectedGeneration, .u64⟩,
  ⟨.expectedRecoveryIndex, .u32⟩,
  ⟨.reservedBody, .reserved 4⟩,
  ⟨.expectedResultDomainId, .bytes 32⟩,
  ⟨.expectedFundingAllocationId, .bytes 32⟩
]

def fundedRequestLayout : List (PlacedField FundedRequestField) :=
  DClutch.AbiSchema.specialize fundedRequestSchema
def fundedRequestBytes : Nat := schemaWidth fundedRequestSchema

namespace FundedRequestField

def all : List FundedRequestField := [
  .magic, .version, .action, .reservedHeader, .expectedGeneration,
  .expectedRecoveryIndex, .reservedBody, .expectedResultDomainId,
  .expectedFundingAllocationId
]

def coordinate (field : FundedRequestField) : Nat × Nat :=
  (coordinate? field fundedRequestLayout).getD (0, 0)

def offset (field : FundedRequestField) : Nat := (coordinate field).1
def width (field : FundedRequestField) : Nat := (coordinate field).2

def rustName : FundedRequestField → String
  | .magic => "FUNDED_REQUEST_MAGIC_OFFSET"
  | .version => "FUNDED_REQUEST_VERSION_OFFSET"
  | .action => "FUNDED_REQUEST_ACTION_OFFSET"
  | .reservedHeader => "FUNDED_REQUEST_RESERVED_HEADER_OFFSET"
  | .expectedGeneration => "FUNDED_REQUEST_EXPECTED_GENERATION_OFFSET"
  | .expectedRecoveryIndex => "FUNDED_REQUEST_EXPECTED_RECOVERY_INDEX_OFFSET"
  | .reservedBody => "FUNDED_REQUEST_RESERVED_BODY_OFFSET"
  | .expectedResultDomainId => "FUNDED_REQUEST_EXPECTED_RESULT_DOMAIN_ID_OFFSET"
  | .expectedFundingAllocationId =>
      "FUNDED_REQUEST_EXPECTED_FUNDING_ALLOCATION_ID_OFFSET"

theorem all_fields_are_schema_order :
    fundedRequestSchema.map (fun field => field.name) = all := by
  native_decide

theorem rust_names_are_unique : (all.map rustName).Nodup := by
  native_decide

end FundedRequestField

theorem funded_request_width : fundedRequestBytes = 96 := by native_decide

theorem funded_request_well_formed : WellFormed fundedRequestSchema := by
  constructor
  · native_decide
  · intro field member
    simp [fundedRequestSchema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;>
      decide

theorem funded_request_fields_disjoint :
    fundedRequestLayout.Pairwise Before := by
  exact specializeFrom_pairwise 0 fundedRequestSchema

theorem funded_request_coordinates_are_canonical :
    coordinates fundedRequestLayout = [
      (.magic, 0, 8), (.version, 8, 2), (.action, 10, 1),
      (.reservedHeader, 11, 5), (.expectedGeneration, 16, 8),
      (.expectedRecoveryIndex, 24, 4), (.reservedBody, 28, 4),
      (.expectedResultDomainId, 32, 32),
      (.expectedFundingAllocationId, 64, 32)
    ] := by
  native_decide

structure FundedTransitionRequestV3 where
  action : FundedAction
  expectedGeneration : Nat
  expectedRecoveryIndex : Nat
  expectedResultDomainId : Nat
  expectedFundingAllocationId : Nat
  deriving DecidableEq, Repr

def encodeFundedRequest (request : FundedTransitionRequestV3) : List UInt8 :=
  fundedRequestMagic ++ Codec.encodeLE 2 fundedRequestVersion ++
  [request.action.tag] ++ List.replicate 5 0 ++
  Codec.encodeLE 8 request.expectedGeneration ++
  Codec.encodeLE 4 request.expectedRecoveryIndex ++ List.replicate 4 0 ++
  Codec.encodeLE 32 request.expectedResultDomainId ++
  Codec.encodeLE 32 request.expectedFundingAllocationId

def decodeFundedRequest (input : List UInt8) : Option FundedTransitionRequestV3 := do
  if input.length != fundedRequestBytes then none else
  if input.take (FundedRequestField.offset .version) != fundedRequestMagic then none else
  if Codec.decodeLE ((input.drop (FundedRequestField.offset .version)).take 2) !=
      fundedRequestVersion then none else
  let action ← decodeFundedAction
    ((input[FundedRequestField.offset .action]?).getD 0)
  if (input.drop (FundedRequestField.offset .reservedHeader)).take
      (FundedRequestField.width .reservedHeader) != List.replicate 5 0 then none else
  if (input.drop (FundedRequestField.offset .reservedBody)).take
      (FundedRequestField.width .reservedBody) != List.replicate 4 0 then none else
  some {
    action
    expectedGeneration := Codec.decodeLE
      ((input.drop (FundedRequestField.offset .expectedGeneration)).take 8)
    expectedRecoveryIndex := Codec.decodeLE
      ((input.drop (FundedRequestField.offset .expectedRecoveryIndex)).take 4)
    expectedResultDomainId := Codec.decodeLE
      ((input.drop (FundedRequestField.offset .expectedResultDomainId)).take 32)
    expectedFundingAllocationId := Codec.decodeLE
      ((input.drop (FundedRequestField.offset .expectedFundingAllocationId)).take 32)
  }

def exampleFundedRequest : FundedTransitionRequestV3 := {
  action := .failNext
  expectedGeneration := 7
  expectedRecoveryIndex := 0
  expectedResultDomainId := 0x1122
  expectedFundingAllocationId := 0x3344
}

theorem encode_funded_request_length (request : FundedTransitionRequestV3) :
    (encodeFundedRequest request).length = fundedRequestBytes := by
  simp [encodeFundedRequest, fundedRequestMagic, fundedRequestBytes,
    fundedRequestSchema, Codec.encodeLE_length]
  native_decide

theorem example_funded_request_round_trip :
    decodeFundedRequest (encodeFundedRequest exampleFundedRequest) =
      some exampleFundedRequest := by
  native_decide

theorem hostile_funded_request_examples_refuse :
    decodeFundedRequest [] = none ∧
    decodeFundedRequest (encodeFundedRequest exampleFundedRequest |>.drop 1) = none ∧
    decodeFundedRequest (List.set (encodeFundedRequest exampleFundedRequest)
      (FundedRequestField.offset .magic) 0) = none ∧
    decodeFundedRequest (List.set (encodeFundedRequest exampleFundedRequest)
      (FundedRequestField.offset .action) 0xff) = none ∧
    decodeFundedRequest (List.set (encodeFundedRequest exampleFundedRequest)
      (FundedRequestField.offset .reservedBody) 1) = none := by
  native_decide

/-- Canonical funding/index coordinates are derived from Source/Config truth,
not from request bytes.  The physical adapter first projects its authenticated
Source material and manifest entry into this pair, then treats request fields
only as optimistic equality checks. -/
def canonicalFundedCoordinates? (config : Config) (state : State) :
    FundedAction → Option (Nat × Nat)
  | .failNext => do
      let index := match state.phase with
        | .primary => 0
        | .recovery current => current + 1
        | _ => config.recoveries.length
      let attempt ← config.recoveries[index]?
      some (index, attempt.entryFundingAllocationId)
  | .exhaust =>
      match state.phase with
      | .recovery current =>
          if current + 1 = config.recoveries.length then
            some (config.recoveries.length, config.exhaustFundingAllocationId)
          else none
      | _ => none
  | .commitFailure =>
      if state.phase = .exhausted then
        some (config.recoveries.length, config.failureFundingAllocationId)
      else none

def fundedCoordinatesMatch (config : Config) (state : State)
    (request : FundedTransitionRequestV3) : Bool :=
  request.expectedGeneration == state.generation &&
  canonicalFundedCoordinates? config state request.action ==
    some (request.expectedRecoveryIndex, request.expectedFundingAllocationId)

#guard canonicalFundedCoordinates? exampleConfig exampleState .failNext == some (0, 43)
#guard canonicalFundedCoordinates? exampleConfig
  ({ exampleState with phase := .recovery 0 }) .exhaust == some (1, 44)
#guard canonicalFundedCoordinates? exampleConfig exhaustedState .commitFailure == some (1, 45)
#guard fundedCoordinatesMatch exampleConfig exampleState ({ exampleFundedRequest with
  expectedGeneration := 1
  expectedFundingAllocationId := 43
})
#guard !fundedCoordinatesMatch exampleConfig exampleState ({ exampleFundedRequest with
  expectedGeneration := 1
  expectedRecoveryIndex := 1
  expectedFundingAllocationId := 43
})

theorem funded_actions_are_disjoint :
    FundedAction.failNext ≠ FundedAction.exhaust ∧
    FundedAction.failNext ≠ FundedAction.commitFailure ∧
    FundedAction.exhaust ≠ FundedAction.commitFailure := by decide

/-! The controller does not introduce a second state machine.  These bridge
theorems expose the determinism, exact funding conservation, refusal atomicity,
immediate recovery successor, and Product-owned failure facts of `specialize`
at the physical ABI boundary. -/

theorem funded_transition_deterministic
    {config : Config} {state : State} {funding : FundingState} {command : Command}
    {left right : Plan}
    (hleft : specialize config state funding command = .ok left)
    (hright : specialize config state funding command = .ok right) : left = right :=
  specialize_deterministic hleft hright

theorem funded_transition_refusal_is_atomic
    {config : Config} {state : State} {funding : FundingState}
    {command : Command} {error : Refusal}
    (h : specialize config state funding command = .error error) :
    executeProjection config state funding command = (state, funding, []) :=
  refusal_is_atomic h

theorem funded_transition_conserves_exact_charge
    {config : Config} {state : State} {funding : FundingState}
    {command : Command} {plan : Plan}
    (h : specialize config state funding command = .ok plan) :
    plan.certificate.fundingAllocationId = plan.fundingPost.allocationId ∧
    plan.certificate.fundingRemaining = plan.fundingPost.remainingCapital ∧
    plan.fundingPost.remainingCapital + plan.certificate.workPaid =
      funding.remainingCapital :=
  specialize_receipt_matches_funding h

theorem funded_recovery_advances_immediate_successor
    {config : Config} {state : State} {funding : FundingState}
    {index now worker receiptAccountId : Nat} {plan : Plan}
    (hphase : state.phase = .recovery index)
    (h : specialize config state funding (.failNext now worker receiptAccountId) =
      .ok plan) :
    plan.sourcePost.phase = .recovery (index + 1) :=
  failNext_from_recovery_advances_one hphase h

theorem funded_exhaustion_commits_last_recovery
    {config : Config} {state : State} {funding : FundingState}
    {now worker receiptAccountId : Nat} {plan : Plan}
    (h : specialize config state funding (.exhaust now worker receiptAccountId) =
      .ok plan) :
    plan.sourcePost.phase = .exhausted ∧
    plan.certificate.kind = .exhausted ∧
    plan.certificate.attemptIndex = config.recoveries.length := by
  unfold specialize at h
  simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals try contradiction
  all_goals
    simp only [pure, Except.pure, Except.ok.injEq] at h
    subst plan
    exact ⟨rfl, rfl, rfl⟩

theorem funded_failure_requires_exhaustion_and_product_selector
    {config : Config} {state : State} {funding : FundingState}
    {worker receiptAccountId : Nat} {plan : Plan}
    (h : specialize config state funding (.commitFailure worker receiptAccountId) =
      .ok plan) :
    state.phase = .exhausted ∧
    plan.certificate.selector = config.productDomain.failureSelector ∧
    plan.sourcePost.phase = .failureCommitted := by
  exact ⟨failure_commit_requires_exhaustion h, failure_commit_uses_product_selector h⟩

def certificateFields : List CertificateField := [
  .magic, .version, .kind, .reservedHeader, .market, .route,
  .sourceMaterial, .product, .providerEvidence, .fundingAllocation,
  .receiptAccount, .generation, .attemptIndex, .scheduleIndex, .selector,
  .reservedBody, .workPaid, .fundingRemaining, .resultNumerator,
  .resultDenominator, .observedAt
]

def certificateCoordinate (field : CertificateField) : Nat × Nat :=
  (coordinate? field certificateLayout).getD (0, 0)

def certificateOffset (field : CertificateField) : Nat :=
  (certificateCoordinate field).1

def certificateWidth (field : CertificateField) : Nat :=
  (certificateCoordinate field).2

def certificateRustName : CertificateField → String
  | .magic => "CERTIFICATE_MAGIC_OFFSET"
  | .version => "CERTIFICATE_VERSION_OFFSET"
  | .kind => "CERTIFICATE_KIND_OFFSET"
  | .reservedHeader => "CERTIFICATE_RESERVED_HEADER_OFFSET"
  | .market => "CERTIFICATE_MARKET_OFFSET"
  | .route => "CERTIFICATE_ROUTE_OFFSET"
  | .sourceMaterial => "CERTIFICATE_SOURCE_MATERIAL_OFFSET"
  | .product => "CERTIFICATE_PRODUCT_OFFSET"
  | .providerEvidence => "CERTIFICATE_PROVIDER_EVIDENCE_OFFSET"
  | .fundingAllocation => "CERTIFICATE_FUNDING_ALLOCATION_OFFSET"
  | .receiptAccount => "CERTIFICATE_RECEIPT_ACCOUNT_OFFSET"
  | .generation => "CERTIFICATE_GENERATION_OFFSET"
  | .attemptIndex => "CERTIFICATE_ATTEMPT_INDEX_OFFSET"
  | .scheduleIndex => "CERTIFICATE_SCHEDULE_INDEX_OFFSET"
  | .selector => "CERTIFICATE_SELECTOR_OFFSET"
  | .reservedBody => "CERTIFICATE_RESERVED_BODY_OFFSET"
  | .workPaid => "CERTIFICATE_WORK_PAID_OFFSET"
  | .fundingRemaining => "CERTIFICATE_FUNDING_REMAINING_OFFSET"
  | .resultNumerator => "CERTIFICATE_RESULT_NUMERATOR_OFFSET"
  | .resultDenominator => "CERTIFICATE_RESULT_DENOMINATOR_OFFSET"
  | .observedAt => "CERTIFICATE_OBSERVED_AT_OFFSET"

theorem certificate_fields_are_schema_order :
    certificateSchema.map (fun field => field.name) = certificateFields := by
  native_decide

theorem certificate_rust_names_are_unique :
    (certificateFields.map certificateRustName).Nodup := by
  native_decide

theorem physical_certificate_width : certificateBytes = 312 := certificate_width

end DClutch.SourceResolution.ControllerAbi
