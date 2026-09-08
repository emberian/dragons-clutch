//! Shared canonical role vocabulary for the 161-coordinate Series Consume frame.
//!
//! Route owners populate only their fixed, disjoint interval from decoded child
//! requests. This module retains the single full-frame assembler and the
//! generated alias table is the only authority for physical reuse.

use std::collections::BTreeMap;

use dclutch_registry::record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_trading_sbf::series::{
    account_profile_v4::{SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4, SERIES_CONSUME_ROUTE_ALIASES_V4},
    artifacts_v3::{SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3, SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3},
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
pub(crate) const SERIES_CONSUME_CORE_OPEN_START_V1: usize = SERIES_CONSUME_CLAIMS_START_V1 + 32;

const _: () = assert!(SERIES_CONSUME_CORE_OPEN_START_V1 == 124);
const _: () = assert!(SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4 == 161);

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
    pub(crate) minimum_slot: u64,
}

/// One named physical source for one fixed Consume coordinate.
#[derive(Clone, Debug)]
pub(crate) enum SeriesConsumeRoleSourceV1<'a> {
    Finalized {
        role: &'static str,
        address: Pubkey,
        expected_owner: Pubkey,
        canonical_body: Option<&'a [u8]>,
    },
    PredictedVacancy {
        role: &'static str,
        address: Pubkey,
        fixed_data_len: u32,
    },
}

impl SeriesConsumeRoleSourceV1<'_> {
    pub(crate) fn address(&self) -> Pubkey {
        match self {
            Self::Finalized { address, .. } | Self::PredictedVacancy { address, .. } => *address,
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
    Ok(SeriesConsumeRoleSourceV1::PredictedVacancy {
        role,
        address,
        fixed_data_len: u32::try_from(fixed_data_len)
            .map_err(|_| Error::new("Series Consume fixed role width escaped u32"))?,
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
        .ok_or_else(|| Error::new("Series Consume M0 omitted finalized non-record account"))?;
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

fn m0_record_by_schema_v1<'a>(
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
    let root = match input.parent_root {
        SeriesParentRootFactV1::Predicted(prediction) => {
            let expected =
                dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5;
            if prediction.data_len != expected {
                return Err(Error::new(
                    "Series Consume predicted root width differed from the release layout",
                ));
            }
            vacancy_v1(
                "predicted Series root",
                prediction.root,
                prediction.data_len,
            )?
        }
        SeriesParentRootFactV1::Finalized {
            root,
            observed_data_len,
            ..
        } => {
            if observed_data_len
                != dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5
            {
                return Err(Error::new(
                    "Series Consume finalized root width differed from the release layout",
                ));
            }
            final_source_v1("active Series root", root, input.trading, None)
        }
    };
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

/// Close the generated physical-alias relation after every semantic route has
/// populated its own coordinates.  A route may name an alias itself only when
/// it names the representative's exact address; the generated table remains
/// the sole authority for filling an omitted alias.
pub(crate) fn resolve_series_consume_aliases_v1<'a>(
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
) -> Result<()> {
    for &(alias, representative) in SERIES_CONSUME_ROUTE_ALIASES_V4 {
        let representative = roles
            .get(representative)
            .and_then(Clone::clone)
            .ok_or_else(|| Error::new("Series Consume generated alias lacked representative"))?;
        let alias_slot = roles
            .get_mut(alias)
            .ok_or_else(|| Error::new("Series Consume generated alias escaped fixed frame"))?;
        if let Some(existing) = alias_slot {
            if existing.address() != representative.address() {
                return Err(Error::new(
                    "Series Consume generated alias named a different physical account",
                ));
            }
            continue;
        }
        *alias_slot = Some(representative);
    }
    Ok(())
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
    let mut addresses = Vec::new();
    for role in roles {
        let address = role.address();
        if address == Pubkey::default() {
            return Err(Error::new("Series Consume role named default Pubkey"));
        }
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
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
            SeriesConsumeRoleSourceV1::PredictedVacancy {
                role,
                fixed_data_len,
                ..
            } => {
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

    fn source(address: Pubkey) -> SeriesConsumeRoleSourceV1<'static> {
        SeriesConsumeRoleSourceV1::PredictedVacancy {
            role: "canonical future role",
            address,
            fixed_data_len: 9,
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
}
