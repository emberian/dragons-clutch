//! Real-bank regression evidence for predictable-PDA prefunding.
//!
//! A System transfer can create a zero-data account at any predictable PDA
//! without that PDA's signature.  Such SOL is a donation, not initialization:
//! Clutch must retain it, fund only a rent shortfall, then PDA-sign `Allocate`
//! and `Assign`.  Conversely, owner, data, and executable state are semantic
//! occupation and must refuse.  These tests drive the real SBF program and
//! real System/Token-2022 programs across market, reservation, and owner-plane
//! creation.

use {
    clutch_sbf::{
        instructions::{genesis, market_init, orders_batch},
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_epoch_id, canonical_order_id,
        reservation::{canonical_reservation_id, ReservationAccount, RESERVATION_ACCOUNT_BYTES},
        EpochAccount, Hash32, Intent, OrderPageAccount, OrderRecord, OrderSlot, PositionAccount,
        PriceGridAccount, EPOCH_PHASE_OPEN, MAX_GRID_TICKS, MAX_ORDERS_PER_PAGE, RELATION_VERSION,
    },
    clutch_solana_reference::ReplayAccount,
    clutch_svm_fixture::{
        build_plane, compute_unit_limit_data, create_market_request, immutable_owner_account_bytes,
        layout_request, Mode, Plane, CASH_ATOMS, COMPUTE_BUDGET, FUNDED_SETS, MARKET_NONCE,
        OUTCOME_COUNT, PROGRAM_ID, RENT_SYSVAR, SYSTEM_PROGRAM, TOKEN_2022,
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_pack::Pack,
    solana_program_test::{tokio, BanksClient, ProgramTest},
    solana_signer::Signer,
    solana_system_interface::{instruction as system_instruction, program as system_program},
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
    spl_token_2022_interface::{
        instruction as token_instruction,
        instruction::AuthorityType,
        state::{Account as TokenAccount, Mint},
    },
};

const CU_LIMIT: u32 = 1_400_000;
const EPOCH_INDEX: u64 = 9;
const PRICE_SCALE: u64 = 10_000;
const COLLATERAL_DECIMALS: u8 = 6;
const CREATOR_LAMPORTS: u64 = 10_000_000_000;
const DONATION: u64 = 77_777;

fn creator_keypair() -> Keypair {
    Keypair::new_from_array([
        0x93, 0x1a, 0x72, 0x4e, 0x05, 0xbc, 0x61, 0x38, 0xdf, 0x20, 0xa4, 0x59, 0x13, 0xe7, 0x8c,
        0x42, 0x6d, 0x99, 0x31, 0x0f, 0xa8, 0x55, 0xc2, 0x17, 0x4b, 0xe0, 0x76, 0x2a, 0x8d, 0x44,
        0xf3, 0x6c,
    ])
}

fn collateral_mint_keypair() -> Keypair {
    Keypair::new_from_array([
        0x2c, 0xe1, 0x17, 0x95, 0x68, 0x34, 0xab, 0x40, 0x73, 0x0d, 0xf9, 0x52, 0x86, 0x1b, 0xc5,
        0x29, 0x74, 0xae, 0x03, 0x67, 0x9c, 0x41, 0xd8, 0x10, 0x5a, 0xb3, 0x26, 0xee, 0x80, 0x4f,
        0x19, 0x62,
    ])
}

fn second_owner_keypair() -> Keypair {
    Keypair::new_from_array([
        0x71, 0x08, 0xd4, 0x39, 0xb6, 0x2f, 0x83, 0x15, 0xca, 0x64, 0x20, 0x9e, 0x47, 0xf1, 0x5b,
        0x32, 0xad, 0x06, 0x78, 0xc3, 0x11, 0xe5, 0x59, 0x24, 0x8a, 0x4d, 0x90, 0x37, 0xfb, 0x6e,
        0x02, 0xac,
    ])
}

fn foreign_owner() -> Address {
    Address::new_from_array([0xa5; 32])
}

fn rent_exempt(space: usize) -> u64 {
    solana_rent::Rent::default().minimum_balance(space).max(1)
}

fn budget() -> Instruction {
    Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &compute_unit_limit_data(CU_LIMIT),
        Vec::new(),
    )
}

fn system_slot(lamports: u64) -> Account {
    Account {
        lamports,
        data: Vec::new(),
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn assert_account_image_eq(actual: &Account, expected: &Account) {
    assert_eq!(actual.lamports, expected.lamports);
    assert_eq!(actual.data, expected.data);
    assert_eq!(actual.owner, expected.owner);
    assert_eq!(actual.executable, expected.executable);
    assert_eq!(actual.rent_epoch, expected.rent_epoch);
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

async fn try_send(
    banks: &mut BanksClient,
    payer: &Keypair,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<(), TransactionError> {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let mut all = vec![payer];
    all.extend_from_slice(signers);
    let transaction =
        Transaction::new_signed_with_payer(instructions, Some(&payer.pubkey()), &all, blockhash);
    banks
        .process_transaction_with_metadata(transaction)
        .await
        .unwrap()
        .result
}

async fn send(
    banks: &mut BanksClient,
    payer: &Keypair,
    instructions: &[Instruction],
    signers: &[&Keypair],
) {
    try_send(banks, payer, instructions, signers)
        .await
        .expect("transaction was expected to succeed");
}

async fn account(banks: &mut BanksClient, address: Address) -> Account {
    banks
        .get_account(address)
        .await
        .unwrap()
        .expect("account must exist")
}

async fn publicly_prefund(
    banks: &mut BanksClient,
    payer: &Keypair,
    address: Address,
    lamports: u64,
) {
    send(
        banks,
        payer,
        &[system_instruction::transfer(
            &payer.pubkey(),
            &address,
            lamports,
        )],
        &[],
    )
    .await;
}

async fn create_collateral_mint(banks: &mut BanksClient, payer: &Keypair, mint: &Keypair) {
    let lamports = banks
        .get_rent()
        .await
        .unwrap()
        .minimum_balance(Mint::LEN)
        .max(1);
    send(
        banks,
        payer,
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &mint.pubkey(),
                lamports,
                Mint::LEN as u64,
                &TOKEN_2022,
            ),
            token_instruction::initialize_mint2(
                &TOKEN_2022,
                &mint.pubkey(),
                &payer.pubkey(),
                None,
                COLLATERAL_DECIMALS,
            )
            .unwrap(),
        ],
        &[mint],
    )
    .await;
}

async fn revoke_mint_authority(banks: &mut BanksClient, payer: &Keypair, mint: Address) {
    send(
        banks,
        payer,
        &[token_instruction::set_authority(
            &TOKEN_2022,
            &mint,
            None,
            AuthorityType::MintTokens,
            &payer.pubkey(),
            &[],
        )
        .unwrap()],
        &[],
    )
    .await;
}

async fn create_token_account(
    banks: &mut BanksClient,
    payer: &Keypair,
    mint: Address,
    owner: Address,
) -> (Keypair, Address) {
    let token = Keypair::new();
    let lamports = banks
        .get_rent()
        .await
        .unwrap()
        .minimum_balance(TokenAccount::LEN)
        .max(1);
    send(
        banks,
        payer,
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &token.pubkey(),
                lamports,
                TokenAccount::LEN as u64,
                &TOKEN_2022,
            ),
            token_instruction::initialize_account3(&TOKEN_2022, &token.pubkey(), &mint, &owner)
                .unwrap(),
        ],
        &[&token],
    )
    .await;
    let address = token.pubkey();
    (token, address)
}

/* --------------------------------------------------------------------- */
/* Market creation                                                        */
/* --------------------------------------------------------------------- */

fn create_market_instruction(plane: &Plane, creator: Address, nonce: u64) -> Instruction {
    let addresses = plane.create_market_addresses(creator);
    let mut metas = vec![AccountMeta::new(addresses[0], true)];
    for (index, address) in addresses.iter().enumerate().skip(1) {
        let writable = matches!(index, 4..=10)
            || index == market_init::IX_HOARD_TOKEN
            || index >= market_init::IX_OUTCOME_MINT_BASE;
        metas.push(if writable {
            AccountMeta::new(*address, false)
        } else {
            AccountMeta::new_readonly(*address, false)
        });
    }
    assert_eq!(metas.len(), market_init::account_count(OUTCOME_COUNT));
    Instruction::new_with_bytes(PROGRAM_ID, &create_market_request(plane, nonce), metas)
}

#[tokio::test]
async fn a_public_over_rent_market_prefund_is_retained_and_not_charged_again() {
    let creator = creator_keypair();
    let mint = collateral_mint_keypair();
    let nonce = 81;
    let plane = build_plane(creator.pubkey(), mint.pubkey(), nonce, Mode::Empty);
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("clutch_sbf", PROGRAM_ID, None);
    test.add_account(creator.pubkey(), system_slot(CREATOR_LAMPORTS));
    for item in &plane.accounts {
        test.add_account(
            item.address,
            Account {
                lamports: rent_exempt(item.data.len()),
                data: item.data.clone(),
                owner: item.owner,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
    let (mut banks, payer, _) = test.start().await;
    create_collateral_mint(&mut banks, &payer, &mint).await;
    let (_collateral_holder, collateral_account) =
        create_token_account(&mut banks, &payer, mint.pubkey(), creator.pubkey()).await;
    send(
        &mut banks,
        &payer,
        &[token_instruction::mint_to(
            &TOKEN_2022,
            &mint.pubkey(),
            &collateral_account,
            &payer.pubkey(),
            &[],
            1,
        )
        .unwrap()],
        &[],
    )
    .await;
    revoke_mint_authority(&mut banks, &payer, mint.pubkey()).await;

    let market_prefund = rent_exempt(account_len::MARKET) + DONATION;
    publicly_prefund(&mut banks, &payer, plane.market.address, market_prefund).await;
    let before_creator = account(&mut banks, creator.pubkey()).await.lamports;
    send(
        &mut banks,
        &payer,
        &[
            budget(),
            create_market_instruction(&plane, creator.pubkey(), nonce),
        ],
        &[&creator],
    )
    .await;

    let market = account(&mut banks, plane.market.address).await;
    assert_eq!(market.owner, PROGRAM_ID);
    assert_eq!(market.data.len(), account_len::MARKET);
    assert_eq!(market.lamports, market_prefund, "the excess stays donated");

    let mut all_created = plane.market_state_addresses().to_vec();
    all_created.push(plane.hoard_token.address);
    all_created.extend(plane.outcome_mints.iter().map(|mint| mint.address));
    let total_after: u64 = {
        let mut sum = 0_u64;
        for address in all_created {
            sum = sum
                .checked_add(account(&mut banks, address).await.lamports)
                .unwrap();
        }
        sum
    };
    let after_creator = account(&mut banks, creator.pubkey()).await.lamports;
    assert_eq!(
        before_creator - after_creator,
        total_after - market_prefund,
        "the creator funds every other new account but contributes zero to the over-rent Market"
    );
}

/* --------------------------------------------------------------------- */
/* Reservation creation                                                   */
/* --------------------------------------------------------------------- */

#[derive(Clone)]
enum ReservationPrestate {
    Absent,
    Genesis(Account),
}

struct OrderWorld {
    banks: BanksClient,
    payer: Keypair,
    actor: Keypair,
    epoch: Address,
    grid: Address,
    page: Address,
    position: Address,
    market: Hash32,
    epoch_id: Hash32,
    owner: Hash32,
    reservation: Address,
}

impl OrderWorld {
    async fn start(prestate: ReservationPrestate) -> Self {
        let actor = creator_keypair();
        let fixture = build_plane(
            actor.pubkey(),
            Address::new_from_array([0x91; 32]),
            MARKET_NONCE,
            Mode::Funded,
        );

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
        let epoch_index = EPOCH_INDEX.to_le_bytes();
        let (epoch_address, epoch_bump) = pda(
            seeds::SEED_EPOCH,
            &[&fixture.market_id.bytes(), &epoch_index],
        );
        let epoch = EpochAccount {
            epoch: epoch_id,
            market: fixture.market_id,
            book: Hash32::from_bytes([0x73; 32]),
            terms: fixture.terms_id,
            price_grid: grid.grid,
            policy: fixture.policy.digest().unwrap(),
            order_set: Hash32::ZERO,
            first_order_id: Hash32::ZERO,
            last_order_id: Hash32::ZERO,
            epoch_index: EPOCH_INDEX,
            relation_version: RELATION_VERSION,
            price_scale: PRICE_SCALE,
            remainder_seed: 19,
            owner_count: 1,
            page_count: 0,
            order_count: 0,
            outcome_count: OUTCOME_COUNT,
            basis_degree: 0,
            phase: EPOCH_PHASE_OPEN,
            stored_bump: epoch_bump,
            flags: 0,
        };

        let page_index = 0_u16.to_le_bytes();
        let (page_address, page_bump) = pda(seeds::SEED_PAGE, &[&epoch_id.bytes(), &page_index]);
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

        let owner = Hash32::from_bytes(actor.pubkey().to_bytes());
        let order_id = canonical_order_id(1);
        let reservation_id =
            canonical_reservation_id(fixture.market_id, epoch_id, owner, 0, order_id);
        let reservation = pda(seeds::SEED_RESERVATION, &[&reservation_id.bytes()]).0;

        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.add_program("clutch_sbf", PROGRAM_ID, None);
        test.add_account(actor.pubkey(), system_slot(CREATOR_LAMPORTS));
        for item in &fixture.accounts {
            test.add_account(
                item.address,
                Account {
                    lamports: rent_exempt(item.data.len()),
                    data: item.data.clone(),
                    owner: item.owner,
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
        if let ReservationPrestate::Genesis(account) = prestate {
            test.add_account(reservation, account);
        }

        let (banks, payer, _) = test.start().await;
        Self {
            banks,
            payer,
            actor,
            epoch: epoch_address,
            grid: grid_address,
            page: page_address,
            position: fixture.position.address,
            market: fixture.market_id,
            epoch_id,
            owner,
            reservation,
        }
    }

    fn place(&self) -> Instruction {
        let slot = OrderSlot::Single(OrderRecord {
            owner: self.owner,
            order_id: canonical_order_id(1),
            outcome: 0,
            side: 0,
            quantity: 8,
            limit: 5_000,
            minimum_fill: 0,
            flags: 0,
            generation: 1,
            expiry_epoch: EPOCH_INDEX,
        });
        let metas = vec![
            AccountMeta::new(self.actor.pubkey(), true),
            AccountMeta::new_readonly(self.epoch, false),
            AccountMeta::new_readonly(self.grid, false),
            AccountMeta::new(self.page, false),
            AccountMeta::new(self.position, false),
            AccountMeta::new(self.reservation, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        assert_eq!(metas.len(), orders_batch::PLACE_ORDER_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::PlaceOrder {
                    market: self.market,
                    epoch: self.epoch_id,
                    max_fee_atoms: 1,
                    slot,
                },
            ),
            metas,
        )
    }
}

#[tokio::test]
async fn one_lamport_and_public_over_rent_reservation_prefunds_cannot_squat() {
    let mut genesis = OrderWorld::start(ReservationPrestate::Genesis(system_slot(1))).await;
    let genesis_actor_before = account(&mut genesis.banks, genesis.actor.pubkey())
        .await
        .lamports;
    let genesis_place = genesis.place();
    send(
        &mut genesis.banks,
        &genesis.payer,
        &[budget(), genesis_place],
        &[&genesis.actor],
    )
    .await;
    let recovered = account(&mut genesis.banks, genesis.reservation).await;
    assert_eq!(recovered.owner, PROGRAM_ID);
    assert_eq!(recovered.data.len(), RESERVATION_ACCOUNT_BYTES);
    assert_eq!(recovered.lamports, rent_exempt(RESERVATION_ACCOUNT_BYTES));
    assert_eq!(
        genesis_actor_before
            - account(&mut genesis.banks, genesis.actor.pubkey())
                .await
                .lamports,
        rent_exempt(RESERVATION_ACCOUNT_BYTES) - 1,
        "only the one-lamport target's exact rent shortfall is charged"
    );
    ReservationAccount::decode(&recovered.data).expect("reservation decodes after recovery");

    let mut public = OrderWorld::start(ReservationPrestate::Absent).await;
    let donation = rent_exempt(RESERVATION_ACCOUNT_BYTES) + DONATION;
    publicly_prefund(
        &mut public.banks,
        &public.payer,
        public.reservation,
        donation,
    )
    .await;
    let before_actor = account(&mut public.banks, public.actor.pubkey())
        .await
        .lamports;
    let public_place = public.place();
    send(
        &mut public.banks,
        &public.payer,
        &[budget(), public_place],
        &[&public.actor],
    )
    .await;
    let after_actor = account(&mut public.banks, public.actor.pubkey())
        .await
        .lamports;
    let retained = account(&mut public.banks, public.reservation).await;
    assert_eq!(
        before_actor, after_actor,
        "an over-rent target needs no top-up"
    );
    assert_eq!(
        retained.lamports, donation,
        "excess SOL is retained exactly"
    );
    assert_eq!(retained.owner, PROGRAM_ID);
    assert_eq!(retained.data.len(), RESERVATION_ACCOUNT_BYTES);
}

#[tokio::test]
async fn reservation_owner_data_and_executable_squats_refuse_without_writes() {
    let hostile = [
        Account {
            lamports: 1,
            data: Vec::new(),
            owner: foreign_owner(),
            executable: false,
            rent_epoch: 0,
        },
        Account {
            lamports: rent_exempt(1),
            data: vec![0x41],
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
        Account {
            lamports: 1,
            data: Vec::new(),
            owner: system_program::ID,
            executable: true,
            rent_epoch: 0,
        },
    ];

    for prestate in hostile {
        let mut world = OrderWorld::start(ReservationPrestate::Genesis(prestate.clone())).await;
        let before_target = account(&mut world.banks, world.reservation).await;
        let before_page = account(&mut world.banks, world.page).await;
        let before_position = account(&mut world.banks, world.position).await;
        let place = world.place();
        let result = try_send(
            &mut world.banks,
            &world.payer,
            &[budget(), place],
            &[&world.actor],
        )
        .await;
        assert!(result.is_err(), "semantic occupation must refuse");
        assert_account_image_eq(
            &account(&mut world.banks, world.reservation).await,
            &before_target,
        );
        assert_account_image_eq(&account(&mut world.banks, world.page).await, &before_page);
        assert_account_image_eq(
            &account(&mut world.banks, world.position).await,
            &before_position,
        );
    }
}

#[tokio::test]
async fn a_later_instruction_failure_rolls_reservation_topup_allocate_and_assign_back() {
    let mut world = OrderWorld::start(ReservationPrestate::Genesis(system_slot(1))).await;
    let before_target = account(&mut world.banks, world.reservation).await;
    let before_page = account(&mut world.banks, world.page).await;
    let before_position = account(&mut world.banks, world.position).await;
    let before_actor = account(&mut world.banks, world.actor.pubkey()).await;
    let place = world.place();
    let result = try_send(
        &mut world.banks,
        &world.payer,
        &[budget(), place.clone(), place],
        &[&world.actor],
    )
    .await;
    assert!(result.is_err(), "the duplicate instruction must refuse");
    assert_account_image_eq(
        &account(&mut world.banks, world.reservation).await,
        &before_target,
    );
    assert_account_image_eq(&account(&mut world.banks, world.page).await, &before_page);
    assert_account_image_eq(
        &account(&mut world.banks, world.position).await,
        &before_position,
    );
    assert_account_image_eq(
        &account(&mut world.banks, world.actor.pubkey()).await,
        &before_actor,
    );
}

/* --------------------------------------------------------------------- */
/* First-Endow owner-plane creation                                       */
/* --------------------------------------------------------------------- */

struct EndowWorld {
    banks: BanksClient,
    payer: Keypair,
    owner: Keypair,
    plane: Plane,
    owner_token: Address,
    position: Address,
    replay: Address,
}

impl EndowWorld {
    async fn start(genesis_prefund: bool, owner_tokens: u64) -> Self {
        let founder = creator_keypair();
        let owner = second_owner_keypair();
        let mint = collateral_mint_keypair();
        let plane = build_plane(founder.pubkey(), mint.pubkey(), MARKET_NONCE, Mode::Funded);
        let (position, replay) = plane.owner_plane(owner.pubkey());

        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.add_program("clutch_sbf", PROGRAM_ID, None);
        test.add_account(founder.pubkey(), system_slot(CREATOR_LAMPORTS));
        test.add_account(owner.pubkey(), system_slot(CREATOR_LAMPORTS));
        for item in &plane.accounts {
            test.add_account(
                item.address,
                Account {
                    lamports: rent_exempt(item.data.len()),
                    data: item.data.clone(),
                    owner: item.owner,
                    executable: false,
                    rent_epoch: 0,
                },
            );
        }
        let hoard_data = immutable_owner_account_bytes(
            mint.pubkey(),
            plane.hoard_authority.address,
            FUNDED_SETS + CASH_ATOMS,
        );
        test.add_account(
            plane.hoard_token.address,
            Account {
                lamports: rent_exempt(hoard_data.len()),
                data: hoard_data,
                owner: TOKEN_2022,
                executable: false,
                rent_epoch: 0,
            },
        );
        if genesis_prefund {
            test.add_account(position.address, system_slot(1));
            test.add_account(replay.address, system_slot(1));
        }

        let (mut banks, payer, _) = test.start().await;
        create_collateral_mint(&mut banks, &payer, &mint).await;
        let (_token_keypair, owner_token) =
            create_token_account(&mut banks, &payer, mint.pubkey(), owner.pubkey()).await;
        send(
            &mut banks,
            &payer,
            &[token_instruction::mint_to(
                &TOKEN_2022,
                &mint.pubkey(),
                &owner_token,
                &payer.pubkey(),
                &[],
                owner_tokens,
            )
            .unwrap()],
            &[],
        )
        .await;
        revoke_mint_authority(&mut banks, &payer, mint.pubkey()).await;

        Self {
            banks,
            payer,
            owner,
            plane,
            owner_token,
            position: position.address,
            replay: replay.address,
        }
    }

    fn endow(&self, amount: u64) -> Instruction {
        let metas = vec![
            AccountMeta::new(self.owner.pubkey(), true),
            AccountMeta::new_readonly(self.plane.market.address, false),
            AccountMeta::new_readonly(self.plane.hoard.address, false),
            AccountMeta::new(self.position, false),
            AccountMeta::new(self.replay, false),
            AccountMeta::new_readonly(self.plane.profile.address, false),
            AccountMeta::new_readonly(self.plane.policy_account, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.plane.collateral_mint, false),
            AccountMeta::new(self.owner_token, false),
            AccountMeta::new(self.plane.hoard_token.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(self.plane.terms.address, false),
            AccountMeta::new_readonly(self.plane.source_spec.address, false),
        ];
        assert_eq!(metas.len(), genesis::ENDOW_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::Endow {
                    market: self.plane.market_id,
                    owner: Hash32::from_bytes(self.owner.pubkey().to_bytes()),
                    amount,
                },
            ),
            metas,
        )
    }
}

/// The production-shaped default ELF has an empty source-release registry.
/// Even a canonical Terms/SourceSpec fixture and a pre-existing Market cannot
/// make it accept collateral: refusal occurs before owner-plane construction
/// or Token-2022, and the historical thirteen-account ABI is rejected too.
/// This failed-transaction ProgramTest is the authoritative rollback evidence:
/// unlike noncommitting RPC simulation, the bank processes the transaction and
/// lets the test reload complete Account images afterward.
#[cfg(not(feature = "non-production-mock-source"))]
#[tokio::test]
async fn default_elf_refuses_endow_without_a_registered_source_release() {
    let mut world = EndowWorld::start(true, 5).await;
    let watched = [
        world.position,
        world.replay,
        world.owner_token,
        world.plane.hoard_token.address,
        world.owner.pubkey(),
    ];
    let mut before = Vec::new();
    for address in watched {
        before.push(account(&mut world.banks, address).await);
    }

    let mut legacy = world.endow(1);
    legacy.accounts.truncate(13);
    let cases = [
        (legacy, clutch_sbf::error::ClutchError::AccountCount as u32),
        (
            world.endow(1),
            clutch_sbf::error::ClutchError::SourceReleaseUnavailable as u32,
        ),
    ];
    for (instruction, expected_code) in cases {
        let result = try_send(
            &mut world.banks,
            &world.payer,
            &[budget(), instruction],
            &[&world.owner],
        )
        .await;
        assert_eq!(
            result,
            Err(TransactionError::InstructionError(
                1,
                solana_instruction::error::InstructionError::Custom(expected_code),
            )),
        );
        for (index, address) in watched.iter().enumerate() {
            assert_account_image_eq(&account(&mut world.banks, *address).await, &before[index]);
        }
    }
}

#[cfg(feature = "non-production-mock-source")]
#[tokio::test]
async fn publicly_prefunded_second_owner_position_and_replay_are_created_by_first_endow() {
    let mut world = EndowWorld::start(false, 9).await;
    let position_prefund = rent_exempt(account_len::POSITION) + DONATION;
    let replay_prefund = rent_exempt(clutch_solana_reference::REPLAY_ACCOUNT_LEN) + DONATION;
    publicly_prefund(
        &mut world.banks,
        &world.payer,
        world.position,
        position_prefund,
    )
    .await;
    publicly_prefund(&mut world.banks, &world.payer, world.replay, replay_prefund).await;
    let owner_before = account(&mut world.banks, world.owner.pubkey())
        .await
        .lamports;
    let endow = world.endow(9);
    send(
        &mut world.banks,
        &world.payer,
        &[budget(), endow],
        &[&world.owner],
    )
    .await;

    let position_account = account(&mut world.banks, world.position).await;
    let replay_account = account(&mut world.banks, world.replay).await;
    assert_eq!(position_account.owner, PROGRAM_ID);
    assert_eq!(position_account.lamports, position_prefund);
    assert_eq!(replay_account.owner, PROGRAM_ID);
    assert_eq!(replay_account.lamports, replay_prefund);
    assert_eq!(
        account(&mut world.banks, world.owner.pubkey())
            .await
            .lamports,
        owner_before,
        "both over-rent targets require zero owner top-up"
    );
    let position = PositionAccount::decode(&position_account.data).unwrap();
    let replay = ReplayAccount::decode(&replay_account.data).unwrap();
    assert_eq!(
        position.owner,
        Hash32::from_bytes(world.owner.pubkey().to_bytes())
    );
    assert_eq!(position.cash_atoms, 9);
    assert_eq!(position.reserved_cash_atoms, 0);
    assert_eq!(replay.owner, position.owner);
    assert_eq!(replay.sequence, 1);
}

#[cfg(feature = "non-production-mock-source")]
#[tokio::test]
async fn late_token_failure_restores_one_lamport_owner_targets_byte_exactly() {
    let mut world = EndowWorld::start(true, 5).await;
    let watched = [
        world.position,
        world.replay,
        world.owner_token,
        world.plane.hoard_token.address,
        world.owner.pubkey(),
    ];
    let mut before = Vec::new();
    for address in watched {
        before.push(account(&mut world.banks, address).await);
    }
    let endow = world.endow(6);
    let result = try_send(
        &mut world.banks,
        &world.payer,
        &[budget(), endow],
        &[&world.owner],
    )
    .await;
    assert!(result.is_err(), "Token-2022 must refuse the overdraw");
    for (index, address) in watched.iter().enumerate() {
        assert_account_image_eq(&account(&mut world.banks, *address).await, &before[index]);
    }
    assert_eq!(before[0].lamports, 1);
    assert_eq!(before[0].owner, system_program::ID);
    assert!(before[0].data.is_empty());
    assert_eq!(before[1].lamports, 1);
    assert_eq!(before[1].owner, system_program::ID);
    assert!(before[1].data.is_empty());
}
