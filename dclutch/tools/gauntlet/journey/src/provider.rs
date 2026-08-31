//! The Pyth transport, driven on a real validator.
//!
//! `dclutch-successor-validator` loads the provenance-pinned Pyth receiver and
//! Wormhole router ELFs beside the seven dClutch roles, and the infrastructure
//! plan publishes the deployment-slot-zero `local_validator_release_v1` record
//! that describes them. Everything in this module is a REAL transaction against
//! those real programs: the router's own initialization, the receiver's own
//! Config, a signed VAA written in chunks and cryptographically verified by the
//! router, a price update posted by the receiver, and then the two dClutch
//! provider legs that carry that update into a terminal certificate.
//!
//! **What is a lab shape here, stated plainly.** The VAA is a captured 13-of-19
//! signature over a synthetic guardian set, not a live Pyth publication; the
//! price update's publication instant is FROZEN at the capture date while this
//! validator's clock is wall-clock. The second fact is the one with teeth, and
//! `§12.3`'s admission is where it lands: an observation must be ABOUT a time
//! inside the window, and its PUBLICATION must be inside a band around the
//! cluster's clock. The Market states a real 300-second terminal window for the
//! first and the fixture's declared shelf life for the second, and this module
//! refuses when the fixture outlives that shelf life rather than letting anyone
//! widen the number again.

use dclutch_market_core_codec::{CoreState, Phase};
use dclutch_provider_transport_v3_operator::{
    ProviderExecuteDeploymentV3, ProviderExecuteIntentV3, ProviderExecuteSnapshotV3,
    ProviderSubmitDeploymentV3, ProviderSubmitIntentV3, ProviderSubmitSnapshotV3,
    build_provider_execute_v3, build_provider_submit_v3,
};
use dclutch_pyth_svm::FullPriceUpdateV2;
use dclutch_release_set_contract::PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2;
use dclutch_resolution_codec::{
    PROVIDER_UPDATE_LIFECYCLE_BYTES_V3, PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
    RESOLUTION_CERTIFICATE_BYTES_V2, ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_resolution_core_v3_operator::ObservedAccount;
use dclutch_source_contract::{
    PythAdapterConfigV1, SourceResolutionPhaseV1, SourceResolutionStateV2, WindowSpecV1,
};
use solana_program::hash::hash;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{create_account, transfer};

use crate::{
    Error, Result,
    model::{SuccessorPlan, TransactionEvidence},
    plan::pubkey,
    resolution::ResolutionAddressesV1,
    rpc::Rpc,
    stages::StageReportV1,
};

/// Captured artifacts. Every one of these is in the eleven-file set
/// `dclutch-successor-validator` verifies by SHA-256 before it starts, so the
/// bytes compiled in here and the programs loaded on that chain are one set.
const ROUTER_INITIALIZE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/router-initialize.data");
const RECEIVER_INITIALIZE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-initialize.data");
const SIGNED_VAA: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/signed.vaa");
const RECEIVER_POST_UPDATE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-post-update.data");
const PRICE_UPDATE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/price-update.account");

const ENCODED_VAA_HEADER_BYTES: usize = 46;
/// One `write_encoded_vaa` payload. The VAA is 952 bytes, so this is two
/// transactions; a larger chunk would not fit a legacy packet.
const WRITE_CHUNK_BYTES: usize = 600;
/// Matches `market.rs`'s `FIXTURE_SHELF_LIFE_SECONDS`, and is checked against
/// the window record the Market actually published rather than assumed.
const FIXTURE_SHELF_LIFE_SECONDS: i64 = 31_536_000;

/// The pinned provider deployment, and the accounts its own programs derive.
#[derive(Clone, Copy)]
pub(crate) struct ProviderAddressesV1 {
    pub(crate) receiver: Pubkey,
    pub(crate) config: Pubkey,
    pub(crate) router: Pubkey,
    pub(crate) guardian_set: Pubkey,
    pub(crate) treasury: Pubkey,
    pub(crate) bridge: Pubkey,
    pub(crate) fee_collector: Pubkey,
}

impl ProviderAddressesV1 {
    /// Derive every provider address from the published Pyth release record.
    ///
    /// The two program identities come off the chain's own release record, not
    /// out of a constant in this file: the launcher, the plan and this campaign
    /// would otherwise each carry their own copy of the same two pubkeys, and
    /// the day one of them moved the other two would keep working against a
    /// program that is not there.
    pub(crate) fn from_release(release: &[u8]) -> Result<Self> {
        let release = dclutch_pyth_svm::PythReleaseV1::decode(release)
            .map_err(|error| Error::new(format!("published Pyth release record: {error:?}")))?;
        let receiver = Pubkey::new_from_array(release.receiver_program());
        let router = Pubkey::new_from_array(release.router_program());
        Ok(Self {
            receiver,
            config: Pubkey::find_program_address(&[b"config"], &receiver).0,
            router,
            guardian_set: Pubkey::find_program_address(
                &[b"GuardianSet", &0_u32.to_be_bytes()],
                &router,
            )
            .0,
            treasury: Pubkey::find_program_address(&[b"treasury", &[0]], &receiver).0,
            bridge: Pubkey::find_program_address(&[b"Bridge"], &router).0,
            fee_collector: Pubkey::find_program_address(&[b"fee_collector"], &router).0,
        })
    }
}

/// Everything the provider legs create, so the ledger can watch it from before
/// it exists.
pub(crate) struct ProviderPlanV1 {
    pub(crate) addresses: ProviderAddressesV1,
    /// The Receiver `PriceUpdateV2` account this campaign posts and the
    /// Resolution lifecycle consumes.
    pub(crate) update: Keypair,
    /// The distinct key that drives the Core execution. It is deliberately not
    /// the submitter: the two roles are separable in the protocol and a
    /// campaign that collapses them cannot tell whether the separation holds.
    pub(crate) resolver: Keypair,
    /// The router-owned buffer the signed VAA is written into.
    pub(crate) encoded_vaa: Keypair,
    /// The Resolution lifecycle for the posted update, derived here rather than
    /// read off the submit report, so the ledger watches it from before it
    /// exists. Its seeds are the domain and the update account, both of which
    /// this campaign draws.
    pub(crate) lifecycle: Pubkey,
}

impl ProviderPlanV1 {
    /// Draw the provider keys and read the deployment off the chain.
    pub(crate) fn derive(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<Self> {
        let (raw, _) = crate::runtime::record(plan, "pyth_release")?;
        let release = rpc.required_account(raw, "published Pyth release record")?;
        let update = Keypair::new();
        let lifecycle = Pubkey::find_program_address(
            &[
                PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
                update.pubkey().as_ref(),
            ],
            &pubkey(&plan.resolution.program_id)?,
        )
        .0;
        Ok(Self {
            addresses: ProviderAddressesV1::from_release(&release.data)?,
            update,
            resolver: Keypair::new(),
            encoded_vaa: Keypair::new(),
            lifecycle,
        })
    }
}

/// Register every account the provider legs create with the conservation
/// ledger, before the first census meets them holding a balance.
pub(crate) fn watch(ledger: &mut crate::ledger::ConservationLedgerV1, plan: &ProviderPlanV1) {
    for (label, address) in [
        ("provider_price_update", plan.update.pubkey()),
        ("provider_resolver", plan.resolver.pubkey()),
        ("provider_encoded_vaa", plan.encoded_vaa.pubkey()),
        ("provider_update_lifecycle", plan.lifecycle),
        ("provider_receiver_config", plan.addresses.config),
        ("provider_receiver_treasury", plan.addresses.treasury),
        ("provider_guardian_set", plan.addresses.guardian_set),
        ("provider_bridge", plan.addresses.bridge),
        ("provider_fee_collector", plan.addresses.fee_collector),
    ] {
        ledger.watch(label, address);
    }
}

/// Carry the Market from `Open` to `Terminal` through the real Pyth transport.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_through_pyth(
    rpc: &mut Rpc,
    payer: &Keypair,
    plan: &SuccessorPlan,
    addresses: &ResolutionAddressesV1,
    provider: &ProviderPlanV1,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(StageReportV1, crate::ledger::LamportClaimV1)> {
    let mut fees = 0_u64;
    let mut compute_units = 0_u64;
    let mut submitted = 0_usize;

    // The tripwire. `max_age_seconds` in the Market's own window record is the
    // captured publication's declared shelf life, and the quantity it bounds
    // grows by 86,400 every day the fixture is not recaptured. Checking it here
    // -- against the record the chain holds and the clock the chain keeps --
    // means this campaign fails with a sentence somebody can act on rather than
    // with an opaque `InvalidPublicationTime` from inside an adapter.
    let update_view = FullPriceUpdateV2::parse(PRICE_UPDATE)
        .map_err(|error| Error::new(format!("captured Pyth price update: {error:?}")))?;
    let slot = rpc.finalized_slot()?;
    let chain_now = rpc.block_time(slot)?;
    let age = chain_now.saturating_sub(update_view.publish_time());
    if age > FIXTURE_SHELF_LIFE_SECONDS {
        return Err(Error::new(format!(
            "the pinned Pyth publication is {age} seconds old and this Market's window admits \
             {FIXTURE_SHELF_LIFE_SECONDS}. The fixture has outlived its declared shelf life. \
             RECAPTURE IT, or restate the shelf life in market.rs together with the reason -- do \
             not widen the number to make this run pass, which is exactly the failure the bound \
             exists to prevent."
        )));
    }

    // ---------------------------------------------------------- the router
    let addresses_p = provider.addresses;
    if rpc.account(addresses_p.guardian_set)?.is_none() {
        send(
            rpc,
            "journey: the captured Wormhole router initializes its synthetic guardian set",
            &[Instruction {
                program_id: addresses_p.router,
                accounts: vec![
                    AccountMeta::new(addresses_p.bridge, false),
                    AccountMeta::new(addresses_p.guardian_set, false),
                    AccountMeta::new(addresses_p.fee_collector, false),
                    AccountMeta::new(payer.pubkey(), true),
                    AccountMeta::new_readonly(sysvar::clock::ID, false),
                    AccountMeta::new_readonly(sysvar::rent::ID, false),
                    AccountMeta::new_readonly(system_program::ID, false),
                ],
                data: ROUTER_INITIALIZE.to_vec(),
            }],
            payer,
            &[],
            &mut fees,
            &mut compute_units,
            &mut submitted,
            transactions,
        )?;
    }

    // ---------------------------------------------------------- the receiver
    if rpc.account(addresses_p.config)?.is_none() {
        send(
            rpc,
            "journey: the captured Pyth receiver initializes its Config",
            &[Instruction {
                program_id: addresses_p.receiver,
                accounts: vec![
                    AccountMeta::new(payer.pubkey(), true),
                    AccountMeta::new(addresses_p.config, false),
                    AccountMeta::new_readonly(system_program::ID, false),
                ],
                data: RECEIVER_INITIALIZE.to_vec(),
            }],
            payer,
            &[],
            &mut fees,
            &mut compute_units,
            &mut submitted,
            transactions,
        )?;
    }
    let treasury_rent = rpc.minimum_balance(0)?;
    if rpc
        .account(addresses_p.treasury)?
        .map(|account| account.lamports)
        .unwrap_or(0)
        < treasury_rent
    {
        send(
            rpc,
            "journey: capitalize the canonical zero-data receiver treasury",
            &[transfer(
                &payer.pubkey(),
                &addresses_p.treasury,
                treasury_rent,
            )],
            payer,
            &[],
            &mut fees,
            &mut compute_units,
            &mut submitted,
            transactions,
        )?;
    }

    // ------------------------------------------------------- the signed VAA
    let encoded = &provider.encoded_vaa;
    let encoded_size = ENCODED_VAA_HEADER_BYTES + SIGNED_VAA.len();
    let encoded_rent = rpc.minimum_balance(encoded_size)?;
    send(
        rpc,
        "journey: create the exact encoded-VAA buffer",
        &[create_account(
            &payer.pubkey(),
            &encoded.pubkey(),
            encoded_rent,
            encoded_size as u64,
            &addresses_p.router,
        )],
        payer,
        &[encoded],
        &mut fees,
        &mut compute_units,
        &mut submitted,
        transactions,
    )?;
    send(
        rpc,
        "journey: the real router initializes the encoded-VAA header",
        &[Instruction {
            program_id: addresses_p.router,
            accounts: vec![
                AccountMeta::new_readonly(payer.pubkey(), true),
                AccountMeta::new(encoded.pubkey(), false),
            ],
            data: anchor_discriminator(b"global:init_encoded_vaa"),
        }],
        payer,
        &[],
        &mut fees,
        &mut compute_units,
        &mut submitted,
        transactions,
    )?;
    for (index, chunk) in SIGNED_VAA.chunks(WRITE_CHUNK_BYTES).enumerate() {
        let offset = index
            .checked_mul(WRITE_CHUNK_BYTES)
            .ok_or_else(|| Error::new("VAA chunk offset overflowed"))?;
        let mut data = anchor_discriminator(b"global:write_encoded_vaa");
        data.extend_from_slice(
            &u32::try_from(offset)
                .map_err(|_| Error::new("VAA chunk offset exceeded u32"))?
                .to_le_bytes(),
        );
        data.extend_from_slice(
            &u32::try_from(chunk.len())
                .map_err(|_| Error::new("VAA chunk length exceeded u32"))?
                .to_le_bytes(),
        );
        data.extend_from_slice(chunk);
        send(
            rpc,
            &format!("journey: the real router writes signed-VAA chunk {index}"),
            &[Instruction {
                program_id: addresses_p.router,
                accounts: vec![
                    AccountMeta::new_readonly(payer.pubkey(), true),
                    AccountMeta::new(encoded.pubkey(), false),
                ],
                data,
            }],
            payer,
            &[],
            &mut fees,
            &mut compute_units,
            &mut submitted,
            transactions,
        )?;
    }
    send(
        rpc,
        "journey: the real router cryptographically verifies the signed VAA",
        &[Instruction {
            program_id: addresses_p.router,
            accounts: vec![
                AccountMeta::new_readonly(payer.pubkey(), true),
                AccountMeta::new(encoded.pubkey(), false),
                AccountMeta::new_readonly(addresses_p.guardian_set, false),
            ],
            data: anchor_discriminator(b"global:verify_encoded_vaa_v1"),
        }],
        payer,
        &[],
        &mut fees,
        &mut compute_units,
        &mut submitted,
        transactions,
    )?;
    let verified = rpc.required_account(encoded.pubkey(), "verified EncodedVaa")?;
    if verified.data.get(8) != Some(&2) {
        return Err(Error::new(
            "the router did not leave the encoded VAA in ProcessingStatus::Verified",
        ));
    }

    // ------------------------------------------------- the dClutch submit leg
    let post_update_body = RECEIVER_POST_UPDATE
        .get(8..)
        .ok_or_else(|| Error::new("captured receiver PostUpdate body is narrower than its tag"))?
        .to_vec();
    let submit = build_provider_submit_v3(
        &submit_snapshot(rpc, addresses, encoded.pubkey(), plan)?,
        ProviderSubmitDeploymentV3 {
            infrastructure: Pubkey::find_program_address(
                &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
                &addresses.core_program,
            )
            .0,
            registry_programdata: pubkey(&plan.registry.programdata_id)?,
            registry_artifact: crate::runtime::record(plan, "registry_artifact_release")?.0,
            registry_artifact_staging: crate::runtime::record(plan, "registry_artifact_release")?.1,
            core_programdata: addresses.core_programdata,
            resolution_program: addresses.resolution_program,
            resolution_programdata: addresses.resolution_programdata,
            receiver_config: addresses_p.config,
            guardian_set: addresses_p.guardian_set,
        },
        &ProviderSubmitIntentV3 {
            submitter: payer.pubkey(),
            refund_recipient: addresses.rent_beneficiary,
            update_account: provider.update.pubkey(),
            // Must not precede the window's own end; the window ended at the
            // captured publication, which is in the past, so any future instant
            // is admissible and an hour is a plainly-stated one.
            reclaim_after_unix_seconds: chain_now.saturating_add(3_600),
            post_update_body: post_update_body.clone(),
        },
    )
    .map_err(|error| Error::new(format!("chain-derived provider submission: {error:?}")))?;
    if submit.lifecycle != provider.lifecycle {
        return Err(Error::new(format!(
            "the operator derives the update lifecycle at {} and this campaign registered {} with \
             the conservation ledger; a lifecycle the ledger does not watch is a lamport placement \
             it cannot see",
            submit.lifecycle, provider.lifecycle
        )));
    }
    let lifecycle_rent = rpc.minimum_balance(PROVIDER_UPDATE_LIFECYCLE_BYTES_V3)?;
    // Both provider frames are wide -- the submit leg carries the release
    // observation, the record pairs and the receiver's own accounts -- so both
    // ride finalized routing tables, the same way the founding's oversized
    // frames do. The lifecycle prepayment goes in its own transaction rather
    // than ahead of the frame, so a routing table never has to cover an
    // account only the prepayment names.
    send(
        rpc,
        "journey: prepay the provider update lifecycle",
        &[transfer(&payer.pubkey(), &submit.lifecycle, lifecycle_rent)],
        payer,
        &[],
        &mut fees,
        &mut compute_units,
        &mut submitted,
        transactions,
    )?;
    let before_tables = transactions.len();
    let (submit_routing, submit_tables) = crate::market::publish_routing_table(
        rpc,
        payer,
        "provider submit",
        std::slice::from_ref(&submit.instruction),
        transactions,
    )?;
    submitted += transactions.len().saturating_sub(before_tables);
    fees = fees.saturating_add(fees_since(transactions, before_tables));
    let mut table_lamports = table_rent(&submit_tables);
    let evidence = rpc.send_v0_with_signers(
        "journey: Resolution submits one update through the real receiver ELF",
        std::slice::from_ref(&submit.instruction),
        payer,
        &[&provider.update],
        submit_routing,
        &submit_tables,
    )?;
    fees = fees.saturating_add(evidence.fee_lamports.unwrap_or(0));
    compute_units = compute_units.saturating_add(evidence.compute_units_consumed.unwrap_or(0));
    submitted += 1;
    transactions.push(evidence);
    let posted = rpc.required_account(provider.update.pubkey(), "posted PriceUpdateV2")?;
    if posted.owner != addresses_p.receiver {
        return Err(Error::new(
            "the posted price update is not owned by the receiver that posted it",
        ));
    }

    // ------------------------------------------------ the dClutch execute leg
    let certificate_rent = rpc.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2)?;
    let resolver_rent = rpc.minimum_balance(0)?;
    send(
        rpc,
        "journey: prepay the terminal certificate and establish the distinct resolver",
        &[
            transfer(&payer.pubkey(), &addresses.certificate, certificate_rent),
            transfer(&payer.pubkey(), &provider.resolver.pubkey(), resolver_rent),
        ],
        payer,
        &[],
        &mut fees,
        &mut compute_units,
        &mut submitted,
        transactions,
    )?;
    // The §12.3 PREFLIGHT. Three questions reach the wire as one number:
    // `InvalidObservationSchedule`, `InvalidPublicationTime` and
    // `InvalidPythObservation` all become `ProviderJoinErrorV3::Provider` and
    // then `ResolutionError::ProviderObservation` (0x800A), even though
    // `normalize_authenticated_update`'s own doc comment says an operator must
    // be able to tell a fresh publication about the wrong period from a stale
    // one about the right period. They cannot, and this tier paid a whole
    // campaign to that collapse.
    //
    // So the campaign evaluates the same three predicates itself, from the
    // records the CHAIN holds and the update the chain posted, and says which
    // one will refuse before it spends 1,070,265 CU finding out. When all three
    // hold it says that too -- which is the useful half, because it means a
    // 0x800A from here is NOT the window and the next reader can stop looking
    // at it.
    let window_note = preflight_window_admission(rpc, addresses, provider, chain_now)?;
    let execute = build_provider_execute_v3(
        &execute_snapshot(rpc, addresses, submit.lifecycle, provider)?,
        ProviderExecuteDeploymentV3 {
            registry_programdata: pubkey(&plan.registry.programdata_id)?,
            registry_artifact: crate::runtime::record(plan, "registry_artifact_release")?.0,
            registry_artifact_staging: crate::runtime::record(plan, "registry_artifact_release")?.1,
            core_programdata: addresses.core_programdata,
            // The TRADING role, and it means it. This campaign passed CUSTODY
            // here first, because that is what the operator's own ProgramTest
            // campaign passes and the field looked like a readonly role
            // observation rather than a callee. The chain refused it:
            // `provider_instruction_v3` authenticates accounts 13/14 against
            // `activation.role(ExecutionRoleV1::Trading).release().program()`
            // and raised `ResolutionRelease` (0x8005) after 681,773 CU. The
            // fixture passes because ITS release set binds Custody's key to the
            // Trading role; a real five-role activation binds five different
            // keys, so the confusion is invisible in ProgramTest and fatal on a
            // validator. The fixture is never the authority.
            trading_program: pubkey(&plan.trading.program_id)?,
            trading_programdata: pubkey(&plan.trading.programdata_id)?,
            resolution_program: addresses.resolution_program,
            resolution_programdata: addresses.resolution_programdata,
            receiver_config: addresses_p.config,
        },
        &ProviderExecuteIntentV3 {
            resolver: provider.resolver.pubkey(),
            terminal_sequence: 1,
            post_update_body,
        },
    )
    .map_err(|error| Error::new(format!("chain-derived Core provider execution: {error:?}")))?;
    let before_execute_tables = transactions.len();
    let (execute_routing, execute_tables) = crate::market::publish_routing_table(
        rpc,
        payer,
        "provider execute",
        std::slice::from_ref(&execute.instruction),
        transactions,
    )?;
    submitted += transactions.len().saturating_sub(before_execute_tables);
    fees = fees.saturating_add(fees_since(transactions, before_execute_tables));
    table_lamports = table_lamports.saturating_add(table_rent(&execute_tables));
    let evidence = rpc.send_v0_with_signers(
        "journey: Core admits the terminal state of an atomically founded Market",
        std::slice::from_ref(&execute.instruction),
        payer,
        &[&provider.resolver],
        execute_routing,
        &execute_tables,
    )?;
    fees = fees.saturating_add(evidence.fee_lamports.unwrap_or(0));
    compute_units = compute_units.saturating_add(evidence.compute_units_consumed.unwrap_or(0));
    submitted += 1;
    transactions.push(evidence);

    // The poststate, asserted rather than assumed.
    let terminal = CoreState::decode(&rpc.required_account(addresses.market, "Market")?.data)
        .map_err(|error| Error::new(format!("terminal Market: {error:?}")))?;
    if terminal.phase != Phase::Terminal {
        return Err(Error::new(format!(
            "the provider execution left the Market at {:?}, not Terminal",
            terminal.phase
        )));
    }
    let receipt = terminal
        .terminal_receipt
        .ok_or_else(|| Error::new("a Terminal Market carries no terminal receipt"))?;
    if receipt.to_bytes() != addresses.certificate.to_bytes() {
        return Err(Error::new(
            "the Market's terminal receipt names an account this campaign did not prepay",
        ));
    }
    let source = SourceResolutionStateV2::decode(
        &rpc.required_account(addresses.source_state, "Source resolution state")?
            .data,
    )
    .map_err(|error| Error::new(format!("resolved Source: {error:?}")))?;
    if source.phase() != SourceResolutionPhaseV1::Resolved {
        return Err(Error::new(format!(
            "the Source resolution state is {:?}, not Resolved",
            source.phase()
        )));
    }
    let certificate = ResolutionCertificateV2::decode(
        &rpc.required_account(addresses.certificate, "terminal certificate")?
            .data,
    )
    .map_err(|error| Error::new(format!("terminal certificate: {error:?}")))?;
    if certificate.kind != ResolutionCertificateKindV2::ResolutionSuccess
        || certificate.market != addresses.market.to_bytes()
        || certificate.generation != addresses.generation
        || certificate.selector != terminal.terminal_winner
    {
        return Err(Error::new(
            "the terminal certificate does not bind this Market, this generation, and the winner \
             the Market itself records",
        ));
    }

    Ok((
        StageReportV1 {
            stage: "resolution: the Pyth transport carries the Market to Terminal".into(),
            outcome: "executed".into(),
            transactions: submitted,
            compute_units,
            note: format!(
                "The Market is RESOLVED. One captured signed VAA written in chunks and verified by \
                 the real Wormhole router, one price update posted by the real Pyth receiver, one \
                 Resolution submission and one Core-driven execution by a resolver key that is not \
                 the submitter -- and the Market is Terminal on outcome {} with a ResolutionSuccess \
                 certificate that binds this Market at generation {}. The publication this resolves \
                 against was {age} seconds old at execution against a window that admits \
                 {FIXTURE_SHELF_LIFE_SECONDS}: the observation is ABOUT a time inside the Market's \
                 300-second terminal window, and its PUBLICATION is inside the band around the \
                 cluster's clock, which is the two-clock shape of the admission and not one \
                 tolerance doing both jobs. Checked before submission, not inferred after: \
                 {window_note}.",
                terminal.terminal_winner, addresses.generation
            ),
        },
        crate::ledger::LamportClaimV1::fees(fees).with_unwatched(
            table_lamports,
            "two address lookup tables, rent-funded to route the two oversized provider frames",
        ),
    ))
}

/// Fees paid by every transaction appended since `from`.
///
/// `publish_routing_table` submits between two and four transactions of its own
/// and returns only the finalized table, so a stage that counted only the fees
/// it paid DIRECTLY under-declared by exactly those. L7 caught it as an
/// unaccounted residual, which is the law working; this is the fix, and it
/// reads the fees off the evidence the publisher appended rather than assuming
/// a count or a price.
pub(crate) fn fees_since(transactions: &[TransactionEvidence], from: usize) -> u64 {
    transactions
        .iter()
        .skip(from)
        .map(|transaction| transaction.fee_lamports.unwrap_or(0))
        .fold(0_u64, u64::saturating_add)
}

/// Rent held by the routing tables a stage published.
///
/// Read off the tables themselves rather than recomputed, so the number L7
/// accepts is the one the chain charged.
pub(crate) fn table_rent(tables: &[ObservedAccount]) -> u64 {
    tables
        .iter()
        .map(|table| table.lamports)
        .fold(0_u64, u64::saturating_add)
}

/// Evaluate §12.3's three admission predicates off-chain, and name the one that
/// will refuse.
///
/// This is a DIAGNOSIS, never an authority: the chain decides, and this refuses
/// early only when it can say exactly why. The inputs are the finalized window
/// and adapter-config records the Market itself published and the price update
/// the receiver actually posted, so it is reading the same facts the adapter
/// will.
fn preflight_window_admission(
    rpc: &mut Rpc,
    addresses: &ResolutionAddressesV1,
    provider: &ProviderPlanV1,
    chain_now: i64,
) -> Result<String> {
    let window = WindowSpecV1::decode(
        &rpc.required_account(addresses.window_spec.raw, "window spec record")?
            .data,
    )
    .map_err(|error| Error::new(format!("WindowSpecV1: {error:?}")))?;
    let config = PythAdapterConfigV1::decode(
        &rpc.required_account(addresses.adapter_config.raw, "Pyth adapter config record")?
            .data,
    )
    .map_err(|error| Error::new(format!("PythAdapterConfigV1: {error:?}")))?;
    let posted = rpc.required_account(provider.update.pubkey(), "posted PriceUpdateV2")?;
    let update = FullPriceUpdateV2::parse(&posted.data)
        .map_err(|error| Error::new(format!("posted PriceUpdateV2: {error:?}")))?;

    let publication = update.publish_time();
    if publication < window.start_unix_seconds() || publication > window.end_unix_seconds() {
        return Err(Error::new(format!(
            "§12.3 SCHEDULE: the posted publication is at {publication} and this Market's terminal \
             window is [{}, {}]. The observation is not ABOUT the period the market sold. On chain \
             this is InvalidObservationSchedule and it reaches the log as 0x800A, indistinguishable \
             from the two predicates below.",
            window.start_unix_seconds(),
            window.end_unix_seconds()
        )));
    }
    let oldest = chain_now.saturating_sub(i64::from(window.max_age_seconds()));
    let newest = chain_now.saturating_add(i64::from(window.max_future_skew_seconds()));
    if publication < oldest || publication > newest {
        return Err(Error::new(format!(
            "§12.3 FRESHNESS: the posted publication is at {publication} and this cluster's clock \
             admits [{oldest}, {newest}] (now {chain_now}, max_age {}, max_future_skew {}). The \
             observation is about the right period and this cluster will not act on it. If the \
             publication is too OLD the pinned fixture has outlived its declared shelf life -- \
             recapture it, do not widen the window. On chain this is InvalidPublicationTime and it \
             reaches the log as 0x800A.",
            window.max_age_seconds(),
            window.max_future_skew_seconds()
        )));
    }
    if update.feed_id() != config.provider_feed_id()
        || update.exponent() != config.expected_exponent()
    {
        return Err(Error::new(
            "§12.3 OBSERVATION: the posted update's feed identity or exponent is not the one this \
             Market's adapter configuration names. On chain this is InvalidPythObservation and it \
             reaches the log as 0x800A.",
        ));
    }
    let admitted = u128::from(update.price().unsigned_abs())
        .saturating_mul(u128::from(config.max_confidence_bps()));
    if u128::from(update.confidence()).saturating_mul(10_000) > admitted {
        return Err(Error::new(format!(
            "§12.3 OBSERVATION: the posted update's confidence {} is wider than this Market's \
             adapter configuration admits at {} bps of price {}. On chain this is \
             InvalidPythObservation and it reaches the log as 0x800A.",
            update.confidence(),
            config.max_confidence_bps(),
            update.price()
        )));
    }
    Ok(format!(
        "all three §12.3 predicates hold off-chain before submission: the publication at \
         {publication} is inside the window [{}, {}] (it is ABOUT the right period), inside the \
         cluster band [{oldest}, {newest}] at clock {chain_now} (it is FRESH ENOUGH), and its feed, \
         exponent and confidence satisfy the adapter configuration. A 0x800A from this frame is \
         therefore NOT the window",
        window.start_unix_seconds(),
        window.end_unix_seconds()
    ))
}

fn submit_snapshot(
    rpc: &mut Rpc,
    addresses: &ResolutionAddressesV1,
    encoded_vaa: Pubkey,
    plan: &SuccessorPlan,
) -> Result<ProviderSubmitSnapshotV3> {
    let pyth_release = crate::runtime::record(plan, "pyth_release")?.0;
    let (_, present) = rpc.finalized_observed_accounts(
        &[
            addresses.market,
            addresses.source_state,
            addresses.source_material.raw,
            addresses.source_spec.raw,
            addresses.provider_release.raw,
            pyth_release,
            addresses.window_spec.raw,
            encoded_vaa,
        ],
        0,
    )?;
    let at = |index: usize| -> Result<ObservedAccount> {
        present
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("finalized observation lost an account"))
    };
    Ok(ProviderSubmitSnapshotV3 {
        market: at(0)?,
        source_state: at(1)?,
        source_material: at(2)?,
        source_spec: at(3)?,
        source_provider_release: at(4)?,
        pyth_release: at(5)?,
        window: at(6)?,
        encoded_vaa: at(7)?,
    })
}

fn execute_snapshot(
    rpc: &mut Rpc,
    addresses: &ResolutionAddressesV1,
    lifecycle: Pubkey,
    provider: &ProviderPlanV1,
) -> Result<ProviderExecuteSnapshotV3> {
    let (_, present) = rpc.finalized_observed_accounts(
        &[
            addresses.market,
            addresses.source_state,
            lifecycle,
            provider.update.pubkey(),
            addresses.source_material.raw,
            addresses.source_spec.raw,
            addresses.provider_release.raw,
            addresses.adapter_config.raw,
            addresses.window_spec.raw,
            addresses.statistic_spec.raw,
            addresses.pyth_release,
            addresses.product.raw,
            addresses.result_domain.raw,
            addresses.portfolio.raw,
        ],
        0,
    )?;
    let at = |index: usize| -> Result<ObservedAccount> {
        present
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("finalized observation lost an account"))
    };
    Ok(ProviderExecuteSnapshotV3 {
        market: at(0)?,
        source_state: at(1)?,
        lifecycle: at(2)?,
        update: at(3)?,
        source_material: at(4)?,
        source_spec: at(5)?,
        source_provider_release: at(6)?,
        adapter_config: at(7)?,
        window: at(8)?,
        statistic: at(9)?,
        pyth_release: at(10)?,
        product: at(11)?,
        result_domain: at(12)?,
        portfolio: at(13)?,
    })
}

/// Submit one provider transaction and accumulate its evidence.
///
/// Free function rather than a closure because the closure had to hold `rpc`
/// mutably across every call site, which made a rent lookup inside an argument
/// list a borrow error rather than a readability question.
#[allow(clippy::too_many_arguments)]
fn send(
    rpc: &mut Rpc,
    label: &str,
    instructions: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
    fees: &mut u64,
    compute_units: &mut u64,
    submitted: &mut usize,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<()> {
    let evidence = rpc.send_with_signers(label, instructions, payer, signers)?;
    *fees = fees.saturating_add(evidence.fee_lamports.unwrap_or(0));
    *compute_units = compute_units.saturating_add(evidence.compute_units_consumed.unwrap_or(0));
    *submitted += 1;
    transactions.push(evidence);
    Ok(())
}

/// Anchor's eight-byte instruction tag: the first eight bytes of the SHA-256 of
/// `global:<name>`. Both captured programs are Anchor programs and this is how
/// their entrypoints dispatch.
fn anchor_discriminator(name: &[u8]) -> Vec<u8> {
    hash(name).to_bytes()[..8].to_vec()
}
