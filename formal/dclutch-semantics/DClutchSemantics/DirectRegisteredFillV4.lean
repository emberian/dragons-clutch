import DClutchSemantics.Direct
import DClutchSemantics.TransitionVMV3
import Std.Tactic

/-!
# Registered ordinary fill Direct V4 transition program

The one authored registered-ordinary-fill admission program, and the last V3
program in the tree that was a hand-written Rust `InstructionV3` array. Every
relation an executor is entitled to enforce appears here; the exact register
schema and the emitted bytes are projections of this list.

Register schema notes that are semantic, not incidental:

* the matcher request is deliberately unsigned. Authority comes from two
  previously authenticated GTC records and their maker replay coordinates, so
  the program re-proves both reservations from persisted state rather than
  trusting a signature over the match.
* there is no Product-owned tail. `itemScalarStride` and `itemIdentityStride`
  are both zero and the item body is empty, so the fold runs the prelude alone
  at every authenticated tail count. The Claims quantity this transition
  transfers is the common `quantity` register; nothing here is Product-width.
* the identity bank is forty registers wide while the schema names
  thirty-two. Registers 32..39 are carried, unwritten and unread: no operation
  in the program addresses them, which `named_identities_are_the_addressed_ones`
  decides. The width is a physical bank size, not a semantic one.
* neither Custody leg is unconditional, so the program derives the enable bits
  that select among the settlement's Custody route declarations, and advances
  the Custody replay revision by exactly as many transfers as it enabled. This
  mirrors `DirectOrdinaryV3.lean`'s block instruction for instruction; see
  "The route enables" below for the fee semantics it inherits.
-/

namespace DClutch.DirectRegisteredFillV4

open DClutch
open DClutch.TransitionVMV3

/-- Ordered common scalar-register schema. Constructor order is the wire
index; the Rust constants are emitted from this typed data. -/
inductive ScalarSlot where
  | rootPhase | slot | outcomeCount | marketGeneration
  | priceScale | policyFeeBps | quantity | executionPrice
  | zero | one | gtc | feeDenominator
  | rootOpenCount
  | sellerSide | sellerLifecycle | sellerOutcome | sellerGeneration
  | sellerNonce | sellerValidFrom | sellerValidThrough | sellerMaximum
  | sellerLimit | sellerFeeBps | sellerFilled | sellerReservedClaims
  | sellerReservedCollateral | sellerCumulativeGross | sellerCumulativeFee
  | sellerNextNonce | sellerLiveCount | sellerMinimumNonce | sellerMakerGeneration
  | buyerSide | buyerLifecycle | buyerOutcome | buyerGeneration
  | buyerNonce | buyerValidFrom | buyerValidThrough | buyerMaximum
  | buyerLimit | buyerFeeBps | buyerFilled | buyerReservedClaims
  | buyerReservedCollateral | buyerCumulativeGross | buyerCumulativeFee
  | buyerNextNonce | buyerLiveCount | buyerMinimumNonce | buyerMakerGeneration
  | sellerFilledAfter | buyerFilledAfter | sellerRemainingAfter | buyerRemainingAfter
  | gross | sellerCumulativeGrossAfter | buyerCumulativeGrossAfter
  | sellerCumulativeFeeAfter | buyerCumulativeFeeAfter
  | sellerFeeDelta | buyerFeeDelta | sellerNet | buyerDebit | totalFee
  | sellerReservedClaimsAfter | buyerReservedCollateralAfter
  | sellerTerminal | buyerTerminal | sellerLiveCountAfter | buyerLiveCountAfter
  | sellerCurrentFeeCheck | buyerCurrentFeeCheck | sellerCurrentRemaining
  | buyerInitialGross | buyerInitialFee | buyerInitialReserve | buyerSpent
  | buyerCurrentReserveCheck | conservation
  | claimSourceRevision | claimDestinationRevision
  | claimSourceRevisionAfter | claimDestinationRevisionAfter
  | custodyRevision | custodyRevisionAfterSeller | custodyRevisionAfterFee
  | terminal
  | sellerMakerRentPrincipal | sellerRecordRentPrincipal
  | buyerMakerRentPrincipal | buyerRecordRentPrincipal
  | feeNonzero | sellerTerminalRouteEnabled
  | sellerIntermediateRouteEnabled | feeSoleRouteEnabled
  deriving DecidableEq, Repr

namespace ScalarSlot

def all : List ScalarSlot := [
  .rootPhase, .slot, .outcomeCount, .marketGeneration,
  .priceScale, .policyFeeBps, .quantity, .executionPrice,
  .zero, .one, .gtc, .feeDenominator,
  .rootOpenCount,
  .sellerSide, .sellerLifecycle, .sellerOutcome, .sellerGeneration,
  .sellerNonce, .sellerValidFrom, .sellerValidThrough, .sellerMaximum,
  .sellerLimit, .sellerFeeBps, .sellerFilled, .sellerReservedClaims,
  .sellerReservedCollateral, .sellerCumulativeGross, .sellerCumulativeFee,
  .sellerNextNonce, .sellerLiveCount, .sellerMinimumNonce, .sellerMakerGeneration,
  .buyerSide, .buyerLifecycle, .buyerOutcome, .buyerGeneration,
  .buyerNonce, .buyerValidFrom, .buyerValidThrough, .buyerMaximum,
  .buyerLimit, .buyerFeeBps, .buyerFilled, .buyerReservedClaims,
  .buyerReservedCollateral, .buyerCumulativeGross, .buyerCumulativeFee,
  .buyerNextNonce, .buyerLiveCount, .buyerMinimumNonce, .buyerMakerGeneration,
  .sellerFilledAfter, .buyerFilledAfter, .sellerRemainingAfter, .buyerRemainingAfter,
  .gross, .sellerCumulativeGrossAfter, .buyerCumulativeGrossAfter,
  .sellerCumulativeFeeAfter, .buyerCumulativeFeeAfter,
  .sellerFeeDelta, .buyerFeeDelta, .sellerNet, .buyerDebit, .totalFee,
  .sellerReservedClaimsAfter, .buyerReservedCollateralAfter,
  .sellerTerminal, .buyerTerminal, .sellerLiveCountAfter, .buyerLiveCountAfter,
  .sellerCurrentFeeCheck, .buyerCurrentFeeCheck, .sellerCurrentRemaining,
  .buyerInitialGross, .buyerInitialFee, .buyerInitialReserve, .buyerSpent,
  .buyerCurrentReserveCheck, .conservation,
  .claimSourceRevision, .claimDestinationRevision,
  .claimSourceRevisionAfter, .claimDestinationRevisionAfter,
  .custodyRevision, .custodyRevisionAfterSeller, .custodyRevisionAfterFee,
  .terminal,
  .sellerMakerRentPrincipal, .sellerRecordRentPrincipal,
  .buyerMakerRentPrincipal, .buyerRecordRentPrincipal,
  .feeNonzero, .sellerTerminalRouteEnabled,
  .sellerIntermediateRouteEnabled, .feeSoleRouteEnabled
]

@[simp] def index : ScalarSlot → Nat
  | .rootPhase => 0
  | .slot => 1
  | .outcomeCount => 2
  | .marketGeneration => 3
  | .priceScale => 4
  | .policyFeeBps => 5
  | .quantity => 6
  | .executionPrice => 7
  | .zero => 8
  | .one => 9
  | .gtc => 10
  | .feeDenominator => 11
  | .rootOpenCount => 12
  | .sellerSide => 13
  | .sellerLifecycle => 14
  | .sellerOutcome => 15
  | .sellerGeneration => 16
  | .sellerNonce => 17
  | .sellerValidFrom => 18
  | .sellerValidThrough => 19
  | .sellerMaximum => 20
  | .sellerLimit => 21
  | .sellerFeeBps => 22
  | .sellerFilled => 23
  | .sellerReservedClaims => 24
  | .sellerReservedCollateral => 25
  | .sellerCumulativeGross => 26
  | .sellerCumulativeFee => 27
  | .sellerNextNonce => 28
  | .sellerLiveCount => 29
  | .sellerMinimumNonce => 30
  | .sellerMakerGeneration => 31
  | .buyerSide => 32
  | .buyerLifecycle => 33
  | .buyerOutcome => 34
  | .buyerGeneration => 35
  | .buyerNonce => 36
  | .buyerValidFrom => 37
  | .buyerValidThrough => 38
  | .buyerMaximum => 39
  | .buyerLimit => 40
  | .buyerFeeBps => 41
  | .buyerFilled => 42
  | .buyerReservedClaims => 43
  | .buyerReservedCollateral => 44
  | .buyerCumulativeGross => 45
  | .buyerCumulativeFee => 46
  | .buyerNextNonce => 47
  | .buyerLiveCount => 48
  | .buyerMinimumNonce => 49
  | .buyerMakerGeneration => 50
  | .sellerFilledAfter => 51
  | .buyerFilledAfter => 52
  | .sellerRemainingAfter => 53
  | .buyerRemainingAfter => 54
  | .gross => 55
  | .sellerCumulativeGrossAfter => 56
  | .buyerCumulativeGrossAfter => 57
  | .sellerCumulativeFeeAfter => 58
  | .buyerCumulativeFeeAfter => 59
  | .sellerFeeDelta => 60
  | .buyerFeeDelta => 61
  | .sellerNet => 62
  | .buyerDebit => 63
  | .totalFee => 64
  | .sellerReservedClaimsAfter => 65
  | .buyerReservedCollateralAfter => 66
  | .sellerTerminal => 67
  | .buyerTerminal => 68
  | .sellerLiveCountAfter => 69
  | .buyerLiveCountAfter => 70
  | .sellerCurrentFeeCheck => 71
  | .buyerCurrentFeeCheck => 72
  | .sellerCurrentRemaining => 73
  | .buyerInitialGross => 74
  | .buyerInitialFee => 75
  | .buyerInitialReserve => 76
  | .buyerSpent => 77
  | .buyerCurrentReserveCheck => 78
  | .conservation => 79
  | .claimSourceRevision => 80
  | .claimDestinationRevision => 81
  | .claimSourceRevisionAfter => 82
  | .claimDestinationRevisionAfter => 83
  | .custodyRevision => 84
  | .custodyRevisionAfterSeller => 85
  | .custodyRevisionAfterFee => 86
  | .terminal => 87
  | .sellerMakerRentPrincipal => 88
  | .sellerRecordRentPrincipal => 89
  | .buyerMakerRentPrincipal => 90
  | .buyerRecordRentPrincipal => 91
  | .feeNonzero => 92
  | .sellerTerminalRouteEnabled => 93
  | .sellerIntermediateRouteEnabled => 94
  | .feeSoleRouteEnabled => 95

def rustName : ScalarSlot → String
  | .rootPhase => "FILL_SCALAR_ROOT_PHASE_V4"
  | .slot => "FILL_SCALAR_SLOT_V4"
  | .outcomeCount => "FILL_SCALAR_OUTCOME_COUNT_V4"
  | .marketGeneration => "FILL_SCALAR_MARKET_GENERATION_V4"
  | .priceScale => "FILL_SCALAR_PRICE_SCALE_V4"
  | .policyFeeBps => "FILL_SCALAR_POLICY_FEE_BPS_V4"
  | .quantity => "FILL_SCALAR_QUANTITY_V4"
  | .executionPrice => "FILL_SCALAR_EXECUTION_PRICE_V4"
  | .zero => "FILL_SCALAR_ZERO_V4"
  | .one => "FILL_SCALAR_ONE_V4"
  | .gtc => "FILL_SCALAR_GTC_V4"
  | .feeDenominator => "FILL_SCALAR_FEE_DENOMINATOR_V4"
  | .rootOpenCount => "FILL_SCALAR_ROOT_OPEN_COUNT_V4"
  | .sellerSide => "FILL_SCALAR_SELLER_SIDE_V4"
  | .sellerLifecycle => "FILL_SCALAR_SELLER_LIFECYCLE_V4"
  | .sellerOutcome => "FILL_SCALAR_SELLER_OUTCOME_V4"
  | .sellerGeneration => "FILL_SCALAR_SELLER_GENERATION_V4"
  | .sellerNonce => "FILL_SCALAR_SELLER_NONCE_V4"
  | .sellerValidFrom => "FILL_SCALAR_SELLER_VALID_FROM_V4"
  | .sellerValidThrough => "FILL_SCALAR_SELLER_VALID_THROUGH_V4"
  | .sellerMaximum => "FILL_SCALAR_SELLER_MAXIMUM_V4"
  | .sellerLimit => "FILL_SCALAR_SELLER_LIMIT_V4"
  | .sellerFeeBps => "FILL_SCALAR_SELLER_FEE_BPS_V4"
  | .sellerFilled => "FILL_SCALAR_SELLER_FILLED_V4"
  | .sellerReservedClaims => "FILL_SCALAR_SELLER_RESERVED_CLAIMS_V4"
  | .sellerReservedCollateral => "FILL_SCALAR_SELLER_RESERVED_COLLATERAL_V4"
  | .sellerCumulativeGross => "FILL_SCALAR_SELLER_CUMULATIVE_GROSS_V4"
  | .sellerCumulativeFee => "FILL_SCALAR_SELLER_CUMULATIVE_FEE_V4"
  | .sellerNextNonce => "FILL_SCALAR_SELLER_NEXT_NONCE_V4"
  | .sellerLiveCount => "FILL_SCALAR_SELLER_LIVE_COUNT_V4"
  | .sellerMinimumNonce => "FILL_SCALAR_SELLER_MINIMUM_NONCE_V4"
  | .sellerMakerGeneration => "FILL_SCALAR_SELLER_MAKER_GENERATION_V4"
  | .buyerSide => "FILL_SCALAR_BUYER_SIDE_V4"
  | .buyerLifecycle => "FILL_SCALAR_BUYER_LIFECYCLE_V4"
  | .buyerOutcome => "FILL_SCALAR_BUYER_OUTCOME_V4"
  | .buyerGeneration => "FILL_SCALAR_BUYER_GENERATION_V4"
  | .buyerNonce => "FILL_SCALAR_BUYER_NONCE_V4"
  | .buyerValidFrom => "FILL_SCALAR_BUYER_VALID_FROM_V4"
  | .buyerValidThrough => "FILL_SCALAR_BUYER_VALID_THROUGH_V4"
  | .buyerMaximum => "FILL_SCALAR_BUYER_MAXIMUM_V4"
  | .buyerLimit => "FILL_SCALAR_BUYER_LIMIT_V4"
  | .buyerFeeBps => "FILL_SCALAR_BUYER_FEE_BPS_V4"
  | .buyerFilled => "FILL_SCALAR_BUYER_FILLED_V4"
  | .buyerReservedClaims => "FILL_SCALAR_BUYER_RESERVED_CLAIMS_V4"
  | .buyerReservedCollateral => "FILL_SCALAR_BUYER_RESERVED_COLLATERAL_V4"
  | .buyerCumulativeGross => "FILL_SCALAR_BUYER_CUMULATIVE_GROSS_V4"
  | .buyerCumulativeFee => "FILL_SCALAR_BUYER_CUMULATIVE_FEE_V4"
  | .buyerNextNonce => "FILL_SCALAR_BUYER_NEXT_NONCE_V4"
  | .buyerLiveCount => "FILL_SCALAR_BUYER_LIVE_COUNT_V4"
  | .buyerMinimumNonce => "FILL_SCALAR_BUYER_MINIMUM_NONCE_V4"
  | .buyerMakerGeneration => "FILL_SCALAR_BUYER_MAKER_GENERATION_V4"
  | .sellerFilledAfter => "FILL_SCALAR_SELLER_FILLED_AFTER_V4"
  | .buyerFilledAfter => "FILL_SCALAR_BUYER_FILLED_AFTER_V4"
  | .sellerRemainingAfter => "FILL_SCALAR_SELLER_REMAINING_AFTER_V4"
  | .buyerRemainingAfter => "FILL_SCALAR_BUYER_REMAINING_AFTER_V4"
  | .gross => "FILL_SCALAR_GROSS_V4"
  | .sellerCumulativeGrossAfter => "FILL_SCALAR_SELLER_CUMULATIVE_GROSS_AFTER_V4"
  | .buyerCumulativeGrossAfter => "FILL_SCALAR_BUYER_CUMULATIVE_GROSS_AFTER_V4"
  | .sellerCumulativeFeeAfter => "FILL_SCALAR_SELLER_CUMULATIVE_FEE_AFTER_V4"
  | .buyerCumulativeFeeAfter => "FILL_SCALAR_BUYER_CUMULATIVE_FEE_AFTER_V4"
  | .sellerFeeDelta => "FILL_SCALAR_SELLER_FEE_DELTA_V4"
  | .buyerFeeDelta => "FILL_SCALAR_BUYER_FEE_DELTA_V4"
  | .sellerNet => "FILL_SCALAR_SELLER_NET_V4"
  | .buyerDebit => "FILL_SCALAR_BUYER_DEBIT_V4"
  | .totalFee => "FILL_SCALAR_TOTAL_FEE_V4"
  | .sellerReservedClaimsAfter => "FILL_SCALAR_SELLER_RESERVED_CLAIMS_AFTER_V4"
  | .buyerReservedCollateralAfter => "FILL_SCALAR_BUYER_RESERVED_COLLATERAL_AFTER_V4"
  | .sellerTerminal => "FILL_SCALAR_SELLER_TERMINAL_V4"
  | .buyerTerminal => "FILL_SCALAR_BUYER_TERMINAL_V4"
  | .sellerLiveCountAfter => "FILL_SCALAR_SELLER_LIVE_COUNT_AFTER_V4"
  | .buyerLiveCountAfter => "FILL_SCALAR_BUYER_LIVE_COUNT_AFTER_V4"
  | .sellerCurrentFeeCheck => "FILL_SCALAR_SELLER_CURRENT_FEE_CHECK_V4"
  | .buyerCurrentFeeCheck => "FILL_SCALAR_BUYER_CURRENT_FEE_CHECK_V4"
  | .sellerCurrentRemaining => "FILL_SCALAR_SELLER_CURRENT_REMAINING_V4"
  | .buyerInitialGross => "FILL_SCALAR_BUYER_INITIAL_GROSS_V4"
  | .buyerInitialFee => "FILL_SCALAR_BUYER_INITIAL_FEE_V4"
  | .buyerInitialReserve => "FILL_SCALAR_BUYER_INITIAL_RESERVE_V4"
  | .buyerSpent => "FILL_SCALAR_BUYER_SPENT_V4"
  | .buyerCurrentReserveCheck => "FILL_SCALAR_BUYER_CURRENT_RESERVE_CHECK_V4"
  | .conservation => "FILL_SCALAR_CONSERVATION_V4"
  | .claimSourceRevision => "FILL_SCALAR_CLAIM_SOURCE_REVISION_V4"
  | .claimDestinationRevision => "FILL_SCALAR_CLAIM_DESTINATION_REVISION_V4"
  | .claimSourceRevisionAfter => "FILL_SCALAR_CLAIM_SOURCE_REVISION_AFTER_V4"
  | .claimDestinationRevisionAfter => "FILL_SCALAR_CLAIM_DESTINATION_REVISION_AFTER_V4"
  | .custodyRevision => "FILL_SCALAR_CUSTODY_REVISION_V4"
  | .custodyRevisionAfterSeller => "FILL_SCALAR_CUSTODY_REVISION_AFTER_SELLER_V4"
  | .custodyRevisionAfterFee => "FILL_SCALAR_CUSTODY_REVISION_AFTER_FEE_V4"
  | .terminal => "FILL_SCALAR_TERMINAL_V4"
  | .sellerMakerRentPrincipal => "FILL_SCALAR_SELLER_MAKER_RENT_PRINCIPAL_V4"
  | .sellerRecordRentPrincipal => "FILL_SCALAR_SELLER_RECORD_RENT_PRINCIPAL_V4"
  | .buyerMakerRentPrincipal => "FILL_SCALAR_BUYER_MAKER_RENT_PRINCIPAL_V4"
  | .buyerRecordRentPrincipal => "FILL_SCALAR_BUYER_RECORD_RENT_PRINCIPAL_V4"
  | .feeNonzero => "FILL_SCALAR_FEE_NONZERO_V4"
  | .sellerTerminalRouteEnabled => "FILL_SCALAR_SELLER_TERMINAL_ROUTE_ENABLED_V4"
  | .sellerIntermediateRouteEnabled => "FILL_SCALAR_SELLER_INTERMEDIATE_ROUTE_ENABLED_V4"
  | .feeSoleRouteEnabled => "FILL_SCALAR_FEE_SOLE_ROUTE_ENABLED_V4"

/-- Emitted Rust documentation for this coordinate. -/
def doc : ScalarSlot → String
  | .rootPhase => "Authenticated root phase."
  | .slot => "Trusted current slot."
  | .outcomeCount => "Product-authenticated outcome count."
  | .marketGeneration => "Core Market generation."
  | .priceScale => "Immutable config price scale."
  | .policyFeeBps => "Immutable config fee basis points."
  | .quantity => "Matcher-selected positive fill quantity."
  | .executionPrice => "Matcher-selected execution price."
  | .zero => "Canonical zero constant."
  | .one => "Canonical one constant."
  | .gtc => "Registered lifecycle constant."
  | .feeDenominator => "Fee denominator constant."
  | .rootOpenCount => "Number of live maker roots; unchanged by record fill."
  | .sellerSide => "Seller persisted side."
  | .sellerLifecycle => "Seller persisted lifecycle."
  | .sellerOutcome => "Seller outcome."
  | .sellerGeneration => "Seller Market generation."
  | .sellerNonce => "Seller record nonce."
  | .sellerValidFrom => "Seller validity start."
  | .sellerValidThrough => "Seller validity end."
  | .sellerMaximum => "Seller maximum quantity."
  | .sellerLimit => "Seller minimum price."
  | .sellerFeeBps => "Seller signed fee rate."
  | .sellerFilled => "Seller already-filled quantity."
  | .sellerReservedClaims => "Seller remaining claim reserve."
  | .sellerReservedCollateral => "Seller collateral reserve, canonically zero."
  | .sellerCumulativeGross => "Seller cumulative gross."
  | .sellerCumulativeFee => "Seller cumulative fee."
  | .sellerNextNonce => "Seller maker replay next nonce."
  | .sellerLiveCount => "Seller maker replay live record count."
  | .sellerMinimumNonce => "Seller replay invalidation threshold."
  | .sellerMakerGeneration => "Seller maker replay Market generation."
  | .buyerSide => "Buyer persisted side."
  | .buyerLifecycle => "Buyer persisted lifecycle."
  | .buyerOutcome => "Buyer outcome."
  | .buyerGeneration => "Buyer Market generation."
  | .buyerNonce => "Buyer record nonce."
  | .buyerValidFrom => "Buyer validity start."
  | .buyerValidThrough => "Buyer validity end."
  | .buyerMaximum => "Buyer maximum quantity."
  | .buyerLimit => "Buyer maximum price."
  | .buyerFeeBps => "Buyer signed fee rate."
  | .buyerFilled => "Buyer already-filled quantity."
  | .buyerReservedClaims => "Buyer claim reserve, canonically zero."
  | .buyerReservedCollateral => "Buyer remaining collateral reserve."
  | .buyerCumulativeGross => "Buyer cumulative gross."
  | .buyerCumulativeFee => "Buyer cumulative fee."
  | .buyerNextNonce => "Buyer maker replay next nonce."
  | .buyerLiveCount => "Buyer maker replay live record count."
  | .buyerMinimumNonce => "Buyer replay invalidation threshold."
  | .buyerMakerGeneration => "Buyer maker replay Market generation."
  | .sellerFilledAfter => "Seller filled quantity after this match."
  | .buyerFilledAfter => "Buyer filled quantity after this match."
  | .sellerRemainingAfter => "Seller remaining quantity after this match."
  | .buyerRemainingAfter => "Buyer remaining quantity after this match."
  | .gross => "Exact common gross quote."
  | .sellerCumulativeGrossAfter => "Seller cumulative gross after this match."
  | .buyerCumulativeGrossAfter => "Buyer cumulative gross after this match."
  | .sellerCumulativeFeeAfter => "Seller cumulative fee after this match."
  | .buyerCumulativeFeeAfter => "Buyer cumulative fee after this match."
  | .sellerFeeDelta => "Seller difference-of-floors fee."
  | .buyerFeeDelta => "Buyer difference-of-floors fee."
  | .sellerNet => "Net collateral credited to the seller."
  | .buyerDebit => "Gross plus buyer fee debited from buyer escrow."
  | .totalFee => "Combined seller and buyer fee transfer."
  | .sellerReservedClaimsAfter => "Seller claim reserve after this match."
  | .buyerReservedCollateralAfter => "Buyer collateral reserve after this match."
  | .sellerTerminal => "One exactly when the seller record becomes terminal."
  | .buyerTerminal => "One exactly when the buyer record becomes terminal."
  | .sellerLiveCountAfter => "Seller maker live count after optional terminal close."
  | .buyerLiveCountAfter => "Buyer maker live count after optional terminal close."
  | .sellerCurrentFeeCheck => "Temporary current seller fee recomputation."
  | .buyerCurrentFeeCheck => "Temporary current buyer fee recomputation."
  | .sellerCurrentRemaining => "Temporary current seller remaining quantity."
  | .buyerInitialGross => "Temporary initial buyer gross reserve."
  | .buyerInitialFee => "Temporary initial buyer fee reserve."
  | .buyerInitialReserve => "Temporary total initial buyer reserve."
  | .buyerSpent => "Temporary buyer amount already spent."
  | .buyerCurrentReserveCheck => "Temporary expected current buyer reserve."
  | .conservation => "Conservation scratch: seller net plus combined fee."
  | .claimSourceRevision => "Seller record Position expected revision."
  | .claimDestinationRevision => "Buyer Position expected revision."
  | .claimSourceRevisionAfter => "Seller record Position resulting revision."
  | .claimDestinationRevisionAfter => "Buyer Position resulting revision."
  | .custodyRevision => "Buyer Custody replay revision before the first transfer."
  | .custodyRevisionAfterSeller =>
      "Buyer Custody revision after the seller transfer, advanced only when one is enabled."
  | .custodyRevisionAfterFee =>
      "Buyer Custody revision after the fee transfer, advanced only when one is enabled."
  | .terminal => "Final terminal constant for child delegated transfer envelopes."
  | .sellerMakerRentPrincipal => "Seller maker replay historical rent principal."
  | .sellerRecordRentPrincipal => "Seller registered-record historical rent principal."
  | .buyerMakerRentPrincipal => "Buyer maker replay historical rent principal."
  | .buyerRecordRentPrincipal => "Buyer registered-record historical rent principal."
  | .feeNonzero => "Derived nonzero combined-fee bit."
  | .sellerTerminalRouteEnabled => "Derived terminal seller-only Custody route enable bit."
  | .sellerIntermediateRouteEnabled =>
      "Derived seller-intermediate plus fee-continuation route enable bit."
  | .feeSoleRouteEnabled => "Derived terminal fee-only Custody route enable bit."

end ScalarSlot

/-- Ordered common identity-register schema. -/
inductive IdentitySlot where
  | parentRequest | market | releaseSet | productRecord
  | semanticBasis | linkedBasis | tradingProgram | realm
  | mint | tokenProgram | feeRecipient
  | sellerMaker | buyerMaker
  | sellerIntentMarket | buyerIntentMarket
  | sellerMakerMarket | buyerMakerMarket
  | sellerRecord | buyerRecord
  | sellerMakerState | buyerMakerState
  | sellerCollateralDestination | buyerCollateralRefund
  | buyerCustodyVault | custodyAuthority
  | claimsAggregate | claimSourceOwner | claimDestinationOwner
  | sellerRentOwner | buyerRentOwner
  | sellerMakerReplayOwner | buyerMakerReplayOwner
  deriving DecidableEq, Repr

namespace IdentitySlot

def all : List IdentitySlot := [
  .parentRequest, .market, .releaseSet, .productRecord,
  .semanticBasis, .linkedBasis, .tradingProgram, .realm,
  .mint, .tokenProgram, .feeRecipient,
  .sellerMaker, .buyerMaker,
  .sellerIntentMarket, .buyerIntentMarket,
  .sellerMakerMarket, .buyerMakerMarket,
  .sellerRecord, .buyerRecord,
  .sellerMakerState, .buyerMakerState,
  .sellerCollateralDestination, .buyerCollateralRefund,
  .buyerCustodyVault, .custodyAuthority,
  .claimsAggregate, .claimSourceOwner, .claimDestinationOwner,
  .sellerRentOwner, .buyerRentOwner,
  .sellerMakerReplayOwner, .buyerMakerReplayOwner
]

@[simp] def index : IdentitySlot → Nat
  | .parentRequest => 0
  | .market => 1
  | .releaseSet => 2
  | .productRecord => 3
  | .semanticBasis => 4
  | .linkedBasis => 5
  | .tradingProgram => 6
  | .realm => 7
  | .mint => 8
  | .tokenProgram => 9
  | .feeRecipient => 10
  | .sellerMaker => 11
  | .buyerMaker => 12
  | .sellerIntentMarket => 13
  | .buyerIntentMarket => 14
  | .sellerMakerMarket => 15
  | .buyerMakerMarket => 16
  | .sellerRecord => 17
  | .buyerRecord => 18
  | .sellerMakerState => 19
  | .buyerMakerState => 20
  | .sellerCollateralDestination => 21
  | .buyerCollateralRefund => 22
  | .buyerCustodyVault => 23
  | .custodyAuthority => 24
  | .claimsAggregate => 25
  | .claimSourceOwner => 26
  | .claimDestinationOwner => 27
  | .sellerRentOwner => 28
  | .buyerRentOwner => 29
  | .sellerMakerReplayOwner => 30
  | .buyerMakerReplayOwner => 31

def rustName : IdentitySlot → String
  | .parentRequest => "FILL_IDENTITY_PARENT_REQUEST_V4"
  | .market => "FILL_IDENTITY_MARKET_V4"
  | .releaseSet => "FILL_IDENTITY_RELEASE_SET_V4"
  | .productRecord => "FILL_IDENTITY_PRODUCT_RECORD_V4"
  | .semanticBasis => "FILL_IDENTITY_SEMANTIC_BASIS_V4"
  | .linkedBasis => "FILL_IDENTITY_LINKED_BASIS_V4"
  | .tradingProgram => "FILL_IDENTITY_TRADING_PROGRAM_V4"
  | .realm => "FILL_IDENTITY_REALM_V4"
  | .mint => "FILL_IDENTITY_MINT_V4"
  | .tokenProgram => "FILL_IDENTITY_TOKEN_PROGRAM_V4"
  | .feeRecipient => "FILL_IDENTITY_FEE_RECIPIENT_V4"
  | .sellerMaker => "FILL_IDENTITY_SELLER_MAKER_V4"
  | .buyerMaker => "FILL_IDENTITY_BUYER_MAKER_V4"
  | .sellerIntentMarket => "FILL_IDENTITY_SELLER_INTENT_MARKET_V4"
  | .buyerIntentMarket => "FILL_IDENTITY_BUYER_INTENT_MARKET_V4"
  | .sellerMakerMarket => "FILL_IDENTITY_SELLER_MAKER_MARKET_V4"
  | .buyerMakerMarket => "FILL_IDENTITY_BUYER_MAKER_MARKET_V4"
  | .sellerRecord => "FILL_IDENTITY_SELLER_RECORD_V4"
  | .buyerRecord => "FILL_IDENTITY_BUYER_RECORD_V4"
  | .sellerMakerState => "FILL_IDENTITY_SELLER_MAKER_STATE_V4"
  | .buyerMakerState => "FILL_IDENTITY_BUYER_MAKER_STATE_V4"
  | .sellerCollateralDestination => "FILL_IDENTITY_SELLER_COLLATERAL_DESTINATION_V4"
  | .buyerCollateralRefund => "FILL_IDENTITY_BUYER_COLLATERAL_REFUND_V4"
  | .buyerCustodyVault => "FILL_IDENTITY_BUYER_CUSTODY_VAULT_V4"
  | .custodyAuthority => "FILL_IDENTITY_CUSTODY_AUTHORITY_V4"
  | .claimsAggregate => "FILL_IDENTITY_CLAIMS_AGGREGATE_V4"
  | .claimSourceOwner => "FILL_IDENTITY_CLAIM_SOURCE_OWNER_V4"
  | .claimDestinationOwner => "FILL_IDENTITY_CLAIM_DESTINATION_OWNER_V4"
  | .sellerRentOwner => "FILL_IDENTITY_SELLER_RENT_OWNER_V4"
  | .buyerRentOwner => "FILL_IDENTITY_BUYER_RENT_OWNER_V4"
  | .sellerMakerReplayOwner => "FILL_IDENTITY_SELLER_MAKER_REPLAY_OWNER_V4"
  | .buyerMakerReplayOwner => "FILL_IDENTITY_BUYER_MAKER_REPLAY_OWNER_V4"

/-- Emitted Rust documentation for this coordinate. -/
def doc : IdentitySlot → String
  | .parentRequest => "Parent request digest seeded by common Hot."
  | .market => "Authenticated Core Market."
  | .releaseSet => "Selected release set."
  | .productRecord => "Authenticated Product record digest."
  | .semanticBasis => "Product semantic LiabilityBasis identity."
  | .linkedBasis => "Authenticated raw ProductBasis digest."
  | .tradingProgram => "Registry-selected Trading program."
  | .realm => "Authenticated Realm."
  | .mint => "Realm collateral mint."
  | .tokenProgram => "Realm token program."
  | .feeRecipient => "Immutable fee recipient."
  | .sellerMaker => "Seller record maker."
  | .buyerMaker => "Buyer record maker."
  | .sellerIntentMarket => "Seller intent Market."
  | .buyerIntentMarket => "Buyer intent Market."
  | .sellerMakerMarket => "Seller maker replay Market."
  | .buyerMakerMarket => "Buyer maker replay Market."
  | .sellerRecord => "Seller record account."
  | .buyerRecord => "Buyer record account."
  | .sellerMakerState => "Seller maker replay account."
  | .buyerMakerState => "Buyer maker replay account."
  | .sellerCollateralDestination => "Signed seller collateral destination."
  | .buyerCollateralRefund => "Signed buyer collateral refund account."
  | .buyerCustodyVault => "Buyer record-keyed Custody vault."
  | .custodyAuthority => "Custody transfer authority."
  | .claimsAggregate => "Claims aggregate selected by the Product basis."
  | .claimSourceOwner => "Seller record Position owner."
  | .claimDestinationOwner => "Buyer user Position owner."
  | .sellerRentOwner => "Seller record RentCredit beneficiary."
  | .buyerRentOwner => "Buyer record RentCredit beneficiary."
  | .sellerMakerReplayOwner => "Seller identity stored in the maker replay account."
  | .buyerMakerReplayOwner => "Buyer identity stored in the maker replay account."

end IdentitySlot

/-- Common scalar coordinate. -/
def s (register : ScalarSlot) : Reg := common register.index

/-- Common identity coordinate. -/
def d (register : IdentitySlot) : Reg := common register.index

/-- Exact common scalar-bank width. -/
def commonScalars : Nat := ScalarSlot.all.length

/-- The common scalar-bank width the TRANSCRIPTION carries, which is the width
the hand-written Rust `InstructionV3` array shipped: the four route-enable
coordinates are appended schema and the transcription predates them. Pinning it
as a literal is what keeps the transcription's emitted bytes byte-identical to
the object it replaced; the bank width is a header field, so deriving it from
`ScalarSlot.all` would have moved those bytes the moment the schema grew. -/
def transcribedCommonScalars : Nat := 92

/-- Named common identity coordinates. -/
def namedIdentities : Nat := IdentitySlot.all.length

/-- Identity registers the bank carries past the named schema. No operation in
the program addresses one; see `named_identities_are_the_addressed_ones`. -/
def reservedIdentities : Nat := 8

/-- Exact common identity-bank width. -/
def commonIdentities : Nat := namedIdentities + reservedIdentities

/-- The registered ordinary fill has no per-Product-item register body. -/
def itemScalarStride : Nat := 0

/-- The registered ordinary fill has no per-Product-item identity body. -/
def itemIdentityStride : Nat := 0

/-! ## The program

The whole admission relation is a prelude: with a zero item stride and an empty
item body there is nothing for the fold to run per tail coordinate, and nothing
for an epilogue to close over. -/

/-- Constants the program owns rather than reads. -/
def constantOps : List Op := [
  .loadConst (s .zero) 0,
  .loadConst (s .one) 1,
  .loadConst (s .gtc) 2,
  .loadConst (s .feeDenominator) DClutch.Direct.feeDenominator,
  .loadConst (s .terminal) 1
]

/-- Root, request, and cross-record identity admission. -/
def admissionOps : List Op := [
  .scalarEq (s .rootPhase) (s .zero),
  .nonzero (s .rootOpenCount),
  .nonzero (s .quantity),
  .scalarLe (s .executionPrice) (s .priceScale),
  .identityEq (d .market) (d .sellerIntentMarket),
  .identityEq (d .market) (d .buyerIntentMarket),
  .identityEq (d .market) (d .sellerMakerMarket),
  .identityEq (d .market) (d .buyerMakerMarket),
  .identityNe (d .sellerMaker) (d .buyerMaker),
  .identityEq (d .sellerMaker) (d .sellerMakerReplayOwner),
  .identityEq (d .buyerMaker) (d .buyerMakerReplayOwner),
  .scalarEq (s .marketGeneration) (s .sellerGeneration),
  .scalarEq (s .marketGeneration) (s .buyerGeneration),
  .scalarEq (s .marketGeneration) (s .sellerMakerGeneration),
  .scalarEq (s .marketGeneration) (s .buyerMakerGeneration),
  .scalarEq (s .sellerSide) (s .zero),
  .scalarEq (s .buyerSide) (s .one),
  .scalarEq (s .sellerLifecycle) (s .gtc),
  .scalarEq (s .buyerLifecycle) (s .gtc),
  .scalarEq (s .sellerOutcome) (s .buyerOutcome),
  .scalarLt (s .sellerOutcome) (s .outcomeCount),
  .scalarEq (s .sellerFeeBps) (s .policyFeeBps),
  .scalarEq (s .buyerFeeBps) (s .policyFeeBps),
  .scalarLe (s .sellerValidFrom) (s .slot),
  .scalarLe (s .slot) (s .sellerValidThrough),
  .scalarLe (s .buyerValidFrom) (s .slot),
  .scalarLe (s .slot) (s .buyerValidThrough),
  .scalarLe (s .sellerLimit) (s .executionPrice),
  .scalarLe (s .executionPrice) (s .buyerLimit),
  .scalarLe (s .buyerLimit) (s .priceScale)
]

/-- Maker replay admission: both records are live, in-window, and reachable
from their replay roots. -/
def replayOps : List Op := [
  .scalarLt (s .sellerNonce) (s .sellerNextNonce),
  .scalarLt (s .buyerNonce) (s .buyerNextNonce),
  .scalarLe (s .sellerMinimumNonce) (s .sellerNonce),
  .scalarLe (s .buyerMinimumNonce) (s .buyerNonce),
  .nonzero (s .sellerLiveCount),
  .nonzero (s .buyerLiveCount),
  .scalarLe (s .sellerLiveCount) (s .sellerNextNonce),
  .scalarLe (s .sellerMinimumNonce) (s .sellerNextNonce),
  .scalarLe (s .buyerLiveCount) (s .buyerNextNonce),
  .scalarLe (s .buyerMinimumNonce) (s .buyerNextNonce),
  .nonzero (s .sellerMakerRentPrincipal),
  .nonzero (s .sellerRecordRentPrincipal),
  .nonzero (s .buyerMakerRentPrincipal),
  .nonzero (s .buyerRecordRentPrincipal)
]

/-- Re-proof of the two persisted reservations. The transition trusts neither
record's stored fee or reserve: it recomputes each from the record's own
cumulative gross and refuses a disagreement. -/
def reservationOps : List Op := [
  .scalarLt (s .sellerFilled) (s .sellerMaximum),
  .scalarLt (s .buyerFilled) (s .buyerMaximum),
  .scalarLe (s .sellerCumulativeGross) (s .sellerFilled),
  .scalarLe (s .buyerCumulativeGross) (s .buyerFilled),
  .mulDivFloor (s .sellerCumulativeGross) (s .policyFeeBps) (s .feeDenominator)
    (s .sellerCurrentFeeCheck),
  .scalarEq (s .sellerCurrentFeeCheck) (s .sellerCumulativeFee),
  .mulDivFloor (s .buyerCumulativeGross) (s .policyFeeBps) (s .feeDenominator)
    (s .buyerCurrentFeeCheck),
  .scalarEq (s .buyerCurrentFeeCheck) (s .buyerCumulativeFee),
  .subInto (s .sellerMaximum) (s .sellerFilled) (s .sellerCurrentRemaining),
  .scalarEq (s .sellerCurrentRemaining) (s .sellerReservedClaims),
  .scalarEq (s .sellerReservedCollateral) (s .zero),
  .mulDivFloor (s .buyerMaximum) (s .buyerLimit) (s .priceScale) (s .buyerInitialGross),
  .mulDivFloor (s .buyerInitialGross) (s .policyFeeBps) (s .feeDenominator)
    (s .buyerInitialFee),
  .checkedAddInto (s .buyerInitialGross) (s .buyerInitialFee) (s .buyerInitialReserve),
  .checkedAddInto (s .buyerCumulativeGross) (s .buyerCumulativeFee) (s .buyerSpent),
  .subInto (s .buyerInitialReserve) (s .buyerSpent) (s .buyerCurrentReserveCheck),
  .scalarEq (s .buyerCurrentReserveCheck) (s .buyerReservedCollateral),
  .scalarEq (s .buyerReservedClaims) (s .zero)
]

/-- Derivation of the match: successor fills, the exact quote, the
cumulative-difference fees, and the conserved collateral movement. -/
def derivationOps : List Op := [
  .checkedAddInto (s .sellerFilled) (s .quantity) (s .sellerFilledAfter),
  .scalarLe (s .sellerFilledAfter) (s .sellerMaximum),
  .checkedAddInto (s .buyerFilled) (s .quantity) (s .buyerFilledAfter),
  .scalarLe (s .buyerFilledAfter) (s .buyerMaximum),
  .subInto (s .sellerMaximum) (s .sellerFilledAfter) (s .sellerRemainingAfter),
  .subInto (s .buyerMaximum) (s .buyerFilledAfter) (s .buyerRemainingAfter),
  .mulDivExact (s .quantity) (s .executionPrice) (s .priceScale) (s .gross),
  .checkedAddInto (s .sellerCumulativeGross) (s .gross) (s .sellerCumulativeGrossAfter),
  .checkedAddInto (s .buyerCumulativeGross) (s .gross) (s .buyerCumulativeGrossAfter),
  .scalarLe (s .sellerCumulativeGrossAfter) (s .sellerFilledAfter),
  .scalarLe (s .buyerCumulativeGrossAfter) (s .buyerFilledAfter),
  .mulDivFloor (s .sellerCumulativeGrossAfter) (s .policyFeeBps) (s .feeDenominator)
    (s .sellerCumulativeFeeAfter),
  .mulDivFloor (s .buyerCumulativeGrossAfter) (s .policyFeeBps) (s .feeDenominator)
    (s .buyerCumulativeFeeAfter),
  .subInto (s .sellerCumulativeFeeAfter) (s .sellerCumulativeFee) (s .sellerFeeDelta),
  .subInto (s .buyerCumulativeFeeAfter) (s .buyerCumulativeFee) (s .buyerFeeDelta),
  .subInto (s .gross) (s .sellerFeeDelta) (s .sellerNet),
  .checkedAddInto (s .gross) (s .buyerFeeDelta) (s .buyerDebit),
  .checkedAddInto (s .sellerFeeDelta) (s .buyerFeeDelta) (s .totalFee),
  .checkedAddInto (s .sellerNet) (s .totalFee) (s .conservation),
  .scalarEq (s .conservation) (s .buyerDebit),
  .subInto (s .sellerReservedClaims) (s .quantity) (s .sellerReservedClaimsAfter),
  .subInto (s .buyerReservedCollateral) (s .buyerDebit) (s .buyerReservedCollateralAfter)
]

/-- Terminal candidacy and the two Claims revisions the effect chain consumes.
Both Claims legs are unconditional because `quantity` is required nonzero, so
the Claims route always moves something. Neither Custody leg is: those revisions
are derived in `routeOps`. -/
def successorOps : List Op := [
  .loadConst (s .sellerTerminal) 0,
  .selectZero (s .sellerRemainingAfter) (s .one) (s .sellerTerminal),
  .loadConst (s .buyerTerminal) 0,
  .selectZero (s .buyerRemainingAfter) (s .one) (s .buyerTerminal),
  .subInto (s .sellerLiveCount) (s .sellerTerminal) (s .sellerLiveCountAfter),
  .subInto (s .buyerLiveCount) (s .buyerTerminal) (s .buyerLiveCountAfter),
  .incrementInto (s .claimSourceRevision) (s .claimSourceRevisionAfter),
  .incrementInto (s .claimDestinationRevision) (s .claimDestinationRevisionAfter)
]

/-- The Custody replay ladder the TRANSCRIPTION carried: two transfers claimed
unconditionally. `the_transcription_claimed_two_transfers_on_a_one_transfer_bank`
decides what that costs on the canonical admitted fill. -/
def transcribedRevisionOps : List Op := [
  .incrementInto (s .custodyRevision) (s .custodyRevisionAfterSeller),
  .incrementInto (s .custodyRevisionAfterSeller) (s .custodyRevisionAfterFee)
]

/-- The transcribed program: exactly the ninety-nine instructions the
hand-written Rust `InstructionV3` array produced, in their original order, over
the ninety-two-register bank it declared.
`transcription_instruction_count` and the byte-identity gate recorded with this
module's landing commit pin it to the object that was already executing. -/
def transcribedProgram : Program := {
  commonScalars := transcribedCommonScalars
  itemScalarStride := itemScalarStride
  commonIdentities := commonIdentities
  itemIdentityStride := itemIdentityStride
  «prelude» := constantOps ++ admissionOps ++ replayOps ++ reservationOps ++
    derivationOps ++ successorOps ++ transcribedRevisionOps
  itemBody := []
  epilogue := []
}

theorem transcription_instruction_count :
    transcribedProgram.operations.length = 99 := by native_decide

theorem transcription_encoded_width :
    (Codec.encodeProgram transcribedProgram).length = 2408 := by native_decide

/-- The transcription's declared bank width, which is a header field and
therefore part of its bytes. Growing `ScalarSlot` must not move it. -/
theorem transcription_common_scalar_count :
    transcribedProgram.commonScalars = 92 := by native_decide

theorem transcription_is_well_formed :
    transcribedProgram.wellFormed = true := by native_decide

/-! ## The strengthening

`73f0793` landed two admission clauses on the ordinary V3 program. Exactly one
of them applies here.

* `policyFeeBps ≤ feeDenominator` APPLIES, and the transcription does not have
  it. Both fee legs are floors of `policyFeeBps / feeDenominator`, and the
  conservation clause is an identity in the fee deltas — expanding it gives
  `gross + buyerFeeDelta` on both sides for any fee whatever — so nothing in the
  transcription bounds the rate. Above the denominator the buyer is debited more
  than the quote while the seller nets less than nothing the moment
  `sellerFeeDelta` exceeds `gross`; below that boundary the only refusal is the
  incidental one `subInto` raises. `a_venue_rate_above_the_denominator_refuses`
  is the witness this closes, and `DirectExecutionConfigV1::new` remains defence
  in depth rather than the authority.
* The Product-tail Claims total DOES NOT APPLY. Ordinary needed it because its
  item body writes one Claims quantity per Product coordinate and the epilogue
  has to prove exactly one of them carried the traded outcome. This program has
  no item body and writes no per-item quantity: its Claims movement is the
  single common `quantity` register, and `sellerOutcome < outcomeCount` is the
  whole of what the tail could constrain here. Carrying the clause would require
  inventing a tail this transition does not have. Recorded rather than faked. -/

/-- The one admission clause `73f0793` landed on ordinary and this program never
had. -/
def feeRateOps : List Op := [
  .scalarLe (s .policyFeeBps) (s .feeDenominator)
]

/-! ## The route enables

The settlement this transition drives moves collateral over at most two Custody
transfers out of the buyer record's Vault: the seller's net, and the combined
fee. Neither is unconditional, and a Custody `Transfer` carrying `amount = 0` is
refused by `CustodyRequestV1::validate` on its own terms. So the program derives
the enable bits an `EffectProgramV4` route declaration reads, exactly as
`DirectOrdinaryV3.lean` does for the inline-ordinary family:

* `sellerTerminalRouteEnabled` — the seller leg alone, and it closes the chain.
  Set exactly when the seller nets something and the combined fee is zero.
* `sellerIntermediateRouteEnabled` — the seller leg with a fee continuation
  behind it. Set exactly when both legs move.
* `feeSoleRouteEnabled` — the fee leg alone, closing the chain. Set exactly when
  the seller nets nothing and the fee moves.
* all three zero — nothing moves. Reachable, and witnessed: a fill at an
  execution price of zero quotes nothing, charges nothing, and still transfers
  Claims.

THE ZERO-FEE DECISION, and it is inherited rather than invented: a zero combined
fee is a NO-TRANSFER PATH, not a refusal. That is what the ordinary family's fee
semantics already say — `feeNonzero` exists in `DirectOrdinaryV3.lean` precisely
so a zero fee routes nothing — and it is the only reading that keeps the
ordinary case working. The fee legs here are DIFFERENCES OF FLOORS of the
cumulative gross: on the canonical admitted fill a hundred-basis-point venue on
a quote of five floors to nothing on both sides, which is why
`canonical_partial_fill_admits` reads `sellerNet = buyerDebit = gross = 5`. A
refusal would refuse the ordinary small fill at a realistic venue rate. It would
also refuse every mid-order fill whose cumulative-difference delta happens to be
zero while the order as a whole pays a fee, which is the entire reason the fee
is charged on the cumulative floor rather than per fill.

The ladder then advances the buyer's Custody replay revision by exactly the
number of transfers the enables turned on — one per enabled route, never more.
The transcription advanced it by two, unconditionally. -/
def routeOps : List Op := [
  .loadConst (s .sellerIntermediateRouteEnabled) 1,
  .selectZero (s .sellerNet) (s .zero) (s .sellerIntermediateRouteEnabled),
  .loadConst (s .feeNonzero) 1,
  .selectZero (s .totalFee) (s .zero) (s .feeNonzero),
  .loadConst (s .sellerTerminalRouteEnabled) 0,
  .selectZero (s .totalFee) (s .sellerIntermediateRouteEnabled) (s .sellerTerminalRouteEnabled),
  .checkedAddInto (s .feeNonzero) (s .zero) (s .sellerIntermediateRouteEnabled),
  .selectZero (s .sellerNet) (s .zero) (s .sellerIntermediateRouteEnabled),
  .loadConst (s .feeSoleRouteEnabled) 0,
  .selectZero (s .sellerNet) (s .feeNonzero) (s .feeSoleRouteEnabled),
  .checkedAddInto (s .sellerTerminalRouteEnabled) (s .sellerIntermediateRouteEnabled)
    (s .custodyRevisionAfterSeller),
  .checkedAddInto (s .custodyRevision) (s .custodyRevisionAfterSeller)
    (s .custodyRevisionAfterSeller),
  .checkedAddInto (s .custodyRevisionAfterSeller) (s .sellerIntermediateRouteEnabled)
    (s .custodyRevisionAfterFee),
  .checkedAddInto (s .custodyRevisionAfterFee) (s .feeSoleRouteEnabled)
    (s .custodyRevisionAfterFee)
]

/-- The authored registered ordinary fill program. -/
def program : Program :=
  { transcribedProgram with
    commonScalars := commonScalars
    «prelude» := constantOps ++ admissionOps ++ feeRateOps ++
      replayOps ++ reservationOps ++ derivationOps ++ successorOps ++ routeOps }

theorem well_formed : program.wellFormed = true := by native_decide

theorem prelude_count : program.prelude.length = 112 := by native_decide

theorem item_count : program.itemBody.length = 0 := by native_decide

theorem epilogue_count : program.epilogue.length = 0 := by native_decide

theorem common_scalar_count : program.commonScalars = 96 := by native_decide

theorem common_identity_count : program.commonIdentities = 40 := by native_decide

theorem encoded_width : (Codec.encodeProgram program).length = 2720 := by native_decide

/-- The whole difference between the shipped object and the authored one, stated
as the two operation lists rather than as a count. Everything the transcription
decided survives verbatim and in order except its unconditional Custody revision
ladder; the authored program inserts one admission clause and replaces that
ladder with the route derivation. -/
theorem transcription_sections :
    transcribedProgram.operations =
      constantOps ++ admissionOps ++ replayOps ++ reservationOps ++ derivationOps ++
        successorOps ++ transcribedRevisionOps := by native_decide

theorem authored_sections :
    program.operations =
      constantOps ++ admissionOps ++ feeRateOps ++ replayOps ++ reservationOps ++
        derivationOps ++ successorOps ++ routeOps := by native_decide

/-- The ADMISSION strengthening is still exactly one clause. The other twelve
net instructions are the settlement's route derivation, which admits and refuses
nothing: every one of them writes. -/
theorem the_admission_strengthening_is_one_clause :
    feeRateOps.length = 1 ∧
      transcribedProgram.operations.length + feeRateOps.length + routeOps.length =
        program.operations.length + transcribedRevisionOps.length := by native_decide

/-- Every identity coordinate any operation addresses is a named one. The bank's
eight further registers are carried width, not silent schema. -/
theorem named_identities_are_the_addressed_ones :
    program.operations.all (fun operation =>
      operation.identityOperands.all (fun register => register.index < namedIdentities)) = true := by
  native_decide

/-! ## Witnesses

Concrete banks the program admits and refuses. These are decided executions of
the authored program, not of any executor: a Rust translation that disagrees
with one of them is wrong about the program, not about its own arithmetic. -/

namespace Witness

/-- Assignments of the canonical admitted frame: a matcher fill of ten at an
execution price of fifty against a scale of one hundred, against two live GTC
records each maximum twenty at limits forty and sixty, on a hundred-basis-point
venue rate all three parties signed. -/
def canonicalScalars : List (ScalarSlot × Nat) := [
  (.slot, 100), (.outcomeCount, 3), (.marketGeneration, 7),
  (.priceScale, 100), (.policyFeeBps, 100), (.rootOpenCount, 2),
  (.quantity, 10), (.executionPrice, 50),
  (.sellerSide, 0), (.sellerLifecycle, 2), (.sellerOutcome, 1),
  (.sellerGeneration, 7), (.sellerNonce, 0),
  (.sellerValidFrom, 90), (.sellerValidThrough, 110),
  (.sellerMaximum, 20), (.sellerLimit, 40), (.sellerFeeBps, 100),
  (.sellerFilled, 0), (.sellerReservedClaims, 20),
  (.sellerReservedCollateral, 0),
  (.sellerCumulativeGross, 0), (.sellerCumulativeFee, 0),
  (.sellerNextNonce, 1), (.sellerLiveCount, 1),
  (.sellerMinimumNonce, 0), (.sellerMakerGeneration, 7),
  (.buyerSide, 1), (.buyerLifecycle, 2), (.buyerOutcome, 1),
  (.buyerGeneration, 7), (.buyerNonce, 0),
  (.buyerValidFrom, 90), (.buyerValidThrough, 110),
  (.buyerMaximum, 20), (.buyerLimit, 60), (.buyerFeeBps, 100),
  (.buyerFilled, 0), (.buyerReservedClaims, 0),
  (.buyerReservedCollateral, 12),
  (.buyerCumulativeGross, 0), (.buyerCumulativeFee, 0),
  (.buyerNextNonce, 1), (.buyerLiveCount, 1),
  (.buyerMinimumNonce, 0), (.buyerMakerGeneration, 7),
  (.claimSourceRevision, 4), (.claimDestinationRevision, 9),
  (.custodyRevision, 3),
  (.sellerMakerRentPrincipal, 1), (.sellerRecordRentPrincipal, 1),
  (.buyerMakerRentPrincipal, 1), (.buyerRecordRentPrincipal, 1)
]

def scalars (overrides : List (ScalarSlot × Nat) := []) : Array Nat :=
  (canonicalScalars ++ overrides).foldl
    (fun bank (assignment : ScalarSlot × Nat) =>
      bank.setIfInBounds assignment.1.index assignment.2)
    (Array.replicate commonScalars 0)

/-- The same frame over the narrower bank the transcription declares. The four
route-enable coordinates do not exist on it. -/
def transcribedScalars (overrides : List (ScalarSlot × Nat) := []) : Array Nat :=
  (canonicalScalars ++ overrides).foldl
    (fun bank (assignment : ScalarSlot × Nat) =>
      bank.setIfInBounds assignment.1.index assignment.2)
    (Array.replicate transcribedCommonScalars 0)

/-- The canonical identity bank: one Market carrying both intents and both
replay roots, and two distinct makers each equal to the identity its own replay
account stores. -/
def identities (overrides : List (IdentitySlot × Nat) := []) : Array Nat :=
  let assignments : List (IdentitySlot × Nat) := [
    (.market, 2), (.sellerIntentMarket, 2), (.buyerIntentMarket, 2),
    (.sellerMakerMarket, 2), (.buyerMakerMarket, 2),
    (.sellerMaker, 3), (.sellerMakerReplayOwner, 3),
    (.buyerMaker, 4), (.buyerMakerReplayOwner, 4)
  ]
  (assignments ++ overrides).foldl
    (fun bank (assignment : IdentitySlot × Nat) =>
      bank.setIfInBounds assignment.1.index assignment.2)
    (Array.replicate commonIdentities 1)

/-- Read the derived coordinates a settlement consumes out of an admitted
result: the quote, the two fee legs, and the two reserves after the match. -/
def settlement (result : Option TransitionVMV3.State) :
    Option (Nat × Nat × Nat × Nat × Nat) :=
  result.map fun state =>
    (state.scalars[ScalarSlot.index .gross]!,
      state.scalars[ScalarSlot.index .sellerNet]!,
      state.scalars[ScalarSlot.index .buyerDebit]!,
      state.scalars[ScalarSlot.index .sellerReservedClaimsAfter]!,
      state.scalars[ScalarSlot.index .buyerReservedCollateralAfter]!)

/-- Read the settlement's route selection out of an admitted result: the three
Custody route enables, then the replay revision after each leg. -/
def routes (result : Option TransitionVMV3.State) :
    Option (Nat × Nat × Nat × Nat × Nat) :=
  result.map fun state =>
    (state.scalars[ScalarSlot.index .sellerTerminalRouteEnabled]!,
      state.scalars[ScalarSlot.index .sellerIntermediateRouteEnabled]!,
      state.scalars[ScalarSlot.index .feeSoleRouteEnabled]!,
      state.scalars[ScalarSlot.index .custodyRevisionAfterSeller]!,
      state.scalars[ScalarSlot.index .custodyRevisionAfterFee]!)

end Witness

open Witness in
/-- The canonical partial fill is admitted. Ten at fifty against a scale of one
hundred quotes five; both cumulative floor fees round to zero at a hundred basis
points, so the seller nets the whole quote and the buyer is debited exactly it. -/
theorem canonical_partial_fill_admits :
    settlement (program.execute 3 ⟨scalars, identities⟩) = some (5, 5, 5, 10, 7) := by
  native_decide

open Witness in
/-- The tail count is not a parameter of this program: with no item body and a
zero stride, every authenticated tail count decides the same way. -/
theorem tail_count_does_not_change_the_result :
    program.execute 0 ⟨scalars, identities⟩ = program.execute 3 ⟨scalars, identities⟩ := by
  native_decide

open Witness in
/-- A venue rate exactly at the denominator is admitted: the fee takes the whole
quote and the seller nets nothing, which is a policy the makers may sign. -/
theorem a_venue_rate_at_the_denominator_admits :
    settlement (program.execute 3
        ⟨scalars [(.policyFeeBps, 10000), (.sellerFeeBps, 10000), (.buyerFeeBps, 10000),
            (.buyerReservedCollateral, 24), (.buyerLimit, 60)],
          identities⟩) = some (5, 0, 10, 10, 14) := by
  native_decide

open Witness in
/-- One basis point above the denominator is refused. This is the clause the
strengthening added; the transcription admits the same bank. -/
theorem a_venue_rate_above_the_denominator_refuses :
    program.execute 3
        ⟨scalars [(.policyFeeBps, 10001), (.sellerFeeBps, 10001), (.buyerFeeBps, 10001),
            (.buyerReservedCollateral, 24)],
          identities⟩ = none := by
  native_decide

open Witness in
/-- The divergence the clause closes, stated against the object it replaced: the
transcription that was executing on chain admits the out-of-bound rate. -/
theorem the_transcription_admitted_the_out_of_bound_rate :
    (transcribedProgram.execute 3
        ⟨transcribedScalars [(.policyFeeBps, 10001), (.sellerFeeBps, 10001),
            (.buyerFeeBps, 10001), (.buyerReservedCollateral, 24)],
          identities⟩).isSome = true := by
  native_decide

open Witness in
/-- A record substituted from another Market refuses. The four Market equalities
are what make the two unsigned records authority for this match. -/
theorem a_record_from_another_market_refuses :
    program.execute 3 ⟨scalars, identities [(.buyerIntentMarket, 9)]⟩ = none := by
  native_decide

open Witness in
/-- One maker on both sides refuses: a self-match would let a single authority
move the venue's fee against itself and clear no risk. -/
theorem a_self_match_refuses :
    program.execute 3
        ⟨scalars, identities [(.buyerMaker, 3), (.buyerMakerReplayOwner, 3)]⟩ = none := by
  native_decide

open Witness in
/-- A maker replay account storing an identity other than the record's maker
refuses: the replay root is the only thing standing behind an unsigned request. -/
theorem a_replay_root_owned_by_another_identity_refuses :
    program.execute 3 ⟨scalars, identities [(.sellerMakerReplayOwner, 9)]⟩ = none := by
  native_decide

open Witness in
/-- A quote that is not integral at the configured scale refuses rather than
rounding: three at fifty against a scale of one hundred is one and a half. -/
theorem a_nonintegral_quote_refuses :
    program.execute 3 ⟨scalars [(.quantity, 3)], identities⟩ = none := by
  native_decide

open Witness in
/-- A record whose stored reserve disagrees with its own cumulative history
refuses. The transition recomputes both reservations rather than trusting the
persisted figures. -/
theorem a_reserve_disagreeing_with_the_record_refuses :
    program.execute 3 ⟨scalars [(.sellerReservedClaims, 19)], identities⟩ = none := by
  native_decide

open Witness in
/-- A fill past the record's own maximum refuses, and so does a nonce below the
replay root's invalidation threshold. -/
theorem an_overfill_refuses :
    program.execute 3 ⟨scalars [(.quantity, 30)], identities⟩ = none := by
  native_decide

open Witness in
theorem an_invalidated_nonce_refuses :
    program.execute 3 ⟨scalars [(.sellerMinimumNonce, 1)], identities⟩ = none := by
  native_decide

open Witness in
/-- Exhausting the seller's remaining quantity derives the terminal bit and
decrements the replay root's live count; the buyer, still partial, does not. -/
theorem an_exact_close_derives_one_terminal_side :
    ((program.execute 3
        ⟨scalars [(.quantity, 20), (.buyerMaximum, 40), (.buyerReservedCollateral, 24),
            (.sellerReservedClaims, 20)],
          identities⟩).map fun state =>
      (state.scalars[ScalarSlot.index .sellerTerminal]!,
        state.scalars[ScalarSlot.index .buyerTerminal]!,
        state.scalars[ScalarSlot.index .sellerLiveCountAfter]!,
        state.scalars[ScalarSlot.index .buyerLiveCountAfter]!)) = some (1, 0, 0, 1) := by
  native_decide

/-! ### The route enables, decided

Four reachable settlements, one per corner of `(sellerNet ≠ 0, totalFee ≠ 0)`.
Each reads `(sellerTerminal, sellerIntermediate, feeSole, revisionAfterSeller,
revisionAfterFee)` off the canonical Custody replay revision of three. -/

open Witness in
/-- The canonical admitted fill charges NOTHING, and enables the seller leg
alone. This is the bank that the transcription's unconditional ladder could not
settle: the fee is zero, so there is no second Custody transfer to claim, and
the replay revision advances by exactly one. -/
theorem the_canonical_zero_fee_fill_enables_only_the_seller_route :
    routes (program.execute 3 ⟨scalars, identities⟩) = some (1, 0, 0, 4, 4) := by
  native_decide

open Witness in
/-- The transcription claimed both transfers on that same bank. This is the
defect the ladder replaces, decided against the object that shipped rather than
described. -/
theorem the_transcription_claimed_two_transfers_on_a_one_transfer_bank :
    ((transcribedProgram.execute 3 ⟨transcribedScalars, identities⟩).map fun state =>
      (state.scalars[ScalarSlot.index .totalFee]!,
        state.scalars[ScalarSlot.index .custodyRevisionAfterSeller]!,
        state.scalars[ScalarSlot.index .custodyRevisionAfterFee]!)) = some (0, 4, 5) := by
  native_decide

open Witness in
/-- A venue rate that clears the floor on both legs enables the seller leg with
a fee continuation behind it: two transfers, two revisions. Twenty per cent of a
quote of five is one on each side, so the seller nets four and the buyer is
debited six. -/
theorem a_fee_that_clears_the_floor_enables_the_seller_and_fee_continuation :
    routes (program.execute 3
        ⟨scalars [(.policyFeeBps, 2000), (.sellerFeeBps, 2000), (.buyerFeeBps, 2000),
            (.buyerReservedCollateral, 14)],
          identities⟩) = some (0, 1, 0, 4, 5) := by
  native_decide

open Witness in
/-- The settlement figures behind that selection. -/
theorem a_fee_that_clears_the_floor_settles_four_and_six :
    settlement (program.execute 3
        ⟨scalars [(.policyFeeBps, 2000), (.sellerFeeBps, 2000), (.buyerFeeBps, 2000),
            (.buyerReservedCollateral, 14)],
          identities⟩) = some (5, 4, 6, 10, 8) := by
  native_decide

open Witness in
/-- A venue rate at the denominator takes the whole quote: the seller nets
nothing, so the fee leg alone is enabled and the replay revision again advances
by exactly one. The seller-net Custody envelope that the transcription's ladder
implied here would have carried a zero amount. -/
theorem a_venue_rate_at_the_denominator_enables_only_the_fee_route :
    routes (program.execute 3
        ⟨scalars [(.policyFeeBps, 10000), (.sellerFeeBps, 10000), (.buyerFeeBps, 10000),
            (.buyerReservedCollateral, 24)],
          identities⟩) = some (0, 0, 1, 3, 4) := by
  native_decide

open Witness in
/-- A fill at an execution price of zero moves Claims and no collateral: the
quote, the fee and the seller net are all nothing, so no Custody route is
enabled and the replay revision does not move at all. The ordinary family admits
the same frame for the same reason — `sellerLimit ≤ executionPrice` is the whole
bound on the price, and a maker may sign a limit of zero. -/
theorem a_zero_price_fill_enables_no_custody_route :
    routes (program.execute 3
        ⟨scalars [(.sellerLimit, 0), (.executionPrice, 0)], identities⟩)
      = some (0, 0, 0, 3, 3) := by
  native_decide

open Witness in
/-- And it still transfers the full Claims quantity, which is why the Claims
route stays unconditional. -/
theorem a_zero_price_fill_still_moves_claims :
    ((program.execute 3
        ⟨scalars [(.sellerLimit, 0), (.executionPrice, 0)], identities⟩).map fun state =>
      (state.scalars[ScalarSlot.index .gross]!,
        state.scalars[ScalarSlot.index .quantity]!,
        state.scalars[ScalarSlot.index .sellerReservedClaimsAfter]!,
        state.scalars[ScalarSlot.index .claimSourceRevisionAfter]!)) = some (0, 10, 10, 5) := by
  native_decide

open Witness in
/-- The ladder is checked, not saturating. A replay revision one below the u64
ceiling admits when one transfer is enabled and refuses when two are, which is
the only difference the enables make to admission. -/
theorem a_saturating_replay_revision_refuses_exactly_when_both_legs_move :
    (program.execute 3
        ⟨scalars [(.custodyRevision, 18446744073709551614)], identities⟩).isSome = true ∧
      program.execute 3
        ⟨scalars [(.custodyRevision, 18446744073709551614), (.policyFeeBps, 2000),
            (.sellerFeeBps, 2000), (.buyerFeeBps, 2000), (.buyerReservedCollateral, 14)],
          identities⟩ = none := by
  native_decide

end DClutch.DirectRegisteredFillV4
