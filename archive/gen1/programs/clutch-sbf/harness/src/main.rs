//! Thin command line over the differential bring-up harness library.
//!
//! Every fixture, address derivation, transaction builder, and plan emitter
//! lives in [`clutch_sbf_harness`].  This binary forwards the process
//! arguments and nothing else, so the plan a script writes and the plan the
//! Operator Bench daemon (`operatord`) drives a browser against come from one
//! implementation rather than two that must be kept in agreement by hand.

fn main() {
    clutch_sbf_harness::run_cli();
}
