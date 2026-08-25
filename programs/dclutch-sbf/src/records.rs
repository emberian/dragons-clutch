//! SVM adapter for grief-resistant immutable raw-record creation.
//!
//! Permanent record accounts contain exactly their semantic bytes. All
//! construction progress and rent ownership lives in the temporary cursor
//! defined by `dclutch-record-contract`. This module owns the selected V1 SVM
//! release: PDA derivation, account frames, current Rent and Clock reads,
//! System allocation, SHA-256, the closed Found-record schema set, atomic page
//! writes, and exact close/refund execution.

pub(crate) use dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1;
use dclutch_capability_contract::{
    CAPABILITY_ENTRY_BYTES, CapabilityManifestV1, MANIFEST_HEADER_BYTES, MAX_MANIFEST_BYTES,
};
use dclutch_core_contract::ContentId as CoreContentId;
#[cfg(test)]
use dclutch_dealer_contract::LiquidityConfigV1;
use dclutch_dealer_contract::{
    frame::DEALER_CONFIG_SCHEMA_RELEASE_ID_V1,
    runtime::{LiquidityConfigViewV1, LiquidityProfileV1},
};
use dclutch_direct_contract::{
    VENUE_FEE_POLICY_BYTES_V3, VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3, VenueFeePolicyV3,
};
use dclutch_general_contract::{
    GENERAL_CONFIG_BYTES, GENERAL_CONFIG_SCHEMA_ID_V1, GeneralConfigV1,
};
use dclutch_product_contract::{
    capacity::{CAPACITY_PROFILE_BYTES, CapacityProfileV1},
    claim::{CATEGORICAL_UNIT_BYTES, CategoricalUnitV1},
    product::{INSTANCE_BYTES, InstanceV1},
    result_domain::FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
};
use dclutch_realm_contract::{REALM_BYTES, RealmV1};
use dclutch_record_contract::{
    AbortObservationV1, AbortRecordV1, AbortTransitionV1, AccountCloseV1, AccountId,
    AddressDerivationObligationV1, AppendPageV1, BeginRecordV1, ContentDigest, FinalizeRecordV1,
    PageEnvelopeKindV1, PageEnvelopeV1, RAW_RECORD_PDA_SEED_V1, RawRecordValidationModeV1,
    RawRecordValidationObligationV1, RecordAdapterV1, RecordKeyV1, STAGING_CURSOR_BYTES_V1,
    STAGING_CURSOR_PDA_SEED_V1, SchemaReleaseId, StagingCursorV1, StagingLamportCloseV1,
    StagingLivenessPolicyV1, authenticate_finalized_raw_record_v1, prepare_abort_v1,
    prepare_append_page_v1, prepare_begin_v1, prepare_finalize_v1,
};
use dclutch_rent_contract::{
    RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1,
    SourceCloseCreditPlanV1,
};
use dclutch_series_contract::{
    CAPITALIZATION_AGGREGATE_BYTES_V1, CapitalizationAggregateV1, DERIVED_OCCURRENCE_BYTES_V1,
    DerivedOccurrenceV1, OCCURRENCE_CAPITALIZATION_BYTES_V1, OccurrenceCapitalizationV1,
    SERIES_RECIPE_BYTES_V1, SeriesRecipeV1,
};
use dclutch_source_contract::{
    SOURCE_MATERIAL_BYTES, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SourceMaterialViewV1,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::{hash, hashv},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, create_account, transfer};

use crate::AdapterError;

const BEGIN_ACCOUNTS_V1: usize = 7;
const APPEND_ACCOUNTS_V1: usize = 3;
const FINALIZE_ACCOUNTS_V1: usize = 3;
const ABORT_ACCOUNTS_V1: usize = 5;

/// Provisional transaction-envelope width selected by this SBF release.
const PAGE_BYTES_V1: u32 = 768;
/// Maximum staging lifetime selected by this SBF release.
///
/// This is a release/profile bound, not a protocol-time conversion. A later
/// measured release can change it without changing any finalized raw PDA.
const MAX_STAGING_LIFETIME_SLOTS_V1: u64 = 216_000;

const PAGE_ENVELOPE_RELEASE_ID_V1: [u8; 32] = [
    0x58, 0x8d, 0x8c, 0x5c, 0x3e, 0x18, 0x13, 0x26, 0xbf, 0x43, 0x44, 0xf5, 0x7d, 0x7b, 0xd7, 0xe3,
    0x14, 0x17, 0xad, 0x80, 0x59, 0x5e, 0xfc, 0x2c, 0x8e, 0x9a, 0x41, 0x53, 0x30, 0xed, 0xf3, 0x49,
];
const STAGING_LIVENESS_RELEASE_ID_V1: [u8; 32] = [
    0xd8, 0x98, 0x6b, 0x24, 0x77, 0x61, 0xf9, 0x90, 0x4e, 0xe6, 0x1b, 0x4f, 0x90, 0x8f, 0xca, 0x23,
    0xce, 0x68, 0x34, 0x58, 0x2c, 0xb4, 0x83, 0xf8, 0x7b, 0xf9, 0x18, 0x07, 0x27, 0xcb, 0x76, 0x56,
];
pub(crate) const REALM_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x94, 0xfe, 0x1f, 0xd6, 0xd7, 0x25, 0x9f, 0x47, 0x50, 0x3d, 0x6a, 0xc5, 0x7e, 0xc7, 0xda, 0x78,
    0xdc, 0x38, 0x06, 0xa5, 0xed, 0x49, 0x8f, 0xea, 0xe4, 0x3e, 0xd3, 0x78, 0x5b, 0x5d, 0x0c, 0x69,
];
pub(crate) const PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x96, 0x20, 0xbc, 0xd9, 0xf3, 0x1a, 0x01, 0xca, 0x6f, 0x42, 0x09, 0x1c, 0x84, 0x57, 0x9d, 0x9a,
    0xcc, 0x48, 0x41, 0x27, 0xc0, 0x8d, 0x86, 0xac, 0xc4, 0x0f, 0xdd, 0x5a, 0x4c, 0xab, 0x1f, 0x14,
];
pub(crate) const CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0xd3, 0x86, 0x0a, 0x71, 0x29, 0x03, 0x33, 0xab, 0x8d, 0x15, 0x43, 0x11, 0x75, 0x29, 0x30, 0x4f,
    0x1d, 0xcd, 0x2a, 0xe1, 0x42, 0xff, 0xdb, 0xd6, 0x0c, 0xc7, 0x15, 0xb8, 0x62, 0x58, 0xcc, 0x6d,
];
pub(crate) const CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0xed, 0x25, 0x2a, 0x2a, 0xc5, 0x55, 0xf0, 0xe3, 0x4f, 0xfc, 0x23, 0xac, 0x91, 0xd8, 0x6c, 0x61,
    0xbe, 0x6d, 0xd9, 0x81, 0x24, 0x47, 0x57, 0x49, 0x94, 0x69, 0xbb, 0x99, 0xba, 0x55, 0x36, 0x50,
];
pub(crate) const SERIES_RECIPE_SCHEMA_RELEASE_ID_V2: [u8; 32] = [
    0xff, 0xb3, 0x98, 0x35, 0xbf, 0x80, 0xbd, 0x3c, 0x21, 0x6f, 0xd9, 0x07, 0xa8, 0x0b, 0xc5, 0x88,
    0xab, 0x57, 0x82, 0x48, 0xe8, 0xea, 0x67, 0x46, 0x7d, 0x75, 0xce, 0x1a, 0xe7, 0xdc, 0x4c, 0x3c,
];
pub(crate) const SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x36, 0xdc, 0xc9, 0xd8, 0x3c, 0x7a, 0x89, 0xeb, 0xb2, 0x4f, 0xd2, 0x44, 0x79, 0x23, 0xca, 0x68,
    0x4c, 0x3c, 0x2c, 0x28, 0x20, 0x54, 0xc7, 0x58, 0x9c, 0x4a, 0xb3, 0x9d, 0xad, 0xec, 0x9a, 0xd5,
];
pub(crate) const SERIES_DERIVED_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x44, 0x14, 0xd1, 0xe5, 0x40, 0x3a, 0x59, 0x42, 0xfb, 0xf2, 0x88, 0x8f, 0x4a, 0x54, 0x84, 0x75,
    0x85, 0x62, 0xcc, 0xc7, 0xd4, 0xb0, 0x53, 0xd8, 0x96, 0x42, 0xc7, 0xee, 0x02, 0xd3, 0x5c, 0xc9,
];
pub(crate) const SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x55, 0x8b, 0xc4, 0xd4, 0x83, 0xae, 0xc0, 0x5c, 0x81, 0x85, 0x33, 0x65, 0x6c, 0x58, 0xf7, 0x7c,
    0x7f, 0x16, 0xe0, 0xb3, 0x42, 0x8f, 0x05, 0xe5, 0xa5, 0xfc, 0x97, 0x69, 0x3a, 0x1d, 0x9f, 0xdf,
];

#[cfg(test)]
const RELEASE_LABELS_AND_IDS_V1: [(&[u8], [u8; 32]); 15] = [
    (
        b"dclutch/sbf-record-page-envelope/provisional-v1",
        PAGE_ENVELOPE_RELEASE_ID_V1,
    ),
    (
        b"dclutch/sbf-record-staging-liveness/v1",
        STAGING_LIVENESS_RELEASE_ID_V1,
    ),
    (b"dclutch/schema/realm-v1", REALM_SCHEMA_RELEASE_ID_V1),
    (
        b"dclutch/schema/product-instance-v1",
        PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
    ),
    (
        b"dclutch/schema/categorical-unit-claim-v1",
        CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1,
    ),
    (
        b"dclutch/schema/product-capacity-profile-v1",
        CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1,
    ),
    (
        b"dclutch/schema/capability-manifest-profile-1-v1",
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    ),
    (
        b"dclutch/schema/direct-venue-fee-policy-v3",
        VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3,
    ),
    (
        b"dclutch/source-material-schema/v1",
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
    ),
    (
        b"dclutch/schema/series-recipe-v2",
        SERIES_RECIPE_SCHEMA_RELEASE_ID_V2,
    ),
    (
        b"dclutch/schema/series-capitalization-aggregate-v1",
        SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1,
    ),
    (
        b"dclutch/schema/series-derived-occurrence-v1",
        SERIES_DERIVED_SCHEMA_RELEASE_ID_V1,
    ),
    (
        b"dclutch/schema/series-occurrence-capitalization-v1",
        SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1,
    ),
    (
        b"dclutch/dealer-liquidity-config/schema/v1",
        DEALER_CONFIG_SCHEMA_RELEASE_ID_V1,
    ),
    (
        b"dclutch/schema/general-config-v1",
        GENERAL_CONFIG_SCHEMA_ID_V1.to_bytes(),
    ),
];

const _: () = assert!(RAW_RECORD_PDA_SEED_V1.len() <= 32);
const _: () = assert!(STAGING_CURSOR_PDA_SEED_V1.len() <= 32);
const _: () = assert!(RENT_CREDIT_PDA_DOMAIN_V1.len() <= 32);

/// Dispatch one exact immutable-record request after top-level magic routing.
pub(crate) fn dispatch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    match instruction_data.get(10).copied() {
        Some(1) => BeginRecordV1::decode(instruction_data)
            .map_err(map_record_error)
            .and_then(|instruction| process_begin(program_id, accounts, instruction)),
        Some(2) => AppendPageV1::decode(instruction_data)
            .map_err(map_record_error)
            .and_then(|instruction| process_append(program_id, accounts, instruction)),
        Some(3) => FinalizeRecordV1::decode(instruction_data)
            .map_err(map_record_error)
            .and_then(|instruction| process_finalize(program_id, accounts, instruction)),
        Some(4) => AbortRecordV1::decode(instruction_data)
            .map_err(map_record_error)
            .and_then(|instruction| process_abort(program_id, accounts, instruction)),
        _ => Err(AdapterError::InvalidInstruction.into()),
    }
}

struct BeginFrame<'a, 'info> {
    sponsor: &'a AccountInfo<'info>,
    raw_record: &'a AccountInfo<'info>,
    cursor: &'a AccountInfo<'info>,
    rent_credit: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    rent_sysvar: &'a AccountInfo<'info>,
    clock_sysvar: &'a AccountInfo<'info>,
}

impl<'a, 'info> BeginFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != BEGIN_ACCOUNTS_V1 {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            sponsor: account(accounts, 0)?,
            raw_record: account(accounts, 1)?,
            cursor: account(accounts, 2)?,
            rent_credit: account(accounts, 3)?,
            system_program: account(accounts, 4)?,
            rent_sysvar: account(accounts, 5)?,
            clock_sysvar: account(accounts, 6)?,
        };
        require_privilege(frame.sponsor, true, true, false)?;
        require_privilege(frame.raw_record, false, true, false)?;
        require_privilege(frame.cursor, false, true, false)?;
        require_privilege(frame.rent_credit, false, false, false)?;
        require_privilege(frame.system_program, false, false, true)?;
        require_privilege(frame.rent_sysvar, false, false, false)?;
        require_privilege(frame.clock_sysvar, false, false, false)?;
        require_distinct(accounts)?;
        Ok(frame)
    }
}

struct AppendFrame<'a, 'info> {
    sponsor: &'a AccountInfo<'info>,
    raw_record: &'a AccountInfo<'info>,
    cursor: &'a AccountInfo<'info>,
}

impl<'a, 'info> AppendFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != APPEND_ACCOUNTS_V1 {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            sponsor: account(accounts, 0)?,
            raw_record: account(accounts, 1)?,
            cursor: account(accounts, 2)?,
        };
        require_readonly_signer(frame.sponsor)?;
        require_privilege(frame.raw_record, false, true, false)?;
        require_privilege(frame.cursor, false, true, false)?;
        require_distinct(accounts)?;
        Ok(frame)
    }
}

struct FinalizeFrame<'a, 'info> {
    raw_record: &'a AccountInfo<'info>,
    cursor: &'a AccountInfo<'info>,
    rent_credit: &'a AccountInfo<'info>,
}

impl<'a, 'info> FinalizeFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != FINALIZE_ACCOUNTS_V1 {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            raw_record: account(accounts, 0)?,
            cursor: account(accounts, 1)?,
            rent_credit: account(accounts, 2)?,
        };
        require_privilege(frame.raw_record, false, false, false)?;
        require_privilege(frame.cursor, false, true, false)?;
        require_privilege(frame.rent_credit, false, true, false)?;
        require_distinct(accounts)?;
        Ok(frame)
    }
}

struct AbortFrame<'a, 'info> {
    actor: &'a AccountInfo<'info>,
    raw_record: &'a AccountInfo<'info>,
    cursor: &'a AccountInfo<'info>,
    rent_credit: &'a AccountInfo<'info>,
    clock_sysvar: &'a AccountInfo<'info>,
}

impl<'a, 'info> AbortFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != ABORT_ACCOUNTS_V1 {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            actor: account(accounts, 0)?,
            raw_record: account(accounts, 1)?,
            cursor: account(accounts, 2)?,
            rent_credit: account(accounts, 3)?,
            clock_sysvar: account(accounts, 4)?,
        };
        require_signer(frame.actor)?;
        require_privilege(frame.raw_record, false, true, false)?;
        require_privilege(frame.cursor, false, true, false)?;
        require_privilege(frame.rent_credit, false, true, false)?;
        require_privilege(frame.clock_sysvar, false, false, false)?;
        require_distinct(accounts)?;
        Ok(frame)
    }
}

#[derive(Clone, Copy)]
struct BeginPlan {
    cursor: StagingCursorV1,
    raw_bump: u8,
    cursor_bump: u8,
    raw_rent: u64,
    cursor_balance: u64,
    raw_before: u64,
    cursor_before: u64,
    raw_top_up: u64,
    cursor_top_up: u64,
    sponsor_before: u64,
}

#[inline(never)]
fn process_begin(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: BeginRecordV1,
) -> Result<(), ProgramError> {
    let frame = BeginFrame::parse(accounts)?;
    let plan = authenticate_begin(program_id, &frame, instruction)?;
    let key = plan.cursor.key();
    let schema = key.schema_release_id().to_bytes();
    let digest = key.expected_digest().to_bytes();

    let raw_space = plan.cursor.exact_length();
    let raw_bump = [plan.raw_bump];
    let raw_signer = [
        RAW_RECORD_PDA_SEED_V1,
        schema.as_slice(),
        digest.as_slice(),
        raw_bump.as_slice(),
    ];
    create_or_allocate_prefunded_pda(
        frame.sponsor,
        frame.raw_record,
        frame.system_program,
        plan.raw_rent,
        raw_space,
        program_id,
        &raw_signer,
    )?;
    let sponsor_after_raw = plan
        .sponsor_before
        .checked_sub(plan.raw_top_up)
        .ok_or(AdapterError::Arithmetic)?;
    if frame.sponsor.lamports() != sponsor_after_raw
        || frame.raw_record.lamports()
            != plan
                .raw_before
                .checked_add(plan.raw_top_up)
                .ok_or(AdapterError::Arithmetic)?
        || frame.raw_record.lamports() < plan.raw_rent
        || frame.raw_record.owner != program_id
        || u64::try_from(frame.raw_record.data_len()).map_err(|_| AdapterError::Arithmetic)?
            != raw_space
    {
        return Err(AdapterError::AccountData.into());
    }

    let cursor_space =
        u64::try_from(STAGING_CURSOR_BYTES_V1).map_err(|_| AdapterError::Arithmetic)?;
    let cursor_bump = [plan.cursor_bump];
    let cursor_signer = [
        STAGING_CURSOR_PDA_SEED_V1,
        schema.as_slice(),
        digest.as_slice(),
        cursor_bump.as_slice(),
    ];
    create_or_allocate_prefunded_pda(
        frame.sponsor,
        frame.cursor,
        frame.system_program,
        plan.cursor_balance,
        cursor_space,
        program_id,
        &cursor_signer,
    )?;

    let expected_sponsor = sponsor_after_raw
        .checked_sub(plan.cursor_top_up)
        .ok_or(AdapterError::Arithmetic)?;
    if frame.sponsor.lamports() != expected_sponsor
        || frame.cursor.lamports()
            != plan
                .cursor_before
                .checked_add(plan.cursor_top_up)
                .ok_or(AdapterError::Arithmetic)?
        || frame.cursor.lamports() < plan.cursor_balance
        || frame.cursor.owner != program_id
        || frame.cursor.data_len() != STAGING_CURSOR_BYTES_V1
    {
        return Err(AdapterError::AccountData.into());
    }
    {
        let mut cursor_data = frame
            .cursor
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::AccountData)?;
        cursor_data.copy_from_slice(&plan.cursor.to_bytes());
    }
    let cursor_data = frame
        .cursor
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    if StagingCursorV1::decode(&cursor_data).map_err(map_record_error)? != plan.cursor {
        return Err(AdapterError::AccountData.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_begin(
    program_id: &Pubkey,
    frame: &BeginFrame<'_, '_>,
    instruction: BeginRecordV1,
) -> Result<BeginPlan, ProgramError> {
    require_system_identity(frame.system_program)?;
    require_rent_identity(frame.rent_sysvar)?;
    require_clock_identity(frame.clock_sysvar)?;
    require_system_wallet(frame.sponsor, true)?;
    require_prefunded_vacant(frame.raw_record)?;
    require_prefunded_vacant(frame.cursor)?;

    let rent = Rent::from_account_info(frame.rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    authenticate_rent_credit(
        program_id,
        frame.rent_credit,
        refund_authority(frame.sponsor.key)?,
        Some(rent.minimum_balance(RENT_CREDIT_BYTES_V1)),
    )?;
    let clock =
        Clock::from_account_info(frame.clock_sysvar).map_err(|_| AdapterError::AccountData)?;
    let raw_length =
        usize::try_from(instruction.exact_length()).map_err(|_| AdapterError::Arithmetic)?;
    let raw_rent = rent.minimum_balance(raw_length);
    let cursor_rent = rent.minimum_balance(STAGING_CURSOR_BYTES_V1);
    let cursor_balance = cursor_rent
        .checked_add(instruction.cleanup_bounty_lamports())
        .ok_or(AdapterError::Arithmetic)?;
    let raw_top_up = raw_rent.saturating_sub(frame.raw_record.lamports());
    let cursor_top_up = cursor_balance.saturating_sub(frame.cursor.lamports());
    let total_debit = raw_top_up
        .checked_add(cursor_top_up)
        .ok_or(AdapterError::Arithmetic)?;
    if frame.sponsor.lamports() < total_debit {
        return Err(AdapterError::FundUnderfunded.into());
    }

    let adapter = SbfRecordAdapter::begin(program_id, frame.raw_record, frame.cursor, cursor_rent);
    if !is_supported_found_schema_release(instruction.key().schema_release_id()) {
        return Err(AdapterError::ReleaseUnavailable.into());
    }
    if !is_admissible_found_schema_length(
        instruction.key().schema_release_id(),
        instruction.exact_length(),
    ) {
        return Err(AdapterError::AccountData.into());
    }
    let liveness = StagingLivenessPolicyV1::new(
        SchemaReleaseId::new(STAGING_LIVENESS_RELEASE_ID_V1).map_err(map_record_error)?,
        MAX_STAGING_LIFETIME_SLOTS_V1,
        cursor_rent,
    )
    .map_err(map_record_error)?;
    let transition = prepare_begin_v1(
        &adapter,
        instruction,
        liveness,
        clock.slot,
        account_id(frame.raw_record.key)?,
        account_id(frame.cursor.key)?,
        account_id(frame.sponsor.key)?,
    )
    .map_err(map_record_error)?;
    let allocation = transition.allocation();
    if allocation.raw_record_account() != account_id(frame.raw_record.key)?
        || allocation.raw_data_length() != instruction.exact_length()
        || allocation.staging_account() != account_id(frame.cursor.key)?
        || allocation.staging_data_length()
            != u64::try_from(STAGING_CURSOR_BYTES_V1).map_err(|_| AdapterError::Arithmetic)?
        || allocation.sponsor_rent_refund() != account_id(frame.sponsor.key)?
        || allocation.cleanup_bounty_lamports() != instruction.cleanup_bounty_lamports()
    {
        return Err(AdapterError::AccountData.into());
    }
    let (expected_raw, raw_bump) = derive_record_pda(program_id, instruction.key(), false);
    let (expected_cursor, cursor_bump) = derive_record_pda(program_id, instruction.key(), true);
    if frame.raw_record.key != &expected_raw || frame.cursor.key != &expected_cursor {
        return Err(AdapterError::AccountIdentity.into());
    }

    // All mutable borrows are proved available before either System CPI. The
    // SVM transaction remains the rollback boundary for later CPI refusal.
    preflight_lamports(frame.sponsor)?;
    preflight_mutable(frame.raw_record)?;
    preflight_mutable(frame.cursor)?;

    Ok(BeginPlan {
        cursor: transition.cursor(),
        raw_bump,
        cursor_bump,
        raw_rent,
        cursor_balance,
        raw_before: frame.raw_record.lamports(),
        cursor_before: frame.cursor.lamports(),
        raw_top_up,
        cursor_top_up,
        sponsor_before: frame.sponsor.lamports(),
    })
}

#[inline(never)]
fn process_append(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: AppendPageV1<'_>,
) -> Result<(), ProgramError> {
    let frame = AppendFrame::parse(accounts)?;
    require_live_record_accounts(program_id, frame.raw_record, frame.cursor)?;
    let cursor = decode_cursor(frame.cursor)?;
    require_canonical_record_addresses(program_id, cursor, frame.raw_record, frame.cursor)?;
    if cursor.sponsor_rent_refund() != account_id(frame.sponsor.key)?
        || u64::try_from(frame.raw_record.data_len()).map_err(|_| AdapterError::Arithmetic)?
            != cursor.exact_length()
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let transition = prepare_append_page_v1(
        cursor,
        account_id(frame.raw_record.key)?,
        account_id(frame.cursor.key)?,
        cursor.exact_length(),
        instruction,
    )
    .map_err(map_record_error)?;
    preflight_data(frame.raw_record)?;
    preflight_data(frame.cursor)?;

    let start =
        usize::try_from(transition.write().offset()).map_err(|_| AdapterError::Arithmetic)?;
    let end = start
        .checked_add(transition.write().page().len())
        .ok_or(AdapterError::Arithmetic)?;
    let next_bytes = transition.next_cursor().to_bytes();
    {
        let mut raw_data = frame
            .raw_record
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::AccountData)?;
        let mut cursor_data = frame
            .cursor
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::AccountData)?;
        raw_data
            .get_mut(start..end)
            .ok_or(AdapterError::AccountData)?
            .copy_from_slice(transition.write().page());
        cursor_data.copy_from_slice(&next_bytes);
    }
    let cursor_after = decode_cursor(frame.cursor)?;
    if cursor_after != transition.next_cursor() {
        return Err(AdapterError::AccountData.into());
    }
    let raw_after = frame
        .raw_record
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    if raw_after.get(start..end) != Some(transition.write().page()) {
        return Err(AdapterError::AccountData.into());
    }
    Ok(())
}

#[inline(never)]
fn process_finalize(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    _instruction: FinalizeRecordV1,
) -> Result<(), ProgramError> {
    let frame = FinalizeFrame::parse(accounts)?;
    require_live_record_accounts(program_id, frame.raw_record, frame.cursor)?;
    let cursor = decode_cursor(frame.cursor)?;
    require_canonical_record_addresses(program_id, cursor, frame.raw_record, frame.cursor)?;
    let refund_authority = refund_authority_from_account_id(cursor.sponsor_rent_refund())?;
    let rent_credit =
        authenticate_rent_credit(program_id, frame.rent_credit, refund_authority, None)?;
    if u64::try_from(frame.raw_record.data_len()).map_err(|_| AdapterError::Arithmetic)?
        != cursor.exact_length()
    {
        return Err(AdapterError::AccountIdentity.into());
    }

    let cursor_balance = frame.cursor.lamports();
    let credit_before = frame.rent_credit.lamports();
    let staging_close = {
        let raw_data = frame
            .raw_record
            .try_borrow_data()
            .map_err(|_| AdapterError::AccountData)?;
        let adapter = SbfRecordAdapter::validation(
            program_id,
            frame.raw_record,
            frame.cursor,
            0,
            AdapterLifecycle::Finalization,
        );
        let transition = prepare_finalize_v1(
            &adapter,
            cursor,
            account_id(frame.raw_record.key)?,
            account_id(frame.cursor.key)?,
            cursor_balance,
            &raw_data,
        )
        .map_err(map_record_error)?;
        if transition.authenticated_record().key() != cursor.key()
            || transition.authenticated_record().raw_record_account()
                != account_id(frame.raw_record.key)?
        {
            return Err(AdapterError::ContentIdentity.into());
        }
        transition.staging_close()
    };

    preflight_lamports(frame.rent_credit)?;
    preflight_mutable(frame.cursor)?;
    close_full_to_rent_credit(
        program_id,
        frame.cursor,
        frame.rent_credit,
        rent_credit,
        staging_close,
    )?;
    let expected_credit = credit_before
        .checked_add(cursor_balance)
        .ok_or(AdapterError::Arithmetic)?;
    if frame.rent_credit.lamports() != expected_credit
        || !is_vacant(frame.cursor)
        || frame.raw_record.owner != program_id
        || frame.raw_record.data_len()
            != usize::try_from(cursor.exact_length()).map_err(|_| AdapterError::Arithmetic)?
        || hash(
            &frame
                .raw_record
                .try_borrow_data()
                .map_err(|_| AdapterError::AccountData)?,
        )
        .to_bytes()
            != cursor.key().expected_digest().to_bytes()
    {
        return Err(AdapterError::AccountData.into());
    }
    Ok(())
}

#[inline(never)]
fn process_abort(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    _instruction: AbortRecordV1,
) -> Result<(), ProgramError> {
    let frame = AbortFrame::parse(accounts)?;
    require_clock_identity(frame.clock_sysvar)?;
    require_live_record_accounts(program_id, frame.raw_record, frame.cursor)?;
    let cursor = decode_cursor(frame.cursor)?;
    require_canonical_record_addresses(program_id, cursor, frame.raw_record, frame.cursor)?;
    let refund_authority = refund_authority_from_account_id(cursor.sponsor_rent_refund())?;
    let rent_credit =
        authenticate_rent_credit(program_id, frame.rent_credit, refund_authority, None)?;
    if u64::try_from(frame.raw_record.data_len()).map_err(|_| AdapterError::Arithmetic)?
        != cursor.exact_length()
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let clock =
        Clock::from_account_info(frame.clock_sysvar).map_err(|_| AdapterError::AccountData)?;
    let raw_balance = frame.raw_record.lamports();
    let cursor_balance = frame.cursor.lamports();
    let transition = prepare_abort_v1(
        cursor,
        AbortObservationV1::new(
            account_id(frame.raw_record.key)?,
            account_id(frame.cursor.key)?,
            cursor.exact_length(),
            raw_balance,
            cursor_balance,
            clock.slot,
            account_id(frame.actor.key)?,
        ),
    )
    .map_err(map_record_error)?;
    if !transition.sponsor_signature_required() && !frame.actor.is_writable {
        return Err(AdapterError::AccountPrivilege.into());
    }

    let actor_before = frame.actor.lamports();
    let credit_before = frame.rent_credit.lamports();
    if transition.staging_close().cleanup_bounty_lamports() > 0 {
        preflight_lamports(frame.actor)?;
    }
    preflight_lamports(frame.rent_credit)?;
    preflight_mutable(frame.raw_record)?;
    preflight_mutable(frame.cursor)?;

    close_full_to_rent_credit(
        program_id,
        frame.raw_record,
        frame.rent_credit,
        rent_credit,
        transition.raw_record_close(),
    )?;
    close_split_to_rent_credit(
        program_id,
        frame.cursor,
        frame.actor,
        frame.rent_credit,
        rent_credit,
        transition.staging_close(),
    )?;

    let (expected_actor, expected_credit) =
        expected_abort_balances(actor_before, credit_before, &transition)?;
    if frame.actor.lamports() != expected_actor
        || frame.rent_credit.lamports() != expected_credit
        || !is_vacant(frame.raw_record)
        || !is_vacant(frame.cursor)
    {
        return Err(AdapterError::AccountData.into());
    }
    Ok(())
}

/// Authenticate a finalized raw record from actual raw/cursor/rent accounts,
/// then expose its move-only authority to one same-instruction consumer.
///
/// `staging_vacancy` must be the actual derived PDA observed as a read-only,
/// system-owned, nonexecuting, empty-data account. Its lamport balance is not
/// semantic: arbitrary dust cannot resurrect a finalized cursor or disable a
/// consumer. A caller boolean or static index is never accepted as finality
/// evidence.
#[allow(dead_code)] // Found consumers are wired by the separately owned routing seam.
pub(crate) fn with_authenticated_finalized_record_v1<'info, T, F>(
    program_id: &Pubkey,
    raw_record: &AccountInfo<'info>,
    staging_vacancy: &AccountInfo<'info>,
    rent_sysvar: &AccountInfo<'info>,
    schema_release_id: [u8; 32],
    expected_digest: [u8; 32],
    consume: F,
) -> Result<T, ProgramError>
where
    F: FnOnce(dclutch_record_contract::AuthenticatedRawRecordV1<'_>) -> Result<T, ProgramError>,
{
    require_privilege(raw_record, false, false, false)?;
    require_privilege(staging_vacancy, false, false, false)?;
    require_privilege(rent_sysvar, false, false, false)?;
    require_distinct_refs(&[raw_record, staging_vacancy, rent_sysvar])?;
    require_rent_identity(rent_sysvar)?;
    if raw_record.owner != program_id || !is_finalized_staging_vacancy(staging_vacancy) {
        return Err(AdapterError::AccountIdentity.into());
    }
    let rent = Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::AccountData)?;
    if raw_record.lamports() < rent.minimum_balance(raw_record.data_len()) {
        return Err(AdapterError::AccountData.into());
    }
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema_release_id).map_err(map_record_error)?,
        ContentDigest::new(expected_digest).map_err(map_record_error)?,
    );
    let (expected_raw, _) = derive_record_pda(program_id, key, false);
    let (expected_cursor, _) = derive_record_pda(program_id, key, true);
    if raw_record.key != &expected_raw || staging_vacancy.key != &expected_cursor {
        return Err(AdapterError::AccountIdentity.into());
    }
    let raw_data = raw_record
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let adapter = SbfRecordAdapter::validation(
        program_id,
        raw_record,
        staging_vacancy,
        rent.minimum_balance(raw_record.data_len()),
        AdapterLifecycle::ConsumerAuthentication,
    );
    let authenticated = authenticate_finalized_raw_record_v1(
        &adapter,
        key,
        account_id(raw_record.key)?,
        account_id(staging_vacancy.key)?,
        &raw_data,
    )
    .map_err(map_record_error)?;
    consume(authenticated)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AdapterLifecycle {
    Begin,
    Finalization,
    ConsumerAuthentication,
}

struct SbfRecordAdapter<'a, 'info> {
    program_id: &'a Pubkey,
    raw_record: &'a AccountInfo<'info>,
    cursor: &'a AccountInfo<'info>,
    minimum_raw_lamports: u64,
    cursor_rent: u64,
    lifecycle: AdapterLifecycle,
}

impl<'a, 'info> SbfRecordAdapter<'a, 'info> {
    fn begin(
        program_id: &'a Pubkey,
        raw_record: &'a AccountInfo<'info>,
        cursor: &'a AccountInfo<'info>,
        cursor_rent: u64,
    ) -> Self {
        Self {
            program_id,
            raw_record,
            cursor,
            minimum_raw_lamports: 0,
            cursor_rent,
            lifecycle: AdapterLifecycle::Begin,
        }
    }

    fn validation(
        program_id: &'a Pubkey,
        raw_record: &'a AccountInfo<'info>,
        cursor: &'a AccountInfo<'info>,
        minimum_raw_lamports: u64,
        lifecycle: AdapterLifecycle,
    ) -> Self {
        Self {
            program_id,
            raw_record,
            cursor,
            minimum_raw_lamports,
            cursor_rent: 0,
            lifecycle,
        }
    }
}

impl RecordAdapterV1 for SbfRecordAdapter<'_, '_> {
    fn validate_page_envelope(&self, envelope: &PageEnvelopeV1) -> bool {
        envelope.kind() == PageEnvelopeKindV1::Provisional
            && envelope.page_bytes() == PAGE_BYTES_V1
            && envelope.basis_id().to_bytes() == PAGE_ENVELOPE_RELEASE_ID_V1
    }

    fn validate_staging_liveness_policy(&self, policy: &StagingLivenessPolicyV1) -> bool {
        self.lifecycle == AdapterLifecycle::Begin
            && policy.policy_id().to_bytes() == STAGING_LIVENESS_RELEASE_ID_V1
            && policy.maximum_lifetime_slots() == MAX_STAGING_LIFETIME_SLOTS_V1
            && self.cursor_rent > 0
            && policy.minimum_cleanup_bounty_lamports() == self.cursor_rent
    }

    fn validate_canonical_addresses(&self, obligation: &AddressDerivationObligationV1) -> bool {
        let (raw, _) = derive_record_pda(self.program_id, obligation.key(), false);
        let (cursor, _) = derive_record_pda(self.program_id, obligation.key(), true);
        obligation.raw_record_account().to_bytes() == raw.to_bytes()
            && obligation.staging_account().to_bytes() == cursor.to_bytes()
            && self.raw_record.key == &raw
            && self.cursor.key == &cursor
    }

    fn validate_raw_record(&self, obligation: &RawRecordValidationObligationV1<'_>) -> bool {
        if self.lifecycle == AdapterLifecycle::Begin
            || obligation.raw_record_account().to_bytes() != self.raw_record.key.to_bytes()
            || obligation.staging_account().to_bytes() != self.cursor.key.to_bytes()
            || self.raw_record.owner != self.program_id
            || self.raw_record.executable
            || self.raw_record.is_writable
            || self.raw_record.lamports() < self.minimum_raw_lamports
            || self.raw_record.data_len() != obligation.exact_content().len()
            || hash(obligation.exact_content()).to_bytes()
                != obligation.key().expected_digest().to_bytes()
            || !validate_found_schema(
                obligation.key().schema_release_id(),
                obligation.exact_content(),
            )
        {
            return false;
        }
        let data_matches = self
            .raw_record
            .try_borrow_data()
            .map(|data| data.as_ref() == obligation.exact_content())
            .unwrap_or(false);
        if !data_matches {
            return false;
        }
        match (self.lifecycle, obligation.mode()) {
            (AdapterLifecycle::Finalization, RawRecordValidationModeV1::Finalization) => {
                self.cursor.owner == self.program_id
                    && self.cursor.is_writable
                    && !self.cursor.executable
                    && self.cursor.data_len() == STAGING_CURSOR_BYTES_V1
                    && self.cursor.lamports() > 0
            }
            (
                AdapterLifecycle::ConsumerAuthentication,
                RawRecordValidationModeV1::ConsumerAuthentication,
            ) => {
                !self.cursor.is_writable
                    && !self.cursor.is_signer
                    && is_finalized_staging_vacancy(self.cursor)
            }
            _ => false,
        }
    }
}

fn validate_found_schema(schema_release_id: SchemaReleaseId, content: &[u8]) -> bool {
    let schema = schema_release_id.to_bytes();
    if schema == REALM_SCHEMA_RELEASE_ID_V1 {
        return RealmV1::decode(content)
            .map(|value| value.to_bytes().as_slice() == content)
            .unwrap_or(false);
    }
    if schema == PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1 {
        return InstanceV1::decode(content)
            .map(|value| value.to_bytes().as_slice() == content)
            .unwrap_or(false);
    }
    if schema == CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1 {
        return CategoricalUnitV1::decode(content)
            .map(|value| value.to_bytes().as_slice() == content)
            .unwrap_or(false);
    }
    if schema == CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1 {
        return CapacityProfileV1::decode(content)
            .map(|value| value.to_bytes().as_slice() == content)
            .unwrap_or(false);
    }
    if schema == CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1 {
        return CapabilityManifestV1::decode(content)
            .map(|value| value.as_bytes() == content)
            .unwrap_or(false);
    }
    if schema == VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3 {
        return VenueFeePolicyV3::decode(content)
            .map(|value| {
                let mut canonical = [0; VENUE_FEE_POLICY_BYTES_V3];
                value.encode(&mut canonical).is_ok() && canonical.as_slice() == content
            })
            .unwrap_or(false);
    }
    if schema == SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1 {
        return SourceMaterialViewV1::decode(content).is_ok()
            && validate_source_material_links(content);
    }
    if schema == SERIES_RECIPE_SCHEMA_RELEASE_ID_V2 {
        return SeriesRecipeV1::decode(content)
            .map(|value| value.to_bytes().as_slice() == content)
            .unwrap_or(false);
    }
    if schema == SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1 {
        return CapitalizationAggregateV1::decode(content)
            .map(|value| value.to_bytes().as_slice() == content)
            .unwrap_or(false);
    }
    if schema == SERIES_DERIVED_SCHEMA_RELEASE_ID_V1 {
        return DerivedOccurrenceV1::decode(content)
            .map(|value| value.to_bytes().as_slice() == content)
            .unwrap_or(false);
    }
    if schema == SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1 {
        return OccurrenceCapitalizationV1::decode(content)
            .map(|value| value.to_bytes().as_slice() == content)
            .unwrap_or(false);
    }
    if schema == DEALER_CONFIG_SCHEMA_RELEASE_ID_V1 {
        return validate_dealer_config(content);
    }
    if schema == GENERAL_CONFIG_SCHEMA_ID_V1.to_bytes() {
        return GeneralConfigV1::decode(content)
            .map(|value| value.to_bytes().as_slice() == content)
            .unwrap_or(false);
    }
    false
}

fn validate_dealer_config(content: &[u8]) -> bool {
    let Ok(content_id) = CoreContentId::new(hash(content).to_bytes()) else {
        return false;
    };
    let mut outcomes = 2usize;
    while outcomes <= 16 {
        if let Ok(profile) = LiquidityProfileV1::from_config_len(outcomes, content.len())
            && (2..=8).contains(&profile.bins())
            && LiquidityConfigViewV1::new(content_id, profile, content).is_ok()
        {
            return true;
        }
        outcomes += 1;
    }
    false
}

/// Check the adapter-owned content-addressed links in the canonical
/// SourceMaterial V1 layout. The Product result-domain is deliberately not a
/// plain record digest: its identity is domain separated before comparison to
/// the embedded ResolutionPolicy.
fn validate_source_material_links(content: &[u8]) -> bool {
    let fixed = [
        (256usize, 288usize, 112usize),
        (400, 432, 192),
        (624, 656, 112),
        (768, 800, 176),
        (1360, 1392, 176),
        (544, 1568, 64),
    ];
    if fixed.iter().any(|(id_offset, value_offset, length)| {
        !content_link_matches(content, *id_offset, *value_offset, *length)
    }) {
        return false;
    }
    let Some(domain) = content.get(1008..1360) else {
        return false;
    };
    let Some(policy_domain_id) = content.get(192..224) else {
        return false;
    };
    if hashv(&[FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, &[0], domain])
        .to_bytes()
        .as_slice()
        != policy_domain_id
    {
        return false;
    }
    let active = content.get(1664 + 10).copied().map(usize::from);
    let Some(active) = active else {
        return false;
    };
    if active == 0 {
        return content
            .get(1632..1664)
            .is_some_and(|id| id.iter().all(|byte| *byte == 0));
    }
    if !content_link_matches(content, 1632, 1664, 528) {
        return false;
    }
    let mut index = 0usize;
    while index < active {
        let source = 2192 + index * 224;
        let provider = 3088 + index * 208;
        let config = 3920 + index * 64;
        if !content_link_matches(content, source, source + 32, 192)
            || !content_link_matches(content, provider, provider + 32, 176)
            || !content_link_matches(content, source + 144, config, 64)
        {
            return false;
        }
        index += 1;
    }
    true
}

fn content_link_matches(
    content: &[u8],
    id_offset: usize,
    value_offset: usize,
    length: usize,
) -> bool {
    let Some(id) = content.get(id_offset..id_offset.saturating_add(32)) else {
        return false;
    };
    let Some(value) = content.get(value_offset..value_offset.saturating_add(length)) else {
        return false;
    };
    hash(value).to_bytes().as_slice() == id
}

fn is_supported_found_schema_release(schema_release_id: SchemaReleaseId) -> bool {
    if schema_release_id.to_bytes() == GENERAL_CONFIG_SCHEMA_ID_V1.to_bytes() {
        return true;
    }
    matches!(
        schema_release_id.to_bytes(),
        REALM_SCHEMA_RELEASE_ID_V1
            | PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1
            | CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1
            | CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1
            | CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1
            | VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3
            | SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1
            | SERIES_RECIPE_SCHEMA_RELEASE_ID_V2
            | SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1
            | SERIES_DERIVED_SCHEMA_RELEASE_ID_V1
            | SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1
            | DEALER_CONFIG_SCHEMA_RELEASE_ID_V1
    )
}

fn is_admissible_found_schema_length(
    schema_release_id: SchemaReleaseId,
    exact_length: u64,
) -> bool {
    let Ok(length) = usize::try_from(exact_length) else {
        return false;
    };
    match schema_release_id.to_bytes() {
        REALM_SCHEMA_RELEASE_ID_V1 => length == REALM_BYTES,
        PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1 => length == INSTANCE_BYTES,
        CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1 => length == CATEGORICAL_UNIT_BYTES,
        CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1 => length == CAPACITY_PROFILE_BYTES,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1 => {
            (MANIFEST_HEADER_BYTES..=MAX_MANIFEST_BYTES).contains(&length)
                && length.saturating_sub(MANIFEST_HEADER_BYTES) % CAPABILITY_ENTRY_BYTES == 0
        }
        VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3 => length == VENUE_FEE_POLICY_BYTES_V3,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1 => length == SOURCE_MATERIAL_BYTES,
        SERIES_RECIPE_SCHEMA_RELEASE_ID_V2 => length == SERIES_RECIPE_BYTES_V1,
        SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1 => length == CAPITALIZATION_AGGREGATE_BYTES_V1,
        SERIES_DERIVED_SCHEMA_RELEASE_ID_V1 => length == DERIVED_OCCURRENCE_BYTES_V1,
        SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1 => length == OCCURRENCE_CAPITALIZATION_BYTES_V1,
        DEALER_CONFIG_SCHEMA_RELEASE_ID_V1 => dealer_config_length_is_admissible(length),
        value if value == GENERAL_CONFIG_SCHEMA_ID_V1.to_bytes() => length == GENERAL_CONFIG_BYTES,
        _ => false,
    }
}

fn dealer_config_length_is_admissible(length: usize) -> bool {
    let Some(payload) = length.checked_sub(80) else {
        return false;
    };
    if payload % 32 != 0 {
        return false;
    }
    let cells = payload / 32;
    let mut outcomes = 2usize;
    while outcomes <= 16 {
        let mut bins = 2usize;
        while bins <= 8 {
            if outcomes.checked_mul(bins) == Some(cells) {
                return true;
            }
            bins += 1;
        }
        outcomes += 1;
    }
    false
}

#[cfg(test)]
fn release_id(label: &[u8]) -> Result<SchemaReleaseId, ProgramError> {
    SchemaReleaseId::new(hash(label).to_bytes()).map_err(map_record_error)
}

pub(crate) fn derive_record_pda(
    program_id: &Pubkey,
    key: RecordKeyV1,
    staging: bool,
) -> (Pubkey, u8) {
    let domain = if staging {
        STAGING_CURSOR_PDA_SEED_V1
    } else {
        RAW_RECORD_PDA_SEED_V1
    };
    Pubkey::find_program_address(
        &[
            domain,
            key.schema_release_id().as_bytes(),
            key.expected_digest().as_bytes(),
        ],
        program_id,
    )
}

fn require_canonical_record_addresses(
    program_id: &Pubkey,
    cursor: StagingCursorV1,
    raw_record: &AccountInfo<'_>,
    cursor_account: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    let (expected_raw, _) = derive_record_pda(program_id, cursor.key(), false);
    let (expected_cursor, _) = derive_record_pda(program_id, cursor.key(), true);
    if raw_record.key != &expected_raw
        || cursor_account.key != &expected_cursor
        || cursor.raw_record_account() != account_id(raw_record.key)?
        || cursor.staging_account() != account_id(cursor_account.key)?
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn decode_cursor(account: &AccountInfo<'_>) -> Result<StagingCursorV1, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    StagingCursorV1::decode(&data).map_err(map_record_error)
}

fn require_live_record_accounts(
    program_id: &Pubkey,
    raw_record: &AccountInfo<'_>,
    cursor: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    if raw_record.owner != program_id
        || cursor.owner != program_id
        || raw_record.executable
        || cursor.executable
        || cursor.data_len() != STAGING_CURSOR_BYTES_V1
        || cursor.lamports() == 0
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

pub(crate) fn refund_authority(key: &Pubkey) -> Result<RefundAuthority, ProgramError> {
    RefundAuthority::new(key.to_bytes()).map_err(map_rent_error)
}

fn refund_authority_from_account_id(
    account_id: AccountId,
) -> Result<RefundAuthority, ProgramError> {
    RefundAuthority::new(account_id.to_bytes()).map_err(map_rent_error)
}

pub(crate) fn authenticate_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authority: RefundAuthority,
    minimum_lamports: Option<u64>,
) -> Result<RentCreditV1, ProgramError> {
    let authority_bytes = authority.to_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, authority_bytes.as_slice()],
        program_id,
    );
    if account.key != &expected
        || account.owner != program_id
        || account.executable
        || account.data_len() != RENT_CREDIT_BYTES_V1
        || minimum_lamports.is_some_and(|minimum| account.lamports() < minimum)
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    let credit = RentCreditV1::decode(&data).map_err(map_rent_error)?;
    credit
        .validate_binding(authority, bump)
        .map_err(map_rent_error)?;
    if credit.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::AccountData.into());
    }
    Ok(credit)
}

pub(crate) fn require_unchanged_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected: RentCreditV1,
) -> Result<(), ProgramError> {
    if account.owner != program_id
        || account.executable
        || account.data_len() != RENT_CREDIT_BYTES_V1
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::AccountData)?;
    if RentCreditV1::decode(&data).map_err(map_rent_error)? != expected
        || expected.to_bytes().as_slice() != &data[..]
    {
        return Err(AdapterError::AccountData.into());
    }
    Ok(())
}

fn close_full_to_rent_credit(
    program_id: &Pubkey,
    source: &AccountInfo<'_>,
    rent_credit_account: &AccountInfo<'_>,
    rent_credit: RentCreditV1,
    plan: AccountCloseV1,
) -> Result<(), ProgramError> {
    if source.owner != program_id
        || source.key == rent_credit_account.key
        || plan.account().to_bytes() != source.key.to_bytes()
        || plan.full_lamport_refund().to_bytes() != rent_credit.refund_authority().to_bytes()
        || plan.observed_lamports() != source.lamports()
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let credit_plan = SourceCloseCreditPlanV1::new(
        source.lamports(),
        rent_credit_account.lamports(),
        plan.observed_lamports(),
    )
    .map_err(map_rent_error)?;
    {
        let mut credit_lamports = rent_credit_account
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        let mut source_lamports = source
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        **credit_lamports = credit_plan.credit_after();
        **source_lamports = 0;
    }
    close_empty_system_account(source)?;
    credit_plan
        .validate_post(source.lamports(), rent_credit_account.lamports())
        .map_err(map_rent_error)?;
    require_unchanged_rent_credit(program_id, rent_credit_account, rent_credit)?;
    Ok(())
}

fn close_split_to_rent_credit(
    program_id: &Pubkey,
    source: &AccountInfo<'_>,
    cleanup_recipient: &AccountInfo<'_>,
    rent_credit_account: &AccountInfo<'_>,
    rent_credit: RentCreditV1,
    plan: StagingLamportCloseV1,
) -> Result<(), ProgramError> {
    plan.validate_conservation().map_err(map_record_error)?;
    if source.owner != program_id
        || source.key == cleanup_recipient.key
        || source.key == rent_credit_account.key
        || cleanup_recipient.key == rent_credit_account.key
        || plan.account().to_bytes() != source.key.to_bytes()
        || plan.cleanup_recipient().to_bytes() != cleanup_recipient.key.to_bytes()
        || plan.sponsor_recipient().to_bytes() != rent_credit.refund_authority().to_bytes()
        || plan.observed_lamports() != source.lamports()
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let cleanup_next = cleanup_recipient
        .lamports()
        .checked_add(plan.cleanup_bounty_lamports())
        .ok_or(AdapterError::Arithmetic)?;
    if plan.cleanup_bounty_lamports() > 0 {
        let source_after_bounty = source
            .lamports()
            .checked_sub(plan.cleanup_bounty_lamports())
            .ok_or(AdapterError::Arithmetic)?;
        if source_after_bounty != plan.sponsor_refund_lamports() {
            return Err(AdapterError::AccountData.into());
        }
        {
            let mut cleanup_lamports = cleanup_recipient
                .try_borrow_mut_lamports()
                .map_err(|_| AdapterError::AccountData)?;
            let mut source_lamports = source
                .try_borrow_mut_lamports()
                .map_err(|_| AdapterError::AccountData)?;
            **cleanup_lamports = cleanup_next;
            **source_lamports = source_after_bounty;
        }
        if cleanup_recipient.lamports() != cleanup_next
            || source.lamports() != plan.sponsor_refund_lamports()
        {
            return Err(AdapterError::AccountData.into());
        }
    }
    let credit_plan = SourceCloseCreditPlanV1::new(
        source.lamports(),
        rent_credit_account.lamports(),
        plan.sponsor_refund_lamports(),
    )
    .map_err(map_rent_error)?;
    {
        let mut credit_lamports = rent_credit_account
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        let mut source_lamports = source
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?;
        **credit_lamports = credit_plan.credit_after();
        **source_lamports = 0;
    }
    close_empty_system_account(source)?;
    credit_plan
        .validate_post(source.lamports(), rent_credit_account.lamports())
        .map_err(map_rent_error)?;
    if cleanup_recipient.lamports() != cleanup_next {
        return Err(AdapterError::AccountData.into());
    }
    require_unchanged_rent_credit(program_id, rent_credit_account, rent_credit)?;
    Ok(())
}

fn close_empty_system_account(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    account.resize(0).map_err(|_| AdapterError::AccountData)?;
    account.assign(&system_program::ID);
    if !is_vacant(account) {
        return Err(AdapterError::AccountData.into());
    }
    Ok(())
}

fn create_or_allocate_prefunded_pda<'info>(
    payer: &AccountInfo<'info>,
    created: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    minimum_balance: u64,
    space: u64,
    owner: &Pubkey,
    signer_seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    let before = created.lamports();
    if !is_prefunded_vacant(created) {
        return Err(AdapterError::AccountIdentity.into());
    }
    let top_up = minimum_balance.saturating_sub(before);
    if before == 0 {
        invoke_signed(
            &create_account(payer.key, created.key, minimum_balance, space, owner),
            &[payer.clone(), created.clone(), system.clone()],
            &[signer_seeds],
        )?;
    } else {
        if top_up != 0 {
            invoke(
                &transfer(payer.key, created.key, top_up),
                &[payer.clone(), created.clone(), system.clone()],
            )?;
        }
        invoke_signed(
            &allocate(created.key, space),
            &[created.clone(), system.clone()],
            &[signer_seeds],
        )?;
        invoke_signed(
            &assign(created.key, owner),
            &[created.clone(), system.clone()],
            &[signer_seeds],
        )?;
    }
    let expected = before.checked_add(top_up).ok_or(AdapterError::Arithmetic)?;
    if created.owner != owner
        || created.executable
        || created.data_len() != usize::try_from(space).map_err(|_| AdapterError::Arithmetic)?
        || created.lamports() != expected
        || created.lamports() < minimum_balance
    {
        return Err(AdapterError::AccountData.into());
    }
    Ok(())
}

fn expected_abort_balances(
    actor_before: u64,
    credit_before: u64,
    transition: &AbortTransitionV1,
) -> Result<(u64, u64), ProgramError> {
    let raw_refund = transition.raw_record_close().observed_lamports();
    let bounty = transition.staging_close().cleanup_bounty_lamports();
    let staging_refund = transition.staging_close().sponsor_refund_lamports();
    let actor = actor_before
        .checked_add(bounty)
        .ok_or(AdapterError::Arithmetic)?;
    let credit = credit_before
        .checked_add(raw_refund)
        .and_then(|value| value.checked_add(staging_refund))
        .ok_or(AdapterError::Arithmetic)?;
    Ok((actor, credit))
}

fn require_privilege(
    account: &AccountInfo<'_>,
    signer: bool,
    writable: bool,
    executable: bool,
) -> Result<(), ProgramError> {
    if account.is_signer != signer
        || account.is_writable != writable
        || account.executable != executable
    {
        return Err(AdapterError::AccountPrivilege.into());
    }
    Ok(())
}

fn require_readonly_signer(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if !account.is_signer || account.is_writable {
        return Err(AdapterError::AccountPrivilege.into());
    }
    Ok(())
}

fn require_signer(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if !account.is_signer {
        return Err(AdapterError::AccountPrivilege.into());
    }
    Ok(())
}

fn require_system_identity(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.key != &system_program::ID || account.owner != &native_loader::ID {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn require_rent_identity(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.key != &sysvar::rent::ID || account.owner != &sysvar::ID {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn require_clock_identity(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.key != &sysvar::clock::ID || account.owner != &sysvar::ID {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn require_system_wallet(account: &AccountInfo<'_>, signer: bool) -> Result<(), ProgramError> {
    if account.owner != &system_program::ID
        || account.executable
        || account.is_signer != signer
        || !account.is_writable
        || !account
            .try_data_is_empty()
            .map_err(|_| AdapterError::AccountData)?
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn require_prefunded_vacant(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if !is_prefunded_vacant(account) {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn is_vacant(account: &AccountInfo<'_>) -> bool {
    account.owner == &system_program::ID
        && !account.executable
        && account.lamports() == 0
        && account.try_data_is_empty().unwrap_or(false)
}

fn is_prefunded_vacant(account: &AccountInfo<'_>) -> bool {
    account.owner == &system_program::ID
        && !account.executable
        && account.try_data_is_empty().unwrap_or(false)
}

fn is_finalized_staging_vacancy(account: &AccountInfo<'_>) -> bool {
    is_prefunded_vacant(account)
}

fn preflight_lamports(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    drop(
        account
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::AccountData)?,
    );
    Ok(())
}

fn preflight_data(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    drop(
        account
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::AccountData)?,
    );
    Ok(())
}

fn preflight_mutable(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    preflight_lamports(account)?;
    preflight_data(account)
}

fn require_distinct(accounts: &[AccountInfo<'_>]) -> Result<(), ProgramError> {
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other.key == account.key)
        {
            return Err(AdapterError::AccountIdentity.into());
        }
    }
    Ok(())
}

fn require_distinct_refs(accounts: &[&AccountInfo<'_>]) -> Result<(), ProgramError> {
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other.key == account.key)
        {
            return Err(AdapterError::AccountIdentity.into());
        }
    }
    Ok(())
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| AdapterError::AccountFrameLength.into())
}

fn account_id(key: &Pubkey) -> Result<AccountId, ProgramError> {
    AccountId::new(key.to_bytes()).map_err(map_record_error)
}

fn map_record_error(error: dclutch_record_contract::Error) -> ProgramError {
    use dclutch_record_contract::Error as RecordError;

    match error {
        RecordError::ArithmeticOverflow | RecordError::LamportConservationMismatch => {
            AdapterError::Arithmetic.into()
        }
        RecordError::AccountAlias
        | RecordError::AddressDerivationRefused
        | RecordError::CursorBindingMismatch => AdapterError::AccountIdentity.into(),
        RecordError::PageReplay
        | RecordError::PageOutOfOrder
        | RecordError::PageOverlap
        | RecordError::PageGap
        | RecordError::CursorComplete => AdapterError::ReplayMismatch.into(),
        RecordError::AdapterValidationRefused => AdapterError::ContentIdentity.into(),
        RecordError::InvalidPageEnvelope
        | RecordError::PageEnvelopeRefused
        | RecordError::StagingPolicyRefused
        | RecordError::InvalidExpiry
        | RecordError::InsufficientCleanupBounty => AdapterError::ReleaseUnavailable.into(),
        RecordError::AbortBeforeExpiry => AdapterError::ReplayMismatch.into(),
        RecordError::InvalidLength
        | RecordError::InvalidMagic
        | RecordError::UnsupportedSchema
        | RecordError::UnknownAction
        | RecordError::NonCanonicalReservedBytes
        | RecordError::ZeroIdentity
        | RecordError::ZeroRecordLength
        | RecordError::GeometryMismatch
        | RecordError::PageLengthMismatch
        | RecordError::CursorIncomplete
        | RecordError::InvalidCursorStatus
        | RecordError::OutputLength => AdapterError::AccountData.into(),
    }
}

pub(crate) fn map_rent_error(error: dclutch_rent_contract::Error) -> ProgramError {
    use dclutch_rent_contract::Error as RentError;

    match error {
        RentError::ArithmeticOverflow => AdapterError::Arithmetic.into(),
        RentError::ZeroAuthorityOrAccount
        | RentError::AccountAlias
        | RentError::CreditBindingMismatch => AdapterError::AccountIdentity.into(),
        RentError::InvalidLength
        | RentError::InvalidMagic
        | RentError::UnsupportedSchema
        | RentError::UnknownAction
        | RentError::NonCanonicalReservedBytes
        | RentError::InvalidAccountPrivilege
        | RentError::InvalidSystemProgram
        | RentError::InvalidRentSysvar
        | RentError::InvalidSystemWallet
        | RentError::CreationFundingMismatch
        | RentError::ZeroWithdrawal
        | RentError::WithdrawalExceedsClaimable
        | RentError::SourceCreditMismatch
        | RentError::CloseNotSupported => AdapterError::AccountData.into(),
    }
}

#[cfg(test)]
mod tests {
    use alloc::{boxed::Box, vec, vec::Vec};
    use dclutch_capability_contract::CapabilityManifestV1;
    use dclutch_general_contract::{
        ContentId as GeneralContentId, GENERAL_CAPABILITY_RELEASE_ID_V1, GeneralConfigV1Input,
    };
    use dclutch_realm_contract::{FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1Input};

    use super::*;

    fn test_account(
        key: Pubkey,
        signer: bool,
        writable: bool,
        lamports: u64,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(lamports)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn record_key() -> RecordKeyV1 {
        RecordKeyV1::new(
            SchemaReleaseId::new(REALM_SCHEMA_RELEASE_ID_V1).expect("schema release"),
            ContentDigest::new([7; 32]).expect("digest"),
        )
    }

    #[test]
    fn pda_domains_are_bounded_distinct_and_bind_both_identities() {
        let program_id = Pubkey::new_unique();
        let key = record_key();
        let (raw, _) = derive_record_pda(&program_id, key, false);
        let (cursor, _) = derive_record_pda(&program_id, key, true);
        assert_ne!(raw, cursor);
        assert!(RAW_RECORD_PDA_SEED_V1.len() <= 32);
        assert!(STAGING_CURSOR_PDA_SEED_V1.len() <= 32);

        let changed = RecordKeyV1::new(
            key.schema_release_id(),
            ContentDigest::new([8; 32]).expect("digest"),
        );
        assert_ne!(derive_record_pda(&program_id, changed, false).0, raw);
    }

    #[test]
    fn hardcoded_release_ids_match_their_named_preimages() {
        for (label, expected) in RELEASE_LABELS_AND_IDS_V1 {
            assert_eq!(hash(label).to_bytes(), expected);
        }
    }

    #[test]
    fn direct_venue_fee_v3_is_the_only_admitted_fee_record() {
        let policy = VenueFeePolicyV3::new([9; 32], 25).expect("valid V3 policy");
        let mut bytes = [0; VENUE_FEE_POLICY_BYTES_V3];
        policy.encode(&mut bytes).expect("canonical V3 bytes");
        let schema =
            SchemaReleaseId::new(VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3).expect("V3 schema");
        assert!(validate_found_schema(schema, &bytes));
        assert!(is_admissible_found_schema_length(
            schema,
            u64::try_from(VENUE_FEE_POLICY_BYTES_V3).expect("bounded V3 length"),
        ));
        assert!(!is_admissible_found_schema_length(
            schema,
            u64::try_from(VENUE_FEE_POLICY_BYTES_V3 + 1).expect("bounded length"),
        ));
        bytes[12] = 1;
        assert!(!validate_found_schema(schema, &bytes));
    }

    #[test]
    fn begin_schema_admission_is_closed_and_checks_owned_geometry() {
        for (schema_id, exact_length) in [
            (REALM_SCHEMA_RELEASE_ID_V1, REALM_BYTES),
            (PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1, INSTANCE_BYTES),
            (
                CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1,
                CATEGORICAL_UNIT_BYTES,
            ),
            (
                CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1,
                CAPACITY_PROFILE_BYTES,
            ),
            (
                CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
                MANIFEST_HEADER_BYTES,
            ),
            (
                VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3,
                VENUE_FEE_POLICY_BYTES_V3,
            ),
            (SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SOURCE_MATERIAL_BYTES),
            (SERIES_RECIPE_SCHEMA_RELEASE_ID_V2, SERIES_RECIPE_BYTES_V1),
            (
                SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1,
                CAPITALIZATION_AGGREGATE_BYTES_V1,
            ),
            (
                SERIES_DERIVED_SCHEMA_RELEASE_ID_V1,
                DERIVED_OCCURRENCE_BYTES_V1,
            ),
            (
                SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1,
                OCCURRENCE_CAPITALIZATION_BYTES_V1,
            ),
            (DEALER_CONFIG_SCHEMA_RELEASE_ID_V1, 208),
            (GENERAL_CONFIG_SCHEMA_ID_V1.to_bytes(), GENERAL_CONFIG_BYTES),
        ] {
            let schema = SchemaReleaseId::new(schema_id).expect("known schema");
            assert!(is_supported_found_schema_release(schema));
            assert!(is_admissible_found_schema_length(
                schema,
                u64::try_from(exact_length).expect("bounded length"),
            ));
            assert!(!is_admissible_found_schema_length(
                schema,
                u64::try_from(exact_length.saturating_add(1)).expect("bounded length"),
            ));
        }
        let obsolete_series_recipe =
            SchemaReleaseId::new(hash(b"dclutch/schema/series-recipe-v1").to_bytes())
                .expect("obsolete Series recipe schema");
        assert!(!is_supported_found_schema_release(obsolete_series_recipe));
        assert!(!is_admissible_found_schema_length(
            obsolete_series_recipe,
            u64::try_from(SERIES_RECIPE_BYTES_V1).expect("bounded recipe length"),
        ));
        let manifest = SchemaReleaseId::new(CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1)
            .expect("manifest schema");
        assert!(is_admissible_found_schema_length(
            manifest,
            u64::try_from(MAX_MANIFEST_BYTES).expect("bounded manifest"),
        ));
        let unsupported = release_id(b"dclutch/schema/unsupported-v1").expect("unsupported ID");
        assert!(!is_supported_found_schema_release(unsupported));
        assert!(!is_admissible_found_schema_length(
            unsupported,
            u64::try_from(REALM_BYTES).expect("bounded length"),
        ));
        let dealer =
            SchemaReleaseId::new(DEALER_CONFIG_SCHEMA_RELEASE_ID_V1).expect("Dealer config schema");
        assert!(is_admissible_found_schema_length(dealer, 4_176));
        assert!(!is_admissible_found_schema_length(dealer, 144));
        assert!(!is_admissible_found_schema_length(dealer, 4_177));
        assert!(!validate_found_schema(
            SchemaReleaseId::new(GENERAL_CONFIG_SCHEMA_ID_V1.to_bytes()).expect("General schema"),
            &[0; GENERAL_CONFIG_BYTES],
        ));
    }

    #[test]
    fn selected_page_and_liveness_releases_are_not_caller_attestations() {
        let program_id = Pubkey::new_unique();
        let key = record_key();
        let (raw_key, _) = derive_record_pda(&program_id, key, false);
        let (cursor_key, _) = derive_record_pda(&program_id, key, true);
        let raw = test_account(
            raw_key,
            false,
            true,
            0,
            Vec::new(),
            system_program::ID,
            false,
        );
        let cursor = test_account(
            cursor_key,
            false,
            true,
            0,
            Vec::new(),
            system_program::ID,
            false,
        );
        let adapter = SbfRecordAdapter::begin(&program_id, &raw, &cursor, 41);
        let selected_page = PageEnvelopeV1::new(
            PageEnvelopeKindV1::Provisional,
            PAGE_BYTES_V1,
            SchemaReleaseId::new(PAGE_ENVELOPE_RELEASE_ID_V1).expect("page release"),
        )
        .expect("page envelope");
        assert!(adapter.validate_page_envelope(&selected_page));
        let hostile_page = PageEnvelopeV1::new(
            PageEnvelopeKindV1::Provisional,
            PAGE_BYTES_V1 - 1,
            SchemaReleaseId::new(PAGE_ENVELOPE_RELEASE_ID_V1).expect("page release"),
        )
        .expect("hostile envelope");
        assert!(!adapter.validate_page_envelope(&hostile_page));

        let selected_liveness = StagingLivenessPolicyV1::new(
            SchemaReleaseId::new(STAGING_LIVENESS_RELEASE_ID_V1).expect("liveness release"),
            MAX_STAGING_LIFETIME_SLOTS_V1,
            41,
        )
        .expect("liveness policy");
        assert!(adapter.validate_staging_liveness_policy(&selected_liveness));
        let hostile_liveness = StagingLivenessPolicyV1::new(
            SchemaReleaseId::new(STAGING_LIVENESS_RELEASE_ID_V1).expect("liveness release"),
            MAX_STAGING_LIFETIME_SLOTS_V1 + 1,
            41,
        )
        .expect("hostile policy");
        assert!(!adapter.validate_staging_liveness_policy(&hostile_liveness));
    }

    #[test]
    fn found_schema_dispatch_accepts_exact_owned_records_only() {
        let realm = RealmV1::new(RealmV1Input {
            token_program: [1; 32],
            collateral_mint: [2; 32],
            collateral_adapter_release_id: [3; 32],
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("realm");
        let realm_schema = SchemaReleaseId::new(REALM_SCHEMA_RELEASE_ID_V1).expect("schema");
        assert!(validate_found_schema(realm_schema, &realm.to_bytes()));

        let manifest = CapabilityManifestV1::empty().expect("empty manifest");
        let manifest_schema =
            SchemaReleaseId::new(CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1).expect("schema");
        assert!(validate_found_schema(manifest_schema, manifest.as_bytes()));

        let mut poison = realm.to_bytes();
        *poison.get_mut(0).expect("magic byte") = 0;
        assert!(!validate_found_schema(realm_schema, &poison));
        assert!(!validate_found_schema(
            release_id(b"dclutch/schema/unsupported-v1").expect("unsupported ID"),
            &realm.to_bytes(),
        ));

        for schema_id in [
            PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
            CATEGORICAL_CLAIM_SCHEMA_RELEASE_ID_V1,
            CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        ] {
            assert!(!validate_found_schema(
                SchemaReleaseId::new(schema_id).expect("known schema"),
                &poison,
            ));
        }
    }

    #[test]
    fn series_and_dealer_records_require_exact_canonical_decode() {
        use dclutch_series_contract::{
            CAPABILITY_DERIVATION_RELEASE_ID_V1, IdentityV1, MARKET_DERIVATION_RELEASE_ID_V1,
            OCCURRENCE_DERIVATION_RELEASE_ID_V1, PRODUCT_COMPILER_RELEASE_ID_V1,
            SOURCE_DERIVATION_RELEASE_ID_V1,
        };
        let identity = |value: u8| IdentityV1::new([value; 32]).expect("identity");
        let recipe = SeriesRecipeV1 {
            realm_id: identity(1),
            terms_id: identity(2),
            claim_basis_id: identity(3),
            result_domain_id: identity(4),
            capacity_profile_id: identity(4),
            compiler_release_id: IdentityV1::new(PRODUCT_COMPILER_RELEASE_ID_V1).expect("compiler"),
            occurrence_schedule_id: identity(6),
            source_schedule_id: identity(7),
            capability_template_id: identity(8),
            occurrence_derivation_release_id: IdentityV1::new(OCCURRENCE_DERIVATION_RELEASE_ID_V1)
                .expect("occurrence release"),
            source_derivation_release_id: IdentityV1::new(SOURCE_DERIVATION_RELEASE_ID_V1)
                .expect("source release"),
            capability_derivation_release_id: IdentityV1::new(CAPABILITY_DERIVATION_RELEASE_ID_V1)
                .expect("capability release"),
            market_derivation_release_id: IdentityV1::new(MARKET_DERIVATION_RELEASE_ID_V1)
                .expect("Market release"),
            capitalization_schedule_id: identity(13),
            first_occurrence_time: 100,
            cadence_seconds: 10,
            occurrence_count: 1,
            first_generation: 7,
            outcome_count: 2,
        };
        let aggregate = CapitalizationAggregateV1 {
            recipe_id: identity(20),
            capitalization_schedule_id: identity(13),
            occurrence_count: 1,
            total_principal: 30,
            first_capitalization_id: identity(21),
        };
        let derived = DerivedOccurrenceV1 {
            recipe_id: identity(20),
            occurrence_index: 0,
            occurrence_time: 100,
            generation: 7,
            occurrence_artifact_id: identity(22),
            occurrence_id: identity(23),
            product_instance_id: identity(24),
            source_spec_id: identity(25),
            source_window_id: identity(26),
            statistic_id: identity(27),
            resolution_policy_id: identity(28),
            capability_manifest_id: identity(29),
            market_identity_id: identity(30),
            capitalization_id: identity(21),
        };
        let capitalization = OccurrenceCapitalizationV1 {
            recipe_id: identity(20),
            capitalization_schedule_id: identity(13),
            occurrence_index: 0,
            market_principal: 20,
            ticket_rent: 10,
            total_principal: 30,
            next_capitalization_id: None,
        };
        for (schema, bytes) in [
            (
                SERIES_RECIPE_SCHEMA_RELEASE_ID_V2,
                recipe.to_bytes().to_vec(),
            ),
            (
                SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1,
                aggregate.to_bytes().to_vec(),
            ),
            (
                SERIES_DERIVED_SCHEMA_RELEASE_ID_V1,
                derived.to_bytes().to_vec(),
            ),
            (
                SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1,
                capitalization.to_bytes().to_vec(),
            ),
        ] {
            let release = SchemaReleaseId::new(schema).expect("schema");
            assert!(validate_found_schema(release, &bytes));
            assert!(!validate_found_schema(
                release,
                bytes.get(..bytes.len() - 1).expect("short record"),
            ));
            let mut hostile = bytes;
            *hostile.get_mut(0).expect("magic") ^= 1;
            assert!(!validate_found_schema(release, &hostile));
        }

        let mut bids = [[0u64; 2]; 2];
        let mut asks = [[0u64; 2]; 2];
        for (bid, ask) in bids.iter_mut().zip(asks.iter_mut()) {
            *bid = [4_000, 3_900];
            *ask = [6_000, 6_100];
        }
        let config = LiquidityConfigV1::<2, 2>::new(
            CoreContentId::new([31; 32]).expect("content"),
            [32; 32],
            10_000,
            25,
            100,
            10,
            bids,
            asks,
            [[100; 2]; 2],
            [[100; 2]; 2],
        )
        .expect("Dealer config");
        let mut bytes = vec![0; LiquidityConfigV1::<2, 2>::encoded_len().expect("width")];
        config.encode_into(&mut bytes).expect("encode config");
        let release = SchemaReleaseId::new(DEALER_CONFIG_SCHEMA_RELEASE_ID_V1).expect("schema");
        assert!(validate_found_schema(release, &bytes));
        *bytes.get_mut(11).expect("reserved byte") = 1;
        assert!(!validate_found_schema(release, &bytes));
    }

    #[test]
    fn dealer_record_view_preserves_the_closed_profile_set_and_checks_the_full_slice() {
        let release = SchemaReleaseId::new(DEALER_CONFIG_SCHEMA_RELEASE_ID_V1).expect("schema");
        let large = LiquidityConfigV1::<16, 8>::new(
            CoreContentId::new([41; 32]).expect("content"),
            [42; 32],
            10_000,
            25,
            100,
            10,
            [[500, 499, 498, 497, 496, 495, 494, 493]; 16],
            [[625, 626, 627, 628, 629, 630, 631, 632]; 16],
            [[100; 8]; 16],
            [[100; 8]; 16],
        )
        .expect("largest admitted Dealer config");
        let mut large_bytes = vec![0; LiquidityConfigV1::<16, 8>::encoded_len().expect("width")];
        large.encode_into(&mut large_bytes).expect("encode config");
        assert!(validate_found_schema(release, &large_bytes));

        let last_capacity = large_bytes.len() - core::mem::size_of::<u64>();
        *large_bytes
            .get_mut(last_capacity)
            .expect("last capacity cell") = 0;
        assert!(!validate_found_schema(release, &large_bytes));

        let one_bin = LiquidityConfigV1::<2, 1>::new(
            CoreContentId::new([43; 32]).expect("content"),
            [44; 32],
            10_000,
            25,
            100,
            10,
            [[4_000]; 2],
            [[6_000]; 2],
            [[100]; 2],
            [[100]; 2],
        )
        .expect("contract-valid one-bin config");
        let mut one_bin_bytes = vec![0; LiquidityConfigV1::<2, 1>::encoded_len().expect("width")];
        one_bin
            .encode_into(&mut one_bin_bytes)
            .expect("encode config");
        assert!(!validate_found_schema(release, &one_bin_bytes));
    }

    #[test]
    fn general_record_requires_exact_canonical_decode() {
        let identity = |value: u8| GeneralContentId::new([value; 32]).expect("identity");
        let config = GeneralConfigV1::new(GeneralConfigV1Input {
            capacity_profile_id: identity(1),
            claim_basis_id: identity(3),
            capability_release_id: GENERAL_CAPABILITY_RELEASE_ID_V1,
            generation: 7,
            price_scale: 100,
            collection_slots: 10,
            selection_slots: 10,
            settlement_slots: 10,
            max_orders_per_candidate: 4,
            max_pages_per_candidate: 1,
            continuation_reward_lamports: 1,
            outcome_count: 2,
        })
        .expect("General config");
        let bytes = config.to_bytes();
        let release = SchemaReleaseId::new(GENERAL_CONFIG_SCHEMA_ID_V1.to_bytes()).expect("schema");
        assert!(validate_found_schema(release, &bytes));
        assert!(!validate_found_schema(
            release,
            bytes.get(..bytes.len() - 1).expect("short record"),
        ));

        let mut hostile = bytes;
        *hostile.get_mut(0).expect("magic byte") ^= 1;
        assert!(!validate_found_schema(release, &hostile));

        let mut hostile = bytes;
        *hostile.get_mut(11).expect("reserved byte") = 1;
        assert!(!validate_found_schema(release, &hostile));
    }

    #[test]
    fn dusted_empty_system_pdas_remain_absent_but_not_close_vacant() {
        let dusted = test_account(
            Pubkey::new_unique(),
            false,
            false,
            41,
            Vec::new(),
            system_program::ID,
            false,
        );
        assert!(is_prefunded_vacant(&dusted));
        assert!(is_finalized_staging_vacancy(&dusted));
        assert!(!is_vacant(&dusted));

        let data_poison = test_account(
            Pubkey::new_unique(),
            false,
            false,
            41,
            vec![0],
            system_program::ID,
            false,
        );
        let owner_poison = test_account(
            Pubkey::new_unique(),
            false,
            false,
            41,
            Vec::new(),
            Pubkey::new_unique(),
            false,
        );
        assert!(!is_finalized_staging_vacancy(&data_poison));
        assert!(!is_finalized_staging_vacancy(&owner_poison));
    }

    #[test]
    fn consumer_authentication_requires_the_actual_derived_cursor_vacancy() {
        let program_id = Pubkey::new_unique();
        let realm = RealmV1::new(RealmV1Input {
            token_program: [1; 32],
            collateral_mint: [2; 32],
            collateral_adapter_release_id: [3; 32],
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("realm");
        let content = realm.to_bytes();
        let schema = REALM_SCHEMA_RELEASE_ID_V1;
        let digest = hash(&content).to_bytes();
        let key = RecordKeyV1::new(
            SchemaReleaseId::new(schema).expect("schema"),
            ContentDigest::new(digest).expect("digest"),
        );
        let (raw_key, _) = derive_record_pda(&program_id, key, false);
        let (cursor_key, _) = derive_record_pda(&program_id, key, true);
        let rent = Rent::default();
        let raw = test_account(
            raw_key,
            false,
            false,
            rent.minimum_balance(content.len()),
            content.to_vec(),
            program_id,
            false,
        );
        let vacancy = test_account(
            cursor_key,
            false,
            false,
            0,
            Vec::new(),
            system_program::ID,
            false,
        );
        let mut rent_account = test_account(
            sysvar::rent::ID,
            false,
            false,
            1,
            vec![0; Rent::size_of()],
            sysvar::ID,
            false,
        );
        assert_eq!(rent.to_account_info(&mut rent_account), Some(()));

        let authenticated_length = with_authenticated_finalized_record_v1(
            &program_id,
            &raw,
            &vacancy,
            &rent_account,
            schema,
            digest,
            |receipt| Ok(receipt.exact_content().len()),
        )
        .expect("actual vacancy authenticates");
        assert_eq!(authenticated_length, content.len());

        **vacancy
            .try_borrow_mut_lamports()
            .expect("dust vacancy lamports") = 41;
        assert_eq!(
            with_authenticated_finalized_record_v1(
                &program_id,
                &raw,
                &vacancy,
                &rent_account,
                schema,
                digest,
                |receipt| Ok(receipt.exact_content().len()),
            ),
            Ok(content.len())
        );

        let live_staging_cursor = test_account(
            cursor_key,
            false,
            false,
            1,
            vec![0; STAGING_CURSOR_BYTES_V1],
            program_id,
            false,
        );
        assert_eq!(
            with_authenticated_finalized_record_v1(
                &program_id,
                &raw,
                &live_staging_cursor,
                &rent_account,
                schema,
                digest,
                |_| Ok(()),
            ),
            Err(AdapterError::AccountIdentity.into())
        );
    }

    #[test]
    fn rent_credit_authentication_binds_authority_and_allows_terminal_top_up() {
        let program_id = Pubkey::new_unique();
        let authority = refund_authority(&Pubkey::new_unique()).expect("authority");
        let authority_bytes = authority.to_bytes();
        let (credit_key, bump) = Pubkey::find_program_address(
            &[RENT_CREDIT_PDA_DOMAIN_V1, authority_bytes.as_slice()],
            &program_id,
        );
        let state = RentCreditV1::new(authority, bump);
        let credit = test_account(
            credit_key,
            false,
            false,
            90,
            state.to_bytes().to_vec(),
            program_id,
            false,
        );
        assert_eq!(
            authenticate_rent_credit(&program_id, &credit, authority, None),
            Ok(state)
        );
        assert_eq!(
            authenticate_rent_credit(&program_id, &credit, authority, Some(100)),
            Err(AdapterError::AccountIdentity.into())
        );
        **credit.try_borrow_mut_lamports().expect("credit lamports") = 100;
        assert_eq!(
            authenticate_rent_credit(&program_id, &credit, authority, Some(100)),
            Ok(state)
        );

        let wrong_key = test_account(
            Pubkey::new_unique(),
            false,
            false,
            100,
            state.to_bytes().to_vec(),
            program_id,
            false,
        );
        assert_eq!(
            authenticate_rent_credit(&program_id, &wrong_key, authority, None),
            Err(AdapterError::AccountIdentity.into())
        );
    }

    #[test]
    fn expired_abort_credits_exact_sponsor_share_and_pays_only_bounty() {
        let program_id = Pubkey::new_unique();
        let sponsor_key = Pubkey::new_unique();
        let actor_key = Pubkey::new_unique();
        let key = RecordKeyV1::new(
            SchemaReleaseId::new(REALM_SCHEMA_RELEASE_ID_V1).expect("schema"),
            ContentDigest::new([7; 32]).expect("digest"),
        );
        let (raw_key, _) = derive_record_pda(&program_id, key, false);
        let (cursor_key, _) = derive_record_pda(&program_id, key, true);
        let raw = test_account(
            raw_key,
            false,
            true,
            70,
            vec![0; REALM_BYTES],
            program_id,
            false,
        );
        let cursor_account = test_account(
            cursor_key,
            false,
            true,
            150,
            vec![0; STAGING_CURSOR_BYTES_V1],
            program_id,
            false,
        );
        let page_envelope = PageEnvelopeV1::new(
            PageEnvelopeKindV1::Provisional,
            PAGE_BYTES_V1,
            SchemaReleaseId::new(PAGE_ENVELOPE_RELEASE_ID_V1).expect("page release"),
        )
        .expect("page envelope");
        let liveness = StagingLivenessPolicyV1::new(
            SchemaReleaseId::new(STAGING_LIVENESS_RELEASE_ID_V1).expect("liveness release"),
            MAX_STAGING_LIFETIME_SLOTS_V1,
            41,
        )
        .expect("liveness");
        let request = BeginRecordV1::new(
            key,
            u64::try_from(REALM_BYTES).expect("bounded length"),
            page_envelope,
            liveness.policy_id(),
            10,
            50,
        )
        .expect("begin");
        let adapter = SbfRecordAdapter::begin(&program_id, &raw, &cursor_account, 41);
        let cursor = prepare_begin_v1(
            &adapter,
            request,
            liveness,
            1,
            account_id(raw.key).expect("raw id"),
            account_id(cursor_account.key).expect("cursor id"),
            account_id(&sponsor_key).expect("sponsor id"),
        )
        .expect("cursor")
        .cursor();
        let transition = prepare_abort_v1(
            cursor,
            AbortObservationV1::new(
                account_id(raw.key).expect("raw id"),
                account_id(cursor_account.key).expect("cursor id"),
                u64::try_from(REALM_BYTES).expect("bounded length"),
                70,
                150,
                10,
                account_id(&actor_key).expect("actor id"),
            ),
        )
        .expect("expired abort");
        assert!(!transition.sponsor_signature_required());

        let authority = refund_authority(&sponsor_key).expect("authority");
        let authority_bytes = authority.to_bytes();
        let (credit_key, bump) = Pubkey::find_program_address(
            &[RENT_CREDIT_PDA_DOMAIN_V1, authority_bytes.as_slice()],
            &program_id,
        );
        let credit_state = RentCreditV1::new(authority, bump);
        let credit = test_account(
            credit_key,
            false,
            true,
            100,
            credit_state.to_bytes().to_vec(),
            program_id,
            false,
        );
        let actor = test_account(
            actor_key,
            true,
            true,
            7,
            Vec::new(),
            system_program::ID,
            false,
        );
        close_full_to_rent_credit(
            &program_id,
            &raw,
            &credit,
            credit_state,
            transition.raw_record_close(),
        )
        .expect("raw credit");
        close_split_to_rent_credit(
            &program_id,
            &cursor_account,
            &actor,
            &credit,
            credit_state,
            transition.staging_close(),
        )
        .expect("cursor split");
        assert_eq!(actor.lamports(), 57);
        assert_eq!(credit.lamports(), 270);
        assert!(is_vacant(&raw));
        assert!(is_vacant(&cursor_account));
        assert_eq!(
            &credit.try_borrow_data().expect("credit data")[..],
            &credit_state.to_bytes(),
        );
    }

    #[test]
    fn append_requires_the_cursor_sponsor_and_refuses_poison_without_mutation() {
        let program_id = Pubkey::new_unique();
        let sponsor_key = Pubkey::new_unique();
        let realm = RealmV1::new(RealmV1Input {
            token_program: [1; 32],
            collateral_mint: [2; 32],
            collateral_adapter_release_id: [3; 32],
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("realm");
        let content = realm.to_bytes();
        let key = RecordKeyV1::new(
            SchemaReleaseId::new(REALM_SCHEMA_RELEASE_ID_V1).expect("schema"),
            ContentDigest::new(hash(&content).to_bytes()).expect("digest"),
        );
        let (raw_key, _) = derive_record_pda(&program_id, key, false);
        let (cursor_key, _) = derive_record_pda(&program_id, key, true);
        let raw = test_account(
            raw_key,
            false,
            true,
            1,
            vec![0; content.len()],
            program_id,
            false,
        );
        let cursor_shell = test_account(
            cursor_key,
            false,
            true,
            1,
            vec![0; STAGING_CURSOR_BYTES_V1],
            program_id,
            false,
        );
        let page_envelope = PageEnvelopeV1::new(
            PageEnvelopeKindV1::Provisional,
            PAGE_BYTES_V1,
            SchemaReleaseId::new(PAGE_ENVELOPE_RELEASE_ID_V1).expect("page release"),
        )
        .expect("page envelope");
        let liveness = StagingLivenessPolicyV1::new(
            SchemaReleaseId::new(STAGING_LIVENESS_RELEASE_ID_V1).expect("liveness release"),
            MAX_STAGING_LIFETIME_SLOTS_V1,
            41,
        )
        .expect("liveness");
        let request = BeginRecordV1::new(
            key,
            u64::try_from(content.len()).expect("bounded content"),
            page_envelope,
            liveness.policy_id(),
            100,
            41,
        )
        .expect("begin");
        let adapter = SbfRecordAdapter::begin(&program_id, &raw, &cursor_shell, 41);
        let initial_cursor = prepare_begin_v1(
            &adapter,
            request,
            liveness,
            1,
            account_id(raw.key).expect("raw id"),
            account_id(cursor_shell.key).expect("cursor id"),
            account_id(&sponsor_key).expect("sponsor id"),
        )
        .expect("cursor")
        .cursor();
        {
            let mut data = cursor_shell
                .try_borrow_mut_data()
                .expect("cursor data borrow");
            data.copy_from_slice(&initial_cursor.to_bytes());
        }
        let sponsor = test_account(
            sponsor_key,
            true,
            false,
            1,
            vec![0xa5],
            Pubkey::new_unique(),
            true,
        );
        let attacker = test_account(
            Pubkey::new_unique(),
            true,
            false,
            1,
            Vec::new(),
            system_program::ID,
            false,
        );
        let poison = [0xa5; REALM_BYTES];
        let hostile_append = AppendPageV1::new(0, 0, &poison).expect("hostile append");
        let raw_before = raw.try_borrow_data().expect("raw data").to_vec();
        let cursor_before = cursor_shell
            .try_borrow_data()
            .expect("cursor data")
            .to_vec();
        assert_eq!(
            process_append(
                &program_id,
                &[attacker, raw.clone(), cursor_shell.clone()],
                hostile_append,
            ),
            Err(AdapterError::AccountIdentity.into())
        );
        assert_eq!(&raw.try_borrow_data().expect("raw data")[..], &raw_before);
        assert_eq!(
            &cursor_shell.try_borrow_data().expect("cursor data")[..],
            &cursor_before,
        );

        let append = AppendPageV1::new(0, 0, &content).expect("append");
        process_append(
            &program_id,
            &[sponsor, raw.clone(), cursor_shell.clone()],
            append,
        )
        .expect("sponsor append");
        assert_eq!(&raw.try_borrow_data().expect("raw data")[..], &content);
        assert!(
            decode_cursor(&cursor_shell)
                .expect("advanced cursor")
                .is_complete()
        );
    }

    #[test]
    fn unified_abort_frame_accepts_readonly_sponsor_or_writable_cleaner() {
        let sponsor = test_account(
            Pubkey::new_unique(),
            true,
            false,
            1,
            vec![0xa5],
            Pubkey::new_unique(),
            true,
        );
        let raw = test_account(
            Pubkey::new_unique(),
            false,
            true,
            1,
            Vec::new(),
            Pubkey::new_unique(),
            false,
        );
        let cursor = test_account(
            Pubkey::new_unique(),
            false,
            true,
            1,
            vec![0; STAGING_CURSOR_BYTES_V1],
            Pubkey::new_unique(),
            false,
        );
        let clock = test_account(
            sysvar::clock::ID,
            false,
            false,
            1,
            Vec::new(),
            sysvar::ID,
            false,
        );
        let rent_credit = test_account(
            Pubkey::new_unique(),
            false,
            true,
            1,
            vec![0; RENT_CREDIT_BYTES_V1],
            Pubkey::new_unique(),
            false,
        );
        assert!(
            AbortFrame::parse(&[
                sponsor,
                raw.clone(),
                cursor.clone(),
                rent_credit.clone(),
                clock.clone(),
            ])
            .is_ok()
        );

        let cleaner = test_account(
            Pubkey::new_unique(),
            true,
            true,
            1,
            Vec::new(),
            system_program::ID,
            false,
        );
        assert!(
            AbortFrame::parse(&[
                cleaner.clone(),
                raw.clone(),
                cursor.clone(),
                rent_credit.clone(),
                clock.clone(),
            ])
            .is_ok()
        );
        assert!(AbortFrame::parse(&[cleaner, raw.clone(), cursor, raw, clock]).is_err());
    }
}
