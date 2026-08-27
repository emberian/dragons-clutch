//! Complete-frame packet bound for compact projected Series execution.
//!
//! The semantic fixture will intentionally reuse many keys across the common
//! frame and Profile13 representatives. This bound instead makes every lookup
//! key distinct, so it measures the largest serialized ALT index set for the
//! same complete account-slot geometry rather than understating the packet.

use dclutch_core_contract::ContentId;
use dclutch_operator::series_projected_v2::{
    SERIES_PROJECTED_HOT_PREFIX_ACCOUNT_COUNT_V2,
    SERIES_PROJECTED_INJECTED_RUNTIME_ACCOUNT_COUNT_V2, build_series_projected_consume_v2,
};
use dclutch_series_v3_kernel::request::{SeriesActionV3, encode_series_action_header_v3};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_hash::Hash;
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

const MAXIMUM_FUNDING_COUNT: usize = 16;
const MAXIMUM_PROOF_WORDS: u8 = 9;
const PROFILE13_PHYSICAL_ACCOUNT_BASE: usize = 64;
const SOLANA_PACKET_BYTES: usize = 1_232;
const REQUIRED_PACKET_MARGIN: usize = 128;

fn id(value: u8) -> ContentId {
    ContentId::new([value; 32]).expect("identity")
}

fn family_request() -> Vec<u8> {
    let header = encode_series_action_header_v3(
        SeriesActionV3::Consume,
        id(1),
        Some(id(2)),
        Some(id(3)),
        4,
        5,
        MAXIMUM_PROOF_WORDS,
    )
    .expect("Series Consume header");
    let mut request = header.to_vec();
    request.extend_from_slice(&vec![9; usize::from(MAXIMUM_PROOF_WORDS) * 32]);
    request
}

#[test]
fn maximum_complete_outer_frame_fits_v0_with_compute_budget_and_alt() {
    let projected = build_series_projected_consume_v2(
        &family_request(),
        u8::try_from(MAXIMUM_FUNDING_COUNT).expect("bounded Funding count"),
    )
    .expect("compact projected execution");
    assert_eq!(projected.data().len(), 432);

    let runtime_physical = PROFILE13_PHYSICAL_ACCOUNT_BASE + MAXIMUM_FUNDING_COUNT;
    let runtime_suffix = runtime_physical - SERIES_PROJECTED_INJECTED_RUNTIME_ACCOUNT_COUNT_V2;
    let complete_frame = SERIES_PROJECTED_HOT_PREFIX_ACCOUNT_COUNT_V2 + runtime_suffix;
    // 45 / 120 before the validated-artifact seal joined the fixed hot prefix.
    assert_eq!(SERIES_PROJECTED_HOT_PREFIX_ACCOUNT_COUNT_V2, 46);
    assert_eq!(runtime_physical, 80);
    assert_eq!(complete_frame, 121);

    let payer = Pubkey::new_from_array([1; 32]);
    let trading_program = Pubkey::new_from_array([2; 32]);
    let addresses = (0..complete_frame)
        .map(|index| {
            Pubkey::new_from_array(
                [u8::try_from(index + 3).expect("bounded complete-frame index"); 32],
            )
        })
        .collect::<Vec<_>>();
    let accounts = addresses
        .iter()
        .enumerate()
        .map(|(index, key)| {
            if index.is_multiple_of(5) {
                AccountMeta::new(*key, false)
            } else {
                AccountMeta::new_readonly(*key, false)
            }
        })
        .collect::<Vec<_>>();
    let instruction = Instruction {
        program_id: trading_program,
        accounts,
        data: projected.data().to_vec(),
    };
    let lookup = AddressLookupTableAccount {
        key: Pubkey::new_from_array([254; 32]),
        addresses,
    };
    let message = v0::Message::try_compile(
        &payer,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
            ComputeBudgetInstruction::set_compute_unit_price(1),
            instruction,
        ],
        &[lookup],
        Hash::new_from_array([255; 32]),
    )
    .expect("complete projected Series v0 message");
    assert_eq!(message.account_keys.len(), 3);
    assert_eq!(message.address_table_lookups.len(), 1);
    let loaded = message
        .address_table_lookups
        .first()
        .expect("lookup")
        .writable_indexes
        .len()
        + message
            .address_table_lookups
            .first()
            .expect("lookup")
            .readonly_indexes
            .len();
    assert_eq!(loaded, complete_frame);
    let required_signatures = usize::from(message.header.num_required_signatures);
    let wire_bytes = 1 + required_signatures * 64 + VersionedMessage::V0(message).serialize().len();
    // 930 before the validated-artifact seal joined the fixed hot prefix: one
    // ALT-routed key costs one index byte in each of the two account lists.
    assert_eq!(wire_bytes, 932);
    assert!(
        wire_bytes + REQUIRED_PACKET_MARGIN <= SOLANA_PACKET_BYTES,
        "{wire_bytes}B complete-frame packet leaves less than {REQUIRED_PACKET_MARGIN}B margin"
    );
}
