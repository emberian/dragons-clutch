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
//! | [`genesis`] | `Intent::InitRealm`, `Intent::InitProfile`, `Intent::InitPriceGrid`, `Intent::InitTerms`, `Intent::InitOrderPage`, `Intent::Endow` |
//! | [`split`] | `Intent::Split` |
//! | [`merge_materialize`] | `Intent::Merge`, `Intent::Materialize`, `Intent::Dematerialize` |
//! | [`market_init`] | `Intent::CreateMarket` |
//! | [`observe_resolve`] | `Intent::FeedAdvance`, `Action::Resolve`, `Action::RedeemInternal` |
//! | [`orders_batch`] | `Intent::PlaceOrder`, `Intent::CancelOrder`, `Intent::SettlePage` |
//!
//! Implemented: genesis (the five account-creating initializers plus `Endow`),
//! split, merge_materialize (Merge/Materialize/Dematerialize), market_init,
//! observe_resolve (FeedAdvance/Resolve/RedeemInternal), and orders_batch's
//! PlaceOrder and CancelOrder (page-v4 tombstone retirement).
//! SettlePage refuses with a recorded finding (the relation does not fit an
//! SBF frame and the page-to-book projection has not landed).  A stub must read
//! no account, write no byte, and return a refusal.  A stub that validated
//! accounts and *then* refused would be worse, not better — it would suggest
//! that the account list it validated is the right one, and choosing that list
//! is precisely the decision the owning lane has to make.
//!
//! [`genesis`] is the only module that creates accounts.  Every other module
//! writes over accounts that arrived already created and correctly sized, and
//! that split is deliberate: the account-creation CPI, the rent computation
//! and the `invoke_signed` seed plumbing are one concern with one owner.

pub mod construction;
pub mod genesis;
pub mod market_init;
pub mod merge_materialize;
pub mod observe_resolve;
pub mod orders_batch;
pub mod split;
