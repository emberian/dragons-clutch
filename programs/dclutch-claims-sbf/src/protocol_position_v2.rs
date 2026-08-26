//! Canonical protocol-owned Claims Position admission.
//!
//! The current Registry-selected Trading program authorizes one exact request.
//! Claims derives the ordinary Position PDA from the logical Market and the
//! authenticated Trading-owned child root, admits width from finalized Product
//! and LiabilityBasisV2 truth, and creates zero inventory without touching the
//! aggregate. A separate Claims-owned admission record persists finality and
//! prepaid-rent coordinates that do not belong in the exact Position ABI.

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use core::convert::{TryFrom, TryInto};

use dclutch_claims_svm::{ClaimsAggregateSeedsV1, ClaimsPositionSeedsV1};
use dclutch_core_contract::ContentId;
use dclutch_economic_slice_kernel::{
    POSITION_HEADER_BYTES, Phase as EconomicPhase, SCALAR_BYTES, initialize_position,
    market_identity, market_outcome_count, market_phase, market_registry_program,
    market_release_set_id, market_revision,
};
use dclutch_liability_basis_v2_kernel::product_claims::{
    AdmittedBasisV2, ContentIdV2, LinkedBasisRecordV2,
};
use dclutch_market_core_codec::{CoreState, Phase as CorePhase, STATE_BYTES};
use dclutch_product_contract::product::{InstanceV1, PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::Instruction,
    program::{invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign};

use super::{liability_basis_v2::LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2, reauthenticate};

/// Protocol Position admission request magic.
pub const PROTOCOL_POSITION_REQUEST_MAGIC_V2: [u8; 8] = *b"DCLPPR02";
/// Exact protocol Position admission request width.
pub const PROTOCOL_POSITION_REQUEST_BYTES_V2: usize = 224;
/// Claims-owned protocol Position admission state width.
pub const PROTOCOL_POSITION_ADMISSION_BYTES_V2: usize = 512;
/// Exact admission receipt width.
pub const PROTOCOL_POSITION_RECEIPT_BYTES_V2: usize = 512;
/// Claims admission-record PDA seed domain.
pub const PROTOCOL_POSITION_ADMISSION_SEED_V2: &[u8] = b"dclutch:protocol-position:v2";
/// Exact account count for protocol Position admission.
pub const PROTOCOL_POSITION_ACCOUNT_COUNT_V2: usize = 20;

const ABI_VERSION_V2: u16 = 2;
const ADMISSION_MAGIC_V2: [u8; 8] = *b"DCLPPS02";
const RECEIPT_MAGIC_V2: [u8; 8] = *b"DCLPPC02";
const BASIS_SEMANTIC_ID_DOMAIN_V2: &[u8] = b"dclutch/lbv2/semantic-id/v2";

const REQUEST_RELEASE_OFFSET: usize = 16;
const REQUEST_MARKET_OFFSET: usize = 48;
const REQUEST_CHILD_ROOT_OFFSET: usize = 80;
const REQUEST_PARENT_DIGEST_OFFSET: usize = 112;
const REQUEST_REFUND_OFFSET: usize = 144;
const REQUEST_GENERATION_OFFSET: usize = 176;
const REQUEST_MARKET_REVISION_OFFSET: usize = 184;
const REQUEST_POSITION_RENT_OFFSET: usize = 192;
const REQUEST_ADMISSION_RENT_OFFSET: usize = 200;

const EVIDENCE_RELEASE_OFFSET: usize = 16;
const EVIDENCE_MARKET_OFFSET: usize = 48;
const EVIDENCE_CHILD_ROOT_OFFSET: usize = 80;
const EVIDENCE_POSITION_OFFSET: usize = 112;
const EVIDENCE_ADMISSION_OFFSET: usize = 144;
const EVIDENCE_PRODUCT_OFFSET: usize = 176;
const EVIDENCE_BASIS_OFFSET: usize = 208;
const EVIDENCE_LINKED_DIGEST_OFFSET: usize = 240;
const EVIDENCE_PARENT_DIGEST_OFFSET: usize = 272;
const EVIDENCE_REQUEST_DIGEST_OFFSET: usize = 304;
const EVIDENCE_REFUND_OFFSET: usize = 336;
const EVIDENCE_CLAIMS_PROGRAM_OFFSET: usize = 368;
const EVIDENCE_TRADING_PROGRAM_OFFSET: usize = 400;
const EVIDENCE_POSITION_DIGEST_OFFSET: usize = 432;
const EVIDENCE_GENERATION_OFFSET: usize = 464;
const EVIDENCE_OUTCOME_COUNT_OFFSET: usize = 472;
const EVIDENCE_POSITION_RENT_OFFSET: usize = 480;
const EVIDENCE_ADMISSION_RENT_OFFSET: usize = 488;
const EVIDENCE_MARKET_REVISION_BEFORE_OFFSET: usize = 496;
const EVIDENCE_MARKET_REVISION_AFTER_OFFSET: usize = 504;

const AUTHORITY_ACCOUNT: usize = 0;
const MARKET_ACCOUNT: usize = 1;
const POSITION_ACCOUNT: usize = 2;
const ADMISSION_ACCOUNT: usize = 3;
const BASIS_RECORD_ACCOUNT: usize = 4;
const BASIS_STAGING_ACCOUNT: usize = 5;
const PRODUCT_RECORD_ACCOUNT: usize = 6;
const PRODUCT_STAGING_ACCOUNT: usize = 7;
const RENT_ACCOUNT: usize = 8;
const SYSTEM_ACCOUNT: usize = 9;
const CORE_MARKET_ACCOUNT: usize = 10;
const CACHE_ACCOUNT: usize = 11;
const REGISTRY_ACCOUNT: usize = 12;
const TRADING_PROGRAM_ACCOUNT: usize = 13;
const TRADING_PROGRAMDATA_ACCOUNT: usize = 14;
const CLAIMS_PROGRAM_ACCOUNT: usize = 15;
const CLAIMS_PROGRAMDATA_ACCOUNT: usize = 16;
const CORE_PROGRAM_ACCOUNT: usize = 17;
const CORE_PROGRAMDATA_ACCOUNT: usize = 18;
const CHILD_ROOT_ACCOUNT: usize = 19;

/// Stable protocol Position admission refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ProtocolPositionSbfErrorV2 {
    /// Request bytes were not the sole canonical V2 request.
    Instruction = 140,
    /// Account count, order, privilege, owner, or alias checks refused.
    Accounts = 141,
    /// Registry-selected current deployments or caller authority refused.
    Release = 142,
    /// Claims aggregate, Core Market, or immutable request join refused.
    Market = 143,
    /// Product, linked basis, width, or finalized-record evidence refused.
    ProductBasis = 144,
    /// Position/admission PDA vacancy or prepaid-rent facts refused.
    Vacancy = 145,
    /// System allocation/assignment refused.
    Allocation = 146,
    /// Candidate state could not be committed atomically.
    Commit = 147,
}

impl From<ProtocolPositionSbfErrorV2> for ProgramError {
    fn from(value: ProtocolPositionSbfErrorV2) -> Self {
        Self::Custom(value as u32)
    }
}

/// Exact immutable protocol Position admission request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolPositionRequestV2 {
    /// Immutable selected execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market identity.
    pub market: [u8; 32],
    /// Canonical Trading-owned child root that will own inventory.
    pub child_root: [u8; 32],
    /// Exact parent activation request digest.
    pub parent_request_digest: [u8; 32],
    /// Immutable beneficiary of both prepaid-rent principals.
    pub rent_refund: [u8; 32],
    /// Exact Market generation.
    pub generation: u64,
    /// Claims aggregate revision that must remain unchanged.
    pub expected_market_revision: u64,
    /// Exact prepaid Position lamports.
    pub position_rent_lamports: u64,
    /// Exact prepaid admission-record lamports.
    pub admission_rent_lamports: u64,
}

impl ProtocolPositionRequestV2 {
    /// Decode one exact canonical request.
    pub fn decode(input: &[u8]) -> Result<Self, ProtocolPositionSbfErrorV2> {
        if input.len() != PROTOCOL_POSITION_REQUEST_BYTES_V2
            || read_array::<8>(input, 0)? != PROTOCOL_POSITION_REQUEST_MAGIC_V2
            || read_u16(input, 8)? != ABI_VERSION_V2
        {
            return Err(ProtocolPositionSbfErrorV2::Instruction);
        }
        require_zero(input, 10, 6)?;
        require_zero(input, 208, 16)?;
        let value = Self {
            release_set: read_array(input, REQUEST_RELEASE_OFFSET)?,
            market: read_array(input, REQUEST_MARKET_OFFSET)?,
            child_root: read_array(input, REQUEST_CHILD_ROOT_OFFSET)?,
            parent_request_digest: read_array(input, REQUEST_PARENT_DIGEST_OFFSET)?,
            rent_refund: read_array(input, REQUEST_REFUND_OFFSET)?,
            generation: read_u64(input, REQUEST_GENERATION_OFFSET)?,
            expected_market_revision: read_u64(input, REQUEST_MARKET_REVISION_OFFSET)?,
            position_rent_lamports: read_u64(input, REQUEST_POSITION_RENT_OFFSET)?,
            admission_rent_lamports: read_u64(input, REQUEST_ADMISSION_RENT_OFFSET)?,
        };
        if [
            value.release_set,
            value.market,
            value.child_root,
            value.parent_request_digest,
            value.rent_refund,
        ]
        .iter()
        .any(is_zero)
            || value.child_root == value.rent_refund
            || value.position_rent_lamports == 0
            || value.admission_rent_lamports == 0
        {
            return Err(ProtocolPositionSbfErrorV2::Instruction);
        }
        Ok(value)
    }

    /// Encode one exact canonical request.
    pub fn to_bytes(
        self,
    ) -> Result<[u8; PROTOCOL_POSITION_REQUEST_BYTES_V2], ProtocolPositionSbfErrorV2> {
        let mut output = [0_u8; PROTOCOL_POSITION_REQUEST_BYTES_V2];
        put(&mut output, 0, &PROTOCOL_POSITION_REQUEST_MAGIC_V2)?;
        put(&mut output, 8, &ABI_VERSION_V2.to_le_bytes())?;
        for (offset, value) in [
            (REQUEST_RELEASE_OFFSET, self.release_set),
            (REQUEST_MARKET_OFFSET, self.market),
            (REQUEST_CHILD_ROOT_OFFSET, self.child_root),
            (REQUEST_PARENT_DIGEST_OFFSET, self.parent_request_digest),
            (REQUEST_REFUND_OFFSET, self.rent_refund),
        ] {
            put(&mut output, offset, &value)?;
        }
        for (offset, value) in [
            (REQUEST_GENERATION_OFFSET, self.generation),
            (
                REQUEST_MARKET_REVISION_OFFSET,
                self.expected_market_revision,
            ),
            (REQUEST_POSITION_RENT_OFFSET, self.position_rent_lamports),
            (REQUEST_ADMISSION_RENT_OFFSET, self.admission_rent_lamports),
        ] {
            put(&mut output, offset, &value.to_le_bytes())?;
        }
        Self::decode(&output)?;
        Ok(output)
    }
}

/// Exact evidence returned after a protocol Position admission commits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolPositionAdmissionReceiptV2 {
    /// Immutable selected execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market identity.
    pub market: [u8; 32],
    /// Trading-owned protocol inventory owner.
    pub child_root: [u8; 32],
    /// Canonical Claims Position PDA.
    pub position: [u8; 32],
    /// Canonical Claims admission-record PDA.
    pub admission: [u8; 32],
    /// Authenticated Product Instance digest.
    pub product_instance_id: [u8; 32],
    /// Product-owned semantic liability-basis ID.
    pub semantic_basis_id: [u8; 32],
    /// Independently authenticated full linked-record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Exact parent activation request digest.
    pub parent_request_digest: [u8; 32],
    /// Digest of the exact Claims admission request.
    pub request_digest: [u8; 32],
    /// Immutable prepaid-rent refund beneficiary.
    pub rent_refund: [u8; 32],
    /// Registry-selected current Claims program.
    pub claims_program: [u8; 32],
    /// Registry-selected current Trading program.
    pub trading_program: [u8; 32],
    /// Digest of the exact initialized zero Position bytes.
    pub position_state_digest: [u8; 32],
    /// Exact Market generation.
    pub generation: u64,
    /// Runtime admitted outcome width.
    pub outcome_count: u32,
    /// Exact prepaid Position lamports.
    pub position_rent_lamports: u64,
    /// Exact prepaid admission-record lamports.
    pub admission_rent_lamports: u64,
    /// Claims aggregate revision observed before admission.
    pub market_revision_before: u64,
    /// Claims aggregate revision rechecked after admission.
    pub market_revision_after: u64,
}

impl ProtocolPositionAdmissionReceiptV2 {
    /// Decode the Claims-owned persisted admission record.
    pub fn decode_admission(input: &[u8]) -> Result<Self, ProtocolPositionSbfErrorV2> {
        decode_evidence(input, ADMISSION_MAGIC_V2)
    }

    /// Decode one exact admission receipt.
    pub fn decode(input: &[u8]) -> Result<Self, ProtocolPositionSbfErrorV2> {
        decode_evidence(input, RECEIPT_MAGIC_V2)
    }

    fn encode_with_magic_into(
        &self,
        magic: [u8; 8],
        output: &mut [u8],
    ) -> Result<(), ProtocolPositionSbfErrorV2> {
        if output.len() != PROTOCOL_POSITION_RECEIPT_BYTES_V2 {
            return Err(ProtocolPositionSbfErrorV2::Instruction);
        }
        output.fill(0);
        put(output, 0, &magic)?;
        put(output, 8, &ABI_VERSION_V2.to_le_bytes())?;
        for (offset, value) in [
            (EVIDENCE_RELEASE_OFFSET, self.release_set),
            (EVIDENCE_MARKET_OFFSET, self.market),
            (EVIDENCE_CHILD_ROOT_OFFSET, self.child_root),
            (EVIDENCE_POSITION_OFFSET, self.position),
            (EVIDENCE_ADMISSION_OFFSET, self.admission),
            (EVIDENCE_PRODUCT_OFFSET, self.product_instance_id),
            (EVIDENCE_BASIS_OFFSET, self.semantic_basis_id),
            (
                EVIDENCE_LINKED_DIGEST_OFFSET,
                self.linked_basis_record_digest,
            ),
            (EVIDENCE_PARENT_DIGEST_OFFSET, self.parent_request_digest),
            (EVIDENCE_REQUEST_DIGEST_OFFSET, self.request_digest),
            (EVIDENCE_REFUND_OFFSET, self.rent_refund),
            (EVIDENCE_CLAIMS_PROGRAM_OFFSET, self.claims_program),
            (EVIDENCE_TRADING_PROGRAM_OFFSET, self.trading_program),
            (EVIDENCE_POSITION_DIGEST_OFFSET, self.position_state_digest),
        ] {
            put(output, offset, &value)?;
        }
        put(
            output,
            EVIDENCE_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        )?;
        put(
            output,
            EVIDENCE_OUTCOME_COUNT_OFFSET,
            &self.outcome_count.to_le_bytes(),
        )?;
        for (offset, value) in [
            (EVIDENCE_POSITION_RENT_OFFSET, self.position_rent_lamports),
            (EVIDENCE_ADMISSION_RENT_OFFSET, self.admission_rent_lamports),
            (
                EVIDENCE_MARKET_REVISION_BEFORE_OFFSET,
                self.market_revision_before,
            ),
            (
                EVIDENCE_MARKET_REVISION_AFTER_OFFSET,
                self.market_revision_after,
            ),
        ] {
            put(output, offset, &value.to_le_bytes())?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Accounts<'accounts, 'info> {
    authority: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    position: &'accounts AccountInfo<'info>,
    admission: &'accounts AccountInfo<'info>,
    basis_record: &'accounts AccountInfo<'info>,
    basis_staging: &'accounts AccountInfo<'info>,
    product_record: &'accounts AccountInfo<'info>,
    product_staging: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    system: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    child_root: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> Accounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        Ok(Self {
            authority: account(accounts, AUTHORITY_ACCOUNT)?,
            market: account(accounts, MARKET_ACCOUNT)?,
            position: account(accounts, POSITION_ACCOUNT)?,
            admission: account(accounts, ADMISSION_ACCOUNT)?,
            basis_record: account(accounts, BASIS_RECORD_ACCOUNT)?,
            basis_staging: account(accounts, BASIS_STAGING_ACCOUNT)?,
            product_record: account(accounts, PRODUCT_RECORD_ACCOUNT)?,
            product_staging: account(accounts, PRODUCT_STAGING_ACCOUNT)?,
            rent: account(accounts, RENT_ACCOUNT)?,
            system: account(accounts, SYSTEM_ACCOUNT)?,
            core_market: account(accounts, CORE_MARKET_ACCOUNT)?,
            cache: account(accounts, CACHE_ACCOUNT)?,
            registry: account(accounts, REGISTRY_ACCOUNT)?,
            trading_program: account(accounts, TRADING_PROGRAM_ACCOUNT)?,
            trading_programdata: account(accounts, TRADING_PROGRAMDATA_ACCOUNT)?,
            claims_program: account(accounts, CLAIMS_PROGRAM_ACCOUNT)?,
            claims_programdata: account(accounts, CLAIMS_PROGRAMDATA_ACCOUNT)?,
            core_program: account(accounts, CORE_PROGRAM_ACCOUNT)?,
            core_programdata: account(accounts, CORE_PROGRAMDATA_ACCOUNT)?,
            child_root: account(accounts, CHILD_ROOT_ACCOUNT)?,
        })
    }
}

struct Admission {
    position_width: usize,
    evidence: Box<ProtocolPositionAdmissionReceiptV2>,
    position_candidate: Vec<u8>,
    admission_candidate: Vec<u8>,
}

/// Execute one canonical protocol Position admission.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if account_infos.len() != PROTOCOL_POSITION_ACCOUNT_COUNT_V2 {
        return Err(ProtocolPositionSbfErrorV2::Accounts.into());
    }
    let request = ProtocolPositionRequestV2::decode(instruction_data)?;
    let request_digest = hash(instruction_data).to_bytes();
    let accounts = Accounts::parse(account_infos)?;
    authenticate_privileges(program_id, &accounts, request)?;
    authenticate_releases(program_id, &accounts, request, request_digest)?;
    let mut admission = prepare_admission(program_id, &accounts, request, request_digest)?;
    allocate_accounts(program_id, &accounts, request, &admission)?;
    let market_revision_after = read_market_revision(accounts.market)?;
    if market_revision_after != request.expected_market_revision {
        return Err(ProtocolPositionSbfErrorV2::Commit.into());
    }
    admission.evidence.market_revision_after = market_revision_after;
    let mut receipt_bytes = alloc::vec![0_u8; PROTOCOL_POSITION_RECEIPT_BYTES_V2];
    admission
        .evidence
        .encode_with_magic_into(RECEIPT_MAGIC_V2, &mut receipt_bytes)?;
    commit_candidates(&accounts, &admission)?;
    set_return_data(&receipt_bytes);
    Ok(())
}

fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: &Accounts<'_, '_>,
    request: ProtocolPositionRequestV2,
) -> Result<(), ProgramError> {
    if !accounts.authority.is_signer
        || accounts.authority.is_writable
        || accounts.authority.executable
        || accounts.market.is_signer
        || accounts.market.is_writable
        || accounts.market.executable
        || accounts.position.is_signer
        || !accounts.position.is_writable
        || accounts.position.executable
        || accounts.admission.is_signer
        || !accounts.admission.is_writable
        || accounts.admission.executable
        || accounts.position.key == accounts.admission.key
        || accounts.rent.key != &sysvar::rent::ID
        || accounts.rent.is_signer
        || accounts.rent.is_writable
        || accounts.rent.executable
        || accounts.system.key != &system_program::ID
        || accounts.system.is_signer
        || accounts.system.is_writable
        || !accounts.system.executable
        || accounts.claims_program.key != program_id
        || !accounts.claims_program.executable
        || !accounts.trading_program.executable
        || !accounts.registry.executable
        || !accounts.core_program.executable
        || accounts.child_root.key.to_bytes() != request.child_root
        || accounts.child_root.owner != accounts.trading_program.key
        || accounts.child_root.is_signer
        || accounts.child_root.is_writable
        || accounts.child_root.executable
    {
        return Err(ProtocolPositionSbfErrorV2::Accounts.into());
    }
    for account in [
        accounts.basis_record,
        accounts.basis_staging,
        accounts.product_record,
        accounts.product_staging,
        accounts.core_market,
        accounts.cache,
        accounts.registry,
        accounts.trading_program,
        accounts.trading_programdata,
        accounts.claims_program,
        accounts.claims_programdata,
        accounts.core_program,
        accounts.core_programdata,
    ] {
        if account.is_signer || account.is_writable {
            return Err(ProtocolPositionSbfErrorV2::Accounts.into());
        }
    }
    Ok(())
}

fn authenticate_releases(
    program_id: &Pubkey,
    accounts: &Accounts<'_, '_>,
    request: ProtocolPositionRequestV2,
    request_digest: [u8; 32],
) -> Result<(), ProgramError> {
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Trading,
            accounts.trading_program,
            accounts.trading_programdata,
        ),
        (
            ExecutionRoleV1::Claims,
            accounts.claims_program,
            accounts.claims_programdata,
        ),
        (
            ExecutionRoleV1::Core,
            accounts.core_program,
            accounts.core_programdata,
        ),
    ] {
        let receipt = reauthenticate(
            accounts.registry,
            accounts.cache,
            role,
            program,
            programdata,
        )
        .map_err(|_| ProtocolPositionSbfErrorV2::Release)?;
        if receipt.execution_release_set_id().as_bytes() != &request.release_set {
            return Err(ProtocolPositionSbfErrorV2::Release.into());
        }
    }
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set).map_err(|_| ProtocolPositionSbfErrorV2::Release)?,
        request.market,
        ExecutionRoleV1::Trading,
        request.child_root,
        request_digest,
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::Release)?;
    if accounts.authority.key
        != &Pubkey::find_program_address(&seeds.as_slices(), accounts.trading_program.key).0
        || accounts.claims_program.key != program_id
    {
        return Err(ProtocolPositionSbfErrorV2::Release.into());
    }
    Ok(())
}

fn prepare_admission(
    program_id: &Pubkey,
    accounts: &Accounts<'_, '_>,
    request: ProtocolPositionRequestV2,
    request_digest: [u8; 32],
) -> Result<Admission, ProgramError> {
    let market_data = accounts
        .market
        .try_borrow_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?;
    let outcome_count =
        market_outcome_count(&market_data).map_err(|_| ProtocolPositionSbfErrorV2::Market)?;
    let revision = market_revision(&market_data).map_err(|_| ProtocolPositionSbfErrorV2::Market)?;
    if accounts.market.owner != program_id
        || market_identity(&market_data).map_err(|_| ProtocolPositionSbfErrorV2::Market)?
            != request.market
        || market_release_set_id(&market_data).map_err(|_| ProtocolPositionSbfErrorV2::Market)?
            != request.release_set
        || market_registry_program(&market_data).map_err(|_| ProtocolPositionSbfErrorV2::Market)?
            != accounts.registry.key.to_bytes()
        || market_phase(&market_data).map_err(|_| ProtocolPositionSbfErrorV2::Market)?
            != EconomicPhase::Open
        || revision != request.expected_market_revision
    {
        return Err(ProtocolPositionSbfErrorV2::Market.into());
    }
    let aggregate_seeds = ClaimsAggregateSeedsV1::new(request.market)
        .map_err(|_| ProtocolPositionSbfErrorV2::Market)?;
    if accounts.market.key
        != &Pubkey::find_program_address(&aggregate_seeds.as_slices(), program_id).0
    {
        return Err(ProtocolPositionSbfErrorV2::Market.into());
    }
    drop(market_data);

    if accounts.core_market.owner != accounts.core_program.key
        || accounts.core_market.key.to_bytes() != request.market
        || accounts.core_market.data_len() != STATE_BYTES
    {
        return Err(ProtocolPositionSbfErrorV2::Market.into());
    }
    let core_data = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?;
    let core = CoreState::decode(&core_data).map_err(|_| ProtocolPositionSbfErrorV2::Market)?;
    drop(core_data);
    if core.phase != CorePhase::Open
        || core.identity.market_id.to_bytes() != request.market
        || core.identity.selected_release_set.to_bytes() != request.release_set
        || core.identity.registry_program.to_bytes() != accounts.registry.key.to_bytes()
        || core.identity.generation != request.generation
    {
        return Err(ProtocolPositionSbfErrorV2::Market.into());
    }

    let linked_digest = authenticate_self_finalized_record(
        accounts,
        accounts.basis_record,
        accounts.basis_staging,
        LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2,
    )?;
    let basis_data = accounts
        .basis_record
        .try_borrow_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?;
    let linked = LinkedBasisRecordV2::decode(&basis_data)
        .map_err(|_| ProtocolPositionSbfErrorV2::ProductBasis)?;
    let embedded = linked.basis_record();
    let semantic_id = hashv(&[
        BASIS_SEMANTIC_ID_DOMAIN_V2,
        embedded
            .get(..32)
            .ok_or(ProtocolPositionSbfErrorV2::ProductBasis)?,
        embedded
            .get(64..)
            .ok_or(ProtocolPositionSbfErrorV2::ProductBasis)?,
    ])
    .to_bytes();
    if linked.product_instance_id().to_bytes() != core.identity.product_id.to_bytes()
        || linked.semantic_basis_id().to_bytes() != semantic_id
    {
        return Err(ProtocolPositionSbfErrorV2::ProductBasis.into());
    }
    authenticate_finalized_record(
        accounts,
        accounts.product_record,
        accounts.product_staging,
        PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        core.identity.product_id.to_bytes(),
    )?;
    let product_data = accounts
        .product_record
        .try_borrow_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?;
    let product =
        InstanceV1::decode(&product_data).map_err(|_| ProtocolPositionSbfErrorV2::ProductBasis)?;
    if product.claim_basis_id().to_bytes() != semantic_id
        || product.partition_cell_count() != outcome_count
    {
        return Err(ProtocolPositionSbfErrorV2::ProductBasis.into());
    }
    let semantic =
        ContentIdV2::new(semantic_id).map_err(|_| ProtocolPositionSbfErrorV2::ProductBasis)?;
    let product_id = ContentIdV2::new(core.identity.product_id.to_bytes())
        .map_err(|_| ProtocolPositionSbfErrorV2::ProductBasis)?;
    let admitted = AdmittedBasisV2::admit(embedded, semantic, semantic, product_id)
        .map_err(|_| ProtocolPositionSbfErrorV2::ProductBasis)?;
    if admitted.claim_count() != outcome_count {
        return Err(ProtocolPositionSbfErrorV2::ProductBasis.into());
    }
    drop(product_data);
    drop(basis_data);

    let position_seeds = ClaimsPositionSeedsV1::new(request.market, request.child_root)
        .map_err(|_| ProtocolPositionSbfErrorV2::Vacancy)?;
    let expected_position = Pubkey::find_program_address(&position_seeds.as_slices(), program_id).0;
    let admission_seeds = [
        PROTOCOL_POSITION_ADMISSION_SEED_V2,
        request.market.as_slice(),
        request.child_root.as_slice(),
    ];
    let expected_admission = Pubkey::find_program_address(&admission_seeds, program_id).0;
    let position_width = position_width(outcome_count)?;
    let rent =
        Rent::from_account_info(accounts.rent).map_err(|_| ProtocolPositionSbfErrorV2::Vacancy)?;
    if accounts.position.key != &expected_position
        || accounts.admission.key != &expected_admission
        || request.position_rent_lamports != rent.minimum_balance(position_width)
        || request.admission_rent_lamports
            != rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
    {
        return Err(ProtocolPositionSbfErrorV2::Vacancy.into());
    }
    authenticate_vacant(accounts.position, request.position_rent_lamports)?;
    authenticate_vacant(accounts.admission, request.admission_rent_lamports)?;

    let mut position_candidate = alloc::vec![0_u8; position_width];
    initialize_position(
        &mut position_candidate,
        request.market,
        request.child_root,
        outcome_count,
    )
    .map_err(|_| ProtocolPositionSbfErrorV2::ProductBasis)?;
    let position_state_digest = hash(&position_candidate).to_bytes();
    let evidence = Box::new(ProtocolPositionAdmissionReceiptV2 {
        release_set: request.release_set,
        market: request.market,
        child_root: request.child_root,
        position: expected_position.to_bytes(),
        admission: expected_admission.to_bytes(),
        product_instance_id: core.identity.product_id.to_bytes(),
        semantic_basis_id: semantic_id,
        linked_basis_record_digest: linked_digest,
        parent_request_digest: request.parent_request_digest,
        request_digest,
        rent_refund: request.rent_refund,
        claims_program: program_id.to_bytes(),
        trading_program: accounts.trading_program.key.to_bytes(),
        position_state_digest,
        generation: request.generation,
        outcome_count,
        position_rent_lamports: request.position_rent_lamports,
        admission_rent_lamports: request.admission_rent_lamports,
        market_revision_before: revision,
        market_revision_after: revision,
    });
    let mut admission_candidate = alloc::vec![0_u8; PROTOCOL_POSITION_ADMISSION_BYTES_V2];
    evidence.encode_with_magic_into(ADMISSION_MAGIC_V2, &mut admission_candidate)?;
    Ok(Admission {
        position_width,
        evidence,
        position_candidate,
        admission_candidate,
    })
}

fn allocate_accounts<'info>(
    program_id: &Pubkey,
    accounts: &Accounts<'_, 'info>,
    request: ProtocolPositionRequestV2,
    admission: &Admission,
) -> Result<(), ProgramError> {
    let position_seeds = ClaimsPositionSeedsV1::new(request.market, request.child_root)
        .map_err(|_| ProtocolPositionSbfErrorV2::Allocation)?;
    let [position_domain, market, owner] = position_seeds.as_slices();
    let position_bump = [Pubkey::find_program_address(&position_seeds.as_slices(), program_id).1];
    allocate_and_assign(
        program_id,
        accounts.position,
        accounts.system,
        admission.position_width,
        &[position_domain, market, owner, &position_bump],
    )?;
    let admission_seeds = [
        PROTOCOL_POSITION_ADMISSION_SEED_V2,
        request.market.as_slice(),
        request.child_root.as_slice(),
    ];
    let admission_bump = [Pubkey::find_program_address(&admission_seeds, program_id).1];
    allocate_and_assign(
        program_id,
        accounts.admission,
        accounts.system,
        PROTOCOL_POSITION_ADMISSION_BYTES_V2,
        &[
            PROTOCOL_POSITION_ADMISSION_SEED_V2,
            request.market.as_slice(),
            request.child_root.as_slice(),
            &admission_bump,
        ],
    )
}

fn allocate_and_assign<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    width: usize,
    seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    let space = u64::try_from(width).map_err(|_| ProtocolPositionSbfErrorV2::Allocation)?;
    for instruction in [
        allocate(account.key, space),
        assign(account.key, program_id),
    ] {
        invoke_signed(
            &Instruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts,
                data: instruction.data,
            },
            &[account.clone(), system.clone()],
            &[seeds],
        )
        .map_err(|_| ProtocolPositionSbfErrorV2::Allocation)?;
    }
    if account.owner != program_id || account.data_len() != width {
        return Err(ProtocolPositionSbfErrorV2::Allocation.into());
    }
    Ok(())
}

fn commit_candidates(
    accounts: &Accounts<'_, '_>,
    admission: &Admission,
) -> Result<(), ProgramError> {
    let mut position = accounts
        .position
        .try_borrow_mut_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Commit)?;
    let mut record = accounts
        .admission
        .try_borrow_mut_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Commit)?;
    if position.len() != admission.position_width
        || record.len() != PROTOCOL_POSITION_ADMISSION_BYTES_V2
        || position.iter().any(|byte| *byte != 0)
        || record.iter().any(|byte| *byte != 0)
    {
        return Err(ProtocolPositionSbfErrorV2::Commit.into());
    }
    position.copy_from_slice(&admission.position_candidate);
    record.copy_from_slice(&admission.admission_candidate);
    Ok(())
}

fn authenticate_self_finalized_record(
    accounts: &Accounts<'_, '_>,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
) -> Result<[u8; 32], ProgramError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?;
    let digest = hash(&data).to_bytes();
    drop(data);
    authenticate_finalized_record(accounts, raw, staging, schema, digest)?;
    Ok(digest)
}

fn authenticate_finalized_record(
    accounts: &Accounts<'_, '_>,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<(), ProgramError> {
    if raw.owner != accounts.core_program.key
        || raw.executable
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
        || staging.executable
        || hash(
            &raw.try_borrow_data()
                .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?,
        )
        .to_bytes()
            != digest
    {
        return Err(ProtocolPositionSbfErrorV2::ProductBasis.into());
    }
    let raw_seeds = [RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()];
    let staging_seeds = [
        STAGING_CURSOR_PDA_SEED_V1,
        schema.as_slice(),
        digest.as_slice(),
    ];
    if raw.key != &Pubkey::find_program_address(&raw_seeds, accounts.core_program.key).0
        || staging.key != &Pubkey::find_program_address(&staging_seeds, accounts.core_program.key).0
    {
        return Err(ProtocolPositionSbfErrorV2::ProductBasis.into());
    }
    let rent = Rent::from_account_info(accounts.rent)
        .map_err(|_| ProtocolPositionSbfErrorV2::ProductBasis)?;
    if raw.lamports() < rent.minimum_balance(raw.data_len()) {
        return Err(ProtocolPositionSbfErrorV2::ProductBasis.into());
    }
    Ok(())
}

fn authenticate_vacant(account: &AccountInfo<'_>, lamports: u64) -> Result<(), ProgramError> {
    if account.owner != &system_program::ID
        || account.data_len() != 0
        || account.lamports() != lamports
        || account.is_signer
        || !account.is_writable
        || account.executable
    {
        return Err(ProtocolPositionSbfErrorV2::Vacancy.into());
    }
    Ok(())
}

fn position_width(outcome_count: u32) -> Result<usize, ProgramError> {
    usize::try_from(outcome_count)
        .ok()
        .and_then(|count| count.checked_mul(2))
        .and_then(|count| count.checked_mul(SCALAR_BYTES))
        .and_then(|tail| POSITION_HEADER_BYTES.checked_add(tail))
        .ok_or_else(|| ProtocolPositionSbfErrorV2::ProductBasis.into())
}

fn read_market_revision(account: &AccountInfo<'_>) -> Result<u64, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| ProtocolPositionSbfErrorV2::Accounts)?;
    market_revision(&data).map_err(|_| ProtocolPositionSbfErrorV2::Market.into())
}

fn decode_evidence(
    input: &[u8],
    magic: [u8; 8],
) -> Result<ProtocolPositionAdmissionReceiptV2, ProtocolPositionSbfErrorV2> {
    if input.len() != PROTOCOL_POSITION_RECEIPT_BYTES_V2
        || read_array::<8>(input, 0)? != magic
        || read_u16(input, 8)? != ABI_VERSION_V2
    {
        return Err(ProtocolPositionSbfErrorV2::Instruction);
    }
    require_zero(input, 10, 6)?;
    require_zero(input, 476, 4)?;
    let value = ProtocolPositionAdmissionReceiptV2 {
        release_set: read_array(input, EVIDENCE_RELEASE_OFFSET)?,
        market: read_array(input, EVIDENCE_MARKET_OFFSET)?,
        child_root: read_array(input, EVIDENCE_CHILD_ROOT_OFFSET)?,
        position: read_array(input, EVIDENCE_POSITION_OFFSET)?,
        admission: read_array(input, EVIDENCE_ADMISSION_OFFSET)?,
        product_instance_id: read_array(input, EVIDENCE_PRODUCT_OFFSET)?,
        semantic_basis_id: read_array(input, EVIDENCE_BASIS_OFFSET)?,
        linked_basis_record_digest: read_array(input, EVIDENCE_LINKED_DIGEST_OFFSET)?,
        parent_request_digest: read_array(input, EVIDENCE_PARENT_DIGEST_OFFSET)?,
        request_digest: read_array(input, EVIDENCE_REQUEST_DIGEST_OFFSET)?,
        rent_refund: read_array(input, EVIDENCE_REFUND_OFFSET)?,
        claims_program: read_array(input, EVIDENCE_CLAIMS_PROGRAM_OFFSET)?,
        trading_program: read_array(input, EVIDENCE_TRADING_PROGRAM_OFFSET)?,
        position_state_digest: read_array(input, EVIDENCE_POSITION_DIGEST_OFFSET)?,
        generation: read_u64(input, EVIDENCE_GENERATION_OFFSET)?,
        outcome_count: read_u32(input, EVIDENCE_OUTCOME_COUNT_OFFSET)?,
        position_rent_lamports: read_u64(input, EVIDENCE_POSITION_RENT_OFFSET)?,
        admission_rent_lamports: read_u64(input, EVIDENCE_ADMISSION_RENT_OFFSET)?,
        market_revision_before: read_u64(input, EVIDENCE_MARKET_REVISION_BEFORE_OFFSET)?,
        market_revision_after: read_u64(input, EVIDENCE_MARKET_REVISION_AFTER_OFFSET)?,
    };
    if [
        value.release_set,
        value.market,
        value.child_root,
        value.position,
        value.admission,
        value.product_instance_id,
        value.semantic_basis_id,
        value.linked_basis_record_digest,
        value.parent_request_digest,
        value.request_digest,
        value.rent_refund,
        value.claims_program,
        value.trading_program,
        value.position_state_digest,
    ]
    .iter()
    .any(is_zero)
        || value.outcome_count == 0
        || value.position_rent_lamports == 0
        || value.admission_rent_lamports == 0
        || value.market_revision_before != value.market_revision_after
    {
        return Err(ProtocolPositionSbfErrorV2::Instruction);
    }
    Ok(value)
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| ProtocolPositionSbfErrorV2::Accounts.into())
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, ProtocolPositionSbfErrorV2> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32, ProtocolPositionSbfErrorV2> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, ProtocolPositionSbfErrorV2> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_array<const N: usize>(
    input: &[u8],
    offset: usize,
) -> Result<[u8; N], ProtocolPositionSbfErrorV2> {
    let end = offset
        .checked_add(N)
        .ok_or(ProtocolPositionSbfErrorV2::Instruction)?;
    input
        .get(offset..end)
        .ok_or(ProtocolPositionSbfErrorV2::Instruction)?
        .try_into()
        .map_err(|_| ProtocolPositionSbfErrorV2::Instruction)
}

fn require_zero(
    input: &[u8],
    offset: usize,
    width: usize,
) -> Result<(), ProtocolPositionSbfErrorV2> {
    let end = offset
        .checked_add(width)
        .ok_or(ProtocolPositionSbfErrorV2::Instruction)?;
    if input
        .get(offset..end)
        .ok_or(ProtocolPositionSbfErrorV2::Instruction)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(ProtocolPositionSbfErrorV2::Instruction);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), ProtocolPositionSbfErrorV2> {
    let end = offset
        .checked_add(value.len())
        .ok_or(ProtocolPositionSbfErrorV2::Instruction)?;
    output
        .get_mut(offset..end)
        .ok_or(ProtocolPositionSbfErrorV2::Instruction)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ProtocolPositionRequestV2 {
        ProtocolPositionRequestV2 {
            release_set: [1; 32],
            market: [2; 32],
            child_root: [3; 32],
            parent_request_digest: [4; 32],
            rent_refund: [5; 32],
            generation: 6,
            expected_market_revision: 7,
            position_rent_lamports: 8,
            admission_rent_lamports: 9,
        }
    }

    fn receipt() -> ProtocolPositionAdmissionReceiptV2 {
        ProtocolPositionAdmissionReceiptV2 {
            release_set: [1; 32],
            market: [2; 32],
            child_root: [3; 32],
            position: [4; 32],
            admission: [5; 32],
            product_instance_id: [6; 32],
            semantic_basis_id: [7; 32],
            linked_basis_record_digest: [8; 32],
            parent_request_digest: [9; 32],
            request_digest: [10; 32],
            rent_refund: [11; 32],
            claims_program: [12; 32],
            trading_program: [13; 32],
            position_state_digest: [14; 32],
            generation: 15,
            outcome_count: 16,
            position_rent_lamports: 17,
            admission_rent_lamports: 18,
            market_revision_before: 19,
            market_revision_after: 19,
        }
    }

    #[test]
    fn request_and_receipt_are_exact_canonical_contracts() {
        let request = request();
        let bytes = request.to_bytes().expect("request");
        assert_eq!(ProtocolPositionRequestV2::decode(&bytes), Ok(request));
        let evidence = receipt();
        let mut bytes = alloc::vec![0_u8; PROTOCOL_POSITION_RECEIPT_BYTES_V2];
        evidence
            .encode_with_magic_into(RECEIPT_MAGIC_V2, &mut bytes)
            .expect("receipt");
        assert_eq!(
            ProtocolPositionAdmissionReceiptV2::decode(&bytes),
            Ok(evidence)
        );
    }

    #[test]
    fn reserved_alias_and_evidence_mutations_refuse() {
        let mut bytes = request().to_bytes().expect("request");
        *bytes.get_mut(10).expect("reserved") = 1;
        assert_eq!(
            ProtocolPositionRequestV2::decode(&bytes),
            Err(ProtocolPositionSbfErrorV2::Instruction)
        );

        let mut aliased = request();
        aliased.rent_refund = aliased.child_root;
        assert_eq!(
            aliased.to_bytes(),
            Err(ProtocolPositionSbfErrorV2::Instruction)
        );

        let mut evidence = receipt();
        evidence.market_revision_after += 1;
        let mut bytes = alloc::vec![0_u8; PROTOCOL_POSITION_RECEIPT_BYTES_V2];
        evidence
            .encode_with_magic_into(RECEIPT_MAGIC_V2, &mut bytes)
            .expect("structural bytes");
        assert_eq!(
            ProtocolPositionAdmissionReceiptV2::decode(&bytes),
            Err(ProtocolPositionSbfErrorV2::Instruction)
        );
    }
}
