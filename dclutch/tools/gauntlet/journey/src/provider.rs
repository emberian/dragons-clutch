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

use dclutch_market::{CoreState, Phase, Readiness};
use dclutch_provider_transport_v3_operator::{
    ProviderExecuteDeploymentV3, ProviderExecuteIntentV3, ProviderExecuteLadderV3,
    ProviderExecuteSnapshotV3, ProviderSubmitDeploymentV3, ProviderSubmitIntentV3,
    ProviderSubmitSnapshotV3, build_provider_execute_v3, build_provider_submit_v3,
};
use dclutch_registry::release_set::PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2;
use dclutch_resolution_core_v3_operator::ObservedAccount;
use dclutch_source::pyth::FullPriceUpdateV2;
use dclutch_source::resolution::{
    PROVIDER_UPDATE_LIFECYCLE_BYTES_V3, PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
    RESOLUTION_CERTIFICATE_BYTES_V2, ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_source::{
    PythAdapterConfigV1, SourceResolutionPhaseV1, SourceResolutionRouteV1, SourceResolutionStateV2,
    WindowSpecV1,
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
/// The Anchor tag `receiver-post-update.data` carries ahead of its body.
const POST_UPDATE_TAG_BYTES: usize = 8;
const _: () = assert!(
    RECEIVER_POST_UPDATE.len() > POST_UPDATE_TAG_BYTES,
    "the captured PostUpdate instruction is narrower than its own Anchor tag"
);

/// One Pyth publication, as this transport consumes it.
///
/// WHY THIS IS A PARAMETER AND NOT FOUR CONSTANTS. Everything below used to
/// read the pinned capture directly, so the whole transport was ABOUT one
/// instant frozen in August 2026 -- which is fine for a primary leg whose
/// market's window was compiled to end at that instant, and impossible for a
/// rung, because a rung is entered only after the primary leg's grace expired
/// and the capture is stale by construction by then. A caller that can mint a
/// publication (`pyth_lab_publication.rs`) hands one here; the journey hands
/// the capture, and [`Self::captured`] is that capture in this shape, so the
/// primary walk sends exactly the bytes it always sent.
pub(crate) struct PublicationV1 {
    /// The complete signed VAA the router verifies, written in chunks.
    pub(crate) signed_vaa: Vec<u8>,
    /// `PostUpdateParams` WITHOUT the Anchor tag, which is what both
    /// `ProviderSubmitIntentV3` and `ProviderExecuteIntentV3` take.
    pub(crate) post_update_body: Vec<u8>,
    /// The `PriceUpdateV2` image this publication is about. The transport reads
    /// only the publication instant off it; the CHAIN's own posted bytes are
    /// what a submission digests.
    pub(crate) price_update_image: Vec<u8>,
    /// How old this publication may be before the transport refuses it.
    ///
    /// A fact about the PUBLICATION and the lab that holds it, never about a
    /// market: the captured fixture's staleness grows by 86,400 every day and a
    /// minted publication's is zero at the second it is minted.
    pub(crate) shelf_life_seconds: i64,
}

impl PublicationV1 {
    /// The pinned capture, in the shape the transport now takes.
    pub(crate) fn captured() -> Self {
        Self {
            signed_vaa: SIGNED_VAA.to_vec(),
            post_update_body: RECEIVER_POST_UPDATE[POST_UPDATE_TAG_BYTES..].to_vec(),
            price_update_image: PRICE_UPDATE.to_vec(),
            shelf_life_seconds: FIXTURE_SHELF_LIFE_SECONDS,
        }
    }
}

/// The records a capture rides with when the Market is standing on a rung.
///
/// A rung substitutes a SOURCE and nothing else, which is why exactly three
/// coordinates are here: the alternative `SourceSpecV1` and its own
/// `PythAdapterConfigV1` -- the two positions the execute frame carries the
/// rung's records in rather than the material's -- and the `RecoveryPolicyV2`
/// that names them, which the builder authenticates before it will derive a
/// `source_index` above zero. The window, the statistic, the provider release
/// and the whole Product graph stay the market's.
pub(crate) struct RungCaptureV1 {
    pub(crate) policy: crate::resolution::RecordPairV1,
    pub(crate) source_spec: crate::resolution::RecordPairV1,
    pub(crate) adapter_config: crate::resolution::RecordPairV1,
}

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
        let release = dclutch_source::pyth::PythReleaseV1::decode(release)
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
/// The transport stage's label, in one place.
///
/// It used to end "carries the Market to Terminal", which named a poststate the
/// two routes it drives do not write; the Market's phase byte is the standalone
/// Core `AdmitTerminal`'s, and that is a separate stage.
pub(crate) const PYTH_TRANSPORT_STAGE_V1: &str =
    "resolution: the Pyth transport resolves the Source and mints the terminal certificate";

/// The same transport, answering on the rung the market advanced onto.
///
/// A SEPARATE LABEL BECAUSE IT IS A SEPARATE CLAIM. The primary label says the
/// market's first choice answered; a run that read that sentence off a capture
/// which actually answered on the funded alternative would be reporting the
/// leg the holders paid for as the leg they never needed.
pub(crate) const PYTH_RECOVERY_TRANSPORT_STAGE_V1: &str = "resolution: the Pyth transport answers the market's funded rung and mints the terminal \
     certificate";

/// Which leg a capture is about to answer on, in one place.
pub(crate) const fn transport_stage_v1(rung: Option<&RungCaptureV1>) -> &'static str {
    match rung {
        None => PYTH_TRANSPORT_STAGE_V1,
        Some(_) => PYTH_RECOVERY_TRANSPORT_STAGE_V1,
    }
}

pub(crate) fn resolve_through_pyth(
    rpc: &mut Rpc,
    payer: &Keypair,
    plan: &SuccessorPlan,
    addresses: &ResolutionAddressesV1,
    provider: &ProviderPlanV1,
    capture_dir: &std::path::Path,
    transactions: &mut Vec<TransactionEvidence>,
    publication: &PublicationV1,
    rung: Option<&RungCaptureV1>,
    terminal_sequence: u64,
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
    let update_view = FullPriceUpdateV2::parse(&publication.price_update_image)
        .map_err(|error| Error::new(format!("this capture's Pyth price update: {error:?}")))?;
    let slot = rpc.finalized_slot()?;
    let chain_now = rpc.block_time(slot)?;
    let age = chain_now.saturating_sub(update_view.publish_time());
    let shelf_life_seconds = publication.shelf_life_seconds;
    require_primary_publication_freshness_v1(age, shelf_life_seconds, rung.is_some())?;

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
    let encoded_size = ENCODED_VAA_HEADER_BYTES + publication.signed_vaa.len();
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
    for (index, chunk) in publication.signed_vaa.chunks(WRITE_CHUNK_BYTES).enumerate() {
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
    // ONE READ FOR BOTH PROVIDER FRAMES. The submit and the execute name the
    // same Registry artifact pair, and it is the LIVE profile's, not the plan's.
    let live_registry_artifact = live_registry_artifact_pair_v1(
        rpc,
        addresses.core_program,
        pubkey(&plan.registry.program_id)?,
    )?;
    let post_update_body = publication.post_update_body.clone();
    let submit = build_provider_submit_v3(
        &submit_snapshot(rpc, addresses, encoded.pubkey(), plan)?,
        ProviderSubmitDeploymentV3 {
            infrastructure: Pubkey::find_program_address(
                &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
                &addresses.core_program,
            )
            .0,
            registry_programdata: pubkey(&plan.registry.programdata_id)?,
            registry_artifact: live_registry_artifact.0,
            registry_artifact_staging: live_registry_artifact.1,
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
            // Must not precede the window's own end, and the window ends at the
            // publication this capture carries -- which is at or before the
            // cluster's clock either way, so an hour ahead of that clock is
            // admissible for a captured publication and a minted one alike.
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
    // THE SUBMIT'S `0x8004`, READ BESIDE THE CHAIN'S OWN ANSWER. The probe
    // reports and the transaction is sent regardless; the pair of readings is
    // what localizes the wall.
    let records = authenticate_frame_records_v1(
        rpc,
        pubkey(&plan.registry.program_id)?,
        &submit.instruction,
    )?;
    eprintln!("journey: provider submit frame records: {records}");
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
    write_frame_capture_v1(
        rpc,
        &capture_dir.join("provider-submit.capture.json"),
        "journey: Resolution submits one update through the real receiver ELF",
        std::slice::from_ref(&submit.instruction),
        payer.pubkey(),
        submit_routing,
        &submit_tables,
    )?;
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
    let window_note = preflight_window_admission(rpc, addresses, provider, chain_now, rung)?;
    // WHICH LEG THE MARKET IS STANDING ON, READ BEFORE THE FRAME MOVES IT. A
    // rung capture's certificate must name the attempt the Source is actually
    // on, and the only honest predecessor for that number is the Source's own
    // active attempt at this instant.
    let active_attempt_before = SourceResolutionStateV2::decode(
        &rpc.required_account(addresses.source_state, "Source resolution state")?
            .data,
    )
    .map_err(|error| Error::new(format!("Source before the capture: {error:?}")))?
    .active_attempt();
    let deployment =
        execute_deployment_v3(plan, addresses, addresses_p.config, live_registry_artifact)?;
    // THE MIRROR OF THE LEG THIS CAPTURE IS ABOUT, asked of the builder before
    // the real request is built. A market standing on a rung is not answerable
    // on its primary: the same snapshot with no ladder must refuse `State` off
    // chain, before a transaction exists, by the same rule the program enforces
    // on chain. It costs no lamports and it is the only thing that distinguishes
    // "the builder answered on the rung" from "the builder answers whatever it
    // is handed".
    let primary_refusal = match rung {
        None => None,
        Some(_) => {
            let primary = build_provider_execute_v3(
                &execute_snapshot(rpc, addresses, submit.lifecycle, provider, None)?,
                deployment,
                &ProviderExecuteIntentV3 {
                    resolver: provider.resolver.pubkey(),
                    terminal_sequence,
                    post_update_body: post_update_body.clone(),
                },
            );
            match primary.err() {
                None => {
                    return Err(Error::new(
                        "the operator BUILT a primary capture against a Market standing on a \
                         rung: a capture that can answer on a leg the market has left is a leg \
                         the holders paid for and nobody had to walk",
                    ));
                }
                Some(error) => Some(format!("{error:?}")),
            }
        }
    };
    let execute = build_provider_execute_v3(
        &execute_snapshot(rpc, addresses, submit.lifecycle, provider, rung)?,
        deployment,
        &ProviderExecuteIntentV3 {
            resolver: provider.resolver.pubkey(),
            terminal_sequence,
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
    // THE LABEL NAMES THE ROUTE IT DRIVES. This transaction was labelled
    // "Core admits the terminal state" for the life of the tier, and its own
    // binding row names `core/execute_provider_v3::process#ExecuteProvider` --
    // the two disagreed, and the label is the half that was wrong. Core's
    // standalone `AdmitTerminal` is a different instruction in a different
    // stage, and while one label covered both there was nothing in the
    // transcript to notice that the second had never been sent.
    let evidence = rpc.send_v0_with_signers(
        "journey: Core composes the provider execution that mints the terminal certificate",
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

    // THE POSTSTATE THIS ROUTE ACTUALLY WRITES, asserted rather than assumed.
    //
    // Until 2026-09-06 the three assertions here were `Phase::Terminal`, a
    // present `terminal_receipt` and the certificate -- and the first of them
    // was a poststate `execute_provider_v3` does not write. Its own module
    // comment says so: the route "invokes the Registry-selected Resolution
    // program, checks its immediate receipt and terminal poststate. A LATER
    // STANDALONE CORE `AdmitTerminal` consumes that durable certificate and
    // commits the Market transition." Runs 14 and 15 both stopped here on `the
    // provider execution left the Market at Open, not Terminal` -- a true
    // sentence about the campaign's expectation, not about the chain. The
    // transport's own poststate is a RESOLVED Source and a ResolutionSuccess
    // certificate; the Market's phase byte belongs to `resolution::admit_terminal`,
    // which is the devnet spine's own third act (`31-admit-terminal-*.sh`).
    let executed = CoreState::decode(&rpc.required_account(addresses.market, "Market")?.data)
        .map_err(|error| Error::new(format!("Market after provider execution: {error:?}")))?;
    if executed.phase != Phase::Open || executed.readiness != Readiness::Consumed {
        return Err(Error::new(format!(
            "the provider execution left the Market at ({:?}, {:?}) and this route transitions \
             neither: the standalone Core AdmitTerminal is what moves the phase byte",
            executed.phase, executed.readiness
        )));
    }
    if executed.terminal_receipt.is_some() {
        return Err(Error::new(
            "the provider execution left a terminal receipt on the Market, and no route it drives \
             writes one; AdmitTerminal is the author of that field",
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
        || certificate.receipt_account != addresses.certificate.to_bytes()
    {
        return Err(Error::new(
            "the terminal certificate does not bind this Market, this generation, and the seat \
             this campaign prepaid",
        ));
    }
    // WHICH LEG ANSWERED, ASSERTED RATHER THAN NARRATED. The route and the
    // attempt index are the whole difference between "the market's first choice
    // answered" and "the alternative the holders prepaid answered", and both are
    // written by the program rather than by this campaign. A rung capture that
    // came back Primary would be a run whose whole subject never happened, and
    // nothing else in the poststate distinguishes the two.
    let route = source
        .terminal_projection()
        .map_err(|error| Error::new(format!("resolved Source terminal projection: {error:?}")))?
        .route();
    let leg = match rung {
        None => {
            if route != SourceResolutionRouteV1::Primary || certificate.attempt_index != 0 {
                return Err(Error::new(format!(
                    "a primary capture resolved on route {route:?} at attempt index {}: the \
                     market's first choice is attempt zero on the Primary route and nothing else \
                     is a primary answer",
                    certificate.attempt_index
                )));
            }
            "the market's PRIMARY source answered (route Primary, attempt index 0)".to_owned()
        }
        Some(_) => {
            let expected = u32::from(active_attempt_before).saturating_add(1);
            if route != SourceResolutionRouteV1::Recovery || certificate.attempt_index != expected {
                return Err(Error::new(format!(
                    "a rung capture resolved on route {route:?} at attempt index {} and the \
                     Source stood on active attempt {active_attempt_before} before it, so the \
                     certificate this market paid for names attempt {expected} on the Recovery \
                     route",
                    certificate.attempt_index
                )));
            }
            format!(
                "the market's FUNDED ALTERNATIVE answered (route Recovery, attempt index \
                 {expected} over active attempt {active_attempt_before}), and the same builder \
                 refused the primary-shaped request against this same Source by name -- {} -- \
                 which is what says the rung was ANSWERED rather than merely accepted",
                primary_refusal
                    .as_deref()
                    .unwrap_or("no refusal was recorded")
            )
        }
    };

    Ok((
        StageReportV1 {
            stage: transport_stage_v1(rung).into(),
            outcome: "executed".into(),
            transactions: submitted,
            compute_units,
            note: format!(
                "THE SOURCE IS RESOLVED AND THE CERTIFICATE IS MINTED, and {leg}. One signed VAA \
                 written in chunks and verified by the real Wormhole router, one price update \
                 posted by the real Pyth receiver, one Resolution submission and one Core-driven \
                 execution by a resolver key that is not the submitter -- and the Source is \
                 Resolved with a ResolutionSuccess certificate at {} that binds this Market at \
                 generation {} on selector {}. The Market is still ({:?}, {:?}) and carries no \
                 terminal receipt, which is this route's exact contract: a later standalone Core \
                 AdmitTerminal is what commits the Market transition, and it is the next stage. \
                 The publication this resolves against was {age} seconds old at execution against \
                 a stated shelf life of {shelf_life_seconds}: the observation is ABOUT a time \
                 inside the Market's terminal window, and its PUBLICATION is inside the band \
                 around the cluster's clock, which is the two-clock shape of the admission and \
                 not one tolerance doing both jobs. Checked before submission, not inferred \
                 after: {window_note}.",
                addresses.certificate,
                addresses.generation,
                certificate.selector,
                executed.phase,
                executed.readiness
            ),
        },
        crate::ledger::LamportClaimV1::fees(fees).with_unwatched(
            table_lamports,
            "two address lookup tables, rent-funded to route the two oversized provider frames",
        ),
    ))
}

/// Apply the publication-age floor only to the primary Pyth leg.
///
/// `PythProviderAdapterObligationV2::normalize_authenticated_recovery_update`
/// retains the terminal-window and future-skew checks but deliberately drops
/// the primary's age floor. A recovery rung can be reached only after that
/// primary grace expired, and its own committed deadline takes over. Keeping a
/// second host-only age floor here would turn an accepted chain route into an
/// unreachable campaign path.
fn require_primary_publication_freshness_v1(
    age: i64,
    shelf_life_seconds: i64,
    recovery: bool,
) -> Result<()> {
    if recovery || age <= shelf_life_seconds {
        return Ok(());
    }
    Err(Error::new(format!(
        "the Pyth publication this primary capture carries is {age} seconds old and its stated \
         shelf life is {shelf_life_seconds}. A pinned capture that has outlived its shelf life \
         must be RECAPTURED, or minted at the run's own hour by a producer that can; do not widen \
         the number to make this run pass, which is exactly the failure the bound exists to \
         prevent."
    )))
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
    rung: Option<&RungCaptureV1>,
) -> Result<String> {
    let window = WindowSpecV1::decode(
        &rpc.required_account(addresses.window_spec.raw, "window spec record")?
            .data,
    )
    .map_err(|error| Error::new(format!("WindowSpecV1: {error:?}")))?;
    // THE READING RULE IS THE RUNG'S, and it is the only edge a rung moves. A
    // rung's whole difference from the primary is the confidence bound inside
    // its own `PythAdapterConfigV1`, so a preflight that read the market's
    // configuration while the chain read the attempt's would diagnose a market
    // nobody was resolving.
    let config_record = match rung {
        None => addresses.adapter_config.raw,
        Some(rung) => rung.adapter_config.raw,
    };
    let config = PythAdapterConfigV1::decode(
        &rpc.required_account(config_record, "Pyth adapter config record")?
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
    // THE AGE FLOOR IS THE PRIMARY LEG'S ALONE, and on a rung it is dropped
    // rather than widened. `normalize_authenticated_recovery_update` states why:
    // a market only ever stands on a rung BECAUSE `now - max_age` expired --
    // the crank that advanced it is admissible one second after
    // `window.end + max_age` -- so re-applying the floor here would make every
    // rung structurally unanswerable. The rung's own bound is its committed
    // deadline, which `resolve_recovery_from_authenticated_domain` holds, and
    // the future-skew ceiling stays for both legs because nothing about
    // advancing a ladder makes a publication from the future admissible.
    let oldest = chain_now.saturating_sub(i64::from(window.max_age_seconds()));
    let newest = chain_now.saturating_add(i64::from(window.max_future_skew_seconds()));
    let too_old = rung.is_none() && publication < oldest;
    if too_old || publication > newest {
        return Err(Error::new(format!(
            "§12.3 FRESHNESS: the posted publication is at {publication} and this cluster's clock \
             admits [{}, {newest}] (now {chain_now}, max_age {}, max_future_skew {}). The \
             observation is about the right period and this cluster will not act on it. If the \
             publication is too OLD the publication this capture carries has outlived its shelf \
             life -- mint or recapture one, do not widen the window. On chain this is \
             InvalidPublicationTime and it reaches the log as 0x800A.",
            if rung.is_none() {
                oldest.to_string()
            } else {
                "no floor: the recovery leg drops it".to_owned()
            },
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
         cluster band {} at clock {chain_now} (it is FRESH ENOUGH), and its feed, exponent and \
         confidence satisfy the {} adapter configuration at {config_record}. A 0x800A from this \
         frame is therefore NOT the window",
        window.start_unix_seconds(),
        window.end_unix_seconds(),
        if rung.is_none() {
            format!("[{oldest}, {newest}]")
        } else {
            format!("(-infinity, {newest}] -- the recovery leg drops the age floor")
        },
        if rung.is_none() { "market's" } else { "rung's" }
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

/// The deployment coordinates every Core-driven provider execution reauthenticates.
///
/// ONE AUTHOR. The real capture and the two hostiles that ask the builder to
/// refuse must present the SAME deployment, or a hostile that refused for a
/// deployment reason would be read as a hostile that refused for the reason it
/// was named after.
fn execute_deployment_v3(
    plan: &SuccessorPlan,
    addresses: &ResolutionAddressesV1,
    receiver_config: Pubkey,
    live_registry_artifact: (Pubkey, Pubkey),
) -> Result<ProviderExecuteDeploymentV3> {
    Ok(ProviderExecuteDeploymentV3 {
        registry_programdata: pubkey(&plan.registry.programdata_id)?,
        registry_artifact: live_registry_artifact.0,
        registry_artifact_staging: live_registry_artifact.1,
        core_programdata: addresses.core_programdata,
        // The TRADING role, and it means it. This campaign passed CUSTODY here
        // first, because that is what the operator's own ProgramTest campaign
        // passes and the field looked like a readonly role observation rather
        // than a callee. The chain refused it: `provider_instruction_v3`
        // authenticates accounts 13/14 against
        // `activation.role(ExecutionRoleV1::Trading).release().program()` and
        // raised `ResolutionRelease` (0x8005) after 681,773 CU. The fixture
        // passes because ITS release set binds Custody's key to the Trading
        // role; a real five-role activation binds five different keys, so the
        // confusion is invisible in ProgramTest and fatal on a validator. The
        // fixture is never the authority.
        trading_program: pubkey(&plan.trading.program_id)?,
        trading_programdata: pubkey(&plan.trading.programdata_id)?,
        resolution_program: addresses.resolution_program,
        resolution_programdata: addresses.resolution_programdata,
        receiver_config,
    })
}

/// Ask the builder for a rung capture while the Market still stands on its
/// primary leg, and report the refusal it answers with.
///
/// WHAT THIS CONVICTS AND WHAT IT DOES NOT. It is a hostile against the OPERATOR
/// at an instant where the market has bought a ladder and not yet advanced onto
/// it, and it sends nothing and opens no key. The builder's conjuncts are
/// ordered, and at this instant more than one of them is false -- the Source
/// stands on Primary while the request brings the ladder, AND no update has been
/// submitted, so the lifecycle the request names is a System-owned vacancy. It
/// therefore refuses at whichever conjunct it reaches first, and the sentence it
/// refuses with is recorded verbatim rather than asserted: a stage that claimed
/// a particular discriminant here would be naming a conjunct it did not reach.
/// The phase-versus-ladder conjunct is convicted where it is REACHABLE, inside
/// [`resolve_through_pyth`], against a Source that really is standing on the
/// rung and a lifecycle that really was submitted.
///
/// `Ok(None)` means the builder BUILT one, which is a finding: a capture that
/// can answer a rung the market has not reached is a leg the holders paid for
/// and nobody had to walk.
/// `#[allow(dead_code)]`: this module is linked by `#[path]` into the ladder
/// tier as well, and the rung's hostile is that tier's alone -- the journey
/// founds no ladder and has no rung to ask about. The alternative is a copy of
/// the transport in the tier that uses more of it, which is the drift this
/// linking exists to prevent.
#[allow(dead_code)]
pub(crate) fn refuse_capture_before_the_rung_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    addresses: &ResolutionAddressesV1,
    provider: &ProviderPlanV1,
    publication: &PublicationV1,
    rung: &RungCaptureV1,
) -> Result<Option<String>> {
    let live_registry_artifact = live_registry_artifact_pair_v1(
        rpc,
        addresses.core_program,
        pubkey(&plan.registry.program_id)?,
    )?;
    let deployment = execute_deployment_v3(
        plan,
        addresses,
        provider.addresses.config,
        live_registry_artifact,
    )?;
    let snapshot = prospective_execute_snapshot_v1(rpc, addresses, provider, rung)?;
    let built = build_provider_execute_v3(
        &snapshot,
        deployment,
        &ProviderExecuteIntentV3 {
            resolver: provider.resolver.pubkey(),
            terminal_sequence: 1,
            post_update_body: publication.post_update_body.clone(),
        },
    );
    Ok(built.err().map(|error| format!("{error:?}")))
}

/// The rung's execute snapshot at an instant where some of its accounts do not
/// exist yet.
///
/// `finalized_observed_accounts` returns PRESENT accounts only, which is right
/// for a real capture -- an absent account there is a campaign that lost one --
/// and wrong for a hostile whose whole subject is a prestate. So this observes
/// by key and presents an absent account as the System-owned vacancy the chain
/// actually holds, which is what the builder would read on chain.
/// `#[allow(dead_code)]`: the rung hostile's own helper, on the same footing.
#[allow(dead_code)]
fn prospective_execute_snapshot_v1(
    rpc: &mut Rpc,
    addresses: &ResolutionAddressesV1,
    provider: &ProviderPlanV1,
    rung: &RungCaptureV1,
) -> Result<ProviderExecuteSnapshotV3> {
    let keys = [
        addresses.market,
        addresses.source_state,
        provider.lifecycle,
        provider.update.pubkey(),
        addresses.source_material.raw,
        rung.source_spec.raw,
        addresses.provider_release.raw,
        rung.adapter_config.raw,
        addresses.window_spec.raw,
        addresses.statistic_spec.raw,
        addresses.pyth_release,
        addresses.product.raw,
        addresses.result_domain.raw,
        addresses.portfolio.raw,
        rung.policy.raw,
    ];
    let (observation, present) = rpc.finalized_observed_accounts_admitting_vacant(&keys, 0)?;
    let at = |index: usize| -> Result<ObservedAccount> {
        let key = *keys.get(index).ok_or_else(|| {
            Error::new("prospective snapshot asked for an address it does not name")
        })?;
        Ok(present
            .iter()
            .find(|account| account.key == key)
            .cloned()
            .unwrap_or_else(|| crate::resolution::vacant(observation, key)))
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
        recovery_ladder: Some(ProviderExecuteLadderV3 {
            policy: at(14)?,
            policy_staging: crate::resolution::vacant(observation, rung.policy.staging),
        }),
    })
}

fn execute_snapshot(
    rpc: &mut Rpc,
    addresses: &ResolutionAddressesV1,
    lifecycle: Pubkey,
    provider: &ProviderPlanV1,
    rung: Option<&RungCaptureV1>,
) -> Result<ProviderExecuteSnapshotV3> {
    // THREE POSITIONS MOVE AND NOTHING ELSE DOES. A rung capture carries the
    // attempt's own SourceSpec and adapter configuration in the positions the
    // primary capture carries the material's, and brings the `RecoveryPolicyV2`
    // that names them at the tail. The window, the statistic, the provider
    // release and the whole Product graph are the market's on either leg, which
    // is why they are read once, above, for both.
    let (source_spec_record, adapter_config_record) = match rung {
        None => (addresses.source_spec.raw, addresses.adapter_config.raw),
        Some(rung) => (rung.source_spec.raw, rung.adapter_config.raw),
    };
    // The policy is observed only when a rung brings it, so a primary capture's
    // observation set is the one it has always been.
    let mut observed = vec![
        addresses.market,
        addresses.source_state,
        lifecycle,
        provider.update.pubkey(),
        addresses.source_material.raw,
        source_spec_record,
        addresses.provider_release.raw,
        adapter_config_record,
        addresses.window_spec.raw,
        addresses.statistic_spec.raw,
        addresses.pyth_release,
        addresses.product.raw,
        addresses.result_domain.raw,
        addresses.portfolio.raw,
    ];
    if let Some(rung) = rung {
        observed.push(rung.policy.raw);
    }
    let (observation, present) = rpc.finalized_observed_accounts(&observed, 0)?;
    let at = |index: usize| -> Result<ObservedAccount> {
        present
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("finalized observation lost an account"))
    };
    // `None` is what a primary capture has always sent, and it is what a market
    // standing on its primary source MUST send: the builder refuses `State` off
    // chain for either mismatch, so this field IS the claim about which leg the
    // capture answers on.
    //
    // The staging cursor is a VACANCY rather than an observation, for the same
    // reason every other finalized record's is: finalizing a record CLOSES its
    // cursor, so `finalized_observed_accounts` -- which returns present accounts
    // only -- would drop it and shift every index after it.
    let recovery_ladder = match rung {
        None => None,
        Some(rung) => Some(ProviderExecuteLadderV3 {
            policy: at(14)?,
            policy_staging: crate::resolution::vacant(observation, rung.policy.staging),
        }),
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
        recovery_ladder,
    })
}

/// Submit one provider transaction and accumulate its evidence.
///
/// Free function rather than a closure because the closure had to hold `rpc`
/// mutably across every call site, which made a rent lookup inside an argument
/// list a borrow error rather than a readability question.
#[allow(clippy::too_many_arguments)]
/// Write one frame as a `dclutch-devnet-frame-capture-v1` document.
///
/// THE WALL THIS EXISTS FOR IS A CHAIN THAT ONLY ANSWERS ONCE. A journey run is
/// forty minutes and tears its validator down, so every question asked of a
/// refusing frame cost another run -- and `ResolutionError::FinalizedRecord`
/// 0x8004 is one wire code over twelve raise sites in this route alone. A
/// capture turns that into an offline instrument:
/// `programs/dclutch-trading-sbf/program-test/devnet-replay` replays exactly
/// this document in `ProgramTest`, and `--set-account` moves ONE input at a
/// time, which is how a coarse code is convicted
/// (`docs/design/DEVNET_FRAME_REPLAY_V1.md`, step 5).
///
/// Written whether the frame refuses or lands: a capture of a frame that WORKED
/// is the control every mutation is read against.
///
/// The packet is compiled the way `Rpc::send_v0_*` compiles it -- the same
/// bounded instructions, the same tables, the same fee payer -- rather than
/// intercepted from the send, because the replay rewrites the blockhash and
/// verifies no signature. What must be identical is the message: its accounts,
/// their privileges, and the instruction data.
fn write_frame_capture_v1(
    rpc: &mut Rpc,
    path: &std::path::Path,
    label: &str,
    instructions: &[Instruction],
    fee_payer: Pubkey,
    observation: dclutch_resolution_core_v3_operator::Observation,
    tables: &[ObservedAccount],
) -> Result<()> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    let bounded = crate::rpc::bounded_instructions(instructions, None)?;
    let (blockhash, _) = rpc.recent_blockhash_with_height_v1()?;
    let plan = dclutch_versioned_message_operator::compile_v0_message_with_optional_tables(
        fee_payer,
        &bounded,
        solana_hash::Hash::new_from_array(blockhash.to_bytes()),
        observation,
        tables,
    )
    .map_err(|error| Error::new(format!("{label}: capture message compilation: {error:?}")))?;
    let transaction = solana_sdk::transaction::VersionedTransaction {
        signatures: vec![
            solana_sdk::signature::Signature::default();
            usize::from(plan.required_signatures.max(1))
        ],
        message: plan.message,
    };
    let packet = bincode::serialize(&transaction)
        .map_err(|error| Error::new(format!("{label}: capture serialize: {error}")))?;
    // Every address the packet can name: the frame's own, the programs it
    // invokes, the fee payer, and the lookup tables the runtime resolves
    // through. A table is an account too, and a replay that lacks one resolves
    // no address at all.
    let mut addresses = std::collections::BTreeSet::new();
    addresses.insert(fee_payer);
    for instruction in &bounded {
        addresses.insert(instruction.program_id);
        for meta in &instruction.accounts {
            addresses.insert(meta.pubkey);
        }
    }
    for table in tables {
        addresses.insert(table.key);
    }
    let mut state = serde_json::Map::new();
    for address in addresses {
        // An absent account is simply absent, which is what the replay expects
        // and what the chain itself presented.
        if let Some(account) = rpc.account(address)? {
            state.insert(
                address.to_string(),
                serde_json::json!({
                    "lamports": account.lamports,
                    "owner": account.owner.to_string(),
                    "executable": account.executable,
                    "rentEpoch": account.rent_epoch,
                    "dataBase64": BASE64.encode(&account.data),
                }),
            );
        }
    }
    // THE FRAME, IN ITS OWN ORDER. The replay needs only the packet, but a
    // reader convicting a coarse refusal needs to say "the account at index
    // 17", and recovering an index from a compiled v0 message means resolving
    // its lookup tables by hand. The program reads its accounts positionally,
    // so the positions are what the capture states.
    let frames: Vec<serde_json::Value> = bounded
        .iter()
        .map(|instruction| {
            serde_json::json!({
                "programId": instruction.program_id.to_string(),
                "accounts": instruction
                    .accounts
                    .iter()
                    .map(|meta| meta.pubkey.to_string())
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    let document = serde_json::json!({
        "schema": "dclutch-devnet-frame-capture-v1",
        "label": label,
        "warpSlot": rpc.finalized_slot()?,
        "transactionBase64": BASE64.encode(&packet),
        "instructions": frames,
        "state": state,
    });
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&document)?)?;
    eprintln!(
        "journey: frame capture written to {} ({} accounts, {} packet bytes)",
        path.display(),
        document
            .get("state")
            .and_then(serde_json::Value::as_object)
            .map_or(0, serde_json::Map::len),
        packet.len()
    );
    Ok(())
}

/// The Registry artifact-release record pair the LIVE infrastructure profile
/// names.
///
/// DERIVED FROM THE CHAIN, because the plan's `registry_artifact_release` is the
/// PREDECESSOR. The substrate succeeds the Registry before the founding
/// (`infrastructure_succession`), so the V2 profile Core owns names a successor
/// release id and keeps the plan's id in its `predecessor_registry_artifact`
/// field. `authenticate_infrastructure` reads that profile and requires the
/// frame's record to live at the hash of the CURRENT id, so a frame built from
/// the plan carries a record that is finalized, self-consistent, Registry-owned,
/// exactly 216 bytes, with a vacant cursor -- and is the wrong record.
///
/// That is what refused every Pyth submit this tier ever sent:
/// `ResolutionError::FinalizedRecord` 0x8004 at
/// `programs/dclutch-resolution-proof-sbf/src/provider_transport_v3.rs:672`,
/// convicted by replaying run 13's captured frame and corrupting each of the six
/// records in turn -- none of the six moved the compute units, so the frame
/// never reached five of them (hbox `20260906T140439Z`; profile
/// `ff5763c6...`, frame `616c2f91...`, which is the profile's own predecessor).
///
/// The addresses are the program's own derivation, not a lookup: a succession
/// this tier does not know about moves both together.
fn live_registry_artifact_pair_v1(
    rpc: &mut Rpc,
    core_program: Pubkey,
    registry: Pubkey,
) -> Result<(Pubkey, Pubkey)> {
    use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
    use dclutch_registry::release_set::{
        PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2, ProtocolInfrastructureProfileV2,
    };
    let address = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &core_program,
    )
    .0;
    let account = rpc.required_account(address, "protocol infrastructure profile")?;
    if account.owner != core_program
        || account.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
    {
        return Err(Error::new(format!(
            "the infrastructure profile at {address} is {} bytes owned by {}, not the \
             {PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2} bytes Core owns",
            account.data.len(),
            account.owner
        )));
    }
    let profile = ProtocolInfrastructureProfileV2::decode(&account.data)
        .map_err(|error| Error::new(format!("infrastructure profile: {error:?}")))?;
    let digest = profile.registry().artifact_release().to_bytes();
    let schema = dclutch_registry::ARTIFACT_RELEASE_SCHEMA_ID_V1;
    let raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0;
    // The frame is doomed without it, and a missing record is a statement about
    // the succession rather than about this route.
    rpc.required_account(
        raw,
        "the Registry artifact release the live infrastructure profile names",
    )?;
    Ok((raw, staging))
}

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

/// Say WHICH record the submit's `FinalizedRecord` is about, before sending.
///
/// `ResolutionError::FinalizedRecord` (`0x8004`) is one code over eleven
/// disjuncts and six record pairs, and the operator's own client-side
/// `authenticate_raw` tests four of the eleven -- so a submit that BUILDS and
/// then refuses on chain says nothing about which record, and the journey's
/// transcript recorded exactly that: `custom program error: 0x8004` on
/// instruction 2, unlocalized.
///
/// This is the tree's own instrument first, as the refusal doctrine asks. It
/// takes no account indices and no record widths -- a second copy of the frame
/// layout is a second author for it. It takes the frame's own accounts and asks
/// each one the question the program asks: a finalized raw record LIVES AT THE
/// HASH OF ITS OWN BODY, so an account whose bytes reproduce its own address
/// under one of the schemas this frame can carry is self-consistent, and the
/// staging cursor paired with that same (schema, digest) must be vacant --
/// System-owned, zero lamports, zero bytes -- which is the whole of what
/// "finalized" means here.
///
/// Its verdict is a sentence either way. If every registry-owned account in the
/// frame is self-consistent and every paired cursor is vacant, then no record
/// in the frame is unfinalized, and the refusal is a DISAGREEMENT about which
/// record was expected -- for which this frame has exactly one candidate, the
/// artifact release whose expected digest the program reads out of the on-chain
/// infrastructure profile rather than out of the frame.
fn authenticate_frame_records_v1(
    rpc: &mut Rpc,
    registry: Pubkey,
    instruction: &solana_sdk::instruction::Instruction,
) -> Result<String> {
    use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
    let schemas: [(&str, [u8; 32]); 6] = [
        (
            "ArtifactReleaseV1",
            dclutch_registry::ARTIFACT_RELEASE_SCHEMA_ID_V1,
        ),
        (
            "SourceMaterialV3",
            dclutch_source::SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        ),
        ("SourceSpecV1", dclutch_source::SOURCE_SPEC_SCHEMA_ID_V1),
        (
            "ProviderReleaseV1",
            dclutch_source::PROVIDER_RELEASE_SCHEMA_ID_V1,
        ),
        (
            "PythReleaseV1",
            dclutch_source::resolution::PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        ),
        ("WindowSpecV1", dclutch_source::WINDOW_SPEC_SCHEMA_ID_V1),
    ];
    let mut consistent = Vec::new();
    let mut unexplained = Vec::new();
    let mut unfinalized = Vec::new();
    let mut absent = Vec::new();
    for meta in &instruction.accounts {
        let Some(account) = rpc.account(meta.pubkey)? else {
            // An ABSENCE is reported rather than skipped. Some of this frame's
            // accounts are legitimately vacant before the transaction runs, so
            // this is not a refusal; but a raw record that is simply not there
            // reads to the program exactly like one that is wrong, and a probe
            // that silently passed over it would have measured nothing.
            absent.push(meta.pubkey.to_string());
            continue;
        };
        if account.owner != registry || account.executable || account.data.is_empty() {
            continue;
        }
        let digest = hash(&account.data).to_bytes();
        let matched = schemas.iter().find(|(_, schema)| {
            Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, schema, &digest], &registry).0
                == meta.pubkey
        });
        let Some((name, schema)) = matched else {
            unexplained.push(format!(
                "{} ({} bytes, registry-owned, and its own body's hash reproduces no address                  under any schema this frame carries)",
                meta.pubkey,
                account.data.len()
            ));
            continue;
        };
        let cursor =
            Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, schema, &digest], &registry)
                .0;
        match rpc.account(cursor)? {
            None => consistent.push(*name),
            Some(staging)
                if staging.owner == system_program::ID
                    && staging.lamports == 0
                    && staging.data.is_empty()
                    && !staging.executable =>
            {
                consistent.push(*name);
            }
            Some(staging) => unfinalized.push(format!(
                "{name} {} is NOT finalized: its staging cursor {cursor} is still live ({} \
                 lamports, {} bytes, owner {})",
                meta.pubkey,
                staging.lamports,
                staging.data.len(),
                staging.owner
            )),
        }
    }
    // A REPORT, NEVER A REFUSAL. The transaction is sent afterwards whatever
    // this says: the probe's verdict and the chain's own code are two readings
    // and the pair is what localizes the wall, so a probe that refused would
    // trade a conviction for a suspicion. It also cannot know which
    // registry-owned accounts in the frame are RECORDS -- the activation cache
    // is Registry's too, and run 8 read its 1,288 bytes as "not
    // self-consistent", which was the probe describing an account nobody
    // claimed was a record.
    Ok(format!(
        "{} self-consistent finalized record(s) with vacant cursors ({}); {} registry-owned frame \
         account(s) that are not records of these schemas [{}]; {} UNFINALIZED [{}]; {} frame \
         account(s) vacant [{}]",
        consistent.len(),
        consistent.join(", "),
        unexplained.len(),
        unexplained.join("; "),
        unfinalized.len(),
        unfinalized.join("; "),
        absent.len(),
        absent.join(" ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The primary walk's bytes did not move when the transport stopped being
    /// about one frozen instant.
    ///
    /// `resolve_through_pyth` used to read the four pinned constants directly;
    /// it now takes a [`PublicationV1`], and the journey hands it
    /// [`PublicationV1::captured`]. That refactor is only safe if the captured
    /// publication IS those constants, and this is the only place that can say
    /// so -- the journey itself cannot, because a transport that quietly sent
    /// different bytes would still resolve a market and still go green.
    #[test]
    fn the_captured_publication_is_the_pinned_fixture_byte_for_byte() {
        let captured = PublicationV1::captured();
        assert_eq!(captured.signed_vaa, SIGNED_VAA);
        assert_eq!(
            captured.post_update_body,
            &RECEIVER_POST_UPDATE[POST_UPDATE_TAG_BYTES..],
            "the body is the captured PostUpdate WITHOUT its Anchor tag, which is what both \
             provider intents take"
        );
        assert_eq!(captured.price_update_image, PRICE_UPDATE);
        assert_eq!(captured.shelf_life_seconds, FIXTURE_SHELF_LIFE_SECONDS);
        assert!(
            captured.post_update_body.len() + POST_UPDATE_TAG_BYTES == RECEIVER_POST_UPDATE.len(),
            "exactly the tag was dropped, not a byte more"
        );
    }

    /// The recovery transition is reachable only after the primary freshness
    /// grace elapsed. The canonical adapter keeps the terminal-window and
    /// future-skew bounds on that transition, while deliberately dropping this
    /// lower age bound in favour of the rung's committed deadline.
    #[test]
    fn primary_age_floor_does_not_shadow_the_recovery_deadline() {
        let age = 121_i64;
        let shelf_life = 120_i64;
        let primary = require_primary_publication_freshness_v1(age, shelf_life, false)
            .expect_err("the primary leg still refuses a stale publication");
        assert!(primary.to_string().contains("primary capture"));
        require_primary_publication_freshness_v1(age, shelf_life, true)
            .expect("the recovery leg delegates its later deadline to the canonical adapter");
    }

    /// A capture that answers the funded rung does not report the sentence a
    /// capture on the market's first choice reports.
    ///
    /// The stage label is what a reader of a transcript sees, and the two legs
    /// are the whole difference between a market whose first choice answered
    /// and a market whose holders paid for an alternative that did. One label
    /// for both would report the leg they paid for as the leg they never
    /// needed.
    #[test]
    fn a_rung_capture_and_a_primary_capture_do_not_share_a_stage_label() {
        let rung = RungCaptureV1 {
            policy: crate::resolution::RecordPairV1::derive(Pubkey::new_unique(), [1; 32], b"p"),
            source_spec: crate::resolution::RecordPairV1::derive(
                Pubkey::new_unique(),
                [2; 32],
                b"s",
            ),
            adapter_config: crate::resolution::RecordPairV1::derive(
                Pubkey::new_unique(),
                [3; 32],
                b"a",
            ),
        };
        assert_eq!(transport_stage_v1(None), PYTH_TRANSPORT_STAGE_V1);
        assert_eq!(
            transport_stage_v1(Some(&rung)),
            PYTH_RECOVERY_TRANSPORT_STAGE_V1
        );
        assert_ne!(PYTH_TRANSPORT_STAGE_V1, PYTH_RECOVERY_TRANSPORT_STAGE_V1);
    }

    /// A rung's three record pairs are three distinct addresses, and each is a
    /// function of BOTH the schema and the body.
    ///
    /// This is what makes deriving them by content identity safe where the
    /// founding's evidence map does not publish them: the alternative
    /// `SourceSpecV1` and its `PythAdapterConfigV1` are two different schemas
    /// over two different bodies, so neither can be reached by presenting the
    /// other, and a body that changed by one byte lands somewhere else rather
    /// than shadowing the record the market founded.
    #[test]
    fn a_rungs_records_are_addressed_by_both_their_schema_and_their_body() {
        let registry = Pubkey::new_unique();
        let spec_schema = dclutch_source::SOURCE_SPEC_SCHEMA_ID_V1;
        let adapter_schema = dclutch_source::PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1;
        assert_ne!(
            spec_schema, adapter_schema,
            "the two schemas a rung's records live under are not one schema"
        );
        let body = b"the same bytes under two schemas".as_slice();
        let spec = crate::resolution::RecordPairV1::derive(registry, spec_schema, body);
        let adapter = crate::resolution::RecordPairV1::derive(registry, adapter_schema, body);
        assert_ne!(
            spec.raw, adapter.raw,
            "one body under two schemas is two records"
        );
        assert_ne!(
            spec.raw, spec.staging,
            "a record and its staging cursor are two accounts"
        );
        let mut moved = body.to_vec();
        moved.push(0);
        assert_ne!(
            spec.raw,
            crate::resolution::RecordPairV1::derive(registry, spec_schema, &moved).raw,
            "a body that changed by one byte is a different record"
        );
        assert_ne!(
            spec.raw,
            crate::resolution::RecordPairV1::derive(Pubkey::new_unique(), spec_schema, body).raw,
            "and a record of one registry is not a record of another"
        );
    }
}
