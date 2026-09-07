//! The three occurrence acts of the Series terminal campaign, one verb each.
//!
//! Each is ONE driver over the operator, not a second interpreter: the verb
//! hands every argument to `series_terminal_campaign` and fixes the act the
//! campaign may take. The lifecycle planner (`dclutch_operator::series_lifecycle_v3`)
//! remains the only action selector; a verb never chooses Prepare, Consume or
//! Expire, it refuses when the planner selected a different one, so an
//! invocation named for consuming a ticket cannot quietly expire one whose
//! retry window closed between the operator's read and the send.
//!
//! `prepare` creates the occurrence's Ticket replay and writes its first valid
//! `TicketStateV3` (`prepare_funding_artifacts_v5` is its producer), then
//! initializes, opens and locks the escrow under the FUTURE occurrence Market.
//! `consume` founds and opens that Market atomically through the five-route
//! Consume Effect. `expire` refunds the unallocated permit and escrow after
//! the retry deadline through the pre-Market Expire route.
//!
//! Every name here says `local-private-validator`, because that is the only
//! cluster the campaign admits: it refuses any RPC origin that is not
//! `http://127.0.0.1:` at its own input conjunct, its input schema is
//! `dclutch-owned-loopback-series-terminal-campaign-input-v2`, and nothing in
//! this tree writes that input. A `devnet-` spelling of these verbs would name
//! a path with no producer at either end.

use dclutch_trading::series::request::SeriesActionV3;

use crate::{
    Result,
    series_terminal_campaign::{SeriesCampaignPolicyV1, run_with_policy},
};

/// The occurrence opens: the Prepare act.
pub(crate) const SERIES_TERMINAL_PREPARE_COMMAND_V1: &str =
    "local-private-validator-series-terminal-prepare-v1";
/// The prepared Ticket founds and opens its Market: the Consume act.
pub(crate) const SERIES_TERMINAL_CONSUME_COMMAND_V1: &str =
    "local-private-validator-series-terminal-consume-v1";
/// The retry window elapsed and the permit and escrow refund: the Expire act.
pub(crate) const SERIES_TERMINAL_EXPIRE_COMMAND_V1: &str =
    "local-private-validator-series-terminal-expire-v1";

/// Which act a verb is named for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SeriesVerbV1 {
    Prepare,
    Consume,
    Expire,
}

impl SeriesVerbV1 {
    /// The three occurrence acts, in lifecycle order.
    pub(crate) const ALL: [Self; 3] = [Self::Prepare, Self::Consume, Self::Expire];

    /// The kernel act this verb drives, and refuses every other.
    pub(crate) const fn act(self) -> SeriesActionV3 {
        match self {
            Self::Prepare => SeriesActionV3::Prepare,
            Self::Consume => SeriesActionV3::Consume,
            Self::Expire => SeriesActionV3::Expire,
        }
    }

    pub(crate) const fn command(self) -> &'static str {
        match self {
            Self::Prepare => SERIES_TERMINAL_PREPARE_COMMAND_V1,
            Self::Consume => SERIES_TERMINAL_CONSUME_COMMAND_V1,
            Self::Expire => SERIES_TERMINAL_EXPIRE_COMMAND_V1,
        }
    }
}

/// Run one Series verb: the campaign, with this verb's act required.
pub(crate) fn run(arguments: Vec<String>, verb: SeriesVerbV1) -> Result<()> {
    run_with_policy(arguments, SeriesCampaignPolicyV1::for_act(verb.act()))
}

/// Usage text for the three verbs.
pub(crate) fn usage() -> &'static str {
    "  local-private-validator-series-terminal-prepare-v1 \\\n\
     \x20   | local-private-validator-series-terminal-consume-v1 \\\n\
     \x20   | local-private-validator-series-terminal-expire-v1 \\\n\
     \x20   --input ABSOLUTE_JSON --journal-dir ABSOLUTE_EXISTING_DIR \\\n\
     \x20   --completion ABSOLUTE_NEW_JSON --fee-payer-keypair ABSOLUTE_JSON [--execute]\n\
     \x20 (one driver each over the Series lifecycle planner; the verb refuses when the planner \
     selected another act)\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each verb is named for exactly one kernel act, and the three are the
    /// three occurrence acts -- Retire and Close stay the bare campaign's.
    #[test]
    fn the_three_verbs_are_the_three_occurrence_acts() {
        let acts: Vec<SeriesActionV3> = SeriesVerbV1::ALL
            .into_iter()
            .map(SeriesVerbV1::act)
            .collect();
        assert_eq!(
            acts,
            [
                SeriesActionV3::Prepare,
                SeriesActionV3::Consume,
                SeriesActionV3::Expire
            ]
        );
        assert_eq!(
            SeriesVerbV1::Consume.command(),
            "local-private-validator-series-terminal-consume-v1"
        );
    }

    /// Every verb name says which cluster its campaign reaches, and the usage
    /// teaches the same three names the dispatcher answers to. The campaign
    /// refuses a non-loopback origin at its own input conjunct, so a verb
    /// spelled `devnet-` would document a path that refuses.
    #[test]
    fn every_verb_names_the_only_cluster_its_campaign_admits() {
        for verb in SeriesVerbV1::ALL {
            assert!(
                verb.command().starts_with("local-private-validator-"),
                "{} does not name the cluster its campaign admits",
                verb.command(),
            );
            assert!(
                usage().contains(verb.command()),
                "{} is dispatched and never taught",
                verb.command(),
            );
        }
    }
}
