//! Shared canonical role vocabulary for the 161-coordinate Series Consume frame.
//!
//! Route owners populate only their fixed, disjoint interval from decoded child
//! requests. This module retains the single full-frame assembler and the
//! generated alias table is the only authority for physical reuse.

use std::collections::BTreeMap;

use dclutch_registry::record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_trading_sbf::series::{
    account_profile_v4::{SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4, SERIES_CONSUME_ROUTE_ALIASES_V4},
    artifacts_v3::{
        SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3, SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3,
        SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3, SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3,
    },
    effect_v4::{
        SERIES_CONSUME_CORE_FOUND_PREFIX_ACCOUNT_COUNT_V4,
        SERIES_CONSUME_CORE_FOUND_SUFFIX_ACCOUNT_COUNT_V4,
        SERIES_CONSUME_INJECTED_ACCOUNT_COUNT_V4,
    },
};
use sha2::{Digest as _, Sha256};
use solana_sdk::pubkey::Pubkey;

use crate::{
    Error, Result,
    rpc::Rpc,
    series_found_prepare_campaign::SeriesFoundPreparePreprofileV1,
    series_found_prepare_driver::{
        SeriesPrepareFinalizedAccountV1, SeriesPrepareHydrationRecordsV1, SeriesPrepareM0FrameV1,
    },
    series_found_prepare_input::SeriesParentRootFactV1,
};

pub(crate) const SERIES_CONSUME_LOCK_START_V1: usize =
    SERIES_CONSUME_INJECTED_ACCOUNT_COUNT_V4 as usize;
pub(crate) const SERIES_CONSUME_CORE_FOUND_START_V1: usize =
    SERIES_CONSUME_LOCK_START_V1 + SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3 as usize;
pub(crate) const SERIES_CONSUME_REALIZE_START_V1: usize = SERIES_CONSUME_CORE_FOUND_START_V1
    + (SERIES_CONSUME_CORE_FOUND_PREFIX_ACCOUNT_COUNT_V4
        + SERIES_CONSUME_CORE_FOUND_SUFFIX_ACCOUNT_COUNT_V4) as usize;
pub(crate) const SERIES_CONSUME_CLAIMS_START_V1: usize =
    SERIES_CONSUME_REALIZE_START_V1 + SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3 as usize;
pub(crate) const SERIES_CONSUME_CORE_OPEN_START_V1: usize =
    SERIES_CONSUME_CLAIMS_START_V1 + SERIES_CONSUME_CLAIMS_ACCOUNT_COUNT_V3 as usize;

const _: () = assert!(SERIES_CONSUME_CORE_OPEN_START_V1 == 125);
const _: () = assert!(
    SERIES_CONSUME_CORE_OPEN_START_V1 + SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3 as usize
        == SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4
);

/// Whether the frame feeds the pre-Prepare selected compiler or validates the
/// actual Consume prestate after Prepare. This is deliberately independent of
/// the parent-root fact: a finalized parent can still need canonical future
/// Custody predictions before the first Prepare transaction has created them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SeriesConsumePrestateV1 {
    /// The source compiler runs before Prepare. It may predict only the
    /// Prepare-created resources' canonical address and layout.
    PreparedPrediction,
    /// Prepare has finalized; every resource it created must be observed.
    ObservedPrepared,
}

/// Finalized identities shared by each Consume route. The preprofile is the
/// only child-bank producer; M0 bodies and non-record accounts remain owned
/// by the M0 immutable publisher.
pub(crate) struct SeriesConsumeGeometryInputV1<'a> {
    pub(crate) preprofile: &'a SeriesFoundPreparePreprofileV1,
    pub(crate) m0: SeriesPrepareM0FrameV1<'a>,
    pub(crate) records: SeriesPrepareHydrationRecordsV1<'a>,
    pub(crate) parent_root: SeriesParentRootFactV1,
    pub(crate) registry: Pubkey,
    pub(crate) core: Pubkey,
    pub(crate) trading: Pubkey,
    pub(crate) custody: Pubkey,
    pub(crate) claims: Pubkey,
    pub(crate) rent_program: Pubkey,
    pub(crate) prestate: SeriesConsumePrestateV1,
    pub(crate) minimum_slot: u64,
}

/// One named physical source for one fixed Consume coordinate.  The two
/// prediction forms are intentionally distinct: parent activation and Prepare
/// accounts have a canonical eventual layout before materialization, whereas
/// Consume-owned accounts must begin as zero-width vacancies.
#[derive(Clone, Debug)]
pub(crate) enum SeriesConsumeRoleSourceV1<'a> {
    Finalized {
        role: &'static str,
        address: Pubkey,
        expected_owner: Pubkey,
        canonical_body: Option<&'a [u8]>,
    },
    PreparedPrediction {
        role: &'static str,
        address: Pubkey,
        fixed_data_len: u32,
    },
    ConsumeVacancy {
        role: &'static str,
        address: Pubkey,
        fixed_data_len: u32,
    },
}

impl SeriesConsumeRoleSourceV1<'_> {
    pub(crate) fn address(&self) -> Pubkey {
        match self {
            Self::Finalized { address, .. }
            | Self::PreparedPrediction { address, .. }
            | Self::ConsumeVacancy { address, .. } => *address,
        }
    }
}

pub(crate) fn final_source_v1<'a>(
    role: &'static str,
    address: Pubkey,
    expected_owner: Pubkey,
    canonical_body: Option<&'a [u8]>,
) -> SeriesConsumeRoleSourceV1<'a> {
    SeriesConsumeRoleSourceV1::Finalized {
        role,
        address,
        expected_owner,
        canonical_body,
    }
}

pub(crate) fn vacancy_v1(
    role: &'static str,
    address: Pubkey,
    fixed_data_len: usize,
) -> Result<SeriesConsumeRoleSourceV1<'static>> {
    if fixed_data_len != 0 {
        return Err(Error::new(
            "Series Consume-created vacancy carried a nonzero initial width",
        ));
    }
    Ok(SeriesConsumeRoleSourceV1::ConsumeVacancy {
        role,
        address,
        fixed_data_len: u32::try_from(fixed_data_len)
            .map_err(|_| Error::new("Series Consume fixed role width escaped u32"))?,
    })
}

/// Name an account parent activation or Prepare creates before Consume. This is only
/// legal during the compiler's pre-Prepare normalization pass; the observed
/// post-Prepare pass must replace it with a finalized source.
pub(crate) fn prepared_prediction_v1(
    role: &'static str,
    address: Pubkey,
    fixed_data_len: usize,
) -> Result<SeriesConsumeRoleSourceV1<'static>> {
    if fixed_data_len == 0 {
        return Err(Error::new(
            "Series Prepare prediction omitted its canonical account layout",
        ));
    }
    Ok(SeriesConsumeRoleSourceV1::PreparedPrediction {
        role,
        address,
        fixed_data_len: u32::try_from(fixed_data_len)
            .map_err(|_| Error::new("Series Prepare predicted width escaped u32"))?,
    })
}

pub(crate) fn put_series_consume_role_v1<'a>(
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
    coordinate: usize,
    source: SeriesConsumeRoleSourceV1<'a>,
) -> Result<()> {
    let slot = roles
        .get_mut(coordinate)
        .ok_or_else(|| Error::new("Series Consume route escaped the fixed frame"))?;
    if slot.replace(source).is_some() {
        return Err(Error::new(
            "Series Consume route assigned one fixed coordinate twice",
        ));
    }
    Ok(())
}

/// Borrow one verified publisher-owned M0 role. The full mapper will add the
/// raw-record PDA check before it observes the resulting source; route owners
/// may never invent a body or a non-record owner.
pub(crate) fn m0_nonrecord_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    address: Pubkey,
) -> Result<SeriesConsumeRoleSourceV1<'a>> {
    let account: &SeriesPrepareFinalizedAccountV1 = input
        .m0
        .finalized_accounts
        .iter()
        .find(|account| account.address == address)
        .ok_or_else(|| {
            Error::new(format!(
                "Series Consume M0 omitted finalized non-record account {address}"
            ))
        })?;
    Ok(final_source_v1(
        "finalized M0 account",
        address,
        account.expected_owner,
        None,
    ))
}

/// Route-owner bridge for a canonical M0 ProjectFound coordinate. The final
/// mapper authenticates record PDA/body details in its one snapshot pass.
pub(crate) fn m0_source_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    address: Pubkey,
) -> Result<SeriesConsumeRoleSourceV1<'a>> {
    if address == input.m0.project_found[1] {
        return vacancy_v1("future M0 Core Market", address, 0);
    }
    if input.m0.vacancies.contains(&address) {
        return vacancy_v1("vacant M0 ProjectFound record", address, 0);
    }
    for record in input.m0.records {
        if address == record.raw {
            return Ok(final_source_v1(
                "finalized M0 Registry record",
                address,
                input.registry,
                Some(record.body),
            ));
        }
        if address == record.staging {
            return vacancy_v1("vacant M0 Registry staging", address, 0);
        }
    }
    m0_nonrecord_v1(input, address)
}

pub(crate) fn record_source_v1<'a>(
    role: &'static str,
    registry: Pubkey,
    record: crate::series_found_prepare_driver::SeriesPrepareFinalizedRecordV1<'a>,
) -> Result<SeriesConsumeRoleSourceV1<'a>> {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(record.schema)
            .map_err(|_| Error::new("Series Consume record schema was zero"))?,
        ContentDigest::new(Sha256::digest(record.body).into())
            .map_err(|_| Error::new("Series Consume record digest was zero"))?,
    );
    let derive = |seeds: RecordPdaSeedsV1| {
        Pubkey::find_program_address(
            &[
                seeds.domain(),
                seeds.schema_release_id().as_bytes(),
                seeds.expected_digest().as_bytes(),
            ],
            &registry,
        )
        .0
    };
    if record.raw != derive(key.raw_record_pda_seeds())
        || record.staging != derive(key.staging_cursor_pda_seeds())
    {
        return Err(Error::new(
            "Series Consume Registry record pair was noncanonical",
        ));
    }
    Ok(final_source_v1(
        role,
        record.raw,
        registry,
        Some(record.body),
    ))
}

pub(crate) fn m0_record_by_schema_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    schema: [u8; 32],
    label: &str,
) -> Result<crate::series_found_prepare_driver::SeriesPrepareFinalizedRecordV1<'a>> {
    let rows = input
        .m0
        .records
        .iter()
        .copied()
        .filter(|record| record.schema == schema)
        .collect::<Vec<_>>();
    match rows.as_slice() {
        [record] => Ok(*record),
        [] => Err(Error::new(format!(
            "Series Consume M0 frame omitted {label} record"
        ))),
        _ => Err(Error::new(format!(
            "Series Consume M0 frame duplicated {label} record"
        ))),
    }
}

/// Insert the universal Hot outer exactly once. The subsequent route mappers
/// start at coordinate five and cannot overwrite it.
pub(crate) fn populate_series_consume_prefix_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
) -> Result<()> {
    let root = crate::series_consume_core_geometry::parent_root_source_v1(input)?;
    put_series_consume_role_v1(roles, 0, root)?;
    put_series_consume_role_v1(
        roles,
        1,
        record_source_v1("Series Template", input.registry, input.records.template)?,
    )?;
    put_series_consume_role_v1(
        roles,
        2,
        record_source_v1(
            "M0 Product",
            input.registry,
            m0_record_by_schema_v1(
                input,
                dclutch_product::admission::PRODUCT_RECORD_SCHEMA_ID_V2,
                "Product",
            )?,
        )?,
    )?;
    put_series_consume_role_v1(
        roles,
        3,
        record_source_v1("M0 Portfolio", input.registry, input.records.portfolio)?,
    )?;
    put_series_consume_role_v1(
        roles,
        4,
        record_source_v1(
            "M0 linked Basis",
            input.registry,
            m0_record_by_schema_v1(
                input,
                dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
                "linked Basis",
            )?,
        )?,
    )?;
    Ok(())
}

/// Populate the source-owned pre-Core portion of Consume. This is deliberately
/// a shared assembly seam, so the projected route test exercises the same
/// prefix/route array the full mapper later extends with Core, Claims, and
/// final Open coordinates.
pub(crate) fn populate_series_consume_pre_core_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
) -> Result<()> {
    populate_series_consume_prefix_v1(input, roles)?;
    crate::series_consume_projected_geometry::populate_series_consume_projected_routes_v1(
        input,
        input.preprofile.prepare_children.consume_requests(),
        roles,
    )
}

/// Derive every fixed Consume coordinate from the admitted child bank before
/// any RPC observation.  This is the single semantic constructor used for
/// both the pre-Prepare prediction and the post-Prepare finalized pass.
pub(crate) fn derive_series_consume_role_sources_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
) -> Result<[SeriesConsumeRoleSourceV1<'a>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4]> {
    let mut pending = std::array::from_fn(|_| None);
    populate_series_consume_pre_core_v1(input, &mut pending)?;
    let children = input.preprofile.prepare_children.consume_requests();
    crate::series_consume_claims_geometry::populate_series_consume_claims_route_v1(
        input,
        children,
        &mut pending,
    )?;
    crate::series_consume_core_geometry::populate_series_consume_core_routes_v1(
        input,
        children,
        &mut pending,
    )?;
    resolve_series_consume_aliases_v1(&mut pending)?;
    let roles: [SeriesConsumeRoleSourceV1<'_>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4] = pending
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| Error::new("Series Consume semantic mapper left a coordinate unowned"))?
        .try_into()
        .map_err(|_| Error::new("Series Consume fixed frame cardinality drifted"))?;
    crate::series_consume_claims_geometry::validate_series_consume_claims_roles_v1(input, &roles)?;
    Ok(roles)
}

/// Derive every fixed Consume coordinate from the admitted child bank, then
/// observe the complete post-Prepare prestate at one finalized slot.  The
/// dynamic FundingState span is owned by the release AccountProfile and is
/// intentionally absent from this fixed frame.
pub(crate) fn derive_series_consume_fixed_data_lengths_v1(
    rpc: &mut Rpc,
    input: SeriesConsumeGeometryInputV1<'_>,
) -> Result<[u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4]> {
    let roles = derive_series_consume_role_sources_v1(&input)?;
    observe_series_consume_roles_v1(rpc, &roles, input.minimum_slot)
}

/// The selected pre-Prepare profile may predict only Prepare-created layouts.
/// Once Prepare is finalized, re-observation must produce precisely the same
/// fixed profile widths before its Certificate/include can be retained.
pub(crate) fn require_series_consume_prestate_invariance_v1(
    predicted: &[u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
    observed: &[u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
) -> Result<()> {
    if predicted != observed {
        return Err(Error::new(
            "Series Consume observed prestate changed the selected fixed profile",
        ));
    }
    Ok(())
}

/// Close the generated physical-alias relation after every semantic route has
/// populated its own coordinates.  A route may name an alias itself only when
/// it names the representative's exact address; the generated table remains
/// the sole authority for filling an omitted alias.
pub(crate) fn resolve_series_consume_aliases_v1<'a>(
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
) -> Result<()> {
    for &(alias, representative_coordinate) in SERIES_CONSUME_ROUTE_ALIASES_V4 {
        let representative = roles
            .get(representative_coordinate)
            .and_then(Clone::clone)
            .ok_or_else(|| Error::new("Series Consume generated alias lacked representative"))?;
        let alias_slot = roles
            .get_mut(alias)
            .ok_or_else(|| Error::new("Series Consume generated alias escaped fixed frame"))?;
        if let Some(existing) = alias_slot {
            if existing.address() != representative.address() {
                return Err(Error::new(format!(
                    "Series Consume generated alias {alias}->{representative_coordinate} named different accounts {} and {}",
                    existing.address(),
                    representative.address(),
                )));
            }
            continue;
        }
        *alias_slot = Some(representative);
    }
    Ok(())
}

/// System's all-zero ID is valid only for its finalized NativeLoader-owned
/// source. Future PDA predictions cannot use that reserved physical identity.
pub(crate) fn require_series_geometry_address_v1(
    address: Pubkey,
    expected_owner: Option<Pubkey>,
) -> Result<()> {
    if !dclutch_operator::series_operation_corpus_v1::is_series_source_address_v1(
        address,
        expected_owner,
    ) {
        return Err(Error::new("Series geometry role named default Pubkey"));
    }
    Ok(())
}

fn series_consume_snapshot_addresses_v1(
    roles: &[SeriesConsumeRoleSourceV1<'_>],
) -> Result<Vec<Pubkey>> {
    let mut addresses = Vec::new();
    for role in roles {
        let address = role.address();
        let expected_owner = match role {
            SeriesConsumeRoleSourceV1::Finalized { expected_owner, .. } => Some(*expected_owner),
            _ => None,
        };
        require_series_geometry_address_v1(address, expected_owner)?;
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    Ok(addresses)
}

/// Observe one complete canonical Consume frame at a single finalized slot.
/// Semantic route ownership ends before this function: it only validates the
/// finalized/predicted distinction and turns it into the frozen width vector.
pub(crate) fn observe_series_consume_roles_v1(
    rpc: &mut Rpc,
    roles: &[SeriesConsumeRoleSourceV1<'_>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
    minimum_slot: u64,
) -> Result<[u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4]> {
    if minimum_slot == 0 {
        return Err(Error::new(
            "Series Consume geometry requires a finalized observation slot",
        ));
    }
    require_series_consume_alias_addresses_v1(roles)?;
    let addresses = series_consume_snapshot_addresses_v1(roles)?;
    let (_, snapshot) = rpc.finalized_accounts(&addresses, minimum_slot)?;
    let accounts = addresses
        .into_iter()
        .zip(snapshot)
        .collect::<BTreeMap<_, _>>();
    let mut widths = [0_u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4];
    for (coordinate, role) in roles.iter().enumerate() {
        let account = accounts
            .get(&role.address())
            .ok_or_else(|| Error::new("Series Consume snapshot omitted fixed role"))?;
        widths[coordinate] = match role {
            SeriesConsumeRoleSourceV1::Finalized {
                role,
                expected_owner,
                canonical_body,
                ..
            } => {
                let account = account.as_ref().ok_or_else(|| {
                    Error::new(format!("Series Consume finalized {role} was absent"))
                })?;
                if account.owner != *expected_owner
                    || canonical_body.is_some_and(|body| account.data != body)
                {
                    return Err(Error::new(format!(
                        "Series Consume finalized {role} owner or bytes differed"
                    )));
                }
                u32::try_from(account.data.len())
                    .map_err(|_| Error::new("Series Consume observed width escaped u32"))?
            }
            SeriesConsumeRoleSourceV1::PreparedPrediction {
                role,
                fixed_data_len,
                ..
            } => {
                if account.is_some() {
                    return Err(Error::new(format!(
                        "Series Prepare-predicted {role} already exists"
                    )));
                }
                *fixed_data_len
            }
            SeriesConsumeRoleSourceV1::ConsumeVacancy {
                role,
                fixed_data_len,
                ..
            } => {
                if *fixed_data_len != 0 {
                    return Err(Error::new(
                        "Series Consume-created role carried a nonzero initial width",
                    ));
                }
                if account.is_some() {
                    return Err(Error::new(format!(
                        "Series Consume predicted {role} already exists"
                    )));
                }
                *fixed_data_len
            }
        };
    }
    for &(alias, representative) in SERIES_CONSUME_ROUTE_ALIASES_V4 {
        if widths[alias] != widths[representative] {
            return Err(Error::new(
                "Series Consume alias width differed from representative",
            ));
        }
    }
    Ok(widths)
}

fn require_series_consume_alias_addresses_v1(
    roles: &[SeriesConsumeRoleSourceV1<'_>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
) -> Result<()> {
    for &(alias, representative) in SERIES_CONSUME_ROUTE_ALIASES_V4 {
        if roles[alias].address() != roles[representative].address() {
            return Err(Error::new(
                "Series Consume alias named a different physical account",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_trading_sbf::series::account_profile_v4::{
        SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4, SeriesConsumeAccountProfileInputV4,
        encode_series_consume_account_profile_v4_atomic,
    };

    use crate::series_found_prepare_campaign::{
        derive_series_found_prepare_preprofile_v1,
        tests::{compiler_input_with_plan, prepared_founder_with_plan},
    };
    use crate::series_found_prepare_driver::SeriesPrepareFinalizedRecordV1;

    #[test]
    fn snapshot_accepts_only_native_system_at_the_default_address() {
        let system = final_source_v1(
            "System program",
            solana_sdk_ids::system_program::ID,
            solana_sdk_ids::native_loader::ID,
            None,
        );
        assert_eq!(
            series_consume_snapshot_addresses_v1(&[system])
                .expect("canonical native System program"),
            vec![solana_sdk_ids::system_program::ID],
        );
        let hostile_sources = [
            vacancy_v1("substituted future PDA", Pubkey::default(), 0).unwrap(),
            final_source_v1("wrong owner", Pubkey::default(), Pubkey::new_unique(), None),
        ];
        for source in hostile_sources {
            assert_eq!(
                series_consume_snapshot_addresses_v1(&[source])
                    .unwrap_err()
                    .to_string(),
                "Series geometry role named default Pubkey",
            );
        }
    }

    fn published(
        registry: Pubkey,
        schema: [u8; 32],
        body: &[u8],
    ) -> crate::runtime::PublishedRecord {
        let digest: [u8; 32] = Sha256::digest(body).into();
        crate::runtime::PublishedRecord {
            schema,
            digest,
            raw: Pubkey::find_program_address(
                &[
                    dclutch_registry::record::RAW_RECORD_PDA_SEED_V1,
                    &schema,
                    &digest,
                ],
                &registry,
            )
            .0,
            staging: Pubkey::find_program_address(
                &[
                    dclutch_registry::record::STAGING_CURSOR_PDA_SEED_V1,
                    &schema,
                    &digest,
                ],
                &registry,
            )
            .0,
        }
    }

    fn pair<'a>(
        registry: Pubkey,
        schema: [u8; 32],
        body: &'a [u8],
    ) -> SeriesPrepareFinalizedRecordV1<'a> {
        let key = RecordKeyV1::new(
            SchemaReleaseId::new(schema).expect("fixture schema"),
            ContentDigest::new(Sha256::digest(body).into()).expect("fixture digest"),
        );
        let raw = Pubkey::find_program_address(
            &[
                key.raw_record_pda_seeds().domain(),
                key.raw_record_pda_seeds().schema_release_id().as_bytes(),
                key.raw_record_pda_seeds().expected_digest().as_bytes(),
            ],
            &registry,
        )
        .0;
        let staging = Pubkey::find_program_address(
            &[
                key.staging_cursor_pda_seeds().domain(),
                key.staging_cursor_pda_seeds()
                    .schema_release_id()
                    .as_bytes(),
                key.staging_cursor_pda_seeds().expected_digest().as_bytes(),
            ],
            &registry,
        )
        .0;
        SeriesPrepareFinalizedRecordV1 {
            schema,
            body,
            raw,
            staging,
        }
    }

    /// Construct the test M0 only through Market's ProjectFound projection
    /// owner.  This fixture supplies canonical record facts; it cannot name
    /// the ProjectFound coordinate order or substitute an independently
    /// assembled M0 frame.
    fn canonical_m0_publication(
        prepared: &crate::series_founder::PreparedSeriesFounderV1,
        plan: &crate::model::SuccessorPlan,
        selection: &crate::series_found_prepare_campaign::SeriesFoundPrepareSelectionInputV1<'_>,
        preprofile: &crate::series_found_prepare_campaign::SeriesFoundPreparePreprofileV1,
    ) -> crate::market::FutureMarketImmutablePublicationV1 {
        use dclutch_market::realm::REALM_SCHEMA_RELEASE_ID_V1;
        use dclutch_source::{
            MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1, SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_SPEC_SCHEMA_ID_V1,
        };

        let registry = Pubkey::new_from_array(selection.registry_program.to_bytes());
        let (_, market_input, _, _, _) =
            crate::market::tests::selected_family_compiler_fixture_v1();
        let source_spec = crate::runtime::decode_hex(&market_input.source_spec_hex)
            .expect("canonical source specification");
        let source_capacity = crate::runtime::decode_hex(&market_input.source_capacity_profile_hex)
            .expect("canonical source capacity profile");
        let realm = published(
            registry,
            REALM_SCHEMA_RELEASE_ID_V1,
            &prepared.publication.realm,
        );
        let product = published(
            registry,
            dclutch_product::admission::PRODUCT_RECORD_SCHEMA_ID_V2,
            &prepared.publication.product,
        );
        let domain = published(
            registry,
            dclutch_product::admission::RESULT_DOMAIN_SCHEMA_ID_V2,
            &prepared.publication.domain,
        );
        let portfolio = published(
            registry,
            dclutch_product::admission::PORTFOLIO_SCHEMA_ID_V2,
            &prepared.publication.portfolio,
        );
        let basis = published(
            registry,
            dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            &prepared.publication.basis,
        );
        let source = published(
            registry,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
            &prepared.publication.source,
        );
        let source_spec = published(registry, SOURCE_SPEC_SCHEMA_ID_V1, &source_spec);
        let source_capacity = published(
            registry,
            SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
            &source_capacity,
        );
        let manifest = published(
            registry,
            dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            &prepared.publication.manifest,
        );
        let absent = [0_u8; 32];
        let floor = (
            Pubkey::find_program_address(
                &[
                    dclutch_registry::record::RAW_RECORD_PDA_SEED_V1,
                    &MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
                    &absent,
                ],
                &registry,
            )
            .0,
            Pubkey::find_program_address(
                &[
                    dclutch_registry::record::STAGING_CURSOR_PDA_SEED_V1,
                    &MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
                    &absent,
                ],
                &registry,
            )
            .0,
        );
        let selected_release = preprofile
            .predicted_core
            .identity
            .selected_release_set
            .to_bytes();
        let activation = Pubkey::find_program_address(
            &[
                dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
                &selected_release,
            ],
            &registry,
        )
        .0;
        let registry_artifact = crate::runtime::record(&plan, "registry_artifact_release")
            .expect("canonical Registry artifact pair");
        let rent_artifact = crate::runtime::record(&plan, "rent_artifact_release")
            .expect("canonical Rent artifact pair");
        let project_found = crate::market::project_found_snapshot_from_closure_v2(
            crate::market::MarketProjectFoundProjectionV1 {
                payer: selection.material.payer,
                market: selection.material.market,
                rent_credit: selection.material.rent_credit,
                rent_program: crate::plan::pubkey(&plan.rent_credit.program_id)
                    .expect("canonical Rent program"),
                closure: crate::market::MarketProjectFoundClosureV1 {
                    realm,
                    product,
                    domain,
                    portfolio,
                    basis,
                    source,
                    source_spec,
                    source_capacity_profile: source_capacity,
                    manipulation_floor: floor,
                    manifest,
                    activation,
                    core: selection.material.core,
                    core_programdata: crate::upgrade::target_programdata(selection.material.core),
                    registry,
                    infrastructure: crate::plan::pubkey(&plan.infrastructure_profile.address)
                        .expect("canonical infrastructure profile"),
                    registry_artifact,
                    registry_programdata: crate::upgrade::target_programdata(registry),
                    rent_artifact,
                    rent_programdata: crate::upgrade::target_programdata(
                        selection.material.rent_program,
                    ),
                    price_gate: None,
                },
            },
        )
        .expect("canonical ordinary ProjectFound projection")
        .try_into()
        .expect("canonical ProjectFound width");
        let series_prepare_records = vec![
            (realm, prepared.publication.realm.clone()),
            (product, prepared.publication.product.clone()),
            (domain, prepared.publication.domain.clone()),
            (portfolio, prepared.publication.portfolio.clone()),
            (basis, prepared.publication.basis.clone()),
            (source, prepared.publication.source.clone()),
            (
                source_spec,
                crate::runtime::decode_hex(&market_input.source_spec_hex).expect("source spec"),
            ),
            (
                source_capacity,
                crate::runtime::decode_hex(&market_input.source_capacity_profile_hex)
                    .expect("source capacity"),
            ),
            (manifest, prepared.publication.manifest.clone()),
        ]
        .into_iter()
        .map(|(published, body)| crate::market::FutureMarketFinalizedRecordV1 { published, body })
        .collect();
        crate::market::FutureMarketImmutablePublicationV1 {
            realm,
            product,
            domain,
            portfolio,
            manifest,
            rent_credit: selection.material.rent_credit,
            project_found,
            principal_cap_sets: selection.principal_cap_sets,
            series_prepare_records,
            series_prepare_vacancies: vec![floor.0, floor.1],
        }
    }

    fn source(address: Pubkey) -> SeriesConsumeRoleSourceV1<'static> {
        SeriesConsumeRoleSourceV1::ConsumeVacancy {
            role: "canonical future role",
            address,
            fixed_data_len: 0,
        }
    }

    #[test]
    fn generated_aliases_fill_from_the_canonical_representative() {
        let address = Pubkey::new_unique();
        let mut roles = std::array::from_fn(|_| Some(source(address)));
        let (alias, _) = SERIES_CONSUME_ROUTE_ALIASES_V4[0];
        roles[alias] = None;
        resolve_series_consume_aliases_v1(&mut roles).expect("generated alias fill");
        let roles = roles.map(Option::unwrap);
        require_series_consume_alias_addresses_v1(&roles).expect("canonical alias address");
    }

    #[test]
    fn generated_aliases_refuse_substituted_physical_role() {
        let address = Pubkey::new_unique();
        let mut roles = std::array::from_fn(|_| Some(source(address)));
        let (alias, _) = SERIES_CONSUME_ROUTE_ALIASES_V4[0];
        roles[alias] = Some(source(Pubkey::new_unique()));
        let error = resolve_series_consume_aliases_v1(&mut roles).expect_err("alias substitution");
        assert!(error.to_string().contains("alias"), "{error}");
    }

    #[test]
    fn observed_prestate_cannot_change_the_selected_profile() {
        let predicted = [0_u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4];
        require_series_consume_prestate_invariance_v1(&predicted, &predicted)
            .expect("identical prestate widths");
        let mut observed = predicted;
        observed[6] = 1;
        assert!(
            require_series_consume_prestate_invariance_v1(&predicted, &observed)
                .expect_err("profile substitution")
                .to_string()
                .contains("changed")
        );
    }

    #[test]
    fn full_constructor_compiles_initial_consume_profile_from_the_child_bank() {
        let (prepared, plan) = prepared_founder_with_plan();
        let parent_root = Pubkey::new_unique();
        let mut selection = compiler_input_with_plan(
            &prepared,
            &plan,
            parent_root,
            &prepared.admitted.tickets()[0],
        );
        selection.geometry = None;
        let registry = Pubkey::new_from_array(selection.registry_program.to_bytes());
        let preprofile = derive_series_found_prepare_preprofile_v1(&mut selection)
            .expect("canonical campaign child bank");
        let portfolio = dclutch_product::PortfolioV2::decode(&prepared.publication.portfolio)
            .expect("canonical M0 Portfolio");
        let claims = dclutch_claims::founding_v5::ClaimsFoundingRequestV5::decode(
            preprofile.prepare_children.consume_requests().claims,
        )
        .expect("canonical Claims child");
        let consume_children = preprofile.prepare_children.consume_requests();
        let lock = dclutch_custody::ProjectedCustodyRequestV1::decode(consume_children.lock)
            .expect("canonical projected Lock child");
        let realize = dclutch_custody::ProjectedCustodyRequestV1::decode(consume_children.realize)
            .expect("canonical projected Realize child");
        let realized_replay_from_lock = Pubkey::find_program_address(
            &dclutch_custody::ProjectedCustodyStateSeedsV2::from_request(lock).as_slices(),
            &selection.material.custody,
        )
        .0;
        let realized_replay_from_realize = Pubkey::find_program_address(
            &dclutch_custody::ProjectedCustodyStateSeedsV2::from_request(realize).as_slices(),
            &selection.material.custody,
        )
        .0;
        let source_replay = Pubkey::find_program_address(
            &dclutch_custody::ProjectedCustodySourceReplaySeedsV1::from_request(lock).as_slices(),
            &selection.material.custody,
        )
        .0;
        assert_eq!(
            realized_replay_from_lock, realized_replay_from_realize,
            "Lock and Realize own the same projected State address before Custody rewrites it",
        );
        assert_eq!(
            realized_replay_from_realize,
            preprofile.physical().realized_hoard_replay,
            "the native ProjectedState seed is the in-place normal replay after Realize",
        );
        assert_eq!(
            source_replay,
            preprofile.physical().normal_replay,
            "Lock's distinct source replay remains the SeriesEscrow cursor it closes",
        );
        assert_ne!(source_replay, realized_replay_from_realize);
        assert_eq!(
            claims.claim_count(),
            portfolio.coefficient_count(),
            "the Claims V6 runtime width comes from M0 Portfolio, never a fixture literal",
        );
        dclutch_product::economic_slice::refunding_failure_index(claims.claim_count())
            .expect("every Series founding names a derivable V6 failure escrow");

        let publication = canonical_m0_publication(&prepared, &plan, &selection, &preprofile);
        let m0_records =
            crate::series_found_prepare_driver::series_prepare_records_from_m0_publication_v1(
                &publication,
            );
        let portfolio =
            crate::series_found_prepare_driver::series_prepare_record_from_m0_publication_v1(
                &m0_records,
                dclutch_product::admission::PORTFOLIO_SCHEMA_ID_V2,
                &prepared.publication.portfolio,
                "Portfolio",
            )
            .expect("publisher Portfolio record");
        let template = pair(
            registry,
            dclutch_trading::series::SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
            prepared.admitted.template(),
        );
        let occurrence = pair(
            registry,
            dclutch_trading::series::SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
            &prepared.admitted.occurrences()[0],
        );
        let ticket = pair(
            registry,
            dclutch_trading::series::SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
            &prepared.admitted.tickets()[0],
        );
        let mut finalized_accounts = publication
            .project_found
            .iter()
            .copied()
            .filter(|address| {
                *address != selection.material.market
                    && !m0_records
                        .iter()
                        .any(|record| record.raw == *address || record.staging == *address)
                    && !publication.series_prepare_vacancies.contains(address)
            })
            .map(|address| SeriesPrepareFinalizedAccountV1 {
                address,
                expected_owner: Pubkey::new_unique(),
            })
            .collect::<Vec<_>>();
        finalized_accounts.extend([
            SeriesPrepareFinalizedAccountV1 {
                address: selection.material.mint,
                expected_owner: Pubkey::new_unique(),
            },
            SeriesPrepareFinalizedAccountV1 {
                address: selection.material.token_program,
                expected_owner: Pubkey::new_unique(),
            },
        ]);
        let m0 = crate::series_found_prepare_driver::series_prepare_m0_frame_from_publication_v1(
            &publication,
            &m0_records,
            &finalized_accounts,
        );
        let hydration = SeriesPrepareHydrationRecordsV1 {
            template,
            occurrence,
            ticket,
            portfolio,
        };
        let input = SeriesConsumeGeometryInputV1 {
            preprofile: &preprofile,
            m0,
            records: hydration,
            parent_root: SeriesParentRootFactV1::Finalized {
                root: parent_root,
                observed_data_len: dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
                observed_lamports: 1,
            },
            registry,
            core: selection.material.core,
            trading: selection.material.trading,
            custody: selection.material.custody,
            claims: selection.material.claims,
            rent_program: selection.material.rent_program,
            prestate: SeriesConsumePrestateV1::PreparedPrediction,
            minimum_slot: 1,
        };
        let roles = derive_series_consume_role_sources_v1(&input)
            .expect("all prefix, Custody, Claims, and Core routes derive");
        let ticket_state = Pubkey::new_from_array(selection.ticket_state_account.to_bytes());
        let ticket_roles = roles
            .iter()
            .filter(|role| role.address() == ticket_state)
            .collect::<Vec<_>>();
        assert!(
            !ticket_roles.is_empty(),
            "native Core frame contains the Ticket replay"
        );
        for role in ticket_roles {
            assert!(
                matches!(role, SeriesConsumeRoleSourceV1::PreparedPrediction { fixed_data_len, .. }
                if *fixed_data_len as usize == dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3),
                "Ticket replay is created by Prepare before Consume: {role:?}"
            );
        }
        let observed_input = SeriesConsumeGeometryInputV1 {
            prestate: SeriesConsumePrestateV1::ObservedPrepared,
            ..input
        };
        let observed_roles = derive_series_consume_role_sources_v1(&observed_input)
            .expect("post-Prepare routes require finalized resources");
        for role in observed_roles
            .iter()
            .filter(|role| role.address() == ticket_state)
        {
            assert!(
                matches!(role, SeriesConsumeRoleSourceV1::Finalized { expected_owner, .. }
                if *expected_owner == selection.material.trading)
            );
        }
        let predicted_input = SeriesConsumeGeometryInputV1 {
            parent_root: SeriesParentRootFactV1::Predicted(
                crate::series_found_prepare_input::SeriesPredictedParentRootV1 {
                    root: parent_root,
                    data_len: dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
                },
            ),
            ..input
        };
        let predicted_roles = derive_series_consume_role_sources_v1(&predicted_input)
            .expect("pre-founding parent root preserves its eventual Consume layout");
        assert!(matches!(
            predicted_roles[0],
            SeriesConsumeRoleSourceV1::PreparedPrediction { fixed_data_len, .. }
                if fixed_data_len as usize
                    == dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5
        ));
        assert!(matches!(
            roles[SERIES_CONSUME_CLAIMS_START_V1 + 31],
            SeriesConsumeRoleSourceV1::ConsumeVacancy {
                fixed_data_len: 0,
                ..
            }
        ));
        assert!(matches!(
            roles[67],
            SeriesConsumeRoleSourceV1::ConsumeVacancy {
                fixed_data_len: 0,
                ..
            }
        ));
        assert!(matches!(
            roles[6],
            SeriesConsumeRoleSourceV1::PreparedPrediction { .. }
        ));
        assert_eq!(
            roles[99].address(),
            realized_replay_from_realize,
            "Claims consumes the normal replay that native Realize writes into State",
        );
        assert_eq!(
            roles[154].address(),
            realized_replay_from_realize,
            "Core Open consumes that same in-place normal replay",
        );

        let mut widths = [0_u32; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4];
        for (coordinate, role) in roles.iter().enumerate() {
            widths[coordinate] = match role {
                SeriesConsumeRoleSourceV1::Finalized { .. } => 1,
                SeriesConsumeRoleSourceV1::PreparedPrediction { fixed_data_len, .. } => {
                    *fixed_data_len
                }
                SeriesConsumeRoleSourceV1::ConsumeVacancy { fixed_data_len, .. } => *fixed_data_len,
            };
        }
        for &(alias, representative) in SERIES_CONSUME_ROUTE_ALIASES_V4 {
            widths[alias] = widths[representative];
        }
        let mut scratch = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        let mut output = vec![0_u8; SERIES_CONSUME_ACCOUNT_PROFILE_BYTES_V4];
        encode_series_consume_account_profile_v4_atomic(
            SeriesConsumeAccountProfileInputV4 {
                fixed_data_lengths: &widths,
            },
            &mut scratch,
            &mut output,
        )
        .expect("initial Consume prestate compiles into Profile13");

        let mut substituted_project_found = publication.project_found;
        substituted_project_found[1] = Pubkey::new_unique();
        let substituted_m0 = SeriesPrepareM0FrameV1 {
            project_found: substituted_project_found,
            ..m0
        };
        let substituted = SeriesConsumeGeometryInputV1 {
            m0: substituted_m0,
            ..input
        };
        let error = derive_series_consume_role_sources_v1(&substituted)
            .expect_err("Claims child must reject a substituted M0 Market");
        assert!(error.to_string().contains("did not join"), "{error}");
    }
}
