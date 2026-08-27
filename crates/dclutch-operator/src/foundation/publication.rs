//! Chain-derived immutable-record Begin, Append, and Finalize construction.

use dclutch_capability_contract::CapabilityManifestV1;
use dclutch_product_contract::{
    capacity::CapacityProfileV1, claim::CategoricalUnitV1, product::InstanceV1,
};
use dclutch_realm_contract::RealmV1;
use dclutch_record_contract::{
    APPEND_PAGE_HEADER_BYTES_V1, AppendPageV1, BeginRecordV1,
    CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1, ContentDigest, FinalizeRecordV1, RecordKeyV1,
    STAGING_CURSOR_BYTES_V1, SchemaReleaseId, StagingCursorV1,
};
use dclutch_source_contract::{SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SourceMaterialViewV1};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use super::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_PREIMAGE_V1, CATEGORICAL_CLAIM_SCHEMA_RELEASE_PREIMAGE_V1,
    CreationRecordKindV1, CreationRecordObligationV1, FoundationError,
    PRODUCT_CAPACITY_SCHEMA_RELEASE_PREIMAGE_V1, PRODUCT_INSTANCE_SCHEMA_RELEASE_PREIMAGE_V1,
    REALM_SCHEMA_RELEASE_PREIMAGE_V1, authenticate_distinct, authenticate_sponsor,
    authenticate_system_program, decode_rent, require_observation,
};
use crate::{Observation, ObservedAccount, authenticate_rent_credit};

/// One finalized snapshot used to select the sole next publication action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImmutableRecordPublicationState {
    /// System-owned signing sponsor bound into a live cursor.
    pub sponsor: ObservedAccount,
    /// Canonical content-addressed raw-record account or vacant destination.
    pub raw_record: ObservedAccount,
    /// Canonical staging cursor or vacant destination.
    pub staging_cursor: ObservedAccount,
    /// Permanent sponsor-bound RentCredit used at Finalize.
    pub rent_credit: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Canonical Clock sysvar selecting Begin expiry.
    pub clock_sysvar: ObservedAccount,
}

/// Sole next action for a canonical immutable publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmutableRecordPublicationActionV1 {
    /// Allocate the raw record and staging cursor.
    Begin,
    /// Append exactly the next cursor-selected page.
    Append,
    /// Validate the complete raw content and close the staging cursor.
    Finalize,
    /// Raw content is already finalized and the cursor is vacant.
    Complete,
}

/// Chain-derived publication step and exact funding/progress facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImmutableRecordPublicationReport {
    /// Sole next action selected from observed account state.
    pub action: ImmutableRecordPublicationActionV1,
    /// Unsigned instruction, absent only when publication is complete.
    pub instruction: Option<Instruction>,
    /// Shared finalized observation selecting the action.
    pub observation: Observation,
    /// Current or next zero-based page index.
    pub page_index: u64,
    /// Current or next exact raw byte offset.
    pub byte_offset: u64,
    /// Exact sponsor top-up for this action.
    pub sponsor_debit: u64,
    /// Exact cursor balance returned to the permanent RentCredit on Finalize.
    pub cursor_refund: u64,
}

/// Build exactly one canonical publication step from observed chain state.
///
/// The selected deployment profile comes from `dclutch-record-contract`; this
/// builder carries no copied page, lifetime, or release coordinates.
pub fn build_immutable_record_publication_step_v1(
    program_id: Pubkey,
    state: &ImmutableRecordPublicationState,
    obligation: &CreationRecordObligationV1,
) -> Result<ImmutableRecordPublicationReport, FoundationError> {
    let observation = publication_observation(state)?;
    authenticate_system_program(&state.system_program)?;
    authenticate_sponsor(&state.sponsor)?;
    let rent = decode_rent(&state.rent_sysvar)?;
    let clock = super::decode_clock(&state.clock_sysvar)?;
    validate_obligation(program_id, obligation)?;
    if state.raw_record.key != obligation.raw_record
        || state.staging_cursor.key != obligation.staging_cursor
    {
        return Err(FoundationError::AddressMismatch);
    }
    authenticate_distinct(&[
        state.sponsor.key,
        state.raw_record.key,
        state.staging_cursor.key,
        state.rent_credit.key,
        state.system_program.key,
        state.rent_sysvar.key,
        state.clock_sysvar.key,
    ])?;
    authenticate_rent_credit(program_id, &state.rent_credit, state.sponsor.key)
        .map_err(|_| FoundationError::InvalidOwner)?;
    if !rent.is_exempt(state.rent_credit.lamports, state.rent_credit.data.len()) {
        return Err(FoundationError::AccountNotRentExempt);
    }

    let raw_vacant = vacant(&state.raw_record);
    let cursor_vacant = vacant(&state.staging_cursor);
    match (raw_vacant, cursor_vacant) {
        (true, true) => build_begin(
            program_id,
            state,
            obligation,
            observation,
            &rent,
            clock.slot,
        ),
        (false, false) => build_live(program_id, state, obligation, observation),
        (false, true) => build_complete(program_id, state, obligation, observation, &rent),
        (true, false) => Err(FoundationError::InvalidRecord),
    }
}

fn build_begin(
    program_id: Pubkey,
    state: &ImmutableRecordPublicationState,
    obligation: &CreationRecordObligationV1,
    observation: Observation,
    rent: &solana_program::rent::Rent,
    slot: u64,
) -> Result<ImmutableRecordPublicationReport, FoundationError> {
    let profile = CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1;
    let raw_rent = rent.minimum_balance(obligation.content.len());
    let cursor_rent = rent.minimum_balance(STAGING_CURSOR_BYTES_V1);
    let cursor_balance = cursor_rent
        .checked_add(cursor_rent)
        .ok_or(FoundationError::ArithmeticOverflow)?;
    let raw_top_up = raw_rent.saturating_sub(state.raw_record.lamports);
    let cursor_top_up = cursor_balance.saturating_sub(state.staging_cursor.lamports);
    let sponsor_debit = raw_top_up
        .checked_add(cursor_top_up)
        .ok_or(FoundationError::ArithmeticOverflow)?;
    if state.sponsor.lamports < sponsor_debit {
        return Err(FoundationError::SponsorUnderfunded);
    }
    let expiry = slot
        .checked_add(profile.maximum_staging_lifetime_slots())
        .ok_or(FoundationError::ArithmeticOverflow)?;
    let liveness = profile
        .staging_liveness_policy(cursor_rent)
        .map_err(|_| FoundationError::InvalidRecord)?;
    let key = record_key(obligation)?;
    let wire = BeginRecordV1::new(
        key,
        u64::try_from(obligation.content.len()).map_err(|_| FoundationError::ArithmeticOverflow)?,
        profile
            .page_envelope()
            .map_err(|_| FoundationError::InvalidRecord)?,
        liveness.policy_id(),
        expiry,
        cursor_rent,
    )
    .map_err(|_| FoundationError::InvalidRecord)?;
    let accounts = vec![
        AccountMeta::new(state.sponsor.key, true),
        AccountMeta::new(state.raw_record.key, false),
        AccountMeta::new(state.staging_cursor.key, false),
        AccountMeta::new_readonly(state.rent_credit.key, false),
        AccountMeta::new_readonly(state.system_program.key, false),
        AccountMeta::new_readonly(state.rent_sysvar.key, false),
        AccountMeta::new_readonly(state.clock_sysvar.key, false),
    ];
    Ok(ImmutableRecordPublicationReport {
        action: ImmutableRecordPublicationActionV1::Begin,
        instruction: Some(Instruction {
            program_id,
            accounts,
            data: wire.to_bytes().to_vec(),
        }),
        observation,
        page_index: 0,
        byte_offset: 0,
        sponsor_debit,
        cursor_refund: 0,
    })
}

fn build_live(
    program_id: Pubkey,
    state: &ImmutableRecordPublicationState,
    obligation: &CreationRecordObligationV1,
    observation: Observation,
) -> Result<ImmutableRecordPublicationReport, FoundationError> {
    if state.raw_record.owner != program_id
        || state.raw_record.executable
        || state.staging_cursor.owner != program_id
        || state.staging_cursor.executable
    {
        return Err(FoundationError::InvalidOwner);
    }
    let cursor = StagingCursorV1::decode(&state.staging_cursor.data)
        .map_err(|_| FoundationError::InvalidRecord)?;
    if cursor.to_bytes().as_slice() != state.staging_cursor.data.as_slice()
        || cursor.key() != record_key(obligation)?
        || cursor.raw_record_account().to_bytes() != state.raw_record.key.to_bytes()
        || cursor.staging_account().to_bytes() != state.staging_cursor.key.to_bytes()
        || cursor.sponsor_rent_refund().to_bytes() != state.sponsor.key.to_bytes()
        || cursor.exact_length()
            != u64::try_from(obligation.content.len())
                .map_err(|_| FoundationError::ArithmeticOverflow)?
        || state.raw_record.data.len() != obligation.content.len()
        || !CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1.validates_page_envelope(cursor.page_envelope())
    {
        return Err(FoundationError::ContentLinkMismatch);
    }
    if cursor.is_complete() {
        if state.raw_record.data != obligation.content
            || hash(&state.raw_record.data).to_bytes() != obligation.content_id
        {
            return Err(FoundationError::ContentLinkMismatch);
        }
        return Ok(ImmutableRecordPublicationReport {
            action: ImmutableRecordPublicationActionV1::Finalize,
            instruction: Some(Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new_readonly(state.raw_record.key, false),
                    AccountMeta::new(state.staging_cursor.key, false),
                    AccountMeta::new(state.rent_credit.key, false),
                ],
                data: FinalizeRecordV1.to_bytes().to_vec(),
            }),
            observation,
            page_index: cursor.next_page(),
            byte_offset: cursor.next_offset(),
            sponsor_debit: 0,
            cursor_refund: state.staging_cursor.lamports,
        });
    }
    let start =
        usize::try_from(cursor.next_offset()).map_err(|_| FoundationError::ArithmeticOverflow)?;
    let remaining = obligation
        .content
        .len()
        .checked_sub(start)
        .ok_or(FoundationError::ContentLinkMismatch)?;
    let length = remaining.min(
        usize::try_from(cursor.page_envelope().page_bytes())
            .map_err(|_| FoundationError::ArithmeticOverflow)?,
    );
    let end = start
        .checked_add(length)
        .ok_or(FoundationError::ArithmeticOverflow)?;
    let page = obligation
        .content
        .get(start..end)
        .ok_or(FoundationError::ContentLinkMismatch)?;
    let wire = AppendPageV1::new(cursor.next_page(), cursor.next_offset(), page)
        .map_err(|_| FoundationError::InvalidRecord)?;
    let mut data = vec![0; APPEND_PAGE_HEADER_BYTES_V1 + page.len()];
    wire.encode(&mut data)
        .map_err(|_| FoundationError::InstructionEncoding)?;
    Ok(ImmutableRecordPublicationReport {
        action: ImmutableRecordPublicationActionV1::Append,
        instruction: Some(Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new_readonly(state.sponsor.key, true),
                AccountMeta::new(state.raw_record.key, false),
                AccountMeta::new(state.staging_cursor.key, false),
            ],
            data,
        }),
        observation,
        page_index: cursor.next_page(),
        byte_offset: cursor.next_offset(),
        sponsor_debit: 0,
        cursor_refund: 0,
    })
}

fn build_complete(
    program_id: Pubkey,
    state: &ImmutableRecordPublicationState,
    obligation: &CreationRecordObligationV1,
    observation: Observation,
    rent: &solana_program::rent::Rent,
) -> Result<ImmutableRecordPublicationReport, FoundationError> {
    if state.raw_record.owner != program_id
        || state.raw_record.executable
        || state.raw_record.data != obligation.content
        || hash(&state.raw_record.data).to_bytes() != obligation.content_id
        || !rent.is_exempt(state.raw_record.lamports, state.raw_record.data.len())
    {
        return Err(FoundationError::InvalidRecord);
    }
    Ok(ImmutableRecordPublicationReport {
        action: ImmutableRecordPublicationActionV1::Complete,
        instruction: None,
        observation,
        page_index: 0,
        byte_offset: u64::try_from(obligation.content.len())
            .map_err(|_| FoundationError::ArithmeticOverflow)?,
        sponsor_debit: 0,
        cursor_refund: 0,
    })
}

fn publication_observation(
    state: &ImmutableRecordPublicationState,
) -> Result<Observation, FoundationError> {
    require_observation(&[
        state.sponsor.observation,
        state.raw_record.observation,
        state.staging_cursor.observation,
        state.rent_credit.observation,
        state.system_program.observation,
        state.rent_sysvar.observation,
        state.clock_sysvar.observation,
    ])
}

fn record_key(obligation: &CreationRecordObligationV1) -> Result<RecordKeyV1, FoundationError> {
    Ok(RecordKeyV1::new(
        SchemaReleaseId::new(obligation.schema_release_id)
            .map_err(|_| FoundationError::InvalidRecord)?,
        ContentDigest::new(obligation.content_id).map_err(|_| FoundationError::InvalidRecord)?,
    ))
}

fn vacant(account: &ObservedAccount) -> bool {
    account.owner == system_program::ID && !account.executable && account.data.is_empty()
}

fn validate_obligation(
    program_id: Pubkey,
    obligation: &CreationRecordObligationV1,
) -> Result<(), FoundationError> {
    let expected_schema = match obligation.kind {
        CreationRecordKindV1::Realm => hash(REALM_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes(),
        CreationRecordKindV1::ProductInstance => {
            hash(PRODUCT_INSTANCE_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes()
        }
        CreationRecordKindV1::ClaimBasis => {
            hash(CATEGORICAL_CLAIM_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes()
        }
        CreationRecordKindV1::ProductCapacityProfile => {
            hash(PRODUCT_CAPACITY_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes()
        }
        CreationRecordKindV1::SourceMaterial => SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        CreationRecordKindV1::CapabilityManifest => {
            hash(CAPABILITY_MANIFEST_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes()
        }
    };
    if obligation.schema_release_id != expected_schema
        || hash(&obligation.content).to_bytes() != obligation.content_id
    {
        return Err(FoundationError::ContentLinkMismatch);
    }
    let canonical = match obligation.kind {
        CreationRecordKindV1::Realm => RealmV1::decode(&obligation.content)
            .map(|value| value.to_bytes().to_vec())
            .map_err(|_| FoundationError::InvalidRecord)?,
        CreationRecordKindV1::ProductInstance => InstanceV1::decode(&obligation.content)
            .map(|value| value.to_bytes().to_vec())
            .map_err(|_| FoundationError::InvalidRecord)?,
        CreationRecordKindV1::ClaimBasis => CategoricalUnitV1::decode(&obligation.content)
            .map(|value| value.to_bytes().to_vec())
            .map_err(|_| FoundationError::InvalidRecord)?,
        CreationRecordKindV1::ProductCapacityProfile => {
            CapacityProfileV1::decode(&obligation.content)
                .map(|value| value.to_bytes().to_vec())
                .map_err(|_| FoundationError::InvalidRecord)?
        }
        CreationRecordKindV1::SourceMaterial => SourceMaterialViewV1::decode(&obligation.content)
            .map(|value| value.as_bytes().to_vec())
            .map_err(|_| FoundationError::InvalidRecord)?,
        CreationRecordKindV1::CapabilityManifest => {
            CapabilityManifestV1::decode(&obligation.content)
                .map(|value| value.as_bytes().to_vec())
                .map_err(|_| FoundationError::InvalidRecord)?
        }
    };
    if canonical != obligation.content {
        return Err(FoundationError::NonCanonicalRecord);
    }
    let key = record_key(obligation)?;
    let raw = key.raw_record_pda_seeds();
    let cursor = key.staging_cursor_pda_seeds();
    let (expected_raw, _) = Pubkey::find_program_address(
        &[
            raw.domain(),
            raw.schema_release_id().as_bytes(),
            raw.expected_digest().as_bytes(),
        ],
        &program_id,
    );
    let (expected_cursor, _) = Pubkey::find_program_address(
        &[
            cursor.domain(),
            cursor.schema_release_id().as_bytes(),
            cursor.expected_digest().as_bytes(),
        ],
        &program_id,
    );
    if obligation.raw_record != expected_raw || obligation.staging_cursor != expected_cursor {
        return Err(FoundationError::AddressMismatch);
    }
    Ok(())
}
