//! Claim-check compaction, executed against the real Claims ELF.
//!
//! These scenarios live beside the representation campaign rather than in a
//! test file of their own, because everything a claim-check needs — a founded
//! market that actually went terminal, a funded Hoard, a real Custody replay,
//! a wallet position with claims at the winning coordinate — is already built
//! there. Forking that scaffold to get an independent file would produce a
//! second author for the whole fixture, and a lane that verifies against its
//! own reconstruction has verified nothing.

use super::*;

use solana_program_test::ProgramTestBanksClientExt;

use dclutch_claims_svm::claim_check_request_v1::OpenClaimCheckEscrowRequestV1;
use dclutch_claims_svm::claim_check_v1::{
    CLAIM_CHECK_ESCROW_BYTES_V1, ClaimCheckEscrowSeedsV1, ClaimCheckEscrowV1,
    ClaimCheckVaultSeedsV1,
};

/// Build the 12-account open-escrow frame the route declares.
///
/// The opener is the fee payer rather than the fixture's actor, which is not
/// incidental: the actor is funded for the replay rent it prepays and the
/// escrow's two accounts cost more than that. It also makes the point the route
/// is for — the opener is *anybody*, holding no role and standing in no
/// relation to the market.
fn open_escrow_instruction(
    fixture: &Fixture,
    opener: Pubkey,
    overrides: OpenOverrides,
) -> Instruction {
    let terminal = fixture.terminal_accounts.expect("terminal fixture");
    let request = OpenClaimCheckEscrowRequestV1 {
        release_set: fixture.release_set,
        market: overrides.market.unwrap_or(fixture.market.to_bytes()),
        realm: fixture.realm_id,
        generation: GENERATION,
    }
    .new()
    .expect("open request");
    let aggregate = overrides.aggregate.unwrap_or(fixture.aggregate);
    Instruction {
        program_id: CLAIMS_PROGRAM_ID,
        accounts: Vec::from([
            AccountMeta::new(opener, true),
            AccountMeta::new(
                overrides
                    .escrow
                    .unwrap_or_else(|| escrow_address(aggregate)),
                false,
            ),
            AccountMeta::new(
                overrides.vault.unwrap_or_else(|| vault_address(aggregate)),
                false,
            ),
            AccountMeta::new_readonly(aggregate, false),
            AccountMeta::new_readonly(fixture.market, false),
            AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
            AccountMeta::new_readonly(terminal.realm_raw, false),
            AccountMeta::new_readonly(terminal.realm_staging, false),
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            AccountMeta::new_readonly(
                overrides
                    .collateral_mint
                    .unwrap_or(terminal.collateral_mint),
                false,
            ),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ]),
        data: request.to_bytes().expect("open bytes").to_vec(),
    }
}

#[derive(Clone, Copy, Default)]
struct OpenOverrides {
    escrow: Option<Pubkey>,
    vault: Option<Pubkey>,
    aggregate: Option<Pubkey>,
    market: Option<[u8; 32]>,
    collateral_mint: Option<Pubkey>,
}

fn escrow_address(aggregate: Pubkey) -> Pubkey {
    let seeds = ClaimCheckEscrowSeedsV1::new(aggregate.to_bytes()).expect("escrow seeds");
    Pubkey::find_program_address(&seeds.as_slices(), &CLAIMS_PROGRAM_ID).0
}

fn vault_address(aggregate: Pubkey) -> Pubkey {
    let seeds = ClaimCheckVaultSeedsV1::new(aggregate.to_bytes()).expect("vault seeds");
    Pubkey::find_program_address(&seeds.as_slices(), &CLAIMS_PROGRAM_ID).0
}

/// Submit one open-escrow instruction paid by the context's payer.
async fn submit_open(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    overrides: OpenOverrides,
    label: &str,
) -> Submission {
    let opener = context.payer.pubkey();
    let instruction = open_escrow_instruction(fixture, opener, overrides);
    submit_opener_signed(context, instruction, label).await
}

/// Send one transaction signed by the payer ALONE.
///
/// Deliberately not `submit_legacy_signed`, which co-signs with the fixture's
/// actor. This route admits exactly one signer and the actor is not it, so
/// co-signing would either be rejected by the runtime as an unused keypair or,
/// worse, quietly satisfy a signature the route is supposed to refuse. A test
/// for "only the opener signs" must not itself smuggle a second signature in.
async fn submit_opener_signed(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    label: &str,
) -> Submission {
    let blockhash = context
        .banks_client
        .get_new_latest_blockhash(&context.last_blockhash)
        .await
        .expect("fresh blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer],
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .expect("signed transaction")
        .to_string();
    let wire_bytes = 1 + transaction.signatures.len() * 64 + transaction.message_data().len();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("legacy transaction processing");
    let accepted = processed.result.is_ok();
    let failure = processed.result.err().map(|error| format!("{error:?}"));
    let (logs, compute_units) = processed
        .metadata
        .map(|metadata| (metadata.log_messages, metadata.compute_units_consumed))
        .unwrap_or_default();
    dclutch_program_test_evidence::record(&TransactionEvidence {
        label,
        signature: &signature,
        slot,
        error: failure.as_deref(),
        logs: &logs,
        compute_units_consumed: Some(compute_units),
        wire_bytes: Some(wire_bytes),
    })
    .expect("campaign evidence must be writable when the gauntlet asked for it");
    Submission {
        accepted,
        compute_units,
        wire_bytes,
        logs,
    }
}

async fn maybe_observed(
    context: &mut ProgramTestContext,
    key: Pubkey,
) -> Option<solana_account::Account> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account lookup")
}

/// THE ESCROW OPENS ON A TERMINAL MARKET, PAID BY A STRANGER.
///
/// The first half of the answer to "one sleeping holder blocks retirement
/// forever": before anybody can be compacted, somebody who is nobody has to be
/// able to start the clock, and to be recorded as owed what they advanced.
#[tokio::test]
async fn a_stranger_opens_the_claim_check_escrow_on_a_terminal_market() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    let opener = context.payer.pubkey();
    let escrow = escrow_address(fixture.aggregate);
    let vault = vault_address(fixture.aggregate);

    assert!(
        maybe_observed(&mut context, escrow).await.is_none(),
        "the escrow address is vacant before anybody opens it"
    );
    let opener_before = observed(&mut context, opener).await.lamports;

    let result = submit_open(
        &mut context,
        &fixture,
        OpenOverrides::default(),
        "claims claim-check: a stranger opens the escrow on a terminal market",
    )
    .await;
    if !result.accepted {
        eprintln!("open refusal logs:\n{}", result.logs.join("\n"));
    }
    assert!(result.accepted, "the open must commit");

    // The record says what it is, and says who is owed for it.
    let account = observed(&mut context, escrow).await;
    assert_eq!(account.owner, CLAIMS_PROGRAM_ID);
    assert_eq!(account.data.len(), CLAIM_CHECK_ESCROW_BYTES_V1);
    let record = ClaimCheckEscrowV1::decode(&account.data).expect("escrow decodes");
    assert_eq!(record.aggregate, fixture.aggregate.to_bytes());
    assert_eq!(record.market, fixture.market.to_bytes());
    assert_eq!(record.release_set, fixture.release_set);
    assert_eq!(record.vault, vault.to_bytes());
    assert_eq!(record.opener, opener.to_bytes());
    assert_eq!(record.generation, GENERATION);
    assert_eq!(
        record.outstanding_claim_checks, 0,
        "no claim-check exists until a position is compacted"
    );

    // The vault is an ordinary token account whose AUTHORITY is the escrow PDA.
    // That is the whole of what Custody requires of a transfer destination, and
    // it is why no Custody change was needed.
    let vault_account = observed(&mut context, vault).await;
    assert_eq!(vault_account.owner, TOKEN_PROGRAM_ID);
    assert_eq!(token_amount(&vault_account), 0);

    // Lamports close: the opener paid exactly the two rents, and the record
    // carries exactly that as the debt the cranks will repay.
    let terminal_mint = fixture
        .terminal_accounts
        .expect("terminal fixture")
        .collateral_mint;
    assert_eq!(
        observed(&mut context, terminal_mint).await.owner,
        TOKEN_PROGRAM_ID,
        "the vault was opened against the Realm's own collateral mint"
    );
    let advanced = account.lamports + vault_account.lamports;
    assert_eq!(record.opener_outlay, advanced);
    let opener_after = observed(&mut context, opener).await.lamports;
    assert!(
        opener_before - opener_after >= advanced,
        "the opener funded both accounts out of its own balance"
    );

    // The clock starts here and nowhere else.
    let clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock sysvar");
    assert!(
        record.opened_slot <= clock.slot,
        "the origin is a slot that has happened"
    );
}

/// THE ESCROW ALSO OPENS AFTER A STRANGER HAS PUSHED THE MARKET TO RETIRING.
///
/// This is not a convenience case. `begin_retiring` is permissionless, so a
/// market can be in `Retiring` because somebody put it there; a compaction
/// route that refused in that phase would leave exactly those markets
/// unrescuable, which re-creates the hostage-taking one phase later.
#[tokio::test]
async fn the_escrow_opens_on_a_market_a_stranger_pushed_into_retiring() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;

    a_stranger_begins_retiring(&mut context, &fixture).await;
    assert_eq!(
        core_phase(&mut context, fixture.market).await,
        CorePhase::Retiring,
        "the stranger's transition landed"
    );

    let result = submit_open(
        &mut context,
        &fixture,
        OpenOverrides::default(),
        "claims claim-check: the escrow opens on a retiring market",
    )
    .await;
    if !result.accepted {
        eprintln!("retiring open refusal logs:\n{}", result.logs.join("\n"));
    }
    assert!(
        result.accepted,
        "a market a stranger retired must still be openable"
    );
    let escrow = observed(&mut context, escrow_address(fixture.aggregate)).await;
    ClaimCheckEscrowV1::decode(&escrow.data).expect("escrow decodes");
}

/// A SECOND OPEN IS REFUSED, BECAUSE IT WOULD RESTART THE CLOCK.
///
/// The deadline is the only thing standing between a live market and a crank,
/// so an escrow anybody could re-open is a delay anybody could impose — the
/// precise shape of the defect this design exists to remove.
#[tokio::test]
async fn a_second_open_cannot_restart_the_compaction_clock() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;

    let first = submit_open(
        &mut context,
        &fixture,
        OpenOverrides::default(),
        "claims claim-check: the first open",
    )
    .await;
    assert!(first.accepted, "the first open must commit");
    let opened = ClaimCheckEscrowV1::decode(
        &observed(&mut context, escrow_address(fixture.aggregate))
            .await
            .data,
    )
    .expect("escrow decodes");

    let second = submit_open(
        &mut context,
        &fixture,
        OpenOverrides::default(),
        "claims claim-check: a second open is refused",
    )
    .await;
    assert_refused_with(&second, 0x5605, "a second open");

    let after = ClaimCheckEscrowV1::decode(
        &observed(&mut context, escrow_address(fixture.aggregate))
            .await
            .data,
    )
    .expect("escrow decodes");
    assert_eq!(
        after, opened,
        "the refused open moved no byte of the standing escrow, and above all not its origin slot"
    );
}

/// A VAULT UNDER ANY MINT BUT THE REALM'S IS REFUSED.
///
/// A vault opened against the wrong mint could never be paid into — Custody
/// refuses a transfer whose mint is not the Realm's — so it would be an escrow
/// that looks open and can never receive a single atom. The Realm is the sole
/// author of that fact and the route reads it there.
#[tokio::test]
async fn the_escrow_refuses_a_mint_the_realm_does_not_name() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    let foreign = fixture.assets.first().expect("asset").mint;
    let terminal = fixture.terminal_accounts.expect("terminal fixture");
    assert_ne!(
        foreign, terminal.collateral_mint,
        "the substituted mint must actually differ"
    );

    let result = submit_open(
        &mut context,
        &fixture,
        OpenOverrides {
            collateral_mint: Some(foreign),
            ..Default::default()
        },
        "claims claim-check: a mint the Realm does not name",
    )
    .await;
    assert_refused_with(&result, 0x5609, "a foreign collateral mint");
    assert!(
        maybe_observed(&mut context, escrow_address(fixture.aggregate))
            .await
            .is_none(),
        "a refused open leaves no escrow behind"
    );
}

/// COORDINATES THAT DO NOT DERIVE THE ACCOUNTS PASSED ARE REFUSED.
///
/// Both directions matter. An escrow at an address the aggregate does not
/// derive would be findable by nobody; an aggregate that is not this market's
/// would let one market's escrow be opened against another's collateral.
#[tokio::test]
async fn the_escrow_must_live_where_its_aggregate_derives_it() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;

    // An escrow address that is not the one these seeds derive.
    let elsewhere = submit_open(
        &mut context,
        &fixture,
        OpenOverrides {
            escrow: Some(vault_address(fixture.aggregate)),
            ..Default::default()
        },
        "claims claim-check: an escrow at an address its seeds do not derive",
    )
    .await;
    assert_refused_with(&elsewhere, 0x5602, "a misplaced escrow");

    // A vault address that is not the one these seeds derive.
    let wrong_vault = submit_open(
        &mut context,
        &fixture,
        OpenOverrides {
            vault: Some(escrow_address(fixture.aggregate)),
            ..Default::default()
        },
        "claims claim-check: a vault at an address its seeds do not derive",
    )
    .await;
    assert_refused_with(&wrong_vault, 0x5602, "a misplaced vault");

    // An aggregate that is not this market's.
    let foreign_aggregate = submit_open(
        &mut context,
        &fixture,
        OpenOverrides {
            aggregate: Some(fixture.actor_position),
            ..Default::default()
        },
        "claims claim-check: an aggregate that is not this market's",
    )
    .await;
    assert_refused_with(&foreign_aggregate, 0x5602, "a foreign aggregate");

    assert!(
        maybe_observed(&mut context, escrow_address(fixture.aggregate))
            .await
            .is_none(),
        "no refused open left an escrow behind"
    );
}

/// THE ROUTE ADMITS EXACTLY ONE SIGNER, AND REFUSES THE REST RATHER THAN
/// IGNORING THEM.
///
/// A route that merely did not read a signature would still let a caller
/// present one, and a presented signature is a privilege somebody can be
/// induced to grant.
#[tokio::test]
async fn the_open_admits_the_opener_and_no_other_signer() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    let opener = context.payer.pubkey();

    let mut instruction = open_escrow_instruction(&fixture, opener, OpenOverrides::default());
    // The fixture's actor signs this transaction anyway; promoting it inside the
    // frame is what the route must refuse.
    instruction
        .accounts
        .push(AccountMeta::new_readonly(fixture.actor.pubkey(), true));
    let result = submit_legacy_signed(
        &mut context,
        &fixture,
        instruction,
        "claims claim-check: a second signer in the open frame",
    )
    .await;
    // A thirteenth account is refused on frame width before its signature is
    // even reached, which is the stricter of the two refusals and the one that
    // should fire first.
    assert_refused_with(&result, 0x5600, "an over-wide open frame");
}
