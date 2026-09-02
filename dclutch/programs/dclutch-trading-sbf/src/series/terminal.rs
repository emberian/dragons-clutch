//! Exact account-suffix widths for the two terminal Series V3 actions.
//!
//! `occurrence_artifacts_v4` reads these to size the Retire and Close frames.
//! They are the whole of this module: the executable "terminal differential
//! oracle" that used to live here -- `process_retire_v3` and `process_close_v3`
//! plus their private frame helpers -- was removed once a census showed it had
//! never acquired a caller. Its own header had said "common dispatch must not
//! select this module by a Series tag", so it was never a route; it was
//! declared to be differential evidence for the generic
//! Account/Request/Transition/Effect interpreter, and nothing ever compared the
//! two. Unexercised evidence is not evidence, and a second writer of Ticket
//! retirement that no test drives is a hazard rather than a control.

/// Exact account suffix for one terminal Ticket retirement.
pub const SERIES_RETIRE_ACCOUNT_COUNT_V3: usize = 4;
/// Exact account suffix for terminal Series-root closure.
pub const SERIES_CLOSE_ACCOUNT_COUNT_V3: usize = 3;
