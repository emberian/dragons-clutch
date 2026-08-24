//! Exact fixed instruction construction without Solana SDK or allocation.

use core::convert::TryInto;

use crate::{Address, Error, Result, TokenProgram};

/// Official `CloseAccount` instruction tag.
pub const CLOSE_ACCOUNT_TAG: u8 = 9;
/// Official `TransferChecked` instruction tag.
pub const TRANSFER_CHECKED_TAG: u8 = 12;
/// Official `InitializeAccount3` instruction tag.
pub const INITIALIZE_ACCOUNT3_TAG: u8 = 18;

/// Exact `InitializeAccount3` instruction-data width.
pub const INITIALIZE_ACCOUNT3_DATA_BYTES: usize = 33;
/// Exact `TransferChecked` instruction-data width.
pub const TRANSFER_CHECKED_DATA_BYTES: usize = 10;
/// Exact `CloseAccount` instruction-data width.
pub const CLOSE_ACCOUNT_DATA_BYTES: usize = 1;

/// One SDK-free instruction account role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountMeta {
    address: Address,
    is_signer: bool,
    is_writable: bool,
}

impl AccountMeta {
    const fn new(address: Address, is_signer: bool, is_writable: bool) -> Self {
        Self {
            address,
            is_signer,
            is_writable,
        }
    }

    /// Return the exact account address.
    pub const fn address(&self) -> &Address {
        &self.address
    }

    /// Return whether the role must be a transaction or CPI signer.
    pub const fn is_signer(&self) -> bool {
        self.is_signer
    }

    /// Return whether the role must be writable.
    pub const fn is_writable(&self) -> bool {
        self.is_writable
    }
}

/// One fixed-account, fixed-data instruction ready for an SDK adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstructionSpec<const ACCOUNTS: usize, const DATA: usize> {
    program_id: Address,
    accounts: [AccountMeta; ACCOUNTS],
    data: [u8; DATA],
}

impl<const ACCOUNTS: usize, const DATA: usize> InstructionSpec<ACCOUNTS, DATA> {
    /// Return the authenticated legacy or Token-2022 program address.
    pub const fn program_id(&self) -> &Address {
        &self.program_id
    }

    /// Borrow the exact ordered account roles.
    pub const fn accounts(&self) -> &[AccountMeta; ACCOUNTS] {
        &self.accounts
    }

    /// Borrow the exact instruction data.
    pub const fn data(&self) -> &[u8; DATA] {
        &self.data
    }
}

/// Exact single-authority `InitializeAccount3` specification.
pub type InitializeAccount3Instruction = InstructionSpec<2, INITIALIZE_ACCOUNT3_DATA_BYTES>;
/// Exact single-authority `TransferChecked` specification.
pub type TransferCheckedInstruction = InstructionSpec<4, TRANSFER_CHECKED_DATA_BYTES>;
/// Exact single-authority `CloseAccount` specification.
pub type CloseAccountInstruction = InstructionSpec<3, CLOSE_ACCOUNT_DATA_BYTES>;

/// Build exact `InitializeAccount3` data and its two ordered account roles.
///
/// The account roles are writable account and readonly Mint. The new token
/// owner is encoded in instruction data and is not an account role.
pub fn initialize_account3(
    program_id: Address,
    account: Address,
    mint: Address,
    owner: Address,
) -> Result<InitializeAccount3Instruction> {
    TokenProgram::parse(program_id)?;
    let mut data = [0; INITIALIZE_ACCOUNT3_DATA_BYTES];
    put(&mut data, 0, &[INITIALIZE_ACCOUNT3_TAG]);
    put(&mut data, 1, &owner);
    Ok(InstructionSpec {
        program_id,
        accounts: [
            AccountMeta::new(account, false, true),
            AccountMeta::new(mint, false, false),
        ],
        data,
    })
}

/// Build exact single-authority `TransferChecked` data and ordered roles.
///
/// The roles are writable source, readonly Mint, writable destination, and
/// readonly signer authority. Multisignature expansion is intentionally left
/// to a future separately profiled builder; this fixed form is the PDA/single
/// signer form used by dClutch custody.
pub fn transfer_checked(
    program_id: Address,
    source: Address,
    mint: Address,
    destination: Address,
    authority: Address,
    amount: u64,
    decimals: u8,
) -> Result<TransferCheckedInstruction> {
    TokenProgram::parse(program_id)?;
    let mut data = [0; TRANSFER_CHECKED_DATA_BYTES];
    put(&mut data, 0, &[TRANSFER_CHECKED_TAG]);
    put(&mut data, 1, &amount.to_le_bytes());
    put(&mut data, 9, &[decimals]);
    Ok(InstructionSpec {
        program_id,
        accounts: [
            AccountMeta::new(source, false, true),
            AccountMeta::new(mint, false, false),
            AccountMeta::new(destination, false, true),
            AccountMeta::new(authority, true, false),
        ],
        data,
    })
}

/// Build exact single-authority `CloseAccount` data and ordered roles.
///
/// The roles are writable token account, writable lamport destination, and
/// readonly signer authority.
pub fn close_account(
    program_id: Address,
    account: Address,
    destination: Address,
    authority: Address,
) -> Result<CloseAccountInstruction> {
    TokenProgram::parse(program_id)?;
    Ok(InstructionSpec {
        program_id,
        accounts: [
            AccountMeta::new(account, false, true),
            AccountMeta::new(destination, false, true),
            AccountMeta::new(authority, true, false),
        ],
        data: [CLOSE_ACCOUNT_TAG],
    })
}

/// Borrowed exact `InitializeAccount3` instruction-data view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InitializeAccount3View<'a> {
    raw: &'a [u8],
    owner: Address,
}

impl<'a> InitializeAccount3View<'a> {
    /// Borrow all exact input bytes.
    pub const fn raw(&self) -> &'a [u8] {
        self.raw
    }

    /// Return the owner encoded by the instruction.
    pub const fn owner(&self) -> Address {
        self.owner
    }
}

/// Borrowed exact `TransferChecked` instruction-data view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransferCheckedView<'a> {
    raw: &'a [u8],
    amount: u64,
    decimals: u8,
}

impl<'a> TransferCheckedView<'a> {
    /// Borrow all exact input bytes.
    pub const fn raw(&self) -> &'a [u8] {
        self.raw
    }

    /// Return the exact raw token amount.
    pub const fn amount(&self) -> u64 {
        self.amount
    }

    /// Return the Mint decimal count to be checked by the token program.
    pub const fn decimals(&self) -> u8 {
        self.decimals
    }
}

/// Borrowed exact `CloseAccount` instruction-data view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseAccountView<'a> {
    raw: &'a [u8],
}

impl<'a> CloseAccountView<'a> {
    /// Borrow the one exact tag byte.
    pub const fn raw(&self) -> &'a [u8] {
        self.raw
    }
}

/// One of the three exact borrowed instruction-data shapes owned here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstructionDataView<'a> {
    /// Exact `InitializeAccount3` data.
    InitializeAccount3(InitializeAccount3View<'a>),
    /// Exact `TransferChecked` data.
    TransferChecked(TransferCheckedView<'a>),
    /// Exact `CloseAccount` data.
    CloseAccount(CloseAccountView<'a>),
}

impl<'a> InstructionDataView<'a> {
    /// Parse one supported instruction and reject unknown tags, truncation,
    /// and otherwise-valid encodings with trailing bytes.
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        let tag = bytes.first().copied().ok_or(Error::InvalidInstruction)?;
        match tag {
            INITIALIZE_ACCOUNT3_TAG if bytes.len() == INITIALIZE_ACCOUNT3_DATA_BYTES => {
                Ok(Self::InitializeAccount3(InitializeAccount3View {
                    raw: bytes,
                    owner: read_array(bytes, 1)?,
                }))
            }
            TRANSFER_CHECKED_TAG if bytes.len() == TRANSFER_CHECKED_DATA_BYTES => {
                Ok(Self::TransferChecked(TransferCheckedView {
                    raw: bytes,
                    amount: u64::from_le_bytes(read_array(bytes, 1)?),
                    decimals: bytes.get(9).copied().ok_or(Error::InvalidInstruction)?,
                }))
            }
            CLOSE_ACCOUNT_TAG if bytes.len() == CLOSE_ACCOUNT_DATA_BYTES => {
                Ok(Self::CloseAccount(CloseAccountView { raw: bytes }))
            }
            _ => Err(Error::InvalidInstruction),
        }
    }

    /// Borrow all exact bytes independent of the decoded variant.
    pub const fn raw(&self) -> &'a [u8] {
        match self {
            Self::InitializeAccount3(view) => view.raw(),
            Self::TransferChecked(view) => view.raw(),
            Self::CloseAccount(view) => view.raw(),
        }
    }
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidInstruction)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidInstruction)?
        .try_into()
        .map_err(|_| Error::InvalidInstruction)
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}

#[cfg(test)]
mod tests {
    use crate::{LEGACY_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID};

    use super::*;

    #[test]
    fn builders_match_the_exact_single_authority_abis() {
        let initialized = initialize_account3(LEGACY_TOKEN_PROGRAM_ID, [1; 32], [2; 32], [3; 32]);
        assert!(initialized.is_ok());
        if let Ok(value) = initialized {
            let mut expected = [0; INITIALIZE_ACCOUNT3_DATA_BYTES];
            put(&mut expected, 0, &[18]);
            put(&mut expected, 1, &[3; 32]);
            assert_eq!(value.data(), &expected);
            assert_eq!(
                value.accounts(),
                &[
                    AccountMeta::new([1; 32], false, true),
                    AccountMeta::new([2; 32], false, false),
                ]
            );
        }

        let transferred = transfer_checked(
            TOKEN_2022_PROGRAM_ID,
            [1; 32],
            [2; 32],
            [3; 32],
            [4; 32],
            0x0807_0605_0403_0201,
            9,
        );
        assert!(transferred.is_ok());
        if let Ok(value) = transferred {
            assert_eq!(value.data(), &[12, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
            assert_eq!(
                value.accounts(),
                &[
                    AccountMeta::new([1; 32], false, true),
                    AccountMeta::new([2; 32], false, false),
                    AccountMeta::new([3; 32], false, true),
                    AccountMeta::new([4; 32], true, false),
                ]
            );
        }

        let closed = close_account(LEGACY_TOKEN_PROGRAM_ID, [1; 32], [2; 32], [3; 32]);
        assert!(closed.is_ok());
        if let Ok(value) = closed {
            assert_eq!(value.data(), &[9]);
            assert_eq!(
                value.accounts(),
                &[
                    AccountMeta::new([1; 32], false, true),
                    AccountMeta::new([2; 32], false, true),
                    AccountMeta::new([3; 32], true, false),
                ]
            );
        }
    }

    #[test]
    fn borrowed_views_require_exact_encodings() {
        let initialized = initialize_account3(LEGACY_TOKEN_PROGRAM_ID, [1; 32], [2; 32], [3; 32]);
        if let Ok(value) = initialized {
            assert_eq!(
                InstructionDataView::parse(value.data()),
                Ok(InstructionDataView::InitializeAccount3(
                    InitializeAccount3View {
                        raw: value.data(),
                        owner: [3; 32]
                    }
                ))
            );
        }
        let transferred = transfer_checked(
            LEGACY_TOKEN_PROGRAM_ID,
            [1; 32],
            [2; 32],
            [3; 32],
            [4; 32],
            11,
            6,
        );
        if let Ok(value) = transferred {
            let parsed = InstructionDataView::parse(value.data());
            assert!(matches!(
                parsed,
                Ok(InstructionDataView::TransferChecked(_))
            ));
        }
        assert!(matches!(
            InstructionDataView::parse(&[9]),
            Ok(InstructionDataView::CloseAccount(_))
        ));

        for invalid in [&[][..], &[9, 0][..], &[12][..], &[18][..], &[255][..]] {
            assert_eq!(
                InstructionDataView::parse(invalid),
                Err(Error::InvalidInstruction)
            );
        }
        let transfer_with_trailer = [12, 0, 0, 0, 0, 0, 0, 0, 0, 6, 0];
        let initialize_with_trailer = [18; INITIALIZE_ACCOUNT3_DATA_BYTES + 1];
        assert_eq!(
            InstructionDataView::parse(&transfer_with_trailer),
            Err(Error::InvalidInstruction)
        );
        assert_eq!(
            InstructionDataView::parse(&initialize_with_trailer),
            Err(Error::InvalidInstruction)
        );
    }

    #[test]
    fn every_builder_refuses_an_unprofiled_program() {
        let unknown = [99; 32];
        assert_eq!(
            initialize_account3(unknown, [1; 32], [2; 32], [3; 32]),
            Err(Error::UnsupportedProgram)
        );
        assert_eq!(
            transfer_checked(unknown, [1; 32], [2; 32], [3; 32], [4; 32], 1, 6),
            Err(Error::UnsupportedProgram)
        );
        assert_eq!(
            close_account(unknown, [1; 32], [2; 32], [3; 32]),
            Err(Error::UnsupportedProgram)
        );
    }
}
