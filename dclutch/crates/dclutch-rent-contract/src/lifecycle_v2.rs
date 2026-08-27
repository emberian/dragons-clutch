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

/// Exact width of a persistent lifecycle-scoped credit.
pub const LIFECYCLE_RENT_CREDIT_BYTES_V2: usize = 128;
/// Exact width of a V2 instruction header.
pub const LIFECYCLE_RENT_INSTRUCTION_HEADER_BYTES_V2: usize = 16;
/// Exact width of a Create request.
pub const CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2: usize = 128;
/// Exact width of a Sweep request.
pub const SWEEP_LIFECYCLE_RENT_CREDIT_BYTES_V2: usize = 24;
/// Exact width of a Close request carrying one canonical Core receipt.
pub const CLOSE_LIFECYCLE_RENT_CREDIT_BYTES_V2: usize = 528;
/// Exact width of the immediate Rent close receipt.
pub const LIFECYCLE_RENT_CLOSE_RECEIPT_BYTES_V2: usize = 192;

/// PDA domain for one lifecycle-scoped credit per Market generation.
pub const LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2: &[u8] = b"dclutch/rent-market/v2";
/// Current Core caller-authority domain for an atomic retirement close.
pub const LIFECYCLE_RENT_CORE_CLOSE_AUTHORITY_DOMAIN_V2: &[u8] = b"dclutch/rent-core-close/v2";
/// Canonical persistent-account magic.
pub const LIFECYCLE_RENT_CREDIT_MAGIC_V2: [u8; 8] = *b"DCLRNTL2";
/// Canonical V2 instruction magic.
pub const LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2: [u8; 8] = *b"DCLRNCI2";
/// Canonical immediate close-receipt magic.
pub const LIFECYCLE_RENT_CLOSE_RECEIPT_MAGIC_V2: [u8; 8] = *b"DCLRNCR2";
/// Implemented V2 schema version.
pub const LIFECYCLE_RENT_SCHEMA_VERSION_V2: u16 = 2;

const STATE_VERSION_OFFSET: usize = 8;
const STATE_BUMP_OFFSET: usize = 10;
const STATE_RESERVED_HEADER_OFFSET: usize = 11;
const STATE_REFUND_WALLET_OFFSET: usize = 16;
const STATE_MARKET_OFFSET: usize = 48;
const STATE_RELEASE_SET_OFFSET: usize = 80;
const STATE_GENERATION_OFFSET: usize = 112;
const STATE_RESERVED_BODY_OFFSET: usize = 120;

const INSTRUCTION_VERSION_OFFSET: usize = 8;
const INSTRUCTION_ACTION_OFFSET: usize = 10;
const INSTRUCTION_RESERVED_OFFSET: usize = 11;
const CREATE_REFUND_WALLET_OFFSET: usize = 16;
const CREATE_MARKET_OFFSET: usize = 48;
const CREATE_RELEASE_SET_OFFSET: usize = 80;
const CREATE_GENERATION_OFFSET: usize = 112;
const CREATE_BUMP_OFFSET: usize = 120;
const CREATE_RESERVED_OFFSET: usize = 121;
const SWEEP_AMOUNT_OFFSET: usize = 16;
const CLOSE_RECEIPT_OFFSET: usize = 16;
const RECEIPT_KIND_OFFSET: usize = 10;
const RECEIPT_RESERVED_HEADER_OFFSET: usize = 11;
const RECEIPT_CREDIT_OFFSET: usize = 16;
const RECEIPT_REFUND_WALLET_OFFSET: usize = 48;
const RECEIPT_MARKET_OFFSET: usize = 80;
const RECEIPT_RELEASE_SET_OFFSET: usize = 112;
const RECEIPT_POST_RESOURCE_DIGEST_OFFSET: usize = 144;
const RECEIPT_GENERATION_OFFSET: usize = 176;
const RECEIPT_CLOSED_LAMPORTS_OFFSET: usize = 184;

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
            STATE_VERSION_OFFSET,
        )?;
        require_zero(input, STATE_RESERVED_HEADER_OFFSET, 5)?;
        require_zero(input, STATE_RESERVED_BODY_OFFSET, 8)?;
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
        put(&mut output, 0, &LIFECYCLE_RENT_CREDIT_MAGIC_V2);
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
    Create = 1,
    /// Sweep surplus to the immutable wallet while preserving Rent.
    Sweep = 2,
    /// Close after complete producer-subtree retirement.
    Close = 3,
}

impl LifecycleRentActionV2 {
    fn decode(value: u8) -> LifecycleRentResultV2<Self> {
        match value {
            1 => Ok(Self::Create),
            2 => Ok(Self::Sweep),
            3 => Ok(Self::Close),
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
        require_zero(input, CREATE_RESERVED_OFFSET, 7)?;
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

/// Exact balance plan for a surplus sweep.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleSweepPlanV2 {
    credit_after: u64,
    wallet_after: u64,
}

impl LifecycleSweepPlanV2 {
    /// Plan a sweep that preserves the current Rent minimum.
    pub fn new(
        credit_before: u64,
        wallet_before: u64,
        rent_minimum: u64,
        request: SweepLifecycleRentCreditV2,
    ) -> LifecycleRentResultV2<Self> {
        let credit_after = credit_before
            .checked_sub(request.amount())
            .ok_or(LifecycleRentErrorV2::InvalidSweep)?;
        if credit_after < rent_minimum {
            return Err(LifecycleRentErrorV2::InvalidSweep);
        }
        let wallet_after = wallet_before
            .checked_add(request.amount())
            .ok_or(LifecycleRentErrorV2::Arithmetic)?;
        Ok(Self {
            credit_after,
            wallet_after,
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
            INSTRUCTION_VERSION_OFFSET,
        )?;
        if byte(input, RECEIPT_KIND_OFFSET)? != LifecycleRentActionV2::Close as u8 {
            return Err(LifecycleRentErrorV2::UnknownAction);
        }
        require_zero(input, RECEIPT_RESERVED_HEADER_OFFSET, 5)?;
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
        put(&mut output, 0, &LIFECYCLE_RENT_CLOSE_RECEIPT_MAGIC_V2);
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
        INSTRUCTION_VERSION_OFFSET,
    )?;
    if byte(input, INSTRUCTION_ACTION_OFFSET)? != action as u8 {
        return Err(LifecycleRentErrorV2::UnknownAction);
    }
    require_zero(input, INSTRUCTION_RESERVED_OFFSET, 5)
}

fn require_header(
    input: &[u8],
    magic: &[u8; 8],
    width: usize,
    version_offset: usize,
) -> LifecycleRentResultV2<()> {
    if input.len() != width || input.get(..8) != Some(magic.as_slice()) {
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
    put(output, 0, &LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2);
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
