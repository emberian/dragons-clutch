#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact SDK-free semantics for permanent per-authority native-rent credit.
//!
//! `RentCreditV1` is a program-owned, non-closeable 48-byte account. A
//! source's legacy `rent_refund` bytes remain the refund/beneficiary authority,
//! never a direct payout account. The composing adapter derives this credit
//! PDA and credits it when closing each source. Only the authority may withdraw
//! presently claimable surplus to a separately supplied System wallet.
//!
//! This crate owns byte canonicality, instruction grammar, role/alias policy,
//! and exact balance plans. It does not derive PDAs, inspect account owners or
//! data, deserialize Rent, invoke System, transfer lamports, or close accounts.

use core::convert::TryInto;

/// Exact width of a Solana-compatible public-key byte string.
pub const PUBKEY_BYTES: usize = 32;
/// Exact width of a persistent V1 rent-credit record.
pub const RENT_CREDIT_BYTES_V1: usize = 48;
/// Exact width of the shared instruction header.
pub const RENT_CREDIT_INSTRUCTION_HEADER_BYTES_V1: usize = 16;
/// Exact width of a canonical permissionless Create instruction.
pub const CREATE_RENT_CREDIT_BYTES_V1: usize = 56;
/// Exact width of a canonical Withdraw instruction.
pub const WITHDRAW_RENT_CREDIT_BYTES_V1: usize = 24;

/// PDA domain for one permanent rent credit per refund authority.
///
/// This is 22 bytes, within Solana's 32-byte individual PDA-seed limit.
pub const RENT_CREDIT_PDA_DOMAIN_V1: &[u8] = b"dclutch/rent-credit/v1";
/// Exact byte count of [`RENT_CREDIT_PDA_DOMAIN_V1`].
pub const RENT_CREDIT_PDA_DOMAIN_BYTES_V1: usize = 22;

/// Canonical persistent-account magic.
pub const RENT_CREDIT_MAGIC_V1: [u8; 8] = *b"DCLTRNT1";
/// Canonical rent-credit instruction magic.
pub const RENT_CREDIT_INSTRUCTION_MAGIC_V1: [u8; 8] = *b"DCLTRIX1";
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

const HEADER_SCHEMA_OFFSET: usize = 8;
const HEADER_ACTION_OFFSET: usize = 10;
const HEADER_RESERVED_OFFSET: usize = 11;
const CREATE_AUTHORITY_OFFSET: usize = 16;
const CREATE_BUMP_OFFSET: usize = 48;
const CREATE_RESERVED_OFFSET: usize = 49;
const WITHDRAW_AMOUNT_OFFSET: usize = 16;

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

/// V1 rent-credit action discriminator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RentCreditActionV1 {
    /// Permissionlessly create and fund a credit.
    Create,
    /// Withdraw exact current surplus.
    Withdraw,
}

impl RentCreditActionV1 {
    fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::Create),
            2 => Ok(Self::Withdraw),
            _ => Err(Error::UnknownAction),
        }
    }
    const fn byte(self) -> u8 {
        match self {
            Self::Create => 1,
            Self::Withdraw => 2,
        }
    }
}

/// Exact canonical rent-credit instruction wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RentCreditInstructionV1 {
    /// Permissionless creation data.
    Create(CreateRentCreditV1),
    /// Authority-signed withdrawal data.
    Withdraw(WithdrawRentCreditV1),
}

impl RentCreditInstructionV1 {
    /// Hostile-decode one exact V1 Create or Withdraw wire, rejecting trailing bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        decode_instruction_header(bytes)?;
        match RentCreditActionV1::decode(read_byte(bytes, HEADER_ACTION_OFFSET)?)? {
            RentCreditActionV1::Create => Ok(Self::Create(CreateRentCreditV1::decode(bytes)?)),
            RentCreditActionV1::Withdraw => {
                Ok(Self::Withdraw(WithdrawRentCreditV1::decode(bytes)?))
            }
        }
    }
}

/// Permissionless Create instruction contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateRentCreditV1 {
    refund_authority: RefundAuthority,
    pda_bump: u8,
}

impl CreateRentCreditV1 {
    /// Construct creation data for the authority's one derived credit PDA.
    pub const fn new(refund_authority: RefundAuthority, pda_bump: u8) -> Self {
        Self {
            refund_authority,
            pda_bump,
        }
    }
    /// Hostile-decode one exact canonical Create wire.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        decode_exact_instruction(
            bytes,
            RentCreditActionV1::Create,
            CREATE_RENT_CREDIT_BYTES_V1,
        )?;
        require_zero(bytes, CREATE_RESERVED_OFFSET, 7)?;
        Ok(Self::new(
            RefundAuthority::new(read_array(bytes, CREATE_AUTHORITY_OFFSET)?)?,
            read_byte(bytes, CREATE_BUMP_OFFSET)?,
        ))
    }
    /// Return the exact canonical Create wire.
    pub fn to_bytes(self) -> [u8; CREATE_RENT_CREDIT_BYTES_V1] {
        let mut output = header_56(RentCreditActionV1::Create);
        put(
            &mut output,
            CREATE_AUTHORITY_OFFSET,
            &self.refund_authority.to_bytes(),
        );
        output[CREATE_BUMP_OFFSET] = self.pda_bump;
        output
    }
    /// Return the authority encoded in data rather than supplied as an account.
    pub const fn refund_authority(self) -> RefundAuthority {
        self.refund_authority
    }
    /// Return the derived PDA bump encoded in data and persisted in state.
    pub const fn pda_bump(self) -> u8 {
        self.pda_bump
    }
    /// Return the exact persistent account state this request creates.
    pub const fn credit(self) -> RentCreditV1 {
        RentCreditV1::new(self.refund_authority, self.pda_bump)
    }
}

/// Exact requested native-lamport withdrawal instruction contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WithdrawRentCreditV1 {
    requested_lamports: u64,
}

impl WithdrawRentCreditV1 {
    /// Construct a nonzero exact withdrawal request.
    pub fn new(requested_lamports: u64) -> Result<Self> {
        if requested_lamports == 0 {
            return Err(Error::ZeroWithdrawal);
        }
        Ok(Self { requested_lamports })
    }
    /// Hostile-decode one exact canonical Withdraw wire.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        decode_exact_instruction(
            bytes,
            RentCreditActionV1::Withdraw,
            WITHDRAW_RENT_CREDIT_BYTES_V1,
        )?;
        Self::new(read_u64(bytes, WITHDRAW_AMOUNT_OFFSET)?)
    }
    /// Return the exact canonical Withdraw wire.
    pub fn to_bytes(self) -> [u8; WITHDRAW_RENT_CREDIT_BYTES_V1] {
        let mut output = header_24(RentCreditActionV1::Withdraw);
        put(
            &mut output,
            WITHDRAW_AMOUNT_OFFSET,
            &self.requested_lamports.to_le_bytes(),
        );
        output
    }
    /// Return the exact requested amount; no implicit partial withdrawal exists.
    pub const fn requested_lamports(self) -> u64 {
        self.requested_lamports
    }
}

/// Runtime privilege projection supplied by the SVM adapter for one account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountMetaV1 {
    /// Exact account key bytes.
    pub key: [u8; PUBKEY_BYTES],
    /// Whether the runtime marks this account as a signer.
    pub is_signer: bool,
    /// Whether the runtime marks this account writable.
    pub is_writable: bool,
    /// Whether the runtime marks this account executable.
    pub is_executable: bool,
}

/// Exact ordered Create account roles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateRentCreditRoleV1 {
    /// Writable signer supplying the exact current rent minimum.
    Payer,
    /// Vacant writable PDA that becomes the permanent credit account.
    RentCredit,
    /// Canonical executable System Program.
    SystemProgram,
    /// Canonical nonexecutable Rent sysvar.
    RentSysvar,
}

/// Exact ordered Create frame, independent of Solana SDK account types.
pub const CREATE_RENT_CREDIT_FRAME_V1: [CreateRentCreditRoleV1; 4] = [
    CreateRentCreditRoleV1::Payer,
    CreateRentCreditRoleV1::RentCredit,
    CreateRentCreditRoleV1::SystemProgram,
    CreateRentCreditRoleV1::RentSysvar,
];

/// Hostile runtime projection for the four ordered Create accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateRentCreditFrameV1 {
    accounts: [AccountMetaV1; 4],
}

impl CreateRentCreditFrameV1 {
    /// Validate ordering, privileges, System/Rent identities, and no aliases.
    ///
    /// The adapter must additionally authenticate the payer as a System payer,
    /// the credit key as the vacant PDA derived from Create data, and the Rent
    /// value used to obtain the current minimum. No authority account exists:
    /// creation is a third-party reserve donation.
    pub fn new(accounts: [AccountMetaV1; 4]) -> Result<Self> {
        let payer = accounts[0];
        let credit = accounts[1];
        let system = accounts[2];
        let rent = accounts[3];
        ordinary_key(payer.key)?;
        ordinary_key(credit.key)?;
        if !payer.is_signer
            || !payer.is_writable
            || payer.is_executable
            || credit.is_signer
            || !credit.is_writable
            || credit.is_executable
        {
            return Err(Error::InvalidAccountPrivilege);
        }
        validate_system_program(system)?;
        validate_rent_sysvar(rent)?;
        require_distinct(&[payer.key, credit.key, system.key, rent.key])?;
        Ok(Self { accounts })
    }

    /// Return exact ordered runtime accounts after validation.
    pub const fn accounts(self) -> [AccountMetaV1; 4] {
        self.accounts
    }
}

/// Exact ordered Withdraw account roles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WithdrawRentCreditRoleV1 {
    /// Writable permanent program-owned credit PDA.
    RentCredit,
    /// Readonly signer matching the credit's immutable refund authority.
    Authority,
    /// Writable data-empty System wallet receiving exact surplus.
    Recipient,
    /// Canonical nonexecutable Rent sysvar.
    RentSysvar,
}

/// Exact ordered Withdraw frame.
pub const WITHDRAW_RENT_CREDIT_FRAME_V1: [WithdrawRentCreditRoleV1; 4] = [
    WithdrawRentCreditRoleV1::RentCredit,
    WithdrawRentCreditRoleV1::Authority,
    WithdrawRentCreditRoleV1::Recipient,
    WithdrawRentCreditRoleV1::RentSysvar,
];

/// Authenticated adapter facts establishing that a recipient is a System wallet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemWalletFactsV1 {
    /// Whether the adapter authenticated the account owner as the System Program.
    pub is_system_owned: bool,
    /// Exact observed account-data length; V1 recipient must be data-empty.
    pub data_len: u64,
}

impl SystemWalletFactsV1 {
    /// Construct exact facts for a data-empty System-owned native wallet.
    pub fn new(is_system_owned: bool, data_len: u64) -> Result<Self> {
        if !is_system_owned || data_len != 0 {
            return Err(Error::InvalidSystemWallet);
        }
        Ok(Self {
            is_system_owned,
            data_len,
        })
    }
}

/// Hostile runtime projection for the four ordered Withdraw accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WithdrawRentCreditFrameV1 {
    accounts: [AccountMetaV1; 4],
}

impl WithdrawRentCreditFrameV1 {
    /// Validate exact Withdraw role privileges and V1 aliases.
    ///
    /// Authority may equal recipient only under runtime privilege union: that
    /// one key is signer and writable. Otherwise authority is readonly signer
    /// and recipient is nonsigner writable. For a distinct authority this
    /// checker intentionally does not inspect executable, owner, data, or
    /// lamport facts. The adapter separately authenticates credit PDA/ownership,
    /// authority binding, and recipient wallet facts.
    pub fn new(accounts: [AccountMetaV1; 4], recipient: SystemWalletFactsV1) -> Result<Self> {
        let credit = accounts[0];
        let authority = accounts[1];
        let destination = accounts[2];
        let rent = accounts[3];
        ordinary_key(credit.key)?;
        ordinary_key(authority.key)?;
        ordinary_key(destination.key)?;
        if credit.is_signer || !credit.is_writable || credit.is_executable {
            return Err(Error::InvalidAccountPrivilege);
        }
        validate_rent_sysvar(rent)?;
        if authority.key == destination.key {
            if !authority.is_signer
                || !authority.is_writable
                || authority.is_executable
                || !destination.is_signer
                || !destination.is_writable
                || destination.is_executable
            {
                return Err(Error::InvalidAccountPrivilege);
            }
        } else if !authority.is_signer
            || authority.is_writable
            || destination.is_signer
            || !destination.is_writable
            || destination.is_executable
        {
            return Err(Error::InvalidAccountPrivilege);
        }
        if !recipient.is_system_owned || recipient.data_len != 0 {
            return Err(Error::InvalidSystemWallet);
        }
        if credit.key == authority.key
            || credit.key == destination.key
            || credit.key == rent.key
            || authority.key == rent.key
            || destination.key == rent.key
        {
            return Err(Error::AccountAlias);
        }
        Ok(Self { accounts })
    }

    /// Return exact ordered runtime accounts after validation.
    pub const fn accounts(self) -> [AccountMetaV1; 4] {
        self.accounts
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

/// Exact successful withdrawal balance transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WithdrawBalancePlanV1 {
    credit_before: u64,
    credit_after: u64,
    recipient_before: u64,
    recipient_after: u64,
    current_rent_minimum: u64,
    requested_lamports: u64,
}

impl WithdrawBalancePlanV1 {
    /// Build the sole admitted nonzero exact-withdrawal plan.
    pub fn new(
        credit_before: u64,
        recipient_before: u64,
        current_rent_minimum: u64,
        request: WithdrawRentCreditV1,
    ) -> Result<Self> {
        let requested_lamports = request.requested_lamports();
        if requested_lamports > claimable_lamports(credit_before, current_rent_minimum) {
            return Err(Error::WithdrawalExceedsClaimable);
        }
        let credit_after = credit_before
            .checked_sub(requested_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        let recipient_after = recipient_before
            .checked_add(requested_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(Self {
            credit_before,
            credit_after,
            recipient_before,
            recipient_after,
            current_rent_minimum,
            requested_lamports,
        })
    }
    /// Verify observed post-balances after exact transfer.
    pub fn validate_post(self, credit_after: u64, recipient_after: u64) -> Result<()> {
        if credit_after != self.credit_after || recipient_after != self.recipient_after {
            return Err(Error::WithdrawalExceedsClaimable);
        }
        Ok(())
    }
    /// Return credit lamports before withdrawal.
    pub const fn credit_before(self) -> u64 {
        self.credit_before
    }
    /// Return credit lamports after withdrawal.
    pub const fn credit_after(self) -> u64 {
        self.credit_after
    }
    /// Return recipient lamports before withdrawal.
    pub const fn recipient_before(self) -> u64 {
        self.recipient_before
    }
    /// Return recipient lamports after withdrawal.
    pub const fn recipient_after(self) -> u64 {
        self.recipient_after
    }
    /// Return Rent minimum used for withdrawal floor.
    pub const fn current_rent_minimum(self) -> u64 {
        self.current_rent_minimum
    }
    /// Return one exact requested transferred amount.
    pub const fn requested_lamports(self) -> u64 {
        self.requested_lamports
    }
}

/// Refuse closure explicitly: V1 never closes, drains, or redirects credit.
pub fn close_rent_credit_v1() -> Result<()> {
    Err(Error::CloseNotSupported)
}

fn decode_instruction_header(bytes: &[u8]) -> Result<()> {
    if bytes.len() < RENT_CREDIT_INSTRUCTION_HEADER_BYTES_V1 {
        return Err(Error::InvalidLength);
    }
    if read_array(bytes, 0)? != RENT_CREDIT_INSTRUCTION_MAGIC_V1 {
        return Err(Error::InvalidMagic);
    }
    if read_u16(bytes, HEADER_SCHEMA_OFFSET)? != RENT_CREDIT_SCHEMA_VERSION_V1 {
        return Err(Error::UnsupportedSchema);
    }
    require_zero(bytes, HEADER_RESERVED_OFFSET, 5)
}

fn decode_exact_instruction(bytes: &[u8], action: RentCreditActionV1, width: usize) -> Result<()> {
    if bytes.len() != width {
        return Err(Error::InvalidLength);
    }
    decode_instruction_header(bytes)?;
    if read_byte(bytes, HEADER_ACTION_OFFSET)? != action.byte() {
        return Err(Error::UnknownAction);
    }
    Ok(())
}

fn header_56(action: RentCreditActionV1) -> [u8; CREATE_RENT_CREDIT_BYTES_V1] {
    let mut output = [0; CREATE_RENT_CREDIT_BYTES_V1];
    put(&mut output, 0, &RENT_CREDIT_INSTRUCTION_MAGIC_V1);
    put(
        &mut output,
        HEADER_SCHEMA_OFFSET,
        &RENT_CREDIT_SCHEMA_VERSION_V1.to_le_bytes(),
    );
    output[HEADER_ACTION_OFFSET] = action.byte();
    output
}

fn header_24(action: RentCreditActionV1) -> [u8; WITHDRAW_RENT_CREDIT_BYTES_V1] {
    let mut output = [0; WITHDRAW_RENT_CREDIT_BYTES_V1];
    put(&mut output, 0, &RENT_CREDIT_INSTRUCTION_MAGIC_V1);
    put(
        &mut output,
        HEADER_SCHEMA_OFFSET,
        &RENT_CREDIT_SCHEMA_VERSION_V1.to_le_bytes(),
    );
    output[HEADER_ACTION_OFFSET] = action.byte();
    output
}

fn validate_system_program(account: AccountMetaV1) -> Result<()> {
    if account.key != SYSTEM_PROGRAM_ID
        || account.is_signer
        || account.is_writable
        || !account.is_executable
    {
        return Err(Error::InvalidSystemProgram);
    }
    Ok(())
}

fn validate_rent_sysvar(account: AccountMetaV1) -> Result<()> {
    if account.key != RENT_SYSVAR_ID
        || account.is_signer
        || account.is_writable
        || account.is_executable
    {
        return Err(Error::InvalidRentSysvar);
    }
    Ok(())
}

fn ordinary_key(key: [u8; PUBKEY_BYTES]) -> Result<()> {
    if is_zero(&key) {
        return Err(Error::ZeroAuthorityOrAccount);
    }
    Ok(())
}

fn require_distinct(keys: &[[u8; PUBKEY_BYTES]]) -> Result<()> {
    for (index, key) in keys.iter().enumerate() {
        if keys.iter().take(index).any(|prior| prior == key) {
            return Err(Error::AccountAlias);
        }
    }
    Ok(())
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
fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
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
    fn meta(key: [u8; 32], signer: bool, writable: bool, executable: bool) -> AccountMetaV1 {
        AccountMetaV1 {
            key,
            is_signer: signer,
            is_writable: writable,
            is_executable: executable,
        }
    }
    fn rent() -> AccountMetaV1 {
        meta(RENT_SYSVAR_ID, false, false, false)
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
        let create = CreateRentCreditV1::new(authority(4), 9);
        let canonical = create.to_bytes();
        assert_eq!(
            RentCreditInstructionV1::decode(&canonical),
            Ok(RentCreditInstructionV1::Create(create))
        );
        let mut trailing = [0u8; 57];
        trailing[..56].copy_from_slice(&canonical);
        assert_eq!(
            RentCreditInstructionV1::decode(&trailing),
            Err(Error::InvalidLength)
        );
        let mut dirty_create = canonical;
        dirty_create[55] = 1;
        assert_eq!(
            CreateRentCreditV1::decode(&dirty_create),
            Err(Error::NonCanonicalReservedBytes)
        );
    }

    #[test]
    fn create_is_permissionless_and_funds_exact_current_minimum() {
        let request = CreateRentCreditV1::new(authority(5), 17);
        let frame = CreateRentCreditFrameV1::new([
            meta(key(1), true, true, false),
            meta(key(2), false, true, false),
            meta(SYSTEM_PROGRAM_ID, false, false, true),
            rent(),
        ])
        .expect("frame");
        assert_eq!(frame.accounts()[0].key, key(1));
        assert_eq!(request.credit(), RentCreditV1::new(authority(5), 17));
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

    #[test]
    fn withdraw_reserve_floor_zero_and_overflow() {
        assert_eq!(WithdrawRentCreditV1::new(0), Err(Error::ZeroWithdrawal));
        let request = WithdrawRentCreditV1::new(20).expect("nonzero");
        let plan = WithdrawBalancePlanV1::new(120, 7, 100, request).expect("claimable");
        assert_eq!((plan.credit_after(), plan.recipient_after()), (100, 27));
        assert_eq!(
            WithdrawBalancePlanV1::new(
                120,
                7,
                100,
                WithdrawRentCreditV1::new(21).expect("request")
            ),
            Err(Error::WithdrawalExceedsClaimable)
        );
        assert_eq!(
            WithdrawBalancePlanV1::new(120, u64::MAX, 100, request),
            Err(Error::ArithmeticOverflow)
        );
    }

    #[test]
    fn alias_policy_allows_only_authority_destination_privilege_union() {
        let facts = SystemWalletFactsV1::new(true, 0).expect("wallet");
        let executable_authority = WithdrawRentCreditFrameV1::new(
            [
                meta(key(1), false, true, false),
                meta(key(2), true, false, true),
                meta(key(3), false, true, false),
                rent(),
            ],
            facts,
        )
        .expect("distinct authority executable status is irrelevant");
        assert_eq!(executable_authority.accounts()[1].key, key(2));
        let aliased = WithdrawRentCreditFrameV1::new(
            [
                meta(key(1), false, true, false),
                meta(key(2), true, true, false),
                meta(key(2), true, true, false),
                rent(),
            ],
            facts,
        )
        .expect("union");
        assert_eq!(aliased.accounts()[1].key, aliased.accounts()[2].key);
        assert_eq!(
            WithdrawRentCreditFrameV1::new(
                [
                    meta(key(1), false, true, false),
                    meta(key(2), true, false, false),
                    meta(key(2), true, false, false),
                    rent(),
                ],
                facts
            ),
            Err(Error::InvalidAccountPrivilege)
        );
        assert_eq!(
            WithdrawRentCreditFrameV1::new(
                [
                    meta(key(1), false, true, false),
                    meta(key(1), true, false, false),
                    meta(key(3), false, true, false),
                    rent(),
                ],
                facts
            ),
            Err(Error::AccountAlias)
        );
    }

    #[test]
    fn v1_explicitly_refuses_close_semantics() {
        assert_eq!(close_rent_credit_v1(), Err(Error::CloseNotSupported));
    }
}
