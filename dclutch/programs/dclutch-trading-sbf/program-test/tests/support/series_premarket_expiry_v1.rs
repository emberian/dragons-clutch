//! Exact installation, authority joins, and poststate evidence for the
//! pre-Market Series Expire ProgramTest campaign.
//!
//! This module deliberately does not compile a Series release. The canonical
//! release owner returns [`SeriesSelectedActionV5`]; this support code checks
//! that selection against the actual physical instruction and accounts that
//! the real ELFs receive.

use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    hot_v3::{
        HOT_CONFIG_RAW_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_ROOT_ACCOUNT_V3, HOT_RUNTIME_CONFIG_COORDINATE_V3,
        HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3,
        HOT_RUNTIME_PORTFOLIO_COORDINATE_V3, HOT_RUNTIME_PRODUCT_COORDINATE_V3,
        HOT_RUNTIME_ROOT_COORDINATE_V3, HotExecutionEnvelopeV3,
    },
};
use dclutch_market::{
    Identity, SERIES_FOUNDING_PERMIT_BYTES_V1, SeriesFoundingPermitSeedsV1,
    SeriesUnallocatedPermitExpiryRequestV1,
};
use dclutch_operator::registry::hot_continuation_v1::{
    REGISTRY_HOT_CONTINUATION_PREFIX_ACCOUNTS_V1, TRADING_HOT_CONTINUATION_ADMISSION_ACCOUNT_V1,
};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_market::rent::lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2};
use dclutch_trading_sbf::series::expire_funding_artifacts_v5::SERIES_EXPIRE_PRECOMMIT_CALLER_COORDINATE_V5;
use dclutch_trading_sbf::series::instruction::{SeriesActionRequestV3, SeriesActionV3};
use dclutch_trading_sbf::series::release_v5::{
    SeriesOccurrenceAuthorityV5, SeriesSelectedActionV5,
};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk_ids::system_program;

/// One exact account declaration owned by the Series expiry fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesPremarketExpiryInstallAccountV1 {
    /// Exact account identity.
    pub key: Pubkey,
    /// Exact initial account state.
    pub account: Account,
    /// Whether a hostile transaction must preserve this account byte-for-byte.
    pub snapshot_for_rollback: bool,
}

/// Installed, Series-owned portion of the positive chain fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledSeriesPremarketExpiryV1 {
    /// Accounts actually added by this installer.
    pub installed_keys: Vec<Pubkey>,
    /// Complete material state set for hostile rollback snapshots.
    pub rollback_snapshot_keys: Vec<Pubkey>,
}

/// One exact observed account, including absence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesAccountSnapshotV1 {
    /// Observed account identity.
    pub key: Pubkey,
    /// Exact observed state, or absence from the bank.
    pub account: Option<Account>,
}

/// Ordered bank snapshot used by both success and rollback gates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesAccountSnapshotSetV1 {
    /// Exact observations in caller-requested order.
    pub accounts: Vec<SeriesAccountSnapshotV1>,
}

/// Exact expected account transition for the successful real-ELF campaign.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesExpectedAccountTransitionV1 {
    /// Material account identity.
    pub key: Pubkey,
    /// Exact required prestate, including absence.
    pub before: Option<Account>,
    /// Exact required poststate, including closure.
    pub after: Option<Account>,
}

/// Byte-exact replay replacement expectations produced by the Series kernel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesExpiryReplayExpectationV1 {
    /// Complete parent root bytes before Expire.
    pub root_before: Vec<u8>,
    /// Complete parent root bytes after Expire.
    pub root_after: Vec<u8>,
    /// Complete Ticket-state bytes before Expire.
    pub ticket_before: Vec<u8>,
    /// Complete Ticket-state bytes after Expire.
    pub ticket_after: Vec<u8>,
}

/// Concrete physical report consumed by the terminal adapter and evidence
/// writer after the positive ProgramTest succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesPremarketExpiryPhysicalReportV1 {
    /// Exact Trading Hot instruction before Registry continuation wrapping.
    pub hot_instruction: Instruction,
    /// Exact top-level Registry continuation instruction submitted to the bank.
    pub top_level_instruction: Instruction,
    /// Exact packed runtime account identities in physical ordinal order.
    pub runtime_physical_accounts: Vec<Pubkey>,
    /// Market authenticated by the parent Trading root header.
    pub parent_market: Pubkey,
    /// Parent Market generation authenticated by the root header.
    pub parent_generation: u64,
    /// Future Market authenticated by the Expire authority and finalized
    /// occurrence; always distinct from the Hot envelope's controller Market.
    pub future_market: Pubkey,
    /// Future Market generation authenticated by the Expire authority.
    pub future_generation: u64,
    /// Release set shared by parent root, envelope, and selected Expire action.
    pub release_set: [u8; 32],
    /// Exact parent Trading Series root.
    pub parent_root: Pubkey,
    /// Exact prepared Ticket replay account replaced by Expire.
    pub ticket_state: Pubkey,
    /// Canonical still-System-owned founding-permit PDA.
    pub permit_account: Pubkey,
    /// Immutable refund destination joined through the permit intent.
    pub rent_credit: Pubkey,
    /// Exact readonly Trading caller synthesized as a signer only for Core CPI.
    pub precommit_caller: Pubkey,
    /// Exact kernel-derived root and Ticket replay replacements.
    pub replay: SeriesExpiryReplayExpectationV1,
    /// Every material pre/poststate asserted after successful execution.
    pub success_transitions: Vec<SeriesExpectedAccountTransitionV1>,
    /// Complete material key set asserted byte-for-byte on hostile rollback.
    pub rollback_snapshot_keys: Vec<Pubkey>,
}

/// Inputs whose joins are authenticated into one physical report.
pub struct SeriesPremarketExpiryPhysicalInputV1 {
    /// Exact Registry program receiving the top-level continuation.
    pub registry_program: Pubkey,
    /// Exact selected Trading program.
    pub trading_program: Pubkey,
    /// Exact selected Core program deriving the permit PDA.
    pub core_program: Pubkey,
    /// Exact parent Series root key.
    pub parent_root: Pubkey,
    /// Complete parent Series root prestate bytes.
    pub parent_root_prestate: Vec<u8>,
    /// Exact prepared Ticket replay account.
    pub ticket_state: Pubkey,
    /// Exact still-vacant permit account.
    pub permit_account: Pubkey,
    /// Same-bank Rent parameters used to authenticate prefunding and the
    /// future-Market-scoped RentCredit.
    pub rent: Rent,
    /// Exact readonly Trading caller passed to the Core precommit child.
    pub precommit_caller: Pubkey,
    /// Exact Trading instruction.
    pub hot_instruction: Instruction,
    /// Exact Registry-wrapped top-level instruction.
    pub top_level_instruction: Instruction,
    /// Kernel-derived replay expectations.
    pub replay: SeriesExpiryReplayExpectationV1,
    /// Exact successful material transitions.
    pub success_transitions: Vec<SeriesExpectedAccountTransitionV1>,
    /// Exact hostile rollback material set.
    pub rollback_snapshot_keys: Vec<Pubkey>,
}

/// Stable support refusal; each variant names a distinct fixture authority
/// seam so a red ProgramTest does not degrade into an opaque assertion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesPremarketExpirySupportErrorV1 {
    /// An installed key was zero, repeated, or an unknown external account.
    InstallIdentity,
    /// A nonempty installed account was below the canonical Rent minimum.
    InstallRent,
    /// Selected action was not the exact canonical Expire authority.
    SelectedAuthority,
    /// Expanded logical-to-physical binding geometry was incomplete or invalid.
    PhysicalGeometry,
    /// Hot envelope, fixed root, or Registry continuation wrapper differed.
    Instruction,
    /// Parent root header differed from the selected release coordinates.
    Root,
    /// Permit or precommit-caller coordinates, account state, or PDA differed.
    Permit,
    /// Replay replacement bytes or transition declarations were incomplete.
    Replay,
    /// Snapshot key order or exact account state differed.
    Poststate,
}

/// Install exact Series-owned accounts while preserving explicitly named
/// release-waist, executable, ProgramData, sysvar, and System accounts.
pub fn install_series_premarket_expiry_accounts_v1(
    test: &mut ProgramTest,
    rent: &Rent,
    accounts: &[SeriesPremarketExpiryInstallAccountV1],
    externally_installed: &[Pubkey],
) -> Result<InstalledSeriesPremarketExpiryV1, SeriesPremarketExpirySupportErrorV1> {
    validate_install_identities(accounts, externally_installed)?;
    let mut installed_keys = Vec::with_capacity(accounts.len());
    let mut rollback_snapshot_keys = Vec::new();
    for candidate in accounts {
        if candidate.snapshot_for_rollback {
            rollback_snapshot_keys.push(candidate.key);
        }
        if externally_installed.contains(&candidate.key) {
            // The bank owns this account and this campaign installs nothing
            // over it, so its lamports are not this campaign's to fund. The
            // Rent gate below exists to stop the campaign installing an
            // account the runtime would reap; applied to a BANK-OWNED account
            // it refused the System builtin, whose twenty-one bytes of
            // registered name sit under one lamport and always will.
            continue;
        }
        if !candidate.account.data.is_empty()
            && candidate.account.lamports < rent.minimum_balance(candidate.account.data.len())
        {
            return Err(SeriesPremarketExpirySupportErrorV1::InstallRent);
        }
        test.add_account(candidate.key, candidate.account.clone());
        installed_keys.push(candidate.key);
    }
    Ok(InstalledSeriesPremarketExpiryV1 {
        installed_keys,
        rollback_snapshot_keys,
    })
}

/// Authenticate canonical selection, physical packing, Registry wrapping, root
/// coordinates, and the nested still-vacant permit into one report.
/// Name which of the ten `PhysicalGeometry` conjuncts refused.
///
/// One discriminant covered ten independent accusations, and the campaign that
/// reaches them is the one campaign in the tree with no chain to ask instead:
/// it refuses off chain, before a transaction exists, so a validator log cannot
/// narrow it. The wire cannot carry the distinction -- this is a support enum
/// the fixture reads, not a program error -- so the cause travels in a printed
/// line, which is where a reader looks first.
fn geometry_site_v1(site: &'static str) -> SeriesPremarketExpirySupportErrorV1 {
    std::eprintln!("Series Expire physical geometry refused: {site}");
    SeriesPremarketExpirySupportErrorV1::PhysicalGeometry
}

pub fn authenticate_series_premarket_expiry_physical_report_v1(
    selected: &SeriesSelectedActionV5,
    input: SeriesPremarketExpiryPhysicalInputV1,
) -> Result<SeriesPremarketExpiryPhysicalReportV1, SeriesPremarketExpirySupportErrorV1> {
    let (future_market, future_generation, release_set, parent_root) = match selected.authority {
        SeriesOccurrenceAuthorityV5::Expire {
            market,
            generation,
            release_set,
            parent_root,
        } => (market, generation, release_set, parent_root),
        _ => return Err(SeriesPremarketExpirySupportErrorV1::SelectedAuthority),
    };
    if selected.request_bytes.is_empty() || parent_root != input.parent_root.to_bytes() {
        return Err(SeriesPremarketExpirySupportErrorV1::SelectedAuthority);
    }

    let root_header_bytes = input
        .parent_root_prestate
        .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
        .ok_or(SeriesPremarketExpirySupportErrorV1::Root)?;
    let root_header = CapabilityRootHeaderV1::decode(root_header_bytes)
        .map_err(|_| SeriesPremarketExpirySupportErrorV1::Root)?;
    if root_header.release_set().to_bytes() != release_set
        || root_header.market() == future_market
        || input.parent_root_prestate.len() == CAPABILITY_ROOT_HEADER_BYTES_V1
    {
        return Err(SeriesPremarketExpirySupportErrorV1::Root);
    }

    let (envelope, family_request) =
        HotExecutionEnvelopeV3::split_instruction(&input.hot_instruction.data)
            .map_err(|_| SeriesPremarketExpirySupportErrorV1::Instruction)?;
    if input.hot_instruction.program_id != input.trading_program
        || family_request != selected.request_bytes.as_slice()
        || envelope.release_set() != release_set
        || envelope.market() != root_header.market()
        || envelope.generation() != root_header.generation()
        || envelope.root_prestate_digest() != hash(&input.parent_root_prestate).to_bytes()
    {
        return Err(SeriesPremarketExpirySupportErrorV1::Instruction);
    }
    let fixed_root = input
        .hot_instruction
        .accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or(SeriesPremarketExpirySupportErrorV1::Instruction)?;
    if fixed_root.pubkey != input.parent_root || !fixed_root.is_writable || fixed_root.is_signer {
        return Err(SeriesPremarketExpirySupportErrorV1::Instruction);
    }
    let fixed_market = input
        .hot_instruction
        .accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(SeriesPremarketExpirySupportErrorV1::Instruction)?;
    if fixed_market.pubkey.to_bytes() != root_header.market()
        || fixed_market.pubkey.to_bytes() == future_market
        || fixed_market.is_signer
    {
        return Err(SeriesPremarketExpirySupportErrorV1::Instruction);
    }

    let runtime = runtime_physical_accounts_v1(selected, &input.hot_instruction)?;
    validate_physical_bindings(selected, &runtime)?;
    let root_runtime = selected_role_key(selected.roles.root, selected, &runtime)?;
    let ticket_coordinate = selected
        .roles
        .ticket
        .ok_or_else(|| geometry_site_v1("selected action names no Ticket role"))?;
    let ticket_runtime = selected_role_key(ticket_coordinate, selected, &runtime)?;
    let rent_credit_coordinate = selected
        .roles
        .rent_credit
        .ok_or_else(|| geometry_site_v1("selected action names no RentCredit role"))?;
    let rent_credit = selected_role_key(rent_credit_coordinate, selected, &runtime)?;
    let precommit_caller_runtime = selected_role_key(
        SERIES_EXPIRE_PRECOMMIT_CALLER_COORDINATE_V5,
        selected,
        &runtime,
    )?;
    if root_runtime != input.parent_root
        || ticket_runtime != input.ticket_state
        || precommit_caller_runtime != input.precommit_caller
    {
        return Err(geometry_site_v1(
            "root, Ticket or precommit caller is not the runtime key the fixture installed",
        ));
    }

    validate_registry_wrapper(
        input.registry_program,
        &input.hot_instruction,
        &input.top_level_instruction,
    )?;

    let family = SeriesActionRequestV3::decode(&selected.request_bytes)
        .map_err(|_| SeriesPremarketExpirySupportErrorV1::Permit)?;
    let ticket_context = family
        .ticket()
        .ok_or(SeriesPremarketExpirySupportErrorV1::Permit)?;
    let permit_seeds = SeriesFoundingPermitSeedsV1::new(
        Identity::new(release_set).map_err(|_| SeriesPremarketExpirySupportErrorV1::Permit)?,
        Identity::new(future_market).map_err(|_| SeriesPremarketExpirySupportErrorV1::Permit)?,
        Identity::new(ticket_context.to_bytes())
            .map_err(|_| SeriesPremarketExpirySupportErrorV1::Permit)?,
    );
    let expected_permit =
        Pubkey::find_program_address(&permit_seeds.as_slices(), &input.core_program).0;
    let precommit_request = SeriesUnallocatedPermitExpiryRequestV1::new(
        family.expected_series_revision(),
        family.expected_ticket_revision(),
    )
    .encode();
    let caller_seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set,
        root_header.market(),
        ExecutionRoleV1::Trading,
        ticket_context.to_bytes(),
        hash(&precommit_request).to_bytes(),
    )
    .map_err(|_| SeriesPremarketExpirySupportErrorV1::Permit)?;
    let expected_precommit_caller =
        Pubkey::find_program_address(&caller_seeds.as_slices(), &input.trading_program).0;
    let permit_meta = input
        .hot_instruction
        .accounts
        .iter()
        .skip(HOT_FIXED_ACCOUNT_COUNT_V3)
        .find(|meta| meta.pubkey == input.permit_account)
        .ok_or(SeriesPremarketExpirySupportErrorV1::Permit)?;
    let permit_transition = declared_transition(&input, input.permit_account)?;
    let permit_before = permit_transition
        .before
        .as_ref()
        .ok_or(SeriesPremarketExpirySupportErrorV1::Permit)?;
    let caller_meta = input
        .hot_instruction
        .accounts
        .iter()
        .skip(HOT_FIXED_ACCOUNT_COUNT_V3)
        .find(|meta| meta.pubkey == input.precommit_caller)
        .ok_or(SeriesPremarketExpirySupportErrorV1::Permit)?;
    let caller_transition = declared_transition(&input, input.precommit_caller)?;
    let caller_before = caller_transition
        .before
        .as_ref()
        .ok_or(SeriesPremarketExpirySupportErrorV1::Permit)?;
    if family.action() != SeriesActionV3::Expire
        || expected_permit != input.permit_account
        || expected_precommit_caller != input.precommit_caller
        || !permit_meta.is_writable
        || permit_meta.is_signer
        || permit_before.owner != system_program::ID
        || !permit_before.data.is_empty()
        || permit_before.executable
        || permit_before.lamports < input.rent.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)
        || runtime
            .iter()
            .filter(|key| **key == input.permit_account)
            .count()
            != 1
        || caller_meta.is_writable
        || caller_meta.is_signer
        || caller_before.owner != system_program::ID
        || !caller_before.data.is_empty()
        || caller_before.executable
        || caller_transition.after.as_ref() != Some(caller_before)
        || runtime
            .iter()
            .filter(|key| **key == input.precommit_caller)
            .count()
            != 1
    {
        return Err(SeriesPremarketExpirySupportErrorV1::Permit);
    }
    let rent_credit_transition = declared_transition(&input, rent_credit)?;
    let rent_credit_before = rent_credit_transition
        .before
        .as_ref()
        .ok_or(SeriesPremarketExpirySupportErrorV1::Permit)?;
    let credit = LifecycleRentCreditV2::decode(&rent_credit_before.data)
        .map_err(|_| SeriesPremarketExpirySupportErrorV1::Permit)?;
    let credit_seeds = credit.pda_seeds();
    let credit_bump = [credit_seeds.bump()];
    let credit_market = credit_seeds.market().to_bytes();
    let credit_generation = credit_seeds.generation();
    let expected_credit = Pubkey::create_program_address(
        &[
            credit_seeds.domain(),
            &credit_market,
            &credit_generation,
            credit_bump.as_slice(),
        ],
        &rent_credit_before.owner,
    )
    .map_err(|_| SeriesPremarketExpirySupportErrorV1::Permit)?;
    if credit.market().to_bytes() != future_market
        || credit.release_set().to_bytes() != release_set
        || credit.generation() != future_generation
        || expected_credit != rent_credit
        || rent_credit_before.data.len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || !input
            .rent
            .is_exempt(rent_credit_before.lamports, LIFECYCLE_RENT_CREDIT_BYTES_V2)
    {
        return Err(SeriesPremarketExpirySupportErrorV1::Permit);
    }

    validate_replay_expectations(&input)?;
    validate_success_transition_keys(&input, future_market.into(), rent_credit)?;
    Ok(SeriesPremarketExpiryPhysicalReportV1 {
        hot_instruction: input.hot_instruction,
        top_level_instruction: input.top_level_instruction,
        runtime_physical_accounts: runtime,
        parent_market: Pubkey::new_from_array(root_header.market()),
        parent_generation: root_header.generation(),
        future_market: Pubkey::new_from_array(future_market),
        future_generation,
        release_set,
        parent_root: input.parent_root,
        ticket_state: input.ticket_state,
        permit_account: input.permit_account,
        rent_credit,
        precommit_caller: input.precommit_caller,
        replay: input.replay,
        success_transitions: input.success_transitions,
        rollback_snapshot_keys: input.rollback_snapshot_keys,
    })
}

/// Capture exact states, including absence, in caller-supplied order.
pub async fn capture_series_account_snapshots_v1(
    context: &mut ProgramTestContext,
    keys: &[Pubkey],
) -> Result<SeriesAccountSnapshotSetV1, BanksClientError> {
    let mut accounts = Vec::with_capacity(keys.len());
    for key in keys {
        accounts.push(SeriesAccountSnapshotV1 {
            key: *key,
            account: context.banks_client.get_account(*key).await?,
        });
    }
    Ok(SeriesAccountSnapshotSetV1 { accounts })
}

/// Assert the complete hostile material set was rolled back byte-for-byte.
pub fn assert_series_premarket_expiry_rollback_v1(
    report: &SeriesPremarketExpiryPhysicalReportV1,
    before: &SeriesAccountSnapshotSetV1,
    after: &SeriesAccountSnapshotSetV1,
) -> Result<(), SeriesPremarketExpirySupportErrorV1> {
    if before != after
        || before.accounts.len() != report.rollback_snapshot_keys.len()
        || before
            .accounts
            .iter()
            .zip(&report.rollback_snapshot_keys)
            .any(|(observed, expected)| observed.key != *expected)
    {
        return Err(SeriesPremarketExpirySupportErrorV1::Poststate);
    }
    Ok(())
}

/// Assert every declared successful transition exactly, then independently pin
/// the semantic root/Ticket replacement, permit closure/refund, and future
/// Market vacancy properties that distinguish pre-Market Expire from an
/// ordinary live-Market Hot execution.
pub fn assert_series_premarket_expiry_success_v1(
    report: &SeriesPremarketExpiryPhysicalReportV1,
    before: &SeriesAccountSnapshotSetV1,
    after: &SeriesAccountSnapshotSetV1,
) -> Result<(), SeriesPremarketExpirySupportErrorV1> {
    if before.accounts.len() != report.success_transitions.len()
        || after.accounts.len() != report.success_transitions.len()
    {
        return Err(SeriesPremarketExpirySupportErrorV1::Poststate);
    }
    for ((before_observed, after_observed), expected) in before
        .accounts
        .iter()
        .zip(&after.accounts)
        .zip(&report.success_transitions)
    {
        if before_observed.key != expected.key
            || after_observed.key != expected.key
            || before_observed.account != expected.before
            || after_observed.account != expected.after
        {
            return Err(SeriesPremarketExpirySupportErrorV1::Poststate);
        }
    }
    let root_before = required_snapshot(before, report.parent_root)?;
    let root_after = required_snapshot(after, report.parent_root)?;
    let ticket_before = required_snapshot(before, report.ticket_state)?;
    let ticket_after = required_snapshot(after, report.ticket_state)?;
    if root_before.data != report.replay.root_before
        || root_after.data != report.replay.root_after
        || ticket_before.data != report.replay.ticket_before
        || ticket_after.data != report.replay.ticket_after
        || root_before.owner != report.hot_instruction.program_id
        || root_after.owner != report.hot_instruction.program_id
        || ticket_before.owner != report.hot_instruction.program_id
        || ticket_after.owner != report.hot_instruction.program_id
        || root_before.lamports != root_after.lamports
        || ticket_before.lamports != ticket_after.lamports
    {
        return Err(SeriesPremarketExpirySupportErrorV1::Replay);
    }

    let permit_before = required_snapshot(before, report.permit_account)?;
    let permit_after = snapshot(after, report.permit_account)?;
    if permit_before.owner != system_program::ID
        || !permit_before.data.is_empty()
        || permit_before.lamports == 0
        || !is_system_vacant(permit_after)
        || permit_after.is_some_and(|account| account.lamports != 0)
    {
        return Err(SeriesPremarketExpirySupportErrorV1::Permit);
    }
    let rent_credit_before = required_snapshot(before, report.rent_credit)?;
    let rent_credit_after = required_snapshot(after, report.rent_credit)?;
    let caller_before = required_snapshot(before, report.precommit_caller)?;
    let caller_after = required_snapshot(after, report.precommit_caller)?;
    let mut closed_account_count = 0_u8;
    let mut exact_refund = 0_u64;
    for transition in &report.success_transitions {
        let Some(closed_before) = transition.before.as_ref() else {
            continue;
        };
        let closed_after = transition.after.as_ref();
        if closed_before.lamports != 0
            && closed_after.is_none_or(|account| {
                account.owner == system_program::ID
                    && account.data.is_empty()
                    && !account.executable
                    && account.lamports == 0
            })
        {
            closed_account_count = closed_account_count
                .checked_add(1)
                .ok_or(SeriesPremarketExpirySupportErrorV1::Poststate)?;
            exact_refund = exact_refund
                .checked_add(closed_before.lamports)
                .ok_or(SeriesPremarketExpirySupportErrorV1::Poststate)?;
        }
    }
    if rent_credit_after.lamports
        != rent_credit_before
            .lamports
            .checked_add(exact_refund)
            .ok_or(SeriesPremarketExpirySupportErrorV1::Poststate)?
        || closed_account_count != 5
        || rent_credit_after.owner != rent_credit_before.owner
        || rent_credit_after.data != rent_credit_before.data
        || rent_credit_after.executable != rent_credit_before.executable
        || caller_after != caller_before
        || caller_after.owner != system_program::ID
        || !caller_after.data.is_empty()
        || caller_after.executable
    {
        return Err(SeriesPremarketExpirySupportErrorV1::Poststate);
    }
    let market_before = snapshot(before, report.future_market)?;
    let market_after = snapshot(after, report.future_market)?;
    if market_before != market_after || !is_system_vacant(market_before) {
        return Err(SeriesPremarketExpirySupportErrorV1::Poststate);
    }
    Ok(())
}

/// Refuse an unset or duplicated install coordinate.
///
/// The System program's canonical address IS the all-zero pubkey, so the
/// `Pubkey::default()` rejection below has to be exempted for it explicitly —
/// the sibling chain module wrote the same rule WITHOUT that exemption and was
/// therefore unsatisfiable, refusing every list it could build until `1b8191f9`.
///
/// That repair carries the stronger formulation and this one should adopt it
/// if either is ever touched again: see
/// `series_premarket_expiry_chain_v1.rs::require_disjoint_install_accounts_v1`.
/// It separates the two cases by the ACCOUNT rather than by consulting an
/// external list — at the zero address only the System program is an
/// executable owned by the native loader — so it needs no `externally_installed`
/// argument to decide, and it caps the exemption at one entry so it covers a
/// single builtin rather than a class.
fn validate_install_identities(
    accounts: &[SeriesPremarketExpiryInstallAccountV1],
    externally_installed: &[Pubkey],
) -> Result<(), SeriesPremarketExpirySupportErrorV1> {
    let mut keys = Vec::with_capacity(accounts.len());
    for candidate in accounts {
        let external_system = candidate.key == system_program::ID
            && externally_installed.contains(&system_program::ID);
        if (candidate.key == Pubkey::default() && !external_system) || keys.contains(&candidate.key)
        {
            return Err(SeriesPremarketExpirySupportErrorV1::InstallIdentity);
        }
        keys.push(candidate.key);
    }
    for external in externally_installed {
        let canonical_system = *external == system_program::ID;
        if (*external == Pubkey::default() && !canonical_system) || !keys.contains(external) {
            return Err(SeriesPremarketExpirySupportErrorV1::InstallIdentity);
        }
    }
    Ok(())
}

/// Join the selected release's account geometry to the Hot instruction's
/// runtime account metas.
///
/// MEASURED 2026-09-01, and this is where the pre-Market Expire campaign now
/// stops: `runtime=39 geometry.physical=44 bindings=81 geometry.logical=81`.
/// The logical side agrees exactly and every one of the 44 declared physical
/// ordinals IS bound by some logical coordinate — none is unreferenced — so
/// `release_v5` does NOT over-declare. The instruction under-packs by five.
///
/// The five are named. Route 5 of the Expire profile is Core's
/// permit-expiry precommit frame, and coordinate = 55 + local index into
/// `ExpiryAccounts` (`core-sbf/src/series_permit_expiry.rs:46`), anchored by
/// four independent agreements: local 1 -> 33 `rent_credit`, local 14 -> 0
/// `root`, local 15 -> 70 aliasing the Ticket at 5, local 16 -> 71 aliasing the
/// Template at 1. That makes the missing coordinates
///
///   72 `template_staging`, 73 `occurrence_raw`, 74 `occurrence_staging`,
///   75 `ticket_raw`, 76 `ticket_staging`
///
/// — the finalized Series record raw/staging accounts Core needs to rebuild the
/// Expire request. The chain fixture never constructs them: it contains no
/// reference to any of those five names, so they are never created, installed
/// or packed, and the instruction ends at the precommit caller PDA
/// (`runtime[38] == input.precommit_caller`, verified) with the three builtins
/// shifted five ordinals early into 72..75.
///
/// So the repair belongs to the fixture, not to the release compiler.
/// Rebuild the exact runtime PHYSICAL vector the selected release declares,
/// from the instruction the bank actually receives.
///
/// The submitted account list is not the physical vector and never was:
/// `series_hot_v3.rs` assembles it as the 39 family-neutral fixed accounts,
/// then the action's ExecutionStrategy extras, then
/// `runtime_physical_accounts.skip(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3)` --
/// because physical ordinals 0..5 are the SAME accounts as five members of the
/// fixed prefix and the runtime never repeats them. `validate_injected_runtime`
/// is the authority for which five, and this reads the identical table.
///
/// Reading `accounts[HOT_FIXED_ACCOUNT_COUNT_V3..]` as the physical vector is
/// therefore short by exactly five for every action and every Template, which
/// is what `runtime=39` against `geometry.physical=44` was: not five accounts
/// this fixture failed to install -- all forty-four are installed and bound --
/// but five it declined to count, and an ordinal space shifted under every
/// role lookup by the same five.
fn runtime_physical_accounts_v1(
    selected: &SeriesSelectedActionV5,
    hot: &Instruction,
) -> Result<Vec<Pubkey>, SeriesPremarketExpirySupportErrorV1> {
    // Expire selects no ExecutionStrategy account: the operator's
    // `validate_strategy_accounts_v5` admits a nonzero count only for Consume,
    // and the caller has already refused every non-Expire authority.
    if selected.action != SeriesActionV3::Expire {
        return Err(geometry_site_v1(
            "physical reconstruction was asked for an action whose strategy extras it cannot count",
        ));
    }
    let tail = hot
        .accounts
        .get(HOT_FIXED_ACCOUNT_COUNT_V3..)
        .ok_or_else(|| geometry_site_v1("hot instruction has no runtime tail"))?;
    let mut runtime =
        Vec::with_capacity(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3.saturating_add(tail.len()));
    for (ordinal, fixed) in [
        (HOT_RUNTIME_ROOT_COORDINATE_V3, HOT_ROOT_ACCOUNT_V3),
        (HOT_RUNTIME_CONFIG_COORDINATE_V3, HOT_CONFIG_RAW_ACCOUNT_V3),
        (
            HOT_RUNTIME_PRODUCT_COORDINATE_V3,
            HOT_PRODUCT_RAW_ACCOUNT_V3,
        ),
        (
            HOT_RUNTIME_PORTFOLIO_COORDINATE_V3,
            HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        ),
        (
            HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3,
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        ),
    ] {
        if ordinal != runtime.len() {
            return Err(geometry_site_v1(
                "the injected runtime prefix table is not in physical ordinal order",
            ));
        }
        runtime.push(
            hot.accounts
                .get(fixed)
                .ok_or_else(|| {
                    geometry_site_v1("hot instruction omitted an injected fixed-prefix account")
                })?
                .pubkey,
        );
    }
    runtime.extend(tail.iter().map(|meta| meta.pubkey));
    Ok(runtime)
}

fn validate_physical_bindings(
    selected: &SeriesSelectedActionV5,
    runtime: &[Pubkey],
) -> Result<(), SeriesPremarketExpirySupportErrorV1> {
    if runtime.len() != selected.geometry.physical_accounts
        || selected.account_bindings.len() != selected.geometry.logical_accounts
    {
        std::eprintln!(
            "Series Expire physical geometry: runtime={} declared physical={} bindings={} declared logical={}",
            runtime.len(),
            selected.geometry.physical_accounts,
            selected.account_bindings.len(),
            selected.geometry.logical_accounts,
        );
        return Err(geometry_site_v1(
            "runtime account count differs from the release geometry",
        ));
    }
    let mut physical_seen = vec![false; runtime.len()];
    for (logical, binding) in selected.account_bindings.iter().enumerate() {
        let Some(seen) = physical_seen.get_mut(binding.physical_ordinal) else {
            return Err(geometry_site_v1(
                "a logical binding names a physical ordinal past the runtime tail",
            ));
        };
        if binding.logical != logical
            || binding.representative > binding.logical
            || selected
                .account_bindings
                .get(binding.representative)
                .is_none_or(|representative| {
                    representative.representative != binding.representative
                        || representative.physical_ordinal != binding.physical_ordinal
                })
        {
            return Err(geometry_site_v1(
                "a logical binding is out of order or disagrees with its representative",
            ));
        }
        *seen = true;
    }
    if physical_seen.iter().any(|seen| !seen) {
        return Err(geometry_site_v1(
            "a declared physical ordinal is bound by no logical coordinate",
        ));
    }
    Ok(())
}

fn selected_role_key(
    logical: u16,
    selected: &SeriesSelectedActionV5,
    runtime: &[Pubkey],
) -> Result<Pubkey, SeriesPremarketExpirySupportErrorV1> {
    let binding = selected
        .account_bindings
        .get(usize::from(logical))
        .ok_or_else(|| geometry_site_v1("a named role has no logical binding"))?;
    runtime
        .get(binding.physical_ordinal)
        .copied()
        .ok_or_else(|| geometry_site_v1("a bound logical coordinate has no runtime account"))
}

/// The wrapper is the TRANSPARENT `hot_continuation_v2` seam, so the top-level
/// data is the Trading Hot bytes and nothing else.
///
/// Trading requires exactly that: `authenticate_hot_invocation_v3` compares the
/// instructions-sysvar record of the top-level instruction against the bytes it
/// was handed, and any container header the Registry strips before the CPI is
/// therefore observable at the child as a difference. The legacy headered
/// `continuation_v1` container this fixture used to build reaches Trading and
/// refuses `NativeSignature` for that reason, which
/// `registry_hot_continuation.rs` asserts on purpose.
fn validate_registry_wrapper(
    registry_program: Pubkey,
    hot: &Instruction,
    top_level: &Instruction,
) -> Result<(), SeriesPremarketExpirySupportErrorV1> {
    if top_level.program_id != registry_program || top_level.data != hot.data {
        return Err(SeriesPremarketExpirySupportErrorV1::Instruction);
    }
    let expected_accounts = REGISTRY_HOT_CONTINUATION_PREFIX_ACCOUNTS_V1
        .checked_add(hot.accounts.len())
        .and_then(|width| width.checked_add(1))
        .ok_or(SeriesPremarketExpirySupportErrorV1::Instruction)?;
    if top_level.accounts.len() != expected_accounts {
        return Err(SeriesPremarketExpirySupportErrorV1::Instruction);
    }
    let prefix_admission = top_level
        .accounts
        .get(REGISTRY_HOT_CONTINUATION_PREFIX_ACCOUNTS_V1 - 1)
        .ok_or(SeriesPremarketExpirySupportErrorV1::Instruction)?;
    let child = top_level
        .accounts
        .get(REGISTRY_HOT_CONTINUATION_PREFIX_ACCOUNTS_V1..)
        .ok_or(SeriesPremarketExpirySupportErrorV1::Instruction)?;
    let child_admission = child
        .get(TRADING_HOT_CONTINUATION_ADMISSION_ACCOUNT_V1)
        .ok_or(SeriesPremarketExpirySupportErrorV1::Instruction)?;
    if prefix_admission != child_admission
        || prefix_admission.is_signer
        || prefix_admission.is_writable
    {
        return Err(SeriesPremarketExpirySupportErrorV1::Instruction);
    }
    let mut without_admission = child.to_vec();
    without_admission.remove(TRADING_HOT_CONTINUATION_ADMISSION_ACCOUNT_V1);
    if without_admission != hot.accounts {
        return Err(SeriesPremarketExpirySupportErrorV1::Instruction);
    }
    Ok(())
}

fn validate_replay_expectations(
    input: &SeriesPremarketExpiryPhysicalInputV1,
) -> Result<(), SeriesPremarketExpirySupportErrorV1> {
    if input.replay.root_before != input.parent_root_prestate
        || input.replay.root_after.len() != input.replay.root_before.len()
        || input.replay.ticket_before.is_empty()
        || input.replay.ticket_after.len() != input.replay.ticket_before.len()
        || input.replay.root_after == input.replay.root_before
        || input.replay.ticket_after == input.replay.ticket_before
    {
        return Err(SeriesPremarketExpirySupportErrorV1::Replay);
    }
    Ok(())
}

fn declared_transition<'a>(
    input: &'a SeriesPremarketExpiryPhysicalInputV1,
    key: Pubkey,
) -> Result<&'a SeriesExpectedAccountTransitionV1, SeriesPremarketExpirySupportErrorV1> {
    let mut matches = input
        .success_transitions
        .iter()
        .filter(|transition| transition.key == key);
    let transition = matches
        .next()
        .ok_or(SeriesPremarketExpirySupportErrorV1::Poststate)?;
    if matches.next().is_some() {
        return Err(SeriesPremarketExpirySupportErrorV1::Poststate);
    }
    Ok(transition)
}

fn validate_success_transition_keys(
    input: &SeriesPremarketExpiryPhysicalInputV1,
    future_market: Pubkey,
    rent_credit: Pubkey,
) -> Result<(), SeriesPremarketExpirySupportErrorV1> {
    let required = [
        input.parent_root,
        input.ticket_state,
        input.permit_account,
        future_market,
        rent_credit,
        input.precommit_caller,
    ];
    let mut keys = Vec::with_capacity(input.success_transitions.len());
    for transition in &input.success_transitions {
        if transition.key == Pubkey::default() || keys.contains(&transition.key) {
            return Err(SeriesPremarketExpirySupportErrorV1::Poststate);
        }
        keys.push(transition.key);
    }
    if required.iter().any(|key| !keys.contains(key))
        || input.rollback_snapshot_keys.is_empty()
        || input
            .rollback_snapshot_keys
            .iter()
            .any(|key| !keys.contains(key))
    {
        return Err(SeriesPremarketExpirySupportErrorV1::Poststate);
    }
    Ok(())
}

fn snapshot<'a>(
    set: &'a SeriesAccountSnapshotSetV1,
    key: Pubkey,
) -> Result<Option<&'a Account>, SeriesPremarketExpirySupportErrorV1> {
    let mut found = set.accounts.iter().filter(|value| value.key == key);
    let value = found
        .next()
        .ok_or(SeriesPremarketExpirySupportErrorV1::Poststate)?;
    if found.next().is_some() {
        return Err(SeriesPremarketExpirySupportErrorV1::Poststate);
    }
    Ok(value.account.as_ref())
}

fn required_snapshot<'a>(
    set: &'a SeriesAccountSnapshotSetV1,
    key: Pubkey,
) -> Result<&'a Account, SeriesPremarketExpirySupportErrorV1> {
    snapshot(set, key)?.ok_or(SeriesPremarketExpirySupportErrorV1::Poststate)
}

fn is_system_vacant(account: Option<&Account>) -> bool {
    account.is_none_or(|value| {
        value.owner == system_program::ID && value.data.is_empty() && !value.executable
    })
}

// Keep the external account comparison type pinned at the Solana ABI type the
// submitted instructions carry. This catches accidental migration to a local
// account descriptor truth.
const _: fn(AccountMeta) -> AccountMeta = |meta| meta;
