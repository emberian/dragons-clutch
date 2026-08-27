use clutch_solana_layout::stream::{
    frozen_set_commitment, init_page, seal_page, streamed_page_digest, verify_page,
    write_single_slot, write_tombstone, OrderPageHeader, OrderSlotCursor,
};
use clutch_solana_layout::{
    account_len, canonical_epoch_id, canonical_market_id, canonical_profile_hash,
    canonical_realm_id, CodecError, Hash32, OrderPageAccount, OrderRecord, PROFILE_PARENT_BYTES,
};

use crate::digest::{Rng, Transcript};
use crate::Counts;

const MUTATIONS_PER_SEED: u64 = 256;

pub fn run(seeds: &[u64], transcript: &mut Transcript) -> Counts {
    let mut counts = Counts::default();
    for seed in seeds.iter().copied() {
        run_page(seed, transcript, &mut counts);
    }
    counts
}

fn run_page(seed: u64, transcript: &mut Transcript, counts: &mut Counts) {
    let mut rng = Rng::new(seed ^ 0x1a70_0b1e_c0de_cafe);
    let mut profile_bytes = [0u8; PROFILE_PARENT_BYTES];
    profile_bytes[..8].copy_from_slice(b"DCPROF1\0");
    profile_bytes[8..16].copy_from_slice(&seed.to_le_bytes());
    profile_bytes[16] = 1;
    let profile = canonical_profile_hash(&profile_bytes).unwrap();
    let realm = canonical_realm_id(profile, seed);
    let market = canonical_market_id(realm, profile, seed.rotate_left(13));
    let epoch = canonical_epoch_id(market, seed.rotate_right(7));
    let mut page = vec![0u8; account_len::ORDER_PAGE];
    let mut header = init_page(&mut page, market, epoch, 0, 1, (seed & 0xff) as u8).unwrap();
    let order_count = 1 + rng.below(16) as usize;
    let mut owners = [Hash32::ZERO; 16];

    for (index, owner_slot) in owners.iter_mut().enumerate().take(order_count) {
        let mut owner_bytes = [0u8; 32];
        owner_bytes[..8].copy_from_slice(&seed.to_le_bytes());
        owner_bytes[8..16].copy_from_slice(&(index as u64 + 1).to_le_bytes());
        owner_bytes[31] = 1;
        let owner = Hash32::new(owner_bytes).unwrap();
        *owner_slot = owner;
        let quantity = 1 + rng.below(1_000_000);
        let all_or_none = rng.below(8) == 0;
        let order = OrderRecord {
            owner,
            order_id: header.next_order_id().unwrap(),
            outcome: rng.below(16) as u8,
            side: rng.below(2) as u8,
            quantity,
            limit: rng.next(),
            minimum_fill: if all_or_none {
                quantity
            } else {
                rng.below(quantity + 1)
            },
            flags: u8::from(all_or_none),
            generation: rng.next(),
            expiry_epoch: rng.next(),
        };
        header = write_single_slot(&mut page, &order).unwrap();
    }

    let cancellations = if order_count > 1 {
        rng.below(order_count as u64) as usize
    } else {
        0
    };
    for (index, owner) in owners.iter().copied().enumerate().take(cancellations) {
        let id = clutch_solana_layout::canonical_order_id(index as u64 + 1);
        let current = OrderPageAccount::decode(&page).unwrap();
        let retired_generation = current.orders[index].generation();
        header = write_tombstone(
            &mut page,
            id,
            owner,
            retired_generation.saturating_add(1).max(1),
        )
        .unwrap();
    }
    assert_eq!(header.order_count as usize, order_count);
    let (order_set, set_count) = frozen_set_commitment(&[&page]).unwrap();
    seal_page(&mut page, order_set, set_count).unwrap();

    compare_readers(seed, 0, &page, transcript, counts);
    let decoded = OrderPageAccount::decode(&page).unwrap();
    assert_eq!(
        decoded
            .encode(&mut vec![0u8; account_len::ORDER_PAGE])
            .unwrap(),
        account_len::ORDER_PAGE
    );
    assert_eq!(streamed_page_digest(&page), Ok(decoded.page_digest));
    let mut cursor = OrderSlotCursor::new(&page).unwrap();
    for expected in decoded.orders {
        assert_eq!(cursor.next_slot(), Some(Ok(expected)));
    }
    assert_eq!(cursor.next_slot(), None);

    let mut short = page.clone();
    short.pop();
    compare_readers(seed, 1, &short, transcript, counts);
    let mut long = page.clone();
    long.push(0);
    compare_readers(seed, 2, &long, transcript, counts);

    for case in 0..MUTATIONS_PER_SEED {
        let mut mutated = page.clone();
        let offset = rng.below(mutated.len() as u64) as usize;
        let bit = 1u8 << rng.below(8);
        mutated[offset] ^= bit;
        transcript.u64(offset as u64);
        transcript.byte(bit);
        compare_readers(seed, case + 3, &mutated, transcript, counts);
    }
}

fn compare_readers(
    seed: u64,
    case: u64,
    bytes: &[u8],
    transcript: &mut Transcript,
    counts: &mut Counts,
) {
    let buffered = OrderPageAccount::decode(bytes).map(|page| OrderPageHeader::of_page(&page));
    let streamed = verify_page(bytes);
    assert_eq!(
        buffered,
        streamed,
        "buffered/streaming divergence seed={seed:#x} case={case} len={}",
        bytes.len()
    );
    counts.cases += 1;
    transcript.text("layout-page");
    transcript.u64(seed);
    transcript.u64(case);
    transcript.u64(bytes.len() as u64);
    match buffered {
        Ok(header) => {
            counts.accepted += 1;
            transcript.byte(1);
            transcript.bytes(&header.page_digest.bytes());
            let decoded = OrderPageAccount::decode(bytes).unwrap();
            let mut canonical = vec![0u8; account_len::ORDER_PAGE];
            assert_eq!(decoded.encode(&mut canonical), Ok(account_len::ORDER_PAGE));
            assert_eq!(
                canonical, bytes,
                "accepted bytes were not canonical seed={seed:#x} case={case}"
            );
            assert_eq!(streamed_page_digest(bytes), Ok(decoded.page_digest));
        }
        Err(error) => {
            counts.refused += 1;
            transcript.byte(0);
            transcript.u64(u64::from(error.code()));
            assert!(matches!(
                error,
                CodecError::Truncated
                    | CodecError::TrailingBytes
                    | CodecError::WrongTag
                    | CodecError::WrongVersion
                    | CodecError::InvalidCount
                    | CodecError::InvalidEnum
                    | CodecError::ZeroValue
                    | CodecError::ZeroIdentity
                    | CodecError::NonCanonicalIdentity
                    | CodecError::NonCanonicalPadding
                    | CodecError::InvalidPriceGrid
                    | CodecError::InvalidTick
                    | CodecError::MismatchedBinding
                    | CodecError::AggregateClosureMismatch
                    | CodecError::InvalidConsideration
                    | CodecError::ArithmeticOverflow
                    | CodecError::OutputTooSmall
            ));
        }
    }
}
