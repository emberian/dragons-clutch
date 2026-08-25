import DClutchSemantics.SourceResolutionAbi

/-!
# Physical Source Resolution controller ABI

The first physical Resolution-role specialization admits one already-posted,
fully verified Pyth terminal observation.  The request carries only optimistic
concurrency coordinates.  Market, Source material, Product result-domain,
provider release, observation, and Clock truth remain in authenticated
accounts and are not copied into the request as caller-selected policy.

The successful output is exactly the 312-byte certificate schema already
owned by `SourceResolutionAbi`.  This module adds no second receipt layout; it
only exposes the existing cursor-derived coordinates to the Rust generator.
-/

namespace DClutch.SourceResolution.ControllerAbi

open DClutch DClutch.AbiSchema
open DClutch.SourceResolution.Abi

def requestMagic : List UInt8 :=
  [0x44, 0x43, 0x53, 0x52, 0x50, 0x59, 0x54, 0x31] -- `DCSRPYT1`

def requestVersion : Nat := 1
def acceptPythAction : UInt8 := 0

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
