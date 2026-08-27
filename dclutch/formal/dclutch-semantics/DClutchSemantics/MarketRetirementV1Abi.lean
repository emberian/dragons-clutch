import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import DClutchSemantics.MarketCore

/-!
# Market retirement physical ABI V1

This module owns only the fixed wire boundary for the existing
`MarketCore.retireCandidate` transition.  It does not define a second
retirement transition.  The physical adapter must derive the frame's child
observations from the selected Resolution closure, a Claims-owned aggregate
closure, and the ordered normal-Custody `CloseVault`/`CloseReplay` receipts.
-/

namespace DClutch.MarketRetirementV1Abi

open DClutch
open DClutch.AbiSchema

def version : Nat := 1
def claimsRequestMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x4d, 0x43, 0x51, 0x30, 0x31]
def claimsReceiptMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x4d, 0x43, 0x43, 0x30, 0x31]
def bundleMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x52, 0x42, 0x30, 0x31]
def coreReceiptMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x52, 0x43, 0x30, 0x31]
def claimsPreResourceDigestDomain : List UInt8 :=
  "dclutch/claims-market-close-pre/v1".toUTF8.toList
def claimsPostResourceDigestDomain : List UInt8 :=
  "dclutch/claims-market-close-post/v1".toUTF8.toList
def retiredCandidateDigestDomain : List UInt8 :=
  "dclutch/core-retired-candidate/v1".toUTF8.toList
def corePostResourceDigestDomain : List UInt8 :=
  "dclutch/core-retirement-post/v1".toUTF8.toList
def claimsAction : Nat := 1
def roleCount : Nat := 3
def custodyReceiptCount : Nat := 2
def producerClosureCount : Nat := 4

/-!
`corePostResourceDigestDomain` owns the complete producer-subtree closure
commitment.  Its adapter preimage is ordered and exact:

`domain || roleCount:u8 || custodyReceiptCount:u8 || rentCredit ||
 sourceReceiptDigest || claimsReceiptDigest || closeVaultReceiptDigest ||
 closeReplayReceiptDigest || coreRefund:u64 || claimsRefund:u64 ||
 custodyRefund:u64 || rentCreditPostLamports:u64`.

Thus the one Core receipt commits every producer leaf and refund into the sole
RentCredit account.  The RentCredit contract remains the sole semantic owner of
its immutable refund wallet; a later lifecycle close authenticates that bound
account rather than copying the wallet into this ABI.
-/

inductive ClaimsRequestField where
  | magic | version | action | reservedHeader
  | releaseSet | market | aggregate | rentCredit | parentRequestDigest | coreProgram
  | generation | expectedRevision | resultingRevision | claimCount | reservedBody
  deriving DecidableEq, Repr

def claimsRequestSchema : List (FieldSpec ClaimsRequestField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.action, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.market, .bytes 32⟩,
  ⟨.aggregate, .bytes 32⟩, ⟨.rentCredit, .bytes 32⟩,
  ⟨.parentRequestDigest, .bytes 32⟩, ⟨.coreProgram, .bytes 32⟩,
  ⟨.generation, .u64⟩, ⟨.expectedRevision, .u64⟩,
  ⟨.resultingRevision, .u64⟩, ⟨.claimCount, .u32⟩,
  ⟨.reservedBody, .reserved 20⟩
]

def claimsRequestLayout : List (PlacedField ClaimsRequestField) :=
  specialize claimsRequestSchema
def claimsRequestBytes : Nat := schemaWidth claimsRequestSchema

namespace ClaimsRequestField
def offset (field : ClaimsRequestField) : Nat :=
  (coordinate? field claimsRequestLayout).map (fun value => value.1) |>.getD 0
def rustName : ClaimsRequestField → String
  | .magic => "CLAIMS_CLOSURE_REQUEST_MAGIC_OFFSET"
  | .version => "CLAIMS_CLOSURE_REQUEST_VERSION_OFFSET"
  | .action => "CLAIMS_CLOSURE_REQUEST_ACTION_OFFSET"
  | .reservedHeader => "CLAIMS_CLOSURE_REQUEST_RESERVED_HEADER_OFFSET"
  | .releaseSet => "CLAIMS_CLOSURE_REQUEST_RELEASE_SET_OFFSET"
  | .market => "CLAIMS_CLOSURE_REQUEST_MARKET_OFFSET"
  | .aggregate => "CLAIMS_CLOSURE_REQUEST_AGGREGATE_OFFSET"
  | .rentCredit => "CLAIMS_CLOSURE_REQUEST_RENT_CREDIT_OFFSET"
  | .parentRequestDigest => "CLAIMS_CLOSURE_REQUEST_PARENT_REQUEST_DIGEST_OFFSET"
  | .coreProgram => "CLAIMS_CLOSURE_REQUEST_CORE_PROGRAM_OFFSET"
  | .generation => "CLAIMS_CLOSURE_REQUEST_GENERATION_OFFSET"
  | .expectedRevision => "CLAIMS_CLOSURE_REQUEST_EXPECTED_REVISION_OFFSET"
  | .resultingRevision => "CLAIMS_CLOSURE_REQUEST_RESULTING_REVISION_OFFSET"
  | .claimCount => "CLAIMS_CLOSURE_REQUEST_CLAIM_COUNT_OFFSET"
  | .reservedBody => "CLAIMS_CLOSURE_REQUEST_RESERVED_BODY_OFFSET"
end ClaimsRequestField

inductive ClaimsReceiptField where
  | magic | version | kind | reservedHeader
  | producer | releaseSet | market | aggregate | rentCredit
  | requestDigest | preResourceDigest | postResourceDigest
  | generation | preRevision | postRevision | liabilityUnits | refundLamports
  | claimCount | reservedBody
  deriving DecidableEq, Repr

def claimsReceiptSchema : List (FieldSpec ClaimsReceiptField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.kind, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩,
  ⟨.producer, .bytes 32⟩, ⟨.releaseSet, .bytes 32⟩,
  ⟨.market, .bytes 32⟩, ⟨.aggregate, .bytes 32⟩,
  ⟨.rentCredit, .bytes 32⟩, ⟨.requestDigest, .bytes 32⟩,
  ⟨.preResourceDigest, .bytes 32⟩, ⟨.postResourceDigest, .bytes 32⟩,
  ⟨.generation, .u64⟩, ⟨.preRevision, .u64⟩,
  ⟨.postRevision, .u64⟩, ⟨.liabilityUnits, .u64⟩,
  ⟨.refundLamports, .u64⟩, ⟨.claimCount, .u32⟩,
  ⟨.reservedBody, .reserved 4⟩
]

def claimsReceiptLayout : List (PlacedField ClaimsReceiptField) :=
  specialize claimsReceiptSchema
def claimsReceiptBytes : Nat := schemaWidth claimsReceiptSchema

namespace ClaimsReceiptField
def offset (field : ClaimsReceiptField) : Nat :=
  (coordinate? field claimsReceiptLayout).map (fun value => value.1) |>.getD 0
def rustName : ClaimsReceiptField → String
  | .magic => "CLAIMS_CLOSURE_RECEIPT_MAGIC_OFFSET"
  | .version => "CLAIMS_CLOSURE_RECEIPT_VERSION_OFFSET"
  | .kind => "CLAIMS_CLOSURE_RECEIPT_KIND_OFFSET"
  | .reservedHeader => "CLAIMS_CLOSURE_RECEIPT_RESERVED_HEADER_OFFSET"
  | .producer => "CLAIMS_CLOSURE_RECEIPT_PRODUCER_OFFSET"
  | .releaseSet => "CLAIMS_CLOSURE_RECEIPT_RELEASE_SET_OFFSET"
  | .market => "CLAIMS_CLOSURE_RECEIPT_MARKET_OFFSET"
  | .aggregate => "CLAIMS_CLOSURE_RECEIPT_AGGREGATE_OFFSET"
  | .rentCredit => "CLAIMS_CLOSURE_RECEIPT_RENT_CREDIT_OFFSET"
  | .requestDigest => "CLAIMS_CLOSURE_RECEIPT_REQUEST_DIGEST_OFFSET"
  | .preResourceDigest => "CLAIMS_CLOSURE_RECEIPT_PRE_RESOURCE_DIGEST_OFFSET"
  | .postResourceDigest => "CLAIMS_CLOSURE_RECEIPT_POST_RESOURCE_DIGEST_OFFSET"
  | .generation => "CLAIMS_CLOSURE_RECEIPT_GENERATION_OFFSET"
  | .preRevision => "CLAIMS_CLOSURE_RECEIPT_PRE_REVISION_OFFSET"
  | .postRevision => "CLAIMS_CLOSURE_RECEIPT_POST_REVISION_OFFSET"
  | .liabilityUnits => "CLAIMS_CLOSURE_RECEIPT_LIABILITY_UNITS_OFFSET"
  | .refundLamports => "CLAIMS_CLOSURE_RECEIPT_REFUND_LAMPORTS_OFFSET"
  | .claimCount => "CLAIMS_CLOSURE_RECEIPT_CLAIM_COUNT_OFFSET"
  | .reservedBody => "CLAIMS_CLOSURE_RECEIPT_RESERVED_BODY_OFFSET"
end ClaimsReceiptField

inductive BundleField where
  | magic | version | roleCount | custodyReceiptCount | reservedHeader
  | market | releaseSet | rentCredit | sourceReceiptAccount | claimsAggregate
  | custodyReplay | hoardVault | sourceReceiptDigest | claimsRequestDigest
  | custodyCloseVaultRequestDigest | custodyCloseReplayRequestDigest | corePrestateDigest
  | generation | sourceClosureRevision | claimsPreRevision | claimsPostRevision
  | custodyPreRevision | custodyMiddleRevision | custodyPostRevision | expectedCoreLamports
  | reservedBody
  deriving DecidableEq, Repr

def bundleSchema : List (FieldSpec BundleField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.roleCount, .u8⟩,
  ⟨.custodyReceiptCount, .u8⟩, ⟨.reservedHeader, .reserved 4⟩,
  ⟨.market, .bytes 32⟩, ⟨.releaseSet, .bytes 32⟩,
  ⟨.rentCredit, .bytes 32⟩, ⟨.sourceReceiptAccount, .bytes 32⟩,
  ⟨.claimsAggregate, .bytes 32⟩, ⟨.custodyReplay, .bytes 32⟩,
  ⟨.hoardVault, .bytes 32⟩, ⟨.sourceReceiptDigest, .bytes 32⟩,
  ⟨.claimsRequestDigest, .bytes 32⟩,
  ⟨.custodyCloseVaultRequestDigest, .bytes 32⟩,
  ⟨.custodyCloseReplayRequestDigest, .bytes 32⟩,
  ⟨.corePrestateDigest, .bytes 32⟩,
  ⟨.generation, .u64⟩, ⟨.sourceClosureRevision, .u64⟩,
  ⟨.claimsPreRevision, .u64⟩, ⟨.claimsPostRevision, .u64⟩,
  ⟨.custodyPreRevision, .u64⟩, ⟨.custodyMiddleRevision, .u64⟩,
  ⟨.custodyPostRevision, .u64⟩, ⟨.expectedCoreLamports, .u64⟩,
  ⟨.reservedBody, .reserved 16⟩
]

def bundleLayout : List (PlacedField BundleField) := specialize bundleSchema
def bundleBytes : Nat := schemaWidth bundleSchema

namespace BundleField
def offset (field : BundleField) : Nat :=
  (coordinate? field bundleLayout).map (fun value => value.1) |>.getD 0
def rustName : BundleField → String
  | .magic => "RETIREMENT_BUNDLE_MAGIC_OFFSET"
  | .version => "RETIREMENT_BUNDLE_VERSION_OFFSET"
  | .roleCount => "RETIREMENT_BUNDLE_ROLE_COUNT_OFFSET"
  | .custodyReceiptCount => "RETIREMENT_BUNDLE_CUSTODY_RECEIPT_COUNT_OFFSET"
  | .reservedHeader => "RETIREMENT_BUNDLE_RESERVED_HEADER_OFFSET"
  | .market => "RETIREMENT_BUNDLE_MARKET_OFFSET"
  | .releaseSet => "RETIREMENT_BUNDLE_RELEASE_SET_OFFSET"
  | .rentCredit => "RETIREMENT_BUNDLE_RENT_CREDIT_OFFSET"
  | .sourceReceiptAccount => "RETIREMENT_BUNDLE_SOURCE_RECEIPT_ACCOUNT_OFFSET"
  | .claimsAggregate => "RETIREMENT_BUNDLE_CLAIMS_AGGREGATE_OFFSET"
  | .custodyReplay => "RETIREMENT_BUNDLE_CUSTODY_REPLAY_OFFSET"
  | .hoardVault => "RETIREMENT_BUNDLE_HOARD_VAULT_OFFSET"
  | .sourceReceiptDigest => "RETIREMENT_BUNDLE_SOURCE_RECEIPT_DIGEST_OFFSET"
  | .claimsRequestDigest => "RETIREMENT_BUNDLE_CLAIMS_REQUEST_DIGEST_OFFSET"
  | .custodyCloseVaultRequestDigest => "RETIREMENT_BUNDLE_CUSTODY_CLOSE_VAULT_REQUEST_DIGEST_OFFSET"
  | .custodyCloseReplayRequestDigest => "RETIREMENT_BUNDLE_CUSTODY_CLOSE_REPLAY_REQUEST_DIGEST_OFFSET"
  | .corePrestateDigest => "RETIREMENT_BUNDLE_CORE_PRESTATE_DIGEST_OFFSET"
  | .generation => "RETIREMENT_BUNDLE_GENERATION_OFFSET"
  | .sourceClosureRevision => "RETIREMENT_BUNDLE_SOURCE_CLOSURE_REVISION_OFFSET"
  | .claimsPreRevision => "RETIREMENT_BUNDLE_CLAIMS_PRE_REVISION_OFFSET"
  | .claimsPostRevision => "RETIREMENT_BUNDLE_CLAIMS_POST_REVISION_OFFSET"
  | .custodyPreRevision => "RETIREMENT_BUNDLE_CUSTODY_PRE_REVISION_OFFSET"
  | .custodyMiddleRevision => "RETIREMENT_BUNDLE_CUSTODY_MIDDLE_REVISION_OFFSET"
  | .custodyPostRevision => "RETIREMENT_BUNDLE_CUSTODY_POST_REVISION_OFFSET"
  | .expectedCoreLamports => "RETIREMENT_BUNDLE_EXPECTED_CORE_LAMPORTS_OFFSET"
  | .reservedBody => "RETIREMENT_BUNDLE_RESERVED_BODY_OFFSET"
end BundleField

inductive CoreReceiptField where
  | magic | version | kind | reservedHeader
  | coreProgram | market | releaseSet | rentCredit | bundleDigest
  | sourceReceiptDigest | claimsReceiptDigest | custodyCloseVaultReceiptDigest
  | custodyCloseReplayReceiptDigest | preStateDigest | retiredCandidateDigest
  | postResourceDigest
  | generation | sourceClosureRevision | claimsPostRevision | custodyPostRevision
  | coreRefundLamports | claimsRefundLamports | custodyRefundLamports | reservedBody
  deriving DecidableEq, Repr

def coreReceiptSchema : List (FieldSpec CoreReceiptField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.kind, .u8⟩,
  ⟨.reservedHeader, .reserved 5⟩,
  ⟨.coreProgram, .bytes 32⟩, ⟨.market, .bytes 32⟩,
  ⟨.releaseSet, .bytes 32⟩, ⟨.rentCredit, .bytes 32⟩,
  ⟨.bundleDigest, .bytes 32⟩, ⟨.sourceReceiptDigest, .bytes 32⟩,
  ⟨.claimsReceiptDigest, .bytes 32⟩,
  ⟨.custodyCloseVaultReceiptDigest, .bytes 32⟩,
  ⟨.custodyCloseReplayReceiptDigest, .bytes 32⟩,
  ⟨.preStateDigest, .bytes 32⟩, ⟨.retiredCandidateDigest, .bytes 32⟩,
  ⟨.postResourceDigest, .bytes 32⟩,
  ⟨.generation, .u64⟩, ⟨.sourceClosureRevision, .u64⟩,
  ⟨.claimsPostRevision, .u64⟩, ⟨.custodyPostRevision, .u64⟩,
  ⟨.coreRefundLamports, .u64⟩, ⟨.claimsRefundLamports, .u64⟩,
  ⟨.custodyRefundLamports, .u64⟩, ⟨.reservedBody, .reserved 56⟩
]

def coreReceiptLayout : List (PlacedField CoreReceiptField) := specialize coreReceiptSchema
def coreReceiptBytes : Nat := schemaWidth coreReceiptSchema

namespace CoreReceiptField
def offset (field : CoreReceiptField) : Nat :=
  (coordinate? field coreReceiptLayout).map (fun value => value.1) |>.getD 0
def rustName : CoreReceiptField → String
  | .magic => "RETIREMENT_RECEIPT_MAGIC_OFFSET"
  | .version => "RETIREMENT_RECEIPT_VERSION_OFFSET"
  | .kind => "RETIREMENT_RECEIPT_KIND_OFFSET"
  | .reservedHeader => "RETIREMENT_RECEIPT_RESERVED_HEADER_OFFSET"
  | .coreProgram => "RETIREMENT_RECEIPT_CORE_PROGRAM_OFFSET"
  | .market => "RETIREMENT_RECEIPT_MARKET_OFFSET"
  | .releaseSet => "RETIREMENT_RECEIPT_RELEASE_SET_OFFSET"
  | .rentCredit => "RETIREMENT_RECEIPT_RENT_CREDIT_OFFSET"
  | .bundleDigest => "RETIREMENT_RECEIPT_BUNDLE_DIGEST_OFFSET"
  | .sourceReceiptDigest => "RETIREMENT_RECEIPT_SOURCE_RECEIPT_DIGEST_OFFSET"
  | .claimsReceiptDigest => "RETIREMENT_RECEIPT_CLAIMS_RECEIPT_DIGEST_OFFSET"
  | .custodyCloseVaultReceiptDigest => "RETIREMENT_RECEIPT_CUSTODY_CLOSE_VAULT_RECEIPT_DIGEST_OFFSET"
  | .custodyCloseReplayReceiptDigest => "RETIREMENT_RECEIPT_CUSTODY_CLOSE_REPLAY_RECEIPT_DIGEST_OFFSET"
  | .preStateDigest => "RETIREMENT_RECEIPT_PRE_STATE_DIGEST_OFFSET"
  | .retiredCandidateDigest => "RETIREMENT_RECEIPT_RETIRED_CANDIDATE_DIGEST_OFFSET"
  | .postResourceDigest => "RETIREMENT_RECEIPT_POST_RESOURCE_DIGEST_OFFSET"
  | .generation => "RETIREMENT_RECEIPT_GENERATION_OFFSET"
  | .sourceClosureRevision => "RETIREMENT_RECEIPT_SOURCE_CLOSURE_REVISION_OFFSET"
  | .claimsPostRevision => "RETIREMENT_RECEIPT_CLAIMS_POST_REVISION_OFFSET"
  | .custodyPostRevision => "RETIREMENT_RECEIPT_CUSTODY_POST_REVISION_OFFSET"
  | .coreRefundLamports => "RETIREMENT_RECEIPT_CORE_REFUND_LAMPORTS_OFFSET"
  | .claimsRefundLamports => "RETIREMENT_RECEIPT_CLAIMS_REFUND_LAMPORTS_OFFSET"
  | .custodyRefundLamports => "RETIREMENT_RECEIPT_CUSTODY_REFUND_LAMPORTS_OFFSET"
  | .reservedBody => "RETIREMENT_RECEIPT_RESERVED_BODY_OFFSET"
end CoreReceiptField

theorem exact_widths :
    claimsRequestBytes = 256 ∧ claimsReceiptBytes = 320 ∧
    bundleBytes = 480 ∧ coreReceiptBytes = 512 := by
  native_decide

theorem layouts_are_disjoint :
    claimsRequestLayout.Pairwise Before ∧ claimsReceiptLayout.Pairwise Before ∧
    bundleLayout.Pairwise Before ∧ coreReceiptLayout.Pairwise Before := by
  exact ⟨specializeFrom_pairwise 0 claimsRequestSchema,
    specializeFrom_pairwise 0 claimsReceiptSchema,
    specializeFrom_pairwise 0 bundleSchema,
    specializeFrom_pairwise 0 coreReceiptSchema⟩

structure ClaimsClosureRequestV1 where
  releaseSet : Nat
  market : Nat
  aggregate : Nat
  rentCredit : Nat
  parentRequestDigest : Nat
  coreProgram : Nat
  generation : Nat
  expectedRevision : Nat
  resultingRevision : Nat
  claimCount : Nat
  deriving DecidableEq, Repr

def ClaimsClosureRequestV1.valid (value : ClaimsClosureRequestV1) : Bool :=
  [value.releaseSet, value.market, value.aggregate, value.rentCredit,
    value.parentRequestDigest, value.coreProgram].all fun identity =>
      identity != 0 && identity < 256 ^ 32 &&
  value.generation != 0 && value.generation < 256 ^ 8 &&
  value.expectedRevision < 256 ^ 8 &&
  value.resultingRevision = value.expectedRevision + 1 &&
  value.claimCount ≥ 2 && value.claimCount < 256 ^ 4

structure BundleV1 where
  identities : List Nat
  generation : Nat
  sourceClosureRevision : Nat
  claimsPreRevision : Nat
  claimsPostRevision : Nat
  custodyPreRevision : Nat
  custodyMiddleRevision : Nat
  custodyPostRevision : Nat
  expectedCoreLamports : Nat
  deriving DecidableEq, Repr

def BundleV1.valid (value : BundleV1) : Bool :=
  value.identities.length = 12 &&
  value.identities.all fun identity => identity != 0 && identity < 256 ^ 32 &&
  value.generation != 0 && value.generation < 256 ^ 8 &&
  value.sourceClosureRevision != 0 && value.sourceClosureRevision < 256 ^ 8 &&
  value.claimsPostRevision = value.claimsPreRevision + 1 &&
  value.custodyMiddleRevision = value.custodyPreRevision + 1 &&
  value.custodyPostRevision = value.custodyMiddleRevision + 1 &&
  value.expectedCoreLamports != 0 && value.expectedCoreLamports < 256 ^ 8

inductive ReceiptRoleV1 where | resolution | claims | custody
  deriving DecidableEq, Repr

structure OrderedRetirementEvidenceV1 where
  sourceRole : ReceiptRoleV1
  claimsRole : ReceiptRoleV1
  custodyVaultRole : ReceiptRoleV1
  custodyReplayRole : ReceiptRoleV1
  sourceComplete : Bool
  claimsComplete : Bool
  custodyVaultComplete : Bool
  custodyReplayComplete : Bool
  claimsAggregateEmpty : Bool
  claimsPayout : Nat
  custodyAmount : Nat
  deriving DecidableEq, Repr

def OrderedRetirementEvidenceV1.valid (value : OrderedRetirementEvidenceV1) : Bool :=
  value.sourceRole = .resolution && value.claimsRole = .claims &&
  value.custodyVaultRole = .custody && value.custodyReplayRole = .custody &&
  value.sourceComplete && value.claimsComplete && value.custodyVaultComplete &&
  value.custodyReplayComplete && value.claimsAggregateEmpty &&
  value.claimsPayout = 0 && value.custodyAmount = 0

def canonicalEvidence : OrderedRetirementEvidenceV1 := {
  sourceRole := .resolution, claimsRole := .claims,
  custodyVaultRole := .custody, custodyReplayRole := .custody,
  sourceComplete := true, claimsComplete := true,
  custodyVaultComplete := true, custodyReplayComplete := true,
  claimsAggregateEmpty := true, claimsPayout := 0, custodyAmount := 0
}

theorem canonical_evidence_valid : canonicalEvidence.valid = true := by native_decide

theorem producer_subtree_is_complete :
    producerClosureCount = 1 + 1 + custodyReceiptCount := by
  native_decide

theorem ordered_role_or_nonempty_substitution_refuses :
    ({ canonicalEvidence with claimsRole := .custody }).valid = false ∧
    ({ canonicalEvidence with custodyVaultRole := .claims }).valid = false ∧
    ({ canonicalEvidence with claimsAggregateEmpty := false }).valid = false ∧
    ({ canonicalEvidence with custodyAmount := 1 }).valid = false := by
  native_decide

end DClutch.MarketRetirementV1Abi
