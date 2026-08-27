#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact SDK-free semantics for native-rent credit.
//!
//! The live rent path is [`lifecycle_v2`]: the Market-generation-scoped
//! `LifecycleRentCreditV2` that tier 1 creates, sweeps, and closes. Its whole
//! grammar lives in that module.
//!
//! What remains at this root is the shared accounting primitives.
//!
//! The V1 Create and Withdraw INSTRUCTIONS were deleted on 2026-08-27 (the
//! answered supersession decision in tools/gauntlet/blocked.json; AGENTS.md
//! forbids preserving parallel legacy and current authority paths). With them
//! went the action/instruction grammar, both account frames, the role and alias
//! policy, `SystemWalletFactsV1`, and `WithdrawBalancePlanV1`.
//! [`CreateBalancePlanV1`] survives its name: the lifecycle V2 Create path uses
//! the same exact fund-at-current-Rent-minimum plan.
//!
//! The V1 RECORD went on 2026-08-27 too, once its last reader did. With no
//! Create route no `RentCreditV1` could come into existence, and the type
//! survived only because `dclutch-direct-codec` pinned its width at registered
//! artifact coordinates 7 and 10. That pin is now a 128-byte
//! `LifecycleRentCreditV2` and a Rent program coordinate; the two SVM-harness
//! Markets that planted a V1 record as their rent beneficiary never decoded its
//! bytes, and say so where they plant one. Nothing reads a V1 credit any more,
//! so `RentCreditV1`, `RentCreditPdaSeedsV1`, the 48-byte width, the PDA
//! domain, the magic, the schema version and every V1 field offset are gone.
//!
//! This crate owns byte canonicality and exact balance plans. It does not derive
//! PDAs, inspect account owners or data, deserialize Rent, invoke System,
//! transfer lamports, or close accounts.

/// Lifecycle-scoped successor state and Market-retirement closure semantics.
pub mod lifecycle_v2;

use core::convert::TryInto;

/// Exact width of a Solana-compatible public-key byte string.
pub const PUBKEY_BYTES: usize = 32;
/// Canonical System Program key bytes (`11111111111111111111111111111111`).
pub const SYSTEM_PROGRAM_ID: [u8; PUBKEY_BYTES] = [0; PUBKEY_BYTES];
/// Canonical Rent sysvar key bytes (`SysvarRent111111111111111111111111111111111`).
pub const RENT_SYSVAR_ID: [u8; PUBKEY_BYTES] = [
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155, 161,
    253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
];

/// Refusal from an exact accounting plan or an authority-bytes decode.
///
/// This enum is scoped to what this root module still does. It is NOT a wire
/// grammar: record decoding, instruction dispatch, account-frame checking and
/// close semantics all belong to [`lifecycle_v2`] and refuse under
/// [`lifecycle_v2::LifecycleRentErrorV2`]. Every variant here has a live
/// construction site in this file; keep it that way, because Rust does not
/// warn on an unused public variant and seven of them survived the 2026-08-27
/// V1 deletion invisibly (`InvalidMagic`, `UnsupportedSchema`,
/// `NonCanonicalReservedBytes`, `InvalidRentSysvar`, `ZeroWithdrawal`,
/// `WithdrawalExceedsClaimable`, `CloseNotSupported` — all deleted on review).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input did not have its one exact canonical width.
    InvalidLength,
    /// A required authority or ordinary account key was the all-zero sentinel.
    ZeroAuthorityOrAccount,
    /// Creation was not funded by exactly the current Rent minimum.
    CreationFundingMismatch,
    /// A source close did not prove its complete observed balance was credited.
    SourceCreditMismatch,
    /// Checked native-lamport arithmetic overflowed or underflowed.
    ArithmeticOverflow,
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

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

#[cfg(test)]
mod tests {
    use super::*;

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
