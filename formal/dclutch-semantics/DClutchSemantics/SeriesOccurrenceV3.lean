import DClutchSemantics.ExecutionRelease
import Std.Tactic

/-!
# Series occurrence V3

A recurring Template binds reusable Product, occurrence, and capability-template
generators plus their derivation policies.  It does not freeze one categorical
Product, one outcome width, or one denominator profile across the whole Series.

Each scheduled occurrence is instead a content-addressed realization.  The
realization binds its exact Product record, resolution policy,
nonnegative liability basis, rational representation, capability manifest,
funding list, Market, and four founding compartments.  A physical adapter
refines `projectionMember` with the canonical Merkle proof and finalized-record
checks.  The operator supplies a witness, never an authority to choose any of
those identities.
-/

namespace DClutch.SeriesOccurrenceV3

open DClutch

abbrev Identity := ExecutionRelease.Identity

/-- Reusable generator and schedule authority for a recurring Series. -/
structure Template where
  realmId : Identity
  releaseSetId : Identity
  productGeneratorId : Identity
  occurrenceGeneratorId : Identity
  capabilityTemplateId : Identity
  productDerivationPolicyId : Identity
  occurrenceDerivationPolicyId : Identity
  capabilityDerivationPolicyId : Identity
  fundingDerivationPolicyId : Identity
  occurrenceProjectionRoot : Identity
  seriesRefundOwner : Identity
  occurrenceCount : Nat
  firstOccurrenceSlot : Nat
  periodSlots : Nat
  retryWindowSlots : Nat
  seriesCloseRentLamports : Nat
  deriving DecidableEq, Repr

def scheduledSlot (template : Template) (occurrence : Nat) : Nat :=
  template.firstOccurrenceSlot + occurrence * template.periodSlots

def retryThroughSlot (template : Template) (occurrence : Nat) : Nat :=
  scheduledSlot template occurrence + template.retryWindowSlots

def Template.valid
    (slotLimit lamportLimit : Nat) (template : Template) : Bool :=
  [template.realmId, template.releaseSetId, template.productGeneratorId,
      template.occurrenceGeneratorId, template.capabilityTemplateId,
      template.productDerivationPolicyId, template.occurrenceDerivationPolicyId,
      template.capabilityDerivationPolicyId, template.fundingDerivationPolicyId,
      template.occurrenceProjectionRoot, template.seriesRefundOwner].all
      ExecutionRelease.identityValid &&
  0 < template.occurrenceCount && 0 < template.periodSlots &&
  template.seriesCloseRentLamports < lamportLimit &&
  retryThroughSlot template (template.occurrenceCount - 1) < slotLimit

/-- Exact occurrence-owned founding resources.  The positive Hoard principal
is never combined with native rent, capability, or work funding. -/
structure FoundingFunds where
  hoardPrincipal : Nat
  marketRentLamports : Nat
  capabilityNativeLamports : Nat
  foundingWorkLamports : Nat
  deriving DecidableEq, Repr

/-! Hoard principal is in Realm-collateral atomic units.  The other three
fields are lamports, so there is intentionally no cross-asset total. -/
def FoundingFunds.nativeLamports (funds : FoundingFunds) : Nat :=
  funds.marketRentLamports + funds.capabilityNativeLamports +
    funds.foundingWorkLamports

def FoundingFunds.valid (lamportLimit : Nat) (funds : FoundingFunds) : Bool :=
  0 < funds.hoardPrincipal && funds.nativeLamports < lamportLimit

/-- One exact scheduled realization.  Basis and representation are opaque
content identities; this structure contains no categorical, width, scale, or
denominator discriminator. -/
structure Occurrence where
  occurrence : Nat
  scheduledSlot : Nat
  productRecordId : Identity
  resolutionPolicyId : Identity
  liabilityBasisId : Identity
  rationalRepresentationId : Identity
  capabilityManifestId : Identity
  fundingListId : Identity
  marketId : Identity
  funds : FoundingFunds
  deriving DecidableEq, Repr

/-- Ephemeral facts returned by independently authenticating the Product
Runtime V2 graph selected by `productRecordId`.  Stable Product and result
domain identities are deliberately absent from `Occurrence`; this projection
is recomputed at the physical boundary rather than becoming a second
persisted Series truth. -/
structure AuthenticatedProductProjection where
  productRecordId : Identity
  stableProductId : Identity
  resultDomainId : Identity
  deriving DecidableEq, Repr

def AuthenticatedProductProjection.valid
    (value : AuthenticatedProductProjection) : Bool :=
  [value.productRecordId, value.stableProductId, value.resultDomainId].all
    ExecutionRelease.identityValid

def AuthenticatedProductProjection.exactFor
    (value : AuthenticatedProductProjection) (occurrence : Occurrence) : Bool :=
  value.valid && value.productRecordId = occurrence.productRecordId

theorem authenticated_projection_joins_only_product_record
    (projection : AuthenticatedProductProjection) (occurrence : Occurrence)
    (exact : projection.exactFor occurrence = true) :
    projection.productRecordId = occurrence.productRecordId := by
  simp [AuthenticatedProductProjection.exactFor] at exact
  exact exact.2

theorem substituted_product_record_projection_refuses
    (projection : AuthenticatedProductProjection) (occurrence : Occurrence)
    (different : projection.productRecordId ≠ occurrence.productRecordId) :
    projection.exactFor occurrence = false := by
  simp [AuthenticatedProductProjection.exactFor, different]

def Occurrence.valid
    (lamportLimit : Nat) (template : Template) (value : Occurrence) : Bool :=
  value.occurrence < template.occurrenceCount &&
  value.scheduledSlot = DClutch.SeriesOccurrenceV3.scheduledSlot template value.occurrence &&
  [value.productRecordId, value.resolutionPolicyId,
      value.liabilityBasisId, value.rationalRepresentationId,
      value.capabilityManifestId, value.fundingListId, value.marketId].all
      ExecutionRelease.identityValid &&
  value.funds.valid lamportLimit

/-- Admission after the physical Merkle/content-addressed boundary has returned
the unique projection committed by `template.occurrenceProjectionRoot`. -/
def projectionAccepts
    (slotLimit lamportLimit : Nat)
    (template : Template) (committed observed : Occurrence)
    (projectionMember : Bool) : Bool :=
  projectionMember && template.valid slotLimit lamportLimit &&
  observed.valid lamportLimit template && observed = committed

theorem admitted_projection_is_unique
    (slotLimit lamportLimit : Nat)
    (template : Template) (committed left right : Occurrence)
    (leftMember rightMember : Bool)
    (leftAccepted : projectionAccepts slotLimit lamportLimit template
      committed left leftMember = true)
    (rightAccepted : projectionAccepts slotLimit lamportLimit template
      committed right rightMember = true) :
    left = right := by
  simp [projectionAccepts] at leftAccepted rightAccepted
  exact leftAccepted.2.trans rightAccepted.2.symm

theorem substituted_projection_refuses
    (slotLimit lamportLimit : Nat)
    (template : Template) (committed observed : Occurrence)
    (different : observed ≠ committed) :
    projectionAccepts slotLimit lamportLimit template
      committed observed true = false := by
  simp [projectionAccepts, different]

/-- Ticket-side immutable commitment.  A Ticket never repeats or reinterprets
Product semantics; it locks the exact occurrence content ID, Market, funding
list, founders, and compartment amounts. -/
structure TicketCommitment where
  templateId : Identity
  occurrenceId : Identity
  marketId : Identity
  fundingListId : Identity
  founder : Identity
  refundOwner : Identity
  occurrence : Nat
  funds : FoundingFunds
  deriving DecidableEq, Repr

def TicketCommitment.exactFor
    (templateId occurrenceId : Identity)
    (occurrence : Occurrence) (ticket : TicketCommitment) : Bool :=
  ExecutionRelease.identityValid occurrenceId &&
  [ticket.founder, ticket.refundOwner].all ExecutionRelease.identityValid &&
  ticket.templateId = templateId && ticket.occurrenceId = occurrenceId &&
  ticket.marketId = occurrence.marketId &&
  ticket.fundingListId = occurrence.fundingListId &&
  ticket.occurrence = occurrence.occurrence && ticket.funds = occurrence.funds

theorem exact_ticket_locks_realized_product_resources
    (templateId occurrenceId : Identity)
    (occurrence : Occurrence) (left right : TicketCommitment)
    (leftExact : left.exactFor templateId occurrenceId occurrence = true)
    (rightExact : right.exactFor templateId occurrenceId occurrence = true) :
    left.marketId = right.marketId ∧
    left.fundingListId = right.fundingListId ∧
    left.funds = right.funds := by
  simp_all [TicketCommitment.exactFor]

/-- Replacing a realized Product/basis/representation changes only the
occurrence projection.  The reusable schedule and generator authority remain
unchanged. -/
theorem richer_occurrences_do_not_rewrite_template
    (template : Template) (first second : Occurrence) :
    scheduledSlot template first.occurrence =
      template.firstOccurrenceSlot + first.occurrence * template.periodSlots ∧
    scheduledSlot template second.occurrence =
      template.firstOccurrenceSlot + second.occurrence * template.periodSlots := by
  simp [scheduledSlot]

end DClutch.SeriesOccurrenceV3
