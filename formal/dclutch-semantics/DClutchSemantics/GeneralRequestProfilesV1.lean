import DClutchSemantics.GeneralControllerAbi
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

def requestMagicWord : Nat := Codec.decodeLE requestMagic

def requirePrefix (action : Action) : List Operation := [
  ⟨.requireU64, false, false, 0, 0, requestMagicWord⟩,
  ⟨.requireU16, false, false, 8, 0, abiVersion⟩,
  ⟨.requireU8, false, false, 10, 0, action.tag.toNat⟩,
  ⟨.requireZeroRange, false, false, 11, 0, 5⟩
]

def requireSuffix : List Operation := [
  ⟨.requireZeroRange, false, false, 61, 0, 3⟩
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
  fixedRequestBytes := requestBytes
  itemRequestBytes := 0
  -- The common Strategy bank has one stable geometry for all action-selected
  -- programs. Request projection fills only the action's coordinates; account
  -- projection and Transition own the remaining values.
  commonScalars := 44
  itemScalarStride := 2
  commonIdentities := 27
  itemIdentityStride := 0
  fixedOperations := requirePrefix action ++
    (if action = .freeze then freezeCoordinates else if action = .consider ||
      action = .collect || action = .distribute then rowCoordinates else candidateCoordinates) ++
    requireSuffix
  itemOperations := []
}

def actions : List Action := [
  .consider, .freeze, .initializeSettlement, .collect, .materialize, .distribute, .close
]

def profiles : List Profile := actions.map profile

theorem every_profile_is_well_formed :
    profiles.all Profile.wellFormed = true := by native_decide

theorem every_profile_round_trips :
    profiles.all fun value => decodeProfile (encodeProfile value) = some value := by native_decide

theorem every_profile_has_exact_request_width :
    profiles.all fun value => value.requestWidth 0 = requestBytes := by native_decide

theorem all_actions_have_distinct_checked_profiles :
    (actions.map fun action => encodeProfile (profile action)).Pairwise (· ≠ ·) := by
  native_decide

theorem freeze_has_no_candidate_projection :
    (profile .freeze).commonIdentities = 27 ∧
      (profile .freeze).fixedOperations.any
        (fun operation => operation.kind = .requireZeroRange ∧
          operation.requestOffset = 24 ∧ operation.immediate = 32) := by native_decide

theorem row_actions_project_runtime_coordinates :
    (profile .consider).commonScalars = 44 ∧
      (profile .collect).commonScalars = 44 ∧
      (profile .distribute).commonScalars = 44 ∧
      (profile .consider).itemScalarStride = 2 := by native_decide

end DClutch.General.RequestProfilesV1
