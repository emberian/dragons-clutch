//! Real-ELF evidence for `CloseSeal`: omission P-006's close, on a validator.
//!
//! # What is being proved
//!
//! `trading_semantic_release` is the fourth PDA seed of a capability seal, so a
//! Trading release "does not invalidate a seal so much as stop addressing it"
//! (decision 0005). Every release therefore strands the rent of every seal
//! written under its predecessor — 968 bytes each, across all descriptors times
//! actions, a class that grows with the release cadence and that nothing could
//! reclaim. `P-006` records that, and names the two things a close must supply:
//! a beneficiary ruling, and a close that does not weaken write-once.
//!
//! These cases execute both against the real five-ELF Direct waist.
//!
//! - **A stranger closes a stranded seal and keeps the rent.** Not the seal's
//!   payer, not the Market's payer, not a maker: a keypair that has never
//!   appeared on this chain, which afterwards holds exactly
//!   `minimum_balance(CAPABILITY_SEAL_BYTES_V1)` and nothing else. That is the
//!   funded-crank pattern the E3 ruling chose: the reward is carved out of the
//!   rent the close liberates and out of nothing else, and no Market's funding
//!   can receive it because no Market's funding is in the frame.
//!
//! - **A seal the live release still addresses refuses to close.** The control
//!   is as tight as it can be made: the seal is written by the on-chain seal
//!   outer, in the same test, seconds earlier — so `CloseSealLiveRelease` is the
//!   release comparison and nothing else.
//!
//! - **Write-once survives.** After the close, the seal outer cannot re-create
//!   anything at that address, and the reason is structural rather than a second
//!   guard: `process_capability_seal_v1` derives the address it will write from
//!   the LIVE Trading semantic release, so the only addresses it can reach are
//!   the ones the close refuses to touch. The case below aims the real outer at
//!   a closed address and requires `Content` and a still-vacant account.
//!
//! - **The witness cannot be stale.** On `SlotPinnedSuperseded` — the substrate
//!   where the whole set was redeployed and every release except Trading's was
//!   re-pinned — the activation cache no longer authenticates its own Trading
//!   role, so it cannot be exhibited as proof that some other release is live.
//!   The close refuses with `ReleaseSuperseded` rather than accepting a cache
//!   that describes a deployment that has moved.
//!
//! # What this file does not do
//!
//! It takes no CU measurement. `CloseSeal` touches one account this Program
//! owns and issues no CPI, so its cost is not interesting and a figure here
//! would be one draw from a bump-search lottery either way.

use dclutch_capability_program_contract::hot_v3::{
    DIRECT_HOT_HEAP_FRAME_BYTES_V1, HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
};
use dclutch_capability_seal_contract::{
    CAPABILITY_SEAL_BUMP_OFFSET_V1, CAPABILITY_SEAL_BYTES_V1,
    CAPABILITY_SEAL_TRADING_RELEASE_OFFSET_V1, CapabilitySealCloseRequestV1, CapabilitySealKeyV1,
    CapabilitySealRequestV1, SealedDescriptorClosureV1,
};
use dclutch_direct_codec::execution_v3::DirectExecutionActionV3;
use dclutch_trading_sbf::TradingSbfError;
use solana_account::{Account, AccountSharedData};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program::{
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::TransactionError;
use solana_sdk_ids::{system_program, sysvar};

use dclutch_direct_hot_program_test_support::waist::{
    COMPUTE_LIMIT, DirectCase, FixtureSubstrateV1, REGISTRY_PROGRAM_ID, RefusedExecution, Releases,
    TRADING_PROGRAM_ID, add_lookup_table, add_release_waist, add_release_waist_v2,
    canonical_lookup_addresses, direct_case_v2, direct_case_v3, elves, fixture_substrate,
    program_test, program_test_v2, program_test_without_forced_budget, start_with_substrate,
    submit_v0,
};

// --- Named refusals ----------------------------------------------------------
//
// Every code is derived from the declaring program's own enum and never written
// as a bare number (AGENTS.md "Refusal codes", decision 0007).

/// `TradingSbfError::Content`: the seal outer refused its coordinates.
const TRADING_CONTENT_REFUSAL_CODE: u32 = TradingSbfError::Content as u32;
/// `TradingSbfError::CloseSealAccount`: nothing here is a live canonical seal.
const CLOSE_SEAL_ACCOUNT_REFUSAL_CODE: u32 = TradingSbfError::CloseSealAccount as u32;
/// `TradingSbfError::CloseSealLiveRelease`: the live release still addresses it.
const CLOSE_SEAL_LIVE_RELEASE_REFUSAL_CODE: u32 = TradingSbfError::CloseSealLiveRelease as u32;
/// `TradingSbfError::CloseSealFrame`: the closing frame was not the exact shape.
const CLOSE_SEAL_FRAME_REFUSAL_CODE: u32 = TradingSbfError::CloseSealFrame as u32;
/// `TradingSbfError::ReleaseSuperseded`: the pinned deployment slot moved.
const TRADING_RELEASE_SUPERSEDED_REFUSAL_CODE: u32 = TradingSbfError::ReleaseSuperseded as u32;

/// The Trading semantic release the fixture's activation cache actually names.
///
/// `add_release_waist` builds Trading's release with `content([0x33; 32])`, so
/// this is the live release for every case here and a seal carrying it is a
/// seal the live executable still addresses. Written as a constant so the
/// stranded fills below are visibly NOT it.
const LIVE_TRADING_SEMANTIC_RELEASE: [u8; 32] = [0x33; 32];
/// One stranded predecessor release. Any identity but the live one will do.
const STRANDED_TRADING_SEMANTIC_RELEASE: [u8; 32] = [0x21; 32];

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

fn assert_refusal(refusal: &RefusedExecution, expected: u32) {
    assert_eq!(
        refusal_code(&refusal.error).expect("custom refusal code"),
        expected,
        "refused as {:?} rather than the named code: {:#?}",
        refusal.error,
        refusal.logs
    );
}

async fn maybe_account(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    context.banks_client.get_account(key).await.expect("read")
}

async fn lamports_of(context: &mut ProgramTestContext, key: Pubkey) -> u64 {
    maybe_account(context, key)
        .await
        .map_or(0, |value| value.lamports)
}

/// Whether the address holds nothing a seal reader would honour.
async fn is_vacant(context: &mut ProgramTestContext, key: Pubkey) -> bool {
    maybe_account(context, key)
        .await
        .is_none_or(|value| value.owner == system_program::ID && value.data.is_empty())
}

fn seal_rent() -> u64 {
    Rent::default().minimum_balance(CAPABILITY_SEAL_BYTES_V1)
}

fn direct_action() -> u32 {
    DirectExecutionActionV3::InlineOrdinary as u32
}

/// Build the seal WRITE outer, optionally aimed at a substituted seal address.
///
/// The account list is the hot fixed prefix with the root read-only and the
/// seal writable, followed by the rent payer and the System Program. `aimed_at`
/// exists for exactly one case: pointing the real outer at an address the live
/// release cannot derive, to see it refuse.
fn seal_instruction(direct: &DirectCase, aimed_at: Option<Pubkey>) -> Instruction {
    let mut accounts = direct.chain.capability_seal_accounts.clone();
    assert_eq!(accounts.len(), HOT_FIXED_ACCOUNT_COUNT_V3);
    if let Some(address) = aimed_at {
        accounts
            .get_mut(HOT_CAPABILITY_SEAL_ACCOUNT_V3)
            .expect("capability seal slot")
            .pubkey = address;
    }
    let seal = aimed_at.unwrap_or(direct.chain.capability_seal);
    for meta in accounts.iter_mut() {
        meta.is_writable = meta.pubkey == seal;
        meta.is_signer = false;
    }
    accounts.push(AccountMeta::new(direct.payer.pubkey(), true));
    accounts.push(AccountMeta::new_readonly(system_program::ID, false));
    Instruction {
        program_id: TRADING_PROGRAM_ID,
        accounts,
        data: CapabilitySealRequestV1::new(direct_action(), direct.chain.descriptor_digest)
            .expect("canonical seal request")
            .to_bytes()
            .to_vec(),
    }
}

/// Build the seal CLOSE outer.
///
/// Seven accounts, one signer, no System Program and no payer: the route
/// creates nothing and signs for nothing. The closer is account 1 and is the
/// sole beneficiary, which is why it must sign rather than be named in the
/// request — a request field carrying a refund destination is a field a griefer
/// fills in with someone else's address.
fn close_instruction(seal: Pubkey, closer: Pubkey, releases: Releases) -> Instruction {
    Instruction {
        program_id: TRADING_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(seal, false),
            AccountMeta::new(closer, true),
            AccountMeta::new_readonly(REGISTRY_PROGRAM_ID, false),
            AccountMeta::new_readonly(releases.activation, false),
            AccountMeta::new_readonly(TRADING_PROGRAM_ID, false),
            AccountMeta::new_readonly(releases.trading_programdata, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ],
        data: CapabilitySealCloseRequestV1.to_bytes().to_vec(),
    }
}

/// Derive one seal that is byte-for-byte the fixture's, filed under another
/// Trading semantic release.
///
/// This is the only way such an account can exist in a test: the release is a
/// PDA seed, so a seal minted under a predecessor release lives at a DIFFERENT
/// address, and no executable in this tree can be made to write one now. The
/// body is the fixture's own verdict with two bytes moved — the release field
/// and the canonical bump — and the address is re-derived from the resulting
/// key, so the account this plants is exactly what the predecessor release's
/// own seal outer would have left behind.
fn stranded_seal(direct: &DirectCase, release: [u8; 32]) -> (Pubkey, Vec<u8>) {
    let mut bytes = direct.chain.capability_seal_bytes.clone();
    let live = SealedDescriptorClosureV1::decode(&bytes)
        .expect("the fixture's own seal body")
        .key()
        .expect("the fixture's own seal key");
    assert_ne!(
        release,
        live.trading_semantic_release(),
        "a 'stranded' seal filed under the live release proves nothing"
    );
    let key = CapabilitySealKeyV1::new(
        live.descriptor_schema(),
        live.descriptor_digest(),
        live.action(),
        release,
        live.registry_program(),
    )
    .expect("stranded seal key");
    let (address, bump) =
        Pubkey::find_program_address(&key.seeds().as_slices(), &TRADING_PROGRAM_ID);
    bytes
        .get_mut(
            CAPABILITY_SEAL_TRADING_RELEASE_OFFSET_V1
                ..CAPABILITY_SEAL_TRADING_RELEASE_OFFSET_V1 + 32,
        )
        .expect("sealed Trading release field")
        .copy_from_slice(&release);
    *bytes
        .get_mut(CAPABILITY_SEAL_BUMP_OFFSET_V1)
        .expect("sealed canonical bump") = bump;
    assert_ne!(
        address, direct.chain.capability_seal,
        "the release seed did not move the address"
    );
    (address, bytes)
}

fn plant(context: &mut ProgramTestContext, address: Pubkey, data: Vec<u8>) {
    context.set_account(
        &address,
        &AccountSharedData::from(Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: TRADING_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        }),
    );
}

/// Submit one instruction against the ONE address list the chain's lookup table
/// actually holds.
///
/// Not a convenience: a v0 message resolves lookup entries by INDEX, so
/// compiling against a subset of the installed table silently rebinds every
/// account after the first divergence. A case that submits two different
/// instruction shapes must therefore install the union once and compile every
/// message against that same union -- which is why this takes the list rather
/// than deriving a per-instruction one.
async fn submit(
    context: &mut ProgramTestContext,
    direct: &DirectCase,
    instruction: Instruction,
    addresses: &[Pubkey],
    extra: &[&Keypair],
) -> Result<u64, RefusedExecution> {
    submit_many(
        context,
        direct,
        core::slice::from_ref(&instruction),
        addresses,
        extra,
    )
    .await
}

async fn submit_many(
    context: &mut ProgramTestContext,
    direct: &DirectCase,
    instructions: &[Instruction],
    addresses: &[Pubkey],
    extra: &[&Keypair],
) -> Result<u64, RefusedExecution> {
    submit_v0(
        context,
        instructions,
        addresses.to_vec(),
        Some(&direct.payer),
        extra,
    )
    .await
}

/// The seal WRITE transaction: the two ComputeBudget instructions the outer
/// needs, then the outer.
///
/// The heap grant is not optional decoration.
/// `process_capability_seal_v1` authenticates its Market and root through
/// `reauthenticate_top_level_root_roles_v3`, whose first act is
/// `require_extended_heap_admitted_v1` -- so a seal transaction that carries no
/// `RequestHeapFrame` refuses `HeapFrame` before it reads an artifact. That is
/// the caller's half of the contract; `declares_extended_heap_profile_v1` is
/// the program's, and until 2026-08-31 it named every route but this one.
fn seal_transaction(direct: &DirectCase, aimed_at: Option<Pubkey>) -> Vec<Instruction> {
    vec![
        ComputeBudgetInstruction::set_compute_unit_limit(
            u32::try_from(COMPUTE_LIMIT).expect("compute limit width"),
        ),
        ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
        seal_instruction(direct, aimed_at),
    ]
}

/// The union of every lookup address the named instructions need.
fn lookup_union(instructions: &[Instruction], payer: Pubkey) -> Vec<Pubkey> {
    let mut addresses = canonical_lookup_addresses(instructions, payer);
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    addresses
}

/// The headline: a stranger closes a stranded seal, keeps exactly the rent, and
/// the second closer finds nothing there.
///
/// The closer is a keypair with no account on this chain, no relationship to
/// the Market, and no relationship to whoever paid for the seal. It signs and
/// it is credited; that is the whole authorization story, and it is deliberate.
/// A permissioned close would have to name an authority that outlives every
/// Market the seal ever served, which is the thing a seal is defined not to
/// have.
///
/// Racing is harmless and the second half proves it: the loser of the race
/// refuses by ABSENCE (`CloseSealAccount`), having moved nothing.
#[tokio::test]
async fn a_stranger_closes_a_stranded_seal_and_keeps_the_rent() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let (stranded, body) = stranded_seal(&direct, STRANDED_TRADING_SEMANTIC_RELEASE);
    let stranger = Keypair::new();
    let loser = Keypair::new();
    let close = close_instruction(stranded, stranger.pubkey(), releases);
    let addresses = lookup_union(
        &[
            close.clone(),
            close_instruction(stranded, loser.pubkey(), releases),
        ],
        direct.payer.pubkey(),
    );
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    plant(&mut context, stranded, body.clone());

    assert_eq!(
        lamports_of(&mut context, stranger.pubkey()).await,
        0,
        "the closer must start with nothing, or the payout below proves nothing"
    );
    let planted = maybe_account(&mut context, stranded)
        .await
        .expect("the stranded seal");
    assert_eq!(planted.owner, TRADING_PROGRAM_ID);
    assert_eq!(planted.lamports, seal_rent());

    submit(&mut context, &direct, close, &addresses, &[&stranger])
        .await
        .expect("a stranded seal refused to close");

    // Exactly the rent the close liberated, and not one lamport more: the
    // closer paid no fee (the fixture payer did) and received no other credit.
    assert_eq!(
        lamports_of(&mut context, stranger.pubkey()).await,
        seal_rent(),
        "the closer was not paid exactly the rent the close liberated"
    );
    assert!(
        is_vacant(&mut context, stranded).await,
        "the closed seal is still readable as a seal"
    );

    // The race: the second closer arrives at an address that no longer holds a
    // seal, and refuses by absence rather than by any release comparison.
    let refused = submit(
        &mut context,
        &direct,
        close_instruction(stranded, loser.pubkey(), releases),
        &addresses,
        &[&loser],
    )
    .await
    .expect_err("a closed seal was closed twice");
    assert_refusal(&refused, CLOSE_SEAL_ACCOUNT_REFUSAL_CODE);
    assert_eq!(
        lamports_of(&mut context, loser.pubkey()).await,
        0,
        "the loser of the close race was paid"
    );
}

/// A seal the LIVE release still addresses refuses to close, and the control is
/// the on-chain seal outer writing it moments earlier.
///
/// This is the conjunct the whole route rests on. The seal here is not planted:
/// `process_capability_seal_v1` wrote it, at the address the live release
/// derives, under the live release's own semantic identity. So the refusal can
/// only be the release comparison — every other conjunct is satisfied by
/// construction, and the same close instruction shape succeeds on the stranded
/// case above.
#[tokio::test]
async fn the_live_release_still_addresses_its_own_seal_and_the_close_refuses() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let write = seal_transaction(&direct, None);
    let stranger = Keypair::new();
    let close = close_instruction(direct.chain.capability_seal, stranger.pubkey(), releases);
    let mut all = write.clone();
    all.push(close.clone());
    let addresses = lookup_union(&all, direct.payer.pubkey());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;

    submit_many(&mut context, &direct, &write, &addresses, &[])
        .await
        .expect("the canonical validated-artifact seal");
    let sealed = maybe_account(&mut context, direct.chain.capability_seal)
        .await
        .expect("the seal the outer just wrote");
    assert_eq!(sealed.owner, TRADING_PROGRAM_ID);
    assert_eq!(sealed.data, direct.chain.capability_seal_bytes);
    let live = SealedDescriptorClosureV1::decode(&sealed.data)
        .expect("the on-chain seal body")
        .key()
        .expect("the on-chain seal key");
    assert_eq!(
        live.trading_semantic_release(),
        LIVE_TRADING_SEMANTIC_RELEASE,
        "the fixture's live Trading semantic release moved; the constant is stale"
    );

    let refused = submit(&mut context, &direct, close, &addresses, &[&stranger])
        .await
        .expect_err("a seal the live release addresses was closed");
    assert_refusal(&refused, CLOSE_SEAL_LIVE_RELEASE_REFUSAL_CODE);

    let after = maybe_account(&mut context, direct.chain.capability_seal)
        .await
        .expect("the refused close deleted the seal");
    assert_eq!(after.data, sealed.data);
    assert_eq!(after.lamports, sealed.lamports);
    assert_eq!(after.owner, TRADING_PROGRAM_ID);
    assert_eq!(
        lamports_of(&mut context, stranger.pubkey()).await,
        0,
        "a refused close paid the closer"
    );
}

/// Write-once survives the close, because a closed address is one the live
/// executable cannot write to at all.
///
/// The re-seal is not blocked by a second guard remembering that something used
/// to be here — nothing on chain remembers that. It is blocked by the seed:
/// `process_capability_seal_v1` builds its key from `root.trading_semantic_release`,
/// which is the semantic release of the Trading role in an activation cache
/// that authenticated against THIS deployed program. The only addresses it can
/// reach are live-release addresses, and `CloseSeal` refuses every one of those.
///
/// So this case aims the real outer at the closed address and requires it to
/// refuse on its own address equality, leaving the account vacant. The control
/// is `the_live_release_still_addresses_its_own_seal_and_the_close_refuses`:
/// the same builder, the same fixture, no substitution, and it executes.
///
/// The residual, stated rather than hidden: `semantic_release_id` is
/// publisher-supplied and nothing on chain checks it, so a coalition holding
/// the roles' upgrade authorities could activate a set naming an old semantic
/// release and re-open a closed address under new bytes. That is the same
/// coalition that could ship arbitrary Trading code, and therefore the same one
/// every seal verdict already trusts.
#[tokio::test]
async fn a_closed_address_is_one_the_live_seal_outer_cannot_write_to() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let (stranded, body) = stranded_seal(&direct, STRANDED_TRADING_SEMANTIC_RELEASE);
    let stranger = Keypair::new();
    let close = close_instruction(stranded, stranger.pubkey(), releases);
    let reseal = seal_transaction(&direct, Some(stranded));
    let live_reseal = seal_transaction(&direct, None);
    let mut all = reseal.clone();
    all.extend(live_reseal.clone());
    all.push(close.clone());
    let addresses = lookup_union(&all, direct.payer.pubkey());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    plant(&mut context, stranded, body);

    // The control comes FIRST, and it is the same builder with no substitution:
    // the seal outer executes on this fixture and writes at the live address. So
    // the refusal below is the substituted address and nothing else.
    submit_many(&mut context, &direct, &live_reseal, &addresses, &[])
        .await
        .expect("the seal outer refused its own canonical address");

    submit(&mut context, &direct, close, &addresses, &[&stranger])
        .await
        .expect("the stranded seal refused to close");
    assert!(is_vacant(&mut context, stranded).await);

    let refused = submit_many(&mut context, &direct, &reseal, &addresses, &[])
        .await
        .expect_err("the seal outer wrote at an address the live release cannot derive");
    assert_refusal(&refused, TRADING_CONTENT_REFUSAL_CODE);
    assert!(
        is_vacant(&mut context, stranded).await,
        "a refused re-seal left state at the closed address"
    );
    assert_eq!(
        lamports_of(&mut context, stranger.pubkey()).await,
        seal_rent(),
        "the refused re-seal moved the closer's reward"
    );
}

/// The closing frame is exact, and the two ways to get it wrong that matter.
///
/// A beneficiary who did not sign is the griefing case: without the signature
/// requirement, anyone could name anyone as the refund destination, and the
/// "reward" would stop being a reward for doing the chore. A foreign Registry
/// is the substitution case: the seal's own key names the Registry its record
/// addresses were derived under, so a cache from some other Registry cannot be
/// used to argue about this seal's release.
///
/// The control is the successful close in
/// `a_stranger_closes_a_stranded_seal_and_keeps_the_rent` — the same planted
/// seal, the same activation cache, the same request bytes.
#[tokio::test]
async fn the_close_frame_refuses_an_unsigned_beneficiary_and_a_foreign_registry() {
    let artifacts = elves();
    let mut test = program_test(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case_v2(&mut test, releases, &artifacts, false, true);
    let (stranded, body) = stranded_seal(&direct, STRANDED_TRADING_SEMANTIC_RELEASE);
    let stranger = Keypair::new();

    let mut unsigned = close_instruction(stranded, stranger.pubkey(), releases);
    unsigned.accounts.get_mut(1).expect("closer slot").is_signer = false;

    let mut foreign = close_instruction(stranded, stranger.pubkey(), releases);
    foreign.accounts.get_mut(2).expect("registry slot").pubkey = TRADING_PROGRAM_ID;

    // The unsigned variant carries one fewer signature, so its keypair list is
    // empty; handing `submit` a keypair the compiled message does not ask for
    // is `TooManySigners` and would refuse before Trading is entered at all.
    let hostile = [(unsigned, false), (foreign, true)];
    let addresses = lookup_union(
        &hostile
            .iter()
            .map(|(instruction, _)| instruction.clone())
            .collect::<Vec<_>>(),
        direct.payer.pubkey(),
    );
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    plant(&mut context, stranded, body.clone());

    for (instruction, signs) in hostile {
        let signers: &[&Keypair] = if signs { &[&stranger] } else { &[] };
        let refused = submit(&mut context, &direct, instruction, &addresses, signers)
            .await
            .expect_err("a non-canonical close frame was honoured");
        assert_refusal(&refused, CLOSE_SEAL_FRAME_REFUSAL_CODE);
        let after = maybe_account(&mut context, stranded)
            .await
            .expect("a refused close deleted the seal");
        assert_eq!(after.data, body);
        assert_eq!(after.lamports, seal_rent());
    }
    assert_eq!(lamports_of(&mut context, stranger.pubkey()).await, 0);
}

/// A superseded activation cache cannot witness the live release.
///
/// The close's entire release argument rests on the cache the closer exhibits
/// being CURRENT, and "current" is not a claim the cache makes about itself: it
/// is `cached_role_deployment_observation_v1` requiring the release's pinned
/// deployment slot to equal the slot the Loader wrote into ProgramData
/// (decision 0012). `SlotPinnedSuperseded` is the substrate where the whole set
/// was redeployed and every release except Trading's was re-pinned, so exactly
/// one pin is stale and it is Trading's.
///
/// On that substrate the close refuses with `ReleaseSuperseded` — not with the
/// live-release comparison, because it never gets one. A closer cannot reach
/// past a moved substrate by holding up an old cache.
#[tokio::test]
async fn a_superseded_cache_cannot_witness_the_release_the_close_needs() {
    let substrate = FixtureSubstrateV1::SlotPinnedSuperseded;
    let artifacts = elves();
    let mut test = program_test_v2(&artifacts, substrate);
    let releases = add_release_waist_v2(&mut test, &artifacts, substrate);
    let direct = direct_case_v3(&mut test, releases, &artifacts, false, true, substrate);
    let (stranded, body) = stranded_seal(&direct, STRANDED_TRADING_SEMANTIC_RELEASE);
    let stranger = Keypair::new();
    let close = close_instruction(stranded, stranger.pubkey(), releases);
    let addresses = lookup_union(core::slice::from_ref(&close), direct.payer.pubkey());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, substrate).await;
    plant(&mut context, stranded, body.clone());

    let refused = submit(&mut context, &direct, close, &addresses, &[&stranger])
        .await
        .expect_err("a superseded cache witnessed a live release");
    assert_refusal(&refused, TRADING_RELEASE_SUPERSEDED_REFUSAL_CODE);
    let after = maybe_account(&mut context, stranded)
        .await
        .expect("the refused close deleted the seal");
    assert_eq!(after.data, body);
    assert_eq!(lamports_of(&mut context, stranger.pubkey()).await, 0);
}
