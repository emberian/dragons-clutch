//! One module per instruction family.
//!
//! Every module here has the same shape: a `process` function that receives an
//! already-routed request from [`crate::dispatch`], authenticates its own
//! account list through [`crate::accounts`], and either applies exactly one
//! transition or refuses.  A family module owns its account list, its account
//! order, its role constants, and its write-back; it does not own byte layouts,
//! economic semantics, or the seed schema.
//!
//! The split is by *family* rather than by intent so that the modules partition
//! the work the way the follow-on lanes do, and so that two lanes never edit one
//! file.  The ownership table is in `docs/implementation/SBF_BRINGUP.md`.
//!
//! | module | intents and actions |
//! | --- | --- |
//! | [`construction`] | shared System-CPI construction of the seven-account market state plane |
//! | [`collateral_cash_v3`] | current full-width `Intent::Endow` / `Intent::WithdrawCash` over MarketBindingV2 and Profile-selected collateral code |
//! | [`claim_representation_v3`] | current full-width `Intent::Materialize` / `Intent::Dematerialize` over MarketBindingV2 and GeneralMarketValueAuthorityV2 |
//! | [`external_redemption_v3`] | current full-width `Intent::RedeemExternal` over MarketBindingV2 and Profile-selected collateral code |
//! | [`genesis`] | current `InitRealm`, `InitProfileV2`, and exact revenue-record close; page constructors are historical fixtures |
//! | [`complete_set_v3`] | current full-width `Intent::Split` / `Intent::Merge` over MarketBindingV2 and GeneralMarketValueAuthorityV2 |
//! | [`split`] | historical lowered-ledger Split implementation; no checked dispatch |
//! | [`merge_materialize`] | historical lowered-ledger Merge/representation implementation; no checked dispatch |
//! | [`market_init`] | host-forensic legacy Market founder; no checked dispatch |
//! | [`observe_resolve`] | `Intent::FeedAdvance`, `Action::Resolve`, `Action::RedeemInternal` |
//! | [`source_ingest`] | `Intent::InitSourceSpec`, `Intent::InitSourceArchive`, `Intent::AppendSourceArchive`, `Intent::SealSourceArchive` |
//! | [`source_ingest_v2`] | `Intent::InitSourceSpecV2`, `Intent::InitSourceArchiveV2`, `Intent::AppendSourceArchiveV2`, `Intent::SealSourceArchiveV2` |
//! | [`orders_batch`] | historical General and Direct V2/V3 host fixtures; no checked dispatch |
//! | `general_v2_fee_v5` | current counted-root/rent-owned V5 owner fee authentication and action-38 composition; account order remains General-owned |
//! | `general_v2_receipt_v5` | exact SettlementRoot/retained-Feed/PDA authentication for rent-owned General Receipt V5 |
//! | `general_v2_settlement_root` | exact indexed `0xa9/2` PDA/owner/full-body authentication for the successor dispatcher |
//!
//! Checked profiles route only account families whose current schemas are
//! closed. The shared numeric value tags 2–5 and 15–17 are current full-width
//! Collateral routes; the legacy founder and General placement/clearing
//! families remain compiled only for hostile host fixtures and dependency-
//! lower migration work, with no capability or checked dispatch.
//!
//! [`genesis`] owns namespace construction; [`orders_batch`] reuses its shared
//! prefund-safe System-CPI helper for the two content-addressed submission
//! accounts. Other families write over already-created, correctly sized state.

pub mod artifact;
pub mod cash_exit;
pub mod claim_representation_v3;
pub mod collateral_cash_v3;
pub(crate) mod collateral_position_v3;
pub mod complete_set_v3;
pub mod construction;
/// Non-production executable Dealer facility slice.
#[cfg(feature = "profile-non-production-dealer-policy-catalog-lab")]
pub mod dealer_facility;
/// Non-production immutable Dealer policy/liveness catalog transport.
pub mod dealer_policy;
/// Capability-disabled Dealer facility account and instruction contracts.
pub mod dealer_runtime;
// Legacy Direct V2/V3 source remains in-tree for historical review, but is
// deliberately absent from this executable module graph. Their allocated
// wire coordinates are decode-only and refuse at the capability boundary.
/// Capability-disabled current Direct `80/1` account/authentication plane.
#[cfg(feature = "profile-full")]
pub(crate) mod direct_market_v1;
pub mod external_exit;
pub mod external_redemption_v3;
/// Capability-disabled reusable Market interval account seam.
pub(crate) mod failure_market_interval_v2;
/// Sole current Product/Source/Failure zero-payout attempt composer.
pub(crate) mod failure_market_source_failure_current;
/// Sole current action-10 Product/Source/Failure branch owner.
pub(crate) mod failure_market_action10_current;
pub(crate) mod failure_market_action11_current;
/// Sole current action-12 RootV2/Product/Source/Failure owner.
pub(crate) mod failure_market_action12_current;
/// Concrete checked owners for current Market Failure actions 10-13.
pub(crate) mod failure_market_actions_v2;
/// Capability-disabled atomic paid Failure interval advance.
pub(crate) mod failure_market_interval_advance_v2;
/// Always-compiled checked-disabled current Failure action contract.
pub mod failure_market_dispatch_v2;
/// Shared hostile Product/Failure authentication for callable actions 10-13.
pub(crate) mod failure_market_execution_v2;
/// Capability-disabled atomic Failure/Product/Collateral Resolution V5 composer.
pub(crate) mod failure_market_resolution_v5;
/// Capability-disabled aggregate/replay/family-terminal composer.
pub(crate) mod failure_market_family_terminal_v2;
/// Capability-disabled permanent shared-Market Failure replay seam.
pub(crate) mod failure_market_replay_v2;
/// Capability-disabled atomic shared-Market Recovery close.
pub(crate) mod failure_market_recovery_terminal_v2;
pub mod failure_market_admission;
pub mod failure_market_runtime;
/// Withdrawn occurrence-scoped Recovery78/v1 adapter; no current route imports it.
#[cfg(feature = "non-production-failure-recovery-lab")]
pub mod failure_recovery;
mod fractional_lifecycle;
mod fractional_product_consumer;
/// Capability-disabled current Product compiler authority for Failure Begin.
pub(crate) mod product_failure_begin;
pub mod fractional_redemption;
pub(crate) mod full_principal_funding_v1;
/// Deployable current direct-only rent-owned V5 Egg delivery.
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_direct_v5;
/// Exact merge-payment composer and atomic writer.
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_merge_payment_v5;
/// Selected zero-fill Reservation release and atomic close.
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_unfilled_release_v1;
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_fee_v5;
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub(crate) mod general_v2_fee_creation_v6;
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub(crate) mod general_v2_fee_terminal_pair_v1;
/// Action-38 owner cash, Replay, root, and fee-terminal executor.
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_finalize_owner_v5;
/// Action-25 accounting over the exhaustive retained settlement traversal.
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_account_receipt_v5;
/// Actions 36/37 virtual complete-set conversion and real-end delivery.
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_virtual_v5;
/// Action-24 rent-owned V5 materializer.
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_materialize_v5;
mod general_v2_owner_fee_assessment_v6;
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub(crate) mod general_v2_position_replay;
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_receipt_v5;
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_settlement_root;
/// Action-39 indexed-root, compact-index, cash-pot, and fee producer.
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_settlement_producer_v5;
#[cfg(feature = "profile-successor-chain-attached-dev")]
pub(crate) mod general_market_foundation_v4;
/// Ordered indexed-root terminal lifecycle.
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_exact_index_retirement_v1;
/// Typed counted settlement-child retirement and phase gate.
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_settlement_retirement_v1;
/// Nonempty V5 Epoch freeze successor.
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_freeze_v5;
/// Complete coefficient-portfolio settlement and archive retirement.
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_portfolio_v5;
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_portfolio_retirement_v5;
/// Shared immutable Feed/Page/Product traversal authentication for General V5 settlement.
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub mod general_v2_settlement_traversal_v5;
/// One exhaustive dispatcher seam for the current General settlement successor.
#[cfg(any(
    feature = "profile-non-production-general-v2-empty-book-identity-lab",
    feature = "profile-successor-chain-attached-dev"
))]
pub(crate) mod general_v2_successor;
pub mod genesis;
/// Private pre-root Product/Collateral join for the current General V3 founder.
pub(crate) mod general_market_foundation_v3;
#[cfg(test)]
pub mod market_init;
#[cfg(test)]
pub mod merge_materialize;
#[cfg(not(feature = "profile-successor-chain-attached-dev"))]
pub mod observe_resolve;
pub mod orders_batch;
pub mod product_artifact;
/// Disabled narrow Product authority for founding the current General Market owner.
pub(crate) mod product_general_family;
/// Private full-payer capitalization and authentication of Product `0xba/v2`.
pub(crate) mod product_direct_global_liveness;
/// Current RegistryV3/BundleV6/QuoteV5 Failure attempt compiler authority.
pub(crate) mod product_failure_begin_current;
/// Always-compiled Product Market/link account authentication; routes remain capability-gated.
pub mod product_market;
/// Capability-disabled Product FoundationVault/Recovery/founder compositor.
pub(crate) mod product_market_foundation_init;
/// Immutable current five-family mask and namespace-anchor authority.
pub(crate) mod product_market_family_capability_current;
/// Sole current FundingV4/SourceV3 Product founder authority; no capability route is admitted.
pub(crate) mod product_market_foundation_current;
/// Always-compiled Product/Series semantic owner; executable routes remain
/// independently capability-gated, including in source-empty releases.
pub mod product_series;
pub(crate) mod product_series_current;
/// Current Product V3/V5/V6 to Source occurrence publication authority.
pub(crate) mod product_source_current;
#[cfg(not(feature = "profile-successor-chain-attached-dev"))]
pub mod resolution_work;
/// Realm-owned immutable fee-bearing RevenuePolicyV2 founding and private
/// Product/General authentication receipt.
pub mod revenue_policy_v2;
pub mod series_failure_funding;
#[cfg(not(feature = "profile-successor-chain-attached-dev"))]
pub mod source_ingest;
#[cfg(not(feature = "profile-successor-chain-attached-dev"))]
pub mod source_ingest_v2;
pub mod source_series;
pub mod source_series_successor;
pub(crate) mod source_occurrence_foundation_v1;
/// Private Product-retirement consumer for the prepaid Source lifecycle
/// custody. It is always compiled and has no caller-facing dispatcher.
pub(crate) mod source_funding_custody_retirement_v1;
/// Private post-Product-pin Source terminal owner for exact absence/refusal.
/// Failure consumes its postwrite; no caller-facing dispatcher enters here.
pub(crate) mod source_failure_terminal_v1;
/// Post-release bridge from Source absence/refusal to Product's exact current
/// LinkV2 release receipt. Failure owns the default-refusing join.
pub(crate) mod source_failure_product_release_v1;
/// Private successful Source-to-current-Product release binding.
pub(crate) mod source_resolution_product_release_v1;
/// Unrouted private Source terminal composer. It is always compiled so the
/// current final Failure postwrite can implement its default-refusing bridge;
/// no checked capability tuple enters it until the complete chain is admitted.
pub(crate) mod source_terminal_resolution_v5;
pub mod split;
/// Wrapper-signed Structured custody and current full-vector lifecycle.
#[cfg(feature = "profile-successor-chain-attached-dev")]
pub mod structured_custody;
