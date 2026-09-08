//! Canonical Claims V6 branch of the Series Consume frame.
//!
//! The sole Claims ABI is its thirty-three-account founding frame.  This
//! mapper decodes the child request, derives the route caller and the two
//! failure-escrow accounts from their native owners, and checks the resolved
//! generated aliases against the Claims parser's exact order.

use dclutch_claims::{
    founding_v5::ClaimsFoundingRequestV5, protocol_position_v2::failure_escrow_v1,
};
use dclutch_core_contract::ContentId;
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading_sbf::series::{
    account_profile_v4::SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4,
    artifacts_v3::SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3,
    consume_artifacts_v4::SeriesConsumeChildRequestsV4,
};
use sha2::{Digest as _, Sha256};
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::system_program;

use crate::{
    Error, Result,
    series_consume_geometry::{
        SERIES_CONSUME_CLAIMS_START_V1, SeriesConsumeGeometryInputV1, SeriesConsumeRoleSourceV1,
        m0_record_by_schema_v1, put_series_consume_role_v1, vacancy_v1,
    },
};

const CLAIMS_COUNT: usize = SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3 as usize;
const ESCROW_POSITION_LOCAL: usize = 31;
const ESCROW_ADMISSION_LOCAL: usize = 32;

const _: () = assert!(CLAIMS_COUNT == 33);
const _: () = assert!(SERIES_CONSUME_CLAIMS_START_V1 == 92);

/// Populate only Claims-owned unique coordinates. The generated alias table
/// fills locals 1..30 from the preceding canonical route representatives;
/// `validate_series_consume_claims_roles_v1` proves those aliases still spell
/// the exact native Claims parser frame.
pub(crate) fn populate_series_consume_claims_route_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    children: SeriesConsumeChildRequestsV4<'_>,
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
) -> Result<()> {
    let claims = ClaimsFoundingRequestV5::decode(children.claims)
        .map_err(|_| Error::new("Series Consume Claims child refused decode"))?;
    let expected = expected_claims_addresses_v1(input, claims)?;
    let caller = expected[0];
    let escrow_position = expected[ESCROW_POSITION_LOCAL];
    let escrow_admission = expected[ESCROW_ADMISSION_LOCAL];
    put_series_consume_role_v1(
        roles,
        SERIES_CONSUME_CLAIMS_START_V1,
        vacancy_v1("Series Claims caller", caller, 0)?,
    )?;
    put_series_consume_role_v1(
        roles,
        SERIES_CONSUME_CLAIMS_START_V1 + ESCROW_POSITION_LOCAL,
        vacancy_v1("Series failure escrow Position", escrow_position, 0)?,
    )?;
    put_series_consume_role_v1(
        roles,
        SERIES_CONSUME_CLAIMS_START_V1 + ESCROW_ADMISSION_LOCAL,
        vacancy_v1("Series failure escrow admission", escrow_admission, 0)?,
    )
}

/// After the generated alias resolver fills Claims locals 1..30, require the
/// resolved physical addresses to match the live `FoundingAccounts::parse`
/// order exactly.  This keeps parser order in its native owner and catches an
/// alias-table or route-source drift before any RPC observation.
pub(crate) fn validate_series_consume_claims_roles_v1(
    input: &SeriesConsumeGeometryInputV1<'_>,
    roles: &[SeriesConsumeRoleSourceV1<'_>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
) -> Result<()> {
    let children = input.preprofile.prepare_children.consume_requests();
    let claims = ClaimsFoundingRequestV5::decode(children.claims)
        .map_err(|_| Error::new("Series Consume Claims child refused decode"))?;
    let expected = expected_claims_addresses_v1(input, claims)?;
    for (local, expected) in expected.into_iter().enumerate() {
        let coordinate = SERIES_CONSUME_CLAIMS_START_V1 + local;
        let actual = roles
            .get(coordinate)
            .ok_or_else(|| Error::new("Series Consume Claims coordinate escaped fixed frame"))?
            .address();
        if actual != expected {
            return Err(Error::new(format!(
                "Series Consume Claims parser coordinate {local} differed from canonical address"
            )));
        }
    }
    Ok(())
}

fn expected_claims_addresses_v1(
    input: &SeriesConsumeGeometryInputV1<'_>,
    claims: ClaimsFoundingRequestV5,
) -> Result<[Pubkey; CLAIMS_COUNT]> {
    let physical = input.preprofile.physical();
    let product = m0_record_by_schema_v1(
        input,
        dclutch_product::admission::PRODUCT_RECORD_SCHEMA_ID_V2,
        "Product",
    )?;
    let result_domain = m0_record_by_schema_v1(
        input,
        dclutch_product::admission::RESULT_DOMAIN_SCHEMA_ID_V2,
        "Result domain",
    )?;
    let basis = m0_record_by_schema_v1(
        input,
        dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        "linked Basis",
    )?;
    let product_digest: [u8; 32] = Sha256::digest(product.body).into();
    let basis_digest: [u8; 32] = Sha256::digest(basis.body).into();
    let market = input.m0.project_found[1];
    if claims.release_set()
        != input
            .preprofile
            .predicted_core
            .identity
            .selected_release_set
            .to_bytes()
        || claims.market() != market.to_bytes()
        || claims.product_record_digest() != product_digest
        || claims.linked_basis_record_digest() != basis_digest
        || claims.aggregate() != physical.claims.aggregate.to_bytes()
        || claims.position() != physical.claims.position.to_bytes()
        || claims.admission() != physical.claims.admission.to_bytes()
        || claims.funding_source() != physical.projected.escrow_vault
        || claims.hoard() != physical.projected.hoard_vault
        || claims.custody_replay() != physical.realized_hoard_replay.to_bytes()
        || claims.rent_credit() != physical.projected.rent_credit
        || claims.rent_program() != input.rent_program.to_bytes()
        || claims.claims_program() != input.claims.to_bytes()
        || claims.trading_program() != input.trading.to_bytes()
    {
        return Err(Error::new(
            "Series Consume Claims child did not join admitted M0 and projected facts",
        ));
    }
    let transport = dclutch_claims::series_founding_transport_v1::SeriesClaimsFoundingTransportV1::from_canonical_v5(
        physical.permit.to_bytes(),
        claims,
    )
    .map_err(|_| Error::new("Series Consume Claims transport refused permit normalization"))?;
    let caller = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            ContentId::new(claims.release_set())
                .map_err(|_| Error::new("Series Consume Claims release was zero"))?,
            claims.market(),
            ExecutionRoleV1::Trading,
            physical.permit.to_bytes(),
            Sha256::digest(transport.to_bytes()).into(),
        )
        .map_err(|_| Error::new("Series Consume Claims caller seeds refused"))?
        .as_slices(),
        &input.trading,
    )
    .0;
    let cache = Pubkey::find_program_address(
        &[
            dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
            &claims.release_set(),
        ],
        &input.registry,
    )
    .0;
    let escrow = failure_escrow_v1(
        input.claims,
        claims.market(),
        physical.claims.aggregate,
        claims.claim_count(),
    )
    .map_err(|error| {
        Error::new(format!(
            "Series Consume failure escrow was not derivable for Claims width {}: {error}",
            claims.claim_count()
        ))
    })?;
    Ok([
        caller,
        physical.permit,
        physical.claims.aggregate,
        physical.claims.position,
        physical.claims.admission,
        Pubkey::new_from_array(physical.projected.escrow_vault),
        Pubkey::new_from_array(physical.projected.hoard_vault),
        physical.realized_hoard_replay,
        basis.raw,
        basis.staging,
        product.raw,
        product.staging,
        result_domain.raw,
        result_domain.staging,
        input.records.portfolio.raw,
        input.records.portfolio.staging,
        system_program::ID,
        market,
        cache,
        input.registry,
        input.claims,
        crate::upgrade::target_programdata(input.claims),
        input.core,
        crate::upgrade::target_programdata(input.core),
        input.trading,
        crate::upgrade::target_programdata(input.trading),
        input.custody,
        crate::upgrade::target_programdata(input.custody),
        Pubkey::new_from_array(claims.founder()),
        Pubkey::new_from_array(physical.projected.rent_credit),
        input.rent_program,
        escrow.position,
        escrow.admission,
    ])
}
