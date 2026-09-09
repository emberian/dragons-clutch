//! Terminal close of one never-activated Direct funding ledger.
//!
//! Founding creates the Trading-owned prepaid-lazy ledger before Direct's
//! first use creates its root. A Market that reaches retirement without a
//! trade therefore has a live Pending ledger and no root. This route proves
//! that exact split from the immutable manifest and Market, then returns the
//! ledger's native principal and recorded rent to the Market RentCredit. It
//! cannot close an Active ledger and never changes Core capability count.

use dclutch_core_contract::ContentId;
use dclutch_market::capability_manifest::{
    ActivationPolicy, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, FundingLedgerStatusV2,
    FundingLedgerV2, funding::funded_rent_persists_v1,
};
use dclutch_market::capability_program::{CapabilityRootHeaderV1, SelectedRecordBumpsV1};
use dclutch_market::rent::lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2};
use dclutch_market::{CoreState, MarketCoreStateSeedsV2, Phase, STATE_BYTES};
use dclutch_registry::ActivatedExecutionReleaseSetViewV1;
use dclutch_registry::activation_auth_v1::{
    authenticate_activated_role_in_frame_v1, authenticate_activation_cache_identity_v1,
    require_cache_account,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{CapabilityExecutionSelectionV1, ExecutionRoleV1};
use dclutch_trading::execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3;
use dclutch_trading::retirement_v1::DirectCloseUnusedRequestV1;
pub use dclutch_trading::retirement_v1::is_direct_close_unused_v1;
use solana_program::{
    account_info::AccountInfo, hash::hash, program_error::ProgramError, pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use crate::TradingSbfError;

/// Exact account count.
pub const DIRECT_CLOSE_UNUSED_ACCOUNT_COUNT_V1: usize = 14;

const MARKET: usize = 0;
const LEDGER: usize = 1;
const VACANT_ROOT: usize = 2;
const RENT_CREDIT: usize = 3;
const MANIFEST_RAW: usize = 4;
const MANIFEST_STAGING: usize = 5;
const ACTIVATION_CACHE: usize = 6;
const CORE_PROGRAM: usize = 7;
const CORE_PROGRAMDATA: usize = 8;
const TRADING_PROGRAM: usize = 9;
const TRADING_PROGRAMDATA: usize = 10;
const REGISTRY: usize = 11;
const RENT_PROGRAM: usize = 12;
const SYSTEM_PROGRAM: usize = 13;

fn decode_request(input: &[u8]) -> Result<u16, ProgramError> {
    DirectCloseUnusedRequestV1::decode(input)
        .map(|request| request.entry_index)
        .map_err(|_| TradingSbfError::UnsupportedContent.into())
}

/// Close one exact Pending singleton ledger while the corresponding root is absent.
#[inline(never)]
pub fn process_direct_close_unused_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let entry_index = decode_request(instruction_data)?;
    authenticate_frame(program_id, accounts)?;
    let market = authenticate_market(accounts)?;
    authenticate_release(accounts, market.identity.selected_release_set.to_bytes())?;

    let manifest_account = account(accounts, MANIFEST_RAW)?;
    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_manifest(accounts, market, &manifest_data)?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| TradingSbfError::Content)?;
    let manifest_id = ContentId::new(market.identity.capability_manifest.to_bytes())
        .map_err(|_| TradingSbfError::Content)?;
    let entry = manifest
        .entry(entry_index)
        .map_err(|_| TradingSbfError::Content)?;
    if entry.activation_policy() != ActivationPolicy::PrepaidLazy
        || entry.kind_id().to_bytes() != DIRECT_SUCCESSOR_KIND_ID_V3
    {
        return Err(TradingSbfError::Content.into());
    }
    let selection = CapabilityExecutionSelectionV1::new(
        entry_index,
        manifest_id,
        entry.kind_id(),
        entry.release_id(),
        entry.config_id(),
    )
    .map_err(|_| TradingSbfError::Content)?;
    let header = CapabilityRootHeaderV1::new(
        ContentId::new(market.identity.selected_release_set.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        market.identity.market_id.to_bytes(),
        market.identity.generation,
        selection,
        SelectedRecordBumpsV1::default(),
    )
    .map_err(|_| TradingSbfError::Content)?;
    let expected_root = Pubkey::find_program_address(&header.seeds().as_slices(), program_id).0;
    let root = account(accounts, VACANT_ROOT)?;
    if root.key != &expected_root || root.owner != &system_program::ID || root.data_len() != 0 {
        return Err(TradingSbfError::Root.into());
    }

    let ledger = account(accounts, LEDGER)?;
    let ledger_data = ledger
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let decoded = FundingLedgerV2::decode(&ledger_data).map_err(|_| TradingSbfError::Content)?;
    let selected_bit = 1_u16
        .checked_shl(u32::from(entry_index))
        .ok_or(TradingSbfError::Content)?;
    if decoded.selected_mask() != selected_bit || decoded.slot_count() != 1 {
        return Err(TradingSbfError::Content.into());
    }
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        program_id.to_bytes(),
        market.identity.market_id.to_bytes(),
        market.identity.generation,
        manifest_id,
        decoded,
    )
    .map_err(|_| TradingSbfError::Content)?;
    if ledger.owner != program_id
        || Pubkey::find_program_address(&derivation.seed_components(), program_id).0 != *ledger.key
    {
        return Err(TradingSbfError::Content.into());
    }
    let authenticated = decoded
        .authenticate(manifest_id, manifest)
        .map_err(|_| TradingSbfError::Content)?;
    if authenticated
        .slot(entry_index)
        .map_err(|_| TradingSbfError::Content)?
        .status()
        != FundingLedgerStatusV2::Pending
    {
        return Err(TradingSbfError::Transition.into());
    }
    let exact_rent = authenticated
        .funded_rent_minimum(ledger_data.len())
        .map_err(|_| TradingSbfError::FundedRent)?;
    let principal = authenticated
        .remaining_native_lamports_total()
        .map_err(|_| TradingSbfError::Content)?;
    authenticated
        .validate_native_custody(ledger.lamports(), exact_rent, true)
        .map_err(|_| TradingSbfError::FundedRent)?;
    authenticate_rent_credit(accounts, market)?;
    drop(ledger_data);
    drop(manifest_data);
    close_to_rent_credit(
        ledger,
        account(accounts, RENT_CREDIT)?,
        principal,
        exact_rent,
    )
}

fn authenticate_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    if accounts.len() != DIRECT_CLOSE_UNUSED_ACCOUNT_COUNT_V1 {
        return Err(TradingSbfError::Content.into());
    }
    for (index, value) in accounts.iter().enumerate() {
        let writable = matches!(index, LEDGER | RENT_CREDIT);
        let executable = matches!(
            index,
            CORE_PROGRAM | TRADING_PROGRAM | REGISTRY | RENT_PROGRAM | SYSTEM_PROGRAM
        );
        if value.is_signer
            || value.is_writable != writable
            || value.executable != executable
            || accounts
                .iter()
                .skip(index + 1)
                .any(|other| other.key == value.key)
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    if account(accounts, TRADING_PROGRAM)?.key != program_id
        || account(accounts, SYSTEM_PROGRAM)?.key != &system_program::ID
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_market(accounts: &[AccountInfo<'_>]) -> Result<CoreState, ProgramError> {
    let market_account = account(accounts, MARKET)?;
    let core = account(accounts, CORE_PROGRAM)?;
    let registry = account(accounts, REGISTRY)?;
    let bytes = market_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if market_account.owner != core.key || bytes.len() != STATE_BYTES {
        return Err(TradingSbfError::Content.into());
    }
    let state = CoreState::decode(&bytes).map_err(|_| TradingSbfError::Content)?;
    if state
        .encode()
        .map_err(|_| TradingSbfError::Content)?
        .as_slice()
        != bytes.as_ref()
        || state.phase != Phase::Retiring
        || state.outstanding_capabilities != 0
        || state.identity.market_id.to_bytes() != market_account.key.to_bytes()
        || state.identity.registry_program.to_bytes() != registry.key.to_bytes()
        || Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
            core.key,
        )
        .0 != *market_account.key
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(state)
}

fn authenticate_manifest(
    accounts: &[AccountInfo<'_>],
    market: CoreState,
    bytes: &[u8],
) -> Result<(), ProgramError> {
    let registry = account(accounts, REGISTRY)?;
    let raw = account(accounts, MANIFEST_RAW)?;
    let staging = account(accounts, MANIFEST_STAGING)?;
    let digest = market.identity.capability_manifest.to_bytes();
    let expected_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            &digest,
        ],
        registry.key,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            &digest,
        ],
        registry.key,
    )
    .0;
    if raw.key != &expected_raw
        || raw.owner != registry.key
        || hash(bytes).to_bytes() != digest
        || !funded_rent_persists_v1(raw.lamports())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_release(
    accounts: &[AccountInfo<'_>],
    release_set: [u8; 32],
) -> Result<(), ProgramError> {
    let cache = account(accounts, ACTIVATION_CACHE)?;
    let registry = account(accounts, REGISTRY)?;
    require_cache_account(registry.key, cache).map_err(TradingSbfError::from)?;
    let data = cache
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated =
        ActivatedExecutionReleaseSetViewV1::decode(&data).map_err(|_| TradingSbfError::Release)?;
    authenticate_activation_cache_identity_v1(registry, cache, &release_set, activated)
        .map_err(TradingSbfError::from)?;
    for (role, program, programdata) in [
        (ExecutionRoleV1::Core, CORE_PROGRAM, CORE_PROGRAMDATA),
        (
            ExecutionRoleV1::Trading,
            TRADING_PROGRAM,
            TRADING_PROGRAMDATA,
        ),
    ] {
        authenticate_activated_role_in_frame_v1(
            cache,
            activated,
            role,
            account(accounts, program)?,
            account(accounts, programdata)?,
        )
        .map_err(TradingSbfError::from)?;
    }
    Ok(())
}

fn authenticate_rent_credit(
    accounts: &[AccountInfo<'_>],
    market: CoreState,
) -> Result<(), ProgramError> {
    let credit_account = account(accounts, RENT_CREDIT)?;
    let rent_program = account(accounts, RENT_PROGRAM)?;
    if credit_account.key.to_bytes() != market.rent_beneficiary.to_bytes()
        || credit_account.owner != rent_program.key
        || credit_account.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || !funded_rent_persists_v1(credit_account.lamports())
    {
        return Err(TradingSbfError::Content.into());
    }
    let data = credit_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let credit = LifecycleRentCreditV2::decode(&data).map_err(|_| TradingSbfError::Content)?;
    let seeds = credit.pda_seeds();
    let market_seed = seeds.market().to_bytes();
    let generation = seeds.generation();
    let bump = [seeds.bump()];
    if credit.to_bytes().as_slice() != data.as_ref()
        || credit.market().to_bytes() != market.identity.market_id.to_bytes()
        || credit.release_set().to_bytes() != market.identity.selected_release_set.to_bytes()
        || credit.generation() != market.identity.generation
        || Pubkey::create_program_address(
            &[
                seeds.domain(),
                market_seed.as_slice(),
                generation.as_slice(),
                &bump,
            ],
            rent_program.key,
        )
        .map_err(|_| TradingSbfError::Content)?
            != *credit_account.key
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn close_to_rent_credit(
    ledger: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
    principal: u64,
    rent: u64,
) -> Result<(), ProgramError> {
    let minimum_backing = principal.checked_add(rent).ok_or(TradingSbfError::Commit)?;
    let total = ledger.lamports();
    let credit_post = rent_credit
        .lamports()
        .checked_add(total)
        .ok_or(TradingSbfError::Commit)?;
    if total < minimum_backing {
        return Err(TradingSbfError::Commit.into());
    }
    **rent_credit
        .try_borrow_mut_lamports()
        .map_err(|_| TradingSbfError::Commit)? = credit_post;
    **ledger
        .try_borrow_mut_lamports()
        .map_err(|_| TradingSbfError::Commit)? = 0;
    ledger.resize(0).map_err(|_| TradingSbfError::Commit)?;
    ledger.assign(&system_program::ID);
    if ledger.lamports() != 0 || ledger.data_len() != 0 || ledger.owner != &system_program::ID {
        return Err(TradingSbfError::Commit.into());
    }
    Ok(())
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts.get(index).ok_or(TradingSbfError::Content.into())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{boxed::Box, vec, vec::Vec};

    use dclutch_market::capability_manifest::{
        CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1, FundingAmountsV1,
        FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
        derive_funded_rent_rate_v2, funding_ledger_bytes_v2,
    };
    use dclutch_market::rent::{RefundAuthority, lifecycle_v2::LifecycleAccountIdV2};
    use dclutch_market::{Identity, MarketIdentity, Readiness, StateBumpsV1};
    use dclutch_registry::release_set::{
        ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ProgramIdentityV1,
    };
    use dclutch_registry::{
        ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ArtifactActivationInputV1, ArtifactReleaseV2,
        ArtifactUpgradePolicyV1, DeploymentObservationV2, activate_execution_role_into_v1,
        initialize_activation_cache_v1,
    };
    use solana_program::rent::Rent;
    use solana_sdk_ids::{bpf_loader_upgradeable, native_loader};

    use super::*;

    const ALL_ROLES: [ExecutionRoleV1; 5] = [
        ExecutionRoleV1::Core,
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Trading,
        ExecutionRoleV1::Resolution,
        ExecutionRoleV1::Custody,
    ];

    fn identity(byte: u8) -> Identity {
        Identity::new([byte; 32]).expect("identity")
    }

    #[repr(C)]
    struct SerializedKey {
        original_data_len: u32,
        key: Pubkey,
    }

    fn observed(
        key: Pubkey,
        writable: bool,
        lamports: u64,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        // `AccountInfo::resize` is defined only over the loader's serialized
        // memory shape: four bytes before the key carry the original data
        // length and eight bytes before the data carry its mutable length.
        // Model that exact shape so the accepted close tests the real physical
        // tail instead of invoking `resize` on an ordinary Rust slice.
        let serialized_key = Box::leak(Box::new(SerializedKey {
            original_data_len: u32::try_from(data.len()).expect("test account width"),
            key,
        }));
        let mut serialized_data = vec![0_u8; 8 + data.len()];
        serialized_data[..8].copy_from_slice(
            &u64::try_from(data.len())
                .expect("test account width")
                .to_le_bytes(),
        );
        serialized_data[8..].copy_from_slice(&data);
        let serialized_data = Box::leak(serialized_data.into_boxed_slice());
        AccountInfo::new(
            &serialized_key.key,
            false,
            writable,
            Box::leak(Box::new(lamports)),
            &mut serialized_data[8..],
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn loader_program_bytes(programdata: Pubkey) -> Vec<u8> {
        let mut output = vec![0_u8; 36];
        output[..4].copy_from_slice(&2_u32.to_le_bytes());
        output[4..36].copy_from_slice(programdata.as_ref());
        output
    }

    fn immutable_programdata_bytes(slot: u64, elf: &[u8]) -> Vec<u8> {
        let mut output = vec![0_u8; 45 + elf.len()];
        output[..4].copy_from_slice(&3_u32.to_le_bytes());
        output[4..12].copy_from_slice(&slot.to_le_bytes());
        output[45..].copy_from_slice(elf);
        output
    }

    struct ReleaseFixture {
        release: ArtifactReleaseV2,
        artifact_id: ArtifactReleaseIdV1,
        input: ArtifactActivationInputV1,
        program: AccountInfo<'static>,
        programdata: AccountInfo<'static>,
    }

    fn release_fixture(seed: u8) -> ReleaseFixture {
        let program = Pubkey::new_from_array([seed; 32]);
        let programdata =
            Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
        let slot = 70_u64 + u64::from(seed);
        let elf = vec![seed ^ 0x5a; 96];
        let commitment = dclutch_registry::artifact_code_commitment_v2::code_commitment_v2(&elf)
            .expect("code commitment");
        let release = ArtifactReleaseV2::new(
            ProgramIdentityV1::new(program.to_bytes()).expect("program"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
            programdata.to_bytes(),
            ContentId::new([seed ^ 0xa5; 32]).expect("semantic release"),
            commitment,
            slot,
            ArtifactUpgradePolicyV1::Immutable,
            None,
        )
        .expect("release");
        let artifact_id =
            ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact id");
        let observation = DeploymentObservationV2::new(
            program.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            programdata.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            programdata.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            slot,
            commitment,
            None,
        )
        .expect("observation");
        ReleaseFixture {
            release,
            artifact_id,
            input: ArtifactActivationInputV1::new(artifact_id, release, observation),
            program: observed(
                program,
                false,
                1,
                loader_program_bytes(programdata),
                bpf_loader_upgradeable::ID,
                true,
            ),
            programdata: observed(
                programdata,
                false,
                1,
                immutable_programdata_bytes(slot, &elf),
                bpf_loader_upgradeable::ID,
                false,
            ),
        }
    }

    fn direct_manifest() -> Vec<u8> {
        let native = CompartmentFundingV1::native_lamports(100).expect("native funding");
        let none = CompartmentFundingV1::not_applicable();
        let amounts = FundingAmountsV1::new(native, native, none, none, none, none, none)
            .expect("funding amounts");
        let entry = CapabilityEntryV1::new(
            ContentId::new(DIRECT_SUCCESSOR_KIND_ID_V3).expect("Direct kind"),
            ContentId::new([0x61; 32]).expect("release"),
            ContentId::new([0x62; 32]).expect("config"),
            ContentId::new([0x63; 32]).expect("capacity"),
            ContentId::new([0x64; 32]).expect("schema"),
            ContentId::new([0x65; 32]).expect("derivation"),
            ActivationPolicy::PrepaidLazy,
            500,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            FundingQuoteV1::new(amounts, None).expect("funding quote"),
        )
        .expect("Direct entry");
        let mut bytes = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut bytes).expect("manifest");
        bytes
    }

    struct EntrypointFixture {
        program_id: Pubkey,
        accounts: Vec<AccountInfo<'static>>,
        request: [u8; dclutch_trading::retirement_v1::DIRECT_CLOSE_UNUSED_REQUEST_BYTES_V1],
        ledger_pre_lamports: u64,
        credit_pre_lamports: u64,
        market_pre: Vec<u8>,
    }

    fn entrypoint_fixture(active: bool) -> EntrypointFixture {
        let registry = Pubkey::new_from_array([0x31; 32]);
        let rent_program = Pubkey::new_from_array([0x32; 32]);
        let releases = [
            release_fixture(0x41),
            release_fixture(0x42),
            release_fixture(0x43),
            release_fixture(0x44),
            release_fixture(0x45),
        ];
        let binding = |index: usize| {
            ExecutionRoleBindingV1::new(
                releases[index].release.program(),
                releases[index].artifact_id,
            )
        };
        let release_set =
            ExecutionReleaseSetV1::new(binding(0), binding(1), binding(2), binding(3), binding(4))
                .expect("release set");
        let release_set_id =
            ContentId::new(hash(&release_set.to_bytes()).to_bytes()).expect("release set id");
        let mut cache_bytes = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
        initialize_activation_cache_v1(&mut cache_bytes, release_set_id).expect("cache initialize");
        for (role, release) in ALL_ROLES.into_iter().zip(&releases) {
            activate_execution_role_into_v1(
                &mut cache_bytes,
                release_set_id,
                &release_set,
                role,
                &release.input,
            )
            .expect("role activation");
        }
        let cache = dclutch_registry::activation_auth_v1::activation_cache_address_v1(
            &registry,
            &release_set_id.to_bytes(),
        );

        let manifest_bytes = direct_manifest();
        let manifest_id =
            ContentId::new(hash(&manifest_bytes).to_bytes()).expect("manifest content id");
        let core_program = *releases[0].program.key;
        let program_id = *releases[2].program.key;
        let generation = 9_u64;
        let mut market_identity = MarketIdentity {
            market_id: identity(1),
            realm_id: identity(2),
            product_record: identity(3),
            product_id: identity(4),
            resolution_policy: identity(5),
            capability_manifest: Identity::new(manifest_id.to_bytes()).expect("manifest identity"),
            selected_release_set: Identity::new(release_set_id.to_bytes())
                .expect("release identity"),
            registry_program: Identity::new(registry.to_bytes()).expect("registry identity"),
            generation,
        };
        let market = Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(market_identity).as_slices(),
            &core_program,
        )
        .0;
        market_identity.market_id = Identity::new(market.to_bytes()).expect("market identity");
        let generation_bytes = generation.to_le_bytes();
        let (rent_credit, rent_bump) = Pubkey::find_program_address(
            &[
                dclutch_market::rent::lifecycle_v2::LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
                market.as_ref(),
                &generation_bytes,
            ],
            &rent_program,
        );
        let market_state = CoreState {
            phase: Phase::Retiring,
            readiness: Readiness::Consumed,
            terminal_winner: 0,
            identity: market_identity,
            outstanding_capabilities: 0,
            principal_cap_sets: 1,
            rent_beneficiary: Identity::new(rent_credit.to_bytes()).expect("rent beneficiary"),
            terminal_receipt: Some(identity(6)),
            bumps: StateBumpsV1::UNRECORDED,
        }
        .encode()
        .expect("market state")
        .to_vec();

        let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest decode");
        let ledger_len = funding_ledger_bytes_v2(1).expect("ledger width");
        let rent = Rent::default();
        let rate = derive_funded_rent_rate_v2(
            rent.minimum_balance(0),
            ledger_len,
            rent.minimum_balance(ledger_len),
        )
        .expect("funded rent rate");
        let mut ledger_bytes = vec![0_u8; ledger_len];
        FundingLedgerV2::initialize(&mut ledger_bytes, manifest_id, manifest, 1, rate)
            .expect("ledger initialize");
        if active {
            FundingLedgerV2::activate_in_place(&mut ledger_bytes, manifest_id, manifest, 0, 100)
                .expect("ledger activate");
        }
        let ledger = FundingLedgerV2::decode(&ledger_bytes).expect("ledger decode");
        let ledger_key = Pubkey::find_program_address(
            &CapabilityFundingLedgerDerivationV2::new(
                program_id.to_bytes(),
                market.to_bytes(),
                generation,
                manifest_id,
                ledger,
            )
            .expect("ledger derivation")
            .seed_components(),
            &program_id,
        )
        .0;
        let authenticated = ledger
            .authenticate(manifest_id, manifest)
            .expect("ledger authentication");
        let principal = authenticated
            .remaining_native_lamports_total()
            .expect("principal");
        let exact_rent = authenticated
            .funded_rent_minimum(ledger_len)
            .expect("exact rent");
        let ledger_pre_lamports = principal + exact_rent + 17;

        let entry = manifest.entry(0).expect("entry");
        let selection = CapabilityExecutionSelectionV1::new(
            0,
            manifest_id,
            entry.kind_id(),
            entry.release_id(),
            entry.config_id(),
        )
        .expect("selection");
        let root_header = CapabilityRootHeaderV1::new(
            release_set_id,
            market.to_bytes(),
            generation,
            selection,
            SelectedRecordBumpsV1::default(),
        )
        .expect("root header");
        let vacant_root =
            Pubkey::find_program_address(&root_header.seeds().as_slices(), &program_id).0;
        let manifest_raw = Pubkey::find_program_address(
            &[
                RAW_RECORD_PDA_SEED_V1,
                &CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
                manifest_id.as_bytes(),
            ],
            &registry,
        )
        .0;
        let manifest_staging = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                &CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
                manifest_id.as_bytes(),
            ],
            &registry,
        )
        .0;
        let credit_state = LifecycleRentCreditV2::new(
            RefundAuthority::new([0x71; 32]).expect("refund wallet"),
            LifecycleAccountIdV2::new(market.to_bytes()).expect("market credit id"),
            LifecycleAccountIdV2::new(release_set_id.to_bytes()).expect("release credit id"),
            generation,
            rent_bump,
        )
        .expect("rent credit");
        let credit_pre_lamports = 10_000;
        let accounts = vec![
            observed(market, false, 1, market_state.clone(), core_program, false),
            observed(
                ledger_key,
                true,
                ledger_pre_lamports,
                ledger_bytes,
                program_id,
                false,
            ),
            observed(
                vacant_root,
                false,
                13,
                Vec::new(),
                system_program::ID,
                false,
            ),
            observed(
                rent_credit,
                true,
                credit_pre_lamports,
                credit_state.to_bytes().to_vec(),
                rent_program,
                false,
            ),
            observed(manifest_raw, false, 1, manifest_bytes, registry, false),
            observed(
                manifest_staging,
                false,
                11,
                Vec::new(),
                system_program::ID,
                false,
            ),
            observed(cache, false, 1, cache_bytes, registry, false),
            releases[0].program.clone(),
            releases[0].programdata.clone(),
            releases[2].program.clone(),
            releases[2].programdata.clone(),
            observed(registry, false, 1, Vec::new(), native_loader::ID, true),
            observed(rent_program, false, 1, Vec::new(), native_loader::ID, true),
            observed(
                system_program::ID,
                false,
                1,
                Vec::new(),
                native_loader::ID,
                true,
            ),
        ];
        EntrypointFixture {
            program_id,
            accounts,
            request: DirectCloseUnusedRequestV1 { entry_index: 0 }.to_bytes(),
            ledger_pre_lamports,
            credit_pre_lamports,
            market_pre: market_state,
        }
    }

    #[test]
    fn request_is_exact_and_reserved_bytes_refuse() {
        let request = DirectCloseUnusedRequestV1 { entry_index: 7 }.to_bytes();
        assert_eq!(decode_request(&request), Ok(7));
        for offset in [0, 8, 9, 12, 15] {
            let mut hostile = request;
            hostile[offset] ^= 1;
            assert_eq!(
                decode_request(&hostile),
                Err(TradingSbfError::UnsupportedContent.into())
            );
        }
        assert!(!is_direct_close_unused_v1(&request[..15]));
    }

    #[test]
    fn physical_close_returns_principal_rent_and_donation_to_one_credit() {
        let ledger_key = Pubkey::new_unique();
        let credit_key = Pubkey::new_unique();
        let trading = Pubkey::new_unique();
        let rent_program = Pubkey::new_unique();
        let ledger = observed(ledger_key, true, 160, vec![7_u8; 8], trading, false);
        let credit = observed(credit_key, true, 20, Vec::new(), rent_program, false);
        close_to_rent_credit(&ledger, &credit, 100, 50).expect("donated close");
        assert_eq!(ledger.lamports(), 0);
        assert_eq!(ledger.data_len(), 0);
        assert_eq!(ledger.owner, &system_program::ID);
        assert_eq!(credit.lamports(), 180);
    }

    #[test]
    fn physical_close_refuses_underfunding_without_mutation() {
        let ledger_key = Pubkey::new_unique();
        let credit_key = Pubkey::new_unique();
        let trading = Pubkey::new_unique();
        let rent_program = Pubkey::new_unique();
        let ledger = observed(ledger_key, true, 149, vec![7_u8; 8], trading, false);
        let credit = observed(credit_key, true, 20, Vec::new(), rent_program, false);
        assert_eq!(
            close_to_rent_credit(&ledger, &credit, 100, 50),
            Err(TradingSbfError::Commit.into())
        );
        assert_eq!(ledger.lamports(), 149);
        assert_eq!(credit.lamports(), 20);
        assert_eq!(ledger.owner, &trading);
    }

    #[test]
    fn full_entrypoint_accepts_donated_vacancies_and_returns_every_ledger_lamport() {
        let fixture = entrypoint_fixture(false);
        assert_eq!(
            crate::process_instruction(&fixture.program_id, &fixture.accounts, &fixture.request),
            Ok(())
        );
        assert_eq!(fixture.accounts[LEDGER].lamports(), 0);
        assert_eq!(fixture.accounts[LEDGER].data_len(), 0);
        assert_eq!(fixture.accounts[LEDGER].owner, &system_program::ID);
        assert_eq!(
            fixture.accounts[RENT_CREDIT].lamports(),
            fixture.credit_pre_lamports + fixture.ledger_pre_lamports
        );
        assert_eq!(fixture.accounts[VACANT_ROOT].lamports(), 13);
        assert_eq!(fixture.accounts[MANIFEST_STAGING].lamports(), 11);
        assert_eq!(
            fixture.accounts[MARKET]
                .try_borrow_data()
                .expect("Market after close")
                .as_ref(),
            fixture.market_pre.as_slice()
        );
    }

    #[test]
    fn full_entrypoint_refuses_active_ledger_before_any_balance_changes() {
        let fixture = entrypoint_fixture(true);
        let ledger_before = fixture.accounts[LEDGER].lamports();
        let credit_before = fixture.accounts[RENT_CREDIT].lamports();
        assert_eq!(
            crate::process_instruction(&fixture.program_id, &fixture.accounts, &fixture.request),
            Err(TradingSbfError::Transition.into())
        );
        assert_eq!(fixture.accounts[LEDGER].lamports(), ledger_before);
        assert_eq!(fixture.accounts[RENT_CREDIT].lamports(), credit_before);
    }

    #[test]
    fn full_entrypoint_refuses_a_wrong_vacant_root_before_any_balance_changes() {
        let mut fixture = entrypoint_fixture(false);
        fixture.accounts[VACANT_ROOT].key = Box::leak(Box::new(Pubkey::new_unique()));
        let ledger_before = fixture.accounts[LEDGER].lamports();
        let credit_before = fixture.accounts[RENT_CREDIT].lamports();
        assert_eq!(
            crate::process_instruction(&fixture.program_id, &fixture.accounts, &fixture.request),
            Err(TradingSbfError::Root.into())
        );
        assert_eq!(fixture.accounts[LEDGER].lamports(), ledger_before);
        assert_eq!(fixture.accounts[RENT_CREDIT].lamports(), credit_before);
    }

    #[test]
    fn full_entrypoint_refuses_a_foreign_rent_credit_before_any_balance_changes() {
        let mut fixture = entrypoint_fixture(false);
        fixture.accounts[RENT_CREDIT].key = Box::leak(Box::new(Pubkey::new_unique()));
        let ledger_before = fixture.accounts[LEDGER].lamports();
        let credit_before = fixture.accounts[RENT_CREDIT].lamports();
        assert_eq!(
            crate::process_instruction(&fixture.program_id, &fixture.accounts, &fixture.request),
            Err(TradingSbfError::Content.into())
        );
        assert_eq!(fixture.accounts[LEDGER].lamports(), ledger_before);
        assert_eq!(fixture.accounts[RENT_CREDIT].lamports(), credit_before);
    }
}
