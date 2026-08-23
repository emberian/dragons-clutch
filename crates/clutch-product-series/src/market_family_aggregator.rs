//! Canonical shared-Market family lifecycle aggregation.
//!
//! This module owns only Market-scoped family facts. Series admission plans,
//! occurrence identities, funding, and child-owned lifecycle facts remain in
//! their respective semantic owners. A Product/Market root may retain the
//! typed summary identities derived here; it must not copy their counts.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, ContentId, Error, FixedCodec, MarketInstanceV2Id,
    RegistryCapabilityProfileV2Id, RegistryProgramReleaseV1Id, Result,
};

const MARKET_FAMILY_AGGREGATOR_MAGIC_V1: [u8; 8] = *b"DCMFAGV1";
const MARKET_FAMILY_EXHAUSTIVE_SUMMARY_MAGIC_V1: [u8; 8] = *b"DCMFSUV1";
const MARKET_FAMILY_TERMINAL_PROJECTION_MAGIC_V1: [u8; 8] = *b"DCMFTPV1";
const MARKET_FAMILY_SCHEMA_V1: u16 = 1;

/// Semantic-ID domain for one canonical shared-Market family aggregator.
pub const MARKET_FAMILY_AGGREGATOR_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-family-aggregator/v1";
/// Semantic-ID domain for the immutable aggregator binding.
pub const MARKET_FAMILY_AGGREGATOR_BINDING_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-family-aggregator-binding/v1";
/// Semantic-ID domain for one authenticated family admission transition.
pub const MARKET_FAMILY_ADMISSION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-family-admission/v1";
/// Semantic-ID domain for one authenticated family terminal transition.
pub const MARKET_FAMILY_TERMINAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-family-terminal/v1";
/// Semantic-ID domain for one exhaustive family summary.
pub const MARKET_FAMILY_EXHAUSTIVE_SUMMARY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-family-exhaustive-summary/v1";
/// Semantic-ID domain for the final five-family projection.
pub const MARKET_FAMILY_TERMINAL_PROJECTION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-family-aggregator-terminal/v1";

/// Number of exhaustive Market family partitions.
pub const MARKET_FAMILY_COUNT_V1: usize = 5;
/// Exact persisted width of [`MarketFamilyAggregatorV1`].
pub const MARKET_FAMILY_AGGREGATOR_BYTES_V1: usize = 688;
/// Exact persisted width of [`MarketFamilyExhaustiveSummaryV1`].
pub const MARKET_FAMILY_EXHAUSTIVE_SUMMARY_BYTES_V1: usize = 232;
/// Exact persisted width of [`MarketFamilyAggregatorTerminalProjectionV1`].
pub const MARKET_FAMILY_TERMINAL_PROJECTION_BYTES_V1: usize = 280;

const ALL_FAMILY_MASK_V1: u8 = (1_u8 << MARKET_FAMILY_COUNT_V1) - 1;
const BINDING_PREIMAGE_BYTES_V1: usize = 265;
const FAMILY_TRANSITION_PREIMAGE_BYTES_V1: usize = 141;

/// Canonical exhaustive order of shared-Market product families.
pub const MARKET_FAMILIES_V1: [MarketFamilyV1; MARKET_FAMILY_COUNT_V1] = [
    MarketFamilyV1::General,
    MarketFamilyV1::Direct,
    MarketFamilyV1::Fractional,
    MarketFamilyV1::Dealer,
    MarketFamilyV1::Structured,
];

/// One exhaustive shared-Market product-family partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MarketFamilyV1 {
    /// General candidate-based market family.
    General = 0,
    /// Direct market family without a General candidate intermediary.
    Direct = 1,
    /// Fractional market family.
    Fractional = 2,
    /// Dealer-intermediated market family.
    Dealer = 3,
    /// Structured portfolio market family.
    Structured = 4,
}

impl MarketFamilyV1 {
    /// Stable array index and mask-bit position.
    pub const fn index(self) -> usize {
        match self {
            Self::General => 0,
            Self::Direct => 1,
            Self::Fractional => 2,
            Self::Dealer => 3,
            Self::Structured => 4,
        }
    }

    /// Stable exact encoded byte.
    pub const fn byte(self) -> u8 {
        match self {
            Self::General => 0,
            Self::Direct => 1,
            Self::Fractional => 2,
            Self::Dealer => 3,
            Self::Structured => 4,
        }
    }

    /// Stable capability-mask bit.
    pub const fn mask(self) -> u8 {
        1_u8 << self.byte()
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::General),
            1 => Ok(Self::Direct),
            2 => Ok(Self::Fractional),
            3 => Ok(Self::Dealer),
            4 => Ok(Self::Structured),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Exhaustive state of one family within one shared Market.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MarketFamilyStatusV1 {
    /// The immutable capability profile excludes the family.
    CapabilityDisabled = 0,
    /// The family is enabled, but no child has ever been admitted.
    EnabledNeverFounded = 1,
    /// At least one child was admitted and the family remains nonterminal.
    Live = 2,
    /// Admissions are sealed and every admitted child is terminal.
    Terminal = 3,
}

impl MarketFamilyStatusV1 {
    /// Stable exact encoded byte.
    pub const fn byte(self) -> u8 {
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

/// Canonical admission phase shared by all five family partitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MarketFamilyAggregatorPhaseV1 {
    /// New child admissions and child terminalizations are allowed.
    Open = 1,
    /// Admissions are sealed while already-live children finish.
    Retiring = 2,
    /// Every admitted child is terminal and the root is immutable.
    Terminal = 3,
}

impl MarketFamilyAggregatorPhaseV1 {
    /// Stable exact encoded byte.
    pub const fn byte(self) -> u8 {
        match self {
            Self::Open => 1,
            Self::Retiring => 2,
            Self::Terminal => 3,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Open),
            2 => Ok(Self::Retiring),
            3 => Ok(Self::Terminal),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Exact historical and current child counts for one family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFamilyCountsV1 {
    /// Number of children ever admitted to the family root.
    pub admitted: u32,
    /// Number of admitted children not yet terminal.
    pub live: u32,
    /// Number of admitted children that reached a terminal state.
    pub terminal: u32,
}

impl MarketFamilyCountsV1 {
    const ZERO: Self = Self {
        admitted: 0,
        live: 0,
        terminal: 0,
    };

    fn validate(self) -> Result<()> {
        if self.live.checked_add(self.terminal) != Some(self.admitted) {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
}

/// Immutable binding of one aggregator to its Market and canonical roots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFamilyAggregatorBindingV1 {
    /// Shared economic Market identity; never a Series identity.
    pub market_instance_id: MarketInstanceV2Id,
    /// Nonzero generation of this Market-scoped aggregator account.
    pub generation: u64,
    /// Registry release that authenticated the Market capabilities.
    pub registry_release_id: RegistryProgramReleaseV1Id,
    /// Capability profile that fixes the enabled-family mask.
    pub capability_profile_id: RegistryCapabilityProfileV2Id,
    /// Enabled-family bits in [`MARKET_FAMILIES_V1`] order.
    pub enabled_family_mask: u8,
    /// Canonical Market-scoped family-root account identities.
    ///
    /// Disabled families still have a canonical root identity so an adapter can
    /// authenticate absence at exactly one address.
    pub family_root_ids: [ContentId; MARKET_FAMILY_COUNT_V1],
}

impl MarketFamilyAggregatorBindingV1 {
    /// Validate all immutable identities, mask bits, and root separation.
    pub fn validate(&self) -> Result<()> {
        self.market_instance_id.validate()?;
        self.registry_release_id.validate()?;
        self.capability_profile_id.validate()?;
        if self.generation == 0 || self.enabled_family_mask & !ALL_FAMILY_MASK_V1 != 0 {
            return Err(Error::InvalidParameter);
        }

        let mut ids = [ContentId::ZERO; 8];
        ids[0] = self.market_instance_id.content_id();
        ids[1] = self.registry_release_id.content_id();
        ids[2] = self.capability_profile_id.content_id();
        let mut family_index = 0_usize;
        while family_index < MARKET_FAMILY_COUNT_V1 {
            self.family_root_ids[family_index].validate()?;
            ids[family_index + 3] = self.family_root_ids[family_index];
            family_index += 1;
        }
        let mut left = 0_usize;
        while left < ids.len() {
            let mut right = left + 1;
            while right < ids.len() {
                if ids[left] == ids[right] {
                    return Err(Error::MismatchedArtifact);
                }
                right += 1;
            }
            left += 1;
        }
        Ok(())
    }

    /// Whether the immutable capability profile enables one family.
    pub const fn is_enabled(&self, family: MarketFamilyV1) -> bool {
        self.enabled_family_mask & family.mask() != 0
    }

    /// Canonical family-root identity for one partition.
    pub const fn family_root_id(&self, family: MarketFamilyV1) -> ContentId {
        self.family_root_ids[family.index()]
    }

    /// Typed semantic identity of this immutable binding.
    pub fn id(&self) -> Result<MarketFamilyAggregatorBindingV1Id> {
        self.validate()?;
        let mut body = [0_u8; BINDING_PREIMAGE_BYTES_V1];
        let mut writer = Writer::new(&mut body, BINDING_PREIMAGE_BYTES_V1)?;
        writer.id(self.market_instance_id.content_id());
        writer.id(self.registry_release_id.content_id());
        writer.id(self.capability_profile_id.content_id());
        let mut index = 0_usize;
        while index < MARKET_FAMILY_COUNT_V1 {
            writer.id(self.family_root_ids[index]);
            index += 1;
        }
        writer.u64(self.generation);
        writer.u8(self.enabled_family_mask);
        writer.finish()?;
        let id = MarketFamilyAggregatorBindingV1Id(content_id(
            MARKET_FAMILY_AGGREGATOR_BINDING_DOMAIN_V1,
            &body,
        ));
        id.validate()?;
        Ok(id)
    }
}

/// Typed identity of one immutable aggregator binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct MarketFamilyAggregatorBindingV1Id(ContentId);

impl MarketFamilyAggregatorBindingV1Id {
    /// Construct from exact digest bytes without claiming authenticity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(ContentId::from_bytes(bytes))
    }

    /// Return the exact digest bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0.bytes()
    }

    /// Return this identity through the generic content-ID boundary.
    pub const fn content_id(self) -> ContentId {
        self.0
    }

    /// Refuse the all-zero identity reserved for inactive padding.
    pub fn validate(self) -> Result<()> {
        self.0.validate()
    }
}

/// Default-deny adapter boundary for family-root lifecycle changes.
///
/// Implementing this Rust trait is not cryptographic authentication. A live
/// adapter must make its implementation constructible only after checking the
/// aggregator and family-root account addresses, owners, exact bodies, PDAs,
/// registry provenance, and the named child admission or terminal receipt.
pub trait AuthenticatedMarketFamilyAuthorityV1 {
    /// Authenticate initialization of the exact immutable binding.
    fn authenticate_initialization(
        &self,
        _binding: &MarketFamilyAggregatorBindingV1,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate one unique child admission owned by the named family root.
    fn authenticate_admission(
        &self,
        _current: &MarketFamilyAggregatorV1,
        _family: MarketFamilyV1,
        _family_root_id: ContentId,
        _family_admission_sequence: u32,
        _admission_receipt_id: ContentId,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate one unique child terminal receipt owned by the family root.
    fn authenticate_terminal(
        &self,
        _current: &MarketFamilyAggregatorV1,
        _family: MarketFamilyV1,
        _family_root_id: ContentId,
        _family_terminal_sequence: u32,
        _terminal_receipt_id: ContentId,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }

    /// Authenticate the Market authority that irrevocably seals admissions.
    fn authenticate_begin_retirement(
        &self,
        _current: &MarketFamilyAggregatorV1,
    ) -> Result<()> {
        Err(Error::UnauthenticatedAuthority)
    }
}

/// Explicit default-deny authority useful at unauthenticated call sites.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoMarketFamilyAuthorityV1;

impl AuthenticatedMarketFamilyAuthorityV1 for NoMarketFamilyAuthorityV1 {}

/// Canonical persisted facts for one family partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFamilySlotV1 {
    status: MarketFamilyStatusV1,
    counts: MarketFamilyCountsV1,
    last_admission_transition_id: ContentId,
    last_terminal_transition_id: ContentId,
}

impl MarketFamilySlotV1 {
    const fn initial(enabled: bool) -> Self {
        Self {
            status: if enabled {
                MarketFamilyStatusV1::EnabledNeverFounded
            } else {
                MarketFamilyStatusV1::CapabilityDisabled
            },
            counts: MarketFamilyCountsV1::ZERO,
            last_admission_transition_id: ContentId::ZERO,
            last_terminal_transition_id: ContentId::ZERO,
        }
    }

    /// Exhaustive current family status.
    pub const fn status(&self) -> MarketFamilyStatusV1 {
        self.status
    }

    /// Exact admitted, live, and terminal child counts.
    pub const fn counts(&self) -> MarketFamilyCountsV1 {
        self.counts
    }

    /// Last authenticated admission transition, or zero before first admission.
    pub const fn last_admission_transition_id(&self) -> ContentId {
        self.last_admission_transition_id
    }

    /// Last authenticated terminal transition, or zero before one exists.
    pub const fn last_terminal_transition_id(&self) -> ContentId {
        self.last_terminal_transition_id
    }

    fn validate(&self, enabled: bool, phase: MarketFamilyAggregatorPhaseV1) -> Result<()> {
        self.counts.validate()?;
        if self.counts.admitted == 0 {
            if !self.last_admission_transition_id.is_zero()
                || !self.last_terminal_transition_id.is_zero()
            {
                return Err(Error::InvalidParameter);
            }
        } else {
            self.last_admission_transition_id.validate()?;
            if self.counts.terminal == 0 {
                if !self.last_terminal_transition_id.is_zero() {
                    return Err(Error::InvalidParameter);
                }
            } else {
                self.last_terminal_transition_id.validate()?;
            }
        }

        let expected = if !enabled {
            if self.counts != MarketFamilyCountsV1::ZERO {
                return Err(Error::InvalidParameter);
            }
            MarketFamilyStatusV1::CapabilityDisabled
        } else if self.counts.admitted == 0 {
            MarketFamilyStatusV1::EnabledNeverFounded
        } else {
            match phase {
                MarketFamilyAggregatorPhaseV1::Open => MarketFamilyStatusV1::Live,
                MarketFamilyAggregatorPhaseV1::Retiring => {
                    if self.counts.live == 0 {
                        MarketFamilyStatusV1::Terminal
                    } else {
                        MarketFamilyStatusV1::Live
                    }
                }
                MarketFamilyAggregatorPhaseV1::Terminal => {
                    if self.counts.live != 0 {
                        return Err(Error::InvalidParameter);
                    }
                    MarketFamilyStatusV1::Terminal
                }
            }
        };
        if self.status != expected {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
}

/// Canonical shared-Market owner of all family aggregation facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFamilyAggregatorV1 {
    binding: MarketFamilyAggregatorBindingV1,
    phase: MarketFamilyAggregatorPhaseV1,
    transition_sequence: u64,
    families: [MarketFamilySlotV1; MARKET_FAMILY_COUNT_V1],
}

impl MarketFamilyAggregatorV1 {
    /// Initialize an authenticated Market binding in the open phase.
    pub fn initialize<A: AuthenticatedMarketFamilyAuthorityV1 + ?Sized>(
        authority: &A,
        binding: MarketFamilyAggregatorBindingV1,
    ) -> Result<Self> {
        binding.validate()?;
        authority.authenticate_initialization(&binding)?;
        let families = [
            MarketFamilySlotV1::initial(binding.is_enabled(MarketFamilyV1::General)),
            MarketFamilySlotV1::initial(binding.is_enabled(MarketFamilyV1::Direct)),
            MarketFamilySlotV1::initial(binding.is_enabled(MarketFamilyV1::Fractional)),
            MarketFamilySlotV1::initial(binding.is_enabled(MarketFamilyV1::Dealer)),
            MarketFamilySlotV1::initial(binding.is_enabled(MarketFamilyV1::Structured)),
        ];
        let value = Self {
            binding,
            phase: MarketFamilyAggregatorPhaseV1::Open,
            transition_sequence: 0,
            families,
        };
        value.validate()?;
        Ok(value)
    }

    /// Immutable Market and canonical family-root binding.
    pub const fn binding(&self) -> &MarketFamilyAggregatorBindingV1 {
        &self.binding
    }

    /// Current aggregate admission phase.
    pub const fn phase(&self) -> MarketFamilyAggregatorPhaseV1 {
        self.phase
    }

    /// Exact number of accepted family and root-phase transitions.
    pub const fn transition_sequence(&self) -> u64 {
        self.transition_sequence
    }

    /// Canonical slot for one exhaustive family partition.
    pub const fn family(&self, family: MarketFamilyV1) -> &MarketFamilySlotV1 {
        &self.families[family.index()]
    }

    /// Whether the exact current state admits a new child in this family.
    pub fn admits_new_child(&self, family: MarketFamilyV1) -> bool {
        self.phase == MarketFamilyAggregatorPhaseV1::Open && self.binding.is_enabled(family)
    }

    /// Whether the capability-derived primary market modalities are founded.
    ///
    /// Activation requires an open root, at least one live General or Direct
    /// family, and no enabled General/Direct modality left silently unfounded.
    /// Optional Fractional, Dealer, and Structured families may still be
    /// founded by later authenticated admissions while the root remains open.
    pub fn activation_ready(&self) -> Result<bool> {
        self.validate()?;
        if self.phase != MarketFamilyAggregatorPhaseV1::Open {
            return Ok(false);
        }
        let general = self.family(MarketFamilyV1::General).status;
        let direct = self.family(MarketFamilyV1::Direct).status;
        let at_least_one_live = general == MarketFamilyStatusV1::Live
            || direct == MarketFamilyStatusV1::Live;
        let no_enabled_primary_unfounded = general
            != MarketFamilyStatusV1::EnabledNeverFounded
            && direct != MarketFamilyStatusV1::EnabledNeverFounded;
        Ok(at_least_one_live && no_enabled_primary_unfounded)
    }

    /// Validate all identities, family states, counts, and exact history length.
    pub fn validate(&self) -> Result<()> {
        self.binding.validate()?;
        let mut count_transitions = 0_u64;
        let mut index = 0_usize;
        while index < MARKET_FAMILY_COUNT_V1 {
            let family = MARKET_FAMILIES_V1[index];
            self.families[index].validate(self.binding.is_enabled(family), self.phase)?;
            count_transitions = count_transitions
                .checked_add(u64::from(self.families[index].counts.admitted))
                .and_then(|value| {
                    value.checked_add(u64::from(self.families[index].counts.terminal))
                })
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        let root_transitions = match self.phase {
            MarketFamilyAggregatorPhaseV1::Open => 0_u64,
            MarketFamilyAggregatorPhaseV1::Retiring => 1_u64,
            MarketFamilyAggregatorPhaseV1::Terminal => 2_u64,
        };
        let expected = count_transitions
            .checked_add(root_transitions)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.transition_sequence != expected {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Admit one unique authenticated child to an enabled family root.
    ///
    /// `family_admission_sequence` must equal the current admitted count. The
    /// adapter must separately prove that `admission_receipt_id` is the unique
    /// family-owned receipt at that sequence.
    pub fn admit_child<A: AuthenticatedMarketFamilyAuthorityV1 + ?Sized>(
        &self,
        authority: &A,
        family: MarketFamilyV1,
        family_admission_sequence: u32,
        admission_receipt_id: ContentId,
    ) -> Result<Self> {
        self.validate()?;
        if self.phase != MarketFamilyAggregatorPhaseV1::Open {
            return Err(Error::InvalidParameter);
        }
        if !self.binding.is_enabled(family) {
            return Err(Error::UnsupportedCapability);
        }
        admission_receipt_id.validate()?;
        let current = self.family(family);
        if family_admission_sequence != current.counts.admitted {
            return Err(Error::InvalidParameter);
        }
        authority.authenticate_admission(
            self,
            family,
            self.binding.family_root_id(family),
            family_admission_sequence,
            admission_receipt_id,
        )?;

        let next_transition_sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let transition_id = family_transition_id(
            MARKET_FAMILY_ADMISSION_DOMAIN_V1,
            self,
            family,
            family_admission_sequence,
            admission_receipt_id,
            next_transition_sequence,
        )?;
        let mut next = *self;
        let slot = &mut next.families[family.index()];
        slot.counts.admitted = slot
            .counts
            .admitted
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        slot.counts.live = slot
            .counts
            .live
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        slot.status = MarketFamilyStatusV1::Live;
        slot.last_admission_transition_id = transition_id;
        next.transition_sequence = next_transition_sequence;
        next.validate()?;
        Ok(next)
    }

    /// Mark one unique authenticated live child terminal.
    pub fn terminalize_child<A: AuthenticatedMarketFamilyAuthorityV1 + ?Sized>(
        &self,
        authority: &A,
        family: MarketFamilyV1,
        family_terminal_sequence: u32,
        terminal_receipt_id: ContentId,
    ) -> Result<Self> {
        self.validate()?;
        if self.phase == MarketFamilyAggregatorPhaseV1::Terminal {
            return Err(Error::InvalidParameter);
        }
        if !self.binding.is_enabled(family) {
            return Err(Error::UnsupportedCapability);
        }
        terminal_receipt_id.validate()?;
        let current = self.family(family);
        if current.counts.live == 0 || family_terminal_sequence != current.counts.terminal {
            return Err(Error::InvalidParameter);
        }
        authority.authenticate_terminal(
            self,
            family,
            self.binding.family_root_id(family),
            family_terminal_sequence,
            terminal_receipt_id,
        )?;

        let next_transition_sequence = self
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let transition_id = family_transition_id(
            MARKET_FAMILY_TERMINAL_DOMAIN_V1,
            self,
            family,
            family_terminal_sequence,
            terminal_receipt_id,
            next_transition_sequence,
        )?;
        let mut next = *self;
        let slot = &mut next.families[family.index()];
        slot.counts.live = slot
            .counts
            .live
            .checked_sub(1)
            .ok_or(Error::ArithmeticOverflow)?;
        slot.counts.terminal = slot
            .counts
            .terminal
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        if next.phase == MarketFamilyAggregatorPhaseV1::Retiring && slot.counts.live == 0 {
            slot.status = MarketFamilyStatusV1::Terminal;
        }
        slot.last_terminal_transition_id = transition_id;
        next.transition_sequence = next_transition_sequence;
        next.validate()?;
        Ok(next)
    }

    /// Irrevocably seal all future family admissions.
    pub fn begin_retirement<A: AuthenticatedMarketFamilyAuthorityV1 + ?Sized>(
        &self,
        authority: &A,
    ) -> Result<Self> {
        self.validate()?;
        if self.phase != MarketFamilyAggregatorPhaseV1::Open {
            return Err(Error::InvalidParameter);
        }
        authority.authenticate_begin_retirement(self)?;
        let mut next = *self;
        next.phase = MarketFamilyAggregatorPhaseV1::Retiring;
        next.transition_sequence = next
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut index = 0_usize;
        while index < MARKET_FAMILY_COUNT_V1 {
            if next.families[index].status == MarketFamilyStatusV1::Live
                && next.families[index].counts.live == 0
            {
                next.families[index].status = MarketFamilyStatusV1::Terminal;
            }
            index += 1;
        }
        next.validate()?;
        Ok(next)
    }

    /// Finalize an already-retiring root after every live child is terminal.
    pub fn finalize_terminal(
        &self,
    ) -> Result<(Self, MarketFamilyAggregatorTerminalProjectionV1)> {
        self.validate()?;
        if self.phase != MarketFamilyAggregatorPhaseV1::Retiring {
            return Err(Error::SeriesNotClosed);
        }
        let mut index = 0_usize;
        while index < MARKET_FAMILY_COUNT_V1 {
            if self.families[index].status == MarketFamilyStatusV1::Live {
                return Err(Error::SeriesNotClosed);
            }
            index += 1;
        }
        let mut next = *self;
        next.phase = MarketFamilyAggregatorPhaseV1::Terminal;
        next.transition_sequence = next
            .transition_sequence
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        next.validate()?;
        let summaries = next.exhaustive_summaries()?;
        let projection = MarketFamilyAggregatorTerminalProjectionV1::derive(&next, summaries)?;
        Ok((next, projection))
    }

    /// Derive one typed exhaustive summary for each canonical family.
    pub fn exhaustive_summaries(
        &self,
    ) -> Result<[MarketFamilyExhaustiveSummaryV1; MARKET_FAMILY_COUNT_V1]> {
        self.validate()?;
        Ok([
            MarketFamilyExhaustiveSummaryV1::derive(self, MarketFamilyV1::General)?,
            MarketFamilyExhaustiveSummaryV1::derive(self, MarketFamilyV1::Direct)?,
            MarketFamilyExhaustiveSummaryV1::derive(self, MarketFamilyV1::Fractional)?,
            MarketFamilyExhaustiveSummaryV1::derive(self, MarketFamilyV1::Dealer)?,
            MarketFamilyExhaustiveSummaryV1::derive(self, MarketFamilyV1::Structured)?,
        ])
    }

    /// Typed semantic identity of the exact canonical persisted state.
    pub fn semantic_id(&self) -> Result<MarketFamilyAggregatorV1Id> {
        self.validate()?;
        let mut body = [0_u8; MARKET_FAMILY_AGGREGATOR_BYTES_V1];
        self.encode_into(&mut body)?;
        let id = MarketFamilyAggregatorV1Id(content_id(
            MARKET_FAMILY_AGGREGATOR_DOMAIN_V1,
            &body,
        ));
        id.validate()?;
        Ok(id)
    }
}

impl FixedCodec for MarketFamilyAggregatorV1 {
    const ENCODED_LEN: usize = MARKET_FAMILY_AGGREGATOR_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MARKET_FAMILY_AGGREGATOR_MAGIC_V1);
        writer.u16(MARKET_FAMILY_SCHEMA_V1);
        writer.u8(self.phase.byte());
        writer.u8(self.binding.enabled_family_mask);
        writer.reserved(4);
        writer.id(self.binding.market_instance_id.content_id());
        writer.id(self.binding.registry_release_id.content_id());
        writer.id(self.binding.capability_profile_id.content_id());
        let mut index = 0_usize;
        while index < MARKET_FAMILY_COUNT_V1 {
            writer.id(self.binding.family_root_ids[index]);
            index += 1;
        }
        writer.u64(self.binding.generation);
        writer.u64(self.transition_sequence);
        index = 0;
        while index < MARKET_FAMILY_COUNT_V1 {
            let slot = self.families[index];
            writer.u8(slot.status.byte());
            writer.reserved(3);
            writer.u32(slot.counts.admitted);
            writer.u32(slot.counts.live);
            writer.u32(slot.counts.terminal);
            writer.id(slot.last_admission_transition_id);
            writer.id(slot.last_terminal_transition_id);
            index += 1;
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&MARKET_FAMILY_AGGREGATOR_MAGIC_V1)?;
        if reader.u16() != MARKET_FAMILY_SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        let phase = MarketFamilyAggregatorPhaseV1::decode(reader.u8())?;
        let enabled_family_mask = reader.u8();
        reader.reserved(4)?;
        let market_instance_id = MarketInstanceV2Id::from_bytes(reader.id().bytes());
        let registry_release_id = RegistryProgramReleaseV1Id::from_bytes(reader.id().bytes());
        let capability_profile_id =
            RegistryCapabilityProfileV2Id::from_bytes(reader.id().bytes());
        let mut family_root_ids = [ContentId::ZERO; MARKET_FAMILY_COUNT_V1];
        let mut index = 0_usize;
        while index < MARKET_FAMILY_COUNT_V1 {
            family_root_ids[index] = reader.id();
            index += 1;
        }
        let generation = reader.u64();
        let transition_sequence = reader.u64();
        let mut families = [MarketFamilySlotV1::initial(false); MARKET_FAMILY_COUNT_V1];
        index = 0;
        while index < MARKET_FAMILY_COUNT_V1 {
            let status = MarketFamilyStatusV1::decode(reader.u8())?;
            reader.reserved(3)?;
            let counts = MarketFamilyCountsV1 {
                admitted: reader.u32(),
                live: reader.u32(),
                terminal: reader.u32(),
            };
            families[index] = MarketFamilySlotV1 {
                status,
                counts,
                last_admission_transition_id: reader.id(),
                last_terminal_transition_id: reader.id(),
            };
            index += 1;
        }
        reader.finish()?;
        let value = Self {
            binding: MarketFamilyAggregatorBindingV1 {
                market_instance_id,
                generation,
                registry_release_id,
                capability_profile_id,
                enabled_family_mask,
                family_root_ids,
            },
            phase,
            transition_sequence,
            families,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Typed identity of one exact aggregator state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct MarketFamilyAggregatorV1Id(ContentId);

impl MarketFamilyAggregatorV1Id {
    /// Construct from exact digest bytes without claiming authenticity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(ContentId::from_bytes(bytes))
    }

    /// Return the exact digest bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0.bytes()
    }

    /// Return this identity through the generic content-ID boundary.
    pub const fn content_id(self) -> ContentId {
        self.0
    }

    /// Refuse the all-zero identity reserved for inactive padding.
    pub fn validate(self) -> Result<()> {
        self.0.validate()
    }
}

/// Exact typed snapshot of one family in one aggregator state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFamilyExhaustiveSummaryV1 {
    aggregator_phase: MarketFamilyAggregatorPhaseV1,
    aggregator_state_id: MarketFamilyAggregatorV1Id,
    binding_id: MarketFamilyAggregatorBindingV1Id,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    family: MarketFamilyV1,
    status: MarketFamilyStatusV1,
    family_root_id: ContentId,
    counts: MarketFamilyCountsV1,
    last_admission_transition_id: ContentId,
    last_terminal_transition_id: ContentId,
}

impl MarketFamilyExhaustiveSummaryV1 {
    fn derive(aggregator: &MarketFamilyAggregatorV1, family: MarketFamilyV1) -> Result<Self> {
        let slot = aggregator.family(family);
        let value = Self {
            aggregator_phase: aggregator.phase,
            aggregator_state_id: aggregator.semantic_id()?,
            binding_id: aggregator.binding.id()?,
            market_instance_id: aggregator.binding.market_instance_id,
            generation: aggregator.binding.generation,
            family,
            status: slot.status,
            family_root_id: aggregator.binding.family_root_id(family),
            counts: slot.counts,
            last_admission_transition_id: slot.last_admission_transition_id,
            last_terminal_transition_id: slot.last_terminal_transition_id,
        };
        value.validate()?;
        Ok(value)
    }

    /// Aggregate phase at which this exact snapshot was derived.
    pub const fn aggregator_phase(&self) -> MarketFamilyAggregatorPhaseV1 {
        self.aggregator_phase
    }

    /// Exact aggregator-state identity that owns these facts.
    pub const fn aggregator_state_id(&self) -> MarketFamilyAggregatorV1Id {
        self.aggregator_state_id
    }

    /// Immutable binding identity.
    pub const fn binding_id(&self) -> MarketFamilyAggregatorBindingV1Id {
        self.binding_id
    }

    /// Shared Market identity.
    pub const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Aggregator generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Family partition summarized by this artifact.
    pub const fn family(&self) -> MarketFamilyV1 {
        self.family
    }

    /// Exhaustive family status at the named aggregate state.
    pub const fn status(&self) -> MarketFamilyStatusV1 {
        self.status
    }

    /// Canonical family-root identity.
    pub const fn family_root_id(&self) -> ContentId {
        self.family_root_id
    }

    /// Exact admitted, live, and terminal counts.
    pub const fn counts(&self) -> MarketFamilyCountsV1 {
        self.counts
    }

    /// Whether this exact summary permits a new child admission.
    pub fn admits_new_child(&self) -> bool {
        self.aggregator_phase == MarketFamilyAggregatorPhaseV1::Open
            && (self.status == MarketFamilyStatusV1::EnabledNeverFounded
                || self.status == MarketFamilyStatusV1::Live)
    }

    /// Typed identity of this exact fixed-layout summary.
    pub fn id(&self) -> Result<MarketFamilyExhaustiveSummaryV1Id> {
        self.validate()?;
        let mut body = [0_u8; MARKET_FAMILY_EXHAUSTIVE_SUMMARY_BYTES_V1];
        self.encode_into(&mut body)?;
        let id = MarketFamilyExhaustiveSummaryV1Id(content_id(
            MARKET_FAMILY_EXHAUSTIVE_SUMMARY_DOMAIN_V1,
            &body,
        ));
        id.validate()?;
        Ok(id)
    }

    /// Validate exact status/count/phase semantics and all live identities.
    pub fn validate(&self) -> Result<()> {
        self.aggregator_state_id.validate()?;
        self.binding_id.validate()?;
        self.market_instance_id.validate()?;
        self.family_root_id.validate()?;
        if self.generation == 0 {
            return Err(Error::InvalidParameter);
        }
        self.counts.validate()?;
        if self.counts.admitted == 0 {
            if !self.last_admission_transition_id.is_zero()
                || !self.last_terminal_transition_id.is_zero()
                || (self.status != MarketFamilyStatusV1::CapabilityDisabled
                    && self.status != MarketFamilyStatusV1::EnabledNeverFounded)
            {
                return Err(Error::InvalidParameter);
            }
        } else {
            self.last_admission_transition_id.validate()?;
            if self.counts.terminal == 0 {
                if !self.last_terminal_transition_id.is_zero() {
                    return Err(Error::InvalidParameter);
                }
            } else {
                self.last_terminal_transition_id.validate()?;
            }
            match self.status {
                MarketFamilyStatusV1::Live => {
                    if self.aggregator_phase != MarketFamilyAggregatorPhaseV1::Open
                        && self.counts.live == 0
                    {
                        return Err(Error::InvalidParameter);
                    }
                }
                MarketFamilyStatusV1::Terminal => {
                    if self.aggregator_phase == MarketFamilyAggregatorPhaseV1::Open
                        || self.counts.live != 0
                    {
                        return Err(Error::InvalidParameter);
                    }
                }
                MarketFamilyStatusV1::CapabilityDisabled
                | MarketFamilyStatusV1::EnabledNeverFounded => {
                    return Err(Error::InvalidParameter);
                }
            }
        }
        if self.aggregator_phase == MarketFamilyAggregatorPhaseV1::Terminal
            && self.status == MarketFamilyStatusV1::Live
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
}

impl FixedCodec for MarketFamilyExhaustiveSummaryV1 {
    const ENCODED_LEN: usize = MARKET_FAMILY_EXHAUSTIVE_SUMMARY_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MARKET_FAMILY_EXHAUSTIVE_SUMMARY_MAGIC_V1);
        writer.u16(MARKET_FAMILY_SCHEMA_V1);
        writer.u8(self.aggregator_phase.byte());
        writer.u8(self.family.byte());
        writer.u8(self.status.byte());
        writer.reserved(3);
        writer.id(self.aggregator_state_id.content_id());
        writer.id(self.binding_id.content_id());
        writer.id(self.market_instance_id.content_id());
        writer.u64(self.generation);
        writer.id(self.family_root_id);
        writer.u32(self.counts.admitted);
        writer.u32(self.counts.live);
        writer.u32(self.counts.terminal);
        writer.reserved(4);
        writer.id(self.last_admission_transition_id);
        writer.id(self.last_terminal_transition_id);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&MARKET_FAMILY_EXHAUSTIVE_SUMMARY_MAGIC_V1)?;
        if reader.u16() != MARKET_FAMILY_SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        let aggregator_phase = MarketFamilyAggregatorPhaseV1::decode(reader.u8())?;
        let family = MarketFamilyV1::decode(reader.u8())?;
        let status = MarketFamilyStatusV1::decode(reader.u8())?;
        reader.reserved(3)?;
        let aggregator_state_id = MarketFamilyAggregatorV1Id::from_bytes(reader.id().bytes());
        let binding_id = MarketFamilyAggregatorBindingV1Id::from_bytes(reader.id().bytes());
        let market_instance_id = MarketInstanceV2Id::from_bytes(reader.id().bytes());
        let generation = reader.u64();
        let family_root_id = reader.id();
        let counts = MarketFamilyCountsV1 {
            admitted: reader.u32(),
            live: reader.u32(),
            terminal: reader.u32(),
        };
        reader.reserved(4)?;
        let last_admission_transition_id = reader.id();
        let last_terminal_transition_id = reader.id();
        reader.finish()?;
        let value = Self {
            aggregator_phase,
            aggregator_state_id,
            binding_id,
            market_instance_id,
            generation,
            family,
            status,
            family_root_id,
            counts,
            last_admission_transition_id,
            last_terminal_transition_id,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Typed identity of one exhaustive family summary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct MarketFamilyExhaustiveSummaryV1Id(ContentId);

impl MarketFamilyExhaustiveSummaryV1Id {
    /// Construct from exact digest bytes without claiming authenticity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(ContentId::from_bytes(bytes))
    }

    /// Return the exact digest bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0.bytes()
    }

    /// Return this identity through the generic content-ID boundary.
    pub const fn content_id(self) -> ContentId {
        self.0
    }

    /// Refuse the all-zero identity reserved for inactive padding.
    pub fn validate(self) -> Result<()> {
        self.0.validate()
    }
}

/// Final typed references to all five exhaustive family summaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFamilyAggregatorTerminalProjectionV1 {
    aggregator_state_id: MarketFamilyAggregatorV1Id,
    binding_id: MarketFamilyAggregatorBindingV1Id,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    summary_ids: [MarketFamilyExhaustiveSummaryV1Id; MARKET_FAMILY_COUNT_V1],
}

impl MarketFamilyAggregatorTerminalProjectionV1 {
    fn derive(
        aggregator: &MarketFamilyAggregatorV1,
        summaries: [MarketFamilyExhaustiveSummaryV1; MARKET_FAMILY_COUNT_V1],
    ) -> Result<Self> {
        if aggregator.phase != MarketFamilyAggregatorPhaseV1::Terminal {
            return Err(Error::SeriesNotClosed);
        }
        let mut summary_ids =
            [MarketFamilyExhaustiveSummaryV1Id(ContentId::ZERO); MARKET_FAMILY_COUNT_V1];
        let mut index = 0_usize;
        while index < MARKET_FAMILY_COUNT_V1 {
            if summaries[index].family != MARKET_FAMILIES_V1[index]
                || summaries[index].aggregator_state_id != aggregator.semantic_id()?
            {
                return Err(Error::MismatchedArtifact);
            }
            summary_ids[index] = summaries[index].id()?;
            index += 1;
        }
        let value = Self {
            aggregator_state_id: aggregator.semantic_id()?,
            binding_id: aggregator.binding.id()?,
            market_instance_id: aggregator.binding.market_instance_id,
            generation: aggregator.binding.generation,
            summary_ids,
        };
        value.validate()?;
        Ok(value)
    }

    /// Final aggregator-state identity.
    pub const fn aggregator_state_id(&self) -> MarketFamilyAggregatorV1Id {
        self.aggregator_state_id
    }

    /// Immutable aggregator-binding identity.
    pub const fn binding_id(&self) -> MarketFamilyAggregatorBindingV1Id {
        self.binding_id
    }

    /// Shared Market identity.
    pub const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Aggregator generation.
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Typed current summary identity for one family.
    pub const fn summary_id(
        &self,
        family: MarketFamilyV1,
    ) -> MarketFamilyExhaustiveSummaryV1Id {
        self.summary_ids[family.index()]
    }

    /// All summary identities in canonical family order.
    pub const fn summary_ids(
        &self,
    ) -> [MarketFamilyExhaustiveSummaryV1Id; MARKET_FAMILY_COUNT_V1] {
        self.summary_ids
    }

    /// Typed identity of this final exact projection.
    pub fn id(&self) -> Result<MarketFamilyAggregatorTerminalProjectionV1Id> {
        self.validate()?;
        let mut body = [0_u8; MARKET_FAMILY_TERMINAL_PROJECTION_BYTES_V1];
        self.encode_into(&mut body)?;
        let id = MarketFamilyAggregatorTerminalProjectionV1Id(content_id(
            MARKET_FAMILY_TERMINAL_PROJECTION_DOMAIN_V1,
            &body,
        ));
        id.validate()?;
        Ok(id)
    }

    /// Validate all live identities and family-summary separation.
    pub fn validate(&self) -> Result<()> {
        self.aggregator_state_id.validate()?;
        self.binding_id.validate()?;
        self.market_instance_id.validate()?;
        if self.generation == 0 {
            return Err(Error::InvalidParameter);
        }
        let mut left = 0_usize;
        while left < MARKET_FAMILY_COUNT_V1 {
            self.summary_ids[left].validate()?;
            let mut right = left + 1;
            while right < MARKET_FAMILY_COUNT_V1 {
                if self.summary_ids[left] == self.summary_ids[right] {
                    return Err(Error::MismatchedArtifact);
                }
                right += 1;
            }
            left += 1;
        }
        Ok(())
    }
}

impl FixedCodec for MarketFamilyAggregatorTerminalProjectionV1 {
    const ENCODED_LEN: usize = MARKET_FAMILY_TERMINAL_PROJECTION_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MARKET_FAMILY_TERMINAL_PROJECTION_MAGIC_V1);
        writer.u16(MARKET_FAMILY_SCHEMA_V1);
        writer.reserved(6);
        writer.id(self.aggregator_state_id.content_id());
        writer.id(self.binding_id.content_id());
        writer.id(self.market_instance_id.content_id());
        writer.u64(self.generation);
        let mut index = 0_usize;
        while index < MARKET_FAMILY_COUNT_V1 {
            writer.id(self.summary_ids[index].content_id());
            index += 1;
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&MARKET_FAMILY_TERMINAL_PROJECTION_MAGIC_V1)?;
        if reader.u16() != MARKET_FAMILY_SCHEMA_V1 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let aggregator_state_id = MarketFamilyAggregatorV1Id::from_bytes(reader.id().bytes());
        let binding_id = MarketFamilyAggregatorBindingV1Id::from_bytes(reader.id().bytes());
        let market_instance_id = MarketInstanceV2Id::from_bytes(reader.id().bytes());
        let generation = reader.u64();
        let mut summary_ids =
            [MarketFamilyExhaustiveSummaryV1Id(ContentId::ZERO); MARKET_FAMILY_COUNT_V1];
        let mut index = 0_usize;
        while index < MARKET_FAMILY_COUNT_V1 {
            summary_ids[index] =
                MarketFamilyExhaustiveSummaryV1Id::from_bytes(reader.id().bytes());
            index += 1;
        }
        reader.finish()?;
        let value = Self {
            aggregator_state_id,
            binding_id,
            market_instance_id,
            generation,
            summary_ids,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Typed identity of one final five-family projection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct MarketFamilyAggregatorTerminalProjectionV1Id(ContentId);

impl MarketFamilyAggregatorTerminalProjectionV1Id {
    /// Construct from exact digest bytes without claiming authenticity.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(ContentId::from_bytes(bytes))
    }

    /// Return the exact digest bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0.bytes()
    }

    /// Return this identity through the generic content-ID boundary.
    pub const fn content_id(self) -> ContentId {
        self.0
    }

    /// Refuse the all-zero identity reserved for inactive padding.
    pub fn validate(self) -> Result<()> {
        self.0.validate()
    }
}

fn family_transition_id(
    domain: &[u8],
    current: &MarketFamilyAggregatorV1,
    family: MarketFamilyV1,
    family_sequence: u32,
    receipt_id: ContentId,
    next_transition_sequence: u64,
) -> Result<ContentId> {
    let mut body = [0_u8; FAMILY_TRANSITION_PREIMAGE_BYTES_V1];
    let mut writer = Writer::new(&mut body, FAMILY_TRANSITION_PREIMAGE_BYTES_V1)?;
    writer.id(current.semantic_id()?.content_id());
    writer.id(current.binding.id()?.content_id());
    writer.u8(family.byte());
    writer.id(current.binding.family_root_id(family));
    writer.id(receipt_id);
    writer.u32(family_sequence);
    writer.u64(next_transition_sequence);
    writer.finish()?;
    let id = content_id(domain, &body);
    id.validate()?;
    Ok(id)
}
