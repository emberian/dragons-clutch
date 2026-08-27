//! Emit the browser's DCLTGMF1 golden vectors from the first-party Rust encoders.
//!
//! `apps/dclutch-web/lib/founding/genericFoundingRequest.ts` is a second
//! implementation of `GenericFoundingRequestV1`'s wire, and a second
//! implementation without an oracle is a guess. This binary runs the FIRST
//! implementation -- `dclutch-market-core-codec`, the crate the on-chain
//! `dclutch-trading-sbf` route itself decodes with -- over fully determined
//! inputs and prints the exact bytes it produces.
//!
//! Why this exists at all, given `a5e16cd6` ("banish: the browser's own DCLTCAT1
//! fixture authority") retired the last such pipeline: that one encoded
//! `CategoricalMarketV1`, a Market representation nothing writes. Its commit
//! message called it "a well-built pipeline pointed at" a dead stratum and was
//! explicit about what its removal cost -- "nothing re-derives the record vector
//! now ... a real gate". `GenericFoundingRequestV1` is the opposite case: it is
//! the live founding wire, decoded on chain by
//! `programs/dclutch-trading-sbf/src/generic_market_founding_v1.rs` on every
//! DCLTGMF1 execution. The distinction that commit drew is exactly the one that
//! admits this generator.
//!
//! Run:
//!
//! ```sh
//! cd apps/dclutch-web/fixtures/founding/generator
//! cargo run --locked --quiet > ../generic-founding-vectors.json
//! ```
//!
//! The output is checked in. `lib/founding/genericFoundingRequest.test.ts`
//! re-encodes the same named inputs in TypeScript and byte-compares.

use dclutch_market_core_codec::{
    GENERIC_FOUNDING_ACK_BYTES_V1, GENERIC_FOUNDING_REQUEST_BYTES_V1, GenericFoundingAckV1,
    GenericFoundingRequestV1, GenericFoundingStageV1, Identity,
    generic_founding_funding_list_id_v1,
};

fn id(byte: u8) -> Identity {
    Identity::new([byte; 32]).expect("identity")
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// The exact shape `generic_founding_v1.rs`'s own `mod tests::request` builds.
///
/// Reproducing the upstream test's literals rather than inventing new ones
/// means a change to those literals is visible here as a diff, and means the
/// vector is anchored to something the crate's own suite already exercises.
fn canonical(stage: GenericFoundingStageV1) -> GenericFoundingRequestV1 {
    GenericFoundingRequestV1::new(
        stage,
        3,
        id(1),
        id(2),
        id(3),
        id(4),
        id(5),
        id(6),
        id(7),
        id(8),
        id(9),
        id(10),
        11,
        12,
        13,
        14,
        15,
        16,
        2,
        5,
    )
    .expect("canonical request")
}

/// A second vector at the admitted extremes, so the port is not tested only in
/// the middle of every field's range.
fn extremal() -> GenericFoundingRequestV1 {
    GenericFoundingRequestV1::new(
        GenericFoundingStageV1::Open,
        16,
        id(0xa1),
        id(0xa2),
        id(0xa3),
        id(0xa4),
        id(0xa5),
        id(0xa6),
        id(0xa7),
        id(0xa8),
        id(0xa9),
        id(0xaa),
        u64::MAX,
        1,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u16::MAX,
    )
    .expect("extremal request")
}

fn request_json(name: &str, request: GenericFoundingRequestV1) -> String {
    let bytes: [u8; GENERIC_FOUNDING_REQUEST_BYTES_V1] = request.encode().expect("encode request");
    format!("    {{ \"name\": \"{name}\", \"bytes\": \"{}\" }}", hex(&bytes))
}

fn main() {
    let found = canonical(GenericFoundingStageV1::FoundAndPermit);
    let open = found
        .with_stage(GenericFoundingStageV1::Open)
        .expect("open stage");
    let ack = GenericFoundingAckV1::new(found, id(20), id(21), id(22), id(23));
    let ack_bytes: [u8; GENERIC_FOUNDING_ACK_BYTES_V1] = ack.encode().expect("encode ack");

    let one = [id(0x31)];
    let three = [id(0x31), id(0x32), id(0x33)];
    let sixteen: Vec<Identity> = (0..16_u8).map(|index| id(0x40 + index)).collect();

    println!("{{");
    println!("  \"schema\": \"dclutch-web-generic-founding-vectors-v1\",");
    println!(
        "  \"provenance\": \"emitted by apps/dclutch-web/fixtures/founding/generator over crates/dclutch-market-core-codec; regenerate with `cargo run --locked --quiet` in that directory\","
    );
    println!("  \"requests\": [");
    println!("{},", request_json("canonical-found-and-permit", found));
    println!("{},", request_json("canonical-open", open));
    println!("{}", request_json("extremal-open", extremal()));
    println!("  ],");
    println!("  \"acks\": [");
    println!(
        "    {{ \"name\": \"canonical-found-and-permit\", \"bytes\": \"{}\" }}",
        hex(&ack_bytes)
    );
    println!("  ],");
    println!("  \"fundingListIds\": [");
    for (index, (name, keys)) in [
        ("one", one.as_slice()),
        ("three", three.as_slice()),
        ("sixteen", sixteen.as_slice()),
    ]
    .into_iter()
    .enumerate()
    {
        let digest = generic_founding_funding_list_id_v1(keys).expect("funding list id");
        let members: Vec<String> = keys
            .iter()
            .map(|key| format!("\"{}\"", hex(&key.to_bytes())))
            .collect();
        let comma = if index == 2 { "" } else { "," };
        println!(
            "    {{ \"name\": \"{name}\", \"members\": [{}], \"id\": \"{}\" }}{comma}",
            members.join(", "),
            hex(&digest.to_bytes())
        );
    }
    println!("  ]");
    println!("}}");
}
