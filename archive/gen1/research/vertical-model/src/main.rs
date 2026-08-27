//! Print one deterministic reference trace.
//!
//! With no argument this prints the scalar-lab trace pinned by
//! `golden/basic.trace`, which is a permanent regression and is never
//! rewritten.  `coupled` prints the coupled-relation trace pinned by
//! `golden/coupled.trace`.

use clutch_vertical_model::{
    coupled_golden_scenario, golden_scenario, ResidualSettlementV1, VerticalModel,
};

fn main() {
    let selection = std::env::args().nth(1);
    let model: VerticalModel = match selection.as_deref() {
        None | Some("basic") => {
            golden_scenario().expect("the deterministic reference scenario must remain valid")
        }
        // The residual-pair variant is named, never defaulted.
        Some("coupled") => coupled_golden_scenario(ResidualSettlementV1::UniqueSliceReceipts)
            .expect("the deterministic coupled scenario must remain valid"),
        Some(other) => {
            eprintln!("unknown trace {other:?}; expected `basic` or `coupled`");
            std::process::exit(2);
        }
    };
    for event in model.trace {
        println!("{event}");
    }
}
