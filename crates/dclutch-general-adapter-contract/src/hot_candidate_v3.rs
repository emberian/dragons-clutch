//! Complete General Hot38 candidate-register ABI.
//!
//! The AccountProfile first projects independently authenticated readonly
//! observations into one exact register bank.  General then overwrites only
//! the semantic coordinates it owns, preserving every other byte until the
//! complete runtime-width plan accepts.  The resulting bank contains every
//! identity, revision, Position-table coordinate, and Custody replay fact
//! needed to project canonical `AffineBatchV2`, `ProtocolPositionV2`, and
//! `CustodyRequestV1` packets.  Trading remains the sole writer and CPI
//! authority.

use dclutch_execution_strategy_contract::v2::{ExecutionCandidateV2, register_bank_bytes_v2};
use dclutch_general_codec::Action;

use crate::{
    local_state_v3::{GeneralLocalStateKindV3, GeneralLocalStateLayoutV3},
    runtime_selection::{
        RuntimeSelectionCursorV2, RuntimeSelectionLayoutV2, RuntimeSelectionPhaseV2,
    },
    runtime_settlement::{RuntimeSettlementActionV2, RuntimeSettlementEffectPlanV2},
    runtime_verify::RuntimeCompleteSetMoveV2,
    runtime_width::{SettlementCursorLayoutV2, SettlementCursorV2},
};

/// Exact common scalar-register count in the General Hot38 ABI.
pub const GENERAL_HOT_COMMON_SCALARS_V3: u32 = 87;
/// Outcome index, quantity, three claim magnitudes, and cursor inventory.
pub const GENERAL_HOT_ITEM_SCALAR_STRIDE_V3: u32 = 6;
/// Exact common identity-register count in the General Hot38 ABI.
pub const GENERAL_HOT_COMMON_IDENTITIES_V3: u32 = 40;
/// General has no per-outcome identity tail.
pub const GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3: u32 = 0;

/// Scalar coordinates consumed by exact child-packet projection.
pub mod scalar {
    /// Settlement action tag.
    pub const ACTION: u32 = 0;
    /// Complete-set movement tag.
    pub const COMPLETE_SET_MOVE: u32 = 1;
    /// Whether the affine Claims route is active.
    pub const CLAIMS_AFFINE_ACTIVE: u32 = 2;
    /// Whether the Custody route is active.
    pub const CUSTODY_ACTIVE: u32 = 3;
    /// Whether the General state becomes terminal.
    pub const TERMINAL: u32 = 4;
    /// One-based order coordinate.
    pub const ORDER_COORDINATE: u32 = 5;
    /// Consumed General settlement revision.
    pub const SETTLEMENT_REVISION: u32 = 6;
    /// Signed-order replay nonce.
    pub const ORDER_NONCE: u32 = 7;
    /// Exact collateral quantity and Custody transfer amount.
    pub const QUOTE_QUANTITY: u32 = 8;
    /// Uniform complete-set quantity.
    pub const COMPLETE_SET_QUANTITY: u32 = 9;
    /// Product-authenticated outcome count echoed by AccountProfile.
    pub const OUTCOME_COUNT: u32 = 10;
    /// Nonzero General terminal coordinate.
    pub const TERMINAL_COORDINATE: u32 = 11;
    /// Immutable Market generation.
    pub const GENERATION: u32 = 12;
    /// Immutable page coordinate.
    pub const PAGE_INDEX: u32 = 13;
    /// Immutable execution coordinate.
    pub const EXECUTION_INDEX: u32 = 14;
    /// Ordered Custody transfer coordinate.
    pub const TRANSFER_INDEX: u32 = 15;
    /// Custody replay pre-revision.
    pub const CUSTODY_EXPECTED_REVISION: u32 = 16;
    /// Custody replay required post-revision.
    pub const CUSTODY_RESULTING_REVISION: u32 = 17;
    /// Exact Custody rent movement; zero for settlement transfers.
    pub const CUSTODY_RENT_LAMPORTS: u32 = 18;
    /// Claims aggregate pre-revision.
    pub const CLAIMS_MARKET_REVISION: u32 = 19;
    /// Current row-owner Position pre-revision.
    pub const OWNER_POSITION_REVISION: u32 = 20;
    /// Current settlement Position pre-revision.
    pub const SETTLEMENT_POSITION_REVISION: u32 = 21;
    /// Exact AffineBatch position-table count.
    pub const CLAIMS_POSITION_COUNT: u32 = 22;
    /// Exact AffineBatch row count, equal to Product N when active.
    pub const CLAIMS_ROW_COUNT: u32 = 23;
    /// Reserved zero: successor rows never admit a Position.
    pub const CLAIMS_ADMIT_ACTIVE: u32 = 24;
    /// Reserved zero: successor rows never close a Position.
    pub const CLAIMS_CLOSE_ACTIVE: u32 = 25;
    /// Canonical Custody operation tag (`Transfer`).
    pub const CUSTODY_OPERATION: u32 = 26;
    /// Canonical Custody source-compartment tag.
    pub const CUSTODY_SOURCE_COMPARTMENT: u32 = 27;
    /// Canonical Custody destination-compartment tag.
    pub const CUSTODY_DESTINATION_COMPARTMENT: u32 = 28;
    /// Affine row source-presence bit.
    pub const CLAIMS_SOURCE_PRESENT: u32 = 29;
    /// Affine row destination-presence bit.
    pub const CLAIMS_DESTINATION_PRESENT: u32 = 30;
    /// Canonical sorted Position-table source index.
    pub const CLAIMS_SOURCE_POSITION_INDEX: u32 = 31;
    /// Canonical sorted Position-table destination index.
    pub const CLAIMS_DESTINATION_POSITION_INDEX: u32 = 32;
    /// Affine aggregate signed-magnitude direction.
    pub const CLAIMS_AGGREGATE_DIRECTION: u32 = 33;
    /// Affine source signed-magnitude direction.
    pub const CLAIMS_SOURCE_DIRECTION: u32 = 34;
    /// Affine destination signed-magnitude direction.
    pub const CLAIMS_DESTINATION_DIRECTION: u32 = 35;
    /// Observed settlement Position lamports for lifecycle admission/close.
    pub const OBSERVED_POSITION_LAMPORTS: u32 = 36;
    /// Observed admission-record lamports.
    pub const OBSERVED_ADMISSION_LAMPORTS: u32 = 37;
    /// Current Position rent principal.
    pub const POSITION_RENT_PRINCIPAL: u32 = 38;
    /// Current admission-record rent principal.
    pub const ADMISSION_RENT_PRINCIPAL: u32 = 39;
    /// Whether the settlement Position exists before this action.
    pub const SETTLEMENT_POSITION_PRESENT: u32 = 40;
    /// Canonical sorted Position-table coordinate zero pre-revision.
    pub const POSITION_ZERO_REVISION: u32 = 41;
    /// Canonical sorted Position-table coordinate one pre-revision.
    pub const POSITION_ONE_REVISION: u32 = 42;
    /// Exact total number of active position coordinates (zero, one, or two).
    pub const POSITION_TABLE_COUNT: u32 = 43;
    /// Claims aggregate revision after the one affine mutation.
    pub const CLAIMS_POST_MARKET_REVISION: u32 = 44;
    /// Settlement Position revision after the one affine mutation.
    pub const SETTLEMENT_POST_POSITION_REVISION: u32 = 45;
    /// Exact amount for the selected Custody request.
    pub const CUSTODY_AMOUNT: u32 = 46;
    /// Exact replay-account rent principal used for initialize/terminal close.
    pub const CUSTODY_REPLAY_RENT_LAMPORTS: u32 = 47;
    /// Exact vault-account rent principal used for open/terminal close.
    pub const CUSTODY_VAULT_RENT_LAMPORTS: u32 = 48;
    /// CloseVault pre-revision after the optional terminal surplus transfer.
    pub const CUSTODY_CLOSE_VAULT_EXPECTED_REVISION: u32 = 49;
    /// CloseVault post-revision and CloseReplay pre-revision.
    pub const CUSTODY_CLOSE_VAULT_RESULTING_REVISION: u32 = 50;
    /// Terminal CloseReplay post-revision.
    pub const CUSTODY_CLOSE_REPLAY_RESULTING_REVISION: u32 = 51;
    /// Canonical zero used only by initialization child templates.
    pub const ZERO: u32 = 52;
    /// Selection open/frozen phase.
    pub const SELECTION_PHASE: u32 = 53;
    /// Selection successor revision.
    pub const SELECTION_REVISION: u32 = 54;
    /// Number of distinct submitted candidates considered.
    pub const SELECTION_SUBMITTED_COUNT: u32 = 55;
    /// Best valid submitted Candidate coordinate.
    pub const SELECTION_BEST_CANDIDATE_COORDINATE: u32 = 56;
    /// Verification revision of the best submitted certificate.
    pub const SELECTION_BEST_VERIFIED_REVISION: u32 = 57;
    /// Selection comparison-domain price scale.
    pub const SELECTION_PRICE_SCALE: u32 = 58;
    /// Canonical Selection Cursor magic encoded as a scalar.
    pub const SELECTION_MAGIC: u32 = 59;
    /// Canonical runtime-width record ABI version.
    pub const RUNTIME_WIDTH_VERSION: u32 = 60;
    /// Settlement Cursor successor phase.
    pub const CURSOR_PHASE: u32 = 61;
    /// Settlement Cursor immutable verifier-emitted order count.
    pub const CURSOR_ORDER_COUNT: u32 = 62;
    /// Settlement Cursor next order coordinate.
    pub const CURSOR_NEXT_ORDER: u32 = 63;
    /// Settlement Cursor successor revision.
    pub const CURSOR_RESULTING_REVISION: u32 = 64;
    /// Settlement Cursor successor quote inventory.
    pub const CURSOR_QUOTE_INVENTORY: u32 = 65;
    /// Settlement Cursor immutable complete-set quantity.
    pub const CURSOR_COMPLETE_SET_QUANTITY: u32 = 66;
    /// Settlement Cursor canonical magic encoded as a scalar.
    pub const CURSOR_MAGIC: u32 = 67;
    /// Terminal coordinate persisted in the Settlement Cursor.
    pub const CURSOR_TERMINAL_COORDINATE: u32 = 68;
    /// Untrusted primary-state PDA bump witness projected from the request.
    pub const STATE_BUMP: u32 = 69;
    /// Untrusted Close terminal-record PDA bump witness projected from the request.
    pub const TERMINAL_RECORD_BUMP: u32 = 70;
    /// AccountProfile-owned persisted primary-state bump observation.
    pub const PRIMARY_BUMP_OBSERVATION: u32 = 71;
    /// AccountProfile-owned primary-state historical Rent principal observation.
    pub const PRIMARY_PRINCIPAL_OBSERVATION: u32 = 72;
    /// Lifecycle-owned primary-state created/authenticated branch.
    pub const PRIMARY_CREATED: u32 = 73;
    /// Lifecycle-owned primary-state canonical bump.
    pub const PRIMARY_CANONICAL_BUMP: u32 = 74;
    /// Lifecycle-owned primary-state historical Rent principal.
    pub const PRIMARY_RENT_PRINCIPAL: u32 = 75;
    /// AccountProfile-owned persisted terminal-record bump observation.
    pub const TERMINAL_BUMP_OBSERVATION: u32 = 76;
    /// AccountProfile-owned terminal-record historical Rent principal observation.
    pub const TERMINAL_PRINCIPAL_OBSERVATION: u32 = 77;
    /// Lifecycle-owned terminal-record created/authenticated branch.
    pub const TERMINAL_CREATED: u32 = 78;
    /// Lifecycle-owned terminal-record canonical bump.
    pub const TERMINAL_CANONICAL_BUMP: u32 = 79;
    /// Lifecycle-owned terminal-record historical Rent principal.
    pub const TERMINAL_RENT_PRINCIPAL: u32 = 80;
    /// General local-state envelope magic as one little-endian scalar.
    pub const LOCAL_STATE_MAGIC: u32 = 81;
    /// General local-state envelope ABI version.
    pub const LOCAL_STATE_VERSION: u32 = 82;
    /// General local-state selection/settlement kind.
    pub const LOCAL_STATE_KIND: u32 = 83;
    /// Filled-lots component of the best submitted candidate comparison key.
    pub const SELECTION_BEST_FILLED_LOTS: u32 = 84;
    /// Quote-surplus component of the best submitted candidate comparison key.
    pub const SELECTION_BEST_QUOTE_SURPLUS: u32 = 85;
    /// Trusted canonical input scratch-page count derived from bank geometry.
    pub const INPUT_SCRATCH_PAGE_COUNT: u32 = 86;
}

/// Scalar coordinates within each Product-outcome item bank.
pub mod item_scalar {
    /// Canonical Product outcome index.
    pub const OUTCOME: u32 = 0;
    /// Exact semantic quantity for this outcome.
    pub const QUANTITY: u32 = 1;
    /// Exact aggregate signed-magnitude magnitude.
    pub const CLAIMS_AGGREGATE_MAGNITUDE: u32 = 2;
    /// Exact source-Position signed-magnitude magnitude.
    pub const CLAIMS_SOURCE_MAGNITUDE: u32 = 3;
    /// Exact destination-Position signed-magnitude magnitude.
    pub const CLAIMS_DESTINATION_MAGNITUDE: u32 = 4;
    /// Settlement Cursor successor inventory for this Product outcome.
    pub const CURSOR_INVENTORY: u32 = 5;
}

/// Identity coordinates consumed by exact child-packet projection.
pub mod identity {
    /// Digest of the authenticated Hot parent request.
    pub const PARENT_REQUEST_DIGEST: u32 = 0;
    /// Best-valid-submitted Candidate identity.
    pub const CANDIDATE: u32 = 1;
    /// Current row owner.
    pub const OWNER: u32 = 2;
    /// Current signed-order identity.
    pub const ORDER: u32 = 3;
    /// Immutable terminal surplus beneficiary.
    pub const BENEFICIARY: u32 = 4;
    /// Selected release-set identity.
    pub const RELEASE_SET: u32 = 5;
    /// Canonical logical Market identity.
    pub const MARKET: u32 = 6;
    /// Exact Product record digest.
    pub const PRODUCT_RECORD_DIGEST: u32 = 7;
    /// Semantic LiabilityBasis identity.
    pub const SEMANTIC_BASIS_ID: u32 = 8;
    /// Linked-basis finalized-record digest.
    pub const LINKED_BASIS_RECORD_DIGEST: u32 = 9;
    /// Immutable Realm identity.
    pub const REALM: u32 = 10;
    /// Registry-selected Trading program.
    pub const TRADING_PROGRAM: u32 = 11;
    /// Exact Custody source token account.
    pub const CUSTODY_SOURCE: u32 = 12;
    /// Exact Custody destination token account.
    pub const CUSTODY_DESTINATION: u32 = 13;
    /// Custody source-vault context, zero only for External.
    pub const SOURCE_VAULT_CONTEXT: u32 = 14;
    /// Custody destination-vault context, zero only for External.
    pub const DESTINATION_VAULT_CONTEXT: u32 = 15;
    /// Realm-selected collateral Mint.
    pub const MINT: u32 = 16;
    /// Realm-selected Token or Token-2022 program.
    pub const TOKEN_PROGRAM: u32 = 17;
    /// Custody rent payer; zero for Transfer.
    pub const PAYER: u32 = 18;
    /// Custody rent beneficiary; zero for Transfer.
    pub const RENT_REFUND: u32 = 19;
    /// General settlement Position owner.
    pub const SETTLEMENT_POSITION_OWNER: u32 = 20;
    /// Permanent RentCredit identity for Claims lifecycle.
    pub const RENT_CREDIT: u32 = 21;
    /// Program owning the RentCredit.
    pub const RENT_PROGRAM: u32 = 22;
    /// External Custody source owner, zero for a vault.
    pub const CUSTODY_SOURCE_OWNER: u32 = 23;
    /// External Custody destination owner, zero for a vault.
    pub const CUSTODY_DESTINATION_OWNER: u32 = 24;
    /// Canonical sorted Position-table owner zero.
    pub const POSITION_ZERO_OWNER: u32 = 25;
    /// Canonical sorted Position-table owner one; zero for one-position plans.
    pub const POSITION_ONE_OWNER: u32 = 26;
    /// Canonical General root identity and Custody replay namespace.
    pub const GENERAL_ROOT: u32 = 27;
    /// Product content identity in the selection comparison domain.
    pub const SELECTION_PRODUCT: u32 = 28;
    /// Batch content identity in the selection comparison domain.
    pub const SELECTION_BATCH: u32 = 29;
    /// Immutable interpreted selection-policy content identity.
    pub const SELECTION_POLICY: u32 = 30;
    /// Digest of the exact best submitted VerifiedCandidate record.
    pub const BEST_VERIFIED_DIGEST: u32 = 31;
    /// AccountProfile-owned primary-state RentCredit beneficiary observation.
    pub const PRIMARY_BENEFICIARY_OBSERVATION: u32 = 32;
    /// Lifecycle-owned primary-state RentCredit beneficiary.
    pub const PRIMARY_BENEFICIARY: u32 = 33;
    /// Lifecycle-owned exact primary-state PDA.
    pub const PRIMARY_STATE: u32 = 34;
    /// Lifecycle-owned current Trading program owner.
    pub const PRIMARY_OWNER: u32 = 35;
    /// AccountProfile-owned terminal-record RentCredit beneficiary observation.
    pub const TERMINAL_BENEFICIARY_OBSERVATION: u32 = 36;
    /// Lifecycle-owned terminal-record RentCredit beneficiary.
    pub const TERMINAL_BENEFICIARY: u32 = 37;
    /// Lifecycle-owned exact terminal-record PDA.
    pub const TERMINAL_STATE: u32 = 38;
    /// Lifecycle-owned terminal-record Trading program owner.
    pub const TERMINAL_OWNER: u32 = 39;
}

/// Independently authenticated environment needed by exact Claims/Custody packets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralHotEnvironmentV3 {
    /// Canonical General root identity and Custody replay namespace.
    pub general_root: [u8; 32],
    /// Digest of the exact authenticated Hot request.
    pub parent_request_digest: [u8; 32],
    /// Selected execution release set.
    pub release_set: [u8; 32],
    /// Canonical logical Market.
    pub market: [u8; 32],
    /// Exact Product-record digest.
    pub product_record_digest: [u8; 32],
    /// Exact semantic LiabilityBasis identity.
    pub semantic_basis_id: [u8; 32],
    /// Exact linked-basis finalized-record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Realm collateral authority.
    pub realm: [u8; 32],
    /// Selected Trading program.
    pub trading_program: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact candidate-page coordinate for row actions, zero otherwise.
    pub page_index: u32,
    /// Exact execution coordinate inside the page, zero otherwise.
    pub execution_index: u32,
    /// Claims Market optimistic revision.
    pub claims_market_revision: u64,
    /// Current row-owner Position revision.
    pub owner_position_revision: u64,
    /// Current settlement Position revision.
    pub settlement_position_revision: u64,
    /// Whether the settlement Position exists before this action.
    pub settlement_position_present: bool,
    /// Whether the post-affine settlement Position is proven empty and closeable.
    pub close_settlement_position: bool,
    /// Exact settlement Position owner.
    pub settlement_position_owner: [u8; 32],
    /// Claims Position RentCredit.
    pub rent_credit: [u8; 32],
    /// Program owning the RentCredit.
    pub rent_program: [u8; 32],
    /// Observed Position lamports.
    pub observed_position_lamports: u64,
    /// Observed admission-record lamports.
    pub observed_admission_lamports: u64,
    /// Position rent principal.
    pub position_rent_principal: u64,
    /// Admission-record rent principal.
    pub admission_rent_principal: u64,
    /// Exact Custody source token account.
    pub custody_source: [u8; 32],
    /// Exact Custody destination token account.
    pub custody_destination: [u8; 32],
    /// Exact external source owner or zero.
    pub custody_source_owner: [u8; 32],
    /// Exact external destination owner or zero.
    pub custody_destination_owner: [u8; 32],
    /// Source-vault context or zero for External.
    pub source_vault_context: [u8; 32],
    /// Destination-vault context or zero for External.
    pub destination_vault_context: [u8; 32],
    /// Realm-selected collateral Mint.
    pub mint: [u8; 32],
    /// Realm-selected Token or Token-2022 program.
    pub token_program: [u8; 32],
    /// Custody payer; Transfer requires zero.
    pub payer: [u8; 32],
    /// Custody rent refund; Transfer requires zero.
    pub rent_refund: [u8; 32],
    /// Custody replay pre-revision.
    pub custody_expected_revision: u64,
    /// Ordered transfer index.
    pub transfer_index: u16,
    /// Exact replay-account rent principal.
    pub custody_replay_rent_principal: u64,
    /// Exact vault-account rent principal.
    pub custody_vault_rent_principal: u64,
}

/// Decode the complete independently authenticated environment from the
/// Account/RequestProfile register bank supplied by common Trading.
///
/// The accelerator must not reconstruct these facts from child account order
/// or caller-provided side channels. Every value below is projected once by
/// the selected generic profile and remains read-only at the family boundary.
pub fn general_hot_environment_from_bank_v3(
    bank: &[u8],
    outcome_count: u32,
) -> Result<GeneralHotEnvironmentV3> {
    if bank.len() != general_hot_candidate_bank_len_v3(outcome_count)?
        || read_scalar(bank, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    let scalar_count = general_hot_scalar_count_v3(outcome_count)?;
    let scalar_u32 = |coordinate| {
        u32::try_from(read_scalar(bank, coordinate)?)
            .map_err(|_| GeneralHotCandidateErrorV3::InvalidCoordinate)
    };
    let scalar_u16 = |coordinate| {
        u16::try_from(read_scalar(bank, coordinate)?)
            .map_err(|_| GeneralHotCandidateErrorV3::InvalidCoordinate)
    };
    let present = read_scalar(bank, scalar::SETTLEMENT_POSITION_PRESENT)?;
    if present > 1 {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    Ok(GeneralHotEnvironmentV3 {
        general_root: read_identity(bank, scalar_count, identity::GENERAL_ROOT)?,
        parent_request_digest: read_identity(bank, scalar_count, identity::PARENT_REQUEST_DIGEST)?,
        release_set: read_identity(bank, scalar_count, identity::RELEASE_SET)?,
        market: read_identity(bank, scalar_count, identity::MARKET)?,
        product_record_digest: read_identity(bank, scalar_count, identity::PRODUCT_RECORD_DIGEST)?,
        semantic_basis_id: read_identity(bank, scalar_count, identity::SEMANTIC_BASIS_ID)?,
        linked_basis_record_digest: read_identity(
            bank,
            scalar_count,
            identity::LINKED_BASIS_RECORD_DIGEST,
        )?,
        realm: read_identity(bank, scalar_count, identity::REALM)?,
        trading_program: read_identity(bank, scalar_count, identity::TRADING_PROGRAM)?,
        generation: read_scalar(bank, scalar::GENERATION)?,
        page_index: scalar_u32(scalar::PAGE_INDEX)?,
        execution_index: scalar_u32(scalar::EXECUTION_INDEX)?,
        claims_market_revision: read_scalar(bank, scalar::CLAIMS_MARKET_REVISION)?,
        owner_position_revision: read_scalar(bank, scalar::OWNER_POSITION_REVISION)?,
        settlement_position_revision: read_scalar(bank, scalar::SETTLEMENT_POSITION_REVISION)?,
        settlement_position_present: present == 1,
        close_settlement_position: read_scalar(bank, scalar::POSITION_TABLE_COUNT)? == 0,
        settlement_position_owner: read_identity(
            bank,
            scalar_count,
            identity::SETTLEMENT_POSITION_OWNER,
        )?,
        rent_credit: read_identity(bank, scalar_count, identity::RENT_CREDIT)?,
        rent_program: read_identity(bank, scalar_count, identity::RENT_PROGRAM)?,
        observed_position_lamports: read_scalar(bank, scalar::OBSERVED_POSITION_LAMPORTS)?,
        observed_admission_lamports: read_scalar(bank, scalar::OBSERVED_ADMISSION_LAMPORTS)?,
        position_rent_principal: read_scalar(bank, scalar::POSITION_RENT_PRINCIPAL)?,
        admission_rent_principal: read_scalar(bank, scalar::ADMISSION_RENT_PRINCIPAL)?,
        custody_source: read_identity(bank, scalar_count, identity::CUSTODY_SOURCE)?,
        custody_destination: read_identity(bank, scalar_count, identity::CUSTODY_DESTINATION)?,
        custody_source_owner: read_identity(bank, scalar_count, identity::CUSTODY_SOURCE_OWNER)?,
        custody_destination_owner: read_identity(
            bank,
            scalar_count,
            identity::CUSTODY_DESTINATION_OWNER,
        )?,
        source_vault_context: read_identity(bank, scalar_count, identity::SOURCE_VAULT_CONTEXT)?,
        destination_vault_context: read_identity(
            bank,
            scalar_count,
            identity::DESTINATION_VAULT_CONTEXT,
        )?,
        mint: read_identity(bank, scalar_count, identity::MINT)?,
        token_program: read_identity(bank, scalar_count, identity::TOKEN_PROGRAM)?,
        payer: read_identity(bank, scalar_count, identity::PAYER)?,
        rent_refund: read_identity(bank, scalar_count, identity::RENT_REFUND)?,
        custody_expected_revision: read_scalar(bank, scalar::CUSTODY_EXPECTED_REVISION)?,
        transfer_index: scalar_u16(scalar::TRANSFER_INDEX)?,
        custody_replay_rent_principal: read_scalar(bank, scalar::CUSTODY_REPLAY_RENT_LAMPORTS)?,
        custody_vault_rent_principal: read_scalar(bank, scalar::CUSTODY_VAULT_RENT_LAMPORTS)?,
    })
}

/// Stable refusal from General Hot candidate projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralHotCandidateErrorV3 {
    /// Settlement effect bytes refused.
    InvalidPlan,
    /// Product width differed from the exact effect width.
    TailCountMismatch,
    /// Caller-owned banks were not one exact complete candidate width.
    InvalidCapacity,
    /// An authenticated child coordinate was zero, aliased, or noncanonical.
    InvalidCoordinate,
    /// Position or Custody optimistic revision could not advance.
    RevisionOverflow,
    /// Checked register or byte arithmetic overflowed.
    ArithmeticOverflow,
}

/// Result alias for General Hot candidate projection.
pub type Result<T> = core::result::Result<T, GeneralHotCandidateErrorV3>;

/// Return the exact scalar count for Product width `outcome_count`.
pub fn general_hot_scalar_count_v3(outcome_count: u32) -> Result<u32> {
    GENERAL_HOT_COMMON_SCALARS_V3
        .checked_add(
            outcome_count
                .checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)
}

/// Return the exact scalar-then-identity bank width.
pub fn general_hot_candidate_bank_len_v3(outcome_count: u32) -> Result<usize> {
    if outcome_count == 0 {
        return Err(GeneralHotCandidateErrorV3::TailCountMismatch);
    }
    let bytes = register_bank_bytes_v2(
        general_hot_scalar_count_v3(outcome_count)?,
        GENERAL_HOT_COMMON_IDENTITIES_V3,
    )
    .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    usize::try_from(bytes).map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)
}

/// Authenticated input-bank transport for a read-only General accelerator.
///
/// Implementations may read one contiguous inline request or copy from the
/// exact ordered Trading-owned scratch pages. The destination is explicitly a
/// non-authoritative workspace: callers may observe partial bytes on refusal,
/// but no acknowledgement or state effect may be emitted until projection
/// returns `Ok(())` for the complete Product-derived width.
pub trait GeneralCandidateBankSourceV3 {
    /// Copy the complete authenticated bank into one exact-width workspace.
    fn copy_complete_bank_v3(&self, output: &mut [u8]) -> Result<()>;
}

/// Borrowed contiguous implementation used by inline accelerator requests and
/// host-side differential tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContiguousGeneralBankV3<'a> {
    bytes: &'a [u8],
}

impl<'a> ContiguousGeneralBankV3<'a> {
    /// Construct one exact borrowed bank source.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

impl GeneralCandidateBankSourceV3 for ContiguousGeneralBankV3<'_> {
    fn copy_complete_bank_v3(&self, output: &mut [u8]) -> Result<()> {
        if output.len() != self.bytes.len() {
            return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
        }
        output.copy_from_slice(self.bytes);
        Ok(())
    }
}

/// Project one exact Consider or Freeze selection successor into a candidate bank.
///
/// The selection evaluator produced `selection_after` failure-atomically. This
/// projection preserves all independently authenticated registers and writes
/// only the canonical persisted state fields later consumed by the common
/// EffectProgram. It owns no account and performs no CPI.
pub fn project_general_selection_candidate_v3<'a>(
    action: Action,
    selection_after: &[u8],
    outcome_count: u32,
    authenticated_input: &[u8],
    scratch: &mut [u8],
    output: &'a mut [u8],
) -> Result<ExecutionCandidateV2<'a>> {
    if !matches!(action, Action::Consider | Action::Freeze) {
        return Err(GeneralHotCandidateErrorV3::InvalidPlan);
    }
    exact_candidate_capacities(outcome_count, authenticated_input, scratch, output)?;
    project_general_selection_candidate_scratch_v3(
        action,
        selection_after,
        outcome_count,
        &ContiguousGeneralBankV3::new(authenticated_input),
        scratch,
    )?;
    output.copy_from_slice(scratch);
    Ok(ExecutionCandidateV2::Accepted(output))
}

/// Project Consider/Freeze into one non-authoritative complete workspace.
///
/// The source may be inline or scratch-page backed. The returned bytes remain
/// non-authoritative until the accelerator has committed the whole-bank digest
/// and emitted the exact request-selected acknowledgement chunk.
pub fn project_general_selection_candidate_scratch_v3(
    action: Action,
    selection_after: &[u8],
    outcome_count: u32,
    source: &impl GeneralCandidateBankSourceV3,
    candidate_scratch: &mut [u8],
) -> Result<()> {
    let required = general_hot_candidate_bank_len_v3(outcome_count)?;
    if candidate_scratch.len() != required {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    source.copy_complete_bank_v3(candidate_scratch)?;
    apply_general_selection_candidate_v3(action, selection_after, outcome_count, candidate_scratch)
}

/// Apply Consider/Freeze directly to one complete non-authoritative bank.
///
/// This is the bounded-heap SBF accelerator seam: authenticated scratch pages
/// are assembled into `candidate_scratch` once, then the semantic projection
/// mutates that workspace in place. Callers must not emit an acknowledgement
/// unless this function succeeds for the complete Product-derived width.
pub fn project_general_selection_candidate_in_place_v3(
    action: Action,
    selection_after: &[u8],
    outcome_count: u32,
    candidate_scratch: &mut [u8],
) -> Result<()> {
    if candidate_scratch.len() != general_hot_candidate_bank_len_v3(outcome_count)? {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    apply_general_selection_candidate_v3(action, selection_after, outcome_count, candidate_scratch)
}

fn apply_general_selection_candidate_v3(
    action: Action,
    selection_after: &[u8],
    outcome_count: u32,
    candidate: &mut [u8],
) -> Result<()> {
    if !matches!(action, Action::Consider | Action::Freeze) {
        return Err(GeneralHotCandidateErrorV3::InvalidPlan);
    }
    let selection = RuntimeSelectionCursorV2::decode(selection_after)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let header = selection.header();
    let expected_phase = if action == Action::Consider {
        RuntimeSelectionPhaseV2::Open
    } else {
        RuntimeSelectionPhaseV2::Frozen
    };
    if header.outcome_count != outcome_count || header.phase != expected_phase {
        return Err(GeneralHotCandidateErrorV3::TailCountMismatch);
    }
    write_local_state_constants(candidate, GeneralLocalStateKindV3::Selection)?;
    for (coordinate, value) in [
        (scalar::ACTION, u64::from(action as u8)),
        (scalar::OUTCOME_COUNT, u64::from(outcome_count)),
        (scalar::SELECTION_PHASE, selection_phase_tag(header.phase)),
        (scalar::SELECTION_REVISION, header.revision),
        (
            scalar::SELECTION_SUBMITTED_COUNT,
            u64::from(header.submitted_count),
        ),
        (
            scalar::SELECTION_BEST_CANDIDATE_COORDINATE,
            u64::from(header.best_candidate_coordinate),
        ),
        (
            scalar::SELECTION_BEST_VERIFIED_REVISION,
            header.best_verified_revision,
        ),
        (scalar::SELECTION_PRICE_SCALE, header.price_scale),
        (scalar::SELECTION_BEST_FILLED_LOTS, header.best_filled_lots),
        (
            scalar::SELECTION_BEST_QUOTE_SURPLUS,
            header.best_quote_surplus,
        ),
        (
            scalar::SELECTION_MAGIC,
            RuntimeSelectionLayoutV2::magic_u64(),
        ),
        (
            scalar::RUNTIME_WIDTH_VERSION,
            u64::from(RuntimeSelectionLayoutV2::version_value()),
        ),
    ] {
        write_scalar(candidate, coordinate, value)?;
    }
    let scalar_count = general_hot_scalar_count_v3(outcome_count)?;
    for (coordinate, value) in [
        (identity::CANDIDATE, header.best_candidate_id),
        (identity::SELECTION_PRODUCT, header.product_id),
        (identity::SELECTION_BATCH, header.batch_id),
        (identity::SELECTION_POLICY, header.policy_id),
        (identity::BEST_VERIFIED_DIGEST, header.best_verified_digest),
    ] {
        write_identity(candidate, scalar_count, coordinate, value)?;
    }
    Ok(())
}

/// Project settlement initialization and exact Custody replay/vault creation
/// facts into one complete candidate bank.
pub fn project_general_initialize_candidate_v3<'a>(
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    authenticated_input: &[u8],
    scratch: &mut [u8],
    output: &'a mut [u8],
) -> Result<ExecutionCandidateV2<'a>> {
    exact_candidate_capacities(outcome_count, authenticated_input, scratch, output)?;
    project_general_initialize_candidate_scratch_v3(
        cursor_after,
        outcome_count,
        environment,
        &ContiguousGeneralBankV3::new(authenticated_input),
        scratch,
    )?;
    output.copy_from_slice(scratch);
    Ok(ExecutionCandidateV2::Accepted(output))
}

/// Project settlement initialization into one non-authoritative complete bank.
pub fn project_general_initialize_candidate_scratch_v3(
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    source: &impl GeneralCandidateBankSourceV3,
    candidate_scratch: &mut [u8],
) -> Result<()> {
    let required = general_hot_candidate_bank_len_v3(outcome_count)?;
    if candidate_scratch.len() != required {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    source.copy_complete_bank_v3(candidate_scratch)?;
    apply_general_initialize_candidate_v3(
        cursor_after,
        outcome_count,
        environment,
        candidate_scratch,
    )
}

/// Apply settlement initialization directly to one complete non-authoritative bank.
///
/// This avoids retaining a second 14KiB register bank at hostile widths while
/// preserving Trading's authority boundary: the workspace remains candidate
/// data until its whole-bank digest is acknowledged and accepted by Hot.
pub fn project_general_initialize_candidate_in_place_v3(
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    candidate_scratch: &mut [u8],
) -> Result<()> {
    if candidate_scratch.len() != general_hot_candidate_bank_len_v3(outcome_count)? {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    apply_general_initialize_candidate_v3(
        cursor_after,
        outcome_count,
        environment,
        candidate_scratch,
    )
}

fn apply_general_initialize_candidate_v3(
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<()> {
    let cursor = SettlementCursorV2::decode(cursor_after)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let header = cursor.header();
    if header.outcome_count != outcome_count
        || header.phase != crate::runtime_width::SettlementPhaseV2::Collecting
        || header.revision != 1
        || environment.generation == 0
        || environment.page_index != 0
        || environment.execution_index != 0
        || environment.custody_expected_revision != 0
        || environment.custody_replay_rent_principal == 0
        || environment.custody_vault_rent_principal == 0
        || environment.claims_market_revision == u64::MAX
        || environment.settlement_position_present
        || environment.settlement_position_revision != 0
        || environment.settlement_position_owner == [0; 32]
        || environment.rent_credit == [0; 32]
        || environment.rent_program == [0; 32]
        || environment.position_rent_principal == 0
        || environment.admission_rent_principal == 0
        || environment.observed_position_lamports < environment.position_rent_principal
        || environment.observed_admission_lamports < environment.admission_rent_principal
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    let scalar_count = general_hot_scalar_count_v3(outcome_count)?;
    if read_scalar(candidate, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
        || read_identity(candidate, scalar_count, identity::PARENT_REQUEST_DIGEST)?
            != environment.parent_request_digest
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    for value in [
        environment.general_root,
        environment.parent_request_digest,
        environment.release_set,
        environment.market,
        environment.realm,
        environment.trading_program,
        environment.custody_destination,
        environment.mint,
        environment.token_program,
        environment.payer,
        environment.rent_refund,
    ] {
        if value.iter().all(|byte| *byte == 0) {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
    }
    write_local_state_constants(candidate, GeneralLocalStateKindV3::Settlement)?;
    for (coordinate, value) in [
        (
            scalar::ACTION,
            u64::from(Action::InitializeSettlement as u8),
        ),
        (scalar::OUTCOME_COUNT, u64::from(outcome_count)),
        (scalar::GENERATION, environment.generation),
        (
            scalar::CLAIMS_MARKET_REVISION,
            environment.claims_market_revision,
        ),
        (
            scalar::SETTLEMENT_POSITION_REVISION,
            environment.settlement_position_revision,
        ),
        (
            scalar::OBSERVED_POSITION_LAMPORTS,
            environment.observed_position_lamports,
        ),
        (
            scalar::OBSERVED_ADMISSION_LAMPORTS,
            environment.observed_admission_lamports,
        ),
        (
            scalar::POSITION_RENT_PRINCIPAL,
            environment.position_rent_principal,
        ),
        (
            scalar::ADMISSION_RENT_PRINCIPAL,
            environment.admission_rent_principal,
        ),
        (
            scalar::CUSTODY_REPLAY_RENT_LAMPORTS,
            environment.custody_replay_rent_principal,
        ),
        (
            scalar::CUSTODY_VAULT_RENT_LAMPORTS,
            environment.custody_vault_rent_principal,
        ),
        (scalar::ZERO, 0),
        (scalar::CURSOR_PHASE, settlement_phase_tag(header.phase)),
        (scalar::CURSOR_ORDER_COUNT, u64::from(header.order_count)),
        (scalar::CURSOR_NEXT_ORDER, u64::from(header.next_order)),
        (scalar::CURSOR_RESULTING_REVISION, header.revision),
        (scalar::CURSOR_QUOTE_INVENTORY, header.quote_inventory),
        (
            scalar::CURSOR_COMPLETE_SET_QUANTITY,
            header.complete_set_quantity,
        ),
        (scalar::CURSOR_MAGIC, SettlementCursorLayoutV2::magic_u64()),
        (
            scalar::RUNTIME_WIDTH_VERSION,
            u64::from(SettlementCursorLayoutV2::version_value()),
        ),
        (
            scalar::CURSOR_TERMINAL_COORDINATE,
            header.terminal_coordinate,
        ),
    ] {
        write_scalar(candidate, coordinate, value)?;
    }
    for item in 0..outcome_count {
        let base = GENERAL_HOT_COMMON_SCALARS_V3
            .checked_add(
                item.checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            )
            .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
        write_scalar(
            candidate,
            base.checked_add(item_scalar::OUTCOME)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            u64::from(item),
        )?;
        write_scalar(
            candidate,
            base.checked_add(item_scalar::CURSOR_INVENTORY)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            cursor
                .inventory(item)
                .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?,
        )?;
    }
    for (coordinate, value) in [
        (
            identity::PARENT_REQUEST_DIGEST,
            environment.parent_request_digest,
        ),
        (identity::CANDIDATE, header.candidate_id),
        (identity::RELEASE_SET, environment.release_set),
        (identity::MARKET, environment.market),
        (identity::REALM, environment.realm),
        (identity::TRADING_PROGRAM, environment.trading_program),
        (
            identity::SETTLEMENT_POSITION_OWNER,
            environment.settlement_position_owner,
        ),
        (identity::RENT_CREDIT, environment.rent_credit),
        (identity::RENT_PROGRAM, environment.rent_program),
        (
            identity::CUSTODY_DESTINATION,
            environment.custody_destination,
        ),
        (
            identity::DESTINATION_VAULT_CONTEXT,
            environment.general_root,
        ),
        (identity::MINT, environment.mint),
        (identity::TOKEN_PROGRAM, environment.token_program),
        (identity::PAYER, environment.payer),
        (identity::RENT_REFUND, environment.rent_refund),
        (identity::GENERAL_ROOT, environment.general_root),
    ] {
        write_identity(candidate, scalar_count, coordinate, value)?;
    }
    Ok(())
}

/// Project one complete General plan without discarding authenticated inputs.
///
/// `authenticated_input`, `scratch`, and `output` must have the one exact
/// Product-derived capacity. The entire input is copied to scratch first;
/// output changes only after every semantic and child-ABI coordinate accepts.
pub fn project_general_hot_candidate_v3<'a>(
    effect_plan: &[u8],
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    authenticated_input: &[u8],
    scratch: &mut [u8],
    output: &'a mut [u8],
) -> Result<ExecutionCandidateV2<'a>> {
    exact_candidate_capacities(outcome_count, authenticated_input, scratch, output)?;
    project_general_hot_candidate_scratch_v3(
        effect_plan,
        cursor_after,
        outcome_count,
        environment,
        &ContiguousGeneralBankV3::new(authenticated_input),
        scratch,
    )?;
    output.copy_from_slice(scratch);
    Ok(ExecutionCandidateV2::Accepted(output))
}

/// Project Collect/Materialize/Distribute/Close into one complete scratch bank.
pub fn project_general_hot_candidate_scratch_v3(
    effect_plan: &[u8],
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    source: &impl GeneralCandidateBankSourceV3,
    candidate_scratch: &mut [u8],
) -> Result<()> {
    let required = general_hot_candidate_bank_len_v3(outcome_count)?;
    if candidate_scratch.len() != required {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    source.copy_complete_bank_v3(candidate_scratch)?;
    apply_general_hot_candidate_v3(
        effect_plan,
        cursor_after,
        outcome_count,
        environment,
        candidate_scratch,
    )
}

/// Apply one collect/materialize/distribute/close plan directly to a complete bank.
///
/// The buffer is an accelerator-owned scratch candidate, never persisted
/// state. Semantic refusal may leave scratch bytes changed, but no partial
/// bank can become authoritative because no acknowledgement is emitted.
pub fn project_general_hot_candidate_in_place_v3(
    effect_plan: &[u8],
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    candidate_scratch: &mut [u8],
) -> Result<()> {
    if candidate_scratch.len() != general_hot_candidate_bank_len_v3(outcome_count)? {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    apply_general_hot_candidate_v3(
        effect_plan,
        cursor_after,
        outcome_count,
        environment,
        candidate_scratch,
    )
}

fn apply_general_hot_candidate_v3(
    effect_plan: &[u8],
    cursor_after: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    candidate: &mut [u8],
) -> Result<()> {
    let plan = RuntimeSettlementEffectPlanV2::decode(effect_plan)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let cursor = SettlementCursorV2::decode(cursor_after)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    let cursor_header = cursor.header();
    if plan.header().outcome_count != outcome_count
        || cursor_header.outcome_count != outcome_count
        || cursor_header.candidate_id != plan.header().candidate_id
    {
        return Err(GeneralHotCandidateErrorV3::TailCountMismatch);
    }
    let input_scalar_count = general_hot_scalar_count_v3(outcome_count)?;
    if read_scalar(candidate, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
        || read_identity(
            candidate,
            input_scalar_count,
            identity::PARENT_REQUEST_DIGEST,
        )? != environment.parent_request_digest
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    validate_environment(
        plan.header().action,
        plan.header().custody_active,
        environment,
    )?;
    write_local_state_constants(candidate, GeneralLocalStateKindV3::Settlement)?;
    let header = plan.header();
    let position = position_geometry(header.action, header.claims_active, environment, header)?;
    let custody = custody_geometry(
        header.action,
        header.custody_active,
        header.complete_set_move,
    );
    let custody_resulting_revision = environment
        .custody_expected_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let custody_close_vault_expected_revision = environment
        .custody_expected_revision
        .checked_add(u64::from(header.custody_active))
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let custody_close_vault_resulting_revision = custody_close_vault_expected_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let custody_close_replay_resulting_revision = custody_close_vault_resulting_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let claims_post_market_revision = environment
        .claims_market_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let settlement_post_position_revision = environment
        .settlement_position_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    let custody_amount = if header.action == RuntimeSettlementActionV2::Materialize {
        header.complete_set_quantity
    } else {
        header.quote_quantity
    };
    for (coordinate, value) in [
        (scalar::ACTION, action_tag(header.action)),
        (
            scalar::COMPLETE_SET_MOVE,
            move_tag(header.complete_set_move),
        ),
        (
            scalar::CLAIMS_AFFINE_ACTIVE,
            u64::from(header.claims_active),
        ),
        (scalar::CUSTODY_ACTIVE, u64::from(header.custody_active)),
        (scalar::TERMINAL, u64::from(header.terminal)),
        (scalar::ORDER_COORDINATE, u64::from(header.order_coordinate)),
        (scalar::SETTLEMENT_REVISION, header.revision),
        (scalar::ORDER_NONCE, header.nonce),
        (scalar::QUOTE_QUANTITY, header.quote_quantity),
        (scalar::COMPLETE_SET_QUANTITY, header.complete_set_quantity),
        (scalar::OUTCOME_COUNT, u64::from(outcome_count)),
        (scalar::TERMINAL_COORDINATE, header.terminal_coordinate),
        (scalar::GENERATION, environment.generation),
        (scalar::PAGE_INDEX, u64::from(environment.page_index)),
        (
            scalar::EXECUTION_INDEX,
            u64::from(environment.execution_index),
        ),
        (
            scalar::TRANSFER_INDEX,
            u64::from(environment.transfer_index),
        ),
        (
            scalar::CUSTODY_EXPECTED_REVISION,
            environment.custody_expected_revision,
        ),
        (
            scalar::CUSTODY_RESULTING_REVISION,
            custody_resulting_revision,
        ),
        (scalar::CUSTODY_RENT_LAMPORTS, 0),
        (
            scalar::CLAIMS_MARKET_REVISION,
            environment.claims_market_revision,
        ),
        (
            scalar::OWNER_POSITION_REVISION,
            environment.owner_position_revision,
        ),
        (
            scalar::SETTLEMENT_POSITION_REVISION,
            environment.settlement_position_revision,
        ),
        (scalar::CLAIMS_POSITION_COUNT, u64::from(position.count)),
        (
            scalar::CLAIMS_ROW_COUNT,
            if header.claims_active {
                u64::from(outcome_count)
            } else {
                0
            },
        ),
        (scalar::CLAIMS_ADMIT_ACTIVE, 0),
        (scalar::CLAIMS_CLOSE_ACTIVE, 0),
        (scalar::CUSTODY_OPERATION, 2),
        (scalar::CUSTODY_SOURCE_COMPARTMENT, custody.source),
        (scalar::CUSTODY_DESTINATION_COMPARTMENT, custody.destination),
        (
            scalar::CLAIMS_SOURCE_PRESENT,
            u64::from(position.source_present),
        ),
        (
            scalar::CLAIMS_DESTINATION_PRESENT,
            u64::from(position.destination_present),
        ),
        (
            scalar::CLAIMS_SOURCE_POSITION_INDEX,
            u64::from(position.source_index),
        ),
        (
            scalar::CLAIMS_DESTINATION_POSITION_INDEX,
            u64::from(position.destination_index),
        ),
        (
            scalar::CLAIMS_AGGREGATE_DIRECTION,
            position.aggregate_direction,
        ),
        (scalar::CLAIMS_SOURCE_DIRECTION, position.source_direction),
        (
            scalar::CLAIMS_DESTINATION_DIRECTION,
            position.destination_direction,
        ),
        (
            scalar::OBSERVED_POSITION_LAMPORTS,
            environment.observed_position_lamports,
        ),
        (
            scalar::OBSERVED_ADMISSION_LAMPORTS,
            environment.observed_admission_lamports,
        ),
        (
            scalar::POSITION_RENT_PRINCIPAL,
            environment.position_rent_principal,
        ),
        (
            scalar::ADMISSION_RENT_PRINCIPAL,
            environment.admission_rent_principal,
        ),
        (
            scalar::SETTLEMENT_POSITION_PRESENT,
            u64::from(environment.settlement_position_present),
        ),
        (scalar::POSITION_ZERO_REVISION, position.zero_revision),
        (scalar::POSITION_ONE_REVISION, position.one_revision),
        (scalar::POSITION_TABLE_COUNT, u64::from(position.count)),
        (
            scalar::CLAIMS_POST_MARKET_REVISION,
            claims_post_market_revision,
        ),
        (
            scalar::SETTLEMENT_POST_POSITION_REVISION,
            settlement_post_position_revision,
        ),
        (scalar::CUSTODY_AMOUNT, custody_amount),
        (
            scalar::CUSTODY_REPLAY_RENT_LAMPORTS,
            environment.custody_replay_rent_principal,
        ),
        (
            scalar::CUSTODY_VAULT_RENT_LAMPORTS,
            environment.custody_vault_rent_principal,
        ),
        (
            scalar::CUSTODY_CLOSE_VAULT_EXPECTED_REVISION,
            custody_close_vault_expected_revision,
        ),
        (
            scalar::CUSTODY_CLOSE_VAULT_RESULTING_REVISION,
            custody_close_vault_resulting_revision,
        ),
        (
            scalar::CUSTODY_CLOSE_REPLAY_RESULTING_REVISION,
            custody_close_replay_resulting_revision,
        ),
        (scalar::ZERO, 0),
        (
            scalar::CURSOR_PHASE,
            settlement_phase_tag(cursor_header.phase),
        ),
        (
            scalar::CURSOR_ORDER_COUNT,
            u64::from(cursor_header.order_count),
        ),
        (
            scalar::CURSOR_NEXT_ORDER,
            u64::from(cursor_header.next_order),
        ),
        (scalar::CURSOR_RESULTING_REVISION, cursor_header.revision),
        (
            scalar::CURSOR_QUOTE_INVENTORY,
            cursor_header.quote_inventory,
        ),
        (
            scalar::CURSOR_COMPLETE_SET_QUANTITY,
            cursor_header.complete_set_quantity,
        ),
        (scalar::CURSOR_MAGIC, SettlementCursorLayoutV2::magic_u64()),
        (
            scalar::RUNTIME_WIDTH_VERSION,
            u64::from(SettlementCursorLayoutV2::version_value()),
        ),
        (
            scalar::CURSOR_TERMINAL_COORDINATE,
            cursor_header.terminal_coordinate,
        ),
    ] {
        write_scalar(candidate, coordinate, value)?;
    }
    for item in 0..outcome_count {
        let base = GENERAL_HOT_COMMON_SCALARS_V3
            .checked_add(
                item.checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            )
            .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
        write_scalar(
            candidate,
            base.checked_add(item_scalar::OUTCOME)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            u64::from(item),
        )?;
        let quantity = plan
            .quantity(item)
            .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
        write_scalar(
            candidate,
            base.checked_add(item_scalar::QUANTITY)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            quantity,
        )?;
        write_scalar(
            candidate,
            base.checked_add(item_scalar::CURSOR_INVENTORY)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            cursor
                .inventory(item)
                .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?,
        )?;
        for (coordinate, direction) in [
            (
                item_scalar::CLAIMS_AGGREGATE_MAGNITUDE,
                position.aggregate_direction,
            ),
            (
                item_scalar::CLAIMS_SOURCE_MAGNITUDE,
                position.source_direction,
            ),
            (
                item_scalar::CLAIMS_DESTINATION_MAGNITUDE,
                position.destination_direction,
            ),
        ] {
            write_scalar(
                candidate,
                base.checked_add(coordinate)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
                if direction == 0 { 0 } else { quantity },
            )?;
        }
    }
    let scalar_count = general_hot_scalar_count_v3(outcome_count)?;
    for (coordinate, value) in [
        (
            identity::PARENT_REQUEST_DIGEST,
            environment.parent_request_digest,
        ),
        (identity::CANDIDATE, header.candidate_id),
        (identity::OWNER, header.owner_id),
        (identity::ORDER, header.order_id),
        (identity::BENEFICIARY, header.beneficiary),
        (identity::RELEASE_SET, environment.release_set),
        (identity::MARKET, environment.market),
        (
            identity::PRODUCT_RECORD_DIGEST,
            environment.product_record_digest,
        ),
        (identity::SEMANTIC_BASIS_ID, environment.semantic_basis_id),
        (
            identity::LINKED_BASIS_RECORD_DIGEST,
            environment.linked_basis_record_digest,
        ),
        (identity::REALM, environment.realm),
        (identity::TRADING_PROGRAM, environment.trading_program),
        (identity::CUSTODY_SOURCE, environment.custody_source),
        (
            identity::CUSTODY_DESTINATION,
            environment.custody_destination,
        ),
        (
            identity::SOURCE_VAULT_CONTEXT,
            environment.source_vault_context,
        ),
        (
            identity::DESTINATION_VAULT_CONTEXT,
            environment.destination_vault_context,
        ),
        (identity::MINT, environment.mint),
        (identity::TOKEN_PROGRAM, environment.token_program),
        (identity::PAYER, environment.payer),
        (identity::RENT_REFUND, environment.rent_refund),
        (
            identity::SETTLEMENT_POSITION_OWNER,
            environment.settlement_position_owner,
        ),
        (identity::RENT_CREDIT, environment.rent_credit),
        (identity::RENT_PROGRAM, environment.rent_program),
        (
            identity::CUSTODY_SOURCE_OWNER,
            environment.custody_source_owner,
        ),
        (
            identity::CUSTODY_DESTINATION_OWNER,
            environment.custody_destination_owner,
        ),
        (identity::POSITION_ZERO_OWNER, position.zero_owner),
        (identity::POSITION_ONE_OWNER, position.one_owner),
        (identity::GENERAL_ROOT, environment.general_root),
    ] {
        write_identity(candidate, scalar_count, coordinate, value)?;
    }
    Ok(())
}

fn exact_candidate_capacities(
    outcome_count: u32,
    authenticated_input: &[u8],
    scratch: &[u8],
    output: &[u8],
) -> Result<()> {
    let required = general_hot_candidate_bank_len_v3(outcome_count)?;
    if authenticated_input.len() != required
        || scratch.len() != required
        || output.len() != required
    {
        Err(GeneralHotCandidateErrorV3::InvalidCapacity)
    } else {
        Ok(())
    }
}

const fn selection_phase_tag(phase: RuntimeSelectionPhaseV2) -> u64 {
    match phase {
        RuntimeSelectionPhaseV2::Open => 1,
        RuntimeSelectionPhaseV2::Frozen => 2,
    }
}

const fn settlement_phase_tag(phase: crate::runtime_width::SettlementPhaseV2) -> u64 {
    match phase {
        crate::runtime_width::SettlementPhaseV2::Collecting => 4,
        crate::runtime_width::SettlementPhaseV2::Materializing => 5,
        crate::runtime_width::SettlementPhaseV2::Distributing => 6,
        crate::runtime_width::SettlementPhaseV2::ReadyToClose => 7,
        crate::runtime_width::SettlementPhaseV2::Terminal => 8,
    }
}

#[derive(Clone, Copy)]
struct PositionGeometryV3 {
    count: u32,
    source_present: bool,
    destination_present: bool,
    source_index: u32,
    destination_index: u32,
    aggregate_direction: u64,
    source_direction: u64,
    destination_direction: u64,
    zero_owner: [u8; 32],
    one_owner: [u8; 32],
    zero_revision: u64,
    one_revision: u64,
}

fn position_geometry(
    action: RuntimeSettlementActionV2,
    claims_active: bool,
    environment: GeneralHotEnvironmentV3,
    header: crate::runtime_settlement::RuntimeSettlementEffectHeaderV2,
) -> Result<PositionGeometryV3> {
    if !claims_active {
        return Ok(PositionGeometryV3 {
            count: 0,
            source_present: false,
            destination_present: false,
            source_index: 0,
            destination_index: 0,
            aggregate_direction: 0,
            source_direction: 0,
            destination_direction: 0,
            zero_owner: [0; 32],
            one_owner: [0; 32],
            zero_revision: 0,
            one_revision: 0,
        });
    }
    match action {
        RuntimeSettlementActionV2::Collect | RuntimeSettlementActionV2::Distribute => {
            if !environment.settlement_position_present
                || header.owner_id == environment.settlement_position_owner
            {
                return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
            }
            let collect = action == RuntimeSettlementActionV2::Collect;
            let (source_owner, source_revision, destination_owner, destination_revision) =
                if collect {
                    (
                        header.owner_id,
                        environment.owner_position_revision,
                        environment.settlement_position_owner,
                        environment.settlement_position_revision,
                    )
                } else {
                    (
                        environment.settlement_position_owner,
                        environment.settlement_position_revision,
                        header.owner_id,
                        environment.owner_position_revision,
                    )
                };
            let (zero_owner, zero_revision, one_owner, one_revision, source_index) =
                if source_owner < destination_owner {
                    (
                        source_owner,
                        source_revision,
                        destination_owner,
                        destination_revision,
                        0,
                    )
                } else {
                    (
                        destination_owner,
                        destination_revision,
                        source_owner,
                        source_revision,
                        1,
                    )
                };
            Ok(PositionGeometryV3 {
                count: 2,
                source_present: true,
                destination_present: true,
                source_index,
                destination_index: 1 - source_index,
                aggregate_direction: 0,
                source_direction: 2,
                destination_direction: 1,
                zero_owner,
                one_owner,
                zero_revision,
                one_revision,
            })
        }
        RuntimeSettlementActionV2::Materialize => {
            if !environment.settlement_position_present {
                return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
            }
            let (source_present, destination_present, aggregate, source, destination) =
                match header.complete_set_move {
                    RuntimeCompleteSetMoveV2::Mint => (false, true, 1, 0, 1),
                    RuntimeCompleteSetMoveV2::Merge => (true, false, 2, 2, 0),
                    RuntimeCompleteSetMoveV2::None => {
                        return Err(GeneralHotCandidateErrorV3::InvalidPlan);
                    }
                };
            Ok(PositionGeometryV3 {
                count: 1,
                source_present,
                destination_present,
                source_index: 0,
                destination_index: 0,
                aggregate_direction: aggregate,
                source_direction: source,
                destination_direction: destination,
                zero_owner: environment.settlement_position_owner,
                one_owner: [0; 32],
                zero_revision: environment.settlement_position_revision,
                one_revision: 0,
            })
        }
        RuntimeSettlementActionV2::Close => Err(GeneralHotCandidateErrorV3::InvalidPlan),
    }
}

#[derive(Clone, Copy)]
struct CustodyGeometryV3 {
    source: u64,
    destination: u64,
}

fn custody_geometry(
    action: RuntimeSettlementActionV2,
    active: bool,
    movement: RuntimeCompleteSetMoveV2,
) -> CustodyGeometryV3 {
    if !active {
        return CustodyGeometryV3 {
            source: 0,
            destination: 0,
        };
    }
    let (source, destination) = match action {
        RuntimeSettlementActionV2::Collect => (1, 2),
        RuntimeSettlementActionV2::Distribute | RuntimeSettlementActionV2::Close => (2, 1),
        RuntimeSettlementActionV2::Materialize => match movement {
            RuntimeCompleteSetMoveV2::Mint => (2, 3),
            RuntimeCompleteSetMoveV2::Merge => (3, 2),
            RuntimeCompleteSetMoveV2::None => (0, 0),
        },
    };
    CustodyGeometryV3 {
        source,
        destination,
    }
}

fn validate_environment(
    action: RuntimeSettlementActionV2,
    custody_active: bool,
    environment: GeneralHotEnvironmentV3,
) -> Result<()> {
    for identity in [
        environment.general_root,
        environment.parent_request_digest,
        environment.release_set,
        environment.market,
        environment.product_record_digest,
        environment.semantic_basis_id,
        environment.linked_basis_record_digest,
        environment.realm,
        environment.trading_program,
        environment.settlement_position_owner,
        environment.rent_credit,
        environment.rent_program,
    ] {
        if identity == [0; 32] {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
    }
    if environment.claims_market_revision == u64::MAX
        || environment.owner_position_revision == u64::MAX
        || environment.settlement_position_revision == u64::MAX
        || environment.position_rent_principal == 0
        || environment.admission_rent_principal == 0
        || environment.custody_replay_rent_principal == 0
        || environment.custody_vault_rent_principal == 0
        || environment.observed_position_lamports < environment.position_rent_principal
        || environment.observed_admission_lamports < environment.admission_rent_principal
        || environment.payer != [0; 32]
        || (action == RuntimeSettlementActionV2::Close && environment.rent_refund == [0; 32])
        || (action == RuntimeSettlementActionV2::Close
            && (!environment.settlement_position_present || !environment.close_settlement_position))
        || (action != RuntimeSettlementActionV2::Close && !environment.settlement_position_present)
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    if custody_active {
        for identity in [
            environment.custody_source,
            environment.custody_destination,
            environment.mint,
            environment.token_program,
        ] {
            if identity == [0; 32] {
                return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
            }
        }
        if environment.custody_source == environment.custody_destination
            || environment.custody_expected_revision == u64::MAX
        {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
        let source_external = action == RuntimeSettlementActionV2::Collect;
        let destination_external = matches!(
            action,
            RuntimeSettlementActionV2::Distribute | RuntimeSettlementActionV2::Close
        );
        let source_shape = if source_external {
            environment.source_vault_context == [0; 32]
                && environment.custody_source_owner != [0; 32]
        } else {
            environment.source_vault_context != [0; 32]
                && environment.custody_source_owner == [0; 32]
        };
        let destination_shape = if destination_external {
            environment.destination_vault_context == [0; 32]
                && environment.custody_destination_owner != [0; 32]
        } else {
            environment.destination_vault_context != [0; 32]
                && environment.custody_destination_owner == [0; 32]
        };
        if !source_shape || !destination_shape {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
    }
    Ok(())
}

fn action_tag(action: RuntimeSettlementActionV2) -> u64 {
    match action {
        RuntimeSettlementActionV2::Collect => 1,
        RuntimeSettlementActionV2::Materialize => 2,
        RuntimeSettlementActionV2::Distribute => 3,
        RuntimeSettlementActionV2::Close => 4,
    }
}

fn move_tag(value: RuntimeCompleteSetMoveV2) -> u64 {
    match value {
        RuntimeCompleteSetMoveV2::None => 0,
        RuntimeCompleteSetMoveV2::Mint => 1,
        RuntimeCompleteSetMoveV2::Merge => 2,
    }
}

fn write_local_state_constants(output: &mut [u8], kind: GeneralLocalStateKindV3) -> Result<()> {
    for (coordinate, value) in [
        (
            scalar::LOCAL_STATE_MAGIC,
            GeneralLocalStateLayoutV3::magic_u64(),
        ),
        (
            scalar::LOCAL_STATE_VERSION,
            u64::from(GeneralLocalStateLayoutV3::version_value()),
        ),
        (scalar::LOCAL_STATE_KIND, u64::from(kind.tag())),
    ] {
        write_scalar(output, coordinate, value)?;
    }
    Ok(())
}

fn write_scalar(output: &mut [u8], coordinate: u32, value: u64) -> Result<()> {
    let offset = usize::try_from(coordinate)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    put(output, offset, &value.to_le_bytes())
}

fn read_scalar(input: &[u8], coordinate: u32) -> Result<u64> {
    let offset = usize::try_from(coordinate)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let bytes: [u8; 8] = input
        .get(
            offset
                ..offset
                    .checked_add(8)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )
        .ok_or(GeneralHotCandidateErrorV3::InvalidCapacity)?
        .try_into()
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidCapacity)?;
    Ok(u64::from_le_bytes(bytes))
}

fn write_identity(
    output: &mut [u8],
    scalar_count: u32,
    coordinate: u32,
    value: [u8; 32],
) -> Result<()> {
    let scalar_bytes = usize::try_from(scalar_count)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let identity_offset = usize::try_from(coordinate)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(32)
        .and_then(|offset| scalar_bytes.checked_add(offset))
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    put(output, identity_offset, &value)
}

fn read_identity(input: &[u8], scalar_count: u32, coordinate: u32) -> Result<[u8; 32]> {
    let scalar_bytes = usize::try_from(scalar_count)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let offset = usize::try_from(coordinate)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(32)
        .and_then(|value| scalar_bytes.checked_add(value))
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    input
        .get(
            offset
                ..offset
                    .checked_add(32)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )
        .ok_or(GeneralHotCandidateErrorV3::InvalidCapacity)?
        .try_into()
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidCapacity)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    output
        .get_mut(offset..end)
        .ok_or(GeneralHotCandidateErrorV3::InvalidCapacity)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{vec, vec::Vec};

    use super::*;
    use crate::{
        runtime_settlement::RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2,
        runtime_width::{SettlementCursorHeaderV2, SettlementPhaseV2, settlement_cursor_len},
    };

    fn put_test(output: &mut [u8], offset: usize, value: &[u8]) {
        output[offset..offset + value.len()].copy_from_slice(value);
    }

    fn materialize_plan(outcome_count: u32) -> Vec<u8> {
        let mut output = vec![
            0_u8;
            RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2
                + usize::try_from(outcome_count).expect("test count") * 8
        ];
        put_test(&mut output, 0, b"DCGFXP02");
        put_test(&mut output, 8, &2_u16.to_le_bytes());
        output[10] = RuntimeSettlementActionV2::Materialize as u8;
        output[11] = 1;
        put_test(&mut output, 12, &outcome_count.to_le_bytes());
        output[16] = 0b11;
        put_test(&mut output, 24, &7_u64.to_le_bytes());
        put_test(&mut output, 40, &[0x31; 32]);
        put_test(&mut output, 168, &3_u64.to_le_bytes());
        put_test(&mut output, 176, &3_u64.to_le_bytes());
        for item in 0..outcome_count {
            let offset = RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2
                + usize::try_from(item).expect("item") * 8;
            put_test(&mut output, offset, &3_u64.to_le_bytes());
        }
        RuntimeSettlementEffectPlanV2::decode(&output).expect("canonical plan");
        output
    }

    fn materialize_cursor(outcome_count: u32) -> Vec<u8> {
        let mut output = vec![0; settlement_cursor_len(outcome_count).expect("cursor width")];
        SettlementCursorV2::encode_into(
            SettlementCursorHeaderV2 {
                outcome_count,
                order_count: 1,
                next_order: 0,
                revision: 8,
                candidate_id: [0x31; 32],
                quote_inventory: 0,
                complete_set_quantity: 3,
                terminal_coordinate: 0,
                phase: SettlementPhaseV2::Distributing,
            },
            &vec![3; usize::try_from(outcome_count).expect("test width")],
            &mut output,
        )
        .expect("cursor encode");
        output
    }

    fn initialized_cursor(outcome_count: u32) -> Vec<u8> {
        let mut output = vec![0; settlement_cursor_len(outcome_count).expect("cursor width")];
        SettlementCursorV2::encode_into(
            SettlementCursorHeaderV2 {
                outcome_count,
                order_count: 1,
                next_order: 0,
                revision: 1,
                candidate_id: [0x31; 32],
                quote_inventory: 0,
                complete_set_quantity: 3,
                terminal_coordinate: 0,
                phase: SettlementPhaseV2::Collecting,
            },
            &vec![0; usize::try_from(outcome_count).expect("test width")],
            &mut output,
        )
        .expect("cursor encode");
        output
    }

    fn environment() -> GeneralHotEnvironmentV3 {
        GeneralHotEnvironmentV3 {
            general_root: [21; 32],
            parent_request_digest: [1; 32],
            release_set: [2; 32],
            market: [3; 32],
            product_record_digest: [4; 32],
            semantic_basis_id: [5; 32],
            linked_basis_record_digest: [6; 32],
            realm: [7; 32],
            trading_program: [8; 32],
            generation: 9,
            page_index: 0,
            execution_index: 0,
            claims_market_revision: 10,
            owner_position_revision: 11,
            settlement_position_revision: 0,
            settlement_position_present: false,
            close_settlement_position: false,
            settlement_position_owner: [12; 32],
            rent_credit: [13; 32],
            rent_program: [14; 32],
            observed_position_lamports: 20,
            observed_admission_lamports: 21,
            position_rent_principal: 18,
            admission_rent_principal: 19,
            custody_source: [15; 32],
            custody_destination: [16; 32],
            custody_source_owner: [0; 32],
            custody_destination_owner: [0; 32],
            source_vault_context: [17; 32],
            destination_vault_context: [18; 32],
            mint: [19; 32],
            token_program: [20; 32],
            payer: [0; 32],
            rent_refund: [0; 32],
            custody_expected_revision: 22,
            transfer_index: 1,
            custody_replay_rent_principal: 23,
            custody_vault_rent_principal: 24,
        }
    }

    fn authenticated_input(outcome_count: u32, environment: GeneralHotEnvironmentV3) -> Vec<u8> {
        let len = general_hot_candidate_bank_len_v3(outcome_count).expect("bank length");
        let mut input = vec![0x7a; len];
        write_scalar(&mut input, scalar::OUTCOME_COUNT, u64::from(outcome_count))
            .expect("tail witness");
        write_identity(
            &mut input,
            general_hot_scalar_count_v3(outcome_count).expect("scalar count"),
            identity::PARENT_REQUEST_DIGEST,
            environment.parent_request_digest,
        )
        .expect("parent witness");
        input
    }

    #[test]
    fn runtime_width_one_and_two_fifty_eight_emit_complete_child_facts() {
        for outcome_count in [1_u32, 258] {
            let environment = GeneralHotEnvironmentV3 {
                settlement_position_revision: 12,
                settlement_position_present: true,
                ..environment()
            };
            let input = authenticated_input(outcome_count, environment);
            let mut scratch = vec![0_u8; input.len()];
            let mut output = vec![0x55_u8; input.len()];
            let accepted = project_general_hot_candidate_v3(
                &materialize_plan(outcome_count),
                &materialize_cursor(outcome_count),
                outcome_count,
                environment,
                &input,
                &mut scratch,
                &mut output,
            )
            .expect("complete candidate");
            assert!(matches!(accepted, ExecutionCandidateV2::Accepted(_)));
            let mut in_place = input.clone();
            project_general_hot_candidate_in_place_v3(
                &materialize_plan(outcome_count),
                &materialize_cursor(outcome_count),
                outcome_count,
                environment,
                &mut in_place,
            )
            .expect("one-workspace candidate");
            assert_eq!(in_place, output);
            assert_eq!(read_scalar(&output, scalar::CLAIMS_ADMIT_ACTIVE), Ok(0));
            assert_eq!(read_scalar(&output, scalar::CLAIMS_POSITION_COUNT), Ok(1));
            assert_eq!(
                read_scalar(&output, scalar::CLAIMS_ROW_COUNT),
                Ok(u64::from(outcome_count))
            );
            assert_eq!(
                read_scalar(&output, scalar::CLAIMS_POST_MARKET_REVISION),
                Ok(environment.claims_market_revision + 1)
            );
            assert_eq!(
                read_scalar(&output, scalar::SETTLEMENT_POST_POSITION_REVISION),
                Ok(environment.settlement_position_revision + 1)
            );
            assert_eq!(read_scalar(&output, scalar::CUSTODY_AMOUNT), Ok(3));
            let last = GENERAL_HOT_COMMON_SCALARS_V3
                + (outcome_count - 1) * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            assert_eq!(
                read_scalar(&output, last + item_scalar::OUTCOME),
                Ok(u64::from(outcome_count - 1))
            );
            assert_eq!(read_scalar(&output, last + item_scalar::QUANTITY), Ok(3));
            assert_eq!(
                read_scalar(&output, last + item_scalar::CLAIMS_AGGREGATE_MAGNITUDE,),
                Ok(3)
            );
            assert_eq!(
                read_scalar(&output, last + item_scalar::CLAIMS_SOURCE_MAGNITUDE),
                Ok(0)
            );
            assert_eq!(
                read_scalar(&output, last + item_scalar::CLAIMS_DESTINATION_MAGNITUDE,),
                Ok(3)
            );
            assert_eq!(
                read_identity(
                    &output,
                    general_hot_scalar_count_v3(outcome_count).expect("scalars"),
                    identity::POSITION_ZERO_OWNER,
                ),
                Ok(environment.settlement_position_owner)
            );
        }
    }

    #[test]
    fn initialization_projects_exact_custody_and_cursor_facts_at_runtime_width() {
        for outcome_count in [1_u32, 258] {
            let mut environment = environment();
            environment.payer = [0x51; 32];
            environment.rent_refund = [0x52; 32];
            environment.custody_expected_revision = 0;
            let input = authenticated_input(outcome_count, environment);
            let mut scratch = vec![0_u8; input.len()];
            let mut output = vec![0x55_u8; input.len()];
            assert!(matches!(
                project_general_initialize_candidate_v3(
                    &initialized_cursor(outcome_count),
                    outcome_count,
                    environment,
                    &input,
                    &mut scratch,
                    &mut output,
                ),
                Ok(ExecutionCandidateV2::Accepted(_))
            ));
            let mut in_place = input.clone();
            project_general_initialize_candidate_in_place_v3(
                &initialized_cursor(outcome_count),
                outcome_count,
                environment,
                &mut in_place,
            )
            .expect("one-workspace initialize candidate");
            assert_eq!(in_place, output);
            assert_eq!(
                read_scalar(&output, scalar::CURSOR_RESULTING_REVISION),
                Ok(1)
            );
            assert_eq!(
                read_scalar(&output, scalar::CUSTODY_REPLAY_RENT_LAMPORTS),
                Ok(environment.custody_replay_rent_principal)
            );
            assert_eq!(
                read_identity(
                    &output,
                    general_hot_scalar_count_v3(outcome_count).expect("scalars"),
                    identity::PAYER,
                ),
                Ok(environment.payer)
            );
            let last = GENERAL_HOT_COMMON_SCALARS_V3
                + (outcome_count - 1) * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            assert_eq!(
                read_scalar(&output, last + item_scalar::CURSOR_INVENTORY),
                Ok(0)
            );
        }
    }

    #[test]
    fn substituted_product_or_parent_witness_preserves_output() {
        let outcome_count = 258;
        let environment = environment();
        let plan = materialize_plan(outcome_count);
        let canonical = authenticated_input(outcome_count, environment);
        let mut hostile_tail = canonical.clone();
        write_scalar(&mut hostile_tail, scalar::OUTCOME_COUNT, 257).expect("hostile tail");
        let mut hostile_parent = canonical;
        write_identity(
            &mut hostile_parent,
            general_hot_scalar_count_v3(outcome_count).expect("scalars"),
            identity::PARENT_REQUEST_DIGEST,
            [0x99; 32],
        )
        .expect("hostile parent");
        for input in [hostile_tail, hostile_parent] {
            let mut scratch = vec![0_u8; input.len()];
            let mut output = vec![0x55_u8; input.len()];
            let before = output.clone();
            assert!(
                project_general_hot_candidate_v3(
                    &plan,
                    &materialize_cursor(outcome_count),
                    outcome_count,
                    environment,
                    &input,
                    &mut scratch,
                    &mut output,
                )
                .is_err()
            );
            assert_eq!(output, before);
        }
    }
}
