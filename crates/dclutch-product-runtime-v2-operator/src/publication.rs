//! Chain-derived publication of generic immutable Registry records.
//!
//! The Registry record contract owns the wire and transition semantics. This
//! host adapter only reacquires one finalized snapshot, derives the canonical
//! content-addressed accounts, and selects exactly one unsigned
//! Begin/Append/Finalize step. It accepts no schema-specific DTO and performs
//! no RPC, signing, submission, funding, or account mutation.

use core::convert::TryFrom;

use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_record_contract::{
    APPEND_PAGE_HEADER_BYTES_V1, AppendPageV1, BeginRecordV1,
    CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1, ContentDigest, FinalizeRecordV1,
    RAW_RECORD_PDA_SEED_V1, RecordKeyV1, STAGING_CURSOR_BYTES_V1, STAGING_CURSOR_PDA_SEED_V1,
    SchemaReleaseId, StagingCursorV1,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};

use crate::{AccountObservationV2, CompiledProductRecordsV2};

/// Stable refusal from generic record publication or Product graph joining.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationErrorV1 {
    /// The schema identity, digest, cursor, or wire was not canonical.
    Record,
    /// Account observations did not share one finalized snapshot.
    ObservationMismatch,
    /// An account key, owner, executable bit, or data shape was invalid.
    AccountAuthority,
    /// The supplied raw or staging account was not the canonical PDA.
    AddressMismatch,
    /// Checked offset, length, slot, or lamport arithmetic overflowed.
    ArithmeticOverflow,
    /// Sponsor principal could not cover the current exact allocation debit.
    SponsorUnderfunded,
    /// Product/domain/portfolio bytes did not match the compiled graph.
    ProductGraphMismatch,
}

/// One immutable schema/content pair to publish under the selected Registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordPublicationContentV1<'a> {
    /// Opaque consumer-owned schema and validator release identity.
    pub schema_release_id: [u8; 32],
    /// Complete exact headerless semantic bytes.
    pub content: &'a [u8],
}

/// Same-finalized account snapshot for one publication decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordPublicationStateV1<'a> {
    /// System wallet that signs Begin/Append and receives the cursor refund.
    pub sponsor: AccountObservationV2<'a>,
    /// Canonical raw-record PDA, either prefunded-vacant or Registry-owned.
    pub raw_record: AccountObservationV2<'a>,
    /// Canonical staging PDA, either prefunded-vacant or Registry-owned.
    pub staging_cursor: AccountObservationV2<'a>,
    /// Canonical executable System Program.
    pub system_program: AccountObservationV2<'a>,
    /// Canonical Rent sysvar.
    pub rent: AccountObservationV2<'a>,
    /// Canonical Clock sysvar selecting the absolute staging expiry.
    pub clock: AccountObservationV2<'a>,
}

/// Sole next publication transition selected by current chain state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordPublicationActionV1 {
    /// Allocate the exact raw account and temporary staging cursor.
    Begin,
    /// Append the cursor-selected next semantic page.
    Append,
    /// Authenticate the complete raw bytes and close the cursor.
    Finalize,
    /// The raw record is already finalized and needs no instruction.
    Complete,
}

/// One exact unsigned publication step and its derived consequences.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordPublicationPlanV1 {
    /// Sole next action.
    pub action: RecordPublicationActionV1,
    /// Exact unsigned instruction, absent only for [`RecordPublicationActionV1::Complete`].
    pub instruction: Option<Instruction>,
    /// Canonical raw-record PDA.
    pub raw_record: Pubkey,
    /// Canonical temporary cursor PDA.
    pub staging_cursor: Pubkey,
    /// Digest of the complete exact semantic bytes.
    pub content_digest: [u8; 32],
    /// Current or next zero-based page index.
    pub page_index: u64,
    /// Current or next exact raw byte offset.
    pub byte_offset: u64,
    /// Exact current sponsor debit for Begin; zero otherwise.
    pub sponsor_debit: u64,
    /// Exact cursor balance returned on Finalize; zero otherwise.
    pub cursor_refund: u64,
    /// Finalized RPC observation shared by every input account.
    pub observation_slot: u64,
}

/// One Runtime V2 Product graph's three exact publication payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductPublicationContentV2<'a> {
    /// Fixed Product root record.
    pub product: RecordPublicationContentV1<'a>,
    /// Runtime-width exhaustive result-domain record.
    pub result_domain: RecordPublicationContentV1<'a>,
    /// Runtime-width exact portfolio record.
    pub portfolio: RecordPublicationContentV1<'a>,
}

/// Same-snapshot publication states for one Product graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductPublicationStateV2<'a> {
    /// Product root record publication state.
    pub product: RecordPublicationStateV1<'a>,
    /// Result-domain record publication state.
    pub result_domain: RecordPublicationStateV1<'a>,
    /// Portfolio record publication state.
    pub portfolio: RecordPublicationStateV1<'a>,
}

/// Product graph member selected for the next publication step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductPublicationMemberV2 {
    /// Product root record.
    Product,
    /// Exhaustive result-domain record.
    ResultDomain,
    /// Exact portfolio record.
    Portfolio,
    /// All three records are finalized.
    Complete,
}

/// Canonically ordered next publication step for a Runtime V2 Product graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductPublicationPlanV2 {
    /// Graph member selected by Product→result-domain→portfolio order.
    pub member: ProductPublicationMemberV2,
    /// Generic record plan. A complete graph retains the final Complete plan.
    pub record: RecordPublicationPlanV1,
}

/// Derive the canonical raw and staging addresses for exact semantic bytes.
pub fn derive_record_addresses_v1(
    registry_program: Pubkey,
    content: RecordPublicationContentV1<'_>,
) -> Result<(Pubkey, Pubkey, [u8; 32]), PublicationErrorV1> {
    let key = record_key(content)?;
    let schema = key.schema_release_id().to_bytes();
    let digest = key.expected_digest().to_bytes();
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        &registry_program,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        &registry_program,
    )
    .0;
    Ok((raw, staging, digest))
}

/// Build exactly one generic immutable-record publication step.
pub fn build_record_publication_step_v1(
    registry_program: Pubkey,
    content: RecordPublicationContentV1<'_>,
    state: RecordPublicationStateV1<'_>,
) -> Result<RecordPublicationPlanV1, PublicationErrorV1> {
    let slot = state.sponsor.slot;
    for account in [
        state.raw_record,
        state.staging_cursor,
        state.system_program,
        state.rent,
        state.clock,
    ] {
        if account.slot != slot {
            return Err(PublicationErrorV1::ObservationMismatch);
        }
    }
    authenticate_common(state)?;
    let key = record_key(content)?;
    let (raw, staging, digest) = derive_record_addresses_v1(registry_program, content)?;
    if state.raw_record.key != raw || state.staging_cursor.key != staging {
        return Err(PublicationErrorV1::AddressMismatch);
    }
    let raw_vacant = vacant(state.raw_record);
    let cursor_vacant = vacant(state.staging_cursor);
    match (raw_vacant, cursor_vacant) {
        (true, true) => build_begin(registry_program, key, content, state, digest, slot),
        (false, false) => build_live(registry_program, key, content, state, digest, slot),
        (false, true) => build_complete(registry_program, content, state, digest, slot),
        (true, false) => Err(PublicationErrorV1::AccountAuthority),
    }
}

/// Join compiled Product coordinates to exact bytes without accepting schemas
/// or child identities from a caller.
pub fn product_publication_content_v2<'a>(
    registry_program: Pubkey,
    compiled: CompiledProductRecordsV2,
    product: &'a [u8],
    result_domain: &'a [u8],
    portfolio: &'a [u8],
) -> Result<ProductPublicationContentV2<'a>, PublicationErrorV1> {
    let content = ProductPublicationContentV2 {
        product: RecordPublicationContentV1 {
            schema_release_id: PRODUCT_RECORD_SCHEMA_ID_V2,
            content: product,
        },
        result_domain: RecordPublicationContentV1 {
            schema_release_id: RESULT_DOMAIN_SCHEMA_ID_V2,
            content: result_domain,
        },
        portfolio: RecordPublicationContentV1 {
            schema_release_id: PORTFOLIO_SCHEMA_ID_V2,
            content: portfolio,
        },
    };
    let product_coordinate = derive_record_addresses_v1(registry_program, content.product)?;
    let domain_coordinate = derive_record_addresses_v1(registry_program, content.result_domain)?;
    let portfolio_coordinate = derive_record_addresses_v1(registry_program, content.portfolio)?;
    if !matches_coordinate(compiled.receipt.product, product_coordinate)
        || !matches_coordinate(compiled.receipt.result_domain, domain_coordinate)
        || !matches_coordinate(compiled.receipt.portfolio, portfolio_coordinate)
        || compiled.request.product_digest.to_bytes() != product_coordinate.2
        || compiled.request.result_domain_digest.to_bytes() != domain_coordinate.2
        || compiled.request.portfolio_digest.to_bytes() != portfolio_coordinate.2
    {
        return Err(PublicationErrorV1::ProductGraphMismatch);
    }
    Ok(content)
}

/// Select the first incomplete record in canonical Product→domain→portfolio
/// order and build exactly one unsigned publication instruction.
pub fn build_product_publication_step_v2(
    registry_program: Pubkey,
    content: ProductPublicationContentV2<'_>,
    state: ProductPublicationStateV2<'_>,
) -> Result<ProductPublicationPlanV2, PublicationErrorV1> {
    let product =
        build_record_publication_step_v1(registry_program, content.product, state.product)?;
    if product.action != RecordPublicationActionV1::Complete {
        return Ok(ProductPublicationPlanV2 {
            member: ProductPublicationMemberV2::Product,
            record: product,
        });
    }
    let result_domain = build_record_publication_step_v1(
        registry_program,
        content.result_domain,
        state.result_domain,
    )?;
    if result_domain.action != RecordPublicationActionV1::Complete {
        return Ok(ProductPublicationPlanV2 {
            member: ProductPublicationMemberV2::ResultDomain,
            record: result_domain,
        });
    }
    let portfolio =
        build_record_publication_step_v1(registry_program, content.portfolio, state.portfolio)?;
    let member = if portfolio.action == RecordPublicationActionV1::Complete {
        ProductPublicationMemberV2::Complete
    } else {
        ProductPublicationMemberV2::Portfolio
    };
    Ok(ProductPublicationPlanV2 {
        member,
        record: portfolio,
    })
}

fn build_begin(
    registry_program: Pubkey,
    key: RecordKeyV1,
    content: RecordPublicationContentV1<'_>,
    state: RecordPublicationStateV1<'_>,
    digest: [u8; 32],
    slot: u64,
) -> Result<RecordPublicationPlanV1, PublicationErrorV1> {
    let rent = decode_rent(state.rent)?;
    let clock = decode_clock(state.clock)?;
    let raw_rent = rent.minimum_balance(content.content.len());
    let cursor_rent = rent.minimum_balance(STAGING_CURSOR_BYTES_V1);
    let cursor_balance = cursor_rent
        .checked_add(cursor_rent)
        .ok_or(PublicationErrorV1::ArithmeticOverflow)?;
    let raw_top_up = raw_rent.saturating_sub(state.raw_record.lamports);
    let cursor_top_up = cursor_balance.saturating_sub(state.staging_cursor.lamports);
    let sponsor_debit = raw_top_up
        .checked_add(cursor_top_up)
        .ok_or(PublicationErrorV1::ArithmeticOverflow)?;
    if state.sponsor.lamports < sponsor_debit {
        return Err(PublicationErrorV1::SponsorUnderfunded);
    }
    let profile = CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1;
    let expiry = clock
        .slot
        .checked_add(profile.maximum_staging_lifetime_slots())
        .ok_or(PublicationErrorV1::ArithmeticOverflow)?;
    let liveness = profile
        .staging_liveness_policy(cursor_rent)
        .map_err(|_| PublicationErrorV1::Record)?;
    let request = BeginRecordV1::new(
        key,
        u64::try_from(content.content.len()).map_err(|_| PublicationErrorV1::ArithmeticOverflow)?,
        profile
            .page_envelope()
            .map_err(|_| PublicationErrorV1::Record)?,
        liveness.policy_id(),
        expiry,
        cursor_rent,
    )
    .map_err(|_| PublicationErrorV1::Record)?;
    Ok(plan(
        RecordPublicationActionV1::Begin,
        Some(Instruction {
            program_id: registry_program,
            accounts: vec![
                AccountMeta::new(state.sponsor.key, true),
                AccountMeta::new(state.raw_record.key, false),
                AccountMeta::new(state.staging_cursor.key, false),
                AccountMeta::new_readonly(state.system_program.key, false),
                AccountMeta::new_readonly(state.rent.key, false),
                AccountMeta::new_readonly(state.clock.key, false),
            ],
            data: request.to_bytes().to_vec(),
        }),
        state,
        digest,
        0,
        0,
        sponsor_debit,
        0,
        slot,
    ))
}

fn build_live(
    registry_program: Pubkey,
    key: RecordKeyV1,
    content: RecordPublicationContentV1<'_>,
    state: RecordPublicationStateV1<'_>,
    digest: [u8; 32],
    slot: u64,
) -> Result<RecordPublicationPlanV1, PublicationErrorV1> {
    if state.raw_record.owner != registry_program
        || state.raw_record.executable
        || state.raw_record.data.len() != content.content.len()
        || state.staging_cursor.owner != registry_program
        || state.staging_cursor.executable
        || state.staging_cursor.data.len() != STAGING_CURSOR_BYTES_V1
        || state.staging_cursor.lamports == 0
    {
        return Err(PublicationErrorV1::AccountAuthority);
    }
    let cursor = StagingCursorV1::decode(state.staging_cursor.data)
        .map_err(|_| PublicationErrorV1::Record)?;
    if cursor.to_bytes().as_slice() != state.staging_cursor.data
        || cursor.key() != key
        || cursor.raw_record_account().to_bytes() != state.raw_record.key.to_bytes()
        || cursor.staging_account().to_bytes() != state.staging_cursor.key.to_bytes()
        || cursor.sponsor_rent_refund().to_bytes() != state.sponsor.key.to_bytes()
        || cursor.exact_length()
            != u64::try_from(content.content.len())
                .map_err(|_| PublicationErrorV1::ArithmeticOverflow)?
        || !CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1.validates_page_envelope(cursor.page_envelope())
    {
        return Err(PublicationErrorV1::Record);
    }
    if cursor.is_complete() {
        if state.raw_record.data != content.content
            || hash(state.raw_record.data).to_bytes() != digest
        {
            return Err(PublicationErrorV1::ProductGraphMismatch);
        }
        return Ok(plan(
            RecordPublicationActionV1::Finalize,
            Some(Instruction {
                program_id: registry_program,
                accounts: vec![
                    AccountMeta::new_readonly(state.raw_record.key, false),
                    AccountMeta::new(state.staging_cursor.key, false),
                    AccountMeta::new(state.sponsor.key, false),
                ],
                data: FinalizeRecordV1.to_bytes().to_vec(),
            }),
            state,
            digest,
            cursor.next_page(),
            cursor.next_offset(),
            0,
            state.staging_cursor.lamports,
            slot,
        ));
    }
    let start = usize::try_from(cursor.next_offset())
        .map_err(|_| PublicationErrorV1::ArithmeticOverflow)?;
    let remaining = content
        .content
        .len()
        .checked_sub(start)
        .ok_or(PublicationErrorV1::Record)?;
    let page_width = usize::try_from(cursor.page_envelope().page_bytes())
        .map_err(|_| PublicationErrorV1::ArithmeticOverflow)?;
    let end = start
        .checked_add(remaining.min(page_width))
        .ok_or(PublicationErrorV1::ArithmeticOverflow)?;
    let page = content
        .content
        .get(start..end)
        .ok_or(PublicationErrorV1::Record)?;
    let request = AppendPageV1::new(cursor.next_page(), cursor.next_offset(), page)
        .map_err(|_| PublicationErrorV1::Record)?;
    let mut data = vec![
        0;
        request
            .encoded_len()
            .map_err(|_| PublicationErrorV1::ArithmeticOverflow)?
    ];
    if data.len() != APPEND_PAGE_HEADER_BYTES_V1 + page.len() {
        return Err(PublicationErrorV1::ArithmeticOverflow);
    }
    request
        .encode(&mut data)
        .map_err(|_| PublicationErrorV1::Record)?;
    Ok(plan(
        RecordPublicationActionV1::Append,
        Some(Instruction {
            program_id: registry_program,
            accounts: vec![
                AccountMeta::new_readonly(state.sponsor.key, true),
                AccountMeta::new(state.raw_record.key, false),
                AccountMeta::new(state.staging_cursor.key, false),
            ],
            data,
        }),
        state,
        digest,
        cursor.next_page(),
        cursor.next_offset(),
        0,
        0,
        slot,
    ))
}

fn build_complete(
    registry_program: Pubkey,
    content: RecordPublicationContentV1<'_>,
    state: RecordPublicationStateV1<'_>,
    digest: [u8; 32],
    slot: u64,
) -> Result<RecordPublicationPlanV1, PublicationErrorV1> {
    let rent = decode_rent(state.rent)?;
    if state.raw_record.owner != registry_program
        || state.raw_record.executable
        || state.raw_record.data != content.content
        || hash(state.raw_record.data).to_bytes() != digest
        || !rent.is_exempt(state.raw_record.lamports, state.raw_record.data.len())
    {
        return Err(PublicationErrorV1::Record);
    }
    Ok(plan(
        RecordPublicationActionV1::Complete,
        None,
        state,
        digest,
        0,
        u64::try_from(content.content.len()).map_err(|_| PublicationErrorV1::ArithmeticOverflow)?,
        0,
        0,
        slot,
    ))
}

#[allow(clippy::too_many_arguments)]
fn plan(
    action: RecordPublicationActionV1,
    instruction: Option<Instruction>,
    state: RecordPublicationStateV1<'_>,
    content_digest: [u8; 32],
    page_index: u64,
    byte_offset: u64,
    sponsor_debit: u64,
    cursor_refund: u64,
    observation_slot: u64,
) -> RecordPublicationPlanV1 {
    RecordPublicationPlanV1 {
        action,
        instruction,
        raw_record: state.raw_record.key,
        staging_cursor: state.staging_cursor.key,
        content_digest,
        page_index,
        byte_offset,
        sponsor_debit,
        cursor_refund,
        observation_slot,
    }
}

fn authenticate_common(state: RecordPublicationStateV1<'_>) -> Result<(), PublicationErrorV1> {
    if state.sponsor.owner != system_program::ID
        || state.sponsor.executable
        || !state.sponsor.data.is_empty()
        || state.system_program.key != system_program::ID
        || state.system_program.owner != native_loader::ID
        || !state.system_program.executable
        || !state.system_program.data.is_empty()
        || state.rent.key != sysvar::rent::ID
        || state.rent.owner != sysvar::ID
        || state.rent.executable
        || state.clock.key != sysvar::clock::ID
        || state.clock.owner != sysvar::ID
        || state.clock.executable
    {
        return Err(PublicationErrorV1::AccountAuthority);
    }
    let keys = [
        state.sponsor.key,
        state.raw_record.key,
        state.staging_cursor.key,
        state.system_program.key,
        state.rent.key,
        state.clock.key,
    ];
    for (index, key) in keys.iter().enumerate() {
        if keys
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other == key)
        {
            return Err(PublicationErrorV1::AccountAuthority);
        }
    }
    Ok(())
}

fn record_key(content: RecordPublicationContentV1<'_>) -> Result<RecordKeyV1, PublicationErrorV1> {
    if content.content.is_empty() {
        return Err(PublicationErrorV1::Record);
    }
    Ok(RecordKeyV1::new(
        SchemaReleaseId::new(content.schema_release_id).map_err(|_| PublicationErrorV1::Record)?,
        ContentDigest::new(hash(content.content).to_bytes())
            .map_err(|_| PublicationErrorV1::Record)?,
    ))
}

fn vacant(account: AccountObservationV2<'_>) -> bool {
    account.owner == system_program::ID && !account.executable && account.data.is_empty()
}

fn decode_rent(account: AccountObservationV2<'_>) -> Result<Rent, PublicationErrorV1> {
    let mut lamports = account.lamports;
    let mut data = account.data.to_vec();
    let key = account.key;
    let owner = account.owner;
    let info = AccountInfo::new(
        &key,
        false,
        false,
        &mut lamports,
        &mut data,
        &owner,
        account.executable,
    );
    Rent::from_account_info(&info).map_err(|_| PublicationErrorV1::AccountAuthority)
}

fn decode_clock(account: AccountObservationV2<'_>) -> Result<Clock, PublicationErrorV1> {
    let mut lamports = account.lamports;
    let mut data = account.data.to_vec();
    let key = account.key;
    let owner = account.owner;
    let info = AccountInfo::new(
        &key,
        false,
        false,
        &mut lamports,
        &mut data,
        &owner,
        account.executable,
    );
    Clock::from_account_info(&info).map_err(|_| PublicationErrorV1::AccountAuthority)
}

fn matches_coordinate(
    coordinate: dclutch_product_runtime_v2_admission::FinalizedRecordCoordinateV2,
    derived: (Pubkey, Pubkey, [u8; 32]),
) -> bool {
    coordinate.raw_account.to_bytes() == derived.0.to_bytes()
        && coordinate.staging_account.to_bytes() == derived.1.to_bytes()
        && coordinate.content_digest.to_bytes() == derived.2
}
