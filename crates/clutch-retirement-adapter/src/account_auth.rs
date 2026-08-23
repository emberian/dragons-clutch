// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_retirement::{
    Identity32V1, RetirementErrorV1, RetirementErrorV2, DIRECT_RESERVATION_ACCOUNT_VERSION_V6,
    DIRECT_RESERVATION_ACCOUNT_VERSION_V8, DIRECT_RESERVATION_V6_BYTES,
    DIRECT_RESERVATION_V8_BYTES, EPOCH_ACCOUNT_TAG, EPOCH_ACCOUNT_VERSION_V5, EPOCH_V5_BYTES,
    GENERAL_EPOCH_TOMBSTONE_TAG, GENERAL_EPOCH_TOMBSTONE_V1_BYTES,
    GENERAL_EPOCH_TOMBSTONE_VERSION_V1, MARKET_ACCOUNT_TAG, MARKET_ACCOUNT_VERSION_V2,
    MARKET_V2_BYTES, POSITION_ACCOUNT_TAG, POSITION_ACCOUNT_VERSION_V2,
    POSITION_ACCOUNT_VERSION_V3, POSITION_TOMBSTONE_TAG, POSITION_TOMBSTONE_V1_BYTES,
    POSITION_TOMBSTONE_V2_BYTES, POSITION_TOMBSTONE_V3_BYTES, POSITION_TOMBSTONE_VERSION_V1,
    POSITION_TOMBSTONE_VERSION_V2, POSITION_TOMBSTONE_VERSION_V3, POSITION_V2_BYTES,
    POSITION_V3_BYTES, PURPOSE_REPLAY_ACCOUNT_TAG, PURPOSE_REPLAY_ACCOUNT_VERSION_V3,
    PURPOSE_REPLAY_V3_PREFIX_BYTES, RESERVATION_ACCOUNT_TAG, RESERVATION_ACCOUNT_VERSION_V5,
    RESERVATION_ACCOUNT_VERSION_V7, RESERVATION_V5_BYTES, RESERVATION_V7_BYTES,
};
use clutch_solana_layout::direct_selection_v3::{DIRECT_EPOCH_V4_BYTES, DIRECT_EPOCH_V4_VERSION};
use clutch_solana_layout::registry::{
    REPLAY_SUCCESSOR_ACCOUNT_TAG, REPLAY_SUCCESSOR_ACCOUNT_VERSION,
};

use crate::{RetirementAdapterErrorV1, RetirementAdapterErrorV2};
use clutch_general_v2_contract::{
    FINAL_POT_ACCOUNT_BYTES, FINAL_POT_ACCOUNT_TAG, FINAL_POT_ACCOUNT_VERSION,
    GENERAL_EPOCH_ACCOUNT_BYTES, GENERAL_EPOCH_ACCOUNT_TAG, GENERAL_EPOCH_ACCOUNT_VERSION,
    OWNER_FEE_CARRY_ACCOUNT_TAG, OWNER_FEE_FINALIZATION_ACCOUNT_BYTES,
    OWNER_FEE_FINALIZATION_ACCOUNT_VERSION, OWNER_SETTLEMENT_ACCOUNT_BYTES,
    OWNER_SETTLEMENT_ACCOUNT_TAG, OWNER_SETTLEMENT_ACCOUNT_VERSION,
    SELECTED_CANDIDATE_ACCOUNT_BYTES, SELECTED_CANDIDATE_ACCOUNT_TAG,
    SELECTED_CANDIDATE_ACCOUNT_VERSION,
};

const POSITION_STORED_BUMP_OFFSET: usize = 218;
const POSITION_V3_STORED_BUMP_OFFSET: usize = 5;
const MARKET_STORED_BUMP_OFFSET: usize = 132;
const EPOCH_STORED_BUMP_OFFSET: usize = 327;
const DIRECT_EPOCH_V4_STORED_BUMP_OFFSET: usize = 343;
const RESERVATION_STORED_BUMP_OFFSET: usize = 312;
const POSITION_TOMBSTONE_STORED_BUMP_OFFSET: usize = 75;
const POSITION_TOMBSTONE_V3_STORED_BUMP_OFFSET: usize = 4;
const EPOCH_TOMBSTONE_STORED_BUMP_OFFSET: usize = 83;
const REPLAY_SUCCESSOR_STORED_BUMP_OFFSET: usize = 82;
const PURPOSE_REPLAY_V3_STORED_BUMP_OFFSET: usize = 4;
const EPOCH_BUDGET_STORED_BUMP_OFFSET: usize = 270;
const GENERAL_V2_EPOCH_STORED_BUMP_OFFSET: usize = GENERAL_EPOCH_ACCOUNT_BYTES - 2;
const GENERAL_V2_SELECTED_STORED_BUMP_OFFSET: usize = SELECTED_CANDIDATE_ACCOUNT_BYTES - 2;
const GENERAL_V2_OWNER_SETTLEMENT_STORED_BUMP_OFFSET: usize = OWNER_SETTLEMENT_ACCOUNT_BYTES - 2;
const GENERAL_V2_OWNER_FEE_FINALIZATION_STORED_BUMP_OFFSET: usize =
    OWNER_FEE_FINALIZATION_ACCOUNT_BYTES - 2;
const GENERAL_V2_FINAL_POT_STORED_BUMP_OFFSET: usize = FINAL_POT_ACCOUNT_BYTES - 2;

/// Runtime facts read from one Solana account before any state mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountViewV1<'a> {
    /// Actual account address.
    pub address: Identity32V1,
    /// Actual runtime owner program.
    pub owner: Identity32V1,
    /// Entire current account data slice.
    pub data: &'a [u8],
    /// Whether the transaction declared the account writable.
    pub is_writable: bool,
}

/// Complete runtime metadata required by successor account authentication.
///
/// V1 deliberately omitted the executable bit, so it cannot authorize a live
/// mutation path. This successor view makes executable state explicit without
/// changing the frozen V1 API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountViewV2<'a> {
    /// Actual account address.
    pub address: Identity32V1,
    /// Actual runtime owner program.
    pub owner: Identity32V1,
    /// Entire current account data slice.
    pub data: &'a [u8],
    /// Whether the transaction declared the account writable.
    pub is_writable: bool,
    /// Whether the runtime marks the account executable.
    pub is_executable: bool,
}

/// Runtime facts for proving absence at one canonical PDA.
///
/// Owner bytes are raw because the System program's all-zero address is not a
/// live protocol identity. A positive lamport prefund is allowed: it does not
/// make a System-owned zero-data slot initialized and cannot block reopen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbsentAccountViewV1 {
    /// Actual account address.
    pub address: Identity32V1,
    /// Actual owner bytes, expected to be the all-zero System program id.
    pub owner: [u8; 32],
    /// Actual account data length.
    pub data_len: usize,
    /// Whether the transaction declared the evidence writable.
    pub is_writable: bool,
    /// Whether the runtime marks the slot executable.
    pub is_executable: bool,
}

/// Exact transaction access required for one non-executable state account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountAccessV2 {
    /// The account must be read-only.
    ReadOnly,
    /// The account must be writable.
    Writable,
}

/// Canonical PDA output already derived from exact seeds by the Solana adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalPdaV1 {
    address: Identity32V1,
    bump: u8,
}

impl CanonicalPdaV1 {
    /// Construct only after the adapter derives the PDA from the instruction's
    /// exact seed schema and the authenticated program id.
    pub const fn after_derivation(address: Identity32V1, bump: u8) -> Self {
        Self { address, bump }
    }

    /// Derived address.
    pub const fn address(self) -> Identity32V1 {
        self.address
    }

    /// Derived canonical bump.
    pub const fn bump(self) -> u8 {
        self.bump
    }
}

/// A runtime view that passed address, owner, mutability, exact-header,
/// exact-length, and stored-bump checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedAccountV1<'a> {
    view: AccountViewV1<'a>,
    canonical_bump: u8,
}

/// A successor runtime view that passed exact address, owner, executable,
/// mutability, header, length, and stored-bump checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedAccountV2<'a> {
    view: AccountViewV2<'a>,
    canonical_bump: u8,
}

impl<'a> AuthenticatedAccountV2<'a> {
    /// Exact authenticated bytes passed to the semantic codec next.
    pub const fn data(self) -> &'a [u8] {
        self.view.data
    }

    /// Authenticated canonical address.
    pub const fn address(self) -> Identity32V1 {
        self.view.address
    }

    /// Canonical derived and stored bump.
    pub const fn bump(self) -> u8 {
        self.canonical_bump
    }

    /// Whether the exact authenticated runtime role is writable.
    pub const fn is_writable(self) -> bool {
        self.view.is_writable
    }

    /// Exact program owner authenticated for this account family.
    pub const fn program_id(self) -> Identity32V1 {
        self.view.owner
    }
}

impl<'a> AuthenticatedAccountV1<'a> {
    /// Exact authenticated data passed to a semantic codec next.
    pub const fn data(self) -> &'a [u8] {
        self.view.data
    }

    /// Authenticated canonical address.
    pub const fn address(self) -> Identity32V1 {
        self.view.address
    }

    /// Canonical derived and stored bump.
    pub const fn bump(self) -> u8 {
        self.canonical_bump
    }
}

/// Registry-supplied shape for a version-bumped child with an eight-byte
/// parent-generation tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedChildSchemaV1 {
    tag: u8,
    legacy_version: u8,
    counted_version: u8,
    legacy_len: usize,
    counted_len: usize,
    stored_bump_offset: usize,
}

impl CountedChildSchemaV1 {
    /// Construct only from a globally allocated tag/version pair and the
    /// authoritative legacy codec's exact length and bump offset.
    pub const fn after_registry_allocation(
        tag: u8,
        legacy_version: u8,
        counted_version: u8,
        legacy_len: usize,
        stored_bump_offset: usize,
    ) -> Result<Self, RetirementAdapterErrorV1> {
        if legacy_len < 2 || stored_bump_offset >= legacy_len || legacy_version == counted_version {
            return Err(RetirementAdapterErrorV1::InvalidSchema);
        }
        let counted_len = match legacy_len.checked_add(8) {
            Some(value) => value,
            None => return Err(RetirementAdapterErrorV1::InvalidSchema),
        };
        Ok(Self {
            tag,
            legacy_version,
            counted_version,
            legacy_len,
            counted_len,
            stored_bump_offset,
        })
    }

    pub(crate) const fn tag(self) -> u8 {
        self.tag
    }

    pub(crate) const fn legacy_version(self) -> u8 {
        self.legacy_version
    }

    pub(crate) const fn counted_version(self) -> u8 {
        self.counted_version
    }

    /// Exact legacy body width before appending the generation.
    pub const fn legacy_len(self) -> usize {
        self.legacy_len
    }

    /// Exact promoted width including the generation.
    pub const fn counted_len(self) -> usize {
        self.counted_len
    }
}

#[derive(Clone, Copy)]
struct ExpectedAccountV1 {
    tag: u8,
    version: u8,
    len: usize,
    bump_offset: usize,
}

fn authenticate_v2<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    expected: ExpectedAccountV1,
    access: AccountAccessV2,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    if view.address != canonical_pda.address {
        return Err(RetirementAdapterErrorV2::WrongPda);
    }
    if view.owner != program_id {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    if view.is_executable {
        return Err(RetirementAdapterErrorV2::ExecutableAccount);
    }
    match access {
        AccountAccessV2::ReadOnly if view.is_writable => {
            return Err(RetirementAdapterErrorV2::UnexpectedWritable)
        }
        AccountAccessV2::Writable if !view.is_writable => {
            return Err(RetirementAdapterErrorV2::NotWritable)
        }
        AccountAccessV2::ReadOnly | AccountAccessV2::Writable => {}
    }
    if view.data.len() < expected.len {
        return Err(RetirementErrorV2::Truncated.into());
    }
    if view.data.len() > expected.len {
        return Err(RetirementErrorV2::TrailingBytes.into());
    }
    if expected.len < 2 || expected.bump_offset >= expected.len {
        return Err(RetirementAdapterErrorV2::InvalidSchema);
    }
    if view.data[0] != expected.tag {
        return Err(RetirementErrorV2::WrongTag.into());
    }
    if view.data[1] != expected.version {
        return Err(RetirementErrorV2::WrongVersion.into());
    }
    if view.data[expected.bump_offset] != canonical_pda.bump {
        return Err(RetirementAdapterErrorV2::WrongBump);
    }
    Ok(AuthenticatedAccountV2 {
        view,
        canonical_bump: canonical_pda.bump,
    })
}

/// Authenticate one exact read-only executable program role.
///
/// Program ownership and data geometry are loader-specific and deliberately
/// are not inferred here. The live adapter separately supplies the exact
/// expected program address; this function proves only the runtime properties
/// shared by executable roles.
pub fn authenticate_runtime_executable_v2(
    view: AccountViewV2<'_>,
    expected_address: Identity32V1,
) -> Result<(), RetirementAdapterErrorV2> {
    if view.address != expected_address {
        return Err(RetirementAdapterErrorV2::WrongProgramAddress);
    }
    if view.is_writable {
        return Err(RetirementAdapterErrorV2::UnexpectedWritable);
    }
    if !view.is_executable {
        return Err(RetirementAdapterErrorV2::NotExecutable);
    }
    Ok(())
}

/// Authenticate a variable-width purpose-owned Replay V3 envelope.
///
/// This performs only runtime identity, owner, access, common-header,
/// minimum-length, and stored-bump checks. The caller must next invoke
/// `ReplayV3Envelope::decode` with its concrete hash backend; that decoder
/// enforces the extension's exact committed length and full hash.
pub fn authenticate_purpose_replay_v3_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    access: AccountAccessV2,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    if view.address != canonical_pda.address {
        return Err(RetirementAdapterErrorV2::WrongPda);
    }
    if view.owner != program_id {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    if view.is_executable {
        return Err(RetirementAdapterErrorV2::ExecutableAccount);
    }
    match access {
        AccountAccessV2::ReadOnly if view.is_writable => {
            return Err(RetirementAdapterErrorV2::UnexpectedWritable)
        }
        AccountAccessV2::Writable if !view.is_writable => {
            return Err(RetirementAdapterErrorV2::NotWritable)
        }
        AccountAccessV2::ReadOnly | AccountAccessV2::Writable => {}
    }
    if view.data.len() < PURPOSE_REPLAY_V3_PREFIX_BYTES + 1 {
        return Err(RetirementErrorV2::Truncated.into());
    }
    if view.data[0] != PURPOSE_REPLAY_ACCOUNT_TAG {
        return Err(RetirementErrorV2::WrongTag.into());
    }
    if view.data[1] != PURPOSE_REPLAY_ACCOUNT_VERSION_V3 {
        return Err(RetirementErrorV2::WrongVersion.into());
    }
    if view.data[PURPOSE_REPLAY_V3_STORED_BUMP_OFFSET] != canonical_pda.bump {
        return Err(RetirementAdapterErrorV2::WrongBump);
    }
    Ok(AuthenticatedAccountV2 {
        view,
        canonical_bump: canonical_pda.bump,
    })
}

/// Authenticate the exact fresh counted General Epoch V6 account.
pub fn authenticate_general_epoch_v6_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    access: AccountAccessV2,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: GENERAL_EPOCH_ACCOUNT_TAG,
            version: GENERAL_EPOCH_ACCOUNT_VERSION,
            len: GENERAL_EPOCH_ACCOUNT_BYTES,
            bump_offset: GENERAL_V2_EPOCH_STORED_BUMP_OFFSET,
        },
        access,
    )
}

/// Authenticate one exact General SelectedCandidate account.
pub fn authenticate_general_selected_candidate_v1_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    access: AccountAccessV2,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: SELECTED_CANDIDATE_ACCOUNT_TAG,
            version: SELECTED_CANDIDATE_ACCOUNT_VERSION,
            len: SELECTED_CANDIDATE_ACCOUNT_BYTES,
            bump_offset: GENERAL_V2_SELECTED_STORED_BUMP_OFFSET,
        },
        access,
    )
}

/// Authenticate one exact writable General owner-settlement row.
pub fn authenticate_general_owner_settlement_v1_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: OWNER_SETTLEMENT_ACCOUNT_TAG,
            version: OWNER_SETTLEMENT_ACCOUNT_VERSION,
            len: OWNER_SETTLEMENT_ACCOUNT_BYTES,
            bump_offset: GENERAL_V2_OWNER_SETTLEMENT_STORED_BUMP_OFFSET,
        },
        AccountAccessV2::Writable,
    )
}

/// Authenticate one exact writable 0x83/version-2 owner fee receipt.
pub fn authenticate_general_owner_fee_finalization_v2_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: OWNER_FEE_CARRY_ACCOUNT_TAG,
            version: OWNER_FEE_FINALIZATION_ACCOUNT_VERSION,
            len: OWNER_FEE_FINALIZATION_ACCOUNT_BYTES,
            bump_offset: GENERAL_V2_OWNER_FEE_FINALIZATION_STORED_BUMP_OFFSET,
        },
        AccountAccessV2::Writable,
    )
}

/// Authenticate the exact writable 332-byte General FinalPot.
pub fn authenticate_general_final_pot_v1_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: FINAL_POT_ACCOUNT_TAG,
            version: FINAL_POT_ACCOUNT_VERSION,
            len: FINAL_POT_ACCOUNT_BYTES,
            bump_offset: GENERAL_V2_FINAL_POT_STORED_BUMP_OFFSET,
        },
        AccountAccessV2::Writable,
    )
}

/// Authenticate Position V2 under an exact read-only or writable role.
pub fn authenticate_position_v2_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    access: AccountAccessV2,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: POSITION_ACCOUNT_TAG,
            version: POSITION_ACCOUNT_VERSION_V2,
            len: POSITION_V2_BYTES,
            bump_offset: POSITION_STORED_BUMP_OFFSET,
        },
        access,
    )
}

/// Authenticate the canonical global Position V3 under an exact runtime role.
pub fn authenticate_position_v3_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    access: AccountAccessV2,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: POSITION_ACCOUNT_TAG,
            version: POSITION_ACCOUNT_VERSION_V3,
            len: POSITION_V3_BYTES,
            bump_offset: POSITION_V3_STORED_BUMP_OFFSET,
        },
        access,
    )
}

/// Authenticate Market V2 under an exact read-only or writable role.
pub fn authenticate_market_v2_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    access: AccountAccessV2,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: MARKET_ACCOUNT_TAG,
            version: MARKET_ACCOUNT_VERSION_V2,
            len: MARKET_V2_BYTES,
            bump_offset: MARKET_STORED_BUMP_OFFSET,
        },
        access,
    )
}

/// Authenticate general Epoch V5 under an exact read-only or writable role.
pub fn authenticate_general_epoch_v5_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    access: AccountAccessV2,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: EPOCH_ACCOUNT_TAG,
            version: EPOCH_ACCOUNT_VERSION_V5,
            len: EPOCH_V5_BYTES,
            bump_offset: EPOCH_STORED_BUMP_OFFSET,
        },
        access,
    )
}

/// Authenticate the exact 132-byte generation-scoped Replay successor.
pub fn authenticate_replay_successor_v1_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    access: AccountAccessV2,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: REPLAY_SUCCESSOR_ACCOUNT_TAG,
            version: REPLAY_SUCCESSOR_ACCOUNT_VERSION,
            len: clutch_retirement::PROJECTED_REPLAY_SUCCESSOR_BYTES,
            bump_offset: REPLAY_SUCCESSOR_STORED_BUMP_OFFSET,
        },
        access,
    )
}

/// Authenticate one exact writable permanent Position tombstone for reopen.
pub fn authenticate_position_tombstone_v1_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: POSITION_TOMBSTONE_TAG,
            version: POSITION_TOMBSTONE_VERSION_V1,
            len: POSITION_TOMBSTONE_V1_BYTES,
            bump_offset: POSITION_TOMBSTONE_STORED_BUMP_OFFSET,
        },
        AccountAccessV2::Writable,
    )
}

/// Authenticate one exact writable rent-owner-complete Position tombstone V2.
pub fn authenticate_position_tombstone_v2_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: POSITION_TOMBSTONE_TAG,
            version: POSITION_TOMBSTONE_VERSION_V2,
            len: POSITION_TOMBSTONE_V2_BYTES,
            bump_offset: POSITION_TOMBSTONE_STORED_BUMP_OFFSET,
        },
        AccountAccessV2::Writable,
    )
}

/// Authenticate the canonical full-identity Position V3 tombstone for reopen.
pub fn authenticate_position_tombstone_v3_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: POSITION_TOMBSTONE_TAG,
            version: POSITION_TOMBSTONE_VERSION_V3,
            len: POSITION_TOMBSTONE_V3_BYTES,
            bump_offset: POSITION_TOMBSTONE_V3_STORED_BUMP_OFFSET,
        },
        AccountAccessV2::Writable,
    )
}

/// Authenticate one exact writable permanent general Epoch tombstone.
pub fn authenticate_general_epoch_tombstone_v1_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: GENERAL_EPOCH_TOMBSTONE_TAG,
            version: GENERAL_EPOCH_TOMBSTONE_VERSION_V1,
            len: GENERAL_EPOCH_TOMBSTONE_V1_BYTES,
            bump_offset: EPOCH_TOMBSTONE_STORED_BUMP_OFFSET,
        },
        AccountAccessV2::Writable,
    )
}

/// Authenticate the authoritative General V2 Epoch Budget under an exact role.
pub fn authenticate_epoch_budget_v1_exact<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    access: AccountAccessV2,
) -> Result<AuthenticatedAccountV2<'a>, RetirementAdapterErrorV2> {
    authenticate_v2(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: clutch_general_v2_contract::EPOCH_BUDGET_ACCOUNT_TAG,
            version: clutch_general_v2_contract::EPOCH_BUDGET_ACCOUNT_VERSION,
            len: clutch_general_v2_contract::EPOCH_BUDGET_ACCOUNT_BYTES,
            bump_offset: EPOCH_BUDGET_STORED_BUMP_OFFSET,
        },
        access,
    )
}

/// Prove canonical prior-generation Replay absence for generation-safe reopen.
///
/// This authenticates only runtime absence and the exact PDA. The returned
/// pure projection also binds the seed inputs so `reopen_position_with_replay`
/// can reject substitution. The caller must present this role read-only.
pub fn authenticate_replay_absence_v1_exact(
    view: AbsentAccountViewV1,
    canonical_pda: CanonicalPdaV1,
    market: Identity32V1,
    owner: Identity32V1,
    position_generation: u64,
) -> Result<clutch_retirement::AdapterReplayAbsenceProjectionV1, RetirementAdapterErrorV2> {
    if view.address != canonical_pda.address() {
        return Err(RetirementAdapterErrorV2::WrongPda);
    }
    if view.is_writable {
        return Err(RetirementAdapterErrorV2::UnexpectedWritable);
    }
    if view.is_executable || view.owner != [0; 32] || view.data_len != 0 {
        return Err(RetirementAdapterErrorV2::AccountNotAbsent);
    }
    Ok(clutch_retirement::AdapterReplayAbsenceProjectionV1 {
        account: view.address,
        market,
        owner,
        position_generation,
    })
}

fn authenticate<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    expected: ExpectedAccountV1,
    require_writable: bool,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    if view.address != canonical_pda.address {
        return Err(RetirementAdapterErrorV1::WrongPda);
    }
    if view.owner != program_id {
        return Err(RetirementAdapterErrorV1::WrongOwner);
    }
    if require_writable && !view.is_writable {
        return Err(RetirementAdapterErrorV1::NotWritable);
    }
    if view.data.len() < expected.len {
        return Err(RetirementErrorV1::Truncated.into());
    }
    if view.data.len() > expected.len {
        return Err(RetirementErrorV1::TrailingBytes.into());
    }
    if expected.len < 2 || expected.bump_offset >= expected.len {
        return Err(RetirementAdapterErrorV1::InvalidSchema);
    }
    if view.data[0] != expected.tag {
        return Err(RetirementErrorV1::WrongTag.into());
    }
    if view.data[1] != expected.version {
        return Err(RetirementErrorV1::WrongVersion.into());
    }
    if view.data[expected.bump_offset] != canonical_pda.bump {
        return Err(RetirementAdapterErrorV1::WrongBump);
    }
    Ok(AuthenticatedAccountV1 {
        view,
        canonical_bump: canonical_pda.bump,
    })
}

/// Authenticate one writable counted Position V2 before decoding it.
pub fn authenticate_position_v2<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: POSITION_ACCOUNT_TAG,
            version: POSITION_ACCOUNT_VERSION_V2,
            len: POSITION_V2_BYTES,
            bump_offset: POSITION_STORED_BUMP_OFFSET,
        },
        true,
    )
}

/// Authenticate one writable monotone-cursor Market V2 before decoding it.
pub fn authenticate_market_v2<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: MARKET_ACCOUNT_TAG,
            version: MARKET_ACCOUNT_VERSION_V2,
            len: MARKET_V2_BYTES,
            bump_offset: MARKET_STORED_BUMP_OFFSET,
        },
        true,
    )
}

/// Authenticate one writable counted general Epoch V5 before decoding it.
pub fn authenticate_general_epoch_v5<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: EPOCH_ACCOUNT_TAG,
            version: EPOCH_ACCOUNT_VERSION_V5,
            len: EPOCH_V5_BYTES,
            bump_offset: EPOCH_STORED_BUMP_OFFSET,
        },
        true,
    )
}

/// Authenticate one read-only or writable Direct Epoch V4 before its
/// authoritative codec projects the parent identity, admission lifecycle, and
/// persisted neutral sink.
pub fn authenticate_direct_epoch_v4<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV2> {
    Ok(authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: EPOCH_ACCOUNT_TAG,
            version: DIRECT_EPOCH_V4_VERSION,
            len: DIRECT_EPOCH_V4_BYTES,
            bump_offset: DIRECT_EPOCH_V4_STORED_BUMP_OFFSET,
        },
        false,
    )?)
}

/// Authenticate one writable counted general Reservation V5.
pub fn authenticate_general_reservation_v5<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: RESERVATION_ACCOUNT_TAG,
            version: RESERVATION_ACCOUNT_VERSION_V5,
            len: RESERVATION_V5_BYTES,
            bump_offset: RESERVATION_STORED_BUMP_OFFSET,
        },
        true,
    )
}

/// Authenticate one writable counted direct Reservation V6.
pub fn authenticate_direct_reservation_v6<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: RESERVATION_ACCOUNT_TAG,
            version: DIRECT_RESERVATION_ACCOUNT_VERSION_V6,
            len: DIRECT_RESERVATION_V6_BYTES,
            bump_offset: RESERVATION_STORED_BUMP_OFFSET,
        },
        true,
    )
}

/// Authenticate one writable deletable counted general Reservation V7.
pub fn authenticate_general_reservation_v7<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV2> {
    Ok(authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: RESERVATION_ACCOUNT_TAG,
            version: RESERVATION_ACCOUNT_VERSION_V7,
            len: RESERVATION_V7_BYTES,
            bump_offset: RESERVATION_STORED_BUMP_OFFSET,
        },
        true,
    )?)
}

/// Authenticate one writable deletable counted direct Reservation V8.
pub fn authenticate_direct_reservation_v8<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV2> {
    Ok(authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: RESERVATION_ACCOUNT_TAG,
            version: DIRECT_RESERVATION_ACCOUNT_VERSION_V8,
            len: DIRECT_RESERVATION_V8_BYTES,
            bump_offset: RESERVATION_STORED_BUMP_OFFSET,
        },
        true,
    )?)
}

/// Authenticate one writable permanent Position tombstone.
pub fn authenticate_position_tombstone_v1<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: POSITION_TOMBSTONE_TAG,
            version: POSITION_TOMBSTONE_VERSION_V1,
            len: POSITION_TOMBSTONE_V1_BYTES,
            bump_offset: POSITION_TOMBSTONE_STORED_BUMP_OFFSET,
        },
        true,
    )
}

/// Authenticate one writable permanent general Epoch tombstone.
pub fn authenticate_general_epoch_tombstone_v1<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: GENERAL_EPOCH_TOMBSTONE_TAG,
            version: GENERAL_EPOCH_TOMBSTONE_VERSION_V1,
            len: GENERAL_EPOCH_TOMBSTONE_V1_BYTES,
            bump_offset: EPOCH_TOMBSTONE_STORED_BUMP_OFFSET,
        },
        true,
    )
}

/// Authenticate one writable counted child using its globally allocated
/// schema. Its semantic owner must decode the downgraded base next.
pub fn authenticate_counted_child<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    schema: CountedChildSchemaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: schema.tag,
            version: schema.counted_version,
            len: schema.counted_len,
            bump_offset: schema.stored_bump_offset,
        },
        true,
    )
}

const _: () = assert!(
    POSITION_TOMBSTONE_TAG
        == clutch_solana_layout::registry::RETIREMENT_POSITION_TOMBSTONE_ACCOUNT_TAG
);
const _: () = assert!(
    POSITION_TOMBSTONE_VERSION_V2
        == clutch_solana_layout::registry::RETIREMENT_POSITION_TOMBSTONE_ACCOUNT_VERSION_V2
);

const _: () = assert!(POSITION_STORED_BUMP_OFFSET < POSITION_V2_BYTES);
const _: () = assert!(MARKET_STORED_BUMP_OFFSET < MARKET_V2_BYTES);
const _: () = assert!(EPOCH_STORED_BUMP_OFFSET < EPOCH_V5_BYTES);
const _: () = assert!(DIRECT_EPOCH_V4_STORED_BUMP_OFFSET < DIRECT_EPOCH_V4_BYTES);
const _: () = assert!(RESERVATION_STORED_BUMP_OFFSET < RESERVATION_V5_BYTES);
const _: () = assert!(RESERVATION_STORED_BUMP_OFFSET < RESERVATION_V7_BYTES);
const _: () = assert!(RESERVATION_STORED_BUMP_OFFSET < DIRECT_RESERVATION_V8_BYTES);
