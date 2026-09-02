import DClutchSemantics.ExecutionRelease
import DClutchSemantics.ProtocolInfrastructureProfileAbi
import Std.Tactic

/-!
# Protocol infrastructure bootstrap authority

This module owns the pure authority chain behind the per-Core infrastructure
profile.  A current Core ProgramData upgrade-authority signer may initialize
the one vacant profile PDA exactly once.  Found must then authenticate that
Core-owned profile before it uses the selected Registry or Rent artifacts.

Artifact admission is the **slot pin** of decision 0012
(`docs/decisions/0012-devnet-iteration-substrate.md`), not irrevocability.  A
finalized `ArtifactRelease` record binds an upgrade policy, an upgrade
authority, and the exact ProgramData deployment slot; a reader admits it only
when the release carries one of two canonical pairings and the *observed*
ProgramData authority and slot equal the *bound* ones.  The Loader V3 writes
the current slot into ProgramData on every `Upgrade`, refuses an `Upgrade` in
the deployment's own slot, and refuses the `Close` a redeploy would need — so
observed-slot equality proves the admitted ELF digest is still the digest of
the deployed bytes even for a deployment whose authority was never revoked.
An upgrade therefore does not forge admission; it *refuses* every dependent
open market by name, which is what `ReleaseSupersededByUpgrade` records.

The physical adapter remains responsible for Loader V3 parsing, Program and
ProgramData ownership/linkage, complete ELF hashing, upgrade-authority
observations, account ownership, PDA derivation, signatures, finalized-record
proofs, and transaction rollback.  The booleans below are named assumptions at
that adapter boundary; there is no caller-asserted or Registry-first bypass.
The pin's five fields are not assumptions: they are the values the adapter
read out of the release record and out of the live ProgramData account, and
every equality between them is decided here.
-/

namespace DClutch.ProtocolInfrastructure

abbrev Identity := ExecutionRelease.Identity

/-- One ProgramData deployment slot as recorded by Loader V3. -/
abbrev Slot := Nat

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

/-! ## The slot pin

The single admission argument every infrastructure artifact reader shares,
modelling `dclutch_registry_contract::require_slot_pinned_release_v1`,
`slot_pinned_release_elf_digest_v1`, and `ArtifactReleaseV1::slot_pin_refusal`.
-/

/-- The upgrade policy an `ArtifactRelease` record admits. -/
inductive UpgradePolicy where
  /-- The deployment retained no upgrade authority and can never move. -/
  | immutable
  /-- The deployment can move only under one exact named upgrade authority. -/
  | exactAuthority
  deriving DecidableEq, Repr

/-- The five Loader V3 facts the pin compares.

`bound*` are read out of the finalized, content-addressed `ArtifactRelease`
record that activation hashed the complete ELF against.  `observed*` are read
out of the live ProgramData account in this very invocation.  Passing a
release's own bound values back in as the observation would make every
equality below vacuous, which is exactly why the adapter, not this module,
owns the parse. -/
structure SlotPin where
  boundUpgradePolicy : UpgradePolicy
  boundUpgradeAuthority : Option Identity
  boundDeploymentSlot : Slot
  observedUpgradeAuthority : Option Identity
  observedDeploymentSlot : Slot
  deriving DecidableEq, Repr

/-- The two canonical release pairings, and only those.

`Immutable` with no bound authority, or `ExactAuthority` with an exact bound
authority.  The other two pairings are the residue of the refusal formerly
named "mutable release": an `Immutable` record claiming an authority, and an
`ExactAuthority` record naming none, are both incoherent records rather than
substrates. -/
def SlotPin.canonicalReleaseShape (pin : SlotPin) : Bool :=
  match pin.boundUpgradePolicy with
  | .immutable => pin.boundUpgradeAuthority == none
  | .exactAuthority => pin.boundUpgradeAuthority != none

/-- The observation still matches what the release bound. -/
def SlotPin.holds (pin : SlotPin) : Bool :=
  pin.observedDeploymentSlot == pin.boundDeploymentSlot &&
  pin.observedUpgradeAuthority == pin.boundUpgradeAuthority

/-- Complete artifact-deployment admission: canonical record, live pin. -/
def SlotPin.admits (pin : SlotPin) : Bool :=
  pin.canonicalReleaseShape && pin.holds

/-- How a slot disagreement is named to an operator. -/
inductive PinRefusal where
  /-- The named authority upgraded the substrate; re-release and re-found. -/
  | releaseSupersededByUpgrade
  /-- A stale, substituted, or wrong-generation observation. -/
  | deploymentSlotMismatch
  deriving DecidableEq, Repr

/-- Name a slot disagreement, mirroring `ArtifactReleaseV1::slot_pin_refusal`.

Only a strictly *later* observed slot on an `ExactAuthority` release is an
upgrade: Loader V3 refuses an `Upgrade` in ProgramData's own recorded slot and
refuses the `Close` a redeploy would have to precede it with, so the slot can
only move forward.  An `Immutable` release pins a slot nothing can move, so
any disagreement there is a substituted observation. -/
def SlotPin.slotRefusal (pin : SlotPin) : PinRefusal :=
  match pin.boundUpgradePolicy with
  | .exactAuthority =>
      if pin.boundDeploymentSlot < pin.observedDeploymentSlot then
        PinRefusal.releaseSupersededByUpgrade
      else
        PinRefusal.deploymentSlotMismatch
  | .immutable => PinRefusal.deploymentSlotMismatch

/-- The substrate moved forward under its own named authority. -/
def SlotPin.Superseded (pin : SlotPin) : Prop :=
  pin.boundUpgradePolicy = UpgradePolicy.exactAuthority /\
    pin.boundDeploymentSlot < pin.observedDeploymentSlot

/-- The release record pairs a policy with the wrong authority shape. -/
def SlotPin.NoncanonicalPairing (pin : SlotPin) : Prop :=
  (pin.boundUpgradePolicy = UpgradePolicy.immutable /\
    pin.boundUpgradeAuthority ≠ none) \/
  (pin.boundUpgradePolicy = UpgradePolicy.exactAuthority /\
    pin.boundUpgradeAuthority = none)

theorem canonical_release_shape_admits_exactly_two_pairings (pin : SlotPin) :
    pin.canonicalReleaseShape = true <->
      (pin.boundUpgradePolicy = UpgradePolicy.immutable /\
          pin.boundUpgradeAuthority = none) \/
        (pin.boundUpgradePolicy = UpgradePolicy.exactAuthority /\
          pin.boundUpgradeAuthority ≠ none) := by
  cases policy : pin.boundUpgradePolicy <;>
    simp [SlotPin.canonicalReleaseShape, policy]

theorem noncanonical_release_shape_refuses
    (pin : SlotPin) (noncanonical : pin.NoncanonicalPairing) :
    pin.admits = false := by
  unfold SlotPin.NoncanonicalPairing at noncanonical
  rcases noncanonical with ⟨policy, retained⟩ | ⟨policy, revoked⟩
  · simp [SlotPin.admits, SlotPin.canonicalReleaseShape, policy, retained]
  · simp [SlotPin.admits, SlotPin.canonicalReleaseShape, policy, revoked]

/-- Decision 0012's positive direction: for a canonical release the whole
admission is exactly the two live equalities, with no irrevocability clause
left anywhere in it. -/
theorem canonical_pin_admits_iff_observation_matches_release
    (pin : SlotPin) (canonical : pin.canonicalReleaseShape = true) :
    pin.admits = true <->
      (pin.observedDeploymentSlot = pin.boundDeploymentSlot /\
        pin.observedUpgradeAuthority = pin.boundUpgradeAuthority) := by
  simp [SlotPin.admits, SlotPin.holds, canonical]

/-- An upgradeable substrate under its exact bound authority, observed at the
slot its release pinned, is admitted.  This is the statement the pre-0012 tree
proved the negation of. -/
theorem upgradeable_slot_pinned_release_admits
    (pin : SlotPin) (authority : Identity)
    (policy : pin.boundUpgradePolicy = UpgradePolicy.exactAuthority)
    (bound : pin.boundUpgradeAuthority = some authority)
    (observedAuthority : pin.observedUpgradeAuthority = some authority)
    (observedSlot : pin.observedDeploymentSlot = pin.boundDeploymentSlot) :
    pin.admits = true := by
  simp [SlotPin.admits, SlotPin.canonicalReleaseShape, SlotPin.holds,
    policy, bound, observedAuthority, observedSlot]

/-- The full immutable ceremony still admits: 0012 added a mode and retired
nothing. -/
theorem revoked_immutable_release_admits
    (pin : SlotPin)
    (policy : pin.boundUpgradePolicy = UpgradePolicy.immutable)
    (bound : pin.boundUpgradeAuthority = none)
    (observedAuthority : pin.observedUpgradeAuthority = none)
    (observedSlot : pin.observedDeploymentSlot = pin.boundDeploymentSlot) :
    pin.admits = true := by
  simp [SlotPin.admits, SlotPin.canonicalReleaseShape, SlotPin.holds,
    policy, bound, observedAuthority, observedSlot]

theorem moved_slot_refuses
    (pin : SlotPin)
    (moved : pin.observedDeploymentSlot ≠ pin.boundDeploymentSlot) :
    pin.admits = false := by
  simp [SlotPin.admits, SlotPin.holds, moved]

theorem substituted_upgrade_authority_refuses
    (pin : SlotPin)
    (substituted : pin.observedUpgradeAuthority ≠ pin.boundUpgradeAuthority) :
    pin.admits = false := by
  simp [SlotPin.admits, SlotPin.holds, substituted]

/-- A release claiming `Immutable` over a ProgramData that currently carries an
upgrade authority is refused, whichever way the record was built: with a bound
authority it is a non-canonical pairing, without one the live observation
breaks the pin. -/
theorem retained_authority_over_immutable_release_refuses
    (pin : SlotPin)
    (policy : pin.boundUpgradePolicy = UpgradePolicy.immutable)
    (retained : pin.observedUpgradeAuthority ≠ none) :
    pin.admits = false := by
  cases bound : pin.boundUpgradeAuthority with
  | none =>
      have substituted : pin.observedUpgradeAuthority ≠ pin.boundUpgradeAuthority := by
        rw [bound]
        exact retained
      exact substituted_upgrade_authority_refuses pin substituted
  | some key =>
      simp [SlotPin.admits, SlotPin.canonicalReleaseShape, policy, bound]

/-- An upgrade of the substrate refuses, and is named as an upgrade. -/
theorem superseded_upgradeable_release_refuses
    (pin : SlotPin) (superseded : pin.Superseded) :
    pin.admits = false /\
      pin.slotRefusal = PinRefusal.releaseSupersededByUpgrade := by
  unfold SlotPin.Superseded at superseded
  obtain ⟨policy, moved⟩ := superseded
  refine ⟨?_, ?_⟩
  · have slot : pin.observedDeploymentSlot ≠ pin.boundDeploymentSlot :=
      Nat.ne_of_gt moved
    exact moved_slot_refuses pin slot
  · simp [SlotPin.slotRefusal, policy, moved]

/-- The converse, and the reason the name carries information: supersession is
claimed only for forward movement on a release that named an authority.  An
earlier observed slot, or any `Immutable` disagreement, is a plain mismatch —
so `ReleaseSupersededByUpgrade` is a statement about the substrate moving, not
a restatement of slot inequality. -/
theorem supersession_names_only_forward_movement
    (pin : SlotPin)
    (superseded : pin.slotRefusal = PinRefusal.releaseSupersededByUpgrade) :
    pin.Superseded := by
  unfold SlotPin.Superseded
  cases policy : pin.boundUpgradePolicy with
  | immutable =>
      simp [SlotPin.slotRefusal, policy] at superseded
  | exactAuthority =>
      simp only [SlotPin.slotRefusal, policy] at superseded
      by_cases moved : pin.boundDeploymentSlot < pin.observedDeploymentSlot
      · exact ⟨rfl, moved⟩
      · simp [moved] at superseded

/-- A stale or replayed observation of an upgradeable substrate is never
reported as an upgrade. -/
theorem stale_upgradeable_observation_is_not_supersession
    (pin : SlotPin)
    (stale : pin.observedDeploymentSlot ≤ pin.boundDeploymentSlot) :
    pin.slotRefusal = PinRefusal.deploymentSlotMismatch := by
  by_cases superseded : pin.slotRefusal = PinRefusal.releaseSupersededByUpgrade
  · have moved := (supersession_names_only_forward_movement pin superseded).2
    exact absurd stale (Nat.not_le.mpr moved)
  · cases refusal : pin.slotRefusal with
    | releaseSupersededByUpgrade => exact absurd refusal superseded
    | deploymentSlotMismatch => rfl

/-! ## Decision-case corpus

The rule above is mirrored by hand in `core-sbf/src/infrastructure.rs` and in
`dclutch-registry-contract`, and nothing checked one against the other. These
vectors are that check: every decision case the theorems name, emitted, and
replayed byte-exact through the adapter. They are the bridge, not a second
implementation -- the Rust rule stays hand-written and this says what it must
answer.

`PinOutcome` is coarser than `PinRefusal` on purpose, because the adapter is:
Core names only the superseded case distinctly and folds every other refusal
into one operator-facing code. -/

/-- What an operator reads back from the adapter. -/
inductive PinOutcome where
  /-- The pin holds and the deployment is admitted. -/
  | admit
  /-- The named authority moved the substrate forward; re-release. -/
  | refuseSuperseded
  /-- Every other disagreement, folded as the adapter folds it. -/
  | refuseInfrastructure
  deriving DecidableEq, Repr

def PinOutcome.tag : PinOutcome → Nat
  | .admit => 0
  | .refuseSuperseded => 1
  | .refuseInfrastructure => 2

def PinOutcome.rustName : PinOutcome → String
  | .admit => "Admit"
  | .refuseSuperseded => "RefuseSuperseded"
  | .refuseInfrastructure => "RefuseInfrastructure"

/-- The outcome in the adapter's own conjunct order.

A non-canonical record never reaches the slot comparison -- Rust refuses it at
the release constructor and again at `require_slot_pinned_release_v1` -- so the
superseded name is guarded by the canonical shape. The slot is compared BEFORE
the authority, which is why a release that was both upgraded and re-keyed reads
as superseded rather than as an authority substitution. -/
def SlotPin.outcome (pin : SlotPin) : PinOutcome :=
  match pin.admits with
  | true => PinOutcome.admit
  | false =>
      if pin.canonicalReleaseShape
          && pin.observedDeploymentSlot != pin.boundDeploymentSlot
          && pin.slotRefusal == PinRefusal.releaseSupersededByUpgrade then
        PinOutcome.refuseSuperseded
      else
        PinOutcome.refuseInfrastructure

/-- One named decision case. -/
structure PinVector where
  name : String
  pin : SlotPin

private def authority : Identity := 161
private def substitute : Identity := 178
private def boundSlot : Slot := 7

/-- Every decision case the theorems above name, once each. -/
def pinVectors : List PinVector := [
  { name := "revoked_immutable_release_admits",
    pin := ⟨.immutable, none, boundSlot, none, boundSlot⟩ },
  { name := "upgradeable_slot_pinned_release_admits",
    pin := ⟨.exactAuthority, some authority, boundSlot, some authority, boundSlot⟩ },
  { name := "superseded_upgradeable_release_refuses",
    pin := ⟨.exactAuthority, some authority, boundSlot, some authority, boundSlot + 1⟩ },
  { name := "stale_upgradeable_observation_is_not_supersession",
    pin := ⟨.exactAuthority, some authority, boundSlot, some authority, boundSlot - 1⟩ },
  { name := "moved_slot_refuses_on_immutable_release",
    pin := ⟨.immutable, none, boundSlot, none, boundSlot + 1⟩ },
  { name := "substituted_upgrade_authority_refuses",
    pin := ⟨.exactAuthority, some authority, boundSlot, some substitute, boundSlot⟩ },
  { name := "retained_authority_over_immutable_release_refuses",
    pin := ⟨.immutable, none, boundSlot, some authority, boundSlot⟩ },
  { name := "supersession_is_named_before_authority_substitution",
    pin := ⟨.exactAuthority, some authority, boundSlot, some substitute, boundSlot + 2⟩ },
  { name := "noncanonical_immutable_release_naming_an_authority_refuses",
    pin := ⟨.immutable, some authority, boundSlot, none, boundSlot⟩ },
  { name := "noncanonical_exact_authority_release_naming_none_refuses",
    pin := ⟨.exactAuthority, none, boundSlot, none, boundSlot⟩ }
]

/-! ### Corpus theorems -/

/-- The operator-facing outcome agrees with the admission rule exactly.  This
is what lets a three-valued corpus stand in for the two-valued rule. -/
theorem outcome_is_admit_iff_pin_admits (pin : SlotPin) :
    pin.outcome = PinOutcome.admit <-> pin.admits = true := by
  unfold SlotPin.outcome
  cases hadmits : pin.admits with
  | true => simp
  | false => simp; split <;> simp

/-- The corpus decides every outcome, so a vector list that answers one way
throughout cannot satisfy it. -/
theorem pin_vectors_cover_every_outcome :
    pinVectors.any (fun vector => vector.pin.outcome == PinOutcome.admit) &&
      pinVectors.any (fun vector =>
        vector.pin.outcome == PinOutcome.refuseSuperseded) &&
      pinVectors.any (fun vector =>
        vector.pin.outcome == PinOutcome.refuseInfrastructure) := by
  native_decide

/-- Both canonical pairings are exercised on the admitting side, so "admit"
is not one shape's accident. -/
theorem pin_vectors_admit_both_canonical_pairings :
    pinVectors.any (fun vector =>
        vector.pin.outcome == PinOutcome.admit &&
          vector.pin.boundUpgradePolicy == UpgradePolicy.immutable) &&
      pinVectors.any (fun vector =>
        vector.pin.outcome == PinOutcome.admit &&
          vector.pin.boundUpgradePolicy == UpgradePolicy.exactAuthority) := by
  native_decide

/-- The exact expected outcome of every vector, pinned in order. -/
theorem pin_vector_outcomes_are_exact :
    pinVectors.map (fun vector => vector.pin.outcome.tag) =
      [0, 0, 1, 2, 2, 2, 2, 1, 2, 2] := by
  native_decide

theorem pin_vector_count_is_exact : pinVectors.length = 10 := by
  native_decide

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

/-- Exact finalized ArtifactRelease and live deployment seen during init.

First admission hashes the complete deployed ELF rather than reusing the
record's claimed digest; the record's admission gate is otherwise the same
slot pin every later read applies. -/
structure InitArtifactObservation where
  binding : Binding
  finalizedArtifactRecordAuthenticated : Bool
  currentDeploymentAuthenticated : Bool
  pin : SlotPin
  deriving DecidableEq, Repr

def InitArtifactObservation.accepts
    (expected : Binding) (observation : InitArtifactObservation) : Bool :=
  expected.valid && observation.binding == expected &&
  observation.finalizedArtifactRecordAuthenticated &&
  observation.currentDeploymentAuthenticated &&
  observation.pin.admits

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

theorem unpinned_init_artifact_refuses
    (expected : Binding) (observation : InitArtifactObservation)
    (unpinned : observation.pin.admits = false) :
    observation.accepts expected = false := by
  simp [InitArtifactObservation.accepts, unpinned]

/-- Every pin refusal reaches initialization, whichever infrastructure record
carried it. -/
theorem unpinned_init_registry_or_rent_refuses
    (observation : InitializationObservation)
    (unpinned : observation.registryArtifact.pin.admits = false \/
      observation.rentArtifact.pin.admits = false) :
    initializationAccepts observation = false := by
  rcases unpinned with registry | rent
  · have refused :
        observation.registryArtifact.accepts observation.profile.registry = false :=
      unpinned_init_artifact_refuses observation.profile.registry
        observation.registryArtifact registry
    simp [initializationAccepts, refused]
  · have refused : observation.rentArtifact.accepts observation.profile.rent = false :=
      unpinned_init_artifact_refuses observation.profile.rent
        observation.rentArtifact rent
    simp [initializationAccepts, refused]

theorem superseded_init_registry_or_rent_refuses
    (observation : InitializationObservation)
    (superseded : observation.registryArtifact.pin.Superseded \/
      observation.rentArtifact.pin.Superseded) :
    initializationAccepts observation = false := by
  apply unpinned_init_registry_or_rent_refuses
  rcases superseded with registry | rent
  · exact Or.inl (superseded_upgradeable_release_refuses _ registry).1
  · exact Or.inr (superseded_upgradeable_release_refuses _ rent).1

theorem noncanonical_init_registry_or_rent_release_refuses
    (observation : InitializationObservation)
    (noncanonical : observation.registryArtifact.pin.NoncanonicalPairing \/
      observation.rentArtifact.pin.NoncanonicalPairing) :
    initializationAccepts observation = false := by
  apply unpinned_init_registry_or_rent_refuses
  rcases noncanonical with registry | rent
  · exact Or.inl (noncanonical_release_shape_refuses _ registry)
  · exact Or.inr (noncanonical_release_shape_refuses _ rent)

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
for this stage.  The deployment is admitted by its slot pin, so an iterated
substrate under a named authority is a first-class admitted shape. -/
structure SlotPinnedArtifactObservation where
  binding : Binding
  artifactContentAuthenticated : Bool
  currentDeploymentAuthenticated : Bool
  pin : SlotPin
  deriving DecidableEq, Repr

def SlotPinnedArtifactObservation.accepts
    (expected : Binding) (observation : SlotPinnedArtifactObservation) : Bool :=
  expected.valid && observation.binding == expected &&
  observation.artifactContentAuthenticated &&
  observation.currentDeploymentAuthenticated &&
  observation.pin.admits

theorem substituted_slot_pinned_artifact_refuses
    (expected : Binding) (observation : SlotPinnedArtifactObservation)
    (substituted : observation.binding ≠ expected) :
    observation.accepts expected = false := by
  simp [SlotPinnedArtifactObservation.accepts, substituted]

theorem unpinned_artifact_refuses
    (expected : Binding) (observation : SlotPinnedArtifactObservation)
    (unpinned : observation.pin.admits = false) :
    observation.accepts expected = false := by
  simp [SlotPinnedArtifactObservation.accepts, unpinned]

/-- With identity and adapter facts settled, artifact admission is exactly the
slot pin — there is no remaining immutability requirement to satisfy. -/
theorem artifact_accepts_iff_pin_admits
    (expected : Binding) (observation : SlotPinnedArtifactObservation)
    (valid : expected.valid = true)
    (bound : observation.binding = expected)
    (content : observation.artifactContentAuthenticated = true)
    (deployment : observation.currentDeploymentAuthenticated = true) :
    observation.accepts expected = true <-> observation.pin.admits = true := by
  simp [SlotPinnedArtifactObservation.accepts, valid, bound, content, deployment]

/-- Result of the second Found stage.  Constructing this value requires the
Core, Registry, and Rent current deployments to match slot-pinned artifacts. -/
structure AuthenticatedInfrastructure where
  coreProgram : Identity
  profile : Profile
  coreArtifact : Binding
  deriving DecidableEq, Repr

/-- Authenticate all three slot-pinned artifacts only after profile admission. -/
def authenticateSlotPinnedInfrastructure
    (profile : AuthenticatedProfile)
    (selectedCore : Binding)
    (coreArtifact registryArtifact rentArtifact : SlotPinnedArtifactObservation) :
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
    (coreArtifact registryArtifact rentArtifact : SlotPinnedArtifactObservation)
    (authenticated : AuthenticatedInfrastructure)
    (accepted : authenticateSlotPinnedInfrastructure profile selectedCore coreArtifact
      registryArtifact rentArtifact = some authenticated) :
    authenticated.coreProgram = profile.coreProgram /\
      authenticated.profile = profile.profile /\
      authenticated.coreArtifact = selectedCore := by
  unfold authenticateSlotPinnedInfrastructure at accepted
  split at accepted
  · cases accepted
    exact ⟨rfl, rfl, rfl⟩
  · cases accepted

/-- Registry/Rent-owned facts consumed only after exact slot-pinned
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
  coreArtifact : SlotPinnedArtifactObservation
  registryArtifact : SlotPinnedArtifactObservation
  rentArtifact : SlotPinnedArtifactObservation
  downstream : DownstreamFoundObservation
  deriving DecidableEq, Repr

/-- Ordered Found admission: profile first, then direct slot-pinned artifact
authentication, then and only then Registry-owned records/cache and RentCredit. -/
def foundAccepts (observation : FoundObservation) : Bool :=
  match authenticateProfile observation.profileAccount with
  | none => false
  | some profile =>
      match authenticateSlotPinnedInfrastructure profile observation.selectedCore
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
          authenticateSlotPinnedInfrastructure profile observation.selectedCore
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
        apply substituted_slot_pinned_artifact_refuses
        simpa [profileExact.2] using substituted
      unfold authenticateSlotPinnedInfrastructure
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
        apply substituted_slot_pinned_artifact_refuses
        simpa [profileExact.2] using substituted
      unfold authenticateSlotPinnedInfrastructure
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
          authenticateSlotPinnedInfrastructure profile observation.selectedCore
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
          authenticateSlotPinnedInfrastructure profile observation.selectedCore
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

/-- Every pin refusal reaches Found, whichever of the three infrastructure
artifacts carried it.  This is the lift the pre-0012 tree performed on the
immutability requirement; it now carries the slot pin instead. -/
theorem unpinned_core_registry_or_rent_refuses
    (observation : FoundObservation)
    (unpinned : observation.coreArtifact.pin.admits = false \/
      observation.registryArtifact.pin.admits = false \/
      observation.rentArtifact.pin.admits = false) :
    foundAccepts observation = false := by
  unfold foundAccepts
  generalize profileEquation : authenticateProfile observation.profileAccount = profileResult
  cases profileResult with
  | none => rfl
  | some profile =>
      rcases unpinned with core | registry | rent
      · have refused : observation.coreArtifact.accepts observation.selectedCore = false :=
          unpinned_artifact_refuses observation.selectedCore observation.coreArtifact core
        unfold authenticateSlotPinnedInfrastructure
        simp [refused]
      · have refused : observation.registryArtifact.accepts profile.profile.registry = false :=
          unpinned_artifact_refuses profile.profile.registry observation.registryArtifact registry
        unfold authenticateSlotPinnedInfrastructure
        simp [refused]
      · have refused : observation.rentArtifact.accepts profile.profile.rent = false :=
          unpinned_artifact_refuses profile.profile.rent observation.rentArtifact rent
        unfold authenticateSlotPinnedInfrastructure
        simp [refused]

/-- Upgrading Core, Registry, or Rent refuses every subsequent Found, and the
upgraded artifact names the upgrade rather than a mystery mismatch. -/
theorem superseded_core_registry_or_rent_refuses
    (observation : FoundObservation)
    (superseded : observation.coreArtifact.pin.Superseded \/
      observation.registryArtifact.pin.Superseded \/
      observation.rentArtifact.pin.Superseded) :
    foundAccepts observation = false := by
  apply unpinned_core_registry_or_rent_refuses
  rcases superseded with core | registry | rent
  · exact Or.inl (superseded_upgradeable_release_refuses _ core).1
  · exact Or.inr (Or.inl (superseded_upgradeable_release_refuses _ registry).1)
  · exact Or.inr (Or.inr (superseded_upgradeable_release_refuses _ rent).1)

theorem superseded_found_registry_names_the_upgrade
    (observation : FoundObservation)
    (superseded : observation.registryArtifact.pin.Superseded) :
    foundAccepts observation = false /\
      observation.registryArtifact.pin.slotRefusal =
        PinRefusal.releaseSupersededByUpgrade := by
  refine ⟨?_, (superseded_upgradeable_release_refuses _ superseded).2⟩
  exact superseded_core_registry_or_rent_refuses observation (Or.inr (Or.inl superseded))

/-- The one thing the former "mutable release" refusal still means: a record
pairing `Immutable` with a bound authority, or `ExactAuthority` with none, is
refused at Found. -/
theorem noncanonical_core_registry_or_rent_release_refuses
    (observation : FoundObservation)
    (noncanonical : observation.coreArtifact.pin.NoncanonicalPairing \/
      observation.registryArtifact.pin.NoncanonicalPairing \/
      observation.rentArtifact.pin.NoncanonicalPairing) :
    foundAccepts observation = false := by
  apply unpinned_core_registry_or_rent_refuses
  rcases noncanonical with core | registry | rent
  · exact Or.inl (noncanonical_release_shape_refuses _ core)
  · exact Or.inr (Or.inl (noncanonical_release_shape_refuses _ registry))
  · exact Or.inr (Or.inr (noncanonical_release_shape_refuses _ rent))

/-- Decision 0012's "what stays refused": an `Immutable` release over a
ProgramData that still carries an upgrade authority. -/
theorem retained_authority_over_immutable_core_registry_or_rent_refuses
    (observation : FoundObservation)
    (retained :
      (observation.coreArtifact.pin.boundUpgradePolicy = UpgradePolicy.immutable /\
        observation.coreArtifact.pin.observedUpgradeAuthority ≠ none) \/
      (observation.registryArtifact.pin.boundUpgradePolicy = UpgradePolicy.immutable /\
        observation.registryArtifact.pin.observedUpgradeAuthority ≠ none) \/
      (observation.rentArtifact.pin.boundUpgradePolicy = UpgradePolicy.immutable /\
        observation.rentArtifact.pin.observedUpgradeAuthority ≠ none)) :
    foundAccepts observation = false := by
  apply unpinned_core_registry_or_rent_refuses
  rcases retained with ⟨policy, live⟩ | ⟨policy, live⟩ | ⟨policy, live⟩
  · exact Or.inl (retained_authority_over_immutable_release_refuses _ policy live)
  · exact Or.inr (Or.inl (retained_authority_over_immutable_release_refuses _ policy live))
  · exact Or.inr (Or.inr (retained_authority_over_immutable_release_refuses _ policy live))

/-- Found admits on the strength of the slot pin alone.  No conjunct anywhere
on this path asks whether a substrate is irrevocable, which is decision 0012's
positive claim lifted to the three-artifact stage. -/
theorem found_admits_pin_holding_infrastructure
    (observation : FoundObservation) (profile : AuthenticatedProfile)
    (profileAccepted : authenticateProfile observation.profileAccount = some profile)
    (selected : observation.selectedCore.program = profile.coreProgram)
    (core : observation.coreArtifact.accepts observation.selectedCore = true)
    (registry : observation.registryArtifact.accepts profile.profile.registry = true)
    (rent : observation.rentArtifact.accepts profile.profile.rent = true)
    (downstream : downstreamFoundAccepts
      { coreProgram := profile.coreProgram, profile := profile.profile,
        coreArtifact := observation.selectedCore } observation.downstream = true) :
    foundAccepts observation = true := by
  unfold foundAccepts
  rw [profileAccepted]
  unfold authenticateSlotPinnedInfrastructure
  simp only [selected, core, registry, rent, beq_self_eq_true, Bool.and_self, if_pos]
  simpa using downstream

/-! ## Executable theorem regressions -/

namespace Examples

def coreBinding : Binding := ⟨1, 2⟩
def registryBinding : Binding := ⟨3, 4⟩
def rentBinding : Binding := ⟨5, 6⟩
def profile : Profile := ⟨registryBinding, rentBinding⟩

/-- Slots taken from the runbook's own local measurement: a deploy landed at
slot 167 and its redeploy at slot 531. -/
def pinnedSlot : Slot := 167

def upgradedSlot : Slot := 531

/-- The full immutable ceremony: no authority bound, none observed. -/
def revokedPin : SlotPin := {
  boundUpgradePolicy := .immutable
  boundUpgradeAuthority := none
  boundDeploymentSlot := pinnedSlot
  observedUpgradeAuthority := none
  observedDeploymentSlot := pinnedSlot
}

/-- The devnet iteration substrate: one named authority, pin intact. -/
def slotPinnedUpgradeablePin : SlotPin := {
  boundUpgradePolicy := .exactAuthority
  boundUpgradeAuthority := some 14
  boundDeploymentSlot := pinnedSlot
  observedUpgradeAuthority := some 14
  observedDeploymentSlot := pinnedSlot
}

def supersededPin : SlotPin :=
  { slotPinnedUpgradeablePin with observedDeploymentSlot := upgradedSlot }

def stalePin : SlotPin := { slotPinnedUpgradeablePin with observedDeploymentSlot := 12 }

def substitutedAuthorityPin : SlotPin :=
  { slotPinnedUpgradeablePin with observedUpgradeAuthority := some 15 }

def retainedAuthorityPin : SlotPin :=
  { revokedPin with observedUpgradeAuthority := some 14 }

/-- Non-canonical: `Immutable` claiming a bound authority. -/
def immutableWithAuthorityPin : SlotPin :=
  { revokedPin with
    boundUpgradeAuthority := some 14
    observedUpgradeAuthority := some 14 }

/-- Non-canonical: `ExactAuthority` naming no authority at all. -/
def upgradeableWithoutAuthorityPin : SlotPin :=
  { slotPinnedUpgradeablePin with
    boundUpgradeAuthority := none
    observedUpgradeAuthority := none }

def initArtifact (binding : Binding) (pin : SlotPin) : InitArtifactObservation := {
  binding
  finalizedArtifactRecordAuthenticated := true
  currentDeploymentAuthenticated := true
  pin
}

def initWith (registryPin rentPin : SlotPin) : InitializationObservation := {
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
  registryArtifact := initArtifact registryBinding registryPin
  rentArtifact := initArtifact rentBinding rentPin
}

def init : InitializationObservation := initWith revokedPin revokedPin

def artifact (binding : Binding) (pin : SlotPin) : SlotPinnedArtifactObservation := {
  binding
  artifactContentAuthenticated := true
  currentDeploymentAuthenticated := true
  pin
}

def foundWith (corePin registryPin rentPin : SlotPin) : FoundObservation := {
  profileAccount := {
    currentCoreProgram := 1
    address := 9
    expectedPerCorePda := 9
    owner := 1
    profile
    exactCanonicalBytesAuthenticated := true
  }
  selectedCore := coreBinding
  coreArtifact := artifact coreBinding corePin
  registryArtifact := artifact registryBinding registryPin
  rentArtifact := artifact rentBinding rentPin
  downstream := {
    marketRegistryProgram := 3
    registryFinalizedRecordsAuthenticated := true
    registryActivationCacheAuthenticated := true
    rentCreditOwner := 5
    rentCreditPdaAuthenticated := true
  }
}

def found : FoundObservation := foundWith revokedPin revokedPin revokedPin

theorem valid_initialization_and_found_accept :
    initializationAccepts init = true /\ foundAccepts found = true := by
  native_decide

/-- Decision 0012 in one executable fact: a wholly mutable substrate, every
role retaining its named upgrade authority, initializes and founds. -/
theorem slot_pinned_upgradeable_infrastructure_accepts :
    initializationAccepts
        (initWith slotPinnedUpgradeablePin slotPinnedUpgradeablePin) = true /\
      foundAccepts
        (foundWith slotPinnedUpgradeablePin slotPinnedUpgradeablePin
          slotPinnedUpgradeablePin) = true := by
  native_decide

theorem same_width_registry_and_rent_substitutions_refuse :
    foundAccepts {
      found with registryArtifact := artifact ⟨10, 11⟩ revokedPin
    } = false /\
    foundAccepts {
      found with rentArtifact := artifact ⟨12, 13⟩ revokedPin
    } = false := by
  native_decide

/-- The upgrade the substrate is allowed to perform refuses every open Found,
under both admitted policies and at both stages. -/
theorem upgraded_or_stale_substrate_refuses :
    initializationAccepts (initWith supersededPin revokedPin) = false /\
    foundAccepts (foundWith revokedPin supersededPin revokedPin) = false /\
    foundAccepts (foundWith revokedPin revokedPin stalePin) = false := by
  native_decide

/-- The refusal is named as an upgrade only when the slot actually moved
forward; a stale observation of the same release is a plain mismatch. -/
theorem supersession_is_named_only_for_forward_movement :
    supersededPin.slotRefusal = PinRefusal.releaseSupersededByUpgrade /\
    stalePin.slotRefusal = PinRefusal.deploymentSlotMismatch /\
    retainedAuthorityPin.slotRefusal = PinRefusal.deploymentSlotMismatch := by
  native_decide

/-- A substituted upgrade authority, and a retained authority under a release
claiming `Immutable`, both still refuse. -/
theorem substituted_or_retained_authority_refuses :
    foundAccepts (foundWith revokedPin substitutedAuthorityPin revokedPin) = false /\
    foundAccepts (foundWith revokedPin revokedPin retainedAuthorityPin) = false /\
    initializationAccepts (initWith retainedAuthorityPin revokedPin) = false := by
  native_decide

/-- Both non-canonical pairings refuse: this is all `MutableRegistryRelease`
still means after decision 0012. -/
theorem noncanonical_release_pairings_refuse :
    foundAccepts (foundWith immutableWithAuthorityPin revokedPin revokedPin) = false /\
    foundAccepts
        (foundWith revokedPin upgradeableWithoutAuthorityPin revokedPin) = false /\
    initializationAccepts
        (initWith revokedPin upgradeableWithoutAuthorityPin) = false := by
  native_decide

end Examples

end DClutch.ProtocolInfrastructure
