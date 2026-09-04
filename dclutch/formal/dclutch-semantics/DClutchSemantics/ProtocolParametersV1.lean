import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec

/-!
# The protocol parameters record

One record holding every economic knob the protocol has, so that changing one is
a governed act with a delay and a receipt rather than a source constant that
rides whichever ELF happens to carry it.

## Why the record exists

Every value here lives today as a literal in a crate: the fee band ceiling in
`dclutch-direct-codec`, the closer's reward in `close_maker_v1.rs`, the crank
reward in `claim_check_v1.rs`, and the protocol take and beneficiary nowhere at
all, because there are none.  A literal is a fine way to hold a value nobody
intends to move.  It is a bad way to hold a value everybody knows will move
once, at mainnet, because the only procedure it admits is "ship a new program",
which is also the procedure for changing what the program MEANS -- so the two
kinds of change become indistinguishable to anyone reading a release.

So: one record, one authority, one change procedure with a wait, and a
generation number a census can read.  What this buys is not flexibility.  It is
that the fee cap moving from 500 to 300 is an event with a slot number, a
proposer and seven days of notice, instead of a diff.

## What governance can and cannot do

The bands below are the constitution and the record is the statute.  A band is a
source constant: moving it needs a new ELF, a release, and everything that
attends one.  A parameter is a record field: moving it needs the authority, a
proposal, and the delay.

The sharp case is the fee ceiling.  `absoluteFeeCeilingBasisPoints` is decision
0014 D2's 500 and governance may set `maxFeeBasisPoints` anywhere at or below
it.  **Governance can narrow the fee band and can never widen it past what the
deployed release already allows.**  A holder who read the ELF knows the worst
case without reading the record.

The second is the pair rule.  `protocolTakeBasisPoints` is zero exactly when
`protocolBeneficiary` is the zero key, in both directions.  A take with no payee
and a payee with no take are equally unrepresentable, so ruling D1's *"no
protocol fee take before mainnet; no protocol beneficiary"* is one fact rather
than two that could drift apart.

The third is the freeze.  A zero `governanceAuthority` is legal and it means
NOBODY MAY PROPOSE.  It is the one-way door: a deployment that wants immutable
economics sets the authority to zero and the record is finished forever.  That
is deliberately not reversible, because a reversible freeze is not a freeze.
-/

namespace DClutch.ProtocolParametersV1

open DClutch DClutch.AbiSchema

def abiVersion : Nat := 1

/-- `DCLTPRM1`. -/
def recordMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x50, 0x52, 0x4d, 0x31]

/-- `DCLTPRC1` -- the change receipt a census reads. -/
def receiptMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x50, 0x52, 0x43, 0x31]

/-- `[domain, generation]` -- the one record, per parameter generation. -/
def recordPdaDomain : String := "dclutch:protocol-parameters:v1"

/-!
## The constitution: four source constants a release owns
-/

/-- Basis-point denominator, shared with the Direct fee geometry. -/
def basisPointDenominator : Nat := 10000

/-- The fee band ceiling governance may never exceed.

Decision 0014 D2's `DIRECT_MAX_FEE_BASIS_POINTS_V1`.  It was already the one
enforced band in the tree; here it stops being the value and becomes the BOUND
ON the value, which is the only change of meaning this record makes to an
existing constant. -/
def absoluteFeeCeilingBasisPoints : Nat := 500

/-- Slots the protocol treats as one day.

Not invented here.  `COMPACTION_DEADLINE_SLOTS_V1` is 38,880,000 and the
comment calling it a hundred-and-eighty-day wait is only true at this rate, so
the rate is already load-bearing somewhere it is not written down.  Writing it
down is what lets a second constant derive from it instead of guessing again. -/
def slotsPerNominalDay : Nat := 216000

/-- The compaction deadline, restated in the unit above, so that the derivation
is checked rather than asserted in a comment. -/
def compactionDeadlineSlots : Nat := 38880000

theorem compaction_deadline_is_one_hundred_eighty_nominal_days :
    compactionDeadlineSlots = 180 * slotsPerNominalDay := by decide

/-- The floor on a parameter change's notice period: seven nominal days.

Long enough that every party holding a position when a fee cap moves sees the
move before it binds; short enough that it is a waiting period and not a veto.
Governance sets `changeDelaySlots` at or above this and can never set it below,
so it cannot make itself instantaneous by governing its own delay first. -/
def minimumChangeDelaySlots : Nat := 7 * slotsPerNominalDay

/-!
## The parameters

The two identity fields are modelled as "is the zero key" booleans, because the
zero/nonzero distinction is the ONLY thing any band rule below says about them.
Which key an authority is, is an authentication question the adapter answers
against a signer; it is not a question this record can be wrong about.
-/

structure Parameters where
  /-- True when no key may propose: the record is frozen forever. -/
  governanceAuthorityIsZero : Bool
  /-- True when there is no protocol beneficiary.  True today, by ruling. -/
  protocolBeneficiaryIsZero : Bool
  /-- Bumps by exactly one on every applied change; the census's clock. -/
  generation : Nat
  /-- The slot at which THIS value became effective. -/
  activationSlot : Nat
  /-- Proposal-to-apply wait, at or above `minimumChangeDelaySlots`. -/
  changeDelaySlots : Nat
  /-- The effective fee band a market's rate is checked against. -/
  maxFeeBasisPoints : Nat
  /-- The protocol's own cut of a fill.  Zero, by ruling, until mainnet. -/
  protocolTakeBasisPoints : Nat
  /-- Share of a close's donation slice the permissionless closer may carve. -/
  closerCarveBasisPoints : Nat
  /-- Lamport ceiling on that carve, whatever the share computes to. -/
  closerRewardCapLamports : Nat
  /-- Lamport ceiling on one compaction crank's reward. -/
  crankRewardCapLamports : Nat
  deriving DecidableEq, Repr

/-- Every band, as one decidable predicate.

Written right-nested on purpose: a reader extracting one conjunct from a proof
of this needs the shape to be stable, and a flat `&&` chain's association is a
detail of the notation rather than of the statement. -/
def inBand (p : Parameters) : Bool :=
  (p.maxFeeBasisPoints <= absoluteFeeCeilingBasisPoints) &&
  ((p.protocolTakeBasisPoints <= p.maxFeeBasisPoints) &&
  (((p.protocolTakeBasisPoints == 0) == p.protocolBeneficiaryIsZero) &&
  ((p.closerCarveBasisPoints <= basisPointDenominator) &&
   (minimumChangeDelaySlots <= p.changeDelaySlots))))

/-- The bands read nothing but the five governed values, so applying a change
cannot make a value fall out of band by advancing the bookkeeping. -/
theorem inBand_ignores_bookkeeping (p : Parameters) (generation activationSlot : Nat) :
    inBand { p with generation, activationSlot } = inBand p := rfl

theorem inBand_fee_ceiling {p : Parameters} (banded : inBand p = true) :
    p.maxFeeBasisPoints <= absoluteFeeCeilingBasisPoints := by
  simp only [inBand, Bool.and_eq_true, decide_eq_true_eq] at banded
  exact banded.1

theorem inBand_take_within_the_band {p : Parameters} (banded : inBand p = true) :
    p.protocolTakeBasisPoints <= p.maxFeeBasisPoints := by
  simp only [inBand, Bool.and_eq_true, decide_eq_true_eq] at banded
  exact banded.2.1

theorem inBand_take_iff_beneficiary {p : Parameters} (banded : inBand p = true) :
    (p.protocolTakeBasisPoints == 0) = p.protocolBeneficiaryIsZero := by
  simp only [inBand, Bool.and_eq_true, decide_eq_true_eq, beq_iff_eq] at banded
  exact banded.2.2.1

theorem inBand_carve_is_a_share {p : Parameters} (banded : inBand p = true) :
    p.closerCarveBasisPoints <= basisPointDenominator := by
  simp only [inBand, Bool.and_eq_true, decide_eq_true_eq] at banded
  exact banded.2.2.2.1

theorem inBand_delay_floor {p : Parameters} (banded : inBand p = true) :
    minimumChangeDelaySlots <= p.changeDelaySlots := by
  simp only [inBand, Bool.and_eq_true, decide_eq_true_eq] at banded
  exact banded.2.2.2.2

/-- The values the record is born holding: today's deployed economics, exactly.

`maxFeeBasisPoints` is the ceiling itself, because that is what the tree
enforces now and a record that silently tightened a live band at birth would be
a policy change wearing a refactor's clothes.  `closerCarveBasisPoints` is the
whole donation slice and `closerRewardCapLamports` is zero, which is today's
`DIRECT_CLOSE_MAKER_CLOSER_REWARD_V1 = 0` reached by the CAP rather than by the
share -- so the one number that has to move to pay a closer is the cap.
`crankRewardCapLamports` is `COMPACTION_CRANK_REWARD_LAMPORTS_V1`. -/
def genesis : Parameters := {
  governanceAuthorityIsZero := false
  protocolBeneficiaryIsZero := true
  generation := 0
  activationSlot := 0
  changeDelaySlots := minimumChangeDelaySlots
  maxFeeBasisPoints := absoluteFeeCeilingBasisPoints
  protocolTakeBasisPoints := 0
  closerCarveBasisPoints := basisPointDenominator
  closerRewardCapLamports := 0
  crankRewardCapLamports := 200000
}

theorem genesis_is_in_band : inBand genesis = true := by decide

/-- Ruling D1 item 1, as an arithmetic fact about the shipped record. -/
theorem genesis_takes_nothing_and_names_nobody :
    genesis.protocolTakeBasisPoints = 0 /\
      genesis.protocolBeneficiaryIsZero = true := by decide

/-!
## The change procedure

Two acts.  `propose` is the authority's and writes down a commitment plus the
slot at which it may be executed.  `applyChange` is anybody's -- a governed
change is still a crank, and an authority that had to show up twice could
propose a change and then decline to finish it, leaving the record in a state
only it can leave.
-/

/-- A commitment to a proposed value, or none. -/
structure Pending where
  /-- True when no proposal stands. -/
  digestIsZero : Bool
  /-- The first slot at which a standing proposal may be applied. -/
  earliestApplySlot : Nat
  deriving DecidableEq, Repr

structure Record where
  parameters : Parameters
  pending : Pending
  deriving DecidableEq, Repr

inductive Refusal where
  /-- The authority is the zero key: the record is frozen. -/
  | governanceFrozen
  /-- The signer is not the record's authority. -/
  | unauthorizedGovernance
  /-- A proposal already stands; withdraw it or wait it out. -/
  | proposalOutstanding
  /-- The proposed value is outside a constitutional band. -/
  | parameterOutOfBand
  /-- Nothing has been proposed. -/
  | noPendingProposal
  /-- The delay has not elapsed. -/
  | proposalNotMatured
  /-- The bytes offered are not the bytes proposed. -/
  | proposalDigestMismatch
  deriving DecidableEq, Repr

/-- What a governance act returns.  Its own type rather than `Except`, so that
every witness below can be decided against a literal. -/
inductive Outcome where
  | refused (reason : Refusal)
  | changed (record : Record)
  deriving DecidableEq, Repr

/-- `signerIsAuthority` is the adapter's answer, not this record's. -/
def propose (record : Record) (signerIsAuthority : Bool)
    (proposed : Parameters) (currentSlot : Nat) : Outcome :=
  if record.parameters.governanceAuthorityIsZero then .refused .governanceFrozen
  else if !signerIsAuthority then .refused .unauthorizedGovernance
  else if !record.pending.digestIsZero then .refused .proposalOutstanding
  else if !inBand proposed then .refused .parameterOutOfBand
  else .changed { record with
    pending := {
      digestIsZero := false
      earliestApplySlot := currentSlot + record.parameters.changeDelaySlots } }

/-- The authority may take a standing proposal back; nobody else may. -/
def withdraw (record : Record) (signerIsAuthority : Bool) : Outcome :=
  if record.parameters.governanceAuthorityIsZero then .refused .governanceFrozen
  else if !signerIsAuthority then .refused .unauthorizedGovernance
  else if record.pending.digestIsZero then .refused .noPendingProposal
  else .changed { record with pending := { digestIsZero := true, earliestApplySlot := 0 } }

/-- Permissionless.  `digestMatches` is the adapter comparing the offered bytes
against the commitment the proposal pinned.

The band is re-checked HERE and not only at proposal, because a release landing
between the two acts may have narrowed the constitution under a proposal that
was legal when it was made.  A proposal is a commitment to a value, never a
grant of permission to install it. -/
def applyChange (record : Record) (proposed : Parameters) (digestMatches : Bool)
    (currentSlot : Nat) : Outcome :=
  if record.pending.digestIsZero then .refused .noPendingProposal
  else if currentSlot < record.pending.earliestApplySlot then .refused .proposalNotMatured
  else if !digestMatches then .refused .proposalDigestMismatch
  else if !inBand proposed then .refused .parameterOutOfBand
  else .changed {
    parameters := { proposed with
      generation := record.parameters.generation + 1
      activationSlot := currentSlot }
    pending := { digestIsZero := true, earliestApplySlot := 0 } }

/-!
## The laws

Each of the three hostiles the ruling names, plus the five properties that make
the procedure worth having.
-/

/-- HOSTILE 1: an unauthorized change refuses, by name. -/
theorem unauthorized_change_refuses
    (record : Record) (proposed : Parameters) (slot : Nat)
    (live : record.parameters.governanceAuthorityIsZero = false) :
    propose record false proposed slot = .refused .unauthorizedGovernance := by
  simp [propose, live]

/-- HOSTILE 2: a change inside the delay does not apply. -/
theorem a_change_inside_the_delay_does_not_apply
    (record : Record) (proposed : Parameters) (digestMatches : Bool) (slot : Nat)
    (standing : record.pending.digestIsZero = false)
    (early : slot < record.pending.earliestApplySlot) :
    applyChange record proposed digestMatches slot = .refused .proposalNotMatured := by
  simp [applyChange, standing, early]

/-- HOSTILE 3: a parameter outside its band refuses. -/
theorem a_parameter_outside_its_band_refuses
    (record : Record) (proposed : Parameters) (slot : Nat)
    (live : record.parameters.governanceAuthorityIsZero = false)
    (quiet : record.pending.digestIsZero = true)
    (outside : inBand proposed = false) :
    propose record true proposed slot = .refused .parameterOutOfBand := by
  simp [propose, live, quiet, outside]

/-- The safety property the three hostiles serve: whatever the record holds
after a change, it is inside every band.  This is the statement a reader needs;
the refusals are how it is achieved. -/
theorem applied_parameters_are_in_band
    (record after : Record) (proposed : Parameters) (digestMatches : Bool) (slot : Nat)
    (applied : applyChange record proposed digestMatches slot = .changed after) :
    inBand after.parameters = true := by
  unfold applyChange at applied
  split at applied
  · simp at applied
  split at applied
  · simp at applied
  split at applied
  · simp at applied
  split at applied
  · simp at applied
  · rename_i inside
    simp only [Bool.not_eq_true'] at inside
    simp only [Outcome.changed.injEq] at applied
    subst applied
    simpa [inBand_ignores_bookkeeping] using
      (Bool.not_eq_false _ |>.mp (by simpa using inside))

/-- Governance narrows the fee band and never widens it: whatever it installs,
the ELF's own ceiling still bounds every market's rate. -/
theorem the_fee_band_can_only_narrow
    (record after : Record) (proposed : Parameters) (digestMatches : Bool) (slot : Nat)
    (applied : applyChange record proposed digestMatches slot = .changed after) :
    after.parameters.maxFeeBasisPoints <= absoluteFeeCeilingBasisPoints :=
  inBand_fee_ceiling (applied_parameters_are_in_band record after proposed digestMatches slot applied)

/-- Ruling D1 item 1 made structural: a take and a payee move together or not at
all.  No reachable record has one without the other. -/
theorem a_take_and_a_payee_move_together
    (record after : Record) (proposed : Parameters) (digestMatches : Bool) (slot : Nat)
    (applied : applyChange record proposed digestMatches slot = .changed after) :
    (after.parameters.protocolTakeBasisPoints == 0) =
      after.parameters.protocolBeneficiaryIsZero :=
  inBand_take_iff_beneficiary
    (applied_parameters_are_in_band record after proposed digestMatches slot applied)

/-- Every applied change advances the generation by exactly one, so the receipt
stream a census reads is a total order with no gaps and no repeats. -/
theorem every_applied_change_advances_the_generation
    (record after : Record) (proposed : Parameters) (digestMatches : Bool) (slot : Nat)
    (applied : applyChange record proposed digestMatches slot = .changed after) :
    after.parameters.generation = record.parameters.generation + 1 := by
  unfold applyChange at applied
  split at applied
  · simp at applied
  split at applied
  · simp at applied
  split at applied
  · simp at applied
  split at applied
  · simp at applied
  · simp only [Outcome.changed.injEq] at applied
    subst applied
    rfl

/-- Nobody can shorten the notice below the constitutional floor, including by
governing the delay first: a proposal made under ANY in-band record matures no
sooner than `minimumChangeDelaySlots` from the slot it was made. -/
theorem a_proposal_carries_at_least_the_minimum_notice
    (record after : Record) (proposed : Parameters) (slot : Nat)
    (banded : inBand record.parameters = true)
    (staged : propose record true proposed slot = .changed after) :
    slot + minimumChangeDelaySlots <= after.pending.earliestApplySlot := by
  unfold propose at staged
  split at staged
  · simp at staged
  split at staged
  · simp at staged
  split at staged
  · simp at staged
  split at staged
  · simp at staged
  · simp only [Outcome.changed.injEq] at staged
    subst staged
    exact Nat.add_le_add_left (inBand_delay_floor banded) slot

/-- The one-way door.  A zero authority refuses every proposal, and no act in
this module writes an authority, so the freeze cannot be lifted from inside. -/
theorem frozen_governance_refuses_every_proposal
    (record : Record) (signer : Bool) (proposed : Parameters) (slot : Nat)
    (frozen : record.parameters.governanceAuthorityIsZero = true) :
    propose record signer proposed slot = .refused .governanceFrozen := by
  simp [propose, frozen]

/-!
## Non-vacuity

Every theorem above is an implication, and an implication over an empty domain
is a tautology wearing a proof.  So: one change that actually happens, end to
end, on the one parameter the ruling says will move first.
-/

def genesisRecord : Record :=
  { parameters := genesis, pending := { digestIsZero := true, earliestApplySlot := 0 } }

/-- The closer's reward cap moves from zero to one funded-crank floor: the
rent-exempt minimum of a 288-byte claim check at the kernel's reference rate,
which is `docs/design/FUNDED_CRANK_V1.md` section 3's chain-derived floor. -/
def fundedCloserProposal : Parameters :=
  { genesis with closerRewardCapLamports := 2895360 }

def proposedAt : Nat := 500000000

def stagedRecord : Outcome := propose genesisRecord true fundedCloserProposal proposedAt

theorem the_proposal_stages :
    stagedRecord = .changed
      { parameters := genesis
        pending := {
          digestIsZero := false
          earliestApplySlot := proposedAt + minimumChangeDelaySlots } } := by
  native_decide

theorem the_matured_change_applies :
    applyChange
      { parameters := genesis
        pending := {
          digestIsZero := false
          earliestApplySlot := proposedAt + minimumChangeDelaySlots } }
      fundedCloserProposal true (proposedAt + minimumChangeDelaySlots)
      = .changed {
        parameters := { fundedCloserProposal with
          generation := 1
          activationSlot := proposedAt + minimumChangeDelaySlots }
        pending := { digestIsZero := true, earliestApplySlot := 0 } } := by
  native_decide

/-- And the same change one slot early does not. -/
theorem the_same_change_one_slot_early_refuses :
    applyChange
      { parameters := genesis
        pending := {
          digestIsZero := false
          earliestApplySlot := proposedAt + minimumChangeDelaySlots } }
      fundedCloserProposal true (proposedAt + minimumChangeDelaySlots - 1)
      = .refused .proposalNotMatured := by
  native_decide

/-- And a fee cap one basis point above the ELF ceiling never even stages. -/
theorem a_cap_above_the_release_ceiling_never_stages :
    propose genesisRecord true
      { genesis with maxFeeBasisPoints := absoluteFeeCeilingBasisPoints + 1 } proposedAt
      = .refused .parameterOutOfBand := by
  native_decide

/-- And a take with no payee never stages, which is the ruling's own shape:
today's record has neither, and the only admitted move is to acquire both. -/
theorem a_take_with_no_payee_never_stages :
    propose genesisRecord true { genesis with protocolTakeBasisPoints := 1 } proposedAt
      = .refused .parameterOutOfBand := by
  native_decide

/-!
## The wire

One record layout and one change receipt.  The receipt is what a census reads:
it names the generation, the slot the change became effective, and the slot the
proposal was made, so the notice period is checkable from the receipt alone
without reconstructing the record's history.
-/

inductive RecordField where
  | magic | version | kind | bump | reservedHeader
  | governanceAuthority | protocolBeneficiary | pendingDigest
  | generation | activationSlot | changeDelaySlots | pendingEarliestApplySlot
  | closerRewardCapLamports | crankRewardCapLamports
  | maxFeeBasisPoints | protocolTakeBasisPoints | closerCarveBasisPoints
  | reservedTail
  deriving DecidableEq, Repr

def recordSchema : List (FieldSpec RecordField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.kind, .u8⟩, ⟨.bump, .u8⟩,
  ⟨.reservedHeader, .reserved 4⟩,
  ⟨.governanceAuthority, .bytes 32⟩, ⟨.protocolBeneficiary, .bytes 32⟩,
  ⟨.pendingDigest, .bytes 32⟩,
  ⟨.generation, .u64⟩, ⟨.activationSlot, .u64⟩,
  ⟨.changeDelaySlots, .u64⟩, ⟨.pendingEarliestApplySlot, .u64⟩,
  ⟨.closerRewardCapLamports, .u64⟩, ⟨.crankRewardCapLamports, .u64⟩,
  ⟨.maxFeeBasisPoints, .u16⟩, ⟨.protocolTakeBasisPoints, .u16⟩,
  ⟨.closerCarveBasisPoints, .u16⟩, ⟨.reservedTail, .reserved 26⟩
]

def recordLayout : List (PlacedField RecordField) := specialize recordSchema

def recordBytes : Nat := schemaWidth recordSchema

theorem record_width_is_one_hundred_ninety_two : recordBytes = 192 := by decide

theorem recordSchema_unique_names : (recordSchema.map FieldSpec.name).Nodup := by
  native_decide

theorem recordSchema_wellFormed : WellFormed recordSchema := by
  refine ⟨recordSchema_unique_names, ?_⟩
  native_decide

theorem recordFields_disjoint : recordLayout.Pairwise Before :=
  specializeFrom_pairwise 0 _

inductive ReceiptField where
  | magic | version | reservedHeader
  | previousDigest | newDigest
  | generation | proposedAtSlot | activationSlot | delaySlots
  | reservedTail
  deriving DecidableEq, Repr

def receiptSchema : List (FieldSpec ReceiptField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.reservedHeader, .reserved 6⟩,
  ⟨.previousDigest, .bytes 32⟩, ⟨.newDigest, .bytes 32⟩,
  ⟨.generation, .u64⟩, ⟨.proposedAtSlot, .u64⟩,
  ⟨.activationSlot, .u64⟩, ⟨.delaySlots, .u64⟩
]

def receiptLayout : List (PlacedField ReceiptField) := specialize receiptSchema

def receiptBytes : Nat := schemaWidth receiptSchema

theorem receipt_width_is_one_hundred_twelve : receiptBytes = 112 := by decide

theorem receiptSchema_unique_names : (receiptSchema.map FieldSpec.name).Nodup := by
  native_decide

theorem receiptSchema_wellFormed : WellFormed receiptSchema := by
  refine ⟨receiptSchema_unique_names, ?_⟩
  native_decide

theorem receiptFields_disjoint : receiptLayout.Pairwise Before :=
  specializeFrom_pairwise 0 _

end DClutch.ProtocolParametersV1
