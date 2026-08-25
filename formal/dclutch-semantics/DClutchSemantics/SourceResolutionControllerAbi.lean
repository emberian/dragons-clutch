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

/-! ## Canonical Market-Core effect request and Source closure receipt -/

def coreRequestMagic : List UInt8 :=
  [0x44, 0x43, 0x53, 0x52, 0x43, 0x52, 0x30, 0x31] -- `DCSRCR01`

def coreRequestVersion : Nat := 1
def coreTerminalSuccessKind : Nat := 1
def coreTerminalFailureKind : Nat := 4
def coreClosureKind : Nat := 5

inductive CoreAction where
  | createFund | verifyFundReady | admitTerminal | closeFund
  deriving DecidableEq, Repr

/-- Tags intentionally equal the canonical Core envelope action tags. -/
def CoreAction.tag : CoreAction → UInt8
  | .createFund => 0
  | .verifyFundReady => 1
  | .admitTerminal => 5
  | .closeFund => 8

def decodeCoreAction : UInt8 → Option CoreAction
  | 0 => some .createFund
  | 1 => some .verifyFundReady
  | 5 => some .admitTerminal
  | 8 => some .closeFund
  | _ => none

inductive CoreRequestField where
  | magic | version | action | receiptKind | reservedHeader
  | sourceState | sourceMaterial | capabilityManifest
  | recoveryFunding | exhaustionFunding | failureFunding
  | receipt | beneficiary
  | recoveryEntryIndex | exhaustionEntryIndex | failureEntryIndex
  | reservedBody | receiptSequence
  deriving DecidableEq, Repr

def coreRequestSchema : List (FieldSpec CoreRequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩,
  ⟨.receiptKind, .u8⟩, ⟨.reservedHeader, .reserved 4⟩,
  ⟨.sourceState, .bytes 32⟩, ⟨.sourceMaterial, .bytes 32⟩,
  ⟨.capabilityManifest, .bytes 32⟩,
  ⟨.recoveryFunding, .bytes 32⟩, ⟨.exhaustionFunding, .bytes 32⟩,
  ⟨.failureFunding, .bytes 32⟩, ⟨.receipt, .bytes 32⟩,
  ⟨.beneficiary, .bytes 32⟩,
  ⟨.recoveryEntryIndex, .u16⟩, ⟨.exhaustionEntryIndex, .u16⟩,
  ⟨.failureEntryIndex, .u16⟩, ⟨.reservedBody, .reserved 2⟩,
  ⟨.receiptSequence, .u64⟩
]

def coreRequestLayout : List (PlacedField CoreRequestField) :=
  DClutch.AbiSchema.specialize coreRequestSchema
def coreRequestBytes : Nat := schemaWidth coreRequestSchema

namespace CoreRequestField

def all : List CoreRequestField := [
  .magic, .version, .action, .receiptKind, .reservedHeader,
  .sourceState, .sourceMaterial, .capabilityManifest,
  .recoveryFunding, .exhaustionFunding, .failureFunding, .receipt, .beneficiary,
  .recoveryEntryIndex, .exhaustionEntryIndex, .failureEntryIndex,
  .reservedBody, .receiptSequence
]

def coordinate (field : CoreRequestField) : Nat × Nat :=
  (coordinate? field coreRequestLayout).getD (0, 0)

def offset (field : CoreRequestField) : Nat := (coordinate field).1
def width (field : CoreRequestField) : Nat := (coordinate field).2

def rustName : CoreRequestField → String
  | .magic => "CORE_REQUEST_MAGIC_OFFSET"
  | .version => "CORE_REQUEST_VERSION_OFFSET"
  | .action => "CORE_REQUEST_ACTION_OFFSET"
  | .receiptKind => "CORE_REQUEST_RECEIPT_KIND_OFFSET"
  | .reservedHeader => "CORE_REQUEST_RESERVED_HEADER_OFFSET"
  | .sourceState => "CORE_REQUEST_SOURCE_STATE_OFFSET"
  | .sourceMaterial => "CORE_REQUEST_SOURCE_MATERIAL_OFFSET"
  | .capabilityManifest => "CORE_REQUEST_CAPABILITY_MANIFEST_OFFSET"
  | .recoveryFunding => "CORE_REQUEST_RECOVERY_FUNDING_OFFSET"
  | .exhaustionFunding => "CORE_REQUEST_EXHAUSTION_FUNDING_OFFSET"
  | .failureFunding => "CORE_REQUEST_FAILURE_FUNDING_OFFSET"
  | .receipt => "CORE_REQUEST_RECEIPT_OFFSET"
  | .beneficiary => "CORE_REQUEST_BENEFICIARY_OFFSET"
  | .recoveryEntryIndex => "CORE_REQUEST_RECOVERY_ENTRY_INDEX_OFFSET"
  | .exhaustionEntryIndex => "CORE_REQUEST_EXHAUSTION_ENTRY_INDEX_OFFSET"
  | .failureEntryIndex => "CORE_REQUEST_FAILURE_ENTRY_INDEX_OFFSET"
  | .reservedBody => "CORE_REQUEST_RESERVED_BODY_OFFSET"
  | .receiptSequence => "CORE_REQUEST_RECEIPT_SEQUENCE_OFFSET"

theorem all_fields_are_schema_order :
    coreRequestSchema.map (fun field => field.name) = all := by native_decide

theorem rust_names_are_unique : (all.map rustName).Nodup := by native_decide

end CoreRequestField

theorem core_request_width : coreRequestBytes = 288 := by native_decide

theorem core_request_well_formed : WellFormed coreRequestSchema := by
  constructor
  · native_decide
  · intro field member
    simp [coreRequestSchema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem core_request_fields_disjoint : coreRequestLayout.Pairwise Before := by
  exact specializeFrom_pairwise 0 coreRequestSchema

theorem core_request_coordinates_are_canonical :
    coordinates coreRequestLayout = [
      (.magic, 0, 8), (.version, 8, 2), (.action, 10, 1),
      (.receiptKind, 11, 1), (.reservedHeader, 12, 4),
      (.sourceState, 16, 32), (.sourceMaterial, 48, 32),
      (.capabilityManifest, 80, 32), (.recoveryFunding, 112, 32),
      (.exhaustionFunding, 144, 32), (.failureFunding, 176, 32),
      (.receipt, 208, 32), (.beneficiary, 240, 32),
      (.recoveryEntryIndex, 272, 2), (.exhaustionEntryIndex, 274, 2),
      (.failureEntryIndex, 276, 2), (.reservedBody, 278, 2),
      (.receiptSequence, 280, 8)
    ] := by native_decide

structure ResolutionRoleRequestV1 where
  action : CoreAction
  receiptKind : Nat
  sourceState : Nat
  sourceMaterial : Nat
  capabilityManifest : Nat
  recoveryFunding : Nat
  exhaustionFunding : Nat
  failureFunding : Nat
  receipt : Nat
  beneficiary : Nat
  recoveryEntryIndex : Nat
  exhaustionEntryIndex : Nat
  failureEntryIndex : Nat
  receiptSequence : Nat
  deriving DecidableEq, Repr

def ResolutionRoleRequestV1.commonValid (request : ResolutionRoleRequestV1) : Bool :=
  request.sourceState != 0 && request.sourceMaterial != 0 &&
  request.capabilityManifest != 0 && request.recoveryFunding != 0 &&
  request.exhaustionFunding != 0 && request.failureFunding != 0 &&
  request.recoveryFunding != request.exhaustionFunding &&
  request.recoveryFunding != request.failureFunding &&
  request.exhaustionFunding != request.failureFunding &&
  request.recoveryEntryIndex != request.exhaustionEntryIndex &&
  request.recoveryEntryIndex != request.failureEntryIndex &&
  request.exhaustionEntryIndex != request.failureEntryIndex

def ResolutionRoleRequestV1.shapeValid (request : ResolutionRoleRequestV1) : Bool :=
  request.commonValid && match request.action with
  | .createFund | .verifyFundReady =>
      request.receipt = 0 && request.receiptKind = 0 &&
      request.receiptSequence = 0 && request.beneficiary != 0
  | .admitTerminal =>
      request.receipt != 0 && request.beneficiary = 0 &&
      (request.receiptKind = coreTerminalSuccessKind ||
        request.receiptKind = coreTerminalFailureKind) &&
      request.receiptSequence != 0
  | .closeFund =>
      request.receipt != 0 && request.beneficiary != 0 &&
      request.receiptKind = coreClosureKind && request.receiptSequence != 0

def encodeCoreRequest (request : ResolutionRoleRequestV1) : List UInt8 :=
  coreRequestMagic ++ Codec.encodeLE 2 coreRequestVersion ++
  [request.action.tag, UInt8.ofNat request.receiptKind] ++ List.replicate 4 0 ++
  Codec.encodeLE 32 request.sourceState ++ Codec.encodeLE 32 request.sourceMaterial ++
  Codec.encodeLE 32 request.capabilityManifest ++
  Codec.encodeLE 32 request.recoveryFunding ++
  Codec.encodeLE 32 request.exhaustionFunding ++
  Codec.encodeLE 32 request.failureFunding ++ Codec.encodeLE 32 request.receipt ++
  Codec.encodeLE 32 request.beneficiary ++
  Codec.encodeLE 2 request.recoveryEntryIndex ++
  Codec.encodeLE 2 request.exhaustionEntryIndex ++
  Codec.encodeLE 2 request.failureEntryIndex ++ List.replicate 2 0 ++
  Codec.encodeLE 8 request.receiptSequence

def decodeCoreRequest (input : List UInt8) : Option ResolutionRoleRequestV1 := do
  if input.length != coreRequestBytes then none else
  if input.take (CoreRequestField.offset .version) != coreRequestMagic then none else
  if Codec.decodeLE ((input.drop (CoreRequestField.offset .version)).take 2) !=
      coreRequestVersion then none else
  let action ← decodeCoreAction ((input[CoreRequestField.offset .action]?).getD 0)
  if (input.drop (CoreRequestField.offset .reservedHeader)).take 4 !=
      List.replicate 4 0 then none else
  if (input.drop (CoreRequestField.offset .reservedBody)).take 2 !=
      List.replicate 2 0 then none else
  let request : ResolutionRoleRequestV1 := {
    action
    receiptKind := (input[CoreRequestField.offset .receiptKind]?).getD 0 |>.toNat
    sourceState := Codec.decodeLE ((input.drop (CoreRequestField.offset .sourceState)).take 32)
    sourceMaterial := Codec.decodeLE
      ((input.drop (CoreRequestField.offset .sourceMaterial)).take 32)
    capabilityManifest := Codec.decodeLE
      ((input.drop (CoreRequestField.offset .capabilityManifest)).take 32)
    recoveryFunding := Codec.decodeLE
      ((input.drop (CoreRequestField.offset .recoveryFunding)).take 32)
    exhaustionFunding := Codec.decodeLE
      ((input.drop (CoreRequestField.offset .exhaustionFunding)).take 32)
    failureFunding := Codec.decodeLE
      ((input.drop (CoreRequestField.offset .failureFunding)).take 32)
    receipt := Codec.decodeLE ((input.drop (CoreRequestField.offset .receipt)).take 32)
    beneficiary := Codec.decodeLE
      ((input.drop (CoreRequestField.offset .beneficiary)).take 32)
    recoveryEntryIndex := Codec.decodeLE
      ((input.drop (CoreRequestField.offset .recoveryEntryIndex)).take 2)
    exhaustionEntryIndex := Codec.decodeLE
      ((input.drop (CoreRequestField.offset .exhaustionEntryIndex)).take 2)
    failureEntryIndex := Codec.decodeLE
      ((input.drop (CoreRequestField.offset .failureEntryIndex)).take 2)
    receiptSequence := Codec.decodeLE
      ((input.drop (CoreRequestField.offset .receiptSequence)).take 8)
  }
  if request.shapeValid then some request else none

def exampleCoreRequest : ResolutionRoleRequestV1 := {
  action := .closeFund
  receiptKind := coreClosureKind
  sourceState := 1
  sourceMaterial := 2
  capabilityManifest := 3
  recoveryFunding := 4
  exhaustionFunding := 5
  failureFunding := 6
  receipt := 7
  beneficiary := 8
  recoveryEntryIndex := 0
  exhaustionEntryIndex := 1
  failureEntryIndex := 2
  receiptSequence := 4
}

theorem encode_core_request_length (request : ResolutionRoleRequestV1) :
    (encodeCoreRequest request).length = coreRequestBytes := by
  simp [encodeCoreRequest, coreRequestMagic, coreRequestBytes, coreRequestSchema,
    Codec.encodeLE_length]
  native_decide

theorem example_core_request_round_trip :
    decodeCoreRequest (encodeCoreRequest exampleCoreRequest) = some exampleCoreRequest := by
  native_decide

theorem hostile_core_request_examples_refuse :
    decodeCoreRequest [] = none ∧
    decodeCoreRequest (encodeCoreRequest exampleCoreRequest |>.drop 1) = none ∧
    decodeCoreRequest (List.set (encodeCoreRequest exampleCoreRequest)
      (CoreRequestField.offset .action) 0xff) = none ∧
    decodeCoreRequest (List.set (encodeCoreRequest exampleCoreRequest)
      (CoreRequestField.offset .reservedBody) 1) = none ∧
    decodeCoreRequest (encodeCoreRequest ({ exampleCoreRequest with
      receiptSequence := 0 })) = none := by
  native_decide

theorem core_actions_partition :
    CoreAction.createFund ≠ CoreAction.verifyFundReady ∧
    CoreAction.createFund ≠ CoreAction.admitTerminal ∧
    CoreAction.createFund ≠ CoreAction.closeFund ∧
    CoreAction.verifyFundReady ≠ CoreAction.admitTerminal ∧
    CoreAction.verifyFundReady ≠ CoreAction.closeFund ∧
    CoreAction.admitTerminal ≠ CoreAction.closeFund := by decide

/-! A closure receipt persists the exact terminal and funding-set digests after
the Source and three funding accounts are discharged. Core consumes only the
authenticated receipt coordinate and Resolution acknowledgement; it never
recomputes action-specific allocation or refund arithmetic. -/

def closureMagic : List UInt8 :=
  [0x44, 0x43, 0x53, 0x52, 0x43, 0x4c, 0x53, 0x31] -- `DCSRCLS1`
def closureVersion : Nat := 1

inductive ClosureField where
  | magic | version | kind | reservedHeader
  | market | sourceState | sourceMaterial | capabilityManifest
  | terminalCertificate | receiptAccount | beneficiary
  | sourceStateDigest | terminalCertificateDigest | fundingSetDigest
  | generation | terminalSequence | fundingCount | selector
  | refundLamports | closedAt | reservedBody
  deriving DecidableEq, Repr

def closureSchema : List (FieldSpec ClosureField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.kind, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩,
  ⟨.market, .bytes 32⟩, ⟨.sourceState, .bytes 32⟩,
  ⟨.sourceMaterial, .bytes 32⟩, ⟨.capabilityManifest, .bytes 32⟩,
  ⟨.terminalCertificate, .bytes 32⟩, ⟨.receiptAccount, .bytes 32⟩,
  ⟨.beneficiary, .bytes 32⟩, ⟨.sourceStateDigest, .bytes 32⟩,
  ⟨.terminalCertificateDigest, .bytes 32⟩, ⟨.fundingSetDigest, .bytes 32⟩,
  ⟨.generation, .u64⟩, ⟨.terminalSequence, .u64⟩,
  ⟨.fundingCount, .u32⟩, ⟨.selector, .u32⟩,
  ⟨.refundLamports, .u64⟩, ⟨.closedAt, .u64⟩,
  ⟨.reservedBody, .reserved 8⟩
]

def closureLayout : List (PlacedField ClosureField) :=
  DClutch.AbiSchema.specialize closureSchema
def closureBytes : Nat := schemaWidth closureSchema

namespace ClosureField

def all : List ClosureField := [
  .magic, .version, .kind, .reservedHeader, .market, .sourceState,
  .sourceMaterial, .capabilityManifest, .terminalCertificate, .receiptAccount,
  .beneficiary, .sourceStateDigest, .terminalCertificateDigest,
  .fundingSetDigest, .generation, .terminalSequence, .fundingCount, .selector,
  .refundLamports, .closedAt, .reservedBody
]

def coordinate (field : ClosureField) : Nat × Nat :=
  (coordinate? field closureLayout).getD (0, 0)
def offset (field : ClosureField) : Nat := (coordinate field).1

def rustName : ClosureField → String
  | .magic => "CLOSURE_MAGIC_OFFSET" | .version => "CLOSURE_VERSION_OFFSET"
  | .kind => "CLOSURE_KIND_OFFSET" | .reservedHeader => "CLOSURE_RESERVED_HEADER_OFFSET"
  | .market => "CLOSURE_MARKET_OFFSET" | .sourceState => "CLOSURE_SOURCE_STATE_OFFSET"
  | .sourceMaterial => "CLOSURE_SOURCE_MATERIAL_OFFSET"
  | .capabilityManifest => "CLOSURE_CAPABILITY_MANIFEST_OFFSET"
  | .terminalCertificate => "CLOSURE_TERMINAL_CERTIFICATE_OFFSET"
  | .receiptAccount => "CLOSURE_RECEIPT_ACCOUNT_OFFSET"
  | .beneficiary => "CLOSURE_BENEFICIARY_OFFSET"
  | .sourceStateDigest => "CLOSURE_SOURCE_STATE_DIGEST_OFFSET"
  | .terminalCertificateDigest => "CLOSURE_TERMINAL_CERTIFICATE_DIGEST_OFFSET"
  | .fundingSetDigest => "CLOSURE_FUNDING_SET_DIGEST_OFFSET"
  | .generation => "CLOSURE_GENERATION_OFFSET"
  | .terminalSequence => "CLOSURE_TERMINAL_SEQUENCE_OFFSET"
  | .fundingCount => "CLOSURE_FUNDING_COUNT_OFFSET"
  | .selector => "CLOSURE_SELECTOR_OFFSET"
  | .refundLamports => "CLOSURE_REFUND_LAMPORTS_OFFSET"
  | .closedAt => "CLOSURE_CLOSED_AT_OFFSET"
  | .reservedBody => "CLOSURE_RESERVED_BODY_OFFSET"

theorem all_fields_are_schema_order :
    closureSchema.map (fun field => field.name) = all := by native_decide
theorem rust_names_are_unique : (all.map rustName).Nodup := by native_decide

end ClosureField

theorem closure_width : closureBytes = 384 := by native_decide
theorem closure_fields_disjoint : closureLayout.Pairwise Before := by
  exact specializeFrom_pairwise 0 closureSchema

structure SourceClosureReceiptV1 where
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

def SourceClosureReceiptV1.shapeValid (receipt : SourceClosureReceiptV1) : Bool :=
  receipt.market != 0 && receipt.sourceState != 0 && receipt.sourceMaterial != 0 &&
  receipt.capabilityManifest != 0 && receipt.terminalCertificate != 0 &&
  receipt.receiptAccount != 0 && receipt.beneficiary != 0 &&
  receipt.sourceStateDigest != 0 && receipt.terminalCertificateDigest != 0 &&
  receipt.fundingSetDigest != 0 && receipt.generation != 0 &&
  receipt.terminalSequence != 0 && receipt.selector < 256 &&
  receipt.refundLamports != 0 && receipt.closedAt != 0

def encodeClosure (receipt : SourceClosureReceiptV1) : List UInt8 :=
  closureMagic ++ Codec.encodeLE 2 closureVersion ++ [1] ++ List.replicate 5 0 ++
  Codec.encodeLE 32 receipt.market ++ Codec.encodeLE 32 receipt.sourceState ++
  Codec.encodeLE 32 receipt.sourceMaterial ++
  Codec.encodeLE 32 receipt.capabilityManifest ++
  Codec.encodeLE 32 receipt.terminalCertificate ++
  Codec.encodeLE 32 receipt.receiptAccount ++ Codec.encodeLE 32 receipt.beneficiary ++
  Codec.encodeLE 32 receipt.sourceStateDigest ++
  Codec.encodeLE 32 receipt.terminalCertificateDigest ++
  Codec.encodeLE 32 receipt.fundingSetDigest ++ Codec.encodeLE 8 receipt.generation ++
  Codec.encodeLE 8 receipt.terminalSequence ++ Codec.encodeLE 4 3 ++
  Codec.encodeLE 4 receipt.selector ++ Codec.encodeLE 8 receipt.refundLamports ++
  Codec.encodeLE 8 receipt.closedAt ++ List.replicate 8 0

def decodeClosure (input : List UInt8) : Option SourceClosureReceiptV1 := do
  if input.length != closureBytes then none else
  if input.take (ClosureField.offset .version) != closureMagic then none else
  if Codec.decodeLE ((input.drop (ClosureField.offset .version)).take 2) !=
      closureVersion then none else
  if input[ClosureField.offset .kind]? != some 1 then none else
  if (input.drop (ClosureField.offset .reservedHeader)).take 5 !=
      List.replicate 5 0 then none else
  if (input.drop (ClosureField.offset .reservedBody)).take 8 !=
      List.replicate 8 0 then none else
  if Codec.decodeLE ((input.drop (ClosureField.offset .fundingCount)).take 4) != 3 then none else
  let receipt : SourceClosureReceiptV1 := {
    market := Codec.decodeLE ((input.drop (ClosureField.offset .market)).take 32)
    sourceState := Codec.decodeLE ((input.drop (ClosureField.offset .sourceState)).take 32)
    sourceMaterial := Codec.decodeLE
      ((input.drop (ClosureField.offset .sourceMaterial)).take 32)
    capabilityManifest := Codec.decodeLE
      ((input.drop (ClosureField.offset .capabilityManifest)).take 32)
    terminalCertificate := Codec.decodeLE
      ((input.drop (ClosureField.offset .terminalCertificate)).take 32)
    receiptAccount := Codec.decodeLE
      ((input.drop (ClosureField.offset .receiptAccount)).take 32)
    beneficiary := Codec.decodeLE ((input.drop (ClosureField.offset .beneficiary)).take 32)
    sourceStateDigest := Codec.decodeLE
      ((input.drop (ClosureField.offset .sourceStateDigest)).take 32)
    terminalCertificateDigest := Codec.decodeLE
      ((input.drop (ClosureField.offset .terminalCertificateDigest)).take 32)
    fundingSetDigest := Codec.decodeLE
      ((input.drop (ClosureField.offset .fundingSetDigest)).take 32)
    generation := Codec.decodeLE ((input.drop (ClosureField.offset .generation)).take 8)
    terminalSequence := Codec.decodeLE
      ((input.drop (ClosureField.offset .terminalSequence)).take 8)
    selector := Codec.decodeLE ((input.drop (ClosureField.offset .selector)).take 4)
    refundLamports := Codec.decodeLE
      ((input.drop (ClosureField.offset .refundLamports)).take 8)
    closedAt := Codec.decodeLE ((input.drop (ClosureField.offset .closedAt)).take 8)
  }
  if receipt.shapeValid then some receipt else none

def exampleClosure : SourceClosureReceiptV1 := {
  market := 1
  sourceState := 2
  sourceMaterial := 3
  capabilityManifest := 4
  terminalCertificate := 5
  receiptAccount := 6
  beneficiary := 7
  sourceStateDigest := 8
  terminalCertificateDigest := 9
  fundingSetDigest := 10
  generation := 11
  terminalSequence := 12
  selector := 2
  refundLamports := 13
  closedAt := 14
}

theorem encode_closure_length (receipt : SourceClosureReceiptV1) :
    (encodeClosure receipt).length = closureBytes := by
  simp [encodeClosure, closureMagic, closureBytes, closureSchema, Codec.encodeLE_length]
  native_decide

theorem example_closure_round_trip :
    decodeClosure (encodeClosure exampleClosure) = some exampleClosure := by native_decide

theorem hostile_closure_examples_refuse :
    decodeClosure [] = none ∧
    decodeClosure (encodeClosure exampleClosure |>.drop 1) = none ∧
    decodeClosure (List.set (encodeClosure exampleClosure)
      (ClosureField.offset .fundingCount) 2) = none ∧
    decodeClosure (List.set (encodeClosure exampleClosure)
      (ClosureField.offset .reservedBody) 1) = none ∧
    decodeClosure (encodeClosure ({ exampleClosure with selector := 256 })) = none ∧
    decodeClosure (encodeClosure ({ exampleClosure with refundLamports := 0 })) = none := by
  native_decide

structure NativeDischarge where
  accountRent : Nat
  semanticRemaining : Nat
  donation : Nat
  deriving DecidableEq, Repr

def NativeDischarge.refund (resource : NativeDischarge) : Nat :=
  resource.accountRent + resource.semanticRemaining + resource.donation

def closureRefund (source recovery exhaustion failure : NativeDischarge) : Nat :=
  source.refund + recovery.refund + exhaustion.refund + failure.refund

def discharged : NativeDischarge := ⟨0, 0, 0⟩

theorem closure_refund_is_exact_partition
    (source recovery exhaustion failure : NativeDischarge) :
    closureRefund source recovery exhaustion failure =
      source.accountRent + source.semanticRemaining + source.donation +
      (recovery.accountRent + recovery.semanticRemaining + recovery.donation) +
      (exhaustion.accountRent + exhaustion.semanticRemaining + exhaustion.donation) +
      (failure.accountRent + failure.semanticRemaining + failure.donation) := by
  simp [closureRefund, NativeDischarge.refund, Nat.add_assoc]

theorem closure_post_resources_are_discharged :
    [discharged, discharged, discharged, discharged].all
      (fun resource => resource.accountRent = 0 &&
        resource.semanticRemaining = 0 && resource.donation = 0) = true := by
  native_decide

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
