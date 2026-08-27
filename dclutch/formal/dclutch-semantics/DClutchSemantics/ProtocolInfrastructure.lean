import DClutchSemantics.ExecutionRelease
import DClutchSemantics.ProtocolInfrastructureProfileAbi
import Std.Tactic

/-!
# Protocol infrastructure bootstrap authority

This module owns the pure authority chain behind the per-Core infrastructure
profile.  A current Core ProgramData upgrade-authority signer may initialize
the one vacant profile PDA exactly once.  Found must then authenticate that
Core-owned profile before it uses the selected Registry or Rent artifacts.

The physical adapter remains responsible for Loader V3 parsing, Program and
ProgramData ownership/linkage, complete ELF hashing, upgrade-authority
observations, account ownership, PDA derivation, signatures, finalized-record
proofs, and transaction rollback.  The booleans below are named assumptions at
that adapter boundary; there is no caller-asserted or Registry-first bypass.
-/

namespace DClutch.ProtocolInfrastructure

abbrev Identity := ExecutionRelease.Identity

/-! ## Immutable profile -/

/-- Exact program and ArtifactRelease content selection. -/
structure Binding where
  program : Identity
  artifactRelease : Identity
  deriving DecidableEq, Repr

def Binding.valid (binding : Binding) : Bool :=
  ExecutionRelease.identityValid binding.program &&
  ExecutionRelease.identityValid binding.artifactRelease

/-- The one immutable per-Core selection of protocol infrastructure. -/
structure Profile where
  registry : Binding
  rent : Binding
  deriving DecidableEq, Repr

def Profile.valid (profile : Profile) : Bool :=
  profile.registry.valid && profile.rent.valid &&
  profile.registry.program != profile.rent.program &&
  profile.registry.artifactRelease != profile.rent.artifactRelease

theorem valid_profile_bindings_are_nonzero
    (profile : Profile) (valid : profile.valid = true) :
    profile.registry.valid = true /\ profile.rent.valid = true := by
  simp only [Profile.valid, Bool.and_eq_true] at valid
  exact valid.1.1

theorem valid_profile_infrastructure_is_distinct
    (profile : Profile) (valid : profile.valid = true) :
    profile.registry.program ≠ profile.rent.program /\
      profile.registry.artifactRelease ≠ profile.rent.artifactRelease := by
  simp only [Profile.valid, Bool.and_eq_true, bne_iff_ne] at valid
  exact ⟨valid.1.2, valid.2⟩

/-! ## One-time initialization -/

/-- Normalized current Core Loader observation used only for profile init. -/
structure CoreProgramDataInitObservation where
  currentCoreProgram : Identity
  observedCoreProgram : Identity
  linkedProgramData : Identity
  observedProgramData : Identity
  currentUpgradeAuthority : Option Identity
  initializerSigner : Identity
  initializerSigned : Bool
  loaderV3AccountFactsAuthenticated : Bool
  deriving DecidableEq, Repr

def CoreProgramDataInitObservation.accepts
    (observation : CoreProgramDataInitObservation) : Bool :=
  ExecutionRelease.identityValid observation.currentCoreProgram &&
  observation.observedCoreProgram == observation.currentCoreProgram &&
  ExecutionRelease.identityValid observation.linkedProgramData &&
  observation.observedProgramData == observation.linkedProgramData &&
  ExecutionRelease.identityValid observation.initializerSigner &&
  observation.currentUpgradeAuthority == some observation.initializerSigner &&
  observation.initializerSigned && observation.loaderV3AccountFactsAuthenticated

/-- Exact vacant per-Core profile account observation. -/
structure ProfileInitAccountObservation where
  address : Identity
  expectedPerCorePda : Identity
  vacantSystemAccountAuthenticated : Bool
  deriving DecidableEq, Repr

def ProfileInitAccountObservation.accepts
    (observation : ProfileInitAccountObservation) : Bool :=
  ExecutionRelease.identityValid observation.address &&
  observation.address == observation.expectedPerCorePda &&
  observation.vacantSystemAccountAuthenticated

/-- Exact finalized ArtifactRelease and live deployment seen during init. -/
structure InitArtifactObservation where
  binding : Binding
  finalizedArtifactRecordAuthenticated : Bool
  currentDeploymentAuthenticated : Bool
  upgradePolicyImmutable : Bool
  currentProgramDataAuthority : Option Identity
  deriving DecidableEq, Repr

def InitArtifactObservation.accepts
    (expected : Binding) (observation : InitArtifactObservation) : Bool :=
  expected.valid && observation.binding == expected &&
  observation.finalizedArtifactRecordAuthenticated &&
  observation.currentDeploymentAuthenticated &&
  observation.upgradePolicyImmutable &&
  observation.currentProgramDataAuthority == none

/-- Complete normalized observation for one-time profile initialization. -/
structure InitializationObservation where
  profile : Profile
  core : CoreProgramDataInitObservation
  account : ProfileInitAccountObservation
  registryArtifact : InitArtifactObservation
  rentArtifact : InitArtifactObservation
  deriving DecidableEq, Repr

/-- Initialize only from the current Core upgrade-authority signer and exact
current Registry/Rent artifacts.  Core is distinct from both infrastructure
programs, making the selector independent of either selected implementation. -/
def initializationAccepts (observation : InitializationObservation) : Bool :=
  observation.profile.valid && observation.core.accepts && observation.account.accepts &&
  observation.core.currentCoreProgram != observation.profile.registry.program &&
  observation.core.currentCoreProgram != observation.profile.rent.program &&
  observation.registryArtifact.accepts observation.profile.registry &&
  observation.rentArtifact.accepts observation.profile.rent

theorem initialized_only_by_current_core_upgrade_authority
    (observation : InitializationObservation)
    (accepted : initializationAccepts observation = true) :
    observation.core.currentUpgradeAuthority =
        some observation.core.initializerSigner /\
      observation.core.initializerSigned = true := by
  simp only [initializationAccepts, Bool.and_eq_true, bne_iff_ne] at accepted
  have coreAccepted : observation.core.accepts = true := accepted.1.1.1.1.1.2
  simp only [CoreProgramDataInitObservation.accepts, Bool.and_eq_true, beq_iff_eq] at coreAccepted
  exact ⟨coreAccepted.1.1.2, coreAccepted.1.2⟩

theorem substituted_init_registry_refuses
    (observation : InitializationObservation)
    (substituted : observation.registryArtifact.binding ≠ observation.profile.registry) :
    initializationAccepts observation = false := by
  simp [initializationAccepts, InitArtifactObservation.accepts, substituted]

theorem substituted_init_rent_refuses
    (observation : InitializationObservation)
    (substituted : observation.rentArtifact.binding ≠ observation.profile.rent) :
    initializationAccepts observation = false := by
  simp [initializationAccepts, InitArtifactObservation.accepts, substituted]

/-! ## Ordered Found admission -/

/-- The first Found stage: exact decoded profile at its sole Core-owned PDA. -/
structure ProfileAccountObservation where
  currentCoreProgram : Identity
  address : Identity
  expectedPerCorePda : Identity
  owner : Identity
  profile : Profile
  exactCanonicalBytesAuthenticated : Bool
  deriving DecidableEq, Repr

structure AuthenticatedProfile where
  coreProgram : Identity
  profile : Profile
  deriving DecidableEq, Repr

/-- Authenticate the profile before consulting either selected program. -/
def authenticateProfile
    (observation : ProfileAccountObservation) : Option AuthenticatedProfile :=
  if observation.profile.valid &&
      ExecutionRelease.identityValid observation.currentCoreProgram &&
      observation.owner == observation.currentCoreProgram &&
      ExecutionRelease.identityValid observation.address &&
      observation.address == observation.expectedPerCorePda &&
      observation.currentCoreProgram != observation.profile.registry.program &&
      observation.currentCoreProgram != observation.profile.rent.program &&
      observation.exactCanonicalBytesAuthenticated then
    some { coreProgram := observation.currentCoreProgram, profile := observation.profile }
  else
    none

theorem authenticate_profile_is_exact
    (observation : ProfileAccountObservation) (authenticated : AuthenticatedProfile)
    (accepted : authenticateProfile observation = some authenticated) :
    authenticated.coreProgram = observation.currentCoreProgram /\
      authenticated.profile = observation.profile := by
  unfold authenticateProfile at accepted
  split at accepted
  · cases accepted
    exact ⟨rfl, rfl⟩
  · cases accepted

/-- Direct content/deployment observation made before Registry-owned state is
trusted.  Exact content identity is checked against the authenticated profile
or Market-selected Core binding; Registry account ownership is not authority
for this stage. -/
structure ImmutableArtifactObservation where
  binding : Binding
  artifactContentAuthenticated : Bool
  currentDeploymentAuthenticated : Bool
  upgradePolicyImmutable : Bool
  currentProgramDataAuthority : Option Identity
  deriving DecidableEq, Repr

def ImmutableArtifactObservation.accepts
    (expected : Binding) (observation : ImmutableArtifactObservation) : Bool :=
  expected.valid && observation.binding == expected &&
  observation.artifactContentAuthenticated &&
  observation.currentDeploymentAuthenticated &&
  observation.upgradePolicyImmutable &&
  observation.currentProgramDataAuthority == none

theorem substituted_immutable_artifact_refuses
    (expected : Binding) (observation : ImmutableArtifactObservation)
    (substituted : observation.binding ≠ expected) :
    observation.accepts expected = false := by
  simp [ImmutableArtifactObservation.accepts, substituted]

theorem mutable_artifact_refuses
    (expected : Binding) (observation : ImmutableArtifactObservation)
    (mutableArtifact : observation.upgradePolicyImmutable = false) :
    observation.accepts expected = false := by
  simp [ImmutableArtifactObservation.accepts, mutableArtifact]

/-- Result of the second Found stage.  Constructing this value requires the
Core, Registry, and Rent current deployments to match immutable artifacts. -/
structure AuthenticatedInfrastructure where
  coreProgram : Identity
  profile : Profile
  coreArtifact : Binding
  deriving DecidableEq, Repr

/-- Authenticate all three immutable artifacts only after profile admission. -/
def authenticateImmutableInfrastructure
    (profile : AuthenticatedProfile)
    (selectedCore : Binding)
    (coreArtifact registryArtifact rentArtifact : ImmutableArtifactObservation) :
    Option AuthenticatedInfrastructure :=
  if selectedCore.program == profile.coreProgram &&
      coreArtifact.accepts selectedCore &&
      registryArtifact.accepts profile.profile.registry &&
      rentArtifact.accepts profile.profile.rent then
    some {
      coreProgram := profile.coreProgram
      profile := profile.profile
      coreArtifact := selectedCore
    }
  else
    none

theorem authenticated_infrastructure_is_exact
    (profile : AuthenticatedProfile) (selectedCore : Binding)
    (coreArtifact registryArtifact rentArtifact : ImmutableArtifactObservation)
    (authenticated : AuthenticatedInfrastructure)
    (accepted : authenticateImmutableInfrastructure profile selectedCore coreArtifact
      registryArtifact rentArtifact = some authenticated) :
    authenticated.coreProgram = profile.coreProgram /\
      authenticated.profile = profile.profile /\
      authenticated.coreArtifact = selectedCore := by
  unfold authenticateImmutableInfrastructure at accepted
  split at accepted
  · cases accepted
    exact ⟨rfl, rfl, rfl⟩
  · cases accepted

/-- Registry/Rent-owned facts consumed only after exact immutable
infrastructure has authenticated. -/
structure DownstreamFoundObservation where
  marketRegistryProgram : Identity
  registryFinalizedRecordsAuthenticated : Bool
  registryActivationCacheAuthenticated : Bool
  rentCreditOwner : Identity
  rentCreditPdaAuthenticated : Bool
  deriving DecidableEq, Repr

def downstreamFoundAccepts
    (infrastructure : AuthenticatedInfrastructure)
    (observation : DownstreamFoundObservation) : Bool :=
  observation.marketRegistryProgram == infrastructure.profile.registry.program &&
  observation.registryFinalizedRecordsAuthenticated &&
  observation.registryActivationCacheAuthenticated &&
  observation.rentCreditOwner == infrastructure.profile.rent.program &&
  observation.rentCreditPdaAuthenticated

/-- Complete Found authority observation. -/
structure FoundObservation where
  profileAccount : ProfileAccountObservation
  selectedCore : Binding
  coreArtifact : ImmutableArtifactObservation
  registryArtifact : ImmutableArtifactObservation
  rentArtifact : ImmutableArtifactObservation
  downstream : DownstreamFoundObservation
  deriving DecidableEq, Repr

/-- Ordered Found admission: profile first, then direct immutable artifact
authentication, then and only then Registry-owned records/cache and RentCredit. -/
def foundAccepts (observation : FoundObservation) : Bool :=
  match authenticateProfile observation.profileAccount with
  | none => false
  | some profile =>
      match authenticateImmutableInfrastructure profile observation.selectedCore
          observation.coreArtifact observation.registryArtifact observation.rentArtifact with
      | none => false
      | some infrastructure => downstreamFoundAccepts infrastructure observation.downstream

theorem admitted_found_uses_profile_registry
    (observation : FoundObservation) (accepted : foundAccepts observation = true) :
    observation.downstream.marketRegistryProgram =
      observation.profileAccount.profile.registry.program := by
  unfold foundAccepts at accepted
  generalize profileEquation : authenticateProfile observation.profileAccount = profileResult at accepted
  cases profileResult with
  | none => contradiction
  | some profile =>
      have profileExact := authenticate_profile_is_exact observation.profileAccount profile profileEquation
      generalize infrastructureEquation :
          authenticateImmutableInfrastructure profile observation.selectedCore
            observation.coreArtifact observation.registryArtifact observation.rentArtifact =
            infrastructureResult at accepted
      cases infrastructureResult with
      | none =>
          simp [infrastructureEquation] at accepted
      | some infrastructure =>
          have infrastructureExact := authenticated_infrastructure_is_exact profile
            observation.selectedCore observation.coreArtifact observation.registryArtifact
            observation.rentArtifact infrastructure infrastructureEquation
          simp only [infrastructureEquation, downstreamFoundAccepts, Bool.and_eq_true,
            beq_iff_eq] at accepted
          calc
            observation.downstream.marketRegistryProgram =
                infrastructure.profile.registry.program := accepted.1.1.1.1
            _ = profile.profile.registry.program := by rw [infrastructureExact.2.1]
            _ = observation.profileAccount.profile.registry.program := by rw [profileExact.2]

theorem substituted_found_registry_binding_refuses
    (observation : FoundObservation)
    (substituted : observation.registryArtifact.binding ≠
      observation.profileAccount.profile.registry) :
    foundAccepts observation = false := by
  unfold foundAccepts
  generalize profileEquation : authenticateProfile observation.profileAccount = profileResult
  cases profileResult with
  | none => rfl
  | some profile =>
      have profileExact := authenticate_profile_is_exact observation.profileAccount profile profileEquation
      have refused : observation.registryArtifact.accepts profile.profile.registry = false := by
        apply substituted_immutable_artifact_refuses
        simpa [profileExact.2] using substituted
      unfold authenticateImmutableInfrastructure
      simp [refused]

theorem substituted_found_rent_binding_refuses
    (observation : FoundObservation)
    (substituted : observation.rentArtifact.binding ≠
      observation.profileAccount.profile.rent) :
    foundAccepts observation = false := by
  unfold foundAccepts
  generalize profileEquation : authenticateProfile observation.profileAccount = profileResult
  cases profileResult with
  | none => rfl
  | some profile =>
      have profileExact := authenticate_profile_is_exact observation.profileAccount profile profileEquation
      have refused : observation.rentArtifact.accepts profile.profile.rent = false := by
        apply substituted_immutable_artifact_refuses
        simpa [profileExact.2] using substituted
      unfold authenticateImmutableInfrastructure
      simp [refused]

theorem substituted_market_registry_refuses
    (observation : FoundObservation)
    (substituted : observation.downstream.marketRegistryProgram ≠
      observation.profileAccount.profile.registry.program) :
    foundAccepts observation = false := by
  unfold foundAccepts
  generalize profileEquation : authenticateProfile observation.profileAccount = profileResult
  cases profileResult with
  | none => rfl
  | some profile =>
      have profileExact := authenticate_profile_is_exact observation.profileAccount profile profileEquation
      generalize infrastructureEquation :
          authenticateImmutableInfrastructure profile observation.selectedCore
            observation.coreArtifact observation.registryArtifact observation.rentArtifact =
            infrastructureResult
      cases infrastructureResult with
      | none => simp [infrastructureEquation]
      | some infrastructure =>
          have infrastructureExact := authenticated_infrastructure_is_exact profile
            observation.selectedCore observation.coreArtifact observation.registryArtifact
            observation.rentArtifact infrastructure infrastructureEquation
          have refused : observation.downstream.marketRegistryProgram ≠
              infrastructure.profile.registry.program := by
            simpa [infrastructureExact.2.1, profileExact.2] using substituted
          simp [infrastructureEquation, downstreamFoundAccepts, refused]

theorem substituted_rent_credit_owner_refuses
    (observation : FoundObservation)
    (substituted : observation.downstream.rentCreditOwner ≠
      observation.profileAccount.profile.rent.program) :
    foundAccepts observation = false := by
  unfold foundAccepts
  generalize profileEquation : authenticateProfile observation.profileAccount = profileResult
  cases profileResult with
  | none => rfl
  | some profile =>
      have profileExact := authenticate_profile_is_exact observation.profileAccount profile profileEquation
      generalize infrastructureEquation :
          authenticateImmutableInfrastructure profile observation.selectedCore
            observation.coreArtifact observation.registryArtifact observation.rentArtifact =
            infrastructureResult
      cases infrastructureResult with
      | none => simp [infrastructureEquation]
      | some infrastructure =>
          have infrastructureExact := authenticated_infrastructure_is_exact profile
            observation.selectedCore observation.coreArtifact observation.registryArtifact
            observation.rentArtifact infrastructure infrastructureEquation
          have refused : observation.downstream.rentCreditOwner ≠
              infrastructure.profile.rent.program := by
            simpa [infrastructureExact.2.1, profileExact.2] using substituted
          simp [infrastructureEquation, downstreamFoundAccepts, refused]

theorem mutable_core_registry_or_rent_refuses
    (observation : FoundObservation)
    (mutableArtifact : observation.coreArtifact.upgradePolicyImmutable = false \/
      observation.registryArtifact.upgradePolicyImmutable = false \/
      observation.rentArtifact.upgradePolicyImmutable = false) :
    foundAccepts observation = false := by
  unfold foundAccepts
  generalize profileEquation : authenticateProfile observation.profileAccount = profileResult
  cases profileResult with
  | none => rfl
  | some profile =>
      rcases mutableArtifact with core | registry | rent
      · have refused : observation.coreArtifact.accepts observation.selectedCore = false :=
          mutable_artifact_refuses observation.selectedCore observation.coreArtifact core
        unfold authenticateImmutableInfrastructure
        simp [refused]
      · have refused : observation.registryArtifact.accepts profile.profile.registry = false :=
          mutable_artifact_refuses profile.profile.registry observation.registryArtifact registry
        unfold authenticateImmutableInfrastructure
        simp [refused]
      · have refused : observation.rentArtifact.accepts profile.profile.rent = false :=
          mutable_artifact_refuses profile.profile.rent observation.rentArtifact rent
        unfold authenticateImmutableInfrastructure
        simp [refused]

/-! ## Executable theorem regressions -/

namespace Examples

def coreBinding : Binding := ⟨1, 2⟩
def registryBinding : Binding := ⟨3, 4⟩
def rentBinding : Binding := ⟨5, 6⟩
def profile : Profile := ⟨registryBinding, rentBinding⟩

def init : InitializationObservation := {
  profile
  core := {
    currentCoreProgram := 1
    observedCoreProgram := 1
    linkedProgramData := 7
    observedProgramData := 7
    currentUpgradeAuthority := some 8
    initializerSigner := 8
    initializerSigned := true
    loaderV3AccountFactsAuthenticated := true
  }
  account := {
    address := 9
    expectedPerCorePda := 9
    vacantSystemAccountAuthenticated := true
  }
  registryArtifact := ⟨registryBinding, true, true, true, none⟩
  rentArtifact := ⟨rentBinding, true, true, true, none⟩
}

def immutableArtifact (binding : Binding) : ImmutableArtifactObservation := {
  binding
  artifactContentAuthenticated := true
  currentDeploymentAuthenticated := true
  upgradePolicyImmutable := true
  currentProgramDataAuthority := none
}

def found : FoundObservation := {
  profileAccount := {
    currentCoreProgram := 1
    address := 9
    expectedPerCorePda := 9
    owner := 1
    profile
    exactCanonicalBytesAuthenticated := true
  }
  selectedCore := coreBinding
  coreArtifact := immutableArtifact coreBinding
  registryArtifact := immutableArtifact registryBinding
  rentArtifact := immutableArtifact rentBinding
  downstream := {
    marketRegistryProgram := 3
    registryFinalizedRecordsAuthenticated := true
    registryActivationCacheAuthenticated := true
    rentCreditOwner := 5
    rentCreditPdaAuthenticated := true
  }
}

theorem valid_initialization_and_found_accept :
    initializationAccepts init = true /\ foundAccepts found = true := by
  native_decide

theorem mutable_or_authorized_initialization_refuses :
    initializationAccepts {
      init with registryArtifact := {
        init.registryArtifact with upgradePolicyImmutable := false
      }
    } = false /\
    initializationAccepts {
      init with rentArtifact := {
        init.rentArtifact with currentProgramDataAuthority := some 8
      }
    } = false := by
  native_decide

theorem same_width_registry_and_rent_substitutions_refuse :
    foundAccepts {
      found with registryArtifact := immutableArtifact ⟨10, 11⟩
    } = false /\
    foundAccepts {
      found with rentArtifact := immutableArtifact ⟨12, 13⟩
    } = false := by
  native_decide

theorem mutable_or_authorized_infrastructure_refuses :
    foundAccepts {
      found with registryArtifact := {
        immutableArtifact registryBinding with upgradePolicyImmutable := false
      }
    } = false /\
    foundAccepts {
      found with rentArtifact := {
        immutableArtifact rentBinding with currentProgramDataAuthority := some 8
      }
    } = false := by
  native_decide

end Examples

end DClutch.ProtocolInfrastructure
