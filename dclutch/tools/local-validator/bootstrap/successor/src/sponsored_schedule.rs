//! Schedule the sponsored relay so its observation lands INSIDE the window.
//!
//! COHORT-13 IS WHY THIS EXISTS. Its Terminal window opened at 17:22:39 UTC and
//! closed 1,800 seconds later; nothing relayed inside it; the pinned Pyth
//! account had already moved past it by the time anyone looked, and the honest
//! observation for that window became permanently unavailable -- not late,
//! unreachable. The market resolved by the funded failure walk, which paid the
//! founder and paid the two strangers nothing. No code was wrong. Nothing ran
//! at the right time, and nothing in the tree said when the right time was.
//!
//! THE TWO ACTIONS ARE NOT AT THE SAME TIME, and conflating them is the trap
//! this module is shaped around. Reading the two conjuncts together:
//!
//! - `Capture` refuses once `clock > window.end + max_age_seconds`
//!   (`sponsored_push_v1.rs:127-133`, `ProviderFreshness`), and separately
//!   refuses an update whose own `publish_time` is outside
//!   `[start, end]` (`provider_join_v2.rs:244`, `InvalidObservationSchedule`
//!   mapped to `ProviderWindow`). So a capture must run while the pinned
//!   account holds an in-window observation -- which is INSIDE the window, plus
//!   whatever it takes for one provider push to land after it opens.
//! - `Settle` refuses while `clock <= window.end + max_age_seconds`
//!   (`sponsored_push_v1.rs:875-878`, `ProviderFreshness`). It is legal only
//!   STRICTLY AFTER that deadline -- two hours after the window closed, on
//!   cohort-13's numbers.
//!
//! The candidate captured inside the window survives that wait: settle
//! re-normalizes against the CANDIDATE's own `snapshot_unix_seconds`
//! (`sponsored_push_v1.rs:1139`), not against the live account. That is the
//! whole reason an in-window capture is worth scheduling.
//!
//! A BOUNDED WAIT, NEVER A POLL. `wait_until_unix_seconds_v1` computes one
//! sleep against the chain's own clock, re-reads it once, and refuses when the
//! target is further away than the caller's stated ceiling. The only existing
//! wait-for-wall-clock loop in this tree
//! (`tools/gauntlet/relayed-vertical/src/vertical.rs:1243`) polls every two
//! seconds with no cap at all, which is fine inside a gauntlet that owns its
//! validator and is not fine against a public endpoint.

use std::{path::PathBuf, time::Duration};

use serde::Serialize;

use crate::{Error, Result, rpc::Rpc};

/// This command's name.
pub(crate) const COMMAND_V1: &str = "devnet-sponsored-relay-schedule-v1";

/// The emitted document's schema.
pub(crate) const SCHEDULE_FORMAT_V1: &str = "dclutch-devnet-sponsored-relay-schedule-v1";

/// Default seconds after the window opens before a capture is attempted.
///
/// STATED, NOT DERIVED, and labelled as provisional. The conjunct that decides
/// it is the pinned account's own `publish_time`, and no account in the tree
/// tells a planner what the sponsored feed's cadence is. Cohort-13's window was
/// founded as "1,800 s wide, about 5.75 measured provider cadences", which puts
/// one cadence near 313 s; a margin under that can find an account whose last
/// push predates the window and refuse `ProviderWindow`. That refusal is CHEAP
/// and RETRYABLE -- it costs a preflight and nothing on chain -- so the default
/// is deliberately short and the retry ladder, not the margin, is what makes
/// the capture land. A longer margin trades a retry for window burned.
pub(crate) const DEFAULT_CAPTURE_MARGIN_SECONDS_V1: i64 = 60;

/// Default seconds after the primary deadline before a settle is attempted.
///
/// Settle is legal STRICTLY after `end + max_age`, so any positive margin is
/// admissible and this one only buys clock skew between the planner and the
/// validator. Cohort-13's failure walk fired thirteen seconds after its own
/// deadline and was legal.
pub(crate) const DEFAULT_SETTLE_MARGIN_SECONDS_V1: i64 = 30;

/// The window a schedule is planned against, in its own terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct RelayWindowV1 {
    /// `WindowSpecV1::start_unix_seconds`.
    pub(crate) start_unix_seconds: i64,
    /// `WindowSpecV1::end_unix_seconds`.
    pub(crate) end_unix_seconds: i64,
    /// `WindowSpecV1::max_age_seconds`, which is also the settle deadline's arm.
    pub(crate) max_age_seconds: u32,
    /// `WindowSpecV1::cadence_tolerance_seconds`, inert on this route and zero.
    pub(crate) cadence_tolerance_seconds: u32,
}

impl RelayWindowV1 {
    /// Decode the 112-byte `DCLTWIN1` record this market resolves against.
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self> {
        let window = dclutch_source::WindowSpecV1::decode(bytes)
            .map_err(|error| Error::new(format!("window spec record: {error:?}")))?;
        Self::from_spec(window)
    }

    /// Project the record's own accessors, refusing a window nothing can hit.
    pub(crate) fn from_spec(window: dclutch_source::WindowSpecV1) -> Result<Self> {
        let start = window.start_unix_seconds();
        let end = window.end_unix_seconds();
        if end <= start {
            return Err(Error::new(format!(
                "window {start}..{end} is empty or inverted; no observation can be inside it"
            )));
        }
        Ok(Self {
            start_unix_seconds: start,
            end_unix_seconds: end,
            max_age_seconds: window.max_age_seconds(),
            cadence_tolerance_seconds: window.cadence_tolerance_seconds(),
        })
    }

    /// `window.end + max_age`: the last capture second and the first settle one.
    pub(crate) fn primary_deadline_unix_seconds(self) -> Result<i64> {
        self.end_unix_seconds
            .checked_add(i64::from(self.max_age_seconds))
            .ok_or_else(|| Error::new("window end + max_age overflows"))
    }
}

/// What one scheduled action is waiting for, and whether it can still happen.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RelayVerdictV1 {
    /// The action's earliest legal moment has not arrived; wait.
    Waiting,
    /// Now is inside the action's legal interval; run it.
    Due,
    /// The interval closed. For a capture this is cohort-13's outcome exactly.
    Missed,
}

/// One action's window, in the terms the program will judge it by.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct ScheduledActionV1 {
    /// The first second this schedule will attempt the action.
    pub(crate) at_unix_seconds: i64,
    /// The first second the PROGRAM admits it.
    pub(crate) legal_from_unix_seconds: i64,
    /// The last second the program admits it, when there is one.
    pub(crate) legal_until_unix_seconds: Option<i64>,
    /// Where the supplied clock falls against that interval.
    pub(crate) verdict: RelayVerdictV1,
    /// Seconds from the supplied clock until `at_unix_seconds`, never negative.
    pub(crate) wait_seconds: i64,
}

impl ScheduledActionV1 {
    fn plan(at: i64, legal_from: i64, legal_until: Option<i64>, now: i64) -> Self {
        let verdict = if legal_until.is_some_and(|until| now > until) {
            RelayVerdictV1::Missed
        } else if now < legal_from {
            RelayVerdictV1::Waiting
        } else {
            RelayVerdictV1::Due
        };
        Self {
            at_unix_seconds: at,
            legal_from_unix_seconds: legal_from,
            legal_until_unix_seconds: legal_until,
            verdict,
            wait_seconds: at.saturating_sub(now).max(0),
        }
    }
}

/// The whole schedule, which is one document a runbook step can act on.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct RelaySchedulePlanV1 {
    /// The window this plan is about, restated so a reader need not re-derive it.
    pub(crate) window: RelayWindowV1,
    /// `end + max_age`, the second that separates the two actions.
    pub(crate) primary_deadline_unix_seconds: i64,
    /// The chain clock this plan was computed against.
    pub(crate) observed_unix_seconds: i64,
    /// Capture, which must run while the pinned account holds an in-window push.
    pub(crate) capture: ScheduledActionV1,
    /// Settle, legal only strictly after the primary deadline.
    pub(crate) settle: ScheduledActionV1,
}

/// Plan both actions against one observed clock.
///
/// The capture's legal interval is stated as `[start, end]` rather than as the
/// program's own `clock <= end + max_age`, and the difference is the whole
/// point. A capture is LEGAL for two more hours after the window closes; what
/// it cannot do is find an in-window `publish_time` on a mutable account that
/// has moved on. Reporting the wider interval would have called cohort-13's
/// 18:00 UTC arrival `Due` when the observation it needed was already gone. So
/// this plan reports the interval in which the ACTION CAN SUCCEED, and names
/// the wider one it is a subset of.
pub(crate) fn plan_relay_schedule_v1(
    window: RelayWindowV1,
    capture_margin_seconds: i64,
    settle_margin_seconds: i64,
    now_unix_seconds: i64,
) -> Result<RelaySchedulePlanV1> {
    if capture_margin_seconds < 0 || settle_margin_seconds < 0 {
        return Err(Error::new("relay margins must not be negative"));
    }
    let deadline = window.primary_deadline_unix_seconds()?;
    let capture_at = window
        .start_unix_seconds
        .checked_add(capture_margin_seconds)
        .ok_or_else(|| Error::new("window start + capture margin overflows"))?;
    if capture_at > window.end_unix_seconds {
        return Err(Error::new(format!(
            "capture margin {capture_margin_seconds}s puts the first attempt at {capture_at}, \
             after the window closes at {}; nothing inside the window would ever be tried",
            window.end_unix_seconds
        )));
    }
    let settle_at = deadline
        .checked_add(settle_margin_seconds.max(1))
        .ok_or_else(|| Error::new("primary deadline + settle margin overflows"))?;
    Ok(RelaySchedulePlanV1 {
        window,
        primary_deadline_unix_seconds: deadline,
        observed_unix_seconds: now_unix_seconds,
        capture: ScheduledActionV1::plan(
            capture_at,
            window.start_unix_seconds,
            Some(window.end_unix_seconds),
            now_unix_seconds,
        ),
        // Strictly after: the program's conjunct is `clock <= deadline` refuses.
        settle: ScheduledActionV1::plan(settle_at, deadline + 1, None, now_unix_seconds),
    })
}

/// Sleep once until a unix second the CHAIN agrees has not yet arrived.
///
/// Bounded twice over: it refuses a target further away than `ceiling_seconds`
/// rather than sleeping for it, and it sleeps at most twice -- once for the
/// computed remainder and once for whatever a re-read says is left. A loop that
/// polls until a condition holds cannot say what it will cost; this can.
pub(crate) fn wait_until_unix_seconds_v1(
    rpc: &mut Rpc,
    target_unix_seconds: i64,
    ceiling_seconds: i64,
) -> Result<i64> {
    for _ in 0..2 {
        let slot = rpc.finalized_slot()?;
        let now = rpc.block_time(slot)?;
        let remaining = target_unix_seconds.saturating_sub(now);
        if remaining <= 0 {
            return Ok(now);
        }
        if remaining > ceiling_seconds {
            return Err(Error::new(format!(
                "the chain clock reads {now} and the target is {target_unix_seconds}: \
                 {remaining}s away, past the stated ceiling of {ceiling_seconds}s. \
                 A wait this long is a scheduling decision, not a retry"
            )));
        }
        std::thread::sleep(Duration::from_secs(
            u64::try_from(remaining).map_err(|_| Error::new("negative wait"))?,
        ));
    }
    let slot = rpc.finalized_slot()?;
    rpc.block_time(slot)
}

/// Arguments for [`run`].
#[derive(Debug)]
struct ArgumentsV1 {
    rpc_url: Option<String>,
    devnet_acknowledgment: Option<String>,
    window_record: Option<String>,
    output: Option<PathBuf>,
    capture_margin_seconds: i64,
    settle_margin_seconds: i64,
    replay_window: Option<RelayWindowV1>,
    replay_now: Option<i64>,
    wait_for: Option<ScheduledStepV1>,
    max_wait_seconds: i64,
}

/// Which scheduled step `--wait` blocks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScheduledStepV1 {
    Capture,
    Settle,
}

impl ScheduledStepV1 {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "capture" => Ok(Self::Capture),
            "settle" => Ok(Self::Settle),
            other => Err(Error::new(format!(
                "--wait takes capture or settle, not {other}: those are the two steps this \
                 schedule times, and they are hours apart"
            ))),
        }
    }
}

/// Whether `--wait` may proceed, from the two facts that decide it.
///
/// `replay` is the CALLER's argument shape; `connected` is whether this command
/// kept the endpoint it read the window through. Both refusals are legitimate
/// and they are not the same accusation, so they do not share a sentence.
fn wait_admission_v1(replay: bool, connected: bool) -> Result<()> {
    if replay {
        return Err(Error::new(
            "--wait needs a live endpoint: a replay's clock is an argument, not a chain",
        ));
    }
    if !connected {
        return Err(Error::new(
            "--wait was asked for on a live schedule and no endpoint was kept: that is a defect \
             in this command, not in the caller's arguments",
        ));
    }
    Ok(())
}

/// The ceiling a `--wait` uses when the caller states none.
///
/// One window width plus one primary deadline is the longest a legitimate wait
/// on this route can be; anything past it is a scheduling decision the caller
/// should make out loud.
const DEFAULT_MAX_WAIT_SECONDS_V1: i64 = 3 * 3_600;

/// Usage line.
pub(crate) fn usage() -> String {
    format!(
        "\n  dclutch-local-successor-bootstrap {COMMAND_V1} \
         --rpc-url URL --i-mean-devnet DEVNET_GENESIS --window-record PUBKEY \
         [--output ABSOLUTE_NEW_JSON] [--capture-margin-seconds N] [--settle-margin-seconds N] \
         [--wait capture|settle] [--max-wait-seconds N]\
         \n  dclutch-local-successor-bootstrap {COMMAND_V1} \
         --replay-window START,END,MAX_AGE,TOLERANCE --replay-now UNIX_SECONDS \
         [--capture-margin-seconds N] [--settle-margin-seconds N]\n\n\
         Plans the sponsored relay against the founded market's own DCLTWIN1 window record, \
         which `devnet-sponsored-push-input-v1` names as `accounts.window.raw`. Capture must run \
         while the pinned price account holds a publish_time inside [start, end]; settle is legal \
         only STRICTLY AFTER end + max_age, so the two are hours apart and take separate input \
         documents (terminalSequence 0 and 1). The replay form needs no endpoint and no keys: it \
         answers what this schedule WOULD have done against a window that already closed, which \
         is how a schedule is checked against a market it cannot change.\n"
    )
}

/// Run the schedule planner.
pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let parsed = parse_arguments(arguments)?;
    let (window, now, connected) = match (parsed.replay_window, parsed.replay_now) {
        (Some(window), Some(now)) => (window, now, None),
        (Some(_), None) | (None, Some(_)) => {
            return Err(Error::new(
                "--replay-window and --replay-now are supplied together or not at all: a replay \
                 with no clock has nothing to be a replay of",
            ));
        }
        (None, None) => {
            let rpc_url = parsed
                .rpc_url
                .ok_or_else(|| Error::new("--rpc-url is required"))?;
            let origin = crate::cluster::ClusterOriginV1::parse(
                &rpc_url,
                parsed.devnet_acknowledgment.as_deref(),
            )?;
            // Both halves, the way every other devnet arm takes them: the
            // DOCUMENT's acknowledgment and the ENDPOINT are checked against the
            // same value, so a loopback endpoint with a devnet genesis refuses.
            crate::cluster::ExpectedClusterV1::Devnet.authenticate(&origin)?;
            let record = crate::plan::pubkey(
                &parsed
                    .window_record
                    .ok_or_else(|| Error::new("--window-record is required"))?,
            )?;
            // Read-only: this command plans and never signs.
            let mut rpc = Rpc::connect_cluster(&origin, crate::rpc::WritePolicyV1::ReadsOnly)?;
            let floor = rpc.finalized_slot()?;
            // ONE OBSERVATION, and the clock is read at the slot that answered
            // it. A window read at one slot and a clock read at another is two
            // pictures, and this whole command is about which picture a
            // scheduled action will be judged against.
            let (observed_slot, accounts) = rpc.finalized_accounts(&[record], floor)?;
            let bytes = accounts
                .first()
                .and_then(|entry| entry.as_ref())
                .ok_or_else(|| {
                    Error::new(format!("window spec record {record} is vacant on chain"))
                })?
                .data
                .clone();
            let window = RelayWindowV1::decode(&bytes)?;
            let now = rpc.block_time(observed_slot)?;
            // THE CONNECTION LEAVES THIS ARM IN THE TUPLE, and that is why it
            // cannot be forgotten again. It used to be a `let mut connected =
            // None` declared above the match which this arm never assigned, so
            // `rpc` was dropped here and EVERY live `--wait` refused
            // "a replay's clock is an argument" while holding a live endpoint.
            // Measured 2026-09-03 by the RELAY-C lane against devnet. A local
            // an arm may silently decline to write is not a contract; a tuple
            // slot the arm must produce is.
            (window, now, Some(rpc))
        }
    };
    let plan = plan_relay_schedule_v1(
        window,
        parsed.capture_margin_seconds,
        parsed.settle_margin_seconds,
        now,
    )?;
    let document = serde_json::json!({
        "format": SCHEDULE_FORMAT_V1,
        "plan": plan,
    });
    let rendered = serde_json::to_vec_pretty(&document)
        .map_err(|error| Error::new(format!("render schedule: {error}")))?;
    if let Some(path) = parsed.output.as_ref() {
        crate::release_capture::write_json_atomic_new(path, &document)?;
    }
    println!(
        "{}",
        String::from_utf8(rendered).map_err(|error| Error::new(format!("schedule: {error}")))?
    );
    let Some(step) = parsed.wait_for else {
        return Ok(());
    };
    // A REPLAY NEVER WAITS. Its clock is a number the caller supplied, so
    // sleeping against it would be sleeping against nothing.
    // ONE REFUSAL OVER TWO CAUSES IS WHAT HID THE DEFECT. A replay that asks to
    // wait is the CALLER's mistake; a live schedule reaching here with no
    // connection is THIS COMMAND's. They shared a sentence, and the cause that
    // actually happened was the one the sentence did not describe.
    wait_admission_v1(parsed.replay_window.is_some(), connected.is_some())?;
    let mut rpc = connected.expect("wait_admission_v1 admits only a kept connection");
    let (label, action) = match step {
        ScheduledStepV1::Capture => ("capture", plan.capture),
        ScheduledStepV1::Settle => ("settle", plan.settle),
    };
    if action.verdict == RelayVerdictV1::Missed {
        return Err(Error::new(format!(
            "the {label} step closed at {}; waiting for it would wait forever. This is \
             cohort-13's outcome exactly, and the honest next act is the failure walk",
            action
                .legal_until_unix_seconds
                .unwrap_or(action.legal_from_unix_seconds)
        )));
    }
    let reached =
        wait_until_unix_seconds_v1(&mut rpc, action.at_unix_seconds, parsed.max_wait_seconds)?;
    eprintln!(
        "{COMMAND_V1}: the {label} step is due; the chain clock reads {reached} against a \
         planned {}",
        action.at_unix_seconds
    );
    Ok(())
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut parsed = ArgumentsV1 {
        rpc_url: None,
        devnet_acknowledgment: None,
        window_record: None,
        output: None,
        capture_margin_seconds: DEFAULT_CAPTURE_MARGIN_SECONDS_V1,
        settle_margin_seconds: DEFAULT_SETTLE_MARGIN_SECONDS_V1,
        replay_window: None,
        replay_now: None,
        wait_for: None,
        max_wait_seconds: DEFAULT_MAX_WAIT_SECONDS_V1,
    };
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        match argument.as_str() {
            "--rpc-url" => parsed.rpc_url = Some(value),
            crate::cluster::DEVNET_ACKNOWLEDGMENT_FLAG => {
                parsed.devnet_acknowledgment = Some(value);
            }
            "--window-record" => parsed.window_record = Some(value),
            "--output" => parsed.output = Some(absolute_new(&value)?),
            "--capture-margin-seconds" => {
                parsed.capture_margin_seconds = parse_seconds(&argument, &value)?;
            }
            "--settle-margin-seconds" => {
                parsed.settle_margin_seconds = parse_seconds(&argument, &value)?;
            }
            "--replay-window" => parsed.replay_window = Some(parse_replay_window(&value)?),
            "--replay-now" => parsed.replay_now = Some(parse_seconds(&argument, &value)?),
            "--wait" => parsed.wait_for = Some(ScheduledStepV1::parse(&value)?),
            "--max-wait-seconds" => {
                parsed.max_wait_seconds = parse_seconds(&argument, &value)?;
            }
            other => {
                return Err(Error::new(format!(
                    "unknown {COMMAND_V1} argument: {other}"
                )));
            }
        }
    }
    Ok(parsed)
}

fn absolute_new(value: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(Error::new("--output must be absolute"));
    }
    Ok(path)
}

fn parse_seconds(label: &str, value: &str) -> Result<i64> {
    value
        .parse::<i64>()
        .map_err(|_| Error::new(format!("{label} must be a decimal number of seconds")))
}

/// Parse `START,END,MAX_AGE,TOLERANCE` for the replay form.
fn parse_replay_window(value: &str) -> Result<RelayWindowV1> {
    let fields: Vec<&str> = value.split(',').collect();
    let [start, end, max_age, tolerance] = fields.as_slice() else {
        return Err(Error::new(
            "--replay-window takes exactly START,END,MAX_AGE,TOLERANCE, the four \
             DCLTWIN1 fields a schedule reads",
        ));
    };
    let window = RelayWindowV1 {
        start_unix_seconds: parse_seconds("--replay-window start", start)?,
        end_unix_seconds: parse_seconds("--replay-window end", end)?,
        max_age_seconds: max_age
            .parse::<u32>()
            .map_err(|_| Error::new("--replay-window max_age must be a decimal u32"))?,
        cadence_tolerance_seconds: tolerance
            .parse::<u32>()
            .map_err(|_| Error::new("--replay-window tolerance must be a decimal u32"))?,
    };
    if window.end_unix_seconds <= window.start_unix_seconds {
        return Err(Error::new(
            "--replay-window end must be after start; an empty window admits no observation",
        ));
    }
    Ok(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// COHORT-13'S OWN WINDOW, read off `4CM5e6Eq7nXcAnYbEnhP9Jqfkp6BrsYdokks5dLwQCEu`.
    ///
    /// 112 bytes, `DCLTWIN1`, Registry-owned, kind 1 = Terminal. These four
    /// numbers are the fixture for everything below, so the schedule is checked
    /// against a window that really existed and really closed unobserved rather
    /// than against one invented to make it pass.
    const COHORT_13_WINDOW: RelayWindowV1 = RelayWindowV1 {
        start_unix_seconds: 1_788_369_759,
        end_unix_seconds: 1_788_371_559,
        max_age_seconds: 7_200,
        cadence_tolerance_seconds: 0,
    };

    /// 18:00:00 UTC on 2026-09-02, when the resolution lane opened.
    const COHORT_13_LANE_OPENED: i64 = 1_788_372_000;

    #[test]
    fn the_window_is_exactly_the_one_cohort_13_founded() {
        assert_eq!(
            COHORT_13_WINDOW.end_unix_seconds - COHORT_13_WINDOW.start_unix_seconds,
            1_800,
            "the vetted default width",
        );
        assert_eq!(
            COHORT_13_WINDOW
                .primary_deadline_unix_seconds()
                .expect("deadline"),
            1_788_378_759,
            "19:52:39 UTC -- the last capture second and the first settle one",
        );
    }

    /// THE DRY RUN THE BRIEF ASKS FOR: it would have fired at 13:23 EDT.
    ///
    /// EDT is UTC-4, so 17:22:39 UTC is 13:22:39 EDT and the window's first
    /// minute is the 13:22 one. With the default margin the first capture
    /// attempt is 17:23:39 UTC = 13:23:39 EDT -- inside the window by
    /// 1,740 seconds, and inside the 13:23 minute.
    #[test]
    fn against_cohort_13s_window_the_capture_would_have_fired_at_1323_edt() {
        let plan = plan_relay_schedule_v1(
            COHORT_13_WINDOW,
            DEFAULT_CAPTURE_MARGIN_SECONDS_V1,
            DEFAULT_SETTLE_MARGIN_SECONDS_V1,
            // Staged from founding time, which is when a runbook step arms it.
            COHORT_13_WINDOW.start_unix_seconds - 21_600,
        )
        .expect("cohort-13's window plans");

        assert_eq!(plan.capture.at_unix_seconds, 1_788_369_819);
        assert_eq!(plan.capture.verdict, RelayVerdictV1::Waiting);
        assert_eq!(plan.capture.wait_seconds, 21_660);
        // Inside the window, and by how much, because "inside" with no margin
        // left is not a schedule anybody should ship.
        assert!(plan.capture.at_unix_seconds >= COHORT_13_WINDOW.start_unix_seconds);
        assert_eq!(
            COHORT_13_WINDOW.end_unix_seconds - plan.capture.at_unix_seconds,
            1_740,
            "twenty-nine minutes of window left after the first attempt",
        );

        // Settle is NOT the same event two seconds later. It is two hours after
        // the window closed, and a schedule that ran them together would refuse
        // ProviderFreshness on the second one.
        assert_eq!(plan.settle.at_unix_seconds, 1_788_378_789);
        assert_eq!(plan.settle.legal_from_unix_seconds, 1_788_378_760);
        assert_eq!(
            plan.settle.at_unix_seconds - plan.capture.at_unix_seconds,
            8_970,
            "two hours and twenty-nine minutes apart",
        );
        assert_eq!(plan.settle.verdict, RelayVerdictV1::Waiting);
    }

    /// THE VERDICT COHORT-13 ACTUALLY GOT, computed rather than remembered.
    ///
    /// The lane opened at 18:00 UTC, nine minutes after the window closed. The
    /// schedule says `Missed` for the capture -- and says `Waiting` for the
    /// settle, which is the sharp part: settle was still nearly two hours away
    /// and would have been legal, but with no candidate to settle. That is
    /// exactly the state that left the failure walk as the only reachable
    /// terminal.
    #[test]
    fn at_the_moment_cohort_13_looked_the_capture_was_already_missed() {
        let plan = plan_relay_schedule_v1(
            COHORT_13_WINDOW,
            DEFAULT_CAPTURE_MARGIN_SECONDS_V1,
            DEFAULT_SETTLE_MARGIN_SECONDS_V1,
            COHORT_13_LANE_OPENED,
        )
        .expect("plan");
        assert_eq!(plan.capture.verdict, RelayVerdictV1::Missed);
        assert_eq!(plan.settle.verdict, RelayVerdictV1::Waiting);
        assert_eq!(
            COHORT_13_LANE_OPENED - COHORT_13_WINDOW.end_unix_seconds,
            441,
            "seven minutes and twenty-one seconds after the window closed",
        );
    }

    /// THE LIVE FORM IS WAITABLE, which is the fact the shipped command denied.
    ///
    /// Measured 2026-09-03 by the RELAY-C lane: `--wait capture` against a live
    /// devnet endpoint refused *"a replay's clock is an argument, not a chain"*
    /// while holding that endpoint, because `connected` was initialised to
    /// `None` above the match and the live arm never assigned it. The arm now
    /// yields the connection in the tuple, so the compiler carries that half;
    /// what a test can carry is that the two causes are SEPARATE accusations,
    /// since sharing one sentence is what let the command blame the caller for
    /// a mistake the command had made.
    #[test]
    fn a_live_schedule_may_wait_and_the_two_refusals_are_not_one_sentence() {
        wait_admission_v1(false, true).expect("a live schedule that kept its endpoint may wait");

        let replay = wait_admission_v1(true, false).expect_err("a replay has no chain to wait on");
        assert!(
            replay
                .to_string()
                .contains("a replay's clock is an argument"),
            "the replay refusal names the caller's argument shape: {replay}",
        );

        let defect =
            wait_admission_v1(false, false).expect_err("a live schedule that kept no endpoint");
        assert!(
            defect.to_string().contains("defect in this command"),
            "a dropped connection is this command's fault, not a replay: {defect}",
        );
        assert_ne!(replay.to_string(), defect.to_string());
    }

    /// The three verdicts partition the timeline, checked at every boundary.
    #[test]
    fn the_capture_verdict_changes_exactly_at_the_windows_own_edges() {
        for (now, expected) in [
            (
                COHORT_13_WINDOW.start_unix_seconds - 1,
                RelayVerdictV1::Waiting,
            ),
            (COHORT_13_WINDOW.start_unix_seconds, RelayVerdictV1::Due),
            (COHORT_13_WINDOW.end_unix_seconds, RelayVerdictV1::Due),
            (
                COHORT_13_WINDOW.end_unix_seconds + 1,
                RelayVerdictV1::Missed,
            ),
        ] {
            let plan = plan_relay_schedule_v1(COHORT_13_WINDOW, 60, 30, now).expect("plan");
            assert_eq!(plan.capture.verdict, expected, "at {now}");
        }
        // Settle's first legal second is the one AFTER the deadline, because
        // the program refuses `clock <= deadline`.
        let deadline = COHORT_13_WINDOW
            .primary_deadline_unix_seconds()
            .expect("deadline");
        for (now, expected) in [
            (deadline, RelayVerdictV1::Waiting),
            (deadline + 1, RelayVerdictV1::Due),
        ] {
            let plan = plan_relay_schedule_v1(COHORT_13_WINDOW, 60, 30, now).expect("plan");
            assert_eq!(plan.settle.verdict, expected, "at {now}");
        }
    }

    /// A margin that would put the first attempt outside the window refuses.
    ///
    /// Silently clamping it to the window's end would be worse than refusing:
    /// the caller stated a schedule and would get a different one.
    #[test]
    fn a_margin_wider_than_the_window_refuses_rather_than_clamping() {
        let refusal = plan_relay_schedule_v1(COHORT_13_WINDOW, 1_801, 30, 0)
            .expect_err("a margin past the window must refuse");
        assert!(
            refusal.to_string().contains("after the window closes"),
            "{refusal}"
        );
        // The exact width still plans: the last second of the window is inside it.
        assert!(plan_relay_schedule_v1(COHORT_13_WINDOW, 1_800, 30, 0).is_ok());
    }

    /// A settle margin of zero is lifted to one, because zero is not legal.
    #[test]
    fn a_zero_settle_margin_still_lands_after_the_deadline() {
        let plan = plan_relay_schedule_v1(COHORT_13_WINDOW, 60, 0, 0).expect("plan");
        let deadline = COHORT_13_WINDOW
            .primary_deadline_unix_seconds()
            .expect("deadline");
        assert!(
            plan.settle.at_unix_seconds > deadline,
            "settle refuses `clock <= deadline`, so equality is not a schedule",
        );
        assert_eq!(plan.settle.at_unix_seconds, deadline + 1);
    }

    #[test]
    fn the_replay_form_parses_the_four_dcltwin1_fields_and_refuses_the_rest() {
        assert_eq!(
            parse_replay_window("1788369759,1788371559,7200,0").expect("cohort-13's window"),
            COHORT_13_WINDOW,
        );
        for hostile in [
            "1788369759,1788371559,7200",
            "1788369759,1788371559,7200,0,1",
            "1788371559,1788369759,7200,0",
            "not-a-number,1788371559,7200,0",
        ] {
            assert!(
                parse_replay_window(hostile).is_err(),
                "{hostile} must refuse"
            );
        }
    }

    /// A REPLAY MAY NOT WAIT, and a missed step may not be waited for.
    ///
    /// Both refusals exist because the alternative is a command that blocks
    /// forever and reports nothing -- the same shape as a preflight that plans
    /// an action the chain will certainly refuse.
    #[test]
    fn waiting_refuses_a_replay_clock_and_a_step_that_already_closed() {
        let refusal = run(vec![
            "--replay-window".into(),
            "1788369759,1788371559,7200,0".into(),
            "--replay-now".into(),
            "1788370000".into(),
            "--wait".into(),
            "capture".into(),
        ])
        .expect_err("a replay has no chain to wait on");
        assert!(
            refusal.to_string().contains("--wait needs a live endpoint"),
            "{refusal}"
        );
        assert!(ScheduledStepV1::parse("commit-failure").is_err());
        assert_eq!(
            ScheduledStepV1::parse("capture").expect("capture"),
            ScheduledStepV1::Capture
        );
    }

    #[test]
    fn an_unknown_flag_and_a_half_supplied_replay_both_refuse() {
        let refusal = parse_arguments(vec!["--capture-margin".into(), "60".into()])
            .expect_err("unknown flag");
        assert!(refusal.to_string().contains("unknown"), "{refusal}");
        let refusal = run(vec!["--replay-now".into(), "1788370000".into()])
            .expect_err("a replay clock with no window");
        assert!(
            refusal
                .to_string()
                .contains("supplied together or not at all"),
            "{refusal}"
        );
    }
}
