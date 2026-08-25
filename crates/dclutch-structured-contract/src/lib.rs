#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Optional exact structured-portfolio capability for dClutch.
//!
//! Product remains the sole owner of a rational portfolio recipe. This crate
//! authenticates one canonical
//! [`dclutch_product_contract::portfolio::PortfolioTemplateV1`] and treats its
//! positive denominator as the minimum Product scale represented by one
//! structured unit. The unit's physical backing is the template's normalized
//! nonnegative integer coefficient vector. No fractional claim, collateral
//! remainder, or second payout rule is introduced.
//!
//! [`descriptor::StructuredDescriptorV1`] is immutable. Its template content
//! identity transitively binds the Product claim basis, result domain,
//! denominator, and coefficient bytes without persisting a second recipe.
//! [`transition`] moves actual native claims between an owner's canonical
//! Position and descriptor-owned Position custody. The authenticated
//! Token-2022 receipt Mint is the sole structured-unit supply owner; this crate
//! persists no parallel total or holder ledger. Terminal redemption invokes
//! the existing categorical Market redemption transition for every nonzero
//! backed outcome, including losers.
//!
//! This crate contains no Solana SDK, hashing, PDA derivation, Mint creation,
//! CPI, account memory, or allocation. It reuses the exact hostile Token-2022
//! receipt projections owned by `dclutch-bearer-contract` and the pinned
//! program identity in `dclutch-token-svm`. The required next SBF vertical must
//! parse those observations, authenticate all derivations, and atomically
//! realize the returned MintTo/PermissionedBurn plans.

use core::convert::TryInto;

/// Immutable config, descriptor, Product binding, and exact backing recipe.
pub mod descriptor;
/// Exact-width future-adapter instruction codec.
pub mod instruction;
/// Allocation-free native-custody and terminal transition plans.
pub mod transition;

/// Exact byte width of one opaque identity.
pub const ID_BYTES: usize = 32;

/// Explicit refusal returned by the structured capability contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// An input did not have its one exact canonical width.
    InvalidLength,
    /// A caller-provided output did not have its one exact canonical width.
    OutputLength,
    /// Magic bytes did not identify the requested record.
    InvalidMagic,
    /// A schema or fixed-layout profile is not implemented.
    UnsupportedSchema,
    /// Reserved bytes were nonzero.
    NonCanonicalReservedBytes,
    /// The exact categorical width was outside the selected Product profile.
    InvalidOutcomeCount,
    /// A required content identity, address, or beneficiary was all zero.
    ZeroIdentifier,
    /// A transition quantity which must move units was zero.
    ZeroQuantity,
    /// Checked exact integer arithmetic overflowed or underflowed.
    ArithmeticOverflow,
    /// An instruction action byte was not defined.
    UnknownAction,
    /// A quantity-bearing instruction used the canonical zero refusal value.
    ZeroInstructionQuantity,
    /// A retirement replay guard claimed no live child to retire.
    InvalidChildCount,
    /// The supplied Market account was not the descriptor's immutable Market.
    MarketMismatch,
    /// The immutable Market generation did not match.
    GenerationMismatch,
    /// The Product instance content identity did not match the Market.
    ProductInstanceMismatch,
    /// Product, Market, and template did not name one claim basis.
    ClaimBasisMismatch,
    /// Product instance and template did not name one result domain.
    ResultDomainMismatch,
    /// The exact authenticated PortfolioTemplate content identity differed.
    PortfolioTemplateMismatch,
    /// A supposedly canonical template did not yield its denominator as its least integral lot.
    NonCanonicalRealizationLot,
    /// The selected immutable capability config differed.
    CapabilityConfigMismatch,
    /// The selected semantic release differed from this contract release.
    CapabilityReleaseMismatch,
    /// The manifest entry did not select this exact capability coordinate.
    CapabilitySelectionMismatch,
    /// The permanent RentCredit beneficiary differed from the bound config.
    RentCreditMismatch,
    /// The transition is not admitted in the current Market phase.
    InvalidMarketPhase,
    /// A Position did not name the required owner.
    PositionOwnerMismatch,
    /// Owner and descriptor custody were the same semantic Position.
    PositionAliasing,
    /// The observed receipt Mint differed from the immutable descriptor.
    ReceiptMintMismatch,
    /// The observed receipt-Mint controller differed from the descriptor.
    ReceiptAuthorityMismatch,
    /// The supplied custody Position account differed from the descriptor.
    CustodyPositionMismatch,
    /// A receipt Mint, holder account, custody account, or owner illegally aliased.
    AccountAlias,
    /// Descriptor custody was not exactly coefficient times unit supply.
    BackingMismatch,
    /// Market redemption returned a payout other than the canonical winner amount.
    RedemptionPayoutMismatch,
    /// The visible Position subset exceeded the Market's aggregate supply.
    MarketSupplyMismatch,
    /// Retirement observed live structured units or native custody.
    OutstandingStructuredBacking,
    /// Product-owned template or instance semantics refused the input.
    ProductContract {
        /// Exact Product-contract refusal.
        error: dclutch_product_contract::Error,
    },
    /// Provider-neutral Market semantics refused the transition.
    MarketContract {
        /// Exact Market-contract refusal.
        error: dclutch_market_contract::Error,
    },
    /// Native Position semantics refused the transition.
    RealmContract {
        /// Exact Realm-contract refusal.
        error: dclutch_realm_contract::Error,
    },
    /// Shared Token-2022 receipt-profile semantics refused an observation.
    BearerContract {
        /// Exact bearer/profile-contract refusal.
        error: dclutch_bearer_contract::Error,
    },
}

/// Result alias for the structured capability contract.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn require_nonzero(value: &[u8; ID_BYTES]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentifier)
    } else {
        Ok(())
    }
}

pub(crate) const fn require_quantity(quantity: u64) -> Result<()> {
    if quantity == 0 {
        Err(Error::ZeroQuantity)
    } else {
        Ok(())
    }
}

pub(crate) fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

pub(crate) fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

pub(crate) fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(Error::NonCanonicalReservedBytes)
    } else {
        Ok(())
    }
}

pub(crate) fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    if let Some(destination) = output.get_mut(offset..offset.saturating_add(input.len())) {
        destination.copy_from_slice(input);
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;
