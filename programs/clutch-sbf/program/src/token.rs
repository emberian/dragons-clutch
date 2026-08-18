//! Token-2022 admission and cross-program invocation.
//!
//! This module owns exactly two things and deliberately owns nothing else:
//!
//! 1. **Observation and admission.** Turning the bytes a real Token-2022
//!    program wrote — a mint account, a token account — into a small facts
//!    structure, and deciding accept-or-refuse against a policy.
//! 2. **CPI construction.** Building the Token-2022 instruction and performing
//!    the `invoke` or `invoke_signed`, plus the exact-delta primitives that
//!    check what the CPI actually did.
//!
//! It contains **no economics**. It does not know what a complete set is, what
//! a Hoard is for, or how many atoms a transition should move: every quantity
//! is a parameter supplied by the instruction family that already ran the
//! kernel. It also owns no account list and no ordering — those belong to the
//! family modules, which is why [`crate::instructions::split`] and not this
//! module decides when the CPI happens.
//!
//! ## Where this comes from, and what carries over
//!
//! `toolchain/probes/token2022/src/lib.rs` established the predicate against a
//! real in-process bank. Its `RefusalCode` numbering — which is also
//! `RefusalCode` in `research/collateral-profiles/model.py` — carries over
//! verbatim as [`TokenRefusal`]. The *code* does not carry over and was never
//! meant to: the probe decides over `Vec<u8>` with `Vec<ExtensionType>`
//! observations and the `spl-token-2022-interface` crate in the graph. Inside
//! a 4 KiB SBF frame, offline, none of that is available, so what is here is a
//! second implementation of one decision procedure over `AccountInfo`, with
//! the extension set as a `u64` bitset rather than a heap vector.
//!
//! The bitset is not a shortcut. It is the *same* representation
//! `clutch_solana_layout::collateral::CollateralPolicy` already stores its four
//! extension sets in, so admitting a mint against a Realm policy is a mask
//! comparison and not a translation. `EXTENSION_KNOWN_MASK` is 29 bits and
//! `ExtensionType` in `spl-token-2022-interface` 3.1.1 has exactly 29
//! non-test discriminants; a thirtieth is refused by
//! [`TokenRefusal::UnknownExtension`] rather than ignored, which is the
//! fail-closed rule `docs/implementation/COLLATERAL_PROFILES.md` states and the
//! reason `MintCloseAuthority` is refusable at all.
//!
//! ## Why the TLV walk is written out rather than imported
//!
//! `StateWithExtensions::unpack` allocates a `Vec<ExtensionType>` and pulls in
//! `bytemuck`, `arrayref`, `num_enum` and the whole interface crate. The walk
//! below is byte-for-byte the same decision procedure —
//! `check_min_len_and_not_multisig`, `type_and_tlv_indices`,
//! `try_for_each_tlv_extension_type` — with the same refusals in the same
//! places, over a borrowed slice, with no allocation. Divergence from the real
//! decoder is the risk this trades for; the svm-tests workspace is what
//! detects it, because there the bytes are written by the actual program.
//!
//! ## The two mint roles
//!
//! `docs/implementation/TOKEN2022_PLAN.md` §3.1 separates them and so does
//! this module:
//!
//! | role | policy constructor | mint authority | freeze | decimals | extensions |
//! | --- | --- | --- | --- | --- | --- |
//! | collateral mint | [`MintPolicy::collateral`] | must be absent | absent | profile-fixed | Realm bitset ∩ protocol ceiling |
//! | outcome mint | [`MintPolicy::outcome`] | must be exactly the market PDA | absent | `0` — PROPOSED | none |
//!
//! ## Off-chain, `invoke_signed` is a silent no-op
//!
//! `solana_cpi::invoke_signed` compiles to `Ok(())` under
//! `not(target_os = "solana")`. That is a trap for a host differential: a
//! materialize that "succeeded" off-chain would have moved nothing. The
//! exact-delta check of [`require_exact_credit`] is what turns that trap into a
//! refusal — off-chain the observed delta is zero, the expected delta is *q*,
//! and the instruction refuses [`crate::error::ClutchError::TokenDeltaMismatch`].
//! `crate::instructions::split`'s host tests pin exactly that, so the no-op is
//! a documented, tested property rather than a hazard.

use crate::error::{ClutchError, Refusal};
use clutch_solana_layout::collateral::{
    CollateralPolicy, CurrencyKind, EXTENSION_KNOWN_MASK,
    FLAG_REQUIRE_ACCOUNT_CLOSE_AUTHORITY_NONE, FLAG_REQUIRE_ACCOUNT_DELEGATE_NONE,
    FLAG_REQUIRE_FREEZE_AUTHORITY_NONE, FLAG_REQUIRE_MINT_AUTHORITY_NONE,
    FLAG_REQUIRE_NONZERO_SUPPLY, PROTOCOL_ACCOUNT_EXTENSION_CEILING, TOKEN_2022_PROGRAM,
};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/* ------------------------------------------------------------------------ */
/* Pinned program                                                            */
/* ------------------------------------------------------------------------ */

/// The one token program this adapter invokes.
///
/// Taken from the frozen layout crate rather than re-typed, so the bytes this
/// program compares against and the bytes a `CurrencyRef` is validated against
/// cannot drift apart. `docs/implementation/TOKEN2022_PLAN.md` open decision 7
/// records that a program *id* is not a pin of the program's *behaviour*; this
/// constant is the id, and nothing here claims to be the other thing.
pub const TOKEN_2022_PROGRAM_ID: Pubkey = Pubkey::new_from_array(TOKEN_2022_PROGRAM);

/// Token-2022 instruction discriminants this adapter emits.
///
/// One byte each, from `TokenInstruction::pack` in `spl-token-2022-interface`
/// 3.1.1. Only the six the plan's §3.2 table names are here; an instruction
/// this program never emits has no business having a constant.
mod ix {
    /// `MintTo { amount: u64 }`.
    pub const MINT_TO: u8 = 7;
    /// `Burn { amount: u64 }`.
    pub const BURN: u8 = 8;
    /// `TransferChecked { amount: u64, decimals: u8 }`.
    pub const TRANSFER_CHECKED: u8 = 12;
    /// `InitializeAccount3 { owner: Pubkey }`.
    pub const INITIALIZE_ACCOUNT3: u8 = 18;
    /// `InitializeMint2 { decimals, mint_authority, freeze_authority }`.
    pub const INITIALIZE_MINT2: u8 = 20;
    /// `InitializeImmutableOwner`.
    pub const INITIALIZE_IMMUTABLE_OWNER: u8 = 22;
}

/* ------------------------------------------------------------------------ */
/* Refusals                                                                  */
/* ------------------------------------------------------------------------ */

/// Why an admission failed.
///
/// Numbered identically to `RefusalCode` in
/// `research/collateral-profiles/model.py` and in the probe, so a decision made
/// here, a decision made by the offline model, and a decision made by the probe
/// are directly comparable. The numbering is *not* the on-chain code: seven
/// of these project onto one `ProgramError::Custom` each through
/// [`TokenRefusal::clutch_error`], because
/// `docs/implementation/TOKEN2022_PLAN.md` §3.2 allocates classes and not
/// per-reason identifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u8)]
pub enum TokenRefusal {
    /// Owning token program is not the one the policy names.
    WrongProgram = 1,
    /// Mint identity is not the one the policy names.
    WrongMint = 2,
    /// Mint or token account is not initialized.
    Uninitialized = 3,
    /// Mint decimals differ from the policy.
    WrongDecimals = 4,
    /// Supply is zero and the policy requires a positive supply.
    ZeroSupply = 5,
    /// Supply exceeds the policy's immutable ceiling.
    SupplyExceedsProfile = 6,
    /// A mint authority is present, absent, or not the expected one.
    MintAuthorityPresent = 7,
    /// A freeze authority remains and the policy requires none.
    FreezeAuthorityPresent = 8,
    /// An extension discriminant is not one this build knows.
    UnknownExtension = 9,
    /// An extension appeared on the wrong account kind.
    WrongExtensionLocation = 10,
    /// A known extension is outside the policy's allowed set.
    ExtensionNotAllowed = 11,
    /// A required extension is absent.
    RequiredExtensionMissing = 12,
    /// Token account is frozen.
    FrozenAccount = 13,
    /// Token account owner authority is not the expected one.
    WrongAccountOwner = 14,
    /// Token account carries a delegate and the policy requires none.
    DelegatePresent = 15,
    /// Token account carries a close authority and the policy requires none.
    CloseAuthorityPresent = 16,
    /// The base state or the TLV extension region did not parse.
    MalformedExtensionSet = 17,
}

impl TokenRefusal {
    /// The adapter refusal class this reason projects onto.
    ///
    /// Four classes, chosen so that a transaction log distinguishes the four
    /// things an operator would do differently about: the account is not the
    /// token program's; the asset is the wrong asset; the asset carries
    /// behaviour the Realm forbids; the *holder* account is unusable.
    pub const fn clutch_error(self, subject: Subject) -> ClutchError {
        match self {
            Self::WrongProgram => ClutchError::WrongTokenProgram,
            Self::UnknownExtension
            | Self::WrongExtensionLocation
            | Self::ExtensionNotAllowed
            | Self::RequiredExtensionMissing
            | Self::MalformedExtensionSet => ClutchError::TokenExtensionNotAllowed,
            _ => match subject {
                Subject::Mint => ClutchError::MintNotAdmitted,
                Subject::TokenAccount => ClutchError::TokenAccountNotAdmitted,
            },
        }
    }
}

/// Which kind of account a refusal was raised about.
///
/// The same `TokenRefusal` means different things on the two account kinds —
/// `WrongMint` on a *mint* says "this is not the asset the policy names", and
/// on a *token account* says "this account holds the wrong asset" — and the
/// four refusal classes `TOKEN2022_PLAN.md` §3.2 allocates split exactly along
/// that line.  Keeping the reason numbering identical to the Python model and
/// resolving the class from the subject is what lets both stay true.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Subject {
    /// The refusal was raised about a mint account.
    Mint,
    /// The refusal was raised about a token account.
    TokenAccount,
}

/// A refusal, plus the extension discriminant that caused it when one did.
///
/// The discriminant does not reach the runtime — one `ProgramError::Custom`
/// carries one `u32` — and is here so that this program's own tests, and the
/// svm-tests differential, can assert on the same specificity the probe has.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenFault {
    /// Why admission failed.
    pub code: TokenRefusal,
    /// The offending extension discriminant, when the refusal names one.
    pub extension: Option<u8>,
    /// Which account kind the refusal is about.
    pub subject: Subject,
}

impl TokenFault {
    const fn plain(code: TokenRefusal) -> Self {
        Self {
            code,
            extension: None,
            subject: Subject::Mint,
        }
    }

    const fn at(code: TokenRefusal, extension: u8) -> Self {
        Self {
            code,
            extension: Some(extension),
            subject: Subject::Mint,
        }
    }

    /// Restate this fault as one raised about a token account.
    ///
    /// The shared decoders (`tlv_region`, `walk_tlv`, the `COption` readers)
    /// do not know which account kind they were called for, so the subject is
    /// stamped once at the boundary of [`check_token_account`] rather than
    /// threaded through every helper.
    pub const fn on_account(self) -> Self {
        Self {
            subject: Subject::TokenAccount,
            ..self
        }
    }
}

impl From<TokenFault> for Refusal {
    fn from(value: TokenFault) -> Self {
        Self::Adapter(value.code.clutch_error(value.subject))
    }
}

/* ------------------------------------------------------------------------ */
/* Extension bitsets                                                         */
/* ------------------------------------------------------------------------ */

/// Bit position of `ImmutableOwner`, the one account extension V1 admits.
pub const EXT_IMMUTABLE_OWNER: u8 = 7;

/// Extension discriminants that live on a mint, as a bitset.
///
/// From `ExtensionType::get_account_type` in `spl-token-2022-interface` 3.1.1.
/// A mint extension found on a token account, or the other way round, is
/// [`TokenRefusal::WrongExtensionLocation`] — the probe's check, kept because
/// a policy that allows `ImmutableOwner` on accounts must not thereby admit it
/// on a mint.
pub const MINT_EXTENSIONS: u64 = (1 << 1)   // TransferFeeConfig
    | (1 << 3)   // MintCloseAuthority
    | (1 << 4)   // ConfidentialTransferMint
    | (1 << 6)   // DefaultAccountState
    | (1 << 9)   // NonTransferable
    | (1 << 10)  // InterestBearingConfig
    | (1 << 12)  // PermanentDelegate
    | (1 << 14)  // TransferHook
    | (1 << 16)  // ConfidentialTransferFeeConfig
    | (1 << 18)  // MetadataPointer
    | (1 << 19)  // TokenMetadata
    | (1 << 20)  // GroupPointer
    | (1 << 21)  // TokenGroup
    | (1 << 22)  // GroupMemberPointer
    | (1 << 23)  // TokenGroupMember
    | (1 << 24)  // ConfidentialMintBurn
    | (1 << 25)  // ScaledUiAmount
    | (1 << 26)  // Pausable
    | (1 << 28); // PermissionedBurn

/// Extension discriminants that live on a token account, as a bitset.
pub const ACCOUNT_EXTENSIONS: u64 = (1 << 2)   // TransferFeeAmount
    | (1 << 5)   // ConfidentialTransferAccount
    | (1 << 7)   // ImmutableOwner
    | (1 << 8)   // MemoTransfer
    | (1 << 11)  // CpiGuard
    | (1 << 13)  // NonTransferableAccount
    | (1 << 15)  // TransferHookAccount
    | (1 << 17)  // ConfidentialTransferFeeAmount
    | (1 << 27); // PausableAccount

/* The two location sets must partition every known discriminant except the
 * zero padding marker, or a discriminant would be silently unlocatable. */
const _: () = assert!(MINT_EXTENSIONS & ACCOUNT_EXTENSIONS == 0);
const _: () = assert!((MINT_EXTENSIONS | ACCOUNT_EXTENSIONS | 1) == EXTENSION_KNOWN_MASK);

/// The lowest-numbered extension present in `present` that `allowed` does not
/// admit.
///
/// Lowest first, so the reported discriminant is deterministic and matches the
/// Python model's `min(denied, key=int)` and the probe's `sort_by_key`.
pub const fn first_denied_extension(present: u64, allowed: u64) -> Option<u8> {
    let denied = present & !allowed;
    if denied == 0 {
        None
    } else {
        Some(denied.trailing_zeros() as u8)
    }
}

/// The lowest-numbered extension `required` demands and `present` lacks.
pub const fn first_missing_extension(present: u64, required: u64) -> Option<u8> {
    let missing = required & !present;
    if missing == 0 {
        None
    } else {
        Some(missing.trailing_zeros() as u8)
    }
}

/* ------------------------------------------------------------------------ */
/* Observations                                                              */
/* ------------------------------------------------------------------------ */

/// What a mint account's bytes say.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintObservation {
    /// Atom exponent as written by the token program.
    pub decimals: u8,
    /// Supply in atoms.
    pub supply: u64,
    /// Mint authority, if any.
    pub mint_authority: Option<[u8; 32]>,
    /// Freeze authority, if any.
    pub freeze_authority: Option<[u8; 32]>,
    /// Every extension discriminant present, as a bitset.
    pub extensions: u64,
}

/// What a token account's bytes say.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenAccountObservation {
    /// Mint this account is for.
    pub mint: [u8; 32],
    /// Owner authority.
    pub owner: [u8; 32],
    /// Balance in atoms, excluding anything an extension withheld.
    pub amount: u64,
    /// Whether the account is frozen.
    pub frozen: bool,
    /// Delegate, if any.
    pub delegate: Option<[u8; 32]>,
    /// Close authority, if any.
    pub close_authority: Option<[u8; 32]>,
    /// Every extension discriminant present, as a bitset.
    pub extensions: u64,
}

/// Base (extension-free) Token-2022 mint length.
pub const BASE_MINT_LEN: usize = 82;
/// Base (extension-free) Token-2022 token-account length.
pub const BASE_TOKEN_ACCOUNT_LEN: usize = 165;
/// Multisig length, which no extensible account may have.
const MULTISIG_LEN: usize = 355;
/// `AccountType::Mint`, written at offset 165 when a mint carries extensions.
const ACCOUNT_TYPE_MINT: u8 = 1;
/// `AccountType::Account`, written at offset 165 when an account carries them.
const ACCOUNT_TYPE_ACCOUNT: u8 = 2;

fn read_u64(data: &[u8], at: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&data[at..at + 8]);
    u64::from_le_bytes(bytes)
}

fn read_key(data: &[u8], at: usize) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&data[at..at + 32]);
    bytes
}

/// Decode a 36-byte `COption<Pubkey>`, refusing every tag SPL does not write.
fn read_coption_key(data: &[u8], at: usize) -> Result<Option<[u8; 32]>, TokenFault> {
    match &data[at..at + 4] {
        [0, 0, 0, 0] => Ok(None),
        [1, 0, 0, 0] => Ok(Some(read_key(data, at + 4))),
        _ => Err(TokenFault::plain(TokenRefusal::MalformedExtensionSet)),
    }
}

/// Walk the TLV region and return the set of extension discriminants present.
///
/// The loop is `try_for_each_tlv_extension_type` in `spl-token-2022-interface`
/// 3.1.1, refusal for refusal: a truncated type field ends the walk, a
/// `Uninitialized` type ends the walk, a truncated length or a value that runs
/// past the buffer is malformed, and a discriminant outside the known mask is
/// unknown rather than skipped.
fn walk_tlv(tlv: &[u8]) -> Result<u64, TokenFault> {
    let mut present = 0_u64;
    let mut start = 0_usize;
    while start < tlv.len() {
        let length_start = start + 2;
        if tlv.len() < length_start {
            /* Not enough bytes for the next type. The last byte can be left
             * over from a realloc, so this is the end of the walk and not a
             * fault -- exactly as the reference decoder treats it. */
            return Ok(present);
        }
        let discriminant = u16::from_le_bytes([tlv[start], tlv[start + 1]]);
        if discriminant == 0 {
            return Ok(present);
        }
        let value_start = length_start + 2;
        if tlv.len() < value_start {
            return Err(TokenFault::plain(TokenRefusal::MalformedExtensionSet));
        }
        let length = usize::from(u16::from_le_bytes([
            tlv[length_start],
            tlv[length_start + 1],
        ]));
        let value_end = value_start.saturating_add(length);
        if value_end > tlv.len() {
            return Err(TokenFault::plain(TokenRefusal::MalformedExtensionSet));
        }
        if u32::from(discriminant) >= u64::BITS
            || (1_u64 << discriminant) & EXTENSION_KNOWN_MASK == 0
        {
            /* A discriminant this build does not know. Fail closed: a future
             * Token-2022 release adding extension 29 must make this program
             * refuse, not shrug. */
            return Err(TokenFault::at(
                TokenRefusal::UnknownExtension,
                discriminant.min(255) as u8,
            ));
        }
        present |= 1_u64 << discriminant;
        start = value_end;
    }
    Ok(present)
}

/// Split an extensible account into its base slice and its TLV region.
///
/// `base_len` is 82 for a mint and 165 for a token account. The checks are
/// `check_min_len_and_not_multisig` followed by `type_and_tlv_indices` and
/// `check_account_type` from the reference decoder, in that order.
fn tlv_region(data: &[u8], base_len: usize, account_type: u8) -> Result<&[u8], TokenFault> {
    if data.len() == MULTISIG_LEN || data.len() < base_len {
        return Err(TokenFault::plain(TokenRefusal::MalformedExtensionSet));
    }
    if data.len() == base_len {
        return Ok(&[]);
    }
    let padding_len = BASE_TOKEN_ACCOUNT_LEN.saturating_sub(base_len);
    let type_index = base_len + padding_len;
    let tlv_start = type_index + 1;
    if data.len() < tlv_start {
        return Err(TokenFault::plain(TokenRefusal::MalformedExtensionSet));
    }
    if data[base_len..type_index].iter().any(|byte| *byte != 0) {
        return Err(TokenFault::plain(TokenRefusal::MalformedExtensionSet));
    }
    if data[type_index] != account_type {
        return Err(TokenFault::plain(TokenRefusal::MalformedExtensionSet));
    }
    Ok(&data[tlv_start..])
}

/// Decode a Token-2022 mint account's bytes.
#[inline(never)]
pub fn observe_mint(data: &[u8]) -> Result<MintObservation, TokenFault> {
    let tlv = tlv_region(data, BASE_MINT_LEN, ACCOUNT_TYPE_MINT)?;
    let is_initialized = match data[45] {
        0 => false,
        1 => true,
        _ => return Err(TokenFault::plain(TokenRefusal::MalformedExtensionSet)),
    };
    let mint_authority = read_coption_key(data, 0)?;
    let freeze_authority = read_coption_key(data, 46)?;
    if !is_initialized {
        return Err(TokenFault::plain(TokenRefusal::Uninitialized));
    }
    Ok(MintObservation {
        decimals: data[44],
        supply: read_u64(data, 36),
        mint_authority,
        freeze_authority,
        extensions: walk_tlv(tlv)?,
    })
}

/// Decode a Token-2022 token account's bytes.
#[inline(never)]
pub fn observe_token_account(data: &[u8]) -> Result<TokenAccountObservation, TokenFault> {
    let tlv = tlv_region(data, BASE_TOKEN_ACCOUNT_LEN, ACCOUNT_TYPE_ACCOUNT)?;
    let frozen = match data[108] {
        0 => return Err(TokenFault::plain(TokenRefusal::Uninitialized)),
        1 => false,
        2 => true,
        _ => return Err(TokenFault::plain(TokenRefusal::MalformedExtensionSet)),
    };
    /* `is_native` at 109..121 is a `COption<u64>`; its tag is validated for the
     * same reason the key options are -- a tag SPL never writes means these are
     * not the bytes the token program wrote. */
    match &data[109..113] {
        [0, 0, 0, 0] | [1, 0, 0, 0] => {}
        _ => return Err(TokenFault::plain(TokenRefusal::MalformedExtensionSet)),
    }
    Ok(TokenAccountObservation {
        mint: read_key(data, 0),
        owner: read_key(data, 32),
        amount: read_u64(data, 64),
        frozen,
        delegate: read_coption_key(data, 72)?,
        close_authority: read_coption_key(data, 129)?,
        extensions: walk_tlv(tlv)?,
    })
}

/* ------------------------------------------------------------------------ */
/* Policies                                                                  */
/* ------------------------------------------------------------------------ */

/// What a mint must be for this program to touch it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintPolicy {
    /// Token program that must own the mint account.
    pub token_program: Pubkey,
    /// Mint address the policy names.
    pub mint: Pubkey,
    /// Decimals the policy names.
    pub decimals: u8,
    /// Supply ceiling in atoms.
    pub max_supply_atoms: u64,
    /// Refuse a zero supply.
    pub require_nonzero_supply: bool,
    /// The mint authority that must be present, or `None` for "must be absent".
    ///
    /// A collateral mint must have none — a live mint authority means the
    /// collateral supply is not fixed. An outcome mint must have exactly the
    /// market PDA, because this program is the only thing that may create the
    /// liability it represents.
    pub mint_authority: Option<Pubkey>,
    /// Refuse a mint that still has a freeze authority.
    pub require_freeze_authority_none: bool,
    /// Mint extensions admitted, as a bitset.
    pub allowed_extensions: u64,
    /// Mint extensions required, as a bitset.
    pub required_extensions: u64,
}

impl MintPolicy {
    /// The outcome-mint policy — **PROPOSED**, `TOKEN2022_PLAN.md` §3.1.
    ///
    /// Decimals `0` because kernel quantities are integer complete-set counts
    /// and a nonzero exponent would introduce a UI-versus-atom distinction with
    /// no semantic content. Freeze authority `None` because a freeze authority
    /// on a claim token is discretionary seizure. Mint authority exactly
    /// `authority`, and no extension at all: admission by construction.
    ///
    /// No supply ceiling and no nonzero-supply requirement: a market that has
    /// materialized nothing has a zero-supply outcome mint, and that is the
    /// normal founding state rather than a fault.
    pub const fn outcome(mint: Pubkey, authority: Pubkey) -> Self {
        Self {
            token_program: TOKEN_2022_PROGRAM_ID,
            mint,
            decimals: 0,
            max_supply_atoms: u64::MAX,
            require_nonzero_supply: false,
            mint_authority: Some(authority),
            require_freeze_authority_none: true,
            allowed_extensions: 0,
            required_extensions: 0,
        }
    }

    /// The collateral-mint policy a frozen Realm collateral policy names.
    ///
    /// Every field is read out of the 266 policy bytes: this is the "consume
    /// the bitsets" half of `TOKEN2022_PLAN.md` §3.4, and nothing here is a
    /// second copy of the matrix. The policy's own `validate` has already
    /// refused a Realm bitset above the protocol ceiling, so the sets arriving
    /// here are effective sets.
    pub fn collateral(policy: &CollateralPolicy) -> Self {
        Self {
            token_program: Pubkey::new_from_array(policy.collateral.token_program),
            mint: Pubkey::new_from_array(policy.collateral.mint),
            decimals: policy.collateral.decimals,
            max_supply_atoms: policy.max_supply_atoms,
            require_nonzero_supply: policy.flags & FLAG_REQUIRE_NONZERO_SUPPLY != 0,
            mint_authority: None,
            require_freeze_authority_none: policy.flags & FLAG_REQUIRE_FREEZE_AUTHORITY_NONE != 0,
            allowed_extensions: policy.allowed_mint_extensions,
            required_extensions: policy.required_mint_extensions,
        }
    }
}

/// What a token account must be for this program to touch it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenAccountPolicy {
    /// Token program that must own the account.
    pub token_program: Pubkey,
    /// Mint the account must be for.
    pub mint: Pubkey,
    /// Owner authority the account must carry.
    pub expected_owner_authority: Pubkey,
    /// Refuse a delegate.
    pub require_delegate_none: bool,
    /// Refuse a close authority.
    pub require_close_authority_none: bool,
    /// Account extensions admitted, as a bitset.
    pub allowed_extensions: u64,
    /// Account extensions required, as a bitset.
    pub required_extensions: u64,
}

impl TokenAccountPolicy {
    /// A user's own outcome-token account — `TOKEN2022_PLAN.md` §3.6.
    ///
    /// Validated against mint and owner authority, **not** required to be an
    /// associated token account: requiring an ATA would refuse legitimate
    /// accounts while adding no property the mint-and-owner check does not
    /// already give. A delegate or a close authority on a user's own account
    /// is the user's business — neither can move a claim without the owner
    /// signature this instruction already demands — so neither is refused.
    /// `ImmutableOwner` is admitted because the protocol account ceiling
    /// admits it; nothing else is.
    pub const fn holder(mint: Pubkey, owner: Pubkey) -> Self {
        Self {
            token_program: TOKEN_2022_PROGRAM_ID,
            mint,
            expected_owner_authority: owner,
            require_delegate_none: false,
            require_close_authority_none: false,
            allowed_extensions: PROTOCOL_ACCOUNT_EXTENSION_CEILING,
            required_extensions: 0,
        }
    }

    /// A user's own **collateral** account, from a frozen Realm policy.
    ///
    /// The mint is the policy's, not the caller's: this is what stops a
    /// `Split` from funding complete sets with a token the Realm never
    /// admitted.  The owner authority is the authenticated actor, and the
    /// account is **not** required to be an associated token account —
    /// `TOKEN2022_PLAN.md` §3.6, which argues that requiring one would refuse
    /// legitimate accounts while adding no property the mint-and-owner check
    /// does not already give.
    ///
    /// A delegate or a close authority is the user's own business and is not
    /// refused, which is where this differs from [`TokenAccountPolicy::hoard`]:
    /// the Realm's `FLAG_REQUIRE_ACCOUNT_*_NONE` flags exist to keep a second
    /// exit out of the *Hoard*, and applying them to a wallet would refuse
    /// accounts whose delegate can take nothing this instruction gives it.
    /// The Realm's extension sets do apply, because an extension changes what
    /// a transfer *means* and the exact-delta check would otherwise be the
    /// only thing standing between the Hoard and a short credit.
    pub fn collateral_holder(policy: &CollateralPolicy, owner: Pubkey) -> Self {
        Self {
            token_program: Pubkey::new_from_array(policy.collateral.token_program),
            mint: Pubkey::new_from_array(policy.collateral.mint),
            expected_owner_authority: owner,
            require_delegate_none: false,
            require_close_authority_none: false,
            allowed_extensions: policy.allowed_account_extensions,
            required_extensions: policy.required_account_extensions,
        }
    }

    /// The Hoard's own collateral account, from a frozen Realm policy.
    ///
    /// Strict where the holder policy is permissive: a delegate or a close
    /// authority on the Hoard is a second way collateral leaves, which is the
    /// one thing the Hoard must not have.
    pub fn hoard(policy: &CollateralPolicy, authority: Pubkey) -> Self {
        Self {
            token_program: Pubkey::new_from_array(policy.collateral.token_program),
            mint: Pubkey::new_from_array(policy.collateral.mint),
            expected_owner_authority: authority,
            require_delegate_none: policy.flags & FLAG_REQUIRE_ACCOUNT_DELEGATE_NONE != 0,
            require_close_authority_none: policy.flags & FLAG_REQUIRE_ACCOUNT_CLOSE_AUTHORITY_NONE
                != 0,
            allowed_extensions: policy.allowed_account_extensions,
            required_extensions: policy.required_account_extensions,
        }
    }
}

fn check_extension_sets(
    present: u64,
    allowed: u64,
    required: u64,
    location: u64,
) -> Result<(), TokenFault> {
    if let Some(extension) = first_denied_extension(present, location) {
        return Err(TokenFault::at(
            TokenRefusal::WrongExtensionLocation,
            extension,
        ));
    }
    if let Some(extension) = first_denied_extension(present, allowed) {
        return Err(TokenFault::at(TokenRefusal::ExtensionNotAllowed, extension));
    }
    if let Some(extension) = first_missing_extension(present, required) {
        return Err(TokenFault::at(
            TokenRefusal::RequiredExtensionMissing,
            extension,
        ));
    }
    Ok(())
}

/// Admit or refuse a mint from its bytes.
///
/// `account_owner` is the *runtime* owner of the mint account, which is the
/// token program. It must come from `AccountInfo::owner`; a caller-asserted
/// value defeats the check, which is obligation 2 of
/// `docs/implementation/SOLANA_REFERENCE_ADAPTER.md`.
///
/// The order — program, identity, decode, decimals, supply, authorities,
/// extensions — is the probe's and the Python model's, so that a mint with
/// several faults reports the same one in all three.
#[inline(never)]
pub fn check_mint(
    account_owner: &Pubkey,
    mint_address: &Pubkey,
    data: &[u8],
    policy: &MintPolicy,
) -> Result<MintObservation, TokenFault> {
    if *account_owner != policy.token_program {
        return Err(TokenFault::plain(TokenRefusal::WrongProgram));
    }
    if *mint_address != policy.mint {
        return Err(TokenFault::plain(TokenRefusal::WrongMint));
    }
    let observation = observe_mint(data)?;
    if observation.decimals != policy.decimals {
        return Err(TokenFault::plain(TokenRefusal::WrongDecimals));
    }
    if policy.require_nonzero_supply && observation.supply == 0 {
        return Err(TokenFault::plain(TokenRefusal::ZeroSupply));
    }
    if observation.supply > policy.max_supply_atoms {
        return Err(TokenFault::plain(TokenRefusal::SupplyExceedsProfile));
    }
    let expected_authority = policy.mint_authority.map(|key| key.to_bytes());
    if observation.mint_authority != expected_authority {
        return Err(TokenFault::plain(TokenRefusal::MintAuthorityPresent));
    }
    if policy.require_freeze_authority_none && observation.freeze_authority.is_some() {
        return Err(TokenFault::plain(TokenRefusal::FreezeAuthorityPresent));
    }
    check_extension_sets(
        observation.extensions,
        policy.allowed_extensions,
        policy.required_extensions,
        MINT_EXTENSIONS,
    )?;
    Ok(observation)
}

/// Admit or refuse a token account from its bytes.
pub fn check_token_account(
    account_owner: &Pubkey,
    data: &[u8],
    policy: &TokenAccountPolicy,
) -> Result<TokenAccountObservation, TokenFault> {
    check_token_account_inner(account_owner, data, policy).map_err(TokenFault::on_account)
}

#[inline(never)]
fn check_token_account_inner(
    account_owner: &Pubkey,
    data: &[u8],
    policy: &TokenAccountPolicy,
) -> Result<TokenAccountObservation, TokenFault> {
    if *account_owner != policy.token_program {
        return Err(TokenFault::plain(TokenRefusal::WrongProgram));
    }
    let observation = observe_token_account(data)?;
    if observation.mint != policy.mint.to_bytes() {
        return Err(TokenFault::plain(TokenRefusal::WrongMint));
    }
    if observation.frozen {
        return Err(TokenFault::plain(TokenRefusal::FrozenAccount));
    }
    if observation.owner != policy.expected_owner_authority.to_bytes() {
        return Err(TokenFault::plain(TokenRefusal::WrongAccountOwner));
    }
    if policy.require_delegate_none && observation.delegate.is_some() {
        return Err(TokenFault::plain(TokenRefusal::DelegatePresent));
    }
    if policy.require_close_authority_none && observation.close_authority.is_some() {
        return Err(TokenFault::plain(TokenRefusal::CloseAuthorityPresent));
    }
    check_extension_sets(
        observation.extensions,
        policy.allowed_extensions,
        policy.required_extensions,
        ACCOUNT_EXTENSIONS,
    )?;
    Ok(observation)
}

/// Admit a mint held in an `AccountInfo`, dropping the data borrow before
/// returning.
///
/// A live `RefCell` borrow across an `invoke` is a runtime failure rather than
/// a lint, so no admission function in this module hands a borrow back to its
/// caller. The observation crossing the frame is 82 bytes; the account is not.
#[inline(never)]
pub fn admit_mint(
    account: &AccountInfo,
    policy: &MintPolicy,
) -> Result<MintObservation, TokenFault> {
    let data = account
        .try_borrow_data()
        .map_err(|_| TokenFault::plain(TokenRefusal::MalformedExtensionSet))?;
    check_mint(account.owner, account.key, &data, policy)
}

/// Admit a token account held in an `AccountInfo`, dropping the data borrow.
#[inline(never)]
pub fn admit_token_account(
    account: &AccountInfo,
    policy: &TokenAccountPolicy,
) -> Result<TokenAccountObservation, TokenFault> {
    let data = account
        .try_borrow_data()
        .map_err(|_| TokenFault::plain(TokenRefusal::MalformedExtensionSet).on_account())?;
    check_token_account(account.owner, &data, policy)
}

/// The `supply` field of a mint account, with no admission decision.
///
/// Used for the post-CPI half of the exact-delta check, where the mint was
/// already admitted before the CPI and re-admitting it would report a policy
/// fault where a delta fault is what happened.
#[inline(never)]
pub fn mint_supply(account: &AccountInfo) -> Result<u64, TokenFault> {
    Ok(observe_mint(
        &account
            .try_borrow_data()
            .map_err(|_| TokenFault::plain(TokenRefusal::MalformedExtensionSet))?,
    )?
    .supply)
}

/// The `amount` field of a token account, with no admission decision.
#[inline(never)]
pub fn token_amount(account: &AccountInfo) -> Result<u64, TokenFault> {
    let data = account
        .try_borrow_data()
        .map_err(|_| TokenFault::plain(TokenRefusal::MalformedExtensionSet).on_account())?;
    Ok(observe_token_account(&data)
        .map_err(TokenFault::on_account)?
        .amount)
}

/* ------------------------------------------------------------------------ */
/* Exact deltas                                                              */
/* ------------------------------------------------------------------------ */

/// Require that a balance or supply rose by *exactly* `quantity`.
///
/// Not `>=`, not "at least": `TOKEN2022_PLAN.md` §3.3 step 6. This is what
/// makes solvency independent of the extension refusal being complete — a
/// collateral mint whose transfer is not the identity is caught here even if
/// the matrix admitted it.
pub fn require_exact_credit(pre: u64, post: u64, quantity: u64) -> Result<(), Refusal> {
    match post.checked_sub(pre) {
        Some(observed) if observed == quantity => Ok(()),
        _ => Err(Refusal::Adapter(ClutchError::TokenDeltaMismatch)),
    }
}

/// Require that a balance or supply fell by *exactly* `quantity`.
pub fn require_exact_debit(pre: u64, post: u64, quantity: u64) -> Result<(), Refusal> {
    match pre.checked_sub(post) {
        Some(observed) if observed == quantity => Ok(()),
        _ => Err(Refusal::Adapter(ClutchError::TokenDeltaMismatch)),
    }
}

/* ------------------------------------------------------------------------ */
/* CPI construction                                                          */
/* ------------------------------------------------------------------------ */

/// `MintTo`, signed by a program address.
///
/// Outcome tokens can only be created by this program, so the authority is
/// always a PDA and the call is always `invoke_signed`. `MintTo` rather than
/// `MintToChecked` is deliberate: the decimals argument `MintToChecked` adds is
/// a guard against a caller confusing units, and this caller has already
/// refused any mint whose decimals are not the policy's.
#[inline(never)]
pub fn mint_to_signed<'a>(
    token_program: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    signer_seeds: &[&[u8]],
) -> Result<(), Refusal> {
    let mut data = [0u8; 9];
    data[0] = ix::MINT_TO;
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    let instruction = Instruction::new_with_bytes(
        TOKEN_2022_PROGRAM_ID,
        &data,
        vec![
            AccountMeta::new(*mint.key, false),
            AccountMeta::new(*destination.key, false),
            AccountMeta::new_readonly(*authority.key, true),
        ],
    );
    invoke_signed(
        &instruction,
        &[
            mint.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))
}

/// `Burn`, signed by the authenticated actor.
///
/// The signature the runtime already authenticated propagates into the CPI, so
/// no delegate and no approval step is required. This is the asymmetry the
/// probe measured: moving value *in* to program control needs only the user's
/// signature; moving it *out* is impossible without the program signing.
#[inline(never)]
pub fn burn<'a>(
    token_program: &AccountInfo<'a>,
    source: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
) -> Result<(), Refusal> {
    let mut data = [0u8; 9];
    data[0] = ix::BURN;
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    let instruction = Instruction::new_with_bytes(
        TOKEN_2022_PROGRAM_ID,
        &data,
        vec![
            AccountMeta::new(*source.key, false),
            AccountMeta::new(*mint.key, false),
            AccountMeta::new_readonly(*authority.key, true),
        ],
    );
    invoke(
        &instruction,
        &[
            source.clone(),
            mint.clone(),
            authority.clone(),
            token_program.clone(),
        ],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))
}

/// `TransferChecked` with the actor's propagated signature: collateral *in*.
///
/// Wired by [`crate::instructions::split`]'s `Split`: the actor's collateral
/// token account debits and the Hoard's credits, and the signature the runtime
/// already authenticated is the only authority the token program needs.
#[inline(never)]
pub fn transfer_checked<'a>(
    token_program: &AccountInfo<'a>,
    source: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    decimals: u8,
) -> Result<(), Refusal> {
    let instruction = transfer_checked_instruction(
        source.key,
        mint.key,
        destination.key,
        authority.key,
        amount,
        decimals,
    );
    invoke(
        &instruction,
        &[
            source.clone(),
            mint.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))
}

/// `TransferChecked` signed by a program address: collateral *out*.
///
/// Every outflow is this shape. The probe established there is no other: a
/// token account whose owner authority is a program address refuses a
/// user-signed transfer out with `TokenError::OwnerMismatch`.
///
/// Wired by `Merge` in [`crate::instructions::split`] and by
/// `RedeemInternal` in [`crate::instructions::observe_resolve`]; both sign for
/// [`crate::seeds::hoard_authority_pda`].
///
/// Eight parameters, one over clippy's default: the four accounts a
/// `TransferChecked` names, the token program that will be invoked, the two
/// values the instruction carries, and the seeds to sign with.  Grouping them
/// into a struct would move the same eight values one indirection away and buy
/// nothing but a `#[derive]`.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn transfer_checked_signed<'a>(
    token_program: &AccountInfo<'a>,
    source: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    decimals: u8,
    signer_seeds: &[&[u8]],
) -> Result<(), Refusal> {
    let instruction = transfer_checked_instruction(
        source.key,
        mint.key,
        destination.key,
        authority.key,
        amount,
        decimals,
    );
    invoke_signed(
        &instruction,
        &[
            source.clone(),
            mint.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::TokenDeltaMismatch))
}

/// The `TransferChecked` instruction, split out so both signer shapes and the
/// unit tests build one identical encoding.
pub fn transfer_checked_instruction(
    source: &Pubkey,
    mint: &Pubkey,
    destination: &Pubkey,
    authority: &Pubkey,
    amount: u64,
    decimals: u8,
) -> Instruction {
    let mut data = [0u8; 10];
    data[0] = ix::TRANSFER_CHECKED;
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    data[9] = decimals;
    Instruction::new_with_bytes(
        TOKEN_2022_PROGRAM_ID,
        &data,
        vec![
            AccountMeta::new(*source, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(*destination, false),
            AccountMeta::new_readonly(*authority, true),
        ],
    )
}

/// `InitializeMint2` for an outcome mint: decimals `0`, no freeze authority.
///
/// Emitted by `CreateMarket`, one per active outcome, after
/// [`create_account_signed`] has placed an 82-byte Token-2022-owned account at
/// [`crate::seeds::outcome_mint_pda`].
pub fn initialize_outcome_mint_instruction(mint: &Pubkey, authority: &Pubkey) -> Instruction {
    let mut data = [0u8; 67];
    data[0] = ix::INITIALIZE_MINT2;
    data[1] = 0; // decimals
    data[2..34].copy_from_slice(&authority.to_bytes());
    /* The freeze authority is a `COption<Pubkey>`; a zero tag is `None`, and
     * `None` at creation is the only setting that cannot be abused later. */
    data[34..38].copy_from_slice(&0u32.to_le_bytes());
    Instruction::new_with_bytes(
        TOKEN_2022_PROGRAM_ID,
        &data[..38],
        vec![AccountMeta::new(*mint, false)],
    )
}

/// `InitializeImmutableOwner`, which must precede `InitializeAccount3`.
pub fn initialize_immutable_owner_instruction(account: &Pubkey) -> Instruction {
    Instruction::new_with_bytes(
        TOKEN_2022_PROGRAM_ID,
        &[ix::INITIALIZE_IMMUTABLE_OWNER],
        vec![AccountMeta::new(*account, false)],
    )
}

/// `InitializeAccount3` for the Hoard token account.
pub fn initialize_account3_instruction(
    account: &Pubkey,
    mint: &Pubkey,
    owner: &Pubkey,
) -> Instruction {
    let mut data = [0u8; 33];
    data[0] = ix::INITIALIZE_ACCOUNT3;
    data[1..33].copy_from_slice(&owner.to_bytes());
    Instruction::new_with_bytes(
        TOKEN_2022_PROGRAM_ID,
        &data,
        vec![
            AccountMeta::new(*account, false),
            AccountMeta::new_readonly(*mint, false),
        ],
    )
}

/* ------------------------------------------------------------------------ */
/* Account creation                                                          */
/* ------------------------------------------------------------------------ */

/// The System program's address: thirty-two zero bytes.
///
/// Not re-typed from a base58 string.  `11111111111111111111111111111111` *is*
/// the all-zero address, and writing it as a literal is the one spelling that
/// cannot be mistyped.
pub const SYSTEM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0; 32]);

/// The Rent sysvar's address, `SysvarRent111111111111111111111111111111111`.
pub const RENT_SYSVAR_ID: Pubkey = Pubkey::new_from_array([
    0x06, 0xa7, 0xd5, 0x17, 0x19, 0x2c, 0x5c, 0x51, 0x21, 0x8c, 0xc9, 0x4c, 0x3d, 0x4a, 0xf1, 0x7f,
    0x58, 0xda, 0xee, 0x08, 0x9b, 0xa1, 0xfd, 0x44, 0xe3, 0xdb, 0xd9, 0x8a, 0x00, 0x00, 0x00, 0x00,
]);

/// `SystemInstruction::CreateAccount`, discriminant `0` of the bincode enum.
const SYSTEM_CREATE_ACCOUNT: u32 = 0;

/// Per-account storage the runtime charges rent for on top of the data.
///
/// `solana_rent::ACCOUNT_STORAGE_OVERHEAD`.  Named here rather than depended
/// on: `solana-rent` is not in this program's graph and adding a crate to read
/// one constant would be a worse trade than writing the constant down beside
/// the check that uses it.
const ACCOUNT_STORAGE_OVERHEAD: u64 = 128;

/// Exact byte length of an extension-free Token-2022 mint.
pub const MINT_ACCOUNT_LEN: usize = BASE_MINT_LEN;

/// Exact byte length of a Token-2022 account carrying only `ImmutableOwner`.
///
/// `ExtensionType::try_calculate_account_len::<Account>(&[ImmutableOwner])`:
/// the 165-byte base, the account-type byte at 165, and a four-byte TLV header
/// with a zero-length value.  `ImmutableOwner` carries no value, which is why
/// nothing follows the header.
pub const IMMUTABLE_OWNER_ACCOUNT_LEN: usize = BASE_TOKEN_ACCOUNT_LEN + 1 + 4;

/// The rent-exempt minimum for `space` bytes, read from the Rent sysvar.
///
/// The sysvar is presented as an account rather than fetched through
/// `sol_get_sysvar`: the syscall is `unsafe` and this crate's first-party code
/// is safe (see the crate docs), and an account whose address is compared
/// against [`RENT_SYSVAR_ID`] is exactly as trustworthy — the runtime is the
/// only writer of that address.
#[inline(never)]
pub fn rent_exempt_minimum(rent_account: &AccountInfo, space: usize) -> Result<u64, Refusal> {
    if *rent_account.key != RENT_SYSVAR_ID {
        return Err(Refusal::Adapter(ClutchError::WrongPda));
    }
    let data = rent_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    /* `Rent` is `{ lamports_per_byte_year: u64, exemption_threshold: f64,
     * burn_percent: u8 }`, bincode-serialized: seventeen bytes, little-endian,
     * no padding. */
    if data.len() < 17 {
        return Err(Refusal::Adapter(ClutchError::WrongDataLength));
    }
    let lamports_per_byte_year = read_u64(&data, 0);
    /* `exemption_threshold` is an `f64` and the multiply below is the *only*
     * floating-point arithmetic in this program.  It is here because
     * `solana_rent::Rent::minimum_balance` is
     * `((overhead + bytes) * lamports_per_byte_year) as f64 * threshold`, and
     * an integer approximation would be a second rent rule: this host's bank
     * serves `6960 / 1.0` where a cluster serves `3480 / 2.0`, so "the
     * threshold is always two" is false on the first bank this ran against.
     * Reimplementing the runtime's own expression is the only way the account
     * this program creates is rent-exempt by the runtime's definition rather
     * than by ours.  A threshold that is not a finite non-negative number is
     * refused rather than saturated, because `as u64` would silently make it
     * zero. */
    let threshold = f64::from_bits(read_u64(&data, 8));
    if !threshold.is_finite() || threshold < 0.0 {
        return Err(Refusal::Adapter(ClutchError::NonCanonical));
    }
    let year = (space as u64)
        .checked_add(ACCOUNT_STORAGE_OVERHEAD)
        .and_then(|bytes| bytes.checked_mul(lamports_per_byte_year))
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok((year as f64 * threshold) as u64)
}

/// `SystemInstruction::CreateAccount`, signed by the new account's own seeds.
///
/// Every account this program creates lives at a program-derived address, so
/// the new account's required signature is always this program's and the call
/// is always `invoke_signed`.  The payer's signature is the transaction's own
/// and propagates.
///
/// A failure here is reported as [`ClutchError::AlreadyInitialized`], and the
/// name is accurate for the reachable case: the caller of this function has
/// already refused an account that is not empty, so what remains is the
/// runtime's own "account already in use" — the same fault, observed one frame
/// down.  A payer that cannot fund the account reports it too, which is a
/// weaker diagnostic than it could be and is the cost of one `u32` per
/// refusal.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub fn create_account_signed<'a>(
    system_program: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    new_account: &AccountInfo<'a>,
    lamports: u64,
    space: u64,
    owner: &Pubkey,
    signer_seeds: &[&[u8]],
) -> Result<(), Refusal> {
    let mut data = [0u8; 52];
    data[0..4].copy_from_slice(&SYSTEM_CREATE_ACCOUNT.to_le_bytes());
    data[4..12].copy_from_slice(&lamports.to_le_bytes());
    data[12..20].copy_from_slice(&space.to_le_bytes());
    data[20..52].copy_from_slice(&owner.to_bytes());
    let instruction = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &data,
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*new_account.key, true),
        ],
    );
    invoke_signed(
        &instruction,
        &[payer.clone(), new_account.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AlreadyInitialized))
}

/// `InitializeMint2` over an account this program just created.
///
/// No signature: `InitializeMint2` authenticates nothing, which is why the
/// account's *creation* is the authenticated step and this is only the shape.
/// The mint that results is re-admitted by [`MintPolicy::outcome`] afterwards,
/// so a token program that wrote something else is caught by the policy rather
/// than trusted here.
#[inline(never)]
pub fn initialize_outcome_mint<'a>(
    token_program: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    authority: &Pubkey,
) -> Result<(), Refusal> {
    invoke(
        &initialize_outcome_mint_instruction(mint.key, authority),
        &[mint.clone(), token_program.clone()],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MintNotAdmitted))
}

/// `InitializeImmutableOwner`, which must precede `InitializeAccount3`.
#[inline(never)]
pub fn initialize_immutable_owner<'a>(
    token_program: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
) -> Result<(), Refusal> {
    invoke(
        &initialize_immutable_owner_instruction(account.key),
        &[account.clone(), token_program.clone()],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::TokenAccountNotAdmitted))
}

/// `InitializeAccount3` over an account this program just created.
#[inline(never)]
pub fn initialize_account3<'a>(
    token_program: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    owner: &Pubkey,
) -> Result<(), Refusal> {
    invoke(
        &initialize_account3_instruction(account.key, mint.key, owner),
        &[account.clone(), mint.clone(), token_program.clone()],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::TokenAccountNotAdmitted))
}

/* ------------------------------------------------------------------------ */
/* The collateral mirror                                                     */
/* ------------------------------------------------------------------------ */

/// Require `HoardAccount::collateral_atoms` to equal the Hoard token account's
/// `amount`.
///
/// The checked-mirror half of `TOKEN2022_PLAN.md` open decision 3, §3.5.  The
/// field is in a frozen layout and cannot be deleted by this lane, so instead
/// of carrying two truths silently the two are required to agree — which makes
/// the eventual cutover a *deletion* rather than a change of semantics, and
/// which is the strongest available statement that the CPI did what the kernel
/// thought it did.
///
/// Checked twice per collateral instruction and both times deliberately: once
/// over the pre-state, so a market whose two truths already disagree refuses
/// before anything moves, and once after the CPI, which is the load-bearing
/// one — it composes the kernel's arithmetic with the token program's and
/// requires the composition to be the identity.
pub fn require_hoard_mirror(collateral_atoms: u64, token_amount: u64) -> Result<(), Refusal> {
    if collateral_atoms == token_amount {
        Ok(())
    } else {
        Err(Refusal::Adapter(ClutchError::HoardMirrorMismatch))
    }
}

/* ------------------------------------------------------------------------ */
/* Collateral-policy admission                                               */
/* ------------------------------------------------------------------------ */

/// Refuse a Realm collateral policy this adapter cannot act on.
///
/// V1's `CurrencyRef` admits two token programs and this adapter invokes one.
/// A policy naming the legacy SPL Token program is well-formed and decodes;
/// it is simply outside what this program can drive, and saying so as
/// [`ClutchError::WrongTokenProgram`] is more useful than discovering it at
/// the first CPI.
pub fn require_drivable_collateral(policy: &CollateralPolicy) -> Result<(), Refusal> {
    if !matches!(policy.collateral.kind, CurrencyKind::SplToken)
        || policy.collateral.token_program != TOKEN_2022_PROGRAM
    {
        return Err(Refusal::Adapter(ClutchError::WrongTokenProgram));
    }
    /* [`MintPolicy::collateral`] encodes "the mint authority must be absent" as
     * `mint_authority: None`, which is the *only* thing that `Option` can say.
     * `COLLATERAL_POLICY_STRICT_FLAGS` forces the flag on and
     * `CollateralPolicy::validate` refuses any other flag word, so this is
     * unreachable through a decoded policy -- and it is checked anyway, because
     * a policy constructed in memory rather than decoded would otherwise get a
     * requirement it did not ask for. */
    if policy.flags & FLAG_REQUIRE_MINT_AUTHORITY_NONE == 0 {
        return Err(Refusal::Adapter(ClutchError::MintNotAdmitted));
    }
    Ok(())
}

/// Byte images a real Token-2022 program would have written.
///
/// Crate-visible rather than private to this module's tests because
/// [`crate::instructions::split`]'s host differential needs the same bytes to
/// stand up the optional token leg, and two hand-rolled copies of a frozen
/// external layout is exactly how they drift apart.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::{ACCOUNT_TYPE_MINT, BASE_MINT_LEN, BASE_TOKEN_ACCOUNT_LEN};

    /// A base mint, exactly as the token program writes one.
    pub(crate) fn mint_bytes(
        decimals: u8,
        supply: u64,
        mint_authority: Option<[u8; 32]>,
        freeze_authority: Option<[u8; 32]>,
    ) -> Vec<u8> {
        let mut data = vec![0u8; BASE_MINT_LEN];
        if let Some(authority) = mint_authority {
            data[0..4].copy_from_slice(&1u32.to_le_bytes());
            data[4..36].copy_from_slice(&authority);
        }
        data[36..44].copy_from_slice(&supply.to_le_bytes());
        data[44] = decimals;
        data[45] = 1;
        if let Some(authority) = freeze_authority {
            data[46..50].copy_from_slice(&1u32.to_le_bytes());
            data[50..82].copy_from_slice(&authority);
        }
        data
    }

    /// A base token account, exactly as the token program writes one.
    pub(crate) fn account_bytes(mint: [u8; 32], owner: [u8; 32], amount: u64) -> Vec<u8> {
        let mut data = vec![0u8; BASE_TOKEN_ACCOUNT_LEN];
        data[0..32].copy_from_slice(&mint);
        data[32..64].copy_from_slice(&owner);
        data[64..72].copy_from_slice(&amount.to_le_bytes());
        data[108] = 1;
        data
    }

    /// Re-shape base bytes into an extended account carrying one TLV entry.
    ///
    /// The padding, the account-type byte at offset 165, and the four-byte
    /// TLV header are laid out exactly where `spl-token-2022-interface` puts
    /// them; a mint therefore grows to 170 bytes and not to 86.
    pub(crate) fn with_extension(
        base: &[u8],
        base_len: usize,
        account_type: u8,
        discriminant: u16,
    ) -> Vec<u8> {
        let mut data = vec![0u8; BASE_TOKEN_ACCOUNT_LEN + 1 + 4];
        data[..base_len].copy_from_slice(&base[..base_len]);
        data[BASE_TOKEN_ACCOUNT_LEN] = account_type;
        data[166..168].copy_from_slice(&discriminant.to_le_bytes());
        data[168..170].copy_from_slice(&0u16.to_le_bytes());
        data
    }

    /// A well-formed outcome mint: decimals `0`, `authority` as mint
    /// authority, no freeze authority, no extensions.
    pub(crate) fn outcome_mint_bytes(authority: [u8; 32], supply: u64) -> Vec<u8> {
        mint_bytes(0, supply, Some(authority), None)
    }

    /// The `supply` field of mint bytes, for a test that asserts a move.
    pub(crate) fn supply_of(mint: &[u8]) -> u64 {
        super::read_u64(mint, 36)
    }

    /// The `amount` field of token-account bytes.
    pub(crate) fn amount_of(account: &[u8]) -> u64 {
        super::read_u64(account, 64)
    }

    /// A mint carrying exactly one extension discriminant.
    pub(crate) fn mint_with_extension(
        authority: [u8; 32],
        supply: u64,
        discriminant: u16,
    ) -> Vec<u8> {
        with_extension(
            &mint_bytes(0, supply, Some(authority), None),
            BASE_MINT_LEN,
            ACCOUNT_TYPE_MINT,
            discriminant,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::collateral::{
        CurrencyRef, COLLATERAL_POLICY_SCHEMA, COLLATERAL_POLICY_STRICT_FLAGS,
        EXTENSION_IMMUTABLE_OWNER, LEGACY_TOKEN_PROGRAM,
    };

    use fixtures::{account_bytes, mint_bytes, with_extension};

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn v1_policy(mint: [u8; 32], decimals: u8) -> CollateralPolicy {
        CollateralPolicy {
            schema_version: COLLATERAL_POLICY_SCHEMA,
            flags: COLLATERAL_POLICY_STRICT_FLAGS,
            collateral: CurrencyRef::spl(TOKEN_2022_PROGRAM, mint, decimals),
            fee: CurrencyRef::NATIVE_SOL,
            liveness: CurrencyRef::NATIVE_SOL,
            max_supply_atoms: 1_000_000_000_000_000,
            allowed_mint_extensions: 0,
            required_mint_extensions: 0,
            allowed_account_extensions: EXTENSION_IMMUTABLE_OWNER,
            required_account_extensions: 0,
        }
    }

    #[test]
    fn a_base_mint_decodes_to_exactly_what_the_token_program_wrote() {
        let observation = observe_mint(&mint_bytes(6, 5_000_000, None, None)).expect("decodes");
        assert_eq!(observation.decimals, 6);
        assert_eq!(observation.supply, 5_000_000);
        assert_eq!(observation.mint_authority, None);
        assert_eq!(observation.freeze_authority, None);
        assert_eq!(observation.extensions, 0);

        let authority = [9u8; 32];
        let observation =
            observe_mint(&mint_bytes(0, 0, Some(authority), Some([3u8; 32]))).expect("decodes");
        assert_eq!(observation.mint_authority, Some(authority));
        assert_eq!(observation.freeze_authority, Some([3u8; 32]));
    }

    #[test]
    fn an_uninitialized_or_malformed_mint_refuses_rather_than_decoding() {
        let mut data = mint_bytes(6, 1, None, None);
        data[45] = 0;
        assert_eq!(
            observe_mint(&data).unwrap_err().code,
            TokenRefusal::Uninitialized
        );

        let mut data = mint_bytes(6, 1, None, None);
        data[45] = 2;
        assert_eq!(
            observe_mint(&data).unwrap_err().code,
            TokenRefusal::MalformedExtensionSet
        );

        // A COption tag SPL never writes.
        let mut data = mint_bytes(6, 1, None, None);
        data[0] = 2;
        assert_eq!(
            observe_mint(&data).unwrap_err().code,
            TokenRefusal::MalformedExtensionSet
        );

        // Too short, and exactly Multisig::LEN: both refused by the reference
        // decoder and both refused here.
        assert_eq!(
            observe_mint(&[0u8; 40]).unwrap_err().code,
            TokenRefusal::MalformedExtensionSet
        );
        let mut multisig = mint_bytes(6, 1, None, None);
        multisig.resize(MULTISIG_LEN, 0);
        assert_eq!(
            observe_mint(&multisig).unwrap_err().code,
            TokenRefusal::MalformedExtensionSet
        );
        /* A mint between 83 and 165 bytes cannot carry an account-type byte and
         * is malformed rather than treated as a base mint with slack. */
        let mut short = mint_bytes(6, 1, None, None);
        short.resize(120, 0);
        assert_eq!(
            observe_mint(&short).unwrap_err().code,
            TokenRefusal::MalformedExtensionSet
        );
    }

    #[test]
    fn the_tlv_walk_reports_the_extensions_present() {
        let base = mint_bytes(6, 1, None, None);
        // Row 1 of the V1 matrix: TransferFeeConfig, discriminant 1.
        let data = with_extension(&base, BASE_MINT_LEN, ACCOUNT_TYPE_MINT, 1);
        let observation = observe_mint(&data).expect("decodes");
        assert_eq!(observation.extensions, 1 << 1);
        assert_eq!(observation.supply, 1);

        // Row 3: MintCloseAuthority, discriminant 3.
        let data = with_extension(&base, BASE_MINT_LEN, ACCOUNT_TYPE_MINT, 3);
        assert_eq!(observe_mint(&data).expect("decodes").extensions, 1 << 3);
    }

    #[test]
    fn an_unknown_extension_discriminant_fails_closed() {
        let base = mint_bytes(6, 1, None, None);
        /* `EXTENSION_DISCRIMINANTS` is 29, so 29 is the first discriminant a
         * future Token-2022 release would add.  It must refuse, not shrug. */
        let data = with_extension(&base, BASE_MINT_LEN, ACCOUNT_TYPE_MINT, 29);
        let fault = observe_mint(&data).unwrap_err();
        assert_eq!(fault.code, TokenRefusal::UnknownExtension);
        assert_eq!(fault.extension, Some(29));
        assert_eq!(
            fault.code.clutch_error(fault.subject),
            ClutchError::TokenExtensionNotAllowed
        );

        // And a wildly out-of-range one, which must not shift a u64 by 300.
        let data = with_extension(&base, BASE_MINT_LEN, ACCOUNT_TYPE_MINT, 300);
        assert_eq!(
            observe_mint(&data).unwrap_err().code,
            TokenRefusal::UnknownExtension
        );
    }

    #[test]
    fn a_wrong_account_type_byte_refuses() {
        let base = mint_bytes(6, 1, None, None);
        // Account type 2 on a mint: the bytes claim to be a token account.
        let data = with_extension(&base, BASE_MINT_LEN, ACCOUNT_TYPE_ACCOUNT, 1);
        assert_eq!(
            observe_mint(&data).unwrap_err().code,
            TokenRefusal::MalformedExtensionSet
        );
        // Nonzero padding between the mint body and the account type byte.
        let mut data = with_extension(&base, BASE_MINT_LEN, ACCOUNT_TYPE_MINT, 1);
        data[100] = 0xff;
        assert_eq!(
            observe_mint(&data).unwrap_err().code,
            TokenRefusal::MalformedExtensionSet
        );
    }

    #[test]
    fn the_outcome_mint_policy_admits_exactly_the_shape_the_plan_proposes() {
        let mint = key(0x11);
        let authority = key(0x22);
        let policy = MintPolicy::outcome(mint, authority);

        // Decimals 0, mint authority the market PDA, no freeze authority.
        let data = mint_bytes(0, 0, Some(authority.to_bytes()), None);
        let observation =
            check_mint(&TOKEN_2022_PROGRAM_ID, &mint, &data, &policy).expect("admitted");
        assert_eq!(observation.supply, 0, "a founding outcome mint is empty");

        // Nonzero decimals: refused.
        let data = mint_bytes(6, 0, Some(authority.to_bytes()), None);
        assert_eq!(
            check_mint(&TOKEN_2022_PROGRAM_ID, &mint, &data, &policy)
                .unwrap_err()
                .code,
            TokenRefusal::WrongDecimals
        );

        // A freeze authority on a claim token is discretionary seizure.
        let data = mint_bytes(0, 0, Some(authority.to_bytes()), Some([7u8; 32]));
        assert_eq!(
            check_mint(&TOKEN_2022_PROGRAM_ID, &mint, &data, &policy)
                .unwrap_err()
                .code,
            TokenRefusal::FreezeAuthorityPresent
        );

        // Somebody else's mint authority: this is not our liability token.
        let data = mint_bytes(0, 0, Some([0x99u8; 32]), None);
        assert_eq!(
            check_mint(&TOKEN_2022_PROGRAM_ID, &mint, &data, &policy)
                .unwrap_err()
                .code,
            TokenRefusal::MintAuthorityPresent
        );
        // And no mint authority at all is equally wrong: nothing could mint.
        let data = mint_bytes(0, 0, None, None);
        assert_eq!(
            check_mint(&TOKEN_2022_PROGRAM_ID, &mint, &data, &policy)
                .unwrap_err()
                .code,
            TokenRefusal::MintAuthorityPresent
        );

        // Right bytes, wrong owner program, and right bytes at another address.
        let data = mint_bytes(0, 0, Some(authority.to_bytes()), None);
        assert_eq!(
            check_mint(
                &Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM),
                &mint,
                &data,
                &policy
            )
            .unwrap_err()
            .code,
            TokenRefusal::WrongProgram
        );
        assert_eq!(
            check_mint(&TOKEN_2022_PROGRAM_ID, &key(0x33), &data, &policy)
                .unwrap_err()
                .code,
            TokenRefusal::WrongMint
        );
    }

    #[test]
    fn the_collateral_policy_bitsets_decide_admission() {
        let mint = [0x44u8; 32];
        let policy = v1_policy(mint, 6);
        let mint_policy = MintPolicy::collateral(&policy);
        assert_eq!(mint_policy.allowed_extensions, 0);
        assert!(mint_policy.require_nonzero_supply);
        assert_eq!(mint_policy.mint_authority, None);

        let address = Pubkey::new_from_array(mint);
        let base = mint_bytes(6, 5_000_000, None, None);
        check_mint(&TOKEN_2022_PROGRAM_ID, &address, &base, &mint_policy).expect("admitted");

        // A zero-supply collateral mint is refused before extensions are read.
        let empty = mint_bytes(6, 0, None, None);
        assert_eq!(
            check_mint(&TOKEN_2022_PROGRAM_ID, &address, &empty, &mint_policy)
                .unwrap_err()
                .code,
            TokenRefusal::ZeroSupply
        );

        // Row 1: TransferFeeConfig. The probe measured what this prevents.
        let fee = with_extension(&base, BASE_MINT_LEN, ACCOUNT_TYPE_MINT, 1);
        let fault = check_mint(&TOKEN_2022_PROGRAM_ID, &address, &fee, &mint_policy).unwrap_err();
        assert_eq!(fault.code, TokenRefusal::ExtensionNotAllowed);
        assert_eq!(fault.extension, Some(1));
        assert_eq!(
            fault.code.clutch_error(fault.subject),
            ClutchError::TokenExtensionNotAllowed
        );

        /* Falsifiability, the probe's sixth test in one line: the same bytes
         * under a counterfactual profile that admits row 1 are admitted.  V1
         * forbids that profile -- `CollateralPolicy::validate` refuses a Realm
         * bitset above the empty protocol mint ceiling -- so this cannot be
         * reached through a legal policy, and that is the point. */
        let mut widened = mint_policy;
        widened.allowed_extensions = 1 << 1;
        check_mint(&TOKEN_2022_PROGRAM_ID, &address, &fee, &widened)
            .expect("the widened profile admits the fee mint");
    }

    #[test]
    fn an_extension_on_the_wrong_account_kind_refuses() {
        let mint = [0x44u8; 32];
        let policy = v1_policy(mint, 6);
        let address = Pubkey::new_from_array(mint);
        let base = mint_bytes(6, 5_000_000, None, None);
        /* `ImmutableOwner` (7) is an *account* extension.  A Realm may allow it
         * on accounts -- V1 does -- and it must still be refused on a mint, or
         * the account bitset would leak into the mint decision. */
        let wrong = with_extension(&base, BASE_MINT_LEN, ACCOUNT_TYPE_MINT, 7);
        let mut leaky = MintPolicy::collateral(&policy);
        leaky.allowed_extensions = EXTENSION_IMMUTABLE_OWNER;
        let fault = check_mint(&TOKEN_2022_PROGRAM_ID, &address, &wrong, &leaky).unwrap_err();
        assert_eq!(fault.code, TokenRefusal::WrongExtensionLocation);
        assert_eq!(fault.extension, Some(EXT_IMMUTABLE_OWNER));
    }

    #[test]
    fn a_holder_account_is_bound_to_its_mint_and_its_owner() {
        let mint = key(0x55);
        let owner = key(0x66);
        let policy = TokenAccountPolicy::holder(mint, owner);
        let data = account_bytes(mint.to_bytes(), owner.to_bytes(), 1_234);
        let observation =
            check_token_account(&TOKEN_2022_PROGRAM_ID, &data, &policy).expect("admitted");
        assert_eq!(observation.amount, 1_234);
        assert!(!observation.frozen);

        // Another mint's account at the same owner.
        let data = account_bytes(key(0x77).to_bytes(), owner.to_bytes(), 1);
        assert_eq!(
            check_token_account(&TOKEN_2022_PROGRAM_ID, &data, &policy)
                .unwrap_err()
                .code,
            TokenRefusal::WrongMint
        );

        // Somebody else's account for the right mint.
        let data = account_bytes(mint.to_bytes(), key(0x88).to_bytes(), 1);
        assert_eq!(
            check_token_account(&TOKEN_2022_PROGRAM_ID, &data, &policy)
                .unwrap_err()
                .code,
            TokenRefusal::WrongAccountOwner
        );

        // Frozen.
        let mut data = account_bytes(mint.to_bytes(), owner.to_bytes(), 1);
        data[108] = 2;
        assert_eq!(
            check_token_account(&TOKEN_2022_PROGRAM_ID, &data, &policy)
                .unwrap_err()
                .code,
            TokenRefusal::FrozenAccount
        );

        // `ImmutableOwner` is admitted on a holder account; row 2 is not.
        let base = account_bytes(mint.to_bytes(), owner.to_bytes(), 1);
        let immutable = with_extension(
            &base,
            BASE_TOKEN_ACCOUNT_LEN,
            ACCOUNT_TYPE_ACCOUNT,
            u16::from(EXT_IMMUTABLE_OWNER),
        );
        check_token_account(&TOKEN_2022_PROGRAM_ID, &immutable, &policy)
            .expect("ImmutableOwner is inside the protocol account ceiling");
        let withheld = with_extension(&base, BASE_TOKEN_ACCOUNT_LEN, ACCOUNT_TYPE_ACCOUNT, 2);
        let fault = check_token_account(&TOKEN_2022_PROGRAM_ID, &withheld, &policy).unwrap_err();
        assert_eq!(fault.code, TokenRefusal::ExtensionNotAllowed);
        assert_eq!(fault.extension, Some(2));
    }

    #[test]
    fn the_hoard_policy_is_strict_where_the_holder_policy_is_not() {
        let mint = [0x99u8; 32];
        let policy = v1_policy(mint, 6);
        let authority = key(0xaa);
        let hoard = TokenAccountPolicy::hoard(&policy, authority);
        assert!(hoard.require_delegate_none);
        assert!(hoard.require_close_authority_none);
        assert!(
            !TokenAccountPolicy::holder(Pubkey::new_from_array(mint), authority)
                .require_delegate_none
        );

        let mut data = account_bytes(mint, authority.to_bytes(), 600_000);
        check_token_account(&TOKEN_2022_PROGRAM_ID, &data, &hoard).expect("admitted");

        // A delegate on the Hoard is a second way collateral leaves.
        data[72..76].copy_from_slice(&1u32.to_le_bytes());
        data[76..108].copy_from_slice(&[0xbb; 32]);
        assert_eq!(
            check_token_account(&TOKEN_2022_PROGRAM_ID, &data, &hoard)
                .unwrap_err()
                .code,
            TokenRefusal::DelegatePresent
        );
    }

    #[test]
    fn exact_deltas_refuse_everything_but_the_expected_move() {
        require_exact_credit(100, 107, 7).expect("exactly seven");
        require_exact_debit(100, 93, 7).expect("exactly seven");
        for (pre, post) in [(100u64, 108u64), (100, 106), (100, 100), (100, 93)] {
            assert_eq!(
                require_exact_credit(pre, post, 7).unwrap_err(),
                Refusal::Adapter(ClutchError::TokenDeltaMismatch)
            );
        }
        /* The off-chain `invoke_signed` no-op, stated as a property: nothing
         * moved, so the observed delta is zero and the check refuses. */
        assert_eq!(
            require_exact_credit(100, 100, 7).unwrap_err(),
            Refusal::Adapter(ClutchError::TokenDeltaMismatch)
        );
    }

    #[test]
    fn the_emitted_instructions_are_the_bytes_token_2022_expects() {
        let source = key(1);
        let mint = key(2);
        let destination = key(3);
        let authority = key(4);

        let transfer =
            transfer_checked_instruction(&source, &mint, &destination, &authority, 4_242, 6);
        assert_eq!(transfer.program_id, TOKEN_2022_PROGRAM_ID);
        assert_eq!(transfer.data[0], 12);
        assert_eq!(&transfer.data[1..9], &4_242u64.to_le_bytes());
        assert_eq!(transfer.data[9], 6);
        assert_eq!(transfer.data.len(), 10);
        assert_eq!(transfer.accounts.len(), 4);
        assert!(transfer.accounts[0].is_writable && !transfer.accounts[0].is_signer);
        assert!(!transfer.accounts[1].is_writable, "the mint is read-only");
        assert!(transfer.accounts[2].is_writable);
        assert!(transfer.accounts[3].is_signer && !transfer.accounts[3].is_writable);

        let initialize = initialize_outcome_mint_instruction(&mint, &authority);
        assert_eq!(initialize.data[0], 20);
        assert_eq!(initialize.data[1], 0, "outcome mints have decimals 0");
        assert_eq!(&initialize.data[2..34], &authority.to_bytes());
        assert_eq!(
            &initialize.data[34..38],
            &[0, 0, 0, 0],
            "freeze authority None"
        );
        assert_eq!(initialize.data.len(), 38);

        let immutable = initialize_immutable_owner_instruction(&destination);
        assert_eq!(immutable.data, vec![22]);
        let account3 = initialize_account3_instruction(&destination, &mint, &authority);
        assert_eq!(account3.data[0], 18);
        assert_eq!(&account3.data[1..33], &authority.to_bytes());
    }

    #[test]
    fn a_policy_this_adapter_cannot_drive_is_refused_up_front() {
        let mint = [0x44u8; 32];
        require_drivable_collateral(&v1_policy(mint, 6))
            .expect("token-2022 collateral is drivable");
        let mut legacy = v1_policy(mint, 6);
        legacy.collateral = CurrencyRef::spl(LEGACY_TOKEN_PROGRAM, mint, 6);
        assert_eq!(
            require_drivable_collateral(&legacy).unwrap_err(),
            Refusal::Adapter(ClutchError::WrongTokenProgram)
        );
    }

    #[test]
    fn the_location_bitsets_agree_with_the_frozen_layout_mask() {
        assert_eq!(
            MINT_EXTENSIONS | ACCOUNT_EXTENSIONS | 1,
            EXTENSION_KNOWN_MASK
        );
        assert_eq!(
            ACCOUNT_EXTENSIONS & EXTENSION_IMMUTABLE_OWNER,
            EXTENSION_IMMUTABLE_OWNER
        );
        assert_eq!(1u64 << EXT_IMMUTABLE_OWNER, EXTENSION_IMMUTABLE_OWNER);
        assert_eq!(first_denied_extension(0b1010, 0b0010), Some(3));
        assert_eq!(first_denied_extension(0b0010, 0b1111), None);
        assert_eq!(first_missing_extension(0b0010, 0b1010), Some(3));
    }
}
