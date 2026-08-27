use clutch_source_plane_v3::{
    CompiledInstanceV3, ContentId, DrawdownSummaryV3, LiquidityEnvelopeV3, OpenRawPageV3,
    PartitionViewV3, PayoutTableV3, ProductTemplateV3, RawPageV3, RawRecordV3, SeriesFundingV3,
    SeriesPlanV3, SourceHeadV3, SourcePlaneProgramV3, StatisticKeyV3, StatisticKindV3,
    StatisticResultV3, SummaryProgramV3, WindowClosureReceiptV3, WindowSealV3, WindowSpecV3,
    WindowWorkV3,
    WorkEnvelopeV3, DRAWDOWN_SUMMARY_BYTES, INSTANCE_DESCRIPTOR_BYTES, OPEN_RAW_PAGE_BYTES,
    SERIES_FUNDING_BYTES, SERIES_PLAN_BYTES, SOURCE_HEAD_BYTES, STATISTIC_RESULT_BYTES,
    WINDOW_SEAL_BYTES, WINDOW_WORK_BYTES,
};
use clutch_terminal_identity_v1::{Id, TerminalAccountV1};
use sha2::{Digest, Sha256};

use crate::account::{
    canonical_account_state_digest, AccountBodyV3, AccountFamilyV3, AccountHeaderV3,
};
use crate::pda::PdaRecipeV3;
use crate::v2::V2AuthenticatedRecord;
use crate::{Error, Result};

const TRANSITION_DOMAIN: &[u8] = b"dragons-clutch/source-plane-v3/transition/v1";
const TRANSITION_MAGIC: [u8; 8] = *b"DCSP3TRN";
const TRANSITION_VERSION: u16 = 1;
const STATE_BYTES: usize = 80;
const MUTATION_BYTES: usize = STATE_BYTES * 2;
const CREATION_BYTES: usize = STATE_BYTES + 32 + 32;
const CLOSURE_BYTES: usize = STATE_BYTES + 32 + 8 + 32 + 8;

/// Maximum mutable accounts projected by one boundary transaction.
pub const MAX_MUTATIONS: usize = 2;
/// Maximum account creations projected by one boundary transaction.
pub const MAX_CREATIONS: usize = 4;
/// Maximum terminal closes projected by one boundary transaction.
pub const MAX_CLOSES: usize = 2;
/// Exact fixed transition-plan preimage bytes.
pub const TRANSITION_PLAN_BYTES: usize = 64
    + MAX_MUTATIONS * MUTATION_BYTES
    + MAX_CREATIONS * CREATION_BYTES
    + MAX_CLOSES * CLOSURE_BYTES;

/// Closed proposed transition registry. These are not live dispatcher tags.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum TransitionActionV3 {
    /// Create one canonical source-head generation.
    InitializeSourceHead = 1,
    /// Create mutable page work from the current head snapshot.
    OpenRawPage = 2,
    /// Append exactly one authenticated boundary.
    AppendBoundary = 3,
    /// Freeze one raw page and atomically advance its source head.
    SealRawPage = 4,
    /// Create one predictable WindowWork cursor.
    CreateWindowWork = 5,
    /// Fold one immutable raw page into WindowWork.
    FoldWindowPage = 6,
    /// Close WindowWork and create its immutable WindowSeal.
    SealWindow = 7,
    /// Create a terminal-interval result at StatisticKey.
    WriteTerminalResult = 8,
    /// Create a maximum-drawdown result at StatisticKey.
    WriteDrawdownResult = 9,
    /// Create an immutable SeriesPlan and exact prepaid funding state.
    ActivateSeries = 10,
    /// Instantiate and debit exactly the next Series ordinal.
    CreateSeriesInstance = 11,
    /// Advance one expired ordinal without spending its compartments.
    LapseSeriesOrdinal = 12,
    /// Advance over an independently existing convergent Instance.
    AdvanceExistingInstance = 13,
}

impl TransitionActionV3 {
    /// Decode exact action versions only.
    pub fn decode(value: u16) -> Result<Self> {
        match value {
            1 => Ok(Self::InitializeSourceHead),
            2 => Ok(Self::OpenRawPage),
            3 => Ok(Self::AppendBoundary),
            4 => Ok(Self::SealRawPage),
            5 => Ok(Self::CreateWindowWork),
            6 => Ok(Self::FoldWindowPage),
            7 => Ok(Self::SealWindow),
            8 => Ok(Self::WriteTerminalResult),
            9 => Ok(Self::WriteDrawdownResult),
            10 => Ok(Self::ActivateSeries),
            11 => Ok(Self::CreateSeriesInstance),
            12 => Ok(Self::LapseSeriesOrdinal),
            13 => Ok(Self::AdvanceExistingInstance),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// One exact promoted account state named by family, semantic PDA recipe,
/// terminal generation, and canonical full account-image digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountStateV3 {
    family: u16,
    binding_id: ContentId,
    state_digest: ContentId,
    generation: u64,
}

impl AccountStateV3 {
    const ZERO: Self = Self {
        family: 0,
        binding_id: ContentId::ZERO,
        state_digest: ContentId::ZERO,
        generation: 0,
    };

    /// Account family.
    pub fn family(self) -> Result<AccountFamilyV3> {
        AccountFamilyV3::decode(self.family)
    }

    /// Digest of the exact PDA seed recipe proposal.
    pub const fn binding_id(self) -> ContentId {
        self.binding_id
    }

    /// Digest of the exact promoted runtime envelope and semantic core body.
    pub const fn state_digest(self) -> ContentId {
        self.state_digest
    }

    /// Shared terminal close/reopen generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    fn new(
        family: AccountFamilyV3,
        binding_id: ContentId,
        state_digest: ContentId,
        generation: u64,
    ) -> Result<Self> {
        if binding_id.is_zero() || state_digest.is_zero() || generation == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(Self {
            family: family.word(),
            binding_id,
            state_digest,
            generation,
        })
    }

    fn validate(&self) -> Result<()> {
        self.family()?;
        if self.binding_id.is_zero() || self.state_digest.is_zero() || self.generation == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    fn encode(self, output: &mut [u8]) {
        output.fill(0);
        output[..2].copy_from_slice(&self.family.to_le_bytes());
        output[8..40].copy_from_slice(&self.binding_id.bytes());
        output[40..72].copy_from_slice(&self.state_digest.bytes());
        output[72..80].copy_from_slice(&self.generation.to_le_bytes());
    }
}

/// One compare-and-swap mutation of an existing account generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateMutationV3 {
    /// Exact before-state.
    pub before: AccountStateV3,
    /// Exact after-state at the same PDA and terminal generation.
    pub after: AccountStateV3,
}

/// Exact TerminalIdentity observation for one mutation.
///
/// Construction delegates donation accounting to `TerminalAccountV1`; callers
/// cannot choose an arbitrary after-header or decrease the donation floor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountMutationV3 {
    before_header: AccountHeaderV3,
    after_header: AccountHeaderV3,
    neutral_sink: Id,
    accounted_lamports: u64,
}

impl AccountMutationV3 {
    /// Observe the actual post-transition balance against exact accounted
    /// lamports and derive the only admissible after-header.
    pub fn observe(
        before_header: AccountHeaderV3,
        neutral_sink: Id,
        actual_balance_lamports: u64,
        accounted_lamports: u64,
    ) -> Result<Self> {
        let account = reconstruct_terminal(before_header, neutral_sink)?;
        let observed = account.observe_transition(actual_balance_lamports, accounted_lamports)?;
        Ok(Self {
            before_header,
            after_header: AccountHeaderV3 {
                terminal: observed.header(),
                ..before_header
            },
            neutral_sink,
            accounted_lamports,
        })
    }

    /// Stored before-header committed by compare-and-swap.
    pub const fn before_header(self) -> AccountHeaderV3 {
        self.before_header
    }

    /// Kernel-derived after-header with monotone donation floor.
    pub const fn after_header(self) -> AccountHeaderV3 {
        self.after_header
    }

    fn validate_accounted(self, expected: u64) -> Result<()> {
        if self.accounted_lamports != expected {
            return Err(Error::MismatchedState);
        }
        Ok(())
    }
}

/// Exact once-only terminal split derived by `TerminalAccountV1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountCloseV3 {
    header: AccountHeaderV3,
    neutral_sink: Id,
    neutral_surplus_lamports: u64,
}

impl AccountCloseV3 {
    /// Observe `actual_balance_lamports`, require every non-rent compartment to
    /// have been disposed, and derive the exact payer/sink split.
    pub fn close(
        header: AccountHeaderV3,
        neutral_sink: Id,
        actual_balance_lamports: u64,
    ) -> Result<Self> {
        let account = reconstruct_terminal(header, neutral_sink)?;
        let (_, split) = account.close(actual_balance_lamports, header.terminal.payer_principal)?;
        if split.payer != header.terminal.payer
            || split.payer_principal != header.terminal.payer_principal
            || split.sink != neutral_sink
        {
            return Err(Error::MismatchedState);
        }
        Ok(Self {
            header,
            neutral_sink,
            neutral_surplus_lamports: split.neutral_surplus,
        })
    }

    /// Exact neutral sink selected by the shared terminal identity policy.
    pub const fn neutral_sink(self) -> Id {
        self.neutral_sink
    }

    /// Exact unsolicited surplus routed to the neutral sink.
    pub const fn neutral_surplus_lamports(self) -> u64 {
        self.neutral_surplus_lamports
    }
}

impl StateMutationV3 {
    const ZERO: Self = Self {
        before: AccountStateV3::ZERO,
        after: AccountStateV3::ZERO,
    };

    fn validate(&self) -> Result<()> {
        self.before.validate()?;
        self.after.validate()?;
        if self.before.family != self.after.family
            || self.before.binding_id != self.after.binding_id
            || self.before.generation != self.after.generation
            || self.before.state_digest == self.after.state_digest
        {
            return Err(Error::MismatchedState);
        }
        Ok(())
    }
}

/// One absent-target creation with exact segregated debits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountCreationV3 {
    /// Exact created semantic state.
    pub state: AccountStateV3,
    /// Exact funding wallet recorded by TerminalIdentity.
    pub payer: ContentId,
    /// Exact rent principal; never a fee or work reserve.
    pub rent_principal_lamports: u64,
    /// Exact creation budget moved into independently owned downstream state.
    pub creation_budget_lamports: u64,
    /// Exact independently prepaid mandatory work.
    pub prepaid_work_lamports: u64,
    /// Exact funded-liquidity collateral; never liveness capitalization.
    pub liquidity_collateral: u64,
}

impl AccountCreationV3 {
    const ZERO: Self = Self {
        state: AccountStateV3::ZERO,
        payer: ContentId::ZERO,
        rent_principal_lamports: 0,
        creation_budget_lamports: 0,
        prepaid_work_lamports: 0,
        liquidity_collateral: 0,
    };

    fn validate(&self) -> Result<()> {
        self.state.validate()?;
        // A fully prefunded PDA has no transaction-payer principal and must
        // therefore carry the zero payer identity.  Conversely, any positive
        // rent shortfall must name the exact payer.  Treating a prefund as
        // caller principal would manufacture a refund authority that does not
        // exist in the runtime funding ledger.
        if self.payer.is_zero() != (self.rent_principal_lamports == 0) {
            return Err(Error::InvalidParameter);
        }
        // These compartments have distinct units and ownership. In
        // particular, collateral atoms are not lamports and must never be
        // added to a rent/work budget as an overflow proxy.
        self.rent_principal_lamports
            .checked_add(self.creation_budget_lamports)
            .and_then(|value| value.checked_add(self.prepaid_work_lamports))
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }
}

/// One terminal close with exact principal owner. Any surplus remains governed
/// by the shared terminal identity and is not represented as principal here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountClosureV3 {
    /// Exact state being closed.
    pub state: AccountStateV3,
    /// Stored TerminalIdentity payer and sole principal recipient.
    pub principal_recipient: ContentId,
    /// Exact stored payer principal.
    pub payer_principal_lamports: u64,
    /// Frozen neutral sink receiving every unsolicited lamport.
    pub neutral_sink: ContentId,
    /// Exact observed balance above payer principal.
    pub neutral_surplus_lamports: u64,
}

impl AccountClosureV3 {
    const ZERO: Self = Self {
        state: AccountStateV3::ZERO,
        principal_recipient: ContentId::ZERO,
        payer_principal_lamports: 0,
        neutral_sink: ContentId::ZERO,
        neutral_surplus_lamports: 0,
    };

    fn validate(&self) -> Result<()> {
        self.state.validate()?;
        if self.principal_recipient.is_zero() != (self.payer_principal_lamports == 0)
            || self.neutral_sink.is_zero()
            || (!self.principal_recipient.is_zero()
                && self.principal_recipient == self.neutral_sink)
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
}

/// Exact promoted-runtime observation of one in-place account mutation.
///
/// This is a projection input, not authentication authority. The SBF adapter
/// must populate it only from an authenticated runtime account and the atomic
/// compare-and-swap result produced by that same instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeMutationProjectionV1 {
    /// Digest of the complete authenticated account preimage.
    pub account_data_before_id: ContentId,
    /// Digest of the complete persisted account postimage.
    pub account_data_after_id: ContentId,
    /// Durable terminal/reopen generation, unchanged by the mutation.
    pub generation: u64,
}

/// Exact promoted-runtime observation of one account creation.
///
/// A fully prefunded PDA has a zero payer and zero payer-funded principal;
/// existing lamports remain neutral donation and never create refund rights.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCreationProjectionV1 {
    /// Digest of the complete persisted account postimage.
    pub account_data_id: ContentId,
    /// Durable terminal/reopen generation of the new account.
    pub generation: u64,
    /// Exact rent-principal payer, or zero for a fully prefunded PDA.
    pub payer: ContentId,
    /// Exact payer-funded rent principal.
    pub rent_principal_lamports: u64,
}

/// Exact promoted-runtime observation of one once-only account close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCloseProjectionV1 {
    /// Digest of the complete authenticated account preimage.
    pub account_data_id: ContentId,
    /// Closed durable terminal/reopen generation.
    pub generation: u64,
    /// Exact principal recipient, or zero for a fully prefunded generation.
    pub principal_recipient: ContentId,
    /// Exact payer principal returned on close.
    pub payer_principal_lamports: u64,
    /// Frozen neutral sink receiving every surplus lamport.
    pub neutral_sink: ContentId,
    /// Exact balance above payer principal.
    pub neutral_surplus_lamports: u64,
}

/// Canonical fixed-capacity pure transition projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionPlanV3 {
    action: TransitionActionV3,
    evidence_digest: ContentId,
    mutation_count: u8,
    creation_count: u8,
    close_count: u8,
    current_bucket: u64,
    requested_ordinal: u32,
    mutations: [StateMutationV3; MAX_MUTATIONS],
    creations: [AccountCreationV3; MAX_CREATIONS],
    closes: [AccountClosureV3; MAX_CLOSES],
}

impl TransitionPlanV3 {
    fn new(action: TransitionActionV3, evidence_digest: ContentId) -> Self {
        Self {
            action,
            evidence_digest,
            mutation_count: 0,
            creation_count: 0,
            close_count: 0,
            current_bucket: 0,
            requested_ordinal: 0,
            mutations: [StateMutationV3::ZERO; MAX_MUTATIONS],
            creations: [AccountCreationV3::ZERO; MAX_CREATIONS],
            closes: [AccountClosureV3::ZERO; MAX_CLOSES],
        }
    }

    /// Projected action.
    pub const fn action(self) -> TransitionActionV3 {
        self.action
    }

    /// Authentication/evaluation/existing-state transcript commitment.
    pub const fn evidence_digest(self) -> ContentId {
        self.evidence_digest
    }

    /// Number of active mutations.
    pub const fn mutation_count(self) -> u8 {
        self.mutation_count
    }

    /// Number of active creations.
    pub const fn creation_count(self) -> u8 {
        self.creation_count
    }

    /// Number of active closes.
    pub const fn close_count(self) -> u8 {
        self.close_count
    }

    /// Adapter-clock bucket committed by a Series cursor transition.
    pub const fn current_bucket(self) -> u64 {
        self.current_bucket
    }

    /// Durable ordinal expected by a Series cursor transition.
    pub const fn requested_ordinal(self) -> u32 {
        self.requested_ordinal
    }

    /// Active mutation by index.
    pub fn mutation(self, index: usize) -> Result<StateMutationV3> {
        if index >= usize::from(self.mutation_count) {
            return Err(Error::InvalidParameter);
        }
        Ok(self.mutations[index])
    }

    /// Active creation by index.
    pub fn creation(self, index: usize) -> Result<AccountCreationV3> {
        if index >= usize::from(self.creation_count) {
            return Err(Error::InvalidParameter);
        }
        Ok(self.creations[index])
    }

    /// Active close by index.
    pub fn close(self, index: usize) -> Result<AccountClosureV3> {
        if index >= usize::from(self.close_count) {
            return Err(Error::InvalidParameter);
        }
        Ok(self.closes[index])
    }

    /// Validate action shape, active entries, and exact inactive padding.
    pub fn validate(&self) -> Result<()> {
        let mutation_count = usize::from(self.mutation_count);
        let creation_count = usize::from(self.creation_count);
        let close_count = usize::from(self.close_count);
        if mutation_count > MAX_MUTATIONS
            || creation_count > MAX_CREATIONS
            || close_count > MAX_CLOSES
        {
            return Err(Error::InvalidParameter);
        }
        let expected = match self.action {
            TransitionActionV3::InitializeSourceHead => (0, 1, 0, true),
            TransitionActionV3::OpenRawPage => (0, 1, 0, false),
            TransitionActionV3::AppendBoundary => (1, 0, 0, true),
            TransitionActionV3::SealRawPage => (1, 1, 1, true),
            TransitionActionV3::CreateWindowWork => (0, 1, 0, true),
            TransitionActionV3::FoldWindowPage => (1, 0, 0, true),
            TransitionActionV3::SealWindow => (0, 1, 1, true),
            TransitionActionV3::WriteTerminalResult => (0, 1, 0, true),
            TransitionActionV3::WriteDrawdownResult => (0, 1, 0, true),
            TransitionActionV3::ActivateSeries => (0, 2, 0, true),
            TransitionActionV3::CreateSeriesInstance => (1, 1, 0, true),
            TransitionActionV3::LapseSeriesOrdinal => (1, 0, 0, false),
            TransitionActionV3::AdvanceExistingInstance => (1, 0, 0, true),
        };
        if (mutation_count, creation_count, close_count) != (expected.0, expected.1, expected.2)
            || (expected.3 == self.evidence_digest.is_zero())
        {
            return Err(Error::InvalidParameter);
        }
        let is_series_cursor = matches!(
            self.action,
            TransitionActionV3::CreateSeriesInstance
                | TransitionActionV3::LapseSeriesOrdinal
                | TransitionActionV3::AdvanceExistingInstance
        );
        if !is_series_cursor && (self.current_bucket != 0 || self.requested_ordinal != 0) {
            return Err(Error::NonCanonicalPadding);
        }
        for index in 0..MAX_MUTATIONS {
            if index < mutation_count {
                self.mutations[index].validate()?;
            } else if self.mutations[index] != StateMutationV3::ZERO {
                return Err(Error::NonCanonicalPadding);
            }
        }
        for index in 0..MAX_CREATIONS {
            if index < creation_count {
                self.creations[index].validate()?;
            } else if self.creations[index] != AccountCreationV3::ZERO {
                return Err(Error::NonCanonicalPadding);
            }
        }
        for index in 0..MAX_CLOSES {
            if index < close_count {
                self.closes[index].validate()?;
            } else if self.closes[index] != AccountClosureV3::ZERO {
                return Err(Error::NonCanonicalPadding);
            }
        }
        Ok(())
    }

    /// Encode the exact fixed transition preimage.
    pub fn encode(self) -> Result<[u8; TRANSITION_PLAN_BYTES]> {
        self.validate()?;
        let mut output = [0; TRANSITION_PLAN_BYTES];
        output[..8].copy_from_slice(&TRANSITION_MAGIC);
        output[8..10].copy_from_slice(&TRANSITION_VERSION.to_le_bytes());
        output[10..12].copy_from_slice(&(self.action as u16).to_le_bytes());
        output[12] = self.mutation_count;
        output[13] = self.creation_count;
        output[14] = self.close_count;
        output[16..48].copy_from_slice(&self.evidence_digest.bytes());
        output[48..56].copy_from_slice(&self.current_bucket.to_le_bytes());
        output[56..60].copy_from_slice(&self.requested_ordinal.to_le_bytes());
        let mut at = 64;
        for mutation in self.mutations {
            mutation.before.encode(&mut output[at..at + STATE_BYTES]);
            at += STATE_BYTES;
            mutation.after.encode(&mut output[at..at + STATE_BYTES]);
            at += STATE_BYTES;
        }
        for creation in self.creations {
            creation.state.encode(&mut output[at..at + STATE_BYTES]);
            at += STATE_BYTES;
            output[at..at + 32].copy_from_slice(&creation.payer.bytes());
            at += 32;
            for amount in [
                creation.rent_principal_lamports,
                creation.creation_budget_lamports,
                creation.prepaid_work_lamports,
                creation.liquidity_collateral,
            ] {
                output[at..at + 8].copy_from_slice(&amount.to_le_bytes());
                at += 8;
            }
        }
        for close in self.closes {
            close.state.encode(&mut output[at..at + STATE_BYTES]);
            at += STATE_BYTES;
            output[at..at + 32].copy_from_slice(&close.principal_recipient.bytes());
            at += 32;
            output[at..at + 8].copy_from_slice(&close.payer_principal_lamports.to_le_bytes());
            at += 8;
            output[at..at + 32].copy_from_slice(&close.neutral_sink.bytes());
            at += 32;
            output[at..at + 8].copy_from_slice(&close.neutral_surplus_lamports.to_le_bytes());
            at += 8;
        }
        if at != TRANSITION_PLAN_BYTES {
            return Err(Error::WrongLength);
        }
        Ok(output)
    }

    /// Content identity of the complete state/economics projection.
    pub fn id(self) -> Result<ContentId> {
        let bytes = self.encode()?;
        let mut hasher = Sha256::new();
        hasher.update(TRANSITION_DOMAIN);
        hasher.update(bytes);
        Ok(ContentId::from_bytes(hasher.finalize().into()))
    }

    fn push_mutation(&mut self, value: StateMutationV3) -> Result<()> {
        let index = usize::from(self.mutation_count);
        if index >= MAX_MUTATIONS {
            return Err(Error::InvalidParameter);
        }
        self.mutations[index] = value;
        self.mutation_count += 1;
        Ok(())
    }

    fn push_creation(&mut self, value: AccountCreationV3) -> Result<()> {
        let index = usize::from(self.creation_count);
        if index >= MAX_CREATIONS {
            return Err(Error::InvalidParameter);
        }
        self.creations[index] = value;
        self.creation_count += 1;
        Ok(())
    }

    fn push_close(&mut self, value: AccountClosureV3) -> Result<()> {
        let index = usize::from(self.close_count);
        if index >= MAX_CLOSES {
            return Err(Error::InvalidParameter);
        }
        self.closes[index] = value;
        self.close_count += 1;
        Ok(())
    }

    fn set_series_context(&mut self, current_bucket: u64, requested_ordinal: u32) {
        self.current_bucket = current_bucket;
        self.requested_ordinal = requested_ordinal;
    }

    fn finish(self) -> Result<Self> {
        self.validate()?;
        Ok(self)
    }
}

/// Pure core output paired with the exact adapter transition projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreTransitionV3<T> {
    /// New semantic core value(s).
    pub output: T,
    /// Canonical account/economic plan.
    pub plan: TransitionPlanV3,
}

/// Exact immutable artifacts required by recurring lowering.
#[derive(Clone, Copy, Debug)]
pub struct SeriesBindingsV3<'a> {
    /// Reviewed SourcePlane release.
    pub source_plane: &'a SourcePlaneProgramV3,
    /// Reviewed Summary release.
    pub summary: &'a SummaryProgramV3,
    /// Exact payout table.
    pub payouts: &'a PayoutTableV3,
    /// Exact exhaustive/disjoint/ordered partition projection.
    pub partition: &'a PartitionViewV3,
    /// Reusable Template.
    pub template: &'a ProductTemplateV3,
    /// Prepaid work quote.
    pub work: &'a WorkEnvelopeV3,
    /// Funded-liquidity quote.
    pub liquidity: &'a LiquidityEnvelopeV3,
    /// Finite immutable Series schedule.
    pub series: &'a SeriesPlanV3,
}

/// Exact SourceHead genesis authorization projected from the full V2
/// SourceSpec/Terms/release route. Fields are private so an arbitrary nonzero
/// digest cannot authorize a squatted starting bucket or repair generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceGenesisAuthorizationV3 {
    source_plane_contract_id: ContentId,
    source_spec_id: ContentId,
    next_boundary_bucket: u64,
    repair_generation: u64,
    authorization_digest: ContentId,
}

impl SourceGenesisAuthorizationV3 {
    /// Existing SourceSpec identity selected for genesis.
    pub const fn source_spec_id(self) -> ContentId {
        self.source_spec_id
    }

    /// State-owned first boundary authorized by the route.
    pub const fn next_boundary_bucket(self) -> u64 {
        self.next_boundary_bucket
    }

    /// Exact repair generation authorized by the route.
    pub const fn repair_generation(self) -> u64 {
        self.repair_generation
    }
}

/// Opaque output reserved for a terminal evaluator that authenticates the
/// exact WindowSeal page chain and record-stream root.
///
/// No public constructor exists in this crate version. Therefore a structurally
/// valid caller interval plus an opaque nonzero hash cannot mint a result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalEvaluationV3 {
    low: u128,
    high: u128,
    evidence_digest: ContentId,
}

/// Opaque output reserved for a drawdown evaluator that authenticates the
/// exact WindowSeal page chain and record-stream root.
///
/// No public constructor exists until the resumable evaluator adapter lands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedDrawdownEvaluationV3 {
    summary: DrawdownSummaryV3,
    evidence_digest: ContentId,
}

/// Opaque proof that a WindowWork PDA has never been created or has a durable
/// lineage authorizing exactly the next generation.
///
/// No public constructor exists until the adapter owns a tombstone/lineage
/// registry. Plain account absence is intentionally insufficient.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowWorkLineageV3 {
    authorization_digest: ContentId,
}

/// Opaque runtime-authenticated immutable raw-page account.
///
/// The future live constructor must verify deployed program owner, PDA,
/// address, bump, exact account image, and reviewed SourcePlane contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRawPageV3 {
    page: RawPageV3,
    authentication_digest: ContentId,
}

/// Opaque runtime-authenticated convergent Instance account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedInstanceV3 {
    instance: CompiledInstanceV3,
    authentication_digest: ContentId,
}

/// Opaque checked typed transfer graph for Series activation.
///
/// It must bind debit sources, lamport destinations, the Realm-selected
/// collateral mint, vault ownership, and exact funded amounts. There is no
/// public constructor in this crate version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesActivationTransfersV3 {
    authorization_digest: ContentId,
}

/// Opaque checked typed transfer graph for one Series instantiation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesInstantiationTransfersV3 {
    authorization_digest: ContentId,
}

/// Create one source-head generation only under an explicit genesis-authority
/// transcript. The transcript must authenticate the exact start and repair
/// generation; a mere absent PDA is not genesis authority.
#[allow(clippy::too_many_arguments)]
pub fn project_initialize_source_head(
    source_plane: &SourcePlaneProgramV3,
    authorization: SourceGenesisAuthorizationV3,
    head_header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<CoreTransitionV3<SourceHeadV3>> {
    source_plane.validate()?;
    if source_plane.id()? != authorization.source_plane_contract_id {
        return Err(Error::MismatchedState);
    }
    let head = SourceHeadV3::new(
        authorization.source_spec_id,
        authorization.next_boundary_bucket,
        authorization.repair_generation,
    )?;
    let state = source_head_state(source_plane, &head, head_header, neutral_sink)?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::InitializeSourceHead,
        authorization.authorization_digest,
    );
    plan.push_creation(creation(state, head_header, 0, 0, 0))?;
    Ok(CoreTransitionV3 {
        output: head,
        plan: plan.finish()?,
    })
}

/// Recompute the action-2 transition commitment from the exact runtime
/// postimage and prefund-safe funding partition produced by the live adapter.
///
/// Unlike the historical research account envelope, the promoted runtime
/// permits a fully prefunded PDA.  Such a creation intentionally records no
/// payer and zero payer principal; every prefunded lamport remains neutral
/// donation.  The exact runtime account-data digest is committed directly so
/// an intent cannot authorize a different promoted account image.
#[allow(clippy::too_many_arguments)]
pub fn project_runtime_initialize_source_head(
    source_plane: &SourcePlaneProgramV3,
    head: &SourceHeadV3,
    semantic_binding_id: ContentId,
    runtime_account_data_id: ContentId,
    generation: u64,
    authorization_digest: ContentId,
    payer: ContentId,
    rent_principal_lamports: u64,
) -> Result<TransitionPlanV3> {
    source_plane.validate()?;
    head.validate()?;
    if semantic_binding_id.is_zero()
        || runtime_account_data_id.is_zero()
        || authorization_digest.is_zero()
        || generation == 0
        || payer.is_zero() != (rent_principal_lamports == 0)
    {
        return Err(Error::InvalidParameter);
    }
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::InitializeSourceHead,
        authorization_digest,
    );
    plan.push_creation(AccountCreationV3 {
        state: AccountStateV3::new(
            AccountFamilyV3::SourceHead,
            semantic_binding_id,
            runtime_account_data_id,
            generation,
        )?,
        payer,
        rent_principal_lamports,
        creation_budget_lamports: 0,
        prepaid_work_lamports: 0,
        liquidity_collateral: 0,
    })?;
    plan.finish()
}

/// Recompute the action-3 creation commitment from an authenticated SourceHead
/// and the exact promoted OpenRawPage postimage.
#[allow(clippy::too_many_arguments)]
pub fn project_runtime_open_raw_page(
    source_plane: &SourcePlaneProgramV3,
    head: &SourceHeadV3,
    open: &OpenRawPageV3,
    semantic_binding_id: ContentId,
    runtime_account_data_id: ContentId,
    generation: u64,
    payer: ContentId,
    rent_principal_lamports: u64,
) -> Result<TransitionPlanV3> {
    source_plane.validate()?;
    head.validate()?;
    open.validate_against_head(head)?;
    if semantic_binding_id.is_zero()
        || runtime_account_data_id.is_zero()
        || generation == 0
        || payer.is_zero() != (rent_principal_lamports == 0)
    {
        return Err(Error::InvalidParameter);
    }
    let mut plan = TransitionPlanV3::new(TransitionActionV3::OpenRawPage, ContentId::ZERO);
    plan.push_creation(AccountCreationV3 {
        state: AccountStateV3::new(
            AccountFamilyV3::OpenRawPage,
            semantic_binding_id,
            runtime_account_data_id,
            generation,
        )?,
        payer,
        rent_principal_lamports,
        creation_budget_lamports: 0,
        prepaid_work_lamports: 0,
        liquidity_collateral: 0,
    })?;
    plan.finish()
}

/// Recompute the action-4 mutation commitment from the exact authenticated
/// boundary and promoted OpenRawPage preimage/postimage digests.
#[allow(clippy::too_many_arguments)]
pub fn project_runtime_append_boundary(
    source_plane: &SourcePlaneProgramV3,
    head: &SourceHeadV3,
    open_before: &OpenRawPageV3,
    open_after: &OpenRawPageV3,
    record: RawRecordV3,
    semantic_binding_id: ContentId,
    runtime_account_data_before_id: ContentId,
    runtime_account_data_after_id: ContentId,
    generation: u64,
    boundary_authentication_id: ContentId,
) -> Result<TransitionPlanV3> {
    source_plane.validate()?;
    head.validate()?;
    open_before.validate_against_head(head)?;
    open_after.validate_against_head(head)?;
    let expected_after = open_before.append_observation(record)?;
    if expected_after != *open_after
        || semantic_binding_id.is_zero()
        || runtime_account_data_before_id.is_zero()
        || runtime_account_data_after_id.is_zero()
        || runtime_account_data_before_id == runtime_account_data_after_id
        || generation == 0
        || boundary_authentication_id.is_zero()
    {
        return Err(Error::InvalidParameter);
    }
    let before = AccountStateV3::new(
        AccountFamilyV3::OpenRawPage,
        semantic_binding_id,
        runtime_account_data_before_id,
        generation,
    )?;
    let after = AccountStateV3::new(
        AccountFamilyV3::OpenRawPage,
        semantic_binding_id,
        runtime_account_data_after_id,
        generation,
    )?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::AppendBoundary,
        boundary_authentication_id,
    );
    plan.push_mutation(StateMutationV3 { before, after })?;
    plan.finish()
}

/// Recompute action 5 from the exact promoted runtime observations.
///
/// The semantic PDA recipes are derived here from the reviewed SourcePlane
/// identity and the canonical core bodies. Callers cannot substitute binding
/// digests. The evidence digest is the atomic runtime receipt that binds the
/// authenticated head/open preimages to both semantic outputs.
#[allow(clippy::too_many_arguments)]
pub fn project_runtime_seal_raw_page(
    source_plane: &SourcePlaneProgramV3,
    head_before: &SourceHeadV3,
    open_before: &OpenRawPageV3,
    head_after: &SourceHeadV3,
    sealed_page: &RawPageV3,
    head_runtime: RuntimeMutationProjectionV1,
    open_runtime: RuntimeCloseProjectionV1,
    page_runtime: RuntimeCreationProjectionV1,
    transition_receipt_id: ContentId,
) -> Result<TransitionPlanV3> {
    source_plane.validate()?;
    head_before.validate()?;
    open_before.validate_against_head(head_before)?;
    sealed_page.validate()?;
    let expected_page = open_before.seal()?;
    let expected_head = head_before.commit_page(&expected_page)?;
    if expected_page != *sealed_page
        || expected_head != *head_after
        || transition_receipt_id.is_zero()
        || head_runtime.account_data_before_id.is_zero()
        || head_runtime.account_data_after_id.is_zero()
        || head_runtime.account_data_before_id == head_runtime.account_data_after_id
        || head_runtime.generation == 0
        || open_runtime.account_data_id.is_zero()
        || open_runtime.generation == 0
        || page_runtime.account_data_id.is_zero()
        || page_runtime.generation == 0
        || page_runtime.payer.is_zero() != (page_runtime.rent_principal_lamports == 0)
        || open_runtime.principal_recipient.is_zero()
            != (open_runtime.payer_principal_lamports == 0)
        || open_runtime.neutral_sink.is_zero()
        || (!open_runtime.principal_recipient.is_zero()
            && open_runtime.principal_recipient == open_runtime.neutral_sink)
    {
        return Err(Error::InvalidParameter);
    }
    let source_plane_id = source_plane.id()?;
    let head_binding_id = PdaRecipeV3::source_head(
        source_plane_id,
        head_before.source_spec_id,
        head_before.repair_generation,
    )?
    .id()?;
    let open_binding_id = PdaRecipeV3::open_raw_page(
        source_plane_id,
        open_before.source_spec_id,
        open_before.repair_generation,
        open_before.page_index,
    )?
    .id()?;
    let page_binding_id = PdaRecipeV3::raw_page(source_plane_id, sealed_page.id()?)?.id()?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::SealRawPage,
        transition_receipt_id,
    );
    plan.push_mutation(StateMutationV3 {
        before: AccountStateV3::new(
            AccountFamilyV3::SourceHead,
            head_binding_id,
            head_runtime.account_data_before_id,
            head_runtime.generation,
        )?,
        after: AccountStateV3::new(
            AccountFamilyV3::SourceHead,
            head_binding_id,
            head_runtime.account_data_after_id,
            head_runtime.generation,
        )?,
    })?;
    plan.push_creation(AccountCreationV3 {
        state: AccountStateV3::new(
            AccountFamilyV3::RawPage,
            page_binding_id,
            page_runtime.account_data_id,
            page_runtime.generation,
        )?,
        payer: page_runtime.payer,
        rent_principal_lamports: page_runtime.rent_principal_lamports,
        creation_budget_lamports: 0,
        prepaid_work_lamports: 0,
        liquidity_collateral: 0,
    })?;
    plan.push_close(AccountClosureV3 {
        state: AccountStateV3::new(
            AccountFamilyV3::OpenRawPage,
            open_binding_id,
            open_runtime.account_data_id,
            open_runtime.generation,
        )?,
        principal_recipient: open_runtime.principal_recipient,
        payer_principal_lamports: open_runtime.payer_principal_lamports,
        neutral_sink: open_runtime.neutral_sink,
        neutral_surplus_lamports: open_runtime.neutral_surplus_lamports,
    })?;
    plan.finish()
}

/// Recompute action 6 from the canonical WindowSpec, exact initial work body,
/// promoted runtime postimage, and Product/Source occurrence-window join.
pub fn project_runtime_initialize_window_work(
    source_plane: &SourcePlaneProgramV3,
    window: &WindowSpecV3,
    work: &WindowWorkV3,
    work_runtime: RuntimeCreationProjectionV1,
    occurrence_window_authentication_id: ContentId,
) -> Result<TransitionPlanV3> {
    source_plane.validate()?;
    window.validate()?;
    work.validate_against(window)?;
    let expected_work = WindowWorkV3::new(window)?;
    if source_plane.id()? != window.source_plane_program_id
        || expected_work != *work
        || work_runtime.account_data_id.is_zero()
        || work_runtime.generation == 0
        || work_runtime.payer.is_zero() != (work_runtime.rent_principal_lamports == 0)
        || occurrence_window_authentication_id.is_zero()
    {
        return Err(Error::InvalidParameter);
    }
    let binding_id = PdaRecipeV3::window_work(window.id()?)?.id()?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::CreateWindowWork,
        occurrence_window_authentication_id,
    );
    plan.push_creation(AccountCreationV3 {
        state: AccountStateV3::new(
            AccountFamilyV3::WindowWork,
            binding_id,
            work_runtime.account_data_id,
            work_runtime.generation,
        )?,
        payer: work_runtime.payer,
        rent_principal_lamports: work_runtime.rent_principal_lamports,
        creation_budget_lamports: 0,
        prepaid_work_lamports: 0,
        liquidity_collateral: 0,
    })?;
    plan.finish()
}

/// Recompute action 7 from one exact canonical page fold and promoted
/// WindowWork compare-and-swap observation.
#[allow(clippy::too_many_arguments)]
pub fn project_runtime_fold_window_page(
    source_plane: &SourcePlaneProgramV3,
    window: &WindowSpecV3,
    work_before: &WindowWorkV3,
    page: &RawPageV3,
    work_after: &WindowWorkV3,
    work_runtime: RuntimeMutationProjectionV1,
    fold_authentication_id: ContentId,
) -> Result<TransitionPlanV3> {
    source_plane.validate()?;
    window.validate()?;
    work_before.validate_against(window)?;
    page.validate()?;
    work_after.validate_against(window)?;
    let expected_after = work_before.push_page(window, page)?;
    if source_plane.id()? != window.source_plane_program_id
        || expected_after != *work_after
        || work_runtime.account_data_before_id.is_zero()
        || work_runtime.account_data_after_id.is_zero()
        || work_runtime.account_data_before_id == work_runtime.account_data_after_id
        || work_runtime.generation == 0
        || fold_authentication_id.is_zero()
    {
        return Err(Error::InvalidParameter);
    }
    let binding_id = PdaRecipeV3::window_work(window.id()?)?.id()?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::FoldWindowPage,
        fold_authentication_id,
    );
    plan.push_mutation(StateMutationV3 {
        before: AccountStateV3::new(
            AccountFamilyV3::WindowWork,
            binding_id,
            work_runtime.account_data_before_id,
            work_runtime.generation,
        )?,
        after: AccountStateV3::new(
            AccountFamilyV3::WindowWork,
            binding_id,
            work_runtime.account_data_after_id,
            work_runtime.generation,
        )?,
    })?;
    plan.finish()
}

/// Recompute action 8 from exact mature page semantics, the immutable
/// WindowSeal postimage, and the once-only consumed WindowWork close split.
#[allow(clippy::too_many_arguments)]
pub fn project_runtime_seal_window(
    source_plane: &SourcePlaneProgramV3,
    window: &WindowSpecV3,
    work: &WindowWorkV3,
    maturity_page: &RawPageV3,
    closure: &WindowClosureReceiptV3,
    seal: &WindowSealV3,
    work_runtime: RuntimeCloseProjectionV1,
    seal_runtime: RuntimeCreationProjectionV1,
    window_evidence_authentication_id: ContentId,
) -> Result<TransitionPlanV3> {
    source_plane.validate()?;
    window.validate()?;
    work.validate_against(window)?;
    maturity_page.validate()?;
    seal.validate_against(window)?;
    let expected_closure = WindowClosureReceiptV3::from_page(source_plane, window, maturity_page)?;
    let expected_seal = work.finish(window, &expected_closure)?;
    if source_plane.id()? != window.source_plane_program_id
        || expected_closure != *closure
        || expected_seal != *seal
        || work_runtime.account_data_id.is_zero()
        || work_runtime.generation == 0
        || work_runtime.principal_recipient.is_zero()
            != (work_runtime.payer_principal_lamports == 0)
        || work_runtime.neutral_sink.is_zero()
        || (!work_runtime.principal_recipient.is_zero()
            && work_runtime.principal_recipient == work_runtime.neutral_sink)
        || seal_runtime.account_data_id.is_zero()
        || seal_runtime.generation == 0
        || seal_runtime.payer.is_zero() != (seal_runtime.rent_principal_lamports == 0)
        || window_evidence_authentication_id.is_zero()
    {
        return Err(Error::InvalidParameter);
    }
    let window_id = window.id()?;
    let work_binding_id = PdaRecipeV3::window_work(window_id)?.id()?;
    let seal_binding_id = PdaRecipeV3::window_seal(window_id)?.id()?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::SealWindow,
        window_evidence_authentication_id,
    );
    plan.push_creation(AccountCreationV3 {
        state: AccountStateV3::new(
            AccountFamilyV3::WindowSeal,
            seal_binding_id,
            seal_runtime.account_data_id,
            seal_runtime.generation,
        )?,
        payer: seal_runtime.payer,
        rent_principal_lamports: seal_runtime.rent_principal_lamports,
        creation_budget_lamports: 0,
        prepaid_work_lamports: 0,
        liquidity_collateral: 0,
    })?;
    plan.push_close(AccountClosureV3 {
        state: AccountStateV3::new(
            AccountFamilyV3::WindowWork,
            work_binding_id,
            work_runtime.account_data_id,
            work_runtime.generation,
        )?,
        principal_recipient: work_runtime.principal_recipient,
        payer_principal_lamports: work_runtime.payer_principal_lamports,
        neutral_sink: work_runtime.neutral_sink,
        neutral_surplus_lamports: work_runtime.neutral_surplus_lamports,
    })?;
    plan.finish()
}

/// Recompute action 9 from the exact release-authenticated evaluator result
/// and promoted immutable StatisticResult postimage.
#[allow(clippy::too_many_arguments)]
pub fn project_runtime_evaluate_statistic(
    source_plane: &SourcePlaneProgramV3,
    window: &WindowSpecV3,
    key: &StatisticKeyV3,
    summary: &SummaryProgramV3,
    seal: &WindowSealV3,
    result: &StatisticResultV3,
    result_runtime: RuntimeCreationProjectionV1,
    evaluation_authentication_id: ContentId,
) -> Result<TransitionPlanV3> {
    source_plane.validate()?;
    window.validate()?;
    key.validate()?;
    summary.validate()?;
    seal.validate_against(window)?;
    result.validate_against(key, summary, seal, window)?;
    if source_plane.id()? != window.source_plane_program_id
        || key.window_id != window.id()?
        || key.summary_program_id != summary.id()?
        || result_runtime.account_data_id.is_zero()
        || result_runtime.generation == 0
        || result_runtime.payer.is_zero() != (result_runtime.rent_principal_lamports == 0)
        || evaluation_authentication_id.is_zero()
    {
        return Err(Error::InvalidParameter);
    }
    let action = match key.statistic {
        StatisticKindV3::TerminalInterval => TransitionActionV3::WriteTerminalResult,
        StatisticKindV3::MaximumDrawdownInterval => TransitionActionV3::WriteDrawdownResult,
    };
    let binding_id = PdaRecipeV3::statistic_result(key.id()?)?.id()?;
    let mut plan = TransitionPlanV3::new(action, evaluation_authentication_id);
    plan.push_creation(AccountCreationV3 {
        state: AccountStateV3::new(
            AccountFamilyV3::StatisticResult,
            binding_id,
            result_runtime.account_data_id,
            result_runtime.generation,
        )?,
        payer: result_runtime.payer,
        rent_principal_lamports: result_runtime.rent_principal_lamports,
        creation_budget_lamports: 0,
        prepaid_work_lamports: 0,
        liquidity_collateral: 0,
    })?;
    plan.finish()
}

/// Create page work at the exact state-owned head cursor.
pub fn project_open_raw_page(
    source_plane: &SourcePlaneProgramV3,
    head: &SourceHeadV3,
    open_header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<CoreTransitionV3<OpenRawPageV3>> {
    source_plane.validate()?;
    let open = head.open_page()?;
    open.validate_against_head(head)?;
    let state = open_page_state(source_plane, &open, open_header, neutral_sink)?;
    let mut plan = TransitionPlanV3::new(TransitionActionV3::OpenRawPage, ContentId::ZERO);
    plan.push_creation(creation(state, open_header, 0, 0, 0))?;
    Ok(CoreTransitionV3 {
        output: open,
        plan: plan.finish()?,
    })
}

/// Transcode and append exactly one fully authenticated V2 boundary.
pub fn project_append_v2_boundary(
    source_plane: &SourcePlaneProgramV3,
    head: &SourceHeadV3,
    open: &OpenRawPageV3,
    authenticated: V2AuthenticatedRecord,
    open_mutation: AccountMutationV3,
) -> Result<CoreTransitionV3<OpenRawPageV3>> {
    source_plane.validate()?;
    open.validate_against_head(head)?;
    let source_plane_id = source_plane.id()?;
    let expected_bucket = open
        .start_bucket
        .checked_add(u64::from(open.record_count))
        .ok_or(Error::ArithmeticOverflow)?;
    let record = authenticated.project_v3(head.source_spec_id, source_plane_id, expected_bucket)?;
    let next = open.append_observation(record)?;
    open_mutation.validate_accounted(open_mutation.before_header.terminal.payer_principal)?;
    let before = open_page_state(
        source_plane,
        open,
        open_mutation.before_header,
        open_mutation.neutral_sink,
    )?;
    let after = open_page_state(
        source_plane,
        &next,
        open_mutation.after_header,
        open_mutation.neutral_sink,
    )?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::AppendBoundary,
        authenticated.authentication_digest(),
    );
    plan.push_mutation(StateMutationV3 { before, after })?;
    Ok(CoreTransitionV3 {
        output: next,
        plan: plan.finish()?,
    })
}

/// Create resumable work at the predictable WindowKey.
pub fn project_create_window_work(
    window: &WindowSpecV3,
    lineage: WindowWorkLineageV3,
    work_header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<CoreTransitionV3<WindowWorkV3>> {
    if lineage.authorization_digest.is_zero() {
        return Err(Error::ReopenGenerationUnavailable);
    }
    let work = WindowWorkV3::new(window)?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::CreateWindowWork,
        lineage.authorization_digest,
    );
    plan.push_creation(creation(
        window_work_state(window, &work, work_header, neutral_sink)?,
        work_header,
        0,
        0,
        0,
    ))?;
    Ok(CoreTransitionV3 {
        output: work,
        plan: plan.finish()?,
    })
}

/// Fold one immutable page into WindowWork. Page identity is committed as the
/// transition evidence, so a mutable-tail substitute cannot share an intent.
pub fn project_fold_window_page(
    window: &WindowSpecV3,
    work: WindowWorkV3,
    authenticated_page: AuthenticatedRawPageV3,
    work_mutation: AccountMutationV3,
) -> Result<CoreTransitionV3<WindowWorkV3>> {
    if authenticated_page.authentication_digest.is_zero() {
        return Err(Error::ZeroIdentity);
    }
    let next = work.push_page(window, &authenticated_page.page)?;
    work_mutation.validate_accounted(work_mutation.before_header.terminal.payer_principal)?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::FoldWindowPage,
        authenticated_page.authentication_digest,
    );
    plan.push_mutation(StateMutationV3 {
        before: window_work_state(
            window,
            &work,
            work_mutation.before_header,
            work_mutation.neutral_sink,
        )?,
        after: window_work_state(
            window,
            &next,
            work_mutation.after_header,
            work_mutation.neutral_sink,
        )?,
    })?;
    Ok(CoreTransitionV3 {
        output: next,
        plan: plan.finish()?,
    })
}

/// Seal a mature window and close its work state. V2's end cursor is not a V3
/// closure receipt; the supplied V3 page must itself reach V3 maturity.
#[allow(clippy::too_many_arguments)]
pub fn project_seal_window(
    source_plane: &SourcePlaneProgramV3,
    window: &WindowSpecV3,
    work: WindowWorkV3,
    maturity_page: AuthenticatedRawPageV3,
    seal_header: AccountHeaderV3,
    seal_neutral_sink: Id,
    work_close: AccountCloseV3,
) -> Result<CoreTransitionV3<(WindowClosureReceiptV3, WindowSealV3)>> {
    if maturity_page.authentication_digest.is_zero() {
        return Err(Error::ZeroIdentity);
    }
    let closure = WindowClosureReceiptV3::from_page(source_plane, window, &maturity_page.page)?;
    let seal = work.finish(window, &closure)?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::SealWindow,
        maturity_page.authentication_digest,
    );
    plan.push_creation(creation(
        window_seal_state(window, &seal, seal_header, seal_neutral_sink)?,
        seal_header,
        0,
        0,
        0,
    ))?;
    plan.push_close(account_closure(
        window_work_state(window, &work, work_close.header, work_close.neutral_sink)?,
        work_close,
    ))?;
    Ok(CoreTransitionV3 {
        output: (closure, seal),
        plan: plan.finish()?,
    })
}

/// Write a terminal result after a separate exact evaluator has authenticated
/// its page-chain derivation. `evaluation_digest` commits that derivation; this
/// adapter contract does not pretend caller-supplied endpoints are evidence.
#[allow(clippy::too_many_arguments)]
pub fn project_write_terminal_result(
    key: &StatisticKeyV3,
    summary: &SummaryProgramV3,
    seal: &WindowSealV3,
    window: &WindowSpecV3,
    evaluation: TerminalEvaluationV3,
    result_header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<CoreTransitionV3<StatisticResultV3>> {
    if evaluation.evidence_digest.is_zero() {
        return Err(Error::ZeroIdentity);
    }
    let result =
        StatisticResultV3::terminal(key, summary, seal, window, evaluation.low, evaluation.high)?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::WriteTerminalResult,
        evaluation.evidence_digest,
    );
    plan.push_creation(creation(
        statistic_result_state(key, &result, result_header, neutral_sink)?,
        result_header,
        0,
        0,
        0,
    ))?;
    Ok(CoreTransitionV3 {
        output: result,
        plan: plan.finish()?,
    })
}

/// Write a drawdown result only from an exact full-window core summary.
#[allow(clippy::too_many_arguments)]
pub fn project_write_drawdown_result(
    key: &StatisticKeyV3,
    summary_program: &SummaryProgramV3,
    seal: &WindowSealV3,
    window: &WindowSpecV3,
    evaluation: VerifiedDrawdownEvaluationV3,
    result_header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<CoreTransitionV3<StatisticResultV3>> {
    evaluation.summary.validate()?;
    let span = window
        .end_bucket_exclusive
        .checked_sub(window.start_bucket)
        .ok_or(Error::ArithmeticOverflow)?;
    if evaluation.summary.start_bucket() != window.start_bucket
        || evaluation.summary.end_bucket_exclusive() != window.end_bucket_exclusive
        || evaluation.summary.record_count() != span
        || evaluation.evidence_digest.is_zero()
    {
        return Err(Error::MismatchedState);
    }
    let result = StatisticResultV3::drawdown(
        key,
        summary_program,
        seal,
        window,
        evaluation.summary.interval(),
    )?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::WriteDrawdownResult,
        evaluation.evidence_digest,
    );
    plan.push_creation(creation(
        statistic_result_state(key, &result, result_header, neutral_sink)?,
        result_header,
        0,
        0,
        0,
    ))?;
    Ok(CoreTransitionV3 {
        output: result,
        plan: plan.finish()?,
    })
}

/// Activate a finite Series with exact funding for every scheduled Instance.
#[allow(clippy::too_many_arguments)]
pub fn project_activate_series(
    bindings: SeriesBindingsV3<'_>,
    creation_lamports: u64,
    liveness_lamports: u64,
    liquidity_collateral: u64,
    transfers: SeriesActivationTransfersV3,
    plan_header: AccountHeaderV3,
    funding_header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<CoreTransitionV3<SeriesFundingV3>> {
    if transfers.authorization_digest.is_zero() {
        return Err(Error::SeriesTransferGraphUnavailable);
    }
    bindings.template.validate_bindings(
        bindings.source_plane,
        bindings.summary,
        bindings.payouts,
        bindings.partition,
    )?;
    let funding = SeriesFundingV3::activate(
        bindings.series,
        bindings.template,
        bindings.work,
        bindings.liquidity,
        creation_lamports,
        liveness_lamports,
        liquidity_collateral,
    )?;
    if plan_header.terminal.payer != funding_header.terminal.payer {
        return Err(Error::MismatchedState);
    }
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::ActivateSeries,
        transfers.authorization_digest,
    );
    plan.push_creation(creation(
        series_plan_state(bindings.series, plan_header, neutral_sink)?,
        plan_header,
        0,
        0,
        0,
    ))?;
    plan.push_creation(creation(
        series_funding_state(bindings.series, &funding, funding_header, neutral_sink)?,
        funding_header,
        creation_lamports,
        liveness_lamports,
        liquidity_collateral,
    ))?;
    Ok(CoreTransitionV3 {
        output: funding,
        plan: plan.finish()?,
    })
}

/// Instantiate exactly the next ordinal and atomically debit all three finite
/// compartments. No future fee field participates.
#[allow(clippy::too_many_arguments)]
pub fn project_create_next_instance(
    bindings: SeriesBindingsV3<'_>,
    funding: SeriesFundingV3,
    requested_ordinal: u32,
    current_bucket: u64,
    transfers: SeriesInstantiationTransfersV3,
    funding_mutation: AccountMutationV3,
    instance_header: AccountHeaderV3,
    instance_neutral_sink: Id,
) -> Result<CoreTransitionV3<(SeriesFundingV3, CompiledInstanceV3)>> {
    if transfers.authorization_digest.is_zero() {
        return Err(Error::SeriesTransferGraphUnavailable);
    }
    let (next, instance) = funding.instantiate_next(
        bindings.source_plane,
        bindings.summary,
        bindings.payouts,
        bindings.partition,
        bindings.template,
        bindings.work,
        bindings.liquidity,
        bindings.series,
        requested_ordinal,
        current_bucket,
    )?;
    let downstream_creation = instance
        .creation_lamports()
        .checked_sub(instance_header.terminal.payer_principal)
        .ok_or(Error::MismatchedState)?;
    let expected_accounted = funding_accounted_lamports(funding_mutation.before_header, &next)?;
    funding_mutation.validate_accounted(expected_accounted)?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::CreateSeriesInstance,
        transfers.authorization_digest,
    );
    plan.set_series_context(current_bucket, requested_ordinal);
    plan.push_mutation(StateMutationV3 {
        before: series_funding_state(
            bindings.series,
            &funding,
            funding_mutation.before_header,
            funding_mutation.neutral_sink,
        )?,
        after: series_funding_state(
            bindings.series,
            &next,
            funding_mutation.after_header,
            funding_mutation.neutral_sink,
        )?,
    })?;
    plan.push_creation(creation(
        instance_state(&instance, instance_header, instance_neutral_sink)?,
        instance_header,
        downstream_creation,
        instance.liveness_lamports(),
        instance.liquidity_collateral(),
    ))?;
    Ok(CoreTransitionV3 {
        output: (next, instance),
        plan: plan.finish()?,
    })
}

/// Advance one expired ordinal without relabeling its unused allocation.
pub fn project_lapse_next_instance(
    bindings: SeriesBindingsV3<'_>,
    funding: SeriesFundingV3,
    current_bucket: u64,
    funding_mutation: AccountMutationV3,
) -> Result<CoreTransitionV3<SeriesFundingV3>> {
    let next = funding.lapse_next(
        bindings.series,
        bindings.template,
        bindings.work,
        bindings.liquidity,
        current_bucket,
    )?;
    let expected_accounted = funding_accounted_lamports(funding_mutation.before_header, &next)?;
    funding_mutation.validate_accounted(expected_accounted)?;
    let mut plan = TransitionPlanV3::new(TransitionActionV3::LapseSeriesOrdinal, ContentId::ZERO);
    plan.set_series_context(current_bucket, funding.next_ordinal());
    plan.push_mutation(StateMutationV3 {
        before: series_funding_state(
            bindings.series,
            &funding,
            funding_mutation.before_header,
            funding_mutation.neutral_sink,
        )?,
        after: series_funding_state(
            bindings.series,
            &next,
            funding_mutation.after_header,
            funding_mutation.neutral_sink,
        )?,
    })?;
    Ok(CoreTransitionV3 {
        output: next,
        plan: plan.finish()?,
    })
}

/// Advance over the exact same independently created Instance without spending
/// this Series' still-refundable allocation.
pub fn project_advance_existing_instance(
    bindings: SeriesBindingsV3<'_>,
    funding: SeriesFundingV3,
    existing: AuthenticatedInstanceV3,
    current_bucket: u64,
    funding_mutation: AccountMutationV3,
) -> Result<CoreTransitionV3<SeriesFundingV3>> {
    if existing.authentication_digest.is_zero() {
        return Err(Error::ZeroIdentity);
    }
    let next = funding.advance_existing(
        bindings.source_plane,
        bindings.summary,
        bindings.payouts,
        bindings.partition,
        bindings.template,
        bindings.work,
        bindings.liquidity,
        bindings.series,
        &existing.instance,
        current_bucket,
    )?;
    let expected_accounted = funding_accounted_lamports(funding_mutation.before_header, &next)?;
    funding_mutation.validate_accounted(expected_accounted)?;
    let mut plan = TransitionPlanV3::new(
        TransitionActionV3::AdvanceExistingInstance,
        existing.authentication_digest,
    );
    plan.set_series_context(current_bucket, funding.next_ordinal());
    plan.push_mutation(StateMutationV3 {
        before: series_funding_state(
            bindings.series,
            &funding,
            funding_mutation.before_header,
            funding_mutation.neutral_sink,
        )?,
        after: series_funding_state(
            bindings.series,
            &next,
            funding_mutation.after_header,
            funding_mutation.neutral_sink,
        )?,
    })?;
    Ok(CoreTransitionV3 {
        output: next,
        plan: plan.finish()?,
    })
}

/// Explicit refusal boundary for terminal Series refund.
///
/// The semantic core deliberately keeps lapsed/advanced allocations visible,
/// but this adapter does not yet own a typed graph for returning lamports and
/// Realm-selected collateral, closing vaults, and then terminally splitting
/// the funding account. Absence of an action tag is not treated as permission
/// to improvise that lifecycle.
pub fn project_refund_series_funding(
    _bindings: SeriesBindingsV3<'_>,
    _funding: SeriesFundingV3,
) -> Result<TransitionPlanV3> {
    Err(Error::SeriesTerminalRefundUnavailable)
}

fn source_head_state(
    source_plane: &SourcePlaneProgramV3,
    head: &SourceHeadV3,
    header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<AccountStateV3> {
    let recipe = PdaRecipeV3::source_head(
        source_plane.id()?,
        head.source_spec_id,
        head.repair_generation,
    )?;
    typed_state::<SOURCE_HEAD_BYTES, _>(header, head, neutral_sink, recipe.id()?)
}

fn open_page_state(
    source_plane: &SourcePlaneProgramV3,
    open: &OpenRawPageV3,
    header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<AccountStateV3> {
    let recipe = PdaRecipeV3::open_raw_page(
        source_plane.id()?,
        open.source_spec_id,
        open.repair_generation,
        open.page_index,
    )?;
    typed_state::<OPEN_RAW_PAGE_BYTES, _>(header, open, neutral_sink, recipe.id()?)
}

fn window_work_state(
    window: &WindowSpecV3,
    work: &WindowWorkV3,
    header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<AccountStateV3> {
    work.validate_against(window)?;
    let recipe = PdaRecipeV3::window_work(window.id()?)?;
    typed_state::<WINDOW_WORK_BYTES, _>(header, work, neutral_sink, recipe.id()?)
}

fn window_seal_state(
    window: &WindowSpecV3,
    seal: &WindowSealV3,
    header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<AccountStateV3> {
    seal.validate_against(window)?;
    let recipe = PdaRecipeV3::window_seal(window.id()?)?;
    typed_state::<WINDOW_SEAL_BYTES, _>(header, seal, neutral_sink, recipe.id()?)
}

fn statistic_result_state(
    key: &StatisticKeyV3,
    result: &StatisticResultV3,
    header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<AccountStateV3> {
    let recipe = PdaRecipeV3::statistic_result(key.id()?)?;
    typed_state::<STATISTIC_RESULT_BYTES, _>(header, result, neutral_sink, recipe.id()?)
}

fn series_plan_state(
    series: &SeriesPlanV3,
    header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<AccountStateV3> {
    let recipe = PdaRecipeV3::series_plan(series.id()?)?;
    typed_state::<SERIES_PLAN_BYTES, _>(header, series, neutral_sink, recipe.id()?)
}

fn series_funding_state(
    series: &SeriesPlanV3,
    funding: &SeriesFundingV3,
    header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<AccountStateV3> {
    if funding.series_id() != series.id()? {
        return Err(Error::MismatchedState);
    }
    let recipe = PdaRecipeV3::series_funding(series.id()?)?;
    typed_state::<SERIES_FUNDING_BYTES, _>(header, funding, neutral_sink, recipe.id()?)
}

fn instance_state(
    instance: &CompiledInstanceV3,
    header: AccountHeaderV3,
    neutral_sink: Id,
) -> Result<AccountStateV3> {
    let recipe = PdaRecipeV3::instance(instance.instance_id())?;
    typed_state::<INSTANCE_DESCRIPTOR_BYTES, _>(
        header,
        &instance.descriptor(),
        neutral_sink,
        recipe.id()?,
    )
}

fn creation(
    state: AccountStateV3,
    header: AccountHeaderV3,
    creation_budget_lamports: u64,
    prepaid_work_lamports: u64,
    liquidity_collateral: u64,
) -> AccountCreationV3 {
    AccountCreationV3 {
        state,
        payer: ContentId::from_bytes(header.terminal.payer.bytes()),
        rent_principal_lamports: header.terminal.payer_principal,
        creation_budget_lamports,
        prepaid_work_lamports,
        liquidity_collateral,
    }
}

fn account_closure(state: AccountStateV3, close: AccountCloseV3) -> AccountClosureV3 {
    AccountClosureV3 {
        state,
        principal_recipient: ContentId::from_bytes(close.header.terminal.payer.bytes()),
        payer_principal_lamports: close.header.terminal.payer_principal,
        neutral_sink: ContentId::from_bytes(close.neutral_sink.bytes()),
        neutral_surplus_lamports: close.neutral_surplus_lamports,
    }
}

fn funding_accounted_lamports(header: AccountHeaderV3, funding: &SeriesFundingV3) -> Result<u64> {
    header
        .terminal
        .payer_principal
        .checked_add(funding.creation_lamports())
        .and_then(|value| value.checked_add(funding.liveness_lamports()))
        .ok_or(Error::ArithmeticOverflow)
}

fn reconstruct_terminal(header: AccountHeaderV3, neutral_sink: Id) -> Result<TerminalAccountV1> {
    header.terminal.validate(neutral_sink)?;
    if header.terminal.generation != 1 {
        return Err(Error::ReopenGenerationUnavailable);
    }
    let balance_after = header
        .terminal
        .donation_floor
        .checked_add(header.terminal.payer_principal)
        .ok_or(Error::ArithmeticOverflow)?;
    let account = TerminalAccountV1::create(
        header.terminal.payer,
        neutral_sink,
        header.terminal.donation_floor,
        header.terminal.payer_principal,
        balance_after,
    )?;
    if account.header() != header.terminal {
        return Err(Error::MismatchedState);
    }
    Ok(account)
}

fn typed_state<const N: usize, T: AccountBodyV3>(
    header: AccountHeaderV3,
    body: &T,
    neutral_sink: Id,
    binding_id: ContentId,
) -> Result<AccountStateV3> {
    header.validate::<T>(neutral_sink)?;
    if header.terminal.generation != 1 {
        return Err(Error::ReopenGenerationUnavailable);
    }
    AccountStateV3::new(
        header.family,
        binding_id,
        canonical_account_state_digest::<N, T>(header, body, neutral_sink)?,
        header.terminal.generation,
    )
}

const _: () = assert!(CREATION_BYTES == 144);
const _: () = assert!(TRANSITION_PLAN_BYTES == 1_280);
const _: () = assert!(DRAWDOWN_SUMMARY_BYTES == 112);
