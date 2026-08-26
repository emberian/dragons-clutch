#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact, SDK-free views of the SPL Token base-state and instruction ABIs used
//! by dClutch collateral adapters.
//!
//! This crate owns byte parsing, fixed instruction construction, and two
//! deliberately narrow exact-transfer profiles. It does not receive SVM
//! account metadata. A composing adapter must still prove account keys,
//! account owners, signer/writable/executable privileges, PDA authority,
//! rent, CPI success, and exact pre/post token and lamport deltas.
//!
//! Token-2022 state is accepted only at the exact legacy base widths. That is
//! the `Token2022ZeroExtensionExactTransferV1` profile: every account-type or
//! TLV extension representation is refused by length, not partially decoded.

#[cfg(test)]
extern crate std;

/// Fixed instruction specifications and exact borrowed instruction-data views.
pub mod instruction;
/// Exact-transfer program profiles and cross-account checks.
pub mod profile;
/// Canonical immutable collateral-adapter release preimages.
pub mod release;
/// Exact Mint and base Account state parsers.
pub mod state;

pub use instruction::{
    AccountMeta, CloseAccountInstruction, CloseAccountView, InitializeAccount3Instruction,
    InitializeAccount3View, InstructionDataView, InstructionSpec, RevokeInstruction, RevokeView,
    TransferCheckedInstruction, TransferCheckedView, close_account, initialize_account3, revoke,
    transfer_checked,
};
pub use profile::{AuthorityRole, ExactTransferFacts, ExactTransferInput, ExactTransferProfileV1};
pub use release::{
    ADAPTER_RELEASE_BYTES, ADAPTER_RELEASE_MAGIC, ADAPTER_RELEASE_SCHEMA_VERSION,
    CollateralAdapterReleaseV1, ExtensionStoragePolicy, PRODUCTION_ADAPTER_RELEASES, ProfileKind,
};
pub use state::{ACCOUNT_BYTES, AccountState, COption, MINT_BYTES, Mint, TokenAccount};

/// One raw SVM public-key or address value without an SDK dependency.
pub type Address = [u8; 32];

/// Base58 spelling of [`LEGACY_TOKEN_PROGRAM_ID`].
pub const LEGACY_TOKEN_PROGRAM_ID_BASE58: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// Exact legacy SPL Token program address bytes.
pub const LEGACY_TOKEN_PROGRAM_ID: Address = [
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
];

/// Base58 spelling of [`TOKEN_2022_PROGRAM_ID`].
pub const TOKEN_2022_PROGRAM_ID_BASE58: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
/// Exact SPL Token-2022 program address bytes.
pub const TOKEN_2022_PROGRAM_ID: Address = [
    6, 221, 246, 225, 238, 117, 143, 222, 24, 66, 93, 188, 228, 108, 205, 218, 182, 26, 252, 77,
    131, 185, 13, 39, 254, 189, 249, 40, 216, 161, 139, 252,
];

/// Checked token-program identity recognized by this crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenProgram {
    /// Legacy SPL Token.
    Legacy,
    /// SPL Token-2022.
    Token2022,
}

impl TokenProgram {
    /// Authenticate one raw program address.
    pub fn parse(program_id: Address) -> Result<Self> {
        if program_id == LEGACY_TOKEN_PROGRAM_ID {
            Ok(Self::Legacy)
        } else if program_id == TOKEN_2022_PROGRAM_ID {
            Ok(Self::Token2022)
        } else {
            Err(Error::UnsupportedProgram)
        }
    }

    /// Return the exact program address for this identity.
    pub const fn program_id(self) -> Address {
        match self {
            Self::Legacy => LEGACY_TOKEN_PROGRAM_ID,
            Self::Token2022 => TOKEN_2022_PROGRAM_ID,
        }
    }
}

/// Explicit refusal returned by the SDK-free token boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A state or instruction did not have its one exact width.
    InvalidLength,
    /// A four-byte `COption` tag was neither exact little-endian zero nor one.
    InvalidOptionTag,
    /// A packed Boolean was neither zero nor one.
    InvalidBoolean,
    /// A token Account state byte was outside the three defined states.
    InvalidAccountState,
    /// Instruction bytes did not name one supported exact instruction shape.
    InvalidInstruction,
    /// A token program address was neither legacy Token nor Token-2022.
    UnsupportedProgram,
    /// A program address did not match the selected immutable profile.
    ProfileProgramMismatch,
    /// A Mint used by an exact-transfer profile was uninitialized.
    MintUninitialized,
    /// A token Account used by an exact-transfer profile was uninitialized.
    AccountUninitialized,
    /// A token Account used by an exact-transfer profile was frozen.
    AccountFrozen,
    /// A source or destination Account named a different Mint.
    MintMismatch,
    /// The instruction decimals did not equal the authenticated Mint decimals.
    DecimalsMismatch,
    /// A native/wrapped-SOL Account was outside the token-only profile.
    NativeAccount,
    /// A custody account retained a delegate or delegated amount.
    DelegatePresent,
    /// A custody account retained a distinct close authority.
    CloseAuthorityPresent,
    /// The selected transfer authority was neither owner nor funded delegate.
    AuthorityMismatch,
    /// A selected delegate did not retain the requested transfer allowance.
    InsufficientDelegateAllowance,
    /// The source did not contain the requested exact amount.
    InsufficientFunds,
    /// The exact destination post-balance exceeded `u64`.
    ArithmeticOverflow,
    /// A collateral-adapter release preimage was not one exact production row.
    InvalidAdapterRelease,
}

/// Result alias for this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Pinned upstream-source and license evidence compiled into this crate.
pub const ABI_PROVENANCE: &str = include_str!("../PROVENANCE.md");

#[cfg(test)]
mod tests {
    use super::*;

    const BASE58_ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    fn decode_base58_address(text: &str) -> core::result::Result<Address, ()> {
        let mut output = [0u8; 32];
        for encoded in text.bytes() {
            let position = BASE58_ALPHABET
                .iter()
                .position(|candidate| *candidate == encoded)
                .ok_or(())?;
            let digit = u32::try_from(position).map_err(|_| ())?;
            let mut carry = digit;
            for byte in output.iter_mut().rev() {
                carry = carry
                    .checked_add(u32::from(*byte).checked_mul(58).ok_or(())?)
                    .ok_or(())?;
                *byte = u8::try_from(carry & 0xff).map_err(|_| ())?;
                carry >>= 8;
            }
            if carry != 0 {
                return Err(());
            }
        }
        Ok(output)
    }

    #[test]
    fn program_ids_are_distinct_and_exactly_profiled() {
        assert_ne!(LEGACY_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID);
        assert_eq!(
            decode_base58_address(LEGACY_TOKEN_PROGRAM_ID_BASE58),
            Ok(LEGACY_TOKEN_PROGRAM_ID)
        );
        assert_eq!(
            decode_base58_address(TOKEN_2022_PROGRAM_ID_BASE58),
            Ok(TOKEN_2022_PROGRAM_ID)
        );
        assert_eq!(
            TokenProgram::parse(LEGACY_TOKEN_PROGRAM_ID),
            Ok(TokenProgram::Legacy)
        );
        assert_eq!(
            TokenProgram::parse(TOKEN_2022_PROGRAM_ID),
            Ok(TokenProgram::Token2022)
        );
        assert_eq!(TokenProgram::parse([0; 32]), Err(Error::UnsupportedProgram));
        assert!(ABI_PROVENANCE.contains("7143c4e676984047"));
        assert!(ABI_PROVENANCE.contains("821d96d034ea31c4"));
    }
}
