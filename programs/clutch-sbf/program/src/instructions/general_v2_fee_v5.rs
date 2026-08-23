//! Current counted-root fee-bearing owner evidence for General settlement.
//!
//! This module authenticates the immutable selected fee record, terminal
//! owner carry, persisted payer-allocation snapshot, and both policy
//! preimages before entering the fee semantic owner's V4 projection. It then
//! composes those facts with the canonical counted-root V5 row, Position,
//! Replay prestate, and cash-pot realization. It does not freeze General's
//! positional account ABI or perform writes; the General handler owns the
//! atomic account mutation and root-counter capability.
//!
//! The returned evidence proves fee selection and payer allocation only.  In
//! particular it proves no current Reservation balance, Position cash,
//! collateral custody, Hoard principal, rent funding, future revenue, or
//! liveness capitalization.

use core::cell::Ref;

use clutch_batch_policy_identity::revenue_policy_v1::{
    revenue_policy_digest, RevenuePolicyV1,
};
use clutch_batch_policy_identity::{
    batch_policy_digest, decode_batch_policy, Identity32V1, BATCH_POLICY_BYTES,
};
use clutch_fee_runtime_contract::intent::{
    FeeRecordAccountIdV1, OwnerFeeCarryAccountIdV1, OwnerFeeTransitionIntentV1,
    OwnerSettlementAccountIdV1, PayerAllocationAccountIdV1,
};
use clutch_fee_runtime_contract::projection::{
    project_pre_row_owner_fee_v4, reauthenticate_persisted_payer_allocation_snapshot_v1,
    AuthenticatedSelectedOwnerFeeV4,
};
use clutch_fee_runtime_contract::Id as FeeId;
use clutch_fee_runtime_contract::selected::SelectedCompositeFeeV1;
use clutch_general_v2_contract::{
    payer_allocation_account_data_id_v1, project_general_replay_transition_v1,
    GeneralPositionReplayPrestateV1, GeneralReplayTransitionKindV1,
    GeneralReplayTransitionPlanV1, Id32, OwnerFeeCarryV1AccountV1, DeletableRentOwnerV1,
    OwnerSettlementSeedTupleV5, OwnerSettlementV5AccountV1, PayerAllocationV1AccountV1,
    SelectedFeeRecordV1AccountV1, SettlementRootChildStateV1, SettlementRootPhaseV1,
    SettlementRootV1AccountV1, Sha256BackendV1,
    OWNER_FEE_CARRY_ACCOUNT_BYTES, PAYER_ALLOCATION_ACCOUNT_BYTES,
    SELECTED_FEE_RECORD_ACCOUNT_BYTES, OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
};
use clutch_general_v2_runtime::{
    derive_root_owner_basis_v4, derive_zero_fee_owner_finalization_evidence_v5,
    prepare_realize_owner_cash_v5,
    project_owner_settlement_account_v5, CandidateEntitlementProjectionV4,
    OwnerCashRealizationPlanV5, OwnerRowFeeEvidenceV4, OwnerSettlementAccountProjectionV5,
    OwnerSettlementAccountViewV5, SettlementTraversalProjectionV4,
};
use clutch_owner_settlement::{
    OwnerSettlementExpectationBasisV4, OwnerSettlementExpectationV4, OwnerSettlementStateV4,
    SelectedOwnerFeeV1, SettlementCashPotV1,
};
use clutch_retirement::{PositionV3Sha256Backend, ReplayV3HashBackend};
use clutch_solana_layout::revenue::{RevenuePolicyRecordV1, REVENUE_POLICY_RECORD_BYTES};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::general_v2_settlement_root::AuthenticatedGeneralSettlementRootV1;
use crate::seeds;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeSha256;

impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl PositionV3Sha256Backend for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for RuntimeSha256 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

/// Named account frame shared by current action-24 and action-38 composers.
///
/// The struct deliberately defines no positional ABI.  The eventual General
/// handler must select these roles from its own frozen action account list.
#[derive(Clone, Copy, Debug)]
pub struct OwnerFeeSnapshotAccountFrameV5<'a, 'info> {
    /// Fresh or existing canonical rent-owned owner-row successor.
    pub owner_row: &'a AccountInfo<'info>,
    /// Immutable candidate-wide selected composite-fee record.
    pub selected_fee_record: &'a AccountInfo<'info>,
    /// Terminal owner-scoped exact-rational carry.
    pub owner_fee_carry: &'a AccountInfo<'info>,
    /// Persisted payer-allocation snapshot.
    pub payer_allocation: &'a AccountInfo<'info>,
    /// Exact immutable batch-policy artifact bytes.
    pub batch_policy: &'a AccountInfo<'info>,
    /// Realm-owned immutable revenue-policy record.
    pub revenue_policy_record: &'a AccountInfo<'info>,
}

/// Presence-explicit fee account input selected only from the counted root.
#[derive(Clone, Copy, Debug)]
pub enum OwnerFeeAccountInputV5<'a, 'info> {
    /// A live root fee record requires the complete real account graph and its
    /// registered revenue-policy preimage.
    CandidateFee {
        /// Exact named fee account frame; no remaining-account tail.
        frame: OwnerFeeSnapshotAccountFrameV5<'a, 'info>,
        /// Registered immutable policy preimage whose digest is rederived.
        revenue_policy: &'a RevenuePolicyV1,
    },
    /// An absent root fee record carries no placeholder accounts.
    NoFeeRecord,
}

/// How the caller will use the authenticated snapshot accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerFeeSnapshotUseV5 {
    /// Action 24 reads the terminal fee snapshot while creating a row.
    Materialize,
    /// Action 38 consumes the payer snapshot and transitions the carry.
    Finalize,
}

/// Root/traversal-derived immutable owner facts.
///
/// Fields stay private so hostile callers cannot manufacture an owner basis
/// and then ask this module to bless it.  Constructors live beside the exact
/// action-24/action-38 root binders once the rent-owned row successor is
/// integrated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RootDerivedOwnerFeeContextV5<'a> {
    root_account: Id32,
    root: &'a SettlementRootV1AccountV1,
    realm: Id32,
    owner_row: Id32,
    basis: OwnerSettlementExpectationBasisV4,
}

impl<'a> RootDerivedOwnerFeeContextV5<'a> {
    fn new(
        root_account: Id32,
        root: &'a SettlementRootV1AccountV1,
        realm: Id32,
        owner_row: Id32,
        basis: OwnerSettlementExpectationBasisV4,
    ) -> Outcome<Self> {
        require(
            !root_account.is_zero()
                && !realm.is_zero()
                && !owner_row.is_zero()
                && root.market().bytes() == basis.market()
                && root.epoch().bytes() == basis.epoch()
                && root.settlement_candidate_id().bytes() == basis.candidate()
                && root.owner_order_set_digest().bytes() == basis.owner_order_set_digest()
                && root.counts().expected_owner_rows != 0,
            ClutchError::MismatchedState,
        )?;
        Ok(Self {
            root_account,
            root,
            realm,
            owner_row,
            basis,
        })
    }
}

/// Private-construction fee evidence ready for one General atomic composer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedOwnerFeeSnapshotV5 {
    root_account: Id32,
    owner_row: Id32,
    selected_fee: AuthenticatedSelectedOwnerFeeV4,
}

/// Exact canonical absence proof for one zero-fee owner row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedNoFeeOwnerV5 {
    root_account: Id32,
    owner_row: Id32,
    expectation: OwnerSettlementExpectationV4,
}

impl PreparedNoFeeOwnerV5 {
    /// Exact authenticated counted SettlementRoot account.
    pub const fn settlement_root_account(&self) -> Id32 {
        self.root_account
    }

    /// Exact root-derived owner-row successor address.
    pub const fn owner_row_account(&self) -> Id32 {
        self.owner_row
    }

    /// Canonical root-derived owner expectation sealed with zero fee.
    pub const fn expectation(&self) -> OwnerSettlementExpectationV4 {
        self.expectation
    }
}

/// Disjoint fee-bearing or canonical no-fee evidence for one owner row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedOwnerFeeEvidenceV5 {
    /// A real selected fee record and all three owner/candidate fee accounts exist.
    CandidateFee(PreparedOwnerFeeSnapshotV5),
    /// The counted root says the candidate has no fee record; no phantom fee
    /// account appears in this variant.
    NoFeeRecord(PreparedNoFeeOwnerV5),
}

/// Root-bound allocation-only result for one action-24 V5 row creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedOwnerFeeAction24V5 {
    owner_row_seed: OwnerSettlementSeedTupleV5,
    owner_row_bump: u8,
    evidence: PreparedOwnerFeeEvidenceV5,
}

/// Root-bound authenticated V5 owner-row/fee prestate for action 38.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedOwnerFeeAction38V5 {
    owner_row: OwnerSettlementAccountProjectionV5,
    evidence: PreparedOwnerFeeEvidenceV5,
}

impl PreparedOwnerFeeAction38V5 {
    /// Canonical writable V5 owner-row PDA.
    pub const fn owner_row_account(&self) -> Id32 {
        self.owner_row.account()
    }

    /// Exact hostile-byte-decoded V5 outer and V4 semantic prestate.
    pub const fn owner_row(&self) -> OwnerSettlementV5AccountV1 {
        self.owner_row.envelope()
    }

    /// Canonical full-rent V5 account projection consumed by pure settlement.
    pub const fn owner_row_projection(&self) -> &OwnerSettlementAccountProjectionV5 {
        &self.owner_row
    }

    /// Complete-data identity of the exact 340-byte owner-row prestate.
    pub const fn owner_row_prestate_data_id(&self) -> Id32 {
        self.owner_row.data_id()
    }

    /// Exact root/policy/payer evidence with an explicit no-fee branch.
    pub const fn evidence(&self) -> PreparedOwnerFeeEvidenceV5 {
        self.evidence
    }

    /// Complete payer-prestate data identity only when a real fee record exists.
    pub const fn payer_allocation_data_id(&self) -> Option<FeeId> {
        match self.evidence {
            PreparedOwnerFeeEvidenceV5::CandidateFee(value) => {
                Some(value.payer_allocation_data_id())
            }
            PreparedOwnerFeeEvidenceV5::NoFeeRecord(_) => None,
        }
    }
}

/// Complete current semantic action-38 composition before atomic SBF writes.
///
/// The General handler must atomically apply the returned GEN1 Replay and, for
/// the candidate-fee branch only, replace the carry with its `0x83/2` terminal
/// receipt while deleting the payer snapshot. This plan already owns the exact
/// row, Position, pot, Replay, and counted-root successors; it cannot be
/// constructed from a caller balance summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedOwnerSettlementAction38V5 {
    fee: PreparedOwnerFeeAction38V5,
    realization: OwnerCashRealizationPlanV5,
    transition_evidence_id: Id32,
    replay: GeneralReplayTransitionPlanV1,
}

impl PreparedOwnerSettlementAction38V5 {
    /// Exact current V5 fee and row prestate.
    pub const fn fee(&self) -> &PreparedOwnerFeeAction38V5 {
        &self.fee
    }

    /// Canonical V5 row/Position/pot/root successor plan.
    pub const fn realization(&self) -> &OwnerCashRealizationPlanV5 {
        &self.realization
    }

    /// Exact payer-prestate or plan-derived zero-fee evidence committed by GEN1.
    pub const fn transition_evidence_id(&self) -> Id32 {
        self.transition_evidence_id
    }

    /// Canonical purpose-owned Replay successor paired with the Position write.
    pub const fn replay(&self) -> GeneralReplayTransitionPlanV1 {
        self.replay
    }

    /// Whether this atomic transition must mint a real `0x83/2` successor.
    pub const fn fee_finalization_required(&self) -> bool {
        self.realization.fee_finalization_required()
    }

    /// Complete payer prestate ID for GEN1 and fee-finalization evidence.
    /// The zero-fee route has no payer account and returns `None`.
    pub const fn payer_allocation_data_id(&self) -> Option<FeeId> {
        self.fee.payer_allocation_data_id()
    }
}

impl PreparedOwnerFeeAction24V5 {
    /// Exact fresh V5 owner-row seed tuple.
    pub const fn owner_row_seed(&self) -> OwnerSettlementSeedTupleV5 {
        self.owner_row_seed
    }

    /// Canonical V5 owner-row bump.
    pub const fn owner_row_bump(&self) -> u8 {
        self.owner_row_bump
    }

    /// Exact root/policy/payer evidence with an explicit no-fee branch.
    pub const fn evidence(&self) -> PreparedOwnerFeeEvidenceV5 {
        self.evidence
    }

    /// Dependency-ready input for the pure General row materializer.
    pub const fn owner_row_fee_evidence(&self) -> OwnerRowFeeEvidenceV4 {
        match self.evidence {
            PreparedOwnerFeeEvidenceV5::CandidateFee(value) => {
                OwnerRowFeeEvidenceV4::CandidateFee(value.selected_fee())
            }
            PreparedOwnerFeeEvidenceV5::NoFeeRecord(_) => OwnerRowFeeEvidenceV4::NoFeeRecord,
        }
    }
}

impl PreparedOwnerFeeEvidenceV5 {
    /// Exact authenticated counted SettlementRoot account.
    pub const fn settlement_root_account(&self) -> Id32 {
        match self {
            Self::CandidateFee(value) => value.settlement_root_account(),
            Self::NoFeeRecord(value) => value.settlement_root_account(),
        }
    }

    /// Exact root-derived owner-row successor address.
    pub const fn owner_row_account(&self) -> Id32 {
        match self {
            Self::CandidateFee(value) => value.owner_row_account(),
            Self::NoFeeRecord(value) => value.owner_row_account(),
        }
    }

    /// Exact sealed owner expectation in either branch.
    pub const fn expectation(&self) -> OwnerSettlementExpectationV4 {
        match self {
            Self::CandidateFee(value) => value.selected_fee().expectation(),
            Self::NoFeeRecord(value) => value.expectation(),
        }
    }

    /// Whether action 38 must consume real carry/payer state and mint the
    /// fee-finalization successor.
    pub const fn fee_record_present(&self) -> bool {
        matches!(self, Self::CandidateFee(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpectedSelectedFeeBindingV5 {
    fee_record: Id32,
    realm: Id32,
    market: Id32,
    epoch: Id32,
    candidate: Id32,
    batch_policy: Id32,
    revenue_policy: Id32,
    price_scale: u64,
    outcome_count: u8,
}

impl PreparedOwnerFeeSnapshotV5 {
    /// Exact authenticated counted SettlementRoot account.
    pub const fn settlement_root_account(&self) -> Id32 {
        self.root_account
    }

    /// Exact root-derived owner-row successor address.
    pub const fn owner_row_account(&self) -> Id32 {
        self.owner_row
    }

    /// Allocation-only candidate-fee evidence for the pure General composer.
    pub const fn selected_fee(&self) -> AuthenticatedSelectedOwnerFeeV4 {
        self.selected_fee
    }

    /// Exact complete-data identity of the temporary payer outer.
    pub const fn payer_allocation_data_id(&self) -> FeeId {
        self.selected_fee.payer_allocation_data_id()
    }
}

fn id(key: &Pubkey) -> Id32 {
    Id32::from_bytes(key.to_bytes())
}

fn fee_id(key: &Pubkey) -> FeeId {
    Identity32V1(key.to_bytes())
}

fn borrow_data<'a, 'info>(account: &'a AccountInfo<'info>) -> Outcome<Ref<'a, [u8]>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    Ok(Ref::map(data, |bytes| &**bytes))
}

fn require_read_only_program_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    exact_len: usize,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(account.data_len() == exact_len, ClutchError::WrongDataLength)
}

fn require_snapshot_state(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    exact_len: usize,
    writable: bool,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    if writable {
        require(account.is_writable, ClutchError::NotWritable)?;
    } else {
        require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    }
    require(account.data_len() == exact_len, ClutchError::WrongDataLength)
}

fn require_distinct_keys(keys: &[[u8; 32]; 7]) -> Outcome<()> {
    let mut left = 0usize;
    while left < keys.len() {
        let mut right = left + 1;
        while right < keys.len() {
            require(keys[left] != keys[right], ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn require_distinct_frame(
    root_account: Id32,
    frame: &OwnerFeeSnapshotAccountFrameV5<'_, '_>,
) -> Outcome<()> {
    require_distinct_keys(&[
        root_account.bytes(),
        frame.owner_row.key.to_bytes(),
        frame.selected_fee_record.key.to_bytes(),
        frame.owner_fee_carry.key.to_bytes(),
        frame.payer_allocation.key.to_bytes(),
        frame.batch_policy.key.to_bytes(),
        frame.revenue_policy_record.key.to_bytes(),
    ])
}

fn map_fee<T>(result: clutch_fee_runtime_contract::Result<T>) -> Outcome<T> {
    result.map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn prepare_no_fee_owner_v5(
    context: RootDerivedOwnerFeeContextV5<'_>,
) -> Outcome<PreparedOwnerFeeEvidenceV5> {
    let cash = context.root.cash_pot_expectation()?;
    require(
        context.root.fee_record_state() == SettlementRootChildStateV1::Absent
            && context.root.fee_record().is_zero()
            && cash.fee_record == [0; 32]
            && cash.selected_fee_atoms == 0,
        ClutchError::MismatchedState,
    )?;
    let expectation = context
        .basis
        .with_selected_fee(SelectedOwnerFeeV1 {
            owner: context.basis.owner(),
            fee_atoms: 0,
        })
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(PreparedOwnerFeeEvidenceV5::NoFeeRecord(
        PreparedNoFeeOwnerV5 {
            root_account: context.root_account,
            owner_row: context.owner_row,
            expectation,
        },
    ))
}

fn require_selected_fee_binding_v5(
    selected: &SelectedCompositeFeeV1,
    revenue_record: &RevenuePolicyRecordV1,
    expected: ExpectedSelectedFeeBindingV5,
) -> Outcome<()> {
    require(
        selected.fee_record().0 == expected.fee_record.bytes()
            && selected.realm().0 == expected.realm.bytes()
            && selected.market().0 == expected.market.bytes()
            && selected.epoch().0 == expected.epoch.bytes()
            && selected.selected_candidate().0 == expected.candidate.bytes()
            && selected.batch_policy().0 == expected.batch_policy.bytes()
            && selected.revenue_policy().0 == expected.revenue_policy.bytes()
            && selected.price_scale() == expected.price_scale
            && selected.outcome_count() == expected.outcome_count
            && revenue_record.realm.bytes() == expected.realm.bytes()
            && revenue_record.policy_digest.bytes() == expected.revenue_policy.bytes()
            && revenue_record.treasury.bytes() == selected.treasury_owner().0,
        ClutchError::MismatchedState,
    )
}

fn require_owner_row_rent_v5(
    rent: DeletableRentOwnerV1,
    owner_row: Id32,
    settlement_root: Id32,
    current_lamports: u64,
) -> Outcome<()> {
    rent.validate()?;
    let recorded_balance_floor = rent
        .refundable_principal
        .checked_add(rent.donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        rent.payer != owner_row
            && rent.payer != settlement_root
            && current_lamports >= recorded_balance_floor,
        ClutchError::MismatchedState,
    )
}

#[inline(never)]
fn authenticate_owner_fee_snapshot_v5(
    program_id: &Pubkey,
    context: RootDerivedOwnerFeeContextV5<'_>,
    frame: OwnerFeeSnapshotAccountFrameV5<'_, '_>,
    revenue_policy: &RevenuePolicyV1,
    usage: OwnerFeeSnapshotUseV5,
) -> Outcome<PreparedOwnerFeeSnapshotV5> {
    require_distinct_frame(context.root_account, &frame)?;
    require(!frame.owner_row.executable, ClutchError::ExecutableAccount)?;
    require(frame.owner_row.is_writable, ClutchError::NotWritable)?;
    require(
        id(frame.owner_row.key) == context.owner_row,
        ClutchError::MismatchedState,
    )?;

    require_read_only_program_state(
        program_id,
        frame.selected_fee_record,
        SELECTED_FEE_RECORD_ACCOUNT_BYTES,
    )?;
    require_read_only_program_state(program_id, frame.batch_policy, BATCH_POLICY_BYTES)?;
    require_read_only_program_state(
        program_id,
        frame.revenue_policy_record,
        REVENUE_POLICY_RECORD_BYTES,
    )?;
    let snapshot_writable = usage == OwnerFeeSnapshotUseV5::Finalize;
    require_snapshot_state(
        program_id,
        frame.owner_fee_carry,
        OWNER_FEE_CARRY_ACCOUNT_BYTES,
        snapshot_writable,
    )?;
    require_snapshot_state(
        program_id,
        frame.payer_allocation,
        PAYER_ALLOCATION_ACCOUNT_BYTES,
        snapshot_writable,
    )?;

    let batch = decode_batch_policy(&borrow_data(frame.batch_policy)?)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let batch_id = batch_policy_digest(&batch)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expect_pda(
        frame.batch_policy.key,
        seeds::batch_policy_pda(
            program_id,
            &context.root.epoch().bytes(),
            &batch_id.0,
        ),
        None,
    )?;

    let revenue_digest = revenue_policy_digest(revenue_policy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let revenue_record = RevenuePolicyRecordV1::decode(&borrow_data(frame.revenue_policy_record)?)?;
    expect_pda(
        frame.revenue_policy_record.key,
        seeds::revenue_policy_pda(program_id, &revenue_record.realm.bytes()),
        Some(revenue_record.stored_bump),
    )?;

    let selected_account = SelectedFeeRecordV1AccountV1::decode(
        &borrow_data(frame.selected_fee_record)?,
        &batch,
        revenue_policy,
    )?;
    let selected = selected_account.semantic;
    expect_pda(
        frame.selected_fee_record.key,
        seeds::general_v2_selected_fee_record_pda(
            program_id,
            &context.root.settlement_candidate_id().bytes(),
        ),
        Some(selected_account.stored_bump),
    )?;
    let cash_expectation = context.root.cash_pot_expectation()?;
    require(
        context.root.phase() == SettlementRootPhaseV1::Materializing
            || context.root.phase() == SettlementRootPhaseV1::Settling,
        ClutchError::MismatchedState,
    )?;
    require(
        context.root.fee_record_state() == SettlementRootChildStateV1::Live
            && context.root.fee_record().bytes() == frame.selected_fee_record.key.to_bytes()
            && selected.batch_policy() == batch_id
            && selected.batch_policy().0 == context.root.batch_policy_id().bytes()
            && selected.revenue_policy() == revenue_digest
            && cash_expectation.fee_record == frame.selected_fee_record.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    require_selected_fee_binding_v5(
        &selected,
        &revenue_record,
        ExpectedSelectedFeeBindingV5 {
            fee_record: id(frame.selected_fee_record.key),
            realm: context.realm,
            market: context.root.market(),
            epoch: context.root.epoch(),
            candidate: context.root.settlement_candidate_id(),
            batch_policy: context.root.batch_policy_id(),
            revenue_policy: Id32::from_bytes(revenue_digest.0),
            price_scale: cash_expectation.price_scale,
            outcome_count: context.root.outcome_count(),
        },
    )?;

    let carry_account = OwnerFeeCarryV1AccountV1::decode(
        &borrow_data(frame.owner_fee_carry)?,
        &selected,
    )?;
    let owner = carry_account.semantic.owner();
    expect_pda(
        frame.owner_fee_carry.key,
        seeds::general_v2_owner_fee_carry_pda(
            program_id,
            &selected.fee_record().0,
            &owner.0,
        ),
        Some(carry_account.stored_bump),
    )?;
    let payer_account =
        PayerAllocationV1AccountV1::decode_persisted(&borrow_data(frame.payer_allocation)?)?;
    expect_pda(
        frame.payer_allocation.key,
        seeds::general_v2_payer_allocation_pda(
            program_id,
            &selected.fee_record().0,
            &owner.0,
        ),
        Some(payer_account.stored_bump),
    )?;

    require(
        context.basis.owner() == owner.0,
        ClutchError::MismatchedState,
    )?;
    let transition = map_fee(OwnerFeeTransitionIntentV1::bind(
        &selected,
        owner,
        map_fee(FeeRecordAccountIdV1::admit(fee_id(
            frame.selected_fee_record.key,
        )))?,
        map_fee(OwnerFeeCarryAccountIdV1::admit(fee_id(
            frame.owner_fee_carry.key,
        )))?,
        map_fee(PayerAllocationAccountIdV1::admit(fee_id(
            frame.payer_allocation.key,
        )))?,
        map_fee(OwnerSettlementAccountIdV1::admit(fee_id(
            frame.owner_row.key,
        )))?,
    ))?;
    let payer_data = borrow_data(frame.payer_allocation)?;
    let payer_data_id = payer_allocation_account_data_id_v1(&payer_data, &RuntimeSha256)?;
    drop(payer_data);
    let snapshot = map_fee(reauthenticate_persisted_payer_allocation_snapshot_v1(
        &selected,
        &transition,
        &carry_account.semantic,
        &payer_account.semantic,
        payer_data_id,
    ))?;
    let selected_fee = map_fee(project_pre_row_owner_fee_v4(
        &selected,
        fee_id(frame.owner_row.key),
        context.basis,
        snapshot,
    ))?;
    require(
        selected_fee.expectation().owner_order_set_digest()
            == context.root.owner_order_set_digest().bytes(),
        ClutchError::MismatchedState,
    )?;
    Ok(PreparedOwnerFeeSnapshotV5 {
        root_account: context.root_account,
        owner_row: context.owner_row,
        selected_fee,
    })
}

fn prepare_root_owner_fee_evidence_v5(
    program_id: &Pubkey,
    context: RootDerivedOwnerFeeContextV5<'_>,
    accounts: OwnerFeeAccountInputV5<'_, '_>,
    usage: OwnerFeeSnapshotUseV5,
) -> Outcome<PreparedOwnerFeeEvidenceV5> {
    match accounts {
        OwnerFeeAccountInputV5::CandidateFee {
            frame,
            revenue_policy,
        } => Ok(PreparedOwnerFeeEvidenceV5::CandidateFee(
            authenticate_owner_fee_snapshot_v5(
                program_id,
                context,
                frame,
                revenue_policy,
                usage,
            )?,
        )),
        OwnerFeeAccountInputV5::NoFeeRecord => prepare_no_fee_owner_v5(context),
    }
}

/// Prepare fee evidence for one fresh rent-owned V5 owner row in action 24.
///
/// `entitlement` is privately constructed from the complete retained Feed and
/// frozen V5 page traversal.  The separately authenticated SBF root must equal
/// its exact root prestate.  This function derives the V5 row PDA itself and
/// returns no SettlementRoot counter successor; the General action-24
/// composer remains the sole owner of that atomic write.
pub fn prepare_owner_fee_action24_v5(
    program_id: &Pubkey,
    authenticated_root: &AuthenticatedGeneralSettlementRootV1,
    entitlement: &CandidateEntitlementProjectionV4,
    owner: Id32,
    owner_row: &AccountInfo<'_>,
    fee_accounts: OwnerFeeAccountInputV5<'_, '_>,
) -> Outcome<PreparedOwnerFeeAction24V5> {
    require(
        authenticated_root.account() == entitlement.settlement_root_account()
            && authenticated_root.root() == entitlement.settlement_root()
            && authenticated_root.root().phase() == SettlementRootPhaseV1::Materializing,
        ClutchError::MismatchedState,
    )?;
    let basis = derive_root_owner_basis_v4(
        authenticated_root.account(),
        authenticated_root.root(),
        entitlement.traversal(),
        owner,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let owner_row_seed = OwnerSettlementSeedTupleV5::new(
        authenticated_root.root().epoch(),
        authenticated_root.root().settlement_candidate_id(),
        owner,
    )?;
    let derived = seeds::find(
        program_id,
        &[
            owner_row_seed.domain(),
            owner_row_seed.epoch(),
            owner_row_seed.settlement_candidate(),
            owner_row_seed.owner(),
        ],
    );
    require(!owner_row.executable, ClutchError::ExecutableAccount)?;
    require(owner_row.is_writable, ClutchError::NotWritable)?;
    require(*owner_row.key == derived.0, ClutchError::WrongPda)?;
    let context = RootDerivedOwnerFeeContextV5::new(
        authenticated_root.account(),
        authenticated_root.root(),
        Id32::from_bytes(entitlement.position_market_binding().realm_id.bytes()),
        id(owner_row.key),
        basis,
    )?;
    let evidence = prepare_root_owner_fee_evidence_v5(
        program_id,
        context,
        fee_accounts,
        OwnerFeeSnapshotUseV5::Materialize,
    )?;
    Ok(PreparedOwnerFeeAction24V5 {
        owner_row_seed,
        owner_row_bump: derived.1,
        evidence,
    })
}

/// Authenticate the rent-owned owner-row and fee prestates for action 38.
///
/// The traversal must be the exhaustive retained-Feed/V5-page projection
/// bound to the independently authenticated counted root.  This function
/// requires the exact accounting-complete row and complete merge-delivery
/// latch, but it does not inspect or mutate Position, Replay, cash pot, fee
/// finalization, or root counters.  The General composer must atomically join
/// those semantic owners before any write or close.
pub fn prepare_owner_fee_action38_v5(
    program_id: &Pubkey,
    authenticated_root: &AuthenticatedGeneralSettlementRootV1,
    traversal: &SettlementTraversalProjectionV4,
    owner_row: &AccountInfo<'_>,
    owner_row_rent_minimum: u64,
    fee_accounts: OwnerFeeAccountInputV5<'_, '_>,
) -> Outcome<PreparedOwnerFeeAction38V5> {
    require(
        authenticated_root.root().phase() == SettlementRootPhaseV1::Settling,
        ClutchError::MismatchedState,
    )?;
    require(owner_row.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!owner_row.executable, ClutchError::ExecutableAccount)?;
    require(owner_row.is_writable, ClutchError::NotWritable)?;
    require(
        owner_row.data_len() == OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
        ClutchError::WrongDataLength,
    )?;
    require(owner_row_rent_minimum != 0, ClutchError::MismatchedState)?;
    let owner_row_data = borrow_data(owner_row)?;
    let decoded = OwnerSettlementV5AccountV1::decode(&owner_row_data)?;
    let expectation = decoded.semantic.expectation();
    let owner = Id32::from_bytes(expectation.owner());
    let owner_row_seed = OwnerSettlementSeedTupleV5::new(
        authenticated_root.root().epoch(),
        authenticated_root.root().settlement_candidate_id(),
        owner,
    )?;
    let derived_owner_row = seeds::find(
        program_id,
        &[
            owner_row_seed.domain(),
            owner_row_seed.epoch(),
            owner_row_seed.settlement_candidate(),
            owner_row_seed.owner(),
        ],
    );
    expect_pda(
        owner_row.key,
        derived_owner_row,
        Some(decoded.stored_bump),
    )?;
    require_owner_row_rent_v5(
        decoded.rent,
        id(owner_row.key),
        authenticated_root.account(),
        owner_row.lamports(),
    )?;
    let owner_row_projection = project_owner_settlement_account_v5(
        OwnerSettlementAccountViewV5 {
            account: id(owner_row.key),
            program_owner: id(owner_row.owner),
            exact_body: &owner_row_data,
            lamports: owner_row.lamports(),
            rent_minimum: owner_row_rent_minimum,
            canonical_bump: derived_owner_row.1,
            writable: owner_row.is_writable,
        },
        id(program_id),
        owner_row_seed,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(owner_row_data);
    require(
        decoded.semantic.state() == OwnerSettlementStateV4::AccountingComplete
            && decoded
                .semantic
                .merge_delivered_count()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == expectation.expected_merge_delivery_count(),
        ClutchError::MismatchedState,
    )?;
    let basis = derive_root_owner_basis_v4(
        authenticated_root.account(),
        authenticated_root.root(),
        traversal,
        owner,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let context = RootDerivedOwnerFeeContextV5::new(
        authenticated_root.account(),
        authenticated_root.root(),
        Id32::from_bytes(traversal.position_market_binding().realm_id.bytes()),
        id(owner_row.key),
        basis,
    )?;
    let evidence = prepare_root_owner_fee_evidence_v5(
        program_id,
        context,
        fee_accounts,
        OwnerFeeSnapshotUseV5::Finalize,
    )?;
    require(
        evidence.expectation() == expectation,
        ClutchError::MismatchedState,
    )?;
    Ok(PreparedOwnerFeeAction38V5 {
        owner_row: owner_row_projection,
        evidence,
    })
}

/// Compose the current V5 action-38 row/Position/pot/root transition.
///
/// `position_replay` and `pot_before` must come from General's exact account
/// loaders. This function re-enters the dependency-lower V5 realization and
/// exact-joins its selected fee with the authenticated payer-or-absence
/// branch. It creates no zero-fee carry or payer identity and performs no
/// write, close, transfer, or SettlementRoot counter mutation on its own.
#[allow(clippy::too_many_arguments)]
pub fn compose_owner_settlement_action38_v5(
    program_id: &Pubkey,
    authenticated_root: &AuthenticatedGeneralSettlementRootV1,
    traversal: &SettlementTraversalProjectionV4,
    owner_row: &AccountInfo<'_>,
    owner_row_rent_minimum: u64,
    fee_accounts: OwnerFeeAccountInputV5<'_, '_>,
    settlement_cash_pot_account: Id32,
    position_replay: GeneralPositionReplayPrestateV1,
    pot_before: SettlementCashPotV1,
) -> Outcome<PreparedOwnerSettlementAction38V5> {
    let fee = prepare_owner_fee_action38_v5(
        program_id,
        authenticated_root,
        traversal,
        owner_row,
        owner_row_rent_minimum,
        fee_accounts,
    )?;
    let realization = prepare_realize_owner_cash_v5(
        authenticated_root.account(),
        authenticated_root.root(),
        traversal,
        fee.owner_row_projection(),
        settlement_cash_pot_account,
        position_replay,
        pot_before,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let transition_evidence_id = match fee.payer_allocation_data_id() {
        Some(value) => Id32::from_bytes(value.0),
        None => derive_zero_fee_owner_finalization_evidence_v5(&realization)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
    };
    let replay = project_general_replay_transition_v1(
        position_replay,
        realization.position(),
        GeneralReplayTransitionKindV1::FinalizeOwnerSettlement,
        realization.finalized_owner_row_data_id(),
        transition_evidence_id,
        &RuntimeSha256,
    )?;
    let expected_fee_atoms = fee.evidence().expectation().selected_fee_atoms();
    require(
        realization.settlement_root_account() == authenticated_root.account()
            && realization.owner_settlement_account() == fee.owner_row_account()
            && realization.owner_settlement_prestate_data_id()
                == fee.owner_row_prestate_data_id()
            && realization.semantic().expectation() == fee.evidence().expectation()
            && realization.disposition().selected_fee_atoms() == expected_fee_atoms
            && realization.fee_finalization_required()
                == fee.evidence().fee_record_present()
            && fee.payer_allocation_data_id().is_some()
                == realization.fee_finalization_required(),
        ClutchError::MismatchedState,
    )?;
    Ok(PreparedOwnerSettlementAction38V5 {
        fee,
        realization,
        transition_evidence_id,
        replay,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_batch::relation_v1::{
        AllocationPolicyV1, AonPolicyV1, FeeBaseV1, FrozenPolicyV1,
        PairingWitnessPolicyV1, PortfolioLotPolicyV1, ResidualSettlementV1,
        RoundingBoundaryV1, ScorePolicyV1, SelfCrossPolicyV1, TransferPhaseV1,
    };
    use clutch_batch::DustPolicy;
    use clutch_batch_policy_identity::revenue_policy_v1::{
        LamportSinkV1, RevenueResidualV1, StandingMakerV1, REVENUE_POLICY_SCHEMA_V1,
    };
    use clutch_solana_layout::Hash32;

    fn fee_id(byte: u8) -> FeeId {
        Identity32V1([byte; 32])
    }

    fn rated_policy() -> FrozenPolicyV1 {
        FrozenPolicyV1 {
            allocation: AllocationPolicyV1::PricePriorityMarginalProRata,
            self_cross: SelfCrossPolicyV1::RefuseOverlap,
            aon: AonPolicyV1::RefuseAdmission,
            rounding: RoundingBoundaryV1::TerminalOwnerFloor,
            residual_settlement: ResidualSettlementV1::UniqueSliceReceipts,
            transfer_phase: TransferPhaseV1::ActiveOrResolved,
            portfolio_lots: PortfolioLotPolicyV1::StrictWholeOrder,
            pairing_witness: PairingWitnessPolicyV1::ExplicitSlices,
            dust: DustPolicy::AssignCanonical,
            score: ScorePolicyV1::LexicographicDispersionV1,
            fee_base: FeeBaseV1::CompositeDispersionFloor {
                dispersion_bps: 25,
                floor_range_bps: 10,
            },
        }
    }

    fn revenue_policy() -> RevenuePolicyV1 {
        RevenuePolicyV1 {
            version: u32::from(REVENUE_POLICY_SCHEMA_V1),
            treasury: [9; 32],
            maker_rebate_num: 60,
            executor_num: 0,
            treasury_num: 40,
            split_den: 100,
            residual: RevenueResidualV1::Treasury,
            standing_maker: StandingMakerV1::AllRestingMakers,
            lamport_sink: LamportSinkV1::None,
        }
    }

    fn selected_and_binding() -> (
        SelectedCompositeFeeV1,
        RevenuePolicyRecordV1,
        ExpectedSelectedFeeBindingV5,
    ) {
        let batch = rated_policy();
        let revenue = revenue_policy();
        let selected = SelectedCompositeFeeV1::select(
            fee_id(1),
            fee_id(2),
            fee_id(3),
            fee_id(4),
            fee_id(5),
            fee_id(6),
            10_000,
            2,
            &batch,
            &revenue,
        )
        .unwrap();
        let revenue_id = revenue_policy_digest(&revenue).unwrap();
        let record = RevenuePolicyRecordV1 {
            realm: Hash32::from_bytes([2; 32]),
            policy_digest: Hash32::from_bytes(revenue_id.0),
            treasury: Hash32::from_bytes([9; 32]),
            terminal_payer: Hash32::from_bytes([10; 32]),
            terminal_payer_principal: 1,
            terminal_donation_floor: 0,
            terminal_generation: 1,
            stored_bump: 7,
            flags: 0,
        };
        let expected = ExpectedSelectedFeeBindingV5 {
            fee_record: Id32::from_bytes([1; 32]),
            realm: Id32::from_bytes([2; 32]),
            market: Id32::from_bytes([3; 32]),
            epoch: Id32::from_bytes([4; 32]),
            candidate: Id32::from_bytes([5; 32]),
            batch_policy: Id32::from_bytes(batch_policy_digest(&batch).unwrap().0),
            revenue_policy: Id32::from_bytes(revenue_id.0),
            price_scale: 10_000,
            outcome_count: 2,
        };
        (selected, record, expected)
    }

    #[test]
    fn exact_selected_policy_chain_is_accepted() {
        let (selected, record, expected) = selected_and_binding();
        assert_eq!(
            require_selected_fee_binding_v5(&selected, &record, expected),
            Ok(())
        );
    }

    #[test]
    fn substituted_batch_realm_or_treasury_is_refused() {
        let (selected, record, expected) = selected_and_binding();
        let mut wrong_batch = expected;
        wrong_batch.batch_policy = Id32::from_bytes([21; 32]);
        assert_eq!(
            require_selected_fee_binding_v5(&selected, &record, wrong_batch),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
        let mut wrong_realm = expected;
        wrong_realm.realm = Id32::from_bytes([22; 32]);
        assert_eq!(
            require_selected_fee_binding_v5(&selected, &record, wrong_realm),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
        let hostile_record = RevenuePolicyRecordV1 {
            treasury: Hash32::from_bytes([23; 32]),
            ..record
        };
        assert_eq!(
            require_selected_fee_binding_v5(&selected, &hostile_record, expected),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
    }

    #[test]
    fn fee_snapshot_roles_cannot_alias_root_row_or_policy() {
        let distinct = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32], [7; 32]];
        assert_eq!(require_distinct_keys(&distinct), Ok(()));
        let mut root_alias = distinct;
        root_alias[1] = root_alias[0];
        assert_eq!(
            require_distinct_keys(&root_alias),
            Err(Refusal::Adapter(ClutchError::AccountAlias))
        );
        let mut policy_alias = distinct;
        policy_alias[6] = policy_alias[3];
        assert_eq!(
            require_distinct_keys(&policy_alias),
            Err(Refusal::Adapter(ClutchError::AccountAlias))
        );
    }

    #[test]
    fn v5_rent_principal_and_prefund_are_not_fee_value() {
        let row = Id32::from_bytes([31; 32]);
        let root = Id32::from_bytes([32; 32]);
        let rent = DeletableRentOwnerV1 {
            payer: Id32::from_bytes([33; 32]),
            refundable_principal: 1_000,
            donation_floor: 40,
        };
        assert_eq!(require_owner_row_rent_v5(rent, row, root, 1_040), Ok(()));
        assert_eq!(
            require_owner_row_rent_v5(rent, row, root, 1_039),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
        assert_eq!(
            require_owner_row_rent_v5(
                DeletableRentOwnerV1 { payer: row, ..rent },
                row,
                root,
                1_040,
            ),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
    }
}
