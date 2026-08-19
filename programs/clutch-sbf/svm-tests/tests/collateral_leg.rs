//! Market founding and the collateral leg, driven by an in-process Agave bank
//! against the **real** `clutch_sbf` ELF and the **real** Token-2022 program.
//!
//! This file exists because of two rows of the gap ledger in
//! `docs/implementation/TOKEN2022_PLAN.md` §0.1 that used to read "not
//! implemented" and "constructed, not wired":
//!
//! * **`CreateMarket` creates the mints.** Nothing in this program used to
//!   create an outcome mint or the Hoard's token account, which is why the
//!   token legs were optional. Here a market is founded from twelve all-zero
//!   accounts and comes out of the transaction with one Token-2022 mint per
//!   outcome — decimals `0`, mint authority the Market PDA, **no freeze
//!   authority**, no extensions — and a Hoard token account owned by the Hoard
//!   authority PDA and carrying `ImmutableOwner`. Every one of those facts is
//!   read back out of bytes the token program wrote.
//! * **The collateral leg moves real atoms.** `Split` transfers collateral in
//!   under the actor's propagated signature, `Merge` and `RedeemInternal`
//!   transfer it out under an `invoke_signed` for the Hoard authority, and
//!   `HoardAccount::collateral_atoms` equals the Hoard token account's `amount`
//!   after every one of them or the transaction refuses.
//!
//! What is still **not** established: an in-process bank is not a cluster.
//! Transaction replay, durable nonces, instruction duplication inside one
//! transaction, fee payment, rent collection over time and program upgrade are
//! outside what `solana-program-test` can show.

use {
    clutch_sbf::error::ClutchError,
    clutch_sbf::instructions::genesis,
    clutch_sbf::instructions::market_init,
    clutch_sbf::instructions::observe_resolve,
    clutch_sbf::instructions::split as seam,
    clutch_solana_layout::{Hash32, HoardAccount, Intent, PositionAccount, SupplyLedgerAccount},
    clutch_svm_fixture::{
        build_plane, compute_unit_limit_data, create_market_request, immutable_owner_account_bytes,
        layout_request, Mode, Plane, CASH_ATOMS, COMPUTE_BUDGET, FOUNDING_MARKET_NONCE,
        FUNDED_SETS, MARKET_NONCE, OUTCOME_COUNT, POLICY_ACCOUNT, PROGRAM_ID, TOKEN_2022,
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_pack::Pack,
    solana_program_test::{tokio, BanksClient, ProgramTest},
    solana_signer::Signer,
    solana_system_interface::instruction as system_instruction,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
    spl_token_2022_interface::{
        error::TokenError,
        extension::{BaseStateWithExtensions, ExtensionType, StateWithExtensions},
        instruction as token_instruction,
        instruction::AuthorityType,
        state::{Account as TokenAccount, Mint},
    },
};

/// Collateral the actor is minted before anything else happens.
///
/// Deliberately **smaller** than the founding position's free cash (80), so
/// that a `Split` exists which the kernel admits and the token program then
/// refuses — which is the only way to reach the post-CPI rollback of E5.
const ACTOR_COLLATERAL: u64 = 50;
/// The collateral mint's decimals; the Realm's policy fixes it.
const COLLATERAL_DECIMALS: u8 = 6;

/// A deterministic actor and a deterministic collateral mint.
///
/// `find_program_address` searches downward from bump 255, so a random keypair
/// costs a different number of iterations every run and the compute-unit
/// figures below would drift by thousands between runs for no visible reason.
fn actor_keypair() -> Keypair {
    Keypair::new_from_array([
        0x3a, 0x91, 0x0d, 0x64, 0x22, 0xb8, 0x57, 0xe1, 0x4c, 0x05, 0xf3, 0x7a, 0x98, 0x11, 0x2d,
        0x6b, 0x40, 0xcf, 0x83, 0x19, 0x75, 0xa2, 0x0e, 0xd4, 0x66, 0x37, 0xbb, 0x52, 0x08, 0xe9,
        0x71, 0xc3,
    ])
}

fn collateral_mint_keypair() -> Keypair {
    Keypair::new_from_array([
        0x5c, 0x2e, 0x81, 0x40, 0x1b, 0xd7, 0x66, 0x0a, 0x93, 0x14, 0xaf, 0x25, 0x7e, 0x38, 0xc1,
        0x09, 0x6d, 0xb2, 0x44, 0xf8, 0x1c, 0x53, 0x90, 0x2a, 0xe7, 0x0b, 0x31, 0xd6, 0x88, 0x4e,
        0x17, 0xba,
    ])
}

struct Scenario {
    banks: BanksClient,
    payer: Keypair,
    actor: Keypair,
    plane: Plane,
    /// The actor's own Token-2022 account for the collateral mint.
    actor_collateral: Address,
}

impl Scenario {
    async fn start(nonce: u64, mode: Mode) -> Self {
        let actor = actor_keypair();
        let mint = collateral_mint_keypair();
        let plane = build_plane(actor.pubkey(), mint.pubkey(), nonce, mode);

        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.add_program("clutch_sbf", PROGRAM_ID, None);
        test.add_account(
            actor.pubkey(),
            Account {
                lamports: 10_000_000_000,
                data: Vec::new(),
                owner: solana_system_interface::program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
        for account in &plane.accounts {
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

        /* A pre-funded market already holds collateral, and its Hoard token
         * account lives at a program-derived address nothing outside the
         * program can create — so for `Mode::Funded` and `Mode::Resolved` it is
         * installed at genesis with exactly `HoardAccount::collateral_atoms`,
         * which is the mirror the first instruction re-checks.  The founding
         * scenario installs nothing there: `CreateMarket` creates it. */
        if !matches!(mode, Mode::Empty) {
            let data = immutable_owner_account_bytes(
                mint.pubkey(),
                plane.hoard_authority.address,
                plane
                    .hoard_atoms
                    .checked_add(CASH_ATOMS)
                    .expect("fixture custody fits u64"),
            );
            test.add_account(
                plane.hoard_token.address,
                Account {
                    lamports: rent_exempt(data.len()),
                    data,
                    owner: TOKEN_2022,
                    executable: false,
                    rent_epoch: 0,
                },
            );
            for outcome in &plane.outcome_mints {
                let data = clutch_svm_fixture::outcome_mint_bytes(plane.market.address, 0);
                test.add_account(
                    outcome.address,
                    Account {
                        lamports: rent_exempt(data.len()),
                        data,
                        owner: TOKEN_2022,
                        executable: false,
                        rent_epoch: 0,
                    },
                );
            }
        }

        let (banks, payer, _blockhash) = test.start().await;
        let mut scenario = Self {
            banks,
            payer,
            actor,
            plane,
            actor_collateral: Address::default(),
        };
        scenario.create_collateral_mint(&mint).await;
        scenario.actor_collateral = scenario
            .create_token_account(mint.pubkey(), scenario.actor.pubkey())
            .await;
        scenario
            .mint_collateral(&mint, scenario.actor_collateral, ACTOR_COLLATERAL)
            .await;
        scenario.revoke_mint_authority(&mint).await;
        scenario
    }

    /// Create the collateral mint the Realm's frozen policy names.
    ///
    /// Its bytes are Token-2022's, not this harness's, and its *address* is
    /// load-bearing far beyond the mint: the policy names it, the Profile
    /// identity is the parent hash over the policy's digest, and every PDA in
    /// the plane descends from that. A different mint would be a different
    /// market.
    async fn create_collateral_mint(&mut self, mint: &Keypair) {
        let space = Mint::LEN;
        let lamports = self.rent_for(space).await;
        let payer = self.payer.pubkey();
        self.send(
            &[
                system_instruction::create_account(
                    &payer,
                    &mint.pubkey(),
                    lamports,
                    space as u64,
                    &TOKEN_2022,
                ),
                token_instruction::initialize_mint2(
                    &TOKEN_2022,
                    &mint.pubkey(),
                    &payer,
                    None,
                    COLLATERAL_DECIMALS,
                )
                .unwrap(),
            ],
            &[mint],
        )
        .await;
    }

    async fn mint_collateral(&mut self, mint: &Keypair, destination: Address, amount: u64) {
        let payer = self.payer.pubkey();
        self.send(
            &[token_instruction::mint_to(
                &TOKEN_2022,
                &mint.pubkey(),
                &destination,
                &payer,
                &[],
                amount,
            )
            .unwrap()],
            &[],
        )
        .await;
    }

    /// Revoke the mint authority, which the Realm's V1 policy requires absent.
    async fn revoke_mint_authority(&mut self, mint: &Keypair) {
        let payer = self.payer.pubkey();
        self.send(
            &[token_instruction::set_authority(
                &TOKEN_2022,
                &mint.pubkey(),
                None,
                AuthorityType::MintTokens,
                &payer,
                &[],
            )
            .unwrap()],
            &[],
        )
        .await;
    }

    async fn create_token_account(&mut self, mint: Address, owner: Address) -> Address {
        let account = Keypair::new();
        let space = TokenAccount::LEN;
        let lamports = self.rent_for(space).await;
        let payer = self.payer.pubkey();
        self.send(
            &[
                system_instruction::create_account(
                    &payer,
                    &account.pubkey(),
                    lamports,
                    space as u64,
                    &TOKEN_2022,
                ),
                token_instruction::initialize_account3(
                    &TOKEN_2022,
                    &account.pubkey(),
                    &mint,
                    &owner,
                )
                .unwrap(),
            ],
            &[&account],
        )
        .await;
        account.pubkey()
    }

    async fn rent_for(&mut self, space: usize) -> u64 {
        self.banks
            .get_rent()
            .await
            .unwrap()
            .minimum_balance(space)
            .max(1)
    }

    async fn send(&mut self, instructions: &[Instruction], signers: &[&Keypair]) -> u64 {
        let (result, units) = self.try_send(instructions, signers).await;
        result.expect("transaction was expected to succeed");
        units
    }

    async fn try_send(
        &mut self,
        instructions: &[Instruction],
        signers: &[&Keypair],
    ) -> (Result<(), TransactionError>, u64) {
        let blockhash = self.banks.get_latest_blockhash().await.unwrap();
        let mut all: Vec<&Keypair> = vec![&self.payer];
        all.extend_from_slice(signers);
        let transaction = Transaction::new_signed_with_payer(
            instructions,
            Some(&self.payer.pubkey()),
            &all,
            blockhash,
        );
        let outcome = self
            .banks
            .process_transaction_with_metadata(transaction)
            .await
            .unwrap();
        let units = outcome
            .metadata
            .map(|m| m.compute_units_consumed)
            .unwrap_or_default();
        (outcome.result, units)
    }

    async fn refusal_code(&mut self, instructions: &[Instruction], signers: &[&Keypair]) -> u32 {
        let (result, _) = self.try_send(instructions, signers).await;
        match result {
            Ok(()) => panic!("transaction was expected to be refused, and was not"),
            Err(TransactionError::InstructionError(_, error)) => match error {
                solana_instruction::error::InstructionError::Custom(code) => code,
                other => panic!("expected a custom program error, got {other:?}"),
            },
            Err(other) => panic!("expected an instruction error, got {other:?}"),
        }
    }

    async fn data(&mut self, address: Address) -> Vec<u8> {
        self.banks
            .get_account(address)
            .await
            .unwrap()
            .expect("account should exist")
            .data
    }

    async fn maybe_data(&mut self, address: Address) -> Option<Vec<u8>> {
        self.banks
            .get_account(address)
            .await
            .unwrap()
            .map(|account| account.data)
    }

    async fn amount(&mut self, address: Address) -> u64 {
        let data = self.data(address).await;
        StateWithExtensions::<TokenAccount>::unpack(&data)
            .expect("the token program's own account must unpack")
            .base
            .amount
    }

    async fn hoard_atoms(&mut self) -> u64 {
        let data = self.data(self.plane.hoard.address).await;
        HoardAccount::decode(&data)
            .expect("hoard decodes")
            .collateral_atoms
    }

    async fn position(&mut self) -> PositionAccount {
        let data = self.data(self.plane.position.address).await;
        PositionAccount::decode(&data).expect("position decodes")
    }

    /// The sixteen-account `Split`/`Merge` instruction.
    fn collateral_seam(&self, request: Vec<u8>) -> Instruction {
        let state = self.plane.seam_addresses();
        let mut metas = vec![
            AccountMeta::new(state[0], true),
            AccountMeta::new_readonly(state[1], false),
            AccountMeta::new_readonly(state[2], false),
        ];
        for address in &state[3..] {
            metas.push(AccountMeta::new(*address, false));
        }
        let leg = self.plane.collateral_leg(self.actor_collateral);
        metas.push(AccountMeta::new_readonly(leg[0], false));
        metas.push(AccountMeta::new_readonly(leg[1], false));
        metas.push(AccountMeta::new_readonly(leg[2], false));
        metas.push(AccountMeta::new(leg[3], false));
        metas.push(AccountMeta::new_readonly(leg[4], false));
        metas.push(AccountMeta::new(leg[5], false));
        assert_eq!(metas.len(), seam::ACCOUNT_COUNT_COLLATERAL);
        Instruction::new_with_bytes(PROGRAM_ID, &request, metas)
    }

    fn split(&self, sequence: u64, quantity: u64) -> Instruction {
        self.collateral_seam(layout_request(
            sequence,
            Intent::Split {
                market: self.plane.market_id,
                owner: Hash32::from_bytes(self.actor.pubkey().to_bytes()),
                quantity,
            },
        ))
    }

    fn merge(&self, sequence: u64, quantity: u64) -> Instruction {
        self.collateral_seam(layout_request(
            sequence,
            Intent::Merge {
                market: self.plane.market_id,
                owner: Hash32::from_bytes(self.actor.pubkey().to_bytes()),
                quantity,
            },
        ))
    }

    /// The eleven-account backed `Endow` deposit.
    fn endow(&self, sequence: u64, amount: u64) -> Instruction {
        let request = layout_request(
            sequence,
            Intent::Endow {
                market: self.plane.market_id,
                owner: Hash32::from_bytes(self.actor.pubkey().to_bytes()),
                amount,
            },
        );
        let metas = vec![
            AccountMeta::new_readonly(self.actor.pubkey(), true),
            AccountMeta::new_readonly(self.plane.market.address, false),
            AccountMeta::new_readonly(self.plane.hoard.address, false),
            AccountMeta::new(self.plane.position.address, false),
            AccountMeta::new(self.plane.replay.address, false),
            AccountMeta::new_readonly(self.plane.profile.address, false),
            AccountMeta::new_readonly(POLICY_ACCOUNT, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.plane.collateral_mint, false),
            AccountMeta::new(self.actor_collateral, false),
            AccountMeta::new(self.plane.hoard_token.address, false),
        ];
        assert_eq!(metas.len(), genesis::ENDOW_ACCOUNT_COUNT);
        Instruction::new_with_bytes(PROGRAM_ID, &request, metas)
    }

    /// The thirteen-account `Materialize`/`Dematerialize` instruction.
    fn outcome_seam(&self, request: Vec<u8>, outcome: usize, holder: Address) -> Instruction {
        let state = self.plane.seam_addresses();
        let mut metas = vec![
            AccountMeta::new(state[0], true),
            AccountMeta::new_readonly(state[1], false),
            AccountMeta::new_readonly(state[2], false),
        ];
        for address in &state[3..] {
            metas.push(AccountMeta::new(*address, false));
        }
        let leg = self.plane.outcome_leg(outcome, holder);
        metas.push(AccountMeta::new_readonly(leg[0], false));
        metas.push(AccountMeta::new(leg[1], false));
        metas.push(AccountMeta::new(leg[2], false));
        assert_eq!(metas.len(), seam::ACCOUNT_COUNT_OUTCOME);
        Instruction::new_with_bytes(PROGRAM_ID, &request, metas)
    }

    fn materialize(
        &self,
        sequence: u64,
        outcome: u8,
        quantity: u64,
        holder: Address,
    ) -> Instruction {
        self.outcome_seam(
            layout_request(
                sequence,
                Intent::Materialize {
                    market: self.plane.market_id,
                    owner: Hash32::from_bytes(self.actor.pubkey().to_bytes()),
                    destination: Hash32::from_bytes(self.plane.external.address.to_bytes()),
                    outcome,
                    quantity,
                },
            ),
            usize::from(outcome),
            holder,
        )
    }

    fn dematerialize(
        &self,
        sequence: u64,
        outcome: u8,
        quantity: u64,
        holder: Address,
    ) -> Instruction {
        self.outcome_seam(
            layout_request(
                sequence,
                Intent::Dematerialize {
                    market: self.plane.market_id,
                    owner: Hash32::from_bytes(self.actor.pubkey().to_bytes()),
                    source: Hash32::from_bytes(self.plane.external.address.to_bytes()),
                    outcome,
                    quantity,
                },
            ),
            usize::from(outcome),
            holder,
        )
    }

    async fn mint_supply(&mut self, mint: Address) -> u64 {
        let data = self.data(mint).await;
        StateWithExtensions::<Mint>::unpack(&data)
            .expect("the token program's own mint must unpack")
            .base
            .supply
    }

    async fn external_supply(&mut self, outcome: usize) -> u64 {
        let data = self.data(self.plane.supply.address).await;
        SupplyLedgerAccount::decode(&data)
            .expect("ledger decodes")
            .external_supply[outcome]
    }

    /// The nineteen-account `RedeemInternal` instruction.
    fn redeem(&self, sequence: u64, outcome: u8, quantity: u64) -> Instruction {
        let evidence = self.plane.evidence_addresses();
        let mut metas = vec![AccountMeta::new(evidence[0], true)];
        for address in &evidence[1..8] {
            metas.push(AccountMeta::new(*address, false));
        }
        for address in &evidence[8..12] {
            metas.push(AccountMeta::new_readonly(*address, false));
        }
        let leg = self.plane.redeem_leg(self.actor_collateral);
        metas.push(AccountMeta::new_readonly(leg[0], false));
        metas.push(AccountMeta::new_readonly(leg[1], false));
        metas.push(AccountMeta::new_readonly(leg[2], false));
        metas.push(AccountMeta::new_readonly(leg[3], false));
        metas.push(AccountMeta::new(leg[4], false));
        metas.push(AccountMeta::new_readonly(leg[5], false));
        metas.push(AccountMeta::new(leg[6], false));
        assert_eq!(metas.len(), observe_resolve::REDEEM_ACCOUNT_COUNT);
        let mut data = vec![0xd1_u8, 1];
        data.extend_from_slice(&sequence.to_le_bytes());
        data.push(2); // ACTION_REDEEM_INTERNAL
        data.push(outcome);
        data.extend_from_slice(&quantity.to_le_bytes());
        Instruction::new_with_bytes(PROGRAM_ID, &data, metas)
    }

    /// The `CreateMarket` instruction, at its exact account count.
    fn create_market(&self, nonce: u64) -> Instruction {
        let addresses = self.plane.create_market_addresses(self.actor.pubkey());
        let mut metas = vec![AccountMeta::new(addresses[0], true)];
        for (index, address) in addresses.iter().enumerate().skip(1) {
            let writable = matches!(index, 4..=11) || index >= market_init::IX_HOARD_TOKEN;
            metas.push(if writable {
                AccountMeta::new(*address, false)
            } else {
                AccountMeta::new_readonly(*address, false)
            });
        }
        assert_eq!(metas.len(), market_init::account_count(OUTCOME_COUNT));
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &create_market_request(&self.plane, nonce),
            metas,
        )
    }
}

/// Unit ceiling for a seam or redemption transaction.
///
/// The 200 000-unit default is not enough any more and the reason is worth
/// stating: the collateral leg binds the Realm's 266 policy bytes by
/// recomputed digest, and `clutch-solana-layout` is a dependency-free crate
/// with its **own software SHA-256** rather than the `sol_sha256` syscall.
/// Two digests per instruction is thousands of units the outcome leg never
/// paid.  Raising the ceiling here measures the cost rather than hiding it;
/// moving those digests onto the syscall is an obligation on the layout crate,
/// not something this lane may do to a frozen codec.
const SEAM_UNITS: u32 = 600_000;
/// Unit ceiling for `CreateMarket`, which recomputes the terms digest twice
/// over a multi-kilobyte body *and* performs seven CPIs.
const CREATE_UNITS: u32 = 1_400_000;

fn budget(units: u32) -> Instruction {
    Instruction::new_with_bytes(COMPUTE_BUDGET, &compute_unit_limit_data(units), Vec::new())
}

fn rent_exempt(space: usize) -> u64 {
    solana_rent::Rent::default().minimum_balance(space).max(1)
}

/// **`CreateMarket` creates the mints and the Hoard token account.**
///
/// The gap-ledger row that used to read "not implemented". A market founded
/// from twelve all-zero accounts comes out of one transaction with one
/// Token-2022 mint per outcome and a Hoard token account, and every property
/// the plan's §3.1 table demands of them is read back from the bytes the token
/// program wrote — not from what this program intended to write.
#[tokio::test]
async fn create_market_founds_the_outcome_mints_and_the_hoard_token_account() {
    let mut scenario = Scenario::start(FOUNDING_MARKET_NONCE, Mode::Empty).await;

    // Nothing exists at those addresses before the transaction.
    for mint in scenario.plane.outcome_mints.clone() {
        assert!(
            scenario
                .maybe_data(mint.address)
                .await
                .is_none_or(|data| data.is_empty()),
            "an outcome mint must not exist before CreateMarket"
        );
    }
    let hoard_token = scenario.plane.hoard_token.address;
    assert!(scenario
        .maybe_data(hoard_token)
        .await
        .is_none_or(|data| data.is_empty()));

    let actor = scenario.actor.insecure_clone();
    let units = scenario
        .send(
            &[
                budget(CREATE_UNITS),
                scenario.create_market(FOUNDING_MARKET_NONCE),
            ],
            &[&actor],
        )
        .await;

    let market = scenario.plane.market.address;
    for (index, mint) in scenario.plane.outcome_mints.clone().iter().enumerate() {
        let data = scenario.data(mint.address).await;
        let state = StateWithExtensions::<Mint>::unpack(&data)
            .expect("the token program's own mint must unpack");
        assert_eq!(state.base.decimals, 0, "outcome mint {index}: decimals");
        assert_eq!(
            Option::<Address>::from(state.base.mint_authority),
            Some(market),
            "outcome mint {index}: the Market PDA is the only minter"
        );
        assert_eq!(
            Option::<Address>::from(state.base.freeze_authority),
            None,
            "outcome mint {index}: a freeze authority is discretionary seizure"
        );
        assert_eq!(
            state.base.supply, 0,
            "outcome mint {index}: founding supply"
        );
        assert!(state.base.is_initialized);
        assert_eq!(
            state.get_extension_types().unwrap(),
            Vec::<ExtensionType>::new(),
            "outcome mint {index}: admission by construction means no extension"
        );
    }

    let data = scenario.data(hoard_token).await;
    let state = StateWithExtensions::<TokenAccount>::unpack(&data)
        .expect("the token program's own account must unpack");
    assert_eq!(state.base.mint, scenario.plane.collateral_mint);
    assert_eq!(
        state.base.owner, scenario.plane.hoard_authority.address,
        "the Hoard's owner authority is a program address, which is its whole security story"
    );
    assert_eq!(state.base.amount, 0);
    assert_eq!(Option::<Address>::from(state.base.delegate), None);
    assert_eq!(Option::<Address>::from(state.base.close_authority), None);
    assert_eq!(
        state.get_extension_types().unwrap(),
        vec![ExtensionType::ImmutableOwner],
        "SetAuthority(AccountOwner) is exactly what ImmutableOwner forbids"
    );

    // And the founding mirror holds at zero.
    assert_eq!(scenario.hoard_atoms().await, 0);
    assert_eq!(scenario.amount(hoard_token).await, 0);

    println!("SVM create_market: 21 accounts, 2 outcome mints + hoard token, {units} CU");
}

/// **A second founding at the same address is refused, and creates nothing.**
#[tokio::test]
async fn founding_a_market_twice_refuses() {
    let mut scenario = Scenario::start(FOUNDING_MARKET_NONCE, Mode::Empty).await;
    let actor = scenario.actor.insecure_clone();
    scenario
        .send(
            &[
                budget(CREATE_UNITS),
                scenario.create_market(FOUNDING_MARKET_NONCE),
            ],
            &[&actor],
        )
        .await;
    let code = scenario
        .refusal_code(
            &[
                budget(CREATE_UNITS),
                scenario.create_market(FOUNDING_MARKET_NONCE),
            ],
            &[&actor],
        )
        .await;
    assert_eq!(
        code,
        ClutchError::AlreadyInitialized as u32,
        "a market that already exists has nonzero bytes at its canonical address"
    );
    println!("SVM create_market idempotence: second founding refused with Custom({code}) (0x0040)");
}

/// **E2 — pooled-custody reclassification, with real Token-2022 accounts.**
///
/// `Split` locks cash already inside the Hoard and `Merge` unlocks it. Neither
/// may charge or pay the actor a second time, and custody stays constant.
#[tokio::test]
async fn split_and_merge_reclassify_without_a_second_token_transfer() {
    let mut scenario = Scenario::start(MARKET_NONCE, Mode::Funded).await;
    let actor = scenario.actor.insecure_clone();
    let hoard_token = scenario.plane.hoard_token.address;
    let actor_token = scenario.actor_collateral;

    assert_eq!(scenario.amount(actor_token).await, ACTOR_COLLATERAL);
    let custody = FUNDED_SETS + CASH_ATOMS;
    assert_eq!(scenario.amount(hoard_token).await, custody);
    assert_eq!(scenario.hoard_atoms().await, FUNDED_SETS);

    let split_units = scenario
        .send(&[budget(SEAM_UNITS), scenario.split(0, 12)], &[&actor])
        .await;
    assert_eq!(scenario.amount(actor_token).await, ACTOR_COLLATERAL);
    assert_eq!(scenario.amount(hoard_token).await, custody);
    assert_eq!(scenario.hoard_atoms().await, FUNDED_SETS + 12);
    assert_eq!(scenario.position().await.internal[0], FUNDED_SETS + 12);

    let merge_units = scenario
        .send(&[budget(SEAM_UNITS), scenario.merge(1, 5)], &[&actor])
        .await;
    assert_eq!(scenario.amount(actor_token).await, ACTOR_COLLATERAL);
    assert_eq!(scenario.amount(hoard_token).await, custody);
    assert_eq!(scenario.hoard_atoms().await, FUNDED_SETS + 7);
    assert_eq!(scenario.position().await.internal[1], FUNDED_SETS + 7);

    println!("SVM collateral leg: split={split_units} CU  merge={merge_units} CU");
}

/// `Endow` is the one inbound collateral boundary.
#[tokio::test]
async fn endow_debits_the_owner_and_credits_cash_and_custody_exactly() {
    let mut scenario = Scenario::start(MARKET_NONCE, Mode::Funded).await;
    let actor = scenario.actor.insecure_clone();
    let actor_token = scenario.actor_collateral;
    let hoard_token = scenario.plane.hoard_token.address;
    let pre_cash = scenario.position().await.cash_atoms;
    let pre_custody = scenario.amount(hoard_token).await;

    let units = scenario
        .send(&[budget(SEAM_UNITS), scenario.endow(0, 10)], &[&actor])
        .await;

    assert_eq!(scenario.amount(actor_token).await, ACTOR_COLLATERAL - 10);
    assert_eq!(scenario.amount(hoard_token).await, pre_custody + 10);
    assert_eq!(scenario.position().await.cash_atoms, pre_cash + 10);
    assert_eq!(scenario.hoard_atoms().await, FUNDED_SETS);
    println!("SVM Endow: exact 10-atom deposit, {units} CU");
}

/// **The whole cycle, with both legs and a holder who is not the actor.**
///
/// `Split` puts real collateral into the Hoard, `Materialize` mints real
/// outcome tokens, **those tokens are transferred to a second wallet outside
/// this program**, `Dematerialize` burns what is left, and `Merge` takes real
/// collateral back out. Two token programs' worth of arithmetic and one
/// kernel's, required to agree at every step.
///
/// The external transfer is the part worth having: it is what makes the
/// *per-owner* external shadow wrong while the market-wide term stays right,
/// which is the argument `TOKEN2022_PLAN.md` §0.3 makes for reconciling the
/// aggregate and not the per-owner balance. A holder may keep outcome tokens
/// anywhere and give them to anybody; the mint's `supply` is the only
/// counterpart the ledger has.
#[tokio::test]
async fn the_whole_cycle_survives_a_holder_who_is_not_the_actor() {
    let mut scenario = Scenario::start(MARKET_NONCE, Mode::Funded).await;
    let actor = scenario.actor.insecure_clone();
    let outcome_mint = scenario.plane.outcome_mints[0].address;
    let holder = scenario
        .create_token_account(outcome_mint, actor.pubkey())
        .await;
    let stranger = Keypair::new();
    let elsewhere = scenario
        .create_token_account(outcome_mint, stranger.pubkey())
        .await;

    let hoard_token = scenario.plane.hoard_token.address;
    let actor_token = scenario.actor_collateral;

    // 1. Pooled cash becomes locked collateral; no second deposit occurs.
    let custody = FUNDED_SETS + CASH_ATOMS;
    let split_units = scenario
        .send(&[budget(SEAM_UNITS), scenario.split(0, 12)], &[&actor])
        .await;
    assert_eq!(scenario.amount(actor_token).await, ACTOR_COLLATERAL);
    assert_eq!(scenario.amount(hoard_token).await, custody);

    // 2. Claims become tokens.
    let materialize_units = scenario
        .send(
            &[budget(SEAM_UNITS), scenario.materialize(1, 0, 7, holder)],
            &[&actor],
        )
        .await;
    assert_eq!(scenario.mint_supply(outcome_mint).await, 7);
    assert_eq!(scenario.amount(holder).await, 7);

    // 3. Three of them leave, with no clutch instruction involved at all.
    scenario
        .send(
            &[token_instruction::transfer_checked(
                &TOKEN_2022,
                &holder,
                &outcome_mint,
                &elsewhere,
                &actor.pubkey(),
                &[],
                3,
                0,
            )
            .unwrap()],
            &[&actor],
        )
        .await;
    assert_eq!(scenario.amount(elsewhere).await, 3);
    assert_eq!(scenario.amount(holder).await, 4);
    assert_eq!(
        scenario.mint_supply(outcome_mint).await,
        7,
        "a transfer conserves supply, which is why the aggregate is the reconcilable term"
    );

    // 4. What the actor still holds goes back to internal claims.
    let dematerialize_units = scenario
        .send(
            &[budget(SEAM_UNITS), scenario.dematerialize(2, 0, 4, holder)],
            &[&actor],
        )
        .await;
    assert_eq!(scenario.amount(holder).await, 0);
    assert_eq!(
        scenario.mint_supply(outcome_mint).await,
        3,
        "the three tokens the stranger holds are the whole remaining supply"
    );
    assert_eq!(
        scenario.external_supply(0).await,
        3,
        "and the market-wide external term equals it, which the program enforced"
    );

    // 5. Locked collateral returns to pooled cash; this is not a withdrawal.
    let position = scenario.position().await;
    assert_eq!(position.internal[0], FUNDED_SETS + 12 - 7 + 4);
    assert_eq!(position.internal[1], FUNDED_SETS + 12);
    let merge_units = scenario
        .send(&[budget(SEAM_UNITS), scenario.merge(3, 9)], &[&actor])
        .await;
    assert_eq!(scenario.amount(actor_token).await, ACTOR_COLLATERAL);
    assert_eq!(scenario.amount(hoard_token).await, custody);
    assert_eq!(scenario.hoard_atoms().await, FUNDED_SETS + 3);

    println!(
        "SVM full cycle: split={split_units} CU  materialize={materialize_units} CU  \
         dematerialize={dematerialize_units} CU  merge={merge_units} CU  \
         (3 outcome tokens left with a third party and the aggregate still reconciles)"
    );
}

/// A failed deposit cannot mint ledger cash or consume replay.
///
/// `Endow` validates its ledger post-state before invoking Token-2022 but
/// writes it only after the exact debit and credit are observed.  An amount
/// above the actor's balance therefore leaves both protocol accounts and both
/// token accounts byte-identical.
#[tokio::test]
async fn a_failed_endow_leaves_ledger_replay_and_tokens_unchanged() {
    let mut scenario = Scenario::start(MARKET_NONCE, Mode::Funded).await;
    let actor = scenario.actor.insecure_clone();

    let watched: Vec<Address> = vec![
        scenario.plane.position.address,
        scenario.plane.replay.address,
        scenario.plane.hoard_token.address,
        scenario.actor_collateral,
    ];
    let mut before = Vec::new();
    for address in &watched {
        before.push(scenario.data(*address).await);
    }

    let (result, _) = scenario
        .try_send(
            &[
                budget(SEAM_UNITS),
                scenario.endow(0, ACTOR_COLLATERAL + 1),
            ],
            &[&actor],
        )
        .await;
    assert!(result.is_err(), "an unfunded deposit must fail");

    for (index, address) in watched.iter().enumerate() {
        let now = scenario.data(*address).await;
        assert_eq!(
            now, before[index],
            "account {index} changed across a refused deposit"
        );
    }
    println!("SVM Endow rollback: ledger, replay, actor token, and Hoard token unchanged");
}

/// **The wallet-signed outflow refusal, on the collateral path.**
///
/// The Hoard's token account is at the address `seeds::hoard_token_pda`
/// derives and its owner authority is the address `seeds::hoard_authority_pda`
/// derives. A wallet that signs for itself cannot take a single atom out of it;
/// only an `invoke_signed` for the Hoard authority seeds can, which is what
/// `Merge` and `RedeemInternal` do.
#[tokio::test]
async fn no_wallet_signature_can_take_collateral_out_of_the_hoard() {
    let mut scenario = Scenario::start(MARKET_NONCE, Mode::Funded).await;
    let actor = scenario.actor.insecure_clone();
    let hoard_token = scenario.plane.hoard_token.address;
    let mint = scenario.plane.collateral_mint;

    // A deposit needs only the depositor's signature.
    scenario
        .send(
            &[token_instruction::transfer_checked(
                &TOKEN_2022,
                &scenario.actor_collateral,
                &mint,
                &hoard_token,
                &actor.pubkey(),
                &[],
                5,
                COLLATERAL_DECIMALS,
            )
            .unwrap()],
            &[&actor],
        )
        .await;
    let donated_custody = FUNDED_SETS + CASH_ATOMS + 5;
    assert_eq!(scenario.amount(hoard_token).await, donated_custody);

    // Taking it back out with the same signature is impossible.
    let code = scenario
        .refusal_code(
            &[token_instruction::transfer_checked(
                &TOKEN_2022,
                &hoard_token,
                &mint,
                &scenario.actor_collateral,
                &actor.pubkey(),
                &[],
                5,
                COLLATERAL_DECIMALS,
            )
            .unwrap()],
            &[&actor],
        )
        .await;
    assert_eq!(code, TokenError::OwnerMismatch as u32);

    /* The unsolicited deposit is harmless surplus: it creates no position
     * cash claim and cannot grief later reclassification. */
    scenario
        .send(&[budget(SEAM_UNITS), scenario.split(0, 1)], &[&actor])
        .await;
    assert_eq!(scenario.amount(hoard_token).await, donated_custody);
    assert_eq!(scenario.hoard_atoms().await, FUNDED_SETS + 1);
    println!(
        "SVM hoard outflow: wallet withdrawal refused with Custom({code}); \
         donated surplus did not block Split"
    );
}

/// **Redemption credits pooled cash without withdrawing it.**
///
/// A winning claim lowers locked collateral and raises position cash by the
/// same amount. Token custody and the actor's wallet remain unchanged.
#[tokio::test]
async fn redeem_reclassifies_locked_collateral_into_position_cash() {
    let mut scenario = Scenario::start(MARKET_NONCE, Mode::Resolved(0)).await;
    let actor = scenario.actor.insecure_clone();
    let hoard_token = scenario.plane.hoard_token.address;
    let actor_token = scenario.actor_collateral;

    let quantity = 6;
    let units = scenario
        .send(
            &[budget(SEAM_UNITS), scenario.redeem(0, 0, quantity)],
            &[&actor],
        )
        .await;

    /* Payout vector 0 is `[1, 0]` with denominator 1, so six winning claims
     * pay six atoms. */
    let custody = FUNDED_SETS + CASH_ATOMS;
    assert_eq!(scenario.amount(actor_token).await, ACTOR_COLLATERAL);
    assert_eq!(scenario.amount(hoard_token).await, custody);
    assert_eq!(scenario.hoard_atoms().await, FUNDED_SETS - 6);
    let position = scenario.position().await;
    assert_eq!(position.internal[0], FUNDED_SETS - quantity);
    assert_eq!(position.cash_atoms, CASH_ATOMS + quantity);

    println!("SVM redeem: 19 accounts, payout={quantity} atoms, {units} CU");

    /* The losing outcome pays zero and custody still does not move. */
    let hoard_before = scenario.amount(hoard_token).await;
    let zero_units = scenario
        .send(&[budget(SEAM_UNITS), scenario.redeem(1, 1, 4)], &[&actor])
        .await;
    assert_eq!(scenario.amount(hoard_token).await, hoard_before);
    assert_eq!(scenario.amount(actor_token).await, ACTOR_COLLATERAL);
    println!("SVM redeem (losing claim): payout=0 atoms, {zero_units} CU");
}

/// **The token leg is not optional any more.**
///
/// The ten-account seam plane that used to be accepted — and that
/// `programs/clutch-sbf/harness` still emits — is refused, and so is every
/// count that is not the one the intent names.
#[tokio::test]
async fn the_shadow_only_plane_is_gone() {
    let mut scenario = Scenario::start(MARKET_NONCE, Mode::Funded).await;
    let actor = scenario.actor.insecure_clone();
    let request = layout_request(
        0,
        Intent::Split {
            market: scenario.plane.market_id,
            owner: Hash32::from_bytes(actor.pubkey().to_bytes()),
            quantity: 4,
        },
    );
    let state = scenario.plane.seam_addresses();
    let mut metas = vec![
        AccountMeta::new(state[0], true),
        AccountMeta::new_readonly(state[1], false),
        AccountMeta::new_readonly(state[2], false),
    ];
    for address in &state[3..] {
        metas.push(AccountMeta::new(*address, false));
    }
    assert_eq!(metas.len(), seam::ACCOUNT_COUNT);
    let code = scenario
        .refusal_code(
            &[
                budget(SEAM_UNITS),
                Instruction::new_with_bytes(PROGRAM_ID, &request, metas),
            ],
            &[&actor],
        )
        .await;
    assert_eq!(
        code,
        ClutchError::AccountCount as u32,
        "a ten-account Split moved collateral nowhere and is now refused"
    );
    println!("SVM mandatory leg: the ten-account Split refused with Custom({code}) (0x0001)");
}

/// A `Split` whose collateral account belongs to somebody else is refused.
#[tokio::test]
async fn a_stranger_cannot_fund_a_split_from_an_account_they_do_not_own() {
    let mut scenario = Scenario::start(MARKET_NONCE, Mode::Funded).await;
    let actor = scenario.actor.insecure_clone();
    let stranger = Keypair::new();
    let mint = scenario.plane.collateral_mint;
    let foreign = scenario.create_token_account(mint, stranger.pubkey()).await;

    scenario.actor_collateral = foreign;
    let code = scenario
        .refusal_code(&[budget(SEAM_UNITS), scenario.split(0, 4)], &[&actor])
        .await;
    assert_eq!(code, ClutchError::TokenAccountNotAdmitted as u32);
    println!("SVM collateral holder: a stranger's account refused with Custom({code}) (0x001b)");
}
