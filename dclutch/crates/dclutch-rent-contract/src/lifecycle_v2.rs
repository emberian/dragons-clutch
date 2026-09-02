//! Lifecycle-scoped RentCredit successor semantics.
//!
//! V2 replaces the permanent per-authority V1 account with one credit bound to
//! an immutable Market, release set, generation, and refund wallet. Surplus may
//! be swept only to that wallet. The account may close only after the current
//! Core role has produced the canonical complete producer-subtree retirement
//! receipt for the same Market lifecycle.

use core::convert::TryInto;

use dclutch_market_core_codec::RetirementReceiptV1;

use crate::{PUBKEY_BYTES, RENT_SYSVAR_ID, RefundAuthority, SYSTEM_PROGRAM_ID};

// Every width, coordinate, magic, seed domain and action tag below comes from
// `formal/dclutch-semantics/DClutchSemantics/LifecycleRentV2Abi.lean` through
// `EmitLifecycleRentV2Rust.lean`, and `check-generated.sh` byte-compares it.
// The three instructions share the sixteen-byte prologue this file used to
// implement by writing 0, 8, 10 and 11 into three encoders, and the Close
// request's payload is a whole Core retirement receipt, so its 528 is the
// prologue plus `MarketRetirementV1Abi.coreReceiptBytes` rather than a number
// of its own.
include!("generated_lifecycle_v2.rs");

/// Stable hostile-decode, binding, or exact-balance refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleRentErrorV2 {
    /// Input did not have its one exact width.
    InvalidLength,
    /// Magic or schema version selected another wire family.
    InvalidHeader,
    /// Action discriminator was not implemented.
    UnknownAction,
    /// Reserved bytes were not zero.
    NonCanonical,
    /// A required identity or generation was zero.
    ZeroIdentity,
    /// Required distinct identities aliased.
    AccountAlias,
    /// Persistent state did not bind the expected Market lifecycle.
    BindingMismatch,
    /// A sweep was zero or exceeded balance above the rent floor.
    InvalidSweep,
    /// The Core retirement receipt did not close this exact lifecycle.
    RetirementMismatch,
    /// The current Core role did not produce the supplied receipt.
    UnauthenticatedCore,
    /// Checked lamport arithmetic overflowed or underflowed.
    Arithmetic,
    /// Account privileges or runtime identities refused.
    InvalidFrame,
    /// Observed post-balances did not match the exact sweep plan.
    SweepPostcondition,
}

/// Result alias for lifecycle-scoped rent operations.
pub type LifecycleRentResultV2<T> = core::result::Result<T, LifecycleRentErrorV2>;

/// Validated nonzero account identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct LifecycleAccountIdV2([u8; PUBKEY_BYTES]);

impl LifecycleAccountIdV2 {
    /// Construct a nonzero identity.
    pub fn new(bytes: [u8; PUBKEY_BYTES]) -> LifecycleRentResultV2<Self> {
        if bytes.iter().all(|byte| *byte == 0) {
            Err(LifecycleRentErrorV2::ZeroIdentity)
        } else {
            Ok(Self(bytes))
        }
    }

    /// Return exact public-key bytes.
    pub const fn to_bytes(self) -> [u8; PUBKEY_BYTES] {
        self.0
    }
}

/// Exact runtime observation of the already-closed Market supplied to the
/// final lifecycle-credit close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRetiredMarketObservationV2 {
    /// Observed Market account identity.
    pub market: LifecycleAccountIdV2,
    /// Current account owner after Core closure.
    pub owner: [u8; PUBKEY_BYTES],
    /// Current account data length.
    pub data_len: u64,
    /// Current account lamports.
    pub lamports: u64,
    /// Child-frame signer privilege.
    pub signer: bool,
    /// Child-frame writable privilege.
    pub writable: bool,
    /// Executable flag.
    pub executable: bool,
}

impl LifecycleRetiredMarketObservationV2 {
    /// Require the exact immutable, empty System account produced by Core
    /// before Rent closes the lifecycle credit.
    pub fn validate(self, expected_market: LifecycleAccountIdV2) -> LifecycleRentResultV2<()> {
        if self.market != expected_market
            || self.owner != SYSTEM_PROGRAM_ID
            || self.data_len != 0
            || self.lamports != 0
            || self.signer
            || self.writable
            || self.executable
        {
            return Err(LifecycleRentErrorV2::InvalidFrame);
        }
        Ok(())
    }
}

/// Immutable lifecycle-scoped credit state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRentCreditV2 {
    refund_wallet: RefundAuthority,
    market: LifecycleAccountIdV2,
    release_set: LifecycleAccountIdV2,
    generation: u64,
    pda_bump: u8,
}

impl LifecycleRentCreditV2 {
    /// Construct one exact Market-lifecycle binding.
    pub fn new(
        refund_wallet: RefundAuthority,
        market: LifecycleAccountIdV2,
        release_set: LifecycleAccountIdV2,
        generation: u64,
        pda_bump: u8,
    ) -> LifecycleRentResultV2<Self> {
        if generation == 0 {
            return Err(LifecycleRentErrorV2::ZeroIdentity);
        }
        if refund_wallet.to_bytes() == market.to_bytes()
            || refund_wallet.to_bytes() == release_set.to_bytes()
            || market == release_set
        {
            return Err(LifecycleRentErrorV2::AccountAlias);
        }
        Ok(Self {
            refund_wallet,
            market,
            release_set,
            generation,
            pda_bump,
        })
    }

    /// Hostile-decode one exact persistent record.
    pub fn decode(input: &[u8]) -> LifecycleRentResultV2<Self> {
        require_header(
            input,
            &LIFECYCLE_RENT_CREDIT_MAGIC_V2,
            LIFECYCLE_RENT_CREDIT_BYTES_V2,
            STATE_MAGIC_OFFSET,
            STATE_VERSION_OFFSET,
        )?;
        require_zero(
            input,
            STATE_RESERVED_HEADER_OFFSET,
            STATE_RESERVED_HEADER_BYTES,
        )?;
        require_zero(input, STATE_RESERVED_BODY_OFFSET, STATE_RESERVED_BODY_BYTES)?;
        Self::new(
            RefundAuthority::new(array(input, STATE_REFUND_WALLET_OFFSET)?)
                .map_err(|_| LifecycleRentErrorV2::ZeroIdentity)?,
            LifecycleAccountIdV2::new(array(input, STATE_MARKET_OFFSET)?)?,
            LifecycleAccountIdV2::new(array(input, STATE_RELEASE_SET_OFFSET)?)?,
            u64_at(input, STATE_GENERATION_OFFSET)?,
            byte(input, STATE_BUMP_OFFSET)?,
        )
    }

    /// Encode canonical persistent bytes.
    pub fn to_bytes(self) -> [u8; LIFECYCLE_RENT_CREDIT_BYTES_V2] {
        let mut output = [0; LIFECYCLE_RENT_CREDIT_BYTES_V2];
        put(
            &mut output,
            STATE_MAGIC_OFFSET,
            &LIFECYCLE_RENT_CREDIT_MAGIC_V2,
        );
        put_u16(
            &mut output,
            STATE_VERSION_OFFSET,
            LIFECYCLE_RENT_SCHEMA_VERSION_V2,
        );
        output[STATE_BUMP_OFFSET] = self.pda_bump;
        put(
            &mut output,
            STATE_REFUND_WALLET_OFFSET,
            &self.refund_wallet.to_bytes(),
        );
        put(&mut output, STATE_MARKET_OFFSET, &self.market.to_bytes());
        put(
            &mut output,
            STATE_RELEASE_SET_OFFSET,
            &self.release_set.to_bytes(),
        );
        put_u64(&mut output, STATE_GENERATION_OFFSET, self.generation);
        output
    }

    /// Return the sole immutable refund wallet.
    pub const fn refund_wallet(self) -> RefundAuthority {
        self.refund_wallet
    }

    /// Return the bound Market.
    pub const fn market(self) -> LifecycleAccountIdV2 {
        self.market
    }

    /// Return the immutable release set.
    pub const fn release_set(self) -> LifecycleAccountIdV2 {
        self.release_set
    }

    /// Return the immutable Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Return the persisted PDA bump.
    pub const fn pda_bump(self) -> u8 {
        self.pda_bump
    }

    /// Return exact PDA seeds for an SDK-owning adapter.
    pub const fn pda_seeds(self) -> LifecycleRentCreditPdaSeedsV2 {
        LifecycleRentCreditPdaSeedsV2 {
            market: self.market,
            generation: self.generation.to_le_bytes(),
            bump: self.pda_bump,
        }
    }
}

/// Exact PDA seed projection for a lifecycle credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRentCreditPdaSeedsV2 {
    market: LifecycleAccountIdV2,
    generation: [u8; 8],
    bump: u8,
}

/// Exact current-Core signer seed projection for closing one lifecycle credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRentCoreCloseAuthoritySeedsV2 {
    credit: LifecycleAccountIdV2,
    post_resource_digest: [u8; 32],
}

impl LifecycleRentCoreCloseAuthoritySeedsV2 {
    /// Construct seeds bound to one credit and complete producer-subtree digest.
    pub fn new(
        credit: LifecycleAccountIdV2,
        post_resource_digest: [u8; 32],
    ) -> LifecycleRentResultV2<Self> {
        if post_resource_digest.iter().all(|byte| *byte == 0) {
            return Err(LifecycleRentErrorV2::ZeroIdentity);
        }
        Ok(Self {
            credit,
            post_resource_digest,
        })
    }

    /// Return the fixed authority domain.
    pub const fn domain(self) -> &'static [u8] {
        LIFECYCLE_RENT_CORE_CLOSE_AUTHORITY_DOMAIN_V2
    }

    /// Return the exact lifecycle-credit seed.
    pub const fn credit(self) -> LifecycleAccountIdV2 {
        self.credit
    }

    /// Return the complete producer-subtree digest seed.
    pub const fn post_resource_digest(self) -> [u8; 32] {
        self.post_resource_digest
    }
}

impl LifecycleRentCreditPdaSeedsV2 {
    /// Return the fixed PDA domain.
    pub const fn domain(self) -> &'static [u8] {
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2
    }

    /// Return the Market seed.
    pub const fn market(self) -> LifecycleAccountIdV2 {
        self.market
    }

    /// Return the generation seed bytes.
    pub const fn generation(self) -> [u8; 8] {
        self.generation
    }

    /// Return the PDA bump.
    pub const fn bump(self) -> u8 {
        self.bump
    }
}

/// V2 instruction action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LifecycleRentActionV2 {
    /// Create one Market-scoped credit.
    Create = LIFECYCLE_RENT_ACTION_CREATE_V2,
    /// Sweep surplus to the immutable wallet while preserving Rent.
    Sweep = LIFECYCLE_RENT_ACTION_SWEEP_V2,
    /// Close after complete producer-subtree retirement.
    Close = LIFECYCLE_RENT_ACTION_CLOSE_V2,
}

impl LifecycleRentActionV2 {
    fn decode(value: u8) -> LifecycleRentResultV2<Self> {
        match value {
            LIFECYCLE_RENT_ACTION_CREATE_V2 => Ok(Self::Create),
            LIFECYCLE_RENT_ACTION_SWEEP_V2 => Ok(Self::Sweep),
            LIFECYCLE_RENT_ACTION_CLOSE_V2 => Ok(Self::Close),
            _ => Err(LifecycleRentErrorV2::UnknownAction),
        }
    }
}

/// Canonical Create request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateLifecycleRentCreditV2(LifecycleRentCreditV2);

impl CreateLifecycleRentCreditV2 {
    /// Construct a request from exact successor state.
    pub const fn new(state: LifecycleRentCreditV2) -> Self {
        Self(state)
    }

    /// Decode one exact Create request.
    pub fn decode(input: &[u8]) -> LifecycleRentResultV2<Self> {
        require_instruction(
            input,
            LifecycleRentActionV2::Create,
            CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2,
        )?;
        require_zero(input, CREATE_RESERVED_OFFSET, CREATE_RESERVED_BYTES)?;
        Ok(Self(LifecycleRentCreditV2::new(
            RefundAuthority::new(array(input, CREATE_REFUND_WALLET_OFFSET)?)
                .map_err(|_| LifecycleRentErrorV2::ZeroIdentity)?,
            LifecycleAccountIdV2::new(array(input, CREATE_MARKET_OFFSET)?)?,
            LifecycleAccountIdV2::new(array(input, CREATE_RELEASE_SET_OFFSET)?)?,
            u64_at(input, CREATE_GENERATION_OFFSET)?,
            byte(input, CREATE_BUMP_OFFSET)?,
        )?))
    }

    /// Encode canonical request bytes.
    pub fn to_bytes(self) -> [u8; CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2] {
        let mut output = instruction_header(LifecycleRentActionV2::Create);
        put(
            &mut output,
            CREATE_REFUND_WALLET_OFFSET,
            &self.0.refund_wallet().to_bytes(),
        );
        put(
            &mut output,
            CREATE_MARKET_OFFSET,
            &self.0.market().to_bytes(),
        );
        put(
            &mut output,
            CREATE_RELEASE_SET_OFFSET,
            &self.0.release_set().to_bytes(),
        );
        put_u64(&mut output, CREATE_GENERATION_OFFSET, self.0.generation());
        output[CREATE_BUMP_OFFSET] = self.0.pda_bump();
        output
    }

    /// Return the exact state to create.
    pub const fn state(self) -> LifecycleRentCreditV2 {
        self.0
    }
}

/// Canonical surplus sweep request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SweepLifecycleRentCreditV2(u64);

impl SweepLifecycleRentCreditV2 {
    /// Construct a nonzero exact sweep.
    pub fn new(amount: u64) -> LifecycleRentResultV2<Self> {
        if amount == 0 {
            Err(LifecycleRentErrorV2::InvalidSweep)
        } else {
            Ok(Self(amount))
        }
    }

    /// Decode one exact Sweep request.
    pub fn decode(input: &[u8]) -> LifecycleRentResultV2<Self> {
        require_instruction(
            input,
            LifecycleRentActionV2::Sweep,
            SWEEP_LIFECYCLE_RENT_CREDIT_BYTES_V2,
        )?;
        Self::new(u64_at(input, SWEEP_AMOUNT_OFFSET)?)
    }

    /// Encode canonical request bytes.
    pub fn to_bytes(self) -> [u8; SWEEP_LIFECYCLE_RENT_CREDIT_BYTES_V2] {
        let mut output = [0; SWEEP_LIFECYCLE_RENT_CREDIT_BYTES_V2];
        put_instruction_header(&mut output, LifecycleRentActionV2::Sweep);
        put_u64(&mut output, SWEEP_AMOUNT_OFFSET, self.0);
        output
    }

    /// Return the requested surplus amount.
    pub const fn amount(self) -> u64 {
        self.0
    }
}

/// Canonical close request carrying the exact immediate Core receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseLifecycleRentCreditV2(RetirementReceiptV1);

impl CloseLifecycleRentCreditV2 {
    /// Construct from one hostile-decoded Core retirement receipt.
    pub const fn new(receipt: RetirementReceiptV1) -> Self {
        Self(receipt)
    }

    /// Decode one exact Close request.
    pub fn decode(input: &[u8]) -> LifecycleRentResultV2<Self> {
        require_instruction(
            input,
            LifecycleRentActionV2::Close,
            CLOSE_LIFECYCLE_RENT_CREDIT_BYTES_V2,
        )?;
        RetirementReceiptV1::decode(
            input
                .get(CLOSE_RECEIPT_OFFSET..)
                .ok_or(LifecycleRentErrorV2::InvalidLength)?,
        )
        .map(Self)
        .map_err(|_| LifecycleRentErrorV2::RetirementMismatch)
    }

    /// Encode canonical request bytes.
    pub fn to_bytes(self) -> [u8; CLOSE_LIFECYCLE_RENT_CREDIT_BYTES_V2] {
        let mut output = [0; CLOSE_LIFECYCLE_RENT_CREDIT_BYTES_V2];
        put_instruction_header(&mut output, LifecycleRentActionV2::Close);
        put(&mut output, CLOSE_RECEIPT_OFFSET, &self.0.to_bytes());
        output
    }

    /// Return the validated Core receipt.
    pub const fn receipt(self) -> RetirementReceiptV1 {
        self.0
    }
}

/// Hostile-decoded V2 instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleRentInstructionV2 {
    /// Create one lifecycle credit.
    Create,
    /// Sweep surplus to its wallet.
    Sweep,
    /// Close after complete retirement.
    Close,
}

impl LifecycleRentInstructionV2 {
    /// Decode exactly one canonical V2 instruction.
    pub fn decode(input: &[u8]) -> LifecycleRentResultV2<Self> {
        if input.len() < LIFECYCLE_RENT_INSTRUCTION_HEADER_BYTES_V2 {
            return Err(LifecycleRentErrorV2::InvalidLength);
        }
        match LifecycleRentActionV2::decode(byte(input, INSTRUCTION_ACTION_OFFSET)?)? {
            LifecycleRentActionV2::Create => {
                CreateLifecycleRentCreditV2::decode(input).map(|_| Self::Create)
            }
            LifecycleRentActionV2::Sweep => {
                SweepLifecycleRentCreditV2::decode(input).map(|_| Self::Sweep)
            }
            LifecycleRentActionV2::Close => {
                CloseLifecycleRentCreditV2::decode(input).map(|_| Self::Close)
            }
        }
    }
}

/// Denominator of the crank's share of one admitted surplus sweep.
///
/// The reward is `min(rent_floor, swept / DIVISOR)`, so the refund wallet keeps
/// at least `(DIVISOR - 1) / DIVISOR` of every admitted sweep.
///
/// **A ratio, deliberately, where `docs/design/FUNDED_CRANK_V1.md` §3 forbids a
/// lamport literal.** That rule exists because a lamport constant goes stale
/// when the fee market moves while `minimum_balance` re-derives itself every
/// block — the magnitude here still comes from the Rent sysvar. A *share* is
/// not a magnitude and does not go stale.
///
/// **Why a share rather than the plain `min(floor, residual)` cap that a closing
/// route uses.** Sweep is the tree's only *surplus* route (§3.1): the credit
/// survives, and the amount is chosen by the caller rather than fixed by what
/// an account held. Under a plain floor cap a cranker would sweep exactly the
/// floor and take 100% of it, repeatedly, and the wallet would receive nothing
/// forever from a route that reads as funded. A share makes that unprofitable
/// by construction instead of by threshold.
pub const LIFECYCLE_SWEEP_CRANK_SHARE_DIVISOR_V2: u64 = 16;

/// Exact balance plan for a surplus sweep, with or without a paid crank.
///
/// The unpaid shape is not deprecated: when the caller *is* the refund wallet
/// they are already the beneficiary and need no reward, which is the census's
/// GREEN-SELF shape rather than a gap. The crank recipient is optional so that
/// stays a first-class path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleSweepPlanV2 {
    credit_after: u64,
    wallet_after: u64,
    crank_reward: u64,
}

impl LifecycleSweepPlanV2 {
    /// Plan a sweep that preserves the current Rent minimum and pays no crank.
    pub fn new(
        credit_before: u64,
        wallet_before: u64,
        rent_minimum: u64,
        request: SweepLifecycleRentCreditV2,
    ) -> LifecycleRentResultV2<Self> {
        Self::plan(credit_before, wallet_before, rent_minimum, request, 0)
    }

    /// Plan the same sweep with a capped share paid to a permissionless crank.
    ///
    /// `reward_cap` is the chain-derived floor the caller's share is clamped to
    /// — the Rent minimum of the credit, which the frame has already computed.
    /// **This can never refuse for lack of funds.** The reward is a share of a
    /// sum that is already moving, so a thin sweep yields a small reward, or
    /// zero, and never an error: a crank that could refuse for money is an
    /// unturned crank, which is the unfunded-liveness defect through the
    /// funding door (`FUNDED_CRANK_V1.md` §2).
    pub fn new_with_crank(
        credit_before: u64,
        wallet_before: u64,
        rent_minimum: u64,
        request: SweepLifecycleRentCreditV2,
        reward_cap: u64,
    ) -> LifecycleRentResultV2<Self> {
        let share = request
            .amount()
            .checked_div(LIFECYCLE_SWEEP_CRANK_SHARE_DIVISOR_V2)
            .ok_or(LifecycleRentErrorV2::Arithmetic)?;
        let reward = if reward_cap < share {
            reward_cap
        } else {
            share
        };
        Self::plan(credit_before, wallet_before, rent_minimum, request, reward)
    }

    fn plan(
        credit_before: u64,
        wallet_before: u64,
        rent_minimum: u64,
        request: SweepLifecycleRentCreditV2,
        crank_reward: u64,
    ) -> LifecycleRentResultV2<Self> {
        let credit_after = credit_before
            .checked_sub(request.amount())
            .ok_or(LifecycleRentErrorV2::InvalidSweep)?;
        if credit_after < rent_minimum {
            return Err(LifecycleRentErrorV2::InvalidSweep);
        }
        let wallet_credit = request
            .amount()
            .checked_sub(crank_reward)
            .ok_or(LifecycleRentErrorV2::InvalidSweep)?;
        let wallet_after = wallet_before
            .checked_add(wallet_credit)
            .ok_or(LifecycleRentErrorV2::Arithmetic)?;
        Ok(Self {
            credit_after,
            wallet_after,
            crank_reward,
        })
    }

    /// Return the exact credit post-balance.
    pub const fn credit_after(self) -> u64 {
        self.credit_after
    }

    /// Return the exact wallet post-balance.
    pub const fn wallet_after(self) -> u64 {
        self.wallet_after
    }

    /// Return the exact lamports owed to the crank, zero when none is present.
    pub const fn crank_reward(self) -> u64 {
        self.crank_reward
    }

    /// Recheck the plan against observed post-balances.
    ///
    /// `crank_after` is the crank recipient's observed balance and its
    /// pre-balance, or `None` when the sweep named no crank.
    ///
    /// **This is the non-vacuous half, and the distinction matters.** A
    /// conservation identity over these fields alone would be a tautology —
    /// the constructor computes `credit_after` and `wallet_after` from the same
    /// amount, so they reconcile whatever the code does. What is worth checking
    /// is *plan against observation*: that the program applied what the plan
    /// said, to all three accounts, and moved nothing else. That is why this
    /// takes observations rather than proving something about `self`, and why
    /// it matches the `validate_post` shape its sibling plans already use.
    pub fn validate_post(
        self,
        credit_after: u64,
        wallet_after: u64,
        crank_after: Option<(u64, u64)>,
    ) -> LifecycleRentResultV2<()> {
        if credit_after != self.credit_after || wallet_after != self.wallet_after {
            return Err(LifecycleRentErrorV2::SweepPostcondition);
        }
        match crank_after {
            Some((before, after)) => {
                let expected = before
                    .checked_add(self.crank_reward)
                    .ok_or(LifecycleRentErrorV2::Arithmetic)?;
                if after != expected {
                    return Err(LifecycleRentErrorV2::SweepPostcondition);
                }
            }
            None => {
                if self.crank_reward != 0 {
                    return Err(LifecycleRentErrorV2::SweepPostcondition);
                }
            }
        }
        Ok(())
    }
}

/// Exact balance and closure plan after authenticated retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleClosePlanV2 {
    wallet_after: u64,
    closed_lamports: u64,
    post_resource_digest: [u8; 32],
}

impl LifecycleClosePlanV2 {
    /// Validate exact state/receipt bindings and plan full account closure.
    pub fn new(
        state: LifecycleRentCreditV2,
        credit: LifecycleAccountIdV2,
        current_core_program: LifecycleAccountIdV2,
        current_core_authenticated: bool,
        credit_lamports: u64,
        wallet_lamports: u64,
        request: CloseLifecycleRentCreditV2,
    ) -> LifecycleRentResultV2<Self> {
        if !current_core_authenticated {
            return Err(LifecycleRentErrorV2::UnauthenticatedCore);
        }
        let receipt = request.receipt().input();
        if receipt.core_program != current_core_program.to_bytes()
            || receipt.market != state.market().to_bytes()
            || receipt.release_set != state.release_set().to_bytes()
            || receipt.rent_credit != credit.to_bytes()
            || receipt.generation != state.generation()
        {
            return Err(LifecycleRentErrorV2::RetirementMismatch);
        }
        let wallet_after = wallet_lamports
            .checked_add(credit_lamports)
            .ok_or(LifecycleRentErrorV2::Arithmetic)?;
        Ok(Self {
            wallet_after,
            closed_lamports: credit_lamports,
            post_resource_digest: receipt.post_resource_digest,
        })
    }

    /// Return the exact wallet post-balance.
    pub const fn wallet_after(self) -> u64 {
        self.wallet_after
    }

    /// Return all lamports removed from the closed credit.
    pub const fn closed_lamports(self) -> u64 {
        self.closed_lamports
    }

    /// Return the complete producer-subtree closure commitment.
    pub const fn post_resource_digest(self) -> [u8; 32] {
        self.post_resource_digest
    }

    /// Materialize the canonical immediate Rent close receipt.
    pub fn receipt(
        self,
        state: LifecycleRentCreditV2,
        credit: LifecycleAccountIdV2,
    ) -> LifecycleRentResultV2<LifecycleRentCloseReceiptV2> {
        LifecycleRentCloseReceiptV2::new(LifecycleRentCloseReceiptInputV2 {
            credit,
            refund_wallet: state.refund_wallet(),
            market: state.market(),
            release_set: state.release_set(),
            post_resource_digest: self.post_resource_digest,
            generation: state.generation(),
            closed_lamports: self.closed_lamports,
        })
    }
}

/// Construction input for one immediate lifecycle-credit close receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRentCloseReceiptInputV2 {
    /// Closed lifecycle-credit account.
    pub credit: LifecycleAccountIdV2,
    /// Sole immutable refund wallet.
    pub refund_wallet: RefundAuthority,
    /// Retired Market.
    pub market: LifecycleAccountIdV2,
    /// Immutable release set.
    pub release_set: LifecycleAccountIdV2,
    /// Complete producer-subtree closure commitment.
    pub post_resource_digest: [u8; 32],
    /// Retired Market generation.
    pub generation: u64,
    /// Full lifecycle-credit balance transferred to the wallet.
    pub closed_lamports: u64,
}

/// Immediate producer-bound Rent close acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRentCloseReceiptV2(LifecycleRentCloseReceiptInputV2);

impl LifecycleRentCloseReceiptV2 {
    /// Construct and validate an immediate close receipt.
    pub fn new(input: LifecycleRentCloseReceiptInputV2) -> LifecycleRentResultV2<Self> {
        if input.post_resource_digest.iter().all(|byte| *byte == 0)
            || input.generation == 0
            || input.closed_lamports == 0
        {
            return Err(LifecycleRentErrorV2::ZeroIdentity);
        }
        if input.credit.to_bytes() == input.refund_wallet.to_bytes()
            || input.credit == input.market
            || input.credit == input.release_set
        {
            return Err(LifecycleRentErrorV2::AccountAlias);
        }
        Ok(Self(input))
    }

    /// Hostile-decode one exact immediate close receipt.
    pub fn decode(input: &[u8]) -> LifecycleRentResultV2<Self> {
        require_header(
            input,
            &LIFECYCLE_RENT_CLOSE_RECEIPT_MAGIC_V2,
            LIFECYCLE_RENT_CLOSE_RECEIPT_BYTES_V2,
            RECEIPT_MAGIC_OFFSET,
            RECEIPT_VERSION_OFFSET,
        )?;
        if byte(input, RECEIPT_KIND_OFFSET)? != LifecycleRentActionV2::Close as u8 {
            return Err(LifecycleRentErrorV2::UnknownAction);
        }
        require_zero(
            input,
            RECEIPT_RESERVED_HEADER_OFFSET,
            RECEIPT_RESERVED_HEADER_BYTES,
        )?;
        Self::new(LifecycleRentCloseReceiptInputV2 {
            credit: LifecycleAccountIdV2::new(array(input, RECEIPT_CREDIT_OFFSET)?)?,
            refund_wallet: RefundAuthority::new(array(input, RECEIPT_REFUND_WALLET_OFFSET)?)
                .map_err(|_| LifecycleRentErrorV2::ZeroIdentity)?,
            market: LifecycleAccountIdV2::new(array(input, RECEIPT_MARKET_OFFSET)?)?,
            release_set: LifecycleAccountIdV2::new(array(input, RECEIPT_RELEASE_SET_OFFSET)?)?,
            post_resource_digest: array(input, RECEIPT_POST_RESOURCE_DIGEST_OFFSET)?,
            generation: u64_at(input, RECEIPT_GENERATION_OFFSET)?,
            closed_lamports: u64_at(input, RECEIPT_CLOSED_LAMPORTS_OFFSET)?,
        })
    }

    /// Encode canonical immediate receipt bytes.
    pub fn to_bytes(self) -> [u8; LIFECYCLE_RENT_CLOSE_RECEIPT_BYTES_V2] {
        let mut output = [0; LIFECYCLE_RENT_CLOSE_RECEIPT_BYTES_V2];
        put(
            &mut output,
            RECEIPT_MAGIC_OFFSET,
            &LIFECYCLE_RENT_CLOSE_RECEIPT_MAGIC_V2,
        );
        put_u16(
            &mut output,
            INSTRUCTION_VERSION_OFFSET,
            LIFECYCLE_RENT_SCHEMA_VERSION_V2,
        );
        output[RECEIPT_KIND_OFFSET] = LifecycleRentActionV2::Close as u8;
        put(
            &mut output,
            RECEIPT_CREDIT_OFFSET,
            &self.0.credit.to_bytes(),
        );
        put(
            &mut output,
            RECEIPT_REFUND_WALLET_OFFSET,
            &self.0.refund_wallet.to_bytes(),
        );
        put(
            &mut output,
            RECEIPT_MARKET_OFFSET,
            &self.0.market.to_bytes(),
        );
        put(
            &mut output,
            RECEIPT_RELEASE_SET_OFFSET,
            &self.0.release_set.to_bytes(),
        );
        put(
            &mut output,
            RECEIPT_POST_RESOURCE_DIGEST_OFFSET,
            &self.0.post_resource_digest,
        );
        put_u64(&mut output, RECEIPT_GENERATION_OFFSET, self.0.generation);
        put_u64(
            &mut output,
            RECEIPT_CLOSED_LAMPORTS_OFFSET,
            self.0.closed_lamports,
        );
        output
    }

    /// Borrow all validated receipt coordinates.
    pub const fn input(self) -> LifecycleRentCloseReceiptInputV2 {
        self.0
    }
}

/// Minimal account facts used by SDK-owning adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleAccountMetaV2 {
    /// Public-key bytes.
    pub key: [u8; PUBKEY_BYTES],
    /// Transaction signer privilege.
    pub signer: bool,
    /// Writable privilege.
    pub writable: bool,
    /// Executable runtime flag.
    pub executable: bool,
}

/// Validate the fixed four-account Create frame.
///
/// Ordered payer, credit, System Program, Rent sysvar. The adapter must
/// additionally authenticate the payer as a System payer, the credit key as the
/// vacant PDA derived from Create data, and the Rent value used to obtain the
/// current minimum. No authority account exists: creation is a third-party
/// reserve donation.
///
/// This policy came from `CreateRentCreditFrameV1`, which the V2 Create route
/// had been borrowing; it moved here when the V1 routes were deleted on
/// 2026-08-27, unchanged in what it admits.
pub fn validate_create_frame_v2(
    payer: LifecycleAccountMetaV2,
    credit: LifecycleAccountMetaV2,
    system: LifecycleAccountMetaV2,
    rent: LifecycleAccountMetaV2,
) -> LifecycleRentResultV2<()> {
    if payer.key == [0; PUBKEY_BYTES]
        || credit.key == [0; PUBKEY_BYTES]
        || !payer.signer
        || !payer.writable
        || payer.executable
        || credit.signer
        || !credit.writable
        || credit.executable
        || system.key != SYSTEM_PROGRAM_ID
        || system.signer
        || system.writable
        || !system.executable
        || rent.key != RENT_SYSVAR_ID
        || rent.signer
        || rent.writable
        || rent.executable
        || payer.key == credit.key
        || payer.key == system.key
        || payer.key == rent.key
        || credit.key == system.key
        || credit.key == rent.key
        || system.key == rent.key
    {
        Err(LifecycleRentErrorV2::InvalidFrame)
    } else {
        Ok(())
    }
}

/// Validate the fixed three-account Sweep frame.
pub fn validate_sweep_frame_v2(
    state: LifecycleRentCreditV2,
    credit: LifecycleAccountMetaV2,
    wallet: LifecycleAccountMetaV2,
    rent: LifecycleAccountMetaV2,
) -> LifecycleRentResultV2<()> {
    if !credit.writable
        || credit.signer
        || credit.executable
        || !wallet.writable
        || wallet.executable
        || wallet.key != state.refund_wallet().to_bytes()
        || rent.key != RENT_SYSVAR_ID
        || rent.signer
        || rent.writable
        || rent.executable
        || credit.key == wallet.key
        || credit.key == rent.key
        || wallet.key == rent.key
    {
        Err(LifecycleRentErrorV2::InvalidFrame)
    } else {
        Ok(())
    }
}

/// Validate the optional crank recipient of a funded Sweep.
///
/// **Deliberately silent about `signer`, in either direction.** The crank is
/// usually the fee payer and therefore usually signs, but it does not have to:
/// a signature here would establish *who is owed*, never *who is permitted*
/// (`docs/design/FUNDED_CRANK_V1.md` §6). Requiring one would gate a
/// permissionless verb on a signature; *refusing* one is the live defect §6
/// names, where a cleanup's beneficiary is forbidden to pay its own fee and so
/// nobody turns the crank at all.
///
/// The recipient must be an ordinary System wallet — the same shape the refund
/// wallet is held to — and must alias none of the fixed three.
pub fn validate_sweep_crank_v2(
    crank: LifecycleAccountMetaV2,
    owner: [u8; PUBKEY_BYTES],
    data_len: u64,
    credit: LifecycleAccountMetaV2,
    wallet: LifecycleAccountMetaV2,
    rent: LifecycleAccountMetaV2,
) -> LifecycleRentResultV2<()> {
    if !crank.writable
        || crank.executable
        || owner != SYSTEM_PROGRAM_ID
        || data_len != 0
        || crank.key == credit.key
        || crank.key == wallet.key
        || crank.key == rent.key
    {
        Err(LifecycleRentErrorV2::InvalidFrame)
    } else {
        Ok(())
    }
}

/// Validate the immutable refund wallet's runtime shape.
pub fn validate_refund_wallet_v2(
    owner: [u8; PUBKEY_BYTES],
    data_len: u64,
) -> LifecycleRentResultV2<()> {
    if owner == SYSTEM_PROGRAM_ID && data_len == 0 {
        Ok(())
    } else {
        Err(LifecycleRentErrorV2::InvalidFrame)
    }
}

fn require_instruction(
    input: &[u8],
    action: LifecycleRentActionV2,
    width: usize,
) -> LifecycleRentResultV2<()> {
    require_header(
        input,
        &LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2,
        width,
        INSTRUCTION_MAGIC_OFFSET,
        INSTRUCTION_VERSION_OFFSET,
    )?;
    if byte(input, INSTRUCTION_ACTION_OFFSET)? != action as u8 {
        return Err(LifecycleRentErrorV2::UnknownAction);
    }
    require_zero(
        input,
        INSTRUCTION_RESERVED_OFFSET,
        INSTRUCTION_RESERVED_BYTES,
    )
}

fn require_header(
    input: &[u8],
    magic: &[u8; 8],
    width: usize,
    magic_offset: usize,
    version_offset: usize,
) -> LifecycleRentResultV2<()> {
    if input.len() != width
        || input.get(magic_offset..magic_offset + magic.len()) != Some(magic.as_slice())
    {
        return Err(LifecycleRentErrorV2::InvalidLength);
    }
    if u16_at(input, version_offset)? != LIFECYCLE_RENT_SCHEMA_VERSION_V2 {
        return Err(LifecycleRentErrorV2::InvalidHeader);
    }
    Ok(())
}

fn instruction_header(
    action: LifecycleRentActionV2,
) -> [u8; CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2] {
    let mut output = [0; CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2];
    put_instruction_header(&mut output, action);
    output
}

fn put_instruction_header(output: &mut [u8], action: LifecycleRentActionV2) {
    put(
        output,
        INSTRUCTION_MAGIC_OFFSET,
        &LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2,
    );
    put_u16(
        output,
        INSTRUCTION_VERSION_OFFSET,
        LIFECYCLE_RENT_SCHEMA_VERSION_V2,
    );
    if let Some(target) = output.get_mut(INSTRUCTION_ACTION_OFFSET) {
        *target = action as u8;
    }
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> LifecycleRentResultV2<()> {
    let end = offset
        .checked_add(width)
        .ok_or(LifecycleRentErrorV2::InvalidLength)?;
    if input
        .get(offset..end)
        .ok_or(LifecycleRentErrorV2::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(LifecycleRentErrorV2::NonCanonical)
    } else {
        Ok(())
    }
}

fn byte(input: &[u8], offset: usize) -> LifecycleRentResultV2<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(LifecycleRentErrorV2::InvalidLength)
}

fn array(input: &[u8], offset: usize) -> LifecycleRentResultV2<[u8; 32]> {
    let end = offset
        .checked_add(32)
        .ok_or(LifecycleRentErrorV2::InvalidLength)?;
    input
        .get(offset..end)
        .ok_or(LifecycleRentErrorV2::InvalidLength)?
        .try_into()
        .map_err(|_| LifecycleRentErrorV2::InvalidLength)
}

fn u16_at(input: &[u8], offset: usize) -> LifecycleRentResultV2<u16> {
    let end = offset
        .checked_add(2)
        .ok_or(LifecycleRentErrorV2::InvalidLength)?;
    Ok(u16::from_le_bytes(
        input
            .get(offset..end)
            .ok_or(LifecycleRentErrorV2::InvalidLength)?
            .try_into()
            .map_err(|_| LifecycleRentErrorV2::InvalidLength)?,
    ))
}

fn u64_at(input: &[u8], offset: usize) -> LifecycleRentResultV2<u64> {
    let end = offset
        .checked_add(8)
        .ok_or(LifecycleRentErrorV2::InvalidLength)?;
    Ok(u64::from_le_bytes(
        input
            .get(offset..end)
            .ok_or(LifecycleRentErrorV2::InvalidLength)?
            .try_into()
            .map_err(|_| LifecycleRentErrorV2::InvalidLength)?,
    ))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset.saturating_add(value.len())) {
        target.copy_from_slice(value);
    }
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    put(output, offset, &value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    put(output, offset, &value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use dclutch_market_core_codec::{RetirementReceiptInputV1, RetirementReceiptV1};

    use super::*;

    fn id(seed: u8) -> LifecycleAccountIdV2 {
        LifecycleAccountIdV2::new([seed; 32]).expect("nonzero")
    }

    fn state() -> LifecycleRentCreditV2 {
        LifecycleRentCreditV2::new(
            RefundAuthority::new([1; 32]).expect("wallet"),
            id(2),
            id(3),
            4,
            255,
        )
        .expect("state")
    }

    fn receipt(credit: LifecycleAccountIdV2) -> RetirementReceiptV1 {
        RetirementReceiptV1::new(RetirementReceiptInputV1 {
            core_program: [4; 32],
            market: state().market().to_bytes(),
            release_set: state().release_set().to_bytes(),
            rent_credit: credit.to_bytes(),
            bundle_digest: [5; 32],
            source_receipt_digest: [6; 32],
            claims_receipt_digest: [7; 32],
            custody_close_vault_receipt_digest: [8; 32],
            custody_close_replay_receipt_digest: [9; 32],
            pre_state_digest: [10; 32],
            retired_candidate_digest: [11; 32],
            post_resource_digest: [12; 32],
            generation: state().generation(),
            source_closure_revision: 13,
            claims_post_revision: 14,
            custody_post_revision: 15,
            core_refund_lamports: 16,
            claims_refund_lamports: 17,
            custody_refund_lamports: 18,
        })
        .expect("receipt")
    }

    #[test]
    fn state_and_instructions_are_canonical() {
        let state = state();
        assert_eq!(LifecycleRentCreditV2::decode(&state.to_bytes()), Ok(state));
        let create = CreateLifecycleRentCreditV2::new(state);
        assert_eq!(
            CreateLifecycleRentCreditV2::decode(&create.to_bytes()),
            Ok(create)
        );
        let sweep = SweepLifecycleRentCreditV2::new(7).expect("sweep");
        assert_eq!(
            SweepLifecycleRentCreditV2::decode(&sweep.to_bytes()),
            Ok(sweep)
        );
        let close = CloseLifecycleRentCreditV2::new(receipt(id(20)));
        assert_eq!(
            CloseLifecycleRentCreditV2::decode(&close.to_bytes()),
            Ok(close)
        );
    }

    #[test]
    fn reserved_and_trailing_bytes_refuse() {
        let mut state_bytes = state().to_bytes();
        state_bytes[121] = 1;
        assert_eq!(
            LifecycleRentCreditV2::decode(&state_bytes),
            Err(LifecycleRentErrorV2::NonCanonical)
        );
        let create = CreateLifecycleRentCreditV2::new(state()).to_bytes();
        let mut trailing = create.to_vec();
        trailing.push(0);
        assert_eq!(
            LifecycleRentInstructionV2::decode(&trailing),
            Err(LifecycleRentErrorV2::InvalidLength)
        );
    }

    #[test]
    fn sweep_preserves_rent_and_closes_only_to_wallet() {
        let sweep = SweepLifecycleRentCreditV2::new(20).expect("sweep");
        let plan = LifecycleSweepPlanV2::new(120, 7, 100, sweep).expect("plan");
        assert_eq!((plan.credit_after(), plan.wallet_after()), (100, 27));
        assert_eq!(
            LifecycleSweepPlanV2::new(
                120,
                7,
                100,
                SweepLifecycleRentCreditV2::new(21).expect("sweep")
            ),
            Err(LifecycleRentErrorV2::InvalidSweep)
        );
        assert_eq!(validate_refund_wallet_v2(SYSTEM_PROGRAM_ID, 0), Ok(()));
        assert_eq!(
            validate_refund_wallet_v2([9; 32], 0),
            Err(LifecycleRentErrorV2::InvalidFrame)
        );
    }

    #[test]
    fn a_crank_takes_a_capped_share_and_the_wallet_keeps_every_other_lamport() {
        let floor = 1_781_760;

        // The share binds: 1_600_000 / 16 == 100_000, well under the floor.
        let plan = LifecycleSweepPlanV2::new_with_crank(
            10_000_000,
            0,
            1_000_000,
            SweepLifecycleRentCreditV2::new(1_600_000).expect("sweep"),
            floor,
        )
        .expect("plan");
        assert_eq!(plan.crank_reward(), 100_000);
        assert_eq!(plan.credit_after(), 8_400_000);
        assert_eq!(plan.wallet_after(), 1_500_000);

        // The floor binds: 64_000_000 / 16 == 4_000_000, above the floor.
        let plan = LifecycleSweepPlanV2::new_with_crank(
            65_000_000,
            0,
            1_000_000,
            SweepLifecycleRentCreditV2::new(64_000_000).expect("sweep"),
            floor,
        )
        .expect("plan");
        assert_eq!(plan.crank_reward(), floor);
        assert_eq!(plan.wallet_after(), 64_000_000 - floor);

        // Every lamport leaving the credit arrives somewhere.
        assert_eq!(
            65_000_000 - plan.credit_after(),
            plan.wallet_after() + plan.crank_reward()
        );
    }

    #[test]
    fn a_thin_sweep_pays_a_smaller_crank_and_never_refuses_for_money() {
        // The property FUNDED_CRANK_V1.md section 2 makes mandatory: a crank
        // that could refuse for lack of funds is an unturned crank.
        for amount in [1_u64, 8, 15, 16, 17, 160] {
            let plan = LifecycleSweepPlanV2::new_with_crank(
                1_000_000 + amount,
                0,
                1_000_000,
                SweepLifecycleRentCreditV2::new(amount).expect("sweep"),
                1_781_760,
            )
            .expect("a thin sweep must plan, not refuse");
            assert_eq!(plan.crank_reward(), amount / 16);
            assert_eq!(plan.wallet_after(), amount - amount / 16);
        }
    }

    #[test]
    fn a_cranker_cannot_farm_the_surplus_away_from_the_refund_wallet() {
        // The exploit a plain min(floor, residual) cap would admit on a SURPLUS
        // route whose amount is caller-chosen: sweep exactly the floor, take
        // all of it, repeat, and the wallet receives nothing forever.
        let floor = 1_781_760;
        for amount in [1_u64, 1_000, floor, floor + 1, 40_000_000] {
            let plan = LifecycleSweepPlanV2::new_with_crank(
                80_000_000 + amount,
                0,
                1_000_000,
                SweepLifecycleRentCreditV2::new(amount).expect("sweep"),
                floor,
            )
            .expect("plan");
            let wallet_credit = plan.wallet_after();
            assert!(
                wallet_credit >= amount - amount / LIFECYCLE_SWEEP_CRANK_SHARE_DIVISOR_V2,
                "the wallet must keep at least 15/16 of {amount}, kept {wallet_credit}"
            );
            assert!(
                wallet_credit > plan.crank_reward() || amount < 2,
                "the crank must never out-earn the beneficiary on {amount}"
            );
        }
    }

    #[test]
    fn the_sweep_postcondition_refuses_every_observation_the_plan_did_not_predict() {
        let floor = 1_781_760;
        let plan = LifecycleSweepPlanV2::new_with_crank(
            10_000_000,
            7,
            1_000_000,
            SweepLifecycleRentCreditV2::new(1_600_000).expect("sweep"),
            floor,
        )
        .expect("plan");
        assert_eq!(plan.crank_reward(), 100_000);

        // The honest application.
        assert_eq!(
            plan.validate_post(8_400_000, 1_500_007, Some((0, 100_000))),
            Ok(())
        );

        // *** THE NEGATIVE CONTROL. *** Before this change the sweep credited
        // the wallet the FULL amount and there was no third recipient. That is
        // exactly the observation below, and the new postcondition must refuse
        // it -- otherwise the assertion proves nothing about the new code.
        assert_eq!(
            plan.validate_post(8_400_000, 1_600_007, None),
            Err(LifecycleRentErrorV2::SweepPostcondition)
        );

        // One account at a time, each pinned.
        assert_eq!(
            plan.validate_post(8_400_001, 1_500_007, Some((0, 100_000))),
            Err(LifecycleRentErrorV2::SweepPostcondition)
        );
        assert_eq!(
            plan.validate_post(8_400_000, 1_500_008, Some((0, 100_000))),
            Err(LifecycleRentErrorV2::SweepPostcondition)
        );
        // The crank was promised a reward and did not receive it.
        assert_eq!(
            plan.validate_post(8_400_000, 1_500_007, Some((0, 0))),
            Err(LifecycleRentErrorV2::SweepPostcondition)
        );
        // A frame that named no crank cannot carry a reward.
        assert_eq!(
            plan.validate_post(8_400_000, 1_500_007, None),
            Err(LifecycleRentErrorV2::SweepPostcondition)
        );

        // And the unpaid shape still validates against itself, so the
        // GREEN-SELF path is not collateral damage.
        let unpaid = LifecycleSweepPlanV2::new(
            10_000_000,
            7,
            1_000_000,
            SweepLifecycleRentCreditV2::new(1_600_000).expect("sweep"),
        )
        .expect("plan");
        assert_eq!(unpaid.crank_reward(), 0);
        assert_eq!(unpaid.validate_post(8_400_000, 1_600_007, None), Ok(()));
    }

    #[test]
    fn the_crank_validator_owns_each_recipient_refusal_and_ignores_signing() {
        // Pins WHICH check refuses, so the program tests above cannot pass by
        // refusing somewhere else for an unrelated reason.
        let meta = |key: [u8; PUBKEY_BYTES], signer: bool, writable: bool| LifecycleAccountMetaV2 {
            key,
            signer,
            writable,
            executable: false,
        };
        let credit = meta(id(1).to_bytes(), false, true);
        let wallet = meta(id(2).to_bytes(), false, true);
        let rent = meta(RENT_SYSVAR_ID, false, false);
        let good = meta(id(3).to_bytes(), false, true);

        assert_eq!(
            validate_sweep_crank_v2(good, SYSTEM_PROGRAM_ID, 0, credit, wallet, rent),
            Ok(())
        );
        // Signing is not an admission decision in either direction.
        assert_eq!(
            validate_sweep_crank_v2(
                meta(id(3).to_bytes(), true, true),
                SYSTEM_PROGRAM_ID,
                0,
                credit,
                wallet,
                rent
            ),
            Ok(())
        );
        for (label, crank, owner, data_len) in [
            (
                "not writable",
                meta(id(3).to_bytes(), false, false),
                SYSTEM_PROGRAM_ID,
                0,
            ),
            ("carries data", good, SYSTEM_PROGRAM_ID, 8),
            ("foreign owner", good, [7; 32], 0),
            ("aliases the credit", credit, SYSTEM_PROGRAM_ID, 0),
            ("aliases the wallet", wallet, SYSTEM_PROGRAM_ID, 0),
            ("aliases the Rent sysvar", rent, SYSTEM_PROGRAM_ID, 0),
        ] {
            assert_eq!(
                validate_sweep_crank_v2(crank, owner, data_len, credit, wallet, rent),
                Err(LifecycleRentErrorV2::InvalidFrame),
                "a crank that {label} must be refused here"
            );
        }
    }

    #[test]
    fn close_requires_current_core_and_exact_lifecycle() {
        let credit = id(20);
        let request = CloseLifecycleRentCreditV2::new(receipt(credit));
        let plan = LifecycleClosePlanV2::new(state(), credit, id(4), true, 99, 1, request)
            .expect("complete retirement");
        assert_eq!((plan.closed_lamports(), plan.wallet_after()), (99, 100));
        assert_eq!(plan.post_resource_digest(), [12; 32]);
        let rent_receipt = plan.receipt(state(), credit).expect("rent receipt");
        assert_eq!(
            LifecycleRentCloseReceiptV2::decode(&rent_receipt.to_bytes()),
            Ok(rent_receipt)
        );
        assert_eq!(rent_receipt.input().closed_lamports, 99);
        assert_eq!(
            LifecycleClosePlanV2::new(state(), credit, id(4), false, 99, 1, request),
            Err(LifecycleRentErrorV2::UnauthenticatedCore)
        );
        assert_eq!(
            LifecycleClosePlanV2::new(state(), id(21), id(4), true, 99, 1, request),
            Err(LifecycleRentErrorV2::RetirementMismatch)
        );
    }

    #[test]
    fn close_observes_the_exact_already_retired_market() {
        let expected = state().market();
        let observation = LifecycleRetiredMarketObservationV2 {
            market: expected,
            owner: SYSTEM_PROGRAM_ID,
            data_len: 0,
            lamports: 0,
            signer: false,
            writable: false,
            executable: false,
        };
        assert_eq!(observation.validate(expected), Ok(()));

        let hostile = [
            LifecycleRetiredMarketObservationV2 {
                market: id(21),
                ..observation
            },
            LifecycleRetiredMarketObservationV2 {
                owner: [9; 32],
                ..observation
            },
            LifecycleRetiredMarketObservationV2 {
                data_len: 1,
                ..observation
            },
            LifecycleRetiredMarketObservationV2 {
                lamports: 1,
                ..observation
            },
            LifecycleRetiredMarketObservationV2 {
                signer: true,
                ..observation
            },
            LifecycleRetiredMarketObservationV2 {
                writable: true,
                ..observation
            },
            LifecycleRetiredMarketObservationV2 {
                executable: true,
                ..observation
            },
        ];
        for candidate in hostile {
            assert_eq!(
                candidate.validate(expected),
                Err(LifecycleRentErrorV2::InvalidFrame)
            );
        }
    }
}
