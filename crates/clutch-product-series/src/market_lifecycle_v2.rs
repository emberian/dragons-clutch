//! Current shared Market lifecycle and per-Series admission links.
//!
//! This successor binds the exact 47-slot ScheduleV3/GraphV3 and QuoteV5.
//! Historical root/link V1 bytes remain decode-only and are never interpreted
//! as these states.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, AuthenticatedMarketFamilyAuthorityV1, CompiledProductSeriesBundleV6Id,
    ContentId, Error, FixedCodec,
    MarketFamilyAggregatorPhaseV1, MarketFamilyAggregatorV1, MarketFamilyV1,
    MarketFoundationAccountGraphV3, MarketFoundationAccountGraphV3Id,
    MarketFoundationScheduleV3, MarketFoundationScheduleV3Id, MarketFoundationSlotV3,
    MarketInstanceV2Id, Result, SeriesAttachmentPlanV5Id, SeriesFundingQuoteV5Id,
    SeriesFundingTermsV2Id,
    SeriesMarketDispositionV1, SeriesMarketLinkV2Id, SeriesPlanV5Id, SourceOccurrenceV1Id,
    MARKET_FAMILY_AGGREGATOR_BYTES_V1, MARKET_FOUNDATION_CORE_SLOT_COUNT_V3,
    MARKET_FOUNDATION_MAX_OUTCOMES_V3, MARKET_FOUNDATION_SLOT_COUNT_V3,
};

const ROOT_MAGIC_V2: [u8; 8] = *b"DCMKRTV2";
const ROOT_VERSION_V2: u16 = 2;
const LINK_MAGIC_V2: [u8; 8] = *b"DCSMLKV2";
const LINK_VERSION_V2: u16 = 2;
const MARKET_BINDING_ID_COUNT_V2: usize = 34;
const LINK_BINDING_ID_COUNT_V2: usize = 25;

/// Exact number of mandatory shared-core terminal owners outside the product-family aggregator.
pub const MARKET_SHARED_CORE_COUNT_V2: usize = 5;
/// Exact number of Series-link-scoped attachment obligations.
pub const SERIES_LINK_OBLIGATION_COUNT_V2: usize = 4;
/// Exact shared 0xaa/version2 semantic body width.
pub const MARKET_LIFECYCLE_ROOT_BYTES_V2: usize = 2_480;
/// Exact per-Series 0xad/version2 semantic body width.
pub const SERIES_MARKET_LINK_BYTES_V2: usize = 1_232;

/// Current shared Market binding identity domain.
pub const MARKET_LIFECYCLE_BINDING_DOMAIN_V2: &[u8] = b"dragons-clutch/market-lifecycle-binding/v2";
/// Current mutable shared Market root domain.
pub const MARKET_LIFECYCLE_ROOT_DOMAIN_V2: &[u8] = b"dragons-clutch/market-lifecycle-root/v2";
/// Current per-Series Market link domain.
pub const SERIES_MARKET_LINK_DOMAIN_V2: &[u8] = b"dragons-clutch/series-market-link/v2";
/// Current once-only Product/Failure Resolution activation domain.
pub const MARKET_RESOLUTION_ACTIVATION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/market-resolution-activation/v2";
/// Current whole-Market terminal projection domain.
pub const MARKET_INSTANCE_TERMINAL_PROJECTION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/market-instance-terminal-projection/v2";

/// Shared Market lifecycle phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MarketLifecyclePhaseV2 {
    /// Prepaid construction is permissionlessly continuable; liabilities are disabled.
    Founding = 1,
    /// Every foundation poststate is exact and liabilities may open.
    Active = 2,
    /// No Series links remain live and family summaries are being collected.
    Retiring = 3,
    /// Every market-scoped family summary has been consumed.
    Terminal = 4,
    /// Timed-out inert construction is closing in reverse dependency order.
    Aborting = 5,
}

impl MarketLifecyclePhaseV2 {
    const fn byte(self) -> u8 {
        match self {
            Self::Founding => 1,
            Self::Active => 2,
            Self::Retiring => 3,
            Self::Terminal => 4,
            Self::Aborting => 5,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Founding),
            2 => Ok(Self::Active),
            3 => Ok(Self::Retiring),
            4 => Ok(Self::Terminal),
            5 => Ok(Self::Aborting),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Complete immutable, Market-scoped semantic and deployment binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketLifecycleBindingV2 {
    /// Full-width economic Market identity.
    pub market_instance_id: MarketInstanceV2Id,
    /// Shared Failure/Resolution generation.
    pub generation: u64,
    /// Exact active outcome count.
    pub outcome_count: u8,
    /// Immutable upper bound on admitted Series links.
    pub maximum_series_links: u32,
    /// Product template.
    pub product_template_id: ContentId,
    /// Native claim basis.
    pub native_claim_basis_id: ContentId,
    /// Evidence-only Recovery policy.
    pub recovery_policy_id: ContentId,
    /// Exact quantized price policy.
    pub price_measure_policy_id: ContentId,
    /// Market genesis profile.
    pub market_genesis_profile_id: ContentId,
    /// Current central Registry release.
    pub registry_release_id: ContentId,
    /// Exact central capability profile.
    pub capability_profile_id: ContentId,
    /// Immutable Realm.
    pub realm_id: ContentId,
    /// Realm collateral profile.
    pub collateral_profile_id: ContentId,
    /// Collateral policy artifact.
    pub collateral_policy_id: ContentId,
    /// Reviewed collateral adapter release.
    pub collateral_release_id: ContentId,
    /// Authenticated Source release.
    pub source_release_id: ContentId,
    /// Market-scoped Source route.
    pub source_route_id: ContentId,
    /// Sole Clock policy.
    pub clock_policy_id: ContentId,
    /// SourcePlane semantic contract.
    pub source_plane_contract_id: ContentId,
    /// Source specification.
    pub source_spec_id: ContentId,
    /// Exact primary Window.
    pub primary_window_id: ContentId,
    /// Exact Statistic key.
    pub statistic_key_id: ContentId,
    /// Market-scoped Failure policy binding.
    pub market_failure_policy_binding_id: ContentId,
    /// Shared Recovery state.
    pub recovery_state_id: ContentId,
    /// Central-derived interval-consensus profile.
    pub interval_consensus_profile_id: ContentId,
    /// Existing authenticated market-scoped runtime-liveness policy.
    pub failure_liveness_policy_id: ContentId,
    /// Exact Recovery-compartment schedule owned by that liveness policy.
    pub failure_liveness_quote_schedule_id: ContentId,
    /// Canonical Resolution V5 account.
    pub resolution_account_id: ContentId,
    /// Market-scoped FoundationVault PDA.
    pub foundation_vault_id: ContentId,
    /// Canonical pairwise-distinct physical foundation account graph.
    pub foundation_account_graph_id: MarketFoundationAccountGraphV3Id,
    /// Exact itemized foundation schedule.
    pub foundation_schedule_id: MarketFoundationScheduleV3Id,
    /// Product-owned DirectGlobalLiveness binding which transitively commits
    /// the lifecycle account, both Direct policies, and capitalization bundle.
    pub direct_global_liveness_binding_id: ContentId,
    /// Accepted collateral liability founding plan.
    pub market_liability_founding_id: ContentId,
    /// Exact claim-mint founding plan.
    pub claim_mint_founding_plan_id: ContentId,
    /// Canonical accepted claim-issuance binding shared by all claim consumers.
    pub claim_issuance_binding_id: ContentId,
    /// General-private MarketBinding/Runtime founding capability.
    pub general_founding_capability_id: ContentId,
    /// Product-owned abort policy.
    pub founding_abort_policy_id: ContentId,
    /// Last bucket at which permissionless founding remains eligible.
    pub founding_deadline_bucket: u64,
    /// Largest inclusive interval width.
    pub maximum_interval_width: u64,
    /// Largest coordinate chunk.
    pub maximum_coordinates_per_advance: u16,
}

impl MarketLifecycleBindingV2 {
    /// Validate all immutable identities and finite bounds.
    pub fn validate(self) -> Result<()> {
        self.market_instance_id.validate()?;
        self.foundation_account_graph_id.validate()?;
        self.foundation_schedule_id.validate()?;
        if self.generation == 0
            || self.outcome_count == 0
            || usize::from(self.outcome_count) > MARKET_FOUNDATION_MAX_OUTCOMES_V3
            || self.maximum_series_links == 0
            || self.founding_deadline_bucket == 0
            || self.maximum_interval_width == u64::MAX
            || self.maximum_coordinates_per_advance == 0
        {
            return Err(Error::InvalidParameter);
        }
        let ids = self.identity_ids();
        for id in ids {
            id.validate()?;
        }
        require_pairwise_distinct(&ids)
    }

    /// Domain-separated identity of the complete binding.
    pub fn id(self) -> Result<ContentId> {
        self.validate()?;
        let mut body = [0u8; 1_119];
        let mut at = 0usize;
        for id in self.identity_ids() {
            body[at..at + 32].copy_from_slice(&id.bytes());
            at += 32;
        }
        body[at..at + 8].copy_from_slice(&self.generation.to_le_bytes());
        at += 8;
        body[at] = self.outcome_count;
        at += 1;
        body[at..at + 4].copy_from_slice(&self.maximum_series_links.to_le_bytes());
        at += 4;
        body[at..at + 8].copy_from_slice(&self.founding_deadline_bucket.to_le_bytes());
        at += 8;
        body[at..at + 8].copy_from_slice(&self.maximum_interval_width.to_le_bytes());
        at += 8;
        body[at..at + 2].copy_from_slice(&self.maximum_coordinates_per_advance.to_le_bytes());
        Ok(content_id(MARKET_LIFECYCLE_BINDING_DOMAIN_V2, &body))
    }

    fn identity_ids(self) -> [ContentId; MARKET_BINDING_ID_COUNT_V2] {
        [
            self.market_instance_id.content_id(),
            self.product_template_id,
            self.native_claim_basis_id,
            self.recovery_policy_id,
            self.price_measure_policy_id,
            self.market_genesis_profile_id,
            self.registry_release_id,
            self.capability_profile_id,
            self.realm_id,
            self.collateral_profile_id,
            self.collateral_policy_id,
            self.collateral_release_id,
            self.source_release_id,
            self.source_route_id,
            self.clock_policy_id,
            self.source_plane_contract_id,
            self.source_spec_id,
            self.primary_window_id,
            self.statistic_key_id,
            self.market_failure_policy_binding_id,
            self.recovery_state_id,
            self.interval_consensus_profile_id,
            self.failure_liveness_policy_id,
            self.failure_liveness_quote_schedule_id,
            self.resolution_account_id,
            self.foundation_vault_id,
            self.foundation_account_graph_id.content_id(),
            self.foundation_schedule_id.content_id(),
            self.direct_global_liveness_binding_id,
            self.market_liability_founding_id,
            self.claim_mint_founding_plan_id,
            self.claim_issuance_binding_id,
            self.general_founding_capability_id,
            self.founding_abort_policy_id,
        ]
    }
}

/// Founder-owned shared principal and Recovery liveness decomposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFoundationCapitalV2 {
    /// Exact founding `0xad` link.
    pub founder_link_id: SeriesMarketLinkV2Id,
    /// Private MarketCore vault-debit receipt.
    pub market_core_debit_receipt_id: ContentId,
    /// Private RecoveryReserve vault-debit receipt.
    pub recovery_debit_receipt_id: ContentId,
    /// Immutable principal refund owner; no signature is required for continuation.
    pub rent_refund_owner: ContentId,
    /// System-owned sink for unsolicited lamports.
    pub neutral_lamport_sink: ContentId,
    /// Original exact MarketCore principal.
    pub principal_total_lamports: u64,
    /// Principal not yet spent from FoundationVault.
    pub principal_remaining_lamports: u64,
    /// Vault donation floor observed before the founder debit.
    pub vault_donation_floor_lamports: u64,
    /// Current donation amount, never spendable as principal.
    pub vault_current_donation_lamports: u64,
    /// Shared Recovery work principal.
    pub recovery_work_principal_lamports: u64,
    /// Shared Recovery rent principal.
    pub recovery_rent_principal_lamports: u64,
}

impl MarketFoundationCapitalV2 {
    fn validate(self, schedule: &MarketFoundationScheduleV3) -> Result<()> {
        self.founder_link_id.validate()?;
        let ids = [
            self.market_core_debit_receipt_id,
            self.recovery_debit_receipt_id,
            self.rent_refund_owner,
            self.neutral_lamport_sink,
        ];
        for id in ids {
            id.validate()?;
        }
        require_pairwise_distinct(&ids)?;
        if self.principal_total_lamports != schedule.total_principal_lamports()?
            || self.principal_remaining_lamports > self.principal_total_lamports
            || self.vault_current_donation_lamports < self.vault_donation_floor_lamports
            || self.recovery_work_principal_lamports == 0
            || self.recovery_rent_principal_lamports == 0
        {
            return Err(Error::InvalidParameter);
        }
        self.recovery_work_principal_lamports
            .checked_add(self.recovery_rent_principal_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }
}

/// Exact bounded founding progress and rolling receipt transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFoundationProgressV2 {
    /// Slots required by the outcome count.
    pub expected_bitmap: u64,
    /// Authenticated initialized/prefunded slots.
    pub initialized_bitmap: u64,
    /// Initialized slots already closed during abort.
    pub abort_closed_bitmap: u64,
    /// Monotone initialize/abort step sequence.
    pub sequence: u32,
    /// Rolling ordered receipt transcript.
    pub transcript_id: ContentId,
}

impl MarketFoundationProgressV2 {
    /// Whether every expected slot is initialized exactly once.
    pub fn complete(self) -> bool {
        self.initialized_bitmap == self.expected_bitmap
    }
}

/// Structural projection for one adapter-authenticated FoundationVault spend/postwrite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFoundationStepProjectionV3 {
    /// Exact Market binding.
    pub binding_id: ContentId,
    /// Exact next slot.
    pub slot: MarketFoundationSlotV3,
    /// Root transition sequence after this step.
    pub root_transition_sequence: u64,
    /// Exact quote-owned principal spent.
    pub principal_lamports: u64,
    /// FoundationVault principal before/after.
    pub principal_before_lamports: u64,
    /// FoundationVault principal after.
    pub principal_after_lamports: u64,
    /// Donation observation before/after; both must be equal for a founding spend.
    pub donation_before_lamports: u64,
    /// Donation observation after.
    pub donation_after_lamports: u64,
    /// Exact initialized account.
    pub account_id: ContentId,
    /// Exact accepted zero-poststate receipt.
    pub accepted_poststate_receipt_id: ContentId,
}

impl MarketFoundationStepProjectionV3 {
    /// Content identity used in the rolling founding transcript.
    pub fn id(self) -> Result<ContentId> {
        let slot = self.slot.index()?;
        let slot_byte = u8::try_from(slot).map_err(|_| Error::InvalidParameter)?;
        let ids = [
            self.binding_id,
            self.account_id,
            self.accepted_poststate_receipt_id,
        ];
        for id in ids {
            id.validate()?;
        }
        let mut body = [0u8; 145];
        body[0] = slot_byte;
        body[1..33].copy_from_slice(&self.binding_id.bytes());
        body[33..41].copy_from_slice(&self.root_transition_sequence.to_le_bytes());
        body[41..49].copy_from_slice(&self.principal_lamports.to_le_bytes());
        body[49..57].copy_from_slice(&self.principal_before_lamports.to_le_bytes());
        body[57..65].copy_from_slice(&self.principal_after_lamports.to_le_bytes());
        body[65..73].copy_from_slice(&self.donation_before_lamports.to_le_bytes());
        body[73..81].copy_from_slice(&self.donation_after_lamports.to_le_bytes());
        body[81..113].copy_from_slice(&self.account_id.bytes());
        body[113..145].copy_from_slice(&self.accepted_poststate_receipt_id.bytes());
        Ok(content_id(
            b"dragons-clutch/market-foundation-step/v3",
            &body,
        ))
    }
}

/// Mandatory shared-core owner outside the optional product-family aggregator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MarketSharedCoreV2 {
    /// Aggregate native ClaimLedger V3.
    ClaimLedger = 0,
    /// Hoard V2 collateral/custody root.
    Hoard = 1,
    /// Exhaustive Failure market root.
    Failure = 2,
    /// Exhaustive Source market root.
    Source = 3,
    /// Exhaustive Position market root.
    Position = 4,
}

impl MarketSharedCoreV2 {
    /// Stable array index.
    pub const fn index(self) -> usize {
        match self {
            Self::ClaimLedger => 0,
            Self::Hoard => 1,
            Self::Failure => 2,
            Self::Source => 3,
            Self::Position => 4,
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::ClaimLedger => 0,
            Self::Hoard => 1,
            Self::Failure => 2,
            Self::Source => 3,
            Self::Position => 4,
        }
    }
}

/// Typed structural terminal evidence for one mandatory shared-core owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketSharedCoreTerminalProjectionV2 {
    id: ContentId,
    market_binding_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    owner: MarketSharedCoreV2,
    owner_account_id: ContentId,
    owner_release_id: ContentId,
    owner_terminal_receipt_id: ContentId,
    root_transition_sequence: u64,
}

impl MarketSharedCoreTerminalProjectionV2 {
    /// Construct a deterministic projection; live SBF promotes it only from a private owner receipt.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        binding: MarketLifecycleBindingV2,
        owner: MarketSharedCoreV2,
        owner_account_id: ContentId,
        owner_release_id: ContentId,
        owner_terminal_receipt_id: ContentId,
        root_transition_sequence: u64,
    ) -> Result<Self> {
        binding.validate()?;
        let ids = [
            owner_account_id,
            owner_release_id,
            owner_terminal_receipt_id,
        ];
        for id in ids {
            id.validate()?;
        }
        require_pairwise_distinct(&ids)?;
        if root_transition_sequence == 0 {
            return Err(Error::InvalidParameter);
        }
        let market_binding_id = binding.id()?;
        let mut body = [0u8; 177];
        body[0] = owner.byte();
        body[1..33].copy_from_slice(&market_binding_id.bytes());
        body[33..65].copy_from_slice(&binding.market_instance_id.bytes());
        body[65..73].copy_from_slice(&binding.generation.to_le_bytes());
        body[73..105].copy_from_slice(&owner_account_id.bytes());
        body[105..137].copy_from_slice(&owner_release_id.bytes());
        body[137..169].copy_from_slice(&owner_terminal_receipt_id.bytes());
        body[169..177].copy_from_slice(&root_transition_sequence.to_le_bytes());
        let id = content_id(b"dragons-clutch/market-shared-core-terminal/v2", &body);
        Ok(Self {
            id,
            market_binding_id,
            market_instance_id: binding.market_instance_id,
            generation: binding.generation,
            owner,
            owner_account_id,
            owner_release_id,
            owner_terminal_receipt_id,
            root_transition_sequence,
        })
    }

    /// Projection identity.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Shared Market binding.
    pub const fn market_binding_id(self) -> ContentId {
        self.market_binding_id
    }
    /// Shared Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    /// Failure/Source generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Mandatory core owner.
    pub const fn owner(self) -> MarketSharedCoreV2 {
        self.owner
    }
    /// Exact owner account.
    pub const fn owner_account_id(self) -> ContentId {
        self.owner_account_id
    }
    /// Exact owner release.
    pub const fn owner_release_id(self) -> ContentId {
        self.owner_release_id
    }
    /// Exact terminal receipt.
    pub const fn owner_terminal_receipt_id(self) -> ContentId {
        self.owner_terminal_receipt_id
    }
    /// Expected Product root transition.
    pub const fn root_transition_sequence(self) -> u64 {
        self.root_transition_sequence
    }
}

/// Series-link-scoped attachment obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesLinkObligationV2 {
    /// Dealer facility/lease obligations selected by this Series.
    Dealer = 0,
    /// Structured descriptor/lot obligations selected by this Series.
    Structured = 1,
    /// Passive-liquidity attachment selected by this Series.
    Liquidity = 2,
    /// Wrapper descriptor/mint/vault attachment selected by this Series.
    Wrapper = 3,
}

impl SeriesLinkObligationV2 {
    /// Stable fixed-array index.
    pub const fn index(self) -> usize {
        match self {
            Self::Dealer => 0,
            Self::Structured => 1,
            Self::Liquidity => 2,
            Self::Wrapper => 3,
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::Dealer => 0,
            Self::Structured => 1,
            Self::Liquidity => 2,
            Self::Wrapper => 3,
        }
    }
}

/// Terminal or capability-authenticated absence for one link obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesLinkObligationDispositionV2 {
    /// Enabled obligation terminal.
    Terminal = 1,
    /// Immutable profile and canonical account graph proved absence.
    Absent = 2,
}

impl SeriesLinkObligationDispositionV2 {
    /// Stable exhaustive wire byte used by current private adapter receipts.
    pub const fn wire_byte(self) -> u8 {
        match self {
            Self::Terminal => 1,
            Self::Absent => 2,
        }
    }
}

/// Exhaustive immutable/current state of one Series-link obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesLinkObligationStatusV2 {
    /// The central capability profile excludes the obligation.
    CapabilityDisabled = 0,
    /// The capability exists, but the immutable attachment admits no child.
    EnabledNeverFounded = 1,
    /// The attachment admitted a child which still has work or custody.
    Live = 2,
    /// The exact live child terminated or authenticated absence was consumed.
    Terminal = 3,
}

impl SeriesLinkObligationStatusV2 {
    /// Stable exhaustive wire byte used by current private adapter receipts.
    pub const fn wire_byte(self) -> u8 {
        match self {
            Self::CapabilityDisabled => 0,
            Self::EnabledNeverFounded => 1,
            Self::Live => 2,
            Self::Terminal => 3,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::CapabilityDisabled),
            1 => Ok(Self::EnabledNeverFounded),
            2 => Ok(Self::Live),
            3 => Ok(Self::Terminal),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Immutable capability/profile and attachment-derived obligation configuration.
///
/// The pure value is structural only. A live adapter must construct it from the
/// authenticated central capability profile and exact attachment artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesLinkObligationConfigurationV2 {
    /// Authenticated central capability profile.
    pub capability_profile_id: ContentId,
    /// Exact Series attachment plan.
    pub attachment_plan_id: ContentId,
    /// Exhaustive initial states in [`SeriesLinkObligationV2`] order.
    pub initial_statuses: [SeriesLinkObligationStatusV2; SERIES_LINK_OBLIGATION_COUNT_V2],
}

impl SeriesLinkObligationConfigurationV2 {
    /// Validate that every obligation begins in a nonterminal, explicit state.
    pub fn validate(self) -> Result<()> {
        self.capability_profile_id.validate()?;
        self.attachment_plan_id.validate()?;
        if self.capability_profile_id == self.attachment_plan_id
            || self.initial_statuses.iter().any(|status| {
                matches!(
                    status,
                    SeriesLinkObligationStatusV2::Live | SeriesLinkObligationStatusV2::Terminal
                )
            })
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Typed content identity of this immutable configuration.
    pub fn id(self) -> Result<SeriesLinkObligationConfigurationV2Id> {
        self.validate()?;
        let mut body = [0_u8; 68];
        body[..32].copy_from_slice(&self.capability_profile_id.bytes());
        body[32..64].copy_from_slice(&self.attachment_plan_id.bytes());
        let mut index = 0_usize;
        while index < SERIES_LINK_OBLIGATION_COUNT_V2 {
            body[64 + index] = self.initial_statuses[index].wire_byte();
            index += 1;
        }
        Ok(SeriesLinkObligationConfigurationV2Id(content_id(
            b"dragons-clutch/series-link-obligation-configuration/v2",
            &body,
        )))
    }
}

/// Typed identity of one immutable Series-link obligation configuration.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct SeriesLinkObligationConfigurationV2Id(ContentId);

impl SeriesLinkObligationConfigurationV2Id {
    /// Construct from exact digest bytes without claiming authenticity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(ContentId::from_bytes(bytes))
    }

    /// Return exact digest bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0.bytes()
    }

    /// Return through the generic content-ID boundary.
    pub const fn content_id(self) -> ContentId {
        self.0
    }

    /// Refuse the all-zero reserved identity.
    pub fn validate(self) -> Result<()> {
        self.0.validate()
    }
}

/// Complete immutable Series/ordinal/Source admission binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesMarketLinkBindingV2 {
    /// Exact Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact finite ordinal.
    pub ordinal: u32,
    /// Shared Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Shared Market root account.
    pub market_root_account_id: ContentId,
    /// Shared Market immutable binding.
    pub market_binding_id: ContentId,
    /// Founder or exact converger.
    pub disposition: SeriesMarketDispositionV1,
    /// Series funding ownership terms.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Current six-compartment QuoteV5.
    pub funding_quote_id: SeriesFundingQuoteV5Id,
    /// Exact current attachment plan.
    pub attachment_plan_id: SeriesAttachmentPlanV5Id,
    /// Exact central capability profile inherited from the shared Market.
    pub capability_profile_id: ContentId,
    /// Exact capability/attachment-derived obligation configuration.
    pub obligation_configuration_id: SeriesLinkObligationConfigurationV2Id,
    /// Exact current compiler output bundle.
    pub compiler_bundle_id: CompiledProductSeriesBundleV6Id,
    /// Exact compiled Source occurrence.
    pub source_occurrence_id: SourceOccurrenceV1Id,
    /// Physical Source occurrence account.
    pub source_occurrence_account_id: ContentId,
    /// Full hostile account-authentication receipt.
    pub source_occurrence_account_authentication_id: ContentId,
    /// Move-only Product pre-root Source postwrite identity. The inner
    /// occurrence record/account/authentication remain in the preceding fields.
    pub source_occurrence_receipt_id: ContentId,
    /// Authenticated Source release.
    pub source_release_id: ContentId,
    /// Exact Source route.
    pub source_route_id: ContentId,
    /// Sole Clock policy.
    pub clock_policy_id: ContentId,
    /// SourcePlane contract.
    pub source_plane_contract_id: ContentId,
    /// Source specification.
    pub source_spec_id: ContentId,
    /// Exact Window.
    pub window_spec_id: ContentId,
    /// Exact Statistic.
    pub statistic_key_id: ContentId,
    /// Exact funding state account.
    pub funding_state_account_id: ContentId,
    /// Private pending-reservation debit receipt.
    pub funding_debit_receipt_id: ContentId,
    /// `0xad` refundable rent owner.
    pub rent_refund_owner: ContentId,
    /// `0xad` donation sink.
    pub neutral_lamport_sink: ContentId,
    /// Shared Failure/Source generation.
    pub generation: u64,
    /// Exact Source repair generation.
    pub source_repair_generation: u64,
    /// Funding transition sequence reserved for this link.
    pub funding_transition_sequence: u64,
}

impl SeriesMarketLinkBindingV2 {
    /// Validate the complete link without turning it into authority.
    pub fn validate(self) -> Result<()> {
        self.series_plan_id.validate()?;
        self.market_instance_id.validate()?;
        self.funding_terms_id.validate()?;
        self.funding_quote_id.validate()?;
        self.attachment_plan_id.validate()?;
        self.compiler_bundle_id.validate()?;
        self.source_occurrence_id.validate()?;
        self.obligation_configuration_id.validate()?;
        if self.generation == 0
            || self.source_repair_generation == 0
            || self.funding_transition_sequence == 0
        {
            return Err(Error::InvalidParameter);
        }
        let ids = self.identity_ids();
        for id in ids {
            id.validate()?;
        }
        require_pairwise_distinct(&ids)
    }

    /// Immutable link-binding identity.
    pub fn id(self) -> Result<ContentId> {
        self.validate()?;
        let mut body = [0u8; 829];
        let mut at = 0usize;
        for id in self.identity_ids() {
            body[at..at + 32].copy_from_slice(&id.bytes());
            at += 32;
        }
        body[at..at + 4].copy_from_slice(&self.ordinal.to_le_bytes());
        at += 4;
        body[at] = match self.disposition {
            SeriesMarketDispositionV1::Founder => 1,
            SeriesMarketDispositionV1::Converger => 2,
        };
        at += 1;
        body[at..at + 8].copy_from_slice(&self.generation.to_le_bytes());
        at += 8;
        body[at..at + 8].copy_from_slice(&self.source_repair_generation.to_le_bytes());
        at += 8;
        body[at..at + 8].copy_from_slice(&self.funding_transition_sequence.to_le_bytes());
        Ok(content_id(SERIES_MARKET_LINK_DOMAIN_V2, &body))
    }

    fn identity_ids(self) -> [ContentId; LINK_BINDING_ID_COUNT_V2] {
        [
            self.series_plan_id.content_id(),
            self.market_instance_id.content_id(),
            self.market_root_account_id,
            self.market_binding_id,
            self.funding_terms_id.content_id(),
            self.funding_quote_id.content_id(),
            self.attachment_plan_id.content_id(),
            self.capability_profile_id,
            self.obligation_configuration_id.content_id(),
            self.compiler_bundle_id.content_id(),
            self.source_occurrence_id.content_id(),
            self.source_occurrence_account_id,
            self.source_occurrence_account_authentication_id,
            self.source_occurrence_receipt_id,
            self.source_release_id,
            self.source_route_id,
            self.clock_policy_id,
            self.source_plane_contract_id,
            self.source_spec_id,
            self.window_spec_id,
            self.statistic_key_id,
            self.funding_state_account_id,
            self.funding_debit_receipt_id,
            self.rent_refund_owner,
            self.neutral_lamport_sink,
        ]
    }
}

/// Link lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesMarketLinkPhaseV2 {
    /// `0xad` exists and Series funding is pending, but shared Market is inert.
    PendingMarket = 1,
    /// Shared Market is Active and this ordinal is committed.
    Active = 2,
    /// All link obligations closed; root consumption is pending.
    Retiring = 3,
    /// Shared root consumed the link-retirement receipt.
    Retired = 4,
    /// Inert shared Market abort authorized restoration of the ordinal.
    Aborted = 5,
}

impl SeriesMarketLinkPhaseV2 {
    const fn byte(self) -> u8 {
        match self {
            Self::PendingMarket => 1,
            Self::Active => 2,
            Self::Retiring => 3,
            Self::Retired => 4,
            Self::Aborted => 5,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::PendingMarket),
            2 => Ok(Self::Active),
            3 => Ok(Self::Retiring),
            4 => Ok(Self::Retired),
            5 => Ok(Self::Aborted),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// One link-obligation terminal projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesLinkObligationTerminalProjectionV2 {
    /// Exact link state before consumption.
    pub link_semantic_id: SeriesMarketLinkV2Id,
    /// Exact obligation.
    pub obligation: SeriesLinkObligationV2,
    /// Terminal or authenticated absence.
    pub disposition: SeriesLinkObligationDispositionV2,
    /// Link transition sequence after consumption.
    pub link_transition_sequence: u64,
    /// Typed owner receipt.
    pub owner_terminal_receipt_id: ContentId,
}

/// One authenticated Series-scoped child admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesLinkObligationAdmissionProjectionV2 {
    /// Exact link state before admission.
    pub link_semantic_id: SeriesMarketLinkV2Id,
    /// Exact obligation family.
    pub obligation: SeriesLinkObligationV2,
    /// Link transition sequence after admission.
    pub link_transition_sequence: u64,
    /// Family-owned admission receipt.
    pub owner_admission_receipt_id: ContentId,
}

impl SeriesLinkObligationAdmissionProjectionV2 {
    /// Deterministic receipt identity.
    pub fn id(self) -> Result<ContentId> {
        self.link_semantic_id.validate()?;
        self.owner_admission_receipt_id.validate()?;
        if self.link_transition_sequence == 0 {
            return Err(Error::InvalidParameter);
        }
        let mut body = [0u8; 73];
        body[0] = self.obligation.byte();
        body[1..33].copy_from_slice(&self.link_semantic_id.bytes());
        body[33..41].copy_from_slice(&self.link_transition_sequence.to_le_bytes());
        body[41..73].copy_from_slice(&self.owner_admission_receipt_id.bytes());
        Ok(content_id(
            b"dragons-clutch/series-link-obligation-admission/v2",
            &body,
        ))
    }
}

impl SeriesLinkObligationTerminalProjectionV2 {
    /// Deterministic receipt identity.
    pub fn id(self) -> Result<ContentId> {
        self.link_semantic_id.validate()?;
        self.owner_terminal_receipt_id.validate()?;
        if self.link_transition_sequence == 0 {
            return Err(Error::InvalidParameter);
        }
        let mut body = [0u8; 74];
        body[0] = self.obligation.byte();
        body[1] = self.disposition.wire_byte();
        body[2..34].copy_from_slice(&self.link_semantic_id.bytes());
        body[34..42].copy_from_slice(&self.link_transition_sequence.to_le_bytes());
        body[42..74].copy_from_slice(&self.owner_terminal_receipt_id.bytes());
        Ok(content_id(
            b"dragons-clutch/series-link-obligation-terminal/v2",
            &body,
        ))
    }
}

/// Mutable per-Series admission/link state stored under central tag `0xad/1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesMarketLinkV2 {
    binding: SeriesMarketLinkBindingV2,
    phase: SeriesMarketLinkPhaseV2,
    transition_sequence: u64,
    market_admission_sequence: u64,
    market_admission_receipt_id: ContentId,
    rent_principal_lamports: u64,
    donation_floor_lamports: u64,
    current_donation_lamports: u64,
    obligation_statuses: [SeriesLinkObligationStatusV2; SERIES_LINK_OBLIGATION_COUNT_V2],
    admission_receipts: [ContentId; SERIES_LINK_OBLIGATION_COUNT_V2],
    terminal_receipts: [ContentId; SERIES_LINK_OBLIGATION_COUNT_V2],
    active_failure_sessions: u32,
    failure_sessions_started: u32,
    failure_session_transcript_id: ContentId,
}

impl SeriesMarketLinkV2 {
    /// Invalid storage used only as an adapter out-parameter decode target.
    pub fn decode_buffer() -> Self {
        Self {
            binding: link_binding_from_ids(
                [ContentId::ZERO; LINK_BINDING_ID_COUNT_V2],
                0,
                SeriesMarketDispositionV1::Founder,
                0,
                0,
                0,
            ),
            phase: SeriesMarketLinkPhaseV2::PendingMarket,
            transition_sequence: 0,
            market_admission_sequence: 0,
            market_admission_receipt_id: ContentId::ZERO,
            rent_principal_lamports: 0,
            donation_floor_lamports: 0,
            current_donation_lamports: 0,
            obligation_statuses: [SeriesLinkObligationStatusV2::CapabilityDisabled;
                SERIES_LINK_OBLIGATION_COUNT_V2],
            admission_receipts: [ContentId::ZERO; SERIES_LINK_OBLIGATION_COUNT_V2],
            terminal_receipts: [ContentId::ZERO; SERIES_LINK_OBLIGATION_COUNT_V2],
            active_failure_sessions: 0,
            failure_sessions_started: 0,
            failure_session_transcript_id: ContentId::ZERO,
        }
    }

    /// Create one pending link from exact SeriesAdmission principal.
    pub fn initialize_pending(
        binding: SeriesMarketLinkBindingV2,
        obligation_configuration: SeriesLinkObligationConfigurationV2,
        rent_principal_lamports: u64,
        donation_floor_lamports: u64,
    ) -> Result<Self> {
        binding.validate()?;
        obligation_configuration.validate()?;
        if binding.attachment_plan_id.content_id() != obligation_configuration.attachment_plan_id
            || binding.capability_profile_id != obligation_configuration.capability_profile_id
            || binding.obligation_configuration_id != obligation_configuration.id()?
        {
            return Err(Error::MismatchedArtifact);
        }
        if rent_principal_lamports == 0 {
            return Err(Error::InsufficientPrepayment);
        }
        rent_principal_lamports
            .checked_add(donation_floor_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        let value = Self {
            binding,
            phase: SeriesMarketLinkPhaseV2::PendingMarket,
            transition_sequence: 0,
            market_admission_sequence: 0,
            market_admission_receipt_id: ContentId::ZERO,
            rent_principal_lamports,
            donation_floor_lamports,
            current_donation_lamports: donation_floor_lamports,
            obligation_statuses: obligation_configuration.initial_statuses,
            admission_receipts: [ContentId::ZERO; SERIES_LINK_OBLIGATION_COUNT_V2],
            terminal_receipts: [ContentId::ZERO; SERIES_LINK_OBLIGATION_COUNT_V2],
            active_failure_sessions: 0,
            failure_sessions_started: 0,
            failure_session_transcript_id: ContentId::ZERO,
        };
        value.validate()?;
        Ok(value)
    }

    /// Activate only after the shared root admitted this exact link and became Active.
    pub fn activate(
        self,
        market_admission_sequence: u64,
        market_admission_receipt_id: ContentId,
    ) -> Result<Self> {
        self.validate()?;
        market_admission_receipt_id.validate()?;
        if self.phase != SeriesMarketLinkPhaseV2::PendingMarket || market_admission_sequence == 0 {
            return Err(Error::WorkStateMismatch);
        }
        let next = Self {
            phase: SeriesMarketLinkPhaseV2::Active,
            transition_sequence: 1,
            market_admission_sequence,
            market_admission_receipt_id,
            ..self
        };
        next.validate()?;
        Ok(next)
    }
}

impl MarketLifecycleRootV2 {
    /// Invalid zeroed storage used only as an adapter out-parameter decode
    /// target. No caller may treat this value as state unless decode returns
    /// success after replacing every field.
    pub const fn decode_buffer() -> Self {
        Self {
            binding: MarketLifecycleBindingV2 {
                market_instance_id: MarketInstanceV2Id::from_bytes([0; 32]),
                generation: 0,
                outcome_count: 0,
                maximum_series_links: 0,
                product_template_id: ContentId::ZERO,
                native_claim_basis_id: ContentId::ZERO,
                recovery_policy_id: ContentId::ZERO,
                price_measure_policy_id: ContentId::ZERO,
                market_genesis_profile_id: ContentId::ZERO,
                registry_release_id: ContentId::ZERO,
                capability_profile_id: ContentId::ZERO,
                realm_id: ContentId::ZERO,
                collateral_profile_id: ContentId::ZERO,
                collateral_policy_id: ContentId::ZERO,
                collateral_release_id: ContentId::ZERO,
                source_release_id: ContentId::ZERO,
                source_route_id: ContentId::ZERO,
                clock_policy_id: ContentId::ZERO,
                source_plane_contract_id: ContentId::ZERO,
                source_spec_id: ContentId::ZERO,
                primary_window_id: ContentId::ZERO,
                statistic_key_id: ContentId::ZERO,
                market_failure_policy_binding_id: ContentId::ZERO,
                recovery_state_id: ContentId::ZERO,
                interval_consensus_profile_id: ContentId::ZERO,
                failure_liveness_policy_id: ContentId::ZERO,
                failure_liveness_quote_schedule_id: ContentId::ZERO,
                resolution_account_id: ContentId::ZERO,
                foundation_vault_id: ContentId::ZERO,
                foundation_account_graph_id: MarketFoundationAccountGraphV3Id::from_bytes([0; 32]),
                foundation_schedule_id: MarketFoundationScheduleV3Id::from_bytes([0; 32]),
                direct_global_liveness_binding_id: ContentId::ZERO,
                market_liability_founding_id: ContentId::ZERO,
                claim_mint_founding_plan_id: ContentId::ZERO,
                claim_issuance_binding_id: ContentId::ZERO,
                general_founding_capability_id: ContentId::ZERO,
                founding_abort_policy_id: ContentId::ZERO,
                founding_deadline_bucket: 0,
                maximum_interval_width: 0,
                maximum_coordinates_per_advance: 0,
            },
            phase: MarketLifecyclePhaseV2::Founding,
            transition_sequence: 0,
            capital: MarketFoundationCapitalV2 {
                founder_link_id: SeriesMarketLinkV2Id::from_bytes([0; 32]),
                market_core_debit_receipt_id: ContentId::ZERO,
                recovery_debit_receipt_id: ContentId::ZERO,
                rent_refund_owner: ContentId::ZERO,
                neutral_lamport_sink: ContentId::ZERO,
                principal_total_lamports: 0,
                principal_remaining_lamports: 0,
                vault_donation_floor_lamports: 0,
                vault_current_donation_lamports: 0,
                recovery_work_principal_lamports: 0,
                recovery_rent_principal_lamports: 0,
            },
            foundation: MarketFoundationProgressV2 {
                expected_bitmap: 0,
                initialized_bitmap: 0,
                abort_closed_bitmap: 0,
                sequence: 0,
                transcript_id: ContentId::ZERO,
            },
            admitted_series_links: 0,
            live_series_links: 0,
            retired_series_links: 0,
            series_link_transcript_id: ContentId::ZERO,
            product_families: MarketFamilyAggregatorV1::decode_buffer(),
            shared_core_terminal_receipts: [ContentId::ZERO; MARKET_SHARED_CORE_COUNT_V2],
            fractional_terminal_state_ids: [ContentId::ZERO; 2],
            resolution_semantic_id: ContentId::ZERO,
            resolution_data_id: ContentId::ZERO,
            resolution_activation_receipt_id: ContentId::ZERO,
        }
    }

    /// Record finalized Resolution V5 once while the Market is Active.
    pub fn record_resolution_activation(
        self,
        activation: MarketResolutionActivationV2,
    ) -> Result<Self> {
        let mut output = Self::decode_buffer();
        self.record_resolution_activation_into(activation, &mut output)?;
        Ok(output)
    }

    /// Frame-bounded once-only Resolution activation into caller-owned storage.
    ///
    /// The SBF adapter uses this form so it does not retain a second 2,480-byte
    /// root successor on its instruction stack.
    pub fn record_resolution_activation_into(
        &self,
        activation: MarketResolutionActivationV2,
        output: &mut Self,
    ) -> Result<()> {
        self.validate()?;
        if self.phase != MarketLifecyclePhaseV2::Active
            || self.resolution_activation_receipt_id != ContentId::ZERO
            || activation.market_binding_id != self.binding.id()?
            || activation.market_instance_id != self.binding.market_instance_id
            || activation.generation != self.binding.generation
            || activation.resolution_account_id != self.binding.resolution_account_id
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        *output = *self;
        output.transition_sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        output.resolution_semantic_id = activation.resolution_semantic_id;
        output.resolution_data_id = activation.resolution_data_id;
        output.resolution_activation_receipt_id = activation.id;
        output.validate()
    }

    /// Consume one retiring Series link and decrement only the dynamic link count.
    pub fn retire_series_link(
        self,
        retirement: SeriesMarketLinkRetirementProjectionV2,
    ) -> Result<Self> {
        let mut output = Self::decode_buffer();
        self.retire_series_link_into(retirement, &mut output)?;
        Ok(output)
    }

    /// Consume one retiring Series link into caller-owned storage.
    ///
    /// The SBF owner uses this frame-bounded form so a 2,452-byte root
    /// successor is never retained as an additional adapter-frame local.
    pub fn retire_series_link_into(
        &self,
        retirement: SeriesMarketLinkRetirementProjectionV2,
        output: &mut Self,
    ) -> Result<()> {
        self.validate()?;
        if self.phase != MarketLifecyclePhaseV2::Active
            || retirement.market_instance_id != self.binding.market_instance_id
            || retirement.generation != self.binding.generation
            || retirement.market_admission_receipt_id == ContentId::ZERO
            || self.live_series_links == 0
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        let sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        *output = Self {
            transition_sequence: sequence,
            live_series_links: self
                .live_series_links
                .checked_sub(1)
                .ok_or(Error::ArithmeticOverflow)?,
            retired_series_links: self
                .retired_series_links
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            series_link_transcript_id: rolling_id(
                b"dragons-clutch/market-series-link-transcript/v2",
                self.series_link_transcript_id,
                retirement.id,
                sequence,
            ),
            ..*self
        };
        output.validate()
    }

    /// Disable new links and delegate product-family admission sealing.
    pub fn begin_retirement<A: AuthenticatedMarketFamilyAuthorityV1 + ?Sized>(
        self,
        authority: &A,
    ) -> Result<Self> {
        self.validate()?;
        if self.phase != MarketLifecyclePhaseV2::Active
            || self.live_series_links != 0
            || self.admitted_series_links == 0
            || self.retired_series_links != self.admitted_series_links
            || self.resolution_activation_receipt_id == ContentId::ZERO
        {
            return Err(Error::WorkIncomplete);
        }
        let product_families = self.product_families.begin_retirement(authority)?;
        let next = Self {
            phase: MarketLifecyclePhaseV2::Retiring,
            transition_sequence: self
                .transition_sequence
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            product_families,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Consume one mandatory shared-core terminal receipt exactly once.
    pub fn consume_shared_core_terminal(
        self,
        projection: MarketSharedCoreTerminalProjectionV2,
    ) -> Result<Self> {
        self.validate()?;
        if self.phase != MarketLifecyclePhaseV2::Retiring
            || projection.market_binding_id != self.binding.id()?
            || projection.market_instance_id != self.binding.market_instance_id
            || projection.generation != self.binding.generation
            || projection.root_transition_sequence
                != self
                    .transition_sequence
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        let index = projection.owner.index();
        if self.shared_core_terminal_receipts[index] != ContentId::ZERO
            || self
                .shared_core_terminal_receipts
                .iter()
                .any(|seen| *seen == projection.id)
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        let mut receipts = self.shared_core_terminal_receipts;
        receipts[index] = projection.id;
        let next = Self {
            transition_sequence: projection.root_transition_sequence,
            shared_core_terminal_receipts: receipts,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Seal the Market and emit the only whole-Market terminal projection.
    pub fn finalize_terminal(self) -> Result<(Self, MarketInstanceTerminalProjectionV2)> {
        self.validate()?;
        if self.phase != MarketLifecyclePhaseV2::Retiring
            || self
                .shared_core_terminal_receipts
                .iter()
                .any(|receipt| receipt.is_zero())
        {
            return Err(Error::WorkIncomplete);
        }
        let (product_families, _) = self.product_families.finalize_terminal()?;
        let next = Self {
            phase: MarketLifecyclePhaseV2::Terminal,
            transition_sequence: self
                .transition_sequence
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            product_families,
            ..self
        };
        next.validate()?;
        let projection = next.terminal_projection()?;
        Ok((next, projection))
    }

    /// Re-derive the whole-Market terminal projection from a terminal root.
    ///
    /// This is structural only. A live adapter must first authenticate the
    /// exact program owner, PDA, full account bytes, Market, and generation.
    pub fn terminal_projection(&self) -> Result<MarketInstanceTerminalProjectionV2> {
        self.validate()?;
        if self.phase != MarketLifecyclePhaseV2::Terminal {
            return Err(Error::WorkIncomplete);
        }
        let root_semantic_id = self.semantic_id()?;
        let product_family_terminal_projection_id = self
            .product_families
            .terminal_projection()?
            .id()?
            .content_id();
        let mut body = [0u8; 444];
        body[..32].copy_from_slice(&root_semantic_id.bytes());
        body[32..64].copy_from_slice(&self.binding.market_instance_id.bytes());
        body[64..72].copy_from_slice(&self.binding.generation.to_le_bytes());
        body[72..80].copy_from_slice(&self.transition_sequence.to_le_bytes());
        body[80..112].copy_from_slice(&product_family_terminal_projection_id.bytes());
        let mut at = 112usize;
        for receipt in self.shared_core_terminal_receipts {
            body[at..at + 32].copy_from_slice(&receipt.bytes());
            at += 32;
        }
        for id in self.fractional_terminal_state_ids {
            body[at..at + 32].copy_from_slice(&id.bytes());
            at += 32;
        }
        body[336..368].copy_from_slice(&self.resolution_semantic_id.bytes());
        body[368..400].copy_from_slice(&self.resolution_data_id.bytes());
        body[400..432].copy_from_slice(&self.resolution_activation_receipt_id.bytes());
        body[432..436].copy_from_slice(&self.admitted_series_links.to_le_bytes());
        body[436..444].copy_from_slice(&self.capital.principal_total_lamports.to_le_bytes());
        let id = content_id(MARKET_INSTANCE_TERMINAL_PROJECTION_DOMAIN_V2, &body);
        Ok(MarketInstanceTerminalProjectionV2 {
            id,
            root_semantic_id,
            market_instance_id: self.binding.market_instance_id,
            generation: self.binding.generation,
            final_transition_sequence: self.transition_sequence,
            product_family_terminal_projection_id,
            shared_core_terminal_receipts: self.shared_core_terminal_receipts,
            fractional_terminal_state_ids: self.fractional_terminal_state_ids,
            resolution_semantic_id: self.resolution_semantic_id,
            resolution_data_id: self.resolution_data_id,
            resolution_activation_receipt_id: self.resolution_activation_receipt_id,
            admitted_series_links: self.admitted_series_links,
        })
    }

    /// Enter timeout abort and refund all still-unspent FoundationVault principal.
    pub fn begin_abort(
        self,
        authenticated_current_bucket: u64,
        observed_vault_donation_lamports: u64,
        vault_refund_receipt_id: ContentId,
    ) -> Result<Self> {
        let mut output = Self::decode_buffer();
        self.begin_abort_into(
            authenticated_current_bucket,
            observed_vault_donation_lamports,
            vault_refund_receipt_id,
            &mut output,
        )?;
        Ok(output)
    }

    /// Frame-bounded timeout-abort transition into caller-owned storage.
    pub fn begin_abort_into(
        &self,
        authenticated_current_bucket: u64,
        observed_vault_donation_lamports: u64,
        vault_refund_receipt_id: ContentId,
        output: &mut Self,
    ) -> Result<()> {
        self.validate()?;
        vault_refund_receipt_id.validate()?;
        if self.phase != MarketLifecyclePhaseV2::Founding
            || authenticated_current_bucket <= self.binding.founding_deadline_bucket
            || self.admitted_series_links != 1
            || self.retired_series_links != 0
            || self.resolution_activation_receipt_id != ContentId::ZERO
            || observed_vault_donation_lamports < self.capital.vault_current_donation_lamports
        {
            return Err(Error::WorkStateMismatch);
        }
        let sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut capital = self.capital;
        capital.principal_remaining_lamports = 0;
        capital.vault_current_donation_lamports = observed_vault_donation_lamports;
        let mut foundation = self.foundation;
        foundation.sequence = foundation
            .sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        foundation.transcript_id = rolling_id(
            b"dragons-clutch/market-foundation-abort/v2",
            foundation.transcript_id,
            vault_refund_receipt_id,
            sequence,
        );
        *output = *self;
        output.phase = MarketLifecyclePhaseV2::Aborting;
        output.transition_sequence = sequence;
        output.capital = capital;
        output.foundation = foundation;
        output.validate()
    }

    /// Close one initialized non-root slot in exact reverse slot order.
    pub fn record_abort_close(
        self,
        slot: MarketFoundationSlotV3,
        close_receipt_id: ContentId,
    ) -> Result<Self> {
        self.validate()?;
        close_receipt_id.validate()?;
        if self.phase != MarketLifecyclePhaseV2::Aborting {
            return Err(Error::WorkStateMismatch);
        }
        let index = slot.index()?;
        if index == MarketFoundationSlotV3::LifecycleRoot.index()? {
            return Err(Error::InvalidParameter);
        }
        let remaining = self.foundation.initialized_bitmap
            & !self.foundation.abort_closed_bitmap
            & !slot_bit(MarketFoundationSlotV3::LifecycleRoot.index()?)?;
        let expected = highest_set_index(remaining).ok_or(Error::WorkIncomplete)?;
        if index != expected {
            return Err(Error::WorkStateMismatch);
        }
        let sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut foundation = self.foundation;
        foundation.abort_closed_bitmap |= slot_bit(index)?;
        foundation.sequence = foundation
            .sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        foundation.transcript_id = rolling_id(
            b"dragons-clutch/market-foundation-abort/v2",
            foundation.transcript_id,
            close_receipt_id,
            sequence,
        );
        let next = Self {
            transition_sequence: sequence,
            foundation,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Authorize atomic root close and pending Series reservation restoration.
    pub fn finalize_abort(self) -> Result<MarketFoundingAbortProjectionV2> {
        self.validate()?;
        let root_bit = slot_bit(MarketFoundationSlotV3::LifecycleRoot.index()?)?;
        if self.phase != MarketLifecyclePhaseV2::Aborting
            || self.capital.principal_remaining_lamports != 0
            || self.foundation.abort_closed_bitmap
                != (self.foundation.initialized_bitmap & !root_bit)
        {
            return Err(Error::WorkIncomplete);
        }
        let sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut body = [0u8; 200];
        body[..32].copy_from_slice(&self.binding.market_instance_id.bytes());
        body[32..40].copy_from_slice(&self.binding.generation.to_le_bytes());
        body[40..72].copy_from_slice(&self.capital.founder_link_id.bytes());
        body[72..104].copy_from_slice(&self.capital.rent_refund_owner.bytes());
        body[104..136].copy_from_slice(&self.capital.neutral_lamport_sink.bytes());
        body[136..144].copy_from_slice(&self.capital.principal_total_lamports.to_le_bytes());
        body[144..152].copy_from_slice(&self.capital.vault_current_donation_lamports.to_le_bytes());
        body[152..160].copy_from_slice(&sequence.to_le_bytes());
        body[160..192].copy_from_slice(&self.foundation.transcript_id.bytes());
        body[192..200].copy_from_slice(&self.foundation.initialized_bitmap.to_le_bytes());
        let id = content_id(b"dragons-clutch/market-founding-abort/v2", &body);
        Ok(MarketFoundingAbortProjectionV2 {
            id,
            market_instance_id: self.binding.market_instance_id,
            generation: self.binding.generation,
            founder_link_id: self.capital.founder_link_id,
            refund_owner: self.capital.rent_refund_owner,
            neutral_lamport_sink: self.capital.neutral_lamport_sink,
            refundable_principal_lamports: self.capital.principal_total_lamports,
            donation_lamports: self.capital.vault_current_donation_lamports,
            final_transition_sequence: sequence,
        })
    }

    /// Immutable Market binding.
    pub const fn binding(&self) -> MarketLifecycleBindingV2 {
        self.binding
    }
    /// Lifecycle phase.
    pub const fn phase(&self) -> MarketLifecyclePhaseV2 {
        self.phase
    }
    /// Transition sequence.
    pub const fn transition_sequence(&self) -> u64 {
        self.transition_sequence
    }
    /// Founder capitalization and Recovery decomposition.
    pub const fn capital(&self) -> MarketFoundationCapitalV2 {
        self.capital
    }
    /// Bounded founding progress.
    pub const fn foundation(&self) -> MarketFoundationProgressV2 {
        self.foundation
    }
    /// Total admitted Series links.
    pub const fn admitted_series_links(&self) -> u32 {
        self.admitted_series_links
    }
    /// Links not yet retired.
    pub const fn live_series_links(&self) -> u32 {
        self.live_series_links
    }
    /// Links retired into this root.
    pub const fn retired_series_links(&self) -> u32 {
        self.retired_series_links
    }
    /// Embedded exhaustive product-family semantic owner.
    pub const fn product_families(&self) -> &MarketFamilyAggregatorV1 {
        &self.product_families
    }
    /// Mandatory shared-core terminal receipts in [`MarketSharedCoreV2`] order.
    pub const fn shared_core_terminal_receipts(&self) -> [ContentId; MARKET_SHARED_CORE_COUNT_V2] {
        self.shared_core_terminal_receipts
    }
    /// Exact terminal receipt consumed for one mandatory shared-core owner.
    pub const fn shared_core_terminal_receipt(&self, owner: MarketSharedCoreV2) -> ContentId {
        self.shared_core_terminal_receipts[owner.index()]
    }
    /// Exact exhaustive Failure-family receipt consumed before Market terminality.
    pub const fn failure_terminal_receipt_id(&self) -> ContentId {
        self.shared_core_terminal_receipt(MarketSharedCoreV2::Failure)
    }
    /// Resolution semantic identity, zero before activation.
    pub const fn resolution_semantic_id(&self) -> ContentId {
        self.resolution_semantic_id
    }
    /// Resolution data identity, zero before activation.
    pub const fn resolution_data_id(&self) -> ContentId {
        self.resolution_data_id
    }
    /// Once-only Resolution activation receipt.
    pub const fn resolution_activation_receipt_id(&self) -> ContentId {
        self.resolution_activation_receipt_id
    }

    /// Domain-separated complete state identity.
    pub fn semantic_id(&self) -> Result<ContentId> {
        let mut body = [0u8; MARKET_LIFECYCLE_ROOT_BYTES_V2];
        self.encode_into(&mut body)?;
        Ok(content_id(MARKET_LIFECYCLE_ROOT_DOMAIN_V2, &body))
    }

    fn validate_against_schedule(&self, schedule: &MarketFoundationScheduleV3) -> Result<()> {
        self.validate()?;
        schedule.validate()?;
        if schedule.id()? != self.binding.foundation_schedule_id
            || schedule.outcome_count != self.binding.outcome_count
        {
            return Err(Error::MismatchedArtifact);
        }
        self.capital.validate(schedule)?;
        if self.phase != MarketLifecyclePhaseV2::Aborting {
            let mut spent = 0u64;
            let mut index = 0usize;
            while index < MARKET_FOUNDATION_SLOT_COUNT_V3 {
                if (self.foundation.initialized_bitmap & slot_bit(index)?) != 0 {
                    spent = spent
                        .checked_add(schedule.slot_principal_lamports[index])
                        .ok_or(Error::ArithmeticOverflow)?;
                }
                index += 1;
            }
            let expected_remaining = self
                .capital
                .principal_total_lamports
                .checked_sub(spent)
                .ok_or(Error::InvalidComponentStatus)?;
            if expected_remaining != self.capital.principal_remaining_lamports {
                return Err(Error::InvalidComponentStatus);
            }
        }
        Ok(())
    }

    fn validate(self) -> Result<()> {
        self.binding.validate()?;
        self.capital.founder_link_id.validate()?;
        for id in [
            self.capital.market_core_debit_receipt_id,
            self.capital.recovery_debit_receipt_id,
            self.capital.rent_refund_owner,
            self.capital.neutral_lamport_sink,
            self.foundation.transcript_id,
        ] {
            id.validate()?;
        }
        let expected_bitmap = expected_foundation_bitmap(self.binding.outcome_count)?;
        let root_bit = slot_bit(MarketFoundationSlotV3::LifecycleRoot.index()?)?;
        if self.foundation.expected_bitmap != expected_bitmap
            || (self.foundation.initialized_bitmap & !expected_bitmap) != 0
            || (self.foundation.abort_closed_bitmap & !self.foundation.initialized_bitmap) != 0
            || (self.foundation.initialized_bitmap & root_bit) == 0
            || self.foundation.sequence == 0
            || self.capital.principal_total_lamports == 0
            || self.capital.principal_remaining_lamports > self.capital.principal_total_lamports
            || self.capital.vault_current_donation_lamports
                < self.capital.vault_donation_floor_lamports
            || self.capital.recovery_work_principal_lamports == 0
            || self.capital.recovery_rent_principal_lamports == 0
        {
            return Err(Error::WorkStateMismatch);
        }
        if self
            .live_series_links
            .checked_add(self.retired_series_links)
            .ok_or(Error::ArithmeticOverflow)?
            != self.admitted_series_links
            || self.admitted_series_links > self.binding.maximum_series_links
            || (self.admitted_series_links == 0)
                != (self.series_link_transcript_id == ContentId::ZERO)
        {
            return Err(Error::WorkStateMismatch);
        }
        self.product_families.validate()?;
        if self.product_families.binding().market_instance_id != self.binding.market_instance_id
            || self.product_families.binding().generation != self.binding.generation
            || self
                .product_families
                .binding()
                .registry_release_id
                .content_id()
                != self.binding.registry_release_id
            || self
                .product_families
                .binding()
                .capability_profile_id
                .content_id()
                != self.binding.capability_profile_id
        {
            return Err(Error::MismatchedArtifact);
        }
        let mut index = 0_usize;
        while index < MARKET_SHARED_CORE_COUNT_V2 {
            let receipt = self.shared_core_terminal_receipts[index];
            if receipt != ContentId::ZERO {
                receipt.validate()?;
                let mut prior = 0_usize;
                while prior < index {
                    if self.shared_core_terminal_receipts[prior] == receipt {
                        return Err(Error::MismatchedArtifact);
                    }
                    prior += 1;
                }
            }
            index += 1;
        }
        let expected_aggregator_phase = match self.phase {
            MarketLifecyclePhaseV2::Founding
            | MarketLifecyclePhaseV2::Active
            | MarketLifecyclePhaseV2::Aborting => MarketFamilyAggregatorPhaseV1::Open,
            MarketLifecyclePhaseV2::Retiring => MarketFamilyAggregatorPhaseV1::Retiring,
            MarketLifecyclePhaseV2::Terminal => MarketFamilyAggregatorPhaseV1::Terminal,
        };
        if self.product_families.phase() != expected_aggregator_phase {
            return Err(Error::WorkStateMismatch);
        }
        let resolution = [
            self.resolution_semantic_id,
            self.resolution_data_id,
            self.resolution_activation_receipt_id,
        ];
        let resolution_absent = resolution == [ContentId::ZERO; 3];
        let resolution_live = resolution.iter().all(|id| id.validate().is_ok())
            && resolution[0] != resolution[1]
            && resolution[0] != resolution[2]
            && resolution[1] != resolution[2];
        if !resolution_absent && !resolution_live {
            return Err(Error::WorkStateMismatch);
        }
        match self.phase {
            MarketLifecyclePhaseV2::Founding => {
                if !resolution_absent
                    || self
                        .shared_core_terminal_receipts
                        .iter()
                        .any(|receipt| *receipt != ContentId::ZERO)
                    || self.foundation.abort_closed_bitmap != 0
                {
                    return Err(Error::WorkStateMismatch);
                }
            }
            MarketLifecyclePhaseV2::Active => {
                if !self.foundation.complete()
                    || self.capital.principal_remaining_lamports != 0
                    || self.admitted_series_links == 0
                    || !self.product_families.activation_ready()?
                    || self
                        .shared_core_terminal_receipts
                        .iter()
                        .any(|receipt| *receipt != ContentId::ZERO)
                {
                    return Err(Error::WorkStateMismatch);
                }
            }
            MarketLifecyclePhaseV2::Retiring => {
                if !resolution_live || self.live_series_links != 0 {
                    return Err(Error::WorkStateMismatch);
                }
            }
            MarketLifecyclePhaseV2::Terminal => {
                if !resolution_live
                    || self.live_series_links != 0
                    || self
                        .shared_core_terminal_receipts
                        .iter()
                        .any(|receipt| *receipt == ContentId::ZERO)
                {
                    return Err(Error::WorkStateMismatch);
                }
            }
            MarketLifecyclePhaseV2::Aborting => {
                if !resolution_absent
                    || self.capital.principal_remaining_lamports != 0
                    || self.admitted_series_links != 1
                    || self.retired_series_links != 0
                    || self
                        .shared_core_terminal_receipts
                        .iter()
                        .any(|receipt| *receipt != ContentId::ZERO)
                {
                    return Err(Error::WorkStateMismatch);
                }
            }
        }
        let fractional_terminal_count = self
            .product_families
            .family(MarketFamilyV1::Fractional)
            .counts()
            .terminal;
        let fractional_absent = self.fractional_terminal_state_ids == [ContentId::ZERO; 2];
        let fractional_live = self.fractional_terminal_state_ids[0].validate().is_ok()
            && self.fractional_terminal_state_ids[1].validate().is_ok()
            && self.fractional_terminal_state_ids[0] != self.fractional_terminal_state_ids[1];
        if (fractional_terminal_count == 0 && !fractional_absent)
            || (fractional_terminal_count == 1 && !fractional_live)
            || fractional_terminal_count > 1
        {
            return Err(Error::WorkStateMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod successor_tests {
    use super::*;

    fn ids<const N: usize>(start: u8) -> [ContentId; N] {
        let mut output = [ContentId::ZERO; N];
        let mut index = 0usize;
        while index < N {
            output[index] = ContentId::from_bytes([
                start.checked_add(u8::try_from(index).unwrap()).unwrap(); 32
            ]);
            index += 1;
        }
        output
    }

    #[test]
    fn direct_global_liveness_binding_is_mandatory_and_identity_bearing() {
        let binding = binding_from_ids(ids::<MARKET_BINDING_ID_COUNT_V2>(1), 1, 2, 3, 4, 5, 6);
        let original = binding.id().unwrap();
        let mut missing = binding;
        missing.direct_global_liveness_binding_id = ContentId::ZERO;
        assert_eq!(missing.validate(), Err(Error::ZeroIdentity));
        let mut substituted = binding;
        substituted.direct_global_liveness_binding_id = ContentId::from_bytes([250; 32]);
        assert_ne!(substituted.id().unwrap(), original);
    }

    #[test]
    fn link_bundle_and_attachment_substitution_change_current_binding() {
        let binding = link_binding_from_ids(
            ids::<LINK_BINDING_ID_COUNT_V2>(40),
            3,
            SeriesMarketDispositionV1::Founder,
            7,
            8,
            9,
        );
        let original = binding.id().unwrap();
        let mut bundle = binding;
        bundle.compiler_bundle_id = CompiledProductSeriesBundleV6Id::from_bytes([200; 32]);
        assert_ne!(bundle.id().unwrap(), original);
        let mut attachment = binding;
        attachment.attachment_plan_id = SeriesAttachmentPlanV5Id::from_bytes([201; 32]);
        assert_ne!(attachment.id().unwrap(), original);
    }

    #[test]
    fn obligation_wire_partitions_are_exhaustive_and_disjoint() {
        assert_eq!(SeriesLinkObligationStatusV2::CapabilityDisabled.wire_byte(), 0);
        assert_eq!(SeriesLinkObligationStatusV2::EnabledNeverFounded.wire_byte(), 1);
        assert_eq!(SeriesLinkObligationStatusV2::Live.wire_byte(), 2);
        assert_eq!(SeriesLinkObligationStatusV2::Terminal.wire_byte(), 3);
        assert_eq!(SeriesLinkObligationDispositionV2::Terminal.wire_byte(), 1);
        assert_eq!(SeriesLinkObligationDispositionV2::Absent.wire_byte(), 2);
    }

    #[test]
    fn wrapper_terminal_projection_refuses_owner_and_link_substitution() {
        let semantic = SeriesMarketLinkV2Id::from_bytes([211; 32]);
        let projection = SeriesLinkObligationTerminalProjectionV2 {
            link_semantic_id: semantic,
            obligation: SeriesLinkObligationV2::Wrapper,
            disposition: SeriesLinkObligationDispositionV2::Terminal,
            link_transition_sequence: 17,
            owner_terminal_receipt_id: ContentId::from_bytes([212; 32]),
        };
        let original = projection.id().unwrap();
        let mut owner_splice = projection;
        owner_splice.owner_terminal_receipt_id = ContentId::from_bytes([213; 32]);
        assert_ne!(owner_splice.id().unwrap(), original);
        let mut semantic_splice = projection;
        semantic_splice.link_semantic_id = SeriesMarketLinkV2Id::from_bytes([214; 32]);
        assert_ne!(semantic_splice.id().unwrap(), original);
    }
}

/// Pure terminal output; SBF must reauthenticate the exact persisted root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketInstanceTerminalProjectionV2 {
    id: ContentId,
    root_semantic_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    final_transition_sequence: u64,
    product_family_terminal_projection_id: ContentId,
    shared_core_terminal_receipts: [ContentId; MARKET_SHARED_CORE_COUNT_V2],
    fractional_terminal_state_ids: [ContentId; 2],
    resolution_semantic_id: ContentId,
    resolution_data_id: ContentId,
    resolution_activation_receipt_id: ContentId,
    admitted_series_links: u32,
}

impl MarketInstanceTerminalProjectionV2 {
    /// Projection identity.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Exact terminal root semantic ID.
    pub const fn root_semantic_id(self) -> ContentId {
        self.root_semantic_id
    }
    /// Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    /// Generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Final root sequence.
    pub const fn final_transition_sequence(self) -> u64 {
        self.final_transition_sequence
    }
    /// Exact terminal projection of the embedded five-family aggregator.
    pub const fn product_family_terminal_projection_id(self) -> ContentId {
        self.product_family_terminal_projection_id
    }
    /// Exact ClaimLedger/Hoard/Failure/Source/Position terminal receipts.
    pub const fn shared_core_terminal_receipts(self) -> [ContentId; MARKET_SHARED_CORE_COUNT_V2] {
        self.shared_core_terminal_receipts
    }
    /// Fractional terminal a4/a5 IDs.
    pub const fn fractional_terminal_state_ids(self) -> [ContentId; 2] {
        self.fractional_terminal_state_ids
    }
    /// Resolution semantic ID.
    pub const fn resolution_semantic_id(self) -> ContentId {
        self.resolution_semantic_id
    }
    /// Resolution data ID.
    pub const fn resolution_data_id(self) -> ContentId {
        self.resolution_data_id
    }
    /// Resolution activation receipt.
    pub const fn resolution_activation_receipt_id(self) -> ContentId {
        self.resolution_activation_receipt_id
    }
    /// Exact number of Series links admitted/retired.
    pub const fn admitted_series_links(self) -> u32 {
        self.admitted_series_links
    }
}

impl FixedCodec for MarketLifecycleRootV2 {
    const ENCODED_LEN: usize = MARKET_LIFECYCLE_ROOT_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&ROOT_MAGIC_V2);
        writer.u16(ROOT_VERSION_V2);
        writer.u8(self.phase.byte());
        writer.reserved(5);
        for id in self.binding.identity_ids() {
            writer.id(id);
        }
        writer.u64(self.binding.generation);
        writer.u8(self.binding.outcome_count);
        writer.reserved(3);
        writer.u32(self.binding.maximum_series_links);
        writer.u64(self.binding.founding_deadline_bucket);
        writer.u64(self.binding.maximum_interval_width);
        writer.u16(self.binding.maximum_coordinates_per_advance);
        writer.reserved(6);
        writer.id(self.capital.founder_link_id.content_id());
        writer.id(self.capital.market_core_debit_receipt_id);
        writer.id(self.capital.recovery_debit_receipt_id);
        writer.id(self.capital.rent_refund_owner);
        writer.id(self.capital.neutral_lamport_sink);
        writer.u64(self.capital.principal_total_lamports);
        writer.u64(self.capital.principal_remaining_lamports);
        writer.u64(self.capital.vault_donation_floor_lamports);
        writer.u64(self.capital.vault_current_donation_lamports);
        writer.u64(self.capital.recovery_work_principal_lamports);
        writer.u64(self.capital.recovery_rent_principal_lamports);
        writer.u64(self.foundation.expected_bitmap);
        writer.u64(self.foundation.initialized_bitmap);
        writer.u64(self.foundation.abort_closed_bitmap);
        writer.u32(self.foundation.sequence);
        writer.reserved(4);
        writer.id(self.foundation.transcript_id);
        writer.u32(self.admitted_series_links);
        writer.u32(self.live_series_links);
        writer.u32(self.retired_series_links);
        writer.reserved(4);
        writer.id(self.series_link_transcript_id);
        let mut product_family_body = [0_u8; MARKET_FAMILY_AGGREGATOR_BYTES_V1];
        self.product_families
            .encode_into(&mut product_family_body)?;
        writer.bytes(&product_family_body);
        for receipt in self.shared_core_terminal_receipts {
            writer.id(receipt);
        }
        for id in self.fractional_terminal_state_ids {
            writer.id(id);
        }
        writer.id(self.resolution_semantic_id);
        writer.id(self.resolution_data_id);
        writer.id(self.resolution_activation_receipt_id);
        writer.u64(self.transition_sequence);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut value = Self::decode_buffer();
        Self::decode_into(input, &mut value)?;
        Ok(value)
    }
}

impl MarketLifecycleRootV2 {
    /// Hostile-decode into caller-owned storage so an SBF adapter need not
    /// retain a second 2,448-byte root value on its frame.
    pub fn decode_into(input: &[u8], output: &mut Self) -> Result<()> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&ROOT_MAGIC_V2)?;
        if reader.u16() != ROOT_VERSION_V2 {
            return Err(Error::BadVersion);
        }
        let phase = MarketLifecyclePhaseV2::decode(reader.u8())?;
        reader.reserved(5)?;
        let ids = read_ids::<MARKET_BINDING_ID_COUNT_V2>(&mut reader);
        let binding = binding_from_ids(
            ids,
            reader.u64(),
            reader.u8(),
            {
                reader.reserved(3)?;
                reader.u32()
            },
            reader.u64(),
            reader.u64(),
            reader.u16(),
        );
        reader.reserved(6)?;
        let capital = MarketFoundationCapitalV2 {
            founder_link_id: SeriesMarketLinkV2Id::from_bytes(reader.id().bytes()),
            market_core_debit_receipt_id: reader.id(),
            recovery_debit_receipt_id: reader.id(),
            rent_refund_owner: reader.id(),
            neutral_lamport_sink: reader.id(),
            principal_total_lamports: reader.u64(),
            principal_remaining_lamports: reader.u64(),
            vault_donation_floor_lamports: reader.u64(),
            vault_current_donation_lamports: reader.u64(),
            recovery_work_principal_lamports: reader.u64(),
            recovery_rent_principal_lamports: reader.u64(),
        };
        let foundation = MarketFoundationProgressV2 {
            expected_bitmap: reader.u64(),
            initialized_bitmap: reader.u64(),
            abort_closed_bitmap: reader.u64(),
            sequence: reader.u32(),
            transcript_id: {
                reader.reserved(4)?;
                reader.id()
            },
        };
        let admitted_series_links = reader.u32();
        let live_series_links = reader.u32();
        let retired_series_links = reader.u32();
        reader.reserved(4)?;
        let series_link_transcript_id = reader.id();
        let product_families =
            MarketFamilyAggregatorV1::decode(&reader.bytes::<MARKET_FAMILY_AGGREGATOR_BYTES_V1>())?;
        let mut shared_core_terminal_receipts = [ContentId::ZERO; MARKET_SHARED_CORE_COUNT_V2];
        for receipt in &mut shared_core_terminal_receipts {
            *receipt = reader.id();
        }
        let fractional_terminal_state_ids = [reader.id(), reader.id()];
        let resolution_semantic_id = reader.id();
        let resolution_data_id = reader.id();
        let resolution_activation_receipt_id = reader.id();
        let transition_sequence = reader.u64();
        reader.finish()?;
        output.binding = binding;
        output.phase = phase;
        output.transition_sequence = transition_sequence;
        output.capital = capital;
        output.foundation = foundation;
        output.admitted_series_links = admitted_series_links;
        output.live_series_links = live_series_links;
        output.retired_series_links = retired_series_links;
        output.series_link_transcript_id = series_link_transcript_id;
        output.product_families = product_families;
        output.shared_core_terminal_receipts = shared_core_terminal_receipts;
        output.fractional_terminal_state_ids = fractional_terminal_state_ids;
        output.resolution_semantic_id = resolution_semantic_id;
        output.resolution_data_id = resolution_data_id;
        output.resolution_activation_receipt_id = resolution_activation_receipt_id;
        output.validate()
    }
}

impl FixedCodec for SeriesMarketLinkV2 {
    const ENCODED_LEN: usize = SERIES_MARKET_LINK_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&LINK_MAGIC_V2);
        writer.u16(LINK_VERSION_V2);
        writer.u8(self.phase.byte());
        writer.reserved(5);
        for id in self.binding.identity_ids() {
            writer.id(id);
        }
        writer.u32(self.binding.ordinal);
        writer.u8(match self.binding.disposition {
            SeriesMarketDispositionV1::Founder => 1,
            SeriesMarketDispositionV1::Converger => 2,
        });
        writer.reserved(3);
        writer.u64(self.binding.generation);
        writer.u64(self.binding.source_repair_generation);
        writer.u64(self.binding.funding_transition_sequence);
        writer.reserved(8);
        writer.u64(self.transition_sequence);
        writer.u64(self.market_admission_sequence);
        writer.id(self.market_admission_receipt_id);
        writer.u64(self.rent_principal_lamports);
        writer.u64(self.donation_floor_lamports);
        writer.u64(self.current_donation_lamports);
        for status in self.obligation_statuses {
            writer.u8(status.wire_byte());
        }
        writer.reserved(4);
        for receipt in self.admission_receipts {
            writer.id(receipt);
        }
        for receipt in self.terminal_receipts {
            writer.id(receipt);
        }
        writer.u32(self.active_failure_sessions);
        writer.u32(self.failure_sessions_started);
        writer.id(self.failure_session_transcript_id);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut value = Self::decode_buffer();
        Self::decode_into(input, &mut value)?;
        Ok(value)
    }
}

impl SeriesMarketLinkV2 {
    /// Hostile-decode into caller-owned storage for frame-bounded adapters.
    pub fn decode_into(input: &[u8], output: &mut Self) -> Result<()> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&LINK_MAGIC_V2)?;
        if reader.u16() != LINK_VERSION_V2 {
            return Err(Error::BadVersion);
        }
        let phase = SeriesMarketLinkPhaseV2::decode(reader.u8())?;
        reader.reserved(5)?;
        let ids = read_ids::<LINK_BINDING_ID_COUNT_V2>(&mut reader);
        let ordinal = reader.u32();
        let disposition = match reader.u8() {
            1 => SeriesMarketDispositionV1::Founder,
            2 => SeriesMarketDispositionV1::Converger,
            _ => return Err(Error::InvalidParameter),
        };
        reader.reserved(3)?;
        let binding = link_binding_from_ids(
            ids,
            ordinal,
            disposition,
            reader.u64(),
            reader.u64(),
            reader.u64(),
        );
        reader.reserved(8)?;
        let transition_sequence = reader.u64();
        let market_admission_sequence = reader.u64();
        let market_admission_receipt_id = reader.id();
        let rent_principal_lamports = reader.u64();
        let donation_floor_lamports = reader.u64();
        let current_donation_lamports = reader.u64();
        let mut obligation_statuses =
            [SeriesLinkObligationStatusV2::CapabilityDisabled; SERIES_LINK_OBLIGATION_COUNT_V2];
        for status in &mut obligation_statuses {
            *status = SeriesLinkObligationStatusV2::decode(reader.u8())?;
        }
        reader.reserved(4)?;
        let mut admission_receipts = [ContentId::ZERO; SERIES_LINK_OBLIGATION_COUNT_V2];
        for receipt in &mut admission_receipts {
            *receipt = reader.id();
        }
        let mut terminal_receipts = [ContentId::ZERO; SERIES_LINK_OBLIGATION_COUNT_V2];
        for receipt in &mut terminal_receipts {
            *receipt = reader.id();
        }
        let active_failure_sessions = reader.u32();
        let failure_sessions_started = reader.u32();
        let failure_session_transcript_id = reader.id();
        reader.finish()?;
        output.binding = binding;
        output.phase = phase;
        output.transition_sequence = transition_sequence;
        output.market_admission_sequence = market_admission_sequence;
        output.market_admission_receipt_id = market_admission_receipt_id;
        output.rent_principal_lamports = rent_principal_lamports;
        output.donation_floor_lamports = donation_floor_lamports;
        output.current_donation_lamports = current_donation_lamports;
        output.obligation_statuses = obligation_statuses;
        output.admission_receipts = admission_receipts;
        output.terminal_receipts = terminal_receipts;
        output.active_failure_sessions = active_failure_sessions;
        output.failure_sessions_started = failure_sessions_started;
        output.failure_session_transcript_id = failure_session_transcript_id;
        output.validate()
    }
}

fn binding_from_ids(
    ids: [ContentId; MARKET_BINDING_ID_COUNT_V2],
    generation: u64,
    outcome_count: u8,
    maximum_series_links: u32,
    founding_deadline_bucket: u64,
    maximum_interval_width: u64,
    maximum_coordinates_per_advance: u16,
) -> MarketLifecycleBindingV2 {
    MarketLifecycleBindingV2 {
        market_instance_id: MarketInstanceV2Id::from_bytes(ids[0].bytes()),
        generation,
        outcome_count,
        maximum_series_links,
        product_template_id: ids[1],
        native_claim_basis_id: ids[2],
        recovery_policy_id: ids[3],
        price_measure_policy_id: ids[4],
        market_genesis_profile_id: ids[5],
        registry_release_id: ids[6],
        capability_profile_id: ids[7],
        realm_id: ids[8],
        collateral_profile_id: ids[9],
        collateral_policy_id: ids[10],
        collateral_release_id: ids[11],
        source_release_id: ids[12],
        source_route_id: ids[13],
        clock_policy_id: ids[14],
        source_plane_contract_id: ids[15],
        source_spec_id: ids[16],
        primary_window_id: ids[17],
        statistic_key_id: ids[18],
        market_failure_policy_binding_id: ids[19],
        recovery_state_id: ids[20],
        interval_consensus_profile_id: ids[21],
        failure_liveness_policy_id: ids[22],
        failure_liveness_quote_schedule_id: ids[23],
        resolution_account_id: ids[24],
        foundation_vault_id: ids[25],
        foundation_account_graph_id: MarketFoundationAccountGraphV3Id::from_bytes(ids[26].bytes()),
        foundation_schedule_id: MarketFoundationScheduleV3Id::from_bytes(ids[27].bytes()),
        direct_global_liveness_binding_id: ids[28],
        market_liability_founding_id: ids[29],
        claim_mint_founding_plan_id: ids[30],
        claim_issuance_binding_id: ids[31],
        general_founding_capability_id: ids[32],
        founding_abort_policy_id: ids[33],
        founding_deadline_bucket,
        maximum_interval_width,
        maximum_coordinates_per_advance,
    }
}

fn link_binding_from_ids(
    ids: [ContentId; LINK_BINDING_ID_COUNT_V2],
    ordinal: u32,
    disposition: SeriesMarketDispositionV1,
    generation: u64,
    source_repair_generation: u64,
    funding_transition_sequence: u64,
) -> SeriesMarketLinkBindingV2 {
    SeriesMarketLinkBindingV2 {
        series_plan_id: SeriesPlanV5Id::from_bytes(ids[0].bytes()),
        ordinal,
        market_instance_id: MarketInstanceV2Id::from_bytes(ids[1].bytes()),
        market_root_account_id: ids[2],
        market_binding_id: ids[3],
        disposition,
        funding_terms_id: SeriesFundingTermsV2Id::from_bytes(ids[4].bytes()),
        funding_quote_id: SeriesFundingQuoteV5Id::from_bytes(ids[5].bytes()),
        attachment_plan_id: SeriesAttachmentPlanV5Id::from_bytes(ids[6].bytes()),
        capability_profile_id: ids[7],
        obligation_configuration_id: SeriesLinkObligationConfigurationV2Id::from_bytes(
            ids[8].bytes(),
        ),
        compiler_bundle_id: CompiledProductSeriesBundleV6Id::from_bytes(ids[9].bytes()),
        source_occurrence_id: SourceOccurrenceV1Id::from_bytes(ids[10].bytes()),
        source_occurrence_account_id: ids[11],
        source_occurrence_account_authentication_id: ids[12],
        source_occurrence_receipt_id: ids[13],
        source_release_id: ids[14],
        source_route_id: ids[15],
        clock_policy_id: ids[16],
        source_plane_contract_id: ids[17],
        source_spec_id: ids[18],
        window_spec_id: ids[19],
        statistic_key_id: ids[20],
        funding_state_account_id: ids[21],
        funding_debit_receipt_id: ids[22],
        rent_refund_owner: ids[23],
        neutral_lamport_sink: ids[24],
        generation,
        source_repair_generation,
        funding_transition_sequence,
    }
}

fn read_ids<const N: usize>(reader: &mut Reader<'_>) -> [ContentId; N] {
    let mut ids = [ContentId::ZERO; N];
    for id in &mut ids {
        *id = reader.id();
    }
    ids
}

fn expected_foundation_bitmap(outcome_count: u8) -> Result<u64> {
    let outcomes = usize::from(outcome_count);
    if outcomes == 0 || outcomes > MARKET_FOUNDATION_MAX_OUTCOMES_V3 {
        return Err(Error::InvalidParameter);
    }
    let mut bitmap = 0u64;
    let mut index = 0usize;
    while index < MARKET_FOUNDATION_CORE_SLOT_COUNT_V3 {
        bitmap |= slot_bit(index)?;
        index += 1;
    }
    index = 0;
    while index < outcomes {
        bitmap |= slot_bit(
            MARKET_FOUNDATION_CORE_SLOT_COUNT_V3
                .checked_add(index)
                .ok_or(Error::ArithmeticOverflow)?,
        )?;
        bitmap |= slot_bit(
            MARKET_FOUNDATION_CORE_SLOT_COUNT_V3
                .checked_add(MARKET_FOUNDATION_MAX_OUTCOMES_V3)
                .and_then(|base| base.checked_add(index))
                .ok_or(Error::ArithmeticOverflow)?,
        )?;
        index += 1;
    }
    Ok(bitmap)
}

fn slot_bit(index: usize) -> Result<u64> {
    if index >= MARKET_FOUNDATION_SLOT_COUNT_V3 {
        return Err(Error::InvalidParameter);
    }
    let shift = u32::try_from(index).map_err(|_| Error::InvalidParameter)?;
    1u64.checked_shl(shift).ok_or(Error::ArithmeticOverflow)
}

fn highest_set_index(bitmap: u64) -> Option<usize> {
    let mut index = MARKET_FOUNDATION_SLOT_COUNT_V3;
    while index > 0 {
        index -= 1;
        if let Ok(bit) = slot_bit(index) {
            if (bitmap & bit) != 0 {
                return Some(index);
            }
        }
    }
    None
}

fn require_pairwise_distinct<const N: usize>(ids: &[ContentId; N]) -> Result<()> {
    let mut left = 0usize;
    while left < N {
        let mut right = left + 1;
        while right < N {
            if ids[left] == ids[right] {
                return Err(Error::MismatchedArtifact);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn rolling_id(domain: &[u8], previous: ContentId, receipt: ContentId, sequence: u64) -> ContentId {
    let mut body = [0u8; 72];
    body[..32].copy_from_slice(&previous.bytes());
    body[32..64].copy_from_slice(&receipt.bytes());
    body[64..72].copy_from_slice(&sequence.to_le_bytes());
    content_id(domain, &body)
}

impl SeriesMarketLinkV2 {
    /// Admit one exact Series-scoped obligation child once.
    pub fn admit_obligation(
        self,
        projection: SeriesLinkObligationAdmissionProjectionV2,
    ) -> Result<Self> {
        self.validate()?;
        if self.phase != SeriesMarketLinkPhaseV2::Active
            || projection.link_semantic_id != self.semantic_id()?
            || projection.link_transition_sequence
                != self
                    .transition_sequence
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        let index = projection.obligation.index();
        if self.obligation_statuses[index] != SeriesLinkObligationStatusV2::EnabledNeverFounded
            || self.admission_receipts[index] != ContentId::ZERO
        {
            return Err(Error::WorkStateMismatch);
        }
        let receipt = projection.id()?;
        if self
            .admission_receipts
            .iter()
            .chain(self.terminal_receipts.iter())
            .any(|seen| *seen == receipt)
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        let mut admission_receipts = self.admission_receipts;
        admission_receipts[index] = receipt;
        let mut obligation_statuses = self.obligation_statuses;
        obligation_statuses[index] = SeriesLinkObligationStatusV2::Live;
        let next = Self {
            transition_sequence: projection.link_transition_sequence,
            obligation_statuses,
            admission_receipts,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Pin the initiating Source occurrence while one Failure interval session is live.
    pub fn pin_failure_session(self, failure_begin_receipt_id: ContentId) -> Result<Self> {
        self.validate()?;
        failure_begin_receipt_id.validate()?;
        if self.phase != SeriesMarketLinkPhaseV2::Active || self.active_failure_sessions != 0 {
            return Err(Error::WorkStateMismatch);
        }
        let sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let transcript = rolling_id(
            b"dragons-clutch/series-link-failure-session/v2",
            self.failure_session_transcript_id,
            failure_begin_receipt_id,
            sequence,
        );
        let next = Self {
            transition_sequence: sequence,
            active_failure_sessions: self
                .active_failure_sessions
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            failure_sessions_started: self
                .failure_sessions_started
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            failure_session_transcript_id: transcript,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Release one exact terminal Failure session; this does not discharge Market Failure.
    pub fn release_failure_session(self, failure_terminal_receipt_id: ContentId) -> Result<Self> {
        self.validate()?;
        failure_terminal_receipt_id.validate()?;
        if self.phase != SeriesMarketLinkPhaseV2::Active || self.active_failure_sessions != 1 {
            return Err(Error::WorkStateMismatch);
        }
        let sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let transcript = rolling_id(
            b"dragons-clutch/series-link-failure-session/v2",
            self.failure_session_transcript_id,
            failure_terminal_receipt_id,
            sequence,
        );
        let next = Self {
            transition_sequence: sequence,
            active_failure_sessions: self
                .active_failure_sessions
                .checked_sub(1)
                .ok_or(Error::ArithmeticOverflow)?,
            failure_session_transcript_id: transcript,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Consume one exact Series-scoped terminal/absence receipt once.
    pub fn consume_obligation(
        self,
        projection: SeriesLinkObligationTerminalProjectionV2,
    ) -> Result<Self> {
        self.validate()?;
        if self.phase != SeriesMarketLinkPhaseV2::Active
            || projection.link_semantic_id != self.semantic_id()?
            || projection.link_transition_sequence
                != self
                    .transition_sequence
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        let index = projection.obligation.index();
        let expected_disposition = match self.obligation_statuses[index] {
            SeriesLinkObligationStatusV2::CapabilityDisabled
            | SeriesLinkObligationStatusV2::EnabledNeverFounded => {
                SeriesLinkObligationDispositionV2::Absent
            }
            SeriesLinkObligationStatusV2::Live => SeriesLinkObligationDispositionV2::Terminal,
            SeriesLinkObligationStatusV2::Terminal => return Err(Error::WorkStateMismatch),
        };
        if projection.disposition != expected_disposition {
            return Err(Error::WorkStateMismatch);
        }
        let receipt = projection.id()?;
        let mut terminal_receipts = self.terminal_receipts;
        if self
            .admission_receipts
            .iter()
            .chain(terminal_receipts.iter())
            .any(|seen| *seen == receipt)
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        terminal_receipts[index] = receipt;
        let mut obligation_statuses = self.obligation_statuses;
        obligation_statuses[index] = SeriesLinkObligationStatusV2::Terminal;
        let next = Self {
            transition_sequence: projection.link_transition_sequence,
            obligation_statuses,
            terminal_receipts,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Enter link retirement only after every obligation and Failure pin closes.
    pub fn begin_retirement(self) -> Result<Self> {
        let mut output = Self::decode_buffer();
        self.begin_retirement_into(&mut output)?;
        Ok(output)
    }

    /// Enter link retirement into caller-owned storage.
    pub fn begin_retirement_into(&self, output: &mut Self) -> Result<()> {
        self.validate()?;
        if self.phase != SeriesMarketLinkPhaseV2::Active
            || self.active_failure_sessions != 0
            || self
                .obligation_statuses
                .iter()
                .any(|status| *status != SeriesLinkObligationStatusV2::Terminal)
        {
            return Err(Error::WorkIncomplete);
        }
        *output = Self {
            phase: SeriesMarketLinkPhaseV2::Retiring,
            transition_sequence: self
                .transition_sequence
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            ..*self
        };
        output.validate()
    }

    /// Emit the exact link retirement projection consumed by the shared root.
    pub fn retirement_projection(self) -> Result<SeriesMarketLinkRetirementProjectionV2> {
        self.validate()?;
        if self.phase != SeriesMarketLinkPhaseV2::Retiring {
            return Err(Error::WorkStateMismatch);
        }
        let semantic_id = self.semantic_id()?;
        let mut body = [0u8; 148];
        body[..32].copy_from_slice(&semantic_id.bytes());
        body[32..64].copy_from_slice(&self.binding.market_instance_id.bytes());
        body[64..96].copy_from_slice(&self.binding.series_plan_id.bytes());
        body[96..100].copy_from_slice(&self.binding.ordinal.to_le_bytes());
        body[100..108].copy_from_slice(&self.binding.generation.to_le_bytes());
        body[108..116].copy_from_slice(&self.transition_sequence.to_le_bytes());
        body[116..148].copy_from_slice(&self.market_admission_receipt_id.bytes());
        let id = content_id(b"dragons-clutch/series-market-link-retirement/v2", &body);
        Ok(SeriesMarketLinkRetirementProjectionV2 {
            id,
            link_semantic_id: semantic_id,
            market_instance_id: self.binding.market_instance_id,
            series_plan_id: self.binding.series_plan_id,
            ordinal: self.binding.ordinal,
            generation: self.binding.generation,
            transition_sequence: self.transition_sequence,
            market_admission_receipt_id: self.market_admission_receipt_id,
        })
    }

    /// Mark the link retired only in the same atomic instruction as root consumption.
    pub fn mark_retired(self, projection: SeriesMarketLinkRetirementProjectionV2) -> Result<Self> {
        let mut output = Self::decode_buffer();
        self.mark_retired_into(projection, &mut output)?;
        Ok(output)
    }

    /// Mark the link retired into caller-owned storage.
    pub fn mark_retired_into(
        &self,
        projection: SeriesMarketLinkRetirementProjectionV2,
        output: &mut Self,
    ) -> Result<()> {
        self.validate()?;
        if self.phase != SeriesMarketLinkPhaseV2::Retiring
            || projection.link_semantic_id != self.semantic_id()?
            || projection.market_admission_receipt_id != self.market_admission_receipt_id
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        *output = Self {
            phase: SeriesMarketLinkPhaseV2::Retired,
            transition_sequence: self
                .transition_sequence
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            ..*self
        };
        output.validate()
    }

    /// Mark a never-activated link aborted after exact shared-root abort authorization.
    pub fn mark_aborted(self, market_abort_receipt_id: ContentId) -> Result<Self> {
        self.validate()?;
        market_abort_receipt_id.validate()?;
        if self.phase != SeriesMarketLinkPhaseV2::PendingMarket
            || self.market_admission_receipt_id != ContentId::ZERO
        {
            return Err(Error::WorkStateMismatch);
        }
        let next = Self {
            phase: SeriesMarketLinkPhaseV2::Aborted,
            transition_sequence: 1,
            market_admission_receipt_id: market_abort_receipt_id,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Immutable link binding.
    pub const fn binding(self) -> SeriesMarketLinkBindingV2 {
        self.binding
    }
    /// Link phase.
    pub const fn phase(self) -> SeriesMarketLinkPhaseV2 {
        self.phase
    }
    /// Active Failure sessions pinning Source state.
    pub const fn active_failure_sessions(self) -> u32 {
        self.active_failure_sessions
    }
    /// Monotone number of Failure sessions ever pinned to this link.
    pub const fn failure_sessions_started(self) -> u32 {
        self.failure_sessions_started
    }
    /// Persistent transcript proving whether any Failure session was ever pinned.
    pub const fn failure_session_transcript_id(self) -> ContentId {
        self.failure_session_transcript_id
    }
    /// Exhaustive current state of one attachment obligation.
    pub const fn obligation_status(
        self,
        obligation: SeriesLinkObligationV2,
    ) -> SeriesLinkObligationStatusV2 {
        self.obligation_statuses[obligation.index()]
    }
    /// Exact owner admission transcript for one live/terminal obligation.
    pub const fn obligation_admission_receipt_id(
        self,
        obligation: SeriesLinkObligationV2,
    ) -> ContentId {
        self.admission_receipts[obligation.index()]
    }
    /// Exact Product terminal projection receipt for a consumed obligation.
    pub const fn obligation_terminal_receipt_id(
        self,
        obligation: SeriesLinkObligationV2,
    ) -> ContentId {
        self.terminal_receipts[obligation.index()]
    }
    /// Link transition sequence.
    pub const fn transition_sequence(self) -> u64 {
        self.transition_sequence
    }
    /// Exact root admission sequence.
    pub const fn market_admission_sequence(self) -> u64 {
        self.market_admission_sequence
    }
    /// Exact root admission/abort receipt.
    pub const fn market_admission_receipt_id(self) -> ContentId {
        self.market_admission_receipt_id
    }
    /// Refundable link rent principal.
    pub const fn rent_principal_lamports(self) -> u64 {
        self.rent_principal_lamports
    }
    /// Current donation residue.
    pub const fn current_donation_lamports(self) -> u64 {
        self.current_donation_lamports
    }

    /// Semantic state identity.
    pub fn semantic_id(self) -> Result<SeriesMarketLinkV2Id> {
        let mut body = [0u8; SERIES_MARKET_LINK_BYTES_V2];
        self.encode_into(&mut body)?;
        Ok(SeriesMarketLinkV2Id::from_bytes(
            content_id(SERIES_MARKET_LINK_DOMAIN_V2, &body).bytes(),
        ))
    }

    fn validate(self) -> Result<()> {
        self.binding.validate()?;
        if self.rent_principal_lamports == 0
            || self.current_donation_lamports < self.donation_floor_lamports
        {
            return Err(Error::InvalidParameter);
        }
        let admitted = self.market_admission_receipt_id != ContentId::ZERO;
        match self.phase {
            SeriesMarketLinkPhaseV2::PendingMarket => {
                if self.transition_sequence != 0 || admitted || self.market_admission_sequence != 0
                {
                    return Err(Error::WorkStateMismatch);
                }
            }
            SeriesMarketLinkPhaseV2::Active | SeriesMarketLinkPhaseV2::Retiring => {
                if self.transition_sequence == 0 || !admitted || self.market_admission_sequence == 0
                {
                    return Err(Error::WorkStateMismatch);
                }
            }
            SeriesMarketLinkPhaseV2::Retired => {
                if self.transition_sequence < 2 || !admitted || self.active_failure_sessions != 0 {
                    return Err(Error::WorkStateMismatch);
                }
            }
            SeriesMarketLinkPhaseV2::Aborted => {
                if self.transition_sequence != 1
                    || !admitted
                    || self.market_admission_sequence != 0
                    || self.active_failure_sessions != 0
                    || self
                        .obligation_statuses
                        .iter()
                        .any(|status| *status == SeriesLinkObligationStatusV2::Terminal)
                {
                    return Err(Error::WorkStateMismatch);
                }
            }
        }
        let mut index = 0usize;
        while index < SERIES_LINK_OBLIGATION_COUNT_V2 {
            let admission_present = self.admission_receipts[index] != ContentId::ZERO;
            let terminal_present = self.terminal_receipts[index] != ContentId::ZERO;
            let receipts_match = match self.obligation_statuses[index] {
                SeriesLinkObligationStatusV2::CapabilityDisabled
                | SeriesLinkObligationStatusV2::EnabledNeverFounded => {
                    !admission_present && !terminal_present
                }
                SeriesLinkObligationStatusV2::Live => admission_present && !terminal_present,
                SeriesLinkObligationStatusV2::Terminal => terminal_present,
            };
            if !receipts_match {
                return Err(Error::WorkStateMismatch);
            }
            index += 1;
        }
        if matches!(
            self.phase,
            SeriesMarketLinkPhaseV2::Retiring | SeriesMarketLinkPhaseV2::Retired
        ) && self
            .obligation_statuses
            .iter()
            .any(|status| *status != SeriesLinkObligationStatusV2::Terminal)
        {
            return Err(Error::WorkStateMismatch);
        }
        if self.active_failure_sessions > 1
            || self.active_failure_sessions > self.failure_sessions_started
            || (self.failure_sessions_started == 0)
                != (self.failure_session_transcript_id == ContentId::ZERO)
        {
            return Err(Error::WorkStateMismatch);
        }
        Ok(())
    }
}

/// Exact link retirement receipt consumed once by the shared root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesMarketLinkRetirementProjectionV2 {
    id: ContentId,
    link_semantic_id: SeriesMarketLinkV2Id,
    market_instance_id: MarketInstanceV2Id,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    generation: u64,
    transition_sequence: u64,
    market_admission_receipt_id: ContentId,
}

impl SeriesMarketLinkRetirementProjectionV2 {
    /// Projection identity.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Exact retiring link state.
    pub const fn link_semantic_id(self) -> SeriesMarketLinkV2Id {
        self.link_semantic_id
    }
    /// Shared Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    /// Series.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }
    /// Ordinal.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
    /// Generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Link transition sequence.
    pub const fn transition_sequence(self) -> u64 {
        self.transition_sequence
    }
    /// Exact original root admission receipt.
    pub const fn market_admission_receipt_id(self) -> ContentId {
        self.market_admission_receipt_id
    }
}

/// Root-side one-shot Series-link admission projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesMarketAdmissionProjectionV2 {
    id: ContentId,
    market_binding_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    link_semantic_id: SeriesMarketLinkV2Id,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    disposition: SeriesMarketDispositionV1,
    admission_sequence: u64,
}

impl SeriesMarketAdmissionProjectionV2 {
    /// Construct the deterministic root/link admission join.
    pub fn new(
        market_binding: MarketLifecycleBindingV2,
        link: SeriesMarketLinkV2,
        admission_sequence: u64,
    ) -> Result<Self> {
        market_binding.validate()?;
        link.validate()?;
        if link.phase != SeriesMarketLinkPhaseV2::PendingMarket
            || link.binding.market_instance_id != market_binding.market_instance_id
            || link.binding.market_binding_id != market_binding.id()?
            || link.binding.capability_profile_id != market_binding.capability_profile_id
            || link.binding.generation != market_binding.generation
            || admission_sequence == 0
        {
            return Err(Error::MismatchedArtifact);
        }
        let link_semantic_id = link.semantic_id()?;
        let market_binding_id = market_binding.id()?;
        let mut body = [0u8; 181];
        body[..32].copy_from_slice(&market_binding_id.bytes());
        body[32..64].copy_from_slice(&market_binding.market_instance_id.bytes());
        body[64..96].copy_from_slice(&link_semantic_id.bytes());
        body[96..128].copy_from_slice(&link.binding.series_plan_id.bytes());
        body[128..132].copy_from_slice(&link.binding.ordinal.to_le_bytes());
        body[132] = match link.binding.disposition {
            SeriesMarketDispositionV1::Founder => 1,
            SeriesMarketDispositionV1::Converger => 2,
        };
        body[133..141].copy_from_slice(&market_binding.generation.to_le_bytes());
        body[141..149].copy_from_slice(&admission_sequence.to_le_bytes());
        body[149..181].copy_from_slice(&link.binding.funding_debit_receipt_id.bytes());
        let id = content_id(b"dragons-clutch/series-market-admission/v2", &body);
        Ok(Self {
            id,
            market_binding_id,
            market_instance_id: market_binding.market_instance_id,
            link_semantic_id,
            series_plan_id: link.binding.series_plan_id,
            ordinal: link.binding.ordinal,
            disposition: link.binding.disposition,
            admission_sequence,
        })
    }

    /// Admission receipt ID.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Shared binding.
    pub const fn market_binding_id(self) -> ContentId {
        self.market_binding_id
    }
    /// Shared Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    /// Exact pending link state.
    pub const fn link_semantic_id(self) -> SeriesMarketLinkV2Id {
        self.link_semantic_id
    }
    /// Series.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }
    /// Ordinal.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
    /// Founder/converger.
    pub const fn disposition(self) -> SeriesMarketDispositionV1 {
        self.disposition
    }
    /// Monotone root admission sequence.
    pub const fn admission_sequence(self) -> u64 {
        self.admission_sequence
    }
}

/// Once-only Resolution V5 postimage authorized by Product + private Failure receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketResolutionActivationV2 {
    id: ContentId,
    market_binding_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    resolution_account_id: ContentId,
    resolution_semantic_id: ContentId,
    resolution_data_id: ContentId,
    failure_resolution_receipt_id: ContentId,
    product_certificate_id: ContentId,
    composite_finalization_evidence_id: ContentId,
}

impl MarketResolutionActivationV2 {
    /// Construct the deterministic postimage; live authority remains adapter-private.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        binding: MarketLifecycleBindingV2,
        resolution_semantic_id: ContentId,
        resolution_data_id: ContentId,
        failure_resolution_receipt_id: ContentId,
        product_certificate_id: ContentId,
        composite_finalization_evidence_id: ContentId,
    ) -> Result<Self> {
        binding.validate()?;
        let ids = [
            resolution_semantic_id,
            resolution_data_id,
            failure_resolution_receipt_id,
            product_certificate_id,
            composite_finalization_evidence_id,
        ];
        for id in ids {
            id.validate()?;
        }
        require_pairwise_distinct(&ids)?;
        let market_binding_id = binding.id()?;
        let mut body = [0u8; 296];
        let all_ids = [
            market_binding_id,
            binding.market_instance_id.content_id(),
            binding.native_claim_basis_id,
            binding.resolution_account_id,
            resolution_semantic_id,
            resolution_data_id,
            failure_resolution_receipt_id,
            product_certificate_id,
            composite_finalization_evidence_id,
        ];
        let mut at = 0usize;
        for id in all_ids {
            body[at..at + 32].copy_from_slice(&id.bytes());
            at += 32;
        }
        body[288..296].copy_from_slice(&binding.generation.to_le_bytes());
        let id = content_id(MARKET_RESOLUTION_ACTIVATION_DOMAIN_V2, &body);
        Ok(Self {
            id,
            market_binding_id,
            market_instance_id: binding.market_instance_id,
            generation: binding.generation,
            resolution_account_id: binding.resolution_account_id,
            resolution_semantic_id,
            resolution_data_id,
            failure_resolution_receipt_id,
            product_certificate_id,
            composite_finalization_evidence_id,
        })
    }

    /// Activation ID.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Binding ID.
    pub const fn market_binding_id(self) -> ContentId {
        self.market_binding_id
    }
    /// Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    /// Generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Resolution account.
    pub const fn resolution_account_id(self) -> ContentId {
        self.resolution_account_id
    }
    /// Resolution semantic state.
    pub const fn resolution_semantic_id(self) -> ContentId {
        self.resolution_semantic_id
    }
    /// Exact account-data digest.
    pub const fn resolution_data_id(self) -> ContentId {
        self.resolution_data_id
    }
    /// Private Failure receipt.
    pub const fn failure_resolution_receipt_id(self) -> ContentId {
        self.failure_resolution_receipt_id
    }
    /// Product certificate.
    pub const fn product_certificate_id(self) -> ContentId {
        self.product_certificate_id
    }
    /// Composite finalization evidence written into V5.
    pub const fn composite_finalization_evidence_id(self) -> ContentId {
        self.composite_finalization_evidence_id
    }
}

/// Exact inert-foundation abort output used to restore the pending Series ordinal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFoundingAbortProjectionV2 {
    id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    founder_link_id: SeriesMarketLinkV2Id,
    refund_owner: ContentId,
    neutral_lamport_sink: ContentId,
    refundable_principal_lamports: u64,
    donation_lamports: u64,
    final_transition_sequence: u64,
}

impl MarketFoundingAbortProjectionV2 {
    /// Abort receipt.
    pub const fn id(self) -> ContentId {
        self.id
    }
    /// Market.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    /// Generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Founding link whose pending reservation may be restored.
    pub const fn founder_link_id(self) -> SeriesMarketLinkV2Id {
        self.founder_link_id
    }
    /// Exact principal owner.
    pub const fn refund_owner(self) -> ContentId {
        self.refund_owner
    }
    /// Donation sink.
    pub const fn neutral_lamport_sink(self) -> ContentId {
        self.neutral_lamport_sink
    }
    /// Total exact refundable shared principal.
    pub const fn refundable_principal_lamports(self) -> u64 {
        self.refundable_principal_lamports
    }
    /// Donation residue.
    pub const fn donation_lamports(self) -> u64 {
        self.donation_lamports
    }
    /// Final root sequence.
    pub const fn final_transition_sequence(self) -> u64 {
        self.final_transition_sequence
    }
}

/// Shared `0xaa/1` lifecycle owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketLifecycleRootV2 {
    binding: MarketLifecycleBindingV2,
    phase: MarketLifecyclePhaseV2,
    transition_sequence: u64,
    capital: MarketFoundationCapitalV2,
    foundation: MarketFoundationProgressV2,
    admitted_series_links: u32,
    live_series_links: u32,
    retired_series_links: u32,
    series_link_transcript_id: ContentId,
    product_families: MarketFamilyAggregatorV1,
    shared_core_terminal_receipts: [ContentId; MARKET_SHARED_CORE_COUNT_V2],
    fractional_terminal_state_ids: [ContentId; 2],
    resolution_semantic_id: ContentId,
    resolution_data_id: ContentId,
    resolution_activation_receipt_id: ContentId,
}

impl MarketLifecycleRootV2 {
    /// Create an inert root and consume only the root-account slot principal.
    pub fn initialize_founder(
        binding: MarketLifecycleBindingV2,
        schedule: &MarketFoundationScheduleV3,
        account_graph: &MarketFoundationAccountGraphV3,
        mut capital: MarketFoundationCapitalV2,
        product_families: &MarketFamilyAggregatorV1,
        root_poststate_receipt_id: ContentId,
    ) -> Result<Self> {
        binding.validate()?;
        schedule.validate()?;
        account_graph.validate(schedule)?;
        product_families.validate()?;
        root_poststate_receipt_id.validate()?;
        if binding.foundation_schedule_id != schedule.id()?
            || binding.foundation_account_graph_id != account_graph.id(schedule)?
            || binding.market_instance_id != account_graph.market_instance_id
            || binding.generation != account_graph.generation
            || binding.outcome_count != schedule.outcome_count
            || product_families.phase() != MarketFamilyAggregatorPhaseV1::Open
            || product_families.binding().market_instance_id != binding.market_instance_id
            || product_families.binding().generation != binding.generation
            || product_families.binding().registry_release_id.content_id()
                != binding.registry_release_id
            || product_families
                .binding()
                .capability_profile_id
                .content_id()
                != binding.capability_profile_id
        {
            return Err(Error::MismatchedArtifact);
        }
        capital.validate(schedule)?;
        let root_index = MarketFoundationSlotV3::LifecycleRoot.index()?;
        let root_principal = schedule.slot_principal_lamports[root_index];
        capital.principal_remaining_lamports = capital
            .principal_total_lamports
            .checked_sub(root_principal)
            .ok_or(Error::InsufficientPrepayment)?;
        let expected_bitmap = expected_foundation_bitmap(schedule.outcome_count)?;
        let root_bit = slot_bit(root_index)?;
        let foundation = MarketFoundationProgressV2 {
            expected_bitmap,
            initialized_bitmap: root_bit,
            abort_closed_bitmap: 0,
            sequence: 1,
            transcript_id: rolling_id(
                b"dragons-clutch/market-foundation-transcript/v2",
                ContentId::ZERO,
                root_poststate_receipt_id,
                1,
            ),
        };
        let value = Self {
            binding,
            phase: MarketLifecyclePhaseV2::Founding,
            transition_sequence: 1,
            capital,
            foundation,
            admitted_series_links: 0,
            live_series_links: 0,
            retired_series_links: 0,
            series_link_transcript_id: ContentId::ZERO,
            product_families: *product_families,
            shared_core_terminal_receipts: [ContentId::ZERO; MARKET_SHARED_CORE_COUNT_V2],
            fractional_terminal_state_ids: [ContentId::ZERO; 2],
            resolution_semantic_id: ContentId::ZERO,
            resolution_data_id: ContentId::ZERO,
            resolution_activation_receipt_id: ContentId::ZERO,
        };
        value.validate()?;
        Ok(value)
    }

    /// Spend one itemized slot from FoundationVault and count its accepted postwrite.
    pub fn record_foundation_step(
        self,
        schedule: &MarketFoundationScheduleV3,
        account_graph: &MarketFoundationAccountGraphV3,
        step: MarketFoundationStepProjectionV3,
    ) -> Result<Self> {
        let mut output = Self::decode_buffer();
        self.record_foundation_step_into(schedule, account_graph, step, &mut output)?;
        Ok(output)
    }

    /// Frame-bounded foundation-step transition into caller-owned storage.
    pub fn record_foundation_step_into(
        &self,
        schedule: &MarketFoundationScheduleV3,
        account_graph: &MarketFoundationAccountGraphV3,
        step: MarketFoundationStepProjectionV3,
        output: &mut Self,
    ) -> Result<()> {
        self.validate_against_schedule(schedule)?;
        account_graph.validate(schedule)?;
        if self.phase != MarketLifecyclePhaseV2::Founding
            || account_graph.id(schedule)? != self.binding.foundation_account_graph_id
            || step.binding_id != self.binding.id()?
            || step.root_transition_sequence
                != self
                    .transition_sequence
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        let index = step.slot.index()?;
        if step.account_id != account_graph.account_ids[index] {
            return Err(Error::MismatchedArtifact);
        }
        let bit = slot_bit(index)?;
        if (self.foundation.expected_bitmap & bit) == 0
            || (self.foundation.initialized_bitmap & bit) != 0
            || step.principal_lamports != schedule.slot_principal_lamports[index]
            || step.principal_before_lamports != self.capital.principal_remaining_lamports
            || step.principal_after_lamports
                != step
                    .principal_before_lamports
                    .checked_sub(step.principal_lamports)
                    .ok_or(Error::InsufficientPrepayment)?
            || step.donation_before_lamports < self.capital.vault_current_donation_lamports
            || step.donation_after_lamports != step.donation_before_lamports
        {
            return Err(Error::InvalidComponentStatus);
        }
        let step_id = step.id()?;
        let mut foundation = self.foundation;
        foundation.initialized_bitmap |= bit;
        foundation.sequence = foundation
            .sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        foundation.transcript_id = rolling_id(
            b"dragons-clutch/market-foundation-transcript/v2",
            foundation.transcript_id,
            step_id,
            u64::from(foundation.sequence),
        );
        let mut capital = self.capital;
        capital.principal_remaining_lamports = step.principal_after_lamports;
        capital.vault_current_donation_lamports = step.donation_after_lamports;
        *output = *self;
        output.transition_sequence = step.root_transition_sequence;
        output.capital = capital;
        output.foundation = foundation;
        output.validate_against_schedule(schedule)
    }

    /// Admit one pending `0xad` exactly once while Founding or Active.
    pub fn admit_series_link(self, admission: SeriesMarketAdmissionProjectionV2) -> Result<Self> {
        self.validate()?;
        if !matches!(
            self.phase,
            MarketLifecyclePhaseV2::Founding | MarketLifecyclePhaseV2::Active
        ) || admission.market_binding_id != self.binding.id()?
            || admission.market_instance_id != self.binding.market_instance_id
            || admission.admission_sequence
                != u64::from(self.admitted_series_links)
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?
            || self.admitted_series_links >= self.binding.maximum_series_links
            || (self.admitted_series_links == 0
                && admission.disposition != SeriesMarketDispositionV1::Founder)
            || (self.admitted_series_links != 0
                && admission.disposition != SeriesMarketDispositionV1::Converger)
            || (self.phase == MarketLifecyclePhaseV2::Founding
                && admission.disposition != SeriesMarketDispositionV1::Founder)
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        let next_count = self
            .admitted_series_links
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let next = Self {
            transition_sequence: sequence,
            admitted_series_links: next_count,
            live_series_links: self
                .live_series_links
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            series_link_transcript_id: rolling_id(
                b"dragons-clutch/market-series-link-transcript/v2",
                self.series_link_transcript_id,
                admission.id,
                sequence,
            ),
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Delegate one authenticated product-family child admission to the embedded owner.
    pub fn admit_product_family_child<A: AuthenticatedMarketFamilyAuthorityV1 + ?Sized>(
        self,
        authority: &A,
        family: MarketFamilyV1,
        family_admission_sequence: u32,
        admission_receipt_id: ContentId,
    ) -> Result<Self> {
        let mut output = Self::decode_buffer();
        self.admit_product_family_child_into(
            authority,
            family,
            family_admission_sequence,
            admission_receipt_id,
            &mut output,
        )?;
        Ok(output)
    }

    /// Frame-bounded family admission into caller-owned RootV2 storage.
    pub fn admit_product_family_child_into<
        A: AuthenticatedMarketFamilyAuthorityV1 + ?Sized,
    >(
        &self,
        authority: &A,
        family: MarketFamilyV1,
        family_admission_sequence: u32,
        admission_receipt_id: ContentId,
        output: &mut Self,
    ) -> Result<()> {
        self.validate()?;
        if !matches!(
            self.phase,
            MarketLifecyclePhaseV2::Founding | MarketLifecyclePhaseV2::Active
        ) {
            return Err(Error::WorkStateMismatch);
        }
        let product_families = self.product_families.admit_child(
            authority,
            family,
            family_admission_sequence,
            admission_receipt_id,
        )?;
        *output = *self;
        output.transition_sequence = self
                .transition_sequence
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
        output.product_families = product_families;
        output.validate()
    }

    /// Delegate one authenticated product-family child terminal transition.
    pub fn terminalize_product_family_child<A: AuthenticatedMarketFamilyAuthorityV1 + ?Sized>(
        self,
        authority: &A,
        family: MarketFamilyV1,
        family_terminal_sequence: u32,
        terminal_receipt_id: ContentId,
    ) -> Result<Self> {
        self.validate()?;
        if !matches!(
            self.phase,
            MarketLifecyclePhaseV2::Active | MarketLifecyclePhaseV2::Retiring
        ) || family == MarketFamilyV1::Fractional
        {
            return Err(Error::WorkStateMismatch);
        }
        let product_families = self.product_families.terminalize_child(
            authority,
            family,
            family_terminal_sequence,
            terminal_receipt_id,
        )?;
        let next = Self {
            transition_sequence: self
                .transition_sequence
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            product_families,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Terminalize the single market-scoped Fractional owner and retain exact a4/a5 states.
    pub fn terminalize_fractional_family<A: AuthenticatedMarketFamilyAuthorityV1 + ?Sized>(
        self,
        authority: &A,
        family_terminal_sequence: u32,
        terminal_receipt_id: ContentId,
        fractional_policy_terminal_state_id: ContentId,
        fractional_ledger_terminal_state_id: ContentId,
    ) -> Result<Self> {
        let mut output = Self::decode_buffer();
        self.terminalize_fractional_family_into(
            authority,
            family_terminal_sequence,
            terminal_receipt_id,
            fractional_policy_terminal_state_id,
            fractional_ledger_terminal_state_id,
            &mut output,
        )?;
        Ok(output)
    }

    /// Frame-bounded Fractional terminal transition into caller-owned RootV2 storage.
    pub fn terminalize_fractional_family_into<
        A: AuthenticatedMarketFamilyAuthorityV1 + ?Sized,
    >(
        &self,
        authority: &A,
        family_terminal_sequence: u32,
        terminal_receipt_id: ContentId,
        fractional_policy_terminal_state_id: ContentId,
        fractional_ledger_terminal_state_id: ContentId,
        output: &mut Self,
    ) -> Result<()> {
        self.validate()?;
        if !matches!(
            self.phase,
            MarketLifecyclePhaseV2::Active | MarketLifecyclePhaseV2::Retiring
        ) || self.fractional_terminal_state_ids != [ContentId::ZERO; 2]
            || fractional_policy_terminal_state_id == fractional_ledger_terminal_state_id
        {
            return Err(Error::WorkStateMismatch);
        }
        fractional_policy_terminal_state_id.validate()?;
        fractional_ledger_terminal_state_id.validate()?;
        let fractional = self.product_families.family(MarketFamilyV1::Fractional);
        if fractional.counts().admitted != 1 || fractional.counts().live != 1 {
            return Err(Error::WorkStateMismatch);
        }
        let product_families = self.product_families.terminalize_child(
            authority,
            MarketFamilyV1::Fractional,
            family_terminal_sequence,
            terminal_receipt_id,
        )?;
        *output = *self;
        output.transition_sequence = self
                .transition_sequence
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
        output.product_families = product_families;
        output.fractional_terminal_state_ids = [
            fractional_policy_terminal_state_id,
            fractional_ledger_terminal_state_id,
        ];
        output.validate()
    }

    /// Activate trading only after every shared slot is accepted and founder link admitted.
    pub fn activate(
        self,
        schedule: &MarketFoundationScheduleV3,
        accepted_market_core_receipt_id: ContentId,
    ) -> Result<Self> {
        self.validate_against_schedule(schedule)?;
        accepted_market_core_receipt_id.validate()?;
        if self.phase != MarketLifecyclePhaseV2::Founding
            || !self.foundation.complete()
            || self.capital.principal_remaining_lamports != 0
            || self.admitted_series_links != 1
            || self.live_series_links != 1
            || !self.product_families.activation_ready()?
        {
            return Err(Error::WorkIncomplete);
        }
        let sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let foundation = MarketFoundationProgressV2 {
            transcript_id: rolling_id(
                b"dragons-clutch/market-foundation-activation/v2",
                self.foundation.transcript_id,
                accepted_market_core_receipt_id,
                sequence,
            ),
            ..self.foundation
        };
        let next = Self {
            phase: MarketLifecyclePhaseV2::Active,
            transition_sequence: sequence,
            foundation,
            ..self
        };
        next.validate_against_schedule(schedule)?;
        Ok(next)
    }
}
