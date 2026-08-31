import DClutchSemantics.GeneralControllerAbi
import DClutchSemantics.GeneralControllerRequestV2
import DClutchSemantics.GeneralControllerRequestV3
import DClutchSemantics.RequestProfileAbi

/-!
# General action request profiles

Each General action selects one exact RequestProfile through CapabilityProgramSetV1.
The profile revalidates the same action, the complete fixed-width controller
request, every reserved byte, and the action-specific coordinate grammar before
projecting caller-owned registers. This is a data artifact, not a family switch
inside Trading.
-/

namespace DClutch.General.RequestProfilesV1

open DClutch
open DClutch.General.ControllerAbi
open DClutch.RequestProfileAbi

def requestMagicWord : Nat := Codec.decodeLE ControllerRequestV2.requestMagic

/-- The width-preserving V3 request magic the GEN-SEVEN actions speak. Same 64
bytes, same action selector at offset 10; the wire break is the magic and the
three-bump tail grammar. -/
def requestMagicWordV3 : Nat := Codec.decodeLE ControllerRequestV3.requestMagic

def requirePrefix (action : Action) : List Operation := [
  ⟨.requireU64, false, false, 0, 0, requestMagicWord⟩,
  ⟨.requireU16, false, false, 8, 0, ControllerRequestV2.abiVersion⟩,
  ⟨.requireU8, false, false, 10, 0, action.tag.toNat⟩,
  ⟨.requireZeroRange, false, false, 12, 0, 4⟩
]

def requirePrefixV3 (action : Action) : List Operation := [
  ⟨.requireU64, false, false, 0, 0, requestMagicWordV3⟩,
  ⟨.requireU16, false, false, 8, 0, ControllerRequestV3.abiVersion⟩,
  ⟨.requireU8, false, false, 10, 0, action.tag.toNat⟩,
  ⟨.requireZeroRange, false, false, 12, 0, 4⟩
]

/-- OpenBatch and CloseBatch. The optimistic root revision (offset 16) lands in
scalar 94 (`ROOT_EXPECTED_REVISION`), the batch identity (offset 24) in
identity 29 (`SELECTION_BATCH`) -- the same register the batch-state PDA recipe
is keyed by -- and the primary bump witness in scalar 69. Every other
coordinate of the exact V3 grammar is required zero, so an unused byte cannot
become an unauthenticated extension point. -/
def batchCoordinates : List Operation := [
  ⟨.requireU8, false, false, 11, 0, 0⟩,
  ⟨.projectU64, false, false, 16, 94, 0⟩,
  ⟨.projectIdentity, false, false, 24, 29, 0⟩,
  ⟨.requireU32, false, false, 56, 0, 0⟩,
  ⟨.requireU8, false, false, 60, 0, 0⟩,
  ⟨.projectU8, false, false, 61, 69, 0⟩,
  ⟨.requireU8, false, false, 62, 0, 0⟩,
  ⟨.requireU8, false, false, 63, 0, 0⟩
]

def manifestOrderCoordinate : List Operation := [
  ⟨.projectU8, false, false, 11, 87, 0⟩
]

def requireNoManifestOrder : List Operation := [
  ⟨.requireU8, false, false, 11, 0, 0⟩
]

def bumpCoordinates (action : Action) : List Operation := [
  ⟨.projectU8, false, false, 61, 69, 0⟩,
  if action = .close then
    ⟨.projectU8, false, false, 62, 70, 0⟩
  else
    ⟨.requireU8, false, false, 62, 0, 0⟩,
  ⟨.requireZeroRange, false, false, 63, 0, 1⟩
]

def rowCoordinates : List Operation := [
  ⟨.projectU64, false, false, 16, 0, 0⟩,
  ⟨.projectIdentity, false, false, 24, 0, 0⟩,
  ⟨.projectU32, false, false, 56, 1, 0⟩,
  ⟨.projectU8, false, false, 60, 2, 0⟩
]

def candidateCoordinates : List Operation := [
  ⟨.projectU64, false, false, 16, 0, 0⟩,
  ⟨.projectIdentity, false, false, 24, 0, 0⟩,
  ⟨.requireU32, false, false, 56, 0, 0⟩,
  ⟨.requireU8, false, false, 60, 0, 0⟩
]

def freezeCoordinates : List Operation := [
  ⟨.projectU64, false, false, 16, 0, 0⟩,
  ⟨.requireZeroRange, false, false, 24, 0, 32⟩,
  ⟨.requireU32, false, false, 56, 0, 0⟩,
  ⟨.requireU8, false, false, 60, 0, 0⟩
]

def profile (action : Action) : Profile := {
  fixedRequestBytes := ControllerRequestV2.requestBytes
  itemRequestBytes := 0
  -- The common Strategy bank has one stable geometry for all action-selected
  -- programs. Request projection fills only the action's coordinates; account
  -- projection and Transition own the remaining values.
  -- Coordinates 88 and 89 are the root-lifecycle conjunct: 88 is the
  -- AccountProfile-projected capability-root lifecycle byte and 89 is the
  -- Transition-owned Active constant. Neither is request-projected, because a
  -- caller may not state whether the capability it is acting on is still live.
  -- Coordinates 90..150 are the GEN-SEVEN widening: the collection and
  -- candidate banks. No settlement action's request touches them.
  commonScalars := 151
  itemScalarStride := 6
  commonIdentities := 45
  itemIdentityStride := 0
  fixedOperations :=
    if action = .openBatch || action = .closeBatch then
      requirePrefixV3 action ++ batchCoordinates
    else
      requirePrefix action ++
        (if action = .collect || action = .distribute then manifestOrderCoordinate
          else requireNoManifestOrder) ++
        (if action = .freeze then freezeCoordinates else if action = .consider ||
          action = .collect || action = .distribute then rowCoordinates else candidateCoordinates) ++
        bumpCoordinates action
  itemOperations := []
}

def actions : List Action := [
  .consider, .freeze, .initializeSettlement, .collect, .materialize, .distribute, .close,
  .openBatch, .closeBatch
]

def profiles : List Profile := actions.map profile

theorem every_profile_is_well_formed :
    profiles.all Profile.wellFormed = true := by native_decide

theorem every_profile_round_trips :
    profiles.all fun value => decodeProfile (encodeProfile value) = some value := by native_decide

theorem every_profile_has_exact_request_width :
    profiles.all fun value => value.requestWidth 0 = ControllerRequestV2.requestBytes := by
  native_decide

theorem all_actions_have_distinct_checked_profiles :
    (actions.map fun action => encodeProfile (profile action)).Pairwise (· ≠ ·) := by
  native_decide

theorem freeze_has_no_candidate_projection :
    (profile .freeze).commonIdentities = 45 ∧
      (profile .freeze).fixedOperations.any
        (fun operation => operation.kind = .requireZeroRange ∧
          operation.requestOffset = 24 ∧ operation.immediate = 32) := by native_decide

theorem row_actions_project_runtime_coordinates :
    (profile .consider).commonScalars = 151 ∧
      (profile .collect).commonScalars = 151 ∧
      (profile .distribute).commonScalars = 151 ∧
      (profile .consider).itemScalarStride = 6 := by native_decide

/-- No action's request may write the root-lifecycle conjunct at 88 or 89. -/
theorem no_action_request_projects_the_root_lifecycle_conjunct :
    actions.all fun action =>
      (profile action).fixedOperations.all
        (fun operation => operation.register ≠ 88 ∧ operation.register ≠ 89) := by
  native_decide

theorem settlement_rows_alone_project_manifest_order :
    ((profile .collect).fixedOperations.any
      (fun operation => operation.kind = .projectU8 ∧
        operation.requestOffset = 11 ∧ operation.register = 87)) ∧
    ((profile .distribute).fixedOperations.any
      (fun operation => operation.kind = .projectU8 ∧
        operation.requestOffset = 11 ∧ operation.register = 87)) ∧
    ([.consider, .freeze, .initializeSettlement, .materialize, .close].all fun action =>
      (profile action).fixedOperations.any
        (fun operation => operation.kind = .requireU8 ∧
          operation.requestOffset = 11 ∧ operation.immediate = 0)) := by
  native_decide

theorem close_alone_projects_terminal_record_bump :
    (profile .close).fixedOperations.any
        (fun operation => operation.kind = .projectU8 ∧
          operation.requestOffset = 62 ∧ operation.register = 70) ∧
      (actions.erase .close).all fun action =>
        (profile action).fixedOperations.any
          (fun operation => operation.kind = .requireU8 ∧
            operation.requestOffset = 62 ∧ operation.immediate = 0) := by
  native_decide

/-- The batch pair speaks the V3 request: it requires the V3 magic word, and it
projects the optimistic root revision into the root replay-guard register the
transition compares against the observed root. The settlement seven keep the V2
magic, so one request byte stream can never satisfy both grammars. -/
theorem the_batch_pair_speaks_the_v3_request :
    ([Action.openBatch, Action.closeBatch].all fun action =>
      (profile action).fixedOperations.any (fun operation =>
        operation.kind = .requireU64 && operation.requestOffset = 0 &&
          operation.immediate = requestMagicWordV3) &&
      (profile action).fixedOperations.any (fun operation =>
        operation.kind = .projectU64 && operation.requestOffset = 16 &&
          operation.register = 94) &&
      (profile action).fixedOperations.any (fun operation =>
        operation.kind = .projectIdentity && operation.requestOffset = 24 &&
          operation.register = 29)) = true ∧
    requestMagicWordV3 ≠ requestMagicWord := by native_decide

end DClutch.General.RequestProfilesV1
