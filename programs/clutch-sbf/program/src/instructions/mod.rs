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
//! | [`cash_exit`] | `Intent::WithdrawCash` |
//! | [`genesis`] | `Intent::InitRealm`, `Intent::InitProfile`, `Intent::InitPriceGrid`, `Intent::InitTerms`, `Intent::InitOrderPage`, `Intent::Endow`, `Intent::CloseRevenuePolicyRecord` |
//! | [`split`] | `Intent::Split` |
//! | [`merge_materialize`] | `Intent::Merge`, `Intent::Materialize`, `Intent::Dematerialize` |
//! | [`market_init`] | `Intent::CreateMarket` |
//! | [`observe_resolve`] | `Intent::FeedAdvance`, `Action::Resolve`, `Action::RedeemInternal` |
//! | [`source_ingest`] | `Intent::InitSourceSpec`, `Intent::InitSourceArchive`, `Intent::AppendSourceArchive`, `Intent::SealSourceArchive` |
//! | [`source_ingest_v2`] | `Intent::InitSourceSpecV2`, `Intent::InitSourceArchiveV2`, `Intent::AppendSourceArchiveV2`, `Intent::SealSourceArchiveV2` |
//! | [`orders_batch`] | `Intent::PlaceOrder`, `Intent::CancelOrder`, `Intent::SubmitDirectPage`, `Intent::SettlePage`, `Intent::InitClearWork`, `Intent::GrowClearWork`, `Intent::InitEpoch`, `Intent::FreezeEpoch`, `Intent::AdvanceClearWork`, `Intent::AdvanceClearSlices`, `Intent::CompleteClearWork`, `Intent::SubmitCandidate`, `Intent::WriteCandidateFeed`, `Intent::SealCandidate`, `Intent::FinalizeSelection`, `Intent::FreezeEntitlement`, `Intent::EntitleSlice` |
//! | `general_v2_fee_terminal` | capability-disabled exact pre/post seam for General action 38; no dispatch route |
//! | `general_v2_receipt_v3` | capability-disabled exact Selected/Feed/PDA authentication for General Receipt V3; no dispatch route |
//!
//! Implemented: genesis (the five account-creating initializers plus `Endow`),
//! split, merge_materialize (Merge/Materialize/Dematerialize), market_init,
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
pub mod construction;
pub mod direct_selection;
pub mod direct_selection_v3;
pub mod external_exit;
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub mod general_v2_fee_terminal;
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub mod general_v2_identity;
#[cfg(feature = "profile-non-production-general-v2-empty-book-identity-lab")]
pub mod general_v2_receipt_v3;
pub mod genesis;
pub mod market_init;
pub mod merge_materialize;
pub mod observe_resolve;
pub mod orders_batch;
#[cfg(feature = "non-production-product-series-lab")]
pub mod product_series;
pub mod resolution_work;
pub mod source_ingest;
pub mod source_ingest_v2;
pub mod split;
