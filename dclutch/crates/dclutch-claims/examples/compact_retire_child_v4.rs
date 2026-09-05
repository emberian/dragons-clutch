//! Emit one canonical compact RetireReceipt child, from the contract that owns it.
//!
//! `packages/dclutch-sdk/lib/rationalRetireReceiptV4.ts` builds the `DCRLHC04`
//! family request a wallet signs and the `DCRRLC02` Claims child whose SHA-256
//! the Hot route binds. It had no authority for either. Its digest assertion
//! was a REGRESSION PIN computed by the encoder it was checking, and it said so
//! in its own comment: move `LIFECYCLE_ROW_CUSTODY_OWNER_OFFSET` in the client
//! and every assertion around it stays green while the wallet signs a different
//! child. That is the exact shape of the defect `e78fa027d` had already left in
//! the tree for six days -- the vacancy group went from four accounts to five
//! in Rust and the client stayed at four -- so the hole was not hypothetical.
//!
//! This example closes it from the owning side. Every byte below comes from the
//! contract's own encoders:
//!
//!   * `RationalLifecycleCompactHotRequestV4::from_header_into` for the family,
//!   * `specialize_child_header_into` for the child header, which is where the
//!     parent context becomes SHA-256 of the family and the coordinate count
//!     stops being zero,
//!   * `LifecycleCoordinateV2::encode_into` for each vacancy row, which is
//!     where the five accounts take their slots, and
//!   * `LifecycleRequestV2::encode_into` for the assembled child.
//!
//! Nothing here re-states an offset. Move one and the emitted digest moves,
//! which is the whole point: `scripts/generate-rational-retire-child-v4.mjs`
//! writes this output into the package's fixtures and the SDK test asserts its
//! own child against it.
//!
//! IT IS A FIXTURE, NOT A DEVNET OBSERVATION. These identities are chosen
//! constants, not addresses any chain derived; what it proves is that two
//! implementations of one layout agree, which is precisely what nothing in the
//! tree could say before.
//!
//! Run: `cargo run -p dclutch-claims
//!       --example compact_retire_child_v4`

use dclutch_claims::rational_lifecycle::{
    ABSENT_POSITION_REVISION_V2, LIFECYCLE_COORDINATE_BYTES_V2, LIFECYCLE_HEADER_BYTES_V2,
    LifecycleActionV2, LifecycleCoordinateV2, LifecycleHeaderV2, LifecycleRequestV2,
    compact_hot_v4::RationalLifecycleCompactHotRequestV4,
};
use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;
use sha2::{Digest, Sha256};

/// One 32-byte identity, distinct by its fill byte.
const fn id(fill: u8) -> [u8; 32] {
    [fill; 32]
}

/// The descriptor's ordered positive support: outcome, coefficient.
///
/// Three rows rather than one, so the emitted child exercises the row STRIDE
/// and the ordering as well as the five slots inside a row.
const SUPPORT: [(u32, u64); 3] = [(1, 7), (2, 5), (4, 9)];

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

/// The five vacancy accounts of one row, in the contract's own slot order.
///
/// Named as a group because the group is the thing that moved: the custody
/// OWNER sits between the Structured custody account and the Position, and a
/// client that omits it lays four accounts where the program reads five.
fn vacancy_row(index: usize, outcome: u32, coefficient: u64) -> LifecycleCoordinateV2 {
    let base = 0x40 + u8::try_from(index).expect("row index") * 5;
    LifecycleCoordinateV2 {
        outcome,
        coefficient,
        shard_mint: id(base),
        structured_custody_account: id(base + 1),
        claims_custody_owner: id(base + 2),
        claims_custody_position: id(base + 3),
        position_admission: id(base + 4),
        observed_shard_lamports: 0,
        observed_structured_lamports: 0,
        observed_position_lamports: 0,
        observed_admission_lamports: 0,
        shard_rent_principal: 0,
        structured_rent_principal: 0,
        position_rent_principal: 0,
        admission_rent_principal: 0,
        expected_shard_supply: 0,
        expected_structured_amount: 0,
        expected_position_revision: ABSENT_POSITION_REVISION_V2,
    }
}

fn main() {
    let observed_receipt_lamports = 10_u64;
    let rent_credit_before = 100_u64;
    let header = LifecycleHeaderV2 {
        action: LifecycleActionV2::RetireReceipt,
        release_set: id(0x0f),
        market: id(0x0e),
        graph_id: id(0x0b),
        descriptor_id: id(0x15),
        // Erased by the compact form; the child's is SHA-256 of the family.
        parent_context: id(0x01),
        representation_authority: id(0x16),
        receipt_mint: id(0x10),
        token_program: TOKEN_2022_PROGRAM_ID,
        rent_credit: id(0x18),
        rent_program: id(0x19),
        generation: 14,
        expected_claims_market_revision: 3,
        observed_receipt_lamports,
        receipt_rent_principal: 10,
        expected_receipt_supply: 0,
        outcome_count: 5,
        coordinate_count: 0,
        rent_credit_before,
        rent_credit_after: rent_credit_before + observed_receipt_lamports,
    };

    let mut family_bytes = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
    let family = RationalLifecycleCompactHotRequestV4::from_header_into(header, &mut family_bytes)
        .expect("compact family");
    let family_digest = sha256(family.as_bytes());

    let count = u32::try_from(SUPPORT.len()).expect("support width");
    let mut child_header = [0_u8; LIFECYCLE_HEADER_BYTES_V2];
    let specialized = family
        .specialize_child_header_into(family_digest, count, &mut child_header)
        .expect("specialized child header");

    let mut rows = vec![0_u8; SUPPORT.len() * LIFECYCLE_COORDINATE_BYTES_V2];
    for (index, (outcome, coefficient)) in SUPPORT.iter().copied().enumerate() {
        let start = index * LIFECYCLE_COORDINATE_BYTES_V2;
        let end = start + LIFECYCLE_COORDINATE_BYTES_V2;
        vacancy_row(index, outcome, coefficient)
            .encode_into(rows.get_mut(start..end).expect("row window"))
            .expect("row encoding");
    }

    let child = LifecycleRequestV2::new(specialized, &rows).expect("child request");
    let mut child_bytes = vec![0_u8; LIFECYCLE_HEADER_BYTES_V2 + rows.len()];
    child.encode_into(&mut child_bytes).expect("child encoding");
    // The header the contract projected and the header it re-encodes are the
    // same bytes, or this fixture would describe a child nobody builds.
    assert_eq!(
        child_bytes.get(..LIFECYCLE_HEADER_BYTES_V2),
        Some(&child_header[..]),
    );

    let rows_json: Vec<String> = SUPPORT
        .iter()
        .copied()
        .enumerate()
        .map(|(index, (outcome, coefficient))| {
            let row = vacancy_row(index, outcome, coefficient);
            format!(
                "    {{\n      \"outcome\": {outcome},\n      \"coefficient\": \"{coefficient}\",\n      \"shardMint\": \"{}\",\n      \"structuredCustody\": \"{}\",\n      \"owner\": \"{}\",\n      \"position\": \"{}\",\n      \"admission\": \"{}\"\n    }}",
                hex(&row.shard_mint),
                hex(&row.structured_custody_account),
                hex(&row.claims_custody_owner),
                hex(&row.claims_custody_position),
                hex(&row.position_admission),
            )
        })
        .collect();

    println!("{{");
    println!(
        "  \"note\": \"EMITTED BY RUST. crates/dclutch-claims/rational_lifecycle/examples/compact_retire_child_v4.rs, through the contract's own family, child-header, row and request encoders. Regenerate with `npm run abi:rational-retire-child` from packages/dclutch-sdk; never edit by hand. Fixture evidence, not devnet: the identities are chosen constants.\","
    );
    println!("  \"familyInput\": {{");
    println!("    \"releaseSet\": \"{}\",", hex(&header.release_set));
    println!("    \"market\": \"{}\",", hex(&header.market));
    println!("    \"graphId\": \"{}\",", hex(&header.graph_id));
    println!("    \"descriptorId\": \"{}\",", hex(&header.descriptor_id));
    println!(
        "    \"representationAuthority\": \"{}\",",
        hex(&header.representation_authority)
    );
    println!("    \"receiptMint\": \"{}\",", hex(&header.receipt_mint));
    println!("    \"tokenProgram\": \"{}\",", hex(&header.token_program));
    println!("    \"rentCredit\": \"{}\",", hex(&header.rent_credit));
    println!("    \"rentProgram\": \"{}\",", hex(&header.rent_program));
    println!("    \"generation\": \"{}\",", header.generation);
    println!(
        "    \"claimsRevision\": \"{}\",",
        header.expected_claims_market_revision
    );
    println!(
        "    \"receiptLamports\": \"{}\",",
        header.observed_receipt_lamports
    );
    println!(
        "    \"receiptRent\": \"{}\",",
        header.receipt_rent_principal
    );
    println!("    \"outcomeCount\": {},", header.outcome_count);
    println!("    \"rentBefore\": \"{}\"", header.rent_credit_before);
    println!("  }},");
    println!("  \"support\": [");
    println!("{}", rows_json.join(",\n"));
    println!("  ],");
    println!("  \"coordinateBytes\": {LIFECYCLE_COORDINATE_BYTES_V2},");
    println!("  \"headerBytes\": {LIFECYCLE_HEADER_BYTES_V2},");
    println!("  \"family\": \"{}\",", hex(family.as_bytes()));
    println!("  \"familyDigest\": \"{}\",", hex(&family_digest));
    println!("  \"child\": \"{}\",", hex(&child_bytes));
    println!("  \"childDigest\": \"{}\"", hex(&sha256(&child_bytes)));
    println!("}}");
}
