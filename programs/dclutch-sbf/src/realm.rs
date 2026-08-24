//! Exact immutable Realm creation through one authenticated System CPI.

use dclutch_collateral_contract::{
    AccountPrivilege, CreateRealmV1, InstructionTag, validate_account_frame,
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_BYTES, REALM_PDA_DOMAIN, RealmV1,
};
use dclutch_token_svm::{COption, CollateralAdapterReleaseV1, PRODUCTION_ADAPTER_RELEASES};
use solana_program::{
    account_info::AccountInfo, hash::hash, program::invoke_signed, program_error::ProgramError,
    pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader, bpf_loader_upgradeable, native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;

use crate::AdapterError;

const CREATE_REALM_ACCOUNTS: usize = 6;

struct CreateRealmFrame<'a, 'info> {
    sponsor: &'a AccountInfo<'info>,
    realm: &'a AccountInfo<'info>,
    mint: &'a AccountInfo<'info>,
    token_program: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    rent_sysvar: &'a AccountInfo<'info>,
}

impl<'a, 'info> CreateRealmFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != CREATE_REALM_ACCOUNTS {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            sponsor: account(accounts, 0)?,
            realm: account(accounts, 1)?,
            mint: account(accounts, 2)?,
            token_program: account(accounts, 3)?,
            system_program: account(accounts, 4)?,
            rent_sysvar: account(accounts, 5)?,
        };
        let privileges = [
            privilege(frame.sponsor),
            privilege(frame.realm),
            privilege(frame.mint),
            privilege(frame.token_program),
            privilege(frame.system_program),
            privilege(frame.rent_sysvar),
        ];
        validate_account_frame(InstructionTag::CreateRealm, &privileges)
            .map_err(|_| AdapterError::AccountPrivilege)?;
        require_distinct(accounts)?;
        Ok(frame)
    }
}

#[derive(Clone, Copy)]
struct CreateRealmPlan {
    realm: RealmV1,
    realm_digest: [u8; 32],
    bump: u8,
    rent_lamports: u64,
    sponsor_before: u64,
}

/// Authenticate, create, and persist one immutable reusable Realm.
pub(crate) fn process_create_realm(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: CreateRealmV1,
) -> Result<(), ProgramError> {
    let frame = CreateRealmFrame::parse(accounts)?;
    let plan = authenticate_create_realm(program_id, &frame, instruction.realm())?;

    let space = u64::try_from(REALM_BYTES).map_err(|_| AdapterError::Arithmetic)?;
    let create = create_account(
        frame.sponsor.key,
        frame.realm.key,
        plan.rent_lamports,
        space,
        program_id,
    );
    let bump_seed = [plan.bump];
    let realm_signer = [
        REALM_PDA_DOMAIN,
        plan.realm_digest.as_slice(),
        bump_seed.as_slice(),
    ];
    invoke_signed(
        &create,
        &[
            frame.sponsor.clone(),
            frame.realm.clone(),
            frame.system_program.clone(),
        ],
        &[&realm_signer],
    )
    .map_err(|_| AdapterError::RealmCreateCpi)?;

    let expected_sponsor = plan
        .sponsor_before
        .checked_sub(plan.rent_lamports)
        .ok_or(AdapterError::Arithmetic)?;
    if frame.sponsor.lamports() != expected_sponsor
        || frame.realm.lamports() != plan.rent_lamports
        || frame.realm.owner != program_id
        || frame.realm.data_len() != REALM_BYTES
    {
        return Err(AdapterError::RealmPostcondition.into());
    }
    let mut realm_data = frame
        .realm
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::RealmPostcondition)?;
    plan.realm
        .encode(&mut realm_data)
        .map_err(|_| AdapterError::RealmPostcondition)?;
    if RealmV1::decode(&realm_data) != Ok(plan.realm) {
        return Err(AdapterError::RealmPostcondition.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_create_realm(
    program_id: &Pubkey,
    frame: &CreateRealmFrame<'_, '_>,
    realm: RealmV1,
) -> Result<CreateRealmPlan, ProgramError> {
    if frame.sponsor.owner != &system_program::ID
        || !frame.sponsor.data_is_empty()
        || frame.realm.owner != &system_program::ID
        || !frame.realm.data_is_empty()
        || frame.realm.lamports() != 0
        || frame.system_program.key != &system_program::ID
        || frame.system_program.owner != &native_loader::ID
        || frame.rent_sysvar.key != &sysvar::rent::ID
        || frame.rent_sysvar.owner != &sysvar::ID
    {
        return Err(AdapterError::RealmAuthentication.into());
    }

    let token_program_key = frame.token_program.key.to_bytes();
    if realm.token_program() != &token_program_key
        || realm.collateral_mint() != frame.mint.key.as_ref()
        || frame.mint.owner != frame.token_program.key
        || !recognized_program_loader(frame.token_program.owner)
    {
        return Err(AdapterError::RealmAuthentication.into());
    }
    let release = select_adapter_release(*realm.collateral_adapter_release_id())?;
    if release.token_program() != token_program_key {
        return Err(AdapterError::RealmAuthentication.into());
    }
    let mint_data = frame
        .mint
        .try_borrow_data()
        .map_err(|_| AdapterError::RealmAuthentication)?;
    let mint = release
        .profile()
        .check_mint(token_program_key, &mint_data)
        .map_err(|_| AdapterError::RealmAuthentication)?;
    require_authority_policy(realm.mint_authority_policy(), &mint.mint_authority)?;
    require_freeze_policy(realm.freeze_authority_policy(), &mint.freeze_authority)?;

    let realm_bytes = realm.to_bytes();
    let realm_digest = hash(&realm_bytes).to_bytes();
    let (expected_realm, bump) =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_digest], program_id);
    if frame.realm.key != &expected_realm {
        return Err(AdapterError::RealmAuthentication.into());
    }
    let rent = Rent::from_account_info(frame.rent_sysvar)
        .map_err(|_| AdapterError::RealmAuthentication)?;
    let rent_lamports = rent.minimum_balance(REALM_BYTES);
    if frame.sponsor.lamports() < rent_lamports {
        return Err(AdapterError::RealmAuthentication.into());
    }

    // Preflight all mutable borrows before invoking the System Program. Runtime
    // rollback remains the authority if any post-CPI check nevertheless fails.
    drop(
        frame
            .sponsor
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::RealmAuthentication)?,
    );
    drop(
        frame
            .realm
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::RealmAuthentication)?,
    );
    drop(
        frame
            .realm
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::RealmAuthentication)?,
    );

    Ok(CreateRealmPlan {
        realm,
        realm_digest,
        bump,
        rent_lamports,
        sponsor_before: frame.sponsor.lamports(),
    })
}

fn select_adapter_release(
    release_id: [u8; 32],
) -> Result<CollateralAdapterReleaseV1, ProgramError> {
    for release in PRODUCTION_ADAPTER_RELEASES {
        if hash(&release.to_bytes()).to_bytes() == release_id {
            return Ok(release);
        }
    }
    Err(AdapterError::RealmAuthentication.into())
}

fn require_authority_policy(
    policy: MintAuthorityPolicy,
    authority: &COption<[u8; 32]>,
) -> Result<(), ProgramError> {
    match policy {
        MintAuthorityPolicy::RequireAbsent if !authority.is_none() => {
            Err(AdapterError::RealmAuthentication.into())
        }
        MintAuthorityPolicy::RequireAbsent | MintAuthorityPolicy::AdmitIssuerControl => Ok(()),
    }
}

fn require_freeze_policy(
    policy: FreezeAuthorityPolicy,
    authority: &COption<[u8; 32]>,
) -> Result<(), ProgramError> {
    match policy {
        FreezeAuthorityPolicy::RequireAbsent if !authority.is_none() => {
            Err(AdapterError::RealmAuthentication.into())
        }
        FreezeAuthorityPolicy::RequireAbsent | FreezeAuthorityPolicy::AdmitIssuerControl => Ok(()),
    }
}

fn recognized_program_loader(owner: &Pubkey) -> bool {
    owner == &bpf_loader::ID || owner == &bpf_loader_upgradeable::ID
}

fn privilege(account: &AccountInfo<'_>) -> AccountPrivilege {
    AccountPrivilege {
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        is_executable: account.executable,
    }
}

fn require_distinct(accounts: &[AccountInfo<'_>]) -> Result<(), ProgramError> {
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other.key == account.key)
        {
            return Err(AdapterError::AccountIdentity.into());
        }
    }
    Ok(())
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or(AdapterError::AccountFrameLength.into())
}

#[cfg(test)]
mod tests {
    use dclutch_realm_contract::RealmV1Input;
    use dclutch_token_svm::{LEGACY_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, state::MINT_BYTES};
    use std::{boxed::Box, vec, vec::Vec};

    use super::*;

    const MINT_AUTHORITY_OFFSET: usize = 0;
    const MINT_DECIMALS_OFFSET: usize = 44;
    const MINT_INITIALIZED_OFFSET: usize = 45;
    const MINT_FREEZE_AUTHORITY_OFFSET: usize = 46;

    fn leak_account(
        key: Pubkey,
        signer: bool,
        writable: bool,
        lamports: u64,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(lamports)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn mint_data(mint_authority: bool, freeze_authority: bool) -> Vec<u8> {
        let mut data = vec![0; MINT_BYTES];
        if let Some(decimals) = data.get_mut(MINT_DECIMALS_OFFSET) {
            *decimals = 6;
        }
        if let Some(initialized) = data.get_mut(MINT_INITIALIZED_OFFSET) {
            *initialized = 1;
        }
        if mint_authority {
            put(&mut data, MINT_AUTHORITY_OFFSET, &1_u32.to_le_bytes());
            put(&mut data, MINT_AUTHORITY_OFFSET + 4, &[7; 32]);
        }
        if freeze_authority {
            put(
                &mut data,
                MINT_FREEZE_AUTHORITY_OFFSET,
                &1_u32.to_le_bytes(),
            );
            put(&mut data, MINT_FREEZE_AUTHORITY_OFFSET + 4, &[8; 32]);
        }
        data
    }

    fn release_for(program: [u8; 32]) -> CollateralAdapterReleaseV1 {
        PRODUCTION_ADAPTER_RELEASES
            .iter()
            .find(|release| release.token_program() == program)
            .copied()
            .expect("profile release")
    }

    fn realm(
        token_program: [u8; 32],
        mint: Pubkey,
        mint_policy: MintAuthorityPolicy,
        freeze_policy: FreezeAuthorityPolicy,
    ) -> RealmV1 {
        let release = release_for(token_program);
        RealmV1::new(RealmV1Input {
            collateral_semantic_id: [9; 32],
            token_program,
            collateral_mint: mint.to_bytes(),
            collateral_adapter_release_id: hash(&release.to_bytes()).to_bytes(),
            mint_authority_policy: mint_policy,
            freeze_authority_policy: freeze_policy,
        })
        .expect("valid Realm")
    }

    fn accounts(
        program_id: Pubkey,
        realm: RealmV1,
        mint_data: Vec<u8>,
    ) -> [AccountInfo<'static>; CREATE_REALM_ACCOUNTS] {
        let digest = hash(&realm.to_bytes()).to_bytes();
        let (realm_key, _) =
            Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &digest], &program_id);
        let token_program = Pubkey::new_from_array(*realm.token_program());
        let mint = Pubkey::new_from_array(*realm.collateral_mint());
        let mut output = [
            leak_account(
                Pubkey::new_unique(),
                true,
                true,
                10_000_000,
                vec![],
                system_program::ID,
                false,
            ),
            leak_account(realm_key, false, true, 0, vec![], system_program::ID, false),
            leak_account(mint, false, false, 1, mint_data, token_program, false),
            leak_account(token_program, false, false, 1, vec![], bpf_loader::ID, true),
            leak_account(
                system_program::ID,
                false,
                false,
                1,
                vec![],
                native_loader::ID,
                true,
            ),
            leak_account(
                sysvar::rent::ID,
                false,
                false,
                1,
                vec![0; Rent::size_of()],
                sysvar::ID,
                false,
            ),
        ];
        if let Some(rent_account) = output.get_mut(5) {
            assert_eq!(Rent::default().to_account_info(rent_account), Some(()));
        }
        output
    }

    fn authenticate(
        program_id: &Pubkey,
        accounts: &[AccountInfo<'_>],
        realm: RealmV1,
    ) -> Result<CreateRealmPlan, ProgramError> {
        let frame = CreateRealmFrame::parse(accounts)?;
        authenticate_create_realm(program_id, &frame, realm)
    }

    #[test]
    fn both_exact_profiles_authenticate_and_bind_content_address() {
        for token_program in [LEGACY_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID] {
            let program_id = Pubkey::new_unique();
            let mint_key = Pubkey::new_unique();
            let realm = realm(
                token_program,
                mint_key,
                MintAuthorityPolicy::RequireAbsent,
                FreezeAuthorityPolicy::RequireAbsent,
            );
            let accounts = accounts(program_id, realm, mint_data(false, false));
            let plan = authenticate(&program_id, &accounts, realm).expect("authenticated Realm");
            assert_eq!(plan.realm, realm);
            assert_eq!(plan.realm_digest, hash(&realm.to_bytes()).to_bytes());
            assert_eq!(
                plan.rent_lamports,
                Rent::default().minimum_balance(REALM_BYTES)
            );
        }
    }

    #[test]
    fn authority_policies_are_explicit_not_global_restrictions() {
        let program_id = Pubkey::new_unique();
        let mint_key = Pubkey::new_unique();
        let admitted = realm(
            LEGACY_TOKEN_PROGRAM_ID,
            mint_key,
            MintAuthorityPolicy::AdmitIssuerControl,
            FreezeAuthorityPolicy::AdmitIssuerControl,
        );
        let admitted_accounts = accounts(program_id, admitted, mint_data(true, true));
        assert!(authenticate(&program_id, &admitted_accounts, admitted).is_ok());

        let refused = realm(
            LEGACY_TOKEN_PROGRAM_ID,
            mint_key,
            MintAuthorityPolicy::RequireAbsent,
            FreezeAuthorityPolicy::RequireAbsent,
        );
        let refused_accounts = accounts(program_id, refused, mint_data(true, true));
        assert_eq!(
            authenticate(&program_id, &refused_accounts, refused).err(),
            Some(ProgramError::from(AdapterError::RealmAuthentication))
        );
    }

    #[test]
    fn wrong_release_pda_owner_existing_state_and_aliases_refuse() {
        let program_id = Pubkey::new_unique();
        let mint_key = Pubkey::new_unique();
        let canonical = realm(
            LEGACY_TOKEN_PROGRAM_ID,
            mint_key,
            MintAuthorityPolicy::RequireAbsent,
            FreezeAuthorityPolicy::RequireAbsent,
        );

        let mut input = canonical.to_bytes();
        if let Some(release_byte) = input.get_mut(112) {
            *release_byte ^= 1;
        }
        let wrong_release = RealmV1::decode(&input).expect("still canonical Realm shape");
        let wrong_release_accounts = accounts(program_id, wrong_release, mint_data(false, false));
        assert_eq!(
            authenticate(&program_id, &wrong_release_accounts, wrong_release).err(),
            Some(ProgramError::from(AdapterError::RealmAuthentication))
        );

        let wrong_mint_owner = accounts(program_id, canonical, mint_data(false, false));
        if let Some(mint_account) = wrong_mint_owner.get(2) {
            mint_account.assign(&Pubkey::new_unique());
        }
        assert_eq!(
            authenticate(&program_id, &wrong_mint_owner, canonical).err(),
            Some(ProgramError::from(AdapterError::RealmAuthentication))
        );

        let existing = accounts(program_id, canonical, mint_data(false, false));
        if let Some(realm_account) = existing.get(1) {
            **realm_account
                .try_borrow_mut_lamports()
                .expect("fixture lamports") = 1;
        }
        assert_eq!(
            authenticate(&program_id, &existing, canonical).err(),
            Some(ProgramError::from(AdapterError::RealmAuthentication))
        );

        let mut aliased = accounts(program_id, canonical, mint_data(false, false));
        let sponsor = aliased.first().cloned().expect("sponsor");
        if let Some(realm_account) = aliased.get_mut(1) {
            *realm_account = sponsor;
        }
        assert_eq!(
            CreateRealmFrame::parse(&aliased).err(),
            Some(ProgramError::from(AdapterError::AccountIdentity))
        );
    }

    #[test]
    fn hostile_frame_length_and_missing_privilege_refuse_before_authentication() {
        let program_id = Pubkey::new_unique();
        let mint_key = Pubkey::new_unique();
        let realm = realm(
            LEGACY_TOKEN_PROGRAM_ID,
            mint_key,
            MintAuthorityPolicy::RequireAbsent,
            FreezeAuthorityPolicy::RequireAbsent,
        );
        let canonical_accounts = accounts(program_id, realm, mint_data(false, false));
        let short = canonical_accounts.get(..5).expect("fixture prefix");
        assert_eq!(
            CreateRealmFrame::parse(short).err(),
            Some(ProgramError::from(AdapterError::AccountFrameLength))
        );

        let mut missing_signer = accounts(program_id, realm, mint_data(false, false));
        if let Some(sponsor) = missing_signer.first_mut() {
            sponsor.is_signer = false;
        }
        assert_eq!(
            CreateRealmFrame::parse(&missing_signer).err(),
            Some(ProgramError::from(AdapterError::AccountPrivilege))
        );
    }

    fn put(output: &mut [u8], offset: usize, input: &[u8]) {
        for (destination, source) in output.iter_mut().skip(offset).zip(input) {
            *destination = *source;
        }
    }
}
