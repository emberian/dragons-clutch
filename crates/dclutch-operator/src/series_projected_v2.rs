//! Host-only construction of compact projected-Market Series Consume data.
//!
//! The caller first derives an exact recurring-Series Consume family request
//! from finalized Template/Occurrence/Ticket/replay observations.  This module
//! wraps that already-admitted header and proof once in the production
//! family-neutral projected executor wire.  It does not construct account
//! authority, sign, submit, or treat the bounded Funding count as attested;
//! current Core promotes that hint only through `SeriesCoreFoundAckV2`.

use dclutch_account_profile_contract::v2::{AccountProfileV2, PhysicalAccountDataGeometryV2};
use dclutch_capability_program_contract::hot_v3::{
    HOT_CONFIG_RAW_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
    HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3,
    HOT_ROOT_ACCOUNT_V3,
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::v2::FixedRole;
use dclutch_series_v3_kernel::request::{
    SERIES_ACTION_HEADER_BYTES_V3, SeriesActionRequestV3, SeriesActionV3,
};
use dclutch_trading_sbf::{
    hot_v3::{HOT_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3, HOT_SHADOW_RUNTIME_ACCOUNTS_START_V3},
    projected_market_v2::{
        PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V2, ProjectedMarketExecutionV2,
        encode_projected_market_execution_v2,
    },
    series::{
        artifacts_v3::SERIES_CONSUME_ROUTE_COUNT_V3, artifacts_v4::SeriesConsumeArtifactBundleV4,
    },
};
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

/// Maximum ordered receipt dependencies on one canonical Series Consume route.
pub const SERIES_PROJECTED_MAX_RECEIPT_DEPENDENCIES_V2: usize = 2;
/// Exact fixed Hot, Shadow-evidence, and caller-authority prefix before runtime.
pub const SERIES_PROJECTED_HOT_PREFIX_ACCOUNT_COUNT_V2: usize =
    HOT_SHADOW_RUNTIME_ACCOUNTS_START_V3;
/// Number of authenticated fixed Hot accounts injected into Profile13 runtime.
pub const SERIES_PROJECTED_INJECTED_RUNTIME_ACCOUNT_COUNT_V2: usize = 5;

const _: () = assert!(HOT_FIXED_ACCOUNT_COUNT_V3 == 38);
const _: () = assert!(HOT_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3 == 44);
const _: () = assert!(SERIES_PROJECTED_HOT_PREFIX_ACCOUNT_COUNT_V2 == 45);

/// Stable refusal from compact projected-Series data construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesProjectedOperatorErrorV2 {
    /// Family bytes were not one exact recurring-Series request.
    Request,
    /// The request selected another action or omitted its occurrence proof.
    Action,
    /// The pre-Core Funding span hint was outside the protocol bound.
    FundingCount,
    /// Checked width arithmetic or canonical projected encoding refused.
    Encoding,
    /// The supplied plan was not one fully joined five-route Series successor.
    Artifact,
    /// Logical account observations did not share one exact chain snapshot.
    Observation,
    /// The authenticated Profile13 logical or physical geometry differed.
    AccountGeometry,
    /// Two logical aliases supplied different keys or account-data widths.
    AliasSubstitution,
    /// A physical representative differed from its authenticated data geometry.
    AccountData,
    /// Fixed Hot, Shadow evidence, caller authority, or injected aliases differed.
    HotPrefix,
}

/// One chain observation at one expanded logical Profile13 coordinate.
///
/// Repeated route-local coordinates deliberately repeat the same observation.
/// The operator uses the authenticated AccountProfile—not caller flags—to
/// collapse these observations into one union-privileged physical meta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesProjectedLogicalAccountV2 {
    /// Same-finalized-observation identity shared by the whole account vector.
    pub observation: ContentId,
    /// Exact observed account public key.
    pub key: Pubkey,
    /// Exact observed pre-execution account-data width.
    pub data_len: usize,
}

/// Complete pre-runtime account observations for one Shadow-selected Hot call.
///
/// Coordinates `0..38` are the canonical common Hot prefix, `38..44` are the
/// six additional Shadow certificate/artifact/deployment observations, and
/// coordinate `44` is the nonsigner Trading caller-authority PDA. Privileges
/// are never supplied here: the operator derives them from the frozen common
/// layout and Profile13.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesProjectedHotPrefixV2<'a> {
    /// Same-snapshot key and data-width observations in exact canonical order.
    pub accounts:
        &'a [SeriesProjectedLogicalAccountV2; SERIES_PROJECTED_HOT_PREFIX_ACCOUNT_COUNT_V2],
}

/// Static ordered receipt expectation owned by the authenticated Effect.
///
/// Runtime provenance additionally binds the current selected producer,
/// invocation, request kind/digest, receipt magic, and exact returned bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesProjectedReceiptDependencyV2 {
    /// Release-selected producer role.
    pub producer_role: FixedRole,
    /// Strictly earlier global route ordinal.
    pub producer_route: u16,
    /// Exact raw return-data width appended by the common receipt bank.
    pub expected_receipt_bytes: u16,
}

/// Exact request and prior-receipt commitment for one global child route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesProjectedRouteCommitmentV2 {
    /// Release-selected child role.
    pub role: FixedRole,
    /// SHA-256 of the fixed child request plus every authenticated borrowed range.
    pub base_request_digest: [u8; 32],
    /// Exact ordered earlier-receipt dependencies; unused entries are `None`.
    pub receipt_dependencies:
        [Option<SeriesProjectedReceiptDependencyV2>; SERIES_PROJECTED_MAX_RECEIPT_DEPENDENCIES_V2],
}

/// Complete unsigned projected-Series instruction derived from one admitted plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsignedSeriesProjectedExecutionV2 {
    instruction: Instruction,
    observation: ContentId,
    effect_digest: [u8; 32],
    routes: [SeriesProjectedRouteCommitmentV2; SERIES_CONSUME_ROUTE_COUNT_V3],
}

impl UnsignedSeriesProjectedExecutionV2 {
    /// Exact unsigned top-level Trading instruction for the packed runtime frame.
    pub const fn instruction(&self) -> &Instruction {
        &self.instruction
    }

    /// Same-finalized observation from which every logical account was read.
    pub const fn observation(&self) -> ContentId {
        self.observation
    }

    /// SHA-256 of the exact admitted DCE5 bytes owning all five routes.
    pub const fn effect_digest(&self) -> [u8; 32] {
        self.effect_digest
    }

    /// Exact ordered child request/receipt commitments for routes zero through four.
    pub const fn routes(
        &self,
    ) -> &[SeriesProjectedRouteCommitmentV2; SERIES_CONSUME_ROUTE_COUNT_V3] {
        &self.routes
    }
}

/// Unsigned compact data for the production projected-Market Trading executor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsignedSeriesProjectedConsumeV2 {
    data: Vec<u8>,
    family_request_digest: [u8; 32],
    funding_count_hint: u8,
}

impl UnsignedSeriesProjectedConsumeV2 {
    /// Borrow the exact projected-executor instruction bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// SHA-256 of the exact Series header and proof supplied once in the wire.
    pub const fn family_request_digest(&self) -> [u8; 32] {
        self.family_request_digest
    }

    /// Bounded routing hint before current Core authenticates the Funding list.
    pub const fn funding_count_hint(&self) -> u8 {
        self.funding_count_hint
    }
}

/// Encode one exact compact projected-Series Consume instruction body.
pub fn build_series_projected_consume_v2(
    family_request: &[u8],
    funding_count_hint: u8,
) -> Result<UnsignedSeriesProjectedConsumeV2, SeriesProjectedOperatorErrorV2> {
    let request = SeriesActionRequestV3::decode(family_request)
        .map_err(|_| SeriesProjectedOperatorErrorV2::Request)?;
    if request.action() != SeriesActionV3::Consume || request.proof_count() == 0 {
        return Err(SeriesProjectedOperatorErrorV2::Action);
    }
    let header: &[u8; SERIES_ACTION_HEADER_BYTES_V3] = family_request
        .get(..SERIES_ACTION_HEADER_BYTES_V3)
        .ok_or(SeriesProjectedOperatorErrorV2::Request)?
        .try_into()
        .map_err(|_| SeriesProjectedOperatorErrorV2::Request)?;
    let witness = family_request
        .get(SERIES_ACTION_HEADER_BYTES_V3..)
        .ok_or(SeriesProjectedOperatorErrorV2::Request)?;
    let width = PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V2
        .checked_add(witness.len())
        .ok_or(SeriesProjectedOperatorErrorV2::Encoding)?;
    let mut data = vec![0_u8; width];
    encode_projected_market_execution_v2(&mut data, header, witness, funding_count_hint)
        .map_err(|error| match error {
            dclutch_trading_sbf::projected_market_v2::ProjectedMarketExecutionErrorV2::NonCanonical => {
                SeriesProjectedOperatorErrorV2::FundingCount
            }
            _ => SeriesProjectedOperatorErrorV2::Encoding,
        })?;
    let decoded = ProjectedMarketExecutionV2::decode(&data)
        .map_err(|_| SeriesProjectedOperatorErrorV2::Encoding)?;
    if decoded.family_request() != family_request
        || decoded.affine_count() != funding_count_hint
        || decoded.witness_words() != request.proof_count()
    {
        return Err(SeriesProjectedOperatorErrorV2::Encoding);
    }
    Ok(UnsignedSeriesProjectedConsumeV2 {
        data,
        family_request_digest: hash(family_request).to_bytes(),
        funding_count_hint,
    })
}

/// Build the complete projected Hot frame from one fully authenticated plan.
///
/// The artifact bundle has already joined the finalized ProgramSet,
/// CapabilityProgramV4, LifecycleV5, AccountProfile, request/transition
/// programs, and DCE5 Effect. `logical_accounts` is the expanded route vector
/// observed at one snapshot; it supplies keys and widths only. Signer/writable
/// bits, aliases, and physical order come solely from the authenticated profile.
pub fn build_series_projected_execution_v2(
    trading_program: Pubkey,
    family_request: &[u8],
    artifacts: SeriesConsumeArtifactBundleV4<'_>,
    hot_prefix: SeriesProjectedHotPrefixV2<'_>,
    logical_accounts: &[SeriesProjectedLogicalAccountV2],
) -> Result<UnsignedSeriesProjectedExecutionV2, SeriesProjectedOperatorErrorV2> {
    require_request_slices(family_request, artifacts)?;
    let funding_count = u8::try_from(artifacts.effect.funding_count_hint())
        .map_err(|_| SeriesProjectedOperatorErrorV2::Artifact)?;
    let projected = build_series_projected_consume_v2(family_request, funding_count)?;
    let observation = logical_accounts
        .first()
        .map(|account| account.observation)
        .ok_or(SeriesProjectedOperatorErrorV2::Observation)?;
    if logical_accounts
        .iter()
        .any(|account| account.observation != observation)
    {
        return Err(SeriesProjectedOperatorErrorV2::Observation);
    }
    let accounts = pack_projected_hot_accounts(
        hot_prefix,
        artifacts.account_profile,
        u32::from(funding_count),
        logical_accounts,
    )?;
    let routes = route_commitments(
        artifacts.effect,
        artifacts.request.proof_count(),
        artifacts.slices.witness.len(),
        family_request,
    )?;
    Ok(UnsignedSeriesProjectedExecutionV2 {
        instruction: Instruction {
            program_id: trading_program,
            accounts,
            data: projected.data().to_vec(),
        },
        observation,
        effect_digest: hash(artifacts.effect.program().bytes()).to_bytes(),
        routes,
    })
}

fn pack_projected_hot_accounts(
    prefix: SeriesProjectedHotPrefixV2<'_>,
    profile: AccountProfileV2<'_>,
    funding_count: u32,
    logical_accounts: &[SeriesProjectedLogicalAccountV2],
) -> Result<Vec<AccountMeta>, SeriesProjectedOperatorErrorV2> {
    let observation = logical_accounts
        .first()
        .map(|account| account.observation)
        .ok_or(SeriesProjectedOperatorErrorV2::Observation)?;
    if prefix
        .accounts
        .iter()
        .any(|account| account.observation != observation)
        || prefix.accounts.iter().enumerate().any(|(index, account)| {
            account.key == Pubkey::default()
                || prefix
                    .accounts
                    .get(..index)
                    .is_some_and(|prior| prior.iter().any(|other| other.key == account.key))
        })
    {
        return Err(SeriesProjectedOperatorErrorV2::HotPrefix);
    }
    for (logical, fixed) in [
        (0, HOT_ROOT_ACCOUNT_V3),
        (1, HOT_CONFIG_RAW_ACCOUNT_V3),
        (2, HOT_PRODUCT_RAW_ACCOUNT_V3),
        (3, HOT_PORTFOLIO_RAW_ACCOUNT_V3),
        (4, HOT_LINKED_BASIS_RAW_ACCOUNT_V3),
    ] {
        if logical_accounts.get(logical) != prefix.accounts.get(fixed) {
            return Err(SeriesProjectedOperatorErrorV2::HotPrefix);
        }
    }
    let caller = prefix
        .accounts
        .get(HOT_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3)
        .ok_or(SeriesProjectedOperatorErrorV2::HotPrefix)?;
    if caller.data_len != 0
        || logical_accounts
            .iter()
            .any(|runtime| runtime.key == caller.key)
    {
        return Err(SeriesProjectedOperatorErrorV2::HotPrefix);
    }

    let runtime = pack_profile13_accounts(profile, funding_count, logical_accounts)?;
    if runtime.len() < SERIES_PROJECTED_INJECTED_RUNTIME_ACCOUNT_COUNT_V2 {
        return Err(SeriesProjectedOperatorErrorV2::AccountGeometry);
    }
    let capacity = SERIES_PROJECTED_HOT_PREFIX_ACCOUNT_COUNT_V2
        .checked_add(runtime.len())
        .and_then(|width| width.checked_sub(SERIES_PROJECTED_INJECTED_RUNTIME_ACCOUNT_COUNT_V2))
        .ok_or(SeriesProjectedOperatorErrorV2::AccountGeometry)?;
    let mut accounts = Vec::with_capacity(capacity);
    for (index, account) in prefix.accounts.iter().enumerate() {
        accounts.push(
            if index == HOT_MARKET_ACCOUNT_V3 || index == HOT_ROOT_ACCOUNT_V3 {
                AccountMeta::new(account.key, false)
            } else {
                AccountMeta::new_readonly(account.key, false)
            },
        );
    }
    accounts.extend(
        runtime
            .into_iter()
            .skip(SERIES_PROJECTED_INJECTED_RUNTIME_ACCOUNT_COUNT_V2),
    );
    if accounts.len() != capacity {
        return Err(SeriesProjectedOperatorErrorV2::HotPrefix);
    }
    Ok(accounts)
}

fn require_request_slices(
    family_request: &[u8],
    artifacts: SeriesConsumeArtifactBundleV4<'_>,
) -> Result<(), SeriesProjectedOperatorErrorV2> {
    let header_len = artifacts.slices.header.len();
    let expected = header_len
        .checked_add(artifacts.slices.witness.len())
        .ok_or(SeriesProjectedOperatorErrorV2::Artifact)?;
    if family_request.len() != expected
        || family_request.get(..header_len) != Some(artifacts.slices.header)
        || family_request.get(header_len..) != Some(artifacts.slices.witness)
    {
        return Err(SeriesProjectedOperatorErrorV2::Artifact);
    }
    Ok(())
}

fn pack_profile13_accounts(
    profile: AccountProfileV2<'_>,
    funding_count: u32,
    logical_accounts: &[SeriesProjectedLogicalAccountV2],
) -> Result<Vec<AccountMeta>, SeriesProjectedOperatorErrorV2> {
    let observation = logical_accounts
        .first()
        .map(|account| account.observation)
        .ok_or(SeriesProjectedOperatorErrorV2::Observation)?;
    if logical_accounts
        .iter()
        .any(|account| account.observation != observation)
    {
        return Err(SeriesProjectedOperatorErrorV2::Observation);
    }
    let spans = [funding_count];
    let logical_count = profile
        .logical_account_count_with_dynamic_spans(0, &spans)
        .map_err(|_| SeriesProjectedOperatorErrorV2::AccountGeometry)?;
    if logical_accounts.len() != logical_count {
        return Err(SeriesProjectedOperatorErrorV2::AccountGeometry);
    }
    let physical_count = profile
        .physical_account_count_with_dynamic_spans(0, &spans)
        .map_err(|_| SeriesProjectedOperatorErrorV2::AccountGeometry)?;
    let mut observations: Vec<Option<SeriesProjectedLogicalAccountV2>> = vec![None; physical_count];
    for (logical, account) in logical_accounts.iter().copied().enumerate() {
        let ordinal = profile
            .physical_account_ordinal_with_dynamic_spans(0, &spans, logical)
            .map_err(|_| SeriesProjectedOperatorErrorV2::AccountGeometry)?;
        let observed = observations
            .get_mut(ordinal)
            .ok_or(SeriesProjectedOperatorErrorV2::AccountGeometry)?;
        if observed.is_some_and(|representative| {
            representative.key != account.key || representative.data_len != account.data_len
        }) {
            return Err(SeriesProjectedOperatorErrorV2::AliasSubstitution);
        }
        *observed = Some(account);
    }
    let mut accounts = Vec::with_capacity(physical_count);
    let mut physical_ordinal = 0_usize;
    while physical_ordinal < physical_count {
        let geometry = profile
            .physical_account_geometry_with_dynamic_spans(0, &spans, physical_ordinal)
            .map_err(|_| SeriesProjectedOperatorErrorV2::AccountGeometry)?;
        let representative = observations
            .get(physical_ordinal)
            .copied()
            .flatten()
            .ok_or(SeriesProjectedOperatorErrorV2::AccountGeometry)?;
        if logical_accounts
            .get(geometry.logical_representative())
            .copied()
            != Some(representative)
        {
            return Err(SeriesProjectedOperatorErrorV2::AccountGeometry);
        }
        require_data_geometry(geometry.data(), representative.data_len)?;
        let privileges = geometry.privileges();
        accounts.push(if privileges.writable() {
            AccountMeta::new(representative.key, privileges.signer())
        } else {
            AccountMeta::new_readonly(representative.key, privileges.signer())
        });
        physical_ordinal = physical_ordinal
            .checked_add(1)
            .ok_or(SeriesProjectedOperatorErrorV2::AccountGeometry)?;
    }
    Ok(accounts)
}

fn require_data_geometry(
    geometry: PhysicalAccountDataGeometryV2,
    observed: usize,
) -> Result<(), SeriesProjectedOperatorErrorV2> {
    let accepted = match geometry {
        PhysicalAccountDataGeometryV2::Exact { bytes } => observed == bytes,
        PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes } => {
            observed == 0 || observed == live_bytes
        }
        PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
            observed >= minimum_bytes
        }
        PhysicalAccountDataGeometryV2::Opaque => true,
    };
    if accepted {
        Ok(())
    } else {
        Err(SeriesProjectedOperatorErrorV2::AccountData)
    }
}

fn route_commitments(
    effect: dclutch_trading_sbf::series::effect_v4::SeriesConsumeEffectV4<'_>,
    proof_count: u8,
    witness_len: usize,
    family_request: &[u8],
) -> Result<
    [SeriesProjectedRouteCommitmentV2; SERIES_CONSUME_ROUTE_COUNT_V3],
    SeriesProjectedOperatorErrorV2,
> {
    let program = effect.program();
    let base = program.base();
    if usize::from(base.route_count()) != SERIES_CONSUME_ROUTE_COUNT_V3 {
        return Err(SeriesProjectedOperatorErrorV2::Artifact);
    }
    let scalars = [
        u64::try_from(SERIES_ACTION_HEADER_BYTES_V3)
            .map_err(|_| SeriesProjectedOperatorErrorV2::Artifact)?,
        u64::try_from(witness_len).map_err(|_| SeriesProjectedOperatorErrorV2::Artifact)?,
        u64::from(proof_count),
        32,
        u64::from(effect.funding_count_hint()),
    ];
    let mut output = [SeriesProjectedRouteCommitmentV2 {
        role: FixedRole::Core,
        base_request_digest: [0; 32],
        receipt_dependencies: [None; SERIES_PROJECTED_MAX_RECEIPT_DEPENDENCIES_V2],
    }; SERIES_CONSUME_ROUTE_COUNT_V3];
    let mut route_index = 0_u16;
    while usize::from(route_index) < SERIES_CONSUME_ROUTE_COUNT_V3 {
        let route = base
            .route(route_index)
            .map_err(|_| SeriesProjectedOperatorErrorV2::Artifact)?;
        let template = base
            .route_template(route_index)
            .map_err(|_| SeriesProjectedOperatorErrorV2::Artifact)?
            .0;
        let borrowed_count = program
            .borrowed_range_count_for_route(route_index)
            .map_err(|_| SeriesProjectedOperatorErrorV2::Artifact)?;
        let mut request_parts = Vec::with_capacity(usize::from(borrowed_count) + 1);
        request_parts.push(template);
        let mut borrowed = 0_u16;
        while borrowed < borrowed_count {
            let range = program
                .resolved_borrowed_range(route_index, borrowed, &scalars)
                .map_err(|_| SeriesProjectedOperatorErrorV2::Artifact)?;
            request_parts.push(
                range
                    .slice(family_request)
                    .map_err(|_| SeriesProjectedOperatorErrorV2::Artifact)?,
            );
            borrowed = borrowed
                .checked_add(1)
                .ok_or(SeriesProjectedOperatorErrorV2::Artifact)?;
        }
        let dependency_count = usize::from(route.receipt_dependency_count());
        if dependency_count > SERIES_PROJECTED_MAX_RECEIPT_DEPENDENCIES_V2 {
            return Err(SeriesProjectedOperatorErrorV2::Artifact);
        }
        let mut dependencies = [None; SERIES_PROJECTED_MAX_RECEIPT_DEPENDENCIES_V2];
        let mut dependency = 0_u16;
        while usize::from(dependency) < dependency_count {
            let exact = base
                .route_receipt_dependency(route_index, dependency)
                .map_err(|_| SeriesProjectedOperatorErrorV2::Artifact)?;
            *dependencies
                .get_mut(usize::from(dependency))
                .ok_or(SeriesProjectedOperatorErrorV2::Artifact)? =
                Some(SeriesProjectedReceiptDependencyV2 {
                    producer_role: exact.producer_role(),
                    producer_route: exact.producer_route(),
                    expected_receipt_bytes: exact.expected_receipt_bytes(),
                });
            dependency = dependency
                .checked_add(1)
                .ok_or(SeriesProjectedOperatorErrorV2::Artifact)?;
        }
        *output
            .get_mut(usize::from(route_index))
            .ok_or(SeriesProjectedOperatorErrorV2::Artifact)? = SeriesProjectedRouteCommitmentV2 {
            role: route.role(),
            base_request_digest: hashv(&request_parts).to_bytes(),
            receipt_dependencies: dependencies,
        };
        route_index = route_index
            .checked_add(1)
            .ok_or(SeriesProjectedOperatorErrorV2::Artifact)?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use dclutch_account_profile_contract::v2::{AccountProfileV2, PhysicalAccountDataGeometryV2};
    use dclutch_core_contract::ContentId;
    use dclutch_series_v3_kernel::request::encode_series_action_header_v3;
    use dclutch_trading_sbf::series::{
        account_profile_v4::{
            SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4, SeriesConsumeAccountProfileInputV4,
            encode_series_consume_account_profile_v4_atomic,
        },
        artifacts_v3::{
            SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3, SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
            SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
        },
        consume_artifacts_v4::{
            SERIES_CONSUME_BASE_EFFECT_BYTES_V4, SERIES_CONSUME_EFFECT_BYTES_V4,
            SeriesConsumeChildRequestsV4, encode_series_consume_effect_v4_from_requests_atomic,
        },
        effect_v4::SeriesConsumeEffectV4,
    };
    use solana_compute_budget_interface::ComputeBudgetInstruction;
    use solana_hash::Hash;
    use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
    use solana_program::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
    };

    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("identity")
    }

    fn request_with_proof(action: SeriesActionV3, proof_count: u8) -> Vec<u8> {
        let occurrence_bound = action.occurrence_bound();
        let header = encode_series_action_header_v3(
            action,
            id(1),
            occurrence_bound.then_some(id(2)),
            (action != SeriesActionV3::Close).then_some(id(3)),
            4,
            if matches!(action, SeriesActionV3::Prepare | SeriesActionV3::Close) {
                0
            } else {
                5
            },
            if occurrence_bound { proof_count } else { 0 },
        )
        .expect("header");
        let mut bytes = header.to_vec();
        if occurrence_bound {
            bytes.extend_from_slice(&vec![9; usize::from(proof_count) * 32]);
        }
        bytes
    }

    fn request(action: SeriesActionV3) -> Vec<u8> {
        request_with_proof(action, 2)
    }

    fn profile() -> Vec<u8> {
        let lengths = [0_u32; 157];
        let mut scratch = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        let mut output = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        encode_series_consume_account_profile_v4_atomic(
            SeriesConsumeAccountProfileInputV4 {
                fixed_data_lengths: &lengths,
            },
            &mut scratch,
            &mut output,
        )
        .expect("canonical Series Profile13");
        output
    }

    fn logical_accounts(
        profile: AccountProfileV2<'_>,
        funding_count: u32,
    ) -> Vec<SeriesProjectedLogicalAccountV2> {
        let spans = [funding_count];
        let logical_count = profile
            .logical_account_count_with_dynamic_spans(0, &spans)
            .expect("logical account count");
        (0..logical_count)
            .map(|logical| {
                let ordinal = profile
                    .physical_account_ordinal_with_dynamic_spans(0, &spans, logical)
                    .expect("physical ordinal");
                let geometry = profile
                    .physical_account_geometry_with_dynamic_spans(0, &spans, ordinal)
                    .expect("physical geometry");
                let data_len = match geometry.data() {
                    PhysicalAccountDataGeometryV2::Exact { bytes } => bytes,
                    PhysicalAccountDataGeometryV2::VacantOrExact { .. }
                    | PhysicalAccountDataGeometryV2::Opaque => 0,
                    PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable {
                        minimum_bytes,
                    } => minimum_bytes,
                };
                SeriesProjectedLogicalAccountV2 {
                    observation: id(240),
                    key: Pubkey::new_from_array(
                        [u8::try_from(ordinal + 1).expect("bounded physical ordinal"); 32],
                    ),
                    data_len,
                }
            })
            .collect()
    }

    fn hot_prefix(
        logical: &[SeriesProjectedLogicalAccountV2],
    ) -> [SeriesProjectedLogicalAccountV2; SERIES_PROJECTED_HOT_PREFIX_ACCOUNT_COUNT_V2] {
        let observation = id(240);
        let mut prefix = core::array::from_fn(|index| SeriesProjectedLogicalAccountV2 {
            observation,
            key: Pubkey::new_from_array(
                [u8::try_from(index + 180).expect("bounded Hot prefix index"); 32],
            ),
            data_len: 64,
        });
        for (runtime, fixed) in [
            (0, HOT_ROOT_ACCOUNT_V3),
            (1, HOT_CONFIG_RAW_ACCOUNT_V3),
            (2, HOT_PRODUCT_RAW_ACCOUNT_V3),
            (3, HOT_PORTFOLIO_RAW_ACCOUNT_V3),
            (4, HOT_LINKED_BASIS_RAW_ACCOUNT_V3),
        ] {
            prefix[fixed] = logical[runtime];
        }
        prefix[HOT_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3].data_len = 0;
        prefix
    }

    fn effect_bytes() -> Vec<u8> {
        let lock = [0x11; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3];
        let core = [0x22; SERIES_CONSUME_CORE_REQUEST_BYTES_V3];
        let realize = [0x33; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3];
        let claims = [0x44; SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3];
        let mut base_scratch = vec![0_u8; SERIES_CONSUME_BASE_EFFECT_BYTES_V4];
        let mut base = vec![0_u8; SERIES_CONSUME_BASE_EFFECT_BYTES_V4];
        let mut scratch = vec![0_u8; SERIES_CONSUME_EFFECT_BYTES_V4];
        let mut output = vec![0_u8; SERIES_CONSUME_EFFECT_BYTES_V4];
        encode_series_consume_effect_v4_from_requests_atomic(
            SeriesConsumeChildRequestsV4 {
                lock: &lock,
                core: &core,
                realize: &realize,
                claims: &claims,
            },
            &mut base_scratch,
            &mut base,
            &mut scratch,
            &mut output,
        )
        .expect("canonical Series DCE5");
        output
    }

    #[test]
    fn consume_header_and_proof_are_encoded_once() {
        let family_request = request(SeriesActionV3::Consume);
        let built = build_series_projected_consume_v2(&family_request, 3).expect("projected data");
        let decoded = ProjectedMarketExecutionV2::decode(built.data()).expect("decode");
        assert_eq!(decoded.family_request(), family_request);
        assert_eq!(decoded.witness_words(), 2);
        assert_eq!(decoded.affine_count(), 3);
        assert_eq!(
            built.family_request_digest(),
            hash(&family_request).to_bytes()
        );
        assert_eq!(built.data().len(), 208);
    }

    #[test]
    fn action_funding_and_padding_substitution_refuse() {
        assert_eq!(
            build_series_projected_consume_v2(&request(SeriesActionV3::Prepare), 3),
            Err(SeriesProjectedOperatorErrorV2::Action)
        );
        let consume = request(SeriesActionV3::Consume);
        for count in [0, 17] {
            assert_eq!(
                build_series_projected_consume_v2(&consume, count),
                Err(SeriesProjectedOperatorErrorV2::FundingCount)
            );
        }
        let mut padded = consume;
        padded.push(0);
        assert_eq!(
            build_series_projected_consume_v2(&padded, 3),
            Err(SeriesProjectedOperatorErrorV2::Request)
        );
    }

    #[test]
    fn profile13_packs_chain_observations_with_owned_union_privileges() {
        const FUNDING_COUNT: u32 = 7;
        let profile_bytes = profile();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("Profile13");
        let logical = logical_accounts(profile, FUNDING_COUNT);
        let packed = pack_profile13_accounts(profile, FUNDING_COUNT, &logical)
            .expect("packed physical frame");
        assert_eq!(packed.len(), 71);
        for (ordinal, meta) in packed.iter().enumerate() {
            let geometry = profile
                .physical_account_geometry_with_dynamic_spans(0, &[FUNDING_COUNT], ordinal)
                .expect("geometry");
            assert_eq!(meta.pubkey, logical[geometry.logical_representative()].key);
            assert_eq!(meta.is_signer, geometry.privileges().signer());
            assert_eq!(meta.is_writable, geometry.privileges().writable());
        }
    }

    #[test]
    fn complete_projected_hot_frame_owns_prefix_and_skips_injected_aliases() {
        const FUNDING_COUNT: u32 = 7;
        let profile_bytes = profile();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("Profile13");
        let logical = logical_accounts(profile, FUNDING_COUNT);
        let prefix = hot_prefix(&logical);
        let packed = pack_projected_hot_accounts(
            SeriesProjectedHotPrefixV2 { accounts: &prefix },
            profile,
            FUNDING_COUNT,
            &logical,
        )
        .expect("complete projected Hot frame");
        assert_eq!(
            packed.len(),
            104 + usize::try_from(FUNDING_COUNT).expect("count")
        );
        assert!(packed[HOT_MARKET_ACCOUNT_V3].is_writable);
        assert!(packed[HOT_ROOT_ACCOUNT_V3].is_writable);
        assert!(!packed[HOT_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3].is_signer);
        assert!(!packed[HOT_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3].is_writable);
        assert_eq!(packed[0].pubkey, prefix[0].key);
        assert_eq!(packed[44].pubkey, prefix[44].key);
        assert!(packed.get(45).is_some());
    }

    #[test]
    fn complete_projected_hot_frame_refuses_prefix_and_caller_substitution() {
        const FUNDING_COUNT: u32 = 7;
        let profile_bytes = profile();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("Profile13");
        let logical = logical_accounts(profile, FUNDING_COUNT);
        let prefix = hot_prefix(&logical);

        let mut wrong_injected = prefix;
        wrong_injected[HOT_ROOT_ACCOUNT_V3].key = Pubkey::new_from_array([249; 32]);
        assert_eq!(
            pack_projected_hot_accounts(
                SeriesProjectedHotPrefixV2 {
                    accounts: &wrong_injected,
                },
                profile,
                FUNDING_COUNT,
                &logical,
            ),
            Err(SeriesProjectedOperatorErrorV2::HotPrefix)
        );

        let mut duplicate = prefix;
        duplicate[2].key = duplicate[3].key;
        assert_eq!(
            pack_projected_hot_accounts(
                SeriesProjectedHotPrefixV2 {
                    accounts: &duplicate,
                },
                profile,
                FUNDING_COUNT,
                &logical,
            ),
            Err(SeriesProjectedOperatorErrorV2::HotPrefix)
        );

        let mut caller_data = prefix;
        caller_data[HOT_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3].data_len = 1;
        assert_eq!(
            pack_projected_hot_accounts(
                SeriesProjectedHotPrefixV2 {
                    accounts: &caller_data,
                },
                profile,
                FUNDING_COUNT,
                &logical,
            ),
            Err(SeriesProjectedOperatorErrorV2::HotPrefix)
        );
    }

    #[test]
    fn profile13_refuses_alias_observation_and_data_substitution() {
        const FUNDING_COUNT: u32 = 7;
        let profile_bytes = profile();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("Profile13");
        let logical = logical_accounts(profile, FUNDING_COUNT);

        let mut wrong_alias = logical.clone();
        wrong_alias[20].key = Pubkey::new_from_array([251; 32]);
        assert_eq!(
            pack_profile13_accounts(profile, FUNDING_COUNT, &wrong_alias),
            Err(SeriesProjectedOperatorErrorV2::AliasSubstitution)
        );

        let mut wrong_observation = logical.clone();
        wrong_observation[1].observation = id(241);
        assert_eq!(
            pack_profile13_accounts(profile, FUNDING_COUNT, &wrong_observation),
            Err(SeriesProjectedOperatorErrorV2::Observation)
        );

        let mut wrong_data = logical;
        wrong_data[1].data_len = wrong_data[1].data_len.saturating_add(1);
        assert_eq!(
            pack_profile13_accounts(profile, FUNDING_COUNT, &wrong_data),
            Err(SeriesProjectedOperatorErrorV2::AccountData)
        );
    }

    #[test]
    fn admitted_effect_owns_child_request_and_receipt_commitments() {
        const FUNDING_COUNT: u16 = 7;
        let family_request = request_with_proof(SeriesActionV3::Consume, 9);
        let effect_bytes = effect_bytes();
        let scalars = [128, 288, 9, 32, u64::from(FUNDING_COUNT)];
        let identities = [[9_u8; 32]];
        let effect = SeriesConsumeEffectV4::decode(
            &effect_bytes,
            &family_request,
            0,
            &scalars,
            &identities,
            FUNDING_COUNT,
        )
        .expect("Series DCE5");
        let routes = route_commitments(effect, 9, 288, &family_request).expect("route commitments");
        assert_eq!(
            routes.map(|route| route.role),
            [
                FixedRole::Custody,
                FixedRole::Core,
                FixedRole::Custody,
                FixedRole::Claims,
                FixedRole::Core,
            ]
        );
        assert_eq!(
            routes.map(|route| { route.receipt_dependencies.into_iter().flatten().count() }),
            [0, 1, 0, 2, 1]
        );
        assert_eq!(
            routes[1].receipt_dependencies[0].map(|dep| dep.producer_route),
            Some(0)
        );
        assert_eq!(
            routes[3].receipt_dependencies[0].map(|dep| dep.producer_route),
            Some(0)
        );
        assert_eq!(
            routes[3].receipt_dependencies[1].map(|dep| dep.producer_route),
            Some(2)
        );
        assert_eq!(
            routes[4].receipt_dependencies[0].map(|dep| dep.producer_route),
            Some(3)
        );
        assert_eq!(routes[1].base_request_digest, routes[4].base_request_digest);
        assert_ne!(routes[0].base_request_digest, routes[2].base_request_digest);
    }

    #[test]
    fn maximum_projected_runtime_subframe_has_a_v0_packet_margin() {
        const MAXIMUM_FUNDING_COUNT: u32 = 16;
        const SOLANA_PACKET_BYTES: usize = 1_232;
        const REQUIRED_PACKET_MARGIN: usize = 256;

        let profile_bytes = profile();
        let profile = AccountProfileV2::decode(&profile_bytes).expect("Profile13 decode");
        let physical_accounts = profile
            .physical_account_count_with_dynamic_spans(0, &[MAXIMUM_FUNDING_COUNT])
            .expect("physical account count");
        assert_eq!(physical_accounts, 80);

        let family_request = request_with_proof(SeriesActionV3::Consume, 9);
        let projected = build_series_projected_consume_v2(
            &family_request,
            u8::try_from(MAXIMUM_FUNDING_COUNT).expect("bounded count"),
        )
        .expect("maximum projected Consume");
        assert_eq!(projected.data().len(), 432);

        let payer = Pubkey::new_from_array([1; 32]);
        let trading_program = Pubkey::new_from_array([2; 32]);
        let addresses = (0..physical_accounts)
            .map(|index| {
                Pubkey::new_from_array(
                    [u8::try_from(index + 3).expect("bounded representative index"); 32],
                )
            })
            .collect::<Vec<_>>();
        let accounts = addresses
            .iter()
            .enumerate()
            .map(|(index, key)| {
                if index.is_multiple_of(5) {
                    AccountMeta::new(*key, false)
                } else {
                    AccountMeta::new_readonly(*key, false)
                }
            })
            .collect::<Vec<_>>();
        let instruction = Instruction {
            program_id: trading_program,
            accounts,
            data: projected.data().to_vec(),
        };
        let lookup = AddressLookupTableAccount {
            key: Pubkey::new_from_array([254; 32]),
            addresses,
        };
        let message = v0::Message::try_compile(
            &payer,
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
                ComputeBudgetInstruction::set_compute_unit_price(1),
                instruction,
            ],
            &[lookup],
            Hash::new_from_array([255; 32]),
        )
        .expect("maximum projected Consume v0 message");
        assert_eq!(message.account_keys.len(), 3);
        assert_eq!(message.address_table_lookups.len(), 1);
        let loaded_addresses = message.address_table_lookups[0].writable_indexes.len()
            + message.address_table_lookups[0].readonly_indexes.len();
        assert_eq!(loaded_addresses, physical_accounts);
        let required_signatures = usize::from(message.header.num_required_signatures);
        let wire_bytes =
            1 + required_signatures * 64 + VersionedMessage::V0(message).serialize().len();
        assert_eq!(wire_bytes, 850);
        assert!(
            wire_bytes + REQUIRED_PACKET_MARGIN <= SOLANA_PACKET_BYTES,
            "{wire_bytes}B runtime-subframe packet leaves less than {REQUIRED_PACKET_MARGIN}B margin"
        );
    }
}
