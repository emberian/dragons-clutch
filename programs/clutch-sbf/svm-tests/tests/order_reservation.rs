//! Real-SBF evidence for funded order admission and cancellation.
//!
//! These scenarios exercise the actual account plane: System-program creation
//! of one canonical reservation PDA, exact Position encumbrance, cancellation
//! release, refusal rollback, and isolation between two owners sharing one
//! page.  They do not exercise settlement; `SettlePage` remains fail-closed.

use {
    clutch_sbf::{error::ClutchError, instructions::orders_batch, seeds},
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id,
        reservation::{canonical_reservation_id, ReservationAccount},
        EpochAccount, Hash32, Intent, OrderPageAccount, OrderRecord, OrderSlot, PositionAccount,
        PriceGridAccount, EPOCH_PHASE_OPEN, MAX_GRID_TICKS, MAX_ORDERS_PER_PAGE, MAX_OUTCOMES,
        RELATION_VERSION,
    },
    clutch_svm_fixture::{
        build_plane, compute_unit_limit_data, layout_request, Mode, COMPUTE_BUDGET, PROGRAM_ID,
        RENT_SYSVAR, SYSTEM_PROGRAM,
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_test::{tokio, BanksClient, ProgramTest},
    solana_signer::Signer,
    solana_system_interface::program as system_program,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

const EPOCH_INDEX: u64 = 9;
const PRICE_SCALE: u64 = 10_000;

fn first_actor() -> Keypair {
    Keypair::new_from_array([
        0x31, 0x7a, 0x49, 0x03, 0x99, 0x51, 0xd2, 0x8b, 0xe0, 0x0d, 0x73, 0x42, 0xaf, 0x14, 0x57,
        0x6c, 0x92, 0x05, 0xbb, 0x18, 0x81, 0x6e, 0x3a, 0x44, 0x09, 0xcf, 0x62, 0xf1, 0x35, 0x77,
        0xa0, 0x5d,
    ])
}

fn second_actor() -> Keypair {
    Keypair::new_from_array([
        0x42, 0x11, 0xb0, 0x92, 0x63, 0xad, 0x05, 0x5e, 0x2c, 0x74, 0x8d, 0x3a, 0xf1, 0x09, 0xc0,
        0x36, 0x6a, 0x99, 0x20, 0xe2, 0x15, 0x7b, 0x5c, 0x88, 0x41, 0xde, 0x03, 0x67, 0xfa, 0x24,
        0x59, 0x9c,
    ])
}

fn encode<F>(len: usize, writer: F) -> Vec<u8>
where
    F: FnOnce(&mut [u8]) -> Result<usize, clutch_solana_layout::CodecError>,
{
    let mut bytes = vec![0; len];
    assert_eq!(writer(&mut bytes).unwrap(), len);
    bytes
}

fn pda(prefix: &[u8], suffixes: &[&[u8]]) -> (Address, u8) {
    let mut all = Vec::with_capacity(1 + suffixes.len());
    all.push(prefix);
    all.extend_from_slice(suffixes);
    Address::find_program_address(&all, &PROGRAM_ID)
}

fn rent_exempt(len: usize) -> u64 {
    solana_rent::Rent::default().minimum_balance(len).max(1)
}

struct OrderPlane {
    epoch: Address,
    grid: Address,
    page: Address,
    first_position: Address,
    second_position: Address,
    market: Hash32,
    epoch_id: Hash32,
    terms: Hash32,
    policy: Hash32,
    first_owner: Hash32,
    second_owner: Hash32,
}

impl OrderPlane {
    fn order(&self, owner: Hash32, rank: u64, side: u8, quantity: u64) -> OrderSlot {
        OrderSlot::Single(OrderRecord {
            owner,
            order_id: canonical_order_id(rank),
            outcome: 0,
            side,
            quantity,
            limit: 5_000,
            minimum_fill: 0,
            flags: 0,
            generation: rank,
            expiry_epoch: EPOCH_INDEX,
        })
    }

    fn reservation(&self, owner: Hash32, order_id: Hash32) -> Address {
        let id = canonical_reservation_id(self.market, self.epoch_id, owner, 0, order_id);
        pda(seeds::SEED_RESERVATION, &[&id.bytes()]).0
    }

    fn place(
        &self,
        actor: Address,
        position: Address,
        sequence: u64,
        max_fee_atoms: u64,
        slot: OrderSlot,
    ) -> Instruction {
        let reservation = self.reservation(slot.owner(), slot.order_id());
        let metas = vec![
            AccountMeta::new(actor, true),
            AccountMeta::new_readonly(self.epoch, false),
            AccountMeta::new_readonly(self.grid, false),
            AccountMeta::new(self.page, false),
            AccountMeta::new(position, false),
            AccountMeta::new(reservation, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        assert_eq!(metas.len(), orders_batch::PLACE_ORDER_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                Intent::PlaceOrder {
                    market: self.market,
                    epoch: self.epoch_id,
                    max_fee_atoms,
                    slot,
                },
            ),
            metas,
        )
    }

    fn cancel(
        &self,
        actor: Address,
        position: Address,
        reservation: Address,
        sequence: u64,
        owner: Hash32,
        order_id: Hash32,
    ) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(actor, true),
            AccountMeta::new_readonly(self.epoch, false),
            AccountMeta::new(self.page, false),
            AccountMeta::new(position, false),
            AccountMeta::new(reservation, false),
        ];
        assert_eq!(metas.len(), orders_batch::CANCEL_ORDER_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                Intent::CancelOrder {
                    market: self.market,
                    epoch: self.epoch_id,
                    owner,
                    order_id,
                    generation: sequence,
                },
            ),
            metas,
        )
    }
}

async fn start() -> (BanksClient, Keypair, Keypair, Keypair, OrderPlane) {
    let first = first_actor();
    let second = second_actor();
    let collateral_mint = Address::new_from_array([0x91; 32]);
    let fixture = build_plane(first.pubkey(), collateral_mint, 9, Mode::Funded);

    let mut ticks = [0; MAX_GRID_TICKS];
    ticks[..3].copy_from_slice(&[2_500, 5_000, 7_500]);
    let mut grid = PriceGridAccount {
        grid: Hash32::ZERO,
        realm: fixture.realm_id,
        price_scale: PRICE_SCALE,
        tick_count: 3,
        ticks,
        stored_bump: 0,
        flags: 0,
    };
    grid.grid = grid.recomputed_grid_id().unwrap();
    let (grid_address, grid_bump) = pda(
        seeds::SEED_GRID,
        &[&fixture.realm_id.bytes(), &grid.grid.bytes()],
    );
    grid.stored_bump = grid_bump;

    let epoch_id = canonical_epoch_id(fixture.market_id, EPOCH_INDEX);
    let epoch_index_bytes = EPOCH_INDEX.to_le_bytes();
    let (epoch_address, epoch_bump) = pda(
        seeds::SEED_EPOCH,
        &[&fixture.market_id.bytes(), &epoch_index_bytes],
    );
    let policy = fixture
        .policy
        .digest()
        .expect("the fixture policy has one immutable identity");
    let epoch = EpochAccount {
        epoch: epoch_id,
        market: fixture.market_id,
        book: Hash32::from_bytes([0x73; 32]),
        terms: fixture.terms_id,
        price_grid: grid.grid,
        policy,
        order_set: Hash32::ZERO,
        first_order_id: Hash32::ZERO,
        last_order_id: Hash32::ZERO,
        epoch_index: EPOCH_INDEX,
        relation_version: RELATION_VERSION,
        price_scale: PRICE_SCALE,
        remainder_seed: 19,
        owner_count: 2,
        page_count: 0,
        order_count: 0,
        outcome_count: 2,
        phase: EPOCH_PHASE_OPEN,
        stored_bump: epoch_bump,
        flags: 0,
    };

    let page_index_bytes = 0_u16.to_le_bytes();
    let (page_address, page_bump) = pda(seeds::SEED_PAGE, &[&epoch_id.bytes(), &page_index_bytes]);
    let mut page = OrderPageAccount {
        market: fixture.market_id,
        epoch: epoch_id,
        order_set: Hash32::ZERO,
        page_digest: Hash32::ZERO,
        first_order_id: Hash32::ZERO,
        last_order_id: Hash32::ZERO,
        prev_page_last_order_id: Hash32::ZERO,
        page_index: 0,
        page_count: 1,
        set_order_count: 0,
        order_count: 0,
        tombstone_count: 0,
        frozen: 0,
        stored_bump: page_bump,
        orders: [OrderSlot::Empty; MAX_ORDERS_PER_PAGE],
    };
    page.page_digest = page.recomputed_page_digest().unwrap();

    let first_owner = Hash32::from_bytes(first.pubkey().to_bytes());
    let second_owner = Hash32::from_bytes(second.pubkey().to_bytes());
    let (second_position, second_position_bump) = pda(
        seeds::SEED_POSITION,
        &[&fixture.market_id.bytes(), &second_owner.bytes()],
    );
    let mut second_internal = [0; MAX_OUTCOMES];
    second_internal[0] = 20;
    second_internal[1] = 20;
    let second_position_state = PositionAccount {
        market: fixture.market_id,
        owner: second_owner,
        generation: 0,
        internal: second_internal,
        cash_atoms: 60,
        reserved_cash_atoms: 3,
        stored_bump: second_position_bump,
        close_state: 0,
    };

    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("clutch_sbf", PROGRAM_ID, None);
    for actor in [&first, &second] {
        test.add_account(
            actor.pubkey(),
            Account {
                lamports: 2_000_000_000,
                data: Vec::new(),
                owner: system_program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    for account in &fixture.accounts {
        test.add_account(
            account.address,
            Account {
                lamports: rent_exempt(account.data.len()),
                data: account.data.clone(),
                owner: account.owner,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    for (address, data) in [
        (
            grid_address,
            encode(account_len::PRICE_GRID, |out| grid.encode(out)),
        ),
        (
            epoch_address,
            encode(account_len::EPOCH, |out| epoch.encode(out)),
        ),
        (
            page_address,
            encode(account_len::ORDER_PAGE, |out| page.encode(out)),
        ),
        (
            second_position,
            encode(account_len::POSITION, |out| {
                second_position_state.encode(out)
            }),
        ),
    ] {
        test.add_account(
            address,
            Account {
                lamports: rent_exempt(data.len()),
                data,
                owner: PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    let order_plane = OrderPlane {
        epoch: epoch_address,
        grid: grid_address,
        page: page_address,
        first_position: fixture.position.address,
        second_position,
        market: fixture.market_id,
        epoch_id,
        terms: fixture.terms_id,
        policy,
        first_owner,
        second_owner,
    };
    let (banks, payer, _) = test.start().await;
    (banks, payer, first, second, order_plane)
}

async fn send(
    banks: &mut BanksClient,
    payer: &Keypair,
    instruction: Instruction,
    signer: &Keypair,
) -> (Result<(), TransactionError>, u64) {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let budget = Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &compute_unit_limit_data(1_400_000),
        Vec::new(),
    );
    let transaction = Transaction::new_signed_with_payer(
        &[budget, instruction],
        Some(&payer.pubkey()),
        &[payer, signer],
        blockhash,
    );
    let outcome = banks
        .process_transaction_with_metadata(transaction)
        .await
        .unwrap();
    let units = outcome
        .metadata
        .map(|metadata| metadata.compute_units_consumed)
        .unwrap_or_default();
    (outcome.result, units)
}

async fn bytes(banks: &mut BanksClient, address: Address) -> Vec<u8> {
    banks
        .get_account(address)
        .await
        .unwrap()
        .expect("account must exist")
        .data
}

async fn absent(banks: &mut BanksClient, address: Address) -> bool {
    banks.get_account(address).await.unwrap().is_none()
}

fn custom(result: Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => code,
        other => panic!("expected custom refusal, got {other:?}"),
    }
}

#[tokio::test]
async fn funded_orders_reserve_release_and_isolate_owners() {
    let (mut banks, payer, first, second, plane) = start().await;
    let first_slot = plane.order(plane.first_owner, 1, 0, 8);
    let first_reservation = plane.reservation(plane.first_owner, first_slot.order_id());
    let (first_result, first_units) = send(
        &mut banks,
        &payer,
        plane.place(first.pubkey(), plane.first_position, 0, 1, first_slot),
        &first,
    )
    .await;
    first_result.unwrap();
    assert!(first_units < 1_400_000);

    let first_position = PositionAccount::decode(&bytes(&mut banks, plane.first_position).await)
        .expect("first Position remains canonical");
    assert_eq!(first_position.reserved_cash_atoms, 12); // fixture 7 + ceil(8*0.5) + fee 1
    let first_reservation_state =
        ReservationAccount::decode(&bytes(&mut banks, first_reservation).await).unwrap();
    assert_eq!(first_reservation_state.remaining_cash_atoms, 5);
    assert_eq!(first_reservation_state.terms, plane.terms);
    assert_eq!(first_reservation_state.policy, plane.policy);

    let second_slot = plane.order(plane.second_owner, 2, 1, 6);
    let second_reservation = plane.reservation(plane.second_owner, second_slot.order_id());
    let (second_result, second_units) = send(
        &mut banks,
        &payer,
        plane.place(second.pubkey(), plane.second_position, 1, 0, second_slot),
        &second,
    )
    .await;
    second_result.unwrap();
    assert!(second_units < 1_400_000);
    eprintln!("PlaceOrder CU: buy={first_units}, sell={second_units}");
    let second_position = PositionAccount::decode(&bytes(&mut banks, plane.second_position).await)
        .expect("second Position remains canonical");
    assert_eq!(second_position.internal[0], 14);

    // Substituting the other owner's otherwise-valid reservation must leave
    // all three writable accounts byte-for-byte unchanged.
    let before_page = bytes(&mut banks, plane.page).await;
    let before_first = bytes(&mut banks, plane.first_position).await;
    let before_second_reservation = bytes(&mut banks, second_reservation).await;
    let wrong = plane.cancel(
        first.pubkey(),
        plane.first_position,
        second_reservation,
        2,
        plane.first_owner,
        first_slot.order_id(),
    );
    assert_eq!(
        custom(send(&mut banks, &payer, wrong, &first).await.0),
        ClutchError::MismatchedState as u32
    );
    assert_eq!(bytes(&mut banks, plane.page).await, before_page);
    assert_eq!(bytes(&mut banks, plane.first_position).await, before_first);
    assert_eq!(
        bytes(&mut banks, second_reservation).await,
        before_second_reservation
    );

    let (first_cancel_result, first_cancel_units) = send(
        &mut banks,
        &payer,
        plane.cancel(
            first.pubkey(),
            plane.first_position,
            first_reservation,
            2,
            plane.first_owner,
            first_slot.order_id(),
        ),
        &first,
    )
    .await;
    first_cancel_result.unwrap();
    let first_released = PositionAccount::decode(&bytes(&mut banks, plane.first_position).await)
        .expect("release restores the Position");
    assert_eq!(first_released.reserved_cash_atoms, 7);
    let second_still_reserved =
        ReservationAccount::decode(&bytes(&mut banks, second_reservation).await).unwrap();
    assert_eq!(second_still_reserved.remaining_internal[0], 6);

    let (second_cancel_result, second_cancel_units) = send(
        &mut banks,
        &payer,
        plane.cancel(
            second.pubkey(),
            plane.second_position,
            second_reservation,
            3,
            plane.second_owner,
            second_slot.order_id(),
        ),
        &second,
    )
    .await;
    second_cancel_result.unwrap();
    assert!(first_cancel_units < 1_400_000 && second_cancel_units < 1_400_000);
    eprintln!("CancelOrder CU: buy={first_cancel_units}, sell={second_cancel_units}");
    let second_released =
        PositionAccount::decode(&bytes(&mut banks, plane.second_position).await).unwrap();
    assert_eq!(second_released.internal[0], 20);
}

#[tokio::test]
async fn unfunded_place_refuses_without_creating_or_mutating_anything() {
    let (mut banks, payer, first, _second, plane) = start().await;
    let slot = plane.order(plane.first_owner, 1, 0, 1_000_000);
    let reservation = plane.reservation(plane.first_owner, slot.order_id());
    let before_page = bytes(&mut banks, plane.page).await;
    let before_position = bytes(&mut banks, plane.first_position).await;
    assert!(absent(&mut banks, reservation).await);

    let result = send(
        &mut banks,
        &payer,
        plane.place(first.pubkey(), plane.first_position, 0, 0, slot),
        &first,
    )
    .await;
    assert!(custom(result.0) != 0);
    assert!(absent(&mut banks, reservation).await);
    assert_eq!(bytes(&mut banks, plane.page).await, before_page);
    assert_eq!(
        bytes(&mut banks, plane.first_position).await,
        before_position
    );
}
