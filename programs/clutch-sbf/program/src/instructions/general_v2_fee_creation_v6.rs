//! Current action39 fee construction from one authenticated V5 traversal.
//!
//! This module is deliberately private to the SBF adapter. It scans the same
//! retained Feed/page authority used by settlement-root creation, derives
//! owner terminal fees and maker weights without caller rows, and retains only
//! compact scalar/fold state. No 64-row fee book or recipient allocation is
//! materialized on an SBF frame.

use clutch_batch_policy_identity::revenue_policy_v2::{
    MakerWeightAuthorityV2, RevenuePolicyV2,
};
use clutch_batch_policy_identity::{batch_policy_digest, decode_batch_policy, Identity32V1,
    BATCH_POLICY_BYTES};
use clutch_fee_runtime_contract::codec::{
    BorrowedRecipientAllocationRowV1, CertifiedRecipientAllocationAccessV3,
};
use clutch_fee_runtime_contract::projection::SelectedOwnerFeeBookHashV1;
use clutch_fee_runtime_contract::retirement::{
    CompletedOwnerFeeBookV2, FeeRetirementAccumulatorV1, FeeRetirementHashV1,
    StreamingOwnerFeeBookV2,
};
use clutch_fee_runtime_contract::selected::SelectedCompositeFeeV2;
use clutch_fee_runtime_contract::treasury::TreasuryLedgerV1;
use clutch_fee_runtime_contract::weight_v2::{
    composite_fee_hamilton_share_v2, CompositeFeeWeightTranscriptV2,
};
use clutch_fee_runtime_contract::Id as FeeId;
use clutch_general_v2_contract::{
    encode_recipient_allocation_v3_account_from_access,
    fee_runtime_semantic_release_id_v2, FeeRetirementAccumulatorV1AccountV1, Id32,
    Sha256BackendV1,
};
use clutch_general_v2_runtime::{
    commit_visited_portfolio_fee_weight_cache_v2, visit_portfolio_fee_weight_rows_v2,
    AdapterPositionMarketBindingV3, PortfolioFeeWeightPositionAccessV2,
    PortfolioFeeWeightVisitSummaryV2, SettlementAdapterErrorV1,
    SettlementRootExpectationProjectionV1, SettlementTraversalAccessV5,
    VisitedPortfolioFeeWeightRowV2,
};
use clutch_retirement::PositionPurposeV3;
use clutch_solana_layout::Hash32;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::revenue_policy_v2::{
    accept_treasury_service_transition_v1, authenticate_treasury_service_ledger_v1,
    prepare_treasury_service_admission_v1, AuthenticatedRevenuePolicyRecordV2,
    AuthenticatedTreasuryServiceAdmissionV1, PreparedTreasuryServiceTransitionV1,
    RevenueMarketTreasuryDerivationV1,
};
use crate::seeds;

use super::general_v2_settlement_traversal_v5::AuthenticatedSettlementTraversalV5;
use super::general_market_current_v5::AuthenticatedGeneralMarketCurrentV5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeSha256;

impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl SelectedOwnerFeeBookHashV1 for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl FeeRetirementHashV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

/// Exact physical policy/service roles added to the four fresh fee globals.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CandidateFeeAuthorityFrameV6<'a, 'info> {
    pub(crate) realm: &'a AccountInfo<'info>,
    pub(crate) batch_policy: &'a AccountInfo<'info>,
    pub(crate) revenue_policy_record: &'a AccountInfo<'info>,
    pub(crate) treasury_service_ledger: &'a AccountInfo<'info>,
}

/// Compact one-shot action39 fee plan. The completed book is consumed when
/// the accumulator is constructed; it cannot be copied into a caller DTO.
pub(crate) struct PreparedCandidateFeeCreationV6 {
    pub(crate) selected: SelectedCompositeFeeV2,
    pub(crate) revenue_policy: RevenuePolicyV2,
    pub(crate) book: CompletedOwnerFeeBookV2,
    rows: Box<Vec<CachedCandidateFeeRowV6>>,
    transcript: CompositeFeeWeightTranscriptV2,
    owner_order_set_digest: FeeId,
    traversed_owner_count: u16,
    nonzero_weight_row_count: u8,
    maker_rebate_total: u64,
    executor_atoms: u64,
    treasury_atoms: u64,
    collected_fee_atoms: u64,
    pub(crate) service_transition: PreparedTreasuryServiceTransitionV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CachedCandidateFeeRowV6 {
    visited: VisitedPortfolioFeeWeightRowV2,
    maker_rebate_atoms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DerivedPositionAccessV6 {
    program_id: Pubkey,
    market_binding: AdapterPositionMarketBindingV3,
    purpose_binding: [u8; 32],
}

impl CertifiedRecipientAllocationAccessV3 for PreparedCandidateFeeCreationV6 {
    fn fee_record(&self) -> FeeId { self.selected.fee_record() }
    fn row_count(&self) -> u8 { self.nonzero_weight_row_count }
    fn maker_rebate_total(&self) -> u64 { self.maker_rebate_total }
    fn executor_atoms(&self) -> u64 { self.executor_atoms }
    fn treasury_atoms(&self) -> u64 { self.treasury_atoms }
    fn collected_fee_atoms(&self) -> u64 { self.collected_fee_atoms }
    fn row(
        &self,
        index: u8,
    ) -> clutch_fee_runtime_contract::Result<Option<BorrowedRecipientAllocationRowV1>> {
        self.rows
            .get(usize::from(index))
            .map(|row| {
                BorrowedRecipientAllocationRowV1::structural(
                    row.visited.weight_row().position(),
                    row.maker_rebate_atoms,
                )
            })
            .transpose()
    }
    fn weight_policy_id(&self) -> FeeId { self.transcript.policy_id() }
    fn weight_transcript_id(&self) -> FeeId { self.transcript.transcript_id() }
    fn owner_order_set_digest(&self) -> FeeId { self.owner_order_set_digest }
    fn traversed_owner_count(&self) -> u16 { self.traversed_owner_count }
    fn nonzero_weight_row_count(&self) -> u8 { self.row_count() }
}

impl PortfolioFeeWeightPositionAccessV2 for DerivedPositionAccessV6 {
    fn market_binding(&self) -> AdapterPositionMarketBindingV3 { self.market_binding }

    fn position_account(&self, owner: Id32) -> Result<Id32, SettlementAdapterErrorV1> {
        let position = seeds::position_v3_pda(
            &self.program_id,
            &self.market_binding.market_instance_id.bytes(),
            &owner.bytes(),
            PositionPurposeV3::General,
            &self.purpose_binding,
        );
        Id32::new(position.0.to_bytes())
            .map_err(|_| SettlementAdapterErrorV1::PositionSetMismatch)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TreasuryServiceAdmissionEvidenceV6 {
    realm: Hash32,
    market_instance_v2_id: Hash32,
    revenue_policy_record_account: Pubkey,
    revenue_policy_record_v2_id: Hash32,
    revenue_policy_v2_digest: Hash32,
    treasury_owner: Hash32,
    treasury_position_account: Pubkey,
    treasury_service_ledger_account: Pubkey,
    epoch_semantic_id: Hash32,
    admitted_epoch_count_before: u64,
    settled_epoch_count_before: u64,
}

impl AuthenticatedTreasuryServiceAdmissionV1 for TreasuryServiceAdmissionEvidenceV6 {
    fn realm(&self) -> Option<Hash32> { Some(self.realm) }
    fn market_instance_v2_id(&self) -> Option<Hash32> { Some(self.market_instance_v2_id) }
    fn revenue_policy_record_account(&self) -> Option<Pubkey> {
        Some(self.revenue_policy_record_account)
    }
    fn revenue_policy_record_v2_id(&self) -> Option<Hash32> {
        Some(self.revenue_policy_record_v2_id)
    }
    fn revenue_policy_v2_digest(&self) -> Option<Hash32> {
        Some(self.revenue_policy_v2_digest)
    }
    fn treasury_owner(&self) -> Option<Hash32> { Some(self.treasury_owner) }
    fn treasury_position_account(&self) -> Option<Pubkey> {
        Some(self.treasury_position_account)
    }
    fn treasury_service_ledger_account(&self) -> Option<Pubkey> {
        Some(self.treasury_service_ledger_account)
    }
    fn epoch_semantic_id(&self) -> Option<Hash32> { Some(self.epoch_semantic_id) }
    fn admitted_epoch_count_before(&self) -> Option<u64> {
        Some(self.admitted_epoch_count_before)
    }
    fn settled_epoch_count_before(&self) -> Option<u64> {
        Some(self.settled_epoch_count_before)
    }
}

fn map_fee<T>(value: clutch_fee_runtime_contract::Result<T>) -> Outcome<T> {
    value.map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn borrow_data<'a, 'info>(account: &'a AccountInfo<'info>) -> Outcome<core::cell::Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(core::cell::Ref::map(data, |bytes| &**bytes))
}

fn authenticate_batch_policy_v6(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    epoch: Id32,
    expected_digest: Id32,
) -> Outcome<clutch_batch::relation_v1::FrozenPolicyV1> {
    require(
        account.owner == program_id
            && !account.executable
            && !account.is_writable
            && !account.is_signer
            && account.data_len() == BATCH_POLICY_BYTES,
        ClutchError::MismatchedState,
    )?;
    let policy = decode_batch_policy(&borrow_data(account)?)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let digest = batch_policy_digest(&policy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(digest.0 == expected_digest.bytes(), ClutchError::MismatchedState)?;
    expect_pda(
        account.key,
        seeds::batch_policy_pda(program_id, &epoch.bytes(), &digest.0),
        None,
    )?;
    Ok(policy)
}

fn authenticate_revenue_authority_v6(
    current: &AuthenticatedGeneralMarketCurrentV5,
    frame: CandidateFeeAuthorityFrameV6<'_, '_>,
) -> Outcome<(AuthenticatedRevenuePolicyRecordV2, RevenueMarketTreasuryDerivationV1)> {
    let authority = current.revenue();
    let derivation = current.treasury();
    let persisted = current.binding().authority();
    require(
        frame.realm.key == &current.realm_account()
            && frame.revenue_policy_record.key == &authority.record_account()
            && persisted.revenue_policy_record_account().bytes()
                == authority.record_account().to_bytes()
            && persisted.revenue_policy_record_v2_id().bytes()
                == authority.record_semantic_id().bytes()
            && persisted.revenue_policy_v2_digest().bytes()
                == authority.policy_digest().bytes()
            && persisted.treasury_owner().bytes() == authority.treasury_owner().bytes()
            && persisted.treasury_position_derivation_policy_v2_id().bytes()
                == authority.treasury_position_derivation_policy_id().bytes()
            && persisted.treasury_service_ledger_account().bytes()
                == frame.treasury_service_ledger.key.to_bytes()
            && derivation.treasury_position_account().to_bytes()
                == persisted.treasury_position_account().bytes()
            && derivation.treasury_service_ledger_account().to_bytes()
                == persisted.treasury_service_ledger_account().bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok((authority, derivation))
}

fn map_portfolio<T>(value: Result<T, SettlementAdapterErrorV1>) -> Outcome<T> {
    value.map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn cache_fee_rows_v6(
    program_id: &Pubkey,
    traversal: &dyn SettlementTraversalAccessV5,
    selected: &SelectedCompositeFeeV2,
    batch: &clutch_batch::relation_v1::FrozenPolicyV1,
) -> Outcome<(
    Box<Vec<CachedCandidateFeeRowV6>>,
    PortfolioFeeWeightVisitSummaryV2,
    CompositeFeeWeightTranscriptV2,
)> {
    let projection = traversal.projection();
    let positions = DerivedPositionAccessV6 {
        program_id: *program_id,
        market_binding: projection.position_market_binding(),
        purpose_binding: projection.feed().market.bytes(),
    };
    let mut rows = Box::new(Vec::new());
    let summary = map_portfolio(visit_portfolio_fee_weight_rows_v2(
        traversal,
        &positions,
        selected,
        batch,
        |visited| {
            if rows.len() >= clutch_fee_runtime_contract::MAX_FEE_ROWS_V1 {
                return Err(SettlementAdapterErrorV1::FeeOwnerMismatch);
            }
            rows.push(CachedCandidateFeeRowV6 { visited, maker_rebate_atoms: 0 });
            Ok(())
        },
    ))?;
    rows.sort_unstable_by_key(|row| row.visited.weight_row().position());
    let transcript = map_portfolio(commit_visited_portfolio_fee_weight_cache_v2(
        summary,
        selected,
        |index| Ok(rows.get(usize::from(index)).map(|row| row.visited.weight_row())),
    ))?;
    require(
        rows.len() == usize::from(summary.nonzero_weight_row_count())
            && transcript.len() == summary.nonzero_weight_row_count()
            && transcript.total_weight() == summary.total_weight()
            && summary.owner_order_set_digest() == projection.owner_order_set_digest()
            && summary.traversed_owner_count() == projection.expected_owner_count(),
        ClutchError::MismatchedState,
    )?;
    Ok((rows, summary, transcript))
}

fn allocate_cached_hamilton_v6(
    rows: &mut [CachedCandidateFeeRowV6],
    transcript: CompositeFeeWeightTranscriptV2,
    maker_pool: u64,
) -> Outcome<()> {
    if rows.is_empty() {
        return require(
            transcript.len() == 0 && transcript.total_weight() == 0 && maker_pool == 0,
            ClutchError::MismatchedState,
        );
    }
    require(
        rows.len() == usize::from(transcript.len()) && transcript.total_weight() != 0,
        ClutchError::MismatchedState,
    )?;
    let mut floor_sum = 0u64;
    for row in rows.iter() {
        let share = map_fee(composite_fee_hamilton_share_v2(
            maker_pool,
            row.visited.weight_row().exact_numerator(),
            transcript.total_weight(),
        ))?;
        floor_sum = floor_sum
            .checked_add(share.floor_atoms())
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    let dust = maker_pool
        .checked_sub(floor_sum)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let row_count = u64::try_from(rows.len())
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(dust < row_count, ClutchError::MismatchedState)?;
    let mut target_index = 0usize;
    while target_index < rows.len() {
        let target = map_fee(composite_fee_hamilton_share_v2(
            maker_pool,
            rows[target_index].visited.weight_row().exact_numerator(),
            transcript.total_weight(),
        ))?;
        let target_position = rows[target_index].visited.weight_row().position();
        let mut rank = 0u64;
        for other in rows.iter() {
            let share = map_fee(composite_fee_hamilton_share_v2(
                maker_pool,
                other.visited.weight_row().exact_numerator(),
                transcript.total_weight(),
            ))?;
            if share.remainder() > target.remainder()
                || (share.remainder() == target.remainder()
                    && other.visited.weight_row().position() < target_position)
            {
                rank = rank
                    .checked_add(1)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
            }
        }
        rows[target_index].maker_rebate_atoms = target
            .floor_atoms()
            .checked_add(u64::from(rank < dust))
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        target_index = target_index
            .checked_add(1)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    }
    Ok(())
}

/// Authenticate both policy bodies, V4 Revenue authority, writable 0xbb, and
/// derive the complete Position-sorted fee-book commitment from the retained
/// traversal. No fresh fee account is written by this preparation step.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_candidate_fee_creation_v6(
    program_id: &Pubkey,
    traversal: &AuthenticatedSettlementTraversalV5<'_>,
    current: &AuthenticatedGeneralMarketCurrentV5,
    frame: CandidateFeeAuthorityFrameV6<'_, '_>,
    revenue_policy: RevenuePolicyV2,
    epoch_account: Id32,
    epoch_semantic_id: Id32,
    candidate: Id32,
    selected_fee_record: Id32,
) -> Outcome<Box<PreparedCandidateFeeCreationV6>> {
    let market = traversal.market();
    let base = market.base().base();
    let batch = authenticate_batch_policy_v6(
        program_id,
        frame.batch_policy,
        epoch_account,
        market.base().batch_policy_id(),
    )?;
    require(
        current.binding().base() == market && current.revenue().policy() == revenue_policy,
        ClutchError::MismatchedState,
    )?;
    let (revenue_authority, treasury_derivation) =
        authenticate_revenue_authority_v6(current, frame)?;
    let selected = map_fee(SelectedCompositeFeeV2::select(
        FeeId(selected_fee_record.bytes()),
        FeeId(traversal.projection().realm().bytes()),
        FeeId(base.market.bytes()),
        FeeId(epoch_account.bytes()),
        FeeId(candidate.bytes()),
        FeeId(treasury_derivation.treasury_position_account().to_bytes()),
        base.price_scale,
        base.outcome_count,
        &batch,
        &revenue_policy,
    ))?;
    require(
        revenue_policy.maker_weight_authority
            == MakerWeightAuthorityV2::CertifiedOwnerNettedCompositeNumerator,
        ClutchError::MismatchedState,
    )?;
    let (mut rows, summary, transcript) =
        cache_fee_rows_v6(program_id, traversal.traversal(), &selected, &batch)?;
    let split = revenue_policy
        .allocate_split(summary.collected_fee_atoms())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    allocate_cached_hamilton_v6(&mut rows, transcript, split.maker_rebate_atoms)?;
    let mut book = map_fee(StreamingOwnerFeeBookV2::begin(
        &selected,
        FeeId(traversal.projection().owner_order_set_digest().bytes()),
        summary.nonzero_weight_row_count(),
        u128::from(summary.collected_fee_atoms()),
        &RuntimeSha256,
    ))?;
    for row in rows.iter() {
        book = map_fee(book.fold(
            row.visited.weight_row().position(),
            FeeId(row.visited.owner().bytes()),
            row.visited.terminal_fee_atoms(),
            &RuntimeSha256,
        ))?;
    }
    let book = map_fee(book.complete(&RuntimeSha256))?;

    let authenticated_service = authenticate_treasury_service_ledger_v1(
        program_id,
        frame.treasury_service_ledger,
        treasury_derivation,
        true,
    )?;
    let service_body = authenticated_service.body();
    let service_evidence = TreasuryServiceAdmissionEvidenceV6 {
        realm: revenue_authority.realm(),
        market_instance_v2_id: treasury_derivation.market_instance_v2_id(),
        revenue_policy_record_account: revenue_authority.record_account(),
        revenue_policy_record_v2_id: revenue_authority.record_semantic_id(),
        revenue_policy_v2_digest: revenue_authority.policy_digest(),
        treasury_owner: revenue_authority.treasury_owner(),
        treasury_position_account: treasury_derivation.treasury_position_account(),
        treasury_service_ledger_account: authenticated_service.account(),
        epoch_semantic_id: Hash32::from_bytes(epoch_semantic_id.bytes()),
        admitted_epoch_count_before: service_body.admitted_epoch_count,
        settled_epoch_count_before: service_body.settled_epoch_count,
    };
    let service_transition = prepare_treasury_service_admission_v1(
        authenticated_service,
        treasury_derivation,
        &service_evidence,
    )?;
    let maker_sum = rows.iter().try_fold(0u64, |sum, row| {
        sum.checked_add(row.maker_rebate_atoms)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
    })?;
    require(
        maker_sum == split.maker_rebate_atoms
            && (rows.is_empty()) == (summary.collected_fee_atoms() == 0),
        ClutchError::MismatchedState,
    )?;
    Ok(Box::new(PreparedCandidateFeeCreationV6 {
        selected,
        revenue_policy,
        book,
        rows,
        transcript,
        owner_order_set_digest: FeeId(summary.owner_order_set_digest().bytes()),
        traversed_owner_count: summary.traversed_owner_count(),
        nonzero_weight_row_count: summary.nonzero_weight_row_count(),
        maker_rebate_total: split.maker_rebate_atoms,
        executor_atoms: split.executor_atoms,
        treasury_atoms: split.treasury_atoms,
        collected_fee_atoms: summary.collected_fee_atoms(),
        service_transition,
    }))
}
/// Stream exact Hamilton rows directly into the already-created 0x85/v3.
pub(crate) fn encode_candidate_recipient_v6(
    prepared: &PreparedCandidateFeeCreationV6,
    output: &mut [u8],
    rent: clutch_general_v2_contract::DeletableRentOwnerV1,
    stored_bump: u8,
) -> Outcome<()> {
    encode_recipient_allocation_v3_account_from_access(
        prepared,
        rent,
        stored_bump,
        output,
    )
    .map_err(Refusal::Codec)
}

/// Consume the completed book and construct the current accumulator after the
/// recipient outer's exact data identity is known.
#[allow(clippy::too_many_arguments)]
pub(crate) fn complete_candidate_fee_creation_v6<C: CertifiedRecipientAllocationAccessV3 + ?Sized>(
    program_id: &Pubkey,
    prepared: PreparedCandidateFeeCreationV6,
    settlement_root: Id32,
    selected_feed_data_id: Id32,
    recipient_account: Id32,
    recipient_data_id: Id32,
    treasury_ledger: Id32,
    settlement_cash_pot: Id32,
    accumulator_rent: clutch_general_v2_contract::DeletableRentOwnerV1,
    accumulator_bump: u8,
    certified: &C,
) -> Outcome<(FeeRetirementAccumulatorV1AccountV1, PreparedTreasuryServiceTransitionV1)> {
    let runtime_release = fee_runtime_semantic_release_id_v2(&RuntimeSha256)?;
    let accumulator = map_fee(FeeRetirementAccumulatorV1::begin_streaming(
        FeeId(program_id.to_bytes()),
        FeeId(runtime_release.bytes()),
        FeeId(settlement_root.bytes()),
        FeeId(selected_feed_data_id.bytes()),
        FeeId(recipient_account.bytes()),
        FeeId(recipient_data_id.bytes()),
        FeeId(treasury_ledger.bytes()),
        FeeId(settlement_cash_pot.bytes()),
        &prepared.selected,
        prepared.book,
        certified,
        &RuntimeSha256,
    ))?;
    Ok((
        FeeRetirementAccumulatorV1AccountV1 {
            semantic: accumulator,
            rent: accumulator_rent,
            stored_bump: accumulator_bump,
        },
        prepared.service_transition,
    ))
}

pub(crate) fn accept_candidate_fee_service_admission_v6(
    service_ledger: &AccountInfo<'_>,
    transition: PreparedTreasuryServiceTransitionV1,
) -> Outcome<()> {
    accept_treasury_service_transition_v1(service_ledger, transition)?;
    Ok(())
}

pub(crate) fn treasury_ledger_v6(
    selected: &SelectedCompositeFeeV2,
) -> Outcome<TreasuryLedgerV1> {
    map_fee(map_fee(TreasuryLedgerV1::admit(selected))?.begin_epoch(selected.fee_record()))
}

pub(crate) fn derive_root_expectation_v6(
    traversal: &dyn SettlementTraversalAccessV5,
    selected: &SelectedCompositeFeeV2,
    certified: clutch_fee_runtime_contract::codec::CertifiedRecipientAllocationSummaryV3,
) -> Outcome<SettlementRootExpectationProjectionV1> {
    clutch_general_v2_runtime::derive_settlement_root_expectation_from_certified_fee_v3(
        traversal.projection(),
        selected,
        certified,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}
