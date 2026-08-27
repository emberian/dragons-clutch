import DClutchSemantics.SourceResolution
import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec

/-!
# Source successor executor and certificate ABI

The Source specializer emits the same compact outer shape as the shared Effect
executor: an eight-byte `DCEF` header and fixed sixteen-byte mutation records.
This V2 semantic profile adds only Source-owned role/resource tags; it does not
add a second interpreter language.  The physical executor must authenticate its
account-role frame before applying the plan atomically.

Certificates use one typed, cursor-specialized 312-byte schema.  Lean owns all
offsets through `AbiSchema.specialize`; no handwritten offset table is consumed
by the semantic transition.
-/

namespace DClutch.SourceResolution.Abi

open DClutch
open DClutch.SourceResolution
open DClutch.AbiSchema

/-! ## Shared-executor Source effect profile -/

def effectMagic : List UInt8 := DClutch.Codec.magic
def effectVersion : UInt8 := 2
def effectHeaderBytes : Nat := 8
def effectBytes : Nat := 16
def maximumEffects : Nat := 5

def operationTag : Operation → UInt8
  | .set => 0
  | .debit => 1
  | .credit => 2

/-- Source roles occupy a disjoint extension range from Direct V1. -/
def roleTag : AccountRole → UInt8
  | .sourceState => 16
  | .fundingState => 17
  | .worker => 18
  | .productResolution => 19
  | .receipt => 20

def resourceTag : Resource → UInt8
  | .phase => 16
  | .generation => 17
  | .workCapital => 18
  | .resolutionOutcome => 19
  | .terminalReceipt => 20

def decodeOperation : UInt8 → Option Operation
  | 0 => some .set
  | 1 => some .debit
  | 2 => some .credit
  | _ => none

def decodeRole : UInt8 → Option AccountRole
  | 16 => some .sourceState
  | 17 => some .fundingState
  | 18 => some .worker
  | 19 => some .productResolution
  | 20 => some .receipt
  | _ => none

def decodeResource : UInt8 → Option Resource
  | 16 => some .phase
  | 17 => some .generation
  | 18 => some .workCapital
  | 19 => some .resolutionOutcome
  | 20 => some .terminalReceipt
  | _ => none

@[simp] theorem decodeOperation_operationTag (operation : Operation) :
    decodeOperation (operationTag operation) = some operation := by
  cases operation <;> rfl

@[simp] theorem decodeRole_roleTag (role : AccountRole) :
    decodeRole (roleTag role) = some role := by
  cases role <;> rfl

@[simp] theorem decodeResource_resourceTag (resource : Resource) :
    decodeResource (resourceTag resource) = some resource := by
  cases resource <;> rfl

/-- Canonical sixteen-byte mutation record: opcode, role, resource, zero,
`u32` coordinate, `u64` value. -/
def encodeEffect (effect : Effect) : List UInt8 :=
  [operationTag effect.operation, roleTag effect.role,
    resourceTag effect.resource, 0] ++
  DClutch.Codec.encodeLE 4 effect.coordinate ++
  DClutch.Codec.encodeLE 8 effect.value

theorem encodeEffect_length (effect : Effect) :
    (encodeEffect effect).length = effectBytes := by
  simp [encodeEffect, effectBytes, DClutch.Codec.encodeLE_length]

def EffectEncodable (effect : Effect) : Prop :=
  effect.coordinate < 256 ^ 4 ∧ effect.value < 256 ^ 8

/-- Hostile exact-record decoder.  Prefixes and trailing bytes refuse. -/
def decodeEffect (bytes : List UInt8) : Option Effect := do
  if bytes.length != effectBytes then none else
  let operationByte ← bytes[0]?
  let roleByte ← bytes[1]?
  let resourceByte ← bytes[2]?
  let reserved ← bytes[3]?
  let operation ← decodeOperation operationByte
  let role ← decodeRole roleByte
  let resource ← decodeResource resourceByte
  if reserved != 0 then none else
  let coordinate := DClutch.Codec.decodeLE ((bytes.drop 4).take 4)
  let value := DClutch.Codec.decodeLE (bytes.drop 8)
  some { operation, role, resource, coordinate, value }

theorem decodeEffect_encodeEffect (effect : Effect)
    (encodable : EffectEncodable effect) :
    decodeEffect (encodeEffect effect) = some effect := by
  rcases encodable with ⟨coordinateFits, valueFits⟩
  have coordinateDecoded := DClutch.Codec.decodeLE_encodeLE
    4 effect.coordinate coordinateFits
  have valueDecoded := DClutch.Codec.decodeLE_encodeLE 8 effect.value valueFits
  cases effect with
  | mk operation role resource coordinate value =>
      cases operation <;> cases role <;> cases resource <;>
        simp [decodeEffect, encodeEffect, effectBytes, operationTag, roleTag,
          resourceTag, decodeOperation, decodeRole, decodeResource,
          DClutch.Codec.encodeLE_length, coordinateDecoded, valueDecoded]

def encodeEffectHeader (count : Nat) : List UInt8 :=
  effectMagic ++ [effectVersion, UInt8.ofNat count, 0, 0]

def encodeEffectPlan (plan : EffectPlan) : List UInt8 :=
  encodeEffectHeader plan.effects.length ++ plan.effects.flatMap encodeEffect

private theorem flatMap_effect_length : ∀ effects : List Effect,
    (effects.flatMap encodeEffect).length = effects.length * effectBytes
  | [] => by simp
  | effect :: rest => by
      simp [encodeEffect_length, flatMap_effect_length rest, effectBytes]
      omega

theorem encodeEffectPlan_length (plan : EffectPlan) :
    (encodeEffectPlan plan).length =
      effectHeaderBytes + plan.effects.length * effectBytes := by
  unfold encodeEffectPlan
  rw [List.length_append, flatMap_effect_length]
  simp [encodeEffectHeader, effectMagic, DClutch.Codec.magic, effectHeaderBytes]

def decodeEffects : Nat → List UInt8 → Option (List Effect)
  | 0, bytes => if bytes.isEmpty then some [] else none
  | count + 1, bytes => do
      let effect ← decodeEffect (bytes.take effectBytes)
      let rest ← decodeEffects count (bytes.drop effectBytes)
      some (effect :: rest)

def decodeEffectPlan : List UInt8 → Option EffectPlan
  | 0x44 :: 0x43 :: 0x45 :: 0x46 :: version :: count :: reservedA :: reservedB :: records => do
      if version != effectVersion || reservedA != 0 || reservedB != 0 then none else
      let count := count.toNat
      if count > maximumEffects || records.length != count * effectBytes then none else
      let effects ← decodeEffects count records
      some { effects }
  | _ => none

private theorem decodeEffects_encodeEffects
    (effects : List Effect)
    (encodable : ∀ effect ∈ effects, EffectEncodable effect) :
    decodeEffects effects.length (effects.flatMap encodeEffect) = some effects := by
  induction effects with
  | nil => simp [decodeEffects]
  | cons effect rest induction =>
      have head := encodable effect (by simp)
      have tail : ∀ candidate ∈ rest, EffectEncodable candidate := by
        intro candidate member
        exact encodable candidate (by simp [member])
      simp [decodeEffects, encodeEffect_length,
        decodeEffect_encodeEffect effect head, induction tail]

theorem decodeEffectPlan_encodeEffectPlan
    (plan : EffectPlan)
    (countFits : plan.effects.length ≤ maximumEffects)
    (encodable : ∀ effect ∈ plan.effects, EffectEncodable effect) :
    decodeEffectPlan (encodeEffectPlan plan) = some plan := by
  have countLt : plan.effects.length < 256 :=
    Nat.lt_of_le_of_lt countFits (by decide)
  have countByte : (UInt8.ofNat plan.effects.length).toNat = plan.effects.length := by
    simp [UInt8.toNat_ofNat', Nat.mod_eq_of_lt countLt]
  have recordsLength := flatMap_effect_length plan.effects
  simp [encodeEffectPlan, encodeEffectHeader, effectMagic, DClutch.Codec.magic,
    decodeEffectPlan, countByte, countFits, recordsLength,
    decodeEffects_encodeEffects plan.effects encodable]

/-! ## Typed certificate layout and canonical bytes -/

inductive CertificateField where
  | magic | version | kind | reservedHeader
  | market | route | sourceMaterial | product | providerEvidence
  | fundingAllocation | receiptAccount
  | generation | attemptIndex | scheduleIndex | selector | reservedBody
  | workPaid | fundingRemaining | resultNumerator | resultDenominator | observedAt
  deriving DecidableEq, Repr

def certificateSchema : List (FieldSpec CertificateField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.kind, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.route, .bytes 32⟩,
  ⟨.sourceMaterial, .bytes 32⟩,
  ⟨.product, .bytes 32⟩,
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

def certificateLayout : List (PlacedField CertificateField) :=
  DClutch.AbiSchema.specialize certificateSchema

def certificateBytes : Nat := schemaWidth certificateSchema

theorem certificate_width : certificateBytes = 312 := by
  native_decide

theorem certificate_fields_disjoint : certificateLayout.Pairwise Before := by
  exact specializeFrom_pairwise 0 certificateSchema

theorem certificate_names_unique :
    (certificateSchema.map fun field => field.name).Nodup := by
  native_decide

def certificateMagic : List UInt8 :=
  [0x44, 0x43, 0x53, 0x52, 0x43, 0x45, 0x52, 0x31] -- `DCSRCER1`

/-- Canonical two's-complement residue for a fixed byte width. -/
def signedResidue (width : Nat) (value : Int) : Nat :=
  if value < 0 then 256 ^ width - value.natAbs else value.natAbs

def encodeCertificate (certificate : Certificate) : List UInt8 :=
  certificateMagic ++
  DClutch.Codec.encodeLE 2 1 ++
  [UInt8.ofNat certificate.kind.tag] ++
  List.replicate 5 0 ++
  DClutch.Codec.encodeLE 32 certificate.marketId ++
  DClutch.Codec.encodeLE 32 certificate.routeId ++
  DClutch.Codec.encodeLE 32 certificate.sourceMaterialId ++
  DClutch.Codec.encodeLE 32 certificate.productId ++
  DClutch.Codec.encodeLE 32 certificate.providerEvidenceId ++
  DClutch.Codec.encodeLE 32 certificate.fundingAllocationId ++
  DClutch.Codec.encodeLE 32 certificate.receiptAccountId ++
  DClutch.Codec.encodeLE 8 certificate.generation ++
  DClutch.Codec.encodeLE 4 certificate.attemptIndex ++
  DClutch.Codec.encodeLE 4 certificate.scheduleIndex ++
  DClutch.Codec.encodeLE 4 certificate.selector ++
  List.replicate 4 0 ++
  DClutch.Codec.encodeLE 8 certificate.workPaid ++
  DClutch.Codec.encodeLE 8 certificate.fundingRemaining ++
  DClutch.Codec.encodeLE 16 (signedResidue 16 certificate.result.numerator) ++
  DClutch.Codec.encodeLE 8 certificate.result.denominator ++
  DClutch.Codec.encodeLE 8 certificate.observedAt

theorem encodeCertificate_length (certificate : Certificate) :
    (encodeCertificate certificate).length = certificateBytes := by
  calc
    (encodeCertificate certificate).length = 312 := by
      simp [encodeCertificate, certificateMagic, DClutch.Codec.encodeLE_length]
    _ = certificateBytes := certificate_width.symm

/-- Semantic encodability is independent of Solana account capacity.  The
`i128` premise is the separately named physical V1 result profile. -/
def CertificateEncodable (certificate : Certificate) : Prop :=
  certificate.marketId < 256 ^ 32 ∧ certificate.routeId < 256 ^ 32 ∧
  certificate.sourceMaterialId < 256 ^ 32 ∧ certificate.productId < 256 ^ 32 ∧
  certificate.providerEvidenceId < 256 ^ 32 ∧
  certificate.fundingAllocationId < 256 ^ 32 ∧
  certificate.receiptAccountId < 256 ^ 32 ∧
  certificate.generation < 256 ^ 8 ∧ certificate.attemptIndex < 256 ^ 4 ∧
  certificate.scheduleIndex < 256 ^ 4 ∧ certificate.selector < 256 ^ 4 ∧
  certificate.workPaid < 256 ^ 8 ∧ certificate.fundingRemaining < 256 ^ 8 ∧
  -(2 ^ 127 : Int) ≤ certificate.result.numerator ∧
  certificate.result.numerator < (2 ^ 127 : Int) ∧
  certificate.result.denominator < 256 ^ 8 ∧ certificate.observedAt < 256 ^ 8

/-! ## Emission bounds and executable examples -/

theorem specialize_effect_count_bounded
    {config : Config} {state : State} {funding : FundingState}
    {command : Command} {plan : Plan}
    (h : specialize config state funding command = .ok plan) :
    plan.effectPlan.effects.length ≤ maximumEffects := by
  unfold specialize at h
  simp only [bind, Except.bind] at h
  repeat' split at h
  all_goals try contradiction
  all_goals
    simp only [pure, Except.pure, Except.ok.injEq] at h
    subst plan
    simp [fundingEffects, maximumEffects]

def exampleDomain : ProductDomain := {
  productId := 21
  coordinateDomainId := 22
  resultUnitId := 23
  releaseId := 24
  cutDenominator := 1
  cuts := [0, 10]
}

def exampleRelease (seed : Nat) : ProviderRelease := {
  sourceMaterialId := 31
  sourceId := 32 + seed
  providerFamilyId := 33
  providerReleaseId := 34 + seed
  adapterReleaseId := 35
  decodingRulesId := 36
  transportProfileId := 37
  scheduleId := 38 + seed
}

/-- A real-shaped leg: the window has width, and the acceptance deadline sits
after the window closes.  A degenerate `windowStart = windowEnd` example would
pass every check below while hiding the case a real provider cadence produces. -/
def examplePrimary : Leg := {
  release := exampleRelease 0
  scheduleIndex := 0
  windowStart := 1_000
  windowEnd := 1_006
  acceptThrough := 1_010
  maximumPublicationAge := 20
  fundingAllocationId := 41
  workQuote := 3
}

def exampleRecovery : RecoveryAttempt := {
  leg := {
    release := exampleRelease 1
    scheduleIndex := 1
    windowStart := 1_020
    windowEnd := 1_026
    acceptThrough := 1_030
    maximumPublicationAge := 20
    fundingAllocationId := 42
    workQuote := 4
  }
  entryFundingAllocationId := 43
  entryWorkQuote := 2
}

def exampleConfig : Config := {
  marketId := 11
  generation := 1
  sourceOwnerId := 12
  sourceStateId := 13
  productResolutionStateId := 14
  receiptOwnerId := 15
  productDomain := exampleDomain
  primary := examplePrimary
  recoveries := [exampleRecovery]
  exhaustFundingAllocationId := 44
  exhaustWorkQuote := 2
  failureFundingAllocationId := 45
  failureWorkQuote := 5
}

def wrongMaterialRecovery : RecoveryAttempt := {
  exampleRecovery with
  leg := {
    exampleRecovery.leg with
    release := {
      exampleRecovery.leg.release with sourceMaterialId := 999
    }
  }
}

/-- A well-formed recovery leg that nevertheless closes before the primary does.
The refusal is the ordering rule, not the leg's own shape, so the leg is kept
internally valid (`windowStart ≤ windowEnd ≤ acceptThrough`). -/
def earlyRecovery : RecoveryAttempt := {
  exampleRecovery with
  leg := { exampleRecovery.leg with
    windowStart := 1_000, windowEnd := 1_004, acceptThrough := 1_005 }
}

/-- A leg whose window closes after this cluster stops accepting it. Nothing
could ever submit an admissible observation for the tail of that window. -/
def unacceptableRecovery : RecoveryAttempt := {
  exampleRecovery with
  leg := { exampleRecovery.leg with windowEnd := 1_031 }
}

def exampleState : State := {
  marketId := 11
  generation := 1
  sourceMaterialId := 31
  phase := .primary
  transitionSequence := 0
  terminalEvidenceId := 0
}

def exampleEvidence : NormalizedEvidence := {
  adapterAuthenticated := true
  sourceMaterialId := 31
  sourceId := 32
  providerFamilyId := 33
  providerReleaseId := 34
  adapterReleaseId := 35
  decodingRulesId := 36
  transportProfileId := 37
  scheduleId := 38
  scheduleIndex := 0
  observationTime := 1_000
  publicationTime := 995
  evidenceId := 51
  value := ⟨5, 1⟩
}

def exampleFunding (allocation capital : Nat) : FundingState := {
  allocationId := allocation
  initialCapital := capital
  remainingCapital := capital
  paidCapital := 0
  callCount := 0
}

#guard exampleConfig.valid
#guard !({ exampleConfig with recoveries := [wrongMaterialRecovery] }).valid
#guard !({ exampleConfig with recoveries := [earlyRecovery] }).valid
#guard !({ exampleConfig with recoveries := [unacceptableRecovery] }).valid
#guard !({ exampleConfig with
  primary := { examplePrimary with windowStart := 1_007 } }).valid
#guard exampleState.valid
#guard exampleDomain.map (.observed ⟨-1, 1⟩) == 0
#guard exampleDomain.map (.observed ⟨0, 1⟩) == 1
#guard exampleDomain.map (.observed ⟨10, 1⟩) == 2
#guard exampleDomain.map .failure == 3

def acceptedExample : Except Refusal Plan :=
  specialize exampleConfig exampleState (exampleFunding 41 10)
    (.accept exampleEvidence 1_005 61 62)

#guard match acceptedExample with
  | .ok plan =>
      plan.sourcePost.phase == .resolved 1 &&
      plan.fundingPost.remainingCapital == 7 &&
      plan.certificate.selector == 1 &&
      plan.effectPlan.effects.length == 5 &&
      (encodeEffectPlan plan.effectPlan).length == 88 &&
      (encodeCertificate plan.certificate).length == 312
  | .error _ => false

#guard match specialize exampleConfig exampleState (exampleFunding 43 10)
    (.failNext 1_011 61 63) with
  | .ok plan =>
      plan.sourcePost.phase == .recovery 0 &&
      plan.fundingPost.remainingCapital == 8 &&
      plan.certificate.kind == .recoveryAdvanced
  | .error _ => false

def exhaustedState : State := {
  exampleState with phase := .exhausted, transitionSequence := 2
}

#guard match specialize exampleConfig exhaustedState (exampleFunding 45 10)
    (.commitFailure 61 64) with
  | .ok plan =>
      plan.sourcePost.phase == .failureCommitted &&
      plan.certificate.selector == exampleDomain.failureSelector &&
      plan.fundingPost.paidCapital == 5 &&
      plan.effectPlan.effects.length == 5
  | .error _ => false

#guard match specialize exampleConfig exampleState (exampleFunding 45 10)
    (.commitFailure 61 64) with
  | .error .notExhausted => true
  | _ => false

#guard match checkEvidence examplePrimary
    { exampleEvidence with providerReleaseId := 999 } 1_005 with
  | .error .wrongRelease => true
  | _ => false

/-! ### The widened window, executed

`examplePrimary` sells `[1_000, 1_006]`.  These guards walk the whole shape a
one-instant window could not express: an observation strictly inside the window
resolves, both edges resolve, one second outside each edge refuses with its own
refusal, and a submission after `acceptThrough` refuses even though its
observation is squarely inside the window. -/

/-! An observation from the interior of the window — impossible under a
one-instant window, and the ordinary case on a real provider cadence. -/
#guard match specialize exampleConfig exampleState (exampleFunding 41 10)
    (.accept { exampleEvidence with observationTime := 1_003 } 1_005 61 62) with
  | .ok plan => plan.sourcePost.phase == .resolved 1
  | .error _ => false

/-! Both closed edges are inside. -/
#guard match checkEvidence examplePrimary
    { exampleEvidence with observationTime := 1_000 } 1_005 with
  | .ok _ => true
  | .error _ => false

#guard match checkEvidence examplePrimary
    { exampleEvidence with observationTime := 1_006 } 1_006 with
  | .ok _ => true
  | .error _ => false

/-! One second before the window opens. -/
#guard match checkEvidence examplePrimary
    { exampleEvidence with observationTime := 999 } 1_005 with
  | .error .beforeObservationTime => true
  | _ => false

/-! One second after the window closes: a *late* observation, the exact shape a
provider cadence straddling the deadline produces. -/
#guard match checkEvidence examplePrimary
    { exampleEvidence with observationTime := 1_007 } 1_008 with
  | .error .wrongObservationTime => true
  | _ => false

/-! Squarely inside the window, submitted after this cluster stopped accepting. -/
#guard match checkEvidence examplePrimary
    { exampleEvidence with observationTime := 1_006, publicationTime := 1_005 }
    1_011 with
  | .error .legExpired => true
  | _ => false

/-! Exactly one answer, executed: the post-state of a successful acceptance
refuses a second admissible observation from elsewhere in the same window. -/
#guard match acceptedExample with
  | .ok plan =>
      match specialize exampleConfig plan.sourcePost plan.fundingPost
          (.accept { exampleEvidence with
            observationTime := 1_004, evidenceId := 52 } 1_006 61 65) with
        | .error .wrongPhase => true
        | _ => false
  | .error _ => false

end DClutch.SourceResolution.Abi
