//! Typed geometry assembly for the first Series Prepare frame.
//!
//! A Prepare Profile has 111 logical coordinates, while only 55 carry a
//! physical account after the release-owned alias compression.  This module
//! makes that distinction explicit: live accounts are observations, immutable
//! Registry records are canonical bodies, and accounts the first Prepare will
//! create are predicted fixed-layout states.  A future vacancy is never
//! presented as an observed account of length zero.

use dclutch_trading_sbf::series::{
    artifacts_v3::{
        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
    },
    lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
    occurrence_artifacts_v4::SeriesPrepareChildRequestsV4,
    prepare_funding_artifacts_v5::{
        SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5, SeriesPrepareAccountProfileInputV5,
        emit_series_prepare_funding_artifacts_v5,
    },
};

use crate::{Error, Result, rpc::RpcAccount};

const PREPARE_PREFIX: usize = 6;
const PROJECTED_INITIALIZE: usize = 47;
const PROJECTED_OPEN: usize = 15;
const REPLAY_INITIALIZE: usize = 13;
const ESCROW_OPEN: usize = 16;
const ESCROW_LOCK: usize = 14;

const _: () = assert!(
    PREPARE_PREFIX
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
            Self::Observed { role, data_len } | Self::Predicted { role, data_len } => {
                (*role, *data_len)
            }
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

    fn require_predicted(&self, expected: u32, coordinate: usize) -> Result<()> {
        match self {
            Self::Predicted { data_len, .. } if *data_len == expected => Ok(()),
            _ => Err(Error::new(format!(
                "Series Prepare coordinate {coordinate} must name the release-owned predicted width {expected}"
            ))),
        }
    }
}

/// The six pre-route representatives.  Their order is protocol-owned by the
/// Prepare artifact: root, Template, occurrence, Portfolio, Ticket record,
/// and the vacant Ticket replay state.
#[derive(Clone, Debug)]
pub(crate) struct SeriesPrepareOuterWidthsV1 {
    pub(crate) root: SeriesPrepareWidthV1,
    pub(crate) template: SeriesPrepareWidthV1,
    pub(crate) occurrence: SeriesPrepareWidthV1,
    pub(crate) portfolio: SeriesPrepareWidthV1,
    pub(crate) ticket: SeriesPrepareWidthV1,
    pub(crate) ticket_state: SeriesPrepareWidthV1,
}

/// Complete canonical five-route physical source for Prepare.
///
/// Each array uses the selected child's own account-frame cardinality.  The
/// source constructor supplies finalized account widths for existing accounts
/// and [`SeriesPrepareWidthV1::Predicted`] only for fixed layouts created by
/// this first Prepare.  No raw `[u32; 111]` crosses this boundary.
#[derive(Clone, Debug)]
pub(crate) struct SeriesPrepareGeometryInputV1 {
    pub(crate) outer: SeriesPrepareOuterWidthsV1,
    pub(crate) projected_initialize: [SeriesPrepareWidthV1; PROJECTED_INITIALIZE],
    pub(crate) projected_open: [SeriesPrepareWidthV1; PROJECTED_OPEN],
    pub(crate) replay_initialize: [SeriesPrepareWidthV1; REPLAY_INITIALIZE],
    pub(crate) escrow_open: [SeriesPrepareWidthV1; ESCROW_OPEN],
    pub(crate) escrow_lock: [SeriesPrepareWidthV1; ESCROW_LOCK],
}

/// Produce exactly the 111 logical widths consumed by the V5 Prepare profile.
pub(crate) fn build_series_prepare_geometry_v1(
    input: &SeriesPrepareGeometryInputV1,
) -> Result<[u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize]> {
    let mut widths = [0_u32; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize];
    for (coordinate, source) in [
        &input.outer.root,
        &input.outer.template,
        &input.outer.occurrence,
        &input.outer.portfolio,
        &input.outer.ticket,
        &input.outer.ticket_state,
    ]
    .into_iter()
    .enumerate()
    {
        widths[coordinate] = source.width(coordinate)?;
    }
    input.outer.root.require_predicted(
        u32::try_from(SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5)
            .map_err(|_| Error::new("Series Prepare root width escaped u32"))?,
        0,
    )?;
    input.outer.ticket_state.require_predicted(
        u32::try_from(dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3)
            .map_err(|_| Error::new("Series Prepare Ticket width escaped u32"))?,
        5,
    )?;
    let mut next = PREPARE_PREFIX;
    for route in [
        input.projected_initialize.as_slice(),
        input.projected_open.as_slice(),
        input.replay_initialize.as_slice(),
        input.escrow_open.as_slice(),
        input.escrow_lock.as_slice(),
    ] {
        for source in route {
            widths[next] = source.width(next)?;
            next += 1;
        }
    }
    if next != widths.len() {
        return Err(Error::new(
            "Series Prepare route geometry did not cover 111 coordinates",
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
        let input = SeriesPrepareGeometryInputV1 {
            outer: SeriesPrepareOuterWidthsV1 {
                root: predicted("series-root", root),
                template: SeriesPrepareWidthV1::canonical_record(
                    "template",
                    &[0; dclutch_trading::series::SERIES_TEMPLATE_BYTES_V3],
                ),
                occurrence: SeriesPrepareWidthV1::canonical_record(
                    "occurrence",
                    &[0; dclutch_trading::series::SERIES_OCCURRENCE_BYTES_V3],
                ),
                portfolio: SeriesPrepareWidthV1::canonical_record("portfolio", &[0; 96]),
                ticket: SeriesPrepareWidthV1::canonical_record(
                    "ticket",
                    &[0; dclutch_trading::series::SERIES_TICKET_BYTES_V3],
                ),
                ticket_state: predicted("ticket-state", ticket_state),
            },
            projected_initialize: std::array::from_fn(|_| observed("projected-initialize", 4_096)),
            projected_open: std::array::from_fn(|_| observed("projected-open", 4_096)),
            replay_initialize: std::array::from_fn(|_| observed("replay-initialize", 4_096)),
            escrow_open: std::array::from_fn(|_| observed("escrow-open", 4_096)),
            escrow_lock: std::array::from_fn(|_| observed("escrow-lock", 4_096)),
        };
        let widths = build_series_prepare_geometry_v1(&input).unwrap();
        assert_eq!(widths.len(), 111);
        let zero_projected = [0; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3];
        let zero_escrow = [0; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3];
        let artifacts = emit_series_prepare_funding_artifacts_v5(
            SeriesPrepareAccountProfileInputV5 {
                fixed_data_lengths: &widths,
            },
            SeriesPrepareChildRequestsV4 {
                projected_initialize: &zero_projected,
                projected_open: &zero_projected,
                replay_initialize: &zero_escrow,
                escrow_open: &zero_escrow,
                escrow_lock: &zero_escrow,
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
}
