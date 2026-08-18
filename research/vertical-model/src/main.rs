fn main() {
    let model = clutch_vertical_model::golden_scenario()
        .expect("the deterministic reference scenario must remain valid");
    for event in model.trace {
        println!("{event}");
    }
}
