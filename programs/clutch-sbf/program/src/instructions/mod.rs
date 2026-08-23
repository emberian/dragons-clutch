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
//! | [`collateral_cash_v3`] | withdrawn MarketBindingV1 Endow/Withdraw implementation; no checked dispatch |
//! | [`claim_representation_v3`] | withdrawn MarketBindingV1 Materialize/Dematerialize implementation; no checked dispatch |
//! | [`external_redemption_v3`] | withdrawn MarketBindingV1 external-redemption implementation; no checked dispatch |
//! | [`genesis`] | current `InitRealm`, `InitProfileV2`, direct-only `InitOrderPage`, and exact revenue-record close |
//! | [`split`] | `Intent::Split` |
//! | [`merge_materialize`] | withdrawn lowered-ledger migration implementation; no live dispatch |
//! | [`market_init`] | host-forensic legacy Market founder; no checked dispatch |
//! | [`observe_resolve`] | `Intent::FeedAdvance`, `Action::Resolve`, `Action::RedeemInternal` |
//! | [`source_ingest`] | `Intent::InitSourceSpec`, `Intent::InitSourceArchive`, `Intent::AppendSourceArchive`, `Intent::SealSourceArchive` |
//! | [`source_ingest_v2`] | `Intent::InitSourceSpecV2`, `Intent::InitSourceArchiveV2`, `Intent::AppendSourceArchiveV2`, `Intent::SealSourceArchiveV2` |
//! | [`orders_batch`] | `Intent::PlaceOrder`, `Intent::CancelOrder`, `Intent::SubmitDirectPage`, `Intent::SettlePage`, `Intent::InitClearWork`, `Intent::GrowClearWork`, `Intent::InitEpoch`, `Intent::FreezeEpoch`, `Intent::AdvanceClearWork`, `Intent::AdvanceClearSlices`, `Intent::CompleteClearWork`, `Intent::SubmitCandidate`, `Intent::WriteCandidateFeed`, `Intent::SealCandidate`, `Intent::FinalizeSelection`, `Intent::FreezeEntitlement`, `Intent::EntitleSlice` |
//! | `general_v2_fee_v5` | current counted-root/rent-owned V5 owner fee authentication and action-38 composition; account order remains General-owned |
//! | `general_v2_receipt_v5` | exact SettlementRoot/retained-Feed/PDA authentication for rent-owned General Receipt V5 |
//! | `general_v2_settlement_root` | capability-disabled exact `0xa9/1` PDA/owner/full-body authentication; no dispatch route |
//!
//! Checked profiles route only account families whose current schemas are
//! closed. Historical implementations remain compiled for hostile host
//! fixtures and dependency-lower migration work, but capability admission,
//! request decoding, and dispatch cannot reach them.
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
pub mod direct_selection;
pub mod direct_selection_v3;
pub mod external_exit;
pub mod external_redemption_v3;
/// Capability-disabled reusable Market interval account seam.
#[cfg(feature = "non-production-failure-recovery-lab")]
pub(crate) mod failure_market_interval_v2;
/// Capability-disabled permanent shared-Market Failure replay seam.
#[cfg(feature = "non-production-failure-recovery-lab")]
pub(crate) mod failure_market_replay_v2;
#[cfg(feature = "non-production-failure-recovery-lab")]
pub mod failure_market_admission;
#[cfg(feature = "non-production-failure-recovery-lab")]
pub mod failure_market_runtime;
#[cfg(feature = "non-production-failure-recovery-lab")]
pub mod failure_recovery;
pub mod fractional_redemption;
/// Deployable current direct-only rent-owned V5 Egg delivery.
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab"
))]
pub mod general_v2_direct_v5;
/// Staged-disabled exact merge-payment composer and atomic writer.
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab"
))]
pub mod general_v2_merge_payment_v5;
/// Staged-disabled selected zero-fill Reservation release and atomic close.
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab"
))]
pub mod general_v2_unfilled_release_v1;
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub mod general_v2_fee_v5;
/// Staged action-24 rent-owned V5 materializer; route remains disabled.
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub mod general_v2_materialize_v5;
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab"
))]
pub(crate) mod general_v2_position_replay;
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab"
))]
pub mod general_v2_receipt_v5;
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab"
))]
pub mod general_v2_settlement_root;
/// Staged action-39 producer; the route remains capability-disabled until
/// action-24 materialization is reachable under the same profile.
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub mod general_v2_settlement_producer_v5;
/// Shared immutable Feed/Page/Product traversal authentication for General V5 settlement.
#[cfg(any(
    all(
        feature = "profile-full",
        not(feature = "profile-non-production-dealer-policy-catalog-lab")
    ),
    feature = "profile-non-production-general-v2-empty-book-identity-lab"
))]
pub mod general_v2_settlement_traversal_v5;
pub mod genesis;
pub mod market_init;
pub mod merge_materialize;
pub mod observe_resolve;
pub mod orders_batch;
pub mod product_artifact;
/// Disabled narrow Product authority for founding the current General Market owner.
pub(crate) mod product_general_family;
/// Always-compiled Product Market/link account authentication; routes remain capability-gated.
pub mod product_market;
#[cfg(feature = "non-production-product-series-lab")]
pub mod product_series;
pub mod resolution_work;
pub mod series_failure_funding;
pub mod source_ingest;
pub mod source_ingest_v2;
pub mod source_series;
pub mod split;
