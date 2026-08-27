#![forbid(unsafe_code)]

mod batch;
mod digest;
mod kernel;
mod layout;

use digest::Transcript;

const SEEDS: [u64; 16] = [
    0x0000_0000_0000_0001,
    0x0000_0000_0000_0002,
    0x0000_0000_0000_0003,
    0x0000_0000_0000_0005,
    0x0000_0000_0000_0008,
    0x0000_0000_0000_000d,
    0x0000_0000_0000_0015,
    0x0000_0000_0000_0022,
    0x0123_4567_89ab_cdef,
    0x0ddc_0ffe_e15e_beef,
    0x3141_5926_5358_9793,
    0x5eed_5eed_5eed_5eed,
    0x8000_0000_0000_0001,
    0xa5a5_5a5a_f0f0_0f0f,
    0xdead_beef_cafe_babe,
    0xffff_ffff_ffff_ffff,
];

#[derive(Clone, Copy, Debug, Default)]
struct Counts {
    cases: u64,
    accepted: u64,
    refused: u64,
}

impl Counts {
    fn add(&mut self, other: Self) {
        self.cases += other.cases;
        self.accepted += other.accepted;
        self.refused += other.refused;
    }
}

fn main() {
    let mut transcript = Transcript::new(0x4443_494e_565f_3031);
    let mut total = Counts::default();

    let kernel = kernel::run(&SEEDS, &mut transcript);
    print_lane("kernel", kernel, transcript.finish());
    total.add(kernel);

    let layout = layout::run(&SEEDS, &mut transcript);
    print_lane("layout", layout, transcript.finish());
    total.add(layout);

    let batch = batch::run(&SEEDS, &mut transcript);
    print_lane("batch", batch, transcript.finish());
    total.add(batch);

    println!(
        "campaign=all seeds={} cases={} accepted={} refused={} digest={:032x}",
        SEEDS.len(),
        total.cases,
        total.accepted,
        total.refused,
        transcript.finish()
    );
}

fn print_lane(name: &str, counts: Counts, digest: u128) {
    println!(
        "campaign={name} cases={} accepted={} refused={} cumulative_digest={digest:032x}",
        counts.cases, counts.accepted, counts.refused
    );
}
