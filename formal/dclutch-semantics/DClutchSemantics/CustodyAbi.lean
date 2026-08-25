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

inductive Operation where
  | initializeReplay | openVault | transfer | closeVault
  deriving DecidableEq, Repr

def Operation.tag : Operation -> UInt8
  | .initializeReplay => 0
  | .openVault => 1
  | .transfer => 2
  | .closeVault => 3

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

theorem protected_compartments_are_distinct :
    Compartment.hoardPrincipal != .feeVault ∧
    Compartment.hoardPrincipal != .livenessVault ∧
    Compartment.hoardPrincipal != .recoveryReserve ∧
    Compartment.feeVault != .livenessVault := by decide

inductive RequestField where
  | magic | version | operation | callerRole | sourceCompartment
  | destinationCompartment | transferIndex | releaseSet | market | realm
  | context | callerProgram | candidate | actor | order | parentRequestDigest
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
  ⟨.actor, .bytes 32⟩, ⟨.order, .bytes 32⟩,
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
  | magic | version | status | callerRole | reservedHeader | releaseSet
  | market | realm | context | callerProgram | rentRefund | nextRevision
  | generation | lastRequestDigest | lastPoststateCommitment
  deriving DecidableEq, Repr

def replaySchema : List (FieldSpec ReplayField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.status, .u8⟩,
  ⟨.callerRole, .u8⟩, ⟨.reservedHeader, .reserved 4⟩,
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
    requestBytes = 640 ∧ replayBytes = 288 ∧ receiptBytes = 384 := by
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
    (.candidate, 176, 32), (.actor, 208, 32), (.order, 240, 32),
    (.parentRequestDigest, 272, 32), (.source, 304, 32),
    (.destination, 336, 32), (.sourceVaultContext, 368, 32),
    (.destinationVaultContext, 400, 32), (.mint, 432, 32),
    (.tokenProgram, 464, 32), (.payer, 496, 32), (.rentRefund, 528, 32),
    (.expectedRevision, 560, 8), (.resultingRevision, 568, 8),
    (.orderNonce, 576, 8), (.generation, 584, 8), (.amount, 592, 8),
    (.rentLamports, 600, 8), (.pageIndex, 608, 4),
    (.executionIndex, 612, 4), (.reserved, 616, 24)] := by native_decide

end DClutch.CustodyAbi
