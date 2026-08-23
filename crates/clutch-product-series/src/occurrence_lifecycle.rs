//! Occurrence-scoped owner of whole-Market terminality.
//!
//! Every child family remains the sole owner of its internal liabilities and
//! child counts. This root counts only adapter-authenticated terminal
//! projections emitted at those family boundaries. In particular, General
//! contributes one market-scoped summary proving every admitted Epoch and
//! SettlementRoot terminal; none of its candidate, reservation, owner, or
//! fee-child facts are copied here.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, ContentId, Error, FixedCodec, MarketInstanceV2Id, Result, SeriesPlanV5Id,
    SourceOccurrenceV1Id,
};

const ROOT_MAGIC_V1: [u8; 8] = *b"DCPORV1\0";
const ROOT_VERSION_V1: u16 = 1;
const ROOT_HEADER_BYTES_V1: usize = 16;
const ROOT_IDENTITY_COUNT_V1: usize = 32;
const ROOT_SCALAR_BYTES_V1: usize = 48 + 8 * 8;

/// Exact number of independently owned terminal families.
pub const PRODUCT_OCCURRENCE_FAMILY_COUNT_V1: usize = 10;
/// Exact canonical semantic-body width of [`ProductOccurrenceRootV1`].
pub const PRODUCT_OCCURRENCE_ROOT_BYTES_V1: usize = ROOT_HEADER_BYTES_V1
    + ROOT_IDENTITY_COUNT_V1 * 32
    + ROOT_SCALAR_BYTES_V1
    + PRODUCT_OCCURRENCE_FAMILY_COUNT_V1 * 3 * 4
    + PRODUCT_OCCURRENCE_FAMILY_COUNT_V1 * 32
    + 2 * 32;
/// Product occurrence semantic-state identity domain.
pub const PRODUCT_OCCURRENCE_ROOT_DOMAIN_V1: &[u8] = b"dragons-clutch/product-occurrence-root/v1";
/// Immutable occurrence-binding identity domain.
pub const PRODUCT_OCCURRENCE_BINDING_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-occurrence-binding/v1";
/// Family-terminal adapter receipt identity domain.
pub const PRODUCT_OCCURRENCE_FAMILY_TERMINAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-occurrence-family-terminal/v1";
/// Pure whole-Market terminal projection identity domain.
pub const MARKET_INSTANCE_TERMINAL_PROJECTION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/market-instance-terminal-projection/v1";

const _: () = assert!(PRODUCT_OCCURRENCE_ROOT_BYTES_V1 == 1_656);

/// Independently owned occurrence family counted by the Product root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProductOccurrenceFamilyV1 {
    /// Aggregate native-claim liability owner.
    ClaimLedger = 0,
    /// Classified collateral-liability and custody owner.
    Hoard = 1,
    /// Market-scoped General summary over every admitted Epoch/SettlementRoot.
    General = 2,
    /// Market-scoped Dealer root proving every facility/lease child terminal.
    Dealer = 3,
    /// Failure interval-resolution dependent.
    Failure = 4,
    /// Source generation/window terminal owner.
    Source = 5,
    /// Position-family root proving every admitted Position terminal or absent.
    Position = 6,
    /// Fractional-credit ledger terminal owner.
    Fractional = 7,
    /// Structured-family root proving every descriptor/lot terminal or absent.
    Structured = 8,
    /// This ordinal's recurring-Series occurrence terminal owner.
    SeriesOccurrence = 9,
}

/// How one exhaustive family summary discharged its Product-root obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProductOccurrenceFamilyDispositionV1 {
    /// The enabled family retired every occurrence-scoped liability.
    Terminal = 1,
    /// The immutable capability profile disabled the optional family and its
    /// adapter authenticated zero admissions at every canonical family PDA.
    Absent = 2,
}

impl ProductOccurrenceFamilyDispositionV1 {
    const fn byte(self) -> u8 {
        match self {
            Self::Terminal => 1,
            Self::Absent => 2,
        }
    }
}

impl ProductOccurrenceFamilyV1 {
    /// Frozen wire discriminant.
    pub const fn byte(self) -> u8 {
        match self {
            Self::ClaimLedger => 0,
            Self::Hoard => 1,
            Self::General => 2,
            Self::Dealer => 3,
            Self::Failure => 4,
            Self::Source => 5,
            Self::Position => 6,
            Self::Fractional => 7,
            Self::Structured => 8,
            Self::SeriesOccurrence => 9,
        }
    }

    /// Canonical fixed-array index.
    pub const fn index(self) -> usize {
        match self {
            Self::ClaimLedger => 0,
            Self::Hoard => 1,
            Self::General => 2,
            Self::Dealer => 3,
            Self::Failure => 4,
            Self::Source => 5,
            Self::Position => 6,
            Self::Fractional => 7,
            Self::Structured => 8,
            Self::SeriesOccurrence => 9,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::ClaimLedger),
            1 => Ok(Self::Hoard),
            2 => Ok(Self::General),
            3 => Ok(Self::Dealer),
            4 => Ok(Self::Failure),
            5 => Ok(Self::Source),
            6 => Ok(Self::Position),
            7 => Ok(Self::Fractional),
            8 => Ok(Self::Structured),
            9 => Ok(Self::SeriesOccurrence),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Monotone lifecycle of the whole occurrence root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProductOccurrencePhaseV1 {
    /// Child families may still carry economic liabilities.
    Active = 1,
    /// New liabilities are disabled and terminal capabilities are collected.
    Retiring = 2,
    /// Every expected family capability has been consumed.
    Terminal = 3,
}

impl ProductOccurrencePhaseV1 {
    fn byte(self) -> u8 {
        match self {
            Self::Active => 1,
            Self::Retiring => 2,
            Self::Terminal => 3,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Active),
            2 => Ok(Self::Retiring),
            3 => Ok(Self::Terminal),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Complete immutable Product/Series/Source identity graph for one occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductOccurrenceBindingV1 {
    /// Exact full-width economic Market identity.
    pub market_instance_id: MarketInstanceV2Id,
    /// Exact recurring Series identity.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact ordinal within the finite Series.
    pub ordinal: u32,
    /// One-shot occurrence generation.
    pub generation: u64,
    /// ProductTemplate V4 identity.
    pub product_template_id: ContentId,
    /// NativeClaimBasis V1 identity.
    pub native_claim_basis_id: ContentId,
    /// Evidence-only Recovery policy identity.
    pub recovery_policy_id: ContentId,
    /// Quantized price-measure policy identity.
    pub price_measure_policy_id: ContentId,
    /// MarketGenesisProfile V2 identity.
    pub market_genesis_profile_id: ContentId,
    /// SeriesFundingTerms V2 identity.
    pub funding_terms_id: ContentId,
    /// Per-occurrence SeriesFundingQuote identity.
    pub funding_quote_id: ContentId,
    /// Operational attachment-plan identity.
    pub attachment_plan_id: ContentId,
    /// Exact compiler-output bundle identity.
    pub compiler_output_id: ContentId,
    /// Exact compiled Source occurrence record identity.
    pub source_occurrence_id: SourceOccurrenceV1Id,
    /// Exact physical Source occurrence account.
    pub source_occurrence_account_id: ContentId,
    /// Receipt authenticating the complete physical occurrence account bytes.
    pub source_occurrence_account_authentication_id: ContentId,
    /// Exact Product/Source occurrence join receipt.
    pub source_occurrence_receipt_id: ContentId,
    /// Authenticated Source release-manifest identity.
    pub source_release_manifest_id: ContentId,
    /// Exact authenticated Source route.
    pub source_route_id: ContentId,
    /// Exact Clock policy selected by that Source route.
    pub source_clock_policy_id: ContentId,
    /// SourcePlane semantic contract identity.
    pub source_plane_contract_id: ContentId,
    /// Source specification identity.
    pub source_spec_id: ContentId,
    /// Exact absolute Window specification identity.
    pub window_spec_id: ContentId,
    /// Exact statistic-key identity.
    pub statistic_key_id: ContentId,
    /// Exact Source repair generation authenticated for this occurrence.
    pub source_repair_generation: u64,
    /// Prefunded Failure interval-consensus work PDA.
    pub failure_interval_work_account_id: ContentId,
    /// Prefunded permanent Failure interval replay PDA.
    pub failure_interval_replay_account_id: ContentId,
    /// Prefunded canonical full-width Resolution V5 PDA.
    pub resolution_account_id: ContentId,
    /// Exact Failure policy binding admitted for this occurrence.
    pub failure_policy_binding_id: ContentId,
    /// Exact admitted Recovery semantic-state identity.
    pub recovery_state_id: ContentId,
    /// Central-profile-derived interval-consensus resource profile identity.
    pub interval_consensus_profile_id: ContentId,
    /// Largest admitted inclusive interval width.
    pub maximum_interval_width: u64,
    /// Largest coordinate count accepted by one paid advance.
    pub maximum_coordinates_per_advance: u16,
    /// Current authenticated central Registry release.
    pub registry_release_id: ContentId,
    /// Exact immutable capability-profile identity.
    pub capability_profile_id: ContentId,
    /// Payer which owns the root account's refundable rent principal.
    pub rent_payer: ContentId,
    /// System-owned destination for unsolicited root-account donations.
    pub neutral_lamport_sink: ContentId,
}

impl ProductOccurrenceBindingV1 {
    /// Refuse incomplete or aliased top-level identity graphs.
    pub fn validate(self) -> Result<()> {
        self.market_instance_id.validate()?;
        self.series_plan_id.validate()?;
        self.source_occurrence_id.validate()?;
        if self.generation == 0
            || self.source_repair_generation == 0
            || self.maximum_interval_width == u64::MAX
            || self.maximum_coordinates_per_advance == 0
        {
            return Err(Error::InvalidParameter);
        }
        let ids = self.identity_ids();
        for id in ids {
            id.validate()?;
        }
        let mut left = 0usize;
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

    /// Content identity of the complete immutable occurrence binding.
    pub fn id(self) -> Result<ContentId> {
        self.validate()?;
        let mut bytes = [0; 1_054];
        let mut at = 0usize;
        for id in self.identity_ids() {
            bytes[at..at + 32].copy_from_slice(&id.bytes());
            at += 32;
        }
        bytes[at..at + 4].copy_from_slice(&self.ordinal.to_le_bytes());
        at += 4;
        bytes[at..at + 8].copy_from_slice(&self.generation.to_le_bytes());
        at += 8;
        bytes[at..at + 8].copy_from_slice(&self.source_repair_generation.to_le_bytes());
        at += 8;
        bytes[at..at + 8].copy_from_slice(&self.maximum_interval_width.to_le_bytes());
        at += 8;
        bytes[at..at + 2].copy_from_slice(&self.maximum_coordinates_per_advance.to_le_bytes());
        let id = content_id(PRODUCT_OCCURRENCE_BINDING_DOMAIN_V1, &bytes);
        id.validate()?;
        Ok(id)
    }

    fn content_ids(self) -> [ContentId; 30] {
        [
            self.product_template_id,
            self.native_claim_basis_id,
            self.recovery_policy_id,
            self.price_measure_policy_id,
            self.market_genesis_profile_id,
            self.funding_terms_id,
            self.funding_quote_id,
            self.attachment_plan_id,
            self.compiler_output_id,
            self.source_occurrence_account_id,
            self.source_occurrence_account_authentication_id,
            self.source_occurrence_receipt_id,
            self.source_release_manifest_id,
            self.source_route_id,
            self.source_clock_policy_id,
            self.source_plane_contract_id,
            self.source_spec_id,
            self.window_spec_id,
            self.statistic_key_id,
            self.failure_interval_work_account_id,
            self.failure_interval_replay_account_id,
            self.resolution_account_id,
            self.failure_policy_binding_id,
            self.recovery_state_id,
            self.interval_consensus_profile_id,
            self.registry_release_id,
            self.capability_profile_id,
            self.rent_payer,
            self.neutral_lamport_sink,
            self.source_occurrence_id.content_id(),
        ]
    }

    fn identity_ids(self) -> [ContentId; 32] {
        let content = self.content_ids();
        let mut ids = [ContentId::ZERO; 32];
        ids[0] = self.market_instance_id.content_id();
        ids[1] = self.series_plan_id.content_id();
        ids[2..].copy_from_slice(&content);
        ids
    }
}

/// Immutable, disjoint present funding for occurrence-owned account creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductOccurrenceCapitalizationV1 {
    /// Exact payer-funded principal for `[0xaa root, 0xab work, 0xac replay, Resolution V5]`.
    pub principal_lamports: [u64; 4],
    /// Preexisting third-party lamports retained separately as donations.
    pub donation_floor_lamports: [u64; 4],
}

impl ProductOccurrenceCapitalizationV1 {
    /// Refuse missing principal and balance overflow.
    pub fn validate(self) -> Result<()> {
        let mut index = 0usize;
        while index < self.principal_lamports.len() {
            if self.principal_lamports[index] == 0 {
                return Err(Error::InsufficientPrepayment);
            }
            self.principal_lamports[index]
                .checked_add(self.donation_floor_lamports[index])
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        Ok(())
    }

    /// Exact post-capitalization balances in root/work/replay/resolution order.
    pub fn postfund_balances(self) -> Result<[u64; 4]> {
        self.validate()?;
        Ok([
            self.principal_lamports[0]
                .checked_add(self.donation_floor_lamports[0])
                .ok_or(Error::ArithmeticOverflow)?,
            self.principal_lamports[1]
                .checked_add(self.donation_floor_lamports[1])
                .ok_or(Error::ArithmeticOverflow)?,
            self.principal_lamports[2]
                .checked_add(self.donation_floor_lamports[2])
                .ok_or(Error::ArithmeticOverflow)?,
            self.principal_lamports[3]
                .checked_add(self.donation_floor_lamports[3])
                .ok_or(Error::ArithmeticOverflow)?,
        ])
    }
}

/// Exhaustive root-owned counts of family-boundary capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductOccurrenceCountsV1 {
    /// Frozen family-root capability count; canonically one for every family.
    pub expected: [u32; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1],
    /// Capabilities not yet presented to this root.
    pub live: [u32; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1],
    /// Capabilities consumed monotonically by this root.
    pub terminal: [u32; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1],
}

impl ProductOccurrenceCountsV1 {
    fn validate(self) -> Result<()> {
        let mut index = 0usize;
        while index < PRODUCT_OCCURRENCE_FAMILY_COUNT_V1 {
            if self.expected[index] == 0
                || self.live[index]
                    .checked_add(self.terminal[index])
                    .ok_or(Error::ArithmeticOverflow)?
                    != self.expected[index]
            {
                return Err(Error::InvalidParameter);
            }
            index += 1;
        }
        for expected in self.expected {
            if expected != 1 {
                return Err(Error::InvalidParameter);
            }
        }
        Ok(())
    }

    /// Whether every independently owned terminal capability was consumed.
    pub fn complete(self) -> Result<bool> {
        self.validate()?;
        let mut index = 0usize;
        while index < PRODUCT_OCCURRENCE_FAMILY_COUNT_V1 {
            if self.live[index] != 0 || self.terminal[index] != self.expected[index] {
                return Ok(false);
            }
            index += 1;
        }
        Ok(true)
    }
}

/// Pure terminal projection carried across one family boundary.
///
/// This is deliberately not authentication. The SBF adapter wraps it in a
/// private receipt only after authenticating the family-owned root and terminal
/// capability in the same instruction which mutates Product state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductOccurrenceFamilyTerminalProjectionV1 {
    id: ContentId,
    binding_id: ContentId,
    family: ProductOccurrenceFamilyV1,
    disposition: ProductOccurrenceFamilyDispositionV1,
    market_instance_id: MarketInstanceV2Id,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    generation: u64,
    family_terminal_sequence: u32,
    root_transition_sequence: u64,
    owner_program_id: ContentId,
    owner_release_id: ContentId,
    owner_account_id: ContentId,
    owner_terminal_receipt_id: ContentId,
    terminal_state_ids: [ContentId; 2],
}

impl ProductOccurrenceFamilyTerminalProjectionV1 {
    /// Construct the deterministic projection which a live adapter must authenticate.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        family: ProductOccurrenceFamilyV1,
        binding: ProductOccurrenceBindingV1,
        family_terminal_sequence: u32,
        root_transition_sequence: u64,
        owner_program_id: ContentId,
        owner_release_id: ContentId,
        owner_account_id: ContentId,
        owner_terminal_receipt_id: ContentId,
        terminal_state_ids: [ContentId; 2],
    ) -> Result<Self> {
        Self::new_with_disposition(
            family,
            ProductOccurrenceFamilyDispositionV1::Terminal,
            binding,
            family_terminal_sequence,
            root_transition_sequence,
            owner_program_id,
            owner_release_id,
            owner_account_id,
            owner_terminal_receipt_id,
            terminal_state_ids,
        )
    }

    /// Construct a disabled-family absence summary.
    ///
    /// Only Dealer, Fractional, and Structured are optional in V1. The live
    /// adapter must authenticate both the immutable disabled capability and
    /// the family's canonical zero-admission/PDA-absence facts before this
    /// pure projection is sealed behind a private receipt.
    #[allow(clippy::too_many_arguments)]
    pub fn absent(
        family: ProductOccurrenceFamilyV1,
        binding: ProductOccurrenceBindingV1,
        family_terminal_sequence: u32,
        root_transition_sequence: u64,
        owner_program_id: ContentId,
        owner_release_id: ContentId,
        owner_account_id: ContentId,
        owner_absence_receipt_id: ContentId,
    ) -> Result<Self> {
        Self::new_with_disposition(
            family,
            ProductOccurrenceFamilyDispositionV1::Absent,
            binding,
            family_terminal_sequence,
            root_transition_sequence,
            owner_program_id,
            owner_release_id,
            owner_account_id,
            owner_absence_receipt_id,
            [ContentId::ZERO; 2],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_disposition(
        family: ProductOccurrenceFamilyV1,
        disposition: ProductOccurrenceFamilyDispositionV1,
        binding: ProductOccurrenceBindingV1,
        family_terminal_sequence: u32,
        root_transition_sequence: u64,
        owner_program_id: ContentId,
        owner_release_id: ContentId,
        owner_account_id: ContentId,
        owner_terminal_receipt_id: ContentId,
        terminal_state_ids: [ContentId; 2],
    ) -> Result<Self> {
        binding.validate()?;
        owner_program_id.validate()?;
        owner_release_id.validate()?;
        owner_account_id.validate()?;
        owner_terminal_receipt_id.validate()?;
        if disposition == ProductOccurrenceFamilyDispositionV1::Absent
            && !matches!(
                family,
                ProductOccurrenceFamilyV1::Dealer
                    | ProductOccurrenceFamilyV1::Fractional
                    | ProductOccurrenceFamilyV1::Structured
            )
        {
            return Err(Error::UnsupportedCapability);
        }
        if disposition == ProductOccurrenceFamilyDispositionV1::Terminal
            && family == ProductOccurrenceFamilyV1::Fractional
        {
            terminal_state_ids[0].validate()?;
            terminal_state_ids[1].validate()?;
            if terminal_state_ids[0] == terminal_state_ids[1] {
                return Err(Error::MismatchedArtifact);
            }
        } else if terminal_state_ids != [ContentId::ZERO; 2] {
            return Err(Error::MismatchedArtifact);
        }
        if root_transition_sequence == 0
            || owner_program_id == owner_release_id
            || owner_program_id == owner_account_id
            || owner_program_id == owner_terminal_receipt_id
            || owner_release_id == owner_account_id
            || owner_release_id == owner_terminal_receipt_id
            || owner_account_id == owner_terminal_receipt_id
            || owner_account_id == binding.market_instance_id.content_id()
            || owner_terminal_receipt_id == binding.market_instance_id.content_id()
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        let binding_id = binding.id()?;
        let id = content_id(
            PRODUCT_OCCURRENCE_FAMILY_TERMINAL_DOMAIN_V1,
            &family_terminal_preimage(
                family,
                disposition,
                binding,
                family_terminal_sequence,
                root_transition_sequence,
                owner_program_id,
                owner_release_id,
                owner_account_id,
                owner_terminal_receipt_id,
                binding_id,
                terminal_state_ids,
            ),
        );
        id.validate()?;
        Ok(Self {
            id,
            binding_id,
            family,
            disposition,
            market_instance_id: binding.market_instance_id,
            series_plan_id: binding.series_plan_id,
            ordinal: binding.ordinal,
            generation: binding.generation,
            family_terminal_sequence,
            root_transition_sequence,
            owner_program_id,
            owner_release_id,
            owner_account_id,
            owner_terminal_receipt_id,
            terminal_state_ids,
        })
    }

    /// Canonical adapter receipt identity.
    pub const fn id(self) -> ContentId {
        self.id
    }

    /// Complete immutable Product/Series/Source binding consumed by this receipt.
    pub const fn binding_id(self) -> ContentId {
        self.binding_id
    }

    /// Independently owned terminal family.
    pub const fn family(self) -> ProductOccurrenceFamilyV1 {
        self.family
    }

    /// Whether the family retired live state or was canonically absent.
    pub const fn disposition(self) -> ProductOccurrenceFamilyDispositionV1 {
        self.disposition
    }

    /// Exact full-width Market occurrence.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact recurring Series.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    /// Exact occurrence ordinal.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    /// Exact occurrence generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact family-local monotone terminal ordinal.
    pub const fn family_terminal_sequence(self) -> u32 {
        self.family_terminal_sequence
    }

    /// Exact Product-root transition sequence consumed by this receipt.
    pub const fn root_transition_sequence(self) -> u64 {
        self.root_transition_sequence
    }

    /// Program owning the authenticated family root/account.
    pub const fn owner_program_id(self) -> ContentId {
        self.owner_program_id
    }

    /// Exact reviewed release identity for that family owner.
    pub const fn owner_release_id(self) -> ContentId {
        self.owner_release_id
    }

    /// Authenticated family root/account identity.
    pub const fn owner_account_id(self) -> ContentId {
        self.owner_account_id
    }

    /// Exact terminal receipt which the live adapter must authenticate.
    pub const fn owner_terminal_receipt_id(self) -> ContentId {
        self.owner_terminal_receipt_id
    }

    /// Family-owned terminal state IDs retained by the Product root.
    ///
    /// V1 uses both slots only for Fractional's closed a4 policy and a5
    /// ledger. Every other family emits the canonical zero pair.
    pub const fn terminal_state_ids(self) -> [ContentId; 2] {
        self.terminal_state_ids
    }
}

/// Full persisted semantic state of one Product occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductOccurrenceRootV1 {
    binding: ProductOccurrenceBindingV1,
    capitalization: ProductOccurrenceCapitalizationV1,
    phase: ProductOccurrencePhaseV1,
    transition_sequence: u64,
    counts: ProductOccurrenceCountsV1,
    last_terminal_receipts: [ContentId; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1],
    fractional_terminal_state_ids: [ContentId; 2],
}

impl ProductOccurrenceRootV1 {
    /// Found one active occurrence with one required summary per family owner.
    pub fn initialize(
        binding: ProductOccurrenceBindingV1,
        capitalization: ProductOccurrenceCapitalizationV1,
    ) -> Result<Self> {
        binding.validate()?;
        capitalization.validate()?;
        let expected = [1; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1];
        let counts = ProductOccurrenceCountsV1 {
            expected,
            live: expected,
            terminal: [0; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1],
        };
        counts.validate()?;
        let value = Self {
            binding,
            capitalization,
            phase: ProductOccurrencePhaseV1::Active,
            transition_sequence: 0,
            counts,
            last_terminal_receipts: [ContentId::ZERO; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1],
            fractional_terminal_state_ids: [ContentId::ZERO; 2],
        };
        value.validate()?;
        Ok(value)
    }

    /// Enter the terminal-capability collection phase exactly once.
    pub fn begin_retirement(self) -> Result<Self> {
        self.validate()?;
        if self.phase != ProductOccurrencePhaseV1::Active {
            return Err(Error::WorkStateMismatch);
        }
        let next = Self {
            phase: ProductOccurrencePhaseV1::Retiring,
            transition_sequence: 1,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Consume one exact next family terminal capability.
    pub fn consume_family_terminal(
        self,
        terminal: ProductOccurrenceFamilyTerminalProjectionV1,
    ) -> Result<Self> {
        self.validate()?;
        if self.phase != ProductOccurrencePhaseV1::Retiring
            || terminal.market_instance_id != self.binding.market_instance_id
            || terminal.series_plan_id != self.binding.series_plan_id
            || terminal.ordinal != self.binding.ordinal
            || terminal.generation != self.binding.generation
            || terminal.binding_id != self.binding.id()?
        {
            return Err(Error::UnauthenticatedAuthority);
        }
        let index = terminal.family.index();
        if self.counts.live[index] == 0
            || terminal.family_terminal_sequence != self.counts.terminal[index]
            || terminal.root_transition_sequence
                != self
                    .transition_sequence
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?
        {
            return Err(Error::WorkStateMismatch);
        }
        let mut other = 0usize;
        while other < self.last_terminal_receipts.len() {
            if self.last_terminal_receipts[other] == terminal.id {
                return Err(Error::UnauthenticatedAuthority);
            }
            other += 1;
        }
        let mut counts = self.counts;
        counts.live[index] = counts.live[index]
            .checked_sub(1)
            .ok_or(Error::ArithmeticOverflow)?;
        counts.terminal[index] = counts.terminal[index]
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut receipts = self.last_terminal_receipts;
        receipts[index] = terminal.id;
        let fractional_terminal_state_ids =
            if terminal.family == ProductOccurrenceFamilyV1::Fractional {
                if self.fractional_terminal_state_ids != [ContentId::ZERO; 2] {
                    return Err(Error::WorkStateMismatch);
                }
                terminal.terminal_state_ids
            } else {
                self.fractional_terminal_state_ids
            };
        let next = Self {
            transition_sequence: terminal.root_transition_sequence,
            counts,
            last_terminal_receipts: receipts,
            fractional_terminal_state_ids,
            ..self
        };
        next.validate()?;
        Ok(next)
    }

    /// Seal the pure root and emit its whole-Market terminal projection.
    pub fn finalize_terminal(self) -> Result<(Self, MarketInstanceTerminalProjectionV1)> {
        self.validate()?;
        if self.phase != ProductOccurrencePhaseV1::Retiring || !self.counts.complete()? {
            return Err(Error::WorkIncomplete);
        }
        let next = Self {
            phase: ProductOccurrencePhaseV1::Terminal,
            transition_sequence: self
                .transition_sequence
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            ..self
        };
        next.validate()?;
        let root_semantic_id = next.semantic_id()?;
        let id = content_id(
            MARKET_INSTANCE_TERMINAL_PROJECTION_DOMAIN_V1,
            &terminal_projection_preimage(
                root_semantic_id,
                next.binding,
                next.transition_sequence,
                next.last_terminal_receipts,
                next.fractional_terminal_state_ids,
            ),
        );
        id.validate()?;
        let projection = MarketInstanceTerminalProjectionV1 {
            id,
            root_semantic_id,
            binding: next.binding,
            final_transition_sequence: next.transition_sequence,
            family_terminal_receipts: next.last_terminal_receipts,
            fractional_terminal_state_ids: next.fractional_terminal_state_ids,
        };
        Ok((next, projection))
    }

    /// Complete immutable identity binding.
    pub const fn binding(self) -> ProductOccurrenceBindingV1 {
        self.binding
    }

    /// Immutable disjoint principal and donation floors capitalized at creation.
    pub const fn capitalization(self) -> ProductOccurrenceCapitalizationV1 {
        self.capitalization
    }

    /// Current monotone lifecycle phase.
    pub const fn phase(self) -> ProductOccurrencePhaseV1 {
        self.phase
    }

    /// Exact root transition sequence.
    pub const fn transition_sequence(self) -> u64 {
        self.transition_sequence
    }

    /// Exact expected/live/terminal family-boundary counts.
    pub const fn counts(self) -> ProductOccurrenceCountsV1 {
        self.counts
    }

    /// Last consumed terminal receipt for each family, or zero before the first.
    pub const fn last_terminal_receipts(self) -> [ContentId; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1] {
        self.last_terminal_receipts
    }

    /// Exact terminal state IDs of the closed Fractional a4 policy and a5 ledger.
    pub const fn fractional_terminal_state_ids(self) -> [ContentId; 2] {
        self.fractional_terminal_state_ids
    }

    /// Domain-separated identity of the complete mutable state image.
    pub fn semantic_id(self) -> Result<ContentId> {
        let mut body = [0; PRODUCT_OCCURRENCE_ROOT_BYTES_V1];
        self.encode_into(&mut body)?;
        let id = content_id(PRODUCT_OCCURRENCE_ROOT_DOMAIN_V1, &body);
        id.validate()?;
        Ok(id)
    }

    fn validate(self) -> Result<()> {
        self.binding.validate()?;
        self.capitalization.validate()?;
        self.counts.validate()?;
        match self.phase {
            ProductOccurrencePhaseV1::Active => {
                if self.transition_sequence != 0
                    || self.counts.terminal.iter().any(|count| *count != 0)
                    || self
                        .last_terminal_receipts
                        .iter()
                        .any(|receipt| *receipt != ContentId::ZERO)
                {
                    return Err(Error::WorkStateMismatch);
                }
            }
            ProductOccurrencePhaseV1::Retiring => {
                if self.transition_sequence == 0 {
                    return Err(Error::WorkStateMismatch);
                }
            }
            ProductOccurrencePhaseV1::Terminal => {
                if self.transition_sequence == 0 || !self.counts.complete()? {
                    return Err(Error::WorkStateMismatch);
                }
            }
        }
        let mut index = 0usize;
        while index < PRODUCT_OCCURRENCE_FAMILY_COUNT_V1 {
            let receipt = self.last_terminal_receipts[index];
            if (self.counts.terminal[index] == 0) != (receipt == ContentId::ZERO) {
                return Err(Error::WorkStateMismatch);
            }
            index += 1;
        }
        let fractional_terminal =
            self.counts.terminal[ProductOccurrenceFamilyV1::Fractional.index()] == 1;
        let fractional_absent = self.fractional_terminal_state_ids == [ContentId::ZERO; 2];
        let fractional_closed = self.fractional_terminal_state_ids[0].validate().is_ok()
            && self.fractional_terminal_state_ids[1].validate().is_ok()
            && self.fractional_terminal_state_ids[0] != self.fractional_terminal_state_ids[1];
        if (!fractional_terminal && !fractional_absent)
            || (fractional_terminal && !fractional_absent && !fractional_closed)
        {
            return Err(Error::WorkStateMismatch);
        }
        Ok(())
    }
}

impl FixedCodec for ProductOccurrenceRootV1 {
    const ENCODED_LEN: usize = PRODUCT_OCCURRENCE_ROOT_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&ROOT_MAGIC_V1);
        writer.u16(ROOT_VERSION_V1);
        writer.u8(self.phase.byte());
        writer.reserved(5);
        write_binding_ids(&mut writer, self.binding);
        writer.u32(self.binding.ordinal);
        writer.reserved(4);
        writer.u64(self.binding.generation);
        writer.u64(self.binding.source_repair_generation);
        writer.u64(self.binding.maximum_interval_width);
        writer.u16(self.binding.maximum_coordinates_per_advance);
        writer.reserved(6);
        writer.u64(self.transition_sequence);
        for amount in self.capitalization.principal_lamports {
            writer.u64(amount);
        }
        for amount in self.capitalization.donation_floor_lamports {
            writer.u64(amount);
        }
        for counts in [self.counts.expected, self.counts.live, self.counts.terminal] {
            for count in counts {
                writer.u32(count);
            }
        }
        for receipt in self.last_terminal_receipts {
            writer.id(receipt);
        }
        for state_id in self.fractional_terminal_state_ids {
            writer.id(state_id);
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&ROOT_MAGIC_V1)?;
        if reader.u16() != ROOT_VERSION_V1 {
            return Err(Error::BadVersion);
        }
        let phase = ProductOccurrencePhaseV1::decode(reader.u8())?;
        reader.reserved(5)?;
        let mut binding = read_binding_ids(&mut reader);
        binding.ordinal = reader.u32();
        reader.reserved(4)?;
        binding.generation = reader.u64();
        binding.source_repair_generation = reader.u64();
        binding.maximum_interval_width = reader.u64();
        binding.maximum_coordinates_per_advance = reader.u16();
        reader.reserved(6)?;
        let transition_sequence = reader.u64();
        let mut principal_lamports = [0; 4];
        let mut donation_floor_lamports = [0; 4];
        for amount in &mut principal_lamports {
            *amount = reader.u64();
        }
        for amount in &mut donation_floor_lamports {
            *amount = reader.u64();
        }
        let mut expected = [0; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1];
        let mut live = [0; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1];
        let mut terminal = [0; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1];
        for counts in [&mut expected, &mut live, &mut terminal] {
            for count in counts {
                *count = reader.u32();
            }
        }
        let mut last_terminal_receipts = [ContentId::ZERO; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1];
        for receipt in &mut last_terminal_receipts {
            *receipt = reader.id();
        }
        let fractional_terminal_state_ids = [reader.id(), reader.id()];
        reader.finish()?;
        let value = Self {
            binding,
            capitalization: ProductOccurrenceCapitalizationV1 {
                principal_lamports,
                donation_floor_lamports,
            },
            phase,
            transition_sequence,
            counts: ProductOccurrenceCountsV1 {
                expected,
                live,
                terminal,
            },
            last_terminal_receipts,
            fractional_terminal_state_ids,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Pure whole-Market terminal projection emitted only by a terminal root.
///
/// The live SBF adapter must authenticate the exact persisted root account and
/// wrap this value in its non-decodable private capability before any external
/// lifecycle owner consumes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketInstanceTerminalProjectionV1 {
    id: ContentId,
    root_semantic_id: ContentId,
    binding: ProductOccurrenceBindingV1,
    final_transition_sequence: u64,
    family_terminal_receipts: [ContentId; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1],
    fractional_terminal_state_ids: [ContentId; 2],
}

impl MarketInstanceTerminalProjectionV1 {
    /// Canonical pure projection identity.
    pub const fn id(self) -> ContentId {
        self.id
    }

    /// Exact terminal ProductOccurrenceRoot semantic identity.
    pub const fn root_semantic_id(self) -> ContentId {
        self.root_semantic_id
    }

    /// Full immutable Product/Series/Source occurrence binding.
    pub const fn binding(self) -> ProductOccurrenceBindingV1 {
        self.binding
    }

    /// Exact terminal transition sequence.
    pub const fn final_transition_sequence(self) -> u64 {
        self.final_transition_sequence
    }

    /// Exact last family-boundary terminal receipt identities.
    pub const fn family_terminal_receipts(self) -> [ContentId; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1] {
        self.family_terminal_receipts
    }

    /// Exact terminal state IDs of Fractional's closed a4 policy and a5 ledger.
    pub const fn fractional_terminal_state_ids(self) -> [ContentId; 2] {
        self.fractional_terminal_state_ids
    }
}

fn write_binding_ids(writer: &mut Writer<'_>, binding: ProductOccurrenceBindingV1) {
    writer.id(binding.market_instance_id.content_id());
    writer.id(binding.series_plan_id.content_id());
    for id in binding.content_ids() {
        writer.id(id);
    }
}

fn read_binding_ids(reader: &mut Reader<'_>) -> ProductOccurrenceBindingV1 {
    ProductOccurrenceBindingV1 {
        market_instance_id: MarketInstanceV2Id::from_bytes(reader.id().bytes()),
        series_plan_id: SeriesPlanV5Id::from_bytes(reader.id().bytes()),
        ordinal: 0,
        generation: 0,
        product_template_id: reader.id(),
        native_claim_basis_id: reader.id(),
        recovery_policy_id: reader.id(),
        price_measure_policy_id: reader.id(),
        market_genesis_profile_id: reader.id(),
        funding_terms_id: reader.id(),
        funding_quote_id: reader.id(),
        attachment_plan_id: reader.id(),
        compiler_output_id: reader.id(),
        source_occurrence_account_id: reader.id(),
        source_occurrence_account_authentication_id: reader.id(),
        source_occurrence_receipt_id: reader.id(),
        source_release_manifest_id: reader.id(),
        source_route_id: reader.id(),
        source_clock_policy_id: reader.id(),
        source_plane_contract_id: reader.id(),
        source_spec_id: reader.id(),
        window_spec_id: reader.id(),
        statistic_key_id: reader.id(),
        source_repair_generation: 0,
        failure_interval_work_account_id: reader.id(),
        failure_interval_replay_account_id: reader.id(),
        resolution_account_id: reader.id(),
        failure_policy_binding_id: reader.id(),
        recovery_state_id: reader.id(),
        interval_consensus_profile_id: reader.id(),
        maximum_interval_width: 0,
        maximum_coordinates_per_advance: 0,
        registry_release_id: reader.id(),
        capability_profile_id: reader.id(),
        rent_payer: reader.id(),
        neutral_lamport_sink: reader.id(),
        source_occurrence_id: SourceOccurrenceV1Id::from_bytes(reader.id().bytes()),
    }
}

fn family_terminal_preimage(
    family: ProductOccurrenceFamilyV1,
    disposition: ProductOccurrenceFamilyDispositionV1,
    binding: ProductOccurrenceBindingV1,
    family_terminal_sequence: u32,
    root_transition_sequence: u64,
    owner_program_id: ContentId,
    owner_release_id: ContentId,
    owner_account_id: ContentId,
    owner_terminal_receipt_id: ContentId,
    binding_id: ContentId,
    terminal_state_ids: [ContentId; 2],
) -> [u8; 314] {
    let mut bytes = [0; 314];
    bytes[0] = family.byte();
    bytes[1] = disposition.byte();
    bytes[2..34].copy_from_slice(&binding.market_instance_id.bytes());
    bytes[34..66].copy_from_slice(&binding.series_plan_id.bytes());
    bytes[66..70].copy_from_slice(&binding.ordinal.to_le_bytes());
    bytes[70..78].copy_from_slice(&binding.generation.to_le_bytes());
    bytes[78..82].copy_from_slice(&family_terminal_sequence.to_le_bytes());
    bytes[82..90].copy_from_slice(&root_transition_sequence.to_le_bytes());
    bytes[90..122].copy_from_slice(&owner_program_id.bytes());
    bytes[122..154].copy_from_slice(&owner_release_id.bytes());
    bytes[154..186].copy_from_slice(&owner_account_id.bytes());
    bytes[186..218].copy_from_slice(&owner_terminal_receipt_id.bytes());
    bytes[218..250].copy_from_slice(&binding_id.bytes());
    bytes[250..282].copy_from_slice(&terminal_state_ids[0].bytes());
    bytes[282..314].copy_from_slice(&terminal_state_ids[1].bytes());
    bytes
}

fn terminal_projection_preimage(
    root_semantic_id: ContentId,
    binding: ProductOccurrenceBindingV1,
    final_transition_sequence: u64,
    receipts: [ContentId; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1],
    fractional_terminal_state_ids: [ContentId; 2],
) -> [u8; 500] {
    let mut bytes = [0; 500];
    bytes[..32].copy_from_slice(&root_semantic_id.bytes());
    bytes[32..64].copy_from_slice(&binding.market_instance_id.bytes());
    bytes[64..96].copy_from_slice(&binding.series_plan_id.bytes());
    bytes[96..100].copy_from_slice(&binding.ordinal.to_le_bytes());
    bytes[100..108].copy_from_slice(&binding.generation.to_le_bytes());
    bytes[108..116].copy_from_slice(&final_transition_sequence.to_le_bytes());
    let mut at = 116usize;
    for receipt in receipts {
        bytes[at..at + 32].copy_from_slice(&receipt.bytes());
        at += 32;
    }
    bytes[436..468].copy_from_slice(&fractional_terminal_state_ids[0].bytes());
    bytes[468..500].copy_from_slice(&fractional_terminal_state_ids[1].bytes());
    bytes
}
