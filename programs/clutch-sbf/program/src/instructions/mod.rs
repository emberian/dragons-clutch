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
//! | [`collateral_cash_v3`] | `Intent::Endow`, `Intent::WithdrawCash` over full-width PositionV3/HoardV2/GEN1 |
//! | [`claim_representation_v3`] | `Intent::Materialize`, `Intent::Dematerialize` over PositionV3/ClaimLedgerV3/GEN1 and the separate claim release |
//! | [`external_redemption_v3`] | `Intent::RedeemExternal` over ResolutionV5/HoardV2/ClaimLedgerV3 and bearer Token-2022 claims |
//! | [`genesis`] | `Intent::InitRealm`, `Intent::InitProfileV2`, `Intent::InitPriceGrid`, `Intent::InitTerms`, `Intent::InitOrderPage`, `Intent::CloseRevenuePolicyRecord` |
//! | [`split`] | `Intent::Split` |
//! | [`merge_materialize`] | withdrawn lowered-ledger migration implementation; no live dispatch |
//! | [`market_init`] | `Intent::CreateMarket` |
//! | [`observe_resolve`] | `Intent::FeedAdvance`, `Action::Resolve`, `Action::RedeemInternal` |
//! | [`source_ingest`] | `Intent::InitSourceSpec`, `Intent::InitSourceArchive`, `Intent::AppendSourceArchive`, `Intent::SealSourceArchive` |
//! | [`source_ingest_v2`] | `Intent::InitSourceSpecV2`, `Intent::InitSourceArchiveV2`, `Intent::AppendSourceArchiveV2`, `Intent::SealSourceArchiveV2` |
//! | [`orders_batch`] | `Intent::PlaceOrder`, `Intent::CancelOrder`, `Intent::SubmitDirectPage`, `Intent::SettlePage`, `Intent::InitClearWork`, `Intent::GrowClearWork`, `Intent::InitEpoch`, `Intent::FreezeEpoch`, `Intent::AdvanceClearWork`, `Intent::AdvanceClearSlices`, `Intent::CompleteClearWork`, `Intent::SubmitCandidate`, `Intent::WriteCandidateFeed`, `Intent::SealCandidate`, `Intent::FinalizeSelection`, `Intent::FreezeEntitlement`, `Intent::EntitleSlice` |
//! | `general_v2_fee_terminal` | capability-disabled exact pre/post seam for General action 38; no dispatch route |
//! | `general_v2_receipt_v5` | exact SettlementRoot/retained-Feed/PDA authentication for rent-owned General Receipt V5 |
//! | `general_v2_settlement_root` | capability-disabled exact `0xa9/1` PDA/owner/full-body authentication; no dispatch route |
//!
//! Implemented: genesis (the five account-creating initializers), full-width
//! collateral_cash_v3 (Endow/WithdrawCash), claim_representation_v3
//! (Materialize/Dematerialize), complete_set_v3 (Split/Merge), market_init,
//! observe_resolve (FeedAdvance/Resolve/RedeemInternal), and the whole Tier 2
//! general clearing lifecycle in orders_batch: funded placement and
//! cancellation, the general epoch open/freeze, the on-chain streaming walk
//! to a verified verdict, candidate submission/selection, the entitlement
//! freeze (verified-summary pot + per-slice receipts, `ACTIVE → ENTITLED`),
//! and entitled `SettlePage` consumption for single-Egg direct slices and
//! portfolio full pairs.  The standing refusals — partial fills, virtual
//! pots, terminal closure — live in `orders_batch::settlement`'s ledger.
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
#[cfg(feature = "non-production-failure-recovery-lab")]
pub mod failure_interval_consensus;
#[cfg(feature = "non-production-failure-recovery-lab")]
pub mod failure_market_admission;
#[cfg(feature = "non-production-failure-recovery-lab")]
pub mod failure_recovery;
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub mod general_v2_fee_terminal;
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub mod general_v2_identity;
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub mod general_v2_receipt_v5;
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub mod general_v2_settlement_root;
pub mod genesis;
pub mod market_init;
pub mod merge_materialize;
pub mod observe_resolve;
pub mod orders_batch;
pub mod product_artifact;
#[cfg(feature = "non-production-product-series-lab")]
pub mod product_series;
pub mod resolution_work;
pub mod series_failure_funding;
pub mod source_ingest;
pub mod source_ingest_v2;
pub mod source_series;
pub mod split;
