//! Real-ELF proof of the Series permit-expiry Hot reachability wall.
//!
//! Core's permissionless expiry route deliberately accepts only a prefunded,
//! still-unallocated permit.  In the honest failed-founding world both that
//! permit and its future Market are System-owned and data-empty.  The generic
//! Hot outer, however, authenticates the same Market before it reads the
//! selected descriptor and requires a Core-owned, exact-width `CoreState`.
//! This campaign submits an exact Series Expire family request while carrying
//! the exact nested permit request and its unallocated accounts, then proves
//! the current Trading ELF refuses before any child CPI and rolls every
//! material account back byte-for-byte.

use std::{env, fs, path::Path};

use dclutch_market::capability_program::hot_v3::{
    HOT_EXECUTION_ENVELOPE_BYTES_V3, HotExecutionEnvelopeV3,
};
use dclutch_core_contract::ContentId;
use dclutch_market::{
    FoundingIntentV5, Identity, SERIES_FOUNDING_PERMIT_BYTES_V1,
    SERIES_PERMIT_EXPIRY_REQUEST_BYTES_V1, STATE_BYTES, SeriesFoundingPermitSeedsV1,
    SeriesFoundingPermitV1, SeriesPermitExpiryRequestV1,
};
use dclutch_market::rent::{
    RefundAuthority,
    lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleAccountIdV2, LifecycleRentCreditV2},
};
use dclutch_trading_sbf::{
    TradingSbfError,
    series::instruction::{
        SERIES_ACTION_HEADER_BYTES_V3, SeriesActionRequestV3, SeriesActionV3,
        encode_series_action_header_v3,
    },
};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, InstructionError},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTestContext};
use solana_sdk::transaction::TransactionError;
use solana_sdk_ids::system_program;

use dclutch_direct_hot_program_test_support::waist::{
    CLAIMS_PROGRAM_ID, CORE_PROGRAM_ID, CUSTODY_PROGRAM_ID, REGISTRY_PROGRAM_ID, RENT_PROGRAM_ID,
    TRADING_PROGRAM_ID, add_lookup_table, add_release_waist, canonical_lookup_addresses,
    direct_case, elves, fixture_substrate, program_test_without_forced_budget,
    start_with_substrate, submit_v0_observed,
};

const PROOF_SIBLING: [u8; 32] = [0x42; 32];

fn identity(byte: u8) -> Identity {
    Identity::new([byte; 32]).expect("nonzero identity")
}

fn content(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("nonzero content identity")
}

fn refusal_code(error: &BanksClientError) -> Option<u32> {
    let transaction = match error {
        BanksClientError::TransactionError(value) => value,
        BanksClientError::SimulationError { err, .. } => err,
        _ => return None,
    };
    match transaction {
        TransactionError::InstructionError(_, InstructionError::Custom(code)) => Some(*code),
        _ => None,
    }
}

fn exact_expire_family_request(ticket: Identity) -> Vec<u8> {
    let header = encode_series_action_header_v3(
        SeriesActionV3::Expire,
        content(0xb1),
        Some(content(0xb2)),
        Some(ContentId::new(ticket.to_bytes()).expect("Ticket content identity")),
        7,
        3,
        1,
    )
    .expect("canonical Expire header");
    let mut request = Vec::with_capacity(SERIES_ACTION_HEADER_BYTES_V3 + PROOF_SIBLING.len());
    request.extend_from_slice(&header);
    request.extend_from_slice(&PROOF_SIBLING);
    let decoded = SeriesActionRequestV3::decode(&request).expect("canonical Expire request");
    assert_eq!(decoded.action(), SeriesActionV3::Expire);
    assert_eq!(decoded.proof_bytes(), PROOF_SIBLING);
    request
}

fn exact_rent_credit(
    market: Pubkey,
    release_set: [u8; 32],
    generation: u64,
) -> (Pubkey, LifecycleRentCreditV2) {
    let refund = RefundAuthority::new([0xd0; 32]).expect("refund wallet");
    let market_id = LifecycleAccountIdV2::new(market.to_bytes()).expect("Market identity");
    let release = LifecycleAccountIdV2::new(release_set).expect("release-set identity");
    let provisional = LifecycleRentCreditV2::new(refund, market_id, release, generation, 0)
        .expect("provisional RentCredit");
    let seeds = provisional.pda_seeds();
    let generation_seed = generation.to_le_bytes();
    let (address, bump) = Pubkey::find_program_address(
        &[seeds.domain(), market.as_ref(), generation_seed.as_slice()],
        &RENT_PROGRAM_ID,
    );
    let credit = LifecycleRentCreditV2::new(refund, market_id, release, generation, bump)
        .expect("canonical RentCredit");
    assert_eq!(credit.to_bytes().len(), LIFECYCLE_RENT_CREDIT_BYTES_V2);
    (address, credit)
}

fn exact_unallocated_permit(
    envelope: HotExecutionEnvelopeV3,
    root: Pubkey,
) -> (
    Pubkey,
    SeriesFoundingPermitV1,
    Vec<u8>,
    Pubkey,
    LifecycleRentCreditV2,
) {
    let release = Identity::new(envelope.release_set()).expect("release-set identity");
    let market = Identity::new(envelope.market()).expect("Market identity");
    let ticket = identity(0xa7);
    let generation = envelope.generation();
    let (rent_credit, credit) = exact_rent_credit(
        Pubkey::new_from_array(envelope.market()),
        envelope.release_set(),
        generation,
    );
    let seeds = SeriesFoundingPermitSeedsV1::new(release, market, ticket);
    let (permit_address, bump) = Pubkey::find_program_address(&seeds.as_slices(), &CORE_PROGRAM_ID);
    let intent = FoundingIntentV5::new(
        bump,
        release,
        market,
        identity(0xa1),
        identity(0xa2),
        identity(0xa3),
        ticket,
        Identity::new(root.to_bytes()).expect("root identity"),
        identity(0xa4),
        identity(0xa5),
        identity(0xa6),
        identity(0xa8),
        identity(0xa9),
        Identity::new(TRADING_PROGRAM_ID.to_bytes()).expect("Trading identity"),
        Identity::new(CLAIMS_PROGRAM_ID.to_bytes()).expect("Claims identity"),
        Identity::new(rent_credit.to_bytes()).expect("RentCredit identity"),
        generation,
        1,
        1,
        2,
        1,
        1,
    )
    .expect("canonical founding intent");
    let permit = SeriesFoundingPermitV1::new(intent, identity(0xaa), identity(0xab))
        .expect("canonical Series permit");
    let child_request = SeriesPermitExpiryRequestV1::new(permit)
        .encode()
        .expect("canonical permit-expiry request")
        .to_vec();
    assert_eq!(child_request.len(), SERIES_PERMIT_EXPIRY_REQUEST_BYTES_V1);
    assert_eq!(
        SeriesPermitExpiryRequestV1::decode(&child_request)
            .expect("round-trip permit-expiry request")
            .permit(),
        permit,
    );
    (permit_address, permit, child_request, rent_credit, credit)
}

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
        .expect("live account")
}

async fn snapshots(
    context: &mut ProgramTestContext,
    keys: &[Pubkey],
) -> Vec<(Pubkey, Option<Account>)> {
    let mut output = Vec::with_capacity(keys.len());
    for key in keys {
        output.push((
            *key,
            context
                .banks_client
                .get_account(*key)
                .await
                .expect("rollback account read"),
        ));
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("hex formatting");
    }
    output
}

fn evidence_json(
    trading_elf: &[u8],
    family_request: &[u8],
    child_request: &[u8],
    market: Pubkey,
    permit: Pubkey,
    rent_credit: Pubkey,
    refusal: u32,
    compute_units: u64,
    before: &[(Pubkey, Option<Account>)],
    after: &[(Pubkey, Option<Account>)],
) -> String {
    let mut poststates = Vec::with_capacity(before.len());
    for ((before_key, before_account), (after_key, after_account)) in before.iter().zip(after) {
        assert_eq!(before_key, after_key, "snapshot order");
        poststates.push(match before_account {
            Some(account) => format!(
                concat!(
                    "    {{\"key\":\"{}\",\"present\":true,\"owner\":\"{}\",",
                    "\"lamports\":{},\"dataLen\":{},\"dataSha256\":\"{}\",",
                    "\"unchanged\":{}}}"
                ),
                before_key,
                account.owner,
                account.lamports,
                account.data.len(),
                hex(hash(&account.data).as_ref()),
                before_account == after_account,
            ),
            None => format!(
                "    {{\"key\":\"{before_key}\",\"present\":false,\"unchanged\":{}}}",
                before_account == after_account,
            ),
        });
    }
    format!(
        concat!(
            "{{\n",
            "  \"schema\": \"dclutch-series-permit-expiry-hot-wall-v1\",\n",
            "  \"realElf\": {{\"program\":\"{}\",\"sha256\":\"{}\"}},\n",
            "  \"transaction\": {{\"result\":\"refused\",\"customCode\":{},",
            "\"computeUnits\":{},\"childCpiCount\":0}},\n",
            "  \"requests\": {{\"familyAction\":\"Expire\",\"familyBytes\":{},",
            "\"familySha256\":\"{}\",\"proofBytes\":32,\"childBytes\":{},",
            "\"childSha256\":\"{}\"}},\n",
            "  \"geometry\": {{\"market\":\"{}\",\"marketOwner\":\"{}\",",
            "\"marketDataLen\":0,\"hotRequiredMarketOwner\":\"{}\",",
            "\"hotRequiredMarketDataLen\":{},\"permit\":\"{}\",",
            "\"permitOwner\":\"{}\",\"permitDataLen\":0,",
            "\"permitFundedForBytes\":{},\"rentCredit\":\"{}\",",
            "\"rentCreditOwner\":\"{}\",\"rentCreditDataLen\":{}}},\n",
            "  \"poststates\": [\n{}\n  ]\n",
            "}}\n"
        ),
        TRADING_PROGRAM_ID,
        hex(hash(trading_elf).as_ref()),
        refusal,
        compute_units,
        family_request.len(),
        hex(hash(family_request).as_ref()),
        child_request.len(),
        hex(hash(child_request).as_ref()),
        market,
        system_program::ID,
        CORE_PROGRAM_ID,
        STATE_BYTES,
        permit,
        system_program::ID,
        SERIES_FOUNDING_PERMIT_BYTES_V1,
        rent_credit,
        RENT_PROGRAM_ID,
        LIFECYCLE_RENT_CREDIT_BYTES_V2,
        poststates.join(",\n"),
    )
}

/// The exact honest prestate cannot reach the receiptless child branch.
///
/// The selected Direct records in the reused release waist are deliberately
/// irrelevant: Market authentication runs before program-set selection or any
/// descriptor/artifact read.  Keeping that mature fixed-frame fixture gives
/// this wall a current-source, real-ELF top-level invocation without inventing
/// a second author for the common Hot substrate.
#[tokio::test]
async fn unallocated_series_expiry_refuses_before_descriptor_or_child_cpi() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let mut direct = direct_case(&mut test, releases, &artifacts, false);
    let (old_envelope, _) =
        HotExecutionEnvelopeV3::split_instruction(&direct.chain.hot_instruction.data)
            .expect("canonical Hot envelope");
    let ticket = identity(0xa7);
    let family_request = exact_expire_family_request(ticket);
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(family_request.len()).expect("family request width"),
        old_envelope.release_set(),
        old_envelope.market(),
        old_envelope.generation(),
        old_envelope.root_prestate_digest(),
    )
    .expect("Series Hot envelope");
    let (permit_address, permit, child_request, rent_credit, credit) =
        exact_unallocated_permit(envelope, direct.chain.root);

    assert_eq!(
        permit.intent().market().to_bytes(),
        envelope.market(),
        "the child request and Hot outer must name the same Market",
    );
    assert_eq!(
        permit.intent().parent_root().to_bytes(),
        direct.chain.root.to_bytes()
    );
    assert_eq!(
        permit.intent().release_set().to_bytes(),
        envelope.release_set()
    );
    assert_eq!(permit.intent().generation(), envelope.generation());
    assert_eq!(
        permit.intent().trading_program().to_bytes(),
        TRADING_PROGRAM_ID.to_bytes()
    );

    let mut data = Vec::with_capacity(HOT_EXECUTION_ENVELOPE_BYTES_V3 + family_request.len());
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(&family_request);
    direct.chain.hot_instruction.data = data;
    // These are the two mutable accounts whose Core expiry poststate would
    // change.  They ride in the actual transaction even though the common Hot
    // wall refuses before the selected AccountProfile can project them.
    direct
        .chain
        .hot_instruction
        .accounts
        .push(AccountMeta::new(permit_address, false));
    direct
        .chain
        .hot_instruction
        .accounts
        .push(AccountMeta::new(rent_credit, false));

    let market_lamports = direct
        .chain
        .accounts
        .iter()
        .find(|candidate| candidate.key == direct.chain.market)
        .expect("fixture Market")
        .account
        .lamports;
    test.add_account(
        direct.chain.market,
        Account {
            lamports: market_lamports,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    let rent = Rent::default();
    test.add_account(
        permit_address,
        Account {
            lamports: rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1),
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
    test.add_account(
        rent_credit,
        Account {
            lamports: rent.minimum_balance(LIFECYCLE_RENT_CREDIT_BYTES_V2),
            data: credit.to_bytes().to_vec(),
            owner: RENT_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let instructions = [direct.chain.hot_instruction.clone()];
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    let mut snapshot_keys = direct.chain.rollback_snapshot_keys.clone();
    for key in [
        direct.chain.market,
        direct.chain.root,
        permit_address,
        rent_credit,
    ] {
        if !snapshot_keys.contains(&key) {
            snapshot_keys.push(key);
        }
    }
    let before = snapshots(&mut context, &snapshot_keys).await;
    let market_before = account(&mut context, direct.chain.market).await;
    let permit_before = account(&mut context, permit_address).await;
    let credit_before = account(&mut context, rent_credit).await;
    assert_eq!(market_before.owner, system_program::ID);
    assert!(market_before.data.is_empty());
    assert_eq!(permit_before.owner, system_program::ID);
    assert!(permit_before.data.is_empty());
    assert_eq!(
        permit_before.lamports,
        rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1),
    );
    assert_eq!(credit_before.owner, RENT_PROGRAM_ID);
    assert_eq!(credit_before.data, credit.to_bytes());

    let outcome = submit_v0_observed(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await;
    let refusal = match outcome {
        Ok(_) => panic!("a vacant future Market passed the current Hot outer"),
        Err(refusal) => refusal,
    };
    assert_eq!(
        refusal_code(&refusal.error),
        Some(TradingSbfError::Content as u32),
        "the exact fixed-Market owner/width conjunct owns this refusal: {:#?}",
        refusal.logs,
    );
    let child_programs = [
        REGISTRY_PROGRAM_ID,
        CORE_PROGRAM_ID,
        CLAIMS_PROGRAM_ID,
        CUSTODY_PROGRAM_ID,
        RENT_PROGRAM_ID,
    ];
    for program in child_programs {
        let invocation = format!("Program {program} invoke [2]");
        assert!(
            !refusal.logs.iter().any(|line| line == &invocation),
            "the pre-descriptor refusal unexpectedly invoked child {program}: {:#?}",
            refusal.logs,
        );
    }
    let trading_invocation = format!("Program {TRADING_PROGRAM_ID} invoke [1]");
    assert!(
        refusal.logs.iter().any(|line| line == &trading_invocation),
        "the transaction did not reach the real Trading ELF: {:#?}",
        refusal.logs,
    );

    let after = snapshots(&mut context, &snapshot_keys).await;
    assert_eq!(
        after, before,
        "pre-descriptor refusal must roll back exactly"
    );
    let evidence = evidence_json(
        &artifacts.trading,
        &family_request,
        &child_request,
        direct.chain.market,
        permit_address,
        rent_credit,
        TradingSbfError::Content as u32,
        refusal.compute_units_consumed,
        &before,
        &after,
    );
    println!("{evidence}");
    if let Some(path) = env::var_os("DCLUTCH_SERIES_HOT_WALL_EVIDENCE") {
        let path = Path::new(&path);
        assert!(path.is_absolute(), "evidence path must be absolute");
        fs::write(path, evidence).expect("write machine-readable evidence");
    }
}
