import DClutchSemantics.DirectSuccessorAbi
import DClutchSemantics.TransitionVMV3
import Std.Tactic

/-!
# Ordinary Direct V3 transition program

The one authored inline-ordinary admission program. Every relation an executor
is entitled to enforce appears here; the runtime-tail fold, the exact register
schema, and the emitted bytes are all projections of this list.

Register schema notes that are semantic, not incidental:

* `makerMagic` deliberately reuses the fee-denominator coordinate. The
  denominator is a prelude constant consumed by the floor-fee division; the
  maker replay magic word is written into the same coordinate after the last
  read of it, so the bank carries both facts without a further common register.
* `maxFeeBps` is the decision-0014-D2 venue band, and the prelude compares the
  immutable config's rate against it rather than against the denominator. The
  band is what retires the `feeSole` route: `feeSoleRouteEnabled` is still
  derived, and then required zero, because `Direct.banded_fee_leaves_a_positive_
  seller_net` proves no admissible rate can enable it.
* `feeContinuationRouteEnabled` is pinned to zero. The fee leg settles in a
  second transaction (`docs/design/FEE_SECOND_TRANSACTION_V1.md`), so tx1's
  seller leg is the only Custody route this transition ever enables, and it
  stays NON-terminal whenever a fee is owed.
* the Product-owned tail carries one canonical coordinate and one Claims
  quantity per item; nothing else in the bank is Product-width.
-/

namespace DClutch.DirectOrdinaryV3

open DClutch
open DClutch.TransitionVMV3

/-- The maker replay magic word, read from the same Lean-owned ABI bytes the
record header is emitted from. -/
def makerMagicWord : Nat := DClutch.Codec.decodeLE DirectSuccessorAbi.makerMagic

/-- Ordered common scalar-register schema. Constructor order is the wire
index; the Rust constants are emitted from this typed data. -/
inductive ScalarSlot where
  | rootPhase | slot | sellerValidFrom | sellerValidThrough
  | buyerValidFrom | buyerValidThrough | sellerSide | buyerSide
  | sellerGeneration | buyerGeneration | marketGeneration | sellerOutcome
  | buyerOutcome | outcomeCount | sellerLifecycle | sellerMaximum
  | buyerLifecycle | buyerMaximum | sellerNonce | buyerNonce
  | sellerNextNonce | buyerNextNonce | sellerLimit | executionPrice
  | buyerLimit | priceScale | sellerFeeBps | buyerFeeBps
  | policyFeeBps | fill | claimsMarketRevision | sellerPositionRevision
  | buyerPositionRevision | custodyRevision | rootOpenCount | rootOpenCountAfter
  | sellerCreated | sellerBumpObservation | zero | one
  | feeDenominator | sellerNonceAfter | buyerNonceAfter | gross
  | fee | sellerNet | buyerDebit | combinedFee
  | sellerTerminalRouteEnabled | buyerRentPrincipalObservation | buyerRentPrincipal | makerVersion
  | sellerIntermediateRouteEnabled | feeNonzero | custodyAfterSeller | custodyAfterFee
  | sellerBump | sellerRentPrincipalObservation | sellerRentPrincipal | buyerCreated
  | buyerBumpObservation | buyerBump | claimTransfer | feeSoleRouteEnabled
  | makerCurrentRentMinimum | claimTailTotal | maxFeeBps
  | feeContinuationRouteEnabled
  deriving DecidableEq, Repr

namespace ScalarSlot

def all : List ScalarSlot := [
  .rootPhase, .slot, .sellerValidFrom, .sellerValidThrough,
  .buyerValidFrom, .buyerValidThrough, .sellerSide, .buyerSide,
  .sellerGeneration, .buyerGeneration, .marketGeneration, .sellerOutcome,
  .buyerOutcome, .outcomeCount, .sellerLifecycle, .sellerMaximum,
  .buyerLifecycle, .buyerMaximum, .sellerNonce, .buyerNonce,
  .sellerNextNonce, .buyerNextNonce, .sellerLimit, .executionPrice,
  .buyerLimit, .priceScale, .sellerFeeBps, .buyerFeeBps,
  .policyFeeBps, .fill, .claimsMarketRevision, .sellerPositionRevision,
  .buyerPositionRevision, .custodyRevision, .rootOpenCount, .rootOpenCountAfter,
  .sellerCreated, .sellerBumpObservation, .zero, .one,
  .feeDenominator, .sellerNonceAfter, .buyerNonceAfter, .gross,
  .fee, .sellerNet, .buyerDebit, .combinedFee,
  .sellerTerminalRouteEnabled, .buyerRentPrincipalObservation, .buyerRentPrincipal, .makerVersion,
  .sellerIntermediateRouteEnabled, .feeNonzero, .custodyAfterSeller, .custodyAfterFee,
  .sellerBump, .sellerRentPrincipalObservation, .sellerRentPrincipal, .buyerCreated,
  .buyerBumpObservation, .buyerBump, .claimTransfer, .feeSoleRouteEnabled,
  .makerCurrentRentMinimum, .claimTailTotal, .maxFeeBps,
  .feeContinuationRouteEnabled
]

@[simp] def index : ScalarSlot → Nat
  | .rootPhase => 0
  | .slot => 1
  | .sellerValidFrom => 2
  | .sellerValidThrough => 3
  | .buyerValidFrom => 4
  | .buyerValidThrough => 5
  | .sellerSide => 6
  | .buyerSide => 7
  | .sellerGeneration => 8
  | .buyerGeneration => 9
  | .marketGeneration => 10
  | .sellerOutcome => 11
  | .buyerOutcome => 12
  | .outcomeCount => 13
  | .sellerLifecycle => 14
  | .sellerMaximum => 15
  | .buyerLifecycle => 16
  | .buyerMaximum => 17
  | .sellerNonce => 18
  | .buyerNonce => 19
  | .sellerNextNonce => 20
  | .buyerNextNonce => 21
  | .sellerLimit => 22
  | .executionPrice => 23
  | .buyerLimit => 24
  | .priceScale => 25
  | .sellerFeeBps => 26
  | .buyerFeeBps => 27
  | .policyFeeBps => 28
  | .fill => 29
  | .claimsMarketRevision => 30
  | .sellerPositionRevision => 31
  | .buyerPositionRevision => 32
  | .custodyRevision => 33
  | .rootOpenCount => 34
  | .rootOpenCountAfter => 35
  | .sellerCreated => 36
  | .sellerBumpObservation => 37
  | .zero => 38
  | .one => 39
  | .feeDenominator => 40
  | .sellerNonceAfter => 41
  | .buyerNonceAfter => 42
  | .gross => 43
  | .fee => 44
  | .sellerNet => 45
  | .buyerDebit => 46
  | .combinedFee => 47
  | .sellerTerminalRouteEnabled => 48
  | .buyerRentPrincipalObservation => 49
  | .buyerRentPrincipal => 50
  | .makerVersion => 51
  | .sellerIntermediateRouteEnabled => 52
  | .feeNonzero => 53
  | .custodyAfterSeller => 54
  | .custodyAfterFee => 55
  | .sellerBump => 56
  | .sellerRentPrincipalObservation => 57
  | .sellerRentPrincipal => 58
  | .buyerCreated => 59
  | .buyerBumpObservation => 60
  | .buyerBump => 61
  | .claimTransfer => 62
  | .feeSoleRouteEnabled => 63
  | .makerCurrentRentMinimum => 64
  | .claimTailTotal => 65
  | .maxFeeBps => 66
  | .feeContinuationRouteEnabled => 67

def rustName : ScalarSlot → String
  | .rootPhase => "SCALAR_ROOT_PHASE_V3"
  | .slot => "SCALAR_SLOT_V3"
  | .sellerValidFrom => "SCALAR_SELLER_VALID_FROM_V3"
  | .sellerValidThrough => "SCALAR_SELLER_VALID_THROUGH_V3"
  | .buyerValidFrom => "SCALAR_BUYER_VALID_FROM_V3"
  | .buyerValidThrough => "SCALAR_BUYER_VALID_THROUGH_V3"
  | .sellerSide => "SCALAR_SELLER_SIDE_V3"
  | .buyerSide => "SCALAR_BUYER_SIDE_V3"
  | .sellerGeneration => "SCALAR_SELLER_GENERATION_V3"
  | .buyerGeneration => "SCALAR_BUYER_GENERATION_V3"
  | .marketGeneration => "SCALAR_MARKET_GENERATION_V3"
  | .sellerOutcome => "SCALAR_SELLER_OUTCOME_V3"
  | .buyerOutcome => "SCALAR_BUYER_OUTCOME_V3"
  | .outcomeCount => "SCALAR_OUTCOME_COUNT_V3"
  | .sellerLifecycle => "SCALAR_SELLER_LIFECYCLE_V3"
  | .sellerMaximum => "SCALAR_SELLER_MAXIMUM_V3"
  | .buyerLifecycle => "SCALAR_BUYER_LIFECYCLE_V3"
  | .buyerMaximum => "SCALAR_BUYER_MAXIMUM_V3"
  | .sellerNonce => "SCALAR_SELLER_NONCE_V3"
  | .buyerNonce => "SCALAR_BUYER_NONCE_V3"
  | .sellerNextNonce => "SCALAR_SELLER_NEXT_NONCE_V3"
  | .buyerNextNonce => "SCALAR_BUYER_NEXT_NONCE_V3"
  | .sellerLimit => "SCALAR_SELLER_LIMIT_V3"
  | .executionPrice => "SCALAR_EXECUTION_PRICE_V3"
  | .buyerLimit => "SCALAR_BUYER_LIMIT_V3"
  | .priceScale => "SCALAR_PRICE_SCALE_V3"
  | .sellerFeeBps => "SCALAR_SELLER_FEE_BPS_V3"
  | .buyerFeeBps => "SCALAR_BUYER_FEE_BPS_V3"
  | .policyFeeBps => "SCALAR_POLICY_FEE_BPS_V3"
  | .fill => "SCALAR_FILL_V3"
  | .claimsMarketRevision => "SCALAR_CLAIMS_MARKET_REVISION_V3"
  | .sellerPositionRevision => "SCALAR_SELLER_POSITION_REVISION_V3"
  | .buyerPositionRevision => "SCALAR_BUYER_POSITION_REVISION_V3"
  | .custodyRevision => "SCALAR_CUSTODY_REVISION_V3"
  | .rootOpenCount => "SCALAR_ROOT_OPEN_COUNT_V3"
  | .rootOpenCountAfter => "SCALAR_ROOT_OPEN_COUNT_AFTER_V3"
  | .sellerCreated => "SCALAR_SELLER_CREATED_V3"
  | .sellerBumpObservation => "SCALAR_SELLER_BUMP_OBSERVATION_V3"
  | .zero => "SCALAR_ZERO_V3"
  | .one => "SCALAR_ONE_V3"
  | .feeDenominator => "SCALAR_FEE_DENOMINATOR_V3"
  | .sellerNonceAfter => "SCALAR_SELLER_NONCE_AFTER_V3"
  | .buyerNonceAfter => "SCALAR_BUYER_NONCE_AFTER_V3"
  | .gross => "SCALAR_GROSS_V3"
  | .fee => "SCALAR_FEE_V3"
  | .sellerNet => "SCALAR_SELLER_NET_V3"
  | .buyerDebit => "SCALAR_BUYER_DEBIT_V3"
  | .combinedFee => "SCALAR_COMBINED_FEE_V3"
  | .sellerTerminalRouteEnabled => "SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V3"
  | .buyerRentPrincipalObservation => "SCALAR_BUYER_RENT_PRINCIPAL_OBSERVATION_V3"
  | .buyerRentPrincipal => "SCALAR_BUYER_RENT_PRINCIPAL_V3"
  | .makerVersion => "SCALAR_MAKER_VERSION_V3"
  | .sellerIntermediateRouteEnabled => "SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V3"
  | .feeNonzero => "SCALAR_FEE_NONZERO_V3"
  | .custodyAfterSeller => "SCALAR_CUSTODY_AFTER_SELLER_V3"
  | .custodyAfterFee => "SCALAR_CUSTODY_AFTER_FEE_V3"
  | .sellerBump => "SCALAR_SELLER_BUMP_V3"
  | .sellerRentPrincipalObservation => "SCALAR_SELLER_RENT_PRINCIPAL_OBSERVATION_V3"
  | .sellerRentPrincipal => "SCALAR_SELLER_RENT_PRINCIPAL_V3"
  | .buyerCreated => "SCALAR_BUYER_CREATED_V3"
  | .buyerBumpObservation => "SCALAR_BUYER_BUMP_OBSERVATION_V3"
  | .buyerBump => "SCALAR_BUYER_BUMP_V3"
  | .claimTransfer => "SCALAR_CLAIM_TRANSFER_V3"
  | .feeSoleRouteEnabled => "SCALAR_FEE_SOLE_ROUTE_ENABLED_V3"
  | .makerCurrentRentMinimum => "SCALAR_MAKER_CURRENT_RENT_MINIMUM_V5"
  | .claimTailTotal => "SCALAR_CLAIM_TAIL_TOTAL_V3"
  | .maxFeeBps => "SCALAR_MAX_FEE_BPS_V3"
  | .feeContinuationRouteEnabled => "SCALAR_FEE_CONTINUATION_ROUTE_ENABLED_V3"

/-- Emitted Rust documentation for this coordinate. -/
def doc : ScalarSlot → String
  | .rootPhase => "Scalar register: Direct root phase (`Open = 0`)."
  | .slot => "Scalar register: trusted Clock slot."
  | .sellerValidFrom => "Scalar register: seller inclusive validity start."
  | .sellerValidThrough => "Scalar register: seller inclusive validity end."
  | .buyerValidFrom => "Scalar register: buyer inclusive validity start."
  | .buyerValidThrough => "Scalar register: buyer inclusive validity end."
  | .sellerSide => "Scalar register: seller side tag."
  | .buyerSide => "Scalar register: buyer side tag."
  | .sellerGeneration => "Scalar register: seller generation."
  | .buyerGeneration => "Scalar register: buyer generation."
  | .marketGeneration => "Scalar register: authenticated Core Market generation."
  | .sellerOutcome => "Scalar register: seller Product outcome coordinate."
  | .buyerOutcome => "Scalar register: buyer Product outcome coordinate."
  | .outcomeCount => "Scalar register: authenticated Product runtime outcome count."
  | .sellerLifecycle => "Scalar register: seller inline lifecycle tag."
  | .sellerMaximum => "Scalar register: seller maximum fill."
  | .buyerLifecycle => "Scalar register: buyer inline lifecycle tag."
  | .buyerMaximum => "Scalar register: buyer maximum fill."
  | .sellerNonce => "Scalar register: seller signed nonce."
  | .buyerNonce => "Scalar register: buyer signed nonce."
  | .sellerNextNonce => "Scalar register: seller replay next nonce."
  | .buyerNextNonce => "Scalar register: buyer replay next nonce."
  | .sellerLimit => "Scalar register: seller minimum price."
  | .executionPrice => "Scalar register: matcher execution price."
  | .buyerLimit => "Scalar register: buyer maximum price."
  | .priceScale => "Scalar register: immutable config price scale."
  | .sellerFeeBps => "Scalar register: seller-signed fee basis points."
  | .buyerFeeBps => "Scalar register: buyer-signed fee basis points."
  | .policyFeeBps => "Scalar register: immutable config fee basis points."
  | .fill => "Scalar register: positive matcher-selected fill."
  | .claimsMarketRevision => "Scalar register: Claims aggregate pre-revision."
  | .sellerPositionRevision => "Scalar register: seller Position pre-revision."
  | .buyerPositionRevision => "Scalar register: buyer Position pre-revision."
  | .custodyRevision => "Scalar register: Custody replay pre-revision."
  | .rootOpenCount => "Scalar register: exact pre-transition open-maker-root count."
  | .rootOpenCountAfter => "Scalar register: exact post-transition open-maker-root count."
  | .sellerCreated => "Scalar register: lifecycle-owned seller first-use bit."
  | .sellerBumpObservation => "Scalar register: seller persisted bump observation."
  | .zero => "Program-owned zero constant."
  | .one => "Program-owned one constant."
  | .feeDenominator => "Program-owned basis-point denominator."
  | .sellerNonceAfter => "Derived seller successor nonce."
  | .buyerNonceAfter => "Derived buyer successor nonce."
  | .gross => "Derived exact gross collateral."
  | .fee => "Derived one-side cumulative floor fee."
  | .sellerNet => "Derived seller-net collateral transfer."
  | .buyerDebit => "Derived total buyer collateral debit."
  | .combinedFee => "Derived combined seller-plus-buyer fee transfer."
  | .sellerTerminalRouteEnabled => "Derived terminal seller-only Custody route enable bit."
  | .buyerRentPrincipalObservation => "Buyer persisted historical-rent-principal observation."
  | .buyerRentPrincipal => "Lifecycle-owned buyer historical rent principal."
  | .makerVersion => "Program-owned maker replay ABI version after Transition."
  | .sellerIntermediateRouteEnabled => "Derived non-terminal seller-net Custody route enable bit."
  | .feeNonzero => "Derived nonzero combined-fee bit."
  | .custodyAfterSeller => "Reserved for the replay revision after seller-net."
  | .custodyAfterFee => "Reserved for the replay revision after combined fee."
  | .sellerBump => "Lifecycle-owned seller canonical bump."
  | .sellerRentPrincipalObservation => "Seller persisted historical-rent-principal observation."
  | .sellerRentPrincipal => "Lifecycle-owned seller historical rent principal."
  | .buyerCreated => "Scalar register: lifecycle-owned buyer first-use bit."
  | .buyerBumpObservation => "Scalar register: buyer persisted bump observation."
  | .buyerBump => "Lifecycle-owned buyer canonical bump."
  | .claimTransfer => "Reserved for exact Claims transfer quantity."
  | .feeSoleRouteEnabled => "Derived terminal fee-only Custody route enable bit."
  | .makerCurrentRentMinimum => "Lifecycle V5 adapter-authenticated current Rent minimum for a 152-byte maker root."
  | .claimTailTotal => "Program-owned total of the Claims quantities written across the Product tail."
  | .maxFeeBps => "Program-owned venue fee band (decision 0014 D2), in basis points."
  | .feeContinuationRouteEnabled => "Fee-continuation Custody route enable bit, program-pinned to zero: the fee leg settles in a second transaction."

/-- The maker replay magic word is written into the fee-denominator coordinate
after the floor-fee division has consumed it. The reuse is deliberate and the
alias is emitted so no Rust caller has to know the coincidence. -/
def makerMagic : ScalarSlot := .feeDenominator

/-- Emitted Rust aliases: a name, and the slot whose index it repeats. -/
def aliases : List (String × String × ScalarSlot) :=
  [("SCALAR_MAKER_MAGIC_V3",
    "Program-owned maker replay magic word after fee arithmetic completes.",
    makerMagic)]

end ScalarSlot

/-- Ordered common identity-register schema. -/
inductive IdentitySlot where
  | parentRequestDigest | sellerRentBeneficiary | feeRecipient
  | market | sellerNativeSigner | buyerNativeSigner
  | sellerRequestMaker | buyerRequestMaker | sellerIntentMarket
  | buyerIntentMarket | releaseSet | productRecordDigest
  | semanticBasis | linkedBasisRecord | tradingProgram
  | sellerStateOwner | buyerStateOwner | realm
  | mint | tokenProgram | buyerRentBeneficiary
  | sellerMakerRoot | buyerMakerRoot | systemProgram
  | custodyAuthority | sellerRentBeneficiaryObservation | buyerRentBeneficiaryObservation
  | feeTokenAccount | sellerCollateralRequest | buyerCollateralRequest
  | sellerTokenAccount | buyerTokenAccount
  deriving DecidableEq, Repr

namespace IdentitySlot

def all : List IdentitySlot := [
  .parentRequestDigest, .sellerRentBeneficiary, .feeRecipient,
  .market, .sellerNativeSigner, .buyerNativeSigner,
  .sellerRequestMaker, .buyerRequestMaker, .sellerIntentMarket,
  .buyerIntentMarket, .releaseSet, .productRecordDigest,
  .semanticBasis, .linkedBasisRecord, .tradingProgram,
  .sellerStateOwner, .buyerStateOwner, .realm,
  .mint, .tokenProgram, .buyerRentBeneficiary,
  .sellerMakerRoot, .buyerMakerRoot, .systemProgram,
  .custodyAuthority, .sellerRentBeneficiaryObservation, .buyerRentBeneficiaryObservation,
  .feeTokenAccount, .sellerCollateralRequest, .buyerCollateralRequest,
  .sellerTokenAccount, .buyerTokenAccount
]

@[simp] def index : IdentitySlot → Nat
  | .parentRequestDigest => 0
  | .sellerRentBeneficiary => 1
  | .feeRecipient => 2
  | .market => 3
  | .sellerNativeSigner => 4
  | .buyerNativeSigner => 5
  | .sellerRequestMaker => 6
  | .buyerRequestMaker => 7
  | .sellerIntentMarket => 8
  | .buyerIntentMarket => 9
  | .releaseSet => 10
  | .productRecordDigest => 11
  | .semanticBasis => 12
  | .linkedBasisRecord => 13
  | .tradingProgram => 14
  | .sellerStateOwner => 15
  | .buyerStateOwner => 16
  | .realm => 17
  | .mint => 18
  | .tokenProgram => 19
  | .buyerRentBeneficiary => 20
  | .sellerMakerRoot => 21
  | .buyerMakerRoot => 22
  | .systemProgram => 23
  | .custodyAuthority => 24
  | .sellerRentBeneficiaryObservation => 25
  | .buyerRentBeneficiaryObservation => 26
  | .feeTokenAccount => 27
  | .sellerCollateralRequest => 28
  | .buyerCollateralRequest => 29
  | .sellerTokenAccount => 30
  | .buyerTokenAccount => 31

def rustName : IdentitySlot → String
  | .parentRequestDigest => "IDENTITY_PARENT_REQUEST_DIGEST_V3"
  | .sellerRentBeneficiary => "IDENTITY_SELLER_RENT_BENEFICIARY_V3"
  | .feeRecipient => "IDENTITY_FEE_RECIPIENT_V3"
  | .market => "IDENTITY_MARKET_V3"
  | .sellerNativeSigner => "IDENTITY_SELLER_NATIVE_SIGNER_V3"
  | .buyerNativeSigner => "IDENTITY_BUYER_NATIVE_SIGNER_V3"
  | .sellerRequestMaker => "IDENTITY_SELLER_REQUEST_MAKER_V3"
  | .buyerRequestMaker => "IDENTITY_BUYER_REQUEST_MAKER_V3"
  | .sellerIntentMarket => "IDENTITY_SELLER_INTENT_MARKET_V3"
  | .buyerIntentMarket => "IDENTITY_BUYER_INTENT_MARKET_V3"
  | .releaseSet => "IDENTITY_RELEASE_SET_V3"
  | .productRecordDigest => "IDENTITY_PRODUCT_RECORD_DIGEST_V3"
  | .semanticBasis => "IDENTITY_SEMANTIC_BASIS_V3"
  | .linkedBasisRecord => "IDENTITY_LINKED_BASIS_RECORD_V3"
  | .tradingProgram => "IDENTITY_TRADING_PROGRAM_V3"
  | .sellerStateOwner => "IDENTITY_SELLER_STATE_OWNER_V3"
  | .buyerStateOwner => "IDENTITY_BUYER_STATE_OWNER_V3"
  | .realm => "IDENTITY_REALM_V3"
  | .mint => "IDENTITY_MINT_V3"
  | .tokenProgram => "IDENTITY_TOKEN_PROGRAM_V3"
  | .buyerRentBeneficiary => "IDENTITY_BUYER_RENT_BENEFICIARY_V3"
  | .sellerMakerRoot => "IDENTITY_SELLER_MAKER_ROOT_V3"
  | .buyerMakerRoot => "IDENTITY_BUYER_MAKER_ROOT_V3"
  | .systemProgram => "IDENTITY_SYSTEM_PROGRAM_V3"
  | .custodyAuthority => "IDENTITY_CUSTODY_AUTHORITY_V3"
  | .sellerRentBeneficiaryObservation => "IDENTITY_SELLER_RENT_BENEFICIARY_OBSERVATION_V3"
  | .buyerRentBeneficiaryObservation => "IDENTITY_BUYER_RENT_BENEFICIARY_OBSERVATION_V3"
  | .feeTokenAccount => "IDENTITY_FEE_TOKEN_ACCOUNT_V3"
  | .sellerCollateralRequest => "IDENTITY_SELLER_COLLATERAL_REQUEST_V3"
  | .buyerCollateralRequest => "IDENTITY_BUYER_COLLATERAL_REQUEST_V3"
  | .sellerTokenAccount => "IDENTITY_SELLER_TOKEN_ACCOUNT_V3"
  | .buyerTokenAccount => "IDENTITY_BUYER_TOKEN_ACCOUNT_V3"

/-- Emitted Rust documentation for this coordinate. -/
def doc : IdentitySlot → String
  | .parentRequestDigest => "Identity register: SHA-256 of the complete family request."
  | .sellerRentBeneficiary => "Lifecycle-owned seller immutable rent beneficiary."
  | .feeRecipient => "Identity register: immutable config fee recipient."
  | .market => "Identity register: authenticated logical Core Market."
  | .sellerNativeSigner => "Identity register: native-Ed25519 seller signer."
  | .buyerNativeSigner => "Identity register: native-Ed25519 buyer signer."
  | .sellerRequestMaker => "Identity register: request-carried seller maker."
  | .buyerRequestMaker => "Identity register: request-carried buyer maker."
  | .sellerIntentMarket => "Identity register: seller signed-intent Market."
  | .buyerIntentMarket => "Identity register: buyer signed-intent Market."
  | .releaseSet => "Identity register: immutable execution release set."
  | .productRecordDigest => "Identity register: finalized Product record digest."
  | .semanticBasis => "Identity register: semantic LiabilityBasis identity."
  | .linkedBasisRecord => "Identity register: finalized linked-basis record digest."
  | .tradingProgram => "Identity register: current Registry-selected Trading program."
  | .sellerStateOwner => "Lifecycle-owned seller state owner, equal to current Trading."
  | .buyerStateOwner => "Lifecycle-owned buyer state owner, equal to current Trading."
  | .realm => "Identity register: immutable Realm record identity."
  | .mint => "Identity register: Realm-selected collateral mint."
  | .tokenProgram => "Identity register: Realm-selected token program."
  | .buyerRentBeneficiary => "Lifecycle-owned buyer immutable rent beneficiary."
  | .sellerMakerRoot => "Lifecycle-owned seller maker replay root."
  | .buyerMakerRoot => "Lifecycle-owned buyer maker replay root and Custody context."
  | .systemProgram => "Identity register: independently observed System Program."
  | .custodyAuthority => "Identity register: canonical Custody transfer authority."
  | .sellerRentBeneficiaryObservation => "Seller persisted rent-beneficiary observation."
  | .buyerRentBeneficiaryObservation => "Buyer persisted rent-beneficiary observation."
  | .feeTokenAccount => "Identity register: fee recipient's exact collateral token account."
  | .sellerCollateralRequest => "Identity register: seller-signed collateral token account."
  | .buyerCollateralRequest => "Identity register: buyer-signed collateral token account."
  | .sellerTokenAccount => "Identity register: authenticated seller destination token account."
  | .buyerTokenAccount => "Identity register: authenticated buyer source token account."

end IdentitySlot

/-- Per-Product-item scalar stride. -/
inductive ItemScalarSlot where
  | outcomeCoordinate | claimQuantity
  deriving DecidableEq, Repr

namespace ItemScalarSlot

def all : List ItemScalarSlot := [
  .outcomeCoordinate, .claimQuantity
]

@[simp] def index : ItemScalarSlot → Nat
  | .outcomeCoordinate => 0
  | .claimQuantity => 1

def rustName : ItemScalarSlot → String
  | .outcomeCoordinate => "ITEM_SCALAR_INDEX_V3"
  | .claimQuantity => "ITEM_SCALAR_CLAIM_QUANTITY_V3"

/-- Emitted Rust documentation for this coordinate. -/
def doc : ItemScalarSlot → String
  | .outcomeCoordinate => "Per-item scalar slot containing the canonical Product item index."
  | .claimQuantity => "Per-item scalar slot containing the exact Claims transfer quantity."

end ItemScalarSlot

/-- Common scalar coordinate. -/
def s (register : ScalarSlot) : Reg := common register.index

/-- Common identity coordinate. -/
def d (register : IdentitySlot) : Reg := common register.index

/-- Per-item scalar coordinate. -/
def t (register : ItemScalarSlot) : Reg := item register.index

/-- Exact common scalar-bank width. -/
def commonScalars : Nat := ScalarSlot.all.length

/-- Exact common identity-bank width. -/
def commonIdentities : Nat := IdentitySlot.all.length

/-- Exact per-Product-item scalar stride. -/
def itemScalarStride : Nat := ItemScalarSlot.all.length

/-- The fixed admission and derivation prelude. -/
def preludeOps : List Op := [
  .loadConst (s .zero) 0,
  .loadConst (s .one) 1,
  .loadConst (s .feeDenominator) DClutch.Direct.feeDenominator,
  .loadConst (s .maxFeeBps) DClutch.Direct.maxFeeBasisPoints,
  .scalarEq (s .rootPhase) (s .zero),
  .nonzero (s .fill),
  .scalarLe (s .sellerValidFrom) (s .slot),
  .scalarLe (s .slot) (s .sellerValidThrough),
  .scalarLe (s .buyerValidFrom) (s .slot),
  .scalarLe (s .slot) (s .buyerValidThrough),
  .scalarEq (s .sellerSide) (s .zero),
  .scalarEq (s .buyerSide) (s .one),
  .identityEq (d .sellerIntentMarket) (d .buyerIntentMarket),
  .identityEq (d .sellerIntentMarket) (d .market),
  .scalarEq (s .sellerGeneration) (s .buyerGeneration),
  .scalarEq (s .sellerGeneration) (s .marketGeneration),
  .scalarEq (s .sellerOutcome) (s .buyerOutcome),
  .identityEq (d .sellerNativeSigner) (d .sellerRequestMaker),
  .identityEq (d .buyerNativeSigner) (d .buyerRequestMaker),
  .identityNe (d .sellerRequestMaker) (d .buyerRequestMaker),
  .identityEq (d .sellerCollateralRequest) (d .sellerTokenAccount),
  .identityEq (d .buyerCollateralRequest) (d .buyerTokenAccount),
  .scalarLt (s .sellerOutcome) (s .outcomeCount),
  .nonzero (s .priceScale),
  .lifecycleAccepts (s .sellerLifecycle) (s .sellerMaximum) (s .fill),
  .lifecycleAccepts (s .buyerLifecycle) (s .buyerMaximum) (s .fill),
  .scalarEq (s .sellerNonce) (s .sellerNextNonce),
  .scalarEq (s .buyerNonce) (s .buyerNextNonce),
  .incrementInto (s .sellerNextNonce) (s .sellerNonceAfter),
  .incrementInto (s .buyerNextNonce) (s .buyerNonceAfter),
  .scalarLe (s .sellerLimit) (s .executionPrice),
  .scalarLe (s .executionPrice) (s .buyerLimit),
  .scalarLe (s .executionPrice) (s .priceScale),
  .scalarEq (s .sellerFeeBps) (s .policyFeeBps),
  .scalarEq (s .buyerFeeBps) (s .policyFeeBps),
  .scalarLe (s .policyFeeBps) (s .maxFeeBps),
  .mulDivExact (s .fill) (s .executionPrice) (s .priceScale) (s .gross),
  .mulDivFloor (s .gross) (s .policyFeeBps) (s .feeDenominator) (s .fee),
  .subInto (s .gross) (s .fee) (s .sellerNet),
  .checkedAddInto (s .gross) (s .fee) (s .buyerDebit),
  .checkedAddInto (s .fee) (s .fee) (s .combinedFee),
  .checkedAddInto (s .sellerNet) (s .combinedFee) (s .sellerTerminalRouteEnabled),
  .scalarEq (s .sellerTerminalRouteEnabled) (s .buyerDebit),
  .scalarLe (s .sellerCreated) (s .one),
  .scalarLe (s .buyerCreated) (s .one),
  .checkedAddInto (s .rootOpenCount) (s .sellerCreated) (s .rootOpenCountAfter),
  .checkedAddInto (s .rootOpenCountAfter) (s .buyerCreated) (s .rootOpenCountAfter),
  .identityEq (d .sellerStateOwner) (d .tradingProgram),
  .identityEq (d .buyerStateOwner) (d .tradingProgram),
  .loadConst (s .sellerIntermediateRouteEnabled) 1,
  .selectZero (s .sellerNet) (s .zero) (s .sellerIntermediateRouteEnabled),
  .loadConst (s .feeNonzero) 1,
  .selectZero (s .combinedFee) (s .zero) (s .feeNonzero),
  .loadConst (s .sellerTerminalRouteEnabled) 0,
  .selectZero (s .combinedFee) (s .sellerIntermediateRouteEnabled) (s .sellerTerminalRouteEnabled),
  .checkedAddInto (s .feeNonzero) (s .zero) (s .sellerIntermediateRouteEnabled),
  .selectZero (s .sellerNet) (s .zero) (s .sellerIntermediateRouteEnabled),
  .loadConst (s .feeSoleRouteEnabled) 0,
  .selectZero (s .sellerNet) (s .feeNonzero) (s .feeSoleRouteEnabled),
  .scalarEq (s .feeSoleRouteEnabled) (s .zero),
  .loadConst (s .feeContinuationRouteEnabled) 0,
  .checkedAddInto (s .sellerTerminalRouteEnabled) (s .sellerIntermediateRouteEnabled) (s .custodyAfterSeller),
  .checkedAddInto (s .custodyRevision) (s .custodyAfterSeller) (s .custodyAfterSeller),
  .checkedAddInto (s .custodyAfterSeller) (s .feeContinuationRouteEnabled) (s .custodyAfterFee),
  .checkedAddInto (s .custodyAfterFee) (s .feeSoleRouteEnabled) (s .custodyAfterFee),
  .checkedAddInto (s .fill) (s .zero) (s .claimTransfer),
  .loadConst (s .claimTailTotal) 0,
  .loadConst (s .makerVersion) DirectSuccessorAbi.version,
  .loadConst (s ScalarSlot.makerMagic) makerMagicWord
]

/-- The per-Product-item body, folded once per tail coordinate. -/
def itemOps : List Op := [
  .loadConst (t .claimQuantity) 0,
  .selectEq (t .outcomeCoordinate) (s .sellerOutcome) (s .claimTransfer) (t .claimQuantity),
  .checkedAddInto (s .claimTailTotal) (t .claimQuantity) (s .claimTailTotal)
]

/-- The closing checks, run once after the whole tail has folded.

The Claims quantities the fold wrote across the Product tail must sum to exactly
the quantity the transition transferred. Because `fill` is required nonzero and
`claimTransfer` copies it, this holds only when exactly one item coordinate
carries the traded outcome: a traded outcome outside the Product tail sums to
zero, and a tail that repeats a coordinate sums to a multiple. -/
def epilogueOps : List Op := [
  .scalarEq (s .claimTailTotal) (s .claimTransfer)
]

/-- The one authored ordinary Direct V3 transition program. -/
def program : Program := {
  commonScalars := commonScalars
  itemScalarStride := itemScalarStride
  commonIdentities := commonIdentities
  itemIdentityStride := 0
  «prelude» := preludeOps
  itemBody := itemOps
  epilogue := epilogueOps
}

theorem well_formed : program.wellFormed = true := by native_decide

theorem prelude_count : program.prelude.length = 69 := by native_decide

theorem item_count : program.itemBody.length = 3 := by native_decide

theorem epilogue_count : program.epilogue.length = 1 := by native_decide

theorem common_scalar_count : program.commonScalars = 68 := by native_decide

theorem common_identity_count : program.commonIdentities = 32 := by native_decide

theorem encoded_width : (Codec.encodeProgram program).length = 1784 := by native_decide

/-! ## Witnesses

Concrete banks the program admits and refuses. These are decided executions of
the authored program, not of any executor: a Rust translation that disagrees
with one of them is wrong about the program, not about its own arithmetic. -/

namespace Witness

/-- Assignments of the canonical admitted frame: a signed fill of ten at an
execution price of fifty against a scale of one hundred, a hundred-basis-point
venue rate all three parties signed, and a three-outcome Product. -/
def canonicalScalars : List (ScalarSlot × Nat) := [
  (.slot, 100), (.sellerValidFrom, 90), (.sellerValidThrough, 110),
  (.buyerValidFrom, 90), (.buyerValidThrough, 110), (.buyerSide, 1),
  (.sellerGeneration, 7), (.buyerGeneration, 7), (.marketGeneration, 7),
  (.sellerOutcome, 1), (.buyerOutcome, 1), (.outcomeCount, 3),
  (.sellerLifecycle, 1), (.sellerMaximum, 10),
  (.buyerLifecycle, 1), (.buyerMaximum, 10),
  (.sellerNonce, 1), (.buyerNonce, 2),
  (.sellerNextNonce, 1), (.buyerNextNonce, 2),
  (.sellerLimit, 40), (.executionPrice, 50), (.buyerLimit, 60),
  (.priceScale, 100), (.sellerFeeBps, 100), (.buyerFeeBps, 100),
  (.policyFeeBps, 100), (.fill, 10), (.custodyRevision, 3), (.rootOpenCount, 2)
]

/-- The canonical input bank at one tail count. Every Product item carries its
own canonical coordinate, which is what the projection writes. -/
def scalars (tailCount : Nat) (overrides : List (ScalarSlot × Nat) := []) : Array Nat :=
  let empty : Array Nat := Array.replicate (program.scalarWidth tailCount) 0
  let common := (canonicalScalars ++ overrides).foldl
    (fun bank (assignment : ScalarSlot × Nat) =>
      bank.setIfInBounds assignment.1.index assignment.2) empty
  (List.range tailCount).foldl
    (fun bank ordinal =>
      bank.setIfInBounds
        (program.commonScalars + ordinal * program.itemScalarStride +
          ItemScalarSlot.index .outcomeCoordinate)
        ordinal)
    common

/-- The canonical identity bank: distinct makers, each bound to its own native
signer and collateral account, both intents on the Market, both live states
owned by the Trading program. -/
def identities : Array Nat :=
  let assignments : List (IdentitySlot × Nat) := [
    (.market, 2), (.sellerIntentMarket, 2), (.buyerIntentMarket, 2),
    (.sellerNativeSigner, 3), (.sellerRequestMaker, 3),
    (.buyerNativeSigner, 4), (.buyerRequestMaker, 4),
    (.sellerCollateralRequest, 5), (.sellerTokenAccount, 5),
    (.buyerCollateralRequest, 6), (.buyerTokenAccount, 6),
    (.tradingProgram, 7), (.sellerStateOwner, 7), (.buyerStateOwner, 7)
  ]
  assignments.foldl
    (fun bank (assignment : IdentitySlot × Nat) =>
      bank.setIfInBounds assignment.1.index assignment.2)
    (Array.replicate program.commonIdentities 1)

/-- The smallest overrides of the canonical frame that carry a nonzero floor
fee inside the band: forty at fifty of a hundred is a gross of twenty, and five
percent of twenty is exactly one per side. -/
def feeBearing : List (ScalarSlot × Nat) :=
  [(.fill, 40), (.sellerMaximum, 40), (.buyerMaximum, 40),
    (.sellerFeeBps, 500), (.buyerFeeBps, 500), (.policyFeeBps, 500)]

/-- Read every Custody route enable bit out of an admitted result, with the
replay revision those routes advance it to. -/
def routes (result : Option TransitionVMV3.State) : Option (Nat × Nat × Nat × Nat) :=
  result.map fun state =>
    (state.scalars[ScalarSlot.index .sellerIntermediateRouteEnabled]!,
      state.scalars[ScalarSlot.index .feeContinuationRouteEnabled]!,
      state.scalars[ScalarSlot.index .feeSoleRouteEnabled]!,
      state.scalars[ScalarSlot.index .custodyAfterFee]!)

/-- Read the fee arithmetic out of an admitted result. -/
def ledger (result : Option TransitionVMV3.State) : Option (Nat × Nat × Nat) :=
  result.map fun state =>
    (state.scalars[ScalarSlot.index .fee]!,
      state.scalars[ScalarSlot.index .sellerNet]!,
      state.scalars[ScalarSlot.index .combinedFee]!)

/-- Read three derived coordinates out of an admitted result. -/
def quote (result : Option TransitionVMV3.State) : Option (Nat × Nat × Nat) :=
  result.map fun state =>
    (state.scalars[ScalarSlot.index .gross]!,
      state.scalars[ScalarSlot.index .fee]!,
      state.scalars[ScalarSlot.index .claimTailTotal]!)

end Witness

open Witness in
/-- The canonical frame is admitted, and the Claims quantities the fold wrote
across the tail sum to the transferred quantity. -/
theorem canonical_frame_admits :
    quote (program.execute 3 ⟨scalars 3, identities⟩) = some (5, 0, 10) := by
  native_decide

open Witness in
/-- The divergence this clause closes. With an authenticated outcome count of
five, a traded outcome of four, and a Product tail of three, no item carries the
traded outcome: the fold writes nothing, and the epilogue refuses a fill whose
Claims quantities are all zero. -/
theorem a_traded_outcome_outside_the_product_tail_refuses :
    program.execute 3
        ⟨scalars 3 [(.outcomeCount, 5), (.sellerOutcome, 4), (.buyerOutcome, 4)],
          identities⟩ = none := by
  native_decide

open Witness in
/-- An empty Product tail is the same refusal: there is nowhere for the
transferred Claims to land. -/
theorem an_empty_product_tail_refuses :
    program.execute 0 ⟨scalars 0, identities⟩ = none := by native_decide

open Witness in
/-- The band's own edge is admitted. Five percent on a gross of five floors to
a fee of zero at this witness's scale, which is exactly the arithmetic the
band leaves reachable. -/
theorem a_venue_rate_at_the_band_admits :
    quote (program.execute 3
        ⟨scalars 3
            [(.sellerFeeBps, 500), (.buyerFeeBps, 500), (.policyFeeBps, 500)],
          identities⟩) = some (5, 0, 10) := by
  native_decide

open Witness in
/-- One basis point above the band is refused. This is decision 0014 D2 as a
protocol relation; before this clause the band was a `test` in one shell script
and a founding that avoided that script was unbounded. -/
theorem a_venue_rate_above_the_band_refuses :
    program.execute 3
        ⟨scalars 3
            [(.sellerFeeBps, 501), (.buyerFeeBps, 501), (.policyFeeBps, 501)],
          identities⟩ = none := by
  native_decide

open Witness in
/-- The take-everything market: a rate at the denominator used to be admitted,
and it was the ONLY rate that could enable `CUSTODY_ROUTES_V3` slot 3. The band
refuses it, which is what retires that route. -/
theorem a_venue_rate_at_the_denominator_refuses :
    program.execute 3
        ⟨scalars 3
            [(.sellerFeeBps, 10000), (.buyerFeeBps, 10000), (.policyFeeBps, 10000)],
          identities⟩ = none := by
  native_decide

open Witness in
/-- The fee leg is not routed in tx1. On an admitted fee-bearing frame the
seller route's enable bit is set, the fee-continuation and fee-sole bits are
zero, and the Custody replay advances by exactly one. -/
theorem a_fee_bearing_frame_routes_the_seller_leg_alone :
    ledger (program.execute 3 ⟨scalars 3 feeBearing, identities⟩) = some (1, 19, 2)
      ∧ routes (program.execute 3 ⟨scalars 3 feeBearing, identities⟩) = some (1, 0, 0, 4) := by
  native_decide

open Witness in
/-- And the zero-fee frame keeps its shipped single terminal route: the
Custody replay still advances by exactly one, so nothing about the zero-fee
path's Custody arithmetic moves. -/
theorem a_zero_fee_frame_keeps_its_terminal_seller_route :
    ledger (program.execute 3 ⟨scalars 3, identities⟩) = some (0, 5, 0)
      ∧ routes (program.execute 3 ⟨scalars 3, identities⟩) = some (0, 0, 0, 4) := by
  native_decide

/-! ## Geometry

The one authored program is the program of EVERY market geometry, not of the
canonical demo one. Product Runtime V2 fixes `region_count = cut_count + 1` and
`outcome_count = region_count + 1`, so a market's geometry is the single number
`outcome_count = cut_count + 2`; the canonical three-outcome demo is one cut.

Nothing above is written per geometry. The tail count is an ARGUMENT of
`execute`, the item body is folded once per tail coordinate, and the bank
widths are affine in it — so the encoded program, and every artifact whose
content identity is a digest of it, is the same at every geometry. These
theorems are that claim, stated where a Rust translation can be checked
against it. -/

/-- The scalar bank is affine in the tail count at the emitted stride, so the
tail count is a runtime width and never a compile-time one. -/
theorem the_scalar_bank_is_affine_in_the_tail_count (tailCount : Nat) :
    program.scalarWidth tailCount = 68 + tailCount * 2 := rfl

/-- The identity bank does not grow with the geometry at all: ordinary Direct
has no per-Product-item identity. -/
theorem the_identity_bank_is_geometry_invariant (tailCount : Nat) :
    program.identityWidth tailCount = 32 := by
  simp [Program.identityWidth, program, commonIdentities, IdentitySlot.all]

open Witness in
/-- The geometry JRNY-2 named as the wall: four outcomes, two cuts. The SAME
program admits it, with the same derived quote, at a tail of four. -/
theorem the_four_outcome_geometry_admits :
    quote (program.execute 4 ⟨scalars 4 [(.outcomeCount, 4)], identities⟩)
      = some (5, 0, 10) := by
  native_decide

open Witness in
/-- Two more geometries, either side of it: two outcomes (zero cuts) and five
(three cuts). The traded outcome stays inside the tail and the fold still sums
to exactly the transferred quantity. -/
theorem the_two_and_five_outcome_geometries_admit :
    quote (program.execute 2 ⟨scalars 2 [(.outcomeCount, 2)], identities⟩)
        = some (5, 0, 10)
      ∧ quote (program.execute 5 ⟨scalars 5 [(.outcomeCount, 5)], identities⟩)
        = some (5, 0, 10) := by
  native_decide

open Witness in
/-- The epilogue's exactly-one-item relation is not a property of the canonical
geometry. At four outcomes, a traded outcome the Product tail does not carry is
refused exactly as it is at three. -/
theorem a_traded_outcome_outside_a_four_outcome_tail_refuses :
    program.execute 4
        ⟨scalars 4 [(.outcomeCount, 6), (.sellerOutcome, 5), (.buyerOutcome, 5)],
          identities⟩ = none := by
  native_decide

open Witness in
/-- And the divergence the tail count itself could open: an authenticated
outcome count of four executed against a tail of THREE. The traded outcome is
in range for the market and absent from the tail, so the fold writes nothing
and the epilogue refuses. A runtime that projected a market's outcome count
into the bank while folding a shorter tail would transfer Claims that no item
coordinate accounts for; this is the clause that stops it. -/
theorem a_tail_shorter_than_the_market_geometry_refuses :
    program.execute 3
        ⟨scalars 3 [(.outcomeCount, 4), (.sellerOutcome, 3), (.buyerOutcome, 3)],
          identities⟩ = none := by
  native_decide

end DClutch.DirectOrdinaryV3
