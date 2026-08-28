//! Chain-derived CPI for a Resolution-owned subset ledger before Market creation.

use dclutch_capability_contract::{
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, ContentId as CapabilityContentId,
    FundingLedgerV2, funding_ledger_bytes_v2,
};
use dclutch_market_core_codec::ProjectFoundRequestV2;
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_resolution_codec::{
    PreMarketFundingReceiptV1, PreMarketFundingRequestV1, RESOLUTION_CONTROLLER_RELEASE_ID_V5,
    pre_market_funding_prestate_digest_v1,
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::{
    Finality, ObservedAccount, ResolutionCoreOperatorErrorV3, decode_rent, deployment_observation,
};

/// Exact Core ProjectFound frame width consumed by the initializer CPI.
pub const PRE_MARKET_PROJECT_FOUND_ACCOUNT_COUNT_V1: usize = 37;
/// Exact complete pre-Market initializer CPI frame width.
pub const PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1: usize =
    7 + PRE_MARKET_PROJECT_FOUND_ACCOUNT_COUNT_V1;

const FOUND_MANIFEST_RAW: usize = 22;
const FOUND_RENT_CREDIT: usize = 2;
const FOUND_ACTIVATION_CACHE: usize = 24;
const FOUND_CORE_PROGRAM: usize = 25;
const FOUND_REGISTRY_PROGRAM: usize = 27;
const FOUND_RENT: usize = 28;
const FOUND_SYSTEM: usize = 29;

/// Same-finalized inputs for one pre-Market Resolution ledger CPI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreMarketFundingSnapshotV1 {
    /// Current Resolution program receiving the CPI.
    pub resolution_program: ObservedAccount,
    /// Release-selected Trading caller program.
    pub caller_program: ObservedAccount,
    /// Current Loader V3 ProgramData for the Trading caller.
    pub caller_programdata: ObservedAccount,
    /// Current Loader V3 ProgramData for the Resolution callee.
    pub resolution_programdata: ObservedAccount,
    /// Signer paying exact ledger Rent and native principal.
    pub funding_source: ObservedAccount,
    /// Vacant canonical Resolution ledger PDA.
    pub ledger: ObservedAccount,
    /// Exact read-only Core ProjectFound frame in Core ABI order.
    pub project_found_accounts: Vec<ObservedAccount>,
}

/// Exact initializer CPI, target funding, and expected return receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreMarketFundingReportV1 {
    /// Instruction the Trading caller invokes with its PDA signer.
    pub instruction: Instruction,
    /// Canonical caller-authority PDA Trading must sign.
    pub caller_authority: Pubkey,
    /// Exact lamports debited from `funding_source`.
    pub exact_funding_lamports: u64,
    /// Exact initialized ledger Rent reserve.
    pub exact_rent_lamports: u64,
    /// Exact aggregate native principal.
    pub exact_native_principal: u64,
    /// Canonical three-row Resolution mask.
    pub selected_mask: u16,
    /// Exact expected return-data receipt.
    pub expected_receipt: PreMarketFundingReceiptV1,
}

/// Build one release-authenticated pre-Market Resolution ledger CPI.
pub fn build_pre_market_funding_v1(
    snapshot: &PreMarketFundingSnapshotV1,
    project_found: ProjectFoundRequestV2,
) -> Result<PreMarketFundingReportV1, ResolutionCoreOperatorErrorV3> {
    authenticate_snapshot(snapshot)?;
    let found = &snapshot.project_found_accounts;
    let manifest_account = found
        .get(FOUND_MANIFEST_RAW)
        .ok_or(ResolutionCoreOperatorErrorV3::Frame)?;
    let manifest = CapabilityManifestV1::decode(&manifest_account.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let manifest_digest = hash(&manifest_account.data).to_bytes();
    let manifest_id = CapabilityContentId::new(manifest_digest)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let selected_mask = resolution_mask(manifest)?;
    let width = funding_ledger_bytes_v2(3).map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let mut ledger_bytes = vec![0_u8; width];
    FundingLedgerV2::initialize(&mut ledger_bytes, manifest_id, manifest, selected_mask)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let authenticated = FundingLedgerV2::decode(&ledger_bytes)
        .and_then(|ledger| ledger.authenticate(manifest_id, manifest))
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let exact_native_principal = authenticated
        .remaining_native_lamports_total()
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    for entry_index in 0_u16..manifest.entry_count() {
        if selected_mask & (1_u16 << entry_index) != 0
            && manifest
                .entry(entry_index)
                .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?
                .funding_quote()
                .realm_collateral()
                .is_some()
        {
            return Err(ResolutionCoreOperatorErrorV3::Funding);
        }
    }
    let market = Pubkey::new_from_array(project_found.found.market.to_bytes());
    let generation = project_found.found.generation;
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        snapshot.resolution_program.key.to_bytes(),
        market.to_bytes(),
        generation,
        manifest_id,
        FundingLedgerV2::decode(&ledger_bytes)
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    if Pubkey::find_program_address(
        &derivation.seed_components(),
        &snapshot.resolution_program.key,
    )
    .0 != snapshot.ledger.key
    {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    let prestate_digest = prestate_digest(&snapshot.ledger)?;
    let request = PreMarketFundingRequestV1 {
        project_found,
        manifest: manifest_digest,
        selected_mask,
        funding_source: snapshot.funding_source.key.to_bytes(),
        ledger: snapshot.ledger.key.to_bytes(),
        prestate_digest,
    };
    let request_bytes = request
        .encode()
        .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let cache = found
        .get(FOUND_ACTIVATION_CACHE)
        .ok_or(ResolutionCoreOperatorErrorV3::Frame)?;
    let registry = found
        .get(FOUND_REGISTRY_PROGRAM)
        .ok_or(ResolutionCoreOperatorErrorV3::Frame)?;
    let activation = ActivatedExecutionReleaseSetViewV1::decode(&cache.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    let release_set = activation
        .execution_release_set_id()
        .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    let trading = activation
        .role(ExecutionRoleV1::Trading)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    let resolution = activation
        .role(ExecutionRoleV1::Resolution)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    if cache.owner != registry.key
        || Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_bytes()],
            &registry.key,
        )
        .0 != cache.key
        || trading.release().program().to_bytes() != snapshot.caller_program.key.to_bytes()
        || resolution.release().program().to_bytes() != snapshot.resolution_program.key.to_bytes()
        || resolution.release().semantic_release_id().to_bytes()
            != RESOLUTION_CONTROLLER_RELEASE_ID_V5
    {
        return Err(ResolutionCoreOperatorErrorV3::Record);
    }
    trading
        .authenticate_current_deployment(deployment_observation(
            &snapshot.caller_program,
            &snapshot.caller_programdata,
            trading.release(),
        )?)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Release)?;
    resolution
        .authenticate_current_deployment(deployment_observation(
            &snapshot.resolution_program,
            &snapshot.resolution_programdata,
            resolution.release(),
        )?)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Release)?;
    let authority_seeds = CallerAuthoritySeedsV1::new(
        release_set,
        market.to_bytes(),
        ExecutionRoleV1::Trading,
        manifest_digest,
        hash(&request_bytes).to_bytes(),
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let caller_authority =
        Pubkey::find_program_address(&authority_seeds.as_slices(), &snapshot.caller_program.key).0;
    if caller_authority == snapshot.caller_program.key
        || caller_authority == snapshot.caller_programdata.key
        || caller_authority == snapshot.resolution_program.key
        || caller_authority == snapshot.resolution_programdata.key
        || caller_authority == snapshot.funding_source.key
        || caller_authority == snapshot.ledger.key
        || found.iter().any(|account| account.key == caller_authority)
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    let rent = decode_rent(
        found
            .get(FOUND_RENT)
            .ok_or(ResolutionCoreOperatorErrorV3::Frame)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    let exact_rent_lamports = rent.minimum_balance(width);
    let exact_funding_lamports = exact_rent_lamports
        .checked_add(exact_native_principal)
        .ok_or(ResolutionCoreOperatorErrorV3::Funding)?;
    if snapshot.funding_source.lamports < exact_funding_lamports {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    let found_request_digest = hash(
        &project_found
            .found
            .encode()
            .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?,
    )
    .to_bytes();
    let expected_receipt = PreMarketFundingReceiptV1 {
        market: market.to_bytes(),
        generation,
        manifest: manifest_digest,
        selected_mask,
        ledger: snapshot.ledger.key.to_bytes(),
        prestate_digest,
        poststate_digest: hash(&ledger_bytes).to_bytes(),
        exact_rent_lamports,
        exact_native_principal,
        found_request_digest,
        funding_source: snapshot.funding_source.key.to_bytes(),
        rent_credit: found
            .get(FOUND_RENT_CREDIT)
            .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
            .key
            .to_bytes(),
    };
    let mut accounts = Vec::with_capacity(PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1);
    accounts.push(AccountMeta::new_readonly(caller_authority, true));
    accounts.push(AccountMeta::new_readonly(
        snapshot.caller_program.key,
        false,
    ));
    accounts.push(AccountMeta::new_readonly(
        snapshot.caller_programdata.key,
        false,
    ));
    accounts.push(AccountMeta::new_readonly(
        snapshot.resolution_program.key,
        false,
    ));
    accounts.push(AccountMeta::new_readonly(
        snapshot.resolution_programdata.key,
        false,
    ));
    accounts.push(AccountMeta::new(snapshot.funding_source.key, true));
    accounts.push(AccountMeta::new(snapshot.ledger.key, false));
    accounts.extend(
        found
            .iter()
            .map(|account| AccountMeta::new_readonly(account.key, false)),
    );
    Ok(PreMarketFundingReportV1 {
        instruction: Instruction {
            program_id: snapshot.resolution_program.key,
            accounts,
            data: request_bytes.to_vec(),
        },
        caller_authority,
        exact_funding_lamports,
        exact_rent_lamports,
        exact_native_principal,
        selected_mask,
        expected_receipt,
    })
}

/// Hostile-decode and authenticate the initializer's exact return receipt.
///
/// The composing Trading program separately authenticates the return-data
/// program id. This comparison binds every projected coordinate, including the
/// future Core-owned RentCredit beneficiary, to the report it invoked.
pub fn authenticate_pre_market_funding_receipt_v1(
    receipt_data: &[u8],
    expected: PreMarketFundingReceiptV1,
) -> Result<PreMarketFundingReceiptV1, ResolutionCoreOperatorErrorV3> {
    let observed = PreMarketFundingReceiptV1::decode(receipt_data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    if observed != expected {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    Ok(observed)
}

fn authenticate_snapshot(
    snapshot: &PreMarketFundingSnapshotV1,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    if snapshot.project_found_accounts.len() != PRE_MARKET_PROJECT_FOUND_ACCOUNT_COUNT_V1
        || !snapshot.resolution_program.executable
        || !snapshot.caller_program.executable
        || snapshot.caller_programdata.executable
        || snapshot.resolution_programdata.executable
        || snapshot.ledger.owner != system_program::ID
        || snapshot.ledger.executable
        || snapshot.ledger.lamports != 0
        || !snapshot.ledger.data.is_empty()
        || snapshot.funding_source.executable
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    let observation = snapshot.resolution_program.observation;
    if observation.finality != Finality::Finalized
        || snapshot.caller_program.observation != observation
        || snapshot.caller_programdata.observation != observation
        || snapshot.resolution_programdata.observation != observation
        || snapshot.funding_source.observation != observation
        || snapshot.ledger.observation != observation
        || snapshot
            .project_found_accounts
            .iter()
            .any(|account| account.observation != observation)
    {
        return Err(ResolutionCoreOperatorErrorV3::Snapshot);
    }
    let prefix = [
        &snapshot.caller_program,
        &snapshot.caller_programdata,
        &snapshot.resolution_program,
        &snapshot.resolution_programdata,
        &snapshot.funding_source,
        &snapshot.ledger,
    ];
    for (index, account) in prefix.iter().enumerate() {
        if prefix
            .iter()
            .skip(index + 1)
            .any(|other| other.key == account.key)
        {
            return Err(ResolutionCoreOperatorErrorV3::Frame);
        }
    }
    for (index, account) in snapshot.project_found_accounts.iter().enumerate() {
        if snapshot
            .project_found_accounts
            .iter()
            .skip(index + 1)
            .any(|other| other.key == account.key)
        {
            return Err(ResolutionCoreOperatorErrorV3::Frame);
        }
        let executable = matches!(
            index,
            3 | FOUND_CORE_PROGRAM | FOUND_REGISTRY_PROGRAM | FOUND_SYSTEM
        );
        if account.executable != executable || prefix.iter().any(|prefix| account.key == prefix.key)
        {
            return Err(ResolutionCoreOperatorErrorV3::Frame);
        }
    }
    let core_program = snapshot
        .project_found_accounts
        .get(FOUND_CORE_PROGRAM)
        .ok_or(ResolutionCoreOperatorErrorV3::Frame)?;
    if core_program.key == snapshot.resolution_program.key
        || snapshot
            .project_found_accounts
            .get(FOUND_RENT)
            .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
            .key
            != solana_sdk_ids::sysvar::rent::ID
        || snapshot
            .project_found_accounts
            .get(FOUND_SYSTEM)
            .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
            .key
            != system_program::ID
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    Ok(())
}

fn resolution_mask(
    manifest: CapabilityManifestV1<'_>,
) -> Result<u16, ResolutionCoreOperatorErrorV3> {
    let mut mask = 0_u16;
    for entry_index in 0_u16..manifest.entry_count() {
        if manifest
            .entry(entry_index)
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?
            .release_id()
            .to_bytes()
            == RESOLUTION_CONTROLLER_RELEASE_ID_V5
        {
            mask |= 1_u16 << entry_index;
        }
    }
    if mask.count_ones() != 3 {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    Ok(mask)
}

fn prestate_digest(ledger: &ObservedAccount) -> Result<[u8; 32], ResolutionCoreOperatorErrorV3> {
    let data_len =
        u64::try_from(ledger.data.len()).map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    Ok(pre_market_funding_prestate_digest_v1(
        ledger.key.to_bytes(),
        ledger.owner.to_bytes(),
        ledger.lamports,
        data_len,
    ))
}

#[cfg(test)]
mod receipt_tests {
    use super::*;
    use crate::Observation;
    use dclutch_core_contract::ContentId;
    use dclutch_registry_contract::{ArtifactReleaseV1, ArtifactUpgradePolicyV1};
    use dclutch_release_set_contract::ProgramIdentityV1;
    use solana_sdk_ids::bpf_loader_upgradeable;

    fn receipt() -> PreMarketFundingReceiptV1 {
        PreMarketFundingReceiptV1 {
            market: [1; 32],
            generation: 2,
            manifest: [3; 32],
            selected_mask: 0b111,
            ledger: [4; 32],
            prestate_digest: [5; 32],
            poststate_digest: [6; 32],
            exact_rent_lamports: 7,
            exact_native_principal: 8,
            found_request_digest: [9; 32],
            funding_source: [10; 32],
            rent_credit: [11; 32],
        }
    }

    #[test]
    fn receipt_authentication_refuses_substituted_rent_credit() {
        let expected = receipt();
        let exact = expected.encode().expect("receipt");
        assert_eq!(
            authenticate_pre_market_funding_receipt_v1(&exact, expected),
            Ok(expected)
        );
        let mut substituted = expected;
        substituted.rent_credit = [12; 32];
        let bytes = substituted.encode().expect("substituted receipt");
        assert_eq!(
            authenticate_pre_market_funding_receipt_v1(&bytes, expected),
            Err(ResolutionCoreOperatorErrorV3::Funding)
        );
    }

    fn observed(index: u8, observation: Observation) -> ObservedAccount {
        ObservedAccount {
            observation,
            key: Pubkey::new_from_array([index; 32]),
            owner: system_program::ID,
            lamports: 0,
            executable: false,
            data: Vec::new(),
        }
    }

    fn snapshot() -> PreMarketFundingSnapshotV1 {
        let observation = Observation {
            slot: 1,
            unix_timestamp: 1,
            finality: Finality::Finalized,
        };
        let mut found: Vec<_> = (40_u8..77)
            .map(|index| observed(index, observation))
            .collect();
        for index in [3, FOUND_CORE_PROGRAM, FOUND_REGISTRY_PROGRAM, FOUND_SYSTEM] {
            found[index].executable = true;
        }
        found[FOUND_RENT].key = solana_sdk_ids::sysvar::rent::ID;
        found[FOUND_SYSTEM].key = system_program::ID;
        PreMarketFundingSnapshotV1 {
            resolution_program: ObservedAccount {
                executable: true,
                ..observed(1, observation)
            },
            caller_program: ObservedAccount {
                executable: true,
                ..observed(2, observation)
            },
            caller_programdata: observed(3, observation),
            resolution_programdata: observed(4, observation),
            funding_source: observed(5, observation),
            ledger: observed(6, observation),
            project_found_accounts: found,
        }
    }

    #[test]
    fn snapshot_refuses_every_cross_prefix_alias_and_executable_substitution() {
        assert_eq!(PRE_MARKET_FUNDING_ACCOUNT_COUNT_V1, 44);
        let exact = snapshot();
        assert_eq!(authenticate_snapshot(&exact), Ok(()));

        let mut funding_as_found_payer = exact.clone();
        funding_as_found_payer.funding_source.key =
            funding_as_found_payer.project_found_accounts[0].key;
        assert_eq!(
            authenticate_snapshot(&funding_as_found_payer),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );

        let mut ledger_alias = exact.clone();
        ledger_alias.ledger.key = ledger_alias.caller_program.key;
        assert_eq!(
            authenticate_snapshot(&ledger_alias),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );

        let mut caller_programdata_alias = exact.clone();
        caller_programdata_alias.caller_programdata.key =
            caller_programdata_alias.resolution_programdata.key;
        assert_eq!(
            authenticate_snapshot(&caller_programdata_alias),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );

        let mut resolution_programdata_executable = exact.clone();
        resolution_programdata_executable
            .resolution_programdata
            .executable = true;
        assert_eq!(
            authenticate_snapshot(&resolution_programdata_executable),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );

        let mut system_not_executable = exact.clone();
        system_not_executable.project_found_accounts[FOUND_SYSTEM].executable = false;
        assert_eq!(
            authenticate_snapshot(&system_not_executable),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );

        let mut registry_not_executable = exact;
        registry_not_executable.project_found_accounts[FOUND_REGISTRY_PROGRAM].executable = false;
        assert_eq!(
            authenticate_snapshot(&registry_not_executable),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );
    }

    fn deployment_fixture() -> (ArtifactReleaseV1, ObservedAccount, ObservedAccount) {
        let observation = Observation {
            slot: 1,
            unix_timestamp: 1,
            finality: Finality::Finalized,
        };
        let program_key = Pubkey::new_from_array([21; 32]);
        let programdata_key =
            Pubkey::find_program_address(&[program_key.as_ref()], &bpf_loader_upgradeable::ID).0;
        let authority = [22; 32];
        let elf = [23; 64];
        let release = ArtifactReleaseV1::new(
            ProgramIdentityV1::new(program_key.to_bytes()).expect("program identity"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader identity"),
            programdata_key.to_bytes(),
            ContentId::new([24; 32]).expect("semantic release"),
            hash(&elf).to_bytes(),
            25,
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some(authority),
        )
        .expect("artifact release");
        let mut program_data = vec![0_u8; 36];
        program_data
            .get_mut(..4)
            .expect("program variant")
            .copy_from_slice(&2_u32.to_le_bytes());
        program_data
            .get_mut(4..36)
            .expect("programdata link")
            .copy_from_slice(programdata_key.as_ref());
        let mut programdata_data = vec![0_u8; 45 + elf.len()];
        programdata_data
            .get_mut(..4)
            .expect("programdata variant")
            .copy_from_slice(&3_u32.to_le_bytes());
        programdata_data
            .get_mut(4..12)
            .expect("deployment slot")
            .copy_from_slice(&25_u64.to_le_bytes());
        *programdata_data.get_mut(12).expect("upgrade authority tag") = 1;
        programdata_data
            .get_mut(13..45)
            .expect("upgrade authority")
            .copy_from_slice(&authority);
        programdata_data
            .get_mut(45..)
            .expect("ELF tail")
            .copy_from_slice(&elf);
        (
            release,
            ObservedAccount {
                observation,
                key: program_key,
                owner: bpf_loader_upgradeable::ID,
                lamports: 1,
                executable: true,
                data: program_data,
            },
            ObservedAccount {
                observation,
                key: programdata_key,
                owner: bpf_loader_upgradeable::ID,
                lamports: 1,
                executable: false,
                data: programdata_data,
            },
        )
    }

    #[test]
    fn current_deployment_observation_binds_link_slot_elf_and_upgrade_authority() {
        let (release, program, programdata) = deployment_fixture();
        let exact = deployment_observation(&program, &programdata, release)
            .expect("canonical Loader observation");
        assert_eq!(release.authenticate_deployment(exact), Ok(()));

        let mut wrong_link = program.clone();
        wrong_link
            .data
            .get_mut(4..36)
            .expect("programdata link")
            .copy_from_slice(&[26; 32]);
        assert_eq!(
            deployment_observation(&wrong_link, &programdata, release),
            Err(ResolutionCoreOperatorErrorV3::Release)
        );

        let mut wrong_slot = programdata.clone();
        wrong_slot
            .data
            .get_mut(4..12)
            .expect("deployment slot")
            .copy_from_slice(&26_u64.to_le_bytes());
        let observation = deployment_observation(&program, &wrong_slot, release)
            .expect("well-formed substituted slot");
        assert!(release.authenticate_deployment(observation).is_err());

        let mut wrong_elf = programdata.clone();
        *wrong_elf.data.last_mut().expect("ELF byte") ^= 1;
        let observation = deployment_observation(&program, &wrong_elf, release)
            .expect("well-formed substituted ELF");
        assert!(release.authenticate_deployment(observation).is_err());

        let mut wrong_authority = programdata;
        wrong_authority
            .data
            .get_mut(13..45)
            .expect("upgrade authority")
            .copy_from_slice(&[27; 32]);
        let observation = deployment_observation(&program, &wrong_authority, release)
            .expect("well-formed substituted authority");
        assert!(release.authenticate_deployment(observation).is_err());
    }
}
