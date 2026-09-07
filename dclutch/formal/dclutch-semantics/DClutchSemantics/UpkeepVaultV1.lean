import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import DClutchSemantics.CustodyAbi

/-!
# The upkeep vault

One protocol-owned lamport PDA with no authority, chartered by decision 0024
item 4 exactly as `docs/design/UPKEEP_VAULT_V0.md` sketches it and no wider.
The record shape is `docs/design/ECONOMICS_MODELS_2026_09_04.md` section 7;
this module is that shape as a statement rather than a table.

## The three invariants, as things this module can be wrong about

**I1 -- no involuntary inflow.** Every credit names its source from
`SourceClass`, which has exactly four members and no member a trade fee, a
rent principal or a recorded receivable could be presented as.  The enum is
closed by construction: `SourceClass.decode` is total over the byte and refuses
every value that is not one of the four.

**I2 -- no discretionary outflow.** There is no spend instruction.  That is a
property of `Operation`, which has two members -- `found` and `credit` -- and
of `decodeOperation`, which refuses the reserved debit tag BY NAME rather than
as an unknown byte, so a caller who tries to spend is told which charter
stopped them.  `credit_never_moves_the_outflow_total` is the same fact stated
over the only transition the record has.

**I3 -- full legibility.** `legible` says the account's lamports are exactly
its own rent minimum plus `inflowTotal - outflowTotal`; `credit` preserves it
for a credit of exactly the amount that landed.  Lamports that reach the
address without a credit -- anyone may transfer into any account -- are
`unreceipted`, a quantity the census holds to a declared delta like every other
class and never a place to hide.

## What it deliberately does not have

No `authority`, no `pending`, no price table.  The parameter record next door
(`ProtocolParametersV1`) is governable because its values are policy; this one
is not, because a governed price is a discretionary payout wearing a schedule.
An outflow route, when one is ruled, carries its own price derived from the
Rent sysvar (`docs/design/FUNDED_CRANK_V1.md` section 3) and moves
`outflowTotal`; until then the field is a running number a census holds at
zero.
-/

namespace DClutch.UpkeepVaultV1

open DClutch DClutch.AbiSchema

def abiVersion : Nat := 1

/-- `DCLCUPK1` -- the vault record. -/
def recordMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x43, 0x55, 0x50, 0x4b, 0x31]

/-- `DCLCUPQ1` -- the one request wire, for both routes. -/
def requestMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x43, 0x55, 0x50, 0x51, 0x31]

/-- `DCLCUPC1` -- the credit receipt a caller reads back. -/
def receiptMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x43, 0x55, 0x50, 0x43, 0x31]

theorem magics_are_pairwise_distinct :
    [recordMagic, requestMagic, receiptMagic].Nodup := by native_decide

/-- The vault's seeds are the domain alone: one vault per Custody deployment. -/
def pdaDomain : String := CustodyAbi.upkeepVaultPdaDomain

theorem the_vault_domain_is_one_of_custodys :
    pdaDomain ∈ CustodyAbi.pdaDomains := by native_decide

/-!
## The source classes (I1)
-/

/-- The only provenances a lamport may enter the vault under.

`residue` is a residue a ruling would otherwise send nowhere (escrow-close
residue after the opener is serviced, compaction dust, a rate-change surplus).
`donation` is an explicitly ruled donation slice -- today the maker-replay
close's `unclassified_donation` less the closer's carve.  `seatRent` is the
certificate seat's prepaid rent nothing reimburses, the one class that has
carried real money on every cohort.  `deposit` is a voluntary deposit by a
signer.  There is no fifth member, and in particular no member for a trade fee,
a rent principal or a recorded receivable. -/
inductive SourceClass where
  | residue | donation | seatRent | deposit
  deriving DecidableEq, Repr

def SourceClass.tag : SourceClass -> UInt8
  | .residue => 0 | .donation => 1 | .seatRent => 2 | .deposit => 3

def SourceClass.all : List SourceClass := [.residue, .donation, .seatRent, .deposit]

def SourceClass.rustName : SourceClass -> String
  | .residue => "UPKEEP_SOURCE_RESIDUE_TAG_V1"
  | .donation => "UPKEEP_SOURCE_DONATION_TAG_V1"
  | .seatRent => "UPKEEP_SOURCE_SEAT_RENT_TAG_V1"
  | .deposit => "UPKEEP_SOURCE_DEPOSIT_TAG_V1"

/-- Total over the byte, and closed: every byte decodes to one of the four or
to nothing. -/
def SourceClass.decode : UInt8 -> Option SourceClass
  | 0 => some .residue
  | 1 => some .donation
  | 2 => some .seatRent
  | 3 => some .deposit
  | _ => none

theorem source_classes_are_exactly_four : SourceClass.all.length = 4 := by decide

theorem source_class_tags_are_pairwise_distinct :
    (SourceClass.all.map SourceClass.tag).Nodup := by native_decide

theorem source_class_decode_inverts_tag (c : SourceClass) :
    SourceClass.decode c.tag = some c := by
  cases c <;> rfl

/-- I1's closed-enum clause: a byte that decodes at all decodes to a member of
`all`, so there is no reachable fifth provenance. -/
theorem every_decoded_source_class_is_one_of_the_four
    (byte : UInt8) (c : SourceClass) (decoded : SourceClass.decode byte = some c) :
    c ∈ SourceClass.all := by
  cases c <;> simp [SourceClass.all]

/-- Only a signer may deposit; every other class is a protocol route's word for
lamports it moved out of an account it owns.  This is the split the adapter
authenticates: `deposit` needs a signer and a System transfer, the other three
need the release set's caller-authority PDA. -/
def SourceClass.isVoluntary : SourceClass -> Bool
  | .deposit => true
  | _ => false

/-!
## The operations (I2)
-/

/-- `found` creates the record once; `credit` adds lamports under a class.
There is no third member. -/
inductive Operation where
  | found | credit
  deriving DecidableEq, Repr

def Operation.tag : Operation -> UInt8
  | .found => 0 | .credit => 1

/-- The tag a spend instruction would have had.  Reserved forever and refused by
name, so that the hostile which tries to spend is told "there is no spend
route" rather than "unknown byte". -/
def debitTag : UInt8 := 2

inductive OperationRefusal where
  | noSpendRoute | unknownOperation
  deriving DecidableEq, Repr

/-- Its own type rather than `Except`, so every witness below can be decided
against a literal (the same reason `ProtocolParametersV1.Outcome` exists). -/
inductive OperationDecode where
  | ok (op : Operation)
  | refused (reason : OperationRefusal)
  deriving DecidableEq, Repr

def decodeOperation (byte : UInt8) : OperationDecode :=
  if byte == 0 then .ok .found
  else if byte == 1 then .ok .credit
  else if byte == debitTag then .refused .noSpendRoute
  else .refused .unknownOperation

/-- I2, as the theorem the ruling asked for: the spend tag refuses, by name. -/
theorem there_is_no_spend_instruction :
    decodeOperation debitTag = .refused .noSpendRoute := by decide

/-- And nothing else decodes to an operation that moves lamports out. -/
theorem every_operation_is_found_or_credit
    (byte : UInt8) (op : Operation) (decoded : decodeOperation byte = .ok op) :
    op = .found ∨ op = .credit := by
  cases op <;> simp

theorem the_two_operations_round_trip (op : Operation) :
    decodeOperation op.tag = .ok op := by
  cases op <;> rfl

/-!
## The record and its one transition
-/

structure Vault where
  inflowTotal : Nat
  outflowTotal : Nat
  inflowResidue : Nat
  inflowDonation : Nat
  inflowSeatRent : Nat
  inflowDeposit : Nat
  /-- One per receipted credit; the census's clock. -/
  creditCount : Nat
  deriving DecidableEq, Repr

def genesis : Vault :=
  { inflowTotal := 0, outflowTotal := 0, inflowResidue := 0, inflowDonation := 0,
    inflowSeatRent := 0, inflowDeposit := 0, creditCount := 0 }

/-- What the record says the vault holds above its own rent. -/
def Vault.receipted (v : Vault) : Nat := v.inflowTotal - v.outflowTotal

def Vault.classesSum (v : Vault) : Nat :=
  v.inflowResidue + v.inflowDonation + v.inflowSeatRent + v.inflowDeposit

/-- A record this module could have written. -/
def Vault.consistent (v : Vault) : Prop :=
  v.outflowTotal <= v.inflowTotal ∧ v.inflowTotal = v.classesSum

/-- I3: the account's lamports are exactly its rent minimum plus the receipted
position. -/
def legible (v : Vault) (lamports rentMinimum : Nat) : Prop :=
  lamports = rentMinimum + v.receipted

/-- Lamports at the address that no credit has receipted.  A class the census
holds to a declared delta; never negative, because nothing can debit. -/
def unreceipted (v : Vault) (lamports rentMinimum : Nat) : Nat :=
  lamports - rentMinimum - v.receipted

inductive Refusal where
  /-- A credit of zero is no act. -/
  | zeroAmount
  deriving DecidableEq, Repr

/-- What the one transition returns; decidable, so witnesses are literals. -/
inductive CreditOutcome where
  | refused (reason : Refusal)
  | credited (vault : Vault)
  deriving DecidableEq, Repr

def Vault.creditClass (v : Vault) (c : SourceClass) (amount : Nat) : Vault :=
  match c with
  | .residue => { v with inflowResidue := v.inflowResidue + amount }
  | .donation => { v with inflowDonation := v.inflowDonation + amount }
  | .seatRent => { v with inflowSeatRent := v.inflowSeatRent + amount }
  | .deposit => { v with inflowDeposit := v.inflowDeposit + amount }

/-- The only transition.  Adds `amount` to the total and to its class, and
advances the count by one.  It does not read a balance: the adapter has
already required that at least `amount` unreceipted lamports sit at the address
before it calls this. -/
def credit (v : Vault) (c : SourceClass) (amount : Nat) : CreditOutcome :=
  if amount == 0 then .refused .zeroAmount
  else
    let classed := v.creditClass c amount
    .credited { classed with
      inflowTotal := v.inflowTotal + amount
      creditCount := v.creditCount + 1 }

theorem credit_of_zero_refuses (v : Vault) (c : SourceClass) :
    credit v c 0 = .refused .zeroAmount := by
  simp [credit]

theorem credit_advances_the_count_by_one
    (v after : Vault) (c : SourceClass) (amount : Nat)
    (ok : credit v c amount = .credited after) :
    after.creditCount = v.creditCount + 1 := by
  unfold credit at ok
  split at ok
  · simp at ok
  · simp only [CreditOutcome.credited.injEq] at ok
    subst ok
    cases c <;> rfl

theorem credit_adds_exactly_the_amount_to_the_total
    (v after : Vault) (c : SourceClass) (amount : Nat)
    (ok : credit v c amount = .credited after) :
    after.inflowTotal = v.inflowTotal + amount := by
  unfold credit at ok
  split at ok
  · simp at ok
  · simp only [CreditOutcome.credited.injEq] at ok
    subst ok
    cases c <;> rfl

/-- I2 over the transition: no credit moves the outflow total, and there is no
other transition. -/
theorem credit_never_moves_the_outflow_total
    (v after : Vault) (c : SourceClass) (amount : Nat)
    (ok : credit v c amount = .credited after) :
    after.outflowTotal = v.outflowTotal := by
  unfold credit at ok
  split at ok
  · simp at ok
  · simp only [CreditOutcome.credited.injEq] at ok
    subst ok
    cases c <;> rfl

/-- The receipted position never falls: nothing debits. -/
theorem the_receipted_position_never_falls
    (v after : Vault) (c : SourceClass) (amount : Nat)
    (ok : credit v c amount = .credited after) :
    v.receipted <= after.receipted := by
  have total := credit_adds_exactly_the_amount_to_the_total v after c amount ok
  have outflow := credit_never_moves_the_outflow_total v after c amount ok
  unfold Vault.receipted
  rw [total, outflow]
  omega

/-- I3 is preserved by a credit of exactly what landed. -/
theorem credit_preserves_legibility
    (v after : Vault) (c : SourceClass) (amount lamports rentMinimum : Nat)
    (before : legible v lamports rentMinimum)
    (consistent : v.outflowTotal <= v.inflowTotal)
    (ok : credit v c amount = .credited after) :
    legible after (lamports + amount) rentMinimum := by
  have total := credit_adds_exactly_the_amount_to_the_total v after c amount ok
  have outflow := credit_never_moves_the_outflow_total v after c amount ok
  unfold legible Vault.receipted at *
  rw [total, outflow]
  omega

theorem credit_preserves_consistency
    (v after : Vault) (c : SourceClass) (amount : Nat)
    (consistent : v.consistent)
    (ok : credit v c amount = .credited after) :
    after.consistent := by
  unfold credit at ok
  split at ok
  · simp at ok
  · simp only [CreditOutcome.credited.injEq] at ok
    subst ok
    unfold Vault.consistent Vault.classesSum at *
    cases c <;> simp [Vault.creditClass] <;> omega

theorem genesis_is_consistent : genesis.consistent := ⟨Nat.le_refl _, rfl⟩

theorem genesis_is_legible_at_its_rent (rentMinimum : Nat) :
    legible genesis rentMinimum rentMinimum := by
  simp [legible, genesis, Vault.receipted]

/-! ## Non-vacuity: one credit of each class, end to end. -/

theorem a_donation_credit_lands_in_its_class :
    credit genesis .donation 1463040 =
      .credited { genesis with inflowTotal := 1463040, inflowDonation := 1463040, creditCount := 1 } := by
  native_decide

theorem a_seat_rent_credit_lands_in_its_class :
    credit genesis .seatRent 2786520 =
      .credited { genesis with inflowTotal := 2786520, inflowSeatRent := 2786520, creditCount := 1 } := by
  native_decide

theorem a_second_credit_sums_and_counts :
    credit { genesis with inflowTotal := 5, inflowDeposit := 5, creditCount := 1 } .residue 7 =
      .credited { genesis with
        inflowTotal := 12, inflowDeposit := 5, inflowResidue := 7, creditCount := 2 } := by
  native_decide

/-!
## The wires
-/

inductive RecordField where
  | magic | version | kind | bump | reservedHeader
  | inflowTotal | outflowTotal
  | inflowResidue | inflowDonation | inflowSeatRent | inflowDeposit
  | creditCount | lastCreditDigest | reservedTail
  deriving DecidableEq, Repr

def recordSchema : List (FieldSpec RecordField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.kind, .u8⟩, ⟨.bump, .u8⟩,
  ⟨.reservedHeader, .reserved 4⟩,
  ⟨.inflowTotal, .u64⟩, ⟨.outflowTotal, .u64⟩,
  ⟨.inflowResidue, .u64⟩, ⟨.inflowDonation, .u64⟩,
  ⟨.inflowSeatRent, .u64⟩, ⟨.inflowDeposit, .u64⟩,
  ⟨.creditCount, .u64⟩, ⟨.lastCreditDigest, .bytes 32⟩,
  ⟨.reservedTail, .reserved 24⟩
]

def recordLayout : List (PlacedField RecordField) := specialize recordSchema
def recordBytes : Nat := schemaWidth recordSchema

theorem record_width_is_one_hundred_twenty_eight : recordBytes = 128 := by decide

theorem recordSchema_unique_names : (recordSchema.map FieldSpec.name).Nodup := by
  native_decide

theorem recordSchema_wellFormed : WellFormed recordSchema := by
  refine ⟨recordSchema_unique_names, ?_⟩
  native_decide

theorem recordFields_disjoint : recordLayout.Pairwise Before :=
  specializeFrom_pairwise 0 _

/-- One wire for both routes.  `found` carries zeros below the header; `credit`
carries its class, its amount and -- for the three protocol classes -- the
release set, market, role and context the caller-authority PDA is derived
from, plus the digest of the calling route's own receipt so the act is
receipted twice: once by the caller, once here. -/
inductive RequestField where
  | magic | version | operation | sourceClass | callerRole | reservedHeader
  | releaseSet | market | context | receiptDigest
  | amount | reservedTail
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.operation, .u8⟩,
  ⟨.sourceClass, .u8⟩, ⟨.callerRole, .u8⟩, ⟨.reservedHeader, .reserved 3⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.market, .bytes 32⟩,
  ⟨.context, .bytes 32⟩, ⟨.receiptDigest, .bytes 32⟩,
  ⟨.amount, .u64⟩, ⟨.reservedTail, .reserved 8⟩
]

def requestLayout : List (PlacedField RequestField) := specialize requestSchema
def requestBytes : Nat := schemaWidth requestSchema

theorem request_width_is_one_hundred_sixty : requestBytes = 160 := by decide

theorem requestSchema_unique_names : (requestSchema.map FieldSpec.name).Nodup := by
  native_decide

theorem requestSchema_wellFormed : WellFormed requestSchema := by
  refine ⟨requestSchema_unique_names, ?_⟩
  native_decide

theorem requestFields_disjoint : requestLayout.Pairwise Before :=
  specializeFrom_pairwise 0 _

inductive ReceiptField where
  | magic | version | operation | sourceClass | reservedHeader
  | requestDigest | vault
  | amount | inflowTotalAfter | outflowTotalAfter | creditCountAfter
  deriving DecidableEq, Repr

def receiptSchema : List (FieldSpec ReceiptField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.operation, .u8⟩,
  ⟨.sourceClass, .u8⟩, ⟨.reservedHeader, .reserved 4⟩,
  ⟨.requestDigest, .bytes 32⟩, ⟨.vault, .bytes 32⟩,
  ⟨.amount, .u64⟩, ⟨.inflowTotalAfter, .u64⟩,
  ⟨.outflowTotalAfter, .u64⟩, ⟨.creditCountAfter, .u64⟩
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

/-!
## The frames

Stated here because the SBF adapter and the operator both read them, and a
frame width with two authors is the defect `CUSTODY_BUMP_RELAY_BYTES_V1`'s
docstring records.
-/

/-- `[vault, payer, system_program, rent_sysvar]`. -/
def foundAccountCount : Nat := 4

/-- `[vault, depositor, system_program, rent_sysvar]`: a signer's deposit. -/
def depositCreditAccountCount : Nat := 4

/-- `[vault, caller_authority, activation_cache, registry_program,
caller_program, caller_programdata, rent_sysvar]`: a protocol route's credit,
signed by the release set's caller-authority PDA exactly as every other Custody
route is. -/
def protocolCreditAccountCount : Nat := 7

end DClutch.UpkeepVaultV1
