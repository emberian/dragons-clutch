//! Typed geometry assembly for the first Series Prepare frame.
//!
//! A Prepare Profile has 116 logical coordinates; the native profile owns
//! their exact physical representatives after alias compression. This module
//! makes that distinction explicit: live accounts are observations, immutable
//! Registry records are canonical bodies, and accounts the first Prepare will
//! create are predicted fixed-layout states.  A future vacancy is never
//! presented as an observed account of length zero.

use dclutch_trading_sbf::series::{
    lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
    prepare_funding_artifacts_v5::{
        SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5, SERIES_PREPARE_ROUTE_ALIASES_V5,
    },
};

#[cfg(test)]
use dclutch_trading_sbf::series::{
    artifacts_v3::{
        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
    },
    prepare_funding_artifacts_v5::{
        SeriesPrepareAccountProfileInputV5, emit_series_prepare_funding_artifacts_v5,
    },
};

use std::collections::BTreeMap;

use solana_sdk::pubkey::Pubkey;

use crate::{
    Error, Result,
    rpc::{Rpc, RpcAccount},
};

const PREPARE_PREFIX: usize = 6;
const PROJECTED_INITIALIZE: usize = 47;
const PROJECTED_OPEN: usize = 15;
const REPLAY_INITIALIZE: usize = 13;
const ESCROW_OPEN: usize = 16;
const ESCROW_LOCK: usize = 14;

const _: () = assert!(
    PREPARE_PREFIX
        + 5
        + PROJECTED_INITIALIZE
        + PROJECTED_OPEN
        + REPLAY_INITIALIZE
        + ESCROW_OPEN
        + ESCROW_LOCK
        == SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize
);

/// A width whose semantic owner is stated at the observation boundary.
#[derive(Clone, Debug)]
pub(crate) enum SeriesPrepareWidthV1 {
    /// A finalized account was read from the validator.
    Observed { role: &'static str, data_len: u32 },
    /// An immutable Registry record body is the account bytes.
    CanonicalRecord { role: &'static str, body_len: usize },
    /// A not-yet-created account has a release-owned fixed layout.
    Predicted { role: &'static str, data_len: u32 },
}

impl SeriesPrepareWidthV1 {
    /// Bind one width to a finalized RPC account rather than a caller number.
    pub(crate) fn observed(role: &'static str, account: &RpcAccount) -> Result<Self> {
        let data_len = u32::try_from(account.data.len())
            .map_err(|_| Error::new(format!("Series Prepare {role} account exceeds u32")))?;
        Ok(Self::Observed { role, data_len })
    }

    /// Bind one immutable Registry record coordinate to the exact body used to
    /// derive its address.
    pub(crate) fn canonical_record(role: &'static str, body: &[u8]) -> Self {
        Self::CanonicalRecord {
            role,
            body_len: body.len(),
        }
    }

    /// Name a fixed-layout account that Prepare itself will create.
    pub(crate) const fn predicted(role: &'static str, data_len: u32) -> Self {
        Self::Predicted { role, data_len }
    }

    fn width(&self, coordinate: usize) -> Result<u32> {
        let (role, width) = match self {
            Self::Observed { role, data_len } => (*role, *data_len),
            // A child-created account has zero bytes at outer admission. Its
            // anticipated layout remains available to the typed owner checks.
            Self::Predicted { role, .. } => (*role, 0),
            Self::CanonicalRecord { role, body_len } => (
                *role,
                u32::try_from(*body_len).map_err(|_| {
                    Error::new(format!(
                        "Series Prepare {role} body exceeds u32 at {coordinate}"
                    ))
                })?,
            ),
        };
        if role.is_empty() {
            return Err(Error::new(format!(
                "Series Prepare coordinate {coordinate} omitted its semantic role"
            )));
        }
        Ok(width)
    }

    fn require_root_width(&self, expected: u32, coordinate: usize) -> Result<()> {
        match self {
            Self::Observed { data_len, .. } | Self::Predicted { data_len, .. }
                if *data_len == expected =>
            {
                Ok(())
            }
            _ => Err(Error::new(format!(
                "Series Prepare coordinate {coordinate} must name the finalized or pre-activation predicted root at width {expected}"
            ))),
        }
    }

    fn require_predicted(&self, expected: u32, coordinate: usize) -> Result<()> {
        match self {
            Self::Predicted { data_len, .. } if *data_len == expected => Ok(()),
            _ => Err(Error::new(format!(
                "Series Prepare coordinate {coordinate} must name the fixed predicted width {expected}"
            ))),
        }
    }
}

/// The six pre-route representatives.  Their order is protocol-owned by the
/// Prepare artifact: root, Template, Product, Portfolio, LinkedBasis,
/// and the vacant Ticket replay state.
#[derive(Clone, Debug)]
pub(crate) struct SeriesPrepareOuterWidthsV1 {
    pub(crate) root: SeriesPrepareWidthV1,
    pub(crate) template: SeriesPrepareWidthV1,
    pub(crate) product: SeriesPrepareWidthV1,
    pub(crate) portfolio: SeriesPrepareWidthV1,
    pub(crate) linked_basis: SeriesPrepareWidthV1,
    pub(crate) ticket_state: SeriesPrepareWidthV1,
}

/// Complete canonical five-route physical source for Prepare.
///
/// Each array uses the selected child's own account-frame cardinality.  The
/// source constructor supplies finalized account widths for existing accounts
/// and [`SeriesPrepareWidthV1::Predicted`] only for fixed layouts created by
/// this first Prepare.  No raw `[u32; 116]` crosses this boundary.
#[derive(Clone, Debug)]
pub(crate) struct SeriesPrepareGeometryInputV1 {
    pub(crate) outer: SeriesPrepareOuterWidthsV1,
    pub(crate) projected_initialize: [SeriesPrepareWidthV1; PROJECTED_INITIALIZE],
    pub(crate) projected_open: [SeriesPrepareWidthV1; PROJECTED_OPEN],
    pub(crate) replay_initialize: [SeriesPrepareWidthV1; REPLAY_INITIALIZE],
    pub(crate) escrow_open: [SeriesPrepareWidthV1; ESCROW_OPEN],
    pub(crate) escrow_lock: [SeriesPrepareWidthV1; ESCROW_LOCK],
    pub(crate) occurrence_evidence: [SeriesPrepareWidthV1; 4],
    pub(crate) custody_program: SeriesPrepareWidthV1,
}

/// One account role in the pre-Prepare frame, before logical aliases are
/// compressed.  Registry bodies and every currently-live account are read at
/// finality; only an account the first Prepare itself creates may be a vacant
/// fixed-layout prediction.
#[derive(Clone, Debug)]
pub(crate) enum SeriesPrepareRoleSourceV1<'a> {
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

/// Full source-owned role layout for all five first-Prepare children.
#[derive(Clone, Debug)]
pub(crate) struct SeriesPrepareRoleLayoutV1<'a> {
    pub(crate) outer: [SeriesPrepareRoleSourceV1<'a>; PREPARE_PREFIX],
    pub(crate) projected_initialize: [SeriesPrepareRoleSourceV1<'a>; PROJECTED_INITIALIZE],
    pub(crate) projected_open: [SeriesPrepareRoleSourceV1<'a>; PROJECTED_OPEN],
    pub(crate) replay_initialize: [SeriesPrepareRoleSourceV1<'a>; REPLAY_INITIALIZE],
    pub(crate) escrow_open: [SeriesPrepareRoleSourceV1<'a>; ESCROW_OPEN],
    pub(crate) escrow_lock: [SeriesPrepareRoleSourceV1<'a>; ESCROW_LOCK],
    pub(crate) occurrence_evidence: [SeriesPrepareRoleSourceV1<'a>; 4],
    pub(crate) custody_program: SeriesPrepareRoleSourceV1<'a>,
}

/// Read the complete current Prepare frame at one finalized slot and derive
/// its 116 profile widths.  Alias coordinates must name the same physical
/// address as their representative; this keeps the final account snapshot
/// below the RPC's 100-key ceiling and turns an accidental alias split into a
/// source error before an artifact can be emitted.
pub(crate) fn observe_series_prepare_geometry_v1(
    rpc: &mut Rpc,
    layout: &SeriesPrepareRoleLayoutV1<'_>,
    minimum_slot: u64,
) -> Result<[u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize]> {
    let sources = prepare_sources_v1(layout);
    require_role_alias_addresses_v1(&sources)?;
    let mut keys = Vec::new();
    for source in &sources {
        let address = source.address();
        if address == Pubkey::default() {
            return Err(Error::new("Series Prepare role named the default address"));
        }
        if !keys.contains(&address) {
            keys.push(address);
        }
    }
    if keys.len() > 100 {
        return Err(Error::new(
            "Series Prepare aliases did not reduce the live frame below one finalized RPC snapshot",
        ));
    }
    let (slot, accounts) = rpc.finalized_accounts(&keys, minimum_slot)?;
    if slot < minimum_slot {
        return Err(Error::new(
            "Series Prepare finalized observation regressed its slot floor",
        ));
    }
    let observed = keys.into_iter().zip(accounts).collect::<BTreeMap<_, _>>();
    let width = |source: &SeriesPrepareRoleSourceV1<'_>| -> Result<SeriesPrepareWidthV1> {
        let account = observed
            .get(&source.address())
            .ok_or_else(|| Error::new("Series Prepare observation omitted a requested role"))?;
        match source {
            SeriesPrepareRoleSourceV1::Finalized {
                role,
                expected_owner,
                canonical_body,
                ..
            } => {
                let account = account.as_ref().ok_or_else(|| {
                    Error::new(format!(
                        "Series Prepare finalized {role} account was absent"
                    ))
                })?;
                if account.owner != *expected_owner
                    || canonical_body.is_some_and(|body| account.data != body)
                {
                    return Err(Error::new(format!(
                        "Series Prepare finalized {role} owner or canonical bytes changed"
                    )));
                }
                match canonical_body {
                    Some(body) => Ok(SeriesPrepareWidthV1::canonical_record(role, body)),
                    None => SeriesPrepareWidthV1::observed(role, account),
                }
            }
            SeriesPrepareRoleSourceV1::PredictedVacancy {
                role,
                fixed_data_len,
                ..
            } => {
                if account.is_some() {
                    return Err(Error::new(format!(
                        "Series Prepare predicted {role} was already allocated"
                    )));
                }
                Ok(SeriesPrepareWidthV1::predicted(role, *fixed_data_len))
            }
        }
    };
    let mut widths = sources.iter().map(width).collect::<Result<Vec<_>>>()?;
    let outer = take_array_v1::<PREPARE_PREFIX>(&mut widths, "outer")?;
    let projected_initialize =
        take_array_v1::<PROJECTED_INITIALIZE>(&mut widths, "projected initialize")?;
    let projected_open = take_array_v1::<PROJECTED_OPEN>(&mut widths, "projected open")?;
    let replay_initialize = take_array_v1::<REPLAY_INITIALIZE>(&mut widths, "replay initialize")?;
    let escrow_open = take_array_v1::<ESCROW_OPEN>(&mut widths, "escrow open")?;
    let escrow_lock = take_array_v1::<ESCROW_LOCK>(&mut widths, "escrow lock")?;
    let occurrence_evidence = take_array_v1::<4>(&mut widths, "occurrence evidence")?;
    let [custody_program] = take_array_v1::<1>(&mut widths, "Custody callee")?;
    let [
        root,
        template,
        product,
        portfolio,
        linked_basis,
        ticket_state,
    ] = outer;
    build_series_prepare_geometry_v1(&SeriesPrepareGeometryInputV1 {
        outer: SeriesPrepareOuterWidthsV1 {
            root,
            template,
            product,
            portfolio,
            linked_basis,
            ticket_state,
        },
        projected_initialize,
        projected_open,
        replay_initialize,
        escrow_open,
        escrow_lock,
        occurrence_evidence,
        custody_program,
    })
}

/// Bind the pre-activation geometry embedded in immutable selected artifacts
/// to the post-activation finalized frame. M1's root cannot be observed until
/// selector-255 creates it, so the compiler carries its release-owned
/// predicted width; every role is then re-read before Prepare.
pub(crate) fn require_series_prepare_geometry_invariance_v1(
    predicted: &[u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize],
    observed: &[u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize],
) -> Result<()> {
    for (coordinate, (expected, actual)) in predicted.iter().zip(observed).enumerate() {
        if expected != actual {
            return Err(Error::new(format!(
                "Series Prepare finalized geometry changed at coordinate {coordinate}: predicted {expected}, observed {actual}"
            )));
        }
    }
    Ok(())
}

impl SeriesPrepareRoleSourceV1<'_> {
    pub(crate) fn address(&self) -> Pubkey {
        match self {
            Self::Finalized { address, .. } | Self::PredictedVacancy { address, .. } => *address,
        }
    }
}

pub(crate) fn prepare_sources_v1<'a>(
    layout: &'a SeriesPrepareRoleLayoutV1<'a>,
) -> Vec<SeriesPrepareRoleSourceV1<'a>> {
    layout
        .outer
        .iter()
        .chain(&layout.projected_initialize)
        .chain(&layout.projected_open)
        .chain(&layout.replay_initialize)
        .chain(&layout.escrow_open)
        .chain(&layout.escrow_lock)
        .chain(&layout.occurrence_evidence)
        .chain(core::iter::once(&layout.custody_program))
        .cloned()
        .collect()
}

fn take_array_v1<const N: usize>(
    values: &mut Vec<SeriesPrepareWidthV1>,
    label: &str,
) -> Result<[SeriesPrepareWidthV1; N]> {
    if values.len() < N {
        return Err(Error::new(format!(
            "Series Prepare {label} width source was truncated"
        )));
    }
    Ok(values
        .drain(..N)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| {
            Error::new(format!(
                "Series Prepare {label} width source changed cardinality"
            ))
        })?)
}

fn require_role_alias_addresses_v1(sources: &[SeriesPrepareRoleSourceV1<'_>]) -> Result<()> {
    for &(alias, representative) in SERIES_PREPARE_ROUTE_ALIASES_V5 {
        let alias = usize::from(alias);
        let representative = usize::from(representative);
        let alias = sources
            .get(alias)
            .ok_or_else(|| Error::new("Series Prepare alias index escaped frame"))?;
        let representative = sources
            .get(representative)
            .ok_or_else(|| Error::new("Series Prepare representative index escaped frame"))?;
        if alias.address() != representative.address() {
            return Err(Error::new(format!(
                "Series Prepare alias {alias:?} did not name its representative {representative:?}"
            )));
        }
    }
    Ok(())
}

/// Produce exactly the 116 logical widths consumed by the V5 Prepare profile.
pub(crate) fn build_series_prepare_geometry_v1(
    input: &SeriesPrepareGeometryInputV1,
) -> Result<[u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize]> {
    let mut widths = [0_u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize];
    for (coordinate, source) in [
        &input.outer.root,
        &input.outer.template,
        &input.outer.product,
        &input.outer.portfolio,
        &input.outer.linked_basis,
        &input.outer.ticket_state,
    ]
    .into_iter()
    .enumerate()
    {
        widths[coordinate] = source.width(coordinate)?;
    }
    input.outer.root.require_root_width(
        u32::try_from(SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5)
            .map_err(|_| Error::new("Series Prepare root width escaped u32"))?,
        0,
    )?;
    input.outer.ticket_state.require_predicted(
        u32::try_from(dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3)
            .map_err(|_| Error::new("Series Prepare Ticket width escaped u32"))?,
        5,
    )?;
    // The root exists before Prepare; Ticket alone is created by outer FundingV5.
    widths[0] = SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5 as u32;
    widths[5] = dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3 as u32;
    let mut next = PREPARE_PREFIX;
    for route in [
        input.projected_initialize.as_slice(),
        input.projected_open.as_slice(),
        input.replay_initialize.as_slice(),
        input.escrow_open.as_slice(),
        input.escrow_lock.as_slice(),
        input.occurrence_evidence.as_slice(),
        core::slice::from_ref(&input.custody_program),
    ] {
        for source in route {
            widths[next] = source.width(next)?;
            next += 1;
        }
    }
    if next != widths.len() {
        return Err(Error::new(
            "Series Prepare route geometry did not cover 116 coordinates",
        ));
    }
    require_release_aliases_v1(&widths)?;
    Ok(widths)
}

// `prepare_funding_artifacts_v5` owns the aliases when it emits the profile.
// Mirroring their value relation here is a source-census control: every
// repeated physical representative must have the same source width before the
// artifact intentionally erases the alias's own width.
fn require_release_aliases_v1(
    widths: &[u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize],
) -> Result<()> {
    for &(alias, representative) in SERIES_PREPARE_ROUTE_ALIASES_V5 {
        let alias = usize::from(alias);
        let representative = usize::from(representative);
        if widths[alias] != widths[representative] {
            return Err(Error::new(format!(
                "Series Prepare alias coordinate {alias} differed from representative {representative}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_vm::account_profile::v3::AccountProfileV3;

    fn observed(role: &'static str, data_len: u32) -> SeriesPrepareWidthV1 {
        // The public constructor takes an RPC account; this narrow test helper
        // supplies one synthetic decoded observation solely to test the
        // release-owned profile mapping.
        SeriesPrepareWidthV1::Observed { role, data_len }
    }

    fn predicted(role: &'static str, data_len: u32) -> SeriesPrepareWidthV1 {
        SeriesPrepareWidthV1::predicted(role, data_len)
    }

    #[test]
    fn full_prepare_profile_preserves_each_native_child_caller() {
        use crate::series_found_prepare_campaign::{
            derive_series_found_prepare_preprofile_v1,
            tests::{compiler_input_with_plan, prepared_founder_with_plan},
        };
        use dclutch_core_contract::ContentId;
        use dclutch_custody::{
            CustodyRequestV1, ProjectedCustodyCallerSeedsV1, ProjectedCustodyRequestV1,
        };
        use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
        use dclutch_trading_sbf::series::prepare_funding_artifacts_v5::{
            SERIES_PREPARE_ROUTE_STARTS_V5, SERIES_PREPARE_TRADING_PROGRAM_IDENTITY_V5,
        };
        use dclutch_vm::account_profile::{
            AccountObservationV1,
            v2::{Error as ProfileError, ProjectionRegistersV2, project_atomic},
        };
        use solana_program::hash::hash;

        let (prepared, plan) = prepared_founder_with_plan();
        let mut selection = compiler_input_with_plan(
            &prepared,
            &plan,
            Pubkey::new_unique(),
            &prepared.admitted.tickets()[0],
        );
        selection.geometry = None;
        let bank = derive_series_found_prepare_preprofile_v1(&mut selection)
            .expect("production child bank");
        let requests = bank.prepare_children.prepare_requests();
        let trading = selection.material.trading;
        let projected = |bytes: &[u8]| {
            let request =
                ProjectedCustodyRequestV1::decode(bytes).expect("native projected request");
            Pubkey::find_program_address(
                &ProjectedCustodyCallerSeedsV1::new(request, hash(bytes).to_bytes()).as_slices(),
                &trading,
            )
            .0
            .to_bytes()
        };
        let normal = |bytes: &[u8]| {
            let request = CustodyRequestV1::decode(bytes).expect("native Custody request");
            let seeds = CallerAuthoritySeedsV1::new(
                ContentId::new(request.release_set).unwrap(),
                request.market,
                ExecutionRoleV1::Trading,
                request.context,
                hash(bytes).to_bytes(),
            )
            .unwrap();
            Pubkey::find_program_address(&seeds.as_slices(), &trading)
                .0
                .to_bytes()
        };
        let callers = [
            projected(requests.projected_initialize),
            projected(requests.projected_open),
            normal(requests.replay_initialize),
            normal(requests.escrow_open),
            normal(requests.escrow_lock),
        ];
        for (index, caller) in callers.iter().enumerate() {
            assert!(
                !callers[..index].contains(caller),
                "native request-digest callers are distinct"
            );
        }
        let lengths = [0_u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize];
        let artifacts = emit_series_prepare_funding_artifacts_v5(
            SeriesPrepareAccountProfileInputV5 {
                fixed_data_lengths: &lengths,
            },
            1,
        )
        .expect("complete native Prepare profile");
        let profile = AccountProfileV3::decode(&artifacts.account_profile)
            .unwrap()
            .base();
        println!(
            "native Prepare profile: logical={}, physical={}, canonical_callers={}",
            profile.fixed_account_count(),
            profile.physical_account_count(0).unwrap(),
            callers.len()
        );
        // Neutral non-caller observations isolate the profile's complete alias
        // and uniqueness rules; caller keys come from the production child bank.
        let mut keys = (0..SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5)
            .map(|_| Pubkey::new_unique().to_bytes())
            .collect::<Vec<_>>();
        let mut bodies = Vec::new();
        let mut privileges = Vec::new();
        for coordinate in 0..usize::from(SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5) {
            let representative = profile.representative(0, coordinate).unwrap();
            let rule = profile
                .rule(false, u16::try_from(representative).unwrap())
                .unwrap();
            if representative != coordinate {
                keys[coordinate] = keys[representative];
            }
            bodies.push(vec![0_u8; usize::try_from(rule.data_length()).unwrap()]);
            privileges.push(rule.privileges());
        }
        for (coordinate, caller) in SERIES_PREPARE_ROUTE_STARTS_V5.iter().zip(callers) {
            keys[usize::from(*coordinate)] = caller;
        }
        let owner = trading.to_bytes();
        let project = |keys: &[[u8; 32]]| {
            let observations = keys
                .iter()
                .enumerate()
                .map(|(i, key)| {
                    AccountObservationV1::new(
                        key,
                        &owner,
                        1,
                        &bodies[i],
                        privileges[i] & 1 != 0,
                        privileges[i] & 2 != 0,
                        privileges[i] & 4 != 0,
                    )
                })
                .collect::<Vec<_>>();
            let scalars = vec![0_u64; usize::from(profile.common_scalar_count())];
            let mut identities = vec![[0_u8; 32]; usize::from(profile.common_identity_count())];
            identities[usize::from(SERIES_PREPARE_TRADING_PROGRAM_IDENTITY_V5)] = owner;
            let mut scratch_s = scalars.clone();
            let mut output_s = scalars.clone();
            let mut scratch_i = identities.clone();
            let mut output_i = identities.clone();
            project_atomic(
                profile,
                0,
                &observations,
                ProjectionRegistersV2 {
                    input_scalars: &scalars,
                    input_identities: &identities,
                    scratch_scalars: &mut scratch_s,
                    scratch_identities: &mut scratch_i,
                    output_scalars: &mut output_s,
                    output_identities: &mut output_i,
                },
                None,
            )
        };
        assert_eq!(
            project(&keys),
            Ok(()),
            "full native profile must accept all five canonical callers"
        );
        for coordinate in &SERIES_PREPARE_ROUTE_STARTS_V5[1..] {
            let mut coalesced = keys.clone();
            coalesced[usize::from(*coordinate)] = callers[0];
            assert_eq!(
                project(&coalesced),
                Err(ProfileError::CrossItemAlias),
                "coalesced child caller at {coordinate}"
            );
        }
    }

    #[test]
    fn full_prepare_geometry_decodes_and_preserves_every_representative() {
        let root = u32::try_from(SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5).unwrap();
        let ticket_state =
            u32::try_from(dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3).unwrap();
        let mut input = SeriesPrepareGeometryInputV1 {
            outer: SeriesPrepareOuterWidthsV1 {
                root: observed("series-root", root),
                template: SeriesPrepareWidthV1::canonical_record(
                    "template",
                    &[0; dclutch_trading::series::SERIES_TEMPLATE_BYTES_V3],
                ),
                product: SeriesPrepareWidthV1::canonical_record(
                    "product",
                    &[0; dclutch_product::admission::PRODUCT_RECORD_BYTES_V2],
                ),
                portfolio: SeriesPrepareWidthV1::canonical_record("portfolio", &[0; 96]),
                linked_basis: SeriesPrepareWidthV1::canonical_record("linked-basis", &[0; 96]),
                ticket_state: predicted("ticket-state", ticket_state),
            },
            projected_initialize: std::array::from_fn(|_| observed("projected-initialize", 4_096)),
            projected_open: std::array::from_fn(|_| observed("projected-open", 4_096)),
            replay_initialize: std::array::from_fn(|_| observed("replay-initialize", 4_096)),
            escrow_open: std::array::from_fn(|_| observed("escrow-open", 4_096)),
            escrow_lock: std::array::from_fn(|_| observed("escrow-lock", 4_096)),
            occurrence_evidence: [
                observed(
                    "occurrence",
                    dclutch_trading::series::SERIES_OCCURRENCE_BYTES_V3 as u32,
                ),
                observed("occurrence-staging", 0),
                observed(
                    "ticket",
                    dclutch_trading::series::SERIES_TICKET_BYTES_V3 as u32,
                ),
                observed("ticket-staging", 0),
            ],
            custody_program: observed("Custody executable", 36),
        };
        input.projected_initialize[1] = predicted(
            "projected-state",
            dclutch_custody::PROJECTED_CUSTODY_STATE_BYTES_V2 as u32,
        );
        input.projected_open[7] = predicted(
            "hoard-vault",
            dclutch_custody::token_svm::ACCOUNT_BYTES as u32,
        );
        input.replay_initialize[8] = predicted(
            "source-replay",
            dclutch_custody::CUSTODY_REPLAY_BYTES_V1 as u32,
        );
        input.escrow_open[10] = predicted(
            "escrow-vault",
            dclutch_custody::token_svm::ACCOUNT_BYTES as u32,
        );
        // The later native windows borrow these same initially vacant accounts.
        input.projected_open[1] = input.projected_initialize[1].clone();
        input.escrow_open[8] = input.replay_initialize[8].clone();
        input.escrow_lock[8] = input.replay_initialize[8].clone();
        input.escrow_lock[11] = input.escrow_open[10].clone();
        let widths = build_series_prepare_geometry_v1(&input).unwrap();
        for coordinate in dclutch_trading_sbf::series::prepare_funding_artifacts_v5::SERIES_PREPARE_CHILD_CREATED_COORDINATES_V5 {
            assert_eq!(widths[coordinate as usize], 0, "initial child-owned vacancy");
        }

        assert_eq!(widths.len(), 116);
        let artifacts = emit_series_prepare_funding_artifacts_v5(
            SeriesPrepareAccountProfileInputV5 {
                fixed_data_lengths: &widths,
            },
            1,
        )
        .unwrap();
        let profile = AccountProfileV3::decode(&artifacts.account_profile).unwrap();
        for coordinate in 0..SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 {
            let rule = profile.base().rule(false, coordinate).unwrap();
            let alias = SERIES_PREPARE_ROUTE_ALIASES_V5
                .iter()
                .any(|(alias, _)| *alias == coordinate);
            if alias {
                assert_eq!(rule.data_length(), 0, "alias {coordinate}");
            } else if coordinate == 0 {
                assert_eq!(rule.data_length(), root);
            } else if coordinate == 1 {
                assert_eq!(
                    rule.data_length(),
                    dclutch_trading::series::SERIES_TEMPLATE_BYTES_V3 as u32
                );
            } else if coordinate == 3 {
                assert_eq!(
                    rule.data_length(),
                    dclutch_product::PORTFOLIO_HEADER_BYTES as u32
                );
            } else if coordinate == 5 {
                assert_eq!(rule.data_length(), ticket_state);
            } else {
                assert_eq!(
                    rule.data_length(),
                    widths[coordinate as usize],
                    "coordinate {coordinate}"
                );
            }
        }
    }

    #[test]
    fn split_alias_address_is_rejected_before_the_rpc_snapshot() {
        let representative = Pubkey::new_unique();
        let mut sources = (0..SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize)
            .map(|_| SeriesPrepareRoleSourceV1::PredictedVacancy {
                role: "future",
                address: representative,
                fixed_data_len: 0,
            })
            .collect::<Vec<_>>();
        sources[17] = SeriesPrepareRoleSourceV1::PredictedVacancy {
            role: "split-alias",
            address: Pubkey::new_unique(),
            fixed_data_len: 0,
        };
        let error = require_role_alias_addresses_v1(&sources).unwrap_err();
        assert!(error.to_string().contains("alias"));
    }

    #[test]
    fn root_and_ticket_state_keep_their_prestate_kinds() {
        let root = u32::try_from(SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5).unwrap();
        let ticket_state =
            u32::try_from(dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3).unwrap();
        let input = SeriesPrepareGeometryInputV1 {
            outer: SeriesPrepareOuterWidthsV1 {
                root: SeriesPrepareWidthV1::canonical_record("root-is-not-a-record", &[0; 1]),
                template: SeriesPrepareWidthV1::canonical_record("template", &[0; 1]),
                product: SeriesPrepareWidthV1::canonical_record("occurrence", &[0; 1]),
                portfolio: SeriesPrepareWidthV1::canonical_record("portfolio", &[0; 1]),
                linked_basis: SeriesPrepareWidthV1::canonical_record("ticket", &[0; 1]),
                ticket_state: observed("ticket-state-must-be-vacant", ticket_state),
            },
            projected_initialize: std::array::from_fn(|_| observed("live", 1)),
            projected_open: std::array::from_fn(|_| observed("live", 1)),
            replay_initialize: std::array::from_fn(|_| observed("live", 1)),
            escrow_open: std::array::from_fn(|_| observed("live", 1)),
            escrow_lock: std::array::from_fn(|_| observed("live", 1)),
            occurrence_evidence: std::array::from_fn(|_| observed("evidence", 1)),
            custody_program: observed("Custody executable", 36),
        };
        let error = build_series_prepare_geometry_v1(&input).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("finalized or pre-activation predicted root")
        );
    }

    #[test]
    fn finalized_geometry_must_equal_the_compiled_pre_activation_prediction() {
        let expected = [7_u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize];
        let mut changed = expected;
        changed[76] = 8;
        let error = require_series_prepare_geometry_invariance_v1(&expected, &changed)
            .expect_err("a changed live role must not reuse selected bytes");
        assert!(error.to_string().contains("coordinate 76"));
    }
}
