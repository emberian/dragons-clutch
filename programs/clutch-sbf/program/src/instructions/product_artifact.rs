//! Shared authentication for immutable Product and registry capability artifacts.
//!
//! This module is always compiled. Product/Series and Failure consumers can
//! therefore authenticate the same content-addressed accounts without either
//! feature enabling the other's routes or duplicating hostile-byte logic.

use crate::accounts::{expect_pda, require, Outcome};
use crate::capabilities;
use crate::error::{ClutchError, Refusal};
use crate::loader_state::{decode_loader_pair_v1, LoaderAccountViewV1};
use crate::seeds;
use clutch_product_series::{
    CompiledProductSeriesBundleV1, ContentId, EvidenceOnlyRecoveryPolicyV1, FixedCodec,
    MarketGenesisProfileV2, MarketInstancePreimageV2, NativeClaimBasisV1, PriceMeasurePolicyV1,
    ProductTemplateV4, RegistryCapabilityProfileV2, RegistryCapabilityProjectionV2,
    RegistryProgramReleaseV1, SeriesAttachmentPlanV1, SeriesFundingQuoteV1, SeriesFundingTermsV2,
    SeriesPlanV5, SeriesPlanV5Id,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::product_series::{
    SeriesRegistryAccountV1, SERIES_REGISTRY_ACCOUNT_BYTES_V1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Codec and typed-identity owner for one content-addressed Product artifact.
pub trait ProductArtifactTypeV1: FixedCodec + Sized {
    /// Frozen artifact wire kind.
    const KIND: ArtifactKind;

    /// Recompute the artifact's semantic content identity.
    fn semantic_id(&self) -> clutch_product_series::Result<ContentId>;

    /// Hostile-decode directly into owned storage.
    fn decode_boxed(input: &[u8]) -> Outcome<Box<Self>> {
        Self::decode(input)
            .map(Box::new)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
    }
}

macro_rules! product_artifact_type {
    ($type:ty, $kind:ident) => {
        impl ProductArtifactTypeV1 for $type {
            const KIND: ArtifactKind = ArtifactKind::$kind;

            fn semantic_id(&self) -> clutch_product_series::Result<ContentId> {
                Ok(self.id()?.content_id())
            }
        }
    };
}

product_artifact_type!(EvidenceOnlyRecoveryPolicyV1, EvidenceOnlyRecoveryPolicyV1);
product_artifact_type!(ProductTemplateV4, ProductTemplateV4);
product_artifact_type!(PriceMeasurePolicyV1, PriceMeasurePolicyV1);
product_artifact_type!(MarketGenesisProfileV2, MarketGenesisProfileV2);
product_artifact_type!(SeriesFundingQuoteV1, SeriesFundingQuoteV1);
product_artifact_type!(SeriesAttachmentPlanV1, SeriesAttachmentPlanV1);
product_artifact_type!(SeriesPlanV5, SeriesPlanV5);
product_artifact_type!(SeriesFundingTermsV2, SeriesFundingTermsV2);
product_artifact_type!(CompiledProductSeriesBundleV1, CompiledProductSeriesBundleV1);
product_artifact_type!(MarketInstancePreimageV2, MarketInstancePreimageV2);

impl ProductArtifactTypeV1 for NativeClaimBasisV1 {
    const KIND: ArtifactKind = ArtifactKind::NativeClaimBasisV1;

    fn semantic_id(&self) -> clutch_product_series::Result<ContentId> {
        Ok(self.id()?.content_id())
    }

    fn decode_boxed(input: &[u8]) -> Outcome<Box<Self>> {
        let mut value = Box::new(Self::ZEROED);
        Self::decode_into(input, &mut value)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        Ok(value)
    }
}

impl ProductArtifactTypeV1 for RegistryCapabilityProfileV2 {
    const KIND: ArtifactKind = ArtifactKind::RegistryCapabilityProfileV2;

    fn semantic_id(&self) -> clutch_product_series::Result<ContentId> {
        Ok(self.id()?.content_id())
    }
}

impl ProductArtifactTypeV1 for RegistryProgramReleaseV1 {
    const KIND: ArtifactKind = ArtifactKind::RegistryProgramReleaseV1;

    fn semantic_id(&self) -> clutch_product_series::Result<ContentId> {
        Ok(self.id()?.content_id())
    }
}

/// Authenticated immutable Product artifact with a recomputed typed identity.
#[derive(Debug)]
pub struct AuthenticatedProductArtifactV1<T> {
    account: Pubkey,
    semantic_id: ContentId,
    value: Box<T>,
}

impl<T> AuthenticatedProductArtifactV1<T> {
    /// Exact content-addressed artifact account.
    pub const fn account(&self) -> Pubkey {
        self.account
    }

    /// Recomputed semantic identity of the complete hostile-decoded body.
    pub const fn semantic_id(&self) -> ContentId {
        self.semantic_id
    }

    /// Borrow the authenticated owning value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Consume the receipt and return the authenticated owning value.
    pub fn into_value(self) -> Box<T> {
        self.value
    }
}

/// Authenticate one immutable Product artifact mechanically.
///
/// Success proves exact program owner, read-only/non-executable role, exact
/// codec length, content-addressed PDA, hostile decode, and recomputed typed
/// identity. It does not prove higher-level joins between different artifacts.
pub fn authenticate_product_artifact_v1<T: ProductArtifactTypeV1>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_id: ContentId,
) -> Outcome<AuthenticatedProductArtifactV1<T>> {
    expected_id
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(
        account.data_len() == T::ENCODED_LEN && T::ENCODED_LEN == T::KIND.exact_len(),
        ClutchError::WrongDataLength,
    )?;
    expect_pda(
        account.key,
        seeds::product_artifact_pda(program_id, T::KIND.byte(), &expected_id.bytes()),
        None,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = T::decode_boxed(&data)?;
    let semantic_id = value
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(semantic_id == expected_id, ClutchError::MismatchedState)?;
    Ok(AuthenticatedProductArtifactV1 {
        account: *account.key,
        semantic_id,
        value,
    })
}

/// Private proof of exact capability references read from a SeriesRegistry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedSeriesRegistryCapabilityRefsV1 {
    series_registry_account: Pubkey,
    series_plan_id: SeriesPlanV5Id,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
}

/// Authenticate the persistent SeriesRegistry references without a Rent input.
///
/// Success proves exact program owner, read-only/non-executable metadata, the
/// complete hostile account codec, expected Series identity, canonical PDA and
/// stored bump, and a present balance covering the immutable rent principal
/// established when the account was created. A value-bearing consumer must
/// still pass the returned private receipt through
/// [`authenticate_registry_capability_v2`], which freshly authenticates the
/// current loader state and both content-addressed registry artifacts.
pub fn authenticate_series_registry_capability_refs_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series: SeriesPlanV5Id,
) -> Outcome<AuthenticatedSeriesRegistryCapabilityRefsV1> {
    expected_series
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(
        account.data_len() == SERIES_REGISTRY_ACCOUNT_BYTES_V1,
        ClutchError::WrongDataLength,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesRegistryAccountV1::decode(&data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        value.series_plan_id == expected_series,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::series_registry_pda(program_id, &expected_series.bytes()),
        Some(value.stored_bump),
    )?;
    require(
        account.lamports() >= value.rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedSeriesRegistryCapabilityRefsV1 {
        series_registry_account: *account.key,
        series_plan_id: value.series_plan_id,
        registry_release_id: value.registry_release_id,
        capability_profile_id: value.capability_profile_id,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedRegistryArtifactPairV2 {
    release_artifact_account: Pubkey,
    profile_artifact_account: Pubkey,
    release: RegistryProgramReleaseV1,
    profile: RegistryCapabilityProfileV2,
    projection: RegistryCapabilityProjectionV2,
}

/// Loader-authenticated release/profile authority used before Series registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRegistryCapabilityReleaseV2 {
    program_account: Pubkey,
    programdata_account: Pubkey,
    release_artifact_account: Pubkey,
    profile_artifact_account: Pubkey,
    release: RegistryProgramReleaseV1,
    profile: RegistryCapabilityProfileV2,
    projection: RegistryCapabilityProjectionV2,
}

impl AuthenticatedRegistryCapabilityReleaseV2 {
    /// Current executable Program account.
    pub const fn program_account(self) -> Pubkey {
        self.program_account
    }

    /// Current linked ProgramData account.
    pub const fn programdata_account(self) -> Pubkey {
        self.programdata_account
    }

    /// Exact content-addressed RegistryRelease artifact account.
    pub const fn release_artifact_account(self) -> Pubkey {
        self.release_artifact_account
    }

    /// Exact content-addressed CapabilityProfile artifact account.
    pub const fn profile_artifact_account(self) -> Pubkey {
        self.profile_artifact_account
    }

    /// Complete immutable registry release body.
    pub const fn release(self) -> RegistryProgramReleaseV1 {
        self.release
    }

    /// Complete immutable capability profile body.
    pub const fn profile(self) -> RegistryCapabilityProfileV2 {
        self.profile
    }

    /// Exact compiler/runtime projection derived from both artifacts.
    pub const fn projection(self) -> RegistryCapabilityProjectionV2 {
        self.projection
    }
}

/// Persistent SeriesRegistry-bound capability authority for value-bearing consumers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRegistryCapabilityV2 {
    series_registry_account: Pubkey,
    series_plan_id: SeriesPlanV5Id,
    program_account: Pubkey,
    programdata_account: Pubkey,
    release_artifact_account: Pubkey,
    profile_artifact_account: Pubkey,
    release: RegistryProgramReleaseV1,
    profile: RegistryCapabilityProfileV2,
    projection: RegistryCapabilityProjectionV2,
}

impl AuthenticatedRegistryCapabilityV2 {
    /// Exact authenticated SeriesRegistry account owning both references.
    pub const fn series_registry_account(self) -> Pubkey {
        self.series_registry_account
    }

    /// Exact registered recurring Series.
    pub const fn series_plan_id(self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    /// Current executable Program account authenticated for this receipt.
    pub const fn program_account(self) -> Pubkey {
        self.program_account
    }

    /// Current linked ProgramData account authenticated for this receipt.
    pub const fn programdata_account(self) -> Pubkey {
        self.programdata_account
    }

    /// Exact content-addressed registry-release artifact account.
    pub const fn release_artifact_account(self) -> Pubkey {
        self.release_artifact_account
    }

    /// Exact content-addressed capability-profile artifact account.
    pub const fn profile_artifact_account(self) -> Pubkey {
        self.profile_artifact_account
    }

    /// Complete immutable registry release body.
    pub const fn release(self) -> RegistryProgramReleaseV1 {
        self.release
    }

    /// Complete immutable capability profile body.
    pub const fn profile(self) -> RegistryCapabilityProfileV2 {
        self.profile
    }

    /// Recomputed capability-profile identity.
    pub const fn capability_profile_id(self) -> ContentId {
        self.projection.capability_profile_id
    }

    /// Authenticated registry-release identity.
    pub const fn registry_release_id(self) -> ContentId {
        self.projection.registry_release_id
    }

    /// Exact compiler/runtime projection derived from the profile body.
    pub const fn projection(self) -> RegistryCapabilityProjectionV2 {
        self.projection
    }

    /// Exact registry selector for the resolved statistic kind.
    pub const fn statistic_registry_value(self) -> u16 {
        self.profile.statistic_registry_value
    }

    /// Exact resolved statistic kind.
    pub const fn resolved_statistic(self) -> clutch_source_plane_v3::StatisticKindV3 {
        self.profile.resolved_statistic
    }

    /// Exact registry selector for coverage policy.
    pub const fn coverage_policy_registry_value(self) -> u16 {
        self.profile.coverage_policy_registry_value
    }

    /// Exact registry selector for ambiguity policy.
    pub const fn ambiguity_policy_registry_value(self) -> u8 {
        self.profile.ambiguity_policy_registry_value
    }

    /// Exact registry selector for edge policy.
    pub const fn edge_policy_registry_value(self) -> u8 {
        self.profile.edge_policy_registry_value
    }

    /// Registry-resolved edge behavior.
    pub const fn resolved_edge_policy(self) -> clutch_product_series::QuantizedEdgePolicyV1 {
        self.profile.resolved_edge_policy
    }

    /// Exact semantic-owner identities admitted by the profile.
    pub const fn semantic_owners(self) -> clutch_product_series::CapabilitySemanticOwnersV2 {
        self.profile.semantic_owners
    }

    /// Exact immutable Realm/Profile collateral projection.
    pub const fn realm_collateral(self) -> clutch_product_series::RealmCollateralProjectionV1 {
        self.profile.realm_collateral
    }
}

fn authenticate_registry_artifact_pair_v2(
    program_id: &Pubkey,
    release_artifact: &AccountInfo<'_>,
    profile_artifact: &AccountInfo<'_>,
    expected_registry_release_id: ContentId,
    expected_capability_profile_id: ContentId,
) -> Outcome<AuthenticatedRegistryArtifactPairV2> {
    require(
        release_artifact.key != profile_artifact.key,
        ClutchError::AccountAlias,
    )?;
    let authenticated_release = authenticate_product_artifact_v1::<RegistryProgramReleaseV1>(
        program_id,
        release_artifact,
        expected_registry_release_id,
    )?;
    let authenticated_profile = authenticate_product_artifact_v1::<RegistryCapabilityProfileV2>(
        program_id,
        profile_artifact,
        expected_capability_profile_id,
    )?;
    let release = *authenticated_release.value();
    let profile = *authenticated_profile.value();
    require(
        profile.registry_release_id == expected_registry_release_id
            && release.capability_manifest_id.bytes() == capabilities::PROFILE_ID,
        ClutchError::MismatchedState,
    )?;
    let projection = profile
        .projection()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        projection.registry_release_id == expected_registry_release_id
            && projection.capability_profile_id == expected_capability_profile_id,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedRegistryArtifactPairV2 {
        release_artifact_account: authenticated_release.account(),
        profile_artifact_account: authenticated_profile.account(),
        release,
        profile,
        projection,
    })
}

/// Authenticate current loader state and both immutable artifacts against a SeriesRegistry.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_registry_capability_v2(
    program_id: &Pubkey,
    registry_refs: AuthenticatedSeriesRegistryCapabilityRefsV1,
    program_account: &AccountInfo<'_>,
    programdata_account: &AccountInfo<'_>,
    release_artifact: &AccountInfo<'_>,
    profile_artifact: &AccountInfo<'_>,
) -> Outcome<AuthenticatedRegistryCapabilityV2> {
    let authenticated = authenticate_registry_capability_for_registration_v2(
        program_id,
        release_artifact,
        profile_artifact,
        registry_refs.registry_release_id,
        registry_refs.capability_profile_id,
        program_account,
        programdata_account,
    )?;
    for account in [
        authenticated.program_account,
        authenticated.programdata_account,
        authenticated.release_artifact_account,
        authenticated.profile_artifact_account,
    ] {
        require(
            account != registry_refs.series_registry_account,
            ClutchError::AccountAlias,
        )?;
    }
    Ok(AuthenticatedRegistryCapabilityV2 {
        series_registry_account: registry_refs.series_registry_account,
        series_plan_id: registry_refs.series_plan_id,
        program_account: authenticated.program_account,
        programdata_account: authenticated.programdata_account,
        release_artifact_account: authenticated.release_artifact_account,
        profile_artifact_account: authenticated.profile_artifact_account,
        release: authenticated.release,
        profile: authenticated.profile,
        projection: authenticated.projection,
    })
}

/// Strictly authenticate the release/profile pair before persisting Series references.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_registry_capability_for_registration_v2(
    program_id: &Pubkey,
    release_artifact: &AccountInfo<'_>,
    profile_artifact: &AccountInfo<'_>,
    expected_registry_release_id: ContentId,
    expected_capability_profile_id: ContentId,
    program_account: &AccountInfo<'_>,
    programdata_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedRegistryCapabilityReleaseV2> {
    let authenticated = authenticate_registry_artifact_pair_v2(
        program_id,
        release_artifact,
        profile_artifact,
        expected_registry_release_id,
        expected_capability_profile_id,
    )?;
    require(
        program_account.key != programdata_account.key
            && program_account.key != release_artifact.key
            && program_account.key != profile_artifact.key
            && programdata_account.key != release_artifact.key
            && programdata_account.key != profile_artifact.key,
        ClutchError::AccountAlias,
    )?;
    require(
        program_account.key == program_id
            && !program_account.is_signer
            && !program_account.is_writable
            && program_account.executable
            && !programdata_account.is_signer
            && !programdata_account.is_writable
            && !programdata_account.executable,
        ClutchError::MismatchedState,
    )?;
    let program_data = program_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let programdata_data = programdata_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let loader = decode_loader_pair_v1(
        LoaderAccountViewV1::new(
            program_account.key.to_bytes(),
            program_account.owner.to_bytes(),
            program_account.executable,
            &program_data,
        ),
        LoaderAccountViewV1::new(
            programdata_account.key.to_bytes(),
            programdata_account.owner.to_bytes(),
            programdata_account.executable,
            &programdata_data,
        ),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let release = authenticated.release;
    require(
        release.program.bytes() == program_id.to_bytes()
            && release.programdata.bytes() == programdata_account.key.to_bytes()
            && release.deployment_slot == loader.state.deployment_slot
            && release.programdata_sha256.bytes()
                == solana_sha256_hasher::hashv(&[&programdata_data]).to_bytes()
            && release.capability_manifest_id.bytes() == capabilities::PROFILE_ID
            && release
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == expected_registry_release_id,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedRegistryCapabilityReleaseV2 {
        program_account: *program_account.key,
        programdata_account: *programdata_account.key,
        release_artifact_account: authenticated.release_artifact_account,
        profile_artifact_account: authenticated.profile_artifact_account,
        release: authenticated.release,
        profile: authenticated.profile,
        projection: authenticated.projection,
    })
}
