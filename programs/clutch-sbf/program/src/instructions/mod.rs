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
//! | [`split`] | `Intent::Split` |
//! | [`merge_materialize`] | `Intent::Merge`, `Intent::Materialize`, `Intent::Dematerialize` |
//! | [`market_init`] | `Intent::CreateMarket` |
//! | [`observe_resolve`] | `Intent::FeedAdvance`, `Action::Resolve`, `Action::RedeemInternal` |
//! | [`orders_batch`] | `Intent::PlaceOrder`, `Intent::CancelOrder`, `Intent::SettlePage` |
//!
//! Implemented: split, merge_materialize (Merge/Materialize/Dematerialize),
//! market_init, observe_resolve (FeedAdvance/Resolve/RedeemInternal), and
//! orders_batch's PlaceOrder. CancelOrder and SettlePage refuse with recorded
//! findings (cancellation is unrepresentable in the frozen page format; the
//! relation does not fit an SBF frame).
//! no account, write no byte, and return a refusal.  A stub that validated
//! accounts and *then* refused would be worse, not better — it would suggest
//! that the account list it validated is the right one, and choosing that list
//! is precisely the decision the owning lane has to make.

pub mod market_init;
pub mod merge_materialize;
pub mod observe_resolve;
pub mod orders_batch;
pub mod split;
