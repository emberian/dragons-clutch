//! Token-2022 probe scenarios, executed by the in-process Agave bank that
//! `solana-program-test` starts.
//!
//! Every fact asserted below was produced by the real Token-2022 program ELF
//! that `solana-program-binaries` installs at genesis, not by a model of it.
//! What that does establish: Token-2022 mints, token accounts, mint, burn and
//! transfer are reachable in this environment, atoms move the way the adapter's
//! conservation obligations assume, and the `COLLATERAL_PROFILES.md` matrix
//! refuses a hostile mint when it is run against bytes that program wrote.
//!
//! What it does **not** establish: nothing here is a Dragon's Clutch program.
//! There is no CPI, no PDA signature, no clutch instruction, no kernel, and no
//! claim about the adapter's correctness.  An in-process bank is not a cluster.

use {
    clutch_token2022_probe::{
        admit_mint, admit_token_account, observe_mint, observe_token_account, MintPolicy,
        RefusalCode, TokenAccountPolicy,
    },
    solana_address::Address,
    solana_instruction::Instruction,
    solana_keypair::Keypair,
    solana_program_test::{tokio, BanksClient, ProgramTest},
    solana_signer::Signer,
    solana_system_interface::instruction as system_instruction,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
    spl_token_2022_interface::{
        error::TokenError,
        extension::{
            transfer_fee::{instruction::initialize_transfer_fee_config, TransferFeeAmount},
            BaseStateWithExtensions, ExtensionType, StateWithExtensions,
        },
        instruction as token_instruction,
        instruction::AuthorityType,
        state::{Account as TokenAccount, Mint},
    },
};

const DECIMALS: u8 = 6;
const CEILING: u64 = 1_000_000_000_000_000;

fn token_2022_id() -> Address {
    spl_token_2022_interface::id()
}

struct Probe {
    banks: BanksClient,
    payer: Keypair,
}

impl Probe {
    async fn start() -> Self {
        let (banks, payer, _blockhash) = ProgramTest::default().start().await;
        Self { banks, payer }
    }

    /// Send one transaction, returning the compute units the bank metered.
    async fn send(&mut self, instructions: &[Instruction], signers: &[&Keypair]) -> u64 {
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
        outcome.result.expect("transaction was expected to succeed");
        outcome
            .metadata
            .map(|m| m.compute_units_consumed)
            .unwrap_or_default()
    }

    /// Send one transaction that is expected to be refused, returning the
    /// custom program error code the token program produced.
    async fn send_expecting_refusal(
        &mut self,
        instructions: &[Instruction],
        signers: &[&Keypair],
    ) -> u32 {
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
        match outcome.result {
            Ok(()) => panic!("transaction was expected to be refused, and was not"),
            Err(TransactionError::InstructionError(_, error)) => match error {
                solana_instruction::error::InstructionError::Custom(code) => code,
                other => panic!("expected a custom token error, got {other:?}"),
            },
            Err(other) => panic!("expected an instruction error, got {other:?}"),
        }
    }

    async fn account_data(&mut self, address: Address) -> Vec<u8> {
        self.banks
            .get_account(address)
            .await
            .unwrap()
            .expect("account should exist")
            .data
    }

    async fn account_owner(&mut self, address: Address) -> Address {
        self.banks
            .get_account(address)
            .await
            .unwrap()
            .expect("account should exist")
            .owner
    }

    async fn rent_for(&mut self, space: usize) -> u64 {
        self.banks
            .get_rent()
            .await
            .unwrap()
            .minimum_balance(space)
            .max(1)
    }

    /// Create and initialize a mint with the given mint extensions.
    async fn create_mint(
        &mut self,
        extensions: &[ExtensionType],
        mint_authority: &Address,
        freeze_authority: Option<&Address>,
    ) -> Keypair {
        let mint = Keypair::new();
        let space = ExtensionType::try_calculate_account_len::<Mint>(extensions).unwrap();
        let lamports = self.rent_for(space).await;
        let mut instructions = vec![system_instruction::create_account(
            &self.payer.pubkey(),
            &mint.pubkey(),
            lamports,
            space as u64,
            &token_2022_id(),
        )];
        // Extension initialization must precede `InitializeMint`.
        for extension in extensions {
            match extension {
                ExtensionType::TransferFeeConfig => instructions.push(
                    initialize_transfer_fee_config(
                        &token_2022_id(),
                        &mint.pubkey(),
                        Some(mint_authority),
                        Some(mint_authority),
                        100, // 1.00%
                        1_000_000,
                    )
                    .unwrap(),
                ),
                ExtensionType::MintCloseAuthority => instructions.push(
                    token_instruction::initialize_mint_close_authority(
                        &token_2022_id(),
                        &mint.pubkey(),
                        Some(mint_authority),
                    )
                    .unwrap(),
                ),
                other => panic!("probe does not know how to initialize {other:?}"),
            }
        }
        instructions.push(
            token_instruction::initialize_mint2(
                &token_2022_id(),
                &mint.pubkey(),
                mint_authority,
                freeze_authority,
                DECIMALS,
            )
            .unwrap(),
        );
        self.send(&instructions, &[&mint]).await;
        mint
    }

    /// Revoke the mint authority, which is what a real collateral mint must
    /// have done before any Realm can admit it.
    async fn revoke_mint_authority(&mut self, mint: &Address, authority: &Keypair) {
        self.send(
            &[token_instruction::set_authority(
                &token_2022_id(),
                mint,
                None,
                AuthorityType::MintTokens,
                &authority.pubkey(),
                &[],
            )
            .unwrap()],
            &[authority],
        )
        .await;
    }

    /// Create and initialize a token account, including whatever account
    /// extensions the mint's own extensions require.
    async fn create_token_account(
        &mut self,
        mint: &Address,
        owner: &Address,
        mint_extensions: &[ExtensionType],
        extra_extensions: &[ExtensionType],
    ) -> Keypair {
        let account = Keypair::new();
        let mut extensions = {
            #[allow(deprecated)]
            ExtensionType::get_required_init_account_extensions(mint_extensions)
        };
        for extension in extra_extensions {
            if !extensions.contains(extension) {
                extensions.push(*extension);
            }
        }
        let space = ExtensionType::try_calculate_account_len::<TokenAccount>(&extensions).unwrap();
        let lamports = self.rent_for(space).await;
        let mut instructions = vec![system_instruction::create_account(
            &self.payer.pubkey(),
            &account.pubkey(),
            lamports,
            space as u64,
            &token_2022_id(),
        )];
        if extensions.contains(&ExtensionType::ImmutableOwner) {
            instructions.push(
                token_instruction::initialize_immutable_owner(&token_2022_id(), &account.pubkey())
                    .unwrap(),
            );
        }
        instructions.push(
            token_instruction::initialize_account3(
                &token_2022_id(),
                &account.pubkey(),
                mint,
                owner,
            )
            .unwrap(),
        );
        self.send(&instructions, &[&account]).await;
        account
    }
}

/// Obligation 6, the shape of it: materialize is a mint, dematerialize is a
/// burn, and the mint's `supply` field is the only supply truth.  At every
/// step, supply equals the sum of the holder balances.
#[tokio::test]
async fn outcome_mint_lifecycle_conserves_atoms() {
    let mut probe = Probe::start().await;
    let authority = Keypair::new();
    let holder_a = Keypair::new();
    let holder_b = Keypair::new();

    // The outcome-mint shape: the program holds mint authority, no freeze
    // authority exists, and there are no extensions.
    let mint = probe
        .create_mint(&[], &authority.pubkey(), None)
        .await
        .pubkey();
    let account_a = probe
        .create_token_account(&mint, &holder_a.pubkey(), &[], &[])
        .await
        .pubkey();
    let account_b = probe
        .create_token_account(&mint, &holder_b.pubkey(), &[], &[])
        .await
        .pubkey();

    let assert_closed = |supply: u64, a: u64, b: u64| {
        assert_eq!(
            supply,
            a.checked_add(b).unwrap(),
            "supply must equal the sum of every balance"
        );
    };

    // materialize
    let mint_cu = probe
        .send(
            &[token_instruction::mint_to(
                &token_2022_id(),
                &mint,
                &account_a,
                &authority.pubkey(),
                &[],
                1_000,
            )
            .unwrap()],
            &[&authority],
        )
        .await;
    let supply = observe_mint(&probe.account_data(mint).await)
        .unwrap()
        .supply;
    let a = observe_token_account(&probe.account_data(account_a).await)
        .unwrap()
        .amount;
    let b = observe_token_account(&probe.account_data(account_b).await)
        .unwrap()
        .amount;
    assert_eq!((supply, a, b), (1_000, 1_000, 0));
    assert_closed(supply, a, b);

    // an ordinary holder-to-holder transfer: supply unchanged, no fee taken
    let transfer_cu = probe
        .send(
            &[token_instruction::transfer_checked(
                &token_2022_id(),
                &account_a,
                &mint,
                &account_b,
                &holder_a.pubkey(),
                &[],
                400,
                DECIMALS,
            )
            .unwrap()],
            &[&holder_a],
        )
        .await;
    let supply = observe_mint(&probe.account_data(mint).await)
        .unwrap()
        .supply;
    let a = observe_token_account(&probe.account_data(account_a).await)
        .unwrap()
        .amount;
    let b = observe_token_account(&probe.account_data(account_b).await)
        .unwrap()
        .amount;
    assert_eq!((supply, a, b), (1_000, 600, 400));
    assert_closed(supply, a, b);

    // dematerialize
    let burn_cu = probe
        .send(
            &[token_instruction::burn(
                &token_2022_id(),
                &account_b,
                &mint,
                &holder_b.pubkey(),
                &[],
                250,
            )
            .unwrap()],
            &[&holder_b],
        )
        .await;
    let supply = observe_mint(&probe.account_data(mint).await)
        .unwrap()
        .supply;
    let a = observe_token_account(&probe.account_data(account_a).await)
        .unwrap()
        .amount;
    let b = observe_token_account(&probe.account_data(account_b).await)
        .unwrap()
        .amount;
    assert_eq!((supply, a, b), (750, 600, 150));
    assert_closed(supply, a, b);

    println!(
        "PROBE outcome_mint_lifecycle: mint_to={mint_cu} CU  transfer_checked={transfer_cu} CU  \
         burn={burn_cu} CU  (bare instruction, no CPI frame)"
    );
}

/// A base Token-2022 mint with both authorities revoked is admitted by the V1
/// collateral profile, decoded from bytes the token program wrote.
#[tokio::test]
async fn base_mint_is_admitted_as_collateral() {
    let mut probe = Probe::start().await;
    let authority = Keypair::new();
    let holder = Keypair::new();

    let mint = probe
        .create_mint(&[], &authority.pubkey(), None)
        .await
        .pubkey();
    let account = probe
        .create_token_account(&mint, &holder.pubkey(), &[], &[])
        .await
        .pubkey();
    probe
        .send(
            &[token_instruction::mint_to(
                &token_2022_id(),
                &mint,
                &account,
                &authority.pubkey(),
                &[],
                5_000_000,
            )
            .unwrap()],
            &[&authority],
        )
        .await;

    let policy = MintPolicy::v1_collateral(token_2022_id(), mint, DECIMALS, CEILING);

    // Before the mint authority is revoked the profile refuses: a live mint
    // authority means the collateral supply is not fixed.
    let owner = probe.account_owner(mint).await;
    let data = probe.account_data(mint).await;
    let refusal = admit_mint(&owner, &mint, &data, &policy).unwrap_err();
    assert_eq!(refusal.code, RefusalCode::MintAuthorityPresent);

    // Revoke it, and the same bytes are admitted.
    probe
        .send(
            &[token_instruction::set_authority(
                &token_2022_id(),
                &mint,
                None,
                AuthorityType::MintTokens,
                &authority.pubkey(),
                &[],
            )
            .unwrap()],
            &[&authority],
        )
        .await;
    let data = probe.account_data(mint).await;
    let observation =
        admit_mint(&owner, &mint, &data, &policy).expect("base mint must be admitted");
    assert_eq!(observation.decimals, DECIMALS);
    assert_eq!(observation.supply, 5_000_000);
    assert_eq!(observation.mint_authority, None);
    assert_eq!(observation.freeze_authority, None);
    assert!(observation.extensions.is_empty());

    // A profile naming a different mint refuses the same bytes: decoding is
    // not admission.
    let elsewhere = MintPolicy::v1_collateral(
        token_2022_id(),
        Address::new_from_array([7u8; 32]),
        DECIMALS,
        CEILING,
    );
    assert_eq!(
        admit_mint(&owner, &mint, &data, &elsewhere)
            .unwrap_err()
            .code,
        RefusalCode::WrongMint
    );

    // So does a profile naming the legacy token program.
    let legacy = MintPolicy::v1_collateral(
        spl_token_2022_interface::inline_spl_token::id(),
        mint,
        DECIMALS,
        CEILING,
    );
    assert_eq!(
        admit_mint(&owner, &mint, &data, &legacy).unwrap_err().code,
        RefusalCode::WrongProgram
    );

    println!("PROBE base_mint_is_admitted_as_collateral: {observation:?}");

    // And the token account under the V1 Hoard policy.
    let account_policy = TokenAccountPolicy::v1_hoard(token_2022_id(), mint, holder.pubkey());
    let account_owner = probe.account_owner(account).await;
    let account_data = probe.account_data(account).await;
    let account_observation =
        admit_token_account(&account_owner, &account_data, &account_policy).unwrap();
    assert_eq!(account_observation.amount, 5_000_000);
    assert!(account_observation.extensions.is_empty());
}

/// The load-bearing refusal.  `TransferFeeConfig` is row 1 of the V1 matrix in
/// `COLLATERAL_PROFILES.md`, refused because "recipient receives net atoms".
/// This test does not take that on faith: it creates the mint, runs the exact
/// admission check the adapter will run, and then demonstrates the atom
/// inequality the refusal exists to prevent.
#[tokio::test]
async fn transfer_fee_mint_is_refused_and_would_break_conservation() {
    let mut probe = Probe::start().await;
    let authority = Keypair::new();
    let sender = Keypair::new();
    let hoard_authority = Keypair::new();

    let mint_extensions = [ExtensionType::TransferFeeConfig];
    let mint = probe
        .create_mint(&mint_extensions, &authority.pubkey(), None)
        .await
        .pubkey();
    let source = probe
        .create_token_account(&mint, &sender.pubkey(), &mint_extensions, &[])
        .await
        .pubkey();
    // Give the mint a supply first.  The profile refuses a zero-supply mint
    // before it ever looks at extensions, and this test is about the extension
    // refusal, so the earlier refusal must not be what fires.
    probe
        .send(
            &[token_instruction::mint_to(
                &token_2022_id(),
                &mint,
                &source,
                &authority.pubkey(),
                &[],
                10_000_000,
            )
            .unwrap()],
            &[&authority],
        )
        .await;

    // 1. The admission check refuses, and names the offending extension.
    //    The policy is layered and ordered exactly as
    //    `research/collateral-profiles/model.py`: authority faults are reported
    //    before extension faults, so the mint authority has to go before the
    //    extension refusal is the one that fires.
    let owner = probe.account_owner(mint).await;
    let data = probe.account_data(mint).await;
    let policy = MintPolicy::v1_collateral(token_2022_id(), mint, DECIMALS, CEILING);
    assert_eq!(
        admit_mint(&owner, &mint, &data, &policy).unwrap_err().code,
        RefusalCode::MintAuthorityPresent
    );
    probe.revoke_mint_authority(&mint, &authority).await;
    let data = probe.account_data(mint).await;
    let refusal = admit_mint(&owner, &mint, &data, &policy).unwrap_err();
    assert_eq!(refusal.code, RefusalCode::ExtensionNotAllowed);
    assert_eq!(refusal.extension, Some(ExtensionType::TransferFeeConfig));

    // The refusal is not an artifact of decoding failing: the mint decodes
    // fine, and reports the extension.
    let observation = observe_mint(&data).unwrap();
    assert_eq!(
        observation.extensions,
        vec![ExtensionType::TransferFeeConfig]
    );

    // 2. Why it matters.  Deposit 1_000_000 atoms into a Hoard-shaped account
    //    and count what actually arrives.
    let hoard = probe
        .create_token_account(&mint, &hoard_authority.pubkey(), &mint_extensions, &[])
        .await
        .pubkey();

    let sent: u64 = 1_000_000;
    probe
        .send(
            &[token_instruction::transfer_checked(
                &token_2022_id(),
                &source,
                &mint,
                &hoard,
                &sender.pubkey(),
                &[],
                sent,
                DECIMALS,
            )
            .unwrap()],
            &[&sender],
        )
        .await;

    let hoard_data = probe.account_data(hoard).await;
    let hoard_state = StateWithExtensions::<TokenAccount>::unpack(&hoard_data).unwrap();
    let credited = hoard_state.base.amount;
    let withheld: u64 = hoard_state
        .get_extension::<TransferFeeAmount>()
        .unwrap()
        .withheld_amount
        .into();

    // The Hoard's spendable balance is strictly less than what was sent, and
    // the difference is parked in a sub-balance the Hoard cannot spend and the
    // mint's withdraw authority can take.
    assert!(
        credited < sent,
        "a transfer-fee mint must credit fewer atoms than were sent"
    );
    assert_eq!(credited, 990_000);
    assert_eq!(withheld, 10_000);
    assert_eq!(credited + withheld, sent);

    // And the Hoard account itself is refused by the account-side policy,
    // because `TransferFeeAmount` is row 2 of the same matrix.
    let hoard_owner = probe.account_owner(hoard).await;
    let account_policy =
        TokenAccountPolicy::v1_hoard(token_2022_id(), mint, hoard_authority.pubkey());
    let account_refusal =
        admit_token_account(&hoard_owner, &hoard_data, &account_policy).unwrap_err();
    assert_eq!(account_refusal.code, RefusalCode::ExtensionNotAllowed);
    assert_eq!(
        account_refusal.extension,
        Some(ExtensionType::TransferFeeAmount)
    );

    println!(
        "PROBE transfer_fee: sent={sent} credited={credited} withheld={withheld} \
         refusal={refusal:?} account_refusal={account_refusal:?}"
    );
}

/// A second matrix row, to show the refusal is a policy and not a special case
/// for fees.  `MintCloseAuthority` is row 3: a zero-supply mint can be closed
/// and reinitialized with different extensions.
#[tokio::test]
async fn mint_close_authority_mint_is_refused() {
    let mut probe = Probe::start().await;
    let authority = Keypair::new();
    let holder = Keypair::new();
    let extensions = [ExtensionType::MintCloseAuthority];
    let mint = probe
        .create_mint(&extensions, &authority.pubkey(), None)
        .await
        .pubkey();
    let account = probe
        .create_token_account(&mint, &holder.pubkey(), &extensions, &[])
        .await
        .pubkey();
    probe
        .send(
            &[token_instruction::mint_to(
                &token_2022_id(),
                &mint,
                &account,
                &authority.pubkey(),
                &[],
                1_000,
            )
            .unwrap()],
            &[&authority],
        )
        .await;

    probe.revoke_mint_authority(&mint, &authority).await;

    let owner = probe.account_owner(mint).await;
    let data = probe.account_data(mint).await;
    let policy = MintPolicy::v1_collateral(token_2022_id(), mint, DECIMALS, CEILING);
    let refusal = admit_mint(&owner, &mint, &data, &policy).unwrap_err();
    assert_eq!(refusal.code, RefusalCode::ExtensionNotAllowed);
    assert_eq!(refusal.extension, Some(ExtensionType::MintCloseAuthority));
    println!("PROBE mint_close_authority: refusal={refusal:?}");
}

/// The Hoard's account shape.  `ImmutableOwner` is the single admitted account
/// extension, the Hoard authority is a program address, and a wallet signature
/// cannot move collateral out of it — which is the whole reason a redemption
/// has to be a CPI signed by the program, not a user-signed transfer.
#[tokio::test]
async fn pda_owned_hoard_admits_immutable_owner_and_refuses_wallet_withdrawal() {
    let mut probe = Probe::start().await;
    let authority = Keypair::new();
    let depositor = Keypair::new();

    // A stand-in for the adapter's Hoard authority PDA.  The probe defines no
    // clutch seeds; the point is only that the authority is an address nobody
    // can sign for off chain.
    let fake_program_id = Address::new_from_array([9u8; 32]);
    let (hoard_authority, _bump) =
        Address::find_program_address(&[b"probe:hoard-authority"], &fake_program_id);

    let mint = probe
        .create_mint(&[], &authority.pubkey(), None)
        .await
        .pubkey();
    let hoard = probe
        .create_token_account(
            &mint,
            &hoard_authority,
            &[],
            &[ExtensionType::ImmutableOwner],
        )
        .await
        .pubkey();
    let depositor_account = probe
        .create_token_account(&mint, &depositor.pubkey(), &[], &[])
        .await
        .pubkey();

    probe
        .send(
            &[token_instruction::mint_to(
                &token_2022_id(),
                &mint,
                &depositor_account,
                &authority.pubkey(),
                &[],
                1_000_000,
            )
            .unwrap()],
            &[&authority],
        )
        .await;

    // Deposit: an ordinary user-signed transfer in.  No program signature is
    // needed to move collateral *into* the Hoard.
    probe
        .send(
            &[token_instruction::transfer_checked(
                &token_2022_id(),
                &depositor_account,
                &mint,
                &hoard,
                &depositor.pubkey(),
                &[],
                600_000,
                DECIMALS,
            )
            .unwrap()],
            &[&depositor],
        )
        .await;

    let hoard_owner = probe.account_owner(hoard).await;
    let hoard_data = probe.account_data(hoard).await;
    let policy = TokenAccountPolicy::v1_hoard(token_2022_id(), mint, hoard_authority);
    let observation =
        admit_token_account(&hoard_owner, &hoard_data, &policy).expect("Hoard must be admitted");
    assert_eq!(observation.amount, 600_000);
    assert_eq!(observation.owner, hoard_authority);
    assert_eq!(observation.extensions, vec![ExtensionType::ImmutableOwner]);
    assert_eq!(observation.delegate, None);
    assert_eq!(observation.close_authority, None);

    // A Hoard whose authority is not the expected one is refused even though it
    // is a perfectly well-formed token account.
    let wrong_authority =
        TokenAccountPolicy::v1_hoard(token_2022_id(), mint, Address::new_from_array([3u8; 32]));
    assert_eq!(
        admit_token_account(&hoard_owner, &hoard_data, &wrong_authority)
            .unwrap_err()
            .code,
        RefusalCode::WrongAccountOwner
    );

    // Withdrawal: the depositor cannot take collateral back out by signing for
    // it.  Only an `invoke_signed` from the program that owns the authority
    // seeds can.
    let code = probe
        .send_expecting_refusal(
            &[token_instruction::transfer_checked(
                &token_2022_id(),
                &hoard,
                &mint,
                &depositor_account,
                &depositor.pubkey(),
                &[],
                600_000,
                DECIMALS,
            )
            .unwrap()],
            &[&depositor],
        )
        .await;
    assert_eq!(code, TokenError::OwnerMismatch as u32);

    println!(
        "PROBE pda_owned_hoard: admitted={observation:?} wallet_withdrawal_refused_with=Custom({code})"
    );
}

/// A refusal that cannot go the other way is not evidence of anything.
///
/// This is the same mint, the same bytes, and the same predicate as
/// `transfer_fee_mint_is_refused_and_would_break_conservation`, run under a
/// counterfactual Realm profile that wrongly admits `TransferFeeConfig`.  It is
/// admitted — and the atoms that a Realm on that profile would lose are
/// counted.  V1 forbids this profile: the protocol mint ceiling in
/// `COLLATERAL_PROFILES.md` is empty, so no legal V1 Realm can express it.
/// The test exists so the refusal above is falsifiable rather than vacuous.
#[tokio::test]
async fn a_widened_profile_would_admit_the_fee_mint_and_lose_atoms() {
    let mut probe = Probe::start().await;
    let authority = Keypair::new();
    let sender = Keypair::new();
    let hoard_authority = Keypair::new();

    let mint_extensions = [ExtensionType::TransferFeeConfig];
    let mint = probe
        .create_mint(&mint_extensions, &authority.pubkey(), None)
        .await
        .pubkey();
    let source = probe
        .create_token_account(&mint, &sender.pubkey(), &mint_extensions, &[])
        .await
        .pubkey();
    let hoard = probe
        .create_token_account(&mint, &hoard_authority.pubkey(), &mint_extensions, &[])
        .await
        .pubkey();
    probe
        .send(
            &[token_instruction::mint_to(
                &token_2022_id(),
                &mint,
                &source,
                &authority.pubkey(),
                &[],
                10_000_000,
            )
            .unwrap()],
            &[&authority],
        )
        .await;
    probe.revoke_mint_authority(&mint, &authority).await;

    let mut widened = MintPolicy::v1_collateral(token_2022_id(), mint, DECIMALS, CEILING);
    widened.allowed_extensions = &[ExtensionType::TransferFeeConfig];

    let owner = probe.account_owner(mint).await;
    let data = probe.account_data(mint).await;
    let observation =
        admit_mint(&owner, &mint, &data, &widened).expect("the widened profile admits this mint");
    assert_eq!(
        observation.extensions,
        vec![ExtensionType::TransferFeeConfig]
    );

    // What that admission costs.  A deposit of 2_000_000 atoms leaves the Hoard
    // able to honour only 1_980_000 of them.
    let deposited: u64 = 2_000_000;
    probe
        .send(
            &[token_instruction::transfer_checked(
                &token_2022_id(),
                &source,
                &mint,
                &hoard,
                &sender.pubkey(),
                &[],
                deposited,
                DECIMALS,
            )
            .unwrap()],
            &[&sender],
        )
        .await;
    let hoard_data = probe.account_data(hoard).await;
    let spendable = observe_token_account(&hoard_data).unwrap().amount;
    assert_eq!(spendable, 1_980_000);
    assert_eq!(deposited - spendable, 20_000);

    println!(
        "PROBE widened_profile: admitted={:?} deposited={deposited} spendable={spendable} \
         shortfall={}",
        observation.extensions,
        deposited - spendable
    );
}
