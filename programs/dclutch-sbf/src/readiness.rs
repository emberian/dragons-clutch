//! SVM adapter for canonical Market-opening capability readiness.
//!
//! The pure capability contract owns exact instruction decoding, ordered
//! frames, Market-child replay, funding readiness, and derived sealing.  This
//! module owns only the SVM boundary: account identity, canonical byte views,
//! SHA-256 content binding, PDA derivation, current Rent/Clock observations,
//! System creation, and atomic persistence.

use dclutch_capability_contract::{
    CapabilityFundingAuthorityDerivationV1, CapabilityFundingDerivationV1,
    CapabilityFundingVaultDerivationV1, CapabilityManifestV1, FUNDING_STATE_BYTES,
    FundingCustodyObservationV1, FundingStateV1, MARKET_OPENING_READINESS_BYTES,
    MARKET_OPENING_READINESS_PDA_DOMAIN, MarketOpeningReadinessV1, RealmCollateralCustodyV1,
    RealmCollateralVaultObservationV1,
    readiness_frame::{
        AdvanceMarketOpeningReadinessFrameV1, AdvanceMarketOpeningReadinessObservationV1,
        AuthenticatedRentCreditBeneficiaryV1, BeginMarketOpeningReadinessFrameV1,
        ReadinessAccountMetaV1, advance_market_opening_readiness, begin_market_opening_readiness,
    },
    readiness_instruction::{
        AdvanceMarketOpeningReadinessV1, BeginMarketOpeningReadinessV1, ReadinessInstructionV1,
    },
};
use dclutch_core_contract::MarketRoot;
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_rent_contract::{
    RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1,
};
use dclutch_token_svm::{AccountState, Mint, TokenAccount, TokenProgram};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;

use crate::AdapterError;

const BEGIN_ACCOUNTS: usize = 7;
const ADVANCE_NATIVE_ACCOUNTS: usize = 5;
const ADVANCE_REALM_ACCOUNTS: usize = 9;

/// Decode one exact readiness wire and execute the selected canonical route.
pub(crate) fn dispatch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    match ReadinessInstructionV1::decode(instruction_data)
        .map_err(|_| AdapterError::InvalidInstruction)?
    {
        ReadinessInstructionV1::Begin(instruction) => {
            process_begin(program_id, accounts, instruction)
        }
        ReadinessInstructionV1::Advance(instruction) => {
            process_advance(program_id, accounts, instruction)
        }
    }
}

struct BeginFrame<'a, 'info> {
    sponsor: &'a AccountInfo<'info>,
    market: &'a AccountInfo<'info>,
    readiness: &'a AccountInfo<'info>,
    manifest: &'a AccountInfo<'info>,
    rent_credit: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    rent_sysvar: &'a AccountInfo<'info>,
}

impl<'a, 'info> BeginFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != BEGIN_ACCOUNTS {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            sponsor: account(accounts, 0)?,
            market: account(accounts, 1)?,
            readiness: account(accounts, 2)?,
            manifest: account(accounts, 3)?,
            rent_credit: account(accounts, 4)?,
            system_program: account(accounts, 5)?,
            rent_sysvar: account(accounts, 6)?,
        };
        BeginMarketOpeningReadinessFrameV1::new([
            meta(frame.sponsor),
            meta(frame.market),
            meta(frame.readiness),
            meta(frame.manifest),
            meta(frame.rent_credit),
            meta(frame.system_program),
            meta(frame.rent_sysvar),
        ])
        .map_err(map_frame_error)?;
        Ok(frame)
    }

    fn contract_frame(&self) -> Result<BeginMarketOpeningReadinessFrameV1, ProgramError> {
        BeginMarketOpeningReadinessFrameV1::new([
            meta(self.sponsor),
            meta(self.market),
            meta(self.readiness),
            meta(self.manifest),
            meta(self.rent_credit),
            meta(self.system_program),
            meta(self.rent_sysvar),
        ])
        .map_err(map_frame_error)
    }
}

struct AdvanceFrame<'a, 'info> {
    market: &'a AccountInfo<'info>,
    readiness: &'a AccountInfo<'info>,
    manifest: &'a AccountInfo<'info>,
    funding: &'a AccountInfo<'info>,
    rent_sysvar: &'a AccountInfo<'info>,
    realm_custody: Option<RealmCustodyFrame<'a, 'info>>,
}

struct RealmCustodyFrame<'a, 'info> {
    authority: &'a AccountInfo<'info>,
    vault: &'a AccountInfo<'info>,
    mint: &'a AccountInfo<'info>,
    token_program: &'a AccountInfo<'info>,
}

impl<'a, 'info> AdvanceFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        match accounts.len() {
            ADVANCE_NATIVE_ACCOUNTS => {
                let frame = Self {
                    market: account(accounts, 0)?,
                    readiness: account(accounts, 1)?,
                    manifest: account(accounts, 2)?,
                    funding: account(accounts, 3)?,
                    rent_sysvar: account(accounts, 4)?,
                    realm_custody: None,
                };
                frame.contract_frame()?;
                Ok(frame)
            }
            ADVANCE_REALM_ACCOUNTS => {
                let frame = Self {
                    market: account(accounts, 0)?,
                    readiness: account(accounts, 1)?,
                    manifest: account(accounts, 2)?,
                    funding: account(accounts, 3)?,
                    rent_sysvar: account(accounts, 4)?,
                    realm_custody: Some(RealmCustodyFrame {
                        authority: account(accounts, 5)?,
                        vault: account(accounts, 6)?,
                        mint: account(accounts, 7)?,
                        token_program: account(accounts, 8)?,
                    }),
                };
                frame.contract_frame()?;
                Ok(frame)
            }
            _ => Err(AdapterError::AccountFrameLength.into()),
        }
    }

    fn contract_frame(&self) -> Result<AdvanceMarketOpeningReadinessFrameV1, ProgramError> {
        match self.realm_custody.as_ref() {
            None => AdvanceMarketOpeningReadinessFrameV1::native([
                meta(self.market),
                meta(self.readiness),
                meta(self.manifest),
                meta(self.funding),
                meta(self.rent_sysvar),
            ]),
            Some(custody) => AdvanceMarketOpeningReadinessFrameV1::realm([
                meta(self.market),
                meta(self.readiness),
                meta(self.manifest),
                meta(self.funding),
                meta(self.rent_sysvar),
                meta(custody.authority),
                meta(custody.vault),
                meta(custody.mint),
                meta(custody.token_program),
            ]),
        }
        .map_err(map_frame_error)
    }
}

#[derive(Clone, Copy)]
struct BeginPlan {
    root: MarketRoot,
    readiness: MarketOpeningReadinessV1,
    outcome_count: u8,
    readiness_bump: u8,
    readiness_rent: u64,
    sponsor_before: u64,
    market_lamports: u64,
    rent_credit: RentCreditV1,
    rent_credit_lamports: u64,
}

#[derive(Clone, Copy)]
struct AdvancePlan {
    readiness: MarketOpeningReadinessV1,
    market_root: MarketRoot,
    outcome_count: u8,
    funding: FundingStateV1,
    readiness_lamports: u64,
    market_lamports: u64,
    funding_lamports: u64,
}

/// Atomically create and persist the one canonical readiness direct child.
pub(crate) fn process_begin(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: BeginMarketOpeningReadinessV1,
) -> Result<(), ProgramError> {
    let frame = BeginFrame::parse(accounts)?;
    let plan = authenticate_begin(program_id, &frame, instruction)?;

    let space =
        u64::try_from(MARKET_OPENING_READINESS_BYTES).map_err(|_| AdapterError::Arithmetic)?;
    let create = create_account(
        frame.sponsor.key,
        frame.readiness.key,
        plan.readiness_rent,
        space,
        program_id,
    );
    let generation = instruction.generation().to_le_bytes();
    let bump = [plan.readiness_bump];
    let signer = [
        MARKET_OPENING_READINESS_PDA_DOMAIN,
        frame.market.key.as_ref(),
        generation.as_slice(),
        bump.as_slice(),
    ];
    invoke_signed(
        &create,
        &[
            frame.sponsor.clone(),
            frame.readiness.clone(),
            frame.system_program.clone(),
        ],
        &[&signer],
    )
    .map_err(|_| AdapterError::ReadinessCreateCpi)?;

    persist_begin(program_id, &frame, instruction, plan)
}

/// Atomically validate and persist one permissionless readiness advance.
pub(crate) fn process_advance(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: AdvanceMarketOpeningReadinessV1,
) -> Result<(), ProgramError> {
    let frame = AdvanceFrame::parse(accounts)?;
    let plan = authenticate_advance(program_id, &frame, instruction)?;
    persist_advance(program_id, &frame, plan)
}

#[inline(never)]
fn authenticate_begin(
    program_id: &Pubkey,
    frame: &BeginFrame<'_, '_>,
    instruction: BeginMarketOpeningReadinessV1,
) -> Result<BeginPlan, ProgramError> {
    authenticate_begin_identities(program_id, frame)?;
    let (outcome_count, root) = authenticate_market(frame.market)?;
    let rent_credit =
        authenticate_sponsor_rent_credit(program_id, frame.rent_credit, frame.sponsor)?;
    let generation = instruction.generation().to_le_bytes();
    let (expected_readiness, readiness_bump) = Pubkey::find_program_address(
        &[
            MARKET_OPENING_READINESS_PDA_DOMAIN,
            frame.market.key.as_ref(),
            generation.as_slice(),
        ],
        program_id,
    );
    if frame.readiness.key != &expected_readiness {
        return Err(AdapterError::AccountIdentity.into());
    }
    let rent = Rent::from_account_info(frame.rent_sysvar)
        .map_err(|_| AdapterError::ReadinessAuthentication)?;
    let readiness_rent = rent.minimum_balance(MARKET_OPENING_READINESS_BYTES);
    if frame.sponsor.lamports() < readiness_rent {
        return Err(AdapterError::FundUnderfunded.into());
    }
    let beneficiary =
        AuthenticatedRentCreditBeneficiaryV1::new(rent_credit.refund_authority().to_bytes())
            .map_err(|_| AdapterError::ReadinessAuthentication)?;
    let (root, readiness) = {
        let manifest_data = frame
            .manifest
            .try_borrow_data()
            .map_err(|_| AdapterError::ReadinessAuthentication)?;
        let manifest = authenticate_manifest(&manifest_data, root)?;
        let contract_plan = begin_market_opening_readiness(
            root,
            instruction,
            frame.contract_frame()?,
            manifest,
            beneficiary,
        )
        .map_err(|_| AdapterError::ReadinessAuthentication)?;
        if contract_plan.readiness_pda_seeds().domain() != MARKET_OPENING_READINESS_PDA_DOMAIN
            || contract_plan.readiness_pda_seeds().market() != frame.market.key.to_bytes()
            || contract_plan.readiness_pda_seeds().generation_le_bytes() != generation
        {
            return Err(AdapterError::ReadinessAuthentication.into());
        }
        if hash(contract_plan.manifest_commitment().manifest().as_bytes()).to_bytes()
            != contract_plan.manifest_commitment().content_id().to_bytes()
        {
            return Err(AdapterError::ContentIdentity.into());
        }
        (contract_plan.root(), contract_plan.readiness())
    };

    preflight_mutable(frame.sponsor)?;
    preflight_mutable(frame.market)?;
    preflight_mutable(frame.readiness)?;
    drop(
        frame
            .market
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::ReadinessAuthentication)?,
    );
    drop(
        frame
            .readiness
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::ReadinessAuthentication)?,
    );

    Ok(BeginPlan {
        root,
        readiness,
        outcome_count,
        readiness_bump,
        readiness_rent,
        sponsor_before: frame.sponsor.lamports(),
        market_lamports: frame.market.lamports(),
        rent_credit,
        rent_credit_lamports: frame.rent_credit.lamports(),
    })
}

#[inline(never)]
fn authenticate_advance(
    program_id: &Pubkey,
    frame: &AdvanceFrame<'_, '_>,
    instruction: AdvanceMarketOpeningReadinessV1,
) -> Result<AdvancePlan, ProgramError> {
    if frame.market.owner != program_id
        || frame.market.executable
        || frame.readiness.owner != program_id
        || frame.readiness.executable
        || frame.readiness.data_len() != MARKET_OPENING_READINESS_BYTES
        || frame.manifest.owner != program_id
        || frame.manifest.executable
        || frame.funding.owner != program_id
        || frame.funding.executable
        || frame.funding.data_len() != FUNDING_STATE_BYTES
        || frame.rent_sysvar.key != &sysvar::rent::ID
        || frame.rent_sysvar.owner != &sysvar::ID
        || frame.rent_sysvar.is_signer
        || frame.rent_sysvar.is_writable
        || frame.rent_sysvar.executable
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let rent = Rent::from_account_info(frame.rent_sysvar)
        .map_err(|_| AdapterError::ReadinessAuthentication)?;
    let (outcome_count, root) = authenticate_market(frame.market)?;
    let generation = instruction.generation().to_le_bytes();
    let (expected_readiness, _) = Pubkey::find_program_address(
        &[
            MARKET_OPENING_READINESS_PDA_DOMAIN,
            frame.market.key.as_ref(),
            generation.as_slice(),
        ],
        program_id,
    );
    if frame.readiness.key != &expected_readiness {
        return Err(AdapterError::AccountIdentity.into());
    }
    let readiness_data = frame
        .readiness
        .try_borrow_data()
        .map_err(|_| AdapterError::ReadinessAuthentication)?;
    let readiness =
        MarketOpeningReadinessV1::decode(&readiness_data).map_err(|_| AdapterError::AccountData)?;
    if readiness.to_bytes().as_slice() != &readiness_data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    drop(readiness_data);
    let funding_data = frame
        .funding
        .try_borrow_data()
        .map_err(|_| AdapterError::ReadinessAuthentication)?;
    let funding = FundingStateV1::decode(&funding_data).map_err(|_| AdapterError::AccountData)?;
    if funding.to_bytes().as_slice() != &funding_data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    drop(funding_data);
    let current_slot = Clock::get()
        .map_err(|_| AdapterError::ReadinessAuthentication)?
        .slot;
    let next_readiness = {
        let manifest_data = frame
            .manifest
            .try_borrow_data()
            .map_err(|_| AdapterError::ReadinessAuthentication)?;
        let manifest = authenticate_manifest(&manifest_data, root)?;
        let funding_derivation = CapabilityFundingDerivationV1::new(
            frame.market.key.to_bytes(),
            instruction.generation(),
            root.identity().capability_manifest_id(),
            manifest,
            funding,
        )
        .map_err(|_| AdapterError::ReadinessAuthentication)?;
        let funding_seeds = funding_derivation.seed_components();
        let (expected_funding, _) = Pubkey::find_program_address(&funding_seeds, program_id);
        if frame.funding.key != &expected_funding {
            return Err(AdapterError::AccountIdentity.into());
        }
        let custody =
            authenticate_funding_custody(program_id, frame, root, manifest, funding, &rent)?;
        let contract_plan = advance_market_opening_readiness(
            instruction,
            frame.contract_frame()?,
            AdvanceMarketOpeningReadinessObservationV1::new(
                root,
                readiness,
                manifest,
                funding,
                custody,
                current_slot,
            ),
        )
        .map_err(|_| AdapterError::ReadinessAuthentication)?;
        if hash(contract_plan.manifest_commitment().manifest().as_bytes()).to_bytes()
            != contract_plan.manifest_commitment().content_id().to_bytes()
        {
            return Err(AdapterError::ContentIdentity.into());
        }
        contract_plan.readiness()
    };
    preflight_mutable(frame.readiness)?;
    drop(
        frame
            .readiness
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::ReadinessAuthentication)?,
    );
    Ok(AdvancePlan {
        readiness: next_readiness,
        market_root: root,
        outcome_count,
        funding,
        readiness_lamports: frame.readiness.lamports(),
        market_lamports: frame.market.lamports(),
        funding_lamports: frame.funding.lamports(),
    })
}

/// Authenticate the independent native and optional Realm-token custody facts
/// named by the selected immutable capability quote.
fn authenticate_funding_custody(
    program_id: &Pubkey,
    frame: &AdvanceFrame<'_, '_>,
    root: MarketRoot,
    manifest: CapabilityManifestV1<'_>,
    funding: FundingStateV1,
    rent: &Rent,
) -> Result<FundingCustodyObservationV1, ProgramError> {
    let state_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
    let entry = manifest
        .entry(funding.entry_index())
        .map_err(|_| AdapterError::ReadinessAuthentication)?;
    let quote = entry.funding_quote();
    match (quote.realm_collateral(), frame.realm_custody.as_ref()) {
        (None, None) => {
            FundingCustodyObservationV1::native_only(frame.funding.lamports(), state_rent)
                .map_err(|_| AdapterError::FundUnderfunded.into())
        }
        (None, Some(_)) | (Some(_), None) => Err(AdapterError::ReadinessAuthentication.into()),
        (Some(binding), Some(custody_frame)) => {
            if binding.realm_id() != root.identity().realm_id() {
                return Err(AdapterError::ContentIdentity.into());
            }
            let authority_derivation =
                CapabilityFundingAuthorityDerivationV1::new(frame.funding.key.to_bytes())
                    .map_err(|_| AdapterError::ReadinessAuthentication)?;
            let (expected_authority, _) =
                Pubkey::find_program_address(&authority_derivation.seed_components(), program_id);
            if custody_frame.authority.key != &expected_authority {
                return Err(AdapterError::AccountIdentity.into());
            }
            let vault_derivation =
                CapabilityFundingVaultDerivationV1::new(expected_authority.to_bytes(), binding)
                    .map_err(|_| AdapterError::ReadinessAuthentication)?;
            let (expected_vault, _) =
                Pubkey::find_program_address(&vault_derivation.seed_components(), program_id);
            if custody_frame.vault.key != &expected_vault
                || custody_frame.mint.key.to_bytes() != binding.mint()
                || custody_frame.token_program.key.to_bytes() != binding.token_program()
                || custody_frame.vault.owner != custody_frame.token_program.key
                || custody_frame.mint.owner != custody_frame.token_program.key
            {
                return Err(AdapterError::AccountIdentity.into());
            }
            TokenProgram::parse(custody_frame.token_program.key.to_bytes())
                .map_err(|_| AdapterError::ReadinessAuthentication)?;
            let mint_data = custody_frame
                .mint
                .try_borrow_data()
                .map_err(|_| AdapterError::ReadinessAuthentication)?;
            let mint =
                Mint::parse(&mint_data).map_err(|_| AdapterError::ReadinessAuthentication)?;
            if !mint.is_initialized {
                return Err(AdapterError::ReadinessAuthentication.into());
            }
            drop(mint_data);
            let vault_data = custody_frame
                .vault
                .try_borrow_data()
                .map_err(|_| AdapterError::ReadinessAuthentication)?;
            let vault = TokenAccount::parse(&vault_data)
                .map_err(|_| AdapterError::ReadinessAuthentication)?;
            if vault.mint != binding.mint()
                || vault.owner != expected_authority.to_bytes()
                || vault.state != AccountState::Initialized
                || !vault.delegate.is_none()
                || vault.delegated_amount != 0
                || !vault.native_reserve.is_none()
                || !vault.close_authority.is_none()
            {
                return Err(AdapterError::ReadinessAuthentication.into());
            }
            let vault_rent = rent.minimum_balance(vault_data.len());
            drop(vault_data);
            let observation = RealmCollateralVaultObservationV1::new(
                expected_vault.to_bytes(),
                expected_authority.to_bytes(),
                binding.token_program(),
                binding.mint(),
                vault.amount,
                custody_frame.vault.lamports(),
                vault_rent,
            )
            .map_err(|_| AdapterError::ReadinessAuthentication)?;
            let realm = RealmCollateralCustodyV1::new(
                root.identity().realm_id(),
                binding.collateral_release_id(),
                expected_authority.to_bytes(),
                expected_vault.to_bytes(),
                observation,
            )
            .map_err(|_| AdapterError::ReadinessAuthentication)?;
            FundingCustodyObservationV1::with_realm_collateral(
                frame.funding.lamports(),
                state_rent,
                realm,
            )
            .map_err(|_| AdapterError::FundUnderfunded.into())
        }
    }
}

fn authenticate_begin_identities(
    program_id: &Pubkey,
    frame: &BeginFrame<'_, '_>,
) -> Result<(), ProgramError> {
    if frame.sponsor.owner != &system_program::ID
        || !frame.sponsor.data_is_empty()
        || frame.market.owner != program_id
        || frame.market.executable
        || frame.readiness.owner != &system_program::ID
        || frame.readiness.executable
        || !frame.readiness.data_is_empty()
        || frame.readiness.lamports() != 0
        || frame.manifest.owner != program_id
        || frame.manifest.executable
        || frame.system_program.key != &system_program::ID
        || frame.system_program.owner != &native_loader::ID
        || !frame.system_program.executable
        || frame.rent_sysvar.key != &sysvar::rent::ID
        || frame.rent_sysvar.owner != &sysvar::ID
        || frame.rent_sysvar.executable
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    Rent::from_account_info(frame.rent_sysvar)
        .map_err(|_| AdapterError::ReadinessAuthentication)?;
    Ok(())
}

fn authenticate_market(market: &AccountInfo<'_>) -> Result<(u8, MarketRoot), ProgramError> {
    let data = market
        .try_borrow_data()
        .map_err(|_| AdapterError::ReadinessAuthentication)?;
    let outcome_count =
        decode_market_outcome_count(&data).map_err(|_| AdapterError::ReadinessAuthentication)?;
    let root = decode_selected_market(outcome_count, &data)?;
    Ok((outcome_count, root))
}

fn authenticate_manifest<'a>(
    data: &'a [u8],
    root: MarketRoot,
) -> Result<CapabilityManifestV1<'a>, ProgramError> {
    let manifest = CapabilityManifestV1::decode(data).map_err(|_| AdapterError::AccountData)?;
    if manifest.as_bytes() != data
        || hash(manifest.as_bytes()).to_bytes()
            != root.identity().capability_manifest_id().to_bytes()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    // `CapabilityManifestV1` borrows exact account data.  The caller must
    // finish its pure transition before any mutable borrow of this account.
    Ok(manifest)
}

fn authenticate_sponsor_rent_credit(
    program_id: &Pubkey,
    rent_credit_account: &AccountInfo<'_>,
    sponsor: &AccountInfo<'_>,
) -> Result<RentCreditV1, ProgramError> {
    let authority = RefundAuthority::new(sponsor.key.to_bytes())
        .map_err(|_| AdapterError::ReadinessAuthentication)?;
    let authority_bytes = authority.to_bytes();
    let (expected_credit, bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, authority_bytes.as_slice()],
        program_id,
    );
    if rent_credit_account.key != &expected_credit
        || rent_credit_account.owner != program_id
        || rent_credit_account.executable
        || rent_credit_account.data_len() != RENT_CREDIT_BYTES_V1
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = rent_credit_account
        .try_borrow_data()
        .map_err(|_| AdapterError::ReadinessAuthentication)?;
    let credit = RentCreditV1::decode(&data).map_err(|_| AdapterError::AccountData)?;
    credit
        .validate_binding(authority, bump)
        .map_err(|_| AdapterError::ReadinessAuthentication)?;
    if credit.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(credit)
}

fn persist_begin(
    program_id: &Pubkey,
    frame: &BeginFrame<'_, '_>,
    instruction: BeginMarketOpeningReadinessV1,
    plan: BeginPlan,
) -> Result<(), ProgramError> {
    let sponsor_after = plan
        .sponsor_before
        .checked_sub(plan.readiness_rent)
        .ok_or(AdapterError::Arithmetic)?;
    if frame.sponsor.lamports() != sponsor_after
        || frame.market.owner != program_id
        || frame.market.lamports() != plan.market_lamports
        || frame.readiness.owner != program_id
        || frame.readiness.lamports() != plan.readiness_rent
        || frame.readiness.data_len() != MARKET_OPENING_READINESS_BYTES
        || frame.rent_credit.lamports() != plan.rent_credit_lamports
    {
        return Err(AdapterError::ReadinessPostcondition.into());
    }
    require_unchanged_rent_credit(program_id, frame.rent_credit, plan.rent_credit)?;
    {
        let mut readiness_data = frame
            .readiness
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::ReadinessPostcondition)?;
        readiness_data.copy_from_slice(&plan.readiness.to_bytes());
        if MarketOpeningReadinessV1::decode(&readiness_data) != Ok(plan.readiness) {
            return Err(AdapterError::ReadinessPostcondition.into());
        }
    }
    {
        let mut market_data = frame
            .market
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::ReadinessPostcondition)?;
        replace_selected_market_root(plan.outcome_count, &mut market_data, plan.root)?;
    }
    let generation = instruction.generation().to_le_bytes();
    let (expected_readiness, expected_bump) = Pubkey::find_program_address(
        &[
            MARKET_OPENING_READINESS_PDA_DOMAIN,
            frame.market.key.as_ref(),
            generation.as_slice(),
        ],
        program_id,
    );
    if frame.readiness.key != &expected_readiness || plan.readiness_bump != expected_bump {
        return Err(AdapterError::ReadinessPostcondition.into());
    }
    let (_, persisted_root) = authenticate_market(frame.market)?;
    if persisted_root != plan.root {
        return Err(AdapterError::ReadinessPostcondition.into());
    }
    Ok(())
}

fn persist_advance(
    program_id: &Pubkey,
    frame: &AdvanceFrame<'_, '_>,
    plan: AdvancePlan,
) -> Result<(), ProgramError> {
    if frame.market.owner != program_id
        || frame.market.lamports() != plan.market_lamports
        || frame.readiness.owner != program_id
        || frame.readiness.lamports() != plan.readiness_lamports
        || frame.readiness.data_len() != MARKET_OPENING_READINESS_BYTES
        || frame.funding.owner != program_id
        || frame.funding.lamports() != plan.funding_lamports
        || frame.funding.data_len() != FUNDING_STATE_BYTES
    {
        return Err(AdapterError::ReadinessPostcondition.into());
    }
    let (outcome_count, root) = authenticate_market(frame.market)?;
    if outcome_count != plan.outcome_count || root != plan.market_root {
        return Err(AdapterError::ReadinessPostcondition.into());
    }
    let funding_data = frame
        .funding
        .try_borrow_data()
        .map_err(|_| AdapterError::ReadinessPostcondition)?;
    if FundingStateV1::decode(&funding_data) != Ok(plan.funding)
        || plan.funding.to_bytes().as_slice() != &funding_data[..]
    {
        return Err(AdapterError::ReadinessPostcondition.into());
    }
    drop(funding_data);
    let mut readiness_data = frame
        .readiness
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::ReadinessPostcondition)?;
    readiness_data.copy_from_slice(&plan.readiness.to_bytes());
    if MarketOpeningReadinessV1::decode(&readiness_data) != Ok(plan.readiness) {
        return Err(AdapterError::ReadinessPostcondition.into());
    }
    Ok(())
}

fn require_unchanged_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected: RentCreditV1,
) -> Result<(), ProgramError> {
    if account.owner != program_id
        || account.executable
        || account.data_len() != RENT_CREDIT_BYTES_V1
    {
        return Err(AdapterError::ReadinessPostcondition.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::ReadinessPostcondition)?;
    if RentCreditV1::decode(&data) != Ok(expected) || expected.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::ReadinessPostcondition.into());
    }
    Ok(())
}

fn decode_selected_market(outcome_count: u8, data: &[u8]) -> Result<MarketRoot, ProgramError> {
    match outcome_count {
        2 => CategoricalMarketV1::<2>::decode(data).map(|market| market.root()),
        3 => CategoricalMarketV1::<3>::decode(data).map(|market| market.root()),
        4 => CategoricalMarketV1::<4>::decode(data).map(|market| market.root()),
        5 => CategoricalMarketV1::<5>::decode(data).map(|market| market.root()),
        6 => CategoricalMarketV1::<6>::decode(data).map(|market| market.root()),
        7 => CategoricalMarketV1::<7>::decode(data).map(|market| market.root()),
        8 => CategoricalMarketV1::<8>::decode(data).map(|market| market.root()),
        9 => CategoricalMarketV1::<9>::decode(data).map(|market| market.root()),
        10 => CategoricalMarketV1::<10>::decode(data).map(|market| market.root()),
        11 => CategoricalMarketV1::<11>::decode(data).map(|market| market.root()),
        12 => CategoricalMarketV1::<12>::decode(data).map(|market| market.root()),
        13 => CategoricalMarketV1::<13>::decode(data).map(|market| market.root()),
        14 => CategoricalMarketV1::<14>::decode(data).map(|market| market.root()),
        15 => CategoricalMarketV1::<15>::decode(data).map(|market| market.root()),
        16 => CategoricalMarketV1::<16>::decode(data).map(|market| market.root()),
        _ => return Err(AdapterError::ReadinessAuthentication.into()),
    }
    .map_err(|_| AdapterError::ReadinessAuthentication.into())
}

fn replace_selected_market_root(
    outcome_count: u8,
    output: &mut [u8],
    root: MarketRoot,
) -> Result<(), ProgramError> {
    match outcome_count {
        2 => replace_market_root::<2>(output, root),
        3 => replace_market_root::<3>(output, root),
        4 => replace_market_root::<4>(output, root),
        5 => replace_market_root::<5>(output, root),
        6 => replace_market_root::<6>(output, root),
        7 => replace_market_root::<7>(output, root),
        8 => replace_market_root::<8>(output, root),
        9 => replace_market_root::<9>(output, root),
        10 => replace_market_root::<10>(output, root),
        11 => replace_market_root::<11>(output, root),
        12 => replace_market_root::<12>(output, root),
        13 => replace_market_root::<13>(output, root),
        14 => replace_market_root::<14>(output, root),
        15 => replace_market_root::<15>(output, root),
        16 => replace_market_root::<16>(output, root),
        _ => Err(AdapterError::ReadinessPostcondition.into()),
    }
}

fn replace_market_root<const N: usize>(
    output: &mut [u8],
    root: MarketRoot,
) -> Result<(), ProgramError> {
    let previous = CategoricalMarketV1::<N>::decode(output)
        .map_err(|_| AdapterError::ReadinessPostcondition)?;
    let next = CategoricalMarketV1::<N>::new(
        root,
        previous.hoard_atoms(),
        *previous.supply(),
        previous.settlement(),
    )
    .map_err(|_| AdapterError::ReadinessPostcondition)?;
    next.encode(output)
        .map_err(|_| AdapterError::ReadinessPostcondition)?;
    if CategoricalMarketV1::<N>::decode(output) != Ok(next) {
        return Err(AdapterError::ReadinessPostcondition.into());
    }
    Ok(())
}

fn preflight_mutable(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    drop(
        account
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::ReadinessAuthentication)?,
    );
    Ok(())
}

fn meta(account: &AccountInfo<'_>) -> ReadinessAccountMetaV1 {
    ReadinessAccountMetaV1 {
        key: account.key.to_bytes(),
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        is_executable: account.executable,
    }
}

fn map_frame_error(
    error: dclutch_capability_contract::readiness_frame::ReadinessFrameError,
) -> ProgramError {
    match error {
        dclutch_capability_contract::readiness_frame::ReadinessFrameError::InvalidAccountPrivilege => {
            AdapterError::AccountPrivilege.into()
        }
        dclutch_capability_contract::readiness_frame::ReadinessFrameError::AccountAlias
        | dclutch_capability_contract::readiness_frame::ReadinessFrameError::ZeroAccountKey => {
            AdapterError::AccountIdentity.into()
        }
        _ => AdapterError::ReadinessAuthentication.into(),
    }
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
#[allow(clippy::indexing_slicing)]
mod tests {
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
        FundingAmountsV1, FundingCustodyObservationV1, FundingQuoteV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_core_contract::{ContentId, MarketIdentity};
    use dclutch_market_contract::market::CategoricalSettlementSummaryV1;
    use solana_sdk_ids::bpf_loader;
    use std::{boxed::Box, vec, vec::Vec};

    use super::*;

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

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero test content")
    }

    fn manifest_bytes() -> Vec<u8> {
        let quote = FundingQuoteV1::new(
            FundingAmountsV1::new(
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::native_lamports(5).expect("work"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            )
            .expect("typed quote"),
            None,
        )
        .expect("quote");
        let entry = CapabilityEntryV1::new(
            id(20),
            id(21),
            id(22),
            id(23),
            id(24),
            id(25),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .expect("entry");
        let mut bytes = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut bytes).expect("manifest");
        bytes
    }

    fn rent_account() -> AccountInfo<'static> {
        let mut account = leak_account(
            sysvar::rent::ID,
            false,
            false,
            1,
            vec![0; Rent::size_of()],
            sysvar::ID,
            false,
        );
        assert_eq!(Rent::default().to_account_info(&mut account), Some(()));
        account
    }

    fn founding_market(
        program_id: Pubkey,
        sponsor: Pubkey,
        manifest: &[u8],
    ) -> (AccountInfo<'static>, u64) {
        let identity = MarketIdentity::new(
            id(1),
            id(2),
            id(3),
            id(4),
            ContentId::new(hash(manifest).to_bytes()).expect("manifest content"),
            7,
        );
        let root = MarketRoot::founding(identity, sponsor.to_bytes()).expect("founding root");
        let market =
            CategoricalMarketV1::<2>::new(root, 0, [0; 2], CategoricalSettlementSummaryV1::empty())
                .expect("market");
        let mut bytes = vec![0; CategoricalMarketV1::<2>::encoded_len().expect("size")];
        market.encode(&mut bytes).expect("encode market");
        (
            leak_account(
                Pubkey::new_unique(),
                false,
                true,
                1,
                bytes,
                program_id,
                false,
            ),
            7,
        )
    }

    fn begin_fixture(
        program_id: Pubkey,
    ) -> (Vec<AccountInfo<'static>>, BeginMarketOpeningReadinessV1) {
        let sponsor_key = Pubkey::new_unique();
        let manifest = manifest_bytes();
        let (market, generation) = founding_market(program_id, sponsor_key, &manifest);
        let generation_bytes = generation.to_le_bytes();
        let (readiness_key, _) = Pubkey::find_program_address(
            &[
                MARKET_OPENING_READINESS_PDA_DOMAIN,
                market.key.as_ref(),
                generation_bytes.as_slice(),
            ],
            &program_id,
        );
        let authority = RefundAuthority::new(sponsor_key.to_bytes()).expect("authority");
        let (credit_key, bump) = Pubkey::find_program_address(
            &[RENT_CREDIT_PDA_DOMAIN_V1, sponsor_key.as_ref()],
            &program_id,
        );
        let credit = RentCreditV1::new(authority, bump);
        let accounts = vec![
            leak_account(
                sponsor_key,
                true,
                true,
                100_000_000,
                vec![],
                system_program::ID,
                false,
            ),
            market,
            leak_account(
                readiness_key,
                false,
                true,
                0,
                vec![],
                system_program::ID,
                false,
            ),
            leak_account(
                Pubkey::new_unique(),
                false,
                false,
                1,
                manifest,
                program_id,
                false,
            ),
            leak_account(
                credit_key,
                false,
                false,
                1,
                credit.to_bytes().to_vec(),
                program_id,
                false,
            ),
            leak_account(
                system_program::ID,
                false,
                false,
                1,
                vec![],
                native_loader::ID,
                true,
            ),
            rent_account(),
        ];
        (accounts, BeginMarketOpeningReadinessV1::new(generation, 0))
    }

    fn advance_fixture(
        program_id: Pubkey,
    ) -> (Vec<AccountInfo<'static>>, AdvanceMarketOpeningReadinessV1) {
        let (begin_accounts, begin_instruction) = begin_fixture(program_id);
        let market = &begin_accounts[1];
        let sponsor = *begin_accounts[0].key;
        let (_, root) = authenticate_market(market).expect("market");
        let manifest_data = begin_accounts[3].try_borrow_data().expect("manifest data");
        let manifest = CapabilityManifestV1::decode(&manifest_data).expect("manifest");
        let manifest_id = root.identity().capability_manifest_id();
        let readiness = MarketOpeningReadinessV1::begin(
            market.key.to_bytes(),
            begin_instruction.generation(),
            manifest_id,
            manifest,
            sponsor.to_bytes(),
        )
        .expect("readiness");
        let custody = FundingCustodyObservationV1::native_only(5, 0).expect("custody");
        let mut funding = FundingStateV1::new(manifest_id, manifest, 0, custody).expect("funding");
        funding
            .activate(manifest_id, manifest, custody, 0)
            .expect("active funding");
        let funding_derivation = CapabilityFundingDerivationV1::new(
            market.key.to_bytes(),
            begin_instruction.generation(),
            manifest_id,
            manifest,
            funding,
        )
        .expect("funding derivation");
        let funding_seeds = funding_derivation.seed_components();
        let (funding_key, _) = Pubkey::find_program_address(&funding_seeds, &program_id);
        drop(manifest_data);
        let accounts = vec![
            leak_account(
                *market.key,
                false,
                false,
                market.lamports(),
                market.try_borrow_data().expect("market data").to_vec(),
                *market.owner,
                false,
            ),
            leak_account(
                *begin_accounts[2].key,
                false,
                true,
                1,
                readiness.to_bytes().to_vec(),
                program_id,
                false,
            ),
            leak_account(
                *begin_accounts[3].key,
                false,
                false,
                1,
                begin_accounts[3]
                    .try_borrow_data()
                    .expect("manifest data")
                    .to_vec(),
                program_id,
                false,
            ),
            leak_account(
                funding_key,
                false,
                false,
                5,
                funding.to_bytes().to_vec(),
                program_id,
                false,
            ),
            rent_account(),
        ];
        (
            accounts,
            AdvanceMarketOpeningReadinessV1::new(begin_instruction.generation(), 0),
        )
    }

    #[test]
    fn begin_authentication_refuses_wrong_pda_owner_hash_generation_and_child_replay() {
        let program_id = Pubkey::new_unique();
        let (accounts, instruction) = begin_fixture(program_id);
        let frame = BeginFrame::parse(&accounts).expect("frame");
        assert!(authenticate_begin(&program_id, &frame, instruction).is_ok());

        let (mut wrong_pda, instruction) = begin_fixture(program_id);
        wrong_pda[2].key = Box::leak(Box::new(Pubkey::new_unique()));
        let frame = BeginFrame::parse(&wrong_pda).expect("frame");
        assert!(matches!(
            authenticate_begin(&program_id, &frame, instruction),
            Err(ProgramError::Custom(code)) if code == AdapterError::AccountIdentity as u32
        ));

        let (mut wrong_owner, instruction) = begin_fixture(program_id);
        wrong_owner[4].owner = Box::leak(Box::new(bpf_loader::ID));
        let frame = BeginFrame::parse(&wrong_owner).expect("frame");
        assert!(matches!(
            authenticate_begin(&program_id, &frame, instruction),
            Err(ProgramError::Custom(code)) if code == AdapterError::AccountIdentity as u32
        ));

        let (wrong_hash, instruction) = begin_fixture(program_id);
        wrong_hash[3].try_borrow_mut_data().expect("manifest data")[0] ^= 1;
        let frame = BeginFrame::parse(&wrong_hash).expect("frame");
        assert!(authenticate_begin(&program_id, &frame, instruction).is_err());

        let (accounts, instruction) = begin_fixture(program_id);
        let frame = BeginFrame::parse(&accounts).expect("frame");
        assert!(
            authenticate_begin(
                &program_id,
                &frame,
                BeginMarketOpeningReadinessV1::new(instruction.generation() + 1, 0),
            )
            .is_err()
        );
        assert!(
            authenticate_begin(
                &program_id,
                &frame,
                BeginMarketOpeningReadinessV1::new(instruction.generation(), 1),
            )
            .is_err()
        );
    }

    #[test]
    fn begin_refuses_substitute_beneficiary_and_trailing_wire() {
        let program_id = Pubkey::new_unique();
        let (mut accounts, instruction) = begin_fixture(program_id);
        let sponsor = *accounts[0].key;
        let different = Pubkey::new_unique();
        let authority = RefundAuthority::new(different.to_bytes()).expect("authority");
        let (credit_key, bump) = Pubkey::find_program_address(
            &[RENT_CREDIT_PDA_DOMAIN_V1, different.as_ref()],
            &program_id,
        );
        accounts[4].key = Box::leak(Box::new(credit_key));
        accounts[4]
            .try_borrow_mut_data()
            .expect("credit data")
            .copy_from_slice(&RentCreditV1::new(authority, bump).to_bytes());
        let frame = BeginFrame::parse(&accounts).expect("frame");
        assert!(authenticate_begin(&program_id, &frame, instruction).is_err());
        assert_ne!(sponsor, different);

        let mut wire = instruction.to_bytes().to_vec();
        wire.push(0);
        assert_eq!(
            dispatch(&program_id, &accounts, &wire),
            Err(AdapterError::InvalidInstruction.into())
        );
    }

    #[test]
    fn advance_refuses_wrong_pda_underfunded_and_out_of_order_funding() {
        let program_id = Pubkey::new_unique();
        let (accounts, instruction) = advance_fixture(program_id);
        let frame = AdvanceFrame::parse(&accounts).expect("frame");
        // Unit-host syscall stubs do not supply Clock; the adapter intentionally
        // refuses instead of accepting a caller-provided slot.  The remaining
        // hostile cases below must also refuse before persistence.
        assert!(authenticate_advance(&program_id, &frame, instruction).is_err());

        let (mut wrong_pda, instruction) = advance_fixture(program_id);
        wrong_pda[1].key = Box::leak(Box::new(Pubkey::new_unique()));
        let frame = AdvanceFrame::parse(&wrong_pda).expect("frame");
        assert!(matches!(
            authenticate_advance(&program_id, &frame, instruction),
            Err(ProgramError::Custom(code)) if code == AdapterError::AccountIdentity as u32
        ));

        let (underfunded, instruction) = advance_fixture(program_id);
        {
            let mut lamports = underfunded[3].try_borrow_mut_lamports().expect("lamports");
            **lamports = 4;
        }
        let frame = AdvanceFrame::parse(&underfunded).expect("frame");
        assert!(authenticate_advance(&program_id, &frame, instruction).is_err());

        let (out_of_order, instruction) = advance_fixture(program_id);
        let frame = AdvanceFrame::parse(&out_of_order).expect("frame");
        assert!(
            authenticate_advance(
                &program_id,
                &frame,
                AdvanceMarketOpeningReadinessV1::new(instruction.generation(), 1),
            )
            .is_err()
        );
    }

    #[test]
    fn advance_postcondition_refuses_changed_funding_before_persisting_readiness() {
        let program_id = Pubkey::new_unique();
        let (accounts, _) = advance_fixture(program_id);
        let frame = AdvanceFrame::parse(&accounts).expect("frame");
        let (outcome_count, root) = authenticate_market(frame.market).expect("market");
        let readiness_data = frame.readiness.try_borrow_data().expect("readiness data");
        let readiness = MarketOpeningReadinessV1::decode(&readiness_data).expect("readiness");
        drop(readiness_data);
        let funding_data = frame.funding.try_borrow_data().expect("funding data");
        let funding = FundingStateV1::decode(&funding_data).expect("funding");
        drop(funding_data);
        let plan = AdvancePlan {
            readiness,
            market_root: root,
            outcome_count,
            funding,
            readiness_lamports: frame.readiness.lamports(),
            market_lamports: frame.market.lamports(),
            funding_lamports: frame.funding.lamports(),
        };
        {
            let mut funding_lamports = frame
                .funding
                .try_borrow_mut_lamports()
                .expect("funding lamports");
            **funding_lamports = plan.funding_lamports - 1;
        }
        assert!(matches!(
            persist_advance(&program_id, &frame, plan),
            Err(ProgramError::Custom(code)) if code == AdapterError::ReadinessPostcondition as u32
        ));
        let readiness_after = frame.readiness.try_borrow_data().expect("readiness data");
        assert_eq!(
            MarketOpeningReadinessV1::decode(&readiness_after),
            Ok(readiness)
        );
    }
}
