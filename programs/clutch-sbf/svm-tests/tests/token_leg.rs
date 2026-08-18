//! The Token-2022 leg of `Materialize` and `Dematerialize`, driven by an
//! in-process Agave bank against the **real** `clutch_sbf` ELF and the **real**
//! Token-2022 program.
//!
//! What is real here, and what is not:
//!
//! * the program is the ELF `cargo-build-sbf` produced. There is no native
//!   processor, no model of the program, and no in-process shortcut: the bank
//!   executes SBF bytecode.
//! * the token program is `spl_token_2022-10.0.0.so`, installed at genesis by
//!   `solana-program-test` from the published `solana-program-binaries` crate.
//!   Every mint, burn and balance below was computed by it.
//! * the CPI is a real cross-program invocation with a real `invoke_signed`
//!   PDA signature. This is the thing the host differential structurally
//!   cannot reach, because `solana_cpi::invoke_signed` is `Ok(())` off-chain.
//!
//! What is still **not** established by anything in this file: an in-process
//! bank is not a cluster. Transaction replay, durable nonces, instruction
//! duplication inside one transaction, fee payment, rent collection over time
//! and program upgrade are all outside what `solana-program-test` can show, and
//! `docs/implementation/TOKEN2022_PLAN.md` §4 lists them as such.
//!
//! Compute units are printed per scenario. They are *whole-transaction* figures
//! including the program's own validation, the closure obligations, the kernel
//! step, the CPI frame, and account re-serialization — which is exactly what
//! the probe's 1 230 / 1 720 / 1 235 CU bare-instruction figures were not.
//!
//! The outcome leg is **mandatory**: `Materialize` and `Dematerialize` accept
//! thirteen accounts and nothing else. The ten-account shadow-only plane these
//! scenarios used to measure a hole with is now a refusal, which is what
//! `the_ten_account_plane_is_refused_and_moves_nothing` asserts. The
//! collateral leg, market founding, and the redemption payout live in
//! `collateral_leg.rs`.

use {
    clutch_sbf::error::ClutchError,
    clutch_sbf::instructions::split::{
        ACCOUNT_COUNT, ACCOUNT_COUNT_OUTCOME, IX_EXTERNAL, IX_SUPPLY,
    },
    clutch_solana_layout::{Hash32, Intent, SupplyLedgerAccount},
    clutch_solana_reference::ExternalAccount,
    clutch_svm_fixture::{
        build_plane, layout_request, mint_bytes_with_extension, outcome_mint_bytes, Mode, Plane,
        BASE_TOKEN_ACCOUNT_LEN, EXTENDED_MINT_OVERHEAD, FUNDED_SETS, MARKET_NONCE, PROGRAM_ID,
        TOKEN_2022,
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
        extension::ExtensionType,
        instruction as token_instruction,
        state::{Account as TokenAccount, Mint},
    },
};

/// The outcome every scenario acts on.
const OUTCOME: u8 = 1;

/// A running bank with the fixture plane loaded and one outcome mint present.
struct Scenario {
    banks: BanksClient,
    payer: Keypair,
    actor: Keypair,
    plane: Plane,
    holder: Address,
}

/// How the outcome mint should be shaped at genesis.
#[derive(Clone, Copy)]
enum MintShape {
    /// Exactly what the plan proposes: decimals 0, market PDA authority, no
    /// freeze authority, no extensions.
    Proposed,
    /// The same, plus one TLV extension entry.
    WithExtension(u16),
}

impl Scenario {
    async fn start(shape: MintShape) -> Self {
        /* A **fixed** actor, not a random one.  `find_program_address` searches
         * downward from bump 255, so the position, shadow and replay PDAs of a
         * random owner cost a different number of iterations every run -- and
         * the compute-unit figures this file records would drift by thousands
         * between runs for no reason anybody could see.  Deterministic seeds
         * make the CU numbers a measurement instead of a sample. */
        let actor = Keypair::new_from_array([
            0x3a, 0x91, 0x0d, 0x64, 0x22, 0xb8, 0x57, 0xe1, 0x4c, 0x05, 0xf3, 0x7a, 0x98, 0x11,
            0x2d, 0x6b, 0x40, 0xcf, 0x83, 0x19, 0x75, 0xa2, 0x0e, 0xd4, 0x66, 0x37, 0xbb, 0x52,
            0x08, 0xe9, 0x71, 0xc3,
        ]);
        /* A deterministic collateral mint, because the Realm's frozen policy
         * names it and the Profile identity is the parent hash over that
         * policy's digest -- so the mint's address is upstream of every PDA in
         * the plane.  No scenario in this file moves collateral, but the plane
         * cannot be built without knowing which asset it is denominated in. */
        let collateral_mint = Address::new_from_array([0x6d; 32]);
        let plane = build_plane(actor.pubkey(), collateral_mint, MARKET_NONCE, Mode::Funded);
        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.add_program("clutch_sbf", PROGRAM_ID, None);

        // The actor pays for nothing but must exist and be able to sign.
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

        // The nine program-owned state accounts.
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

        /* The outcome mint.  It has to be installed at genesis rather than
         * created in a transaction: it lives at a program-derived address, and
         * `system_instruction::create_account` needs the new account's own
         * signature, which only the owning program can supply.  Creating it is
         * `CreateMarket`'s job and `CreateMarket` creates no account yet. */
        let mint = plane.outcome_mints[usize::from(OUTCOME)].address;
        let mint_extensions: Vec<ExtensionType> = match shape {
            MintShape::Proposed => Vec::new(),
            MintShape::WithExtension(discriminant) => {
                vec![ExtensionType::try_from(discriminant).expect("a known discriminant")]
            }
        };
        let data = match shape {
            MintShape::Proposed => outcome_mint_bytes(plane.market.address, 0),
            MintShape::WithExtension(discriminant) => {
                /* The real value length, from the real decoder's own sizing
                 * function.  A short entry would be a malformed mint, which the
                 * token program refuses for a different reason than the one
                 * under test. */
                let total =
                    ExtensionType::try_calculate_account_len::<Mint>(&mint_extensions).unwrap();
                mint_bytes_with_extension(
                    plane.market.address,
                    0,
                    discriminant,
                    total - EXTENDED_MINT_OVERHEAD,
                )
            }
        };
        test.add_account(
            mint,
            Account {
                lamports: rent_exempt(data.len()),
                data,
                owner: TOKEN_2022,
                executable: false,
                rent_epoch: 0,
            },
        );

        let (banks, payer, _blockhash) = test.start().await;
        let mut scenario = Self {
            banks,
            payer,
            actor,
            plane,
            holder: Address::default(),
        };
        scenario.holder = scenario.create_holder_account(mint, &mint_extensions).await;
        scenario
    }

    /// Create the actor's own Token-2022 account for `mint`.
    ///
    /// This one *is* created by the real token program in a real transaction:
    /// it is an ordinary wallet-owned account, so the account keypair can sign
    /// for its own creation. Its bytes are therefore Token-2022's, not this
    /// harness's.
    async fn create_holder_account(
        &mut self,
        mint: Address,
        mint_extensions: &[ExtensionType],
    ) -> Address {
        let account = Keypair::new();
        /* A mint carrying an extension can force an extension on every account
         * holding it -- `TransferFeeConfig` forces `TransferFeeAmount` -- and
         * the token program refuses to initialize an account too small to
         * carry it.  Sizing from the token program's own rule keeps the
         * scenario about the *policy* refusal rather than about a short
         * account. */
        let account_extensions = {
            #[allow(deprecated)]
            ExtensionType::get_required_init_account_extensions(mint_extensions)
        };
        let space =
            ExtensionType::try_calculate_account_len::<TokenAccount>(&account_extensions).unwrap();
        let lamports = self.rent_for(space).await;
        self.send(
            &[
                system_instruction::create_account(
                    &self.payer.pubkey(),
                    &account.pubkey(),
                    lamports,
                    space as u64,
                    &TOKEN_2022,
                ),
                token_instruction::initialize_account3(
                    &TOKEN_2022,
                    &account.pubkey(),
                    &mint,
                    &self.actor.pubkey(),
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

    /// The custom program error code a refused transaction reported.
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

    fn mint(&self) -> Address {
        self.plane.outcome_mints[usize::from(OUTCOME)].address
    }

    /// The seam instruction, with or without the optional token leg.
    fn seam_instruction(&self, request: Vec<u8>, with_token_leg: bool) -> Instruction {
        let addresses = self.plane.seam_addresses();
        let mut metas = Vec::with_capacity(ACCOUNT_COUNT_OUTCOME);
        metas.push(AccountMeta::new(addresses[0], true));
        metas.push(AccountMeta::new_readonly(addresses[1], false));
        metas.push(AccountMeta::new_readonly(addresses[2], false));
        for address in &addresses[3..ACCOUNT_COUNT] {
            metas.push(AccountMeta::new(*address, false));
        }
        if with_token_leg {
            metas.push(AccountMeta::new_readonly(TOKEN_2022, false));
            metas.push(AccountMeta::new(self.mint(), false));
            metas.push(AccountMeta::new(self.holder, false));
        }
        assert_eq!(
            metas.len(),
            if with_token_leg {
                ACCOUNT_COUNT_OUTCOME
            } else {
                ACCOUNT_COUNT
            }
        );
        Instruction::new_with_bytes(PROGRAM_ID, &request, metas)
    }

    fn materialize(&self, sequence: u64, quantity: u64) -> Instruction {
        self.seam_instruction(
            layout_request(
                sequence,
                Intent::Materialize {
                    market: self.plane.market_id,
                    owner: Hash32::from_bytes(self.actor.pubkey().to_bytes()),
                    destination: Hash32::from_bytes(self.plane.external.address.to_bytes()),
                    outcome: OUTCOME,
                    quantity,
                },
            ),
            true,
        )
    }

    fn dematerialize(&self, sequence: u64, quantity: u64) -> Instruction {
        self.seam_instruction(
            layout_request(
                sequence,
                Intent::Dematerialize {
                    market: self.plane.market_id,
                    owner: Hash32::from_bytes(self.actor.pubkey().to_bytes()),
                    source: Hash32::from_bytes(self.plane.external.address.to_bytes()),
                    outcome: OUTCOME,
                    quantity,
                },
            ),
            true,
        )
    }

    async fn data(&mut self, address: Address) -> Vec<u8> {
        self.banks
            .get_account(address)
            .await
            .unwrap()
            .expect("account should exist")
            .data
    }

    async fn mint_supply(&mut self) -> u64 {
        let data = self.data(self.mint()).await;
        <Mint as Pack>::unpack(&data[..Mint::LEN])
            .expect("the token program's own mint must unpack")
            .supply
    }

    async fn holder_amount(&mut self) -> u64 {
        let data = self.data(self.holder).await;
        <TokenAccount as Pack>::unpack(&data[..TokenAccount::LEN])
            .expect("the token program's own account must unpack")
            .amount
    }

    /// The market-wide external term, the outcome mint's counterpart.
    async fn external_supply(&mut self) -> u64 {
        let data = self.data(self.plane.seam_addresses()[IX_SUPPLY]).await;
        SupplyLedgerAccount::decode(&data)
            .expect("ledger decodes")
            .external_supply[usize::from(OUTCOME)]
    }

    async fn internal_supply(&mut self) -> u64 {
        let data = self.data(self.plane.seam_addresses()[IX_SUPPLY]).await;
        SupplyLedgerAccount::decode(&data)
            .expect("ledger decodes")
            .internal_supply[usize::from(OUTCOME)]
    }

    /// The *per-owner* shadow balance, which is not the mint's counterpart.
    async fn shadow_balance(&mut self) -> u64 {
        let data = self.data(self.plane.seam_addresses()[IX_EXTERNAL]).await;
        ExternalAccount::decode(&data)
            .expect("shadow decodes")
            .balances[usize::from(OUTCOME)]
    }
}

fn rent_exempt(space: usize) -> u64 {
    // `Rent::default()` is what `ProgramTest` seeds genesis with.
    solana_rent::Rent::default().minimum_balance(space).max(1)
}

/// **E1 — one-to-one materialization**, against a real mint.
///
/// `Materialize` of *q* on outcome *o* must increase `outcome_mint[o].supply`
/// by exactly *q*, increase the destination token account by exactly *q*, and
/// decrease `position.internal[o]` by exactly *q*. And — the part this lane
/// adds to the plan's E1 — the market-wide external term must come out equal to
/// the mint's supply, which is the reconciliation that makes the eventual
/// deletion of the shadow a deletion rather than a change of semantics.
#[tokio::test]
async fn e1_materialize_mints_exactly_q_and_the_shadow_reconciles() {
    let mut scenario = Scenario::start(MintShape::Proposed).await;
    assert_eq!(scenario.mint_supply().await, 0, "a founding mint is empty");
    assert_eq!(scenario.holder_amount().await, 0);

    let quantity = 7;
    let units = scenario
        .send(
            &[scenario.materialize(0, quantity)],
            &[&scenario.actor.insecure_clone()],
        )
        .await;

    assert_eq!(scenario.mint_supply().await, quantity, "supply rose by q");
    assert_eq!(scenario.holder_amount().await, quantity, "the holder got q");
    assert_eq!(
        scenario.internal_supply().await,
        FUNDED_SETS - quantity,
        "the internal term fell by q"
    );
    assert_eq!(
        scenario.external_supply().await,
        quantity,
        "the external term rose by q"
    );
    assert_eq!(
        scenario.external_supply().await,
        scenario.mint_supply().await,
        "the reconciliation the program itself enforced"
    );
    assert_eq!(scenario.shadow_balance().await, quantity);

    println!("SVM e1_materialize: quantity={quantity} compute_units={units}");
}

/// **E1, reversed** — `Dematerialize` burns exactly, and reconciles.
#[tokio::test]
async fn e1_dematerialize_burns_exactly_and_the_shadow_reconciles() {
    let mut scenario = Scenario::start(MintShape::Proposed).await;
    let actor = scenario.actor.insecure_clone();
    let mint_units = scenario
        .send(&[scenario.materialize(0, 7)], &[&actor])
        .await;
    let burn_units = scenario
        .send(&[scenario.dematerialize(1, 3)], &[&actor])
        .await;

    assert_eq!(scenario.mint_supply().await, 4, "7 minted, 3 burned");
    assert_eq!(scenario.holder_amount().await, 4);
    assert_eq!(scenario.internal_supply().await, FUNDED_SETS - 4);
    assert_eq!(scenario.external_supply().await, 4);
    assert_eq!(scenario.shadow_balance().await, 4);
    assert_eq!(
        scenario.external_supply().await,
        scenario.mint_supply().await
    );

    println!("SVM e1_dematerialize: materialize={mint_units} CU  dematerialize={burn_units} CU");
}

/// **The reconciliation is load-bearing, and falsifiable.**
///
/// `Burn` is permissionless for a token's owner, so the actor can destroy
/// outcome tokens *outside* this program. The mint's supply falls and the
/// market-wide ledger term does not, and the very next seam instruction on that
/// outcome must refuse rather than carry two disagreeing truths forward.
///
/// This is also the denial-of-service surface the check creates, measured
/// rather than argued: nothing in this program can currently repair the ledger,
/// so that outcome is stuck. It is an argument for deleting the shadow, not
/// against checking it.
#[tokio::test]
async fn a_supply_that_drifted_outside_the_program_is_refused() {
    let mut scenario = Scenario::start(MintShape::Proposed).await;
    let actor = scenario.actor.insecure_clone();
    scenario
        .send(&[scenario.materialize(0, 7)], &[&actor])
        .await;

    // The holder burns two of their own tokens, with no clutch instruction.
    scenario
        .send(
            &[token_instruction::burn(
                &TOKEN_2022,
                &scenario.holder,
                &scenario.mint(),
                &actor.pubkey(),
                &[],
                2,
            )
            .unwrap()],
            &[&actor],
        )
        .await;
    assert_eq!(scenario.mint_supply().await, 5);
    assert_eq!(
        scenario.external_supply().await,
        7,
        "the ledger did not move"
    );

    let code = scenario
        .refusal_code(&[scenario.materialize(1, 1)], &[&actor])
        .await;
    assert_eq!(
        code,
        ClutchError::ShadowSupplyMismatch as u32,
        "the two truths disagree and the program must say so"
    );
    println!("SVM reconciliation: out-of-band burn refused with Custom({code}) (0x001e)");
}

/// **E4, at instruction time.** A mint that decodes and is refused.
///
/// `TOKEN2022_PLAN.md` §3.4 exists because a mint *address* is not a
/// description of a mint's behaviour: `MintCloseAuthority` lets a zero-supply
/// mint be closed and reinitialized carrying a different extension set. So the
/// refusal runs over the mint as loaded in this transaction, and here it runs
/// against bytes a real bank served to a real ELF.
#[tokio::test]
async fn an_extension_on_the_outcome_mint_is_refused_at_instruction_time() {
    for (discriminant, label) in [(1_u16, "TransferFeeConfig"), (3, "MintCloseAuthority")] {
        let mut scenario = Scenario::start(MintShape::WithExtension(discriminant)).await;
        let actor = scenario.actor.insecure_clone();
        let code = scenario
            .refusal_code(&[scenario.materialize(0, 7)], &[&actor])
            .await;
        assert_eq!(
            code,
            ClutchError::TokenExtensionNotAllowed as u32,
            "{label} must be refused"
        );
        assert_eq!(
            scenario.mint_supply().await,
            0,
            "{label}: nothing was minted"
        );
        println!("SVM extension refusal: {label} refused with Custom({code}) (0x001a)");
    }
}

/// **The outflow asymmetry, against this program's own seed schema.**
///
/// The probe established that a token account whose owner authority is *some*
/// program address refuses a wallet-signed transfer out. This establishes it
/// for the address `seeds::hoard_authority_pda` actually derives: a deposit in
/// needs only the depositor's signature, and taking it back out is impossible
/// without the program signing for the Hoard authority seeds.
///
/// The Hoard token account is created at a wallet-controlled address here
/// rather than at `seeds::hoard_token_pda`, because nothing can create an
/// account at a PDA until `CreateMarket` does. What is under test is the
/// *authority*, and that is the derived one.
#[tokio::test]
async fn the_derived_hoard_authority_cannot_be_signed_for_by_a_wallet() {
    let mut scenario = Scenario::start(MintShape::Proposed).await;
    let actor = scenario.actor.insecure_clone();
    let mint = scenario.mint();

    // Give the actor something to deposit.
    scenario
        .send(&[scenario.materialize(0, 7)], &[&actor])
        .await;

    // A Hoard-shaped account whose owner authority is the derived PDA.
    let hoard_account = Keypair::new();
    let lamports = scenario.rent_for(BASE_TOKEN_ACCOUNT_LEN).await;
    let hoard_authority = scenario.plane.hoard_authority.address;
    scenario
        .send(
            &[
                system_instruction::create_account(
                    &scenario.payer.pubkey(),
                    &hoard_account.pubkey(),
                    lamports,
                    BASE_TOKEN_ACCOUNT_LEN as u64,
                    &TOKEN_2022,
                ),
                token_instruction::initialize_account3(
                    &TOKEN_2022,
                    &hoard_account.pubkey(),
                    &mint,
                    &hoard_authority,
                )
                .unwrap(),
            ],
            &[&hoard_account],
        )
        .await;

    // Deposit: a user-signed transfer in needs no program signature.
    scenario
        .send(
            &[token_instruction::transfer_checked(
                &TOKEN_2022,
                &scenario.holder,
                &mint,
                &hoard_account.pubkey(),
                &actor.pubkey(),
                &[],
                5,
                0,
            )
            .unwrap()],
            &[&actor],
        )
        .await;

    // Withdrawal: the same signature cannot take it back out.
    let code = scenario
        .refusal_code(
            &[token_instruction::transfer_checked(
                &TOKEN_2022,
                &hoard_account.pubkey(),
                &mint,
                &scenario.holder,
                &actor.pubkey(),
                &[],
                5,
                0,
            )
            .unwrap()],
            &[&actor],
        )
        .await;
    assert_eq!(code, TokenError::OwnerMismatch as u32);
    println!(
        "SVM hoard outflow: wallet-signed withdrawal from the derived hoard authority \
         refused with Custom({code})"
    );
}

/// **The transitional hole, closed.**
///
/// A ten-account `Materialize` used to be accepted and to move only the
/// shadow, leaving the market-wide ledger term disagreeing with the mint's
/// supply — the divergence the thirteen-account plane then refused to build
/// on. That optionality existed because nothing created the mints;
/// `CreateMarket` now does, so the count is fixed and the smaller plane is
/// `AccountCount`.
///
/// Its replacement measures the *closure* the same way the hole was measured:
/// the shadow-only plane is refused, and after the refusal the two truths are
/// still equal, because nothing moved.
#[tokio::test]
async fn the_ten_account_plane_is_refused_and_moves_nothing() {
    let mut scenario = Scenario::start(MintShape::Proposed).await;
    let actor = scenario.actor.insecure_clone();
    let request = layout_request(
        0,
        Intent::Materialize {
            market: scenario.plane.market_id,
            owner: Hash32::from_bytes(actor.pubkey().to_bytes()),
            destination: Hash32::from_bytes(scenario.plane.external.address.to_bytes()),
            outcome: OUTCOME,
            quantity: 7,
        },
    );
    let code = scenario
        .refusal_code(&[scenario.seam_instruction(request, false)], &[&actor])
        .await;
    assert_eq!(
        code,
        ClutchError::AccountCount as u32,
        "the shadow-only plane is not a plane any more"
    );

    assert_eq!(scenario.mint_supply().await, 0, "no token was minted");
    assert_eq!(
        scenario.external_supply().await,
        0,
        "the shadow did not move"
    );
    assert_eq!(
        scenario.external_supply().await,
        scenario.mint_supply().await,
        "the two truths cannot be made to disagree by presenting a smaller plane"
    );

    // And the plane the intent does name still works from the same state.
    let units = scenario
        .send(&[scenario.materialize(0, 7)], &[&actor])
        .await;
    assert_eq!(scenario.mint_supply().await, 7);
    println!(
        "SVM mandatory leg: the ten-account materialize refused with Custom({code}) (0x0001); \
         the thirteen-account one accepted at {units} CU"
    );
}
