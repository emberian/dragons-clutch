//! LiabilityBasisV2 Product-to-Claims-to-Custody SBF composition.
//!
//! The adapter authenticates finalized Product and basis raw records under the
//! Registry-selected Core program, derives complete runtime-width Claims
//! candidates in memory, executes one exact Registry-selected Custody request,
//! checks its immediate return data and token/replay postconditions, and only
//! then commits the Claims candidate bytes.

extern crate alloc;

use alloc::vec::Vec;
use core::convert::{TryFrom, TryInto};

use dclutch_claims_svm::{
    lbv2_terminal_v2::{
        LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2, Lbv2TerminalRedeemReceiptV2,
        Lbv2TerminalRedeemRequestInputV2, Lbv2TerminalRedeemRequestV2,
    },
    protocol_position_v2::{ProtocolPositionClaimsCapabilitySeedsV2, ProtocolPositionSeedsV2},
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_RECEIPT_BYTES_V1, CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1,
    CustodyAuthoritySeedsV1, CustodyReceiptV1, CustodyReplaySeedsV1, CustodyReplayV1,
    CustodyRequestV1, CustodyVaultSeedsV1, OperationV1,
};
use dclutch_liability_basis_v2_kernel::product_claims::{
    AdmittedBasisV2, BASIS_SEMANTIC_ID_DOMAIN_V2, BasisKindV2, ClaimsCandidateV2, ContentIdV2,
    LinkedBasisRecordV2, ProductClaimsErrorV2, TerminalResultV2, semantic_basis_preimage_v2,
};
use dclutch_market_core_codec::{CoreState, Phase as CorePhase, STATE_BYTES};
use dclutch_product_contract::product::{InstanceV1, PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_token_svm::TokenAccount;
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};

use super::reauthenticate;

/// LiabilityBasisV2 instruction magic.
pub const LIABILITY_BASIS_ACTION_MAGIC_V2: [u8; 8] = *b"DCLLBX02";
/// Exact fixed LiabilityBasisV2 action width before an optional Custody request.
pub const LIABILITY_BASIS_ACTION_BYTES_V2: usize = 80;
/// LiabilityBasisV2 Claims aggregate fixed header width.
pub const LIABILITY_BASIS_MARKET_HEADER_BYTES_V2: usize = 256;
/// LiabilityBasisV2 Claims Position fixed header width.
pub const LIABILITY_BASIS_POSITION_HEADER_BYTES_V2: usize = 128;
/// Exact rational terminal-coordinate record width.
pub const TERMINAL_COORDINATE_BYTES_V2: usize = 32;
/// Canonical rational terminal-coordinate magic.
pub const TERMINAL_COORDINATE_MAGIC_V2: [u8; 8] = *b"DCLTRC02";
/// LiabilityBasisV2 aggregate PDA seed domain.
pub const LIABILITY_BASIS_MARKET_SEED_V2: &[u8] = b"dclutch:lbv2:market";
/// LiabilityBasisV2 schema-release identity used by finalized raw records.
pub const LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2: [u8; 32] = [
    0x5c, 0x84, 0x2a, 0xe9, 0xe9, 0x15, 0x51, 0xd1, 0xaf, 0x99, 0xcf, 0x99, 0xfd, 0x53, 0x7f, 0x64,
    0xfb, 0x8d, 0xbf, 0x6a, 0x4e, 0x88, 0x3f, 0x22, 0xd9, 0x0b, 0xd5, 0xf3, 0x24, 0x5f, 0x6e, 0x2e,
];
/// Rational terminal-coordinate schema identity used by finalized raw records.
pub const TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2: [u8; 32] = [
    0xa8, 0x66, 0x06, 0x2a, 0xe7, 0x6d, 0x3d, 0xc3, 0xa7, 0xc7, 0xce, 0xe5, 0x34, 0x0a, 0xc9, 0xe4,
    0x1f, 0x20, 0x22, 0x69, 0xcb, 0x23, 0xe9, 0xb7, 0x04, 0x61, 0xb0, 0x16, 0xf1, 0x8d, 0x5f, 0x61,
];

const ABI_VERSION_V2: u16 = 2;
const MARKET_MAGIC_V2: [u8; 8] = *b"DCLLBM02";
const POSITION_MAGIC_V2: [u8; 8] = *b"DCLLBP02";
const RECEIPT_MAGIC_V2: [u8; 8] = *b"DCLLBR02";
const RECEIPT_BYTES_V2: usize = 168;
/// Physical candidate commitment used by operator construction and typed
/// Custody composition: domain || exact post-aggregate || exact post-Position.
pub const LIABILITY_BASIS_CANDIDATE_DIGEST_DOMAIN_V2: [u8; 27] = *b"dclutch/lbv2/candidate/v2\0\0";
const POST_RESOURCE_DIGEST_DOMAIN_V2: &[u8] = b"dclutch/lbv2/post-resources/v2";

const OWNER_ACCOUNT: usize = 0;
const MARKET_ACCOUNT: usize = 1;
const POSITION_ACCOUNT: usize = 2;
const BASIS_RECORD_ACCOUNT: usize = 3;
const BASIS_STAGING_ACCOUNT: usize = 4;
const PRODUCT_RECORD_ACCOUNT: usize = 5;
const PRODUCT_STAGING_ACCOUNT: usize = 6;
const RENT_ACCOUNT: usize = 7;
const CORE_MARKET_ACCOUNT: usize = 8;
const TERMINAL_COORDINATE_ACCOUNT: usize = 9;
const TERMINAL_COORDINATE_STAGING_ACCOUNT: usize = 10;
const ACTIVATION_CACHE_ACCOUNT: usize = 11;
const REGISTRY_PROGRAM_ACCOUNT: usize = 12;
const CLAIMS_PROGRAM_ACCOUNT: usize = 13;
const CLAIMS_PROGRAMDATA_ACCOUNT: usize = 14;
const CUSTODY_PROGRAM_ACCOUNT: usize = 15;
const CUSTODY_PROGRAMDATA_ACCOUNT: usize = 16;
const CORE_PROGRAM_ACCOUNT: usize = 17;
const CORE_PROGRAMDATA_ACCOUNT: usize = 18;
const CUSTODY_CALLER_AUTHORITY_ACCOUNT: usize = 19;
const REALM_ACCOUNT: usize = 20;
const REALM_STAGING_ACCOUNT: usize = 21;
const CUSTODY_REPLAY_ACCOUNT: usize = 22;
const COLLATERAL_MINT_ACCOUNT: usize = 23;
const SOURCE_TOKEN_ACCOUNT: usize = 24;
const DESTINATION_TOKEN_ACCOUNT: usize = 25;
const CUSTODY_AUTHORITY_ACCOUNT: usize = 26;
const COLLATERAL_TOKEN_PROGRAM_ACCOUNT: usize = 27;
/// Exact LiabilityBasisV2 account count.
pub const LIABILITY_BASIS_ACCOUNT_COUNT_V2: usize = 28;

/// Exact facts authenticated by the enclosing RationalRepresentationV2 route.
///
/// This witness is crate-private: constructing it asserts that the parent has
/// already authenticated the descriptor, selected outcome, upstream packet,
/// and caller context. The LBV2 executor still reauthenticates every persisted
/// Product, basis, Core, Position, Custody, and release fact independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedLbv2TerminalParentV2 {
    pub(crate) release_set: [u8; 32],
    pub(crate) market: [u8; 32],
    pub(crate) descriptor_id: [u8; 32],
    pub(crate) outcome: u32,
    pub(crate) beneficiary_actor: [u8; 32],
    pub(crate) parent_context: [u8; 32],
    pub(crate) parent_request_digest: [u8; 32],
    pub(crate) custody_request_nonce: u64,
    pub(crate) expected_market_revision: u64,
    pub(crate) expected_position_revision: u64,
    pub(crate) expected_custody_revision: u64,
    pub(crate) debit_quantity: u64,
    pub(crate) admitted_basis: AdmittedBasisV2,
}

const ACTION_KIND_OFFSET: usize = 10;
const ACTION_CUSTODY_PRESENT_OFFSET: usize = 11;
const ACTION_MARKET_REVISION_OFFSET: usize = 16;
const ACTION_POSITION_REVISION_OFFSET: usize = 24;
const ACTION_QUANTITY_OFFSET: usize = 32;
const ACTION_CLAIM_INDEX_OFFSET: usize = 40;
const ACTION_CUSTODY_REVISION_OFFSET: usize = 48;
const ACTION_NONCE_OFFSET: usize = 56;

const MARKET_CLAIM_COUNT_OFFSET: usize = 12;
const MARKET_REVISION_OFFSET: usize = 16;
const MARKET_LOGICAL_ID_OFFSET: usize = 24;
const MARKET_RELEASE_SET_OFFSET: usize = 56;
const MARKET_REGISTRY_OFFSET: usize = 88;
const MARKET_PRODUCT_OFFSET: usize = 120;
const MARKET_BASIS_OFFSET: usize = 152;
const MARKET_REALM_OFFSET: usize = 184;
const MARKET_CUSTODY_CONTEXT_OFFSET: usize = 216;
const MARKET_GENERATION_OFFSET: usize = 248;

const POSITION_CLAIM_COUNT_OFFSET: usize = 12;
const POSITION_REVISION_OFFSET: usize = 16;
const POSITION_MARKET_OFFSET: usize = 24;
const POSITION_OWNER_OFFSET: usize = 56;
const POSITION_BASIS_OFFSET: usize = 88;

/// Stable LiabilityBasisV2 SBF refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum LiabilityBasisSbfErrorV2 {
    /// Instruction bytes were not the sole canonical V2 action.
    Instruction = 100,
    /// Account count, order, privilege, owner, or alias checks refused.
    Accounts = 101,
    /// Claims aggregate or Position bytes/PDA/revision refused.
    ClaimsState = 102,
    /// A finalized raw-record PDA, staging vacancy, rent, or digest refused.
    FinalizedRecord = 103,
    /// Product instance, basis identity, runtime width, or Core join refused.
    ProductLink = 104,
    /// Registry current-deployment authentication refused.
    Release = 105,
    /// The pure exact liability transition refused.
    Candidate = 106,
    /// Exact Custody request, authority, replay, or vault binding refused.
    CustodyRequest = 107,
    /// Custody CPI failed.
    CustodyCpi = 108,
    /// Custody return data or physical postconditions refused.
    Postcondition = 109,
    /// Complete candidate state could not be committed atomically.
    Commit = 110,
}

impl From<LiabilityBasisSbfErrorV2> for ProgramError {
    fn from(value: LiabilityBasisSbfErrorV2) -> Self {
        Self::Custom(value as u32)
    }
}

/// LiabilityBasisV2 Claims operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LiabilityBasisActionKindV2 {
    /// Deposit `qQ` collateral and mint every elementary claim.
    Split = 0,
    /// Burn every elementary claim and withdraw `qQ` collateral.
    Merge = 1,
    /// Burn one terminal claim and withdraw its evaluated payout.
    TerminalRedeem = 2,
}

impl LiabilityBasisActionKindV2 {
    fn decode(value: u8) -> Result<Self, LiabilityBasisSbfErrorV2> {
        match value {
            0 => Ok(Self::Split),
            1 => Ok(Self::Merge),
            2 => Ok(Self::TerminalRedeem),
            _ => Err(LiabilityBasisSbfErrorV2::Instruction),
        }
    }
}

/// Construction input for one exact LiabilityBasisV2 action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiabilityBasisActionInputV2 {
    /// Requested Claims operation.
    pub kind: LiabilityBasisActionKindV2,
    /// Whether exact Custody request bytes follow the fixed action.
    pub custody_present: bool,
    /// Optimistic aggregate revision.
    pub expected_market_revision: u64,
    /// Optimistic Position revision.
    pub expected_position_revision: u64,
    /// Positive complete-set or redemption quantity.
    pub quantity: u64,
    /// Terminal claim index; canonical zero for split and merge.
    pub claim_index: u32,
    /// Optimistic Custody replay revision; zero when no transfer exists.
    pub expected_custody_revision: u64,
    /// Caller-owned replay nonce bound into the Custody request.
    pub request_nonce: u64,
}

/// Hostile-decoded LiabilityBasisV2 action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiabilityBasisActionV2(LiabilityBasisActionInputV2);

impl LiabilityBasisActionV2 {
    /// Construct and validate one canonical action.
    pub fn new(input: LiabilityBasisActionInputV2) -> Result<Self, LiabilityBasisSbfErrorV2> {
        if input.quantity == 0
            || matches!(
                input.kind,
                LiabilityBasisActionKindV2::Split | LiabilityBasisActionKindV2::Merge
            ) && (!input.custody_present || input.claim_index != 0)
            || (!input.custody_present && input.expected_custody_revision != 0)
        {
            return Err(LiabilityBasisSbfErrorV2::Instruction);
        }
        input
            .expected_market_revision
            .checked_add(1)
            .ok_or(LiabilityBasisSbfErrorV2::Instruction)?;
        input
            .expected_position_revision
            .checked_add(1)
            .ok_or(LiabilityBasisSbfErrorV2::Instruction)?;
        if input.custody_present {
            input
                .expected_custody_revision
                .checked_add(1)
                .ok_or(LiabilityBasisSbfErrorV2::Instruction)?;
        }
        Ok(Self(input))
    }

    /// Decode one exact fixed action.
    pub fn decode(input: &[u8]) -> Result<Self, LiabilityBasisSbfErrorV2> {
        if input.len() != LIABILITY_BASIS_ACTION_BYTES_V2
            || read_array::<8>(input, 0)? != LIABILITY_BASIS_ACTION_MAGIC_V2
            || read_u16(input, 8)? != ABI_VERSION_V2
        {
            return Err(LiabilityBasisSbfErrorV2::Instruction);
        }
        require_zero(input, 12, 4, LiabilityBasisSbfErrorV2::Instruction)?;
        require_zero(input, 44, 4, LiabilityBasisSbfErrorV2::Instruction)?;
        require_zero(input, 64, 16, LiabilityBasisSbfErrorV2::Instruction)?;
        let custody_present = match read_byte(input, ACTION_CUSTODY_PRESENT_OFFSET)? {
            0 => false,
            1 => true,
            _ => return Err(LiabilityBasisSbfErrorV2::Instruction),
        };
        Self::new(LiabilityBasisActionInputV2 {
            kind: LiabilityBasisActionKindV2::decode(read_byte(input, ACTION_KIND_OFFSET)?)?,
            custody_present,
            expected_market_revision: read_u64(input, ACTION_MARKET_REVISION_OFFSET)?,
            expected_position_revision: read_u64(input, ACTION_POSITION_REVISION_OFFSET)?,
            quantity: read_u64(input, ACTION_QUANTITY_OFFSET)?,
            claim_index: read_u32(input, ACTION_CLAIM_INDEX_OFFSET)?,
            expected_custody_revision: read_u64(input, ACTION_CUSTODY_REVISION_OFFSET)?,
            request_nonce: read_u64(input, ACTION_NONCE_OFFSET)?,
        })
    }

    /// Encode the exact fixed action bytes.
    pub fn to_bytes(self) -> [u8; LIABILITY_BASIS_ACTION_BYTES_V2] {
        let mut output = [0_u8; LIABILITY_BASIS_ACTION_BYTES_V2];
        put_infallible(&mut output, 0, &LIABILITY_BASIS_ACTION_MAGIC_V2);
        put_infallible(&mut output, 8, &ABI_VERSION_V2.to_le_bytes());
        put_infallible(&mut output, ACTION_KIND_OFFSET, &[self.0.kind as u8]);
        put_infallible(
            &mut output,
            ACTION_CUSTODY_PRESENT_OFFSET,
            &[u8::from(self.0.custody_present)],
        );
        put_infallible(
            &mut output,
            ACTION_MARKET_REVISION_OFFSET,
            &self.0.expected_market_revision.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            ACTION_POSITION_REVISION_OFFSET,
            &self.0.expected_position_revision.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            ACTION_QUANTITY_OFFSET,
            &self.0.quantity.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            ACTION_CLAIM_INDEX_OFFSET,
            &self.0.claim_index.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            ACTION_CUSTODY_REVISION_OFFSET,
            &self.0.expected_custody_revision.to_le_bytes(),
        );
        put_infallible(
            &mut output,
            ACTION_NONCE_OFFSET,
            &self.0.request_nonce.to_le_bytes(),
        );
        output
    }
}

/// Immutable LiabilityBasisV2 aggregate construction input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiabilityBasisMarketInputV2 {
    /// Claims aggregate revision.
    pub revision: u64,
    /// Canonical Core Market PDA.
    pub logical_market: [u8; 32],
    /// Immutable selected release set.
    pub release_set: [u8; 32],
    /// Immutable selected Registry program.
    pub registry_program: [u8; 32],
    /// Finalized Product-instance digest.
    pub product_instance_id: [u8; 32],
    /// Finalized LiabilityBasisV2 digest.
    pub basis_id: [u8; 32],
    /// Immutable Realm digest.
    pub realm_id: [u8; 32],
    /// Custody replay namespace.
    pub custody_context: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
}

/// Immutable LiabilityBasisV2 Position construction input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiabilityBasisPositionInputV2 {
    /// Position revision.
    pub revision: u64,
    /// Claims aggregate account.
    pub market_account: [u8; 32],
    /// Sole Position owner.
    pub owner: [u8; 32],
    /// Finalized LiabilityBasisV2 digest.
    pub basis_id: [u8; 32],
}

/// Encode canonical runtime-width aggregate state for initialization tooling.
pub fn encode_liability_basis_market_v2(
    input: LiabilityBasisMarketInputV2,
    supplies: &[u64],
) -> Result<Vec<u8>, LiabilityBasisSbfErrorV2> {
    require_nonzero_ids(&[
        input.logical_market,
        input.release_set,
        input.registry_program,
        input.product_instance_id,
        input.basis_id,
        input.realm_id,
        input.custody_context,
    ])?;
    let claim_count =
        u32::try_from(supplies.len()).map_err(|_| LiabilityBasisSbfErrorV2::ClaimsState)?;
    if claim_count == 0 {
        return Err(LiabilityBasisSbfErrorV2::ClaimsState);
    }
    let width = vector_width(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, claim_count)?;
    let mut output = alloc::vec![0_u8; width];
    put(&mut output, 0, &MARKET_MAGIC_V2)?;
    put(&mut output, 8, &ABI_VERSION_V2.to_le_bytes())?;
    put(
        &mut output,
        MARKET_CLAIM_COUNT_OFFSET,
        &claim_count.to_le_bytes(),
    )?;
    put(
        &mut output,
        MARKET_REVISION_OFFSET,
        &input.revision.to_le_bytes(),
    )?;
    for (offset, value) in [
        (MARKET_LOGICAL_ID_OFFSET, input.logical_market),
        (MARKET_RELEASE_SET_OFFSET, input.release_set),
        (MARKET_REGISTRY_OFFSET, input.registry_program),
        (MARKET_PRODUCT_OFFSET, input.product_instance_id),
        (MARKET_BASIS_OFFSET, input.basis_id),
        (MARKET_REALM_OFFSET, input.realm_id),
        (MARKET_CUSTODY_CONTEXT_OFFSET, input.custody_context),
    ] {
        put(&mut output, offset, &value)?;
    }
    put(
        &mut output,
        MARKET_GENERATION_OFFSET,
        &input.generation.to_le_bytes(),
    )?;
    write_vector(
        &mut output,
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
        supplies,
    )?;
    Ok(output)
}

/// Encode canonical runtime-width Position state for initialization tooling.
pub fn encode_liability_basis_position_v2(
    input: LiabilityBasisPositionInputV2,
    balances: &[u64],
) -> Result<Vec<u8>, LiabilityBasisSbfErrorV2> {
    require_nonzero_ids(&[input.market_account, input.owner, input.basis_id])?;
    let claim_count =
        u32::try_from(balances.len()).map_err(|_| LiabilityBasisSbfErrorV2::ClaimsState)?;
    if claim_count == 0 {
        return Err(LiabilityBasisSbfErrorV2::ClaimsState);
    }
    let width = vector_width(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, claim_count)?;
    let mut output = alloc::vec![0_u8; width];
    put(&mut output, 0, &POSITION_MAGIC_V2)?;
    put(&mut output, 8, &ABI_VERSION_V2.to_le_bytes())?;
    put(
        &mut output,
        POSITION_CLAIM_COUNT_OFFSET,
        &claim_count.to_le_bytes(),
    )?;
    put(
        &mut output,
        POSITION_REVISION_OFFSET,
        &input.revision.to_le_bytes(),
    )?;
    put(&mut output, POSITION_MARKET_OFFSET, &input.market_account)?;
    put(&mut output, POSITION_OWNER_OFFSET, &input.owner)?;
    put(&mut output, POSITION_BASIS_OFFSET, &input.basis_id)?;
    write_vector(
        &mut output,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        balances,
    )?;
    Ok(output)
}

/// Encode one exact rational terminal-coordinate record.
pub fn encode_terminal_coordinate_v2(
    numerator: i64,
    denominator: u32,
) -> Result<[u8; TERMINAL_COORDINATE_BYTES_V2], LiabilityBasisSbfErrorV2> {
    if denominator == 0 {
        return Err(LiabilityBasisSbfErrorV2::Instruction);
    }
    let mut output = [0_u8; TERMINAL_COORDINATE_BYTES_V2];
    put_infallible(&mut output, 0, &TERMINAL_COORDINATE_MAGIC_V2);
    put_infallible(&mut output, 8, &ABI_VERSION_V2.to_le_bytes());
    put_infallible(&mut output, 16, &numerator.to_le_bytes());
    put_infallible(&mut output, 24, &denominator.to_le_bytes());
    Ok(output)
}

/// Execute one exact LiabilityBasisV2 Claims/Custody composition.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if account_infos.len() != LIABILITY_BASIS_ACCOUNT_COUNT_V2 {
        return Err(LiabilityBasisSbfErrorV2::Accounts.into());
    }
    let (action_bytes, custody_bytes) = split_instruction(instruction_data)?;
    let action = LiabilityBasisActionV2::decode(action_bytes)?;
    let accounts = LiabilityBasisAccountsV2::parse(account_infos)?;
    authenticate_privileges(program_id, &accounts, OwnerAuthenticationV2::PublicSigner)?;
    let executed = execute_authenticated_transition_v2(
        program_id,
        &accounts,
        action,
        custody_bytes,
        hash(action_bytes).to_bytes(),
        accounts.owner.key.to_bytes(),
        None,
    )?;
    let receipt = LiabilityBasisReceiptV2 {
        action: action.0.kind,
        market_revision_before: action.0.expected_market_revision,
        market_revision_after: executed.market_revision_after,
        position_revision_before: action.0.expected_position_revision,
        position_revision_after: executed.position_revision_after,
        collateral_amount: executed.collateral_amount,
        custody_revision_before: action.0.expected_custody_revision,
        custody_revision_after: if action.0.custody_present {
            action
                .0
                .expected_custody_revision
                .checked_add(1)
                .ok_or(LiabilityBasisSbfErrorV2::Postcondition)?
        } else {
            0
        },
        basis_id: executed.basis_id,
        candidate_digest: executed.candidate_digest,
        custody_receipt_digest: executed.custody_receipt_digest,
    };
    let receipt_bytes = receipt.to_bytes();
    set_return_data(&receipt_bytes);
    Ok(())
}

struct ExecutedTransitionV2 {
    market_revision_after: u64,
    position_revision_after: u64,
    collateral_amount: u64,
    basis_id: [u8; 32],
    candidate_digest: [u8; 32],
    custody_receipt_digest: [u8; 32],
    typed_receipt: Option<Box<Lbv2TerminalRedeemReceiptV2>>,
    typed_custody: Option<Box<AuthenticatedLbv2TerminalCustodyV2>>,
}

/// Exact Custody evidence observed by the sole LBV2 terminal writer.
///
/// This is returned only to an already-authenticated in-program composer. It
/// does not create a second Custody authority path: the LBV2 executor has
/// already constructed the request, authenticated the immediate producer and
/// receipt, and checked the post-CPI replay digest before returning it.
pub(crate) struct AuthenticatedLbv2TerminalCustodyV2 {
    pub(crate) request: Box<CustodyRequestV1>,
    pub(crate) request_digest: [u8; 32],
    pub(crate) receipt: Box<CustodyReceiptV1>,
    pub(crate) receipt_digest: [u8; 32],
    pub(crate) replay_digest: [u8; 32],
}

/// Typed terminal result returned to the enclosing RationalRepresentationV2
/// composer after the LBV2 state commit succeeds.
pub(crate) struct AuthenticatedLbv2TerminalResultV2 {
    pub(crate) receipt: Box<Lbv2TerminalRedeemReceiptV2>,
    pub(crate) custody: Option<Box<AuthenticatedLbv2TerminalCustodyV2>>,
}

/// Execute the sole LBV2 terminal candidate/Custody/commit-last writer for an
/// enclosing RationalRepresentationV2 route.
///
/// Account zero is the authenticated actor and Custody beneficiary. The
/// debited Position owner is not an AccountInfo: it is the canonical
/// ClaimsCapability PDA derived from `parent.descriptor_id` and
/// `parent.outcome`, and both the typed request and Position state must name it.
pub(crate) fn execute_parent_authenticated_terminal_v2(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    parent: Box<AuthenticatedLbv2TerminalParentV2>,
) -> Result<Box<AuthenticatedLbv2TerminalResultV2>, ProgramError> {
    if account_infos.len() != LIABILITY_BASIS_ACCOUNT_COUNT_V2
        || parent.beneficiary_actor == [0; 32]
        || parent.parent_context == [0; 32]
        || parent.parent_request_digest == [0; 32]
        || parent.debit_quantity == 0
    {
        return Err(LiabilityBasisSbfErrorV2::Instruction.into());
    }
    let owner_seeds =
        ProtocolPositionClaimsCapabilitySeedsV2::new(parent.descriptor_id, parent.outcome)
            .map_err(|_| LiabilityBasisSbfErrorV2::ClaimsState)?;
    let position_owner = Pubkey::find_program_address(&owner_seeds.as_slices(), program_id)
        .0
        .to_bytes();
    let action = LiabilityBasisActionV2::new(LiabilityBasisActionInputV2 {
        kind: LiabilityBasisActionKindV2::TerminalRedeem,
        custody_present: false,
        expected_market_revision: parent.expected_market_revision,
        expected_position_revision: parent.expected_position_revision,
        quantity: parent.debit_quantity,
        claim_index: parent.outcome,
        expected_custody_revision: 0,
        request_nonce: parent.custody_request_nonce,
    })?;
    let accounts = LiabilityBasisAccountsV2::parse(account_infos)?;
    authenticate_privileges(
        program_id,
        &accounts,
        OwnerAuthenticationV2::ParentActor {
            beneficiary_actor: parent.beneficiary_actor,
        },
    )?;
    let executed = execute_authenticated_transition_v2(
        program_id,
        &accounts,
        action,
        None,
        parent.parent_request_digest,
        position_owner,
        Some(parent.as_ref()),
    )?;
    let receipt = executed
        .typed_receipt
        .ok_or(LiabilityBasisSbfErrorV2::Postcondition)?;
    Ok(Box::new(AuthenticatedLbv2TerminalResultV2 {
        receipt,
        custody: executed.typed_custody,
    }))
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn execute_authenticated_transition_v2<'info>(
    program_id: &Pubkey,
    accounts: &LiabilityBasisAccountsV2<'_, 'info>,
    action: LiabilityBasisActionV2,
    custody_bytes: Option<&[u8]>,
    custody_parent_request_digest: [u8; 32],
    position_owner: [u8; 32],
    typed_terminal: Option<&AuthenticatedLbv2TerminalParentV2>,
) -> Result<ExecutedTransitionV2, ProgramError> {
    let planned = plan_authenticated_transition_v2(
        program_id,
        accounts,
        action,
        position_owner,
        typed_terminal,
    )?;
    complete_authenticated_transition_v2(
        program_id,
        accounts,
        planned.as_ref(),
        custody_bytes,
        custody_parent_request_digest,
        position_owner,
        typed_terminal,
    )
}

struct PlannedTransitionV2 {
    market: MarketViewV2,
    action: LiabilityBasisActionV2,
    terminal: Option<TerminalResultV2>,
    token_before: TokenAmounts,
    candidate: ClaimsCandidateV2,
    market_revision_after: u64,
    position_revision_after: u64,
    market_candidate: Vec<u8>,
    position_candidate: Vec<u8>,
    candidate_digest: [u8; 32],
}

#[inline(never)]
fn plan_authenticated_transition_v2(
    program_id: &Pubkey,
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    action: LiabilityBasisActionV2,
    position_owner: [u8; 32],
    typed_terminal: Option<&AuthenticatedLbv2TerminalParentV2>,
) -> Result<Box<PlannedTransitionV2>, ProgramError> {
    let market_data = accounts
        .market
        .try_borrow_data()
        .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
    let market = MarketViewV2::decode(&market_data)?;
    let aggregate_before = read_vector(
        &market_data,
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
        market.claim_count,
    )?;
    drop(market_data);
    let position_data = accounts
        .position
        .try_borrow_data()
        .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
    let position = PositionViewV2::decode(&position_data)?;
    let position_before = read_vector(
        &position_data,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        position.claim_count,
    )?;
    drop(position_data);
    authenticate_claims_state(
        program_id,
        accounts,
        action,
        market,
        position,
        position_owner,
    )?;
    if typed_terminal.is_some() {
        authenticate_parent_custody_release(accounts, market)?;
    } else {
        authenticate_releases(accounts, market)?;
    }
    let basis = match typed_terminal {
        Some(expected) => {
            let admitted = expected.admitted_basis;
            if admitted.basis_id().to_bytes() != market.basis_id
                || admitted.product_instance_id().to_bytes() != market.product_instance_id
                || admitted.claim_count() != market.claim_count
            {
                return Err(LiabilityBasisSbfErrorV2::ProductLink.into());
            }
            admitted
        }
        None => authenticate_product_and_basis(accounts, market)?,
    };
    let terminal = authenticate_core_and_terminal(accounts, market, basis, action)?;
    let token_before = token_amounts(accounts)?;
    let hoard_before = match action.0.kind {
        LiabilityBasisActionKindV2::Split => token_before.destination,
        LiabilityBasisActionKindV2::Merge | LiabilityBasisActionKindV2::TerminalRedeem => {
            token_before.source
        }
    };
    let mut aggregate_after = alloc::vec![0_u64; aggregate_before.len()];
    let mut position_after = alloc::vec![0_u64; position_before.len()];
    let candidate = plan_candidate(
        basis,
        action,
        terminal,
        &aggregate_before,
        &position_before,
        hoard_before,
        &mut aggregate_after,
        &mut position_after,
    )?;
    let market_revision_after = action
        .0
        .expected_market_revision
        .checked_add(1)
        .ok_or(LiabilityBasisSbfErrorV2::Candidate)?;
    let position_revision_after = action
        .0
        .expected_position_revision
        .checked_add(1)
        .ok_or(LiabilityBasisSbfErrorV2::Candidate)?;
    let market_candidate =
        candidate_market_bytes(accounts.market, market_revision_after, &aggregate_after)?;
    let position_candidate =
        candidate_position_bytes(accounts.position, position_revision_after, &position_after)?;
    let candidate_digest = hashv(&[
        &LIABILITY_BASIS_CANDIDATE_DIGEST_DOMAIN_V2,
        &market_candidate,
        &position_candidate,
    ])
    .to_bytes();
    let action = match typed_terminal {
        Some(expected) => {
            let custody_present = candidate.collateral_out() != 0;
            if custody_present
                == (expected.expected_custody_revision == LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2)
            {
                return Err(LiabilityBasisSbfErrorV2::CustodyRequest.into());
            }
            LiabilityBasisActionV2::new(LiabilityBasisActionInputV2 {
                kind: LiabilityBasisActionKindV2::TerminalRedeem,
                custody_present,
                expected_market_revision: action.0.expected_market_revision,
                expected_position_revision: action.0.expected_position_revision,
                quantity: action.0.quantity,
                claim_index: action.0.claim_index,
                expected_custody_revision: if custody_present {
                    expected.expected_custody_revision
                } else {
                    0
                },
                request_nonce: action.0.request_nonce,
            })?
        }
        None => action,
    };
    Ok(Box::new(PlannedTransitionV2 {
        market,
        action,
        terminal,
        token_before,
        candidate,
        market_revision_after,
        position_revision_after,
        market_candidate,
        position_candidate,
        candidate_digest,
    }))
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn complete_authenticated_transition_v2(
    program_id: &Pubkey,
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    planned: &PlannedTransitionV2,
    custody_bytes: Option<&[u8]>,
    custody_parent_request_digest: [u8; 32],
    position_owner: [u8; 32],
    typed_terminal: Option<&AuthenticatedLbv2TerminalParentV2>,
) -> Result<ExecutedTransitionV2, ProgramError> {
    let parent_custody_bytes = parent_custody_bytes_v2(
        program_id,
        accounts,
        planned,
        custody_parent_request_digest,
        typed_terminal,
    )?;
    let effective_custody_bytes = parent_custody_bytes
        .as_ref()
        .map(|bytes| bytes.as_slice())
        .or(custody_bytes);
    let observed_terminal_request = observe_typed_terminal_request_v2(
        program_id,
        accounts,
        planned,
        effective_custody_bytes,
        position_owner,
        typed_terminal,
    )?;
    let custody_evidence = execute_custody_phase_v2(
        program_id,
        accounts,
        planned,
        effective_custody_bytes,
        custody_parent_request_digest,
    )?;
    authenticate_physical_postconditions(
        accounts,
        planned.action,
        planned.candidate,
        planned.token_before,
    )?;
    let post_resource_digest = hashv(&[
        POST_RESOURCE_DIGEST_DOMAIN_V2,
        &planned.market_candidate,
        &planned.position_candidate,
        &custody_evidence.receipt_digest,
        &custody_evidence.replay_digest,
    ])
    .to_bytes();
    let typed_receipt = build_typed_terminal_receipt_v2(
        observed_terminal_request.as_deref(),
        custody_evidence.as_ref(),
        post_resource_digest,
    )?;
    let collateral_amount = planned
        .candidate
        .collateral_in()
        .checked_add(planned.candidate.collateral_out())
        .ok_or(LiabilityBasisSbfErrorV2::Postcondition)?;
    commit_candidates(
        accounts.market,
        accounts.position,
        &planned.market_candidate,
        &planned.position_candidate,
    )?;
    Ok(ExecutedTransitionV2 {
        market_revision_after: planned.market_revision_after,
        position_revision_after: planned.position_revision_after,
        collateral_amount,
        basis_id: planned.market.basis_id,
        candidate_digest: planned.candidate_digest,
        custody_receipt_digest: custody_evidence.receipt_digest,
        typed_receipt,
        typed_custody: match (
            typed_terminal,
            custody_evidence.request,
            custody_evidence.receipt,
        ) {
            (Some(_), Some(request), Some(receipt)) => {
                Some(Box::new(AuthenticatedLbv2TerminalCustodyV2 {
                    request,
                    request_digest: custody_evidence.request_digest,
                    receipt,
                    receipt_digest: custody_evidence.receipt_digest,
                    replay_digest: custody_evidence.replay_digest,
                }))
            }
            (Some(_), None, None) => None,
            (None, _, _) => None,
            _ => return Err(LiabilityBasisSbfErrorV2::Postcondition.into()),
        },
    })
}

#[inline(never)]
fn parent_custody_bytes_v2(
    program_id: &Pubkey,
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    planned: &PlannedTransitionV2,
    parent_request_digest: [u8; 32],
    typed_terminal: Option<&AuthenticatedLbv2TerminalParentV2>,
) -> Result<Option<Box<[u8; dclutch_custody_contract::CUSTODY_REQUEST_BYTES_V1]>>, ProgramError> {
    match typed_terminal {
        Some(expected) if planned.action.0.custody_present => Ok(Some(Box::new(
            expected_custody_request_v2(
                program_id,
                accounts,
                planned.market,
                planned.action,
                planned.candidate,
                planned.candidate_digest,
                parent_request_digest,
                expected.beneficiary_actor,
            )?
            .to_bytes()
            .map_err(|_| LiabilityBasisSbfErrorV2::CustodyRequest)?,
        ))),
        _ => Ok(None),
    }
}

#[inline(never)]
fn observe_typed_terminal_request_v2(
    program_id: &Pubkey,
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    planned: &PlannedTransitionV2,
    custody_bytes: Option<&[u8]>,
    position_owner: [u8; 32],
    typed_terminal: Option<&AuthenticatedLbv2TerminalParentV2>,
) -> Result<Option<Box<Lbv2TerminalRedeemRequestV2>>, ProgramError> {
    match typed_terminal {
        Some(expected) => Ok(Some(Box::new(authenticate_typed_terminal_request_v2(
            program_id,
            accounts,
            planned.market,
            planned.action,
            planned.terminal,
            planned.candidate,
            planned.candidate_digest,
            custody_bytes,
            position_owner,
            expected,
        )?))),
        None => Ok(None),
    }
}

#[inline(never)]
fn execute_custody_phase_v2(
    program_id: &Pubkey,
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    planned: &PlannedTransitionV2,
    custody_bytes: Option<&[u8]>,
    parent_request_digest: [u8; 32],
) -> Result<Box<CustodyExecutionEvidenceV2>, ProgramError> {
    if planned.action.0.custody_present {
        let request_bytes = custody_bytes.ok_or(LiabilityBasisSbfErrorV2::Instruction)?;
        let request = authenticate_custody_request(
            program_id,
            accounts,
            planned.market,
            planned.action,
            planned.candidate,
            planned.candidate_digest,
            parent_request_digest,
            accounts.owner.key.to_bytes(),
            request_bytes,
        )?;
        return invoke_custody(program_id, accounts, request, request_bytes).map(Box::new);
    }
    if custody_bytes.is_some()
        || planned.candidate.collateral_in() != 0
        || planned.candidate.collateral_out() != 0
    {
        return Err(LiabilityBasisSbfErrorV2::CustodyRequest.into());
    }
    Ok(Box::new(CustodyExecutionEvidenceV2 {
        request: None,
        request_digest: [0; 32],
        receipt: None,
        receipt_digest: [0; 32],
        replay_digest: [0; 32],
    }))
}

#[inline(never)]
fn build_typed_terminal_receipt_v2(
    request: Option<&Lbv2TerminalRedeemRequestV2>,
    custody: &CustodyExecutionEvidenceV2,
    post_resource_digest: [u8; 32],
) -> Result<Option<Box<Lbv2TerminalRedeemReceiptV2>>, ProgramError> {
    match request {
        Some(request) => Ok(Some(Box::new(
            Lbv2TerminalRedeemReceiptV2::new(
                *request,
                hash(&request.to_bytes()).to_bytes(),
                custody.receipt_digest,
                custody.replay_digest,
                post_resource_digest,
            )
            .map_err(|_| LiabilityBasisSbfErrorV2::Postcondition)?,
        ))),
        None => Ok(None),
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_typed_terminal_request_v2(
    program_id: &Pubkey,
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    market: MarketViewV2,
    action: LiabilityBasisActionV2,
    terminal: Option<TerminalResultV2>,
    candidate: ClaimsCandidateV2,
    candidate_digest: [u8; 32],
    custody_bytes: Option<&[u8]>,
    position_owner: [u8; 32],
    expected: &AuthenticatedLbv2TerminalParentV2,
) -> Result<Lbv2TerminalRedeemRequestV2, ProgramError> {
    if action.0.kind != LiabilityBasisActionKindV2::TerminalRedeem
        || expected.release_set != market.release_set
        || expected.market != market.logical_market
        || expected.parent_context != market.custody_context
        || expected.beneficiary_actor != accounts.owner.key.to_bytes()
        || expected.outcome != action.0.claim_index
    {
        return Err(LiabilityBasisSbfErrorV2::ProductLink.into());
    }
    let (terminal_numerator, terminal_denominator) = match terminal {
        Some(TerminalResultV2::RationalCoordinate {
            numerator,
            denominator,
        }) => (numerator, denominator),
        _ => return Err(LiabilityBasisSbfErrorV2::ProductLink.into()),
    };
    let core_data = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
    let core = CoreState::decode(&core_data).map_err(|_| LiabilityBasisSbfErrorV2::ProductLink)?;
    drop(core_data);
    let product_record_digest = hash(
        &accounts
            .product_record
            .try_borrow_data()
            .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?,
    )
    .to_bytes();
    let linked_basis_record_digest = hash(
        &accounts
            .basis_record
            .try_borrow_data()
            .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?,
    )
    .to_bytes();
    let core_product_record = core.identity.product_record.to_bytes();
    let semantic_product_id = core.identity.product_id.to_bytes();
    let terminal_coordinate_digest = core
        .terminal_receipt
        .ok_or(LiabilityBasisSbfErrorV2::ProductLink)?
        .to_bytes();
    if product_record_digest != core_product_record
        || semantic_product_id != market.product_instance_id
        || hash(
            &accounts
                .terminal_coordinate
                .try_borrow_data()
                .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?,
        )
        .to_bytes()
            != terminal_coordinate_digest
    {
        return Err(LiabilityBasisSbfErrorV2::ProductLink.into());
    }
    let evaluated_payout = candidate.collateral_out();
    let custody_request_digest = custody_bytes.map_or([0; 32], |bytes| hash(bytes).to_bytes());
    let (pre_custody_revision, post_custody_revision) = if evaluated_payout == 0 {
        (
            LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2,
            LBV2_TERMINAL_ABSENT_CUSTODY_REVISION_V2,
        )
    } else {
        (
            action.0.expected_custody_revision,
            action
                .0
                .expected_custody_revision
                .checked_add(1)
                .ok_or(LiabilityBasisSbfErrorV2::Candidate)?,
        )
    };
    let observed = Lbv2TerminalRedeemRequestV2::new(Lbv2TerminalRedeemRequestInputV2 {
        release_set: market.release_set,
        market: market.logical_market,
        product_record_digest,
        semantic_product_id,
        semantic_basis_id: market.basis_id,
        linked_basis_record_digest,
        terminal_coordinate_digest,
        owner: position_owner,
        protocol_position: accounts.position.key.to_bytes(),
        claims_program: program_id.to_bytes(),
        custody_request_digest,
        candidate_digest,
        terminal_numerator,
        terminal_denominator,
        claim_index: action.0.claim_index,
        pre_market_revision: action.0.expected_market_revision,
        post_market_revision: action
            .0
            .expected_market_revision
            .checked_add(1)
            .ok_or(LiabilityBasisSbfErrorV2::Candidate)?,
        pre_position_revision: action.0.expected_position_revision,
        post_position_revision: action
            .0
            .expected_position_revision
            .checked_add(1)
            .ok_or(LiabilityBasisSbfErrorV2::Candidate)?,
        debit_quantity: action.0.quantity,
        evaluated_payout,
        pre_custody_revision,
        post_custody_revision,
    })
    .map_err(|_| LiabilityBasisSbfErrorV2::Postcondition)?;
    Ok(observed)
}

fn split_instruction(
    instruction_data: &[u8],
) -> Result<(&[u8], Option<&[u8]>), LiabilityBasisSbfErrorV2> {
    let action_bytes = instruction_data
        .get(..LIABILITY_BASIS_ACTION_BYTES_V2)
        .ok_or(LiabilityBasisSbfErrorV2::Instruction)?;
    let action = LiabilityBasisActionV2::decode(action_bytes)?;
    let expected = if action.0.custody_present {
        LIABILITY_BASIS_ACTION_BYTES_V2
            .checked_add(dclutch_custody_contract::CUSTODY_REQUEST_BYTES_V1)
            .ok_or(LiabilityBasisSbfErrorV2::Instruction)?
    } else {
        LIABILITY_BASIS_ACTION_BYTES_V2
    };
    if instruction_data.len() != expected {
        return Err(LiabilityBasisSbfErrorV2::Instruction);
    }
    let custody = if action.0.custody_present {
        Some(
            instruction_data
                .get(LIABILITY_BASIS_ACTION_BYTES_V2..)
                .ok_or(LiabilityBasisSbfErrorV2::Instruction)?,
        )
    } else {
        None
    };
    Ok((action_bytes, custody))
}

#[derive(Clone, Copy)]
struct LiabilityBasisAccountsV2<'accounts, 'info> {
    owner: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    position: &'accounts AccountInfo<'info>,
    basis_record: &'accounts AccountInfo<'info>,
    basis_staging: &'accounts AccountInfo<'info>,
    product_record: &'accounts AccountInfo<'info>,
    product_staging: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    terminal_coordinate: &'accounts AccountInfo<'info>,
    terminal_coordinate_staging: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    custody_program: &'accounts AccountInfo<'info>,
    custody_programdata: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    custody_caller_authority: &'accounts AccountInfo<'info>,
    realm: &'accounts AccountInfo<'info>,
    realm_staging: &'accounts AccountInfo<'info>,
    custody_replay: &'accounts AccountInfo<'info>,
    collateral_mint: &'accounts AccountInfo<'info>,
    source_token: &'accounts AccountInfo<'info>,
    destination_token: &'accounts AccountInfo<'info>,
    custody_authority: &'accounts AccountInfo<'info>,
    token_program: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> LiabilityBasisAccountsV2<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        Ok(Self {
            owner: account(accounts, OWNER_ACCOUNT)?,
            market: account(accounts, MARKET_ACCOUNT)?,
            position: account(accounts, POSITION_ACCOUNT)?,
            basis_record: account(accounts, BASIS_RECORD_ACCOUNT)?,
            basis_staging: account(accounts, BASIS_STAGING_ACCOUNT)?,
            product_record: account(accounts, PRODUCT_RECORD_ACCOUNT)?,
            product_staging: account(accounts, PRODUCT_STAGING_ACCOUNT)?,
            rent: account(accounts, RENT_ACCOUNT)?,
            core_market: account(accounts, CORE_MARKET_ACCOUNT)?,
            terminal_coordinate: account(accounts, TERMINAL_COORDINATE_ACCOUNT)?,
            terminal_coordinate_staging: account(accounts, TERMINAL_COORDINATE_STAGING_ACCOUNT)?,
            cache: account(accounts, ACTIVATION_CACHE_ACCOUNT)?,
            registry: account(accounts, REGISTRY_PROGRAM_ACCOUNT)?,
            claims_program: account(accounts, CLAIMS_PROGRAM_ACCOUNT)?,
            claims_programdata: account(accounts, CLAIMS_PROGRAMDATA_ACCOUNT)?,
            custody_program: account(accounts, CUSTODY_PROGRAM_ACCOUNT)?,
            custody_programdata: account(accounts, CUSTODY_PROGRAMDATA_ACCOUNT)?,
            core_program: account(accounts, CORE_PROGRAM_ACCOUNT)?,
            core_programdata: account(accounts, CORE_PROGRAMDATA_ACCOUNT)?,
            custody_caller_authority: account(accounts, CUSTODY_CALLER_AUTHORITY_ACCOUNT)?,
            realm: account(accounts, REALM_ACCOUNT)?,
            realm_staging: account(accounts, REALM_STAGING_ACCOUNT)?,
            custody_replay: account(accounts, CUSTODY_REPLAY_ACCOUNT)?,
            collateral_mint: account(accounts, COLLATERAL_MINT_ACCOUNT)?,
            source_token: account(accounts, SOURCE_TOKEN_ACCOUNT)?,
            destination_token: account(accounts, DESTINATION_TOKEN_ACCOUNT)?,
            custody_authority: account(accounts, CUSTODY_AUTHORITY_ACCOUNT)?,
            token_program: account(accounts, COLLATERAL_TOKEN_PROGRAM_ACCOUNT)?,
        })
    }
}

#[derive(Clone, Copy)]
pub(crate) struct MarketViewV2 {
    pub(crate) claim_count: u32,
    pub(crate) revision: u64,
    pub(crate) logical_market: [u8; 32],
    pub(crate) release_set: [u8; 32],
    pub(crate) registry_program: [u8; 32],
    pub(crate) product_instance_id: [u8; 32],
    pub(crate) basis_id: [u8; 32],
    pub(crate) realm_id: [u8; 32],
    pub(crate) custody_context: [u8; 32],
    pub(crate) generation: u64,
}

impl MarketViewV2 {
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, LiabilityBasisSbfErrorV2> {
        if read_array::<8>(bytes, 0)? != MARKET_MAGIC_V2 || read_u16(bytes, 8)? != ABI_VERSION_V2 {
            return Err(LiabilityBasisSbfErrorV2::ClaimsState);
        }
        require_zero(bytes, 10, 2, LiabilityBasisSbfErrorV2::ClaimsState)?;
        let value = Self {
            claim_count: read_u32(bytes, MARKET_CLAIM_COUNT_OFFSET)?,
            revision: read_u64(bytes, MARKET_REVISION_OFFSET)?,
            logical_market: read_array(bytes, MARKET_LOGICAL_ID_OFFSET)?,
            release_set: read_array(bytes, MARKET_RELEASE_SET_OFFSET)?,
            registry_program: read_array(bytes, MARKET_REGISTRY_OFFSET)?,
            product_instance_id: read_array(bytes, MARKET_PRODUCT_OFFSET)?,
            basis_id: read_array(bytes, MARKET_BASIS_OFFSET)?,
            realm_id: read_array(bytes, MARKET_REALM_OFFSET)?,
            custody_context: read_array(bytes, MARKET_CUSTODY_CONTEXT_OFFSET)?,
            generation: read_u64(bytes, MARKET_GENERATION_OFFSET)?,
        };
        require_nonzero_ids(&[
            value.logical_market,
            value.release_set,
            value.registry_program,
            value.product_instance_id,
            value.basis_id,
            value.realm_id,
            value.custody_context,
        ])?;
        if value.claim_count == 0
            || bytes.len()
                != vector_width(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, value.claim_count)?
        {
            return Err(LiabilityBasisSbfErrorV2::ClaimsState);
        }
        Ok(value)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PositionViewV2 {
    pub(crate) claim_count: u32,
    pub(crate) revision: u64,
    pub(crate) market_account: [u8; 32],
    pub(crate) owner: [u8; 32],
    pub(crate) basis_id: [u8; 32],
}

impl PositionViewV2 {
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, LiabilityBasisSbfErrorV2> {
        if read_array::<8>(bytes, 0)? != POSITION_MAGIC_V2 || read_u16(bytes, 8)? != ABI_VERSION_V2
        {
            return Err(LiabilityBasisSbfErrorV2::ClaimsState);
        }
        require_zero(bytes, 10, 2, LiabilityBasisSbfErrorV2::ClaimsState)?;
        require_zero(bytes, 120, 8, LiabilityBasisSbfErrorV2::ClaimsState)?;
        let value = Self {
            claim_count: read_u32(bytes, POSITION_CLAIM_COUNT_OFFSET)?,
            revision: read_u64(bytes, POSITION_REVISION_OFFSET)?,
            market_account: read_array(bytes, POSITION_MARKET_OFFSET)?,
            owner: read_array(bytes, POSITION_OWNER_OFFSET)?,
            basis_id: read_array(bytes, POSITION_BASIS_OFFSET)?,
        };
        require_nonzero_ids(&[value.market_account, value.owner, value.basis_id])?;
        if value.claim_count == 0
            || bytes.len()
                != vector_width(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, value.claim_count)?
        {
            return Err(LiabilityBasisSbfErrorV2::ClaimsState);
        }
        Ok(value)
    }
}

#[derive(Clone, Copy)]
enum OwnerAuthenticationV2 {
    PublicSigner,
    ParentActor { beneficiary_actor: [u8; 32] },
}

fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    owner_authentication: OwnerAuthenticationV2,
) -> Result<(), ProgramError> {
    let owner_is_authenticated = match owner_authentication {
        OwnerAuthenticationV2::PublicSigner => accounts.owner.is_signer,
        OwnerAuthenticationV2::ParentActor { beneficiary_actor } => {
            accounts.owner.is_signer && accounts.owner.key.to_bytes() == beneficiary_actor
        }
    };
    if !owner_is_authenticated
        || accounts.owner.is_writable
        || accounts.owner.executable
        || !accounts.market.is_writable
        || accounts.market.is_signer
        || accounts.market.executable
        || !accounts.position.is_writable
        || accounts.position.is_signer
        || accounts.position.executable
        || accounts.claims_program.key != program_id
        || !accounts.claims_program.executable
        || accounts.claims_program.is_signer
        || accounts.claims_program.is_writable
        || !accounts.registry.executable
        || accounts.registry.is_signer
        || accounts.registry.is_writable
        || !accounts.custody_program.executable
        || accounts.custody_program.is_signer
        || accounts.custody_program.is_writable
        || !accounts.core_program.executable
        || accounts.core_program.is_signer
        || accounts.core_program.is_writable
        || accounts.custody_caller_authority.is_signer
        || accounts.custody_caller_authority.is_writable
        || accounts.custody_caller_authority.executable
        || !accounts.custody_replay.is_writable
        || accounts.custody_replay.is_signer
        || accounts.custody_replay.executable
        || !accounts.source_token.is_writable
        || accounts.source_token.is_signer
        || accounts.source_token.executable
        || !accounts.destination_token.is_writable
        || accounts.destination_token.is_signer
        || accounts.destination_token.executable
        || accounts.source_token.key == accounts.destination_token.key
        || accounts.collateral_mint.is_writable
        || accounts.collateral_mint.is_signer
        || accounts.collateral_mint.executable
        || !accounts.token_program.executable
        || accounts.token_program.is_signer
        || accounts.token_program.is_writable
    {
        return Err(LiabilityBasisSbfErrorV2::Accounts.into());
    }
    for account in [
        accounts.basis_record,
        accounts.basis_staging,
        accounts.product_record,
        accounts.product_staging,
        accounts.rent,
        accounts.core_market,
        accounts.terminal_coordinate,
        accounts.terminal_coordinate_staging,
        accounts.cache,
        accounts.claims_programdata,
        accounts.custody_programdata,
        accounts.core_programdata,
        accounts.realm,
        accounts.realm_staging,
        accounts.custody_authority,
    ] {
        if account.is_signer || account.is_writable {
            return Err(LiabilityBasisSbfErrorV2::Accounts.into());
        }
    }
    Ok(())
}

#[inline(never)]
fn authenticate_claims_state(
    program_id: &Pubkey,
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    action: LiabilityBasisActionV2,
    market: MarketViewV2,
    position: PositionViewV2,
    expected_position_owner: [u8; 32],
) -> Result<(), ProgramError> {
    let market_seeds = [
        LIABILITY_BASIS_MARKET_SEED_V2,
        market.logical_market.as_slice(),
    ];
    let expected_market = Pubkey::find_program_address(&market_seeds, program_id).0;
    let position_seeds =
        ProtocolPositionSeedsV2::new(accounts.market.key.to_bytes(), expected_position_owner)
            .map_err(|_| LiabilityBasisSbfErrorV2::ClaimsState)?;
    let expected_position = Pubkey::find_program_address(&position_seeds.as_slices(), program_id).0;
    if accounts.market.owner != program_id
        || accounts.position.owner != program_id
        || accounts.market.key != &expected_market
        || accounts.position.key != &expected_position
        || position.market_account != accounts.market.key.to_bytes()
        || position.owner != expected_position_owner
        || position.basis_id != market.basis_id
        || position.claim_count != market.claim_count
        || market.registry_program != accounts.registry.key.to_bytes()
        || market.revision != action.0.expected_market_revision
        || position.revision != action.0.expected_position_revision
    {
        return Err(LiabilityBasisSbfErrorV2::ClaimsState.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_releases(
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    market: MarketViewV2,
) -> Result<(), ProgramError> {
    let claims = reauthenticate(
        accounts.registry,
        accounts.cache,
        ExecutionRoleV1::Claims,
        accounts.claims_program,
        accounts.claims_programdata,
    )
    .map_err(|_| LiabilityBasisSbfErrorV2::Release)?;
    let custody = reauthenticate(
        accounts.registry,
        accounts.cache,
        ExecutionRoleV1::Custody,
        accounts.custody_program,
        accounts.custody_programdata,
    )
    .map_err(|_| LiabilityBasisSbfErrorV2::Release)?;
    let core = reauthenticate(
        accounts.registry,
        accounts.cache,
        ExecutionRoleV1::Core,
        accounts.core_program,
        accounts.core_programdata,
    )
    .map_err(|_| LiabilityBasisSbfErrorV2::Release)?;
    for receipt in [claims, custody, core] {
        if receipt.execution_release_set_id().as_bytes() != &market.release_set {
            return Err(LiabilityBasisSbfErrorV2::Release.into());
        }
    }
    Ok(())
}

/// Authenticate the one release role not already proven by the enclosing
/// Claims route. The typed parent path has just authenticated its caller,
/// Claims, Core, Product, basis, and terminal Core phase against these same
/// accounts; Custody remains an independent executable boundary and therefore
/// still requires its own immediate Registry receipt here.
#[inline(never)]
fn authenticate_parent_custody_release(
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    market: MarketViewV2,
) -> Result<(), ProgramError> {
    let custody = reauthenticate(
        accounts.registry,
        accounts.cache,
        ExecutionRoleV1::Custody,
        accounts.custody_program,
        accounts.custody_programdata,
    )
    .map_err(|_| LiabilityBasisSbfErrorV2::Release)?;
    if custody.execution_release_set_id().as_bytes() != &market.release_set {
        return Err(LiabilityBasisSbfErrorV2::Release.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_product_and_basis(
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    market: MarketViewV2,
) -> Result<AdmittedBasisV2, ProgramError> {
    authenticate_product_and_basis_records(
        accounts.basis_record,
        accounts.basis_staging,
        accounts.product_record,
        accounts.product_staging,
        accounts.rent,
        accounts.core_program,
        market,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_product_and_basis_records(
    basis_record: &AccountInfo<'_>,
    basis_staging: &AccountInfo<'_>,
    product_record: &AccountInfo<'_>,
    product_staging: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
    core_program: &AccountInfo<'_>,
    market: MarketViewV2,
) -> Result<AdmittedBasisV2, ProgramError> {
    authenticate_self_finalized_record(
        core_program,
        rent,
        basis_record,
        basis_staging,
        LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2,
    )?;
    let basis_data = basis_record
        .try_borrow_data()
        .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
    let linked = LinkedBasisRecordV2::decode(&basis_data)
        .map_err(|_| LiabilityBasisSbfErrorV2::ProductLink)?;
    if linked.product_instance_id().to_bytes() != market.product_instance_id
        || linked.semantic_basis_id().to_bytes() != market.basis_id
    {
        return Err(LiabilityBasisSbfErrorV2::ProductLink.into());
    }
    let embedded_basis = linked.basis_record();
    let semantic_preimage = semantic_basis_preimage_v2(embedded_basis)
        .map_err(|_| LiabilityBasisSbfErrorV2::ProductLink)?;
    let basis_semantic_id = hashv(&[
        BASIS_SEMANTIC_ID_DOMAIN_V2,
        semantic_preimage.prefix(),
        semantic_preimage.suffix(),
    ])
    .to_bytes();
    if basis_semantic_id != market.basis_id {
        return Err(LiabilityBasisSbfErrorV2::ProductLink.into());
    }
    authenticate_finalized_record(
        core_program,
        rent,
        product_record,
        product_staging,
        PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        market.product_instance_id,
    )?;
    let product_data = product_record
        .try_borrow_data()
        .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
    let product =
        InstanceV1::decode(&product_data).map_err(|_| LiabilityBasisSbfErrorV2::ProductLink)?;
    if product.claim_basis_id().to_bytes() != market.basis_id
        || product.partition_cell_count() != market.claim_count
    {
        return Err(LiabilityBasisSbfErrorV2::ProductLink.into());
    }
    let basis_id = content_id_v2(market.basis_id)?;
    let product_id = content_id_v2(market.product_instance_id)?;
    AdmittedBasisV2::admit(embedded_basis, basis_id, basis_id, product_id)
        .map_err(|_| LiabilityBasisSbfErrorV2::ProductLink.into())
}

pub(crate) fn authenticate_self_finalized_record(
    core_program: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema_release: [u8; 32],
) -> Result<(), ProgramError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
    let digest = hash(&data).to_bytes();
    drop(data);
    authenticate_finalized_record(core_program, rent, raw, staging, schema_release, digest)
}

fn authenticate_finalized_record(
    core_program: &AccountInfo<'_>,
    rent_account: &AccountInfo<'_>,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema_release: [u8; 32],
    expected_digest: [u8; 32],
) -> Result<(), ProgramError> {
    if raw.owner != core_program.key
        || raw.executable
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
        || staging.executable
        || rent_account.key != &sysvar::rent::ID
        || rent_account.executable
        || hash(
            &raw.try_borrow_data()
                .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?,
        )
        .to_bytes()
            != expected_digest
    {
        return Err(LiabilityBasisSbfErrorV2::FinalizedRecord.into());
    }
    let raw_seeds = [
        RAW_RECORD_PDA_SEED_V1,
        schema_release.as_slice(),
        expected_digest.as_slice(),
    ];
    let staging_seeds = [
        STAGING_CURSOR_PDA_SEED_V1,
        schema_release.as_slice(),
        expected_digest.as_slice(),
    ];
    if raw.key != &Pubkey::find_program_address(&raw_seeds, core_program.key).0
        || staging.key != &Pubkey::find_program_address(&staging_seeds, core_program.key).0
    {
        return Err(LiabilityBasisSbfErrorV2::FinalizedRecord.into());
    }
    let rent = Rent::from_account_info(rent_account)
        .map_err(|_| LiabilityBasisSbfErrorV2::FinalizedRecord)?;
    if raw.lamports() < rent.minimum_balance(raw.data_len()) {
        return Err(LiabilityBasisSbfErrorV2::FinalizedRecord.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_core_and_terminal(
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    market: MarketViewV2,
    basis: AdmittedBasisV2,
    action: LiabilityBasisActionV2,
) -> Result<Option<TerminalResultV2>, ProgramError> {
    if accounts.core_market.owner != accounts.core_program.key
        || accounts.core_market.key.to_bytes() != market.logical_market
        || accounts.core_market.data_len() != STATE_BYTES
    {
        return Err(LiabilityBasisSbfErrorV2::ProductLink.into());
    }
    let data = accounts
        .core_market
        .try_borrow_data()
        .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
    let core = CoreState::decode(&data).map_err(|_| LiabilityBasisSbfErrorV2::ProductLink)?;
    drop(data);
    if core.identity.market_id.to_bytes() != market.logical_market
        || core.identity.realm_id.to_bytes() != market.realm_id
        || core.identity.product_id.to_bytes() != market.product_instance_id
        || core.identity.selected_release_set.to_bytes() != market.release_set
        || core.identity.registry_program.to_bytes() != market.registry_program
        || core.identity.generation != market.generation
    {
        return Err(LiabilityBasisSbfErrorV2::ProductLink.into());
    }
    match action.0.kind {
        LiabilityBasisActionKindV2::Split | LiabilityBasisActionKindV2::Merge => {
            if core.phase != CorePhase::Open
                || accounts.terminal_coordinate.key != accounts.core_program.key
                || accounts.terminal_coordinate_staging.key != accounts.core_program.key
            {
                return Err(LiabilityBasisSbfErrorV2::ProductLink.into());
            }
            Ok(None)
        }
        LiabilityBasisActionKindV2::TerminalRedeem => {
            if core.phase != CorePhase::Terminal {
                return Err(LiabilityBasisSbfErrorV2::ProductLink.into());
            }
            match basis.kind() {
                BasisKindV2::CategoricalQ1 => {
                    if accounts.terminal_coordinate.key != accounts.core_program.key
                        || accounts.terminal_coordinate_staging.key != accounts.core_program.key
                    {
                        return Err(LiabilityBasisSbfErrorV2::ProductLink.into());
                    }
                    Ok(Some(TerminalResultV2::Categorical {
                        winner: core.terminal_winner,
                    }))
                }
                BasisKindV2::CappedRampComplement => {
                    let terminal_digest = core
                        .terminal_receipt
                        .ok_or(LiabilityBasisSbfErrorV2::ProductLink)?
                        .to_bytes();
                    authenticate_finalized_record(
                        accounts.core_program,
                        accounts.rent,
                        accounts.terminal_coordinate,
                        accounts.terminal_coordinate_staging,
                        TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2,
                        terminal_digest,
                    )?;
                    let coordinate = accounts
                        .terminal_coordinate
                        .try_borrow_data()
                        .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
                    if coordinate.len() != TERMINAL_COORDINATE_BYTES_V2
                        || read_array::<8>(&coordinate, 0)? != TERMINAL_COORDINATE_MAGIC_V2
                        || read_u16(&coordinate, 8)? != ABI_VERSION_V2
                    {
                        return Err(LiabilityBasisSbfErrorV2::ProductLink.into());
                    }
                    require_zero(&coordinate, 10, 6, LiabilityBasisSbfErrorV2::ProductLink)?;
                    require_zero(&coordinate, 28, 4, LiabilityBasisSbfErrorV2::ProductLink)?;
                    let numerator = read_i64(&coordinate, 16)?;
                    let denominator = read_u32(&coordinate, 24)?;
                    if denominator == 0 {
                        return Err(LiabilityBasisSbfErrorV2::ProductLink.into());
                    }
                    Ok(Some(TerminalResultV2::RationalCoordinate {
                        numerator,
                        denominator,
                    }))
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn plan_candidate(
    basis: AdmittedBasisV2,
    action: LiabilityBasisActionV2,
    terminal: Option<TerminalResultV2>,
    aggregate_before: &[u64],
    position_before: &[u64],
    hoard_before: u64,
    aggregate_after: &mut [u64],
    position_after: &mut [u64],
) -> Result<ClaimsCandidateV2, ProgramError> {
    let result = match action.0.kind {
        LiabilityBasisActionKindV2::Split => basis.plan_split_into(
            aggregate_before,
            position_before,
            action.0.quantity,
            hoard_before,
            aggregate_after,
            position_after,
        ),
        LiabilityBasisActionKindV2::Merge => basis.plan_merge_into(
            aggregate_before,
            position_before,
            action.0.quantity,
            hoard_before,
            aggregate_after,
            position_after,
        ),
        LiabilityBasisActionKindV2::TerminalRedeem => basis.plan_terminal_redeem_into(
            terminal.ok_or(LiabilityBasisSbfErrorV2::ProductLink)?,
            action.0.claim_index,
            aggregate_before,
            position_before,
            action.0.quantity,
            hoard_before,
            aggregate_after,
            position_after,
        ),
    };
    result.map_err(map_candidate_error)
}

fn map_candidate_error(_error: ProductClaimsErrorV2) -> ProgramError {
    LiabilityBasisSbfErrorV2::Candidate.into()
}

#[inline(never)]
fn candidate_market_bytes(
    account: &AccountInfo<'_>,
    revision: u64,
    supplies: &[u64],
) -> Result<Vec<u8>, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
    let mut candidate = data.to_vec();
    drop(data);
    put(
        &mut candidate,
        MARKET_REVISION_OFFSET,
        &revision.to_le_bytes(),
    )?;
    write_vector(
        &mut candidate,
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
        supplies,
    )?;
    Ok(candidate)
}

#[inline(never)]
fn candidate_position_bytes(
    account: &AccountInfo<'_>,
    revision: u64,
    balances: &[u64],
) -> Result<Vec<u8>, ProgramError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
    let mut candidate = data.to_vec();
    drop(data);
    put(
        &mut candidate,
        POSITION_REVISION_OFFSET,
        &revision.to_le_bytes(),
    )?;
    write_vector(
        &mut candidate,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        balances,
    )?;
    Ok(candidate)
}

#[derive(Clone, Copy)]
struct TokenAmounts {
    source: u64,
    destination: u64,
}

#[inline(never)]
fn token_amounts(
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
) -> Result<TokenAmounts, ProgramError> {
    if accounts.source_token.owner != accounts.token_program.key
        || accounts.destination_token.owner != accounts.token_program.key
        || accounts.collateral_mint.owner != accounts.token_program.key
    {
        return Err(LiabilityBasisSbfErrorV2::Accounts.into());
    }
    let source = TokenAccount::parse(
        &accounts
            .source_token
            .try_borrow_data()
            .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?,
    )
    .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
    let destination = TokenAccount::parse(
        &accounts
            .destination_token
            .try_borrow_data()
            .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?,
    )
    .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
    if source.mint != accounts.collateral_mint.key.to_bytes()
        || destination.mint != accounts.collateral_mint.key.to_bytes()
    {
        return Err(LiabilityBasisSbfErrorV2::Accounts.into());
    }
    Ok(TokenAmounts {
        source: source.amount,
        destination: destination.amount,
    })
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_custody_request(
    program_id: &Pubkey,
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    market: MarketViewV2,
    action: LiabilityBasisActionV2,
    candidate: ClaimsCandidateV2,
    candidate_digest: [u8; 32],
    parent_request_digest: [u8; 32],
    beneficiary: [u8; 32],
    request_bytes: &[u8],
) -> Result<CustodyRequestV1, ProgramError> {
    let request = CustodyRequestV1::decode(request_bytes)
        .map_err(|_| LiabilityBasisSbfErrorV2::CustodyRequest)?;
    let expected = expected_custody_request_v2(
        program_id,
        accounts,
        market,
        action,
        candidate,
        candidate_digest,
        parent_request_digest,
        beneficiary,
    )?;
    if request != expected {
        return Err(LiabilityBasisSbfErrorV2::CustodyRequest.into());
    }
    authenticate_custody_request_accounts_v2(program_id, accounts, action, request, request_bytes)?;
    Ok(request)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn expected_custody_request_v2(
    program_id: &Pubkey,
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    market: MarketViewV2,
    action: LiabilityBasisActionV2,
    candidate: ClaimsCandidateV2,
    candidate_digest: [u8; 32],
    parent_request_digest: [u8; 32],
    beneficiary: [u8; 32],
) -> Result<CustodyRequestV1, ProgramError> {
    let amount = candidate
        .collateral_in()
        .checked_add(candidate.collateral_out())
        .ok_or(LiabilityBasisSbfErrorV2::CustodyRequest)?;
    if amount == 0 {
        return Err(LiabilityBasisSbfErrorV2::CustodyRequest.into());
    }
    let split = action.0.kind == LiabilityBasisActionKindV2::Split;
    Ok(CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Claims,
        source_compartment: if split {
            CompartmentV1::External
        } else {
            CompartmentV1::HoardPrincipal
        },
        destination_compartment: if split {
            CompartmentV1::HoardPrincipal
        } else {
            CompartmentV1::External
        },
        release_set: market.release_set,
        market: market.logical_market,
        realm: market.realm_id,
        context: market.custody_context,
        caller_program: program_id.to_bytes(),
        semantic: ContextV1 {
            candidate: candidate_digest,
            source_owner: if split { beneficiary } else { [0; 32] },
            destination_owner: if split { [0; 32] } else { beneficiary },
            order: [0; 32],
            parent_request_digest,
            order_nonce: action.0.request_nonce,
            generation: market.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: accounts.source_token.key.to_bytes(),
        destination: accounts.destination_token.key.to_bytes(),
        source_vault_context: if split {
            [0; 32]
        } else {
            market.logical_market
        },
        destination_vault_context: if split {
            market.logical_market
        } else {
            [0; 32]
        },
        mint: accounts.collateral_mint.key.to_bytes(),
        token_program: accounts.token_program.key.to_bytes(),
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: action.0.expected_custody_revision,
        resulting_revision: action
            .0
            .expected_custody_revision
            .checked_add(1)
            .ok_or(LiabilityBasisSbfErrorV2::CustodyRequest)?,
        amount,
        rent_lamports: 0,
    })
}

fn authenticate_custody_request_accounts_v2(
    program_id: &Pubkey,
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    action: LiabilityBasisActionV2,
    request: CustodyRequestV1,
    request_bytes: &[u8],
) -> Result<(), ProgramError> {
    let split = action.0.kind == LiabilityBasisActionKindV2::Split;
    let request_digest = hash(request_bytes).to_bytes();
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set)
            .map_err(|_| LiabilityBasisSbfErrorV2::CustodyRequest)?,
        request.market,
        ExecutionRoleV1::Claims,
        request.context,
        request_digest,
    )
    .map_err(|_| LiabilityBasisSbfErrorV2::CustodyRequest)?;
    let replay_seeds = CustodyReplaySeedsV1::from_request(request);
    let authority_seeds = CustodyAuthoritySeedsV1::from_request(request);
    let vault_seeds = CustodyVaultSeedsV1::from_request(request, !split);
    if accounts.custody_caller_authority.key
        != &Pubkey::find_program_address(&caller_seeds.as_slices(), program_id).0
        || accounts.custody_replay.key
            != &Pubkey::find_program_address(
                &replay_seeds.as_slices(),
                accounts.custody_program.key,
            )
            .0
        || accounts.custody_authority.key
            != &Pubkey::find_program_address(
                &authority_seeds.as_slices(),
                accounts.custody_program.key,
            )
            .0
        || (if split {
            accounts.destination_token.key
        } else {
            accounts.source_token.key
        }) != &Pubkey::find_program_address(
            &vault_seeds.as_slices(),
            accounts.custody_program.key,
        )
        .0
    {
        return Err(LiabilityBasisSbfErrorV2::CustodyRequest.into());
    }
    let replay_data = accounts
        .custody_replay
        .try_borrow_data()
        .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
    if replay_data.len() != CUSTODY_REPLAY_BYTES_V1 {
        return Err(LiabilityBasisSbfErrorV2::CustodyRequest.into());
    }
    let replay = CustodyReplayV1::decode(&replay_data)
        .map_err(|_| LiabilityBasisSbfErrorV2::CustodyRequest)?;
    if replay.caller_role != CallerRoleV1::Claims
        || replay.release_set != request.release_set
        || replay.market != request.market
        || replay.realm != request.realm
        || replay.context != request.context
        || replay.caller_program != request.caller_program
        || replay.next_revision != request.expected_revision
        || replay.generation != request.semantic.generation
    {
        return Err(LiabilityBasisSbfErrorV2::CustodyRequest.into());
    }
    Ok(())
}

struct CustodyExecutionEvidenceV2 {
    request: Option<Box<CustodyRequestV1>>,
    request_digest: [u8; 32],
    receipt: Option<Box<CustodyReceiptV1>>,
    receipt_digest: [u8; 32],
    replay_digest: [u8; 32],
}

fn invoke_custody<'info>(
    program_id: &Pubkey,
    accounts: &LiabilityBasisAccountsV2<'_, 'info>,
    request: CustodyRequestV1,
    request_bytes: &[u8],
) -> Result<CustodyExecutionEvidenceV2, ProgramError> {
    let instruction = Instruction {
        program_id: *accounts.custody_program.key,
        accounts: Vec::from([
            AccountMeta::new_readonly(*accounts.custody_caller_authority.key, true),
            AccountMeta::new_readonly(*accounts.core_market.key, false),
            AccountMeta::new_readonly(*accounts.cache.key, false),
            AccountMeta::new_readonly(*accounts.registry.key, false),
            AccountMeta::new_readonly(*accounts.claims_program.key, false),
            AccountMeta::new_readonly(*accounts.claims_programdata.key, false),
            AccountMeta::new_readonly(*accounts.realm.key, false),
            AccountMeta::new_readonly(*accounts.realm_staging.key, false),
            AccountMeta::new(*accounts.custody_replay.key, false),
            AccountMeta::new_readonly(*accounts.collateral_mint.key, false),
            AccountMeta::new(*accounts.source_token.key, false),
            AccountMeta::new(*accounts.destination_token.key, false),
            AccountMeta::new_readonly(*accounts.custody_authority.key, false),
            AccountMeta::new_readonly(*accounts.token_program.key, false),
        ]),
        data: request_bytes.to_vec(),
    };
    let request_digest = hash(request_bytes).to_bytes();
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set)
            .map_err(|_| LiabilityBasisSbfErrorV2::CustodyRequest)?,
        request.market,
        ExecutionRoleV1::Claims,
        request.context,
        request_digest,
    )
    .map_err(|_| LiabilityBasisSbfErrorV2::CustodyRequest)?;
    let bump = Pubkey::find_program_address(&caller_seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = caller_seeds.as_slices();
    invoke_signed(
        &instruction,
        &[
            accounts.custody_caller_authority.clone(),
            accounts.core_market.clone(),
            accounts.cache.clone(),
            accounts.registry.clone(),
            accounts.claims_program.clone(),
            accounts.claims_programdata.clone(),
            accounts.realm.clone(),
            accounts.realm_staging.clone(),
            accounts.custody_replay.clone(),
            accounts.collateral_mint.clone(),
            accounts.source_token.clone(),
            accounts.destination_token.clone(),
            accounts.custody_authority.clone(),
            accounts.token_program.clone(),
            accounts.custody_program.clone(),
        ],
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| LiabilityBasisSbfErrorV2::CustodyCpi)?;
    let (producer, receipt_bytes) =
        get_return_data().ok_or(LiabilityBasisSbfErrorV2::Postcondition)?;
    if producer != *accounts.custody_program.key || receipt_bytes.len() != CUSTODY_RECEIPT_BYTES_V1
    {
        return Err(LiabilityBasisSbfErrorV2::Postcondition.into());
    }
    let receipt = CustodyReceiptV1::decode(&receipt_bytes)
        .map_err(|_| LiabilityBasisSbfErrorV2::Postcondition)?;
    let replay_data = accounts
        .custody_replay
        .try_borrow_data()
        .map_err(|_| LiabilityBasisSbfErrorV2::Accounts)?;
    let replay_digest = hashv(&[&replay_data]).to_bytes();
    drop(replay_data);
    receipt
        .verify_for(request, request_digest, replay_digest)
        .map_err(|_| LiabilityBasisSbfErrorV2::Postcondition)?;
    Ok(CustodyExecutionEvidenceV2 {
        request: Some(Box::new(request)),
        request_digest,
        receipt: Some(Box::new(receipt)),
        receipt_digest: hash(&receipt_bytes).to_bytes(),
        replay_digest,
    })
}

#[inline(never)]
fn authenticate_physical_postconditions(
    accounts: &LiabilityBasisAccountsV2<'_, '_>,
    action: LiabilityBasisActionV2,
    candidate: ClaimsCandidateV2,
    before: TokenAmounts,
) -> Result<(), ProgramError> {
    let after = token_amounts(accounts)?;
    let amount = candidate
        .collateral_in()
        .checked_add(candidate.collateral_out())
        .ok_or(LiabilityBasisSbfErrorV2::Postcondition)?;
    let expected = if action.0.custody_present {
        match action.0.kind {
            LiabilityBasisActionKindV2::Split => (
                before
                    .source
                    .checked_sub(amount)
                    .ok_or(LiabilityBasisSbfErrorV2::Postcondition)?,
                before
                    .destination
                    .checked_add(amount)
                    .ok_or(LiabilityBasisSbfErrorV2::Postcondition)?,
            ),
            LiabilityBasisActionKindV2::Merge | LiabilityBasisActionKindV2::TerminalRedeem => (
                before
                    .source
                    .checked_sub(amount)
                    .ok_or(LiabilityBasisSbfErrorV2::Postcondition)?,
                before
                    .destination
                    .checked_add(amount)
                    .ok_or(LiabilityBasisSbfErrorV2::Postcondition)?,
            ),
        }
    } else {
        (before.source, before.destination)
    };
    let hoard_after = match action.0.kind {
        LiabilityBasisActionKindV2::Split => after.destination,
        LiabilityBasisActionKindV2::Merge | LiabilityBasisActionKindV2::TerminalRedeem => {
            after.source
        }
    };
    if (after.source, after.destination) != expected || hoard_after != candidate.hoard_after() {
        return Err(LiabilityBasisSbfErrorV2::Postcondition.into());
    }
    Ok(())
}

#[inline(never)]
fn commit_candidates(
    market: &AccountInfo<'_>,
    position: &AccountInfo<'_>,
    market_candidate: &[u8],
    position_candidate: &[u8],
) -> Result<(), ProgramError> {
    let mut market_data = market
        .try_borrow_mut_data()
        .map_err(|_| LiabilityBasisSbfErrorV2::Commit)?;
    let mut position_data = position
        .try_borrow_mut_data()
        .map_err(|_| LiabilityBasisSbfErrorV2::Commit)?;
    if market_data.len() != market_candidate.len()
        || position_data.len() != position_candidate.len()
    {
        return Err(LiabilityBasisSbfErrorV2::Commit.into());
    }
    market_data.copy_from_slice(market_candidate);
    position_data.copy_from_slice(position_candidate);
    Ok(())
}

#[derive(Clone, Copy)]
struct LiabilityBasisReceiptV2 {
    action: LiabilityBasisActionKindV2,
    market_revision_before: u64,
    market_revision_after: u64,
    position_revision_before: u64,
    position_revision_after: u64,
    collateral_amount: u64,
    custody_revision_before: u64,
    custody_revision_after: u64,
    basis_id: [u8; 32],
    candidate_digest: [u8; 32],
    custody_receipt_digest: [u8; 32],
}

impl LiabilityBasisReceiptV2 {
    fn to_bytes(self) -> [u8; RECEIPT_BYTES_V2] {
        let mut output = [0_u8; RECEIPT_BYTES_V2];
        put_infallible(&mut output, 0, &RECEIPT_MAGIC_V2);
        put_infallible(&mut output, 8, &ABI_VERSION_V2.to_le_bytes());
        put_infallible(&mut output, 10, &[self.action as u8]);
        for (offset, value) in [
            (16, self.market_revision_before),
            (24, self.market_revision_after),
            (32, self.position_revision_before),
            (40, self.position_revision_after),
            (48, self.collateral_amount),
            (56, self.custody_revision_before),
            (64, self.custody_revision_after),
        ] {
            put_infallible(&mut output, offset, &value.to_le_bytes());
        }
        put_infallible(&mut output, 72, &self.basis_id);
        put_infallible(&mut output, 104, &self.candidate_digest);
        put_infallible(&mut output, 136, &self.custody_receipt_digest);
        output
    }
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| LiabilityBasisSbfErrorV2::Accounts.into())
}

fn content_id_v2(value: [u8; 32]) -> Result<ContentIdV2, ProgramError> {
    ContentIdV2::new(value).map_err(|_| LiabilityBasisSbfErrorV2::ProductLink.into())
}

fn require_nonzero_ids(values: &[[u8; 32]]) -> Result<(), LiabilityBasisSbfErrorV2> {
    if values
        .iter()
        .any(|value| value.iter().all(|byte| *byte == 0))
    {
        return Err(LiabilityBasisSbfErrorV2::ClaimsState);
    }
    Ok(())
}

pub(crate) fn vector_width(
    header: usize,
    claim_count: u32,
) -> Result<usize, LiabilityBasisSbfErrorV2> {
    usize::try_from(claim_count)
        .ok()
        .and_then(|count| count.checked_mul(8))
        .and_then(|tail| header.checked_add(tail))
        .ok_or(LiabilityBasisSbfErrorV2::ClaimsState)
}

pub(crate) fn read_vector(
    bytes: &[u8],
    offset: usize,
    claim_count: u32,
) -> Result<Vec<u64>, LiabilityBasisSbfErrorV2> {
    let count = usize::try_from(claim_count).map_err(|_| LiabilityBasisSbfErrorV2::ClaimsState)?;
    let mut output = Vec::with_capacity(count);
    for index in 0..count {
        let relative = index
            .checked_mul(8)
            .and_then(|value| offset.checked_add(value))
            .ok_or(LiabilityBasisSbfErrorV2::ClaimsState)?;
        output.push(read_u64(bytes, relative)?);
    }
    Ok(output)
}

pub(crate) fn write_vector(
    bytes: &mut [u8],
    offset: usize,
    values: &[u64],
) -> Result<(), LiabilityBasisSbfErrorV2> {
    for (index, value) in values.iter().copied().enumerate() {
        let relative = index
            .checked_mul(8)
            .and_then(|value| offset.checked_add(value))
            .ok_or(LiabilityBasisSbfErrorV2::ClaimsState)?;
        put(bytes, relative, &value.to_le_bytes())?;
    }
    Ok(())
}

fn read_byte(bytes: &[u8], offset: usize) -> Result<u8, LiabilityBasisSbfErrorV2> {
    bytes
        .get(offset)
        .copied()
        .ok_or(LiabilityBasisSbfErrorV2::Instruction)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, LiabilityBasisSbfErrorV2> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, LiabilityBasisSbfErrorV2> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, LiabilityBasisSbfErrorV2> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_i64(bytes: &[u8], offset: usize) -> Result<i64, LiabilityBasisSbfErrorV2> {
    Ok(i64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(
    bytes: &[u8],
    offset: usize,
) -> Result<[u8; N], LiabilityBasisSbfErrorV2> {
    let end = offset
        .checked_add(N)
        .ok_or(LiabilityBasisSbfErrorV2::Instruction)?;
    bytes
        .get(offset..end)
        .ok_or(LiabilityBasisSbfErrorV2::Instruction)?
        .try_into()
        .map_err(|_| LiabilityBasisSbfErrorV2::Instruction)
}

fn require_zero(
    bytes: &[u8],
    offset: usize,
    width: usize,
    error: LiabilityBasisSbfErrorV2,
) -> Result<(), LiabilityBasisSbfErrorV2> {
    let end = offset.checked_add(width).ok_or(error)?;
    if bytes
        .get(offset..end)
        .ok_or(error)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(error);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), LiabilityBasisSbfErrorV2> {
    let end = offset
        .checked_add(value.len())
        .ok_or(LiabilityBasisSbfErrorV2::ClaimsState)?;
    output
        .get_mut(offset..end)
        .ok_or(LiabilityBasisSbfErrorV2::ClaimsState)?
        .copy_from_slice(value);
    Ok(())
}

fn put_infallible(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset.saturating_add(value.len())) {
        target.copy_from_slice(value);
    }
}
