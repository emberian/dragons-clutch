//! Same-snapshot authentication of Fractional generic artifacts and Product state.

use dclutch_account_profile_contract::{
    lifecycle_v3::SUCCESSOR_SCHEMA_RELEASE_ID, v2::SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID,
};
use dclutch_capability_program_contract::v3::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID;
use dclutch_effect_kernel::v3::SCHEMA_RELEASE_ID as EFFECT_PROGRAM_SCHEMA_ID;
use dclutch_execution_strategy_contract::v2::EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2;
use dclutch_fractional_claim_contract::{
    ArtifactAdmissionV1, FractionalArtifactAdmissionsV1, FractionalArtifactBundleV1,
    FractionalArtifactBytesV1, FractionalArtifactSelectionV1, FractionalChildProgramsV1,
    FractionalFamilyRequestV1, FractionalRootV1, authenticate_fractional_artifact_bundle_v1,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_TERMS_SCHEMA_ID_V1, FractionalTermsAdmissionV1, FractionalTermsV1,
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_request_profile_contract::SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID;
use dclutch_resolution_core_v3_operator::product_graph_observation_v3::{
    FinalizedProductGraphAccountsV3, authenticate_product_graph_observation_v3,
};
use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};
use dclutch_token_svm::TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2;
use dclutch_transition_vm::v3::SCHEMA_RELEASE_ID as TRANSITION_PROGRAM_SCHEMA_ID;
use sha2::{Digest, Sha256};
use solana_program::{
    account_info::AccountInfo, pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{Error, FractionalRequestContextV1, Result};

/// One finalized raw/staging Record coordinate observed from chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalArtifactRecordSnapshotV1<'a> {
    /// Registry-owned exact raw bytes.
    pub raw: &'a ObservedAccount,
    /// Canonical vacant System-owned staging cursor; lamport dust is accepted.
    pub staging: &'a ObservedAccount,
}

/// Exact authenticated result for one finalized artifact Record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizedArtifactRecordV1<'a> {
    /// Finalized schema identity.
    pub schema: [u8; 32],
    /// SHA-256 over the exact raw bytes.
    pub digest: [u8; 32],
    /// Exact Registry-owned raw body.
    pub bytes: &'a [u8],
}

/// Checked multiprogram release projection supplied by a release verifier.
///
/// This type cannot prove Loader currency by itself. Its constructor requires
/// the separately named `release_authenticated` boundary and the chain reader
/// still requires every selected program account at the common observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedFractionalReleaseInputV1 {
    /// Digest of the user-checked multiprogram release manifest.
    pub checked_manifest_digest: [u8; 32],
    /// Selected Fractional CapabilityProgramV3 content identity.
    pub descriptor_id: [u8; 32],
    /// Selected immutable Fractional terms identity.
    pub terms_id: [u8; 32],
    /// Selected physical child-frame profile identity.
    pub physical_profile: [u8; 32],
    /// Current Registry program.
    pub registry_program: Pubkey,
    /// Current Core program.
    pub core_program: Pubkey,
    /// Current family-neutral Trading program.
    pub trading_program: Pubkey,
    /// Current Claims program.
    pub claims_program: Pubkey,
    /// Current Custody program selected behind Claims.
    pub custody_program: Pubkey,
    /// Current selected Token program.
    pub token_program: Pubkey,
    /// Exact ArtifactRelease and Loader observations were checked externally.
    pub release_authenticated: bool,
}

/// Nonzero, nonaliased checked release coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedFractionalReleaseV1(CheckedFractionalReleaseInputV1);

impl CheckedFractionalReleaseV1 {
    /// Validate a separately checked multiprogram release projection.
    pub fn new(input: CheckedFractionalReleaseInputV1) -> Result<Self> {
        let programs = [
            input.registry_program,
            input.core_program,
            input.trading_program,
            input.claims_program,
            input.custody_program,
            input.token_program,
        ];
        if !input.release_authenticated
            || [
                input.checked_manifest_digest,
                input.descriptor_id,
                input.terms_id,
                input.physical_profile,
            ]
            .iter()
            .any(is_zero)
            || programs.iter().any(|program| *program == Pubkey::default())
            || has_duplicate_program(&programs)
        {
            return Err(Error::ChainArtifacts);
        }
        Ok(Self(input))
    }

    /// Checked release-manifest digest.
    pub const fn checked_manifest_digest(self) -> [u8; 32] {
        self.0.checked_manifest_digest
    }

    /// Selected descriptor digest.
    pub const fn descriptor_id(self) -> [u8; 32] {
        self.0.descriptor_id
    }

    /// Selected terms digest.
    pub const fn terms_id(self) -> [u8; 32] {
        self.0.terms_id
    }

    /// Selected physical profile digest.
    pub const fn physical_profile(self) -> [u8; 32] {
        self.0.physical_profile
    }

    /// Registry program.
    pub const fn registry_program(self) -> Pubkey {
        self.0.registry_program
    }

    /// Core program.
    pub const fn core_program(self) -> Pubkey {
        self.0.core_program
    }

    /// Trading program.
    pub const fn trading_program(self) -> Pubkey {
        self.0.trading_program
    }

    /// Claims program.
    pub const fn claims_program(self) -> Pubkey {
        self.0.claims_program
    }

    /// Custody program.
    pub const fn custody_program(self) -> Pubkey {
        self.0.custody_program
    }

    /// Token program.
    pub const fn token_program(self) -> Pubkey {
        self.0.token_program
    }
}

/// Same-finalized chain snapshot for every Fractional semantic authority.
#[derive(Clone, Copy, Debug)]
pub struct FractionalChainArtifactSnapshotV1<'a> {
    /// Registry executable.
    pub registry_program: &'a ObservedAccount,
    /// Core executable owning the Market.
    pub core_program: &'a ObservedAccount,
    /// Trading executable owning the minimal Fractional root.
    pub trading_program: &'a ObservedAccount,
    /// Claims executable selected for the sole native-liability route.
    pub claims_program: &'a ObservedAccount,
    /// Custody executable selected behind Claims.
    pub custody_program: &'a ObservedAccount,
    /// Exact Token executable selected by TokenBehaviorV2.
    pub token_program: &'a ObservedAccount,
    /// Canonical Rent sysvar used for every raw/root exemption check.
    pub rent_sysvar: &'a ObservedAccount,
    /// Exact Core Market state.
    pub market: &'a ObservedAccount,
    /// Product Runtime V2 root raw Record.
    pub product_raw: &'a ObservedAccount,
    /// Vacant Product root staging cursor.
    pub product_staging: &'a ObservedAccount,
    /// Product-owned ResultDomain raw Record.
    pub result_domain_raw: &'a ObservedAccount,
    /// Vacant ResultDomain staging cursor.
    pub result_domain_staging: &'a ObservedAccount,
    /// Product-owned Portfolio raw Record.
    pub portfolio_raw: &'a ObservedAccount,
    /// Vacant Portfolio staging cursor.
    pub portfolio_staging: &'a ObservedAccount,
    /// Fractional capability descriptor.
    pub descriptor: FractionalArtifactRecordSnapshotV1<'a>,
    /// Immutable Fractional terms.
    pub terms: FractionalArtifactRecordSnapshotV1<'a>,
    /// Selected TokenBehaviorV2.
    pub token_behavior: FractionalArtifactRecordSnapshotV1<'a>,
    /// Generic AccountProfile.
    pub account_profile: FractionalArtifactRecordSnapshotV1<'a>,
    /// Generic StateLifecyclePolicyV4.
    pub lifecycle: FractionalArtifactRecordSnapshotV1<'a>,
    /// Generic RequestProfile.
    pub request: FractionalArtifactRecordSnapshotV1<'a>,
    /// Interpreted ExecutionStrategyV2.
    pub strategy: FractionalArtifactRecordSnapshotV1<'a>,
    /// Generic TransitionVM V3 program.
    pub transition: FractionalArtifactRecordSnapshotV1<'a>,
    /// Generic EffectProgram V3.
    pub effect: FractionalArtifactRecordSnapshotV1<'a>,
    /// Minimal Trading-owned replay/rent root.
    pub root: &'a ObservedAccount,
}

/// Authenticated chain state ready for action planning and request-specific admission.
#[derive(Clone, Copy, Debug)]
pub struct FractionalPreparedChainArtifactsV1<'a> {
    checked: CheckedFractionalReleaseV1,
    observation: Observation,
    root: FractionalRootV1,
    context: FractionalRequestContextV1,
    selection: FractionalArtifactSelectionV1,
    admissions: FractionalArtifactAdmissionsV1,
    artifacts: FractionalArtifactBytesV1<'a>,
    terms: FractionalTermsV1<'a>,
}

impl<'a> FractionalPreparedChainArtifactsV1<'a> {
    /// Common finalized observation.
    pub const fn observation(self) -> Observation {
        self.observation
    }

    /// Minimal authenticated root.
    pub const fn root(self) -> FractionalRootV1 {
        self.root
    }

    /// Kernel-owned exact terms.
    pub const fn terms(self) -> FractionalTermsV1<'a> {
        self.terms
    }

    /// Exact request context derived from Market/Product/terms Records.
    pub const fn request_context(self) -> FractionalRequestContextV1 {
        self.context
    }

    /// Checked multiprogram release boundary.
    pub const fn checked_release(self) -> CheckedFractionalReleaseV1 {
        self.checked
    }
}

/// Authenticate every finalized artifact and Product coordinate before an
/// action request is allowed to select a generic program bundle.
pub fn prepare_fractional_chain_artifacts_v1<'a>(
    snapshot: FractionalChainArtifactSnapshotV1<'a>,
    checked: CheckedFractionalReleaseV1,
) -> Result<FractionalPreparedChainArtifactsV1<'a>> {
    let observation = same_finalized_observation(snapshot)?;
    require_program(
        snapshot.registry_program,
        checked.registry_program(),
        observation,
    )?;
    require_program(snapshot.core_program, checked.core_program(), observation)?;
    require_program(
        snapshot.trading_program,
        checked.trading_program(),
        observation,
    )?;
    require_program(
        snapshot.claims_program,
        checked.claims_program(),
        observation,
    )?;
    require_program(
        snapshot.custody_program,
        checked.custody_program(),
        observation,
    )?;
    require_program(snapshot.token_program, checked.token_program(), observation)?;
    let rent = decode_rent(snapshot.rent_sysvar)?;

    let market = authenticate_market(snapshot, checked, &rent)?;
    let product = authenticate_product_graph_observation_v3(FinalizedProductGraphAccountsV3 {
        registry_program: checked.registry_program(),
        product_raw: snapshot.product_raw,
        product_staging: snapshot.product_staging,
        domain_raw: snapshot.result_domain_raw,
        domain_staging: snapshot.result_domain_staging,
        portfolio_raw: snapshot.portfolio_raw,
        portfolio_staging: snapshot.portfolio_staging,
    })
    .map_err(|_| Error::ChainArtifacts)?;
    for (pair, schema) in [
        (
            FractionalArtifactRecordSnapshotV1 {
                raw: snapshot.product_raw,
                staging: snapshot.product_staging,
            },
            PRODUCT_RECORD_SCHEMA_ID_V2,
        ),
        (
            FractionalArtifactRecordSnapshotV1 {
                raw: snapshot.result_domain_raw,
                staging: snapshot.result_domain_staging,
            },
            RESULT_DOMAIN_SCHEMA_ID_V2,
        ),
        (
            FractionalArtifactRecordSnapshotV1 {
                raw: snapshot.portfolio_raw,
                staging: snapshot.portfolio_staging,
            },
            PORTFOLIO_SCHEMA_ID_V2,
        ),
    ] {
        finalized_record(pair, checked.registry_program(), schema, None, &rent)?;
    }
    if product.product_record != market.identity.product_record.to_bytes() {
        return Err(Error::ChainArtifacts);
    }

    let descriptor = finalized_record(
        snapshot.descriptor,
        checked.registry_program(),
        CAPABILITY_PROGRAM_SCHEMA_ID,
        Some(checked.descriptor_id()),
        &rent,
    )?;
    let terms_record = finalized_record(
        snapshot.terms,
        checked.registry_program(),
        FRACTIONAL_TERMS_SCHEMA_ID_V1,
        Some(checked.terms_id()),
        &rent,
    )?;
    let terms = FractionalTermsV1::decode(
        terms_record.bytes,
        FractionalTermsAdmissionV1 {
            selected_schema_id: FRACTIONAL_TERMS_SCHEMA_ID_V1,
            finalized_schema_id: FRACTIONAL_TERMS_SCHEMA_ID_V1,
            selected_terms_id: terms_record.digest,
            finalized_terms_id: terms_record.digest,
            recomputed_terms_digest: terms_record.digest,
            finalized_terms_digest: terms_record.digest,
            record_authenticated: true,
        },
    )
    .map_err(|_| Error::ChainArtifacts)?;
    let token_behavior = finalized_record(
        snapshot.token_behavior,
        checked.registry_program(),
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        Some(terms.token_behavior_selection_id()),
        &rent,
    )?;
    let account_profile = finalized_record(
        snapshot.account_profile,
        checked.registry_program(),
        ACCOUNT_PROFILE_SCHEMA_ID,
        None,
        &rent,
    )?;
    let lifecycle = finalized_record(
        snapshot.lifecycle,
        checked.registry_program(),
        SUCCESSOR_SCHEMA_RELEASE_ID,
        None,
        &rent,
    )?;
    let request = finalized_record(
        snapshot.request,
        checked.registry_program(),
        REQUEST_PROFILE_SCHEMA_ID,
        None,
        &rent,
    )?;
    let strategy = finalized_record(
        snapshot.strategy,
        checked.registry_program(),
        EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
        None,
        &rent,
    )?;
    let transition = finalized_record(
        snapshot.transition,
        checked.registry_program(),
        TRANSITION_PROGRAM_SCHEMA_ID,
        None,
        &rent,
    )?;
    let effect = finalized_record(
        snapshot.effect,
        checked.registry_program(),
        EFFECT_PROGRAM_SCHEMA_ID,
        None,
        &rent,
    )?;

    let result_domain = digest(&snapshot.result_domain_raw.data);
    if terms.market_id() != snapshot.market.key.to_bytes()
        || terms.result_domain_id() != result_domain
        || terms.release_set_id() != market.identity.selected_release_set.to_bytes()
        || terms.token_program() != checked.token_program().to_bytes()
        || terms.outcome_count() != product.outcome_count
    {
        return Err(Error::ChainArtifacts);
    }
    let root = FractionalRootV1::decode(&snapshot.root.data).ok_or(Error::ChainArtifacts)?;
    if snapshot.root.owner != checked.trading_program()
        || snapshot.root.executable
        || !rent.is_exempt(snapshot.root.lamports, snapshot.root.data.len())
        || root.input().terms != checked.terms_id()
        || root.input().market != snapshot.market.key.to_bytes()
    {
        return Err(Error::ChainArtifacts);
    }

    let admission = |record: FinalizedArtifactRecordV1<'_>| ArtifactAdmissionV1 {
        finalized_digest: record.digest,
        record_authenticated: true,
    };
    let children = FractionalChildProgramsV1 {
        claims: checked.claims_program().to_bytes(),
        custody: checked.custody_program().to_bytes(),
        token: checked.token_program().to_bytes(),
        physical_profile: checked.physical_profile(),
        release_authenticated: true,
    };
    let selection = FractionalArtifactSelectionV1 {
        descriptor_id: descriptor.digest,
        terms_id: terms_record.digest,
        market: snapshot.market.key.to_bytes(),
        market_realm: market.identity.realm_id.to_bytes(),
        product_record: product.product_record,
        result_domain,
        outcome_count: product.outcome_count,
        release_set: market.identity.selected_release_set.to_bytes(),
        children,
        semantic_selection_authenticated: true,
    };
    Ok(FractionalPreparedChainArtifactsV1 {
        checked,
        observation,
        root,
        context: FractionalRequestContextV1 {
            release_set: selection.release_set,
            market: selection.market,
            product_record: selection.product_record,
            result_domain: selection.result_domain,
            terms: selection.terms_id,
            token_behavior: terms.token_behavior_selection_id(),
        },
        selection,
        admissions: FractionalArtifactAdmissionsV1 {
            descriptor: admission(descriptor),
            terms: admission(terms_record),
            token_behavior: admission(token_behavior),
            account_profile: admission(account_profile),
            lifecycle: admission(lifecycle),
            request: admission(request),
            strategy: admission(strategy),
            transition: admission(transition),
            effect: admission(effect),
        },
        artifacts: FractionalArtifactBytesV1 {
            descriptor: descriptor.bytes,
            terms: terms_record.bytes,
            token_behavior: token_behavior.bytes,
            account_profile: account_profile.bytes,
            lifecycle: lifecycle.bytes,
            request: request.bytes,
            strategy: strategy.bytes,
            transition: transition.bytes,
            effect: effect.bytes,
        },
        terms,
    })
}

/// Bind a canonical request to the already authenticated same-snapshot bundle.
pub fn authenticate_fractional_chain_artifacts_v1<'a>(
    prepared: FractionalPreparedChainArtifactsV1<'a>,
    family_request_bytes: &[u8],
) -> Result<FractionalArtifactBundleV1<'a>> {
    let request = FractionalFamilyRequestV1::decode(family_request_bytes)
        .map_err(|_| Error::ChainArtifacts)?;
    if request.input().expected_revision != prepared.root.input().revision {
        return Err(Error::ChainArtifacts);
    }
    authenticate_fractional_artifact_bundle_v1(
        prepared.selection,
        prepared.admissions,
        prepared.artifacts,
        family_request_bytes,
    )
    .map_err(|_| Error::ChainArtifacts)
}

fn authenticate_market(
    snapshot: FractionalChainArtifactSnapshotV1<'_>,
    checked: CheckedFractionalReleaseV1,
    rent: &Rent,
) -> Result<CoreState> {
    let state = CoreState::decode(&snapshot.market.data).map_err(|_| Error::ChainArtifacts)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        &checked.core_program(),
    )
    .0;
    if snapshot.market.key != expected
        || snapshot.market.key.to_bytes() != state.identity.market_id.to_bytes()
        || snapshot.market.owner != checked.core_program()
        || snapshot.market.executable
        || !rent.is_exempt(snapshot.market.lamports, snapshot.market.data.len())
        || state.identity.registry_program.to_bytes() != checked.registry_program().to_bytes()
    {
        return Err(Error::ChainArtifacts);
    }
    Ok(state)
}

fn finalized_record<'a>(
    snapshot: FractionalArtifactRecordSnapshotV1<'a>,
    registry: Pubkey,
    schema: [u8; 32],
    selected_digest: Option<[u8; 32]>,
    rent: &Rent,
) -> Result<FinalizedArtifactRecordV1<'a>> {
    let digest = digest(&snapshot.raw.data);
    let expected_raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    let expected_staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0;
    if selected_digest.is_some_and(|selected| selected != digest)
        || snapshot.raw.key != expected_raw
        || snapshot.raw.owner != registry
        || snapshot.raw.executable
        || snapshot.raw.data.is_empty()
        || !rent.is_exempt(snapshot.raw.lamports, snapshot.raw.data.len())
        || snapshot.staging.key != expected_staging
        || snapshot.staging.owner != system_program::ID
        || snapshot.staging.executable
        || !snapshot.staging.data.is_empty()
    {
        return Err(Error::ChainArtifacts);
    }
    Ok(FinalizedArtifactRecordV1 {
        schema,
        digest,
        bytes: &snapshot.raw.data,
    })
}

fn same_finalized_observation(
    snapshot: FractionalChainArtifactSnapshotV1<'_>,
) -> Result<Observation> {
    let observation = snapshot.market.observation;
    if observation.finality != Finality::Finalized || observation.slot == 0 {
        return Err(Error::ChainArtifacts);
    }
    let accounts = [
        snapshot.registry_program,
        snapshot.core_program,
        snapshot.trading_program,
        snapshot.claims_program,
        snapshot.custody_program,
        snapshot.token_program,
        snapshot.rent_sysvar,
        snapshot.market,
        snapshot.product_raw,
        snapshot.product_staging,
        snapshot.result_domain_raw,
        snapshot.result_domain_staging,
        snapshot.portfolio_raw,
        snapshot.portfolio_staging,
        snapshot.descriptor.raw,
        snapshot.descriptor.staging,
        snapshot.terms.raw,
        snapshot.terms.staging,
        snapshot.token_behavior.raw,
        snapshot.token_behavior.staging,
        snapshot.account_profile.raw,
        snapshot.account_profile.staging,
        snapshot.lifecycle.raw,
        snapshot.lifecycle.staging,
        snapshot.request.raw,
        snapshot.request.staging,
        snapshot.strategy.raw,
        snapshot.strategy.staging,
        snapshot.transition.raw,
        snapshot.transition.staging,
        snapshot.effect.raw,
        snapshot.effect.staging,
        snapshot.root,
    ];
    if accounts
        .iter()
        .any(|account| account.observation != observation)
    {
        return Err(Error::ChainArtifacts);
    }
    Ok(observation)
}

fn require_program(
    observed: &ObservedAccount,
    expected: Pubkey,
    observation: Observation,
) -> Result<()> {
    if observed.observation != observation
        || observed.key != expected
        || !observed.executable
        || observed.owner == system_program::ID
    {
        return Err(Error::ChainArtifacts);
    }
    Ok(())
}

fn decode_rent(account: &ObservedAccount) -> Result<Rent> {
    if account.key != sysvar::rent::ID
        || account.owner != sysvar::ID
        || account.executable
        || account.data.len() != Rent::size_of()
    {
        return Err(Error::ChainArtifacts);
    }
    let mut lamports = account.lamports;
    let mut data = account.data.clone();
    let info = AccountInfo::new(
        &account.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &account.owner,
        false,
    );
    Rent::from_account_info(&info).map_err(|_| Error::ChainArtifacts)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn has_duplicate_program(programs: &[Pubkey]) -> bool {
    programs.iter().enumerate().any(|(index, program)| {
        programs
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other == program)
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::unwrap_used)]

    use super::*;

    fn observation() -> Observation {
        Observation {
            slot: 77,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn account(key: Pubkey, owner: Pubkey, lamports: u64, data: Vec<u8>) -> ObservedAccount {
        ObservedAccount {
            observation: observation(),
            key,
            owner,
            lamports,
            executable: false,
            data,
        }
    }

    fn record_fixture() -> (ObservedAccount, ObservedAccount, Pubkey, [u8; 32], Rent) {
        let registry = Pubkey::new_from_array([1; 32]);
        let schema = [2; 32];
        let body = vec![3; 48];
        let digest = digest(&body);
        let raw =
            Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
        let staging = Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &registry,
        )
        .0;
        let rent = Rent::default();
        (
            account(raw, registry, rent.minimum_balance(body.len()), body),
            account(staging, system_program::ID, 7, Vec::new()),
            registry,
            schema,
            rent,
        )
    }

    #[test]
    fn finalized_records_accept_staging_dust_but_refuse_substitution_and_live_cursor() {
        let (raw, staging, registry, schema, rent) = record_fixture();
        let authenticated = finalized_record(
            FractionalArtifactRecordSnapshotV1 {
                raw: &raw,
                staging: &staging,
            },
            registry,
            schema,
            Some(digest(&raw.data)),
            &rent,
        )
        .unwrap();
        assert_eq!(authenticated.bytes, raw.data.as_slice());

        let mut wrong_raw = raw.clone();
        wrong_raw.key = Pubkey::new_unique();
        assert_eq!(
            finalized_record(
                FractionalArtifactRecordSnapshotV1 {
                    raw: &wrong_raw,
                    staging: &staging,
                },
                registry,
                schema,
                None,
                &rent,
            ),
            Err(Error::ChainArtifacts)
        );
        let mut live_staging = staging.clone();
        live_staging.data.push(1);
        assert_eq!(
            finalized_record(
                FractionalArtifactRecordSnapshotV1 {
                    raw: &raw,
                    staging: &live_staging,
                },
                registry,
                schema,
                None,
                &rent,
            ),
            Err(Error::ChainArtifacts)
        );
    }

    #[test]
    fn checked_release_refuses_program_aliases_and_unchecked_manifest() {
        let input = CheckedFractionalReleaseInputV1 {
            checked_manifest_digest: [1; 32],
            descriptor_id: [2; 32],
            terms_id: [3; 32],
            physical_profile: [4; 32],
            registry_program: Pubkey::new_from_array([11; 32]),
            core_program: Pubkey::new_from_array([12; 32]),
            trading_program: Pubkey::new_from_array([13; 32]),
            claims_program: Pubkey::new_from_array([14; 32]),
            custody_program: Pubkey::new_from_array([15; 32]),
            token_program: Pubkey::new_from_array([16; 32]),
            release_authenticated: true,
        };
        assert!(CheckedFractionalReleaseV1::new(input).is_ok());
        assert_eq!(
            CheckedFractionalReleaseV1::new(CheckedFractionalReleaseInputV1 {
                claims_program: input.custody_program,
                ..input
            }),
            Err(Error::ChainArtifacts)
        );
        assert_eq!(
            CheckedFractionalReleaseV1::new(CheckedFractionalReleaseInputV1 {
                release_authenticated: false,
                ..input
            }),
            Err(Error::ChainArtifacts)
        );
    }
}
