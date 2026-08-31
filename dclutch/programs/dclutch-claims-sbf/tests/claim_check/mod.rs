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

// ---------------------------------------------------------------- compaction

use dclutch_claims_svm::claim_check_compaction_request_v1::CompactPositionToClaimCheckRequestV1;
use dclutch_claims_svm::claim_check_v1::{
    COMPACTION_CRANK_REWARD_LAMPORTS_V1, COMPACTION_DEADLINE_SLOTS_V1, ClaimCheckSeedsV1,
    ClaimCheckV1,
};
use dclutch_claims_svm::protocol_position_v2::{
    PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionActionV2,
    ProtocolPositionAdmissionEvidenceV2, ProtocolPositionAdmissionSeedsV2,
    ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
    ProtocolPositionRequestV2,
};
use dclutch_claims_svm::terminal_settlement_v3::{
    TERMINAL_SETTLEMENT_CUSTODY_CALLER_ACCOUNT_V3, TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3,
};

/// LBV2 Position width at this fixture's outcome count: header plus one `u64`
/// per coordinate.
const POSITION_BYTES: usize = 128 + 8 * K;

fn claim_check_address(aggregate: Pubkey, owner: [u8; 32]) -> Pubkey {
    let seeds = ClaimCheckSeedsV1::new(aggregate.to_bytes(), owner).expect("claim-check seeds");
    Pubkey::find_program_address(&seeds.as_slices(), &CLAIMS_PROGRAM_ID).0
}

fn admission_address(aggregate: Pubkey, owner: [u8; 32]) -> Pubkey {
    let seeds = ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), owner)
        .expect("admission seeds");
    Pubkey::find_program_address(&seeds.as_slices(), &CLAIMS_PROGRAM_ID).0
}

/// Plant the admission record a real `Admit` would have created.
///
/// A FIXTURE GAP, named rather than papered over: this campaign plants LBV2
/// Position accounts directly and never runs `protocol_position_v2::Admit`, so
/// no admission record exists for them. On a real chain every admitted Position
/// has one — `Admit` creates the pair and `Close` closes the pair — and
/// compaction has to close it too, or its rent strands. The bytes below are
/// produced by the production codec, never hand-rolled, so the format still has
/// exactly one author; only the act of putting them on chain is short-circuited.
async fn plant_admission(context: &mut ProgramTestContext, fixture: &Fixture) -> Pubkey {
    plant_admission_of_kind(context, fixture, ProtocolPositionOwnerKindV2::User).await
}

/// Plant the same admission record under a caller-chosen owner-kind tag.
///
/// The tag is the only thing compaction reads off this record, and it is the
/// whole of what separates a position that may be compacted from one that may
/// not. Varying it against a fixture that is otherwise known-compactable is
/// what isolates the gate: every other reason to refuse is already disproved
/// by the sibling test that compacts this exact position successfully.
async fn plant_admission_of_kind(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    owner_kind: ProtocolPositionOwnerKindV2,
) -> Pubkey {
    let owner = fixture.actor.pubkey().to_bytes();
    let address = admission_address(fixture.aggregate, owner);
    // The record's own canonicalization binds the descriptor to the kind: a
    // capability owner must name one, and the other two must not. Deriving it
    // here from the kind rather than passing it in keeps the caller honest and
    // keeps this helper producing records the production codec accepts.
    let capability_descriptor = match owner_kind {
        ProtocolPositionOwnerKindV2::ClaimsCapability => [0x4c; 32],
        ProtocolPositionOwnerKindV2::TradingRecord | ProtocolPositionOwnerKindV2::User => [0; 32],
    };
    let request = ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Admit,
        owner_kind,
        presence: ProtocolPositionPresenceV2::Vacant,
        release_set: fixture.release_set,
        market: fixture.market.to_bytes(),
        position_owner: owner,
        parent_request_digest: [0x41; 32],
        rent_credit: market_rent_credit().to_bytes(),
        rent_program: [0x42; 32],
        generation: GENERATION,
        expected_market_revision: 0,
        expected_position_revision: 0,
        observed_position_lamports: Rent::default().minimum_balance(POSITION_BYTES),
        observed_admission_lamports: Rent::default()
            .minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2),
        position_rent_principal: Rent::default().minimum_balance(POSITION_BYTES),
        admission_rent_principal: Rent::default()
            .minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2),
        capability_descriptor,
        capability_outcome: 0,
    }
    .new()
    .expect("admission request");
    let evidence = ProtocolPositionAdmissionEvidenceV2 {
        product_record_digest: [0x43; 32],
        semantic_basis_id: [0x44; 32],
        linked_basis_record_digest: [0x45; 32],
        request_digest: [0x46; 32],
        claims_program: CLAIMS_PROGRAM_ID.to_bytes(),
        trading_program: [0x47; 32],
        capability_descriptor,
        capability_outcome: 0,
        outcome_count: u32::try_from(K).expect("outcome count"),
    };
    let bytes = ProtocolPositionAdmissionV2::new(request, evidence)
        .expect("admission state")
        .to_state_bytes()
        .expect("admission bytes");
    let mut account = Account::new(
        Rent::default().minimum_balance(bytes.len()),
        bytes.len(),
        &CLAIMS_PROGRAM_ID,
    );
    account.data.copy_from_slice(&bytes);
    context.set_account(&address, &AccountSharedData::from(account));
    address
}

/// Build a compaction instruction out of the WALLET PAYOUT's own builder.
///
/// This is the differential stated as construction rather than as a comment.
/// The request is the one the sleeping holder would have sent, obtained from
/// `wallet_payout_request`, with exactly two fields moved: the recipient owner
/// and the recipient token account. The frame is the wallet payout's own 36
/// accounts with the recipient swapped for the vault, plus the six this route
/// adds. Anything that made the holder's redemption pay what it pays is still
/// here, byte for byte, because it was never restated.
fn compaction_instruction(
    fixture: &Fixture,
    cranker: Pubkey,
    prestate: &WalletPayoutPrestate,
) -> Instruction {
    let escrow = escrow_address(fixture.aggregate);
    let vault = vault_address(fixture.aggregate);
    let overrides = WalletPayoutOverrides {
        authority: Some(cranker),
        ..Default::default()
    };
    let mut input = wallet_payout_request(fixture, overrides).input();
    let owner = input.owner;
    input.recipient_owner = escrow.to_bytes();
    input.recipient_token_account = vault.to_bytes();
    let settlement = TerminalSettlementRequestV3::new(input).expect("compaction settlement");
    let request =
        CompactPositionToClaimCheckRequestV1::new(settlement).expect("compaction request");

    let (mut instruction, _) = wallet_payout_instruction(fixture, overrides, prestate);
    instruction.accounts[TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3] = AccountMeta::new(vault, false);
    // The Custody caller authority is derived from the settlement request's own
    // DIGEST, so changing the recipient changes that PDA. Copying the wallet
    // payout's account here would present an authority for a request nobody
    // sent -- which is precisely the join the chain refused when I first got
    // this wrong.
    instruction.accounts[TERMINAL_SETTLEMENT_CUSTODY_CALLER_ACCOUNT_V3] = AccountMeta::new_readonly(
        wallet_payout_custody_caller(fixture, &settlement.to_bytes(), prestate),
        false,
    );
    instruction.data = request.to_bytes().expect("compaction bytes").to_vec();
    instruction.accounts.extend([
        AccountMeta::new(escrow, false),
        AccountMeta::new(claim_check_address(fixture.aggregate, owner), false),
        AccountMeta::new(admission_address(fixture.aggregate, owner), false),
        AccountMeta::new(market_rent_credit(), false),
        AccountMeta::new(cranker, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ]);
    instruction
}

/// Open the escrow and jump the clock past its release-fixed deadline.
async fn open_and_elapse(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> ClaimCheckEscrowV1 {
    let opened = submit_open(
        context,
        fixture,
        OpenOverrides::default(),
        "claims claim-check: open before compaction",
    )
    .await;
    assert!(opened.accepted, "the escrow must open");
    let escrow = ClaimCheckEscrowV1::decode(
        &observed(context, escrow_address(fixture.aggregate))
            .await
            .data,
    )
    .expect("escrow decodes");
    context
        .warp_to_slot(escrow.opened_slot + COMPACTION_DEADLINE_SLOTS_V1)
        .expect("warp past the compaction deadline");
    escrow
}

/// THE DIFFERENTIAL: A COMPACTED CLAIM-CHECK IS WORTH, TO THE ATOM, WHAT THE
/// HOLDER'S OWN REDEMPTION WOULD HAVE PAID.
///
/// The single most important assertion in the feature, and the reason the
/// compaction route calls the payout derivation rather than re-implementing it.
/// A second author for the payoff function is how a compaction that pays a
/// different number gets built and passes its own tests; the number is
/// somebody's money, and this is the test that would catch it.
///
/// Both halves run against the same fixture, so the figure compared is not one
/// this test typed. It is what the chain paid.
#[tokio::test]
async fn a_compacted_claim_check_is_worth_exactly_what_redemption_would_have_paid() {
    // What the holder's own redemption pays, observed on chain.
    let redeemed = {
        let (test, fixture) = fixture(true);
        let mut context = test.start_with_context().await;
        create_claims_custody_replay(&mut context, &fixture).await;
        let (table, addresses) = wallet_payout_lookup_table(
            &mut context,
            &fixture,
            "claims claim-check: differential redemption",
        )
        .await;
        let before = snapshot(&mut context, &fixture).await;
        let result = submit_wallet_payout(
            &mut context,
            &fixture,
            table,
            &addresses,
            WalletPayoutOverrides::default(),
            "claims claim-check: the holder redeems for itself",
        )
        .await;
        assert!(result.accepted, "the sibling redemption must commit");
        let after = snapshot(&mut context, &fixture).await;
        token_amount(after.recipient.as_ref().expect("recipient"))
            - token_amount(before.recipient.as_ref().expect("pre recipient"))
    };
    assert!(redeemed > 0, "the differential needs a paying position");

    // What a stranger's crank escrows for a holder who never came back.
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    plant_admission(&mut context, &fixture).await;
    let cranker = context.payer.pubkey();
    open_and_elapse(&mut context, &fixture).await;

    let prestate = wallet_payout_prestate(&mut context, &fixture, fixture.actor_position).await;
    let instruction = compaction_instruction(&fixture, cranker, &prestate);
    let addresses = lookup_addresses(cranker, fixture.actor.pubkey(), &[instruction.clone()]);
    let (table, _) =
        create_live_lookup_table(&mut context, &addresses, "claims claim-check: compaction").await;
    let result =
        submit_compaction(&mut context, instruction, table, &addresses, "compaction").await;
    if !result.accepted {
        eprintln!("compaction refusal logs:\n{}", result.logs.join("\n"));
    }
    assert!(result.accepted, "the compaction must commit");

    let owner = fixture.actor.pubkey().to_bytes();
    let record = ClaimCheckV1::decode(
        &observed(&mut context, claim_check_address(fixture.aggregate, owner))
            .await
            .data,
    )
    .expect("claim-check decodes");

    // THE ASSERTION.
    assert_eq!(
        record.entitlement_atoms, redeemed,
        "a compacted claim-check must be worth exactly what the holder's own \
         redemption paid; the two paths share one payout derivation and any \
         difference here means they have stopped doing so"
    );
    assert_eq!(record.owner, owner);
    assert_eq!(record.aggregate, fixture.aggregate.to_bytes());
    assert_eq!(record.vault, vault_address(fixture.aggregate).to_bytes());

    // And the collateral is really in the vault, not merely promised.
    let vault = observed(&mut context, vault_address(fixture.aggregate)).await;
    assert_eq!(
        token_amount(&vault),
        record.entitlement_atoms,
        "the sum of live entitlements equals the vault balance"
    );
}

/// THE CRANK IS PAID, AND PAID BEFORE THE OPENER.
///
/// Observed from chain state rather than from the plan. The order matters: one
/// position's rent does not cover a whole escrow's outlay, so repaying the
/// opener first would leave the first crank with exactly nothing, and an
/// unfunded crank is an unturned crank.
#[tokio::test]
async fn the_crank_is_paid_from_rent_that_was_leaving_anyway() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    plant_admission(&mut context, &fixture).await;
    let cranker = context.payer.pubkey();
    let escrow_before = open_and_elapse(&mut context, &fixture).await;

    let position_lamports = observed(&mut context, fixture.actor_position)
        .await
        .lamports;
    let owner = fixture.actor.pubkey().to_bytes();
    let admission = admission_address(fixture.aggregate, owner);
    let admission_lamports = observed(&mut context, admission).await.lamports;
    let rent_credit_before = maybe_observed(&mut context, market_rent_credit())
        .await
        .map_or(0, |account| account.lamports);

    let prestate = wallet_payout_prestate(&mut context, &fixture, fixture.actor_position).await;
    let instruction = compaction_instruction(&fixture, cranker, &prestate);
    let addresses = lookup_addresses(cranker, fixture.actor.pubkey(), &[instruction.clone()]);
    let (table, _) =
        create_live_lookup_table(&mut context, &addresses, "claims claim-check: paid crank").await;
    let cranker_before = observed(&mut context, cranker).await.lamports;
    let result =
        submit_compaction(&mut context, instruction, table, &addresses, "paid crank").await;
    assert!(result.accepted, "the compaction must commit");

    // Both accounts are gone and kept nothing.
    assert!(
        maybe_observed(&mut context, fixture.actor_position)
            .await
            .is_none()
            || observed(&mut context, fixture.actor_position)
                .await
                .lamports
                == 0,
        "the position closed"
    );
    assert!(
        maybe_observed(&mut context, admission).await.is_none()
            || observed(&mut context, admission).await.lamports == 0,
        "the admission closed"
    );

    // The released rent is fully accounted for by four credits and no more.
    let released = position_lamports + admission_lamports;
    let claim_check_lamports =
        observed(&mut context, claim_check_address(fixture.aggregate, owner))
            .await
            .lamports;
    let escrow_after = ClaimCheckEscrowV1::decode(
        &observed(&mut context, escrow_address(fixture.aggregate))
            .await
            .data,
    )
    .expect("escrow decodes");
    let opener_repaid = escrow_before.opener_outlay - escrow_after.opener_outlay;
    let rent_credit_after = maybe_observed(&mut context, market_rent_credit())
        .await
        .map_or(0, |account| account.lamports);
    let residue = rent_credit_after - rent_credit_before;

    // The crank was paid its cap, and paid FIRST -- the opener is still owed.
    assert_eq!(
        COMPACTION_CRANK_REWARD_LAMPORTS_V1,
        released - claim_check_lamports - opener_repaid - residue,
        "the crank's reward is what the sweep paid it"
    );
    assert!(
        escrow_after.opener_outlay > 0,
        "one position's rent cannot repay a whole escrow's outlay, which is why \
         paying the opener first would have left this crank unfunded"
    );
    assert!(opener_repaid > 0, "and the opener's debt still shrank");
    assert_eq!(
        released,
        claim_check_lamports + COMPACTION_CRANK_REWARD_LAMPORTS_V1 + opener_repaid + residue,
        "everything the two closing accounts held is accounted for, and no more"
    );
    // The cranker is up by its reward, net of the fee it paid to turn the crank.
    let cranker_after = observed(&mut context, cranker).await.lamports;
    assert!(
        cranker_after > cranker_before,
        "a permissionless crank nobody is paid to turn is a crank nobody turns"
    );
    assert_eq!(escrow_after.outstanding_claim_checks, 1);
}

/// A CRANK BEFORE THE DEADLINE IS REFUSED.
#[tokio::test]
async fn a_crank_before_the_deadline_is_refused() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    plant_admission(&mut context, &fixture).await;
    let cranker = context.payer.pubkey();
    let opened = submit_open(
        &mut context,
        &fixture,
        OpenOverrides::default(),
        "claims claim-check: open without elapsing",
    )
    .await;
    assert!(opened.accepted);

    let prestate = wallet_payout_prestate(&mut context, &fixture, fixture.actor_position).await;
    let instruction = compaction_instruction(&fixture, cranker, &prestate);
    let addresses = lookup_addresses(cranker, fixture.actor.pubkey(), &[instruction.clone()]);
    let (table, _) =
        create_live_lookup_table(&mut context, &addresses, "claims claim-check: premature").await;
    let result = submit_compaction(
        &mut context,
        instruction,
        table,
        &addresses,
        "premature crank",
    )
    .await;
    assert_refused_with(&result, 0x5603, "a crank before the deadline");
    assert!(
        maybe_observed(
            &mut context,
            claim_check_address(fixture.aggregate, fixture.actor.pubkey().to_bytes())
        )
        .await
        .is_none(),
        "a premature crank mints nothing"
    );
    assert_eq!(
        lbv2_position_quantity(
            &observed(&mut context, fixture.actor_position).await.data,
            WINNER
        ),
        ACTOR_CLAIMS[WINNERS],
        "and debits nothing"
    );
}

/// A SECOND CRANK ON THE SAME POSITION IS REFUSED.
#[tokio::test]
async fn a_position_cannot_be_compacted_twice() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    plant_admission(&mut context, &fixture).await;
    let cranker = context.payer.pubkey();
    open_and_elapse(&mut context, &fixture).await;

    let prestate = wallet_payout_prestate(&mut context, &fixture, fixture.actor_position).await;
    let instruction = compaction_instruction(&fixture, cranker, &prestate);
    let addresses = lookup_addresses(cranker, fixture.actor.pubkey(), &[instruction.clone()]);
    let (table, _) =
        create_live_lookup_table(&mut context, &addresses, "claims claim-check: twice").await;
    let first = submit_compaction(
        &mut context,
        instruction.clone(),
        table,
        &addresses,
        "first crank",
    )
    .await;
    assert!(first.accepted, "the first crank must commit");
    let owner = fixture.actor.pubkey().to_bytes();
    let minted = ClaimCheckV1::decode(
        &observed(&mut context, claim_check_address(fixture.aggregate, owner))
            .await
            .data,
    )
    .expect("claim-check decodes");

    let second =
        submit_compaction(&mut context, instruction, table, &addresses, "second crank").await;
    assert!(!second.accepted, "a position compacts exactly once");
    let after = ClaimCheckV1::decode(
        &observed(&mut context, claim_check_address(fixture.aggregate, owner))
            .await
            .data,
    )
    .expect("claim-check decodes");
    assert_eq!(
        after, minted,
        "the refused second crank moved no byte of the claim-check"
    );
}

/// A POSITION WHOSE OWNER CANNOT SIGN IS NOT COMPACTED — the Fractional reserve.
///
/// The Fractional reserve Position carries `owner_kind = TradingRecord` with a
/// `position_owner` equal to the Trading-owned Fractional root PDA; that is
/// asserted by the Fractional family itself, at
/// `fractional_retirement_v3.rs`'s admission join. It holds the collateral
/// backing every outstanding shard of one coordinate.
///
/// Compacting it would have written that collateral into a claim-check owned by
/// a program-derived address. `RedeemClaimCheck` pays the record's `owner` and
/// requires it to sign, and a PDA cannot sign a top-level instruction — so the
/// record would be unopenable, by anyone, forever, while the Position every
/// shard holder's own redemption reads was closed in the same transaction.
/// Not a delay. A total loss, reachable by any caller past the deadline.
///
/// The fixture is deliberately the one the differential test compacts
/// SUCCESSFULLY. Only the persisted owner-kind tag differs, so a pass here is a
/// statement about the gate and about nothing else.
#[tokio::test]
async fn a_position_whose_owner_could_never_sign_for_it_is_not_compacted() {
    for kind in [
        ProtocolPositionOwnerKindV2::TradingRecord,
        ProtocolPositionOwnerKindV2::ClaimsCapability,
    ] {
        let (test, fixture) = fixture(true);
        let mut context = test.start_with_context().await;
        create_claims_custody_replay(&mut context, &fixture).await;
        plant_admission_of_kind(&mut context, &fixture, kind).await;
        let cranker = context.payer.pubkey();
        open_and_elapse(&mut context, &fixture).await;

        let prestate = wallet_payout_prestate(&mut context, &fixture, fixture.actor_position).await;
        let instruction = compaction_instruction(&fixture, cranker, &prestate);
        let addresses = lookup_addresses(cranker, fixture.actor.pubkey(), &[instruction.clone()]);
        let (table, _) = create_live_lookup_table(
            &mut context,
            &addresses,
            "claims claim-check: unsignable owner",
        )
        .await;
        let result = submit_compaction(
            &mut context,
            instruction,
            table,
            &addresses,
            "crank on a position whose owner cannot sign",
        )
        .await;
        assert_refused_with(&result, 0x560A, &format!("a {kind:?}-owned position"));

        // The three things a partial refusal would have left behind. A route
        // that refused after minting, or after debiting, would still have
        // destroyed the shard holders' claim.
        assert!(
            maybe_observed(
                &mut context,
                claim_check_address(fixture.aggregate, fixture.actor.pubkey().to_bytes())
            )
            .await
            .is_none(),
            "{kind:?}: no claim-check is minted"
        );
        let position = observed(&mut context, fixture.actor_position).await;
        assert_eq!(
            lbv2_position_quantity(&position.data, WINNER),
            ACTOR_CLAIMS[WINNERS],
            "{kind:?}: the position keeps every atom it held"
        );
        assert_eq!(
            position.owner, CLAIMS_PROGRAM_ID,
            "{kind:?}: and the position is not closed out from under its claimants"
        );
    }
}

/// Submit one compaction through a v0 message carrying the lookup table.
async fn submit_compaction(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    table: Pubkey,
    addresses: &[Pubkey],
    label: &str,
) -> Submission {
    let payer = context.payer.pubkey();
    let blockhash = context
        .get_new_latest_blockhash()
        .await
        .expect("a distinct compaction blockhash");
    let message = VersionedMessage::V0(
        v0::Message::try_compile(
            &payer,
            &[instruction],
            &[AddressLookupTableAccount {
                key: table,
                addresses: addresses.to_vec(),
            }],
            blockhash,
        )
        .expect("compile the compaction message"),
    );
    let transaction =
        VersionedTransaction::try_new(message, &[&context.payer]).expect("sign the compaction");
    let signature = transaction
        .signatures
        .first()
        .expect("signed transaction")
        .to_string();
    let wire_bytes = 1 + transaction.signatures.len() * 64 + transaction.message.serialize().len();
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_or(0, |clock| clock.slot);
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .expect("compaction processing");
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

/// PROBE: does an ORDINARY redemption survive the deadline's own warp?
///
/// Separates a defect in the compaction route from a limitation of the harness.
/// If the holder's own payout stops working merely because the clock advanced
/// far enough for a compaction to be legal, then nothing built on that warp can
/// be trusted, and the deadline needs a different test strategy.
#[tokio::test]
async fn an_ordinary_redemption_survives_the_compaction_deadlines_warp() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    create_claims_custody_replay(&mut context, &fixture).await;
    let (table, addresses) =
        wallet_payout_lookup_table(&mut context, &fixture, "claims claim-check: warp probe").await;
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock")
        .slot;
    context
        .warp_to_slot(slot + COMPACTION_DEADLINE_SLOTS_V1)
        .expect("warp past the deadline");
    let result = submit_wallet_payout(
        &mut context,
        &fixture,
        table,
        &addresses,
        WalletPayoutOverrides::default(),
        "claims claim-check: redemption after a deadline-sized warp",
    )
    .await;
    if !result.accepted {
        eprintln!("WARP PROBE logs:\n{}", result.logs.join("\n"));
    }
    assert!(
        result.accepted,
        "a holder's own redemption must not stop working merely because time passed"
    );
}

// ---------------------------------------------------------------- redemption

use dclutch_claims_svm::claim_check_request_v1::RedeemClaimCheckRequestV1;
use dclutch_claims_svm::claim_check_v1::{
    CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1, ClaimCheckRedemptionRoleV1,
};

/// Build the seven-account redemption frame from its own declared spec.
///
/// The privileges are read off `ClaimCheckRedemptionRoleV1`, not restated here,
/// so the test cannot pass a frame the route's own spec would reject.
fn redeem_instruction(
    fixture: &Fixture,
    holder: Pubkey,
    holder_tokens: Pubkey,
    overrides: RedeemOverrides,
) -> Instruction {
    let terminal = fixture.terminal_accounts.expect("terminal fixture");
    let aggregate = fixture.aggregate;
    let request = RedeemClaimCheckRequestV1 {
        aggregate: aggregate.to_bytes(),
        owner: overrides.owner.unwrap_or(holder.to_bytes()),
    }
    .new()
    .expect("redeem request");
    let addresses = [
        overrides.holder.unwrap_or(holder),
        claim_check_address(aggregate, holder.to_bytes()),
        escrow_address(aggregate),
        vault_address(aggregate),
        holder_tokens,
        terminal.collateral_mint,
        TOKEN_PROGRAM_ID,
    ];
    let accounts = ClaimCheckRedemptionRoleV1::frame()
        .iter()
        .zip(addresses)
        .map(|(role, address)| {
            let (signer, writable) = role.privileges();
            if writable {
                AccountMeta::new(address, signer)
            } else {
                AccountMeta::new_readonly(address, signer)
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(accounts.len(), CLAIM_CHECK_REDEMPTION_ACCOUNT_COUNT_V1);
    Instruction {
        program_id: CLAIMS_PROGRAM_ID,
        accounts,
        data: request.to_bytes().expect("redeem bytes").to_vec(),
    }
}

#[derive(Clone, Copy, Default)]
struct RedeemOverrides {
    holder: Option<Pubkey>,
    owner: Option<[u8; 32]>,
}

/// Run a market all the way to a compacted claim-check, then hand back the
/// holder's keypair and token account.
async fn compact_for_a_sleeping_holder(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
) -> Pubkey {
    create_claims_custody_replay(context, fixture).await;
    plant_admission(context, fixture).await;
    let cranker = context.payer.pubkey();
    open_and_elapse(context, fixture).await;
    let prestate = wallet_payout_prestate(context, fixture, fixture.actor_position).await;
    let instruction = compaction_instruction(fixture, cranker, &prestate);
    let addresses = lookup_addresses(cranker, fixture.actor.pubkey(), &[instruction.clone()]);
    let (table, _) =
        create_live_lookup_table(context, &addresses, "claims claim-check: pre-redemption").await;
    let result = submit_compaction(context, instruction, table, &addresses, "pre-redemption").await;
    assert!(result.accepted, "the compaction must commit");
    fixture
        .terminal_accounts
        .expect("terminal fixture")
        .recipient
}

/// THE HOLDER COMES BACK, AND IS PAID FROM A MARKET THAT NO LONGER RUNS.
///
/// The sentence census R3 says is impossible. Nobody redeemed in time, a
/// stranger compacted the position, and the holder still walks away with their
/// collateral — owner-signed, out of seven accounts, none of which is the
/// market's.
#[tokio::test]
async fn a_sleeping_holder_is_paid_from_a_claim_check_long_after_the_crank() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    let holder_tokens = compact_for_a_sleeping_holder(&mut context, &fixture).await;
    let holder = fixture.actor.pubkey();

    let record = ClaimCheckV1::decode(
        &observed(
            &mut context,
            claim_check_address(fixture.aggregate, holder.to_bytes()),
        )
        .await
        .data,
    )
    .expect("claim-check decodes");
    let owed = record.entitlement_atoms;
    assert!(owed > 0, "the holder is owed something");
    let tokens_before = token_amount(&observed(&mut context, holder_tokens).await);
    let lamports_before = observed(&mut context, holder).await.lamports;

    let instruction =
        redeem_instruction(&fixture, holder, holder_tokens, RedeemOverrides::default());
    let result = submit_holder_signed(
        &mut context,
        &fixture,
        instruction,
        "claims claim-check: the sleeping holder redeems",
    )
    .await;
    if !result.accepted {
        eprintln!("redeem refusal logs:\n{}", result.logs.join("\n"));
    }
    assert!(result.accepted, "the holder must be paid");

    // Paid exactly what the claim-check promised.
    assert_eq!(
        token_amount(&observed(&mut context, holder_tokens).await),
        tokens_before + owed
    );
    // The vault gave up exactly that, and the escrow's count went to zero.
    assert_eq!(
        token_amount(&observed(&mut context, vault_address(fixture.aggregate)).await),
        0,
        "the vault holds nothing once its last claim-check is redeemed"
    );
    let escrow = ClaimCheckEscrowV1::decode(
        &observed(&mut context, escrow_address(fixture.aggregate))
            .await
            .data,
    )
    .expect("escrow decodes");
    assert_eq!(escrow.outstanding_claim_checks, 0);
    assert!(escrow.is_settled());
    // The record is gone and its rent went home with the holder.
    assert!(
        maybe_observed(
            &mut context,
            claim_check_address(fixture.aggregate, holder.to_bytes())
        )
        .await
        .is_none_or(|account| account.lamports == 0 && account.data.is_empty()),
        "a redeemed claim-check keeps nothing"
    );
    assert!(observed(&mut context, holder).await.lamports > lamports_before);
}

/// NOBODY BUT THE HOLDER CAN REDEEM, AND THE RECORD SURVIVES THE ATTEMPT.
#[tokio::test]
async fn a_stranger_cannot_redeem_another_holders_claim_check() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    let holder_tokens = compact_for_a_sleeping_holder(&mut context, &fixture).await;
    let holder = fixture.actor.pubkey();
    let stranger = context.payer.pubkey();
    assert_ne!(stranger, holder);

    let before = observed(
        &mut context,
        claim_check_address(fixture.aggregate, holder.to_bytes()),
    )
    .await;
    let instruction = redeem_instruction(
        &fixture,
        holder,
        holder_tokens,
        RedeemOverrides {
            holder: Some(stranger),
            owner: Some(holder.to_bytes()),
        },
    );
    // Signed by the stranger alone: a real, well-formed attempt.
    let result = submit_opener_signed(
        &mut context,
        instruction,
        "claims claim-check: a stranger tries to redeem",
    )
    .await;
    assert_refused_with(&result, 0x5621, "a non-holder redemption");
    let after = observed(
        &mut context,
        claim_check_address(fixture.aggregate, holder.to_bytes()),
    )
    .await;
    assert_eq!(
        after.data, before.data,
        "the refused redemption moved no byte of the claim-check"
    );
    assert_eq!(
        token_amount(&observed(&mut context, vault_address(fixture.aggregate)).await),
        ClaimCheckV1::decode(&before.data)
            .expect("claim-check decodes")
            .entitlement_atoms,
        "and took nothing from the vault"
    );
}

/// A CLAIM-CHECK REDEEMS EXACTLY ONCE, AND ITS OWN ABSENCE IS THE PROOF.
///
/// There is no cursor and no counter to get wrong: the second attempt finds no
/// account to decode.
#[tokio::test]
async fn a_claim_check_cannot_be_redeemed_twice() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    let holder_tokens = compact_for_a_sleeping_holder(&mut context, &fixture).await;
    let holder = fixture.actor.pubkey();
    let instruction =
        redeem_instruction(&fixture, holder, holder_tokens, RedeemOverrides::default());

    let first = submit_holder_signed(
        &mut context,
        &fixture,
        instruction.clone(),
        "claims claim-check: first redemption",
    )
    .await;
    assert!(first.accepted, "the first redemption must commit");
    let paid = token_amount(&observed(&mut context, holder_tokens).await);

    let second = submit_holder_signed(
        &mut context,
        &fixture,
        instruction,
        "claims claim-check: a second redemption",
    )
    .await;
    assert!(!second.accepted, "a claim-check pays exactly once");
    assert_eq!(
        token_amount(&observed(&mut context, holder_tokens).await),
        paid,
        "and the second attempt paid nothing"
    );
}

/// Send one holder-signed transaction: the holder pays its own fee and signs.
async fn submit_holder_signed(
    context: &mut ProgramTestContext,
    fixture: &Fixture,
    instruction: Instruction,
    label: &str,
) -> Submission {
    let blockhash = context
        .get_new_latest_blockhash()
        .await
        .expect("a distinct redemption blockhash");
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&fixture.actor.pubkey()),
        &[&fixture.actor],
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
        .expect("redemption processing");
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

// -------------------------------------------------------------- escrow close

use dclutch_claims_svm::claim_check_request_v1::CloseClaimCheckEscrowRequestV1;

fn close_escrow_instruction(
    fixture: &Fixture,
    caller: Pubkey,
    caller_tokens: Pubkey,
) -> Instruction {
    let terminal = fixture.terminal_accounts.expect("terminal fixture");
    let request = CloseClaimCheckEscrowRequestV1 {
        aggregate: fixture.aggregate.to_bytes(),
    }
    .new()
    .expect("close request");
    Instruction {
        program_id: CLAIMS_PROGRAM_ID,
        accounts: Vec::from([
            AccountMeta::new(caller, true),
            AccountMeta::new(escrow_address(fixture.aggregate), false),
            AccountMeta::new(vault_address(fixture.aggregate), false),
            // The residue destination. Structurally never written today: the
            // terminal executor's exact-equality check means no transfer fee
            // can survive it, so the vault is empty by the time its last
            // claim-check is redeemed.
            AccountMeta::new(caller_tokens, false),
            AccountMeta::new_readonly(terminal.collateral_mint, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ]),
        data: request.to_bytes().expect("close bytes").to_vec(),
    }
}

/// THE RESIDUE SHRINKS TO ZERO, AND THE LAST REDEMPTION IS WHAT ALLOWS IT.
///
/// The design's honest claim is not that compaction leaves nothing -- an escrow
/// and a vault survive for as long as anybody is owed -- but that what survives
/// is proportional to unredeemed claims and SELF-LIQUIDATING. This is the last
/// clause of that sentence, and the test that makes it true rather than hopeful.
#[tokio::test]
async fn the_escrow_closes_once_its_last_claim_check_is_redeemed() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    let holder_tokens = compact_for_a_sleeping_holder(&mut context, &fixture).await;
    let holder = fixture.actor.pubkey();
    let caller = context.payer.pubkey();
    let escrow = escrow_address(fixture.aggregate);
    let vault = vault_address(fixture.aggregate);

    // While a claim-check is live, the escrow holds somebody's collateral and
    // closing it would be taking their money.
    let premature = submit_opener_signed(
        &mut context,
        close_escrow_instruction(&fixture, caller, holder_tokens),
        "claims claim-check: close with a live claim-check",
    )
    .await;
    assert_refused_with(&premature, 0x5625, "an escrow still owing somebody");
    assert!(
        maybe_observed(&mut context, escrow).await.is_some(),
        "the refused close left the escrow standing"
    );

    // The holder comes back.
    let redeemed = submit_holder_signed(
        &mut context,
        &fixture,
        redeem_instruction(&fixture, holder, holder_tokens, RedeemOverrides::default()),
        "claims claim-check: the last redemption",
    )
    .await;
    assert!(redeemed.accepted, "the holder must be paid");

    let escrow_lamports = observed(&mut context, escrow).await.lamports;
    let vault_lamports = observed(&mut context, vault).await.lamports;
    let caller_before = observed(&mut context, caller).await.lamports;

    let closed = submit_opener_signed(
        &mut context,
        close_escrow_instruction(&fixture, caller, holder_tokens),
        "claims claim-check: the escrow closes",
    )
    .await;
    if !closed.accepted {
        eprintln!("close refusal logs:\n{}", closed.logs.join("\n"));
    }
    assert!(closed.accepted, "a settled escrow must close");

    // Nothing of the market survives. This is the residue reaching zero.
    for gone in [escrow, vault] {
        assert!(
            maybe_observed(&mut context, gone)
                .await
                .is_none_or(|account| account.lamports == 0 && account.data.is_empty()),
            "a closed escrow keeps nothing"
        );
    }
    // And both rents funded the caller, which is why this crank needs no escrow
    // of its own.
    let caller_after = observed(&mut context, caller).await.lamports;
    assert!(
        caller_after > caller_before,
        "the closer is paid out of the rent it recovered"
    );
    assert!(
        caller_after <= caller_before + escrow_lamports + vault_lamports,
        "and paid no more than the two accounts actually held"
    );
}

// ------------------------------------------------------------- the whole arc

/// THE END-TO-END ARC: A MARKET RETIRES ITS SLEEPING HOLDER'S POSITION, AND
/// THE HOLDER STILL GETS PAID.
///
/// Census R3 says this is impossible: "one sleeping holder blocks retirement
/// forever." Every step below runs against the real Claims, Core, Custody,
/// Registry, Resolution and Token-2022 ELFs, in one context, in order.
///
/// resolve -> the holder sleeps -> a stranger opens the escrow -> the deadline
/// elapses -> a stranger cranks -> the position and its admission close and the
/// supply they held is retired -> the holder returns to a market whose
/// machinery is gone and is paid -> the escrow closes and nothing remains.
///
/// # What this proves about R3, exactly
///
/// The claim is scoped deliberately. R3's blocker is a live position holding
/// supply that only its absent owner can retire; what this asserts is that
/// after the crank, that position does not exist and the supply it held is
/// gone from the aggregate — so the sleeping holder is no longer a reason
/// retirement cannot proceed. The remaining reasons are other rows.
///
/// # What it does NOT drive, and why
///
/// `market_closure_v1` itself is not called here, and that is a property of
/// this fixture rather than of the feature. Its market carries Claims
/// capability Positions (it is the representation campaign), and V1 compaction
/// refuses those by design — section 10 says so and predicts exactly this: "a
/// market with one unredeemed fractional shard still blocks retirement
/// forever." Retiring THIS market needs the fractional route, not this one. A
/// campaign whose market is all-native would close the final step, and nothing
/// in the routes below would change.
#[tokio::test]
async fn a_market_retires_a_sleeping_holders_position_and_the_holder_is_still_paid() {
    let (test, fixture) = fixture(true);
    let mut context = test.start_with_context().await;
    let holder = fixture.actor.pubkey();
    let holder_tokens = fixture
        .terminal_accounts
        .expect("terminal fixture")
        .recipient;
    let cranker = context.payer.pubkey();
    let escrow = escrow_address(fixture.aggregate);
    let vault = vault_address(fixture.aggregate);
    let record = claim_check_address(fixture.aggregate, holder.to_bytes());
    let admission = admission_address(fixture.aggregate, holder.to_bytes());

    create_claims_custody_replay(&mut context, &fixture).await;
    plant_admission(&mut context, &fixture).await;

    // The holder's claims, and the supply the aggregate is carrying for them.
    let opening = snapshot(&mut context, &fixture).await;
    let owed_claims = lbv2_position_quantity(&opening.actor_position.data, WINNER);
    let supply_before = lbv2_market_supply(&opening.aggregate.data, WINNER);
    let hoard_before = token_amount(opening.hoard.as_ref().expect("Hoard"));
    let tokens_before = token_amount(&observed(&mut context, holder_tokens).await);
    assert_eq!(owed_claims, ACTOR_CLAIMS[WINNERS]);

    // --- the holder sleeps; a stranger starts the clock ----------------------
    open_and_elapse(&mut context, &fixture).await;

    // --- a stranger turns the crank -----------------------------------------
    let prestate = wallet_payout_prestate(&mut context, &fixture, fixture.actor_position).await;
    let instruction = compaction_instruction(&fixture, cranker, &prestate);
    let addresses = lookup_addresses(cranker, holder, &[instruction.clone()]);
    let (table, _) =
        create_live_lookup_table(&mut context, &addresses, "claims claim-check: the arc").await;
    let cranked = submit_compaction(&mut context, instruction, table, &addresses, "the arc").await;
    assert!(cranked.accepted, "the crank must commit");

    // *** THE R3 CLAIM. *** The sleeping holder's position is gone and the
    // supply it held is retired, so it is no longer a reason this market
    // cannot retire.
    for closed in [fixture.actor_position, admission] {
        assert!(
            maybe_observed(&mut context, closed)
                .await
                .is_none_or(|account| account.lamports == 0 && account.data.is_empty()),
            "the sleeping holder's accounts are gone"
        );
    }
    let after_crank = observed(&mut context, fixture.aggregate).await;
    assert_eq!(
        lbv2_market_supply(&after_crank.data, WINNER),
        supply_before - owed_claims,
        "the aggregate no longer carries the sleeper's liability"
    );
    // Their collateral moved to an escrow only they can open -- it did not stay
    // in the Hoard and it did not go to the cranker.
    let escrowed = token_amount(&observed(&mut context, vault).await);
    assert_eq!(escrowed, owed_claims, "the payout is in the vault");
    assert_eq!(
        token_amount(&observed(&mut context, fixture.terminal_accounts.expect("t").hoard).await),
        hoard_before - owed_claims
    );
    assert_eq!(
        token_amount(&observed(&mut context, holder_tokens).await),
        tokens_before,
        "and not one atom reached the holder yet -- nobody spent it for them"
    );

    // --- the holder returns, to a market whose machinery is gone -------------
    let redeemed = submit_holder_signed(
        &mut context,
        &fixture,
        redeem_instruction(&fixture, holder, holder_tokens, RedeemOverrides::default()),
        "claims claim-check: the sleeper finally returns",
    )
    .await;
    assert!(redeemed.accepted, "the holder must be paid, however late");
    assert_eq!(
        token_amount(&observed(&mut context, holder_tokens).await),
        tokens_before + owed_claims,
        "paid exactly what a redemption in time would have paid"
    );

    // --- and the residue reaches zero ---------------------------------------
    let closed = submit_opener_signed(
        &mut context,
        close_escrow_instruction(&fixture, cranker, holder_tokens),
        "claims claim-check: the escrow closes",
    )
    .await;
    assert!(closed.accepted, "a settled escrow must close");
    for gone in [escrow, vault, record] {
        assert!(
            maybe_observed(&mut context, gone)
                .await
                .is_none_or(|account| account.lamports == 0 && account.data.is_empty()),
            "nothing of the claim-check machinery survives its last holder"
        );
    }

    // Both ledgers close over the whole run: every atom the Hoard gave up
    // reached the holder, and nothing is left in between.
    assert_eq!(
        token_amount(&observed(&mut context, holder_tokens).await) - tokens_before,
        hoard_before
            - token_amount(
                &observed(&mut context, fixture.terminal_accounts.expect("t").hoard).await
            ),
        "collateral is conserved end to end"
    );
}
