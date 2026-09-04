import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec

/-!
# Canonical multiprogram Custody ABI

This module fixes the one request, replay-state, and receipt layout shared by
Core, Claims, Trading venues, and Resolution.  It deliberately owns only
collateral movement coordinates.  Liability supply, venue revisions, fees,
liveness funding, Hoard accounting, rent accounting, hashing, PDA derivation,
Registry CPI, token parsing, and token CPI remain separate semantic or adapter
boundaries.

Every transfer identifies its exact source and destination accounts and labels
their economic compartments.  The labels are evidence consumed by the caller;
Custody does not invent a second copy of the caller's economic transition.
There is no rent compartment, and the tags for Hoard principal, fees, liveness,
and recovery capital are definitionally distinct.
-/

namespace DClutch.CustodyAbi

open DClutch DClutch.AbiSchema

def abiVersion : Nat := 1

def requestMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x43, 0x55, 0x53, 0x52, 0x31] -- `DCLCUSR1`
def replayMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x43, 0x55, 0x53, 0x53, 0x31] -- `DCLCUSS1`
def receiptMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x43, 0x55, 0x53, 0x43, 0x31] -- `DCLCUSC1`

/-!
## The Custody namespace

Four PDA seed domains. The Rust stated all four itself and asserted the
thirty-two-byte seed bound with a `const _` block, which is the right assertion
in the wrong place: an over-long domain refuses every bump inside
`find_program_address`, so the authority it names can never sign and every route
through it is unreachable, and that is a fact about the domain, not about a
crate. Stated here, it holds for every backend that prints these.
-/

/-- `[authorityDomain, market, release_set]` -- the program-owned authority. -/
def authorityPdaDomain : String := "dclutch:custody-authority:v1"

/-- `[replayDomain, market, release_set, role, context]` -- one replay state. -/
def replayPdaDomain : String := "dclutch:custody-replay:v1"

/-- `[vaultDomain, market, release_set, context, compartment]` -- one token
vault.  A Market's Hoard is this domain at the Hoard-principal compartment. -/
def vaultPdaDomain : String := "dclutch:custody-vault:v1"

/-- Separates the adapter's token/replay poststate commitment.  Not a PDA seed,
but held to the same bound because it is the same alphabet of domains and a
reader should not have to know which of the four is which. -/
def poststateDomain : String := "dclutch:custody-poststate:v1"

def pdaDomains : List String :=
  [authorityPdaDomain, replayPdaDomain, vaultPdaDomain, poststateDomain]

/-- A PDA seed may be at most thirty-two bytes.  This is the `const _` assert
the Rust carried, moved to the object it is about. -/
theorem pda_domains_are_admissible_seeds :
    pdaDomains.all (fun domain => domain.toUTF8.toList.length <= 32) := by
  native_decide

/-- No two domains coincide, so no two authorities can share an address. -/
theorem pda_domains_are_pairwise_distinct : pdaDomains.Nodup := by native_decide

/-- The three record magics differ, which is what makes a request, a replay
state and a receipt distinguishable at their first eight bytes. -/
theorem record_magics_are_pairwise_distinct :
    [requestMagic, replayMagic, receiptMagic].Nodup := by native_decide

inductive Operation where
  | initializeReplay | openVault | transfer | closeVault | closeReplay
  deriving DecidableEq, Repr

def Operation.tag : Operation -> UInt8
  | .initializeReplay => 0
  | .openVault => 1
  | .transfer => 2
  | .closeVault => 3
  | .closeReplay => 4

inductive ExecutionRole where
  | core | claims | trading | resolution
  deriving DecidableEq, Repr

def ExecutionRole.tag : ExecutionRole -> UInt8
  | .core => 0 | .claims => 1 | .trading => 2 | .resolution => 3

inductive Compartment where
  | none | external | settlement | hoardPrincipal | tradingPrincipal
  | feeVault | livenessVault | seriesEscrow | recoveryReserve
  deriving DecidableEq, Repr

def Compartment.tag : Compartment -> UInt8
  | .none => 0 | .external => 1 | .settlement => 2
  | .hoardPrincipal => 3 | .tradingPrincipal => 4 | .feeVault => 5
  | .livenessVault => 6 | .seriesEscrow => 7 | .recoveryReserve => 8

namespace Compartment

def all : List Compartment := [
  .none, .external, .settlement, .hoardPrincipal, .tradingPrincipal,
  .feeVault, .livenessVault, .seriesEscrow, .recoveryReserve
]

def rustName : Compartment -> String
  | .none => "CUSTODY_COMPARTMENT_NONE_TAG_V1"
  | .external => "CUSTODY_COMPARTMENT_EXTERNAL_TAG_V1"
  | .settlement => "CUSTODY_COMPARTMENT_SETTLEMENT_TAG_V1"
  | .hoardPrincipal => "CUSTODY_COMPARTMENT_HOARD_PRINCIPAL_TAG_V1"
  | .tradingPrincipal => "CUSTODY_COMPARTMENT_TRADING_PRINCIPAL_TAG_V1"
  | .feeVault => "CUSTODY_COMPARTMENT_FEE_VAULT_TAG_V1"
  | .livenessVault => "CUSTODY_COMPARTMENT_LIVENESS_VAULT_TAG_V1"
  | .seriesEscrow => "CUSTODY_COMPARTMENT_SERIES_ESCROW_TAG_V1"
  | .recoveryReserve => "CUSTODY_COMPARTMENT_RECOVERY_RESERVE_TAG_V1"

end Compartment

theorem protected_compartments_are_distinct :
    Compartment.hoardPrincipal != .feeVault ∧
    Compartment.hoardPrincipal != .livenessVault ∧
    Compartment.hoardPrincipal != .recoveryReserve ∧
    Compartment.feeVault != .livenessVault := by decide

/-- The four named pairs above were the ones a reader was told about.  Every
compartment tag is distinct, and `all` is every compartment, so a decoder that
walks this list cannot map two compartments onto one byte or miss one. -/
theorem compartment_tags_are_pairwise_distinct :
    (Compartment.all.map Compartment.tag).Nodup := by native_decide

theorem compartment_names_are_unique :
    (Compartment.all.map Compartment.rustName).Nodup := by native_decide

/-- Whether the Transfer wire admits one ORDERED pair of compartments.

Two refusals, and they are different in kind.  `none` on either side is not a
transfer at all -- the tag exists for the sides an `OpenVault` or a `CloseReplay`
leaves inactive.  `hoardPrincipal -> feeVault` is a transfer the wire understands
perfectly and refuses anyway: the Hoard is the collateral every outstanding claim
is redeemed against, a fee is revenue, and paying the second out of the first is
the cross-subsidy `AGENTS.md` states as an invariant ("Hoard principal is never
fees, rent, bounty, insurance, work funding, reserve, or treasury capital") and
C-10 exists to forbid.

Until 2026-09-04 this rule lived only in the calling programs, and the atom
census said so out loud: sixty-four ordered pairs were shape-admissible and this
one was among them.  Every FeeVault-funding site in the tree sources
`tradingPrincipal`, so nothing legitimate moves -- but "no caller does it" and
"the wire will not carry it" are different claims, and only the second survives a
caller nobody has written yet. -/
def transferPairAdmissible : Compartment -> Compartment -> Bool
  | .none, _ => false
  | _, .none => false
  | .hoardPrincipal, .feeVault => false
  | _, _ => true

/-- The law itself, named so a reader looking for it finds it here. -/
theorem hoard_principal_never_funds_the_fee_vault :
    transferPairAdmissible .hoardPrincipal .feeVault = false := by decide

/-- And it is exactly one pair, not a family: every other ordered pair of live
compartments is still carried, including `feeVault -> hoardPrincipal`, which is
a different movement with a different argument and is not ruled here. -/
theorem only_the_named_pair_is_refused
    (source destination : Compartment)
    (liveSource : source ≠ .none) (liveDestination : destination ≠ .none)
    (unnamed : ¬(source = .hoardPrincipal ∧ destination = .feeVault)) :
    transferPairAdmissible source destination = true := by
  cases source <;> cases destination <;> simp_all [transferPairAdmissible]

/-- Every ordered pair the census walks: nine compartments each way. -/
def orderedCompartmentPairs : List (Compartment × Compartment) :=
  Compartment.all.flatMap fun source =>
    Compartment.all.map fun destination => (source, destination)

theorem ordered_compartment_pairs_are_eighty_one :
    orderedCompartmentPairs.length = 81 := by native_decide

/-- THE COUNT, stated where the rule is stated.  Eight live compartments each
way is sixty-four; one named refusal leaves sixty-three.  The Rust census
asserts the same number against `CustodyRequestV1::validate`, so the two authors
have to agree or one of them goes red. -/
theorem admissible_ordered_pairs_are_sixty_three :
    (orderedCompartmentPairs.filter
      (fun pair => transferPairAdmissible pair.1 pair.2)).length = 63 := by
  native_decide

inductive RequestField where
  | magic | version | operation | callerRole | sourceCompartment
  | destinationCompartment | transferIndex | releaseSet | market | realm
  | context | callerProgram | candidate | sourceOwner | destinationOwner | order
  | parentRequestDigest
  | source | destination | sourceVaultContext | destinationVaultContext
  | mint | tokenProgram | payer | rentRefund
  | expectedRevision | resultingRevision | orderNonce | generation | amount
  | rentLamports | pageIndex | executionIndex | reserved
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.operation, .u8⟩,
  ⟨.callerRole, .u8⟩, ⟨.sourceCompartment, .u8⟩,
  ⟨.destinationCompartment, .u8⟩, ⟨.transferIndex, .u16⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.market, .bytes 32⟩,
  ⟨.realm, .bytes 32⟩, ⟨.context, .bytes 32⟩,
  ⟨.callerProgram, .bytes 32⟩, ⟨.candidate, .bytes 32⟩,
  ⟨.sourceOwner, .bytes 32⟩, ⟨.destinationOwner, .bytes 32⟩,
  ⟨.order, .bytes 32⟩,
  ⟨.parentRequestDigest, .bytes 32⟩, ⟨.source, .bytes 32⟩,
  ⟨.destination, .bytes 32⟩, ⟨.sourceVaultContext, .bytes 32⟩,
  ⟨.destinationVaultContext, .bytes 32⟩, ⟨.mint, .bytes 32⟩,
  ⟨.tokenProgram, .bytes 32⟩, ⟨.payer, .bytes 32⟩,
  ⟨.rentRefund, .bytes 32⟩, ⟨.expectedRevision, .u64⟩,
  ⟨.resultingRevision, .u64⟩, ⟨.orderNonce, .u64⟩,
  ⟨.generation, .u64⟩, ⟨.amount, .u64⟩,
  ⟨.rentLamports, .u64⟩, ⟨.pageIndex, .u32⟩,
  ⟨.executionIndex, .u32⟩, ⟨.reserved, .reserved 24⟩
]

inductive ReplayField where
  | magic | version | status | callerRole | openVaultCount | releaseSet
  | market | realm | context | callerProgram | rentRefund | nextRevision
  | generation | lastRequestDigest | lastPoststateCommitment
  deriving DecidableEq, Repr

def replaySchema : List (FieldSpec ReplayField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.status, .u8⟩,
  ⟨.callerRole, .u8⟩, ⟨.openVaultCount, .u32⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.market, .bytes 32⟩,
  ⟨.realm, .bytes 32⟩, ⟨.context, .bytes 32⟩,
  ⟨.callerProgram, .bytes 32⟩, ⟨.rentRefund, .bytes 32⟩,
  ⟨.nextRevision, .u64⟩, ⟨.generation, .u64⟩,
  ⟨.lastRequestDigest, .bytes 32⟩,
  ⟨.lastPoststateCommitment, .bytes 32⟩
]

inductive ReceiptField where
  | magic | version | operation | callerRole | sourceCompartment
  | destinationCompartment | transferIndex | releaseSet | market | context
  | parentRequestDigest | requestDigest | source | destination
  | expectedRevision | resultingRevision | sourceBefore | sourceAfter
  | destinationBefore | destinationAfter | amount | rentLamports
  | poststateCommitment | replayStateDigest | reserved
  deriving DecidableEq, Repr

def receiptSchema : List (FieldSpec ReceiptField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.operation, .u8⟩,
  ⟨.callerRole, .u8⟩, ⟨.sourceCompartment, .u8⟩,
  ⟨.destinationCompartment, .u8⟩, ⟨.transferIndex, .u16⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.market, .bytes 32⟩,
  ⟨.context, .bytes 32⟩, ⟨.parentRequestDigest, .bytes 32⟩,
  ⟨.requestDigest, .bytes 32⟩, ⟨.source, .bytes 32⟩,
  ⟨.destination, .bytes 32⟩, ⟨.expectedRevision, .u64⟩,
  ⟨.resultingRevision, .u64⟩, ⟨.sourceBefore, .u64⟩,
  ⟨.sourceAfter, .u64⟩, ⟨.destinationBefore, .u64⟩,
  ⟨.destinationAfter, .u64⟩, ⟨.amount, .u64⟩,
  ⟨.rentLamports, .u64⟩, ⟨.poststateCommitment, .bytes 32⟩,
  ⟨.replayStateDigest, .bytes 32⟩, ⟨.reserved, .reserved 16⟩
]

def requestLayout := specialize requestSchema
def replayLayout := specialize replaySchema
def receiptLayout := specialize receiptSchema
def requestBytes := schemaWidth requestSchema
def replayBytes := schemaWidth replaySchema
def receiptBytes := schemaWidth receiptSchema

theorem exact_physical_widths :
    requestBytes = 672 ∧ replayBytes = 288 ∧ receiptBytes = 384 := by
  native_decide

theorem schemas_well_formed :
    WellFormed requestSchema ∧ WellFormed replaySchema ∧ WellFormed receiptSchema := by
  simp [WellFormed, requestSchema, replaySchema, receiptSchema, FieldKind.byteWidth]

theorem layouts_are_byte_disjoint :
    requestLayout.Pairwise Before ∧ replayLayout.Pairwise Before ∧
    receiptLayout.Pairwise Before := by
  exact ⟨specializeFrom_pairwise 0 _, specializeFrom_pairwise 0 _,
    specializeFrom_pairwise 0 _⟩

theorem request_coordinates_are_canonical : coordinates requestLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.operation, 10, 1),
    (.callerRole, 11, 1), (.sourceCompartment, 12, 1),
    (.destinationCompartment, 13, 1), (.transferIndex, 14, 2),
    (.releaseSet, 16, 32), (.market, 48, 32), (.realm, 80, 32),
    (.context, 112, 32), (.callerProgram, 144, 32),
    (.candidate, 176, 32), (.sourceOwner, 208, 32),
    (.destinationOwner, 240, 32), (.order, 272, 32),
    (.parentRequestDigest, 304, 32), (.source, 336, 32),
    (.destination, 368, 32), (.sourceVaultContext, 400, 32),
    (.destinationVaultContext, 432, 32), (.mint, 464, 32),
    (.tokenProgram, 496, 32), (.payer, 528, 32), (.rentRefund, 560, 32),
    (.expectedRevision, 592, 8), (.resultingRevision, 600, 8),
    (.orderNonce, 608, 8), (.generation, 616, 8), (.amount, 624, 8),
    (.rentLamports, 632, 8), (.pageIndex, 640, 4),
    (.executionIndex, 644, 4), (.reserved, 648, 24)] := by native_decide

end DClutch.CustodyAbi
