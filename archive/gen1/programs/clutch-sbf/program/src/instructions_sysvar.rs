//! Pinned Instructions-sysvar decode for the immediate-post adjacency join.
//!
//! ## Why this exists
//!
//! A pull oracle writes a price into a caller-created ephemeral account and
//! then the same transaction consumes it.  Nothing about that account's
//! *contents* proves who wrote it, so the R2 profile binds the consumption to
//! the transaction's instruction history instead: the update must have been
//! written by the immediately preceding instruction, executed by the pinned
//! receiver program, naming the pinned Config, that exact update account, and
//! the write authority the update body itself records.  See
//! `docs/implementation/PYTH_PULL_PROFILE_R2.md` §"Authentication seam" and
//! `research/source-profile-v1/src/auth_v2.rs`, whose `ImmediatePostV1` this
//! module originally reproduced field-for-field as [`ImmediatePostV1`].  The
//! routed adapter now strengthens that projection with [`PostAbiV2`]: the
//! reviewed discriminator, exact account count, and every account flag are
//! authenticated before the three semantic account roles are projected.
//!
//! That research contract is explicit that the projection "must never be
//! accepted from caller instruction data".  This module is what makes that
//! possible: before it, nothing in `programs/` or `crates/` read the
//! Instructions sysvar at all.
//!
//! ## What this module is not
//!
//! **Nothing here is reachable from [`crate::dispatch`] or from any
//! instruction family.**  It is a capability module with tests.  Adding the
//! Instructions sysvar to the `InitSourceSpec` / Append / Seal account lists is
//! `docs/implementation/R2_PULL_PROMOTION_PLAN.md` P0.5; projecting these
//! refusals onto stable numeric codes is the open P0.8 decision.  Until both
//! close, [`InstructionsSysvarError`] is a module-local vocabulary in the style
//! of [`crate::source::SourceError`], with no entry in [`crate::error`].
//!
//! This module also does not decide *which* account meta of the post
//! instruction is the Config, the update, or the write authority.  That is the
//! reviewed receiver-post ABI, and it belongs to the compiled release triple
//! (R2 plan P0.9), so it arrives here as [`PostAbiV2`].
//!
//! ## Primary source
//!
//! Layout and semantics read from `solana-instructions-sysvar 3.0.1`,
//! `src/lib.rs` — the "Instructions memory layout" comment above
//! `serialize_instructions`, plus `deserialize_instruction` and
//! `load_current_index`.  The layout comment and `deserialize_instruction` are
//! byte-identical in 2.2.2, 3.0.1, and 4.0.0.
//!
//! ```text
//! Header:
//!   [0..2]                      num_instructions (u16 LE)
//!   [2..2 + 2*N]                instruction_offsets ([u16 LE; N])
//! Instruction, at its offset:
//!   [0..2]                      num_accounts (u16 LE)
//!   [2..2 + 33*A]               accounts ([AccountMeta; A])
//!   [2 + 33*A..34 + 33*A]       program_id (32 bytes)
//!   [34 + 33*A..36 + 33*A]      data_len (u16 LE)
//!   [36 + 33*A..36 + 33*A + D]  data
//! AccountMeta:
//!   [0..1]                      flags (bit 0 is_signer, bit 1 is_writable)
//!   [1..33]                     pubkey (32 bytes)
//! Trailer:
//!   [len - 2..len]              current instruction index (u16 LE)
//! ```
//!
//! The trailer is not in the layout comment: `construct_instructions_data`
//! resizes the serialized buffer by two bytes and `load_current_index` reads
//! `data[len - 2..len]`.  Every instruction body therefore lies entirely below
//! `len - 2`, which is why [`InstructionsSysvarV1`] bounds all body reads by
//! that limit — an offset that reaches into the trailer is malformed by
//! construction, not merely suspicious.
//!
//! The account is synthesized by the runtime rather than stored: `solana-svm
//! 4.2.1`, `src/account_loader.rs` `construct_instructions_account` builds it
//! with `owner: sysvar::id()` and the `Account::default()` remainder, so it is
//! never executable.  Both facts are pinned below.
//!
//! The captured byte fixture in this module's tests is real
//! `construct_instructions_data` output at `solana-instructions-sysvar 3.0.1`.

/// Instructions sysvar address, `Sysvar1nstructions1111111111111111111111111`.
///
/// Cross-checked against `solana_sdk_ids::sysvar::instructions::ID` by
/// `pinned_sysvar_ids_match_the_sdk_declarations`.
pub const INSTRUCTIONS_SYSVAR_ID: [u8; 32] = [
    6, 167, 213, 23, 24, 123, 209, 102, 53, 218, 212, 4, 85, 253, 194, 192, 193, 36, 198, 143, 33,
    86, 117, 165, 219, 186, 203, 95, 8, 0, 0, 0,
];

/// Owner of every sysvar account, `Sysvar1111111111111111111111111111111111111`.
pub const SYSVAR_OWNER_ID: [u8; 32] = [
    6, 167, 213, 23, 24, 117, 247, 41, 199, 61, 147, 64, 143, 33, 97, 32, 6, 126, 216, 140, 118,
    224, 140, 40, 127, 193, 148, 96, 0, 0, 0, 0,
];

/// Width of the `num_instructions` header field.
pub const HEADER_COUNT_LEN: usize = 2;
/// Offset of the instruction offset table.
pub const OFFSET_TABLE_OFFSET: usize = HEADER_COUNT_LEN;
/// Width of one offset-table entry.
pub const OFFSET_ENTRY_LEN: usize = 2;
/// Width of the trailing current-instruction-index field.
pub const CURRENT_INDEX_TRAILER_LEN: usize = 2;
/// Width of one serialized account meta: one flag byte plus a 32-byte address.
pub const ACCOUNT_META_LEN: usize = 33;
/// Offset of the address inside one serialized account meta.
pub const ACCOUNT_META_ADDRESS_OFFSET: usize = 1;
/// Width of an instruction's `num_accounts` field.
pub const ACCOUNT_COUNT_LEN: usize = 2;
/// Width of an instruction's `data_len` field.
pub const DATA_LEN_LEN: usize = 2;

/// Account-meta flag bit for `is_signer`.
pub const META_FLAG_IS_SIGNER: u8 = 0b0000_0001;
/// Account-meta flag bit for `is_writable`.
pub const META_FLAG_IS_WRITABLE: u8 = 0b0000_0010;
/// The only flag bits `InstructionsSysvarAccountMeta` can set.
///
/// The serializer builds the byte from a two-flag `bitflags` set, so any other
/// bit is a byte the runtime cannot have produced.
pub const META_FLAG_MASK: u8 = META_FLAG_IS_SIGNER | META_FLAG_IS_WRITABLE;

/// Account count of the reviewed Pyth `post_update` instruction.
///
/// Payer, encoded VAA, Config, treasury, price-update account, System
/// Program, and write authority, in that order.
pub const POST_UPDATE_V2_ACCOUNT_COUNT: usize = 7;

/// Width of an Anchor instruction discriminator.
pub const ANCHOR_DISCRIMINATOR_LEN: usize = 8;
/// Native Ed25519 signature-verification program.
pub const ED25519_PROGRAM_ID: [u8; 32] = [
    3, 125, 70, 214, 124, 147, 251, 190, 18, 249, 66, 143, 131, 141, 64, 255, 5, 112, 116,
    73, 39, 244, 138, 100, 252, 202, 112, 68, 128, 0, 0, 0,
];
/// Exact canonical one-signature Ed25519 header width.
pub const ED25519_ONE_SIGNATURE_HEADER_BYTES: usize = 16;
/// Exact Ed25519 signature width.
pub const ED25519_SIGNATURE_BYTES: usize = 64;
/// Exact Ed25519 public-key width.
pub const ED25519_PUBLIC_KEY_BYTES: usize = 32;
/// Quote admissions sign one SHA-256 identity.
pub const ED25519_QUOTE_MESSAGE_BYTES: usize = 32;
/// Canonical one-key, one-digest Ed25519 instruction bytes.
pub const ED25519_QUOTE_INSTRUCTION_BYTES: usize = ED25519_ONE_SIGNATURE_HEADER_BYTES
    + ED25519_SIGNATURE_BYTES
    + ED25519_PUBLIC_KEY_BYTES
    + ED25519_QUOTE_MESSAGE_BYTES;

/// Shortest data a well-formed Instructions sysvar can carry: the count, one
/// offset entry, and the trailer.
pub const MIN_SYSVAR_DATA_LEN: usize =
    HEADER_COUNT_LEN + OFFSET_ENTRY_LEN + CURRENT_INDEX_TRAILER_LEN;

const _: () = {
    assert!(ACCOUNT_META_LEN == 1 + 32);
    assert!(META_FLAG_MASK == 0b0000_0011);
    assert!(MIN_SYSVAR_DATA_LEN == 6);
};

/// Refusals raised while decoding the pinned Instructions sysvar.
///
/// Every variant is a refusal.  A malformed or out-of-range read is never
/// tolerated as an absent join: an unproven adjacency is not a weak adjacency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstructionsSysvarError {
    /// The presented account was not [`INSTRUCTIONS_SYSVAR_ID`].
    WrongSysvarKey,
    /// The presented account was not owned by [`SYSVAR_OWNER_ID`].
    WrongSysvarOwner,
    /// The presented sysvar account was executable.
    ExecutableSysvar,
    /// The account carried fewer than [`MIN_SYSVAR_DATA_LEN`] bytes.
    ShortData,
    /// The declared instruction count did not fit the offset table and trailer.
    MalformedHeader,
    /// The sysvar declared zero instructions.
    EmptyInstructionList,
    /// The trailing current-instruction index named no declared instruction.
    CurrentIndexOutOfRange,
    /// The requested instruction index was at or past the declared count.
    IndexOutOfRange,
    /// The requested instruction index was after the executing instruction.
    ///
    /// A later instruction has not run.  Reading one would let a transaction
    /// promise a post that may never execute.
    FutureInstruction,
    /// The executing instruction is the transaction's first, so no preceding
    /// instruction exists.
    NoPrecedingInstruction,
    /// An offset-table entry, account count, or data length placed a field
    /// outside the instruction region.
    MalformedOffset,
    /// The requested account-meta position was at or past the instruction's
    /// account count.
    AccountIndexOutOfRange,
    /// An account-meta flag byte set a bit outside [`META_FLAG_MASK`].
    NonCanonicalAccountMetaFlags,
    /// Two roles of the reviewed post ABI named one account-meta position.
    AliasedPostAbiPositions,
    /// The adjacent instruction did not carry exactly the reviewed account
    /// count.
    WrongPostAccountCount,
    /// The adjacent instruction did not begin with the reviewed Anchor
    /// discriminator.
    WrongPostDiscriminator,
    /// One adjacent-instruction account had different signer/writable flags
    /// from the reviewed ABI.
    WrongPostAccountFlags,
    /// The adjacent instruction was not the native Ed25519 verifier.
    WrongEd25519Program,
    /// The Ed25519 instruction did not use the exact one-signature local-data ABI.
    MalformedEd25519Instruction,
    /// The verified Ed25519 public key was not the policy-selected authority.
    WrongEd25519PublicKey,
    /// The verified Ed25519 message was not the exact quote-admission identity.
    WrongEd25519Message,
}

/// One decoded account meta of a serialized instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountMetaV1 {
    /// Account address.
    pub address: [u8; 32],
    /// Whether the transaction presented this account as a signer.
    pub is_signer: bool,
    /// Whether the transaction presented this account as writable.
    pub is_writable: bool,
}

/// A header-validated handle on the pinned Instructions sysvar.
///
/// Constructing one proves the account identity, owner, non-executability, a
/// self-consistent header, and a current index that names a declared
/// instruction.  Everything else is decoded on demand from fixed-size reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstructionsSysvarV1<'a> {
    data: &'a [u8],
    count: u16,
    current: u16,
}

/// One decoded instruction of the currently executing transaction.
///
/// Holds borrowed slices rather than collections: this crate's SBF profile
/// allocates nothing on the decode path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstructionViewV1<'a> {
    index: u16,
    program_id: [u8; 32],
    account_count: u16,
    metas: &'a [u8],
    data: &'a [u8],
}

/// Account-meta positions of the reviewed receiver-post ABI.
///
/// Supplied by the compiled release triple, never by caller instruction data.
/// The R2 plan's registry mechanism decision (P0.9) owns where these live; this
/// module only refuses to read a position that the post instruction does not
/// have, and refuses two roles that name one position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PostAbiPositionsV1 {
    /// Meta position of the receiver `Config` account.
    pub config: u16,
    /// Meta position of the ephemeral price-update account.
    pub update_account: u16,
    /// Meta position of the update's write authority.
    pub write_authority: u16,
}

/// Exact reviewed identity of the Pyth `post_update` instruction.
///
/// This is compiled release data, never caller input.  The fixed flag array
/// also makes the seven-account shape an encoded Rust type invariant; the
/// runtime still compares the decoded account count explicitly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PostAbiV2 {
    /// Anchor discriminator: `sha256("global:post_update")[..8]`.
    pub discriminator: [u8; ANCHOR_DISCRIMINATOR_LEN],
    /// Expected signer/writable flag byte for every account position.
    pub account_flags: [u8; POST_UPDATE_V2_ACCOUNT_COUNT],
    /// One reviewed transaction-global writable elevation caused by two ABI
    /// positions naming the same address.
    ///
    /// The runtime serializes effective message privileges into the
    /// Instructions sysvar. Pyth commonly uses one wallet as both writable
    /// payer and readonly write authority, so the latter position is observed
    /// writable too. No other flag difference is admitted.
    pub writable_alias_elevation: Option<(u16, u16)>,
    /// Positions of the three accounts joined to SourceSpec/update evidence.
    pub positions: PostAbiPositionsV1,
}

/// The projection `research/source-profile-v1/src/auth_v2.rs` expects from a
/// reviewed Instructions-sysvar parser.
///
/// Field-for-field identical to that crate's `ImmediatePostV1`, deliberately:
/// the research model is the executable contract and this module is its runtime
/// implementation, so any divergence must surface as a compile error at the
/// kernel-port step (R2 plan P0.2) rather than as a silent semantic drift.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImmediatePostV1 {
    /// Index of the post instruction.
    pub instruction_index: u16,
    /// Index of the instruction consuming the post, i.e. this one.
    pub consuming_instruction_index: u16,
    /// Program the post instruction invoked.
    pub program: [u8; 32],
    /// Receiver `Config` account named by the post.
    pub config: [u8; 32],
    /// Ephemeral price-update account named by the post.
    pub update_account: [u8; 32],
    /// Write authority named by the post.
    pub write_authority: [u8; 32],
}

impl<'a> InstructionsSysvarV1<'a> {
    /// Authenticate the sysvar account and validate its header.
    ///
    /// Refuses a wrong key, a wrong owner, an executable account, data shorter
    /// than [`MIN_SYSVAR_DATA_LEN`], a declared count whose offset table and
    /// trailer do not fit, a zero count, and a trailing current index that
    /// names no declared instruction.
    pub fn new(
        key: [u8; 32],
        owner: [u8; 32],
        executable: bool,
        data: &'a [u8],
    ) -> Result<Self, InstructionsSysvarError> {
        if key != INSTRUCTIONS_SYSVAR_ID {
            return Err(InstructionsSysvarError::WrongSysvarKey);
        }
        if owner != SYSVAR_OWNER_ID {
            return Err(InstructionsSysvarError::WrongSysvarOwner);
        }
        if executable {
            return Err(InstructionsSysvarError::ExecutableSysvar);
        }
        if data.len() < MIN_SYSVAR_DATA_LEN {
            return Err(InstructionsSysvarError::ShortData);
        }
        let count = u16_at(data, 0);
        if count == 0 {
            return Err(InstructionsSysvarError::EmptyInstructionList);
        }
        let table_end = usize::from(count)
            .checked_mul(OFFSET_ENTRY_LEN)
            .and_then(|table| table.checked_add(OFFSET_TABLE_OFFSET))
            .ok_or(InstructionsSysvarError::MalformedHeader)?;
        let required = table_end
            .checked_add(CURRENT_INDEX_TRAILER_LEN)
            .ok_or(InstructionsSysvarError::MalformedHeader)?;
        if required > data.len() {
            return Err(InstructionsSysvarError::MalformedHeader);
        }
        let current = u16_at(data, data.len() - CURRENT_INDEX_TRAILER_LEN);
        if current >= count {
            return Err(InstructionsSysvarError::CurrentIndexOutOfRange);
        }
        Ok(Self {
            data,
            count,
            current,
        })
    }

    /// Number of instructions the transaction declared.
    pub const fn instruction_count(self) -> u16 {
        self.count
    }

    /// Index of the executing instruction, from the sysvar trailer.
    pub const fn current_index(self) -> u16 {
        self.current
    }

    /// The instruction region: everything below the current-index trailer.
    fn body(self) -> &'a [u8] {
        let data: &'a [u8] = self.data;
        &data[..data.len() - CURRENT_INDEX_TRAILER_LEN]
    }

    /// Decode the instruction at `index`.
    ///
    /// Refuses an index at or past the declared count, an index after the
    /// executing instruction, and any offset, account count, or data length
    /// that would read outside the instruction region.
    pub fn instruction_at(
        self,
        index: u16,
    ) -> Result<InstructionViewV1<'a>, InstructionsSysvarError> {
        if index >= self.count {
            return Err(InstructionsSysvarError::IndexOutOfRange);
        }
        if index > self.current {
            return Err(InstructionsSysvarError::FutureInstruction);
        }
        let body = self.body();
        /* `index < count` and `new` proved `2 + 2*count + 2 <= data.len()`, so
         * this entry is inside the table and the product is bounded by
         * `2 * u16::MAX`; neither can overflow or read out of bounds. */
        let entry = OFFSET_TABLE_OFFSET + usize::from(index) * OFFSET_ENTRY_LEN;
        let start = usize::from(u16_at(self.data, entry));
        let metas_start = bounded_add(start, ACCOUNT_COUNT_LEN, body.len())?;
        let account_count = u16_at(body, start);
        let metas_len = usize::from(account_count)
            .checked_mul(ACCOUNT_META_LEN)
            .ok_or(InstructionsSysvarError::MalformedOffset)?;
        let program_start = bounded_add(metas_start, metas_len, body.len())?;
        let data_len_start = bounded_add(program_start, 32, body.len())?;
        let data_start = bounded_add(data_len_start, DATA_LEN_LEN, body.len())?;
        let data_len = usize::from(u16_at(body, data_len_start));
        let data_end = bounded_add(data_start, data_len, body.len())?;
        Ok(InstructionViewV1 {
            index,
            program_id: address_at(body, program_start),
            account_count,
            metas: &body[metas_start..program_start],
            data: &body[data_start..data_end],
        })
    }

    /// Decode the instruction immediately before the executing one.
    ///
    /// Refuses when the executing instruction is the transaction's first.
    pub fn preceding_instruction(self) -> Result<InstructionViewV1<'a>, InstructionsSysvarError> {
        let index = self
            .current
            .checked_sub(1)
            .ok_or(InstructionsSysvarError::NoPrecedingInstruction)?;
        self.instruction_at(index)
    }

    /// Authenticate an immediately preceding native Ed25519 verification of
    /// one exact 32-byte quote-admission identity.
    ///
    /// The parser admits only the canonical SDK one-signature layout with all
    /// offsets local to the Ed25519 instruction (`u16::MAX`). The native
    /// precompile has already rejected a bad signature before this instruction
    /// executes; this method binds that successful verification to the exact
    /// policy key and message without accepting caller signature claims.
    pub fn preceding_ed25519_quote_v1(
        self,
        expected_public_key: [u8; ED25519_PUBLIC_KEY_BYTES],
        expected_message: [u8; ED25519_QUOTE_MESSAGE_BYTES],
    ) -> Result<(), InstructionsSysvarError> {
        let instruction = self.preceding_instruction()?;
        if instruction.program_id != ED25519_PROGRAM_ID {
            return Err(InstructionsSysvarError::WrongEd25519Program);
        }
        if instruction.account_count != 0 || instruction.data.len() != ED25519_QUOTE_INSTRUCTION_BYTES {
            return Err(InstructionsSysvarError::MalformedEd25519Instruction);
        }
        let data = instruction.data;
        let canonical_header = [
            1u8,
            0,
            16,
            0,
            255,
            255,
            80,
            0,
            255,
            255,
            112,
            0,
            32,
            0,
            255,
            255,
        ];
        if data[..ED25519_ONE_SIGNATURE_HEADER_BYTES] != canonical_header {
            return Err(InstructionsSysvarError::MalformedEd25519Instruction);
        }
        let public_key_start = ED25519_ONE_SIGNATURE_HEADER_BYTES + ED25519_SIGNATURE_BYTES;
        let message_start = public_key_start + ED25519_PUBLIC_KEY_BYTES;
        if data[public_key_start..message_start] != expected_public_key {
            return Err(InstructionsSysvarError::WrongEd25519PublicKey);
        }
        if data[message_start..] != expected_message {
            return Err(InstructionsSysvarError::WrongEd25519Message);
        }
        Ok(())
    }

    /// Project the immediately preceding instruction onto [`ImmediatePostV1`].
    ///
    /// The adjacency is structural rather than asserted: `instruction_index` is
    /// `current - 1` and `consuming_instruction_index` is `current`, both read
    /// from the sysvar, so `auth_v2`'s `PostNotAdjacent` check can never be
    /// satisfied by a caller-supplied pair.
    ///
    /// Refuses when there is no preceding instruction, when the ABI names a
    /// meta position the post does not have, and when two roles name one
    /// position.
    pub fn immediate_post_v1(
        self,
        abi: PostAbiPositionsV1,
    ) -> Result<ImmediatePostV1, InstructionsSysvarError> {
        if abi.config == abi.update_account
            || abi.config == abi.write_authority
            || abi.update_account == abi.write_authority
        {
            return Err(InstructionsSysvarError::AliasedPostAbiPositions);
        }
        let post = self.preceding_instruction()?;
        Ok(ImmediatePostV1 {
            instruction_index: post.index,
            consuming_instruction_index: self.current,
            program: post.program_id,
            config: post.account_meta(abi.config)?.address,
            update_account: post.account_meta(abi.update_account)?.address,
            write_authority: post.account_meta(abi.write_authority)?.address,
        })
    }

    /// Authenticate and project the exact immediately preceding
    /// `post_update` instruction described by `abi`.
    ///
    /// Besides structural adjacency and the three role addresses, this binds
    /// the Anchor discriminator, exact seven-account count, and signer/writable
    /// flags of every account. Instruction arguments after the discriminator
    /// remain receiver-owned input: the pinned receiver program validates
    /// their variable-length encoding before this consumer can execute.
    pub fn immediate_post_v2(
        self,
        abi: PostAbiV2,
    ) -> Result<ImmediatePostV1, InstructionsSysvarError> {
        let positions = abi.positions;
        if positions.config == positions.update_account
            || positions.config == positions.write_authority
            || positions.update_account == positions.write_authority
        {
            return Err(InstructionsSysvarError::AliasedPostAbiPositions);
        }
        let post = self.preceding_instruction()?;
        if usize::from(post.account_count) != POST_UPDATE_V2_ACCOUNT_COUNT {
            return Err(InstructionsSysvarError::WrongPostAccountCount);
        }
        let Some(discriminator) = post.data.get(..ANCHOR_DISCRIMINATOR_LEN) else {
            return Err(InstructionsSysvarError::WrongPostDiscriminator);
        };
        if discriminator != abi.discriminator {
            return Err(InstructionsSysvarError::WrongPostDiscriminator);
        }
        for (position, expected) in abi.account_flags.iter().copied().enumerate() {
            if expected & !META_FLAG_MASK != 0 {
                return Err(InstructionsSysvarError::WrongPostAccountFlags);
            }
            let meta = post.account_meta(position as u16)?;
            let actual = u8::from(meta.is_signer) * META_FLAG_IS_SIGNER
                | u8::from(meta.is_writable) * META_FLAG_IS_WRITABLE;
            if actual != expected {
                let alias_allowed = match abi.writable_alias_elevation {
                    Some((writable_source, elevated_target))
                        if usize::from(elevated_target) == position
                            && actual == (expected | META_FLAG_IS_WRITABLE)
                            && usize::from(writable_source) < POST_UPDATE_V2_ACCOUNT_COUNT =>
                    {
                        post.account_meta(writable_source)?.address == meta.address
                    }
                    _ => false,
                };
                if !alias_allowed {
                    return Err(InstructionsSysvarError::WrongPostAccountFlags);
                }
            }
        }
        Ok(ImmediatePostV1 {
            instruction_index: post.index,
            consuming_instruction_index: self.current,
            program: post.program_id,
            config: post.account_meta(positions.config)?.address,
            update_account: post.account_meta(positions.update_account)?.address,
            write_authority: post.account_meta(positions.write_authority)?.address,
        })
    }
}

impl<'a> InstructionViewV1<'a> {
    /// Position of this instruction in the transaction.
    pub const fn index(self) -> u16 {
        self.index
    }

    /// Program the instruction invoked.
    pub const fn program_id(self) -> [u8; 32] {
        self.program_id
    }

    /// Number of account metas the instruction declared.
    pub const fn account_count(self) -> u16 {
        self.account_count
    }

    /// Instruction data.
    pub const fn data(self) -> &'a [u8] {
        self.data
    }

    /// Decode the account meta at `position`.
    ///
    /// Refuses a position at or past [`Self::account_count`] and a flag byte
    /// carrying a bit outside [`META_FLAG_MASK`].
    pub fn account_meta(self, position: u16) -> Result<AccountMetaV1, InstructionsSysvarError> {
        if position >= self.account_count {
            return Err(InstructionsSysvarError::AccountIndexOutOfRange);
        }
        let at = usize::from(position) * ACCOUNT_META_LEN;
        let flags = self.metas[at];
        if flags & !META_FLAG_MASK != 0 {
            return Err(InstructionsSysvarError::NonCanonicalAccountMetaFlags);
        }
        Ok(AccountMetaV1 {
            address: address_at(self.metas, at + ACCOUNT_META_ADDRESS_OFFSET),
            is_signer: flags & META_FLAG_IS_SIGNER != 0,
            is_writable: flags & META_FLAG_IS_WRITABLE != 0,
        })
    }
}

/// `left + right`, refusing overflow and any sum past `limit`.
fn bounded_add(left: usize, right: usize, limit: usize) -> Result<usize, InstructionsSysvarError> {
    let sum = left
        .checked_add(right)
        .ok_or(InstructionsSysvarError::MalformedOffset)?;
    if sum > limit {
        return Err(InstructionsSysvarError::MalformedOffset);
    }
    Ok(sum)
}

fn u16_at(data: &[u8], offset: usize) -> u16 {
    let mut value = [0_u8; 2];
    value.copy_from_slice(&data[offset..offset + 2]);
    u16::from_le_bytes(value)
}

fn address_at(data: &[u8], offset: usize) -> [u8; 32] {
    let mut address = [0_u8; 32];
    address.copy_from_slice(&data[offset..offset + 32]);
    address
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECEIVER: [u8; 32] = [0xb2; 32];
    const OTHER_PROGRAM: [u8; 32] = [0xa7; 32];
    const CONFIG: [u8; 32] = [0xc3; 32];
    const UPDATE: [u8; 32] = [0xc4; 32];
    const WRITE_AUTHORITY: [u8; 32] = [0x11; 32];

    /* Captured from `solana_instructions_sysvar::construct_instructions_data`
     * at crate version 3.0.1, with the trailer then set to 2.  Three
     * instructions:
     *   0: program 0xa7, one meta (0xc3, signer), data [7, 8, 9]
     *   1: program 0xb2, metas (0xc3 plain, 0xc4 writable, 0x11 signer+writable),
     *      data [0xe0, 0xf1]
     *   2: program 0xa7, no metas, no data  -- the consuming instruction
     * This is real serializer output, not a reading of the layout table. */
    const CAPTURED: &str = concat!(
        "0300",
        "0800",
        "5000",
        "d900",
        "0100",
        "01",
        "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3",
        "a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7",
        "0300",
        "070809",
        "0300",
        "00",
        "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3",
        "02",
        "c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4c4",
        "03",
        "1111111111111111111111111111111111111111111111111111111111111111",
        "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2",
        "0200",
        "e0f1",
        "0000",
        "a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7",
        "0000",
        "0200",
    );
    const CAPTURED_LEN: usize = 255;

    fn captured() -> [u8; CAPTURED_LEN] {
        let source = CAPTURED.as_bytes();
        assert_eq!(source.len(), CAPTURED_LEN * 2);
        let mut out = [0_u8; CAPTURED_LEN];
        let mut index = 0;
        while index < CAPTURED_LEN {
            out[index] = (nibble(source[index * 2]) << 4) | nibble(source[index * 2 + 1]);
            index += 1;
        }
        out
    }

    fn nibble(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("fixture is lowercase hexadecimal"),
        }
    }

    fn sysvar(data: &[u8]) -> Result<InstructionsSysvarV1<'_>, InstructionsSysvarError> {
        InstructionsSysvarV1::new(INSTRUCTIONS_SYSVAR_ID, SYSVAR_OWNER_ID, false, data)
    }

    fn ok(data: &[u8]) -> InstructionsSysvarV1<'_> {
        sysvar(data).unwrap()
    }

    /// Set the trailing current-instruction index.
    fn with_current(bytes: &mut [u8; CAPTURED_LEN], index: u16) {
        let at = CAPTURED_LEN - CURRENT_INDEX_TRAILER_LEN;
        bytes[at..].copy_from_slice(&index.to_le_bytes());
    }

    /// A minimal two-instruction buffer built from the layout table, used where
    /// a hand-shaped malformation is clearer than editing the capture.
    fn built_with_data(
        post_program: [u8; 32],
        post_metas: &[(u8, [u8; 32])],
        post_data: &[u8],
        current: u16,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&2_u16.to_le_bytes());
        let table = out.len();
        out.extend_from_slice(&[0; 4]);
        // Instruction 0: the post.
        let start0 = u16::try_from(out.len()).unwrap();
        out.extend_from_slice(&u16::try_from(post_metas.len()).unwrap().to_le_bytes());
        for (flags, address) in post_metas {
            out.push(*flags);
            out.extend_from_slice(address);
        }
        out.extend_from_slice(&post_program);
        out.extend_from_slice(&u16::try_from(post_data.len()).unwrap().to_le_bytes());
        out.extend_from_slice(post_data);
        // Instruction 1: the consumer.
        let start1 = u16::try_from(out.len()).unwrap();
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&[0x5a; 32]);
        out.extend_from_slice(&0_u16.to_le_bytes());
        out[table..table + 2].copy_from_slice(&start0.to_le_bytes());
        out[table + 2..table + 4].copy_from_slice(&start1.to_le_bytes());
        out.extend_from_slice(&current.to_le_bytes());
        out
    }

    fn built(post_program: [u8; 32], post_metas: &[(u8, [u8; 32])], current: u16) -> Vec<u8> {
        built_with_data(post_program, post_metas, &[], current)
    }

    fn post_abi() -> PostAbiPositionsV1 {
        PostAbiPositionsV1 {
            config: 0,
            update_account: 1,
            write_authority: 2,
        }
    }

    fn exact_post_abi() -> PostAbiV2 {
        PostAbiV2 {
            discriminator: [0x85, 0x5f, 0xcf, 0xaf, 0x0b, 0x4f, 0x76, 0x2c],
            account_flags: [
                META_FLAG_IS_SIGNER | META_FLAG_IS_WRITABLE,
                0,
                0,
                META_FLAG_IS_WRITABLE,
                META_FLAG_IS_SIGNER | META_FLAG_IS_WRITABLE,
                0,
                META_FLAG_IS_SIGNER,
            ],
            writable_alias_elevation: Some((0, 6)),
            positions: PostAbiPositionsV1 {
                config: 2,
                update_account: 4,
                write_authority: 6,
            },
        }
    }

    fn exact_post_metas() -> [(u8, [u8; 32]); POST_UPDATE_V2_ACCOUNT_COUNT] {
        [
            (META_FLAG_IS_SIGNER | META_FLAG_IS_WRITABLE, [0x01; 32]),
            (0, [0x02; 32]),
            (0, CONFIG),
            (META_FLAG_IS_WRITABLE, [0x04; 32]),
            (META_FLAG_IS_SIGNER | META_FLAG_IS_WRITABLE, UPDATE),
            (0, [0x06; 32]),
            (META_FLAG_IS_SIGNER, WRITE_AUTHORITY),
        ]
    }

    #[test]
    fn pinned_sysvar_ids_match_the_sdk_declarations() {
        assert_eq!(
            INSTRUCTIONS_SYSVAR_ID,
            solana_sdk_ids::sysvar::instructions::ID.to_bytes()
        );
        assert_eq!(SYSVAR_OWNER_ID, solana_sdk_ids::sysvar::ID.to_bytes());
    }

    #[test]
    fn captured_fixture_matches_the_documented_byte_offsets() {
        let bytes = captured();
        assert_eq!(u16_at(&bytes, 0), 3, "num_instructions");
        assert_eq!(u16_at(&bytes, OFFSET_TABLE_OFFSET), 8, "offset[0]");
        assert_eq!(u16_at(&bytes, OFFSET_TABLE_OFFSET + 2), 80, "offset[1]");
        assert_eq!(u16_at(&bytes, OFFSET_TABLE_OFFSET + 4), 217, "offset[2]");
        assert_eq!(
            u16_at(&bytes, CAPTURED_LEN - CURRENT_INDEX_TRAILER_LEN),
            2,
            "trailer"
        );
        /* Instruction 0 spans exactly to the start of instruction 1:
         * 8 + 2 + 33*1 + 32 + 2 + 3 == 80. */
        assert_eq!(
            8 + ACCOUNT_COUNT_LEN + ACCOUNT_META_LEN + 32 + DATA_LEN_LEN + 3,
            80
        );
        /* Instruction 1: 80 + 2 + 33*3 + 32 + 2 + 2 == 217. */
        assert_eq!(
            80 + ACCOUNT_COUNT_LEN + 3 * ACCOUNT_META_LEN + 32 + DATA_LEN_LEN + 2,
            217
        );
        /* Instruction 2: 217 + 2 + 0 + 32 + 2 == 253 == len - trailer. */
        assert_eq!(
            217 + ACCOUNT_COUNT_LEN + 32 + DATA_LEN_LEN,
            CAPTURED_LEN - CURRENT_INDEX_TRAILER_LEN
        );
    }

    #[test]
    fn captured_fixture_decodes_every_instruction_and_meta() {
        let bytes = captured();
        let sysvar = ok(&bytes);
        assert_eq!(sysvar.instruction_count(), 3);
        assert_eq!(sysvar.current_index(), 2);

        let first = sysvar.instruction_at(0).unwrap();
        assert_eq!(first.program_id(), OTHER_PROGRAM);
        assert_eq!(first.account_count(), 1);
        assert_eq!(first.data(), &[7, 8, 9]);
        assert_eq!(
            first.account_meta(0).unwrap(),
            AccountMetaV1 {
                address: CONFIG,
                is_signer: true,
                is_writable: false,
            }
        );

        let post = sysvar.instruction_at(1).unwrap();
        assert_eq!(post.program_id(), RECEIVER);
        assert_eq!(post.account_count(), 3);
        assert_eq!(post.data(), &[0xe0, 0xf1]);
        assert_eq!(
            post.account_meta(0).unwrap(),
            AccountMetaV1 {
                address: CONFIG,
                is_signer: false,
                is_writable: false,
            }
        );
        assert_eq!(
            post.account_meta(1).unwrap(),
            AccountMetaV1 {
                address: UPDATE,
                is_signer: false,
                is_writable: true,
            }
        );
        assert_eq!(
            post.account_meta(2).unwrap(),
            AccountMetaV1 {
                address: WRITE_AUTHORITY,
                is_signer: true,
                is_writable: true,
            }
        );

        let current = sysvar.instruction_at(2).unwrap();
        assert_eq!(current.program_id(), OTHER_PROGRAM);
        assert_eq!(current.account_count(), 0);
        assert!(current.data().is_empty());
    }

    #[test]
    fn immediate_post_projects_the_preceding_instruction() {
        let bytes = captured();
        assert_eq!(
            ok(&bytes).immediate_post_v1(post_abi()).unwrap(),
            ImmediatePostV1 {
                instruction_index: 1,
                consuming_instruction_index: 2,
                program: RECEIVER,
                config: CONFIG,
                update_account: UPDATE,
                write_authority: WRITE_AUTHORITY,
            }
        );
    }

    #[test]
    fn exact_post_binds_discriminator_count_every_flag_and_role_position() {
        let abi = exact_post_abi();
        let mut data = abi.discriminator.to_vec();
        data.extend_from_slice(&[0x44; 17]);
        let metas = exact_post_metas();
        let bytes = built_with_data(RECEIVER, &metas, &data, 1);
        assert_eq!(
            ok(&bytes).immediate_post_v2(abi).unwrap(),
            ImmediatePostV1 {
                instruction_index: 0,
                consuming_instruction_index: 1,
                program: RECEIVER,
                config: CONFIG,
                update_account: UPDATE,
                write_authority: WRITE_AUTHORITY,
            }
        );

        for count in [0_usize, 1, POST_UPDATE_V2_ACCOUNT_COUNT - 1] {
            let short = built_with_data(RECEIVER, &metas[..count], &data, 1);
            assert_eq!(
                ok(&short).immediate_post_v2(abi),
                Err(InstructionsSysvarError::WrongPostAccountCount),
                "accepted {count} accounts"
            );
        }
        let mut extra = metas.to_vec();
        extra.push((0, [0x08; 32]));
        let long = built_with_data(RECEIVER, &extra, &data, 1);
        assert_eq!(
            ok(&long).immediate_post_v2(abi),
            Err(InstructionsSysvarError::WrongPostAccountCount)
        );

        for length in 0..ANCHOR_DISCRIMINATOR_LEN {
            let short = built_with_data(RECEIVER, &metas, &data[..length], 1);
            assert_eq!(
                ok(&short).immediate_post_v2(abi),
                Err(InstructionsSysvarError::WrongPostDiscriminator),
                "accepted {length} discriminator bytes"
            );
        }
        for at in 0..ANCHOR_DISCRIMINATOR_LEN {
            let mut wrong = data.clone();
            wrong[at] ^= 1;
            let bytes = built_with_data(RECEIVER, &metas, &wrong, 1);
            assert_eq!(
                ok(&bytes).immediate_post_v2(abi),
                Err(InstructionsSysvarError::WrongPostDiscriminator),
                "accepted discriminator mutation at {at}"
            );
        }

        for position in 0..POST_UPDATE_V2_ACCOUNT_COUNT {
            for bit in [META_FLAG_IS_SIGNER, META_FLAG_IS_WRITABLE] {
                let mut hostile = metas;
                hostile[position].0 ^= bit;
                let bytes = built_with_data(RECEIVER, &hostile, &data, 1);
                assert_eq!(
                    ok(&bytes).immediate_post_v2(abi),
                    Err(InstructionsSysvarError::WrongPostAccountFlags),
                    "accepted flag mutation {bit:#04x} at {position}"
                );
            }
        }

        let mut payer_is_authority = metas;
        payer_is_authority[6].0 |= META_FLAG_IS_WRITABLE;
        payer_is_authority[6].1 = payer_is_authority[0].1;
        let aliased = built_with_data(RECEIVER, &payer_is_authority, &data, 1);
        let projected = ok(&aliased)
            .immediate_post_v2(abi)
            .expect("payer/write-authority alias elevation is reviewed");
        assert_eq!(projected.write_authority, payer_is_authority[0].1);

        let mut unexplained_elevation = metas;
        unexplained_elevation[6].0 |= META_FLAG_IS_WRITABLE;
        let elevated = built_with_data(RECEIVER, &unexplained_elevation, &data, 1);
        assert_eq!(
            ok(&elevated).immediate_post_v2(abi),
            Err(InstructionsSysvarError::WrongPostAccountFlags)
        );

        for (mut positions, label) in [
            (abi.positions, "config"),
            (abi.positions, "update"),
            (abi.positions, "authority"),
        ] {
            match label {
                "config" => positions.config = 1,
                "update" => positions.update_account = 3,
                _ => positions.write_authority = 5,
            }
            let mut wrong = abi;
            wrong.positions = positions;
            let projected = ok(&bytes).immediate_post_v2(wrong).unwrap();
            match label {
                "config" => assert_eq!(projected.config, [0x02; 32]),
                "update" => assert_eq!(projected.update_account, [0x04; 32]),
                _ => assert_eq!(projected.write_authority, [0x06; 32]),
            }
        }
    }

    #[test]
    fn a_non_adjacent_post_is_not_reachable() {
        /* Instruction 1 is the post.  Consuming at index 2 joins it; consuming
         * at any later index projects whatever ran immediately before, never
         * instruction 1.  This is the set/post/restore refusal: it is
         * structural, so no caller claim can restore the join. */
        let mut bytes = captured();
        with_current(&mut bytes, 2);
        assert_eq!(
            ok(&bytes)
                .immediate_post_v1(post_abi())
                .unwrap()
                .instruction_index,
            1
        );
        with_current(&mut bytes, 1);
        let projected = ok(&bytes).immediate_post_v1(post_abi());
        /* Instruction 0 has one meta, so the three-role ABI cannot be read from
         * it at all -- the wrong neighbour refuses rather than projecting. */
        assert_eq!(
            projected,
            Err(InstructionsSysvarError::AccountIndexOutOfRange)
        );
    }

    #[test]
    fn the_first_instruction_has_no_preceding_post() {
        let mut bytes = captured();
        with_current(&mut bytes, 0);
        let sysvar = ok(&bytes);
        assert_eq!(
            sysvar.preceding_instruction(),
            Err(InstructionsSysvarError::NoPrecedingInstruction)
        );
        assert_eq!(
            sysvar.immediate_post_v1(post_abi()),
            Err(InstructionsSysvarError::NoPrecedingInstruction)
        );
    }

    #[test]
    fn reading_past_the_current_instruction_refuses() {
        let mut bytes = captured();
        for current in 0_u16..3 {
            with_current(&mut bytes, current);
            let sysvar = ok(&bytes);
            /* The current instruction itself is readable. */
            assert!(sysvar.instruction_at(current).is_ok(), "current {current}");
            for future in current + 1..3 {
                assert_eq!(
                    sysvar.instruction_at(future),
                    Err(InstructionsSysvarError::FutureInstruction),
                    "current {current} read {future}"
                );
            }
        }
    }

    #[test]
    fn index_out_of_range_refuses_before_the_future_check() {
        let bytes = captured();
        let sysvar = ok(&bytes);
        for index in [3_u16, 4, 100, u16::MAX] {
            assert_eq!(
                sysvar.instruction_at(index),
                Err(InstructionsSysvarError::IndexOutOfRange),
                "accepted index {index}"
            );
        }
    }

    #[test]
    fn wrong_key_owner_or_executability_refuses() {
        let bytes = captured();
        assert_eq!(
            InstructionsSysvarV1::new([0xf0; 32], SYSVAR_OWNER_ID, false, &bytes),
            Err(InstructionsSysvarError::WrongSysvarKey)
        );
        /* The Clock sysvar's own address must not pass for this one. */
        assert_eq!(
            InstructionsSysvarV1::new(
                crate::instructions::artifact::CLOCK_SYSVAR_ID.to_bytes(),
                SYSVAR_OWNER_ID,
                false,
                &bytes
            ),
            Err(InstructionsSysvarError::WrongSysvarKey)
        );
        assert_eq!(
            InstructionsSysvarV1::new(INSTRUCTIONS_SYSVAR_ID, [0xf1; 32], false, &bytes),
            Err(InstructionsSysvarError::WrongSysvarOwner)
        );
        assert_eq!(
            InstructionsSysvarV1::new(INSTRUCTIONS_SYSVAR_ID, SYSVAR_OWNER_ID, true, &bytes),
            Err(InstructionsSysvarError::ExecutableSysvar)
        );
    }

    #[test]
    fn short_data_refuses() {
        let bytes = captured();
        for len in 0..MIN_SYSVAR_DATA_LEN {
            assert_eq!(
                sysvar(&bytes[..len]).err(),
                Some(InstructionsSysvarError::ShortData),
                "accepted {len} bytes"
            );
        }
    }

    #[test]
    fn truncation_at_every_length_refuses_or_refuses_the_read() {
        /* No truncation of a well-formed buffer may ever yield a successful
         * projection of the post: either the header stops being consistent, or
         * a body read runs past the shortened instruction region. */
        let bytes = captured();
        for len in 0..CAPTURED_LEN {
            let short = &bytes[..len];
            let outcome = sysvar(short).and_then(|s| s.immediate_post_v1(post_abi()));
            assert!(
                outcome.is_err(),
                "a {len}-byte truncation projected a post: {outcome:?}"
            );
        }
        /* And the untruncated buffer does succeed, so the loop above is not
         * vacuously green. */
        assert!(ok(&bytes).immediate_post_v1(post_abi()).is_ok());
    }

    #[test]
    fn a_declared_count_that_does_not_fit_refuses() {
        /* 126 offset entries need 2 + 252 + 2 == 256 bytes; the capture has
         * 255.  125 entries fit, so 126 is the exact boundary. */
        let mut bytes = captured();
        for count in [126_u16, 127, 1_000, u16::MAX] {
            bytes[..2].copy_from_slice(&count.to_le_bytes());
            assert_eq!(
                sysvar(&bytes).err(),
                Some(InstructionsSysvarError::MalformedHeader),
                "accepted declared count {count}"
            );
        }
    }

    #[test]
    fn an_overstated_count_within_the_header_still_refuses_the_read() {
        /* A count that fits the offset table but exceeds the instructions the
         * transaction actually carries reads a garbage offset from the body,
         * which must refuse rather than decode. */
        let mut bytes = captured();
        bytes[..2].copy_from_slice(&4_u16.to_le_bytes());
        with_current(&mut bytes, 3);
        let sysvar = ok(&bytes);
        assert_eq!(sysvar.instruction_count(), 4);
        assert_eq!(
            sysvar.instruction_at(3),
            Err(InstructionsSysvarError::MalformedOffset)
        );
        /* And the post join now names instruction 2, the real last instruction,
         * which carries no metas -- so the overstatement buys no projection. */
        assert_eq!(
            sysvar.immediate_post_v1(post_abi()),
            Err(InstructionsSysvarError::AccountIndexOutOfRange)
        );
    }

    #[test]
    fn a_zero_instruction_header_refuses() {
        let mut bytes = captured();
        bytes[..2].copy_from_slice(&0_u16.to_le_bytes());
        assert_eq!(
            sysvar(&bytes).err(),
            Some(InstructionsSysvarError::EmptyInstructionList)
        );
    }

    #[test]
    fn a_current_index_past_the_declared_count_refuses() {
        let mut bytes = captured();
        for current in [3_u16, 4, u16::MAX] {
            with_current(&mut bytes, current);
            assert_eq!(
                sysvar(&bytes).err(),
                Some(InstructionsSysvarError::CurrentIndexOutOfRange),
                "accepted current {current}"
            );
        }
    }

    #[test]
    fn malformed_offsets_refuse() {
        let bytes = captured();
        /* An offset past the instruction region. */
        for offset in [253_u16, 254, 255, 4_000, u16::MAX] {
            let mut broken = bytes;
            broken[OFFSET_TABLE_OFFSET + 2..OFFSET_TABLE_OFFSET + 4]
                .copy_from_slice(&offset.to_le_bytes());
            assert_eq!(
                ok(&broken).instruction_at(1),
                Err(InstructionsSysvarError::MalformedOffset),
                "accepted offset {offset}"
            );
        }
        /* An offset that lands inside the trailer rather than the body. */
        let mut into_trailer = bytes;
        into_trailer[OFFSET_TABLE_OFFSET + 2..OFFSET_TABLE_OFFSET + 4]
            .copy_from_slice(&252_u16.to_le_bytes());
        assert_eq!(
            ok(&into_trailer).instruction_at(1),
            Err(InstructionsSysvarError::MalformedOffset)
        );
    }

    #[test]
    fn an_overstated_account_count_refuses() {
        let bytes = captured();
        for count in [4_u16, 64, 2_000, u16::MAX] {
            let mut broken = bytes;
            broken[80..82].copy_from_slice(&count.to_le_bytes());
            assert_eq!(
                ok(&broken).instruction_at(1),
                Err(InstructionsSysvarError::MalformedOffset),
                "accepted account count {count}"
            );
        }
    }

    #[test]
    fn an_overstated_data_length_refuses() {
        let bytes = captured();
        /* Instruction 1's data_len sits at 80 + 2 + 99 + 32 == 213, so its data
         * starts at 215 and the 253-byte body admits at most 38 bytes. */
        for length in [39_u16, 40, 1_000, u16::MAX] {
            let mut broken = bytes;
            broken[213..215].copy_from_slice(&length.to_le_bytes());
            assert_eq!(
                ok(&broken).instruction_at(1),
                Err(InstructionsSysvarError::MalformedOffset),
                "accepted data length {length}"
            );
        }
    }

    #[test]
    fn an_account_meta_position_past_the_count_refuses() {
        let bytes = captured();
        let post = ok(&bytes).instruction_at(1).unwrap();
        for position in [3_u16, 4, u16::MAX] {
            assert_eq!(
                post.account_meta(position),
                Err(InstructionsSysvarError::AccountIndexOutOfRange),
                "accepted position {position}"
            );
        }
        let empty = ok(&bytes).instruction_at(2).unwrap();
        assert_eq!(
            empty.account_meta(0),
            Err(InstructionsSysvarError::AccountIndexOutOfRange)
        );
    }

    #[test]
    fn non_canonical_meta_flags_refuse() {
        for flags in [0x04_u8, 0x08, 0x40, 0x80, 0xfc, 0xff] {
            let data = built(RECEIVER, &[(flags, CONFIG)], 1);
            assert_eq!(
                ok(&data).instruction_at(0).unwrap().account_meta(0),
                Err(InstructionsSysvarError::NonCanonicalAccountMetaFlags),
                "accepted flag byte {flags:#04x}"
            );
        }
    }

    #[test]
    fn every_canonical_flag_combination_decodes() {
        for flags in 0_u8..=META_FLAG_MASK {
            let data = built(RECEIVER, &[(flags, CONFIG)], 1);
            assert_eq!(
                ok(&data)
                    .instruction_at(0)
                    .unwrap()
                    .account_meta(0)
                    .unwrap(),
                AccountMetaV1 {
                    address: CONFIG,
                    is_signer: flags & META_FLAG_IS_SIGNER != 0,
                    is_writable: flags & META_FLAG_IS_WRITABLE != 0,
                }
            );
        }
    }

    #[test]
    fn aliased_post_abi_positions_refuse() {
        let bytes = captured();
        let sysvar = ok(&bytes);
        for abi in [
            PostAbiPositionsV1 {
                config: 0,
                update_account: 0,
                write_authority: 2,
            },
            PostAbiPositionsV1 {
                config: 0,
                update_account: 1,
                write_authority: 0,
            },
            PostAbiPositionsV1 {
                config: 0,
                update_account: 1,
                write_authority: 1,
            },
        ] {
            assert_eq!(
                sysvar.immediate_post_v1(abi),
                Err(InstructionsSysvarError::AliasedPostAbiPositions),
                "accepted aliased ABI {abi:?}"
            );
        }
    }

    #[test]
    fn an_abi_position_the_post_lacks_refuses() {
        let bytes = captured();
        assert_eq!(
            ok(&bytes).immediate_post_v1(PostAbiPositionsV1 {
                config: 0,
                update_account: 1,
                write_authority: 3,
            }),
            Err(InstructionsSysvarError::AccountIndexOutOfRange)
        );
    }

    #[test]
    fn a_single_instruction_transaction_cannot_manufacture_a_post() {
        /* One instruction, index 0: nothing precedes it, so there is no post to
         * join even though the sysvar is perfectly well formed. */
        let mut data = Vec::new();
        data.extend_from_slice(&1_u16.to_le_bytes());
        data.extend_from_slice(&4_u16.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&RECEIVER);
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        assert_eq!(
            ok(&data).immediate_post_v1(post_abi()),
            Err(InstructionsSysvarError::NoPrecedingInstruction)
        );
    }
}
