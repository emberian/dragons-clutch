import Std.Tactic

/-!
# Capability-neutral execution-release admission

This module is the small formal interface between protocol semantics and the
Registry/Core deployment boundary.  A consumer selects one immutable release
set.  A current receipt must bind the selected set, semantic role, Program,
artifact release, semantic release, activated cache, and current deployment.

The receipt booleans are explicit adapter assumptions.  Loader V3 parsing,
ProgramData linkage, ELF hashing, upgrade-authority checks, account ownership,
and return-data authentication are not proved here.  There is deliberately no
mock-authority or caller-supplied bypass alternative.
-/

namespace DClutch.ExecutionRelease

/-- Abstract content or account identity.  The physical ABI refines this to a
fixed 32-byte nonzero identity. -/
abbrev Identity := Nat

/-- The five independently replaceable semantic execution roles. -/
inductive Role where
  | core
  | claims
  | trading
  | resolution
  | custody
  deriving DecidableEq, Repr

def allRoles : List Role := [
  .core, .claims, .trading, .resolution, .custody
]

/-- One exact deployed role binding. -/
structure Binding where
  program : Identity
  artifactRelease : Identity
  semanticRelease : Identity
  deriving DecidableEq, Repr

/-- Immutable selected release set.  Aliased Programs must share the entire
binding, rather than presenting role-dependent artifact truths. -/
structure ReleaseSet where
  releaseSetId : Identity
  core : Binding
  claims : Binding
  trading : Binding
  resolution : Binding
  custody : Binding
  deriving DecidableEq, Repr

def ReleaseSet.binding (releaseSet : ReleaseSet) : Role -> Binding
  | .core => releaseSet.core
  | .claims => releaseSet.claims
  | .trading => releaseSet.trading
  | .resolution => releaseSet.resolution
  | .custody => releaseSet.custody

def identityValid (identity : Identity) : Bool :=
  identity != 0

def bindingValid (binding : Binding) : Bool :=
  identityValid binding.program &&
  identityValid binding.artifactRelease &&
  identityValid binding.semanticRelease

def aliasedBindingsCoherent (left right : Binding) : Bool :=
  decide (left.program != right.program || left = right)

def releaseSetValid (releaseSet : ReleaseSet) : Bool :=
  identityValid releaseSet.releaseSetId &&
  allRoles.all (fun role => bindingValid (releaseSet.binding role)) &&
  allRoles.all (fun left =>
    allRoles.all (fun right =>
      aliasedBindingsCoherent
        (releaseSet.binding left) (releaseSet.binding right)))

/-- Normalized output of the Registry/Core adapter for one current role. -/
structure Receipt where
  registryProgram : Identity
  releaseSetId : Identity
  role : Role
  observed : Binding
  activationCacheAuthenticated : Bool
  currentDeploymentReauthenticated : Bool
  deriving DecidableEq, Repr

/-- A Market or capability supplies its immutable Registry and release-set
selection alongside the current normalized Registry receipt. -/
structure Admission where
  marketRegistryProgram : Identity
  marketReleaseSetId : Identity
  selected : ReleaseSet
  receipt : Receipt
  deriving DecidableEq, Repr

/-- Propositional view used by named proofs. -/
def Admissible (admission : Admission) (expectedRole : Role) : Prop :=
  releaseSetValid admission.selected = true /\
  identityValid admission.marketRegistryProgram = true /\
  admission.marketReleaseSetId = admission.selected.releaseSetId /\
  admission.receipt.registryProgram = admission.marketRegistryProgram /\
  admission.receipt.releaseSetId = admission.selected.releaseSetId /\
  admission.receipt.role = expectedRole /\
  admission.receipt.observed = admission.selected.binding expectedRole /\
  admission.receipt.activationCacheAuthenticated = true /\
  admission.receipt.currentDeploymentReauthenticated = true

instance (admission : Admission) (expectedRole : Role) :
    Decidable (Admissible admission expectedRole) := by
  unfold Admissible
  infer_instance

/-- Executable release admission. -/
def admits (admission : Admission) (expectedRole : Role) : Bool :=
  decide (Admissible admission expectedRole)

theorem admits_iff_admissible
    (admission : Admission) (expectedRole : Role) :
    admits admission expectedRole = true <-> Admissible admission expectedRole := by
  simp [admits]

/-- An admitted consumer executes exactly the role binding selected by its
immutable release set. -/
theorem admitted_binding_is_exact
    (admission : Admission) (expectedRole : Role)
    (accepted : admits admission expectedRole = true) :
    admission.receipt.observed = admission.selected.binding expectedRole := by
  rcases (admits_iff_admissible admission expectedRole).mp accepted with
    ⟨_, _, _, _, _, _, observed, _, _⟩
  exact observed

/-- Admission contains no mock or caller assertion: both authenticated cache
and current-deployment observations must be true. -/
theorem admitted_only_through_current_registry_receipt
    (admission : Admission) (expectedRole : Role)
    (accepted : admits admission expectedRole = true) :
    admission.receipt.registryProgram = admission.marketRegistryProgram /\
    admission.receipt.activationCacheAuthenticated = true /\
    admission.receipt.currentDeploymentReauthenticated = true := by
  rcases (admits_iff_admissible admission expectedRole).mp accepted with
    ⟨_, _, _, registry, _, _, _, cache, current⟩
  exact ⟨registry, cache, current⟩

/-- Substituting the Market's selected release-set identity is always refused. -/
theorem substituted_market_selection_refuses
    (admission : Admission) (expectedRole : Role)
    (substituted : admission.marketReleaseSetId ≠ admission.selected.releaseSetId) :
    admits admission expectedRole = false := by
  simp [admits, Admissible, substituted]

/-- A receipt produced by any Registry other than the Market's immutable
Registry coordinate is refused, even if that producer equals the Core role
Program. -/
theorem substituted_market_registry_refuses
    (admission : Admission) (expectedRole : Role)
    (substituted :
      admission.receipt.registryProgram ≠ admission.marketRegistryProgram) :
    admits admission expectedRole = false := by
  simp [admits, Admissible, substituted]

end DClutch.ExecutionRelease
