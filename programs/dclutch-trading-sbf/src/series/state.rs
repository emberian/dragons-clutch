//! Series V3 replay evaluator imported from the safe, SDK-free kernel.
//!
//! This adapter defines no parallel mutable-state semantics. Generic Trading
//! IR owns physical writes; the Series account module uses these types only as
//! a differential execution oracle.

pub use dclutch_series_v3_kernel::replay::{
    SERIES_STATE_BYTES_V3, SERIES_TICKET_STATE_BYTES_V3, SERIES_TICKET_STATE_PDA_DOMAIN_V3,
    SeriesPhaseV3, SeriesStateError, SeriesStateV3, TicketPhaseV3, TicketStateSeedsV3,
    TicketStateV3,
};
