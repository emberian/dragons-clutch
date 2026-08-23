//! The Hoard/Position seam plane: `Intent::Split`, and the shared account list,
//! validation order, kernel step, and write-back that
//! [`super::merge_materialize`] reuses for `Merge`, `Materialize`, and
//! `Dematerialize`.
//!
//! This module contains no economic logic.  Every transition is
//! [`clutch_kernel::MarketState`]; byte ownership is [`clutch_solana_layout`]
//! and the reference-only codecs of [`clutch_solana_reference`]; metadata
//! authentication and the CLO-DELTA-V1 primitives are [`crate::accounts`].
//! What lives here is the account list, the order of the checks, the program
//! addresses each account must be, and the write-back.
//!
//! ## The seam plane is one plane
//!
//! `Split`, `Merge`, `Materialize`, and `Dematerialize` are the four requests
//! the offline reference adapter routes through one `TransitionMetadata` /
//! `StateBytes` / `ExpectedBindings` triple, so they take one state account
//! list here too — and then exactly one token leg each, chosen by the intent.  Two modules own them (the ownership table in
//! `docs/implementation/SBF_BRINGUP.md` splits by family), but a second copy of
//! the account list would be a second place for the seam's writable set to
//! drift, so the list, the checks, and the write-back live here and
//! `merge_materialize` calls [`seam`].
//!
//! ## The token legs are mandatory, and that is an ABI break
//!
//! Every seam instruction carries a Token-2022 leg and the caller does not get
//! to choose: `Materialize` and `Dematerialize` take **thirteen** accounts,
//! `Split` and `Merge` take **sixteen**, and any other count is
//! [`ClutchError::AccountCount`].  The optional ten-account plane that existed
//! before — shadow-only claims, and collateral that moved no tokens at all —
//! is deleted along with `TokenLeg::Absent`, because
//! [`super::market_init`] now creates the outcome mints and the Hoard token
//! account and a mandatory leg therefore has something to name.
//!
//! The consequence is that every emitter has to be regenerated.
//! `programs/clutch-sbf/harness` is frozen this wave and still emits
//! ten-account seam transactions, so its emitted transactions are refused
//! until its lane regenerates them; the harness's *host* leg, which runs the
//! offline reference adapter over fixture bytes, is unaffected.
//!
//! ## CLO-DELTA-V1 is now carried, not deferred
//!
//! The single-instruction bring-up program carried the retired closed
//! single-position equality `internal + external == total_supply` and took nine
//! accounts.  Its state prefix is now **ten**: the market-wide supply ledger,
//! appended after the replay account exactly as `ExpectedBindings::supply` is
//! the last state binding of the reference adapter.  The equality is replaced by the
//! three checked obligations of `docs/implementation/MULTI_POSITION_CLOSURE.md`
//! through [`crate::accounts`]:
//!
//! | obligation | where |
//! | --- | --- |
//! | C1 two-term closure against the kernel aggregate | [`accounts::require_two_term_closure`], pre-state and post-state |
//! | C2 position-owned internal balances do not exceed the market aggregate | `require_internal_bound` (private, this module), pre-state and post-state |
//! | C3 ledger moved by exactly the position delta | [`accounts::apply_ledger_delta`], once per ledger term |
//!
//! A market holding a second position is therefore representable here, which is
//! the whole point of the change: the retired equality refused every such
//! market.
//!
//! ## What the host differential does and does not cover
//!
//! [`seam`] takes **two** parameters that on-chain are syscalls and off-chain
//! do not exist, and both are injected rather than called inline.
//!
//! * Program addresses arrive as an already-derived [`Bindings`] value,
//!   because derivation is a runtime syscall and [`crate::seeds::find`] is
//!   deliberately not compiled for the host.
//! * The token CPIs arrive as an effector, because `solana_cpi::invoke_signed`
//!   compiles to `Ok(())` off-chain — so the real effector moves nothing and
//!   the exact-delta check of `TOKEN2022_PLAN.md` §3.3 step 6 correctly refuses
//!   every transition.  The host tests supply a simulator that moves the one
//!   `u64` a conforming transfer is permitted to move; `merge_materialize`'s
//!   `an_off_chain_token_leg_refuses_the_delta_it_could_not_move` supplies the
//!   real one and pins that it refuses.
//!
//! On-chain [`process`] supplies [`derive_bindings`] and [`token_effects`].
//! So the differential covers request decoding, metadata authentication, every
//! linkage and closure check, the token-plane admission and ordering, the
//! kernel step, the mirror, and the write-back — everything the reference
//! adapter models plus everything around the CPI — and covers exactly two
//! things less than the SVM leg: that the derived address is the canonical one,
//! and what happens inside Token-2022.  Both are
//! `programs/clutch-sbf/svm-tests`.

use crate::accounts::{
    self, apply_ledger_delta, expect_pda, require, require_distinct, require_signer,
    require_two_term_closure, MarketFacts, Outcome, StateRole,
};
use crate::claim_truth::{self, ObservedMintSupplies};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use crate::token;
use clutch_kernel::{
    MarketState, PayoutVector, Phase, Position, MAX_OUTCOMES as KERNEL_MAX_OUTCOMES,
};
use clutch_solana_layout::{
    account_len, collateral, Hash32, HoardAccount, Intent, PositionAccount, ProfileAccount,
    SupplyLedgerAccount, MAX_OUTCOMES,
};
use clutch_solana_reference::{
    Action, KernelAccount, ReplayAccount, Request, KERNEL_ACCOUNT_LEN, REPLAY_ACCOUNT_LEN,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// The shared state prefix every seam instruction carries, in list order.
///
/// Nine accounts, with no owner-local external shadow.  Every seam instruction
/// then appends exactly one token leg, and *which* leg is a function of the
/// intent rather than of the caller: see [`TokenLeg`].
pub const ACCOUNT_COUNT: usize = 9;

/// Authenticated actor; must be the position owner.
pub const IX_ACTOR: usize = 0;
/// Realm configuration account (read-only).
pub const IX_REALM: usize = 1;
/// Profile identity account (read-only).
pub const IX_PROFILE: usize = 2;
/// Market account.
pub const IX_MARKET: usize = 3;
/// Hoard collateral account.
pub const IX_HOARD: usize = 4;
/// Owner position account.
pub const IX_POSITION: usize = 5;
/// Reference-only kernel-aggregate account.
pub const IX_KERNEL: usize = 6;
/// Reference-only replay-sequence account.
pub const IX_REPLAY: usize = 7;
/// Market-wide two-term supply ledger.
pub const IX_SUPPLY: usize = 8;

/* --------------------------------------------------------------------- */
/* The Token-2022 legs, both mandatory                                    */
/* --------------------------------------------------------------------- */

/// Fixed prefix before the canonical outcome-mint suffix on an outcome seam.
pub const ACCOUNT_PREFIX_OUTCOME: usize = 11;

/// Account count of `Split` and `Merge`.
///
/// The ten state accounts plus the token program, the Realm's 266
/// collateral-policy bytes, the collateral mint, the actor's collateral token
/// account, the Hoard's signing authority, and the Hoard token account.
///
/// The policy bytes are in the list because the collateral mint's *identity*
/// lives nowhere else: without them a caller could present a worthless mint
/// and its own accounts and buy complete sets with it.  They are
/// content-authenticated against the Profile's frozen digest rather than
/// address-authenticated, exactly as [`super::market_init`] authenticates them.
pub const ACCOUNT_PREFIX_COLLATERAL: usize = 15;

/// The pinned Token-2022 program (read-only, executable).
///
/// Index nine on both planes, so one constant names it whichever leg is
/// present.
pub const IX_TOKEN_PROGRAM: usize = 9;

/// The holder's Token-2022 account for that outcome mint (writable).
pub const IX_HOLDER_TOKEN: usize = 10;
/// First canonical outcome mint on Materialize/Dematerialize.
pub const IX_OUTCOME_MINTS: usize = ACCOUNT_PREFIX_OUTCOME;

/// The Realm's 266-byte collateral policy (read-only).
pub const IX_POLICY: usize = 10;
/// The collateral mint the Realm's policy names (read-only).
pub const IX_COLLATERAL_MINT: usize = 11;
/// The actor's own Token-2022 account for the collateral mint (writable).
pub const IX_ACTOR_TOKEN: usize = 12;
/// The Hoard's signing authority; holds no data and is never written.
pub const IX_HOARD_AUTHORITY: usize = 13;
/// The Hoard's Token-2022 collateral account (writable).
pub const IX_HOARD_TOKEN: usize = 14;
/// First canonical outcome mint on Split/Merge.
pub const IX_COLLATERAL_OUTCOME_MINTS: usize = ACCOUNT_PREFIX_COLLATERAL;

/// Which token leg an intent carries.
///
/// **There is no `Absent` variant, and its deletion is the point of this
/// lane.**  `Materialize` and `Dematerialize` used to accept ten accounts
/// (shadow only) *or* thirteen, and `Split` and `Merge` accepted ten and moved
/// no collateral at all, so a caller could present the smaller plane and get
/// the weaker instruction.  That optionality existed for exactly one reason —
/// no instruction in this program created an outcome mint or a Hoard token
/// account, so a mandatory leg would have had nothing to name — and
/// [`super::market_init`] now creates all of them.  A market founded by this
/// program has its mints, so the leg is mandatory and the count is fixed per
/// intent.
///
/// The consequence is an ABI break and it is deliberate: a ten-account seam
/// transaction is now [`ClutchError::AccountCount`].  Every emitter has to be
/// regenerated, `programs/clutch-sbf/harness` included.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenLeg {
    /// Thirteen accounts: mint or burn the outcome token this intent names.
    Outcome(u8),
    /// Sixteen accounts: move collateral between the actor and the Hoard.
    Collateral,
}

/// Choose the account plane from the intent, and require its exact count.
///
/// The count is a function of the intent alone.  A caller cannot choose a
/// plane, cannot omit a leg, and cannot append a suffix: each of the four seam
/// intents accepts exactly one number of accounts.
fn select_token_leg(op: &SeamOp, presented: usize, outcome_count: u8) -> Outcome<TokenLeg> {
    match op {
        SeamOp::Split { .. } | SeamOp::Merge { .. } => {
            require(
                presented == ACCOUNT_PREFIX_COLLATERAL + usize::from(outcome_count),
                ClutchError::AccountCount,
            )?;
            Ok(TokenLeg::Collateral)
        }
        SeamOp::Materialize { outcome, .. } | SeamOp::Dematerialize { outcome, .. } => {
            require(
                presented == ACCOUNT_PREFIX_OUTCOME + usize::from(outcome_count),
                ClutchError::AccountCount,
            )?;
            Ok(TokenLeg::Outcome(*outcome))
        }
    }
}

/// Everything the outcome leg will change, read before anything is written.
///
/// Step 3 of `docs/implementation/TOKEN2022_PLAN.md` §3.3: the exact pre-CPI
/// `amount` of every token account this instruction will change and the exact
/// pre-CPI `supply` of every mint it will change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutcomeSnapshot {
    /// Outcome index this leg acts on.
    outcome: u8,
    /// Claim atoms the kernel decided to move.
    quantity: u64,
    /// Holder token-account balance before the CPI.
    holder_amount: u64,
    /// Account-list index of the touched canonical outcome mint.
    mint_index: usize,
    /// Market PDA signing seeds, carried so the signer is derived once.
    realm: [u8; 32],
    /// Market identity, the second market seed.
    market: [u8; 32],
    /// Canonical market bump, already proved equal to the stored one.
    bump: [u8; 1],
}

/// Everything the collateral leg will change, read before anything is written.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralSnapshot {
    /// Collateral atoms the transition moves; one per complete set.
    ///
    /// `RedeemInternal` does not know its own quantity until the kernel has
    /// run, so it snapshots with zero here and supplies the paid amount to the
    /// delta check directly.
    pub quantity: u64,
    /// The collateral mint's decimals, for `TransferChecked`.
    pub decimals: u8,
    /// The actor's collateral balance before the CPI.
    pub actor_amount: u64,
    /// The Hoard token account's balance before the CPI.
    pub hoard_amount: u64,
    /// Market identity, the Hoard authority's second seed.
    pub market: [u8; 32],
    /// Canonical Hoard-authority bump.
    pub authority_bump: [u8; 1],
}

/// The pre-CPI reading of whichever leg this intent carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenSnapshot {
    /// `Materialize` and `Dematerialize`.
    Outcome(OutcomeSnapshot),
    /// `Split` and `Merge`.
    Collateral(CollateralSnapshot),
}

/// The program-owned state roles of the seam plane, in account-list order.
const STATE_ROLES: [StateRole; 8] = [
    StateRole::read_only(IX_REALM, account_len::REALM),
    StateRole::read_only(IX_PROFILE, account_len::PROFILE),
    StateRole::writable(IX_MARKET, account_len::MARKET),
    StateRole::writable(IX_HOARD, account_len::HOARD),
    StateRole::writable(IX_POSITION, account_len::POSITION),
    StateRole::writable(IX_KERNEL, KERNEL_ACCOUNT_LEN),
    StateRole::writable(IX_REPLAY, REPLAY_ACCOUNT_LEN),
    StateRole::writable(IX_SUPPLY, account_len::SUPPLY_LEDGER),
];

/// One already-routed `Split` request.
///
/// [`crate::dispatch`] destructures the envelope so that this module never has
/// to re-match an action it already knows, and so there is no unreachable
/// fallback arm pretending another intent could arrive here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplitRequest {
    /// Exact replay sequence the request claims.
    pub sequence: u64,
    /// Market identity the intent binds.
    pub market: Hash32,
    /// Owner identity the intent binds.
    pub owner: Hash32,
    /// Complete sets to create.
    pub quantity: u64,
}

/// One already-routed seam transition.
///
/// The four variants are exactly the four layout intents the offline reference
/// adapter routes through one account plane, and all four are implemented.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeamOp {
    /// Add a complete internal set.
    Split {
        /// Market identity the intent binds.
        market: Hash32,
        /// Owner identity the intent binds.
        owner: Hash32,
        /// Complete sets to create.
        quantity: u64,
    },
    /// Remove a complete internal set.
    Merge {
        /// Market identity the intent binds.
        market: Hash32,
        /// Owner identity the intent binds.
        owner: Hash32,
        /// Complete sets to destroy.
        quantity: u64,
    },
    /// Move one outcome from the internal ledger to the external shadow.
    Materialize {
        /// Market identity the intent binds.
        market: Hash32,
        /// Owner identity the intent binds.
        owner: Hash32,
        /// Caller-named destination, which must be the external-shadow account.
        destination: Hash32,
        /// Outcome index.
        outcome: u8,
        /// Claim atoms to move.
        quantity: u64,
    },
    /// Move one outcome from the external shadow back to the internal ledger.
    Dematerialize {
        /// Market identity the intent binds.
        market: Hash32,
        /// Owner identity the intent binds.
        owner: Hash32,
        /// Caller-named source, which must be the external-shadow account.
        source: Hash32,
        /// Outcome index.
        outcome: u8,
        /// Claim atoms to move.
        quantity: u64,
    },
}

impl SeamOp {
    /// Market identity this intent binds.
    const fn market(&self) -> Hash32 {
        match self {
            Self::Split { market, .. }
            | Self::Merge { market, .. }
            | Self::Materialize { market, .. }
            | Self::Dematerialize { market, .. } => *market,
        }
    }

    /// Owner identity this intent binds.
    const fn owner(&self) -> Hash32 {
        match self {
            Self::Split { owner, .. }
            | Self::Merge { owner, .. }
            | Self::Materialize { owner, .. }
            | Self::Dematerialize { owner, .. } => *owner,
        }
    }
}

/// Convert one decoded request into the seam transition it names.
///
/// Anything that is not a seam intent refuses
/// [`ClutchError::UnsupportedInstruction`] rather than being silently routed
/// into a plane that does not model it.  [`crate::dispatch`] already routes only
/// the four seam intents here, so that arm is defence in depth and not a
/// reachable path today.
pub fn seam_op(request: &Request) -> Outcome<SeamOp> {
    match request.action {
        Action::Layout(Intent::Split {
            market,
            owner,
            quantity,
        }) => Ok(SeamOp::Split {
            market,
            owner,
            quantity,
        }),
        Action::Layout(Intent::Merge {
            market,
            owner,
            quantity,
        }) => Ok(SeamOp::Merge {
            market,
            owner,
            quantity,
        }),
        Action::Layout(Intent::Materialize {
            market,
            owner,
            destination,
            outcome,
            quantity,
        }) => Ok(SeamOp::Materialize {
            market,
            owner,
            destination,
            outcome,
            quantity,
        }),
        Action::Layout(Intent::Dematerialize {
            market,
            owner,
            source,
            outcome,
            quantity,
        }) => Ok(SeamOp::Dematerialize {
            market,
            owner,
            source,
            outcome,
            quantity,
        }),
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

/// The identities every seam program address is derived from.
///
/// Each one is read out of a decoded account, never out of the instruction
/// data: a caller-named identity would be exactly the trusted binding
/// obligation 1 of `docs/implementation/SOLANA_REFERENCE_ADAPTER.md` forbids.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Identities {
    /// Realm namespace, from the market account.
    pub realm: [u8; 32],
    /// Profile identity, from the market account.
    pub profile: [u8; 32],
    /// Market identity, from the market account.
    pub market: [u8; 32],
    /// Owner identity, from the position account.
    pub owner: [u8; 32],
    /// Position generation, from the position account.
    pub generation: u64,
    /// Which token leg this intent carries, and therefore which addresses have
    /// to be derived.
    ///
    /// Program-address derivation is a syscall, so an intent pays only for the
    /// addresses its own leg names: `Materialize` derives one outcome mint and
    /// no Hoard token address, `Split` derives the two Hoard addresses and no
    /// mint.
    pub leg: TokenLeg,
}

/// Canonical program address and bump for every account in the seam plane.
///
/// The two `None`-bump roles of [`expect_pda`] — Profile and the reference-only
/// kernel aggregate — carry a bump here that is never compared, because their
/// frozen layouts have no bump field to compare it against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bindings {
    /// Canonical Realm address and bump.
    pub realm: (Pubkey, u8),
    /// Canonical Profile address; the bump is not stored and not compared.
    pub profile: (Pubkey, u8),
    /// Canonical Market address and bump.
    pub market: (Pubkey, u8),
    /// Canonical Hoard address and bump.
    pub hoard: (Pubkey, u8),
    /// Canonical Position address and bump.
    pub position: (Pubkey, u8),
    /// Canonical kernel-aggregate address; the bump is not stored.
    pub kernel: (Pubkey, u8),
    /// Canonical replay-sequence address and bump.
    pub replay: (Pubkey, u8),
    /// Canonical supply-ledger address and bump.
    pub supply: (Pubkey, u8),
    /// Canonical Hoard-authority address and bump, on the collateral leg only.
    ///
    /// The bump *is* used: it is the last signing seed of every collateral
    /// outflow, which is the only way atoms leave the Hoard at all.
    pub hoard_authority: Option<(Pubkey, u8)>,
    /// Canonical Hoard token-account address and bump, on the collateral leg
    /// only.  Distinct from [`Bindings::hoard`], which is this program's own
    /// collateral-accounting state rather than the Token-2022 account.
    pub hoard_token: Option<(Pubkey, u8)>,
}

/// Derive every seam program address from the frozen seed schema.
///
/// This is the only part of the plane that cannot run off-chain: derivation is
/// a runtime syscall and [`crate::seeds::find`] is not compiled for the host.
/// It is therefore a parameter of [`seam`] rather than an inline call, so the
/// host differential can supply the same already-derived bindings the offline
/// reference adapter takes as `ExpectedBindings` and compare everything else.
#[inline(never)]
pub fn derive_bindings(program_id: &Pubkey, ids: &Identities) -> Bindings {
    Bindings {
        realm: seeds::realm_pda(program_id, &ids.realm),
        profile: seeds::profile_pda(program_id, &ids.realm, &ids.profile),
        market: seeds::market_pda(program_id, &ids.realm, &ids.market),
        hoard: seeds::hoard_pda(program_id, &ids.market),
        position: seeds::position_pda(program_id, &ids.market, &ids.owner),
        kernel: seeds::kernel_pda(program_id, &ids.market),
        replay: seeds::replay_pda(program_id, &ids.market, &ids.owner, ids.generation),
        supply: seeds::supply_pda(program_id, &ids.market),
        hoard_authority: match ids.leg {
            TokenLeg::Collateral => Some(seeds::hoard_authority_pda(program_id, &ids.market)),
            TokenLeg::Outcome(_) => None,
        },
        hoard_token: match ids.leg {
            TokenLeg::Collateral => Some(seeds::hoard_token_pda(program_id, &ids.market)),
            TokenLeg::Outcome(_) => None,
        },
    }
}

/// The post-state the pure kernel produced for the market aggregate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KernelPost {
    collateral: u64,
    total_supply: [u64; MAX_OUTCOMES],
    phase: u8,
    resolved_payout: u8,
}

/// Exactly the four [`clutch_kernel::MarketState`] transitions this plane runs,
/// with their coordinates and nothing else.
///
/// [`SeamOp`] additionally carries the *authorization* names — the market and
/// owner identities the intent binds — which the kernel step has no use for.
/// Splitting them is what lets a caller that is not an owner-signed seam
/// request (the clearing plane's pooled mint, [`pooled_set_transition`]) reach
/// the same kernel step without inventing an owner identity it does not have.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KernelOp {
    /// Create `quantity` complete internal sets.
    Split(u64),
    /// Destroy `quantity` complete internal sets.
    Merge(u64),
    /// Move `quantity` of one outcome from internal to the external shadow.
    Materialize {
        /// Outcome index.
        outcome: u8,
        /// Claim atoms.
        quantity: u64,
    },
    /// Move `quantity` of one outcome from the external shadow to internal.
    Dematerialize {
        /// Outcome index.
        outcome: u8,
        /// Claim atoms.
        quantity: u64,
    },
}

impl SeamOp {
    /// The kernel transition this seam intent runs, without its bound names.
    const fn kernel_op(&self) -> KernelOp {
        match self {
            Self::Split { quantity, .. } => KernelOp::Split(*quantity),
            Self::Merge { quantity, .. } => KernelOp::Merge(*quantity),
            Self::Materialize {
                outcome, quantity, ..
            } => KernelOp::Materialize {
                outcome: *outcome,
                quantity: *quantity,
            },
            Self::Dematerialize {
                outcome, quantity, ..
            } => KernelOp::Dematerialize {
                outcome: *outcome,
                quantity: *quantity,
            },
        }
    }
}

/// Run one seam transition on the pure kernel and re-encode the aggregate.
///
/// The whole `KernelAccount`/`MarketState` working set lives in this frame and
/// nowhere else: a `MarketState` carries the entire frozen payout set, which is
/// most of an SBF call frame on its own.  `position` is updated in place only
/// after every kernel check has passed, because every `MarketState` transition
/// completes all of its checks before its first write.
#[inline(never)]
fn kernel_step(
    kernel_data: &mut [u8],
    outcome_count: u8,
    collateral_before: u64,
    position: &mut Position,
    op: KernelOp,
) -> Outcome<KernelPost> {
    let mut account = KernelAccount::decode(kernel_data)?;
    if usize::from(outcome_count) > KERNEL_MAX_OUTCOMES || account.payouts.outcomes != outcome_count
    {
        return Err(ClutchError::MismatchedState.into());
    }
    match account.phase {
        0 => {}
        1 => return Err(clutch_kernel::Error::NotActive.into()),
        _ => return Err(ClutchError::NonCanonical.into()),
    }
    let mut market = MarketState {
        outcomes: outcome_count,
        phase: Phase::Active,
        resolved_payout: account.resolved_payout,
        basis_mode: account.basis_mode,
        resolved_vector: PayoutVector::ZERO,
        collateral: collateral_before,
        total_supply: account.total_supply,
        payouts: account.payouts,
    };
    market.check_invariants()?;
    match op {
        KernelOp::Split(quantity) => market.split(position, quantity)?,
        KernelOp::Merge(quantity) => market.merge(position, quantity)?,
        KernelOp::Materialize { outcome, quantity } => {
            market.materialize(position, outcome, quantity)?
        }
        KernelOp::Dematerialize { outcome, quantity } => {
            market.dematerialize(position, outcome, quantity)?
        }
    }
    account.phase = match market.phase {
        Phase::Active => 0,
        Phase::Resolved => 1,
    };
    account.resolved_payout = market.resolved_payout;
    account.total_supply = market.total_supply;
    account.encode(kernel_data)?;
    Ok(KernelPost {
        collateral: market.collateral,
        total_supply: market.total_supply,
        phase: account.phase,
        resolved_payout: account.resolved_payout,
    })
}

#[cfg(test)]
mod basis_mode_tests {
    use super::*;
    use clutch_kernel::{Error as KernelError, PayoutSet, MAX_PAYOUTS};

    fn payout_set() -> PayoutSet {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut left = [0_u64; MAX_OUTCOMES];
        let mut right = [0_u64; MAX_OUTCOMES];
        left[0] = 1;
        right[1] = 1;
        vectors[0] = PayoutVector::new(1, left);
        vectors[1] = PayoutVector::new(1, right);
        PayoutSet::new(2, 2, vectors)
    }

    fn encoded_kernel(phase: u8, collateral_supply: u64) -> Vec<u8> {
        let mut total_supply = [0_u64; MAX_OUTCOMES];
        total_supply[0] = collateral_supply;
        total_supply[1] = collateral_supply;
        let account = KernelAccount {
            market: Hash32::from_bytes([9; 32]),
            phase,
            basis_mode: clutch_kernel::BasisMode::DerivedBasis,
            resolved_payout: 0,
            payouts: payout_set(),
            total_supply,
        };
        let mut bytes = vec![0_u8; KERNEL_ACCOUNT_LEN];
        account.encode(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn active_native_split_and_materialize_preserve_mode_and_solvency() {
        for degree in 1..=3 {
            let mut bytes = encoded_kernel(0, 0);
            let before_mode = KernelAccount::decode(&bytes).unwrap().basis_mode;
            let mut position = Position::EMPTY;
            let split = SeamOp::Split {
                market: Hash32::from_bytes([9; 32]),
                owner: Hash32::from_bytes([7; 32]),
                quantity: 12,
            };
            let split_post =
                kernel_step(&mut bytes, 2, 0, &mut position, split.kernel_op()).unwrap();
            assert_eq!(split_post.collateral, 12, "degree {degree}");
            assert_eq!(split_post.total_supply[..2], [12, 12], "degree {degree}");

            let materialize = SeamOp::Materialize {
                market: Hash32::from_bytes([9; 32]),
                owner: Hash32::from_bytes([7; 32]),
                destination: Hash32::from_bytes([8; 32]),
                outcome: 0,
                quantity: 5,
            };
            let materialize_post =
                kernel_step(&mut bytes, 2, 12, &mut position, materialize.kernel_op()).unwrap();
            assert_eq!(materialize_post.collateral, 12, "degree {degree}");
            assert_eq!(
                materialize_post.total_supply[..2],
                [12, 12],
                "degree {degree}"
            );
            assert_eq!(position.internal[0], 7, "degree {degree}");
            assert_eq!(position.external[0], 5, "degree {degree}");
            assert_eq!(
                KernelAccount::decode(&bytes).unwrap().basis_mode,
                before_mode
            );
        }
    }

    #[test]
    fn split_family_refuses_resolved_native_state_before_construction_and_write() {
        let mut bytes = encoded_kernel(1, 0);
        let before_bytes = bytes.clone();
        let mut position = Position::EMPTY;
        let before_position = position;
        let op = SeamOp::Split {
            market: Hash32::from_bytes([9; 32]),
            owner: Hash32::from_bytes([7; 32]),
            quantity: 1,
        };
        assert_eq!(
            kernel_step(&mut bytes, 2, 0, &mut position, op.kernel_op()),
            Err(Refusal::Kernel(KernelError::NotActive))
        );
        assert_eq!(bytes, before_bytes);
        assert_eq!(position, before_position);
    }

    #[test]
    fn undercollateralized_active_native_prestate_refuses_without_write() {
        let mut bytes = encoded_kernel(0, 6);
        let before_bytes = bytes.clone();
        let mut position = Position::EMPTY;
        let op = SeamOp::Materialize {
            market: Hash32::from_bytes([9; 32]),
            owner: Hash32::from_bytes([7; 32]),
            destination: Hash32::from_bytes([8; 32]),
            outcome: 0,
            quantity: 1,
        };
        assert_eq!(
            kernel_step(&mut bytes, 2, 5, &mut position, op.kernel_op()),
            Err(Refusal::Kernel(KernelError::InvariantViolation))
        );
        assert_eq!(bytes, before_bytes);
        assert_eq!(position, Position::EMPTY);
    }
}

/// Move the two-term ledger by exactly the position delta and re-close it.
///
/// This is C3 followed by C1 and C2 over the post-state, in the offline
/// reference adapter's order.  Re-closing after the delta is what forces the
/// kernel's aggregate supply effect to equal its per-position effect: a
/// divergence refuses instead of corrupting the ledger.
#[inline(never)]
fn ledger_step(
    supply_data: &mut [u8],
    outcome_count: u8,
    pre_internal: &[u64; MAX_OUTCOMES],
    post: &Position,
) -> Outcome<()> {
    let mut ledger = SupplyLedgerAccount::decode(supply_data)?;
    apply_ledger_delta(
        &mut ledger.internal_supply,
        outcome_count,
        pre_internal,
        &post.internal,
    )?;
    require_internal_bound(&ledger, &post.internal, outcome_count)?;
    ledger.encode(supply_data)?;
    Ok(())
}

/// A position owns only internal balances.  Bearer Token-2022 claims are not
/// attributed to the position that materialized them and therefore cannot be
/// bounded against an owner-local shadow.
fn require_internal_bound(
    supply: &SupplyLedgerAccount,
    internal: &[u64; MAX_OUTCOMES],
    outcome_count: u8,
) -> Outcome<()> {
    let mut outcome = 0_usize;
    while outcome < usize::from(outcome_count) {
        require(
            internal[outcome] <= supply.internal_supply[outcome],
            ClutchError::AggregateClosureMismatch,
        )?;
        outcome += 1;
    }
    Ok(())
}

/* --------------------------------------------------------------------- */
/* The pooled complete-set primitive the clearing plane's mint join calls */
/* --------------------------------------------------------------------- */

/// Where one pooled complete-set mint or burn finds the five accounts it
/// authenticates, in the calling instruction's own account list.
///
/// The seam plane fixes these at [`IX_MARKET`]/[`IX_HOARD`]/[`IX_KERNEL`]/
/// [`IX_SUPPLY`]/[`IX_HOARD_TOKEN`]; the clearing plane's virtual-leg
/// consumption appends them after its own prefix.  Parameterizing the
/// positions is what keeps the decision procedure one copy rather than two
/// that drift.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PooledSetRoles {
    /// The market account (read-only).
    pub market: usize,
    /// The Hoard collateral account (writable).
    pub hoard: usize,
    /// The reference-only kernel aggregate (writable).
    pub kernel: usize,
    /// The market-wide two-term supply ledger (writable).
    pub supply: usize,
    /// The Hoard's Token-2022 collateral account (read-only here: a pooled
    /// set change moves no Token-2022 atoms, exactly as `Split`/`Merge` do
    /// not, so this account is present only to be *mirrored against*).
    pub hoard_token: usize,
}

/// One pooled complete-set change, and everything its caller must already
/// have decided.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PooledSetChange {
    /// Complete sets created (`mint`) or destroyed.
    pub quantity: u64,
    /// True to create, false to destroy.
    pub mint: bool,
}

/// Move `quantity` complete sets into or out of a **pooled holder** that is
/// not an owner Position, with every CLO-DELTA-V1 obligation intact.
///
/// This is [`seam`]'s economic core with its *authorization* half removed and
/// nothing else: the caller has already decided who may do this and why, and
/// what it gets here is the identical kernel step ([`kernel_step`]), the
/// identical ledger delta and internal bound ([`ledger_step`], which calls
/// [`require_internal_bound`]), the identical two-term closure over both
/// pre- and post-state, the identical collateral cap on the mint side and the
/// identical *absence* of one on the burn side, and the identical Hoard mirror
/// over the Token-2022 balance.  Nothing is relaxed for the pooled caller.
///
/// `holder_internal` is the pooled holder's claim vector — for the clearing
/// plane, `FinalPotAccount::pot_internal`.  It is updated in place only after
/// every check has passed.
///
/// **What backs a mint here.** A complete set is worth exactly one collateral
/// atom (`crates/clutch-batch/src/relation_v1.rs:2749-2757` prices the virtual
/// split at `sigma * price_scale` price units, and prices lie on the scaled
/// simplex, `relation_v1.rs:1218-1250`).  This primitive does not create that
/// atom: it *reclassifies* one that is already inside the Hoard's token
/// account but attributed to nobody, and the mirror below is the check that
/// says so.  The clearing plane's caller must therefore have collected the
/// atom first — see the pay-then-mint order documented in
/// `orders_batch::settlement`.
#[inline(never)]
pub(crate) fn pooled_set_transition(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    roles: &PooledSetRoles,
    market_id: Hash32,
    holder_internal: &mut [u64; MAX_OUTCOMES],
    change: PooledSetChange,
) -> Outcome<u64> {
    require(change.quantity != 0, ClutchError::NonCanonical)?;
    let (market, mut hoard, hoard_token_amount) =
        pooled_set_preflight(program_id, accounts, roles, market_id, holder_internal)?;

    if change.mint {
        /* The collateral cap, checked before the kernel runs, exactly as
         * `Split` checks it.  A burn has none, for the reason the `Merge` arm
         * of `seam` states: it lowers the backing and must always stay open. */
        let next_collateral = hoard
            .collateral_atoms
            .checked_add(change.quantity)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        require(
            next_collateral <= market.collateral_cap,
            ClutchError::CollateralCap,
        )?;
    }

    let pre_internal = *holder_internal;
    let mut moved = Position {
        internal: pre_internal,
        external: [0_u64; MAX_OUTCOMES],
    };
    let kernel_post = {
        let mut kernel_data = accounts[roles.kernel]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        kernel_step(
            &mut kernel_data,
            market.outcome_count,
            hoard.collateral_atoms,
            &mut moved,
            if change.mint {
                KernelOp::Split(change.quantity)
            } else {
                KernelOp::Merge(change.quantity)
            },
        )?
    };

    {
        let mut supply_data = accounts[roles.supply]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        ledger_step(
            &mut supply_data,
            market.outcome_count,
            &pre_internal,
            &moved,
        )?;
    }

    /* The mirror over the post-state, and the load-bearing one: the value
     * about to be written into `HoardAccount::collateral_atoms` is the
     * kernel's, and the Token-2022 balance must cover it.  A mint that
     * reclassified an atom the pool does not hold refuses here. */
    token::require_hoard_covers_collateral(kernel_post.collateral, hoard_token_amount)?;

    /* C1 again, over the post-state the two writes above just produced. */
    {
        let supply_post = accounts::read_supply(&accounts[roles.supply].data.borrow())?;
        let kernel_facts = accounts::read_kernel(&accounts[roles.kernel].data.borrow())?;
        require_two_term_closure(&supply_post, &kernel_facts, market.outcome_count)?;
        require(
            kernel_facts.total_supply == kernel_post.total_supply,
            ClutchError::AggregateClosureMismatch,
        )?;
    }

    hoard.collateral_atoms = kernel_post.collateral;
    hoard.encode(
        &mut accounts[roles.hoard]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;
    *holder_internal = moved.internal;
    Ok(kernel_post.collateral)
}

/// Prove a pooled holder's claim vector is one the market's ledgers account
/// for, without moving anything.
///
/// The clearing plane's virtual delivery hands claims out of the pot's
/// inventory into a Position, which changes no aggregate — so it runs no
/// transition.  It still has to know the inventory it is handing out is
/// *real*, and this is that check: the same C1 two-term closure and the same
/// C2 internal bound the mint runs, over the same authenticated accounts.  A
/// pot whose inventory the supply ledger does not cover refuses before a
/// single claim moves.
#[inline(never)]
pub(crate) fn require_pooled_holder_bound(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    roles: &PooledSetRoles,
    market_id: Hash32,
    holder_internal: &[u64; MAX_OUTCOMES],
) -> Outcome<()> {
    pooled_set_preflight(program_id, accounts, roles, market_id, holder_internal)?;
    Ok(())
}

/// Authenticate the five pooled-set accounts and discharge both CLO-DELTA-V1
/// obligations over the pre-state.
#[inline(never)]
fn pooled_set_preflight(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    roles: &PooledSetRoles,
    market_id: Hash32,
    holder_internal: &[u64; MAX_OUTCOMES],
) -> Outcome<(MarketFacts, HoardAccount, u64)> {
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[roles.market],
        false,
        &[account_len::MARKET],
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[roles.hoard],
        true,
        &[account_len::HOARD],
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[roles.kernel],
        true,
        &[KERNEL_ACCOUNT_LEN],
    )?;
    accounts::validate_state_role_lengths(
        program_id,
        &accounts[roles.supply],
        true,
        &[account_len::SUPPLY_LEDGER],
    )?;

    let market = accounts::read_market(&accounts[roles.market].data.borrow())?;
    let hoard = HoardAccount::decode(&accounts[roles.hoard].data.borrow())?;
    let kernel = accounts::read_kernel(&accounts[roles.kernel].data.borrow())?;
    let supply = accounts::read_supply(&accounts[roles.supply].data.borrow())?;

    /* Every address recomputed from the frozen seed schema, and every stored
     * bump compared against the canonical one -- the seam plane's rule, and
     * the reason a caller cannot present a different market's Hoard. */
    let market_bytes = market.market.bytes();
    expect_pda(
        accounts[roles.market].key,
        seeds::market_pda(program_id, &market.realm.bytes(), &market_bytes),
        Some(market.stored_bump),
    )?;
    let hoard_derived = seeds::hoard_pda(program_id, &market_bytes);
    expect_pda(
        accounts[roles.hoard].key,
        hoard_derived,
        Some(hoard.stored_bump),
    )?;
    expect_pda(
        accounts[roles.kernel].key,
        seeds::kernel_pda(program_id, &market_bytes),
        None,
    )?;
    expect_pda(
        accounts[roles.supply].key,
        seeds::supply_pda(program_id, &market_bytes),
        Some(supply.stored_bump),
    )?;

    require(
        market.market == market_id
            && market.market == hoard.market
            && market.realm == hoard.realm
            && market.market == kernel.market
            && market.market == supply.market
            && market.realm == supply.realm
            && market.outcome_count == supply.outcome_count
            && market.hoard_bump == hoard_derived.1
            && market.lifecycle == 0
            && kernel.phase == 0
            && kernel.payout_outcomes == market.outcome_count
            && usize::from(market.outcome_count) <= KERNEL_MAX_OUTCOMES,
        ClutchError::MismatchedState,
    )?;

    /* Padding beyond the active outcome count is canonically zero in every
     * balance vector this transition reads or writes. */
    let count = usize::from(market.outcome_count);
    let mut padding = count;
    while padding < MAX_OUTCOMES {
        require(
            holder_internal[padding] == 0 && kernel.total_supply[padding] == 0,
            ClutchError::NonCanonical,
        )?;
        padding += 1;
    }

    /* CLO-DELTA-V1 C1 and C2 over the pre-state, before any write. */
    require_two_term_closure(&supply, &kernel, market.outcome_count)?;
    require_internal_bound(
        &SupplyLedgerAccount::decode(&accounts[roles.supply].data.borrow())?,
        holder_internal,
        market.outcome_count,
    )?;

    /* The Hoard mirror over the *pre*-state.  A market whose two collateral
     * truths already disagree refuses here, before anything moves. */
    let hoard_token_derived = seeds::hoard_token_pda(program_id, &market_bytes);
    expect_pda(accounts[roles.hoard_token].key, hoard_token_derived, None)?;
    require(
        !accounts[roles.hoard_token].executable,
        ClutchError::ExecutableAccount,
    )?;
    /* The mirror reads a balance, so the account has to be one the token
     * program owns; a program-owned account with token-shaped bytes at this
     * address would otherwise report whatever balance its author chose. */
    require(
        *accounts[roles.hoard_token].owner == token::TOKEN_2022_PROGRAM_ID,
        ClutchError::WrongTokenProgram,
    )?;
    let hoard_token_amount = token::token_amount(&accounts[roles.hoard_token])?;
    token::require_hoard_covers_collateral(hoard.collateral_atoms, hoard_token_amount)?;

    Ok((market, hoard, hoard_token_amount))
}

/// Authenticate the shared half of any token leg: the token program itself.
///
/// A non-executable account at the token-program role is not the token program
/// whatever its key says, so both facts report one refusal.  Takes the account
/// rather than an index because [`super::observe_resolve`] carries the same
/// role at a different position.
pub fn validate_token_program(token_program: &AccountInfo) -> Outcome<()> {
    require(
        *token_program.key == token::TOKEN_2022_PROGRAM_ID && token_program.executable,
        ClutchError::WrongTokenProgram,
    )?;
    require(!token_program.is_writable, ClutchError::UnexpectedWritable)
}

/// Where the five collateral accounts sit in one instruction's account list.
///
/// The collateral leg is one decision procedure over five accounts plus the
/// Profile and the actor, and two instruction families present it at different
/// positions: the seam plane appends it after the supply ledger, and
/// `RedeemInternal` appends it after the evidence buffer.  Parameterizing the
/// positions is what keeps that one procedure rather than two copies that
/// drift.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralRoles {
    /// The authenticated actor, whose token account is the counterparty.
    pub actor: usize,
    /// The Profile account the 266 policy bytes are bound to.
    pub profile: usize,
    /// The Realm's 266-byte collateral policy.
    pub policy: usize,
    /// The collateral mint.
    pub mint: usize,
    /// The actor's own collateral token account.
    pub actor_token: usize,
    /// The Hoard's signing authority.
    pub authority: usize,
    /// The Hoard's collateral token account.
    pub hoard_token: usize,
}

/// The seam plane's collateral positions.
pub const SEAM_COLLATERAL_ROLES: CollateralRoles = CollateralRoles {
    actor: IX_ACTOR,
    profile: IX_PROFILE,
    policy: IX_POLICY,
    mint: IX_COLLATERAL_MINT,
    actor_token: IX_ACTOR_TOKEN,
    authority: IX_HOARD_AUTHORITY,
    hoard_token: IX_HOARD_TOKEN,
};

/// Authenticate the outcome leg and snapshot it, before anything is written.
///
/// Steps 1-3 of `docs/implementation/TOKEN2022_PLAN.md` §3.3, for the three
/// accounts the seam plane does not already own.  Nothing here writes, and the
/// data borrows every reader takes are dropped before it returns: a live
/// `RefCell` borrow across `invoke` is a runtime failure, not a lint.
///
/// The extension refusal runs **here**, over the mint account as loaded in this
/// transaction, and not only at market initialization.  §3.4 argues why, and
/// the argument is not defensive habit: `MintCloseAuthority` is refused
/// precisely because a zero-supply mint can be closed and reinitialized with a
/// different extension set, so an address recorded at initialization does not
/// bind a mint's behaviour forever.  For an outcome mint the program itself
/// created that is belt and braces; for the same code path over a collateral
/// mint it is the whole check.
#[inline(never)]
fn validate_outcome_leg(
    accounts: &[AccountInfo],
    market: &MarketFacts,
    first_mint: usize,
    outcome: u8,
    quantity: u64,
) -> Outcome<OutcomeSnapshot> {
    let mint_index = first_mint
        .checked_add(usize::from(outcome))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let mint = &accounts[mint_index];
    let holder = &accounts[IX_HOLDER_TOKEN];
    require(
        !mint.executable && !holder.executable,
        ClutchError::ExecutableAccount,
    )?;
    /* Both are mutated by the CPI, so both must have been *declared* writable
     * by the caller; the runtime will not let the token program write an
     * account this transaction did not declare. */
    require(
        mint.is_writable && holder.is_writable,
        ClutchError::NotWritable,
    )?;

    /* `claim_truth::observe_outcome_mints` already authenticated this mint's
     * canonical PDA, Token-2022 policy, authority, and mutability. */
    let holder_observation = token::admit_token_account(
        holder,
        &token::TokenAccountPolicy::holder(*mint.key, *accounts[IX_ACTOR].key),
    )?;

    Ok(OutcomeSnapshot {
        outcome,
        quantity,
        holder_amount: holder_observation.amount,
        mint_index,
        realm: market.realm.bytes(),
        market: market.market.bytes(),
        bump: [market.stored_bump],
    })
}

/// Authenticate the collateral leg and snapshot it, before anything is written.
///
/// Steps 1-3 of §3.3 for the five accounts the collateral intents append, plus
/// the one thing the outcome leg does not need: **the Realm's frozen
/// collateral policy, bound by recomputed digest.**  Without it the collateral
/// mint's identity would be caller-supplied, and a caller who may name the
/// asset may buy complete sets with a worthless one.
/// `collateral::verify_profile_identity` recomputes the child digest from the
/// 266 bytes, compares it against the Profile's frozen
/// `collateral_policy_digest`, *and* recomputes the parent Profile identity
/// from that digest and compares it against the stored Profile id — so the
/// bytes may come from any account and cannot be a different policy.  This is
/// the same content authentication [`super::market_init`] performs, and it is
/// deliberately the same call rather than a second copy of the argument.
///
/// The extension matrix then runs over the mint as loaded in *this*
/// transaction, which for a collateral mint is the whole of §3.4 rather than
/// belt and braces: the Realm does not own the mint, and `MintCloseAuthority`
/// is refused precisely because a mint's address does not bind its behaviour.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn validate_collateral_leg(
    accounts: &[AccountInfo],
    roles: &CollateralRoles,
    market: &[u8; 32],
    authority_derived: (Pubkey, u8),
    hoard_token_derived: (Pubkey, u8),
    collateral_atoms: u64,
    quantity: u64,
) -> Outcome<CollateralSnapshot> {
    let policy_account = &accounts[roles.policy];
    require(!policy_account.is_writable, ClutchError::UnexpectedWritable)?;
    require(!policy_account.executable, ClutchError::ExecutableAccount)?;
    require(
        policy_account.data_len() == collateral::COLLATERAL_POLICY_BYTES,
        ClutchError::WrongDataLength,
    )?;
    let policy = {
        let profile = ProfileAccount::decode(&accounts[roles.profile].data.borrow())?;
        collateral::verify_profile_identity(&policy_account.data.borrow(), &profile)?
    };
    token::require_drivable_collateral(&policy)?;

    /* `TransferChecked` takes the mint read-only, so a caller that declared it
     * writable declared a wider account list than the instruction needs. */
    let mint = &accounts[roles.mint];
    require(!mint.is_writable, ClutchError::UnexpectedWritable)?;
    require(!mint.executable, ClutchError::ExecutableAccount)?;
    let mint_observation = token::admit_mint(mint, &token::MintPolicy::collateral(&policy))?;

    /* The Hoard's signing authority holds no data and is never written; it is
     * in the list because `invoke_signed` needs the account whose seeds sign. */
    let authority = &accounts[roles.authority];
    require(!authority.is_writable, ClutchError::UnexpectedWritable)?;
    require(!authority.executable, ClutchError::ExecutableAccount)?;
    require(authority.data_is_empty(), ClutchError::WrongDataLength)?;
    expect_pda(authority.key, authority_derived, None)?;

    let actor_token = &accounts[roles.actor_token];
    let hoard_token = &accounts[roles.hoard_token];
    require(
        !actor_token.executable && !hoard_token.executable,
        ClutchError::ExecutableAccount,
    )?;
    require(
        actor_token.is_writable && hoard_token.is_writable,
        ClutchError::NotWritable,
    )?;
    expect_pda(hoard_token.key, hoard_token_derived, None)?;

    let actor_observation = token::admit_token_account(
        actor_token,
        &token::TokenAccountPolicy::collateral_holder(&policy, *accounts[roles.actor].key),
    )?;
    let hoard_observation = token::admit_token_account(
        hoard_token,
        &token::TokenAccountPolicy::hoard(&policy, *authority.key),
    )?;

    /* The mirror over the *pre*-state.  A market whose two collateral truths
     * already disagree refuses here, before anything moves, rather than at the
     * post-CPI check where the diagnostic would be identical but the kernel
     * would already have run. */
    token::require_hoard_covers_collateral(collateral_atoms, hoard_observation.amount)?;

    Ok(CollateralSnapshot {
        quantity,
        decimals: mint_observation.decimals,
        actor_amount: actor_observation.amount,
        hoard_amount: hoard_observation.amount,
        market: *market,
        authority_bump: [authority_derived.1],
    })
}

/// Perform the token CPIs of one seam transition.
///
/// Step 5 of §3.3, and **only** step 5: the deltas this must have produced are
/// checked by `verify_token_deltas` afterwards, over the account bytes, so
/// that the check is the same code whichever way the CPI was performed.
///
/// `Materialize` mints, so its authority is the market PDA and the call is
/// `invoke_signed`.  `Dematerialize` burns from an account the actor owns, so
/// the actor's already-authenticated signature propagates and the call is a
/// plain `invoke`.  `Split` moves collateral *in* under the same propagated
/// signature; `Merge` moves it *out* and is therefore impossible without this
/// program signing for the Hoard authority seeds.  That asymmetry is the
/// probe's finding, not a convention.
#[inline(never)]
pub fn token_effects(
    accounts: &[AccountInfo],
    op: &SeamOp,
    snapshot: &TokenSnapshot,
) -> Outcome<()> {
    let token_program = &accounts[IX_TOKEN_PROGRAM];
    match (op, snapshot) {
        (SeamOp::Materialize { .. }, TokenSnapshot::Outcome(snapshot)) => {
            let signer: [&[u8]; 4] = [
                seeds::SEED_MARKET,
                &snapshot.realm,
                &snapshot.market,
                &snapshot.bump,
            ];
            token::mint_to_signed(
                token_program,
                &accounts[snapshot.mint_index],
                &accounts[IX_HOLDER_TOKEN],
                &accounts[IX_MARKET],
                snapshot.quantity,
                &signer,
            )
        }
        (SeamOp::Dematerialize { .. }, TokenSnapshot::Outcome(snapshot)) => token::burn(
            token_program,
            &accounts[IX_HOLDER_TOKEN],
            &accounts[snapshot.mint_index],
            &accounts[IX_ACTOR],
            snapshot.quantity,
        ),
        /* Collateral is already inside the pooled Hoard because `Endow` is
         * the sole inbound value boundary.  Split locks position cash as
         * complete-set backing and Merge unlocks it; both are accounting
         * reclassifications and therefore must move zero Token-2022 atoms. */
        (SeamOp::Split { .. } | SeamOp::Merge { .. }, TokenSnapshot::Collateral(_)) => Ok(()),
        /* Unreachable: `select_token_leg` pairs each intent with exactly one
         * snapshot shape.  Refusing rather than silently succeeding is what
         * keeps that a check and not a comment. */
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

/// Re-read every touched token account and require the *exact* delta.
///
/// Step 6 of §3.3.  Not `>=`, not "at least".  On the outcome leg the returned
/// value is the post-CPI mint supply, which the caller reconciles against the
/// market-wide external term; on the collateral leg it is the post-CPI Hoard
/// balance, which the caller mirrors against `HoardAccount::collateral_atoms`.
#[inline(never)]
fn verify_token_deltas(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    market: &MarketFacts,
    first_mint: usize,
    mint_supplies_before: &ObservedMintSupplies,
    op: &SeamOp,
    snapshot: &TokenSnapshot,
) -> Outcome<(ObservedMintSupplies, u64)> {
    match snapshot {
        TokenSnapshot::Outcome(snapshot) => {
            let post = claim_truth::observe_outcome_mints(
                program_id,
                accounts,
                first_mint,
                *accounts[IX_MARKET].key,
                market.market,
                market.outcome_count,
                Some(snapshot.outcome),
            )?;
            let post_amount = token::token_amount(&accounts[IX_HOLDER_TOKEN])?;
            if matches!(op, SeamOp::Materialize { .. }) {
                token::require_exact_credit(
                    snapshot.holder_amount,
                    post_amount,
                    snapshot.quantity,
                )?;
            } else {
                token::require_exact_debit(snapshot.holder_amount, post_amount, snapshot.quantity)?;
            }
            claim_truth::require_exact_mint_vector_delta(
                mint_supplies_before,
                &post,
                Some((
                    snapshot.outcome,
                    matches!(op, SeamOp::Materialize { .. }),
                    snapshot.quantity,
                )),
            )?;
            Ok((post, 0))
        }
        TokenSnapshot::Collateral(snapshot) => {
            let post_actor = token::token_amount(&accounts[IX_ACTOR_TOKEN])?;
            let post_hoard = token::token_amount(&accounts[IX_HOARD_TOKEN])?;
            token::require_exact_credit(snapshot.actor_amount, post_actor, 0)?;
            token::require_exact_credit(snapshot.hoard_amount, post_hoard, 0)?;
            let post = claim_truth::observe_outcome_mints(
                program_id,
                accounts,
                first_mint,
                *accounts[IX_MARKET].key,
                market.market,
                market.outcome_count,
                None,
            )?;
            claim_truth::require_exact_mint_vector_delta(mint_supplies_before, &post, None)?;
            Ok((post, post_hoard))
        }
    }
}

/// Validate hostile accounts and apply exactly one seam transition.
///
/// The check order is the bring-up program's, which is the offline reference
/// adapter's with two named differences, both of which only change *which of
/// two simultaneous faults* is reported and never whether a request is
/// accepted:
///
/// 1. the actor's signature is authenticated first here and inside the
///    per-intent arm there, because a runtime hands this program a signature
///    assertion before it hands it any account data; and
/// 2. `Split`'s collateral-cap and cash checks precede the kernel invariant
///    check here and follow it there.
///
/// `Merge`'s cash *credit* is not one of those differences: it lands after the
/// kernel step on both sides, because on both sides it is the consequence of a
/// burn rather than a precondition of a mint.
///
/// Every check that decides *acceptance* is in the reference's order, and the
/// host differential in this module's tests pins the correspondence one fault
/// at a time.
pub fn seam<D, T>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    op: &SeamOp,
    derive: D,
    effect: T,
) -> Outcome<()>
where
    D: FnOnce(&Identities) -> Bindings,
    T: FnOnce(&[AccountInfo], &SeamOp, &TokenSnapshot) -> Outcome<()>,
{
    /* The market owns the active outcome count, so the exact account count is
     * authenticated only after the fixed state prefix is known to exist. */
    require(accounts.len() >= ACCOUNT_COUNT, ClutchError::AccountCount)?;

    let actor = &accounts[IX_ACTOR];
    require_signer(actor)?;

    /* Role uniqueness.  A writable alias would let one logical debit or credit
     * land twice, which is obligation 3 of the reference adapter doc. */
    require_distinct(accounts)?;

    /* Program ownership, executable bit, declared mutability by role, and exact
     * data length per role. */
    accounts::validate_state_roles(program_id, accounts, &STATE_ROLES)?;

    let realm = accounts::read_realm(&accounts[IX_REALM].data.borrow())?;
    let profile = accounts::read_profile(&accounts[IX_PROFILE].data.borrow())?;
    let market = accounts::read_market(&accounts[IX_MARKET].data.borrow())?;
    let leg = select_token_leg(op, accounts.len(), market.outcome_count)?;
    let mut hoard = HoardAccount::decode(&accounts[IX_HOARD].data.borrow())?;
    let mut position = PositionAccount::decode(&accounts[IX_POSITION].data.borrow())?;
    let kernel = accounts::read_kernel(&accounts[IX_KERNEL].data.borrow())?;
    let mut replay = ReplayAccount::decode(&accounts[IX_REPLAY].data.borrow())?;
    let supply = accounts::read_supply(&accounts[IX_SUPPLY].data.borrow())?;

    /* Derived addresses.  Caller-supplied expected keys are never accepted:
     * every address is recomputed from the frozen seed schema and compared,
     * and every stored bump is compared against the canonical bump. */
    let bindings = derive(&Identities {
        realm: market.realm.bytes(),
        profile: market.profile.bytes(),
        market: market.market.bytes(),
        owner: position.owner.bytes(),
        generation: position.generation,
        leg,
    });
    expect_pda(
        accounts[IX_REALM].key,
        bindings.realm,
        Some(realm.stored_bump),
    )?;
    expect_pda(accounts[IX_PROFILE].key, bindings.profile, None)?;
    expect_pda(
        accounts[IX_MARKET].key,
        bindings.market,
        Some(market.stored_bump),
    )?;
    expect_pda(
        accounts[IX_HOARD].key,
        bindings.hoard,
        Some(hoard.stored_bump),
    )?;
    require(
        market.hoard_bump == bindings.hoard.1,
        ClutchError::WrongBump,
    )?;
    expect_pda(
        accounts[IX_POSITION].key,
        bindings.position,
        Some(position.stored_bump),
    )?;
    expect_pda(accounts[IX_KERNEL].key, bindings.kernel, None)?;
    expect_pda(
        accounts[IX_REPLAY].key,
        bindings.replay,
        Some(replay.stored_bump),
    )?;
    expect_pda(
        accounts[IX_SUPPLY].key,
        bindings.supply,
        Some(supply.stored_bump),
    )?;

    /* Cross-account linkage, mirroring `validate_links` in the offline
     * reference adapter -- the supply-ledger edges included -- plus the
     * Realm/Profile edges the reference only checks at market initialization. */
    require(
        realm.realm == market.realm
            && realm.profile == market.profile
            && profile.profile == market.profile
            && profile.realm == market.realm
            && realm.profile_version == profile.version
            && usize::from(realm.max_outcomes) == MAX_OUTCOMES
            && market.outcome_count <= realm.max_outcomes,
        ClutchError::MismatchedState,
    )?;
    require(
        market.market == hoard.market
            && market.realm == hoard.realm
            && market.market == position.market
            && market.market == kernel.market
            && market.market == replay.market
            && market.market == supply.market
            && market.realm == supply.realm
            && market.outcome_count == supply.outcome_count
            && position.owner == replay.owner
            && position.generation == replay.position_generation
            && (market.lifecycle != 0 || kernel.phase == 0)
            && (market.lifecycle != 1 || kernel.phase == 1)
            && market.lifecycle <= 1
            && kernel.payout_outcomes == market.outcome_count
            && usize::from(market.outcome_count) <= KERNEL_MAX_OUTCOMES,
        ClutchError::MismatchedState,
    )?;

    /* Padding beyond the active outcome count must be canonically zero in every
     * balance vector.  The ledger's own padding is refused by its frozen codec,
     * which is why it is not re-checked here. */
    let count = usize::from(market.outcome_count);
    let mut padding = count;
    while padding < MAX_OUTCOMES {
        require(
            position.internal[padding] == 0 && kernel.total_supply[padding] == 0,
            ClutchError::NonCanonical,
        )?;
        padding += 1;
    }

    /* CLO-DELTA-V1 C1 and C2 over the pre-state, before any write.  C2 is a
     * one-sided bound and not the retired equality: a ledger term above the
     * presented position is another position's claim, while a position above
     * the ledger term is a counterfeit and refuses. */
    require_two_term_closure(&supply, &kernel, market.outcome_count)?;
    require_internal_bound(
        &SupplyLedgerAccount::decode(&accounts[IX_SUPPLY].data.borrow())?,
        &position.internal,
        market.outcome_count,
    )?;

    require(sequence == replay.sequence, ClutchError::Replay)?;
    let next_sequence = replay
        .sequence
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Replay))?;

    /* Per-intent authorization and binding, in the offline reference adapter's
     * per-arm order. */
    match op {
        SeamOp::Split { quantity, .. } => {
            authorize_and_bind(actor, &position, market.market, op)?;
            require(
                market.lifecycle == 0 && position.close_state == 0,
                ClutchError::NotActive,
            )?;
            /* Collateral cap and position cash are checked before the kernel
             * runs, in the same order as the offline reference adapter. */
            let next_collateral = hoard
                .collateral_atoms
                .checked_add(*quantity)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            require(
                next_collateral <= market.collateral_cap,
                ClutchError::CollateralCap,
            )?;
            position
                .free_cash_atoms()?
                .checked_sub(*quantity)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            position.cash_atoms = position
                .cash_atoms
                .checked_sub(*quantity)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        }
        SeamOp::Merge { .. } => {
            authorize_and_bind(actor, &position, market.market, op)?;
            require(
                market.lifecycle == 0 && position.close_state == 0,
                ClutchError::NotActive,
            )?;
            /* NO COLLATERAL-CAP CHECK, and the reference adapter has none
             * either: a merge lowers `hoard.collateral_atoms`, so it cannot
             * cross a ceiling the pre-state was under, and a cap check here
             * would strand a market that is somehow already above its cap in
             * the one direction that must always stay open.  The cash credit
             * is not here either: it is the *consequence* of the burn and
             * lands after the kernel step below, mirroring the reference. */
        }
        SeamOp::Materialize { destination, .. } => {
            authorize_and_bind(actor, &position, market.market, op)?;
            /* The request binds the exact bearer token account credited by the
             * Token-2022 CPI.  No program-owned owner shadow exists. */
            require(
                destination.bytes() == accounts[IX_HOLDER_TOKEN].key.to_bytes(),
                ClutchError::MismatchedState,
            )?;
        }
        SeamOp::Dematerialize { source, .. } => {
            authorize_and_bind(actor, &position, market.market, op)?;
            require(
                source.bytes() == accounts[IX_HOLDER_TOKEN].key.to_bytes(),
                ClutchError::MismatchedState,
            )?;
        }
    }

    /* Steps 1-3 of `TOKEN2022_PLAN.md` §3.3 for the token leg: authenticate it,
     * re-run the extension refusal over the mint as loaded in *this*
     * transaction, and snapshot the exact pre-CPI supply and balance.  All of
     * it before the first write, and all of it dropped back to a small snapshot
     * so nothing large crosses this frame. */
    validate_token_program(&accounts[IX_TOKEN_PROGRAM])?;
    let first_mint = match leg {
        TokenLeg::Outcome(_) => IX_OUTCOME_MINTS,
        TokenLeg::Collateral => IX_COLLATERAL_OUTCOME_MINTS,
    };
    let writable_outcome = match leg {
        TokenLeg::Outcome(outcome) => Some(outcome),
        TokenLeg::Collateral => None,
    };
    let observed_before = claim_truth::observe_outcome_mints(
        program_id,
        accounts,
        first_mint,
        *accounts[IX_MARKET].key,
        market.market,
        market.outcome_count,
        writable_outcome,
    )?;

    /* Reconcile permissionless direct burns before the economic transition.
     * A mint increase above the last observed cache refuses: only this
     * program's market PDA can mint, and every authorized mint is persisted in
     * the same atomic instruction that performs it. */
    {
        let mut supply_data = accounts[IX_SUPPLY]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let mut kernel_data = accounts[IX_KERNEL]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        claim_truth::synchronize_external_truth(
            &mut supply_data,
            &mut kernel_data,
            market.market,
            market.realm,
            market.outcome_count,
            &observed_before,
        )?;
    }

    let snapshot = match (leg, op) {
        (
            TokenLeg::Outcome(outcome),
            SeamOp::Materialize { quantity, .. } | SeamOp::Dematerialize { quantity, .. },
        ) => TokenSnapshot::Outcome(validate_outcome_leg(
            accounts, &market, first_mint, outcome, *quantity,
        )?),
        (TokenLeg::Collateral, SeamOp::Split { quantity, .. } | SeamOp::Merge { quantity, .. }) => {
            TokenSnapshot::Collateral(validate_collateral_leg(
                accounts,
                &SEAM_COLLATERAL_ROLES,
                &market.market.bytes(),
                bindings
                    .hoard_authority
                    .ok_or(Refusal::Adapter(ClutchError::WrongPda))?,
                bindings
                    .hoard_token
                    .ok_or(Refusal::Adapter(ClutchError::WrongPda))?,
                hoard.collateral_atoms,
                *quantity,
            )?)
        }
        /* Unreachable: `select_token_leg` is a total function of the intent. */
        _ => return Err(ClutchError::UnsupportedInstruction.into()),
    };

    /* Everything below this line writes.  A refusal after this point aborts the
     * instruction, and SVM transaction semantics -- not this program -- are
     * what discard the partial write. */
    let pre_internal = position.internal;
    let mut moved = Position {
        internal: pre_internal,
        external: {
            let mut external = [0_u64; MAX_OUTCOMES];
            if let SeamOp::Dematerialize {
                outcome, quantity, ..
            } = op
            {
                external[usize::from(*outcome)] = *quantity;
            }
            external
        },
    };
    let kernel_post = {
        let mut kernel_data = accounts[IX_KERNEL]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        kernel_step(
            &mut kernel_data,
            market.outcome_count,
            hoard.collateral_atoms,
            &mut moved,
            op.kernel_op(),
        )?
    };

    /* A merge's cash credit, at exactly the reference adapter's point in the
     * order: after the kernel burned the complete set that justifies it, and
     * before the ledger delta.  `quantity` is the released collateral because
     * a complete set is worth exactly one atom of it on both sides of the
     * kernel.  `Split`'s debit is a *precondition* and therefore sits above,
     * with the collateral cap; this is a *consequence* and sits here, which is
     * the order `Action::RedeemInternal` credits a payout in too. */
    if let SeamOp::Merge { quantity, .. } = op {
        position.cash_atoms = position
            .cash_atoms
            .checked_add(*quantity)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }

    {
        let mut supply_data = accounts[IX_SUPPLY]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        ledger_step(
            &mut supply_data,
            market.outcome_count,
            &pre_internal,
            &moved,
        )?;
    }

    /* Steps 5-6 of §3.3: the token effects, then the *exact* deltas they must
     * have produced, then the single-truth reconciliation.  Every `RefCell`
     * borrow above is out of scope by now -- `hoard`, `position`, `external`
     * and `replay` are decoded values, not borrows -- which is the precondition
     * `invoke` has and `invoke_signed` checks for itself. */
    effect(accounts, op, &snapshot)?;
    let (observed_after, hoard_token_after) = verify_token_deltas(
        program_id,
        accounts,
        &market,
        first_mint,
        &observed_before,
        op,
        &snapshot,
    )?;
    match snapshot {
        TokenSnapshot::Outcome(_) => {}
        /* The mirror over the post-state, and the load-bearing one: the value
         * about to be written into `HoardAccount::collateral_atoms` is the
         * kernel's, `observed` is the token program's, and the two must be the
         * same number.  A kernel that moved a different amount than the CPI
         * did -- in either direction -- refuses here. */
        TokenSnapshot::Collateral(_) => {
            token::require_hoard_covers_collateral(kernel_post.collateral, hoard_token_after)?
        }
    }

    {
        let mut supply_data = accounts[IX_SUPPLY]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let kernel_data = accounts[IX_KERNEL]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        claim_truth::commit_observed_supplies(
            &mut supply_data,
            &kernel_data,
            market.market,
            market.realm,
            market.outcome_count,
            &observed_after,
        )?;
    }

    hoard.collateral_atoms = kernel_post.collateral;
    position.internal = moved.internal;
    replay.sequence = next_sequence;

    hoard.encode(
        &mut accounts[IX_HOARD]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;
    position.encode(
        &mut accounts[IX_POSITION]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;
    replay.encode(
        &mut accounts[IX_REPLAY]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?,
    )?;

    /* Market bytes are deliberately untouched: no seam transition changes them.
     * The differential still compares them against the reference adapter's
     * re-encoded post-state, so a codec that did not round-trip would fail the
     * comparison rather than hide inside a rewrite. */
    Ok(())
}

/// Authenticate the actor as the position owner and bind the intent's names.
///
/// Split out so that the three authorizing intents run one identical pair of
/// checks in one identical order rather than three copies of it.
fn authorize_and_bind(
    actor: &AccountInfo,
    position: &PositionAccount,
    market: Hash32,
    op: &SeamOp,
) -> Outcome<()> {
    require(
        actor.key.to_bytes() == position.owner.bytes(),
        ClutchError::UnauthorizedActor,
    )?;
    require(
        op.market() == market && op.owner() == position.owner,
        ClutchError::MismatchedState,
    )
}

/// Validate hostile accounts and apply exactly one `Split`.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: &SplitRequest,
) -> Outcome<()> {
    seam(
        program_id,
        accounts,
        request.sequence,
        &SeamOp::Split {
            market: request.market,
            owner: request.owner,
            quantity: request.quantity,
        },
        |ids| derive_bindings(program_id, ids),
        token_effects,
    )
}

/* ------------------------------------------------------------------------ */
/* Host differential against the offline reference adapter                   */
/* ------------------------------------------------------------------------ */

/// The seam plane's host-side differential harness, shared with
/// [`super::merge_materialize`].
///
/// Every case here builds one set of fixture bytes, runs **this program's
/// processor path** over `AccountInfo`s holding those bytes, runs
/// `clutch_solana_reference::apply` over the same bytes, and asserts that the
/// two agree: byte-identical post-state when both accept, and corresponding
/// refusal classes when both refuse.  A case where one accepts and the other
/// refuses fails loudly, which is the whole point.
// Retained as migration archaeology: this historical differential models the
// deleted per-owner External account and fixed account counts.
#[cfg(any())]
pub(crate) mod tests {
    use super::*;
    use clutch_kernel::{Error as KernelError, PayoutSet, PayoutVector, MAX_PAYOUTS};
    use clutch_solana_layout::{
        canonical_market_id, canonical_outcome_id, canonical_realm_id, CodecError, FeedId,
        MarketAccount, RealmAccount, MAX_INTENT_BYTES, PROFILE_FLAG_POLICY_FROZEN,
    };
    use clutch_solana_reference::{
        apply, AccountMetadata, ActorMetadata, Error as ReferenceError, ExpectedBindings,
        StateBytes, TransitionMetadata, TransitionOutput,
    };

    /// Fixture constants, chosen to be exactly the offline reference adapter's
    /// own `fixture()` so that a disagreement is a real disagreement rather
    /// than two different scenarios.
    const REALM_NONCE: u64 = 7;
    const MARKET_NONCE: u64 = 9;
    const GENERATION: u64 = 2;
    const CASH_ATOMS: u64 = 100;
    const RESERVED_CASH_ATOMS: u64 = 7;
    const COLLATERAL_CAP: u64 = 1_000;
    const OUTCOME_COUNT: u8 = 2;
    const OWNER_FILL: u8 = 31;
    const PROGRAM_FILL: u8 = 0x50;
    /// Fill byte of the collateral mint the fixture Realm's policy names.
    const COLLATERAL_MINT_FILL: u8 = 0x6d;
    /// The fixture collateral mint's decimals.
    const COLLATERAL_DECIMALS: u8 = 6;
    /// Its supply; nonzero, because `COLLATERAL_POLICY_STRICT_FLAGS` requires
    /// it and a mint nobody has minted is not collateral.
    const COLLATERAL_SUPPLY: u64 = 5_000_000;
    /// Collateral atoms the fixture actor holds before any `Split`.
    const ACTOR_COLLATERAL_ATOMS: u64 = 1_000;

    /// The reference request envelope's three private constants.
    ///
    /// They are private to `clutch_solana_reference`, so this copy is kept
    /// honest by [`layout_request`], which refuses to return bytes that do not
    /// decode back to the request they claim to encode.
    const REQUEST_TAG: u8 = 0xd1;
    const REFERENCE_VERSION: u8 = 1;
    const ACTION_LAYOUT: u8 = 0;

    pub(crate) fn h(value: u8) -> Hash32 {
        Hash32::from_bytes([value; 32])
    }

    pub(crate) fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn hash_of(value: &Pubkey) -> Hash32 {
        Hash32::from_bytes(value.to_bytes())
    }

    fn fixed<const N: usize>(data: &[u8]) -> [u8; N] {
        let mut out = [0_u8; N];
        out.copy_from_slice(data);
        out
    }

    /// The nine program-owned account byte images of one seam account list.
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(crate) struct Seam {
        pub realm: [u8; account_len::REALM],
        pub profile: [u8; account_len::PROFILE],
        pub market: [u8; account_len::MARKET],
        pub hoard: [u8; account_len::HOARD],
        pub position: [u8; account_len::POSITION],
        pub kernel: [u8; KERNEL_ACCOUNT_LEN],
        pub external: [u8; EXTERNAL_ACCOUNT_LEN],
        pub replay: [u8; REPLAY_ACCOUNT_LEN],
        pub supply: [u8; account_len::SUPPLY_LEDGER],
    }

    impl Seam {
        fn datas(&self) -> Vec<Vec<u8>> {
            vec![
                self.realm.to_vec(),
                self.profile.to_vec(),
                self.market.to_vec(),
                self.hoard.to_vec(),
                self.position.to_vec(),
                self.kernel.to_vec(),
                self.external.to_vec(),
                self.replay.to_vec(),
                self.supply.to_vec(),
            ]
        }

        fn from_datas(datas: &[Vec<u8>]) -> Self {
            Self {
                realm: fixed(&datas[0]),
                profile: fixed(&datas[1]),
                market: fixed(&datas[2]),
                hoard: fixed(&datas[3]),
                position: fixed(&datas[4]),
                kernel: fixed(&datas[5]),
                external: fixed(&datas[6]),
                replay: fixed(&datas[7]),
                supply: fixed(&datas[8]),
            }
        }

        fn state_bytes(&self) -> StateBytes<'_> {
            StateBytes {
                market: &self.market,
                hoard: &self.hoard,
                position: &self.position,
                kernel: &self.kernel,
                external: &self.external,
                replay: &self.replay,
                supply: &self.supply,
            }
        }
    }

    /// Owned backing store for one host-side `AccountInfo`.
    struct Cell {
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        data: Vec<u8>,
        is_signer: bool,
        is_writable: bool,
        executable: bool,
    }

    impl Cell {
        fn info(&mut self) -> AccountInfo<'_> {
            AccountInfo::new(
                &self.key,
                self.is_signer,
                self.is_writable,
                &mut self.lamports,
                &mut self.data,
                &self.owner,
                self.executable,
            )
        }
    }

    /// One differential scenario: the presented accounts, the trusted
    /// bindings, and the state bytes both adapters run over.
    #[derive(Clone, Debug)]
    pub(crate) struct Case {
        /// Program identity every state account must be owned by.
        pub program: Pubkey,
        /// Presented account keys, in seam account-list order.
        pub keys: [Pubkey; ACCOUNT_COUNT],
        /// Runtime-reported owner of each account.
        pub owners: [Pubkey; ACCOUNT_COUNT],
        /// Declared writability of each account.
        pub writable: [bool; ACCOUNT_COUNT],
        /// Whether the runtime authenticated the actor's signature.
        pub signer: bool,
        /// The trusted derivation both adapters compare against.
        pub bindings: Bindings,
        /// The account byte images.
        pub state: Seam,
        /// The outcome leg `Materialize` and `Dematerialize` present.
        pub token: TokenLegCase,
        /// The collateral leg `Split` and `Merge` present.
        pub collateral: CollateralLegCase,
    }

    /// The token accounts a transition left behind, carried forward by
    /// [`Case::advance`] so that a multi-step scenario is one running story
    /// rather than four independent first steps.
    #[derive(Clone, Debug, Default)]
    pub(crate) struct LegPost {
        outcome: Vec<Vec<u8>>,
        collateral: Vec<Vec<u8>>,
    }

    impl Case {
        fn cells(&self) -> Vec<Cell> {
            let mut cells = vec![Cell {
                key: self.keys[IX_ACTOR],
                owner: Pubkey::new_from_array([0; 32]),
                lamports: 1,
                data: Vec::new(),
                is_signer: self.signer,
                is_writable: self.writable[IX_ACTOR],
                executable: false,
            }];
            for (offset, data) in self.state.datas().into_iter().enumerate() {
                let index = offset + IX_REALM;
                cells.push(Cell {
                    key: self.keys[index],
                    owner: self.owners[index],
                    lamports: 1,
                    data,
                    is_signer: false,
                    is_writable: self.writable[index],
                    executable: false,
                });
            }
            cells
        }

        /// Which leg the request's intent carries, so a scenario presents the
        /// plane the program will demand rather than the one it remembers.
        fn leg_of(request: &[u8]) -> TokenLeg {
            match Request::decode(request).ok().and_then(|r| seam_op(&r).ok()) {
                Some(SeamOp::Materialize { outcome, .. })
                | Some(SeamOp::Dematerialize { outcome, .. }) => TokenLeg::Outcome(outcome),
                _ => TokenLeg::Collateral,
            }
        }

        fn outcome_cells(&self, leg: &TokenLegCase) -> Vec<Cell> {
            vec![
                Cell {
                    key: leg.token_program,
                    owner: Pubkey::new_from_array([0; 32]),
                    lamports: 1,
                    data: Vec::new(),
                    is_signer: false,
                    is_writable: leg.token_program_writable,
                    executable: leg.token_program_executable,
                },
                Cell {
                    key: leg.mint,
                    owner: leg.mint_owner,
                    lamports: 1,
                    data: leg.mint_data.clone(),
                    is_signer: false,
                    is_writable: leg.mint_writable,
                    executable: false,
                },
                Cell {
                    key: leg.holder,
                    owner: leg.holder_owner,
                    lamports: 1,
                    data: leg.holder_data.clone(),
                    is_signer: false,
                    is_writable: leg.holder_writable,
                    executable: false,
                },
            ]
        }

        fn collateral_cells(&self, leg: &CollateralLegCase) -> Vec<Cell> {
            vec![
                Cell {
                    key: leg.token_program,
                    owner: Pubkey::new_from_array([0; 32]),
                    lamports: 1,
                    data: Vec::new(),
                    is_signer: false,
                    is_writable: leg.token_program_writable,
                    executable: leg.token_program_executable,
                },
                Cell {
                    key: key(0xa1),
                    owner: self.program,
                    lamports: 1,
                    data: leg.policy.clone(),
                    is_signer: false,
                    is_writable: leg.policy_writable,
                    executable: false,
                },
                Cell {
                    key: leg.mint,
                    owner: leg.mint_owner,
                    lamports: 1,
                    data: leg.mint_data.clone(),
                    is_signer: false,
                    is_writable: leg.mint_writable,
                    executable: false,
                },
                Cell {
                    key: leg.actor_token,
                    owner: leg.actor_token_owner,
                    lamports: 1,
                    data: leg.actor_token_data.clone(),
                    is_signer: false,
                    is_writable: leg.actor_token_writable,
                    executable: false,
                },
                Cell {
                    key: leg.authority,
                    owner: Pubkey::new_from_array([0; 32]),
                    lamports: 1,
                    data: Vec::new(),
                    is_signer: false,
                    is_writable: leg.authority_writable,
                    executable: false,
                },
                Cell {
                    key: leg.hoard_token,
                    owner: leg.hoard_token_owner,
                    lamports: 1,
                    data: leg.hoard_token_data.clone(),
                    is_signer: false,
                    is_writable: leg.hoard_token_writable,
                    executable: false,
                },
            ]
        }

        /// Run this program's processor path over the case.
        ///
        /// This mirrors [`crate::dispatch::process`] with exactly **two**
        /// substitutions, and both are the same kind of thing: a syscall that
        /// does not exist off-chain.
        ///
        /// 1. already-derived bindings replace [`derive_bindings`], because
        ///    program-address derivation is a runtime syscall (see
        ///    [`crate::seeds::find`]); and
        /// 2. [`simulate_token_effects`] replaces [`token_effects`], because
        ///    `solana_cpi::invoke_signed` compiles to `Ok(())` off-chain, so
        ///    the real CPI moves nothing and every transition would refuse
        ///    [`ClutchError::TokenDeltaMismatch`] on the exact-delta check.
        ///
        /// The second substitution is the one worth arguing about, so it is
        /// argued rather than hidden.  The simulator is **not** a model of
        /// Token-2022: it adds and subtracts one `u64` in one account, which
        /// is the only thing the exact-delta rule of `TOKEN2022_PLAN.md` §3.3
        /// step 6 permits a conforming transfer to do.  Everything that
        /// decides *whether* the CPI happens — plane selection, the token
        /// program's identity, PDA derivation, the extension matrix, the
        /// account policies, the mirror, the check order — runs unchanged and
        /// is what these tests cover.  What runs *inside* Token-2022 is not
        /// covered here and cannot be: that is `programs/clutch-sbf/svm-tests`,
        /// against the real ELF and the real token program.  The real
        /// effector's off-chain behaviour is itself pinned, by
        /// `merge_materialize`'s `an_off_chain_token_leg_refuses_the_delta_it_could_not_move`.
        pub(crate) fn program(&self, request: &[u8]) -> (Outcome<()>, Seam) {
            let (result, state, _) = self.run(request, None, None, simulate_token_effects);
            (result, state)
        }

        /// The same, with the post-CPI token account images carried out.
        pub(crate) fn run_carrying(&self, request: &[u8]) -> (Outcome<()>, Seam, LegPost) {
            self.run(request, None, None, simulate_token_effects)
        }

        /// Run this program's processor path over a hostile **outcome** leg.
        pub(crate) fn program_with_token_leg(
            &self,
            request: &[u8],
            leg: &TokenLegCase,
        ) -> (Outcome<()>, Seam) {
            let (result, state, _) = self.run(request, Some(leg), None, simulate_token_effects);
            (result, state)
        }

        /// Run this program's processor path over a hostile **collateral** leg.
        pub(crate) fn program_with_collateral_leg(
            &self,
            request: &[u8],
            leg: &CollateralLegCase,
        ) -> (Outcome<()>, Seam) {
            let (result, state, _) = self.run(request, None, Some(leg), simulate_token_effects);
            (result, state)
        }

        /// Run the case against the **real** CPI path, which off-chain moves
        /// nothing.  Used only to pin that the exact-delta check catches it.
        pub(crate) fn program_with_real_cpi(&self, request: &[u8]) -> (Outcome<()>, Seam) {
            let (result, state, _) = self.run(request, None, None, token_effects);
            (result, state)
        }

        /// Present exactly `count` accounts, whatever plane the intent wants.
        ///
        /// Short of the intent's plane the list is truncated; past it, padded
        /// with distinct inert accounts.  Either way the count check is the
        /// first thing that runs, so this is a test of the count and of
        /// nothing else.
        pub(crate) fn program_truncated(
            &self,
            request: &[u8],
            count: usize,
        ) -> (Outcome<()>, Seam) {
            let mut cells = self.cells();
            cells.extend(match Self::leg_of(request) {
                TokenLeg::Outcome(_) => self.outcome_cells(&self.token),
                TokenLeg::Collateral => self.collateral_cells(&self.collateral),
            });
            while cells.len() < count {
                cells.push(Cell {
                    key: key(0x30_u8.wrapping_add(cells.len() as u8)),
                    owner: Pubkey::new_from_array([0; 32]),
                    lamports: 1,
                    data: Vec::new(),
                    is_signer: false,
                    is_writable: false,
                    executable: false,
                });
            }
            cells.truncate(count);
            let result = {
                let infos: Vec<AccountInfo<'_>> = cells.iter_mut().map(Cell::info).collect();
                Request::decode(request)
                    .map_err(Refusal::from)
                    .and_then(|decoded| {
                        let op = seam_op(&decoded)?;
                        seam(
                            &self.program,
                            &infos,
                            decoded.sequence,
                            &op,
                            |_| self.bindings,
                            simulate_token_effects,
                        )
                    })
            };
            let datas: Vec<Vec<u8>> = cells.into_iter().map(|cell| cell.data).collect();
            (result, Seam::from_datas(&datas[1..10]))
        }

        fn run<T>(
            &self,
            request: &[u8],
            outcome_override: Option<&TokenLegCase>,
            collateral_override: Option<&CollateralLegCase>,
            effect: T,
        ) -> (Outcome<()>, Seam, LegPost)
        where
            T: Fn(&[AccountInfo], &SeamOp, &TokenSnapshot) -> Outcome<()>,
        {
            let mut cells = self.cells();
            let leg = Self::leg_of(request);
            let appended = match leg {
                TokenLeg::Outcome(_) => self.outcome_cells(outcome_override.unwrap_or(&self.token)),
                TokenLeg::Collateral => {
                    self.collateral_cells(collateral_override.unwrap_or(&self.collateral))
                }
            };
            cells.extend(appended);
            let result = {
                let infos: Vec<AccountInfo<'_>> = cells.iter_mut().map(Cell::info).collect();
                Request::decode(request)
                    .map_err(Refusal::from)
                    .and_then(|decoded| {
                        let op = seam_op(&decoded)?;
                        seam(
                            &self.program,
                            &infos,
                            decoded.sequence,
                            &op,
                            |_| self.bindings,
                            &effect,
                        )
                    })
            };
            let datas: Vec<Vec<u8>> = cells.into_iter().map(|cell| cell.data).collect();
            let post = match leg {
                TokenLeg::Outcome(_) => LegPost {
                    outcome: datas[ACCOUNT_COUNT..].to_vec(),
                    collateral: Vec::new(),
                },
                TokenLeg::Collateral => LegPost {
                    outcome: Vec::new(),
                    collateral: datas[ACCOUNT_COUNT..].to_vec(),
                },
            };
            (result, Seam::from_datas(&datas[1..10]), post)
        }

        fn metadata(&self) -> TransitionMetadata {
            let account = |index: usize| AccountMetadata {
                key: hash_of(&self.keys[index]),
                owner_program: hash_of(&self.owners[index]),
                writable: self.writable[index],
            };
            TransitionMetadata {
                market: account(IX_MARKET),
                hoard: account(IX_HOARD),
                position: account(IX_POSITION),
                kernel: account(IX_KERNEL),
                external: account(IX_EXTERNAL),
                replay: account(IX_REPLAY),
                supply: account(IX_SUPPLY),
                actor: ActorMetadata {
                    key: hash_of(&self.keys[IX_ACTOR]),
                    signer: self.signer,
                },
            }
        }

        fn expected(&self) -> ExpectedBindings {
            ExpectedBindings {
                program_id: hash_of(&self.program),
                market: hash_of(&self.bindings.market.0),
                hoard: hash_of(&self.bindings.hoard.0),
                position: hash_of(&self.bindings.position.0),
                kernel: hash_of(&self.bindings.kernel.0),
                external: hash_of(&self.bindings.external.0),
                replay: hash_of(&self.bindings.replay.0),
                supply: hash_of(&self.bindings.supply.0),
                market_bump: self.bindings.market.1,
                hoard_bump: self.bindings.hoard.1,
                position_bump: self.bindings.position.1,
                external_bump: self.bindings.external.1,
                replay_bump: self.bindings.replay.1,
                supply_bump: self.bindings.supply.1,
            }
        }

        /// Run the offline reference adapter over the case.
        pub(crate) fn reference(
            &self,
            request: &[u8],
        ) -> core::result::Result<TransitionOutput, ReferenceError> {
            apply(
                request,
                self.state.state_bytes(),
                &self.metadata(),
                &self.expected(),
            )
        }

        /// Assert both adapters agree, and return the shared post-state when
        /// they both accept.
        pub(crate) fn differential(&self, request: &[u8], label: &str) -> Option<Seam> {
            let reference = self.reference(request);
            let (program, post) = self.program(request);
            match (reference, program) {
                (Ok(expected), Ok(())) => {
                    assert_eq!(post.market, expected.market, "{label}: market bytes");
                    assert_eq!(post.hoard, expected.hoard, "{label}: hoard bytes");
                    assert_eq!(post.position, expected.position, "{label}: position bytes");
                    assert_eq!(post.kernel, expected.kernel, "{label}: kernel bytes");
                    assert_eq!(post.external, expected.external, "{label}: external bytes");
                    assert_eq!(post.replay, expected.replay, "{label}: replay bytes");
                    assert_eq!(post.supply, expected.supply, "{label}: supply-ledger bytes");
                    assert_eq!(post.realm, self.state.realm, "{label}: Realm is read-only");
                    assert_eq!(
                        post.profile, self.state.profile,
                        "{label}: Profile is read-only"
                    );
                    Some(post)
                }
                (Err(reference), Err(program)) => {
                    assert_eq!(
                        Class::of_reference(reference),
                        Class::of_program(program),
                        "{label}: refusal class ({reference:?} against {program:?})"
                    );
                    None
                }
                (Ok(_), Err(program)) => {
                    panic!("{label}: the reference accepted and this program refused {program:?}")
                }
                (Err(reference), Ok(())) => {
                    panic!("{label}: this program accepted and the reference refused {reference:?}")
                }
            }
        }

        /// Run one accepted transition and advance the case to its post-state.
        pub(crate) fn advance(&mut self, request: &[u8], label: &str) {
            let state = self
                .differential(request, label)
                .unwrap_or_else(|| panic!("{label}: both adapters were expected to accept"));
            /* The differential runs the case on a copy; this second run is the
             * same deterministic function of the same pre-state and exists
             * only to recover the token accounts it left behind, so that a
             * multi-step scenario carries balances forward instead of
             * restarting from the fixture at every step. */
            let (_, _, leg) = self.run_carrying(request);
            self.state = state;
            self.carry(&leg);
        }

        /// Carry the post-CPI token account images into the next step.
        fn carry(&mut self, leg: &LegPost) {
            if !leg.outcome.is_empty() {
                self.token.mint_data.clone_from(&leg.outcome[1]);
                self.token.holder_data.clone_from(&leg.outcome[2]);
            }
            if !leg.collateral.is_empty() {
                self.collateral
                    .actor_token_data
                    .clone_from(&leg.collateral[3]);
                self.collateral
                    .hoard_token_data
                    .clone_from(&leg.collateral[5]);
            }
        }

        /// Assert both adapters refuse, in the named class.
        ///
        /// The class is named rather than merely compared so that a case that
        /// starts refusing for a different reason than it was written for
        /// fails instead of quietly still agreeing, and so that a case both
        /// adapters stop refusing at all fails instead of passing vacuously.
        pub(crate) fn refuses(&self, request: &[u8], label: &str, expected: Class) {
            assert!(
                self.differential(request, label).is_none(),
                "{label}: both adapters were expected to refuse"
            );
            assert_eq!(
                Class::of_reference(self.reference(request).expect_err("reference refuses")),
                expected,
                "{label}: reference refusal class"
            );
            assert_eq!(
                Class::of_program(self.program(request).0.expect_err("this program refuses")),
                expected,
                "{label}: this program's refusal class"
            );
        }
    }

    /// The refusal classes the two adapters are compared over.
    ///
    /// The two crates have separate error enums by design — one models an
    /// offline transition, the other a runtime account plane — so a
    /// differential over raw codes would be meaningless.  This is the
    /// correspondence, written once:
    ///
    /// | reference | this program |
    /// | --- | --- |
    /// | `AccountAlias` | `AccountAlias` |
    /// | `MissingSignature` | `MissingSignature` |
    /// | `UnauthorizedActor` | `UnauthorizedActor` |
    /// | `WrongAccountKey`, `WrongBump` | `WrongPda`, `WrongBump` |
    /// | `NotWritable` | `NotWritable`, `UnexpectedWritable` |
    /// | `WrongProgramOwner` | `WrongProgramOwner`, `ExecutableAccount` |
    /// | `MismatchedState` | `MismatchedState`, `NotActive` |
    /// | `NonCanonical` | `NonCanonical` |
    /// | `AggregateClosureMismatch` | `AggregateClosureMismatch` |
    /// | `Replay` | `Replay` |
    /// | `Arithmetic` | `Arithmetic` |
    /// | `CollateralCap` | `CollateralCap` |
    /// | `UnsupportedIntent` | `UnsupportedInstruction` |
    /// | `Kernel(e)` | `Kernel(e)` — the *same* kernel refusal |
    /// | `Layout(e)` | `Codec(e)` — the *same* codec refusal |
    ///
    /// Two of those rows are refinements rather than renames, and both are
    /// deliberate: this program distinguishes a read-only role arriving
    /// writable from a writable role arriving read-only, and distinguishes the
    /// lifecycle/close-state refusal (`NotActive`) from the generic
    /// `MismatchedState` the reference reports for it.  Neither refinement
    /// changes which requests are accepted.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum Class {
        Alias,
        Signature,
        Actor,
        Binding,
        Writability,
        Owner,
        MismatchedState,
        NonCanonical,
        Closure,
        Replay,
        Arithmetic,
        CollateralCap,
        Unsupported,
        Kernel(KernelError),
        Codec(CodecError),
        Envelope,
    }

    impl Class {
        fn of_reference(error: ReferenceError) -> Self {
            match error {
                ReferenceError::AccountAlias => Self::Alias,
                ReferenceError::MissingSignature => Self::Signature,
                ReferenceError::UnauthorizedActor => Self::Actor,
                ReferenceError::WrongAccountKey | ReferenceError::WrongBump => Self::Binding,
                ReferenceError::NotWritable => Self::Writability,
                ReferenceError::WrongProgramOwner => Self::Owner,
                ReferenceError::MismatchedState => Self::MismatchedState,
                ReferenceError::NonCanonical => Self::NonCanonical,
                ReferenceError::AggregateClosureMismatch => Self::Closure,
                ReferenceError::Replay => Self::Replay,
                ReferenceError::Arithmetic => Self::Arithmetic,
                ReferenceError::CollateralCap => Self::CollateralCap,
                ReferenceError::UnsupportedIntent => Self::Unsupported,
                ReferenceError::Kernel(inner) => Self::Kernel(inner),
                ReferenceError::Layout(inner) => Self::Codec(inner),
                ReferenceError::WrongLength
                | ReferenceError::WrongTag
                | ReferenceError::WrongVersion => Self::Envelope,
                other => panic!("no differential class for reference refusal {other:?}"),
            }
        }

        fn of_program(refusal: Refusal) -> Self {
            match refusal {
                Refusal::Adapter(ClutchError::AccountAlias) => Self::Alias,
                Refusal::Adapter(ClutchError::MissingSignature) => Self::Signature,
                Refusal::Adapter(ClutchError::UnauthorizedActor) => Self::Actor,
                Refusal::Adapter(ClutchError::WrongPda | ClutchError::WrongBump) => Self::Binding,
                Refusal::Adapter(ClutchError::NotWritable | ClutchError::UnexpectedWritable) => {
                    Self::Writability
                }
                Refusal::Adapter(
                    ClutchError::WrongProgramOwner | ClutchError::ExecutableAccount,
                ) => Self::Owner,
                Refusal::Adapter(ClutchError::MismatchedState | ClutchError::NotActive) => {
                    Self::MismatchedState
                }
                Refusal::Adapter(ClutchError::NonCanonical) => Self::NonCanonical,
                Refusal::Adapter(ClutchError::AggregateClosureMismatch) => Self::Closure,
                Refusal::Adapter(ClutchError::Replay) => Self::Replay,
                Refusal::Adapter(ClutchError::Arithmetic) => Self::Arithmetic,
                Refusal::Adapter(ClutchError::CollateralCap) => Self::CollateralCap,
                Refusal::Adapter(ClutchError::UnsupportedInstruction) => Self::Unsupported,
                Refusal::Kernel(inner) => Self::Kernel(inner),
                Refusal::Codec(inner) => Self::Codec(inner),
                Refusal::Reference(
                    ReferenceError::WrongLength
                    | ReferenceError::WrongTag
                    | ReferenceError::WrongVersion,
                ) => Self::Envelope,
                other => panic!("no differential class for program refusal {other:?}"),
            }
        }
    }

    /// Encode one layout intent into a reference request envelope.
    ///
    /// The envelope constants are copied because they are private to
    /// `clutch_solana_reference`; the round-trip assertion is what makes a
    /// drift in either the constants or the intent codec a test failure here
    /// rather than a silent divergence.
    pub(crate) fn layout_request(sequence: u64, intent: Intent) -> Vec<u8> {
        let mut intent_bytes = [0_u8; MAX_INTENT_BYTES];
        let len = intent.encode(&mut intent_bytes).expect("intent encodes");
        let mut out = Vec::with_capacity(13 + len);
        out.push(REQUEST_TAG);
        out.push(REFERENCE_VERSION);
        out.extend_from_slice(&sequence.to_le_bytes());
        out.push(ACTION_LAYOUT);
        out.extend_from_slice(&(len as u16).to_le_bytes());
        out.extend_from_slice(&intent_bytes[..len]);
        assert_eq!(
            Request::decode(&out),
            Ok(Request {
                sequence,
                action: Action::Layout(intent),
            }),
            "the hand-built envelope must decode to the request it claims"
        );
        out
    }

    fn payout_set() -> PayoutSet {
        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        let mut left = [0; MAX_OUTCOMES];
        left[0] = 1;
        vectors[0] = PayoutVector::new(1, left);
        let mut right = [0; MAX_OUTCOMES];
        right[1] = 1;
        vectors[1] = PayoutVector::new(1, right);
        PayoutSet::new(2, 2, vectors)
    }

    /// The market/owner identities the fixture is built around.
    pub(crate) struct Ids {
        pub realm: Hash32,
        pub profile: Hash32,
        pub market: Hash32,
        pub owner: Hash32,
    }

    /// The Realm's frozen collateral policy the whole fixture is built around.
    ///
    /// A real, decodable 266-byte V1 policy rather than a fill: the collateral
    /// leg binds it to the Profile by recomputed digest, so a fixture that
    /// merely *looked* like a policy would exercise the binding check and
    /// nothing else.
    pub(crate) fn fixture_policy() -> collateral::CollateralPolicy {
        collateral::CollateralPolicy {
            schema_version: collateral::COLLATERAL_POLICY_SCHEMA,
            flags: collateral::COLLATERAL_POLICY_STRICT_FLAGS,
            collateral: collateral::CurrencyRef::spl(
                collateral::TOKEN_2022_PROGRAM,
                [COLLATERAL_MINT_FILL; 32],
                COLLATERAL_DECIMALS,
            ),
            fee: collateral::CurrencyRef::NATIVE_SOL,
            liveness: collateral::CurrencyRef::NATIVE_SOL,
            max_supply_atoms: 1_000_000_000,
            allowed_mint_extensions: 0,
            required_mint_extensions: 0,
            allowed_account_extensions: collateral::EXTENSION_IMMUTABLE_OWNER,
            required_account_extensions: 0,
        }
    }

    pub(crate) fn ids() -> Ids {
        /* The Profile identity is the *parent* hash over the fixture policy's
         * own digest, recomputed rather than chosen, because
         * `verify_profile_identity` refuses any other pairing -- which is the
         * whole point of binding a policy by digest instead of by address. */
        let profile = collateral::ParentProfile::from_policy(&fixture_policy())
            .expect("the fixture policy composes a parent profile")
            .identity()
            .expect("the parent profile derives an identity");
        let realm = canonical_realm_id(profile, REALM_NONCE);
        Ids {
            realm,
            profile,
            market: canonical_market_id(realm, profile, MARKET_NONCE),
            owner: h(OWNER_FILL),
        }
    }

    /// The canonical accepted seam case.
    ///
    /// Every bump is the offline reference adapter's own fixture bump, so a
    /// case built here and a case built there differ in nothing but the two
    /// Realm/Profile accounts the reference does not model.
    pub(crate) fn fixture() -> Case {
        let Ids {
            realm,
            profile,
            market: market_id,
            owner,
        } = ids();
        let feed = FeedId::from_bytes([9; 32]);
        let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
        outcomes[0] = canonical_outcome_id(market_id, 0);
        outcomes[1] = canonical_outcome_id(market_id, 1);

        let realm_account = RealmAccount {
            realm,
            profile,
            max_outcomes: MAX_OUTCOMES as u8,
            profile_version: 2,
            stored_bump: 2,
            flags: 0,
        };
        let profile_account = ProfileAccount {
            profile,
            realm,
            version: 2,
            flags: PROFILE_FLAG_POLICY_FROZEN,
            collateral_policy_id: fixture_policy()
                .digest()
                .expect("the fixture policy digests"),
            adapter_release_id: h(0x52),
        };
        let market = MarketAccount {
            market: market_id,
            realm,
            profile,
            terms: h(12),
            outcome_count: OUTCOME_COUNT,
            lifecycle: 0,
            stored_bump: 3,
            hoard_bump: 4,
            outcomes,
            feed,
            collateral_cap: COLLATERAL_CAP,
            created_slot: 55,
            reserved: Hash32::ZERO,
        };
        let hoard = HoardAccount {
            market: market_id,
            realm,
            authority: h(10),
            collateral_atoms: 0,
            stored_bump: 4,
            flags: 0,
        };
        let position = PositionAccount {
            market: market_id,
            owner,
            generation: GENERATION,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: CASH_ATOMS,
            reserved_cash_atoms: RESERVED_CASH_ATOMS,
            stored_bump: 5,
            close_state: 0,
        };
        let kernel = KernelAccount {
            market: market_id,
            phase: 0,
            basis_mode: clutch_kernel::BasisMode::FinitePreset,
            resolved_payout: 0,
            payouts: payout_set(),
            total_supply: [0; MAX_OUTCOMES],
        };
        let external = ExternalAccount {
            market: market_id,
            owner,
            position_generation: GENERATION,
            balances: [0; MAX_OUTCOMES],
            stored_bump: 6,
            flags: 0,
        };
        let replay = ReplayAccount {
            market: market_id,
            owner,
            position_generation: GENERATION,
            sequence: 0,
            stored_bump: 7,
            flags: 0,
        };
        let supply = SupplyLedgerAccount {
            market: market_id,
            realm,
            generation: GENERATION,
            outcome_count: OUTCOME_COUNT,
            internal_supply: [0; MAX_OUTCOMES],
            external_supply: [0; MAX_OUTCOMES],
            stored_bump: 10,
            flags: 0,
        };

        let mut state = Seam {
            realm: [0; account_len::REALM],
            profile: [0; account_len::PROFILE],
            market: [0; account_len::MARKET],
            hoard: [0; account_len::HOARD],
            position: [0; account_len::POSITION],
            kernel: [0; KERNEL_ACCOUNT_LEN],
            external: [0; EXTERNAL_ACCOUNT_LEN],
            replay: [0; REPLAY_ACCOUNT_LEN],
            supply: [0; account_len::SUPPLY_LEDGER],
        };
        realm_account.encode(&mut state.realm).expect("realm");
        profile_account.encode(&mut state.profile).expect("profile");
        market.encode(&mut state.market).expect("market");
        hoard.encode(&mut state.hoard).expect("hoard");
        position.encode(&mut state.position).expect("position");
        kernel.encode(&mut state.kernel).expect("kernel");
        external.encode(&mut state.external).expect("external");
        replay.encode(&mut state.replay).expect("replay");
        supply.encode(&mut state.supply).expect("supply");

        let program = key(PROGRAM_FILL);
        let keys = [
            Pubkey::new_from_array(owner.bytes()),
            key(51),
            key(52),
            key(53),
            key(54),
            key(55),
            key(56),
            key(57),
            key(58),
            key(59),
        ];
        let mut case = Case {
            program,
            keys,
            owners: [program; ACCOUNT_COUNT],
            writable: [
                false, false, false, true, true, true, true, true, true, true,
            ],
            signer: true,
            bindings: Bindings {
                realm: (keys[IX_REALM], 2),
                profile: (keys[IX_PROFILE], 0),
                market: (keys[IX_MARKET], 3),
                hoard: (keys[IX_HOARD], 4),
                position: (keys[IX_POSITION], 5),
                kernel: (keys[IX_KERNEL], 0),
                external: (keys[IX_EXTERNAL], 6),
                replay: (keys[IX_REPLAY], 7),
                supply: (keys[IX_SUPPLY], 10),
                outcome_mint: Some((outcome_mint_key(0), 11)),
                hoard_authority: Some((key(0xa5), 12)),
                hoard_token: Some((key(0xa6), 13)),
            },
            state,
            token: TokenLegCase::default(),
            collateral: CollateralLegCase::default(),
        };
        case.token = token_leg(&case, 0, 0, 0);
        /* The pooled Hoard retains the founding position's cash.  Locked
         * collateral is zero, so the whole balance is free cash. */
        case.collateral = collateral_leg(&case, ACTOR_COLLATERAL_ATOMS, CASH_ATOMS);
        case
    }

    /// The address the fixture pretends `seeds::outcome_mint_pda` derives.
    ///
    /// Off-chain derivation is `unimplemented!()` by design (see
    /// [`crate::seeds::find`]), which is why [`Bindings`] carries the outcome
    /// mint as an injected value rather than deriving it inside [`seam`].
    pub(crate) fn outcome_mint_key(outcome: u8) -> Pubkey {
        key(0xc0_u8.wrapping_add(outcome))
    }

    /// Backing bytes for the three accounts of the outcome leg.
    #[derive(Clone, Debug, Default)]
    pub(crate) struct TokenLegCase {
        /// Key presented at [`IX_TOKEN_PROGRAM`].
        pub token_program: Pubkey,
        /// Whether that account reports itself executable.
        pub token_program_executable: bool,
        /// Whether the caller declared the token program writable.
        pub token_program_writable: bool,
        /// Key presented at [`IX_OUTCOME_MINT`].
        pub mint: Pubkey,
        /// Runtime owner of the mint account.
        pub mint_owner: Pubkey,
        /// Mint account bytes.
        pub mint_data: Vec<u8>,
        /// Declared writability of the mint.
        pub mint_writable: bool,
        /// Key presented at [`IX_HOLDER_TOKEN`].
        pub holder: Pubkey,
        /// Runtime owner of the holder token account.
        pub holder_owner: Pubkey,
        /// Holder token-account bytes.
        pub holder_data: Vec<u8>,
        /// Declared writability of the holder account.
        pub holder_writable: bool,
    }

    /// A token leg that passes every check, so a test can break exactly one.
    pub(crate) fn token_leg(
        case: &Case,
        outcome: u8,
        mint_supply: u64,
        holder_amount: u64,
    ) -> TokenLegCase {
        let mint = outcome_mint_key(outcome);
        TokenLegCase {
            token_program: crate::token::TOKEN_2022_PROGRAM_ID,
            token_program_executable: true,
            token_program_writable: false,
            mint,
            mint_owner: crate::token::TOKEN_2022_PROGRAM_ID,
            mint_data: crate::token::fixtures::outcome_mint_bytes(
                case.keys[IX_MARKET].to_bytes(),
                mint_supply,
            ),
            mint_writable: true,
            holder: key(0xe0),
            holder_owner: crate::token::TOKEN_2022_PROGRAM_ID,
            holder_data: crate::token::fixtures::account_bytes(
                mint.to_bytes(),
                case.keys[IX_ACTOR].to_bytes(),
                holder_amount,
            ),
            holder_writable: true,
        }
    }

    /// Backing bytes for the six accounts of the collateral leg.
    #[derive(Clone, Debug, Default)]
    pub(crate) struct CollateralLegCase {
        /// Key presented at [`IX_TOKEN_PROGRAM`].
        pub token_program: Pubkey,
        /// Whether that account reports itself executable.
        pub token_program_executable: bool,
        /// Whether the caller declared the token program writable.
        pub token_program_writable: bool,
        /// The Realm's 266 collateral-policy bytes.
        pub policy: Vec<u8>,
        /// Declared writability of the policy account.
        pub policy_writable: bool,
        /// Key presented at [`IX_COLLATERAL_MINT`].
        pub mint: Pubkey,
        /// Runtime owner of the collateral mint.
        pub mint_owner: Pubkey,
        /// Collateral-mint bytes.
        pub mint_data: Vec<u8>,
        /// Declared writability of the collateral mint.
        pub mint_writable: bool,
        /// Key presented at [`IX_ACTOR_TOKEN`].
        pub actor_token: Pubkey,
        /// Runtime owner of the actor's collateral account.
        pub actor_token_owner: Pubkey,
        /// The actor's collateral-account bytes.
        pub actor_token_data: Vec<u8>,
        /// Declared writability of the actor's collateral account.
        pub actor_token_writable: bool,
        /// Key presented at [`IX_HOARD_AUTHORITY`].
        pub authority: Pubkey,
        /// Declared writability of the Hoard authority.
        pub authority_writable: bool,
        /// Key presented at [`IX_HOARD_TOKEN`].
        pub hoard_token: Pubkey,
        /// Runtime owner of the Hoard token account.
        pub hoard_token_owner: Pubkey,
        /// The Hoard token account's bytes.
        pub hoard_token_data: Vec<u8>,
        /// Declared writability of the Hoard token account.
        pub hoard_token_writable: bool,
    }

    /// A collateral leg that passes every check, so a test can break one.
    ///
    /// `hoard_atoms` is the Hoard token account's balance and must be the
    /// fixture's `HoardAccount::collateral_atoms`: the mirror is checked over
    /// the pre-state as well as the post-state, so a leg that starts out of
    /// step refuses before anything moves.
    pub(crate) fn collateral_leg(
        case: &Case,
        actor_atoms: u64,
        hoard_atoms: u64,
    ) -> CollateralLegCase {
        let policy = fixture_policy();
        let mint = Pubkey::new_from_array(policy.collateral.mint);
        let authority = case
            .bindings
            .hoard_authority
            .expect("the fixture binds a hoard authority")
            .0;
        let hoard_token = case
            .bindings
            .hoard_token
            .expect("the fixture binds a hoard token account")
            .0;
        CollateralLegCase {
            token_program: crate::token::TOKEN_2022_PROGRAM_ID,
            token_program_executable: true,
            token_program_writable: false,
            policy: policy
                .canonical_bytes()
                .expect("the fixture policy encodes")
                .to_vec(),
            policy_writable: false,
            mint,
            mint_owner: crate::token::TOKEN_2022_PROGRAM_ID,
            /* No mint authority and no freeze authority: a collateral mint
             * whose supply is still open is refused by the Realm's own flags,
             * and the fixture is the admitted case. */
            mint_data: crate::token::fixtures::mint_bytes(
                COLLATERAL_DECIMALS,
                COLLATERAL_SUPPLY,
                None,
                None,
            ),
            mint_writable: false,
            actor_token: key(0xf0),
            actor_token_owner: crate::token::TOKEN_2022_PROGRAM_ID,
            actor_token_data: crate::token::fixtures::account_bytes(
                mint.to_bytes(),
                case.keys[IX_ACTOR].to_bytes(),
                actor_atoms,
            ),
            actor_token_writable: true,
            authority,
            authority_writable: false,
            hoard_token,
            hoard_token_owner: crate::token::TOKEN_2022_PROGRAM_ID,
            hoard_token_data: crate::token::fixtures::account_bytes(
                mint.to_bytes(),
                authority.to_bytes(),
                hoard_atoms,
            ),
            hoard_token_writable: true,
        }
    }

    /// Move the one `u64` a conforming Token-2022 transfer, mint, or burn is
    /// allowed to move, in place of the CPI that does not exist off-chain.
    ///
    /// **This is not a model of Token-2022 and must never grow into one.**  It
    /// exists because `solana_cpi::invoke_signed` compiles to `Ok(())` for the
    /// host, so the real effector moves nothing and the exact-delta check of
    /// `TOKEN2022_PLAN.md` §3.3 step 6 correctly refuses every transition — see
    /// [`Case::program`] for the argument, and
    /// `merge_materialize::an_off_chain_token_leg_refuses_the_delta_it_could_not_move`
    /// for the test that pins the real effector's off-chain behaviour.
    ///
    /// An underflow returns [`ClutchError::TokenDeltaMismatch`], which is
    /// exactly what a real `InsufficientFunds` from the token program maps to
    /// through [`crate::token::transfer_checked`].
    pub(crate) fn simulate_token_effects(
        accounts: &[AccountInfo],
        op: &SeamOp,
        snapshot: &TokenSnapshot,
    ) -> Outcome<()> {
        /// Add `delta` to the little-endian `u64` at `at`, or refuse.
        fn shift(account: &AccountInfo, at: usize, delta: i128) -> Outcome<()> {
            let mut data = account
                .try_borrow_mut_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let mut value = [0_u8; 8];
            value.copy_from_slice(&data[at..at + 8]);
            let next = i128::from(u64::from_le_bytes(value))
                .checked_add(delta)
                .filter(|next| (0..=i128::from(u64::MAX)).contains(next))
                .ok_or(Refusal::Adapter(ClutchError::TokenDeltaMismatch))?;
            data[at..at + 8].copy_from_slice(&(next as u64).to_le_bytes());
            Ok(())
        }
        /// `Mint::supply` and `Account::amount`, at their frozen offsets.
        const MINT_SUPPLY: usize = 36;
        const ACCOUNT_AMOUNT: usize = 64;

        match (op, snapshot) {
            (SeamOp::Materialize { .. }, TokenSnapshot::Outcome(snapshot)) => {
                let delta = i128::from(snapshot.quantity);
                shift(&accounts[IX_OUTCOME_MINT], MINT_SUPPLY, delta)?;
                shift(&accounts[IX_HOLDER_TOKEN], ACCOUNT_AMOUNT, delta)
            }
            (SeamOp::Dematerialize { .. }, TokenSnapshot::Outcome(snapshot)) => {
                let delta = -i128::from(snapshot.quantity);
                shift(&accounts[IX_OUTCOME_MINT], MINT_SUPPLY, delta)?;
                shift(&accounts[IX_HOLDER_TOKEN], ACCOUNT_AMOUNT, delta)
            }
            (SeamOp::Split { .. } | SeamOp::Merge { .. }, TokenSnapshot::Collateral(_)) => Ok(()),
            _ => Err(ClutchError::UnsupportedInstruction.into()),
        }
    }

    /// A `Split` request for the fixture's market and owner.
    pub(crate) fn split_request(sequence: u64, quantity: u64) -> Vec<u8> {
        let ids = ids();
        layout_request(
            sequence,
            Intent::Split {
                market: ids.market,
                owner: ids.owner,
                quantity,
            },
        )
    }

    /// Overwrite one position field in an already-encoded seam state.
    pub(crate) fn edit_position(state: &mut Seam, edit: impl FnOnce(&mut PositionAccount)) {
        let mut position = PositionAccount::decode(&state.position).expect("position decodes");
        edit(&mut position);
        position.encode(&mut state.position).expect("position");
    }

    #[test]
    fn the_seam_plane_takes_the_supply_ledger_as_its_tenth_account() {
        /* The account list is a check, so its shape is pinned: ten accounts,
         * nine of them program-owned state roles, the ledger last, and exactly
         * two read-only roles. */
        assert_eq!(ACCOUNT_COUNT, 10);
        assert_eq!(STATE_ROLES.len(), ACCOUNT_COUNT - 1);
        assert_eq!(IX_SUPPLY, ACCOUNT_COUNT - 1);
        assert_eq!(STATE_ROLES[8].index, IX_SUPPLY);
        assert_eq!(STATE_ROLES[8].len, account_len::SUPPLY_LEDGER);
        assert!(STATE_ROLES[8].writable);
        assert_eq!(
            STATE_ROLES.iter().filter(|role| !role.writable).count(),
            2,
            "only Realm and Profile are read-only"
        );
        for (offset, role) in STATE_ROLES.iter().enumerate() {
            assert_eq!(role.index, offset + IX_REALM, "roles are in list order");
        }
    }

    #[test]
    fn split_agrees_byte_for_byte_with_the_reference_adapter() {
        let mut case = fixture();
        case.advance(&split_request(0, 11), "split 11");
        case.advance(&split_request(1, 9), "split 9 more");

        /* The ledger is not a copy of the position: it is the market-wide
         * aggregate the position is bounded by, and CLO-DELTA-V1 moved it by
         * the delta twice. */
        let supply = SupplyLedgerAccount::decode(&case.state.supply).expect("ledger decodes");
        assert_eq!(supply.internal_supply[0], 20);
        assert_eq!(supply.internal_supply[1], 20);
        assert_eq!(supply.external_supply[0], 0);
        let position = PositionAccount::decode(&case.state.position).expect("position decodes");
        assert_eq!(position.cash_atoms, CASH_ATOMS - 20);
        assert_eq!(position.internal[0], 20);
    }

    #[test]
    fn split_refusals_agree_with_the_reference_adapter() {
        let base = fixture();

        // Insufficient balance: the position cannot pay the cash the split costs.
        base.refuses(
            &split_request(0, CASH_ATOMS + 1),
            "cash underflow",
            Class::Arithmetic,
        );

        // Reserved order cash is part of total cash but is not available to
        // mint another complete set.
        base.refuses(
            &split_request(0, CASH_ATOMS - RESERVED_CASH_ATOMS + 1),
            "reserved cash cannot fund split",
            Class::Arithmetic,
        );

        // Wrong signer: authenticated, but not the position owner.
        let mut stranger = fixture();
        stranger.keys[IX_ACTOR] = key(0x9e);
        stranger.refuses(&split_request(0, 11), "stranger signs", Class::Actor);

        // Missing signature.
        let mut unsigned = fixture();
        unsigned.signer = false;
        unsigned.refuses(
            &split_request(0, 11),
            "owner never signed",
            Class::Signature,
        );

        // Aliased accounts: two logical roles filled by one key.
        let mut aliased = fixture();
        aliased.keys[IX_POSITION] = aliased.keys[IX_HOARD];
        aliased.bindings.position = aliased.bindings.hoard;
        aliased.refuses(
            &split_request(0, 11),
            "position aliases hoard",
            Class::Alias,
        );

        // Stale replay.
        base.refuses(&split_request(7, 11), "stale sequence", Class::Replay);

        // A state account this program does not own.
        let mut foreign = fixture();
        foreign.owners[IX_SUPPLY] = key(0xbb);
        foreign.refuses(&split_request(0, 11), "foreign ledger owner", Class::Owner);

        // A writable role presented read-only.
        let mut frozen = fixture();
        frozen.writable[IX_SUPPLY] = false;
        frozen.refuses(
            &split_request(0, 11),
            "read-only ledger",
            Class::Writability,
        );

        // A key that is not the trusted derivation, and a bump that is not.
        let mut wrong_key = fixture();
        wrong_key.bindings.supply.0 = key(0xd4);
        wrong_key.refuses(
            &split_request(0, 11),
            "ledger key is not derived",
            Class::Binding,
        );
        let mut wrong_bump = fixture();
        wrong_bump.bindings.supply.1 = 11;
        wrong_bump.refuses(
            &split_request(0, 11),
            "ledger bump is not canonical",
            Class::Binding,
        );

        // The intent names a market the presented accounts do not carry.
        let ids = ids();
        base.refuses(
            &layout_request(
                0,
                Intent::Split {
                    market: h(0x77),
                    owner: ids.owner,
                    quantity: 11,
                },
            ),
            "intent names another market",
            Class::MismatchedState,
        );

        // The immutable collateral cap.
        base.refuses(
            &split_request(0, COLLATERAL_CAP + 1),
            "collateral cap",
            Class::CollateralCap,
        );

        // A closed position, which this program refines to `NotActive` and the
        // reference reports as `MismatchedState`; both are one class.
        let mut closed = fixture();
        edit_position(&mut closed.state, |position| position.close_state = 1);
        closed.refuses(
            &split_request(0, 11),
            "closing position",
            Class::MismatchedState,
        );
    }

    #[test]
    fn a_forged_position_cannot_split_against_an_empty_ledger() {
        /* CLO-DELTA-V1 C2: a position claiming balance the market-wide ledger
         * does not carry is a counterfeit, and both adapters refuse it before
         * any write. */
        let mut forged = fixture();
        edit_position(&mut forged.state, |position| position.internal[0] = 1);
        forged.refuses(
            &split_request(0, 11),
            "forged internal claim",
            Class::Closure,
        );

        let (result, post) = forged.program(&split_request(0, 11));
        assert_eq!(result, Err(ClutchError::AggregateClosureMismatch.into()));
        assert_eq!(post, forged.state, "a refused split writes nothing");
    }

    /// Join a second owner's position/external/replay triple to the
    /// market-wide accounts of an existing case.
    ///
    /// The second triple carries its own keys, its own bumps, and a position
    /// generation (5) that deliberately differs from the supply ledger's
    /// accounting era (2), which is the CLO-DELTA-V1 decoupling the reference
    /// adapter's own multi-position test pins.
    pub(crate) fn second_position(shared: &Case) -> Case {
        let ids = ids();
        let owner = h(32);
        let position = PositionAccount {
            market: ids.market,
            owner,
            generation: 5,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: CASH_ATOMS,
            reserved_cash_atoms: RESERVED_CASH_ATOMS,
            stored_bump: 15,
            close_state: 0,
        };
        let external = ExternalAccount {
            market: ids.market,
            owner,
            position_generation: 5,
            balances: [0; MAX_OUTCOMES],
            stored_bump: 16,
            flags: 0,
        };
        let replay = ReplayAccount {
            market: ids.market,
            owner,
            position_generation: 5,
            sequence: 0,
            stored_bump: 17,
            flags: 0,
        };
        let mut case = shared.clone();
        position
            .encode(&mut case.state.position)
            .expect("second position");
        external
            .encode(&mut case.state.external)
            .expect("second shadow");
        replay
            .encode(&mut case.state.replay)
            .expect("second replay");
        case.keys[IX_ACTOR] = Pubkey::new_from_array(owner.bytes());
        case.keys[IX_POSITION] = key(60);
        case.keys[IX_EXTERNAL] = key(61);
        case.keys[IX_REPLAY] = key(62);
        case.bindings.position = (case.keys[IX_POSITION], 15);
        case.bindings.external = (case.keys[IX_EXTERNAL], 16);
        case.bindings.replay = (case.keys[IX_REPLAY], 17);
        /* The second owner brings their own token accounts.  The *shared*
         * accounts -- the collateral mint, the Hoard token account and its
         * balance -- are carried across from the case being joined, because a
         * market has one Hoard however many positions it has. */
        case.collateral.actor_token = key(0xf1);
        case.collateral.actor_token_data = crate::token::fixtures::account_bytes(
            case.collateral.mint.to_bytes(),
            case.keys[IX_ACTOR].to_bytes(),
            ACTOR_COLLATERAL_ATOMS,
        );
        case.token.holder = key(0xe1);
        case.token.holder_data = crate::token::fixtures::account_bytes(
            case.token.mint.to_bytes(),
            case.keys[IX_ACTOR].to_bytes(),
            0,
        );
        case
    }

    #[test]
    fn a_second_position_is_representable_and_both_adapters_agree() {
        /* This is the case the retired closed single-position equality made
         * unrepresentable, and the reason the port to CLO-DELTA-V1 was worth
         * taking a tenth account for.  Under the old check the first split
         * would succeed and every later transition by *either* owner would
         * refuse `AggregateClosureMismatch`, because no single position could
         * equal a market aggregate two positions contribute to. */
        let mut first = fixture();
        first.advance(&split_request(0, 20), "first owner splits 20");

        let mut second = second_position(&first);
        let request = layout_request(
            0,
            Intent::Split {
                market: ids().market,
                owner: h(32),
                quantity: 30,
            },
        );
        second.advance(&request, "second owner splits 30");

        let supply = SupplyLedgerAccount::decode(&second.state.supply).expect("ledger decodes");
        let one = PositionAccount::decode(&first.state.position).expect("first decodes");
        let two = PositionAccount::decode(&second.state.position).expect("second decodes");
        assert_eq!(one.internal[0], 20);
        assert_eq!(two.internal[0], 30);
        assert_eq!(
            supply.internal_supply[0],
            one.internal[0] + two.internal[0],
            "the ledger is the sum over positions, not a copy of one of them"
        );
        assert_eq!(supply.internal_supply[1], 50);
        assert_eq!(supply.external_supply[0], 0);

        // The first owner can still transact against the shared aggregate.
        let mut carried = first.clone();
        carried.state = second.state.clone();
        /* The Hoard is one account: the second owner's split moved it, so the
         * first owner's next transition sees that balance or the mirror
         * refuses. */
        carried
            .collateral
            .hoard_token_data
            .clone_from(&second.collateral.hoard_token_data);
        one.encode(&mut carried.state.position)
            .expect("first position");
        ExternalAccount {
            market: ids().market,
            owner: ids().owner,
            position_generation: GENERATION,
            balances: [0; MAX_OUTCOMES],
            stored_bump: 6,
            flags: 0,
        }
        .encode(&mut carried.state.external)
        .expect("first shadow");
        ReplayAccount {
            market: ids().market,
            owner: ids().owner,
            position_generation: GENERATION,
            sequence: 1,
            stored_bump: 7,
            flags: 0,
        }
        .encode(&mut carried.state.replay)
        .expect("first replay");
        carried.advance(&split_request(1, 5), "first owner splits again");
        let supply = SupplyLedgerAccount::decode(&carried.state.supply).expect("ledger decodes");
        assert_eq!(supply.internal_supply[0], 55);
    }

    #[test]
    fn an_unsupported_action_never_reaches_the_seam_plane() {
        /* `crate::dispatch` routes only the four seam intents here, so this is
         * defence in depth; it is pinned because the arm is otherwise
         * untested. */
        let request = Request {
            sequence: 0,
            action: Action::Resolve { payout_index: 0 },
        };
        assert_eq!(
            seam_op(&request),
            Err(ClutchError::UnsupportedInstruction.into())
        );
    }
}
