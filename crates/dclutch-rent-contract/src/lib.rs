#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact SDK-free semantics for native-rent credit.
//!
//! The live rent path is [`lifecycle_v2`]: the Market-generation-scoped
//! `LifecycleRentCreditV2` that tier 1 creates, sweeps, and closes. Its whole
//! grammar lives in that module.
//!
//! What remains at this root is the V1 record and its accounting primitives.
//! `RentCreditV1` is a program-owned, non-closeable 48-byte account whose
//! immutable field is a source's `rent_refund` refund/beneficiary authority,
//! never a direct payout account.
//!
//! The V1 Create and Withdraw INSTRUCTIONS were deleted on 2026-08-27 (the
//! answered supersession decision in tools/gauntlet/blocked.json; AGENTS.md
//! forbids preserving parallel legacy and current authority paths). With them
//! went the action/instruction grammar, both account frames, the role and alias
//! policy, `SystemWalletFactsV1`, and `WithdrawBalancePlanV1`.
//! [`CreateBalancePlanV1`] survives its name: the lifecycle V2 Create path uses
//! the same exact fund-at-current-Rent-minimum plan.
//!
//! Consequence, stated rather than hidden: with no Create route, no new
//! `RentCreditV1` account can come into existence. The type, its width, and its
//! PDA domain are kept because live code still reads them — most consequentially
//! `dclutch-direct-codec`, which pins `RENT_CREDIT_BYTES_V1` at registered
//! artifact coordinates 7 and 10, where the RentCredit V1/V2 width skew is a
//! known emitter defect owned by DP2. That migration retires the last of V1;
//! this crate does not front-run it under a live emitter lane.
//!
//! This crate owns byte canonicality and exact balance plans. It does not derive
//! PDAs, inspect account owners or data, deserialize Rent, invoke System,
//! transfer lamports, or close accounts.

/// Lifecycle-scoped successor state and Market-retirement closure semantics.
pub mod lifecycle_v2;

use core::convert::TryInto;

/// Exact width of a Solana-compatible public-key byte string.
pub const PUBKEY_BYTES: usize = 32;
/// Exact width of a persistent V1 rent-credit record.
pub const RENT_CREDIT_BYTES_V1: usize = 48;

/// PDA domain for one permanent rent credit per refund authority.
///
/// This is 22 bytes, within Solana's 32-byte individual PDA-seed limit.
pub const RENT_CREDIT_PDA_DOMAIN_V1: &[u8] = b"dclutch/rent-credit/v1";
/// Exact byte count of [`RENT_CREDIT_PDA_DOMAIN_V1`].
pub const RENT_CREDIT_PDA_DOMAIN_BYTES_V1: usize = 22;

/// Canonical persistent-account magic.
pub const RENT_CREDIT_MAGIC_V1: [u8; 8] = *b"DCLTRNT1";
/// Implemented persistent-account and instruction schema version.
pub const RENT_CREDIT_SCHEMA_VERSION_V1: u16 = 1;

/// Offset of the persisted schema version.
pub const RENT_CREDIT_SCHEMA_OFFSET_V1: usize = 8;
/// Offset of the persisted PDA bump.
pub const RENT_CREDIT_PDA_BUMP_OFFSET_V1: usize = 10;
/// Offset of five canonical zero bytes in a persistent credit.
pub const RENT_CREDIT_RESERVED_OFFSET_V1: usize = 11;
/// Offset of the immutable refund/beneficiary authority.
pub const RENT_CREDIT_REFUND_AUTHORITY_OFFSET_V1: usize = 16;

/// Canonical System Program key bytes (`11111111111111111111111111111111`).
pub const SYSTEM_PROGRAM_ID: [u8; PUBKEY_BYTES] = [0; PUBKEY_BYTES];
/// Canonical Rent sysvar key bytes (`SysvarRent111111111111111111111111111111111`).
pub const RENT_SYSVAR_ID: [u8; PUBKEY_BYTES] = [
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155, 161,
    253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
];

/// Refusal from a hostile decoder, frame checker, or exact accounting plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input did not have its one exact canonical width.
    InvalidLength,
    /// Magic bytes did not identify this contract.
    InvalidMagic,
    /// The encoded schema version is not implemented.
    UnsupportedSchema,
    /// An instruction action discriminator is not defined in V1.
    UnknownAction,
    /// Reserved bytes or reserved trailing bytes were not zero.
    NonCanonicalReservedBytes,
    /// A required authority or ordinary account key was the all-zero sentinel.
    ZeroAuthorityOrAccount,
    /// An account did not have the exact role privileges required by V1.
    InvalidAccountPrivilege,
    /// A supplied System Program was not the canonical executable System Program.
    InvalidSystemProgram,
    /// A supplied Rent account was not the canonical nonexecutable Rent sysvar.
    InvalidRentSysvar,
    /// Authenticated wallet facts were not a data-empty System wallet.
    InvalidSystemWallet,
    /// Roles that must be distinct used the same account key.
    AccountAlias,
    /// A record did not bind the supplied authority and bump.
    CreditBindingMismatch,
    /// Creation was not funded by exactly the current Rent minimum.
    CreationFundingMismatch,
    /// A required nonzero requested withdrawal amount was zero.
    ZeroWithdrawal,
    /// The requested withdrawal exceeded the current claimable balance.
    WithdrawalExceedsClaimable,
    /// A source close did not prove its complete observed balance was credited.
    SourceCreditMismatch,
    /// Checked native-lamport arithmetic overflowed or underflowed.
    ArithmeticOverflow,
    /// V1 has no close path for a rent-credit account.
    CloseNotSupported,
}

/// Result alias for rent-credit operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Validated nonzero refund/beneficiary authority bytes.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct RefundAuthority([u8; PUBKEY_BYTES]);

impl RefundAuthority {
    /// Construct one nonzero immutable refund/beneficiary authority.
    pub fn new(bytes: [u8; PUBKEY_BYTES]) -> Result<Self> {
        if is_zero(&bytes) {
            return Err(Error::ZeroAuthorityOrAccount);
        }
        Ok(Self(bytes))
    }

    /// Hostile-decode one exact nonzero authority.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Self::new(read_array(bytes, 0)?)
    }

    /// Return the exact authority bytes.
    pub const fn to_bytes(self) -> [u8; PUBKEY_BYTES] {
        self.0
    }
}

/// Immutable, permanent program-owned native-rent credit state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentCreditV1 {
    refund_authority: RefundAuthority,
    pda_bump: u8,
}

impl RentCreditV1 {
    /// Construct canonical semantic credit state before encoding it.
    pub const fn new(refund_authority: RefundAuthority, pda_bump: u8) -> Self {
        Self {
            refund_authority,
            pda_bump,
        }
    }

    /// Hostile-decode exactly one canonical 48-byte V1 credit account.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != RENT_CREDIT_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if read_array(bytes, 0)? != RENT_CREDIT_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, RENT_CREDIT_SCHEMA_OFFSET_V1)? != RENT_CREDIT_SCHEMA_VERSION_V1 {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, RENT_CREDIT_RESERVED_OFFSET_V1, 5)?;
        Ok(Self::new(
            RefundAuthority::new(read_array(bytes, RENT_CREDIT_REFUND_AUTHORITY_OFFSET_V1)?)?,
            read_byte(bytes, RENT_CREDIT_PDA_BUMP_OFFSET_V1)?,
        ))
    }

    /// Return the exact canonical 48-byte persistent representation.
    pub fn to_bytes(self) -> [u8; RENT_CREDIT_BYTES_V1] {
        let mut output = [0; RENT_CREDIT_BYTES_V1];
        put(&mut output, 0, &RENT_CREDIT_MAGIC_V1);
        put(
            &mut output,
            RENT_CREDIT_SCHEMA_OFFSET_V1,
            &RENT_CREDIT_SCHEMA_VERSION_V1.to_le_bytes(),
        );
        output[RENT_CREDIT_PDA_BUMP_OFFSET_V1] = self.pda_bump;
        put(
            &mut output,
            RENT_CREDIT_REFUND_AUTHORITY_OFFSET_V1,
            &self.refund_authority.to_bytes(),
        );
        output
    }

    /// Return the immutable refund/beneficiary authority.
    pub const fn refund_authority(self) -> RefundAuthority {
        self.refund_authority
    }
    /// Return the persisted PDA bump that the adapter must verify by derivation.
    pub const fn pda_bump(self) -> u8 {
        self.pda_bump
    }
    /// Return the exact PDA seed projection for an SDK-owning adapter.
    pub const fn pda_seeds(self) -> RentCreditPdaSeedsV1 {
        RentCreditPdaSeedsV1 {
            domain: RENT_CREDIT_PDA_DOMAIN_V1,
            refund_authority: self.refund_authority,
            bump: self.pda_bump,
        }
    }
    /// Verify immutable state against a separately derived Create binding.
    pub fn validate_binding(self, authority: RefundAuthority, bump: u8) -> Result<()> {
        if self.refund_authority != authority || self.pda_bump != bump {
            return Err(Error::CreditBindingMismatch);
        }
        Ok(())
    }
}

/// Exact PDA seed projection; actual PDA derivation remains in the SVM adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RentCreditPdaSeedsV1 {
    domain: &'static [u8],
    refund_authority: RefundAuthority,
    bump: u8,
}

impl RentCreditPdaSeedsV1 {
    /// Return the fixed PDA domain.
    pub const fn domain(self) -> &'static [u8] {
        self.domain
    }
    /// Return the immutable authority seed.
    pub const fn refund_authority(self) -> RefundAuthority {
        self.refund_authority
    }
    /// Return the persisted bump seed.
    pub const fn bump(self) -> u8 {
        self.bump
    }
}

/// Exact Create balance transition funded by the payer at current Rent minimum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateBalancePlanV1 {
    payer_before: u64,
    payer_after: u64,
    credit_before: u64,
    credit_after: u64,
    current_rent_minimum: u64,
}

impl CreateBalancePlanV1 {
    /// Build the sole admitted Create transition.
    ///
    /// A vacant PDA begins at zero observed lamports and receives exactly
    /// current Rent minimum. A zero minimum is admitted if canonical Rent
    /// reports it; the exactness rule remains unchanged.
    pub fn new(payer_before: u64, credit_before: u64, current_rent_minimum: u64) -> Result<Self> {
        if credit_before != 0 {
            return Err(Error::CreationFundingMismatch);
        }
        let payer_after = payer_before
            .checked_sub(current_rent_minimum)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(Self {
            payer_before,
            payer_after,
            credit_before,
            credit_after: current_rent_minimum,
            current_rent_minimum,
        })
    }

    /// Verify actual post-observation against this exact Create plan.
    pub fn validate_post(self, payer_after: u64, credit_after: u64) -> Result<()> {
        if payer_after != self.payer_after || credit_after != self.credit_after {
            return Err(Error::CreationFundingMismatch);
        }
        Ok(())
    }
    /// Return payer lamports before creation.
    pub const fn payer_before(self) -> u64 {
        self.payer_before
    }
    /// Return payer lamports after creation.
    pub const fn payer_after(self) -> u64 {
        self.payer_after
    }
    /// Return credit lamports before creation.
    pub const fn credit_before(self) -> u64 {
        self.credit_before
    }
    /// Return credit lamports after creation.
    pub const fn credit_after(self) -> u64 {
        self.credit_after
    }
    /// Return exact Rent minimum used by this plan.
    pub const fn current_rent_minimum(self) -> u64 {
        self.current_rent_minimum
    }
}

/// Exact source-close transfer plan into the authority's permanent credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCloseCreditPlanV1 {
    source_before: u64,
    source_after: u64,
    credit_before: u64,
    credit_after: u64,
    credited_lamports: u64,
}

/// Exact generic nonnegative balance delta into a rent-credit account.
///
/// This is the narrow accounting primitive for a source whose full balance is
/// not transferred to credit, such as a Fund split payout or a terminal-account
/// shrink. It validates only the credit account's before/after delta. The
/// composing adapter remains responsible for conservation and disposition of
/// every non-credit amount. [`SourceCloseCreditPlanV1`] is the stronger wrapper
/// for a source that must close completely into credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreditBalancePlanV1 {
    credit_before: u64,
    credit_after: u64,
    credited_lamports: u64,
}

impl CreditBalancePlanV1 {
    /// Build one exact checked nonnegative credit balance delta.
    pub fn new(credit_before: u64, credited_lamports: u64) -> Result<Self> {
        let credit_after = credit_before
            .checked_add(credited_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(Self {
            credit_before,
            credit_after,
            credited_lamports,
        })
    }

    /// Verify the observed credit post-balance against the exact planned delta.
    pub fn validate_post(self, credit_after: u64) -> Result<()> {
        if credit_after != self.credit_after {
            return Err(Error::SourceCreditMismatch);
        }
        Ok(())
    }

    /// Return credit lamports before this delta.
    pub const fn credit_before(self) -> u64 {
        self.credit_before
    }

    /// Return credit lamports after this delta.
    pub const fn credit_after(self) -> u64 {
        self.credit_after
    }

    /// Return the exact delta credited to the permanent account.
    pub const fn credited_lamports(self) -> u64 {
        self.credited_lamports
    }
}

impl SourceCloseCreditPlanV1 {
    /// Build a close plan only when the complete observed source balance is credited.
    ///
    /// No rent-floor check occurs: credit remains admitted even if a Rent
    /// increase makes it temporarily underfunded. The adapter proves each
    /// source's close and binds legacy `rent_refund` authority to this PDA.
    pub fn new(source_before: u64, credit_before: u64, credited_lamports: u64) -> Result<Self> {
        if source_before != credited_lamports {
            return Err(Error::SourceCreditMismatch);
        }
        let credit = CreditBalancePlanV1::new(credit_before, credited_lamports)?;
        Ok(Self {
            source_before,
            source_after: 0,
            credit_before,
            credit_after: credit.credit_after(),
            credited_lamports,
        })
    }
    /// Verify actual post-observations against this exact source-close plan.
    pub fn validate_post(self, source_after: u64, credit_after: u64) -> Result<()> {
        if source_after != self.source_after || credit_after != self.credit_after {
            return Err(Error::SourceCreditMismatch);
        }
        Ok(())
    }
    /// Return source lamports before closure.
    pub const fn source_before(self) -> u64 {
        self.source_before
    }
    /// Return source lamports after closure, always zero.
    pub const fn source_after(self) -> u64 {
        self.source_after
    }
    /// Return credit lamports before source-close transfer.
    pub const fn credit_before(self) -> u64 {
        self.credit_before
    }
    /// Return credit lamports after source-close transfer.
    pub const fn credit_after(self) -> u64 {
        self.credit_after
    }
    /// Return source's proved exact credited amount.
    pub const fn credited_lamports(self) -> u64 {
        self.credited_lamports
    }
}

/// Return current claimable surplus, including unsolicited donations honestly.
///
/// This is `observed_lamports.saturating_sub(current_rent_minimum)`. It has no
/// provenance filter: donations are claimable surplus, while source credits are
/// separately exact under [`SourceCloseCreditPlanV1`].
pub const fn claimable_lamports(observed_lamports: u64, current_rent_minimum: u64) -> u64 {
    observed_lamports.saturating_sub(current_rent_minimum)
}

fn is_zero(bytes: &[u8; PUBKEY_BYTES]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

fn require_zero(bytes: &[u8], offset: usize, length: usize) -> Result<()> {
    let end = offset.checked_add(length).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}
fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(destination) = output.get_mut(offset..offset.saturating_add(value.len())) {
        destination.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(value: u8) -> [u8; 32] {
        [value; 32]
    }
    fn authority(value: u8) -> RefundAuthority {
        RefundAuthority::new(key(value)).expect("authority")
    }
    #[test]
    fn canonical_roundtrip_bump_and_binding() {
        let record = RentCreditV1::new(authority(7), 254);
        let bytes = record.to_bytes();
        assert_eq!(bytes.len(), 48);
        assert_eq!(bytes[10], 254);
        assert_eq!(RentCreditV1::decode(&bytes), Ok(record));
        assert_eq!(record.pda_seeds().domain(), RENT_CREDIT_PDA_DOMAIN_V1);
        assert_eq!(record.validate_binding(authority(7), 254), Ok(()));
        assert_eq!(
            record.validate_binding(authority(8), 254),
            Err(Error::CreditBindingMismatch)
        );
    }

    #[test]
    fn decode_refuses_reserved_trailing_and_zero_authority() {
        let record = RentCreditV1::new(authority(3), 1);
        let mut dirty = record.to_bytes();
        dirty[11] = 1;
        assert_eq!(
            RentCreditV1::decode(&dirty),
            Err(Error::NonCanonicalReservedBytes)
        );
        let mut zero = record.to_bytes();
        zero[16..48].fill(0);
        assert_eq!(
            RentCreditV1::decode(&zero),
            Err(Error::ZeroAuthorityOrAccount)
        );
    }

    #[test]
    // The V1 Create route is deleted; this plan is not. Lifecycle V2's Create
    // funds a credit by the same exact rule, so its coverage stays here.
    #[test]
    fn creation_funds_exactly_the_current_rent_minimum() {
        let plan = CreateBalancePlanV1::new(120, 0, 100).expect("plan");
        assert_eq!((plan.payer_after(), plan.credit_after()), (20, 100));
        assert_eq!(plan.validate_post(20, 100), Ok(()));
        assert_eq!(
            plan.validate_post(20, 99),
            Err(Error::CreationFundingMismatch)
        );
        assert_eq!(
            CreateBalancePlanV1::new(120, 1, 100),
            Err(Error::CreationFundingMismatch)
        );
    }

    #[test]
    fn under_rent_credit_liveness_and_donation_claimability() {
        let record = RentCreditV1::new(authority(4), 2);
        assert_eq!(RentCreditV1::decode(&record.to_bytes()), Ok(record));
        assert_eq!(claimable_lamports(90, 100), 0);
        let source = SourceCloseCreditPlanV1::new(0, 90, 0).expect("under-rent live");
        assert_eq!(source.credit_after(), 90);
        let donation = SourceCloseCreditPlanV1::new(30, 90, 30).expect("credit");
        assert_eq!(donation.credit_after(), 120);
        assert_eq!(claimable_lamports(120, 100), 20);
        // An unsolicited direct donation has no source-close receipt but is
        // still honestly included in the observed surplus.
        assert_eq!(claimable_lamports(130, 100), 30);
    }

    #[test]
    fn source_close_proves_exact_credit_and_checks_overflow() {
        assert_eq!(
            SourceCloseCreditPlanV1::new(9, 10, 8),
            Err(Error::SourceCreditMismatch)
        );
        assert_eq!(
            SourceCloseCreditPlanV1::new(1, u64::MAX, 1),
            Err(Error::ArithmeticOverflow)
        );
        let plan = SourceCloseCreditPlanV1::new(9, 10, 9).expect("exact");
        assert_eq!(plan.validate_post(0, 19), Ok(()));
        assert_eq!(plan.validate_post(1, 19), Err(Error::SourceCreditMismatch));
    }

    #[test]
    fn generic_credit_delta_allows_split_remainders_and_checks_overflow() {
        let plan = CreditBalancePlanV1::new(10, 7).expect("split remainder credit");
        assert_eq!(plan.credit_after(), 17);
        assert_eq!(plan.validate_post(17), Ok(()));
        assert_eq!(plan.validate_post(16), Err(Error::SourceCreditMismatch));
        assert_eq!(
            CreditBalancePlanV1::new(u64::MAX, 1),
            Err(Error::ArithmeticOverflow)
        );
    }
}
