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
    clutch_sbf::instructions::cash_exit,
    clutch_sbf::instructions::genesis,
    clutch_sbf::instructions::market_init,
    clutch_sbf::instructions::observe_resolve,
    clutch_sbf::instructions::split as seam,
    clutch_solana_layout::{
        Hash32, HoardAccount, Intent, MarketAccount, PositionAccount, ResolutionAccount,
        SupplyLedgerAccount,
    },
    clutch_solana_reference::{KernelAccount, ReplayAccount},
    clutch_svm_fixture::{
        build_plane, compute_unit_limit_data, create_market_request, immutable_owner_account_bytes,
        layout_request, Mode, Plane, CASH_ATOMS, COMPUTE_BUDGET, FOUNDING_MARKET_NONCE,
        FUNDED_SETS, MARKET_NONCE, OUTCOME_COUNT, POLICY_ACCOUNT, PROGRAM_ID, RENT_SYSVAR,
        RESERVED_CASH_ATOMS, SYSTEM_PROGRAM, TOKEN_2022,
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

fn second_actor_keypair() -> Keypair {
    Keypair::new_from_array([
        0x8f, 0x14, 0x42, 0x73, 0xb9, 0x20, 0x66, 0x35, 0x11, 0xed, 0x07, 0x5a, 0xcc, 0x98, 0x41,
        0x2e, 0x75, 0xa0, 0x39, 0x81, 0xd2, 0x54, 0x0b, 0xf6, 0x63, 0x18, 0xae, 0x47, 0x90, 0x2c,
        0xd5, 0x3b,
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
        Self::start_with_blocked_last_mint(nonce, mode, false).await
    }

    /// Start with an optional hostile pre-existing final outcome-mint address.
    ///
    /// This is not protocol state injection: it is a one-lamport System
    /// account used to force a refusal after state construction and one mint
    /// CPI, so the test can observe transaction rollback.
    async fn start_with_blocked_last_mint(nonce: u64, mode: Mode, block_last_mint: bool) -> Self {
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

        if block_last_mint {
            assert_eq!(mode, Mode::Empty, "only the empty constructor is probed");
            let blocked = plane
                .outcome_mints
                .last()
                .expect("a market has at least one outcome")
                .address;
            test.add_account(
                blocked,
                Account {
                    lamports: 1,
                    data: Vec::new(),
                    owner: solana_system_interface::program::ID,
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

    async fn fund_signer(&mut self, signer: Address, lamports: u64) {
        let payer = self.payer.pubkey();
        self.send(
            &[system_instruction::transfer(&payer, &signer, lamports)],
            &[],
        )
        .await;
    }

    async fn transfer_collateral(
        &mut self,
        source: Address,
        destination: Address,
        authority: &Keypair,
        amount: u64,
    ) {
        self.send(
            &[token_instruction::transfer_checked(
                &TOKEN_2022,
                &source,
                &self.plane.collateral_mint,
                &destination,
                &authority.pubkey(),
                &[],
                amount,
                COLLATERAL_DECIMALS,
            )
            .unwrap()],
            &[authority],
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

    async fn account(&mut self, address: Address) -> Account {
        self.banks
            .get_account(address)
            .await
            .unwrap()
            .expect("account should exist")
    }

    async fn data(&mut self, address: Address) -> Vec<u8> {
        self.account(address).await.data
    }

    async fn maybe_account(&mut self, address: Address) -> Option<Account> {
        self.banks.get_account(address).await.unwrap()
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

    /// The fixed collateral prefix plus every canonical outcome mint.
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
        metas.extend(
            self.plane
                .outcome_mints
                .iter()
                .map(|mint| AccountMeta::new_readonly(mint.address, false)),
        );
        assert_eq!(
            metas.len(),
            seam::ACCOUNT_PREFIX_COLLATERAL + usize::from(OUTCOME_COUNT)
        );
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

    /// The thirteen-account backed `Endow` deposit.
    fn endow_for(
        &self,
        owner: Address,
        actor_token: Address,
        position: Address,
        replay: Address,
        sequence: u64,
        amount: u64,
    ) -> Instruction {
        let request = layout_request(
            sequence,
            Intent::Endow {
                market: self.plane.market_id,
                owner: Hash32::from_bytes(owner.to_bytes()),
                amount,
            },
        );
        let metas = vec![
            AccountMeta::new(owner, true),
            AccountMeta::new_readonly(self.plane.market.address, false),
            AccountMeta::new_readonly(self.plane.hoard.address, false),
            AccountMeta::new(position, false),
            AccountMeta::new(replay, false),
            AccountMeta::new_readonly(self.plane.profile.address, false),
            AccountMeta::new_readonly(POLICY_ACCOUNT, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.plane.collateral_mint, false),
            AccountMeta::new(actor_token, false),
            AccountMeta::new(self.plane.hoard_token.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        assert_eq!(metas.len(), genesis::ENDOW_ACCOUNT_COUNT);
        Instruction::new_with_bytes(PROGRAM_ID, &request, metas)
    }

    fn endow(&self, sequence: u64, amount: u64) -> Instruction {
        self.endow_for(
            self.actor.pubkey(),
            self.actor_collateral,
            self.plane.position.address,
            self.plane.replay.address,
            sequence,
            amount,
        )
    }

    /// The twelve-account owner free-cash exit.
    fn withdraw_for(
        &self,
        owner: Address,
        destination: Address,
        position: Address,
        replay: Address,
        sequence: u64,
        amount: u64,
    ) -> Instruction {
        let request = layout_request(
            sequence,
            Intent::WithdrawCash {
                market: self.plane.market_id,
                owner: Hash32::from_bytes(owner.to_bytes()),
                destination: Hash32::from_bytes(destination.to_bytes()),
                amount,
            },
        );
        let metas = vec![
            AccountMeta::new_readonly(owner, true),
            AccountMeta::new_readonly(self.plane.market.address, false),
            AccountMeta::new_readonly(self.plane.hoard.address, false),
            AccountMeta::new(position, false),
            AccountMeta::new(replay, false),
            AccountMeta::new_readonly(self.plane.profile.address, false),
            AccountMeta::new_readonly(POLICY_ACCOUNT, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.plane.collateral_mint, false),
            AccountMeta::new(destination, false),
            AccountMeta::new_readonly(self.plane.hoard_authority.address, false),
            AccountMeta::new(self.plane.hoard_token.address, false),
        ];
        assert_eq!(metas.len(), cash_exit::ACCOUNT_COUNT);
        Instruction::new_with_bytes(PROGRAM_ID, &request, metas)
    }

    fn withdraw(&self, sequence: u64, amount: u64) -> Instruction {
        self.withdraw_for(
            self.actor.pubkey(),
            self.actor_collateral,
            self.plane.position.address,
            self.plane.replay.address,
            sequence,
            amount,
        )
    }

    /// The fixed outcome prefix plus every canonical outcome mint.
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
        let leg = self.plane.outcome_leg(holder);
        metas.push(AccountMeta::new_readonly(leg[0], false));
        metas.push(AccountMeta::new(leg[1], false));
        metas.extend(self.plane.outcome_mints.iter().map(|mint| {
            if mint.address == self.plane.outcome_mints[outcome].address {
                AccountMeta::new(mint.address, false)
            } else {
                AccountMeta::new_readonly(mint.address, false)
            }
        }));
        assert_eq!(
            metas.len(),
            seam::ACCOUNT_PREFIX_OUTCOME + usize::from(OUTCOME_COUNT)
        );
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
                    destination: Hash32::from_bytes(holder.to_bytes()),
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
                    source: Hash32::from_bytes(holder.to_bytes()),
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

    /// The eighteen-account collateral prefix plus every canonical outcome mint.
    fn redeem(&self, sequence: u64, outcome: u8, quantity: u64) -> Instruction {
        let evidence = self.plane.redeem_addresses();
        let mut metas = vec![AccountMeta::new(evidence[0], true)];
        for address in &evidence[1..7] {
            metas.push(AccountMeta::new(*address, false));
        }
        for address in &evidence[7..11] {
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
        metas.extend(
            self.plane
                .outcome_mints
                .iter()
                .map(|mint| AccountMeta::new_readonly(mint.address, false)),
        );
        assert_eq!(
            metas.len(),
            observe_resolve::REDEEM_ACCOUNT_PREFIX + usize::from(OUTCOME_COUNT)
        );
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
            let writable = matches!(index, 4..=10) || index >= market_init::IX_HOARD_TOKEN;
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
/// Unit ceiling for the first backed deposit, including two state-creation CPIs.
const FIRST_ENDOW_UNITS: u32 = 900_000;

fn budget(units: u32) -> Instruction {
    Instruction::new_with_bytes(COMPUTE_BUDGET, &compute_unit_limit_data(units), Vec::new())
}

fn rent_exempt(space: usize) -> u64 {
    solana_rent::Rent::default().minimum_balance(space).max(1)
}

/// **`CreateMarket` creates seven state PDAs, the mints, and Hoard token.**
///
/// The market-specific plane is genuinely absent before the transaction. A
/// normal funded signer pays for seven canonical program accounts and the
/// Token-2022 plane, and every property is read back from the bank.
#[tokio::test]
async fn create_market_founds_seven_state_pdas_and_the_token_plane_from_absence() {
    let mut scenario = Scenario::start(FOUNDING_MARKET_NONCE, Mode::Empty).await;

    for target in scenario.plane.market_state_addresses() {
        assert!(
            scenario.maybe_account(target).await.is_none(),
            "a state target must be absent before CreateMarket"
        );
    }
    for mint in scenario.plane.outcome_mints.clone() {
        assert!(
            scenario.maybe_account(mint.address).await.is_none(),
            "an outcome mint must not exist before CreateMarket"
        );
    }
    let hoard_token = scenario.plane.hoard_token.address;
    assert!(scenario.maybe_account(hoard_token).await.is_none());

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

    for target in scenario.plane.market_state_addresses() {
        let account = scenario.account(target).await;
        assert_eq!(account.owner, PROGRAM_ID, "state target owner");
        assert!(
            account.lamports >= rent_exempt(account.data.len()),
            "state target is rent exempt"
        );
    }

    let market_state = MarketAccount::decode(&scenario.data(scenario.plane.market.address).await)
        .expect("Market decodes");
    assert_eq!(market_state.market, scenario.plane.market_id);
    assert_eq!(market_state.realm, scenario.plane.realm_id);
    assert_eq!(market_state.profile, scenario.plane.profile_id);
    assert_eq!(market_state.terms, scenario.plane.terms_id);
    assert_eq!(market_state.outcome_count, OUTCOME_COUNT);
    assert_eq!(market_state.stored_bump, scenario.plane.market.bump);

    let hoard_state = HoardAccount::decode(&scenario.data(scenario.plane.hoard.address).await)
        .expect("Hoard decodes");
    assert_eq!(hoard_state.market, scenario.plane.market_id);
    assert_eq!(hoard_state.realm, scenario.plane.realm_id);
    assert_eq!(hoard_state.collateral_atoms, 0);
    assert_eq!(hoard_state.stored_bump, scenario.plane.hoard.bump);

    let position_state =
        PositionAccount::decode(&scenario.data(scenario.plane.position.address).await)
            .expect("Position decodes");
    assert_eq!(position_state.market, scenario.plane.market_id);
    assert_eq!(
        position_state.owner,
        Hash32::from_bytes(scenario.actor.pubkey().to_bytes())
    );
    assert_eq!(
        position_state.internal,
        [0; clutch_solana_layout::MAX_OUTCOMES]
    );
    assert_eq!(position_state.cash_atoms, 0);
    assert_eq!(position_state.stored_bump, scenario.plane.position.bump);

    let kernel_state = KernelAccount::decode(&scenario.data(scenario.plane.kernel.address).await)
        .expect("Kernel decodes");
    assert_eq!(kernel_state.market, scenario.plane.market_id);
    assert_eq!(kernel_state.phase, 0);
    assert_eq!(
        kernel_state.total_supply,
        [0; clutch_solana_layout::MAX_OUTCOMES]
    );

    let replay_state = ReplayAccount::decode(&scenario.data(scenario.plane.replay.address).await)
        .expect("Replay decodes");
    assert_eq!(replay_state.market, scenario.plane.market_id);
    assert_eq!(replay_state.position_generation, 0);
    assert_eq!(replay_state.sequence, 0);
    assert_eq!(replay_state.stored_bump, scenario.plane.replay.bump);

    let supply_state =
        SupplyLedgerAccount::decode(&scenario.data(scenario.plane.supply.address).await)
            .expect("SupplyLedger decodes");
    assert_eq!(supply_state.market, scenario.plane.market_id);
    assert_eq!(
        supply_state.internal_supply,
        [0; clutch_solana_layout::MAX_OUTCOMES]
    );
    assert_eq!(
        supply_state.external_supply,
        [0; clutch_solana_layout::MAX_OUTCOMES]
    );
    assert_eq!(supply_state.stored_bump, scenario.plane.supply.bump);

    let resolution_state =
        ResolutionAccount::decode(&scenario.data(scenario.plane.resolution.address).await)
            .expect("Resolution decodes");
    assert_eq!(resolution_state.market, scenario.plane.market_id);
    assert_eq!(resolution_state.terms, scenario.plane.terms_id);
    assert_eq!(resolution_state.feed, scenario.plane.feed_id);
    assert_eq!(resolution_state.stored_bump, scenario.plane.resolution.bump);

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

    println!(
        "SVM create_market: 20 accounts, 7 state PDAs + 2 outcome mints + hoard token, {units} CU"
    );
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

/// **A late token-plane refusal rolls every earlier System CPI back.**
///
/// The final outcome-mint address is occupied by a one-lamport System account.
/// `CreateMarket` therefore constructs all seven state accounts and the first
/// outcome mint before it reaches the refusal. Atomic transaction failure must
/// leave none of those earlier effects in the committed bank.
#[tokio::test]
async fn late_create_market_refusal_rolls_back_state_and_token_construction() {
    let mut scenario =
        Scenario::start_with_blocked_last_mint(FOUNDING_MARKET_NONCE, Mode::Empty, true).await;
    let actor = scenario.actor.insecure_clone();
    let blocked = scenario
        .plane
        .outcome_mints
        .last()
        .expect("fixture outcomes")
        .address;
    let first = scenario.plane.outcome_mints[0].address;
    let hoard_token = scenario.plane.hoard_token.address;

    let code = scenario
        .refusal_code(
            &[
                budget(CREATE_UNITS),
                scenario.create_market(FOUNDING_MARKET_NONCE),
            ],
            &[&actor],
        )
        .await;
    assert_eq!(code, ClutchError::AlreadyInitialized as u32);

    for target in scenario.plane.market_state_addresses() {
        assert!(
            scenario.maybe_account(target).await.is_none(),
            "a failed transaction must roll back state construction"
        );
    }
    assert!(
        scenario.maybe_account(first).await.is_none(),
        "the first outcome mint CPI must roll back"
    );
    assert!(
        scenario.maybe_account(hoard_token).await.is_none(),
        "the Hoard token account was not reached and remains absent"
    );
    let blocker = scenario.account(blocked).await;
    assert_eq!(blocker.owner, solana_system_interface::program::ID);
    assert_eq!(blocker.lamports, 1);
    assert!(blocker.data.is_empty());
}

/// **A second wallet opens its canonical owner plane with its first deposit.**
///
/// No protocol state for the wallet exists beforehand. `Endow` authenticates
/// the owner, System-CPI-creates Position and Replay, transfers real
/// collateral into pooled custody, credits cash, and commits replay sequence
/// one atomically.
#[tokio::test]
async fn first_endow_creates_a_second_wallets_position_and_replay() {
    let mut scenario = Scenario::start(FOUNDING_MARKET_NONCE, Mode::Empty).await;
    let founder = scenario.actor.insecure_clone();
    scenario
        .send(
            &[
                budget(CREATE_UNITS),
                scenario.create_market(FOUNDING_MARKET_NONCE),
            ],
            &[&founder],
        )
        .await;

    let second = second_actor_keypair();
    scenario.fund_signer(second.pubkey(), 100_000_000).await;
    let second_token = scenario
        .create_token_account(scenario.plane.collateral_mint, second.pubkey())
        .await;
    scenario
        .transfer_collateral(scenario.actor_collateral, second_token, &founder, 9)
        .await;
    let (position, replay) = scenario.plane.owner_plane(second.pubkey());
    assert!(scenario.maybe_account(position.address).await.is_none());
    assert!(scenario.maybe_account(replay.address).await.is_none());

    /* An authenticated wallet cannot initialize another owner's plane. The
     * refusal occurs before either System CPI. */
    let mut forged = scenario.endow_for(
        second.pubkey(),
        second_token,
        position.address,
        replay.address,
        0,
        9,
    );
    forged.accounts[0] = AccountMeta::new(founder.pubkey(), true);
    let code = scenario
        .refusal_code(&[budget(FIRST_ENDOW_UNITS), forged], &[&founder])
        .await;
    assert_eq!(code, ClutchError::UnauthorizedActor as u32);
    assert!(scenario.maybe_account(position.address).await.is_none());
    assert!(scenario.maybe_account(replay.address).await.is_none());

    /* The owner is valid and both CPIs now run, but Token-2022 refuses the
     * overdraw after construction. The owner plane must still remain absent. */
    let overdraw = scenario.endow_for(
        second.pubkey(),
        second_token,
        position.address,
        replay.address,
        0,
        10,
    );
    let code = scenario
        .refusal_code(&[budget(FIRST_ENDOW_UNITS), overdraw], &[&second])
        .await;
    assert_eq!(code, TokenError::InsufficientFunds as u32);
    assert!(scenario.maybe_account(position.address).await.is_none());
    assert!(scenario.maybe_account(replay.address).await.is_none());

    let instruction = scenario.endow_for(
        second.pubkey(),
        second_token,
        position.address,
        replay.address,
        0,
        9,
    );
    let units = scenario
        .send(&[budget(FIRST_ENDOW_UNITS), instruction], &[&second])
        .await;

    for target in [position.address, replay.address] {
        let account = scenario.account(target).await;
        assert_eq!(account.owner, PROGRAM_ID);
        assert!(account.lamports >= rent_exempt(account.data.len()));
    }
    let position_state = PositionAccount::decode(&scenario.data(position.address).await)
        .expect("second Position decodes");
    assert_eq!(position_state.market, scenario.plane.market_id);
    assert_eq!(
        position_state.owner,
        Hash32::from_bytes(second.pubkey().to_bytes())
    );
    assert_eq!(position_state.generation, 0);
    assert_eq!(position_state.cash_atoms, 9);
    assert_eq!(position_state.reserved_cash_atoms, 0);
    assert_eq!(position_state.stored_bump, position.bump);
    let replay_state =
        ReplayAccount::decode(&scenario.data(replay.address).await).expect("second Replay decodes");
    assert_eq!(replay_state.owner, position_state.owner);
    assert_eq!(replay_state.position_generation, 0);
    assert_eq!(replay_state.sequence, 1);
    assert_eq!(replay_state.stored_bump, replay.bump);
    assert_eq!(scenario.amount(second_token).await, 0);
    assert_eq!(scenario.amount(scenario.plane.hoard_token.address).await, 9);
    println!("SVM first Endow: 2 state PDAs + backed deposit, {units} CU");
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

/// `WithdrawCash` is the exact inverse value boundary for unreserved cash.
#[tokio::test]
async fn withdraw_pays_only_unreserved_cash_and_preserves_locked_backing() {
    let mut scenario = Scenario::start(MARKET_NONCE, Mode::Funded).await;
    let actor = scenario.actor.insecure_clone();
    let actor_token = scenario.actor_collateral;
    let hoard_token = scenario.plane.hoard_token.address;
    let pre_actor = scenario.amount(actor_token).await;
    let pre_hoard = scenario.amount(hoard_token).await;
    let free = CASH_ATOMS - RESERVED_CASH_ATOMS;

    let watched = [
        scenario.plane.position.address,
        scenario.plane.replay.address,
        actor_token,
        hoard_token,
    ];
    let mut before = Vec::new();
    for address in watched {
        before.push(scenario.data(address).await);
    }
    let code = scenario
        .refusal_code(
            &[budget(SEAM_UNITS), scenario.withdraw(0, free + 1)],
            &[&actor],
        )
        .await;
    assert_eq!(code, 0x2008, "reserved cash must not be withdrawable");
    for (index, address) in watched.iter().enumerate() {
        assert_eq!(scenario.data(*address).await, before[index]);
    }

    let units = scenario
        .send(&[budget(SEAM_UNITS), scenario.withdraw(0, free)], &[&actor])
        .await;
    assert_eq!(scenario.amount(actor_token).await, pre_actor + free);
    assert_eq!(scenario.amount(hoard_token).await, pre_hoard - free);
    assert_eq!(scenario.hoard_atoms().await, FUNDED_SETS);
    let position = scenario.position().await;
    assert_eq!(position.cash_atoms, RESERVED_CASH_ATOMS);
    assert_eq!(position.reserved_cash_atoms, RESERVED_CASH_ATOMS);
    let replay = ReplayAccount::decode(&scenario.data(scenario.plane.replay.address).await)
        .expect("replay decodes");
    assert_eq!(replay.sequence, 1);
    println!("SVM WithdrawCash: paid {free} unreserved atoms, {units} CU");
}

/// A later instruction failure restores an earlier successful Hoard transfer.
#[tokio::test]
async fn duplicate_withdrawal_transaction_rolls_back_the_first_token_cpi() {
    let mut scenario = Scenario::start(MARKET_NONCE, Mode::Funded).await;
    let actor = scenario.actor.insecure_clone();
    let watched = [
        scenario.plane.position.address,
        scenario.plane.replay.address,
        scenario.actor_collateral,
        scenario.plane.hoard_token.address,
    ];
    let mut before = Vec::new();
    for address in watched {
        before.push(scenario.data(address).await);
    }
    let duplicate = scenario.withdraw(0, 5);
    let code = scenario
        .refusal_code(
            &[budget(SEAM_UNITS), duplicate.clone(), duplicate],
            &[&actor],
        )
        .await;
    assert_eq!(code, ClutchError::Replay as u32);
    for (index, address) in watched.iter().enumerate() {
        assert_eq!(
            scenario.data(*address).await,
            before[index],
            "account {index} changed across atomic duplicate refusal"
        );
    }
    println!("SVM WithdrawCash rollback: first CPI restored after duplicate replay refusal");
}

/// Both the signed wire destination and current Token-2022 owner must agree.
#[tokio::test]
async fn withdraw_refuses_destination_substitution_and_foreign_authority() {
    let mut scenario = Scenario::start(MARKET_NONCE, Mode::Funded).await;
    let actor = scenario.actor.insecure_clone();
    let stranger = Keypair::new();
    let foreign = scenario
        .create_token_account(scenario.plane.collateral_mint, stranger.pubkey())
        .await;
    let watched = [
        scenario.plane.position.address,
        scenario.plane.replay.address,
        scenario.actor_collateral,
        scenario.plane.hoard_token.address,
        foreign,
    ];
    let mut before = Vec::new();
    for address in watched {
        before.push(scenario.data(address).await);
    }

    let foreign_authority = scenario.withdraw_for(
        actor.pubkey(),
        foreign,
        scenario.plane.position.address,
        scenario.plane.replay.address,
        0,
        1,
    );
    let code = scenario
        .refusal_code(&[budget(SEAM_UNITS), foreign_authority], &[&actor])
        .await;
    assert_eq!(code, ClutchError::TokenAccountNotAdmitted as u32);

    let mut substituted = scenario.withdraw(0, 1);
    substituted.accounts[cash_exit::IX_DESTINATION] = AccountMeta::new(foreign, false);
    let code = scenario
        .refusal_code(&[budget(SEAM_UNITS), substituted], &[&actor])
        .await;
    assert_eq!(code, ClutchError::MismatchedState as u32);
    for (index, address) in watched.iter().enumerate() {
        assert_eq!(scenario.data(*address).await, before[index]);
    }
}

/// Donations remain unowned while two Positions exit only their own cash.
#[tokio::test]
async fn donation_and_multiple_positions_do_not_cross_credit_withdrawals() {
    let mut scenario = Scenario::start(MARKET_NONCE, Mode::Funded).await;
    let founder = scenario.actor.insecure_clone();
    let hoard_token = scenario.plane.hoard_token.address;
    let initial_hoard = scenario.amount(hoard_token).await;

    /* This direct transfer creates surplus and no Position credit. */
    scenario
        .transfer_collateral(scenario.actor_collateral, hoard_token, &founder, 5)
        .await;

    let second = second_actor_keypair();
    scenario.fund_signer(second.pubkey(), 100_000_000).await;
    let second_token = scenario
        .create_token_account(scenario.plane.collateral_mint, second.pubkey())
        .await;
    scenario
        .transfer_collateral(scenario.actor_collateral, second_token, &founder, 9)
        .await;
    let (second_position, second_replay) = scenario.plane.owner_plane(second.pubkey());
    scenario
        .send(
            &[
                budget(FIRST_ENDOW_UNITS),
                scenario.endow_for(
                    second.pubkey(),
                    second_token,
                    second_position.address,
                    second_replay.address,
                    0,
                    9,
                ),
            ],
            &[&second],
        )
        .await;

    let founder_free = CASH_ATOMS - RESERVED_CASH_ATOMS;
    scenario
        .send(
            &[budget(SEAM_UNITS), scenario.withdraw(0, founder_free)],
            &[&founder],
        )
        .await;
    scenario
        .send(
            &[
                budget(SEAM_UNITS),
                scenario.withdraw_for(
                    second.pubkey(),
                    second_token,
                    second_position.address,
                    second_replay.address,
                    1,
                    9,
                ),
            ],
            &[&second],
        )
        .await;

    let founder_position = scenario.position().await;
    let second_position_state =
        PositionAccount::decode(&scenario.data(second_position.address).await)
            .expect("second position decodes");
    assert_eq!(founder_position.cash_atoms, RESERVED_CASH_ATOMS);
    assert_eq!(founder_position.reserved_cash_atoms, RESERVED_CASH_ATOMS);
    assert_eq!(second_position_state.cash_atoms, 0);
    assert_eq!(scenario.amount(second_token).await, 9);
    assert_eq!(
        scenario.amount(hoard_token).await,
        initial_hoard + 5 + 9 - founder_free - 9,
        "the five-atom donation remains in pooled custody and is not withdrawable"
    );
    assert_eq!(scenario.hoard_atoms().await, FUNDED_SETS);
}

/// **The whole cycle, with both legs and a holder who is not the actor.**
///
/// `Split` puts real collateral into the Hoard, `Materialize` mints real
/// outcome tokens, **those tokens are transferred to a second wallet outside
/// this program**, `Dematerialize` burns what is left, and `Merge` takes real
/// collateral back out. Two token programs' worth of arithmetic and one
/// kernel's, required to agree at every step.
///
/// The external transfer is the part worth having: it demonstrates why actual
/// mint supply is authoritative and no per-owner program shadow can be. A
/// holder may keep outcome tokens anywhere and give them to anybody.
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
            &[budget(SEAM_UNITS), scenario.endow(0, ACTOR_COLLATERAL + 1)],
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

    println!("SVM redeem: 20 accounts, payout={quantity} atoms, {units} CU");

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
