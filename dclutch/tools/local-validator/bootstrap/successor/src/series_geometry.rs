//! Typed geometry assembly for the first Series Prepare frame.
//!
//! A Prepare Profile has 115 logical coordinates, while only 59 carry a
//! physical account after the release-owned alias compression.  This module
//! makes that distinction explicit: live accounts are observations, immutable
//! Registry records are canonical bodies, and accounts the first Prepare will
//! create are predicted fixed-layout states.  A future vacancy is never
//! presented as an observed account of length zero.

use dclutch_trading_sbf::series::{
    lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
    prepare_funding_artifacts_v5::SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5,
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
        + 4
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
/// this first Prepare.  No raw `[u32; 115]` crosses this boundary.
#[derive(Clone, Debug)]
pub(crate) struct SeriesPrepareGeometryInputV1 {
    pub(crate) outer: SeriesPrepareOuterWidthsV1,
    pub(crate) projected_initialize: [SeriesPrepareWidthV1; PROJECTED_INITIALIZE],
    pub(crate) projected_open: [SeriesPrepareWidthV1; PROJECTED_OPEN],
    pub(crate) replay_initialize: [SeriesPrepareWidthV1; REPLAY_INITIALIZE],
    pub(crate) escrow_open: [SeriesPrepareWidthV1; ESCROW_OPEN],
    pub(crate) escrow_lock: [SeriesPrepareWidthV1; ESCROW_LOCK],
    pub(crate) occurrence_evidence: [SeriesPrepareWidthV1; 4],
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
}

/// Read the complete current Prepare frame at one finalized slot and derive
/// its 115 profile widths.  Alias coordinates must name the same physical
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
    fn address(&self) -> Pubkey {
        match self {
            Self::Finalized { address, .. } | Self::PredictedVacancy { address, .. } => *address,
        }
    }
}

fn prepare_sources_v1<'a>(
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
    for (alias, representative) in [
        (17, 14),
        (19, 12),
        (41, 8),
        (42, 13),
        (44, 9),
        (45, 16),
        (53, 6),
        (54, 7),
        (55, 8),
        (56, 9),
        (57, 10),
        (58, 11),
        (59, 12),
        (64, 14),
        (65, 15),
        (66, 16),
        (67, 18),
        (68, 6),
        (69, 18),
        (70, 8),
        (71, 9),
        (72, 10),
        (73, 11),
        (74, 21),
        (75, 22),
        (77, 14),
        (78, 16),
        (79, 15),
        (81, 6),
        (82, 18),
        (83, 8),
        (84, 9),
        (85, 10),
        (86, 11),
        (87, 21),
        (88, 22),
        (89, 76),
        (90, 62),
        (92, 61),
        (93, 63),
        (94, 14),
        (95, 16),
        (96, 15),
        (97, 6),
        (98, 18),
        (99, 8),
        (100, 9),
        (101, 10),
        (102, 11),
        (103, 21),
        (104, 22),
        (105, 76),
        (106, 62),
        (108, 91),
        (109, 61),
        (110, 63),
    ] {
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

/// Produce exactly the 115 logical widths consumed by the V5 Prepare profile.
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
    ] {
        for source in route {
            widths[next] = source.width(next)?;
            next += 1;
        }
    }
    if next != widths.len() {
        return Err(Error::new(
            "Series Prepare route geometry did not cover 115 coordinates",
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
    for (alias, representative) in [
        (17, 14),
        (19, 12),
        (41, 8),
        (42, 13),
        (44, 9),
        (45, 16),
        (53, 6),
        (54, 7),
        (55, 8),
        (56, 9),
        (57, 10),
        (58, 11),
        (59, 12),
        (64, 14),
        (65, 15),
        (66, 16),
        (67, 18),
        (68, 6),
        (69, 18),
        (70, 8),
        (71, 9),
        (72, 10),
        (73, 11),
        (74, 21),
        (75, 22),
        (77, 14),
        (78, 16),
        (79, 15),
        (81, 6),
        (82, 18),
        (83, 8),
        (84, 9),
        (85, 10),
        (86, 11),
        (87, 21),
        (88, 22),
        (89, 76),
        (90, 62),
        (92, 61),
        (93, 63),
        (94, 14),
        (95, 16),
        (96, 15),
        (97, 6),
        (98, 18),
        (99, 8),
        (100, 9),
        (101, 10),
        (102, 11),
        (103, 21),
        (104, 22),
        (105, 76),
        (106, 62),
        (108, 91),
        (109, 61),
        (110, 63),
    ] {
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

        assert_eq!(widths.len(), 115);
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
            let alias = matches!(coordinate, 17|19|41|42|44|45|53..=59|64..=67|68..=75|77..=79|81..=90|92..=96|97..=106|108..=110);
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
