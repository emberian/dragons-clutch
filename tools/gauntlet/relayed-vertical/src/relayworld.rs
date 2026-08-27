//! The devnet-side relay routes: record creation, consumption, and the funded
//! deadline walk, built against the live successor validator.
//!
//! Account frames come from `dclutch_relay_contract::frame`, never from a
//! table in this file, so the campaign cannot drift from the frame the
//! program validates. The consumption rides a v0 message over the Market's
//! address lookup table — it is one of the family's two known over-packet
//! wires — and the deadline walk is deliberately kept on a bare legacy
//! message, because it is the route that must work when nobody cooperating
//! ever published a table.

use solana_program::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::transfer;

use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_relay_contract::{
    RELAYED_RECORD_PDA_DOMAIN_V1,
    frame::{RelayAccountNameV1, RelayFrameKindV1, relay_frame_roles_v1},
    instruction::{
        CommitDeadlineFailureInstructionV1, ConsumeRecordInstructionV1, CreateRecordInstructionV1,
    },
    release::AccountSetEntryV1,
};
use dclutch_resolution_codec::{
    RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    ResolutionCertificateKindV2, ResolutionCertificateV2,
};
use dclutch_source_contract::{SourceResolutionPhaseV1, SourceResolutionStateV2};
use dclutch_versioned_message_operator::build_lookup_table_creation_v1;

use crate::model::TransactionEvidence;
use crate::rpc::Rpc;
use crate::{Error, Result};
use dclutch_resolution_core_v3_operator::{Observation, ObservedAccount};

/// Lean-owned Runtime V2 certificate wire tags, used as PDA seeds.
pub(crate) const RESOLUTION_SUCCESS_KIND: u8 = 1;
pub(crate) const RESOLUTION_FAILURE_KIND: u8 = 4;
pub(crate) const TERMINAL_SEQUENCE: u64 = 1;

/// One finalized record's raw/staging coordinates, derived from its identity.
#[derive(Clone, Copy)]
pub(crate) struct RecordPairV1 {
    pub(crate) raw: Pubkey,
    pub(crate) staging: Pubkey,
}

impl RecordPairV1 {
    pub(crate) fn derive(registry: Pubkey, schema: [u8; 32], digest: [u8; 32]) -> Self {
        Self {
            raw: Pubkey::find_program_address(
                &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
                &registry,
            )
            .0,
            staging: Pubkey::find_program_address(
                &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
                &registry,
            )
            .0,
        }
    }
}

/// Every address a relay frame can name, resolved once.
pub(crate) struct RelayAddressBookV1 {
    pub(crate) worker: Pubkey,
    pub(crate) market: Pubkey,
    pub(crate) core_program: Pubkey,
    pub(crate) activation: Pubkey,
    pub(crate) resolution_program: Pubkey,
    pub(crate) record: Pubkey,
    pub(crate) record_bump: u8,
    pub(crate) material: RecordPairV1,
    pub(crate) spec: RecordPairV1,
    pub(crate) provider: RecordPairV1,
    pub(crate) window: RecordPairV1,
    pub(crate) key_set: RecordPairV1,
    pub(crate) config: RecordPairV1,
    pub(crate) venue: RecordPairV1,
    pub(crate) product: RecordPairV1,
    pub(crate) result_domain: RecordPairV1,
    pub(crate) portfolio: RecordPairV1,
    pub(crate) manifest: RecordPairV1,
    pub(crate) rent_beneficiary: Pubkey,
    pub(crate) source_state: Pubkey,
    pub(crate) failure_funding: Pubkey,
}

impl RelayAddressBookV1 {
    /// The record PDA for one observed slot; the equivocation bound is that
    /// this address is a function of the slot.
    pub(crate) fn record_for_slot(
        resolution_program: Pubkey,
        market: Pubkey,
        generation: u64,
        account_set_id: [u8; 32],
        observed_slot: u64,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                RELAYED_RECORD_PDA_DOMAIN_V1,
                market.as_ref(),
                &generation.to_le_bytes(),
                &account_set_id,
                &observed_slot.to_le_bytes(),
            ],
            &resolution_program,
        )
    }

    pub(crate) fn certificate_of(&self, kind: u8) -> Pubkey {
        Pubkey::find_program_address(
            &[
                RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
                self.source_state.as_ref(),
                &[kind],
                &TERMINAL_SEQUENCE.to_le_bytes(),
            ],
            &self.resolution_program,
        )
        .0
    }

    fn address_of(&self, name: RelayAccountNameV1) -> Result<Pubkey> {
        Ok(match name {
            RelayAccountNameV1::Worker => self.worker,
            RelayAccountNameV1::Market => self.market,
            RelayAccountNameV1::CoreProgram => self.core_program,
            RelayAccountNameV1::RegistryActivation => self.activation,
            RelayAccountNameV1::Record => self.record,
            RelayAccountNameV1::SourceMaterial => self.material.raw,
            RelayAccountNameV1::SourceMaterialStagingVacancy => self.material.staging,
            RelayAccountNameV1::SourceSpec => self.spec.raw,
            RelayAccountNameV1::SourceSpecStagingVacancy => self.spec.staging,
            RelayAccountNameV1::ProviderRelease => self.provider.raw,
            RelayAccountNameV1::ProviderReleaseStagingVacancy => self.provider.staging,
            RelayAccountNameV1::WindowSpec => self.window.raw,
            RelayAccountNameV1::WindowSpecStagingVacancy => self.window.staging,
            RelayAccountNameV1::RelayerKeySet => self.key_set.raw,
            RelayAccountNameV1::RelayerKeySetStagingVacancy => self.key_set.staging,
            RelayAccountNameV1::AdapterConfig => self.config.raw,
            RelayAccountNameV1::AdapterConfigStagingVacancy => self.config.staging,
            RelayAccountNameV1::RentBeneficiary => self.rent_beneficiary,
            RelayAccountNameV1::RentSysvar => sysvar::rent::ID,
            RelayAccountNameV1::ClockSysvar => sysvar::clock::ID,
            RelayAccountNameV1::InstructionsSysvar => sysvar::instructions::ID,
            RelayAccountNameV1::SystemProgram => system_program::ID,
            RelayAccountNameV1::SourceResolutionState => self.source_state,
            RelayAccountNameV1::ResolutionCertificate => {
                return Err(Error::new(
                    "the certificate position depends on the terminal kind; the caller supplies it",
                ));
            }
            RelayAccountNameV1::VenueArtifactRelease => self.venue.raw,
            RelayAccountNameV1::VenueArtifactReleaseStagingVacancy => self.venue.staging,
            RelayAccountNameV1::ProductRecord => self.product.raw,
            RelayAccountNameV1::ProductRecordStagingVacancy => self.product.staging,
            RelayAccountNameV1::ResultDomain => self.result_domain.raw,
            RelayAccountNameV1::ResultDomainStagingVacancy => self.result_domain.staging,
            RelayAccountNameV1::PortfolioRecord => self.portfolio.raw,
            RelayAccountNameV1::PortfolioRecordStagingVacancy => self.portfolio.staging,
            RelayAccountNameV1::CapabilityManifest => self.manifest.raw,
            RelayAccountNameV1::CapabilityManifestStagingVacancy => self.manifest.staging,
            RelayAccountNameV1::FailureFunding => self.failure_funding,
        })
    }

    /// Build one relay frame's exact ordered metas from the contract's roles.
    pub(crate) fn frame_metas(
        &self,
        kind: RelayFrameKindV1,
        certificate: Option<Pubkey>,
    ) -> Result<Vec<AccountMeta>> {
        let roles = relay_frame_roles_v1(kind);
        let mut metas = Vec::with_capacity(roles.len());
        for role in roles {
            let key = match role.name() {
                RelayAccountNameV1::ResolutionCertificate => certificate.ok_or_else(|| {
                    Error::new("this frame names a certificate and none was supplied")
                })?,
                other => self.address_of(other)?,
            };
            metas.push(AccountMeta {
                pubkey: key,
                is_signer: role.is_signer(),
                is_writable: role.is_writable(),
            });
        }
        Ok(metas)
    }
}

/// Build the 21-account record-creation instruction.
pub(crate) fn create_record_instruction(
    book: &RelayAddressBookV1,
    generation: u64,
    observed_slot: u64,
    set_count: u16,
    material_id: [u8; 32],
    spec_id: [u8; 32],
) -> Result<Instruction> {
    let request = CreateRecordInstructionV1::new(
        generation,
        observed_slot,
        set_count,
        1,
        book.record_bump,
        material_id,
        spec_id,
        book.rent_beneficiary.to_bytes(),
    )
    .map_err(|error| Error::new(format!("create-record request: {error:?}")))?;
    Ok(Instruction {
        program_id: book.resolution_program,
        accounts: book.frame_metas(RelayFrameKindV1::CreateRecord, None)?,
        data: request
            .to_bytes()
            .map_err(|error| Error::new(format!("create-record bytes: {error:?}")))?
            .to_vec(),
    })
}

/// Build the 28-account consumption, carrying the pinned entries inline.
pub(crate) fn consume_record_instruction(
    book: &RelayAddressBookV1,
    generation: u64,
    observed_slot: u64,
    material_id: [u8; 32],
    spec_id: [u8; 32],
    entries: &[AccountSetEntryV1],
) -> Result<Instruction> {
    let mut data = ConsumeRecordInstructionV1::new(
        generation,
        observed_slot,
        TERMINAL_SEQUENCE,
        material_id,
        spec_id,
        u16::try_from(entries.len()).map_err(|_| Error::new("entry count overflow"))?,
    )
    .map_err(|error| Error::new(format!("consume request: {error:?}")))?
    .to_prefix_bytes()
    .map_err(|error| Error::new(format!("consume prefix: {error:?}")))?
    .to_vec();
    for entry in entries {
        data.extend_from_slice(&entry.key);
        data.extend_from_slice(&entry.expected_owner);
        data.extend_from_slice(&entry.inline_len.to_le_bytes());
    }
    Ok(Instruction {
        program_id: book.resolution_program,
        accounts: book.frame_metas(
            RelayFrameKindV1::ConsumeRecord,
            Some(book.certificate_of(RESOLUTION_SUCCESS_KIND)),
        )?,
        data,
    })
}

/// Build the 22-account funded deadline walk.
pub(crate) fn deadline_failure_instruction(
    book: &RelayAddressBookV1,
    generation: u64,
) -> Result<Instruction> {
    Ok(Instruction {
        program_id: book.resolution_program,
        accounts: book.frame_metas(
            RelayFrameKindV1::CommitDeadlineFailure,
            Some(book.certificate_of(RESOLUTION_FAILURE_KIND)),
        )?,
        data: CommitDeadlineFailureInstructionV1::new(generation, TERMINAL_SEQUENCE)
            .map_err(|error| Error::new(format!("deadline-failure request: {error:?}")))?
            .to_bytes()
            .map_err(|error| Error::new(format!("deadline-failure bytes: {error:?}")))?
            .to_vec(),
    })
}

/// Prepay one certificate destination: the route allocates and assigns, so the
/// account arrives System-owned holding exactly its rent.
pub(crate) fn prepay_certificate(
    rpc: &mut Rpc,
    payer: &Keypair,
    certificate: Pubkey,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<()> {
    let target = rpc.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2)?;
    let held = rpc
        .account(certificate)?
        .map(|account| account.lamports)
        .unwrap_or(0);
    let top_up = target.saturating_sub(held);
    if top_up > 0 {
        transactions.push(rpc.send(
            "relayed vertical: prepay a terminal certificate destination",
            &[transfer(&payer.pubkey(), &certificate, top_up)],
            payer,
        )?);
    }
    Ok(())
}

/// Publish a finalized routing table over one frame's non-signer coordinates.
///
/// Mirrors the producer's `publish_routing_table` (market.rs): only non-signer
/// coordinates and the invoked Program are routed, the table stays
/// authority-owned, and it is usable strictly after the slot that extended it.
pub(crate) fn publish_routing_table(
    rpc: &mut Rpc,
    payer: &Keypair,
    label: &str,
    instructions: &[Instruction],
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(Pubkey, Vec<Pubkey>, Observation, Vec<ObservedAccount>)> {
    let mut addresses: Vec<Pubkey> = Vec::new();
    let push = |key: Pubkey, addresses: &mut Vec<Pubkey>| {
        if key != payer.pubkey() && !addresses.contains(&key) {
            addresses.push(key);
        }
    };
    for instruction in instructions {
        push(instruction.program_id, &mut addresses);
        for meta in &instruction.accounts {
            if !meta.is_signer {
                push(meta.pubkey, &mut addresses);
            }
        }
    }
    let recent_slot = rpc.finalized_slot()?;
    let plan =
        build_lookup_table_creation_v1(payer.pubkey(), payer.pubkey(), recent_slot, &addresses)
            .map_err(|error| Error::new(format!("{label} routing table plan: {error:?}")))?;
    transactions.push(rpc.send(
        &format!("relayed vertical: create the {label} routing table"),
        std::slice::from_ref(&plan.create),
        payer,
    )?);
    for (index, extension) in plan.extensions.iter().enumerate() {
        transactions.push(rpc.send(
            &format!("relayed vertical: extend the {label} routing table page {index}"),
            std::slice::from_ref(extension),
            payer,
        )?);
    }
    let extended_slot = transactions
        .last()
        .map(|transaction| transaction.slot)
        .ok_or_else(|| Error::new("routing table publication recorded no slot"))?;
    let minimum_slot = extended_slot
        .checked_add(1)
        .ok_or_else(|| Error::new("routing table slot overflow"))?;
    await_finalized_slot(rpc, minimum_slot)?;
    let (observation, tables) =
        rpc.finalized_observed_accounts(&[plan.lookup_table], minimum_slot)?;
    Ok((plan.lookup_table, addresses, observation, tables))
}

pub(crate) fn await_finalized_slot(rpc: &mut Rpc, minimum_slot: u64) -> Result<()> {
    for _ in 0..600 {
        if rpc.finalized_slot()? >= minimum_slot {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err(Error::new(
        "the validator did not finalize a slot after the routing table extension",
    ))
}

/// The extent one signed legacy transaction would occupy on the wire.
pub(crate) fn legacy_wire_extent(instruction: &Instruction, payer: Pubkey) -> usize {
    let message =
        solana_sdk::message::Message::new(std::slice::from_ref(instruction), Some(&payer));
    let serialized = message.serialize();
    1 + message.header.num_required_signatures as usize * 64 + serialized.len()
}

/// Decode and check the terminal certificate the walk or the consumption left.
pub(crate) fn read_certificate(
    rpc: &mut Rpc,
    certificate: Pubkey,
    expected_kind: ResolutionCertificateKindV2,
) -> Result<ResolutionCertificateV2> {
    let account = rpc.required_account(certificate, "terminal certificate")?;
    let decoded = ResolutionCertificateV2::decode(&account.data)
        .map_err(|error| Error::new(format!("ResolutionCertificateV2: {error:?}")))?;
    if decoded.kind != expected_kind {
        return Err(Error::new(format!(
            "the terminal certificate is {:?}, expected {:?}",
            decoded.kind, expected_kind
        )));
    }
    Ok(decoded)
}

/// Decode the Source resolution state and require one exact phase.
pub(crate) fn require_source_phase(
    rpc: &mut Rpc,
    source_state: Pubkey,
    expected: SourceResolutionPhaseV1,
) -> Result<SourceResolutionStateV2> {
    let account = rpc.required_account(source_state, "Source resolution state")?;
    let decoded = SourceResolutionStateV2::decode(&account.data)
        .map_err(|error| Error::new(format!("SourceResolutionStateV2: {error:?}")))?;
    if decoded.phase() != expected {
        return Err(Error::new(format!(
            "the Source resolution state is {:?}, expected {expected:?}",
            decoded.phase()
        )));
    }
    Ok(decoded)
}

