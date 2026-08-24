#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Current Direct-market root and permanent replay semantics.
//!
//! The deletable Direct root owns phases and archive counts. The action account
//! is the sole owner of action ordinals and rolling receipts until atomic
//! family retirement commits its last receipt into Product and closes it.
//! Solana ownership, PDA derivation, account bytes, funding, and transfers
//! remain in default-deny adapter boundaries.

use clutch_product_series::{
    AuthenticatedMarketFamilyAuthorityV1, CompiledProductSeriesBundleV5, ContentId,
    Error as ProductError, MarketFamilyAggregatorV1, MarketFamilyV1, MarketLifecyclePhaseV1,
    MarketLifecycleRootV1, SeriesMarketDispositionV1, SeriesMarketLinkPhaseV1,
    SeriesMarketLinkV1,
};

pub mod selection_v1;
pub mod settlement_v1;
pub mod codec_v1;
pub mod fee_v1;
pub mod liveness_v1;

/// Maximum funded Reservations ever admitted by one minimal Direct root.
pub const MAX_DIRECT_RESERVATIONS_V1: u8 = 2;
/// Maximum retained submitted candidates owned by one Direct Selection.
pub const MAX_DIRECT_CANDIDATES_V1: u8 = 3;
/// Fixed bounded Reservation-admission interval for the staged V1 profile.
pub const DIRECT_ADMISSION_SPAN_SLOTS_V1: u64 = 64;
/// Fixed bounded candidate-submission interval for the staged V1 profile.
pub const DIRECT_SUBMISSION_SPAN_SLOTS_V1: u64 = 64;
/// Fixed bounded retained-candidate verification interval.
pub const DIRECT_VERIFICATION_SPAN_SLOTS_V1: u64 = 64;
/// Fixed bounded interval after selection for settlement or lapse.
pub const DIRECT_SETTLEMENT_SPAN_SLOTS_V1: u64 = 64;
/// Root, replay, Selection, and at most two live Reservations close together.
pub const MAX_DIRECT_RETIREMENT_SOURCES_V1: usize = 5;
/// At most five distinct persisted principal payers receive refunds.
pub const MAX_DIRECT_REFUND_RECIPIENTS_V1: usize = 5;

const FOUNDATION_RECEIPT_DOMAIN_V1: &[u8] = b"dragons-clutch/direct/foundation-receipt/v1\0";
const DIRECT_EPOCH_SEMANTICS_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/epoch-semantics/v1\0";
const DIRECT_SCHEDULE_POLICY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/schedule-policy/v1\0";
const ACTION_TRANSCRIPT_DOMAIN_V1: &[u8] = b"dragons-clutch/direct/action-transcript/v1\0";
const REPLAY_LIVENESS_BATCH_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/replay-liveness-batch/v1\0";
const ROOT_STATE_DOMAIN_V1: &[u8] = b"dragons-clutch/direct/root-state/v1\0";
const REPLAY_STATE_DOMAIN_V1: &[u8] = b"dragons-clutch/direct/replay-state/v1\0";
const TERMINAL_RECEIPT_DOMAIN_V1: &[u8] = b"dragons-clutch/direct/terminal-receipt/v1\0";
const RETIREMENT_TRANSFER_DOMAIN_V1: &[u8] = b"dragons-clutch/direct/retirement-transfer/v1\0";

/// Allocation-free SHA-256 boundary for Direct semantic identities.
pub trait DirectHashBackendV1 {
    /// Hash the exact concatenation of the ordered slices.
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32];
}

/// Deterministic refusal from the Direct semantic owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectMarketErrorV1 {
    /// A required full-width identity was zero.
    ZeroIdentity,
    /// Two roles which must be distinct aliased.
    IdentityAlias,
    /// Immutable Product, Market, Realm, collateral, or root facts disagreed.
    MismatchedBinding,
    /// A transition was attempted from the wrong phase.
    WrongPhase,
    /// A fixed count, cursor, or partition was noncanonical.
    InvalidCount,
    /// A checked integer operation overflowed or underflowed.
    Arithmetic,
    /// An action ordinal or permanent receipt was stale or detached.
    Replay,
    /// The default-deny adapter authority refused.
    UnauthenticatedAuthority,
    /// Product rejected the exact family transition.
    Product,
    /// PositionV3 or its purpose binding was not the exact writable prestate.
    InvalidPosition,
    /// A cash reservation would require a rounding boundary.
    InexactCashConversion,
    /// The owner-blind RelationV2 kernel refused an exact input or candidate.
    Economic(clutch_batch::relation_v2::EconomicErrorV2),
    /// The scalar Direct specialization refused the selected pair.
    DirectPair(clutch_batch::direct_pair_v1::DirectPairErrorV1),
    /// General PositionV3/GEN1 structural projection refused.
    GeneralContract(clutch_general_v2_contract::CodecError),
    /// The shared, present-funded runtime-liveness owner refused the join.
    Liveness(clutch_liveness::runtime_v1::RuntimeLivenessErrorV1),
}

impl From<clutch_batch::relation_v2::EconomicErrorV2> for DirectMarketErrorV1 {
    fn from(value: clutch_batch::relation_v2::EconomicErrorV2) -> Self {
        Self::Economic(value)
    }
}

impl From<clutch_batch::direct_pair_v1::DirectPairErrorV1> for DirectMarketErrorV1 {
    fn from(value: clutch_batch::direct_pair_v1::DirectPairErrorV1) -> Self {
        Self::DirectPair(value)
    }
}

impl From<clutch_general_v2_contract::CodecError> for DirectMarketErrorV1 {
    fn from(value: clutch_general_v2_contract::CodecError) -> Self {
        Self::GeneralContract(value)
    }
}

impl From<clutch_liveness::runtime_v1::RuntimeLivenessErrorV1> for DirectMarketErrorV1 {
    fn from(value: clutch_liveness::runtime_v1::RuntimeLivenessErrorV1) -> Self {
        Self::Liveness(value)
    }
}

/// Exact deletable lamport ownership for one Direct account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRentOwnerV1 {
    /// Principal payer and sole refund owner.
    pub payer: [u8; 32],
    /// Exact refundable principal.
    pub principal_lamports: u64,
    /// Minimum hostile prefund observed at creation.
    pub donation_floor_lamports: u64,
}

impl DirectRentOwnerV1 {
    /// Validate payer, nonzero principal, and exact lamport sum domain.
    pub fn validate(self) -> Result<(), DirectMarketErrorV1> {
        require_live(self.payer)?;
        if self.principal_lamports == 0 {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        self.principal_lamports
            .checked_add(self.donation_floor_lamports)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        Ok(())
    }
}

/// One authenticated account balance immediately before deletion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRetirementSourceV1 {
    /// Exact Direct-owned account being deleted.
    pub account: [u8; 32],
    /// Persisted rent principal and payer owner.
    pub rent: DirectRentOwnerV1,
    /// Hostile observed balance in the same snapshot as terminal bytes.
    pub observed_lamports: u64,
}

impl DirectRetirementSourceV1 {
    fn validate(self) -> Result<(), DirectMarketErrorV1> {
        require_live(self.account)?;
        self.rent.validate()?;
        let floor = self
            .rent
            .principal_lamports
            .checked_add(self.rent.donation_floor_lamports)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        if self.observed_lamports < floor {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        Ok(())
    }

    fn surplus_lamports(self) -> Result<u64, DirectMarketErrorV1> {
        self.observed_lamports
            .checked_sub(self.rent.principal_lamports)
            .ok_or(DirectMarketErrorV1::Arithmetic)
    }
}

/// One sorted, coalesced principal-only payer refund.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPrincipalRefundV1 {
    /// Persisted payer receiving only its own principal.
    pub recipient: [u8; 32],
    /// Sum of exact principals funded by that payer.
    pub lamports: u64,
}

/// Canonical complete transfer vector for one atomic deletion set.
///
/// Sources are sorted by account. Refunds are sorted and coalesced by payer.
/// Every lamport above exact persisted principal, including hostile prefunding
/// and donation floors, goes only to the Realm-authenticated neutral sink.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRetirementTransferV1 {
    /// Active sorted source prefix followed by `None` padding.
    pub sources: [Option<DirectRetirementSourceV1>; MAX_DIRECT_RETIREMENT_SOURCES_V1],
    /// Number of active sources.
    pub source_count: u8,
    /// Active sorted unique refund prefix followed by `None` padding.
    pub refunds: [Option<DirectPrincipalRefundV1>; MAX_DIRECT_REFUND_RECIPIENTS_V1],
    /// Number of active refund recipients.
    pub refund_count: u8,
    /// Realm-authenticated destination for every surplus lamport.
    pub neutral_lamport_sink: [u8; 32],
    /// Exact sum transferred to the neutral sink.
    pub surplus_lamports: u64,
}

impl DirectRetirementTransferV1 {
    /// Validate ordering, exact coalescing, padding, and conservation.
    pub fn validate(self) -> Result<(), DirectMarketErrorV1> {
        require_live(self.neutral_lamport_sink)?;
        let source_count = usize::from(self.source_count);
        let refund_count = usize::from(self.refund_count);
        if source_count == 0
            || source_count > MAX_DIRECT_RETIREMENT_SOURCES_V1
            || refund_count == 0
            || refund_count > MAX_DIRECT_REFUND_RECIPIENTS_V1
        {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        let mut expected_payers = [[0u8; 32]; MAX_DIRECT_REFUND_RECIPIENTS_V1];
        let mut expected_amounts = [0u64; MAX_DIRECT_REFUND_RECIPIENTS_V1];
        let mut expected_count = 0usize;
        let mut expected_surplus = 0u64;
        let mut previous_account = [0u8; 32];
        let mut index = 0usize;
        while index < MAX_DIRECT_RETIREMENT_SOURCES_V1 {
            match self.sources[index] {
                Some(source) if index < source_count => {
                    source.validate()?;
                    if source.account == self.neutral_lamport_sink
                        || (index != 0 && source.account <= previous_account)
                    {
                        return Err(DirectMarketErrorV1::IdentityAlias);
                    }
                    previous_account = source.account;
                    expected_surplus = expected_surplus
                        .checked_add(source.surplus_lamports()?)
                        .ok_or(DirectMarketErrorV1::Arithmetic)?;
                    insert_refund(
                        &mut expected_payers,
                        &mut expected_amounts,
                        &mut expected_count,
                        source.rent.payer,
                        source.rent.principal_lamports,
                    )?;
                }
                None if index >= source_count => {}
                _ => return Err(DirectMarketErrorV1::InvalidCount),
            }
            index += 1;
        }
        if expected_surplus != self.surplus_lamports || expected_count != refund_count {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        index = 0;
        while index < MAX_DIRECT_REFUND_RECIPIENTS_V1 {
            match self.refunds[index] {
                Some(refund) if index < refund_count => {
                    if refund.recipient != expected_payers[index]
                        || refund.lamports != expected_amounts[index]
                        || refund.recipient == self.neutral_lamport_sink
                    {
                        return Err(DirectMarketErrorV1::MismatchedBinding);
                    }
                }
                None if index >= refund_count => {}
                _ => return Err(DirectMarketErrorV1::InvalidCount),
            }
            index += 1;
        }
        Ok(())
    }

    /// Domain-separated identity of the complete transfer vector.
    pub fn semantic_id<B: DirectHashBackendV1>(
        self,
        backend: &B,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        self.validate()?;
        let mut source_accounts = [[0u8; 32]; MAX_DIRECT_RETIREMENT_SOURCES_V1];
        let mut source_payers = [[0u8; 32]; MAX_DIRECT_RETIREMENT_SOURCES_V1];
        let mut source_principals = [0u64; MAX_DIRECT_RETIREMENT_SOURCES_V1];
        let mut source_balances = [0u64; MAX_DIRECT_RETIREMENT_SOURCES_V1];
        let mut refund_recipients = [[0u8; 32]; MAX_DIRECT_REFUND_RECIPIENTS_V1];
        let mut refund_amounts = [0u64; MAX_DIRECT_REFUND_RECIPIENTS_V1];
        let mut index = 0usize;
        while index < usize::from(self.source_count) {
            let source = self.sources[index].ok_or(DirectMarketErrorV1::InvalidCount)?;
            source_accounts[index] = source.account;
            source_payers[index] = source.rent.payer;
            source_principals[index] = source.rent.principal_lamports;
            source_balances[index] = source.observed_lamports;
            index += 1;
        }
        index = 0;
        while index < usize::from(self.refund_count) {
            let refund = self.refunds[index].ok_or(DirectMarketErrorV1::InvalidCount)?;
            refund_recipients[index] = refund.recipient;
            refund_amounts[index] = refund.lamports;
            index += 1;
        }
        let id = backend.sha256_parts(&[
            RETIREMENT_TRANSFER_DOMAIN_V1,
            &[self.source_count],
            &source_accounts[0], &source_accounts[1], &source_accounts[2], &source_accounts[3],
            &source_accounts[4],
            &source_payers[0], &source_payers[1], &source_payers[2], &source_payers[3],
            &source_payers[4],
            &source_principals[0].to_le_bytes(), &source_principals[1].to_le_bytes(),
            &source_principals[2].to_le_bytes(), &source_principals[3].to_le_bytes(),
            &source_principals[4].to_le_bytes(),
            &source_balances[0].to_le_bytes(), &source_balances[1].to_le_bytes(),
            &source_balances[2].to_le_bytes(), &source_balances[3].to_le_bytes(),
            &source_balances[4].to_le_bytes(),
            &[self.refund_count],
            &refund_recipients[0], &refund_recipients[1], &refund_recipients[2],
            &refund_recipients[3], &refund_recipients[4],
            &refund_amounts[0].to_le_bytes(), &refund_amounts[1].to_le_bytes(),
            &refund_amounts[2].to_le_bytes(), &refund_amounts[3].to_le_bytes(),
            &refund_amounts[4].to_le_bytes(),
            &self.neutral_lamport_sink,
            &self.surplus_lamports.to_le_bytes(),
        ]);
        require_live(id)?;
        Ok(id)
    }
}

/// Derive the sole canonical principal-refund/surplus vector from an
/// unordered fixed-capacity source set. The active count, sorted source
/// prefix, coalesced payer prefix, and surplus are all derived here; no caller
/// count or refund amount is accepted.
pub fn build_direct_retirement_transfer_v1(
    supplied: [Option<DirectRetirementSourceV1>; MAX_DIRECT_RETIREMENT_SOURCES_V1],
    neutral_lamport_sink: [u8; 32],
) -> Result<DirectRetirementTransferV1, DirectMarketErrorV1> {
    require_live(neutral_lamport_sink)?;
    let mut sources: [Option<DirectRetirementSourceV1>; MAX_DIRECT_RETIREMENT_SOURCES_V1] =
        [None; MAX_DIRECT_RETIREMENT_SOURCES_V1];
    let mut source_count = 0usize;
    let mut index = 0usize;
    while index < MAX_DIRECT_RETIREMENT_SOURCES_V1 {
        if let Some(source) = supplied[index] {
            source.validate()?;
            if source.account == neutral_lamport_sink {
                return Err(DirectMarketErrorV1::IdentityAlias);
            }
            let mut at = 0usize;
            while at < source_count {
                let current = sources[at].ok_or(DirectMarketErrorV1::InvalidCount)?;
                if current.account >= source.account {
                    break;
                }
                at += 1;
            }
            if at < source_count
                && sources[at].ok_or(DirectMarketErrorV1::InvalidCount)?.account
                    == source.account
            {
                return Err(DirectMarketErrorV1::IdentityAlias);
            }
            let mut cursor = source_count;
            while cursor > at {
                sources[cursor] = sources[cursor - 1];
                cursor -= 1;
            }
            sources[at] = Some(source);
            source_count = source_count
                .checked_add(1)
                .ok_or(DirectMarketErrorV1::Arithmetic)?;
        }
        index += 1;
    }
    if source_count == 0 {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let mut payers = [[0u8; 32]; MAX_DIRECT_REFUND_RECIPIENTS_V1];
    let mut amounts = [0u64; MAX_DIRECT_REFUND_RECIPIENTS_V1];
    let mut refund_count = 0usize;
    let mut surplus_lamports = 0u64;
    index = 0;
    while index < source_count {
        let source = sources[index].ok_or(DirectMarketErrorV1::InvalidCount)?;
        insert_refund(
            &mut payers,
            &mut amounts,
            &mut refund_count,
            source.rent.payer,
            source.rent.principal_lamports,
        )?;
        surplus_lamports = surplus_lamports
            .checked_add(source.surplus_lamports()?)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        index += 1;
    }
    let mut refunds = [None; MAX_DIRECT_REFUND_RECIPIENTS_V1];
    index = 0;
    while index < refund_count {
        refunds[index] = Some(DirectPrincipalRefundV1 {
            recipient: payers[index],
            lamports: amounts[index],
        });
        index += 1;
    }
    let transfer = DirectRetirementTransferV1 {
        sources,
        source_count: u8::try_from(source_count).map_err(|_| DirectMarketErrorV1::Arithmetic)?,
        refunds,
        refund_count: u8::try_from(refund_count).map_err(|_| DirectMarketErrorV1::Arithmetic)?,
        neutral_lamport_sink,
        surplus_lamports,
    };
    transfer.validate()?;
    Ok(transfer)
}

/// Immutable Direct binding to Product, Realm collateral, and Resolution V5.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectMarketBindingV1 {
    /// Full MarketInstanceV2 semantic identity.
    pub market_instance_id: [u8; 32],
    /// Shared nonzero Market/Resolution generation.
    pub generation: u64,
    /// Active outcome width in `2..=16`.
    pub outcome_count: u8,
    /// Immutable Realm identity.
    pub realm_id: [u8; 32],
    /// Realm-selected collateral profile.
    pub collateral_profile_id: [u8; 32],
    /// Immutable collateral policy.
    pub collateral_policy_id: [u8; 32],
    /// Reviewed collateral adapter release.
    pub collateral_release_id: [u8; 32],
    /// Canonical Resolution V5 account.
    pub resolution_account: [u8; 32],
    /// Direct window identity derived before resolution from the immutable
    /// Market, General owner, and complete schedule.
    pub direct_epoch_semantics_id: [u8; 32],
    /// Exact revenue-policy identity selected by GenesisV2.
    pub revenue_policy_id: [u8; 32],
    /// Exact current General batch-policy identity.
    pub batch_policy_id: [u8; 32],
    /// Canonical Direct projection of both complete fee owners.
    pub direct_fee_shape_id: [u8; 32],
    /// Revenue treasury owner, or the authenticated unset sentinel at zero fee.
    pub fee_treasury_owner: [u8; 32],
    /// Composite dispersion rate numerator.
    pub fee_dispersion_bps: u32,
    /// Composite quotient-range rate numerator.
    pub fee_floor_range_bps: u32,
    /// Standing-maker split numerator.
    pub fee_maker_rebate_num: u32,
    /// Treasury split numerator.
    pub fee_treasury_num: u32,
    /// Exact split denominator.
    pub fee_split_den: u32,
    /// Product-selected candidate lifecycle policy.
    pub candidate_lifecycle_policy_id: [u8; 32],
    /// Product-selected present-funded candidate liveness policy.
    pub candidate_liveness_policy_id: [u8; 32],
    /// Product-authenticated allocation from the complete global liveness bundle.
    pub candidate_liveness: liveness_v1::DirectCandidateLivenessBindingV1,
    /// Direct release-owned timing projection over those two Product owners.
    pub direct_schedule_policy_id: [u8; 32],
    /// Product MarketLifecycleRoot account.
    pub product_root_account: [u8; 32],
    /// Exact immutable Product MarketLifecycle binding identity joined by the
    /// current General V3 owner.
    pub product_market_binding_id: [u8; 32],
    /// Exact Product family-aggregator prestate which admitted this occurrence.
    pub product_family_prestate_id: [u8; 32],
    /// One-way Product preauthorization persisted by the current General V3
    /// owner before General-family admission.
    pub general_product_preauthorization_id: [u8; 32],
    /// Zero-based Product Direct-family admission coordinate for this occurrence.
    pub family_admission_sequence: u32,
    /// Exact founder SeriesMarketLink account.
    pub founder_series_link_account: [u8; 32],
    /// Immutable founder SeriesMarketLink binding identity.
    pub founder_series_link_binding_id: [u8; 32],
    /// Exact current CompiledProductSeriesBundleV5 identity.
    pub compiler_bundle_v5_id: [u8; 32],
    /// Exact founder SeriesPlanV5 identity.
    pub founder_series_plan_id: [u8; 32],
    /// Finite founder Series ordinal.
    pub founder_series_ordinal: u32,
    /// Product-assigned canonical Direct family-root account.
    pub direct_root_account: [u8; 32],
    /// Permanent Direct action replay/receipt account.
    pub action_replay_account: [u8; 32],
    /// Exact current General MarketBinding account.
    pub general_market_binding: [u8; 32],
    /// Existing General owner-balance runtime for PositionV3/GEN1.
    pub general_market_runtime: [u8; 32],
    /// Realm-authenticated neutral lamport sink.
    pub neutral_lamport_sink: [u8; 32],
    /// Exact RelationV2 policy identity.
    pub relation_policy_id: [u8; 32],
    /// Exact quantized price policy identity.
    pub price_policy_id: [u8; 32],
    /// Exact integer simplex/cash scale.
    pub price_scale: u64,
}

impl DirectMarketBindingV1 {
    /// Validate active width, scale, identities, and cross-role separation.
    pub fn validate(self) -> Result<(), DirectMarketErrorV1> {
        if self.generation == 0
            || !(2..=16).contains(&usize::from(self.outcome_count))
            || self.price_scale == 0
        {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        let semantic_ids = [
            self.market_instance_id,
            self.realm_id,
            self.collateral_profile_id,
            self.collateral_policy_id,
            self.collateral_release_id,
            self.direct_epoch_semantics_id,
            self.revenue_policy_id,
            self.batch_policy_id,
            self.direct_fee_shape_id,
            self.candidate_lifecycle_policy_id,
            self.candidate_liveness_policy_id,
            self.direct_schedule_policy_id,
            self.product_market_binding_id,
            self.product_family_prestate_id,
            self.general_product_preauthorization_id,
            self.founder_series_link_binding_id,
            self.compiler_bundle_v5_id,
            self.founder_series_plan_id,
            self.relation_policy_id,
            self.price_policy_id,
        ];
        let mut index = 0usize;
        while index < semantic_ids.len() {
            require_live(semantic_ids[index])?;
            index += 1;
        }
        self.fee_policy().validate()?;
        self.candidate_liveness.validate()?;
        require_distinct(&[
            self.resolution_account,
            self.product_root_account,
            self.founder_series_link_account,
            self.direct_root_account,
            self.action_replay_account,
            self.general_market_binding,
            self.general_market_runtime,
            self.neutral_lamport_sink,
            self.candidate_liveness.policy_account,
            self.candidate_liveness.candidate_account,
        ])
    }

    /// Reconstruct the exact copied fee-policy projection.
    pub const fn fee_policy(self) -> fee_v1::DirectFeePolicyV1 {
        fee_v1::DirectFeePolicyV1 {
            batch_policy_id: self.batch_policy_id,
            revenue_policy_id: self.revenue_policy_id,
            treasury_owner: self.fee_treasury_owner,
            dispersion_bps: self.fee_dispersion_bps,
            floor_range_bps: self.fee_floor_range_bps,
            maker_rebate_num: self.fee_maker_rebate_num,
            treasury_num: self.fee_treasury_num,
            split_den: self.fee_split_den,
        }
    }
}

fn validate_product_join_v1(
    product_root: &MarketLifecycleRootV1,
    founder_link: &SeriesMarketLinkV1,
    compiler_bundle: &CompiledProductSeriesBundleV5,
    direct: DirectMarketBindingV1,
    terminal_join: bool,
) -> Result<[u8; 32], DirectMarketErrorV1> {
    let product = product_root.binding();
    let link = founder_link.binding();
    let link_semantic_id = founder_link
        .semantic_id()
        .map_err(|_| DirectMarketErrorV1::Product)?
        .bytes();
    let link_binding_id = link
        .id()
        .map_err(|_| DirectMarketErrorV1::Product)?
        .bytes();
    let bundle_id = compiler_bundle
        .id()
        .map_err(|_| DirectMarketErrorV1::Product)?
        .bytes();
    let phase_matches = if terminal_join {
        matches!(
            founder_link.phase(),
            SeriesMarketLinkPhaseV1::Active | SeriesMarketLinkPhaseV1::Retiring
        )
    } else {
        product_root.phase() == MarketLifecyclePhaseV1::Active
            && founder_link.phase() == SeriesMarketLinkPhaseV1::Active
    };
    if !phase_matches
        || link.disposition != SeriesMarketDispositionV1::Founder
        || link.market_instance_id.bytes() != direct.market_instance_id
        || link.market_root_account_id.bytes() != direct.product_root_account
        || link.market_binding_id
            != product
                .id()
                .map_err(|_| DirectMarketErrorV1::Product)?
        || link.market_binding_id.bytes() != direct.product_market_binding_id
        || link.generation != direct.generation
        || link.capability_profile_id.bytes() != product.capability_profile_id.bytes()
        || link.neutral_lamport_sink.bytes() != direct.neutral_lamport_sink
        || link_binding_id != direct.founder_series_link_binding_id
        || link.compiler_output_id.bytes() != direct.compiler_bundle_v5_id
        || link.series_plan_id.bytes() != direct.founder_series_plan_id
        || link.ordinal != direct.founder_series_ordinal
        || bundle_id != direct.compiler_bundle_v5_id
        || compiler_bundle.registry_release_id != product.registry_release_id
        || compiler_bundle.capability_profile_id.content_id() != product.capability_profile_id
        || compiler_bundle.source_release_manifest_id != product.source_release_id
        || compiler_bundle.source_plane_contract_id != product.source_plane_contract_id
        || compiler_bundle.source_spec_id != product.source_spec_id
        || compiler_bundle.native_claim_basis_id.content_id() != product.native_claim_basis_id
        || compiler_bundle.evidence_only_recovery_policy_id.content_id()
            != product.recovery_policy_id
        || compiler_bundle.product_template_id.content_id() != product.product_template_id
        || compiler_bundle.price_measure_policy_id.content_id()
            != product.price_measure_policy_id
        || compiler_bundle.market_genesis_profile_id.content_id()
            != product.market_genesis_profile_id
        || compiler_bundle.series_plan_id != link.series_plan_id
        || compiler_bundle.funding_terms_id != link.funding_terms_id
        || compiler_bundle.funding_quote_id != link.funding_quote_id
        || compiler_bundle.attachment_plan_id.content_id() != link.attachment_plan_id
        || link.source_release_id != product.source_release_id
        || link.source_plane_contract_id != product.source_plane_contract_id
        || link.source_spec_id != product.source_spec_id
        || link.source_route_id != product.source_route_id
        || link.clock_policy_id != product.clock_policy_id
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    let _ = terminal_join;
    Ok(link_semantic_id)
}

/// Exact finalized Resolution V5 identities consumed only by action 13.
///
/// Foundation deliberately cannot carry these values: Product has not yet
/// activated resolution while a Direct admission window is being founded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFinalResolutionV1 {
    /// Canonical Resolution V5 account fixed at Product foundation.
    pub account: [u8; 32],
    /// Exact finalized Resolution V5 semantic identity.
    pub semantic_id: [u8; 32],
    /// Exact finalized hostile-byte data identity.
    pub data_id: [u8; 32],
}

impl DirectFinalResolutionV1 {
    fn validate(
        self,
        binding: DirectMarketBindingV1,
        product_root: &MarketLifecycleRootV1,
    ) -> Result<(), DirectMarketErrorV1> {
        require_live(self.semantic_id)?;
        require_live(self.data_id)?;
        if self.account != binding.resolution_account
            || product_root.binding().resolution_account_id.bytes() != self.account
            || product_root.resolution_semantic_id().bytes() != self.semantic_id
            || product_root.resolution_data_id().bytes() != self.data_id
        {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        Ok(())
    }
}

/// Derive the pre-resolution Direct window identity from immutable owners.
pub fn direct_epoch_semantics_id_v1<B: DirectHashBackendV1>(
    binding: DirectMarketBindingV1,
    schedule: DirectScheduleV1,
    backend: &B,
) -> Result<[u8; 32], DirectMarketErrorV1> {
    schedule.validate()?;
    let id = backend.sha256_parts(&[
        DIRECT_EPOCH_SEMANTICS_DOMAIN_V1,
        &binding.market_instance_id,
        &binding.generation.to_le_bytes(),
        &binding.direct_root_account,
        &binding.direct_schedule_policy_id,
        &binding.product_market_binding_id,
        &binding.product_family_prestate_id,
        &binding.general_product_preauthorization_id,
        &binding.family_admission_sequence.to_le_bytes(),
        &binding.general_market_binding,
        &binding.general_market_runtime,
        &binding.candidate_liveness.global_lifecycle_id,
        &binding.candidate_liveness.global_bundle_binding_id,
        &binding.candidate_liveness.global_capitalization_receipt_id,
        &binding.candidate_liveness.allocation_receipt_id,
        &binding.candidate_liveness.work_schedule_id,
        &schedule.admission_opens_slot.to_le_bytes(),
        &schedule.admission_closes_slot.to_le_bytes(),
        &schedule.submission_closes_slot.to_le_bytes(),
        &schedule.selection_deadline_slot.to_le_bytes(),
        &schedule.settlement_deadline_slot.to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(id)
}

/// Derive the only Direct V1 timing projection selected by the release.
pub fn direct_schedule_policy_id_v1<B: DirectHashBackendV1>(
    binding: DirectMarketBindingV1,
    backend: &B,
) -> Result<[u8; 32], DirectMarketErrorV1> {
    let id = backend.sha256_parts(&[
        DIRECT_SCHEDULE_POLICY_DOMAIN_V1,
        &binding.candidate_lifecycle_policy_id,
        &binding.candidate_liveness_policy_id,
        &binding.candidate_liveness.work_schedule_id,
        &binding.candidate_liveness.allocation_receipt_id,
        &DIRECT_ADMISSION_SPAN_SLOTS_V1.to_le_bytes(),
        &DIRECT_SUBMISSION_SPAN_SLOTS_V1.to_le_bytes(),
        &DIRECT_VERIFICATION_SPAN_SLOTS_V1.to_le_bytes(),
        &DIRECT_SETTLEMENT_SPAN_SLOTS_V1.to_le_bytes(),
        &[MAX_DIRECT_CANDIDATES_V1],
    ]);
    require_live(id)?;
    Ok(id)
}

/// Immutable half-open Direct schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectScheduleV1 {
    /// First slot which admits a funded Reservation.
    pub admission_opens_slot: u64,
    /// Exclusive Reservation admission and cancellation close.
    pub admission_closes_slot: u64,
    /// Exclusive candidate submission close.
    pub submission_closes_slot: u64,
    /// Exclusive exhaustive selection deadline.
    pub selection_deadline_slot: u64,
    /// Exclusive selected-pair settlement deadline.
    pub settlement_deadline_slot: u64,
}

impl DirectScheduleV1 {
    /// Stamp the bounded V1 schedule from Clock and authenticated candidate policy spans.
    pub fn canonical_from_foundation_slot(
        foundation_slot: u64,
    ) -> Result<Self, DirectMarketErrorV1> {
        let admission_closes_slot = foundation_slot
            .checked_add(DIRECT_ADMISSION_SPAN_SLOTS_V1)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        let submission_closes_slot = admission_closes_slot
            .checked_add(DIRECT_SUBMISSION_SPAN_SLOTS_V1)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        let selection_deadline_slot = submission_closes_slot
            .checked_add(DIRECT_VERIFICATION_SPAN_SLOTS_V1)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        let settlement_deadline_slot = selection_deadline_slot
            .checked_add(DIRECT_SETTLEMENT_SPAN_SLOTS_V1)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        let value = Self {
            admission_opens_slot: foundation_slot,
            admission_closes_slot,
            submission_closes_slot,
            selection_deadline_slot,
            settlement_deadline_slot,
        };
        value.validate()?;
        Ok(value)
    }

    /// Require a strictly ordered finite schedule.
    pub fn validate(self) -> Result<(), DirectMarketErrorV1> {
        if self.admission_opens_slot >= self.admission_closes_slot
            || self.admission_closes_slot >= self.submission_closes_slot
            || self.submission_closes_slot >= self.selection_deadline_slot
            || self.selection_deadline_slot >= self.settlement_deadline_slot
        {
            Err(DirectMarketErrorV1::InvalidCount)
        } else {
            Ok(())
        }
    }
}

impl DirectMarketBindingV1 {
    /// One-based Direct occurrence coordinate used by Relation expiry facts.
    pub fn direct_window_index(self) -> Result<u64, DirectMarketErrorV1> {
        u64::from(self.family_admission_sequence)
            .checked_add(1)
            .ok_or(DirectMarketErrorV1::Arithmetic)
    }
}

/// Exhaustive Direct root phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRootPhaseV1 {
    /// Zero-to-two Reservations may be admitted or cancelled.
    Open,
    /// Frozen exhaustive prefix contains no complete pair.
    FrozenEmpty,
    /// Exact pair exists and bounded candidate submission is open.
    SubmissionOpen,
    /// Retained candidates are traversed in canonical order.
    Verifying,
    /// One best valid submitted candidate is selected.
    Selected,
    /// Economics are terminal; deletion archives remain live.
    Terminal,
}

impl DirectRootPhaseV1 {
    /// Stable persisted byte.
    pub const fn byte(self) -> u8 {
        match self {
            Self::Open => 1,
            Self::FrozenEmpty => 2,
            Self::SubmissionOpen => 3,
            Self::Verifying => 4,
            Self::Selected => 5,
            Self::Terminal => 6,
        }
    }
}

/// Exhaustive economic terminal reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectTerminalReasonV1 {
    /// The open root was never frozen before candidate submission closed.
    MissedFreezeLapse,
    /// No complete pair existed when the prefix froze.
    EmptyLapse,
    /// Action 8 finalized an exhaustive empty candidate traversal as no-trade.
    NoCandidate,
    /// Candidate work did not select before its deadline.
    UnselectedLapse,
    /// Selected authority expired before settlement.
    SelectedLapse,
    /// The exact selected pair settled.
    Settled,
}

impl DirectTerminalReasonV1 {
    /// Stable persisted byte.
    pub const fn byte(self) -> u8 {
        match self {
            Self::EmptyLapse => 1,
            Self::UnselectedLapse => 2,
            Self::SelectedLapse => 3,
            Self::Settled => 4,
            Self::MissedFreezeLapse => 5,
            Self::NoCandidate => 6,
        }
    }
}

/// Direct-owned deletable root state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectMarketRootV1 {
    binding: DirectMarketBindingV1,
    schedule: DirectScheduleV1,
    root_rent: DirectRentOwnerV1,
    phase: DirectRootPhaseV1,
    terminal_reason: Option<DirectTerminalReasonV1>,
    admitted_reservations: u8,
    live_reservations: u8,
    retired_reservations: u8,
    reservation_accounts: [[u8; 32]; 2],
    reservation_semantic_ids: [[u8; 32]; 2],
    selection_account: [u8; 32],
}

impl DirectMarketRootV1 {
    /// Immutable Product/Realm/Resolution binding.
    pub const fn binding(self) -> DirectMarketBindingV1 { self.binding }
    /// Immutable schedule.
    pub const fn schedule(self) -> DirectScheduleV1 { self.schedule }
    /// Root rent ownership.
    pub const fn root_rent(self) -> DirectRentOwnerV1 { self.root_rent }
    /// Exhaustive phase.
    pub const fn phase(self) -> DirectRootPhaseV1 { self.phase }
    /// Terminal reason, absent before terminality.
    pub const fn terminal_reason(self) -> Option<DirectTerminalReasonV1> { self.terminal_reason }
    /// Historical funded Reservation count.
    pub const fn admitted_reservations(self) -> u8 { self.admitted_reservations }
    /// Persisted Reservation archives not yet retired.
    pub const fn live_reservations(self) -> u8 { self.live_reservations }
    /// Reservation archives already retired by cancellation.
    pub const fn retired_reservations(self) -> u8 { self.retired_reservations }
    /// Exact active Reservation account at one compact root coordinate.
    pub fn reservation_account(self, index: u8) -> Result<[u8; 32], DirectMarketErrorV1> {
        let at = usize::from(index);
        if at >= usize::from(self.live_reservations) {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        Ok(self.reservation_accounts[at])
    }
    /// Exact active Reservation semantic ID at one compact root coordinate.
    pub fn reservation_semantic_id(
        self,
        index: u8,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        let at = usize::from(index);
        if at >= usize::from(self.live_reservations) {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        Ok(self.reservation_semantic_ids[at])
    }
    /// Canonical Selection account, zero before freeze.
    pub const fn selection_account(self) -> [u8; 32] { self.selection_account }

    /// Validate the exhaustive count and phase partition.
    pub fn validate(self) -> Result<(), DirectMarketErrorV1> {
        self.binding.validate()?;
        self.schedule.validate()?;
        self.root_rent.validate()?;
        if self.admitted_reservations > MAX_DIRECT_RESERVATIONS_V1
            || self.live_reservations > self.admitted_reservations
            || self.retired_reservations > self.admitted_reservations
            || self.live_reservations.checked_add(self.retired_reservations)
                != Some(self.admitted_reservations)
        {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        let mut index = 0usize;
        while index < 2 {
            if index < usize::from(self.live_reservations) {
                require_fresh_child_account(self.binding, self.reservation_accounts[index])?;
                require_live(self.reservation_semantic_ids[index])?;
                if index != 0
                    && self.reservation_accounts[index - 1] == self.reservation_accounts[index]
                {
                    return Err(DirectMarketErrorV1::IdentityAlias);
                }
            } else if self.reservation_accounts[index] != [0; 32]
                || self.reservation_semantic_ids[index] != [0; 32]
            {
                return Err(DirectMarketErrorV1::InvalidCount);
            }
            index += 1;
        }
        match self.phase {
            DirectRootPhaseV1::Open => {
                if self.selection_account != [0; 32] || self.terminal_reason.is_some() {
                    return Err(DirectMarketErrorV1::WrongPhase);
                }
            }
            DirectRootPhaseV1::FrozenEmpty | DirectRootPhaseV1::SubmissionOpen
            | DirectRootPhaseV1::Verifying | DirectRootPhaseV1::Selected => {
                require_live(self.selection_account)?;
                if self.terminal_reason.is_some() {
                    return Err(DirectMarketErrorV1::WrongPhase);
                }
            }
            DirectRootPhaseV1::Terminal => {
                require_live(self.selection_account)?;
                if self.terminal_reason.is_none() {
                    return Err(DirectMarketErrorV1::WrongPhase);
                }
            }
        }
        Ok(())
    }

    /// Domain-separated identity of complete deletable root state.
    pub fn semantic_id<B: DirectHashBackendV1>(
        self,
        backend: &B,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        self.validate()?;
        let terminal = self.terminal_reason.map_or(0, DirectTerminalReasonV1::byte);
        let id = backend.sha256_parts(&[
            ROOT_STATE_DOMAIN_V1,
            &self.binding.market_instance_id, &self.binding.generation.to_le_bytes(),
            &[self.binding.outcome_count], &self.binding.realm_id,
            &self.binding.collateral_profile_id, &self.binding.collateral_policy_id,
            &self.binding.collateral_release_id, &self.binding.resolution_account,
            &self.binding.direct_epoch_semantics_id, &self.binding.revenue_policy_id,
            &self.binding.batch_policy_id, &self.binding.direct_fee_shape_id,
            &self.binding.fee_treasury_owner,
            &self.binding.fee_dispersion_bps.to_le_bytes(),
            &self.binding.fee_floor_range_bps.to_le_bytes(),
            &self.binding.fee_maker_rebate_num.to_le_bytes(),
            &self.binding.fee_treasury_num.to_le_bytes(),
            &self.binding.fee_split_den.to_le_bytes(),
            &self.binding.candidate_lifecycle_policy_id,
            &self.binding.candidate_liveness_policy_id,
            &self.binding.candidate_liveness.policy_account,
            &self.binding.candidate_liveness.policy_data_id,
            &self.binding.candidate_liveness.global_lifecycle_id,
            &self.binding.candidate_liveness.global_bundle_binding_id,
            &self.binding.candidate_liveness.global_capitalization_receipt_id,
            &self.binding.candidate_liveness.global_bundle_commitment_id,
            &self.binding.candidate_liveness.candidate_account,
            &self.binding.candidate_liveness.candidate_data_id,
            &self.binding.candidate_liveness.candidate_semantic_owner,
            &self.binding.candidate_liveness.candidate_quote_schedule_id,
            &self.binding.candidate_liveness.candidate_receipt_program_id,
            &self.binding.candidate_liveness.candidate_generation.to_le_bytes(),
            &self.binding.candidate_liveness.first_call_ordinal.to_le_bytes(),
            &self.binding.candidate_liveness.reserved_calls.to_le_bytes(),
            &self.binding.candidate_liveness.reserved_work_lamports.to_le_bytes(),
            &self.binding.candidate_liveness.allocation_receipt_id,
            &self.binding.candidate_liveness.work_schedule.freeze_book_lamports.to_le_bytes(),
            &self.binding.candidate_liveness.work_schedule.begin_verification_lamports.to_le_bytes(),
            &self.binding.candidate_liveness.work_schedule.verify_candidate_lamports.to_le_bytes(),
            &self.binding.candidate_liveness.work_schedule.finalize_selection_lamports.to_le_bytes(),
            &self.binding.candidate_liveness.work_schedule.economic_terminal_lamports.to_le_bytes(),
            &self.binding.candidate_liveness.work_schedule.retire_terminal_lamports.to_le_bytes(),
            &self.binding.candidate_liveness.work_schedule.retained_candidate_bond_lamports.to_le_bytes(),
            &self.binding.candidate_liveness.work_schedule_id,
            &self.binding.direct_schedule_policy_id,
            &self.binding.product_root_account, &self.binding.product_market_binding_id,
            &self.binding.product_family_prestate_id,
            &self.binding.general_product_preauthorization_id,
            &self.binding.family_admission_sequence.to_le_bytes(),
            &self.binding.founder_series_link_account,
            &self.binding.founder_series_link_binding_id, &self.binding.compiler_bundle_v5_id,
            &self.binding.founder_series_plan_id,
            &self.binding.founder_series_ordinal.to_le_bytes(),
            &self.binding.direct_root_account,
            &self.binding.action_replay_account, &self.binding.general_market_binding,
            &self.binding.general_market_runtime,
            &self.binding.neutral_lamport_sink, &self.binding.relation_policy_id,
            &self.binding.price_policy_id, &self.binding.price_scale.to_le_bytes(),
            &self.schedule.admission_opens_slot.to_le_bytes(),
            &self.schedule.admission_closes_slot.to_le_bytes(),
            &self.schedule.submission_closes_slot.to_le_bytes(),
            &self.schedule.selection_deadline_slot.to_le_bytes(),
            &self.schedule.settlement_deadline_slot.to_le_bytes(),
            &self.root_rent.payer, &self.root_rent.principal_lamports.to_le_bytes(),
            &self.root_rent.donation_floor_lamports.to_le_bytes(),
            &[self.phase.byte()], &[terminal], &[self.admitted_reservations],
            &[self.live_reservations], &[self.retired_reservations],
            &self.reservation_accounts[0], &self.reservation_accounts[1],
            &self.reservation_semantic_ids[0], &self.reservation_semantic_ids[1],
            &self.selection_account,
        ]);
        require_live(id)?;
        Ok(id)
    }
}

/// Permanent action receipt phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectReplayPhaseV1 {
    /// Direct root still exists and may consume an action.
    Active,
    /// Root and transient archives were retired; receipt is immutable.
    Terminal,
}

impl DirectReplayPhaseV1 {
    /// Stable persisted byte.
    pub const fn byte(self) -> u8 {
        match self { Self::Active => 1, Self::Terminal => 2 }
    }
}

/// Permanent `0xb3` action replay and terminal receipt owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectActionReplayV1 {
    market_instance_id: [u8; 32],
    generation: u64,
    direct_epoch_semantics_id: [u8; 32],
    direct_root_account: [u8; 32],
    replay_account: [u8; 32],
    rent: DirectRentOwnerV1,
    phase: DirectReplayPhaseV1,
    next_action_sequence: u64,
    action_transcript_id: [u8; 32],
    foundation_receipt_id: [u8; 32],
    economic_terminal_receipt_id: [u8; 32],
    family_terminal_receipt_id: [u8; 32],
    candidate_liveness_completed_calls: u32,
    candidate_liveness_last_receipt_id: [u8; 32],
    candidate_liveness_batch_receipt_id: [u8; 32],
    candidate_liveness_pending: bool,
}

impl DirectActionReplayV1 {
    /// Permanent account rent ownership.
    pub const fn rent(self) -> DirectRentOwnerV1 { self.rent }
    /// Active or immutable terminal replay phase.
    pub const fn phase(self) -> DirectReplayPhaseV1 { self.phase }
    /// Ordinal consumed by the next action.
    pub const fn next_action_sequence(self) -> u64 { self.next_action_sequence }
    /// Rolling once-only action transcript.
    pub const fn action_transcript_id(self) -> [u8; 32] { self.action_transcript_id }
    /// Direct-owned Product admission receipt.
    pub const fn foundation_receipt_id(self) -> [u8; 32] { self.foundation_receipt_id }
    /// Economic terminal receipt, zero before action 9..12.
    pub const fn economic_terminal_receipt_id(self) -> [u8; 32] {
        self.economic_terminal_receipt_id
    }
    /// Product family terminal receipt, zero before action 13.
    pub const fn family_terminal_receipt_id(self) -> [u8; 32] {
        self.family_terminal_receipt_id
    }
    /// Number of this occurrence's exact eight Candidate roles already consumed.
    pub const fn candidate_liveness_completed_calls(self) -> u32 {
        self.candidate_liveness_completed_calls
    }
    /// Last shared Candidate work receipt emitted by this occurrence.
    pub const fn candidate_liveness_last_receipt_id(self) -> [u8; 32] {
        self.candidate_liveness_last_receipt_id
    }
    /// Complete latest per-action Candidate receipt-batch commitment.
    pub const fn candidate_liveness_batch_receipt_id(self) -> [u8; 32] {
        self.candidate_liveness_batch_receipt_id
    }
    /// True only in a pure transient plan awaiting the atomic liveness join.
    pub const fn candidate_liveness_pending(self) -> bool {
        self.candidate_liveness_pending
    }

    /// Validate permanent replay facts against the exact current root.
    pub fn validate_against(self, root: DirectMarketRootV1) -> Result<(), DirectMarketErrorV1> {
        root.validate()?;
        self.rent.validate()?;
        require_live(self.market_instance_id)?;
        require_live(self.direct_epoch_semantics_id)?;
        require_live(self.direct_root_account)?;
        require_live(self.replay_account)?;
        require_live(self.action_transcript_id)?;
        require_live(self.foundation_receipt_id)?;
        let binding = root.binding;
        if self.generation == 0 || self.next_action_sequence == 0
            || self.market_instance_id != binding.market_instance_id
            || self.generation != binding.generation
            || self.direct_epoch_semantics_id != binding.direct_epoch_semantics_id
            || self.direct_root_account != binding.direct_root_account
            || self.replay_account != binding.action_replay_account
        {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        if self.candidate_liveness_completed_calls
            > liveness_v1::DIRECT_CANDIDATE_RESERVED_CALLS_V1
            || (self.candidate_liveness_completed_calls == 0)
                != (self.candidate_liveness_last_receipt_id == [0; 32])
            || (self.candidate_liveness_completed_calls == 0)
                != (self.candidate_liveness_batch_receipt_id == [0; 32])
        {
            return Err(DirectMarketErrorV1::Replay);
        }
        if self.candidate_liveness_completed_calls != 0 {
            require_live(self.candidate_liveness_last_receipt_id)?;
            require_live(self.candidate_liveness_batch_receipt_id)?;
        }
        if !self.candidate_liveness_pending {
            let progress_matches = match (root.phase, self.phase) {
                (DirectRootPhaseV1::Open, DirectReplayPhaseV1::Active) => {
                    self.candidate_liveness_completed_calls == 0
                }
                (
                    DirectRootPhaseV1::FrozenEmpty | DirectRootPhaseV1::SubmissionOpen,
                    DirectReplayPhaseV1::Active,
                ) => self.candidate_liveness_completed_calls == 1,
                (DirectRootPhaseV1::Verifying, DirectReplayPhaseV1::Active) => {
                    (2..=5).contains(&self.candidate_liveness_completed_calls)
                }
                (DirectRootPhaseV1::Selected, DirectReplayPhaseV1::Active) => {
                    self.candidate_liveness_completed_calls == 6
                }
                (DirectRootPhaseV1::Terminal, DirectReplayPhaseV1::Active) => {
                    self.candidate_liveness_completed_calls == 7
                }
                (DirectRootPhaseV1::Terminal, DirectReplayPhaseV1::Terminal) => {
                    self.candidate_liveness_completed_calls == 8
                }
                _ => false,
            };
            if !progress_matches {
                return Err(DirectMarketErrorV1::Replay);
            }
        }
        match (root.phase, self.phase) {
            (DirectRootPhaseV1::Terminal, DirectReplayPhaseV1::Active) => {
                require_live(self.economic_terminal_receipt_id)?;
                if self.family_terminal_receipt_id != [0; 32] {
                    return Err(DirectMarketErrorV1::Replay);
                }
            }
            (DirectRootPhaseV1::Terminal, DirectReplayPhaseV1::Terminal) => {
                require_live(self.economic_terminal_receipt_id)?;
                require_live(self.family_terminal_receipt_id)?;
            }
            (_, DirectReplayPhaseV1::Active) => {
                if self.economic_terminal_receipt_id != [0; 32]
                    || self.family_terminal_receipt_id != [0; 32]
                {
                    return Err(DirectMarketErrorV1::Replay);
                }
            }
            (_, DirectReplayPhaseV1::Terminal) => return Err(DirectMarketErrorV1::WrongPhase),
        }
        Ok(())
    }

    /// Domain-separated permanent hostile-state identity.
    pub fn semantic_id<B: DirectHashBackendV1>(
        self,
        root: DirectMarketRootV1,
        backend: &B,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        self.validate_against(root)?;
        let id = backend.sha256_parts(&[
            REPLAY_STATE_DOMAIN_V1, &self.market_instance_id, &self.generation.to_le_bytes(),
            &self.direct_epoch_semantics_id,
            &self.direct_root_account, &self.replay_account, &self.rent.payer,
            &self.rent.principal_lamports.to_le_bytes(),
            &self.rent.donation_floor_lamports.to_le_bytes(), &[self.phase.byte()],
            &self.next_action_sequence.to_le_bytes(), &self.action_transcript_id,
            &self.foundation_receipt_id, &self.economic_terminal_receipt_id,
            &self.family_terminal_receipt_id,
            &self.candidate_liveness_completed_calls.to_le_bytes(),
            &self.candidate_liveness_last_receipt_id,
            &self.candidate_liveness_batch_receipt_id,
            &[u8::from(self.candidate_liveness_pending)],
        ]);
        require_live(id)?;
        Ok(id)
    }

    fn require_action(
        self,
        root: DirectMarketRootV1,
        consumed_sequence: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        self.validate_against(root)?;
        if self.phase != DirectReplayPhaseV1::Active
            || consumed_sequence != self.next_action_sequence
        {
            return Err(DirectMarketErrorV1::Replay);
        }
        Ok(())
    }

    fn advance<B: DirectHashBackendV1>(
        mut self,
        root_pre_id: [u8; 32],
        root_post_id: [u8; 32],
        action: DirectMarketActionV1,
        observed_slot: u64,
        evidence_id: [u8; 32],
        backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        require_live(root_pre_id)?;
        require_live(root_post_id)?;
        require_live(evidence_id)?;
        let consumed = self.next_action_sequence;
        self.next_action_sequence = consumed.checked_add(1).ok_or(DirectMarketErrorV1::Arithmetic)?;
        self.action_transcript_id = backend.sha256_parts(&[
            ACTION_TRANSCRIPT_DOMAIN_V1, &self.market_instance_id, &self.direct_root_account,
            &consumed.to_le_bytes(), &[action.byte()], &root_pre_id, &root_post_id,
            &observed_slot.to_le_bytes(), &evidence_id, &self.action_transcript_id,
        ]);
        require_live(self.action_transcript_id)?;
        self.candidate_liveness_pending = self.candidate_liveness_pending
            || action.requires_candidate_liveness();
        Ok(self)
    }

    pub(crate) fn bind_candidate_liveness_batch<B: DirectHashBackendV1>(
        mut self,
        root: DirectMarketRootV1,
        batch: liveness_v1::DirectCandidateWorkBatchV1,
        backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        self.validate_against(root)?;
        if !self.candidate_liveness_pending
            || batch.completed_calls_before() != self.candidate_liveness_completed_calls
            || (self.candidate_liveness_completed_calls != 0
                && batch.predecessor_receipt_id()
                    != self.candidate_liveness_last_receipt_id)
        {
            return Err(DirectMarketErrorV1::Replay);
        }
        batch.validate_replay_binding(self, root)?;
        let prior_action_transcript = self.action_transcript_id;
        self.candidate_liveness_completed_calls = batch.completed_calls_after();
        self.candidate_liveness_last_receipt_id = batch.last_receipt_id();
        self.candidate_liveness_batch_receipt_id = batch.batch_receipt_id();
        self.candidate_liveness_pending = false;
        self.action_transcript_id = backend.sha256_parts(&[
            REPLAY_LIVENESS_BATCH_DOMAIN_V1,
            &self.market_instance_id,
            &self.direct_root_account,
            &prior_action_transcript,
            &self.candidate_liveness_completed_calls.to_le_bytes(),
            &self.candidate_liveness_last_receipt_id,
            &self.candidate_liveness_batch_receipt_id,
        ]);
        require_live(self.action_transcript_id)?;
        self.validate_against(root)?;
        Ok(self)
    }
}

/// Stable current-family local Direct action coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectMarketActionV1 {
    /// Create root and permanent replay and admit Product family child.
    InitializeMarket,
    /// Admit one fresh funded Reservation.
    AdmitOrder,
    /// Cancel and retire one funded Reservation.
    CancelOrder,
    /// Freeze the complete Reservation prefix into Selection.
    FreezeBook,
    /// Retain one bounded candidate.
    SubmitCandidate,
    /// Begin exact candidate traversal.
    BeginVerification,
    /// Verify one exact next candidate coordinate.
    VerifyCandidate,
    /// Commit the best valid submitted candidate.
    FinalizeSelection,
    /// Apply exact Egg/cash and Position/GEN1 effects.
    SettlePair,
    /// Terminalize an incomplete frozen prefix.
    LapseEmpty,
    /// Terminalize traversal without a selected candidate.
    LapseUnselected,
    /// Terminalize an expired selected candidate.
    LapseSelected,
    /// Close the complete live archive set and terminalize Product family.
    RetireTerminal,
}

impl DirectMarketActionV1 {
    /// Stable local action byte under family `80/1`.
    pub const fn byte(self) -> u8 {
        match self {
            Self::InitializeMarket => 1, Self::AdmitOrder => 2, Self::CancelOrder => 3,
            Self::FreezeBook => 4, Self::SubmitCandidate => 5, Self::BeginVerification => 6,
            Self::VerifyCandidate => 7, Self::FinalizeSelection => 8, Self::SettlePair => 9,
            Self::LapseEmpty => 10, Self::LapseUnselected => 11,
            Self::LapseSelected => 12, Self::RetireTerminal => 13,
        }
    }

    /// Whether this action must join one or more exact Candidate work roles.
    pub const fn requires_candidate_liveness(self) -> bool {
        matches!(
            self,
            Self::FreezeBook
                | Self::BeginVerification
                | Self::VerifyCandidate
                | Self::FinalizeSelection
                | Self::SettlePair
                | Self::LapseEmpty
                | Self::LapseUnselected
                | Self::LapseSelected
                | Self::RetireTerminal
        )
    }
}

/// Atomic root and permanent-replay poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRootReplayPostV1 {
    /// Exact deletable root poststate.
    pub root: DirectMarketRootV1,
    /// Exact permanent action receipt poststate.
    pub replay: DirectActionReplayV1,
}

impl DirectRootReplayPostV1 {
    /// Admit one funded Reservation archive.
    pub(crate) fn admit_reservation<B: DirectHashBackendV1>(
        self,
        consumed_sequence: u64,
        observed_slot: u64,
        reservation_account: [u8; 32],
        reservation_poststate_id: [u8; 32],
        admission_receipt_id: [u8; 32],
        backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        self.replay.require_action(self.root, consumed_sequence)?;
        require_live(reservation_account)?;
        require_live(reservation_poststate_id)?;
        require_live(admission_receipt_id)?;
        require_fresh_child_account(self.root.binding, reservation_account)?;
        if self.root.phase != DirectRootPhaseV1::Open
            || observed_slot < self.root.schedule.admission_opens_slot
            || observed_slot >= self.root.schedule.admission_closes_slot
            || self.root.admitted_reservations >= MAX_DIRECT_RESERVATIONS_V1
        {
            return Err(DirectMarketErrorV1::WrongPhase);
        }
        let root_pre_id = self.root.semantic_id(backend)?;
        let mut root = self.root;
        let at = usize::from(root.live_reservations);
        root.reservation_accounts[at] = reservation_account;
        root.reservation_semantic_ids[at] = reservation_poststate_id;
        root.admitted_reservations = root.admitted_reservations.checked_add(1)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        root.live_reservations = root.live_reservations.checked_add(1)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        let root_post_id = root.semantic_id(backend)?;
        let replay = self.replay.advance(root_pre_id, root_post_id,
            DirectMarketActionV1::AdmitOrder, observed_slot, admission_receipt_id, backend)?;
        replay.validate_against(root)?;
        Ok(Self { root, replay })
    }

    /// Record one exact cancelled Reservation retirement.
    ///
    /// `reservation_retirement_id` binds Reservation terminal state, exact
    /// principal refund, and all surplus to the neutral sink.
    pub(crate) fn cancel_reservation<B: DirectHashBackendV1>(
        self,
        consumed_sequence: u64,
        observed_slot: u64,
        reservation_account: [u8; 32],
        reservation_prestate_id: [u8; 32],
        reservation_retirement_id: [u8; 32],
        backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        self.replay.require_action(self.root, consumed_sequence)?;
        require_live(reservation_account)?;
        require_live(reservation_prestate_id)?;
        require_live(reservation_retirement_id)?;
        if self.root.phase != DirectRootPhaseV1::Open
            || observed_slot < self.root.schedule.admission_opens_slot
            || observed_slot >= self.root.schedule.admission_closes_slot
            || self.root.live_reservations == 0
        {
            return Err(DirectMarketErrorV1::WrongPhase);
        }
        let root_pre_id = self.root.semantic_id(backend)?;
        let mut root = self.root;
        let mut found = None;
        let mut index = 0usize;
        while index < usize::from(root.live_reservations) {
            if root.reservation_accounts[index] == reservation_account
                && root.reservation_semantic_ids[index] == reservation_prestate_id
            {
                found = Some(index);
                break;
            }
            index += 1;
        }
        let at = found.ok_or(DirectMarketErrorV1::MismatchedBinding)?;
        let last = usize::from(root.live_reservations)
            .checked_sub(1)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        root.reservation_accounts[at] = root.reservation_accounts[last];
        root.reservation_semantic_ids[at] = root.reservation_semantic_ids[last];
        root.reservation_accounts[last] = [0; 32];
        root.reservation_semantic_ids[last] = [0; 32];
        root.live_reservations = root.live_reservations.checked_sub(1)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        root.retired_reservations = root.retired_reservations.checked_add(1)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        let root_post_id = root.semantic_id(backend)?;
        let replay = self.replay.advance(root_pre_id, root_post_id,
            DirectMarketActionV1::CancelOrder, observed_slot, reservation_retirement_id, backend)?;
        replay.validate_against(root)?;
        Ok(Self { root, replay })
    }

    /// Freeze the exhaustive zero-, one-, or two-Reservation prefix.
    pub(crate) fn freeze<B: DirectHashBackendV1>(
        self,
        consumed_sequence: u64,
        observed_slot: u64,
        selection_account: [u8; 32],
        selection_poststate_id: [u8; 32],
        backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        self.replay.require_action(self.root, consumed_sequence)?;
        require_live(selection_account)?;
        require_live(selection_poststate_id)?;
        require_fresh_child_account(self.root.binding, selection_account)?;
        if self.root.phase != DirectRootPhaseV1::Open
            || observed_slot < self.root.schedule.admission_closes_slot
            || observed_slot >= self.root.schedule.submission_closes_slot
        {
            return Err(DirectMarketErrorV1::WrongPhase);
        }
        let root_pre_id = self.root.semantic_id(backend)?;
        let mut root = self.root;
        root.selection_account = selection_account;
        root.phase = if root.live_reservations == MAX_DIRECT_RESERVATIONS_V1 {
            DirectRootPhaseV1::SubmissionOpen
        } else { DirectRootPhaseV1::FrozenEmpty };
        let root_post_id = root.semantic_id(backend)?;
        let replay = self.replay.advance(root_pre_id, root_post_id,
            DirectMarketActionV1::FreezeBook, observed_slot, selection_poststate_id, backend)?;
        replay.validate_against(root)?;
        Ok(Self { root, replay })
    }

    /// Record one bounded candidate admission owned by Selection.
    pub(crate) fn record_submission<B: DirectHashBackendV1>(
        self, consumed_sequence: u64, observed_slot: u64,
        selection_poststate_id: [u8; 32], backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        self.same_phase(consumed_sequence, DirectRootPhaseV1::SubmissionOpen,
            DirectMarketActionV1::SubmitCandidate, observed_slot, selection_poststate_id, backend)
    }

    /// Begin exhaustive retained-candidate traversal.
    pub(crate) fn begin_verification<B: DirectHashBackendV1>(
        self, consumed_sequence: u64, observed_slot: u64,
        selection_poststate_id: [u8; 32], backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        self.change_phase(consumed_sequence, DirectRootPhaseV1::SubmissionOpen,
            DirectRootPhaseV1::Verifying, DirectMarketActionV1::BeginVerification,
            observed_slot, selection_poststate_id, backend)
    }

    /// Record the next canonical retained-candidate verification coordinate.
    pub(crate) fn record_verification<B: DirectHashBackendV1>(
        self, consumed_sequence: u64, observed_slot: u64,
        selection_poststate_id: [u8; 32], backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        self.same_phase(consumed_sequence, DirectRootPhaseV1::Verifying,
            DirectMarketActionV1::VerifyCandidate, observed_slot, selection_poststate_id, backend)
    }

    /// Commit the exact complete selected traversal.
    pub(crate) fn select<B: DirectHashBackendV1>(
        self, consumed_sequence: u64, observed_slot: u64,
        selected_traversal_id: [u8; 32], backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        self.change_phase(consumed_sequence, DirectRootPhaseV1::Verifying,
            DirectRootPhaseV1::Selected, DirectMarketActionV1::FinalizeSelection,
            observed_slot, selected_traversal_id, backend)
    }

    /// Commit one exact economic terminal transition.
    pub(crate) fn terminalize<B: DirectHashBackendV1>(
        self,
        consumed_sequence: u64,
        observed_slot: u64,
        reason: DirectTerminalReasonV1,
        terminal_selection_account: [u8; 32],
        economic_terminal_receipt_id: [u8; 32],
        backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        self.replay.require_action(self.root, consumed_sequence)?;
        require_live(economic_terminal_receipt_id)?;
        require_live(terminal_selection_account)?;
        let action = match reason {
            DirectTerminalReasonV1::MissedFreezeLapse => DirectMarketActionV1::LapseEmpty,
            DirectTerminalReasonV1::EmptyLapse =>
                DirectMarketActionV1::LapseEmpty,
            DirectTerminalReasonV1::NoCandidate => DirectMarketActionV1::FinalizeSelection,
            DirectTerminalReasonV1::UnselectedLapse =>
                DirectMarketActionV1::LapseUnselected,
            DirectTerminalReasonV1::SelectedLapse =>
                DirectMarketActionV1::LapseSelected,
            DirectTerminalReasonV1::Settled =>
                DirectMarketActionV1::SettlePair,
        };
        let valid_phase = match reason {
            DirectTerminalReasonV1::MissedFreezeLapse => {
                self.root.phase == DirectRootPhaseV1::Open
                    && self.root.selection_account == [0; 32]
            }
            DirectTerminalReasonV1::EmptyLapse => {
                self.root.phase == DirectRootPhaseV1::FrozenEmpty
            }
            DirectTerminalReasonV1::NoCandidate => {
                self.root.phase == DirectRootPhaseV1::Verifying
            }
            DirectTerminalReasonV1::UnselectedLapse => matches!(
                self.root.phase,
                DirectRootPhaseV1::SubmissionOpen | DirectRootPhaseV1::Verifying
            ),
            DirectTerminalReasonV1::SelectedLapse | DirectTerminalReasonV1::Settled => {
                self.root.phase == DirectRootPhaseV1::Selected
            }
        };
        if !valid_phase { return Err(DirectMarketErrorV1::WrongPhase); }
        if reason != DirectTerminalReasonV1::MissedFreezeLapse
            && terminal_selection_account != self.root.selection_account
        {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        let valid_slot = match reason {
            DirectTerminalReasonV1::MissedFreezeLapse => {
                observed_slot >= self.root.schedule.submission_closes_slot
            }
            DirectTerminalReasonV1::EmptyLapse => {
                observed_slot >= self.root.schedule.admission_closes_slot
            }
            DirectTerminalReasonV1::NoCandidate => {
                observed_slot >= self.root.schedule.submission_closes_slot
                    && observed_slot < self.root.schedule.selection_deadline_slot
            }
            DirectTerminalReasonV1::UnselectedLapse => {
                observed_slot >= self.root.schedule.selection_deadline_slot
            }
            DirectTerminalReasonV1::SelectedLapse => {
                observed_slot >= self.root.schedule.settlement_deadline_slot
            }
            DirectTerminalReasonV1::Settled => {
                observed_slot >= self.root.schedule.admission_closes_slot
                    && observed_slot < self.root.schedule.settlement_deadline_slot
            }
        };
        if !valid_slot { return Err(DirectMarketErrorV1::WrongPhase); }
        let root_pre_id = self.root.semantic_id(backend)?;
        let mut root = self.root;
        root.selection_account = terminal_selection_account;
        root.phase = DirectRootPhaseV1::Terminal;
        root.terminal_reason = Some(reason);
        let root_post_id = root.semantic_id(backend)?;
        let mut replay = self.replay.advance(root_pre_id, root_post_id, action,
            observed_slot, economic_terminal_receipt_id, backend)?;
        replay.economic_terminal_receipt_id = economic_terminal_receipt_id;
        replay.validate_against(root)?;
        Ok(Self { root, replay })
    }

    fn same_phase<B: DirectHashBackendV1>(
        self, consumed_sequence: u64, expected: DirectRootPhaseV1,
        action: DirectMarketActionV1, observed_slot: u64,
        evidence_id: [u8; 32], backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        self.replay.require_action(self.root, consumed_sequence)?;
        require_live(evidence_id)?;
        if self.root.phase != expected { return Err(DirectMarketErrorV1::WrongPhase); }
        let in_window = match action {
            DirectMarketActionV1::SubmitCandidate => {
                observed_slot >= self.root.schedule.admission_closes_slot
                    && observed_slot < self.root.schedule.submission_closes_slot
            }
            DirectMarketActionV1::VerifyCandidate => {
                observed_slot >= self.root.schedule.submission_closes_slot
                    && observed_slot < self.root.schedule.selection_deadline_slot
            }
            _ => false,
        };
        if !in_window { return Err(DirectMarketErrorV1::WrongPhase); }
        let root_id = self.root.semantic_id(backend)?;
        let replay = self.replay.advance(
            root_id, root_id, action, observed_slot, evidence_id, backend,
        )?;
        replay.validate_against(self.root)?;
        Ok(Self { root: self.root, replay })
    }

    fn change_phase<B: DirectHashBackendV1>(
        self, consumed_sequence: u64, expected: DirectRootPhaseV1,
        successor: DirectRootPhaseV1, action: DirectMarketActionV1,
        observed_slot: u64, evidence_id: [u8; 32], backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        self.replay.require_action(self.root, consumed_sequence)?;
        require_live(evidence_id)?;
        if self.root.phase != expected { return Err(DirectMarketErrorV1::WrongPhase); }
        if observed_slot < self.root.schedule.submission_closes_slot
            || observed_slot >= self.root.schedule.selection_deadline_slot
        {
            return Err(DirectMarketErrorV1::WrongPhase);
        }
        let root_pre_id = self.root.semantic_id(backend)?;
        let mut root = self.root;
        root.phase = successor;
        let root_post_id = root.semantic_id(backend)?;
        let replay = self.replay.advance(
            root_pre_id, root_post_id, action, observed_slot, evidence_id, backend,
        )?;
        replay.validate_against(root)?;
        Ok(Self { root, replay })
    }
}

/// Default-deny SBF foundation authentication boundary.
pub trait AuthenticatedDirectFoundationV1 {
    /// Authenticate Product root, policy spans, Clock, absent root/replay PDAs,
    /// funding, and Realm collateral.
    fn authenticate_foundation(
        &self, _product_root: &MarketLifecycleRootV1,
        _founder_link: &SeriesMarketLinkV1,
        _compiler_bundle: &CompiledProductSeriesBundleV5,
        _fee_policy: fee_v1::DirectFeePolicyV1,
        _candidate_liveness: liveness_v1::AuthenticatedDirectCandidateLivenessV1,
        _binding: DirectMarketBindingV1,
        _schedule: DirectScheduleV1, _foundation_slot: u64,
        _root_rent: DirectRentOwnerV1,
        _action_replay_rent: DirectRentOwnerV1, _family_admission_sequence: u32,
    ) -> Result<(), DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Explicit refusing foundation authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectFoundationAuthorityV1;

impl AuthenticatedDirectFoundationV1 for NoDirectFoundationAuthorityV1 {}

/// Private Product admission capability minted only by Direct foundation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectProductAdmissionAuthorityV1 {
    aggregator_prestate_id: ContentId,
    family_root_id: ContentId,
    family_admission_sequence: u32,
    admission_receipt_id: ContentId,
}

impl AuthenticatedMarketFamilyAuthorityV1 for DirectProductAdmissionAuthorityV1 {
    fn authenticate_admission(
        &self, current: &MarketFamilyAggregatorV1, family: MarketFamilyV1,
        family_root_id: ContentId, family_admission_sequence: u32,
        admission_receipt_id: ContentId,
    ) -> Result<(), ProductError> {
        if family == MarketFamilyV1::Direct
            && current.semantic_id().map_err(|_| ProductError::UnauthenticatedAuthority)?
                .content_id() == self.aggregator_prestate_id
            && family_root_id == self.family_root_id
            && family_admission_sequence == self.family_admission_sequence
            && admission_receipt_id == self.admission_receipt_id
        { Ok(()) } else { Err(ProductError::UnauthenticatedAuthority) }
    }
}

/// Atomic pure foundation plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFoundationPlanV1 {
    /// Unique open Direct root and replay poststate.
    pub state: DirectRootReplayPostV1,
    /// Private capability for Product's Direct family slot.
    pub product_authority: DirectProductAdmissionAuthorityV1,
    /// Direct-owned admission receipt consumed by Product.
    pub admission_receipt_id: ContentId,
    /// Product-owned allocation receipt consumed by this exact occurrence.
    pub candidate_liveness_allocation_receipt_id: [u8; 32],
}

/// Prepare the unique Product-attached Direct root and permanent replay.
pub fn prepare_direct_foundation_v1<
    A: AuthenticatedDirectFoundationV1 + ?Sized,
    B: DirectHashBackendV1,
>(
    authority: &A, product_root: &MarketLifecycleRootV1,
    founder_link: &SeriesMarketLinkV1,
    compiler_bundle: &CompiledProductSeriesBundleV5,
    fee_policy: fee_v1::DirectFeePolicyV1,
    candidate_liveness: Option<liveness_v1::AuthenticatedDirectCandidateLivenessV1>,
    binding: DirectMarketBindingV1,
    schedule: DirectScheduleV1, foundation_slot: u64,
    root_rent: DirectRentOwnerV1,
    action_replay_rent: DirectRentOwnerV1, family_admission_sequence: u32, backend: &B,
) -> Result<DirectFoundationPlanV1, DirectMarketErrorV1> {
    let candidate_liveness = candidate_liveness
        .ok_or(DirectMarketErrorV1::UnauthenticatedAuthority)?;
    binding.validate()?;
    schedule.validate()?;
    if schedule != DirectScheduleV1::canonical_from_foundation_slot(foundation_slot)? {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    root_rent.validate()?;
    action_replay_rent.validate()?;
    if root_rent.payer == binding.neutral_lamport_sink
        || action_replay_rent.payer == binding.neutral_lamport_sink
    {
        return Err(DirectMarketErrorV1::IdentityAlias);
    }
    if product_root.phase() != MarketLifecyclePhaseV1::Active
        || founder_link.phase() != SeriesMarketLinkPhaseV1::Active
    {
        return Err(DirectMarketErrorV1::WrongPhase);
    }
    let founder_link_semantic_id = validate_product_join_v1(
        product_root,
        founder_link,
        compiler_bundle,
        binding,
        false,
    )?;
    let product_binding = product_root.binding();
    let families = product_root.product_families();
    let aggregator_prestate_id = families.semantic_id()
        .map_err(|_| DirectMarketErrorV1::Product)?.content_id();
    let direct = families.family(MarketFamilyV1::Direct);
    if product_binding.market_instance_id.bytes() != binding.market_instance_id
        || product_binding.generation != binding.generation
        || product_binding.outcome_count != binding.outcome_count
        || product_binding.realm_id.bytes() != binding.realm_id
        || product_binding.collateral_profile_id.bytes() != binding.collateral_profile_id
        || product_binding.collateral_policy_id.bytes() != binding.collateral_policy_id
        || product_binding.collateral_release_id.bytes() != binding.collateral_release_id
        || product_binding.resolution_account_id.bytes() != binding.resolution_account
        || product_binding.price_measure_policy_id.bytes() != binding.price_policy_id
        || product_binding
            .id()
            .map_err(|_| DirectMarketErrorV1::Product)?
            .bytes()
            != binding.product_market_binding_id
        || families.binding().family_root_id(MarketFamilyV1::Direct).bytes()
            != binding.direct_root_account
        || binding.product_family_prestate_id != aggregator_prestate_id.bytes()
        || binding.family_admission_sequence != family_admission_sequence
        || families.family(MarketFamilyV1::General).counts().live == 0
        || !families.admits_new_child(MarketFamilyV1::Direct)
        || direct.counts().admitted != family_admission_sequence
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    fee_policy.validate()?;
    if binding.fee_policy() != fee_policy
        || binding.candidate_liveness != candidate_liveness.binding()
        || binding.candidate_liveness.work_schedule_id
            != binding.candidate_liveness.work_schedule.semantic_id(
                binding.market_instance_id,
                binding.generation,
                binding.direct_root_account,
                binding.family_admission_sequence,
                binding.candidate_lifecycle_policy_id,
                binding.candidate_liveness_policy_id,
                backend,
            )?
        || binding.direct_fee_shape_id != fee_policy.semantic_id(backend)?
        || binding.direct_schedule_policy_id != direct_schedule_policy_id_v1(binding, backend)?
        || binding.direct_epoch_semantics_id
            != direct_epoch_semantics_id_v1(binding, schedule, backend)?
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    authority.authenticate_foundation(
        product_root,
        founder_link,
        compiler_bundle,
        fee_policy,
        candidate_liveness,
        binding,
        schedule,
        foundation_slot,
        root_rent,
        action_replay_rent,
        family_admission_sequence,
    )?;
    let receipt_bytes = backend.sha256_parts(&[
        FOUNDATION_RECEIPT_DOMAIN_V1, &aggregator_prestate_id.bytes(),
        &binding.market_instance_id, &binding.generation.to_le_bytes(), &[binding.outcome_count],
        &binding.realm_id, &binding.collateral_profile_id, &binding.collateral_policy_id,
        &binding.collateral_release_id, &binding.resolution_account,
        &binding.direct_epoch_semantics_id, &binding.revenue_policy_id,
        &binding.batch_policy_id, &binding.direct_fee_shape_id,
        &binding.fee_treasury_owner,
        &binding.fee_dispersion_bps.to_le_bytes(),
        &binding.fee_floor_range_bps.to_le_bytes(),
        &binding.fee_maker_rebate_num.to_le_bytes(),
        &binding.fee_treasury_num.to_le_bytes(),
        &binding.fee_split_den.to_le_bytes(),
        &binding.candidate_lifecycle_policy_id, &binding.candidate_liveness_policy_id,
        &binding.candidate_liveness.policy_account,
        &binding.candidate_liveness.policy_data_id,
        &binding.candidate_liveness.global_lifecycle_id,
        &binding.candidate_liveness.global_bundle_binding_id,
        &binding.candidate_liveness.global_capitalization_receipt_id,
        &binding.candidate_liveness.global_bundle_commitment_id,
        &binding.candidate_liveness.candidate_account,
        &binding.candidate_liveness.candidate_data_id,
        &binding.candidate_liveness.candidate_semantic_owner,
        &binding.candidate_liveness.candidate_quote_schedule_id,
        &binding.candidate_liveness.candidate_receipt_program_id,
        &binding.candidate_liveness.candidate_generation.to_le_bytes(),
        &binding.candidate_liveness.first_call_ordinal.to_le_bytes(),
        &binding.candidate_liveness.reserved_calls.to_le_bytes(),
        &binding.candidate_liveness.reserved_work_lamports.to_le_bytes(),
        &binding.candidate_liveness.allocation_receipt_id,
        &binding.candidate_liveness.work_schedule_id,
        &binding.direct_schedule_policy_id,
        &binding.product_root_account, &binding.product_market_binding_id,
        &binding.product_family_prestate_id,
        &binding.general_product_preauthorization_id,
        &binding.family_admission_sequence.to_le_bytes(),
        &binding.founder_series_link_account,
        &binding.founder_series_link_binding_id, &founder_link_semantic_id,
        &binding.compiler_bundle_v5_id, &binding.founder_series_plan_id,
        &binding.founder_series_ordinal.to_le_bytes(), &binding.direct_root_account,
        &binding.action_replay_account, &binding.general_market_binding,
        &binding.general_market_runtime,
        &binding.neutral_lamport_sink, &binding.relation_policy_id,
        &binding.price_policy_id, &binding.price_scale.to_le_bytes(),
        &schedule.admission_opens_slot.to_le_bytes(),
        &schedule.admission_closes_slot.to_le_bytes(),
        &schedule.submission_closes_slot.to_le_bytes(),
        &schedule.selection_deadline_slot.to_le_bytes(),
        &schedule.settlement_deadline_slot.to_le_bytes(),
        &family_admission_sequence.to_le_bytes(), &root_rent.payer,
        &root_rent.principal_lamports.to_le_bytes(),
        &root_rent.donation_floor_lamports.to_le_bytes(), &action_replay_rent.payer,
        &action_replay_rent.principal_lamports.to_le_bytes(),
        &action_replay_rent.donation_floor_lamports.to_le_bytes(),
    ]);
    require_live(receipt_bytes)?;
    let initial_transcript = backend.sha256_parts(&[
        ACTION_TRANSCRIPT_DOMAIN_V1, &binding.market_instance_id, &binding.direct_root_account,
        &0u64.to_le_bytes(), &[DirectMarketActionV1::InitializeMarket.byte()],
        &[0; 32], &[0; 32], &0u64.to_le_bytes(), &receipt_bytes, &[0; 32],
    ]);
    require_live(initial_transcript)?;
    let root = DirectMarketRootV1 {
        binding, schedule, root_rent, phase: DirectRootPhaseV1::Open,
        terminal_reason: None, admitted_reservations: 0, live_reservations: 0,
        retired_reservations: 0, reservation_accounts: [[0; 32]; 2],
        reservation_semantic_ids: [[0; 32]; 2], selection_account: [0; 32],
    };
    let replay = DirectActionReplayV1 {
        market_instance_id: binding.market_instance_id, generation: binding.generation,
        direct_epoch_semantics_id: binding.direct_epoch_semantics_id,
        direct_root_account: binding.direct_root_account,
        replay_account: binding.action_replay_account, rent: action_replay_rent,
        phase: DirectReplayPhaseV1::Active, next_action_sequence: 1,
        action_transcript_id: initial_transcript, foundation_receipt_id: receipt_bytes,
        economic_terminal_receipt_id: [0; 32], family_terminal_receipt_id: [0; 32],
        candidate_liveness_completed_calls: 0,
        candidate_liveness_last_receipt_id: [0; 32],
        candidate_liveness_batch_receipt_id: [0; 32],
        candidate_liveness_pending: false,
    };
    replay.validate_against(root)?;
    let admission_receipt_id = ContentId::from_bytes(receipt_bytes);
    Ok(DirectFoundationPlanV1 {
        state: DirectRootReplayPostV1 { root, replay },
        product_authority: DirectProductAdmissionAuthorityV1 {
            aggregator_prestate_id,
            family_root_id: ContentId::from_bytes(binding.direct_root_account),
            family_admission_sequence, admission_receipt_id,
        },
        admission_receipt_id,
        candidate_liveness_allocation_receipt_id:
            binding.candidate_liveness.allocation_receipt_id,
    })
}

/// Default-deny adapter boundary for final archive and Product authentication.
pub trait AuthenticatedDirectTerminalV1 {
    /// Authenticate the complete still-live deletion set and Product prestate.
    fn authenticate_terminal(
        &self, _product_root: &MarketLifecycleRootV1,
        _founder_link: &SeriesMarketLinkV1,
        _compiler_bundle: &CompiledProductSeriesBundleV5,
        _root: &DirectMarketRootV1,
        _root_semantic_id: [u8; 32], _replay: &DirectActionReplayV1,
        _replay_semantic_id: [u8; 32],
        _selection: &crate::selection_v1::DirectSelectionV1,
        _reservations: &[Option<crate::reservation_v1::DirectReservationV1>; 2],
        _final_resolution: DirectFinalResolutionV1,
        _retirement: &DirectRetirementTransferV1,
        _retirement_transfer_id: [u8; 32], _consumed_sequence: u64,
        _observed_slot: u64, _family_terminal_sequence: u32,
    ) -> Result<(), DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Explicit refusing terminal authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectTerminalAuthorityV1;

impl AuthenticatedDirectTerminalV1 for NoDirectTerminalAuthorityV1 {}

/// Private Product terminal capability minted only by complete retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectProductTerminalAuthorityV1 {
    aggregator_prestate_id: ContentId,
    family_root_id: ContentId,
    family_terminal_sequence: u32,
    terminal_receipt_id: ContentId,
}

impl AuthenticatedMarketFamilyAuthorityV1 for DirectProductTerminalAuthorityV1 {
    fn authenticate_terminal(
        &self, current: &MarketFamilyAggregatorV1, family: MarketFamilyV1,
        family_root_id: ContentId, family_terminal_sequence: u32,
        terminal_receipt_id: ContentId,
    ) -> Result<(), ProductError> {
        if family == MarketFamilyV1::Direct
            && current.semantic_id().map_err(|_| ProductError::UnauthenticatedAuthority)?
                .content_id() == self.aggregator_prestate_id
            && family_root_id == self.family_root_id
            && family_terminal_sequence == self.family_terminal_sequence
            && terminal_receipt_id == self.terminal_receipt_id
        { Ok(()) } else { Err(ProductError::UnauthenticatedAuthority) }
    }
}

/// Sole permanent terminal receipt and private Product capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectFamilyTerminalPlanV1 {
    /// Direct root identity immediately before deletion.
    pub root_semantic_id: [u8; 32],
    /// Permanent replay identity immediately before action 13.
    pub replay_pre_semantic_id: [u8; 32],
    /// Canonical complete source/refund/surplus vector.
    pub retirement: DirectRetirementTransferV1,
    /// Identity of the complete transfer vector.
    pub retirement_transfer_id: [u8; 32],
    /// Exact finalized Resolution V5 joined only at family retirement.
    pub final_resolution: DirectFinalResolutionV1,
    /// Terminal replay/receipt successor used to derive the Product receipt
    /// before the owning replay account is closed in the same transition.
    pub replay_post: DirectActionReplayV1,
    /// Direct-owned terminal receipt consumed by Product.
    pub terminal_receipt_id: ContentId,
    /// Private Product Direct-family authority.
    pub product_authority: DirectProductTerminalAuthorityV1,
}

/// Prepare atomic action 13 after economic terminality.
///
/// The vector must contain root, action replay, Selection, and every still-live Reservation.
/// Reservations retired by action 3 remain committed by replay and cannot
/// reappear. The action replay closes only after Product commits its terminal receipt.
pub fn prepare_direct_family_terminal_v1<
    A: AuthenticatedDirectTerminalV1 + ?Sized,
    B: DirectHashBackendV1,
>(
    authority: &A, product_root: &MarketLifecycleRootV1,
    founder_link: &SeriesMarketLinkV1,
    compiler_bundle: &CompiledProductSeriesBundleV5,
    state: &DirectRootReplayPostV1,
    selection: &crate::selection_v1::DirectSelectionV1,
    reservations: &[Option<crate::reservation_v1::DirectReservationV1>; 2],
    final_resolution: DirectFinalResolutionV1,
    retirement: &DirectRetirementTransferV1, consumed_sequence: u64,
    observed_slot: u64, family_terminal_sequence: u32, backend: &B,
) -> Result<DirectFamilyTerminalPlanV1, DirectMarketErrorV1> {
    state.replay.require_action(state.root, consumed_sequence)?;
    selection.validate_against(state.root)?;
    retirement.validate()?;
    if state.root.phase != DirectRootPhaseV1::Terminal
        || !matches!(product_root.phase(), MarketLifecyclePhaseV1::Active | MarketLifecyclePhaseV1::Retiring)
    {
        return Err(DirectMarketErrorV1::WrongPhase);
    }
    let binding = state.root.binding;
    final_resolution.validate(binding, product_root)?;
    let founder_link_semantic_id = validate_product_join_v1(
        product_root,
        founder_link,
        compiler_bundle,
        binding,
        true,
    )?;
    let product_binding = product_root.binding();
    let families = product_root.product_families();
    let direct = families.family(MarketFamilyV1::Direct);
    if product_binding.market_instance_id.bytes() != binding.market_instance_id
        || product_binding.generation != binding.generation
        || families.binding().family_root_id(MarketFamilyV1::Direct).bytes()
            != binding.direct_root_account
        || direct.counts().live == 0
        || direct.counts().terminal != family_terminal_sequence
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    let expected_sources = usize::from(state.root.live_reservations).checked_add(3)
        .ok_or(DirectMarketErrorV1::Arithmetic)?;
    let ordered_reservations = canonical_terminal_reservation_archives(
        &state.root,
        selection,
        reservations,
        backend,
    )?;
    if retirement.neutral_lamport_sink != binding.neutral_lamport_sink
        || usize::from(retirement.source_count) != expected_sources
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    require_terminal_retirement_source_v1(
        retirement,
        binding.direct_root_account,
        state.root.root_rent,
    )?;
    require_terminal_retirement_source_v1(
        retirement,
        binding.action_replay_account,
        state.replay.rent,
    )?;
    require_terminal_retirement_source_v1(
        retirement,
        state.root.selection_account,
        selection.rent(),
    )?;
    let mut reservation_ids = [[0u8; 32]; 2];
    let mut index = 0usize;
    while index < usize::from(state.root.live_reservations) {
        let reservation = ordered_reservations[index]
            .ok_or(DirectMarketErrorV1::InvalidCount)?;
        require_terminal_retirement_source_v1(
            retirement,
            reservation.account(),
            reservation.rent(),
        )?;
        reservation_ids[index] = reservation.semantic_id(backend)?;
        index += 1;
    }
    let root_semantic_id = state.root.semantic_id(backend)?;
    let replay_pre_semantic_id = state.replay.semantic_id(state.root, backend)?;
    let selection_semantic_id = selection.semantic_id(state.root, backend)?;
    let retirement_transfer_id = retirement.semantic_id(backend)?;
    authority.authenticate_terminal(
        product_root,
        founder_link,
        compiler_bundle,
        &state.root,
        root_semantic_id,
        &state.replay,
        replay_pre_semantic_id,
        selection,
        &ordered_reservations,
        final_resolution,
        retirement,
        retirement_transfer_id,
        consumed_sequence,
        observed_slot,
        family_terminal_sequence,
    )?;
    let replay_with_action = state.replay.advance(root_semantic_id, root_semantic_id,
        DirectMarketActionV1::RetireTerminal, observed_slot, retirement_transfer_id, backend)?;
    let aggregator_prestate_id = families.semantic_id()
        .map_err(|_| DirectMarketErrorV1::Product)?.content_id();
    let terminal_bytes = backend.sha256_parts(&[
        TERMINAL_RECEIPT_DOMAIN_V1, &aggregator_prestate_id.bytes(),
        &binding.market_instance_id, &binding.generation.to_le_bytes(),
        &binding.direct_root_account, &binding.action_replay_account, &root_semantic_id,
        &binding.founder_series_link_account, &binding.founder_series_link_binding_id,
        &founder_link_semantic_id, &binding.compiler_bundle_v5_id,
        &final_resolution.account, &final_resolution.semantic_id, &final_resolution.data_id,
        &selection_semantic_id, &reservation_ids[0], &reservation_ids[1],
        &replay_pre_semantic_id, &replay_with_action.action_transcript_id,
        &state.replay.economic_terminal_receipt_id, &retirement_transfer_id,
        &consumed_sequence.to_le_bytes(), &observed_slot.to_le_bytes(),
        &family_terminal_sequence.to_le_bytes(),
    ]);
    require_live(terminal_bytes)?;
    let mut replay_post = replay_with_action;
    replay_post.phase = DirectReplayPhaseV1::Terminal;
    replay_post.family_terminal_receipt_id = terminal_bytes;
    replay_post.validate_against(state.root)?;
    let terminal_receipt_id = ContentId::from_bytes(terminal_bytes);
    Ok(DirectFamilyTerminalPlanV1 {
        root_semantic_id, replay_pre_semantic_id, retirement: *retirement,
        retirement_transfer_id, final_resolution,
        replay_post, terminal_receipt_id,
        product_authority: DirectProductTerminalAuthorityV1 {
            aggregator_prestate_id,
            family_root_id: ContentId::from_bytes(binding.direct_root_account),
            family_terminal_sequence, terminal_receipt_id,
        },
    })
}

fn require_terminal_retirement_source_v1(
    retirement: &DirectRetirementTransferV1,
    required_account: [u8; 32],
    required_rent: DirectRentOwnerV1,
) -> Result<(), DirectMarketErrorV1> {
    let mut index = 0usize;
    while index < usize::from(retirement.source_count) {
        let source = retirement.sources[index].ok_or(DirectMarketErrorV1::InvalidCount)?;
        if source.account == required_account {
            return if source.rent == required_rent {
                Ok(())
            } else {
                Err(DirectMarketErrorV1::MismatchedBinding)
            };
        }
        index += 1;
    }
    Err(DirectMarketErrorV1::MismatchedBinding)
}

fn canonical_terminal_reservation_archives<B: DirectHashBackendV1>(
    root: &DirectMarketRootV1,
    selection: &crate::selection_v1::DirectSelectionV1,
    supplied: &[Option<crate::reservation_v1::DirectReservationV1>; 2],
    backend: &B,
) -> Result<[Option<crate::reservation_v1::DirectReservationV1>; 2], DirectMarketErrorV1> {
    if selection.phase() != crate::selection_v1::DirectSelectionPhaseV1::Terminal
        || selection.reservation_count() != root.live_reservations()
    {
        return Err(DirectMarketErrorV1::WrongPhase);
    }
    let supplied_count = match *supplied {
        [None, None] => 0,
        [Some(_), None] | [None, Some(_)] => 1,
        [Some(_), Some(_)] => 2,
    };
    if u8::try_from(supplied_count).map_err(|_| DirectMarketErrorV1::Arithmetic)?
        != root.live_reservations()
    {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let expected_phase = match root.terminal_reason() {
        Some(DirectTerminalReasonV1::Settled) => {
            crate::reservation_v1::DirectReservationPhaseV1::Settled
        }
        Some(DirectTerminalReasonV1::MissedFreezeLapse)
        | Some(DirectTerminalReasonV1::EmptyLapse)
        | Some(DirectTerminalReasonV1::NoCandidate)
        | Some(DirectTerminalReasonV1::UnselectedLapse)
        | Some(DirectTerminalReasonV1::SelectedLapse) => {
            crate::reservation_v1::DirectReservationPhaseV1::Lapsed
        }
        None => return Err(DirectMarketErrorV1::WrongPhase),
    };
    let transition_id = selection.terminal_receipt_id();
    require_live(transition_id)?;
    let mut ordered = [None; 2];
    let mut supplied_index = 0usize;
    while supplied_index < 2 {
        if let Some(reservation) = supplied[supplied_index] {
            reservation.validate_against_root(*root)?;
            if reservation.phase() != expected_phase
                || reservation.terminal_receipt_id() != transition_id
            {
                return Err(DirectMarketErrorV1::MismatchedBinding);
            }
            let mut expected_index = 0usize;
            let mut found = None;
            while expected_index < usize::from(root.live_reservations()) {
                let bounded =
                    u8::try_from(expected_index).map_err(|_| DirectMarketErrorV1::Arithmetic)?;
                if reservation.account() == selection.reservation_account(bounded)? {
                    found = Some(expected_index);
                    break;
                }
                expected_index += 1;
            }
            let found = found.ok_or(DirectMarketErrorV1::MismatchedBinding)?;
            if ordered[found].is_some() {
                return Err(DirectMarketErrorV1::IdentityAlias);
            }
            require_live(reservation.semantic_id(backend)?)?;
            ordered[found] = Some(reservation);
        }
        supplied_index += 1;
    }
    Ok(ordered)
}

fn require_live(value: [u8; 32]) -> Result<(), DirectMarketErrorV1> {
    if value == [0; 32] { Err(DirectMarketErrorV1::ZeroIdentity) } else { Ok(()) }
}

fn require_fresh_child_account(
    binding: DirectMarketBindingV1,
    child: [u8; 32],
) -> Result<(), DirectMarketErrorV1> {
    require_live(child)?;
    for account in [
        binding.resolution_account,
        binding.product_root_account,
        binding.founder_series_link_account,
        binding.direct_root_account,
        binding.action_replay_account,
        binding.general_market_runtime,
        binding.neutral_lamport_sink,
    ] {
        if child == account {
            return Err(DirectMarketErrorV1::IdentityAlias);
        }
    }
    Ok(())
}

fn require_distinct(values: &[[u8; 32]]) -> Result<(), DirectMarketErrorV1> {
    let mut left = 0usize;
    while left < values.len() {
        require_live(values[left])?;
        let mut right = left + 1;
        while right < values.len() {
            if values[left] == values[right] { return Err(DirectMarketErrorV1::IdentityAlias); }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn insert_refund(
    payers: &mut [[u8; 32]; MAX_DIRECT_REFUND_RECIPIENTS_V1],
    amounts: &mut [u64; MAX_DIRECT_REFUND_RECIPIENTS_V1], count: &mut usize,
    payer: [u8; 32], amount: u64,
) -> Result<(), DirectMarketErrorV1> {
    let mut at = 0usize;
    while at < *count && payers[at] < payer { at += 1; }
    if at < *count && payers[at] == payer {
        amounts[at] = amounts[at].checked_add(amount).ok_or(DirectMarketErrorV1::Arithmetic)?;
        return Ok(());
    }
    if *count >= MAX_DIRECT_REFUND_RECIPIENTS_V1 {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let mut cursor = *count;
    while cursor > at {
        payers[cursor] = payers[cursor - 1];
        amounts[cursor] = amounts[cursor - 1];
        cursor -= 1;
    }
    payers[at] = payer;
    amounts[at] = amount;
    *count = (*count).checked_add(1).ok_or(DirectMarketErrorV1::Arithmetic)?;
    Ok(())
}

#[cfg(test)]
mod tests;

pub mod reservation_v1;
