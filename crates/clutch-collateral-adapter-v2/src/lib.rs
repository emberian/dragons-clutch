// SPDX-License-Identifier: AGPL-3.0-or-later
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Versioned, allocation-free contract for Realm-selected collateral adapters.
//!
//! The crate owns the successor collateral boundary that V1 lacks:
//!
//! - a canonical release record binds parser/CPI code, the external token
//!   deployment, account layouts, and the exact-visible-atom theorem;
//! - a canonical policy selected by an immutable Realm binds that release,
//!   mint, token program, decimals, supply ceiling, and market-cap ceiling;
//! - hostile legacy SPL and conservative Token-2022 account bytes are parsed
//!   through one release-selected interface;
//! - deposit and withdrawal prepare fixed CPI intents and accept state writes
//!   only after exact source, destination, mint-supply, and Hoard checks; and
//! - claim issuance remains an independently identified Token-2022 plane.
//!
//! This is not an SBF dispatcher or a production release catalog. A live
//! adapter must supply a compile-time closed catalog, authenticate loader and
//! deployment identities, derive every PDA, perform the CPI, reload accounts,
//! and commit returned state only after the postcondition succeeds. No V1
//! decoder or route is weakened by this crate.

mod account;
mod binding;
mod bearer_redemption_v3;
mod claim;
mod claim_representation;
mod close;
mod codec;
mod hoard_surplus;
mod market_founding;
mod market_ledger;
mod policy;
mod position_v3;
mod reclassification;
mod redemption;
mod release;
mod resolution_v5;
mod series;
mod transfer;

pub use account::*;
pub use bearer_redemption_v3::*;
pub use binding::*;
pub use claim::*;
pub use claim_representation::*;
pub use close::*;
pub use hoard_surplus::*;
pub use market_founding::*;
pub use market_ledger::*;
pub use policy::*;
pub use position_v3::*;
pub use reclassification::*;
pub use redemption::*;
pub use release::*;
pub use resolution_v5::*;
pub use series::*;
pub use transfer::*;

use sha2::{Digest, Sha256};

/// Width of every canonical content, program, mint, account, or deployment id.
pub const ID_BYTES: usize = 32;

/// Full-width identity. Construction does not itself authenticate the bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Id([u8; ID_BYTES]);

impl Id {
    /// Canonical zero value, reserved for inactive padding except where a type
    /// explicitly denotes Solana's all-zero System Program address.
    pub const ZERO: Self = Self([0; ID_BYTES]);

    /// Wrap exact bytes without claiming that an external account was checked.
    pub const fn from_bytes(bytes: [u8; ID_BYTES]) -> Self {
        Self(bytes)
    }

    /// Return the exact bytes.
    pub const fn bytes(self) -> [u8; ID_BYTES] {
        self.0
    }

    /// Whether this is the reserved zero identity.
    pub fn is_zero(self) -> bool {
        self.0 == [0; ID_BYTES]
    }

    pub(crate) fn require_live(self) -> Result<()> {
        if self.is_zero() {
            Err(Error::ZeroIdentity)
        } else {
            Ok(())
        }
    }
}

/// Deterministic refusal from the V2 collateral contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Error {
    /// Input or output was shorter than its exact layout.
    Truncated,
    /// Input or output was longer than its exact layout.
    TrailingBytes,
    /// A canonical body magic did not match.
    BadMagic,
    /// A schema or semantic version was unknown.
    BadVersion,
    /// Reserved bytes or inactive fixed-width fields were nonzero.
    NonCanonicalPadding,
    /// A required full-width identity was zero.
    ZeroIdentity,
    /// An enum, flag, length, cap, or quantity was outside its admitted domain.
    InvalidParameter,
    /// Checked arithmetic overflowed or underflowed.
    Arithmetic,
    /// Market, Realm, Profile, policy, release, or deployment identities differ.
    MismatchedBinding,
    /// The Realm-selected release does not exist in the compiled closed catalog.
    UnknownAdapterRelease,
    /// Two catalog entries have the same content identity.
    DuplicateAdapterRelease,
    /// The selected external token program is not the release's program.
    WrongProgram,
    /// The presented mint is not the Realm-selected collateral mint.
    WrongMint,
    /// A runtime account role, mutability, signer, or executable bit was wrong.
    WrongAccountRole,
    /// A mint or token account was not initialized.
    Uninitialized,
    /// Mint or checked-transfer decimals differ from the immutable policy.
    WrongDecimals,
    /// Current mint supply is zero or exceeds the immutable supply ceiling.
    SupplyNotAdmitted,
    /// A mint or freeze authority remains on collateral.
    MintAuthorityNotAdmitted,
    /// Account bytes or an option/TLV field were malformed.
    MalformedTokenState,
    /// An extension is unknown, misplaced, disallowed, or required but absent.
    ExtensionNotAdmitted,
    /// A token account is frozen or has native/wrapped-native balance semantics.
    TokenAccountNotTransferable,
    /// A custody account has a delegate or close authority.
    CustodyAuthorityNotAdmitted,
    /// The release's exact owner guard was not established for custody.
    OwnerGuardUnavailable,
    /// Position cash is reserved or otherwise insufficient for withdrawal.
    InsufficientUnreservedCash,
    /// Locking collateral would exceed the immutable per-market cap.
    MarketCapExceeded,
    /// The Hoard's visible token balance does not cover locked principal.
    HoardCoverageMismatch,
    /// CPI source, destination, or mint-supply deltas were not exact.
    TransferDeltaMismatch,
    /// Post-CPI account bytes no longer satisfy the pre-CPI release and policy.
    PostAdmissionFailed,
    /// Collateral and claim adapter identities were incorrectly collapsed.
    CollateralClaimPlaneAliased,
    /// A custody token account retained collateral atoms and cannot be closed.
    CustodyNotEmpty,
    /// A close-account lamport movement or terminal state was not exact.
    CloseDeltaMismatch,
    /// Stored refundable token-vault rent principal was not fully covered.
    RentPrincipalNotCovered,
    /// A Series funding or one-shot terminal receipt join was inconsistent.
    SeriesJoinMismatch,
    /// An aggregate Market liability compartment could not cover a debit.
    AggregateLiabilityInsufficient,
    /// A scaled native-claim payout was not an exact whole collateral atom.
    PayoutRemainder,
}

/// Result alias for the V2 collateral contract.
pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn digest(domain: &[u8], parts: &[&[u8]]) -> Id {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(part);
    }
    Id::from_bytes(hasher.finalize().into())
}
