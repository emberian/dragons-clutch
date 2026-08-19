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
//! | [`genesis`] | `Intent::InitRealm`, `Intent::InitProfile`, `Intent::InitPriceGrid`, `Intent::InitTerms`, `Intent::InitOrderPage`, `Intent::Endow` |
//! | [`split`] | `Intent::Split` |
//! | [`merge_materialize`] | `Intent::Merge`, `Intent::Materialize`, `Intent::Dematerialize` |
//! | [`market_init`] | `Intent::CreateMarket` |
//! | [`observe_resolve`] | `Intent::FeedAdvance`, `Action::Resolve`, `Action::RedeemInternal` |
//! | [`orders_batch`] | `Intent::PlaceOrder`, `Intent::CancelOrder`, `Intent::SubmitDirectPage`, `Intent::SettlePage` |
//!
//! Implemented: genesis (the five account-creating initializers plus `Endow`),
//! split, merge_materialize (Merge/Materialize/Dematerialize), market_init,
//! observe_resolve (FeedAdvance/Resolve/RedeemInternal), and orders_batch's
//! PlaceOrder and CancelOrder (page-v4 tombstone retirement), one deterministic
//! two-order `SUBMITTED` Candidate/CandidateFeed constructor, plus one narrow
//! `SettlePage` consumption seam for a preselected, pre-entitled, same-page
//! direct full slice. Candidate verification/selection and entitlement creation
//! remain explicit lifecycle STOPs.
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
pub mod genesis;
pub mod market_init;
pub mod merge_materialize;
pub mod observe_resolve;
pub mod orders_batch;
pub mod resolution_work;
pub mod source_ingest;
pub mod split;
