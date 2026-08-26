extern crate std;

use std::{boxed::Box, vec, vec::Vec};

use dclutch_capability_program_contract::{CapabilityRootHeaderV1, v3::CapabilityProgramV3};
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::shadow_v3::{
    SHADOW_ACK_SCHEMA_ID_V3, SHADOW_REQUEST_SCHEMA_ID_V3,
};
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyAdmissionV2,
    ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{ArtifactReleaseV1, ArtifactUpgradePolicyV1};
use dclutch_registry_svm::{LOADER_V3_PROGRAM_BYTES, LOADER_V3_PROGRAMDATA_METADATA_BYTES};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CapabilityExecutionSelectionV1, ExecutionRoleV1, ProgramIdentityV1,
};
use solana_program::{
    account_info::AccountInfo, hash::hash, pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

use super::*;
use crate::dispatch::TradingFamilyContextV1;

const SLOT: u64 = 44;
const ELF: &[u8] = b"family-neutral-execution-strategy-v2-fixture";

fn id(value: u8) -> ContentId {
    ContentId::new([value; 32]).expect("content identity")
}

fn schema(value: [u8; 32]) -> ContentId {
    ContentId::new(value).expect("schema identity")
}

fn program_identity(key: Pubkey) -> ProgramIdentityV1 {
    ProgramIdentityV1::new(key.to_bytes()).expect("program identity")
}

fn account(
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    data: Vec<u8>,
    executable: bool,
) -> AccountInfo<'static> {
    AccountInfo::new(
        Box::leak(Box::new(key)),
        false,
        false,
        Box::leak(Box::new(lamports)),
        Box::leak(data.into_boxed_slice()),
        Box::leak(Box::new(owner)),
        executable,
    )
}

fn finalized_record(
    registry: Pubkey,
    schema: [u8; 32],
    data: Vec<u8>,
    rent: &Rent,
) -> [AccountInfo<'static>; 2] {
    let digest = hash(&data).to_bytes();
    let raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0;
    [
        account(raw, registry, rent.minimum_balance(data.len()), data, false),
        account(staging, system_program::ID, 0, Vec::new(), false),
    ]
}

fn loader_program_bytes(programdata: Pubkey) -> Vec<u8> {
    let mut output = vec![0_u8; LOADER_V3_PROGRAM_BYTES];
    output
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&2_u32.to_le_bytes());
    output
        .get_mut(4..36)
        .expect("programdata")
        .copy_from_slice(programdata.as_ref());
    output
}

fn loader_programdata_bytes(authority: Option<Pubkey>, slot: u64, elf: &[u8]) -> Vec<u8> {
    let mut output = vec![0_u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES + elf.len()];
    output
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    output
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&slot.to_le_bytes());
    if let Some(authority) = authority {
        *output.get_mut(12).expect("authority tag") = 1;
        output
            .get_mut(13..45)
            .expect("authority")
            .copy_from_slice(authority.as_ref());
    }
    output
        .get_mut(LOADER_V3_PROGRAMDATA_METADATA_BYTES..)
        .expect("ELF")
        .copy_from_slice(elf);
    output
}

struct Fixture {
    context: TradingFamilyContextV1,
    registry: AccountInfo<'static>,
    rent: AccountInfo<'static>,
    accounts: Vec<AccountInfo<'static>>,
    capability_program_id: ContentId,
    strategy_program_id: ContentId,
    certificate_program_id: Option<ContentId>,
    admission_program_id: Option<ContentId>,
    artifact_release_id: Option<ArtifactReleaseIdV1>,
}

fn fixture_account(fixture: &Fixture, index: usize) -> &AccountInfo<'static> {
    fixture.accounts.get(index).expect("fixture account")
}

fn fixture_account_mut(fixture: &mut Fixture, index: usize) -> &mut AccountInfo<'static> {
    fixture
        .accounts
        .get_mut(index)
        .expect("mutable fixture account")
}

impl Fixture {
    fn new(disposition: StrategyDispositionV2) -> Self {
        Self::with_options(disposition, None, id(5), true)
    }

    fn with_upgrade_authority(
        disposition: StrategyDispositionV2,
        upgrade_authority: Option<Pubkey>,
    ) -> Self {
        Self::with_options(disposition, upgrade_authority, id(5), true)
    }

    fn with_options(
        disposition: StrategyDispositionV2,
        upgrade_authority: Option<Pubkey>,
        certificate_account_profile: ContentId,
        admission_matches_certificate: bool,
    ) -> Self {
        let rent_value = Rent::default();
        let registry_key = Pubkey::new_from_array([201; 32]);
        let registry = account(registry_key, native_loader::ID, 1, Vec::new(), true);
        let mut rent = account(
            sysvar::rent::ID,
            sysvar::ID,
            1,
            vec![0; Rent::size_of()],
            false,
        );
        assert_eq!(rent_value.to_account_info(&mut rent), Some(()));

        let accelerator_program = Pubkey::new_from_array([202; 32]);
        let accelerator_programdata = Pubkey::find_program_address(
            &[accelerator_program.as_ref()],
            &bpf_loader_upgradeable::ID,
        )
        .0;
        let programdata_bytes = loader_programdata_bytes(upgrade_authority, SLOT, ELF);
        let upgrade_policy = if upgrade_authority.is_some() {
            ArtifactUpgradePolicyV1::ExactAuthority
        } else {
            ArtifactUpgradePolicyV1::Immutable
        };
        let release = ArtifactReleaseV1::new(
            program_identity(accelerator_program),
            program_identity(bpf_loader_upgradeable::ID),
            accelerator_programdata.to_bytes(),
            id(20),
            hash(ELF).to_bytes(),
            SLOT,
            upgrade_policy,
            upgrade_authority.map(|value| value.to_bytes()),
        )
        .expect("artifact release");
        let release_bytes = release.to_bytes();
        let artifact_release_id =
            ArtifactReleaseIdV1::new(hash(&release_bytes).to_bytes()).expect("artifact release ID");

        let certificate = ExecutionStrategyCertificateV2::new(
            certificate_account_profile,
            id(9),
            id(10),
            id(11),
            id(12),
            id(8),
            artifact_release_id,
            id(21),
            id(22),
            id(23),
        );
        let certificate_bytes = certificate.to_bytes();
        let certificate_program_id =
            ContentId::new(hash(&certificate_bytes).to_bytes()).expect("certificate ID");
        let admission = ExecutionStrategyAdmissionV2::new(if admission_matches_certificate {
            certificate_program_id
        } else {
            id(207)
        });
        let admission_bytes = admission.to_bytes();
        let admission_program_id =
            ContentId::new(hash(&admission_bytes).to_bytes()).expect("admission ID");

        let (certificate_selection, admission_selection) = match disposition {
            StrategyDispositionV2::Interpreted => (None, None),
            StrategyDispositionV2::ShadowAot => (Some(certificate_program_id), None),
            StrategyDispositionV2::AdmittedAot => {
                (Some(certificate_program_id), Some(admission_program_id))
            }
        };
        let (request_schema, ack_schema) = match disposition {
            StrategyDispositionV2::ShadowAot => (
                schema(SHADOW_REQUEST_SCHEMA_ID_V3),
                schema(SHADOW_ACK_SCHEMA_ID_V3),
            ),
            StrategyDispositionV2::Interpreted | StrategyDispositionV2::AdmittedAot => (
                schema(ACCELERATOR_REQUEST_SCHEMA_ID_V2),
                schema(ACCELERATOR_ACK_SCHEMA_ID_V2),
            ),
        };
        let strategy = ExecutionStrategyProgramV2::new(
            disposition,
            id(11),
            id(12),
            schema(EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2),
            certificate_selection,
            schema(EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2),
            admission_selection,
            request_schema,
            ack_schema,
        )
        .expect("strategy");
        let strategy_bytes = strategy.to_bytes();
        let strategy_program_id =
            ContentId::new(hash(&strategy_bytes).to_bytes()).expect("strategy ID");
        let capability_program = CapabilityProgramV3::new(
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            id(9),
            id(10),
            schema(EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2),
            strategy_program_id,
            64,
        )
        .expect("Capability Program");
        let capability_bytes = capability_program.encode();
        let capability_program_id =
            ContentId::new(hash(&capability_bytes).to_bytes()).expect("Capability Program ID");

        let capability_record = finalized_record(
            registry_key,
            CAPABILITY_PROGRAM_SCHEMA_ID_V3,
            capability_bytes.to_vec(),
            &rent_value,
        );
        let strategy_record = finalized_record(
            registry_key,
            EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
            strategy_bytes.to_vec(),
            &rent_value,
        );
        let certificate_record = finalized_record(
            registry_key,
            EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
            certificate_bytes.to_vec(),
            &rent_value,
        );
        let admission_record = finalized_record(
            registry_key,
            EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2,
            admission_bytes.to_vec(),
            &rent_value,
        );
        let artifact_record = finalized_record(
            registry_key,
            ARTIFACT_RELEASE_SCHEMA_ID_V1,
            release_bytes.to_vec(),
            &rent_value,
        );
        let program = account(
            accelerator_program,
            bpf_loader_upgradeable::ID,
            1,
            loader_program_bytes(accelerator_programdata),
            true,
        );
        let programdata = account(
            accelerator_programdata,
            bpf_loader_upgradeable::ID,
            1,
            programdata_bytes,
            false,
        );

        let mut accounts = Vec::new();
        accounts.extend(capability_record);
        accounts.extend(strategy_record);
        match disposition {
            StrategyDispositionV2::Interpreted => {}
            StrategyDispositionV2::ShadowAot => {
                accounts.extend(certificate_record);
                accounts.extend(artifact_record);
                accounts.extend([program, programdata]);
            }
            StrategyDispositionV2::AdmittedAot => {
                accounts.extend(certificate_record);
                accounts.extend(admission_record);
                accounts.extend(artifact_record);
                accounts.extend([program, programdata]);
            }
        }

        let selection = CapabilityExecutionSelectionV1::new(0, id(30), id(1), id(36), id(31))
            .expect("ProgramSet selection");
        let root = CapabilityRootHeaderV1::new(id(32), [33; 32], 34, selection).expect("root");
        let trading_program = Pubkey::new_from_array([203; 32]);
        let child_root =
            Pubkey::find_program_address(&root.seeds().as_slices(), &trading_program).0;
        let receipt = dclutch_registry_svm::AuthenticatedRoleReceiptV1::new(
            ExecutionRoleV1::Trading,
            id(32),
            program_identity(trading_program),
            ArtifactReleaseIdV1::new([204; 32]).expect("Trading artifact"),
            id(35),
        );
        let context = TradingFamilyContextV1::authenticate_activation(
            &trading_program,
            &child_root,
            root,
            capability_program
                .root_account_bytes()
                .expect("root account bytes"),
            receipt,
        )
        .expect("authenticated context");

        Self {
            context,
            registry,
            rent,
            accounts,
            capability_program_id,
            strategy_program_id,
            certificate_program_id: certificate_selection,
            admission_program_id: admission_selection,
            artifact_release_id: certificate_selection.map(|_| artifact_release_id),
        }
    }

    fn authenticate(&self) -> Result<AuthenticatedExecutionStrategyV2, TradingSbfError> {
        authenticate_execution_strategy_v2(
            self.context,
            self.capability_program_id,
            &self.registry,
            &self.rent,
            &self.accounts,
        )
    }
}

#[test]
fn interpreted_uses_exact_unpadded_record_frame() {
    let fixture = Fixture::new(StrategyDispositionV2::Interpreted);
    let authenticated = fixture.authenticate().expect("interpreted strategy");
    assert_ne!(
        fixture.context.selection().capability_release(),
        fixture.capability_program_id,
        "the root selects a ProgramSet, while the action selects this descriptor"
    );
    assert_eq!(
        fixture.accounts.len(),
        INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2
    );
    assert_eq!(
        authenticated.capability_program_id(),
        fixture.capability_program_id
    );
    assert_eq!(
        authenticated.strategy_program_id(),
        fixture.strategy_program_id
    );
    assert_eq!(
        authenticated.strategy().disposition(),
        StrategyDispositionV2::Interpreted
    );
    assert_eq!(authenticated.certificate_program_id(), None);
    assert_eq!(authenticated.admission_program_id(), None);
    assert_eq!(authenticated.artifact_release_id(), None);
    assert_eq!(authenticated.admitted_authorization(), None);

    let mut padded = fixture.accounts.clone();
    padded.push(account(
        Pubkey::new_from_array([205; 32]),
        system_program::ID,
        0,
        Vec::new(),
        false,
    ));
    assert_eq!(
        authenticate_execution_strategy_v2(
            fixture.context,
            fixture.capability_program_id,
            &fixture.registry,
            &fixture.rent,
            &padded,
        ),
        Err(TradingSbfError::Content)
    );
}

#[test]
fn shadow_authenticates_exact_certificate_artifact_and_current_elf() {
    let fixture = Fixture::new(StrategyDispositionV2::ShadowAot);
    let authenticated = fixture.authenticate().expect("shadow AOT strategy");
    assert_eq!(fixture.accounts.len(), SHADOW_AOT_STRATEGY_ACCOUNT_COUNT_V2);
    assert_eq!(
        authenticated.strategy().disposition(),
        StrategyDispositionV2::ShadowAot
    );
    assert_eq!(
        authenticated.certificate_program_id(),
        fixture.certificate_program_id
    );
    assert_eq!(authenticated.admission_program_id(), None);
    assert_eq!(
        authenticated.artifact_release_id(),
        fixture.artifact_release_id
    );
    assert_eq!(
        authenticated
            .artifact_release()
            .expect("artifact")
            .upgrade_policy(),
        ArtifactUpgradePolicyV1::Immutable
    );
    assert_eq!(authenticated.admitted_authorization(), None);
}

#[test]
fn admitted_requires_the_exact_registry_admission_chain() {
    let fixture = Fixture::new(StrategyDispositionV2::AdmittedAot);
    let authenticated = fixture.authenticate().expect("admitted AOT strategy");
    assert_eq!(
        fixture.accounts.len(),
        ADMITTED_AOT_STRATEGY_ACCOUNT_COUNT_V2
    );
    assert_eq!(
        authenticated.strategy().disposition(),
        StrategyDispositionV2::AdmittedAot
    );
    assert_eq!(
        authenticated.certificate_program_id(),
        fixture.certificate_program_id
    );
    assert_eq!(
        authenticated.admission_program_id(),
        fixture.admission_program_id
    );
    assert!(authenticated.admitted_authorization().is_some());

    let missing_admission = fixture
        .accounts
        .get(..SHADOW_AOT_STRATEGY_ACCOUNT_COUNT_V2)
        .expect("short admitted frame")
        .to_vec();
    assert_eq!(
        authenticate_execution_strategy_v2(
            fixture.context,
            fixture.capability_program_id,
            &fixture.registry,
            &fixture.rent,
            &missing_admission,
        ),
        Err(TradingSbfError::Content)
    );
}

#[test]
fn hostile_record_owner_digest_staging_alias_and_selection_refuse() {
    let mut owner = Fixture::new(StrategyDispositionV2::ShadowAot);
    fixture_account_mut(&mut owner, STRATEGY_RAW).owner = Box::leak(Box::new(system_program::ID));
    assert_eq!(owner.authenticate(), Err(TradingSbfError::Content));

    let digest = Fixture::new(StrategyDispositionV2::ShadowAot);
    fixture_account(&digest, STRATEGY_RAW)
        .try_borrow_mut_data()
        .expect("strategy data")
        .get_mut(32)
        .map(|byte| *byte ^= 1)
        .expect("strategy byte");
    assert_eq!(digest.authenticate(), Err(TradingSbfError::Content));

    let mut staging = Fixture::new(StrategyDispositionV2::ShadowAot);
    let staging_key = *fixture_account(&staging, STRATEGY_STAGING).key;
    *fixture_account_mut(&mut staging, STRATEGY_STAGING) =
        account(staging_key, system_program::ID, 1, vec![1], false);
    assert_eq!(staging.authenticate(), Err(TradingSbfError::Content));

    let mut alias = Fixture::new(StrategyDispositionV2::ShadowAot);
    let raw_alias = fixture_account(&alias, STRATEGY_RAW).clone();
    *fixture_account_mut(&mut alias, STRATEGY_STAGING) = raw_alias;
    assert_eq!(alias.authenticate(), Err(TradingSbfError::Content));

    let selection = Fixture::new(StrategyDispositionV2::ShadowAot);
    fixture_account(&selection, CAPABILITY_RAW)
        .try_borrow_mut_data()
        .expect("Capability data")
        .get_mut(64)
        .map(|byte| *byte ^= 1)
        .expect("Capability byte");
    assert_eq!(selection.authenticate(), Err(TradingSbfError::Content));
}

#[test]
fn hostile_certificate_admission_and_artifact_substitution_refuse() {
    let certificate = Fixture::new(StrategyDispositionV2::ShadowAot);
    fixture_account(&certificate, CERTIFICATE_RAW)
        .try_borrow_mut_data()
        .expect("certificate")
        .get_mut(48)
        .map(|byte| *byte ^= 1)
        .expect("certificate byte");
    assert_eq!(certificate.authenticate(), Err(TradingSbfError::Content));

    let admission = Fixture::new(StrategyDispositionV2::AdmittedAot);
    fixture_account(&admission, ADMITTED_ADMISSION_RAW)
        .try_borrow_mut_data()
        .expect("admission")
        .get_mut(16)
        .map(|byte| *byte ^= 1)
        .expect("admission byte");
    assert_eq!(admission.authenticate(), Err(TradingSbfError::Content));

    let artifact = Fixture::new(StrategyDispositionV2::ShadowAot);
    fixture_account(&artifact, SHADOW_ARTIFACT_RAW)
        .try_borrow_mut_data()
        .expect("artifact")
        .get_mut(144)
        .map(|byte| *byte ^= 1)
        .expect("artifact byte");
    assert_eq!(artifact.authenticate(), Err(TradingSbfError::Content));

    let semantic_certificate =
        Fixture::with_options(StrategyDispositionV2::ShadowAot, None, id(208), true);
    assert_eq!(
        semantic_certificate.authenticate(),
        Err(TradingSbfError::Content),
        "a valid finalized Certificate for another AccountProfile must not join"
    );

    let semantic_admission =
        Fixture::with_options(StrategyDispositionV2::AdmittedAot, None, id(5), false);
    assert_eq!(
        semantic_admission.authenticate(),
        Err(TradingSbfError::Content),
        "a valid finalized Admission for another Certificate must not authorize AOT"
    );
}

#[test]
fn current_loader_slot_elf_owner_link_and_immutability_are_mandatory() {
    let elf = Fixture::new(StrategyDispositionV2::ShadowAot);
    fixture_account(&elf, SHADOW_ACCELERATOR_PROGRAMDATA)
        .try_borrow_mut_data()
        .expect("ProgramData")
        .last_mut()
        .map(|byte| *byte ^= 1)
        .expect("ELF byte");
    assert_eq!(elf.authenticate(), Err(TradingSbfError::Content));

    let slot = Fixture::new(StrategyDispositionV2::ShadowAot);
    fixture_account(&slot, SHADOW_ACCELERATOR_PROGRAMDATA)
        .try_borrow_mut_data()
        .expect("ProgramData")
        .get_mut(4)
        .map(|byte| *byte ^= 1)
        .expect("slot byte");
    assert_eq!(slot.authenticate(), Err(TradingSbfError::Content));

    let mut owner = Fixture::new(StrategyDispositionV2::ShadowAot);
    fixture_account_mut(&mut owner, SHADOW_ACCELERATOR_PROGRAM).owner =
        Box::leak(Box::new(system_program::ID));
    assert_eq!(owner.authenticate(), Err(TradingSbfError::Content));

    let link = Fixture::new(StrategyDispositionV2::ShadowAot);
    fixture_account(&link, SHADOW_ACCELERATOR_PROGRAM)
        .try_borrow_mut_data()
        .expect("Program")
        .get_mut(4)
        .map(|byte| *byte ^= 1)
        .expect("ProgramData link byte");
    assert_eq!(link.authenticate(), Err(TradingSbfError::Content));

    let upgradeable = Fixture::with_upgrade_authority(
        StrategyDispositionV2::ShadowAot,
        Some(Pubkey::new_from_array([206; 32])),
    );
    assert_eq!(
        upgradeable.authenticate(),
        Err(TradingSbfError::Content),
        "a current exact-authority deployment is valid Loader state but is not immutable AOT"
    );
}

#[test]
fn registry_rent_privileges_and_account_width_are_not_caller_trust() {
    let mut registry = Fixture::new(StrategyDispositionV2::Interpreted);
    registry.registry.is_writable = true;
    assert_eq!(registry.authenticate(), Err(TradingSbfError::Content));

    let mut rent = Fixture::new(StrategyDispositionV2::Interpreted);
    rent.rent.owner = Box::leak(Box::new(system_program::ID));
    assert_eq!(rent.authenticate(), Err(TradingSbfError::Content));

    let short = Fixture::new(StrategyDispositionV2::Interpreted);
    assert_eq!(
        authenticate_execution_strategy_v2(
            short.context,
            short.capability_program_id,
            &short.registry,
            &short.rent,
            short.accounts.get(..3).expect("short frame"),
        ),
        Err(TradingSbfError::Content)
    );
}
