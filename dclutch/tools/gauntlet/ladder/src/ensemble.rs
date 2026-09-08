//! Direct Ensemble capture producer for the private-validator ladder.
//!
//! This module owns the member-specific transport boundary.  It consumes a
//! real Pyth update lifecycle posted by this campaign and builds the distinct
//! Resolution member packet; it never calls Journey's terminal producer.

use dclutch_provider_transport_v3_operator::{
    ProviderEnsembleMemberExecuteIntentV3, ProviderExecuteDeploymentV3, ProviderExecuteLadderV3,
    ProviderExecuteSnapshotV3, ProviderTransportReportV3,
    build_provider_ensemble_member_execute_v3,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_source::{
    EnsembleFoldReceiptV1, EnsembleSpecV1, PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1, RecoveryPolicyV2,
    SourceMaterialV3, SourceResolutionPhaseV1, SourceResolutionStateV2,
    relay::instruction::{EnsembleFoldInstructionV1, ReclaimMemberSeatInstructionV1},
    resolution::{
        EnsembleFoldReceiptSeatSeedsV1, EnsembleFragmentSeatSeedsV1,
        RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3, ResolutionCertificateKindV2,
    },
};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Error, Result,
    model::SuccessorPlan,
    provider::ProviderPlanV1,
    resolution::{RecordPairV1, ResolutionAddressesV1},
    rpc::Rpc,
};

/// Build one direct member capture from one finalized post-update lifecycle.
///
/// Member zero carries the primary source records. Later members are derived
/// from the immutable policy rather than chosen by the caller, so the same
/// member byte cannot be redirected to another Pyth configuration.
pub(crate) fn build_member_capture_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    addresses: &ResolutionAddressesV1,
    provider: &ProviderPlanV1,
    lifecycle: Pubkey,
    deployment: ProviderExecuteDeploymentV3,
    member: u8,
    terminal_sequence: u64,
    post_update_body: Vec<u8>,
) -> Result<ProviderTransportReportV3> {
    let material_account =
        rpc.required_account(addresses.source_material.raw, "Ensemble material")?;
    let material = SourceMaterialV3::decode(&material_account.data)
        .map_err(|error| Error::new(format!("Ensemble SourceMaterialV3: {error:?}")))?;
    if !material.ensemble().declares_member(member) || material.ensemble().is_single() {
        return Err(Error::new(
            "requested member is not declared by a multi-member material",
        ));
    }
    let policy_account = rpc.required_account(addresses.recovery_policy.raw, "Ensemble policy")?;
    let policy = RecoveryPolicyV2::decode(&policy_account.data)
        .map_err(|error| Error::new(format!("Ensemble RecoveryPolicyV2: {error:?}")))?;
    let (source_spec, adapter) = if member == 0 {
        (addresses.source_spec, addresses.adapter_config)
    } else {
        let attempt = policy
            .member_attempt(material.ensemble(), member)
            .map_err(|error| Error::new(format!("Ensemble member policy: {error:?}")))?;
        let spec = record_pair_for_id_v1(
            addresses.registry_program,
            dclutch_source::SOURCE_SPEC_SCHEMA_ID_V1,
            attempt.source_spec_id().to_bytes(),
        );
        // The record identity is the hash of its body, not a body we can
        // reconstruct. Read the finalized spec then derive its adapter pair.
        let spec_account = rpc.required_account(spec.raw, "Ensemble member SourceSpec")?;
        let spec_value = dclutch_source::SourceSpecV1::decode(&spec_account.data)
            .map_err(|error| Error::new(format!("Ensemble member SourceSpecV1: {error:?}")))?;
        let adapter = record_pair_for_id_v1(
            addresses.registry_program,
            PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
            spec_value.adapter_config_id().to_bytes(),
        );
        (spec, adapter)
    };
    let keys = [
        addresses.market,
        addresses.source_state,
        lifecycle,
        provider.update.pubkey(),
        addresses.source_material.raw,
        source_spec.raw,
        addresses.provider_release.raw,
        adapter.raw,
        addresses.window_spec.raw,
        addresses.statistic_spec.raw,
        addresses.pyth_release,
        addresses.product.raw,
        addresses.result_domain.raw,
        addresses.portfolio.raw,
        addresses.recovery_policy.raw,
    ];
    let (observation, observed) = rpc.finalized_observed_accounts(&keys, 0)?;
    let at = |index: usize| -> Result<dclutch_resolution_core_v3_operator::ObservedAccount> {
        observed
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("finalized Ensemble capture observation lost an account"))
    };
    let snapshot = ProviderExecuteSnapshotV3 {
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
            policy_staging: crate::resolution::vacant(
                observation,
                addresses.recovery_policy.staging,
            ),
        }),
    };
    let _ = plan; // The checked plan is consumed by the caller's deployment derivation.
    build_provider_ensemble_member_execute_v3(
        &snapshot,
        deployment,
        &ProviderEnsembleMemberExecuteIntentV3 {
            resolver: provider.resolver.pubkey(),
            terminal_sequence,
            member,
            post_update_body,
        },
    )
    .map_err(|error| Error::new(format!("Ensemble direct member operator: {error:?}")))
}

/// Derive a finalized raw/staging pair from an already authenticated content
/// identity. `RecordPairV1::derive` hashes a body; policy attempts carry the
/// identity itself, so hashing it again would name a different record.
fn record_pair_for_id_v1(registry: Pubkey, schema: [u8; 32], identity: [u8; 32]) -> RecordPairV1 {
    RecordPairV1 {
        raw: Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &identity], &registry)
            .0,
        staging: Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &identity],
            &registry,
        )
        .0,
    }
}

/// One real Pyth VAA transport followed by its authenticated direct-member
/// capture.  The VAA is posted through the receiver before the operator sees
/// `provider.update`; no fixture account or native substitute crosses this
/// boundary.
pub(crate) fn post_submit_and_capture_member_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    plan: &SuccessorPlan,
    addresses: &ResolutionAddressesV1,
    provider: &ProviderPlanV1,
    publication: &crate::pyth_lab_publication::LabPublicationV1,
    member: u8,
    terminal_sequence: u64,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<ProviderTransportReportV3> {
    let slot = rpc.finalized_slot()?;
    let chain_now = rpc.block_time(slot)?;
    let pyth = provider.addresses;
    initialize_pyth_transport_v1(rpc, payer, pyth, transactions)?;
    post_verified_vaa_v1(rpc, payer, provider, publication, transactions)?;

    let artifact =
        live_registry_artifact_pair_v1(rpc, addresses.core_program, addresses.registry_program)?;
    let submit = dclutch_provider_transport_v3_operator::build_provider_submit_v3(
        &submit_snapshot_v1(rpc, addresses, provider.encoded_vaa.pubkey(), plan)?,
        dclutch_provider_transport_v3_operator::ProviderSubmitDeploymentV3 {
            infrastructure: solana_sdk::pubkey::Pubkey::find_program_address(
                &[dclutch_registry::release_set::PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
                &addresses.core_program,
            )
            .0,
            registry_programdata: crate::plan::pubkey(&plan.registry.programdata_id)?,
            registry_artifact: artifact.0,
            registry_artifact_staging: artifact.1,
            core_programdata: addresses.core_programdata,
            resolution_program: addresses.resolution_program,
            resolution_programdata: addresses.resolution_programdata,
            receiver_config: pyth.config,
            guardian_set: pyth.guardian_set,
        },
        &dclutch_provider_transport_v3_operator::ProviderSubmitIntentV3 {
            submitter: payer.pubkey(),
            refund_recipient: addresses.rent_beneficiary,
            update_account: provider.update.pubkey(),
            reclaim_after_unix_seconds: chain_now.saturating_add(3_600),
            post_update_body: publication.post_update_body.clone(),
        },
    )
    .map_err(|error| Error::new(format!("Ensemble provider submit builder: {error:?}")))?;
    if submit.lifecycle != provider.lifecycle {
        return Err(Error::new(
            "Ensemble submit lifecycle disagrees with its derived provider plan",
        ));
    }
    let lifecycle_rent =
        rpc.minimum_balance(dclutch_source::resolution::PROVIDER_UPDATE_LIFECYCLE_BYTES_V3)?;
    transactions.push(rpc.send_with_signers(
        "ensemble: prepay the provider update lifecycle",
        &[solana_system_interface::instruction::transfer(
            &payer.pubkey(),
            &submit.lifecycle,
            lifecycle_rent,
        )],
        payer,
        &[],
    )?);
    send_wide_v1(
        rpc,
        payer,
        "ensemble: submit a verified real Pyth update",
        &submit.instruction,
        &[&provider.update],
        transactions,
    )?;
    let posted = rpc.required_account(provider.update.pubkey(), "Ensemble posted PriceUpdateV2")?;
    if posted.owner != pyth.receiver {
        return Err(Error::new(
            "the Ensemble receiver did not own the posted PriceUpdateV2",
        ));
    }

    let deployment = ProviderExecuteDeploymentV3 {
        registry_programdata: crate::plan::pubkey(&plan.registry.programdata_id)?,
        registry_artifact: artifact.0,
        registry_artifact_staging: artifact.1,
        core_programdata: addresses.core_programdata,
        trading_program: crate::plan::pubkey(&plan.trading.program_id)?,
        trading_programdata: crate::plan::pubkey(&plan.trading.programdata_id)?,
        resolution_program: addresses.resolution_program,
        resolution_programdata: addresses.resolution_programdata,
        receiver_config: pyth.config,
    };
    let capture = build_member_capture_v1(
        rpc,
        plan,
        addresses,
        provider,
        submit.lifecycle,
        deployment,
        member,
        terminal_sequence,
        publication.post_update_body.clone(),
    )?;
    let seat = capture
        .instruction
        .accounts
        .get(3)
        .ok_or_else(|| Error::new("direct Ensemble frame omitted its fragment seat"))?
        .pubkey;
    let fragment_rent =
        rpc.minimum_balance(dclutch_source::resolution::RESOLUTION_CERTIFICATE_BYTES_V2)?;
    let resolver_rent = rpc.minimum_balance(0)?;
    transactions.push(rpc.send_with_signers(
        "ensemble: prepay the member fragment seat and distinct resolver",
        &[
            solana_system_interface::instruction::transfer(&payer.pubkey(), &seat, fragment_rent),
            solana_system_interface::instruction::transfer(
                &payer.pubkey(),
                &provider.resolver.pubkey(),
                resolver_rent,
            ),
        ],
        payer,
        &[],
    )?);
    send_wide_v1(
        rpc,
        payer,
        "ensemble: Resolution captures one authenticated member fragment",
        &capture.instruction,
        &[&provider.resolver],
        transactions,
    )?;
    Ok(capture)
}

const ROUTER_INITIALIZE_V1: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/router-initialize.data");
const RECEIVER_INITIALIZE_V1: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-initialize.data");
const ENCODED_VAA_HEADER_BYTES_V1: usize = 46;
const WRITE_CHUNK_BYTES_V1: usize = 600;

fn initialize_pyth_transport_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    pyth: crate::provider::ProviderAddressesV1,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<()> {
    use solana_sdk::{
        instruction::{AccountMeta, Instruction},
        sysvar,
    };
    use solana_sdk_ids::system_program;
    if rpc.account(pyth.guardian_set)?.is_none() {
        transactions.push(rpc.send_with_signers(
            "ensemble: initialize the real Pyth router",
            &[Instruction {
                program_id: pyth.router,
                accounts: vec![
                    AccountMeta::new(pyth.bridge, false),
                    AccountMeta::new(pyth.guardian_set, false),
                    AccountMeta::new(pyth.fee_collector, false),
                    AccountMeta::new(payer.pubkey(), true),
                    AccountMeta::new_readonly(sysvar::clock::ID, false),
                    AccountMeta::new_readonly(sysvar::rent::ID, false),
                    AccountMeta::new_readonly(system_program::ID, false),
                ],
                data: ROUTER_INITIALIZE_V1.to_vec(),
            }],
            payer,
            &[],
        )?);
    }
    if rpc.account(pyth.config)?.is_none() {
        transactions.push(rpc.send_with_signers(
            "ensemble: initialize the real Pyth receiver",
            &[Instruction {
                program_id: pyth.receiver,
                accounts: vec![
                    AccountMeta::new(payer.pubkey(), true),
                    AccountMeta::new(pyth.config, false),
                    AccountMeta::new_readonly(system_program::ID, false),
                ],
                data: RECEIVER_INITIALIZE_V1.to_vec(),
            }],
            payer,
            &[],
        )?);
    }
    let treasury_rent = rpc.minimum_balance(0)?;
    if rpc
        .account(pyth.treasury)?
        .map(|account| account.lamports)
        .unwrap_or(0)
        < treasury_rent
    {
        transactions.push(rpc.send_with_signers(
            "ensemble: capitalize the real Pyth receiver treasury",
            &[solana_system_interface::instruction::transfer(
                &payer.pubkey(),
                &pyth.treasury,
                treasury_rent,
            )],
            payer,
            &[],
        )?);
    }
    Ok(())
}

fn post_verified_vaa_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    provider: &ProviderPlanV1,
    publication: &crate::pyth_lab_publication::LabPublicationV1,
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<()> {
    use solana_program::hash::hash;
    use solana_sdk::instruction::{AccountMeta, Instruction};
    let encoded = &provider.encoded_vaa;
    let rent = rpc.minimum_balance(ENCODED_VAA_HEADER_BYTES_V1 + publication.signed_vaa.len())?;
    transactions.push(rpc.send_with_signers(
        "ensemble: create encoded real-Pyth VAA",
        &[solana_system_interface::instruction::create_account(
            &payer.pubkey(),
            &encoded.pubkey(),
            rent,
            (ENCODED_VAA_HEADER_BYTES_V1 + publication.signed_vaa.len()) as u64,
            &provider.addresses.router,
        )],
        payer,
        &[encoded],
    )?);
    let discriminator = |name: &[u8]| hash(name).to_bytes()[..8].to_vec();
    transactions.push(rpc.send_with_signers(
        "ensemble: initialize encoded real-Pyth VAA",
        &[Instruction {
            program_id: provider.addresses.router,
            accounts: vec![
                AccountMeta::new_readonly(payer.pubkey(), true),
                AccountMeta::new(encoded.pubkey(), false),
            ],
            data: discriminator(b"global:init_encoded_vaa"),
        }],
        payer,
        &[],
    )?);
    for (index, chunk) in publication
        .signed_vaa
        .chunks(WRITE_CHUNK_BYTES_V1)
        .enumerate()
    {
        let offset = index
            .checked_mul(WRITE_CHUNK_BYTES_V1)
            .ok_or_else(|| Error::new("Ensemble VAA chunk offset overflowed"))?;
        let mut data = discriminator(b"global:write_encoded_vaa");
        data.extend_from_slice(
            &u32::try_from(offset)
                .map_err(|_| Error::new("Ensemble VAA offset exceeds u32"))?
                .to_le_bytes(),
        );
        data.extend_from_slice(
            &u32::try_from(chunk.len())
                .map_err(|_| Error::new("Ensemble VAA chunk exceeds u32"))?
                .to_le_bytes(),
        );
        data.extend_from_slice(chunk);
        transactions.push(rpc.send_with_signers(
            &format!("ensemble: write signed real-Pyth VAA chunk {index}"),
            &[Instruction {
                program_id: provider.addresses.router,
                accounts: vec![
                    AccountMeta::new_readonly(payer.pubkey(), true),
                    AccountMeta::new(encoded.pubkey(), false),
                ],
                data,
            }],
            payer,
            &[],
        )?);
    }
    transactions.push(rpc.send_with_signers(
        "ensemble: cryptographically verify the real Pyth VAA",
        &[Instruction {
            program_id: provider.addresses.router,
            accounts: vec![
                AccountMeta::new_readonly(payer.pubkey(), true),
                AccountMeta::new(encoded.pubkey(), false),
                AccountMeta::new_readonly(provider.addresses.guardian_set, false),
            ],
            data: discriminator(b"global:verify_encoded_vaa_v1"),
        }],
        payer,
        &[],
    )?);
    if rpc
        .required_account(encoded.pubkey(), "Ensemble verified EncodedVaa")?
        .data
        .get(8)
        != Some(&2)
    {
        return Err(Error::new(
            "the real Pyth router did not verify the Ensemble VAA",
        ));
    }
    Ok(())
}

fn submit_snapshot_v1(
    rpc: &mut Rpc,
    addresses: &ResolutionAddressesV1,
    encoded_vaa: Pubkey,
    plan: &SuccessorPlan,
) -> Result<dclutch_provider_transport_v3_operator::ProviderSubmitSnapshotV3> {
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
    let at = |index| {
        present
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("Ensemble submit observation lost an account"))
    };
    Ok(
        dclutch_provider_transport_v3_operator::ProviderSubmitSnapshotV3 {
            market: at(0)?,
            source_state: at(1)?,
            source_material: at(2)?,
            source_spec: at(3)?,
            source_provider_release: at(4)?,
            pyth_release: at(5)?,
            window: at(6)?,
            encoded_vaa: at(7)?,
        },
    )
}

fn live_registry_artifact_pair_v1(
    rpc: &mut Rpc,
    core_program: Pubkey,
    registry: Pubkey,
) -> Result<(Pubkey, Pubkey)> {
    use dclutch_registry::{
        record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1},
        release_set::{
            PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2,
            PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, ProtocolInfrastructureProfileV2,
        },
    };
    let profile_address = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &core_program,
    )
    .0;
    let account = rpc.required_account(profile_address, "Ensemble infrastructure profile")?;
    if account.owner != core_program
        || account.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
    {
        return Err(Error::new(
            "Ensemble infrastructure profile has the wrong owner or width",
        ));
    }
    let profile = ProtocolInfrastructureProfileV2::decode(&account.data)
        .map_err(|error| Error::new(format!("Ensemble infrastructure profile: {error:?}")))?;
    let identity = profile.registry().artifact_release().to_bytes();
    let schema = dclutch_registry::ARTIFACT_RELEASE_SCHEMA_ID_V1;
    let raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &identity], &registry).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &identity], &registry)
            .0;
    rpc.required_account(raw, "Ensemble live Registry artifact release")?;
    Ok((raw, staging))
}

/// Build the exact fold frame from the Market's finalized graph and the two
/// resolver keys which actually authored fragments. Empty members still occupy
/// their contract-mandated captor position, but cannot receive a payment.
fn ensemble_fold_instruction_v1(
    rpc: &mut Rpc,
    payer: Pubkey,
    addresses: &ResolutionAddressesV1,
    captors: &[Option<Pubkey>],
    terminal_sequence: u64,
) -> Result<(Instruction, Pubkey, Vec<Pubkey>)> {
    let material_account =
        rpc.required_account(addresses.source_material.raw, "Ensemble material for fold")?;
    let material = SourceMaterialV3::decode(&material_account.data)
        .map_err(|error| Error::new(format!("Ensemble fold SourceMaterialV3: {error:?}")))?;
    let members = material.ensemble().members();
    if captors.len() != usize::from(members) {
        return Err(Error::new(
            "Ensemble fold captor list does not cover every declared member",
        ));
    }
    let receipt = Pubkey::find_program_address(
        &EnsembleFoldReceiptSeatSeedsV1::new(addresses.source_state.to_bytes(), terminal_sequence)
            .seeds(),
        &addresses.resolution_program,
    )
    .0;
    let certificate = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            addresses.source_state.as_ref(),
            &[ResolutionCertificateKindV2::ResolutionSuccess.kind_seed()],
            &terminal_sequence.to_le_bytes(),
        ],
        &addresses.resolution_program,
    )
    .0;
    let mut accounts = vec![
        AccountMeta::new(payer, true),
        AccountMeta::new_readonly(addresses.market, false),
        AccountMeta::new_readonly(addresses.core_program, false),
        AccountMeta::new_readonly(addresses.activation_cache, false),
        AccountMeta::new(addresses.source_state, false),
        AccountMeta::new(certificate, false),
        AccountMeta::new(receipt, false),
        AccountMeta::new_readonly(addresses.source_material.raw, false),
        AccountMeta::new_readonly(addresses.source_material.staging, false),
        AccountMeta::new_readonly(addresses.source_spec.raw, false),
        AccountMeta::new_readonly(addresses.source_spec.staging, false),
        AccountMeta::new_readonly(addresses.window_spec.raw, false),
        AccountMeta::new_readonly(addresses.window_spec.staging, false),
        AccountMeta::new_readonly(addresses.statistic_spec.raw, false),
        AccountMeta::new_readonly(addresses.statistic_spec.staging, false),
        AccountMeta::new_readonly(addresses.recovery_policy.raw, false),
        AccountMeta::new_readonly(addresses.recovery_policy.staging, false),
        AccountMeta::new_readonly(addresses.product.raw, false),
        AccountMeta::new_readonly(addresses.product.staging, false),
        AccountMeta::new_readonly(addresses.result_domain.raw, false),
        AccountMeta::new_readonly(addresses.result_domain.staging, false),
        AccountMeta::new_readonly(addresses.portfolio.raw, false),
        AccountMeta::new_readonly(addresses.portfolio.staging, false),
        AccountMeta::new_readonly(addresses.capability_manifest.raw, false),
        AccountMeta::new_readonly(addresses.capability_manifest.staging, false),
        AccountMeta::new(addresses.funding, false),
        AccountMeta::new_readonly(sysvar::clock::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];
    let mut seats = Vec::with_capacity(usize::from(members));
    for member in 0..members {
        let seat = Pubkey::find_program_address(
            &EnsembleFragmentSeatSeedsV1::new(
                addresses.source_state.to_bytes(),
                member,
                terminal_sequence,
            )
            .seeds(),
            &addresses.resolution_program,
        )
        .0;
        seats.push(seat);
        accounts.push(AccountMeta::new_readonly(seat, false));
    }
    for captor in captors {
        // A vacant member has no named captor. Its writable tail coordinate is
        // still required by the frame but is never credited by the fold.
        accounts.push(AccountMeta::new(
            captor.unwrap_or_else(Pubkey::new_unique),
            false,
        ));
    }
    Ok((
        Instruction {
            program_id: addresses.resolution_program,
            accounts,
            data: EnsembleFoldInstructionV1::new(addresses.generation, terminal_sequence)
                .map_err(|error| Error::new(format!("Ensemble fold request: {error:?}")))?
                .to_bytes()
                .map_err(|error| Error::new(format!("Ensemble fold bytes: {error:?}")))?
                .to_vec(),
        },
        receipt,
        seats,
    ))
}

/// Reclaim only the still-System-owned member seat after a successful fold.
fn reclaim_member_seat_instruction_v1(
    payer: Pubkey,
    addresses: &ResolutionAddressesV1,
    seat: Pubkey,
    member: u8,
    terminal_sequence: u64,
) -> Result<Instruction> {
    Ok(Instruction {
        program_id: addresses.resolution_program,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(addresses.market, false),
            AccountMeta::new_readonly(addresses.core_program, false),
            AccountMeta::new_readonly(addresses.activation_cache, false),
            AccountMeta::new_readonly(addresses.source_state, false),
            AccountMeta::new_readonly(addresses.source_material.raw, false),
            AccountMeta::new_readonly(addresses.source_material.staging, false),
            AccountMeta::new(seat, false),
            AccountMeta::new(addresses.rent_beneficiary, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ReclaimMemberSeatInstructionV1::new(addresses.generation, terminal_sequence, member)
            .map_err(|error| Error::new(format!("Ensemble reclaim request: {error:?}")))?
            .to_bytes()
            .map_err(|error| Error::new(format!("Ensemble reclaim bytes: {error:?}")))?
            .to_vec(),
    })
}

fn send_wide_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    label: &str,
    instruction: &solana_sdk::instruction::Instruction,
    signers: &[&Keypair],
    transactions: &mut Vec<crate::model::TransactionEvidence>,
) -> Result<()> {
    let (routing, tables) = crate::market::publish_routing_table(
        rpc,
        payer,
        label,
        std::slice::from_ref(instruction),
        transactions,
    )?;
    transactions.push(rpc.send_v0_with_signers(
        label,
        std::slice::from_ref(instruction),
        payer,
        signers,
        routing,
        &tables,
    )?);
    Ok(())
}

/// Arguments for the local-only three-capture Ensemble campaign.
pub(crate) struct EnsembleRequestV1 {
    pub(crate) transcript: std::path::PathBuf,
    pub(crate) work: std::path::PathBuf,
    pub(crate) rpc_port: u16,
    pub(crate) checked_release_gate: std::path::PathBuf,
    pub(crate) expected_gate_sha256: String,
    pub(crate) expected_source_revision: String,
    pub(crate) expected_source_tree_sha256: String,
    pub(crate) seed: String,
}

/// Mint a member's distinct VAA about the immutable period the market sold.
///
/// The sequence distinguishes independently authenticated member submissions;
/// it does not advance the terminal observation time after founding.
fn member_publication_request_v1(
    minted_at: i64,
    sequence: u64,
) -> crate::pyth_lab_publication::LabPublicationRequestV1 {
    crate::pyth_lab_publication::LabPublicationRequestV1::at(minted_at, sequence)
}

/// Found a canonical four-member, quorum-three Ensemble market and carry three
/// authenticated Pyth members through the terminal fold, then reclaim the
/// remaining prepaid vacancy. The transcript is only written after all three
/// state transitions exist, so it cannot claim a transport that never crossed
/// the real router/receiver boundary.
pub(crate) fn execute(request: EnsembleRequestV1) -> Result<()> {
    EnsembleSpecV1::new(4, 3)
        .map_err(|error| Error::new(format!("Ensemble input: {error:?}")))?
        .validate_foundable()
        .map_err(|error| Error::new(format!("Ensemble founding quorum: {error:?}")))?;
    std::fs::create_dir_all(&request.work)?;
    let substrate_dir = request.work.join("substrate");
    let checked = crate::substrate::bring_up(&crate::substrate::SubstrateRequestV1 {
        work: &substrate_dir,
        checked_release_gate: &request.checked_release_gate,
        expected_gate_sha256: &request.expected_gate_sha256,
        expected_source_revision: &request.expected_source_revision,
        expected_source_tree_sha256: &request.expected_source_tree_sha256,
        seed: &request.seed,
        rpc_port: request.rpc_port,
    })?;
    let mut rpc = Rpc::connect(&checked.rpc_url)?;
    let finalized_slot = rpc.finalized_slot()?;
    let minted_at = rpc.block_time(finalized_slot)?;
    let publication = crate::pyth_lab_publication::mint_lab_publication_v1(
        crate::pyth_lab_publication::LabPublicationRequestV1::at(minted_at, 1),
        [0; 32],
    )?;
    let registry = crate::plan::pubkey(&checked.plan.registry.program_id)?;
    let direct = crate::direct_market::DirectMarketCompilerOwnedV1::load_local(
        &checked.plan_path,
        &checked.rpc_url,
        registry,
        Some(50),
        Some(Keypair::new().pubkey()),
    )?;
    let shape = crate::market::LocalMarketShapeV1 {
        ensemble: Some(crate::model::EnsembleMarketInputV1 {
            members: 4,
            quorum: 3,
            rungs: 0,
        }),
        price_update_image: Some(publication.projected_price_update.clone()),
        terminal_max_age_seconds: Some(1_200),
        ..crate::market::LocalMarketShapeV1::default()
    };
    let input = crate::market::demo_market_input_shaped(registry, direct.compiler(), &shape)?;
    if input.ensemble.is_none() || input.recovery_policy_hex.is_empty() {
        return Err(Error::new(
            "the Ensemble compiler did not publish member material and policy",
        ));
    }
    let market_path = request.work.join("ensemble-market.json");
    std::fs::write(&market_path, serde_json::to_vec_pretty(&input)?)?;
    let founding = crate::substrate::found_market(
        &checked,
        &mut rpc,
        &market_path,
        &request.work.join("ensemble-founding-evidence.json"),
    )?;
    let accounts = founding.market.accounts;
    let market_addresses = crate::stages::MarketAddressesV1::from_evidence(&accounts)?;
    let addresses =
        crate::resolution::derive(&mut rpc, &checked.plan, &market_addresses, &accounts)?;
    let payer = crate::substrate::campaign_payer_keypair(&checked)?;
    let first_provider = ProviderPlanV1::derive(&mut rpc, &checked.plan)?;
    let mut transactions = founding.transactions;
    let first_report = post_submit_and_capture_member_v1(
        &mut rpc,
        &payer,
        &checked.plan,
        &addresses,
        &first_provider,
        &publication,
        0,
        1,
        &mut transactions,
    )?;
    let first_source = SourceResolutionStateV2::decode(
        &rpc.required_account(
            addresses.source_state,
            "Ensemble Source after first member capture",
        )?
        .data,
    )
    .map_err(|error| Error::new(format!("Ensemble first Source poststate: {error:?}")))?;
    if first_source.phase() != SourceResolutionPhaseV1::Primary {
        return Err(Error::new(
            "first direct member capture changed Source before an Ensemble quorum fold",
        ));
    }
    let first_fragment = first_report
        .instruction
        .accounts
        .get(3)
        .ok_or_else(|| Error::new("first direct member report omitted fragment seat"))?
        .pubkey;
    let first_fragment_account =
        rpc.required_account(first_fragment, "captured first Ensemble member fragment")?;
    if first_fragment_account.owner != addresses.resolution_program {
        return Err(Error::new(
            "first direct member capture did not create a Resolution-owned fragment",
        ));
    }
    // Every member answers the immutable terminal question compiled from the
    // first publication.  A distinct VAA sequence prevents replay, while its
    // publication time must remain ABOUT that same market period; reading the
    // current block time here would produce a fresh but late observation.
    let second_publication = crate::pyth_lab_publication::mint_lab_publication_v1(
        member_publication_request_v1(minted_at, 2),
        [0; 32],
    )?;
    let second_provider = ProviderPlanV1::derive(&mut rpc, &checked.plan)?;
    let second_report = post_submit_and_capture_member_v1(
        &mut rpc,
        &payer,
        &checked.plan,
        &addresses,
        &second_provider,
        &second_publication,
        1,
        1,
        &mut transactions,
    )?;
    let second_source = SourceResolutionStateV2::decode(
        &rpc.required_account(
            addresses.source_state,
            "Ensemble Source after second member capture",
        )?
        .data,
    )
    .map_err(|error| Error::new(format!("Ensemble second Source poststate: {error:?}")))?;
    if second_source.phase() != SourceResolutionPhaseV1::Primary {
        return Err(Error::new(
            "second direct member capture changed Source before the Ensemble fold",
        ));
    }
    let second_fragment = second_report
        .instruction
        .accounts
        .get(3)
        .ok_or_else(|| Error::new("second direct member report omitted fragment seat"))?
        .pubkey;
    let second_fragment_account =
        rpc.required_account(second_fragment, "captured second Ensemble member fragment")?;
    if second_fragment_account.owner != addresses.resolution_program {
        return Err(Error::new(
            "second direct member capture did not create a Resolution-owned fragment",
        ));
    }
    let third_publication = crate::pyth_lab_publication::mint_lab_publication_v1(
        member_publication_request_v1(minted_at, 3),
        [0; 32],
    )?;
    let third_provider = ProviderPlanV1::derive(&mut rpc, &checked.plan)?;
    let third_report = post_submit_and_capture_member_v1(
        &mut rpc,
        &payer,
        &checked.plan,
        &addresses,
        &third_provider,
        &third_publication,
        2,
        1,
        &mut transactions,
    )?;
    let third_source = SourceResolutionStateV2::decode(
        &rpc.required_account(
            addresses.source_state,
            "Ensemble Source after third member capture",
        )?
        .data,
    )
    .map_err(|error| Error::new(format!("Ensemble third Source poststate: {error:?}")))?;
    if third_source.phase() != SourceResolutionPhaseV1::Primary {
        return Err(Error::new(
            "third direct member capture changed Source before the Ensemble fold",
        ));
    }
    let third_fragment = third_report
        .instruction
        .accounts
        .get(3)
        .ok_or_else(|| Error::new("third direct member report omitted fragment seat"))?
        .pubkey;
    if rpc
        .required_account(third_fragment, "captured third Ensemble member fragment")?
        .owner
        != addresses.resolution_program
    {
        return Err(Error::new(
            "third direct member capture did not create a Resolution-owned fragment",
        ));
    }
    let (fold, receipt, seats) = ensemble_fold_instruction_v1(
        &mut rpc,
        payer.pubkey(),
        &addresses,
        &[
            Some(first_provider.resolver.pubkey()),
            Some(second_provider.resolver.pubkey()),
            Some(third_provider.resolver.pubkey()),
            None,
        ],
        1,
    )?;
    send_wide_v1(
        &mut rpc,
        &payer,
        "ensemble: fold the captured three-member quorum to one terminal",
        &fold,
        &[],
        &mut transactions,
    )?;
    let folded_source = SourceResolutionStateV2::decode(
        &rpc.required_account(addresses.source_state, "Ensemble Source after fold")?
            .data,
    )
    .map_err(|error| Error::new(format!("Ensemble folded Source poststate: {error:?}")))?;
    if folded_source.phase() != SourceResolutionPhaseV1::Resolved {
        return Err(Error::new(
            "Ensemble fold did not write the resolved Source terminal",
        ));
    }
    let receipt_value = EnsembleFoldReceiptV1::decode(
        &rpc.required_account(receipt, "Ensemble fold receipt")?.data,
    )
    .map_err(|error| Error::new(format!("Ensemble fold receipt: {error:?}")))?;
    if receipt_value.consumed_count != 3
        || !receipt_value.consumed(0)
        || !receipt_value.consumed(1)
        || !receipt_value.consumed(2)
        || receipt_value.consumed(3)
    {
        return Err(Error::new(
            "Ensemble fold receipt did not consume exactly the three captured members",
        ));
    }
    let vacant_member = 3_u8;
    let vacant_seat = *seats
        .get(usize::from(vacant_member))
        .ok_or_else(|| Error::new("Ensemble fold omitted the vacant member seat"))?;
    let vacant_before = rpc
        .required_account(vacant_seat, "prepaid vacant Ensemble member seat")?
        .lamports;
    let beneficiary_before = rpc
        .required_account(addresses.rent_beneficiary, "Ensemble rent beneficiary")?
        .lamports;
    let reclaim = reclaim_member_seat_instruction_v1(
        payer.pubkey(),
        &addresses,
        vacant_seat,
        vacant_member,
        1,
    )?;
    send_wide_v1(
        &mut rpc,
        &payer,
        "ensemble: reclaim the vacant terminal member seat",
        &reclaim,
        &[],
        &mut transactions,
    )?;
    if rpc
        .account(vacant_seat)?
        .is_some_and(|account| account.lamports != 0)
    {
        return Err(Error::new(
            "Ensemble reclaim left lamports in the vacant member seat",
        ));
    }
    let beneficiary_after = rpc
        .required_account(
            addresses.rent_beneficiary,
            "Ensemble rent beneficiary after reclaim",
        )?
        .lamports;
    if beneficiary_after.checked_sub(beneficiary_before) != Some(vacant_before) {
        return Err(Error::new(
            "Ensemble reclaim did not credit exactly the vacant member seat's rent",
        ));
    }
    let transcript = serde_json::json!({
        "campaign": "ensemble-three-member-capture-fold-reclaim-v1",
        "evidence_level": "local-validator / real router+receiver ELFs / fresh checked cohort",
        "checked_release_gate_sha256": request.expected_gate_sha256,
        "expected_source_revision": request.expected_source_revision,
        "publications": [
            {"publish_time": minted_at, "sequence": publication.request.sequence, "signed_vaa_bytes": publication.signed_vaa.len()},
            {"publish_time": second_publication.request.publish_time, "sequence": second_publication.request.sequence, "signed_vaa_bytes": second_publication.signed_vaa.len()}
            ,{"publish_time": third_publication.request.publish_time, "sequence": third_publication.request.sequence, "signed_vaa_bytes": third_publication.signed_vaa.len()}
        ],
        "captured_members": [
            {"member": 0, "fragment_seat": first_fragment.to_string()},
            {"member": 1, "fragment_seat": second_fragment.to_string()}
            ,{"member": 2, "fragment_seat": third_fragment.to_string()}
        ],
        "source_phase_after_each_capture": "Primary",
        "source_phase_after_fold": "Resolved",
        "fold_receipt": receipt.to_string(),
        "fold_consumed_bitmap": receipt_value.consumed_bitmap,
        "reclaimed_member": vacant_member,
        "reclaimed_seat": vacant_seat.to_string(),
        "transactions": transactions,
    });
    if request.transcript.exists() {
        return Err(Error::new(
            "--transcript already exists; evidence is immutable",
        ));
    }
    std::fs::write(&request.transcript, serde_json::to_vec_pretty(&transcript)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use dclutch_source::{ContentId, WindowKind, WindowSpecV1};

    #[test]
    fn member_vaas_keep_the_market_period_while_sequences_remain_distinct() {
        const MINTED_AT: i64 = 1_788_840_593;
        const LATE_BLOCK_TIME: i64 = 1_788_841_489;
        let window = WindowSpecV1::new(
            ContentId::new([7; 32]).expect("source identity"),
            WindowKind::Terminal,
            MINTED_AT - 300,
            MINTED_AT,
            1_200,
            0,
            ContentId::new([8; 32]).expect("schedule identity"),
        )
        .expect("terminal window");
        let second = super::member_publication_request_v1(MINTED_AT, 2);
        let third = super::member_publication_request_v1(MINTED_AT, 3);

        assert!(
            window
                .contains_observation(second.publish_time)
                .expect("terminal admission"),
            "the second VAA remains about the market period"
        );
        assert!(
            window
                .contains_observation(third.publish_time)
                .expect("terminal admission"),
            "the third VAA remains about the market period"
        );
        assert_ne!(second.sequence, third.sequence, "member VAAs cannot replay");
        assert!(
            !window
                .contains_observation(LATE_BLOCK_TIME)
                .expect("terminal admission"),
            "the observed old block-time construction is late and must refuse"
        );
    }
}
