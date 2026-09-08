//! Authenticated Source projection for one Series Prepare request bank.

use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_source::{
    ContentId as SourceContentId, MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
    MANIPULATION_FLOOR_V1_BYTES, ManipulationFloorV1, SOURCE_CAPACITY_PROFILE_BYTES,
    SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_MATERIAL_V3_BYTES, SOURCE_SPEC_BYTES, SOURCE_SPEC_SCHEMA_ID_V1, SourceCapacityProfileV1,
    SourceMaterialV3, SourcePrincipalPolicyV1, SourceSpecV1,
};
use dclutch_vm::account_profile::AccountObservationV1;
use solana_program::{hash::hash, program_error::ProgramError, pubkey::Pubkey};
use solana_sdk_ids::system_program;

use crate::TradingSbfError;

// Prepare's first child route starts at logical coordinate 6. Its native
// Initialize prefix occupies 11 accounts, so the embedded ProjectFound frame
// starts at 17. These four pairs are ProjectFound's canonical Source roles
// 14..=21, shared with the ordinary Found frame in `found_frame_v3`.
const SOURCE_MATERIAL_RAW: usize = 31;
const SOURCE_MATERIAL_STAGING: usize = 32;
const SOURCE_SPEC_RAW: usize = 33;
const SOURCE_SPEC_STAGING: usize = 34;
const SOURCE_CAPACITY_RAW: usize = 35;
const SOURCE_CAPACITY_STAGING: usize = 36;
const MANIPULATION_FLOOR_RAW: usize = 37;
const MANIPULATION_FLOOR_STAGING: usize = 38;

/// Authenticate Prepare's Source record graph and derive its sole principal cap.
///
/// `expected_source`, `product_record`, `collateral_mint`, and `payout_scale`
/// come from the already-admitted Occurrence and Product Runtime graph. The
/// selected AccountProfile has already enforced these coordinates' readonly
/// privileges and exact data widths; this function independently authenticates
/// Registry ownership, content, PDA derivation, finality, and every semantic
/// edge before the Source kernel performs the cap projection.
#[allow(clippy::too_many_arguments)]
pub(super) fn derive_series_prepare_principal_cap_v1(
    registry: &Pubkey,
    expected_source: [u8; 32],
    product_record: [u8; 32],
    collateral_mint: [u8; 32],
    payout_scale: u64,
    observations: &[AccountObservationV1<'_>],
) -> Result<u64, ProgramError> {
    let material_bytes = authenticate_record(
        registry,
        observations,
        SOURCE_MATERIAL_RAW,
        SOURCE_MATERIAL_STAGING,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        expected_source,
        SOURCE_MATERIAL_V3_BYTES,
    )?;
    let material = SourceMaterialV3::decode(material_bytes).map_err(content_error)?;
    material
        .authenticate_product_record(source_id(product_record)?)
        .map_err(content_error)?;

    let source_spec_id = material.primary_source_spec();
    let source_spec_bytes = authenticate_record(
        registry,
        observations,
        SOURCE_SPEC_RAW,
        SOURCE_SPEC_STAGING,
        SOURCE_SPEC_SCHEMA_ID_V1,
        source_spec_id.to_bytes(),
        SOURCE_SPEC_BYTES,
    )?;
    let source_spec = SourceSpecV1::decode(source_spec_bytes).map_err(content_error)?;

    let capacity_id = source_spec.capacity_profile_id();
    let capacity_bytes = authenticate_record(
        registry,
        observations,
        SOURCE_CAPACITY_RAW,
        SOURCE_CAPACITY_STAGING,
        SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
        capacity_id.to_bytes(),
        SOURCE_CAPACITY_PROFILE_BYTES,
    )?;
    let capacity = SourceCapacityProfileV1::decode(capacity_bytes).map_err(content_error)?;

    let floor = match material.principal_policy() {
        SourcePrincipalPolicyV1::ExplicitlyUnbounded => {
            authenticate_absent_floor(registry, observations)?;
            None
        }
        SourcePrincipalPolicyV1::BoundedByFloor(floor_id) => {
            let floor_bytes = authenticate_record(
                registry,
                observations,
                MANIPULATION_FLOOR_RAW,
                MANIPULATION_FLOOR_STAGING,
                MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
                floor_id.to_bytes(),
                MANIPULATION_FLOOR_V1_BYTES,
            )?;
            let floor = ManipulationFloorV1::decode(floor_bytes).map_err(content_error)?;
            Some((floor_id, floor))
        }
    };
    let principal_cap_sets = material
        .derive_principal_cap_sets(
            source_spec_id,
            source_spec,
            capacity_id,
            capacity,
            floor,
            source_id(collateral_mint)?,
            payout_scale,
        )
        .map_err(content_error)?
        .to_sets();
    if principal_cap_sets == 0 {
        return Err(TradingSbfError::Content.into());
    }
    Ok(principal_cap_sets)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_record<'a>(
    registry: &Pubkey,
    observations: &'a [AccountObservationV1<'a>],
    raw_coordinate: usize,
    staging_coordinate: usize,
    schema: [u8; 32],
    expected_digest: [u8; 32],
    expected_width: usize,
) -> Result<&'a [u8], ProgramError> {
    let raw = observation(observations, raw_coordinate)?;
    let staging = observation(observations, staging_coordinate)?;
    let (digest, data) =
        authenticate_series_record_observations_v1(registry, raw, staging, schema)?;
    if data.len() != expected_width || digest != expected_digest {
        return Err(TradingSbfError::Content.into());
    }
    Ok(data)
}

/// Authenticate one finalized Registry raw/staging observation pair.
///
/// The selected AccountProfile owns privilege and fixed-width admission. This
/// shared runtime check owns the content digest, Registry ownership, canonical
/// PDA derivation, persistent funding, and vacant staging-cursor witness.
pub(super) fn authenticate_series_record_observations_v1<'a>(
    registry: &Pubkey,
    raw: AccountObservationV1<'a>,
    staging: AccountObservationV1<'a>,
    schema: [u8; 32],
) -> Result<([u8; 32], &'a [u8]), ProgramError> {
    let data = raw.data();
    let digest = hash(data).to_bytes();
    let (expected_raw, _) =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], registry);
    let (expected_staging, _) =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], registry);
    if raw.key() != expected_raw.to_bytes()
        || raw.owner() != registry.to_bytes()
        || !funded_rent_persists_v1(raw.lamports())
        || staging.key() != expected_staging.to_bytes()
        || staging.owner() != system_program::ID.to_bytes()
        || !staging.data().is_empty()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok((digest, data))
}

fn authenticate_absent_floor(
    registry: &Pubkey,
    observations: &[AccountObservationV1<'_>],
) -> Result<(), ProgramError> {
    let absent = [0_u8; 32];
    let expected_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
            &absent,
        ],
        registry,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
            &absent,
        ],
        registry,
    )
    .0;
    for (coordinate, expected) in [
        (MANIPULATION_FLOOR_RAW, expected_raw),
        (MANIPULATION_FLOOR_STAGING, expected_staging),
    ] {
        let observed = observation(observations, coordinate)?;
        if observed.key() != expected.to_bytes()
            || observed.owner() != system_program::ID.to_bytes()
            || !observed.data().is_empty()
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    Ok(())
}

fn observation<'a>(
    observations: &'a [AccountObservationV1<'a>],
    coordinate: usize,
) -> Result<AccountObservationV1<'a>, ProgramError> {
    observations
        .get(coordinate)
        .copied()
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn source_id(bytes: [u8; 32]) -> Result<SourceContentId, ProgramError> {
    SourceContentId::new(bytes).map_err(content_error)
}

fn content_error<E>(_: E) -> ProgramError {
    TradingSbfError::Content.into()
}

#[cfg(test)]
mod tests {
    use std::vec;

    use dclutch_source::{
        BONDING_CURVE_FLOOR_DERIVATION_ID_V1, CapacityEnvelope, ManipulationFloorBasis,
        SourceAccessProfile,
    };

    use super::*;

    const BLANK_KEY: [u8; 32] = [200; 32];
    const BLANK_OWNER: [u8; 32] = [201; 32];

    fn id(byte: u8) -> SourceContentId {
        SourceContentId::new([byte; 32]).expect("nonzero fixture identity")
    }

    fn record_keys(registry: &Pubkey, schema: [u8; 32], digest: [u8; 32]) -> ([u8; 32], [u8; 32]) {
        (
            Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], registry)
                .0
                .to_bytes(),
            Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], registry)
                .0
                .to_bytes(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn install_record<'a>(
        observations: &mut [AccountObservationV1<'a>],
        registry: &'a [u8; 32],
        raw_key: &'a [u8; 32],
        staging_key: &'a [u8; 32],
        raw_coordinate: usize,
        staging_coordinate: usize,
        data: &'a [u8],
    ) {
        *observations
            .get_mut(raw_coordinate)
            .expect("raw record coordinate") =
            AccountObservationV1::new(raw_key, registry, 1, data, false, false, false);
        *observations
            .get_mut(staging_coordinate)
            .expect("staging record coordinate") = AccountObservationV1::new(
            staging_key,
            system_program::ID.as_array(),
            0,
            &[],
            false,
            false,
            false,
        );
    }

    #[test]
    fn bounded_source_graph_derives_cap_and_named_substitutions_refuse() {
        let registry = Pubkey::new_from_array([41; 32]);
        let registry_bytes = registry.to_bytes();
        let product = id(42);
        let collateral = id(43);
        let capacity = SourceCapacityProfileV1::new(
            CapacityEnvelope::Provisional,
            1,
            0,
            id(44),
            id(45),
            208,
            0,
        )
        .expect("capacity")
        .bounding_principal(1, 4)
        .expect("principal ratio");
        let capacity_bytes = capacity.to_bytes();
        let capacity_digest = hash(&capacity_bytes).to_bytes();
        let source_spec = SourceSpecV1::new(
            id(46),
            id(47),
            id(48),
            SourceAccessProfile::RelayedObservationRecord,
            id(49),
            id_from_digest(capacity_digest),
        );
        let source_spec_bytes = source_spec.to_bytes();
        let source_spec_digest = hash(&source_spec_bytes).to_bytes();
        let floor = ManipulationFloorV1::new(
            ManipulationFloorBasis::CurveDerived,
            id_from_digest(source_spec_digest),
            id(49),
            collateral,
            id_from_digest(BONDING_CURVE_FLOOR_DERIVATION_ID_V1),
            400,
        );
        let floor_bytes = floor.to_bytes();
        let floor_digest = hash(&floor_bytes).to_bytes();
        let material = SourceMaterialV3::bounded_by_floor(
            product,
            id_from_digest(source_spec_digest),
            id(50),
            id(51),
            None,
            id(52),
            id_from_digest(floor_digest),
        );
        let material_bytes = material.to_bytes();
        let material_digest = hash(&material_bytes).to_bytes();
        let (material_raw, material_staging) = record_keys(
            &registry,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
            material_digest,
        );
        let (spec_raw, spec_staging) =
            record_keys(&registry, SOURCE_SPEC_SCHEMA_ID_V1, source_spec_digest);
        let (capacity_raw, capacity_staging) = record_keys(
            &registry,
            SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
            capacity_digest,
        );
        let (floor_raw, floor_staging) = record_keys(
            &registry,
            MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
            floor_digest,
        );
        let mut observations =
            vec![
                AccountObservationV1::new(&BLANK_KEY, &BLANK_OWNER, 0, &[], false, false, false,);
                MANIPULATION_FLOOR_STAGING + 1
            ];
        install_record(
            &mut observations,
            &registry_bytes,
            &material_raw,
            &material_staging,
            SOURCE_MATERIAL_RAW,
            SOURCE_MATERIAL_STAGING,
            &material_bytes,
        );
        install_record(
            &mut observations,
            &registry_bytes,
            &spec_raw,
            &spec_staging,
            SOURCE_SPEC_RAW,
            SOURCE_SPEC_STAGING,
            &source_spec_bytes,
        );
        install_record(
            &mut observations,
            &registry_bytes,
            &capacity_raw,
            &capacity_staging,
            SOURCE_CAPACITY_RAW,
            SOURCE_CAPACITY_STAGING,
            &capacity_bytes,
        );
        install_record(
            &mut observations,
            &registry_bytes,
            &floor_raw,
            &floor_staging,
            MANIPULATION_FLOOR_RAW,
            MANIPULATION_FLOOR_STAGING,
            &floor_bytes,
        );

        assert_eq!(
            derive_series_prepare_principal_cap_v1(
                &registry,
                material_digest,
                product.to_bytes(),
                collateral.to_bytes(),
                10,
                &observations,
            ),
            Ok(10)
        );
        for (expected_source, product_record, collateral_mint, payout_scale) in [
            ([99; 32], product.to_bytes(), collateral.to_bytes(), 10),
            (
                material_digest,
                id(98).to_bytes(),
                collateral.to_bytes(),
                10,
            ),
            (material_digest, product.to_bytes(), id(97).to_bytes(), 10),
            (
                material_digest,
                product.to_bytes(),
                collateral.to_bytes(),
                0,
            ),
        ] {
            assert_eq!(
                derive_series_prepare_principal_cap_v1(
                    &registry,
                    expected_source,
                    product_record,
                    collateral_mint,
                    payout_scale,
                    &observations,
                ),
                Err(TradingSbfError::Content.into())
            );
        }
    }

    #[test]
    fn explicitly_unbounded_floor_witness_is_the_two_zero_identity_vacancies() {
        let registry = Pubkey::new_from_array([61; 32]);
        let registry_bytes = registry.to_bytes();
        let (absent_raw, absent_staging) =
            record_keys(&registry, MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1, [0; 32]);
        let mut observations =
            vec![
                AccountObservationV1::new(&BLANK_KEY, &BLANK_OWNER, 0, &[], false, false, false,);
                MANIPULATION_FLOOR_STAGING + 1
            ];
        *observations
            .get_mut(MANIPULATION_FLOOR_RAW)
            .expect("floor raw coordinate") = AccountObservationV1::new(
            &absent_raw,
            system_program::ID.as_array(),
            0,
            &[],
            false,
            false,
            false,
        );
        *observations
            .get_mut(MANIPULATION_FLOOR_STAGING)
            .expect("floor staging coordinate") = AccountObservationV1::new(
            &absent_staging,
            system_program::ID.as_array(),
            0,
            &[],
            false,
            false,
            false,
        );
        assert_eq!(authenticate_absent_floor(&registry, &observations), Ok(()));

        *observations
            .get_mut(MANIPULATION_FLOOR_RAW)
            .expect("floor raw coordinate") =
            AccountObservationV1::new(&BLANK_KEY, &registry_bytes, 1, &[1], false, false, false);
        assert_eq!(
            authenticate_absent_floor(&registry, &observations),
            Err(TradingSbfError::Content.into())
        );
    }

    fn id_from_digest(bytes: [u8; 32]) -> SourceContentId {
        SourceContentId::new(bytes).expect("nonzero content digest")
    }
}
