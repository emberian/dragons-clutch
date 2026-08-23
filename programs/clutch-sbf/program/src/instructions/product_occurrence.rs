//! Authenticated SBF boundary for the occurrence-scoped Product lifecycle root.
//!
//! The pure Product crate owns deterministic state and count transitions. This
//! module owns runtime authority: exact `0xaa/1` owner/PDA/bump/body/rent
//! authentication, atomic state writes, private family-terminal receipts, and
//! the non-decodable whole-Market terminal capability. No instruction route is
//! enabled by the existence of these helpers.

use crate::accounts::{expect_pda, require, require_signer, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    transfer_data, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use clutch_product_series::{
    ContentId, MarketInstanceTerminalProjectionV1, MarketInstanceV2Id,
    ProductOccurrenceFamilyTerminalProjectionV1, ProductOccurrenceFamilyV1,
    ProductOccurrencePhaseV1, ProductOccurrenceRootV1, SeriesPlanV5Id,
};
use clutch_solana_layout::product_series::{
    ProductOccurrenceRootAccountV1, PRODUCT_OCCURRENCE_ROOT_ACCOUNT_BYTES_V1,
};
use clutch_solana_layout::registry::{
    FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES, FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES,
};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const AUTHENTICATED_MARKET_TERMINAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/authenticated-market-instance-terminal/v1";
const PRODUCT_OCCURRENCE_FAMILY_CLOSE_AUTHORIZATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-occurrence-family-close-authorization/v1";
const PRODUCT_OCCURRENCE_CAPITALIZATION_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-occurrence-capitalization-authentication/v1";
const PRODUCT_OCCURRENCE_INITIALIZATION_AUTHORIZATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-occurrence-initialization-authorization/v1";
const PRODUCT_OCCURRENCE_FAILURE_CAPITAL_JOIN_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-occurrence-failure-capital-join/v1";
/// Frozen total width of the full-width Resolution V5 account.
///
/// The collateral adapter owns the hostile 304-byte codec. Product uses the
/// width only to capitalize its canonical zero-data PDA before resolution.
const RESOLUTION_V5_ACCOUNT_BYTES: usize = 304;

/// Runtime-authenticated Product occurrence root account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedProductOccurrenceRootV1 {
    account: Pubkey,
    value: ProductOccurrenceRootAccountV1,
}

impl AuthenticatedProductOccurrenceRootV1 {
    /// Exact physical `0xaa/1` account.
    pub const fn account(self) -> Pubkey {
        self.account
    }

    /// Complete authenticated account value.
    pub const fn value(self) -> ProductOccurrenceRootAccountV1 {
        self.value
    }

    /// Complete pure Product lifecycle state.
    pub const fn state(self) -> ProductOccurrenceRootV1 {
        self.value.state
    }
}

/// Private authority to capitalize exactly one authenticated Product occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedProductOccurrenceInitializationV1 {
    id: ContentId,
    state: ProductOccurrenceRootV1,
    market_artifact_account: Pubkey,
    source_occurrence_account: Pubkey,
}

impl AuthenticatedProductOccurrenceInitializationV1 {
    /// Exact typed initialization authorization identity.
    pub const fn id(self) -> ContentId {
        self.id
    }

    /// Canonical kind-46 Market preimage artifact account.
    pub const fn market_artifact_account(self) -> Pubkey {
        self.market_artifact_account
    }

    /// Exact physical Source occurrence account.
    pub const fn source_occurrence_account(self) -> Pubkey {
        self.source_occurrence_account
    }

    /// Complete initialized pure state.
    pub const fn state(self) -> ProductOccurrenceRootV1 {
        self.state
    }
}

/// Private Failure-owned join required before action15 may capitalize an occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedProductOccurrenceFailureCapitalJoinV1 {
    id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    generation: u64,
    failure_policy_binding_id: ContentId,
    recovery_state_id: ContentId,
    recovery_payer: ContentId,
    neutral_lamport_sink: ContentId,
}

impl AuthenticatedProductOccurrenceFailureCapitalJoinV1 {
    /// Exact private Failure join identity.
    pub const fn id(self) -> ContentId {
        self.id
    }

    /// Full-width Market identity.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.market_instance_id
    }

    /// Exact recurring Series.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    /// Exact Series ordinal.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    /// Exact Failure generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Exact admitted Failure policy binding.
    pub const fn failure_policy_binding_id(self) -> ContentId {
        self.failure_policy_binding_id
    }

    /// Exact admitted Recovery state.
    pub const fn recovery_state_id(self) -> ContentId {
        self.recovery_state_id
    }

    /// Liveness payer which must fund occurrence-root and dependent rent principal.
    pub const fn recovery_payer(self) -> ContentId {
        self.recovery_payer
    }

    /// Canonical Recovery/occurrence donation sink.
    pub const fn neutral_lamport_sink(self) -> ContentId {
        self.neutral_lamport_sink
    }
}

/// Seal a typed Failure runtime/admission projection for Product action15.
///
/// Only the Failure adapter should call this after authenticating its runtime,
/// admission receipt, Recovery account, payer, and neutral sink.
#[allow(clippy::too_many_arguments)]
pub(crate) fn mint_product_occurrence_failure_capital_join_v1(
    market_instance_id: MarketInstanceV2Id,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    generation: u64,
    failure_policy_binding_id: ContentId,
    recovery_state_id: ContentId,
    recovery_payer: ContentId,
    neutral_lamport_sink: ContentId,
) -> Outcome<AuthenticatedProductOccurrenceFailureCapitalJoinV1> {
    market_instance_id
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    series_plan_id
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    for id in [
        failure_policy_binding_id,
        recovery_state_id,
        recovery_payer,
        neutral_lamport_sink,
    ] {
        require_live_content_id(id)?;
    }
    require(generation != 0, ClutchError::MismatchedState)?;
    let ordinal_bytes = ordinal.to_le_bytes();
    let generation_bytes = generation.to_le_bytes();
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_OCCURRENCE_FAILURE_CAPITAL_JOIN_DOMAIN_V1,
            &market_instance_id.bytes(),
            &series_plan_id.bytes(),
            &ordinal_bytes,
            &generation_bytes,
            &failure_policy_binding_id.bytes(),
            &recovery_state_id.bytes(),
            &recovery_payer.bytes(),
            &neutral_lamport_sink.bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedProductOccurrenceFailureCapitalJoinV1 {
        id,
        market_instance_id,
        series_plan_id,
        ordinal,
        generation,
        failure_policy_binding_id,
        recovery_state_id,
        recovery_payer,
        neutral_lamport_sink,
    })
}

/// Seal a fully authenticated Product/Series/Source join for action15.
pub(crate) fn mint_product_occurrence_initialization_v1(
    state: ProductOccurrenceRootV1,
    market_artifact_account: Pubkey,
    source_occurrence_account: Pubkey,
    source_occurrence_account_authentication_id: ContentId,
) -> Outcome<AuthenticatedProductOccurrenceInitializationV1> {
    require(
        state.phase() == ProductOccurrencePhaseV1::Active
            && state.transition_sequence() == 0
            && state.binding().source_occurrence_account_id.bytes()
                == source_occurrence_account.to_bytes()
            && state.binding().source_occurrence_account_authentication_id
                == source_occurrence_account_authentication_id
            && market_artifact_account != source_occurrence_account,
        ClutchError::MismatchedState,
    )?;
    let semantic_id = state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_OCCURRENCE_INITIALIZATION_AUTHORIZATION_DOMAIN_V1,
            &semantic_id.bytes(),
            market_artifact_account.as_ref(),
            source_occurrence_account.as_ref(),
            &source_occurrence_account_authentication_id.bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedProductOccurrenceInitializationV1 {
        id,
        state,
        market_artifact_account,
        source_occurrence_account,
    })
}

/// Private proof that action15 supplied exact present rent capital for
/// 0xaa/ab/ac and Resolution V5.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedProductOccurrenceCapitalizationV1 {
    id: ContentId,
    root_account: Pubkey,
    interval_work_account: Pubkey,
    interval_replay_account: Pubkey,
    resolution_account: Pubkey,
    binding: clutch_product_series::ProductOccurrenceBindingV1,
    capitalization: clutch_product_series::ProductOccurrenceCapitalizationV1,
    observed_balances: [u64; 4],
    observed_donations: [u64; 4],
}

impl AuthenticatedProductOccurrenceCapitalizationV1 {
    /// Exact private authentication receipt identity.
    pub const fn id(self) -> ContentId {
        self.id
    }

    /// Product occurrence root which persists the funding facts.
    pub const fn root_account(self) -> Pubkey {
        self.root_account
    }

    /// Exact prefunded 0xab work PDA.
    pub const fn interval_work_account(self) -> Pubkey {
        self.interval_work_account
    }

    /// Exact prefunded permanent 0xac replay PDA.
    pub const fn interval_replay_account(self) -> Pubkey {
        self.interval_replay_account
    }

    /// Exact prefunded canonical Resolution V5 PDA.
    pub const fn resolution_account(self) -> Pubkey {
        self.resolution_account
    }

    /// Full-width Market identity.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.binding.market_instance_id
    }

    /// Exact occurrence generation.
    pub const fn generation(self) -> u64 {
        self.binding.generation
    }

    /// External payer which owns all four principal amounts.
    pub const fn rent_payer(self) -> ContentId {
        self.binding.rent_payer
    }

    /// Canonical sink for eventual unowned lamports.
    pub const fn neutral_lamport_sink(self) -> ContentId {
        self.binding.neutral_lamport_sink
    }

    /// Exact Failure policy binding admitted for this occurrence.
    pub const fn failure_policy_binding_id(self) -> ContentId {
        self.binding.failure_policy_binding_id
    }

    /// Exact Recovery semantic state admitted for this occurrence.
    pub const fn recovery_state_id(self) -> ContentId {
        self.binding.recovery_state_id
    }

    /// Central-profile-derived interval-consensus profile identity.
    pub const fn interval_consensus_profile_id(self) -> ContentId {
        self.binding.interval_consensus_profile_id
    }

    /// Exact maximum interval width.
    pub const fn maximum_interval_width(self) -> u64 {
        self.binding.maximum_interval_width
    }

    /// Exact maximum coordinates evaluated by one paid advance.
    pub const fn maximum_coordinates_per_advance(self) -> u16 {
        self.binding.maximum_coordinates_per_advance
    }

    /// Exact `[0xaa, 0xab, 0xac, Resolution V5]` payer-owned principals.
    pub const fn principal_lamports(self) -> [u64; 4] {
        self.capitalization.principal_lamports
    }

    /// Exact prior-donation floors in root/work/replay/resolution order.
    pub const fn donation_floor_lamports(self) -> [u64; 4] {
        self.capitalization.donation_floor_lamports
    }

    /// Exact postfund balances in root/work/replay/resolution order.
    pub fn postfund_balances(self) -> Outcome<[u64; 4]> {
        self.capitalization
            .postfund_balances()
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))
    }

    /// Current authenticated balances in root/work/replay/resolution order.
    pub const fn observed_balances(self) -> [u64; 4] {
        self.observed_balances
    }

    /// Current donations after subtracting immutable payer principal.
    pub const fn observed_donations(self) -> [u64; 4] {
        self.observed_donations
    }
}

/// Private family-owner capability accepted by the Product root transition.
///
/// No instruction codec exists for this type. A sibling family adapter may
/// mint it only after authenticating its family-owned root/account, terminal
/// receipt, current release, and complete zero-child/zero-liability condition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedProductOccurrenceFamilyTerminalV1 {
    projection: ProductOccurrenceFamilyTerminalProjectionV1,
}

/// Private preauthorization for one family to close its terminal state.
///
/// This capability is minted only from a freshly authenticated Retiring root.
/// It does not decrement the family count. The family adapter must close its
/// own accounts, mint its non-decodable terminal receipt, and feed that receipt
/// back through [`consume_product_occurrence_family_terminal_v1`] atomically.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedProductOccurrenceFamilyCloseAuthorizationV1 {
    id: ContentId,
    root_account: Pubkey,
    root_semantic_id: ContentId,
    binding: clutch_product_series::ProductOccurrenceBindingV1,
    family: ProductOccurrenceFamilyV1,
    family_terminal_sequence: u32,
    root_transition_sequence: u64,
}

impl AuthenticatedProductOccurrenceFamilyCloseAuthorizationV1 {
    /// Exact non-decodable authorization identity.
    pub const fn id(self) -> ContentId {
        self.id
    }

    /// Exact physical Product occurrence root.
    pub const fn root_account(self) -> Pubkey {
        self.root_account
    }

    /// Exact semantic root state before the family close.
    pub const fn root_semantic_id(self) -> ContentId {
        self.root_semantic_id
    }

    /// Complete immutable Product/Series/Source binding.
    pub const fn binding(self) -> clutch_product_series::ProductOccurrenceBindingV1 {
        self.binding
    }

    /// Exact independently owned family authorized to close.
    pub const fn family(self) -> ProductOccurrenceFamilyV1 {
        self.family
    }

    /// Exact family-local terminal sequence.
    pub const fn family_terminal_sequence(self) -> u32 {
        self.family_terminal_sequence
    }

    /// Exact next Product-root transition sequence.
    pub const fn root_transition_sequence(self) -> u64 {
        self.root_transition_sequence
    }
}

impl AuthenticatedProductOccurrenceFamilyTerminalV1 {
    /// Pure projection committed into the Product root transition.
    pub const fn projection(self) -> ProductOccurrenceFamilyTerminalProjectionV1 {
        self.projection
    }
}

/// Private whole-Market terminal authority minted from an authenticated root.
///
/// This type has no hostile-byte decoder and no public constructor. Failure may
/// consume it in the same SBF program only after comparing every getter with
/// its resolved retirement prerequisite; clients cannot supply it as data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedMarketInstanceTerminalCapabilityV1 {
    id: ContentId,
    root_account: Pubkey,
    owner_program: Pubkey,
    projection: MarketInstanceTerminalProjectionV1,
}

impl AuthenticatedMarketInstanceTerminalCapabilityV1 {
    /// Exact private capability receipt identity.
    pub const fn id(self) -> ContentId {
        self.id
    }

    /// Exact physical Product occurrence root account.
    pub const fn root_account(self) -> Pubkey {
        self.root_account
    }

    /// Program owner freshly authenticated for that root.
    pub const fn owner_program(self) -> Pubkey {
        self.owner_program
    }

    /// Full-width Product Market identity.
    pub const fn market_instance_id(self) -> MarketInstanceV2Id {
        self.projection.binding().market_instance_id
    }

    /// Exact recurring Series identity.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.projection.binding().series_plan_id
    }

    /// Exact Series ordinal.
    pub const fn ordinal(self) -> u32 {
        self.projection.binding().ordinal
    }

    /// Exact occurrence generation.
    pub const fn generation(self) -> u64 {
        self.projection.binding().generation
    }

    /// Exact terminal Product root semantic identity.
    pub const fn root_semantic_id(self) -> ContentId {
        self.projection.root_semantic_id()
    }

    /// Pure whole-Market terminal projection sealed behind this receipt.
    pub const fn projection(self) -> MarketInstanceTerminalProjectionV1 {
        self.projection
    }
}

/// Authenticate one Product occurrence root from complete hostile account state.
pub fn authenticate_product_occurrence_root_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_series_plan_id: SeriesPlanV5Id,
    expected_generation: u64,
    writable: bool,
) -> Outcome<AuthenticatedProductOccurrenceRootV1> {
    expected_market_instance_id
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expected_series_plan_id
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(expected_generation != 0, ClutchError::MismatchedState)?;
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    if writable {
        require(account.is_writable, ClutchError::NotWritable)?;
    } else {
        require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    }
    require(
        account.data_len() == PRODUCT_OCCURRENCE_ROOT_ACCOUNT_BYTES_V1,
        ClutchError::WrongDataLength,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = ProductOccurrenceRootAccountV1::decode(&data)?;
    let binding = value.state.binding();
    require(
        binding.market_instance_id == expected_market_instance_id
            && binding.series_plan_id == expected_series_plan_id
            && binding.generation == expected_generation,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::product_occurrence_root_pda(
            program_id,
            &expected_market_instance_id.bytes(),
            expected_generation,
        ),
        Some(value.stored_bump),
    )?;
    let postfund = value
        .state
        .capitalization()
        .postfund_balances()
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        account.lamports() >= postfund[0],
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedProductOccurrenceRootV1 {
        account: *account.key,
        value,
    })
}

/// Authenticate exact present 0xab/0xac principal before Failure Begin.
pub fn authenticate_product_occurrence_capitalization_v1(
    program_id: &Pubkey,
    root: AuthenticatedProductOccurrenceRootV1,
    root_account: &AccountInfo<'_>,
    interval_work: &AccountInfo<'_>,
    interval_replay: &AccountInfo<'_>,
    resolution: &AccountInfo<'_>,
) -> Outcome<AuthenticatedProductOccurrenceCapitalizationV1> {
    let state = root.state();
    let binding = state.binding();
    require(
        state.phase() == ProductOccurrencePhaseV1::Active,
        ClutchError::MismatchedState,
    )?;
    let market = binding.market_instance_id.bytes();
    let (expected_work, _) =
        seeds::failure_interval_consensus_work_pda(program_id, &market, binding.generation);
    let (expected_replay, _) =
        seeds::failure_interval_consensus_replay_pda(program_id, &market, binding.generation);
    let (expected_resolution, _) = seeds::resolution_v5_pda(program_id, &market);
    let postfund = state
        .capitalization()
        .postfund_balances()
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        *root_account.key == root.account
            && root_account.owner == program_id
            && !root_account.is_signer
            && !root_account.executable
            && root_account.data_len() == PRODUCT_OCCURRENCE_ROOT_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    require(
        *interval_work.key == expected_work
            && *interval_replay.key == expected_replay
            && *resolution.key == expected_resolution
            && binding.failure_interval_work_account_id.bytes() == interval_work.key.to_bytes()
            && binding.failure_interval_replay_account_id.bytes() == interval_replay.key.to_bytes()
            && binding.resolution_account_id.bytes() == resolution.key.to_bytes(),
        ClutchError::WrongPda,
    )?;
    for (account, floor) in [interval_work, interval_replay, resolution]
        .into_iter()
        .zip([postfund[1], postfund[2], postfund[3]])
    {
        require(
            account.is_writable
                && !account.is_signer
                && !account.executable
                && account.owner == &SYSTEM_PROGRAM_ID
                && account.data_is_empty()
                && account.lamports() >= floor,
            ClutchError::MismatchedState,
        )?;
    }
    let observed_balances = [
        root_account.lamports(),
        interval_work.lamports(),
        interval_replay.lamports(),
        resolution.lamports(),
    ];
    let principal = state.capitalization().principal_lamports;
    let mut observed_donations = [0; 4];
    let mut index = 0usize;
    while index < observed_balances.len() {
        observed_donations[index] = observed_balances[index]
            .checked_sub(principal[index])
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        require(
            observed_donations[index] >= state.capitalization().donation_floor_lamports[index],
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    let root_semantic_id = state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_OCCURRENCE_CAPITALIZATION_AUTHENTICATION_DOMAIN_V1,
            root.account.as_ref(),
            &root_semantic_id.bytes(),
            interval_work.key.as_ref(),
            interval_replay.key.as_ref(),
            resolution.key.as_ref(),
            &observed_balances[0].to_le_bytes(),
            &observed_balances[1].to_le_bytes(),
            &observed_balances[2].to_le_bytes(),
            &observed_balances[3].to_le_bytes(),
            &observed_donations[0].to_le_bytes(),
            &observed_donations[1].to_le_bytes(),
            &observed_donations[2].to_le_bytes(),
            &observed_donations[3].to_le_bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedProductOccurrenceCapitalizationV1 {
        id,
        root_account: root.account,
        interval_work_account: *interval_work.key,
        interval_replay_account: *interval_replay.key,
        resolution_account: *resolution.key,
        binding,
        capitalization: state.capitalization(),
        observed_balances,
        observed_donations,
    })
}

/// Capitalize 0xaa/ab/ac and Resolution V5 atomically after typed authentication.
///
/// The external payer supplies every exact Rent-derived principal even when a
/// predictable PDA already holds third-party lamports. Those prior balances
/// remain disjoint donation floors; they never discount payer-owned principal.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_product_occurrence_root_v1<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    interval_work: &AccountInfo<'a>,
    interval_replay: &AccountInfo<'a>,
    resolution: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    authorization: AuthenticatedProductOccurrenceInitializationV1,
) -> Outcome<(
    AuthenticatedProductOccurrenceRootV1,
    AuthenticatedProductOccurrenceCapitalizationV1,
)> {
    let state = authorization.state;
    let binding = state.binding();
    let capitalization = state.capitalization();
    let postfund = capitalization
        .postfund_balances()
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        state.phase() == ProductOccurrencePhaseV1::Active
            && state.transition_sequence() == 0
            && binding.rent_payer.bytes() == payer.key.to_bytes()
            && binding.neutral_lamport_sink.bytes() == neutral_sink.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require(
        !neutral_sink.is_signer
            && !neutral_sink.executable
            && neutral_sink.data_len() == 0
            && *neutral_sink.owner == SYSTEM_PROGRAM_ID,
        ClutchError::MismatchedState,
    )?;
    require(
        payer.key != target.key
            && payer.key != interval_work.key
            && payer.key != interval_replay.key
            && payer.key != resolution.key
            && payer.key != neutral_sink.key
            && target.key != interval_work.key
            && target.key != interval_replay.key
            && target.key != resolution.key
            && target.key != neutral_sink.key
            && interval_work.key != interval_replay.key
            && interval_work.key != resolution.key
            && interval_work.key != neutral_sink.key
            && interval_replay.key != resolution.key
            && interval_replay.key != neutral_sink.key
            && resolution.key != neutral_sink.key,
        ClutchError::AccountAlias,
    )?;
    for account in [target, interval_work, interval_replay, resolution] {
        require_creatable(account)?;
    }
    require_system_program(system_program)?;
    let rent = read_rent(rent_sysvar)?;
    let generation_seed = binding.generation.to_le_bytes();
    let market_seed = binding.market_instance_id.bytes();
    let (expected, bump) =
        seeds::product_occurrence_root_pda(program_id, &market_seed, binding.generation);
    require(*target.key == expected, ClutchError::WrongPda)?;
    let (expected_work, _) =
        seeds::failure_interval_consensus_work_pda(program_id, &market_seed, binding.generation);
    let (expected_replay, _) =
        seeds::failure_interval_consensus_replay_pda(program_id, &market_seed, binding.generation);
    let (expected_resolution, _) = seeds::resolution_v5_pda(program_id, &market_seed);
    require(
        *interval_work.key == expected_work
            && *interval_replay.key == expected_replay
            && *resolution.key == expected_resolution
            && binding.failure_interval_work_account_id.bytes() == interval_work.key.to_bytes()
            && binding.failure_interval_replay_account_id.bytes() == interval_replay.key.to_bytes()
            && binding.resolution_account_id.bytes() == resolution.key.to_bytes(),
        ClutchError::WrongPda,
    )?;
    let required_principal = [
        rent.minimum_balance(PRODUCT_OCCURRENCE_ROOT_ACCOUNT_BYTES_V1)?,
        rent.minimum_balance(FAILURE_INTERVAL_CONSENSUS_WORK_ACCOUNT_BYTES)?,
        rent.minimum_balance(FAILURE_INTERVAL_CONSENSUS_REPLAY_ACCOUNT_BYTES)?,
        rent.minimum_balance(RESOLUTION_V5_ACCOUNT_BYTES)?,
    ];
    require(
        capitalization.principal_lamports == required_principal
            && capitalization.donation_floor_lamports
                == [
                    target.lamports(),
                    interval_work.lamports(),
                    interval_replay.lamports(),
                    resolution.lamports(),
                ],
        ClutchError::MismatchedState,
    )?;
    for (destination, amount) in [target, interval_work, interval_replay, resolution]
        .into_iter()
        .zip(required_principal)
    {
        let transfer = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &transfer_data(amount),
            vec![
                AccountMeta::new(*payer.key, true),
                AccountMeta::new(*destination.key, false),
            ],
        );
        invoke(
            &transfer,
            &[payer.clone(), destination.clone(), system_program.clone()],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    }
    require(
        target.lamports() == postfund[0]
            && interval_work.lamports() == postfund[1]
            && interval_replay.lamports() == postfund[2]
            && resolution.lamports() == postfund[3],
        ClutchError::MismatchedState,
    )?;
    let bump_seed = [bump];
    let signer_seeds: &[&[u8]] = &[
        seeds::SEED_PRODUCT_OCCURRENCE_ROOT_V1,
        &market_seed,
        &generation_seed,
        &bump_seed,
    ];
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(PRODUCT_OCCURRENCE_ROOT_ACCOUNT_BYTES_V1),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.owner == program_id
            && target.data_len() == PRODUCT_OCCURRENCE_ROOT_ACCOUNT_BYTES_V1
            && target.lamports() == postfund[0]
            && interval_work.owner == &SYSTEM_PROGRAM_ID
            && interval_work.data_is_empty()
            && interval_work.lamports() == postfund[1]
            && interval_replay.owner == &SYSTEM_PROGRAM_ID
            && interval_replay.data_is_empty()
            && interval_replay.lamports() == postfund[2]
            && resolution.owner == &SYSTEM_PROGRAM_ID
            && resolution.data_is_empty()
            && resolution.lamports() == postfund[3],
        ClutchError::MismatchedState,
    )?;
    let value = ProductOccurrenceRootAccountV1 {
        state,
        stored_bump: bump,
    };
    write_root(target, value)?;
    let root = authenticate_product_occurrence_root_v1(
        program_id,
        target,
        binding.market_instance_id,
        binding.series_plan_id,
        binding.generation,
        true,
    )?;
    let funding = authenticate_product_occurrence_capitalization_v1(
        program_id,
        root,
        target,
        interval_work,
        interval_replay,
        resolution,
    )?;
    Ok((root, funding))
}

/// Mint a private, non-mutating close authorization for one live family.
pub fn authorize_product_occurrence_family_close_v1(
    authenticated: AuthenticatedProductOccurrenceRootV1,
    family: ProductOccurrenceFamilyV1,
) -> Outcome<AuthenticatedProductOccurrenceFamilyCloseAuthorizationV1> {
    let state = authenticated.state();
    let counts = state.counts();
    let index = family.index();
    require(
        state.phase() == ProductOccurrencePhaseV1::Retiring && counts.live[index] == 1,
        ClutchError::MismatchedState,
    )?;
    let family_terminal_sequence = counts.terminal[index];
    let root_transition_sequence = state
        .transition_sequence()
        .checked_add(1)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let root_semantic_id = state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let family_byte = [family.byte()];
    let family_sequence = family_terminal_sequence.to_le_bytes();
    let root_sequence = root_transition_sequence.to_le_bytes();
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_OCCURRENCE_FAMILY_CLOSE_AUTHORIZATION_DOMAIN_V1,
            authenticated.account.as_ref(),
            &root_semantic_id.bytes(),
            &state
                .binding()
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes(),
            &family_byte,
            &family_sequence,
            &root_sequence,
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedProductOccurrenceFamilyCloseAuthorizationV1 {
        id,
        root_account: authenticated.account,
        root_semantic_id,
        binding: state.binding(),
        family,
        family_terminal_sequence,
        root_transition_sequence,
    })
}

/// Enter occurrence retirement and atomically persist the exact successor.
pub fn begin_product_occurrence_retirement_v1(
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedProductOccurrenceRootV1,
) -> Outcome<AuthenticatedProductOccurrenceRootV1> {
    require(
        *account.key == authenticated.account,
        ClutchError::MismatchedState,
    )?;
    let next = authenticated
        .value
        .state
        .begin_retirement()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let value = ProductOccurrenceRootAccountV1 {
        state: next,
        ..authenticated.value
    };
    write_root(account, value)?;
    Ok(AuthenticatedProductOccurrenceRootV1 {
        account: authenticated.account,
        value,
    })
}

/// Seal a family-owner projection behind a private SBF-only capability.
///
/// This constructor is crate-private by design. The calling family module is
/// responsible for authenticating the concrete typed terminal receipt before
/// invoking it; no dispatch payload can directly reach this seam.
#[allow(clippy::too_many_arguments)]
pub(crate) fn mint_product_occurrence_family_terminal_v1(
    root: AuthenticatedProductOccurrenceRootV1,
    authorization: AuthenticatedProductOccurrenceFamilyCloseAuthorizationV1,
    owner_program_id: ContentId,
    owner_release_id: ContentId,
    owner_account: &AccountInfo<'_>,
    owner_terminal_receipt_id: ContentId,
    terminal_state_ids: [ContentId; 2],
) -> Outcome<AuthenticatedProductOccurrenceFamilyTerminalV1> {
    let state = root.state();
    require(
        authorization.root_account == root.account
            && authorization.root_semantic_id
                == state
                    .semantic_id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && authorization.binding == state.binding()
            && !owner_account.executable
            && !owner_account.is_signer
            && owner_account.owner.to_bytes() == owner_program_id.bytes()
            && owner_account.key != &root.account,
        ClutchError::MismatchedState,
    )?;
    let projection = ProductOccurrenceFamilyTerminalProjectionV1::new(
        authorization.family,
        state.binding(),
        authorization.family_terminal_sequence,
        authorization.root_transition_sequence,
        owner_program_id,
        owner_release_id,
        ContentId::from_bytes(owner_account.key.to_bytes()),
        owner_terminal_receipt_id,
        terminal_state_ids,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedProductOccurrenceFamilyTerminalV1 { projection })
}

/// Consume one private family-terminal capability and persist both count deltas.
pub fn consume_product_occurrence_family_terminal_v1(
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedProductOccurrenceRootV1,
    terminal: AuthenticatedProductOccurrenceFamilyTerminalV1,
) -> Outcome<AuthenticatedProductOccurrenceRootV1> {
    require(
        *account.key == authenticated.account,
        ClutchError::MismatchedState,
    )?;
    let next = authenticated
        .value
        .state
        .consume_family_terminal(terminal.projection)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let value = ProductOccurrenceRootAccountV1 {
        state: next,
        ..authenticated.value
    };
    write_root(account, value)?;
    Ok(AuthenticatedProductOccurrenceRootV1 {
        account: authenticated.account,
        value,
    })
}

/// Seal a fully counted root and mint the private whole-Market capability.
pub fn finalize_product_occurrence_terminal_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authenticated: AuthenticatedProductOccurrenceRootV1,
) -> Outcome<AuthenticatedMarketInstanceTerminalCapabilityV1> {
    require(
        *account.key == authenticated.account && account.owner == program_id,
        ClutchError::MismatchedState,
    )?;
    let (next, projection) = authenticated
        .value
        .state
        .finalize_terminal()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let value = ProductOccurrenceRootAccountV1 {
        state: next,
        ..authenticated.value
    };
    write_root(account, value)?;
    let rebound = authenticate_product_occurrence_root_v1(
        program_id,
        account,
        projection.binding().market_instance_id,
        projection.binding().series_plan_id,
        projection.binding().generation,
        true,
    )?;
    require(
        rebound.value.state.phase() == ProductOccurrencePhaseV1::Terminal
            && rebound
                .value
                .state
                .semantic_id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == projection.root_semantic_id(),
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            AUTHENTICATED_MARKET_TERMINAL_DOMAIN_V1,
            &account.key.to_bytes(),
            &program_id.to_bytes(),
            &projection.id().bytes(),
            &projection.root_semantic_id().bytes(),
        ])
        .to_bytes(),
    );
    require_live_content_id(id)?;
    Ok(AuthenticatedMarketInstanceTerminalCapabilityV1 {
        id,
        root_account: *account.key,
        owner_program: *program_id,
        projection,
    })
}

fn write_root(account: &AccountInfo<'_>, value: ProductOccurrenceRootAccountV1) -> Outcome<()> {
    require(account.is_writable, ClutchError::NotWritable)?;
    require(
        account.data_len() == PRODUCT_OCCURRENCE_ROOT_ACCOUNT_BYTES_V1,
        ClutchError::WrongDataLength,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    value.encode(&mut data)?;
    Ok(())
}

fn require_live_content_id(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}
