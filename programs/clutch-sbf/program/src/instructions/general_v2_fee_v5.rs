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
    AuthenticatedSelectedOwnerFeeV4, CertifiedRecipientAllocationV2,
};
use clutch_fee_runtime_contract::Id as FeeId;
use clutch_fee_runtime_contract::selected::SelectedCompositeFeeV1;
use clutch_fee_runtime_contract::terminal::{
    CandidateFeeAccountRoleV1, ExternalFeeAccountClosureV1, FeeTerminalOutcomeV1,
    OwnerFeeFinalizationBindingsV2, OwnerFeeFinalizationReceiptV1,
};
use clutch_general_v2_contract::{
    fee_runtime_semantic_release_id_v1, payer_allocation_account_data_id_v1,
    prepare_owner_fee_rent_transition_v3, project_general_replay_transition_v1,
    recipient_allocation_account_data_id_v2,
    FeeLamportTransferV2,
    GeneralPositionReplayPrestateV1, GeneralReplayTransitionKindV1,
    GeneralReplayTransitionPlanV1, Id32, MarketBindingV2, OwnerFeeCarryV3AccountV1,
    OwnerFeeFinalizationV4AccountV1, OwnerFeeRentTransitionAccountsV3,
    OwnerFeeRentTransitionPlanV3, DeletableRentOwnerV1, OwnerSettlementSeedTupleV5,
    OwnerSettlementV5AccountV1, PayerAllocationV2AccountV1, SelectedFeeRecordV1AccountV1,
    RecipientAllocationV2AccountV1, SettlementRootChildStateV1, SettlementRootPhaseV1,
    SettlementRootV1AccountV1,
    Sha256BackendV1, MARKET_BINDING_ACCOUNT_BYTES_V2, OWNER_FEE_CARRY_ACCOUNT_BYTES_V3,
    OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4, OWNER_SETTLEMENT_ACCOUNT_BYTES_V5,
    PAYER_ALLOCATION_ACCOUNT_BYTES_V2, RECIPIENT_ALLOCATION_ACCOUNT_BYTES_V2,
    SELECTED_FEE_RECORD_ACCOUNT_BYTES,
};
use clutch_general_v2_runtime::{
    derive_root_owner_basis_v4, derive_settlement_root_expectation_from_certified_fee_v2,
    derive_zero_fee_owner_finalization_evidence_v5, prepare_realize_owner_cash_v5,
    project_owner_settlement_account_v5, CandidateEntitlementProjectionV4,
    OwnerCashRealizationPlanV5, OwnerRowFeeEvidenceV5, OwnerSettlementAccountProjectionV5,
    OwnerSettlementAccountViewV5, SettlementRootExpectationProjectionV1,
    SettlementTraversalProjectionV4,
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

/// Named native-rent accounts used only by fee-bearing action 38.
#[derive(Clone, Copy, Debug)]
pub struct OwnerFeeRentAccountFrameV5<'a, 'info> {
    /// Exact immutable MarketBinding V2 that owns the neutral sink.
    pub market_binding: &'a AccountInfo<'info>,
    /// Persisted carry rent payer and sole possible realloc top-up signer.
    pub carry_rent_payer: &'a AccountInfo<'info>,
    /// Persisted payer-snapshot rent principal recipient.
    pub payer_rent_refund_owner: &'a AccountInfo<'info>,
    /// MarketBinding-owned destination for donation and hostile surplus.
    pub neutral_sink: &'a AccountInfo<'info>,
}

/// Presence-explicit action-38 rent input.
#[derive(Clone, Copy, Debug)]
pub enum OwnerFeeTerminalRentInputV5<'a, 'info> {
    /// Exact native-rent graph and authenticated 548-byte rent minimum.
    CandidateFee {
        /// Named current fee-rent accounts.
        frame: OwnerFeeRentAccountFrameV5<'a, 'info>,
        /// Rent-sysvar minimum for the exact 0x83/v4 terminal width.
        carry_terminal_rent_minimum_lamports: u64,
    },
    /// Canonical zero-fee route with no fee rent accounts.
    NoFeeRecord,
}

/// Named immutable fee accounts for the action-39 complete-book join.
#[derive(Clone, Copy, Debug)]
pub struct CandidateFeeCollectionAccountFrameV5<'a, 'info> {
    /// Immutable selected composite-fee record.
    pub selected_fee_record: &'a AccountInfo<'info>,
    /// Immutable rent-owned certified recipient allocation.
    pub certified_recipient_allocation: &'a AccountInfo<'info>,
    /// Exact immutable batch-policy artifact bytes.
    pub batch_policy: &'a AccountInfo<'info>,
    /// Realm-owned immutable revenue-policy record.
    pub revenue_policy_record: &'a AccountInfo<'info>,
}

/// Additional writable roles used only by the candidate-wide terminal close.
///
/// The immutable policy/selected-record roles remain in [`Self::collection`].
/// The recipient account itself must be writable in this frame because the
/// terminal handler deletes it after applying both exact lamport transfers.
#[derive(Clone, Copy, Debug)]
pub struct CandidateFeeCollectionClosureAccountFrameV5<'a, 'info> {
    /// Same exact action-39 account graph, with the recipient role writable.
    pub collection: CandidateFeeCollectionAccountFrameV5<'a, 'info>,
    /// Counted-root-selected immutable MarketBinding V2.
    pub market_binding: &'a AccountInfo<'info>,
    /// Persisted recipient-account rent-principal owner.
    pub rent_refund_owner: &'a AccountInfo<'info>,
    /// MarketBinding-owned neutral destination for hostile prefunding/surplus.
    pub neutral_sink: &'a AccountInfo<'info>,
}

/// Independently authenticated terminal authority facts for closing 0x85/v2.
///
/// This type is not terminal authority by itself. The caller must construct it
/// from the candidate-wide fee terminal composer that simultaneously persists
/// the closure manifest and terminal receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateFeeCollectionClosureExpectationV5 {
    market_binding: Id32,
    recipient_account_data_id: Id32,
    runtime_release: Id32,
    close_receipt: Id32,
    outcome: FeeTerminalOutcomeV1,
}

impl CandidateFeeCollectionClosureExpectationV5 {
    /// Bind a terminal composition to exact immutable and pre-close identities.
    pub fn new(
        market_binding: Id32,
        recipient_account_data_id: Id32,
        runtime_release: Id32,
        close_receipt: Id32,
        outcome: FeeTerminalOutcomeV1,
    ) -> Outcome<Self> {
        require(
            !market_binding.is_zero()
                && !recipient_account_data_id.is_zero()
                && !runtime_release.is_zero()
                && !close_receipt.is_zero()
                && market_binding != recipient_account_data_id
                && market_binding != runtime_release
                && market_binding != close_receipt
                && recipient_account_data_id != runtime_release
                && recipient_account_data_id != close_receipt
                && runtime_release != close_receipt,
            ClutchError::MismatchedState,
        )?;
        Ok(Self {
            market_binding,
            recipient_account_data_id,
            runtime_release,
            close_receipt,
            outcome,
        })
    }
}

/// Independently authenticated action-39 facts expected from General.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateFeeCollectionExpectationV5 {
    realm: Id32,
    market: Id32,
    epoch: Id32,
    candidate: Id32,
    fee_record: Id32,
    batch_policy: Id32,
    owner_order_set_digest: Id32,
    owner_count: u16,
    price_scale: u64,
    outcome_count: u8,
}

impl CandidateFeeCollectionExpectationV5 {
    /// Construct only from the selected node and exhaustive traversal facts.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        realm: Id32,
        market: Id32,
        epoch: Id32,
        candidate: Id32,
        fee_record: Id32,
        batch_policy: Id32,
        owner_order_set_digest: Id32,
        owner_count: u16,
        price_scale: u64,
        outcome_count: u8,
    ) -> Outcome<Self> {
        require(
            !realm.is_zero()
                && !market.is_zero()
                && !epoch.is_zero()
                && !candidate.is_zero()
                && !fee_record.is_zero()
                && !batch_policy.is_zero()
                && !owner_order_set_digest.is_zero()
                && owner_count != 0
                && usize::from(owner_count) <= clutch_owner_settlement::MAX_ORDERS
                && price_scale != 0
                && outcome_count != 0,
            ClutchError::MismatchedState,
        )?;
        Ok(Self {
            realm,
            market,
            epoch,
            candidate,
            fee_record,
            batch_policy,
            owner_order_set_digest,
            owner_count,
            price_scale,
            outcome_count,
        })
    }
}

/// O(1) immutable candidate-wide fee fact for General action 39.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedCandidateFeeCollectionV5 {
    selected: SelectedCompositeFeeV1,
    certified: CertifiedRecipientAllocationV2,
    recipient_account: Id32,
    recipient_account_data_id: Id32,
    owner_fee_book_data_id: FeeId,
    owner_order_set_digest: FeeId,
    owner_count: u16,
    selected_fee_atoms: u64,
    recipient_rent: DeletableRentOwnerV1,
    recipient_bump: u8,
}

impl PreparedCandidateFeeCollectionV5 {
    /// Exact selected composite-fee record.
    pub const fn selected(&self) -> SelectedCompositeFeeV1 {
        self.selected
    }

    /// Exact complete-book certificate authenticated from the 0x85/v2 outer.
    pub const fn certified(&self) -> CertifiedRecipientAllocationV2 {
        self.certified
    }

    /// Canonical 0x85/v2 recipient-allocation PDA.
    pub const fn recipient_account(&self) -> Id32 {
        self.recipient_account
    }

    /// Full-outer content identity of the exact certified account bytes.
    pub const fn recipient_account_data_id(&self) -> Id32 {
        self.recipient_account_data_id
    }

    /// Complete canonical selected-owner fee-book content identity.
    pub const fn owner_fee_book_data_id(&self) -> FeeId {
        self.owner_fee_book_data_id
    }

    /// Exhaustive traversal owner-order-set digest.
    pub const fn owner_order_set_digest(&self) -> FeeId {
        self.owner_order_set_digest
    }

    /// Exact count of canonical participating owners.
    pub const fn owner_count(&self) -> u16 {
        self.owner_count
    }

    /// Sum of all complete-book owner fee rows.
    pub const fn selected_fee_atoms(&self) -> u64 {
        self.selected_fee_atoms
    }

    /// Exact persisted rent owner of the certified recipient account.
    pub const fn recipient_rent(&self) -> DeletableRentOwnerV1 {
        self.recipient_rent
    }

    /// Canonical stored bump of the certified recipient account.
    pub const fn recipient_bump(&self) -> u8 {
        self.recipient_bump
    }
}

/// Fee-bearing action-39 input exact-joined to General's root expectation.
///
/// General remains the sole owner of SettlementRoot creation and counters.
/// This value only pairs its exhaustive structural expectation with the O(1)
/// authenticated complete-book fee certificate that supplied the fee total.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedCandidateFeeCollectionAction39V5 {
    collection: PreparedCandidateFeeCollectionV5,
    root_expectation: SettlementRootExpectationProjectionV1,
}

impl PreparedCandidateFeeCollectionAction39V5 {
    /// Authenticated candidate-wide selected-fee collection.
    pub const fn collection(&self) -> PreparedCandidateFeeCollectionV5 {
        self.collection
    }

    /// Exact root/cash-pot expectation derived from exhaustive traversal.
    pub const fn root_expectation(&self) -> SettlementRootExpectationProjectionV1 {
        self.root_expectation
    }
}

/// Exact rent-only disposition staged for the 0x85/v2 terminal close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedCandidateFeeCollectionClosureV5 {
    collection: PreparedCandidateFeeCollectionV5,
    closure: ExternalFeeAccountClosureV1,
    principal_refund: FeeLamportTransferV2,
    donation_credit: FeeLamportTransferV2,
}

impl PreparedCandidateFeeCollectionClosureV5 {
    /// Reauthenticated candidate-wide fee collection consumed by the close.
    pub const fn collection(&self) -> PreparedCandidateFeeCollectionV5 {
        self.collection
    }

    /// Exact fee-runtime closure component for the terminal manifest.
    pub const fn closure(&self) -> ExternalFeeAccountClosureV1 {
        self.closure
    }

    /// Refund of only the persisted payer-funded rent principal.
    pub const fn principal_refund(&self) -> FeeLamportTransferV2 {
        self.principal_refund
    }

    /// Credit of all hostile prefunding and later native-lamport surplus.
    pub const fn donation_credit(&self) -> FeeLamportTransferV2 {
        self.donation_credit
    }
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
    selected: SelectedCompositeFeeV1,
    carry: clutch_fee_runtime_contract::selected::OwnerFeeCarryV1,
    carry_rent: DeletableRentOwnerV1,
    payer_rent: DeletableRentOwnerV1,
    carry_bump: u8,
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
/// the candidate-fee branch only, replace the carry with its `0x83/4` terminal
/// receipt while deleting the payer snapshot. This plan already owns the exact
/// row, Position, pot, Replay, and counted-root successors; it cannot be
/// constructed from a caller balance summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedOwnerSettlementAction38V5 {
    fee: PreparedOwnerFeeAction38V5,
    realization: OwnerCashRealizationPlanV5,
    transition_evidence_id: Id32,
    replay: GeneralReplayTransitionPlanV1,
    finalization: Option<PreparedOwnerFeeFinalizationV5>,
}

/// Exact fee-bearing terminal receipt plus present-funded native-rent plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedOwnerFeeFinalizationV5 {
    terminal: OwnerFeeFinalizationV4AccountV1,
    rent: OwnerFeeRentTransitionPlanV3,
}

impl PreparedOwnerFeeFinalizationV5 {
    /// Exact 0x83/v4 poststate to write at the unchanged carry PDA.
    pub const fn terminal(&self) -> OwnerFeeFinalizationV4AccountV1 {
        self.terminal
    }

    /// Exact carry realloc and payer close/refund/donation plan.
    pub const fn rent(&self) -> OwnerFeeRentTransitionPlanV3 {
        self.rent
    }

    /// Exact terminal account width used for rent and realloc postchecks.
    pub const fn terminal_account_bytes(&self) -> usize {
        OWNER_FEE_FINALIZATION_ACCOUNT_BYTES_V4
    }
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

    /// Real terminal fee state only for the candidate-fee branch.
    pub const fn finalization(&self) -> Option<PreparedOwnerFeeFinalizationV5> {
        self.finalization
    }

    /// Whether this atomic transition must mint a real `0x83/4` successor.
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
    pub const fn owner_row_fee_evidence(&self) -> OwnerRowFeeEvidenceV5 {
        match self.evidence {
            PreparedOwnerFeeEvidenceV5::CandidateFee(value) => {
                OwnerRowFeeEvidenceV5::CandidateFee(value.selected_fee())
            }
            PreparedOwnerFeeEvidenceV5::NoFeeRecord(_) => OwnerRowFeeEvidenceV5::NoFeeRecord,
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

    /// Exact selected composite-fee semantic record.
    pub const fn selected(&self) -> SelectedCompositeFeeV1 {
        self.selected
    }

    /// Exact terminal carry semantic prestate.
    pub const fn carry(&self) -> clutch_fee_runtime_contract::selected::OwnerFeeCarryV1 {
        self.carry
    }

    /// Persisted live carry rent owner.
    pub const fn carry_rent(&self) -> DeletableRentOwnerV1 {
        self.carry_rent
    }

    /// Persisted payer-snapshot rent owner.
    pub const fn payer_rent(&self) -> DeletableRentOwnerV1 {
        self.payer_rent
    }

    /// Stored canonical bump retained by the in-place terminal successor.
    pub const fn carry_bump(&self) -> u8 {
        self.carry_bump
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

fn require_fee_account_rent_v5(
    rent: DeletableRentOwnerV1,
    fee_account: Id32,
    settlement_root: Id32,
    current_lamports: u64,
) -> Outcome<()> {
    rent.validate()?;
    let recorded_balance_floor = rent
        .refundable_principal
        .checked_add(rent.donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        rent.payer != fee_account
            && rent.payer != settlement_root
            && fee_account != settlement_root
            && current_lamports >= recorded_balance_floor,
        ClutchError::MismatchedState,
    )
}

fn recipient_close_lamports_v5(
    rent: DeletableRentOwnerV1,
    balance_before: u64,
) -> Outcome<(u64, u64)> {
    rent.validate()?;
    let donation = balance_before
        .checked_sub(rent.refundable_principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        donation >= rent.donation_floor,
        ClutchError::MismatchedState,
    )?;
    Ok((rent.refundable_principal, donation))
}

/// Authenticate the immutable complete-book fee certificate for action 39.
///
/// This is O(1) only because 0x85/v2 is a fresh immutable program-owned
/// version whose sole creation contract consumes the complete canonical
/// `SelectedOwnerFeeBookV1`. Historical 0x85/v1 bytes are never admitted here.
pub fn prepare_candidate_fee_collection_action39_v5(
    program_id: &Pubkey,
    expected: CandidateFeeCollectionExpectationV5,
    frame: CandidateFeeCollectionAccountFrameV5<'_, '_>,
    revenue_policy: &RevenuePolicyV1,
) -> Outcome<PreparedCandidateFeeCollectionV5> {
    authenticate_candidate_fee_collection_v5(
        program_id,
        expected,
        frame,
        revenue_policy,
        false,
    )
}

/// Compose the fee-bearing action-39 root writeback expectation.
///
/// The 0x85/v2 creation contract consumes the complete owner fee book. This
/// preparation seam authenticates that exact outer and joins it to General's
/// exhaustive retained-feed/page traversal without rereading owner fee rows
/// or accepting a caller-supplied aggregate. It does not create the
/// SettlementRoot or mutate any root counter.
pub fn compose_candidate_fee_collection_action39_v5(
    program_id: &Pubkey,
    expected: CandidateFeeCollectionExpectationV5,
    frame: CandidateFeeCollectionAccountFrameV5<'_, '_>,
    revenue_policy: &RevenuePolicyV1,
    traversal: &SettlementTraversalProjectionV4,
) -> Outcome<PreparedCandidateFeeCollectionAction39V5> {
    let collection = prepare_candidate_fee_collection_action39_v5(
        program_id,
        expected,
        frame,
        revenue_policy,
    )?;
    let root_expectation = derive_settlement_root_expectation_from_certified_fee_v2(
        traversal,
        &collection.selected,
        &collection.certified,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(PreparedCandidateFeeCollectionAction39V5 {
        collection,
        root_expectation,
    })
}

#[inline(never)]
fn authenticate_candidate_fee_collection_v5(
    program_id: &Pubkey,
    expected: CandidateFeeCollectionExpectationV5,
    frame: CandidateFeeCollectionAccountFrameV5<'_, '_>,
    revenue_policy: &RevenuePolicyV1,
    recipient_writable: bool,
) -> Outcome<PreparedCandidateFeeCollectionV5> {
    for account in [
        frame.selected_fee_record,
        frame.batch_policy,
        frame.revenue_policy_record,
    ] {
        require(!account.executable, ClutchError::ExecutableAccount)?;
        require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    }
    require(
        frame.selected_fee_record.key != frame.certified_recipient_allocation.key
            && frame.selected_fee_record.key != frame.batch_policy.key
            && frame.selected_fee_record.key != frame.revenue_policy_record.key
            && frame.certified_recipient_allocation.key != frame.batch_policy.key
            && frame.certified_recipient_allocation.key != frame.revenue_policy_record.key
            && frame.batch_policy.key != frame.revenue_policy_record.key,
        ClutchError::AccountAlias,
    )?;
    require_read_only_program_state(
        program_id,
        frame.selected_fee_record,
        SELECTED_FEE_RECORD_ACCOUNT_BYTES,
    )?;
    require_snapshot_state(
        program_id,
        frame.certified_recipient_allocation,
        RECIPIENT_ALLOCATION_ACCOUNT_BYTES_V2,
        recipient_writable,
    )?;
    require_read_only_program_state(program_id, frame.batch_policy, BATCH_POLICY_BYTES)?;
    require_read_only_program_state(
        program_id,
        frame.revenue_policy_record,
        REVENUE_POLICY_RECORD_BYTES,
    )?;

    let batch = decode_batch_policy(&borrow_data(frame.batch_policy)?)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let batch_id = batch_policy_digest(&batch)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        batch_id.0 == expected.batch_policy.bytes(),
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        frame.batch_policy.key,
        seeds::batch_policy_pda(program_id, &expected.epoch.bytes(), &batch_id.0),
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
    let selected = SelectedFeeRecordV1AccountV1::decode(
        &borrow_data(frame.selected_fee_record)?,
        &batch,
        revenue_policy,
    )?;
    expect_pda(
        frame.selected_fee_record.key,
        seeds::general_v2_selected_fee_record_pda(program_id, &expected.candidate.bytes()),
        Some(selected.stored_bump),
    )?;
    require(
        id(frame.selected_fee_record.key) == expected.fee_record,
        ClutchError::MismatchedState,
    )?;
    require_selected_fee_binding_v5(
        &selected.semantic,
        &revenue_record,
        ExpectedSelectedFeeBindingV5 {
            fee_record: expected.fee_record,
            realm: expected.realm,
            market: expected.market,
            epoch: expected.epoch,
            candidate: expected.candidate,
            batch_policy: expected.batch_policy,
            revenue_policy: Id32::from_bytes(revenue_digest.0),
            price_scale: expected.price_scale,
            outcome_count: expected.outcome_count,
        },
    )?;

    let recipient_data = borrow_data(frame.certified_recipient_allocation)?;
    let recipient = RecipientAllocationV2AccountV1::decode_persisted(&recipient_data)?;
    let recipient_data_id = recipient_allocation_account_data_id_v2(
        &recipient_data,
        &RuntimeSha256,
    )?;
    drop(recipient_data);
    expect_pda(
        frame.certified_recipient_allocation.key,
        seeds::general_v2_recipient_allocation_pda(
            program_id,
            &selected.semantic.fee_record().0,
        ),
        Some(recipient.stored_bump),
    )?;
    let allocation = recipient.semantic.allocation();
    let recorded_balance_floor = recipient
        .rent
        .refundable_principal
        .checked_add(recipient.rent.donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    recipient.rent.validate()?;
    require(
        recipient.rent.payer != id(frame.certified_recipient_allocation.key)
            && frame.certified_recipient_allocation.lamports() >= recorded_balance_floor
            && allocation.fee_record() == selected.semantic.fee_record()
            && recipient.semantic.owner_order_set_digest().0
                == expected.owner_order_set_digest.bytes()
            && recipient.semantic.owner_count() == expected.owner_count
            && allocation.collected_fee_atoms() != 0,
        ClutchError::MismatchedState,
    )?;
    Ok(PreparedCandidateFeeCollectionV5 {
        selected: selected.semantic,
        certified: recipient.semantic,
        recipient_account: id(frame.certified_recipient_allocation.key),
        recipient_account_data_id: recipient_data_id,
        owner_fee_book_data_id: recipient.semantic.owner_fee_book_data_id(),
        owner_order_set_digest: recipient.semantic.owner_order_set_digest(),
        owner_count: recipient.semantic.owner_count(),
        selected_fee_atoms: allocation.collected_fee_atoms(),
        recipient_rent: recipient.rent,
        recipient_bump: recipient.stored_bump,
    })
}

/// Authenticate and stage the rent-only 0x85/v2 terminal close.
///
/// The returned [`ExternalFeeAccountClosureV1`] must be consumed by the same
/// candidate-wide terminal composer that creates the named close receipt. No
/// write or lamport transfer is performed here. The recipient account holds no
/// fee value: its exact economic allocation remains ordinary Position-ledger
/// accounting. Consequently only persisted native rent principal is refunded;
/// every other lamport is a donation to the authenticated MarketBinding sink.
pub fn prepare_candidate_fee_collection_terminal_close_v5(
    program_id: &Pubkey,
    expected: CandidateFeeCollectionExpectationV5,
    authority: CandidateFeeCollectionClosureExpectationV5,
    frame: CandidateFeeCollectionClosureAccountFrameV5<'_, '_>,
    revenue_policy: &RevenuePolicyV1,
) -> Outcome<PreparedCandidateFeeCollectionClosureV5> {
    let collection = authenticate_candidate_fee_collection_v5(
        program_id,
        expected,
        frame.collection,
        revenue_policy,
        true,
    )?;
    require(
        collection.recipient_account_data_id == authority.recipient_account_data_id,
        ClutchError::MismatchedState,
    )?;
    require_read_only_program_state(
        program_id,
        frame.market_binding,
        MARKET_BINDING_ACCOUNT_BYTES_V2,
    )?;
    let market_binding = MarketBindingV2::decode(&borrow_data(frame.market_binding)?)?;
    let binding = market_binding.base();
    expect_pda(
        frame.market_binding.key,
        seeds::general_v2_market_binding_pda(
            program_id,
            &binding.market_instance_v2_id.bytes(),
        ),
        Some(binding.stored_bump),
    )?;
    let runtime_release = fee_runtime_semantic_release_id_v1(&RuntimeSha256)?;
    require(
        id(frame.market_binding.key) == authority.market_binding
            && binding.market == expected.market
            && market_binding.batch_policy_id() == expected.batch_policy
            && id(frame.neutral_sink.key) == binding.neutral_sink
            && id(frame.rent_refund_owner.key) == collection.recipient_rent.payer
            && authority.runtime_release == runtime_release,
        ClutchError::MismatchedState,
    )?;
    for destination in [frame.rent_refund_owner, frame.neutral_sink] {
        require(!destination.executable, ClutchError::ExecutableAccount)?;
        require(destination.is_writable, ClutchError::NotWritable)?;
    }
    require(
        frame.rent_refund_owner.key != frame.neutral_sink.key
            && frame.rent_refund_owner.key
                != frame.collection.certified_recipient_allocation.key
            && frame.neutral_sink.key != frame.collection.certified_recipient_allocation.key
            && frame.market_binding.key != frame.collection.certified_recipient_allocation.key
            && frame.market_binding.key != frame.rent_refund_owner.key
            && frame.market_binding.key != frame.neutral_sink.key
            && authority.close_receipt != collection.recipient_account
            && authority.close_receipt != collection.recipient_rent.payer
            && authority.close_receipt != id(frame.neutral_sink.key)
            && authority.close_receipt != id(frame.market_binding.key),
        ClutchError::AccountAlias,
    )?;
    let balance_before = frame.collection.certified_recipient_allocation.lamports();
    let (principal_lamports, donation_lamports) =
        recipient_close_lamports_v5(collection.recipient_rent, balance_before)?;
    let account = FeeId(collection.recipient_account.bytes());
    let rent_payer = FeeId(collection.recipient_rent.payer.bytes());
    let neutral_sink = fee_id(frame.neutral_sink.key);
    let closure = map_fee(ExternalFeeAccountClosureV1::admit(
        CandidateFeeAccountRoleV1::RecipientAllocation,
        authority.outcome,
        FeeId(program_id.to_bytes()),
        FeeId(runtime_release.bytes()),
        collection.selected.fee_record(),
        account,
        FeeId([0; 32]),
        FeeId(authority.close_receipt.bytes()),
        rent_payer,
        neutral_sink,
        balance_before,
        principal_lamports,
        donation_lamports,
    ))?;
    Ok(PreparedCandidateFeeCollectionClosureV5 {
        collection,
        closure,
        principal_refund: FeeLamportTransferV2 {
            source: collection.recipient_account,
            destination: collection.recipient_rent.payer,
            lamports: principal_lamports,
        },
        donation_credit: FeeLamportTransferV2 {
            source: collection.recipient_account,
            destination: id(frame.neutral_sink.key),
            lamports: donation_lamports,
        },
    })
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
        OWNER_FEE_CARRY_ACCOUNT_BYTES_V3,
        snapshot_writable,
    )?;
    require_snapshot_state(
        program_id,
        frame.payer_allocation,
        PAYER_ALLOCATION_ACCOUNT_BYTES_V2,
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

    let carry_account = OwnerFeeCarryV3AccountV1::decode(
        &borrow_data(frame.owner_fee_carry)?,
        &selected,
    )?;
    require_fee_account_rent_v5(
        carry_account.rent,
        id(frame.owner_fee_carry.key),
        context.root_account,
        frame.owner_fee_carry.lamports(),
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
        PayerAllocationV2AccountV1::decode_persisted(&borrow_data(frame.payer_allocation)?)?;
    require_fee_account_rent_v5(
        payer_account.rent,
        id(frame.payer_allocation.key),
        context.root_account,
        frame.payer_allocation.lamports(),
    )?;
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
        selected,
        carry: carry_account.semantic,
        carry_rent: carry_account.rent,
        payer_rent: payer_account.rent,
        carry_bump: carry_account.stored_bump,
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

#[allow(clippy::too_many_arguments)]
fn prepare_owner_fee_finalization_v5(
    program_id: &Pubkey,
    authenticated_root: &AuthenticatedGeneralSettlementRootV1,
    fee: &PreparedOwnerFeeAction38V5,
    fee_accounts: OwnerFeeAccountInputV5<'_, '_>,
    rent_input: OwnerFeeTerminalRentInputV5<'_, '_>,
    realization: &OwnerCashRealizationPlanV5,
    replay: GeneralReplayTransitionPlanV1,
) -> Outcome<Option<PreparedOwnerFeeFinalizationV5>> {
    let (
        PreparedOwnerFeeEvidenceV5::CandidateFee(snapshot),
        OwnerFeeAccountInputV5::CandidateFee {
            frame: fee_frame, ..
        },
        OwnerFeeTerminalRentInputV5::CandidateFee {
            frame: rent_frame,
            carry_terminal_rent_minimum_lamports,
        },
    ) = (fee.evidence(), fee_accounts, rent_input)
    else {
        return match (fee.evidence(), fee_accounts, rent_input) {
            (
                PreparedOwnerFeeEvidenceV5::NoFeeRecord(_),
                OwnerFeeAccountInputV5::NoFeeRecord,
                OwnerFeeTerminalRentInputV5::NoFeeRecord,
            ) => Ok(None),
            _ => Err(Refusal::Adapter(ClutchError::MismatchedState)),
        };
    };

    require_read_only_program_state(
        program_id,
        rent_frame.market_binding,
        MARKET_BINDING_ACCOUNT_BYTES_V2,
    )?;
    let market_binding = MarketBindingV2::decode(&borrow_data(rent_frame.market_binding)?)?;
    let binding = market_binding.base();
    expect_pda(
        rent_frame.market_binding.key,
        seeds::general_v2_market_binding_pda(
            program_id,
            &binding.market_instance_v2_id.bytes(),
        ),
        Some(binding.stored_bump),
    )?;
    require(
        id(rent_frame.market_binding.key) == authenticated_root.root().market_binding()
            && binding.market == authenticated_root.root().market()
            && binding.market_instance_v2_id
                == authenticated_root.root().market_instance_v2_id()
            && market_binding.batch_policy_id() == authenticated_root.root().batch_policy_id()
            && id(rent_frame.neutral_sink.key) == binding.neutral_sink,
        ClutchError::MismatchedState,
    )?;
    for destination in [
        rent_frame.carry_rent_payer,
        rent_frame.payer_rent_refund_owner,
        rent_frame.neutral_sink,
    ] {
        require(!destination.executable, ClutchError::ExecutableAccount)?;
        require(destination.is_writable, ClutchError::NotWritable)?;
    }
    require(
        id(rent_frame.carry_rent_payer.key) == snapshot.carry_rent().payer
            && id(rent_frame.payer_rent_refund_owner.key) == snapshot.payer_rent().payer
            && rent_frame.neutral_sink.key != fee_frame.owner_fee_carry.key
            && rent_frame.neutral_sink.key != fee_frame.payer_allocation.key
            && rent_frame.carry_rent_payer.key != fee_frame.owner_fee_carry.key
            && rent_frame.carry_rent_payer.key != fee_frame.payer_allocation.key
            && rent_frame.payer_rent_refund_owner.key != fee_frame.owner_fee_carry.key
            && rent_frame.payer_rent_refund_owner.key != fee_frame.payer_allocation.key,
        ClutchError::AccountAlias,
    )?;
    let rent = prepare_owner_fee_rent_transition_v3(
        OwnerFeeRentTransitionAccountsV3 {
            carry_account: id(fee_frame.owner_fee_carry.key),
            payer_allocation_account: id(fee_frame.payer_allocation.key),
            neutral_sink: id(rent_frame.neutral_sink.key),
        },
        snapshot.carry_rent(),
        snapshot.payer_rent(),
        fee_frame.owner_fee_carry.lamports(),
        fee_frame.payer_allocation.lamports(),
        carry_terminal_rent_minimum_lamports,
        &RuntimeSha256,
    )?;
    require(
        rent.carry_top_up().lamports() == 0 || rent_frame.carry_rent_payer.is_signer,
        ClutchError::MissingSignature,
    )?;
    let runtime_release = fee_runtime_semantic_release_id_v1(&RuntimeSha256)?;
    let bindings = OwnerFeeFinalizationBindingsV2 {
        runtime_release: FeeId(runtime_release.bytes()),
        payer_allocation_data_id: snapshot.payer_allocation_data_id(),
        owner_settlement_account: FeeId(realization.owner_settlement_account().bytes()),
        owner_settlement_final_data_id: FeeId(
            realization.finalized_owner_row_data_id().bytes(),
        ),
        settlement_cash_pot: FeeId(realization.settlement_cash_pot_account().bytes()),
        position_poststate_semantic_id: FeeId(
            replay.position_poststate_semantic_id().bytes(),
        ),
        replay_poststate_semantic_id: FeeId(replay.replay_poststate_semantic_id().bytes()),
        replay_next_sequence: replay.next_sequence(),
        settlement_cash_pot_poststate_data_id: FeeId(
            realization.pot_poststate_data_id().bytes(),
        ),
        rent_disposition: rent.semantic(),
    };
    let semantic = OwnerFeeFinalizationReceiptV1::settle_delivery_complete_v4(
        &snapshot.selected(),
        &snapshot.selected_fee(),
        &snapshot.carry(),
        bindings,
        realization.semantic(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(Some(PreparedOwnerFeeFinalizationV5 {
        terminal: OwnerFeeFinalizationV4AccountV1 {
            semantic,
            rent: rent.carry_rent_after(),
            stored_bump: snapshot.carry_bump(),
        },
        rent,
    }))
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
    fee_rent: OwnerFeeTerminalRentInputV5<'_, '_>,
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
    let finalization = prepare_owner_fee_finalization_v5(
        program_id,
        authenticated_root,
        &fee,
        fee_accounts,
        fee_rent,
        &realization,
        replay,
    )?;
    require(
        finalization.is_some() == realization.fee_finalization_required(),
        ClutchError::MismatchedState,
    )?;
    Ok(PreparedOwnerSettlementAction38V5 {
        fee,
        realization,
        transition_evidence_id,
        replay,
        finalization,
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

    #[test]
    fn candidate_certificate_close_never_turns_surplus_into_refund() {
        let rent = DeletableRentOwnerV1 {
            payer: Id32::from_bytes([41; 32]),
            refundable_principal: 1_000,
            donation_floor: 40,
        };
        assert_eq!(recipient_close_lamports_v5(rent, 1_075), Ok((1_000, 75)));
        assert_eq!(recipient_close_lamports_v5(rent, 1_039), Err(
            Refusal::Adapter(ClutchError::MismatchedState)
        ));
        assert_eq!(recipient_close_lamports_v5(rent, 999), Err(
            Refusal::Adapter(ClutchError::Arithmetic)
        ));
    }

    #[test]
    fn candidate_certificate_close_authority_ids_are_disjoint() {
        let outcome = FeeTerminalOutcomeV1::Settled;
        assert!(CandidateFeeCollectionClosureExpectationV5::new(
            Id32::from_bytes([51; 32]),
            Id32::from_bytes([52; 32]),
            Id32::from_bytes([53; 32]),
            Id32::from_bytes([54; 32]),
            outcome,
        )
        .is_ok());
        assert_eq!(
            CandidateFeeCollectionClosureExpectationV5::new(
                Id32::from_bytes([51; 32]),
                Id32::from_bytes([52; 32]),
                Id32::from_bytes([53; 32]),
                Id32::from_bytes([51; 32]),
                outcome,
            ),
            Err(Refusal::Adapter(ClutchError::MismatchedState))
        );
    }
}
