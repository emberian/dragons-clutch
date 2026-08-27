//! Family-neutral creation of the projected-Custody prestate a founding needs.
//!
//! Custody's `Initialize` and `OpenHoard` each require a signing
//! `ProjectedCustodyCallerSeedsV1` PDA derived under the Trading program, so no
//! wallet can drive them and only a Trading CPI can. Until this route existed
//! the sole in-tree constructor of those two requests was Series-shaped and had
//! no non-test caller, which left the atomic founding outer's Lock stage
//! demanding a `HoardOpen` replay that nothing could create.
//!
//! This route creates exactly that prestate and nothing else. It is bound to
//! one terminal `LockHoardAndCloseSource` request and one founding artifact,
//! and it authenticates their join with the same predicate the founding outer
//! uses, so a replay this route creates is admissible at Lock by construction
//! rather than by two constructors agreeing. Both transitions run in one
//! rollback domain: a Market is never left with a replay but no Hoard.
//!
//! No family, escrow shape, or ticket namespace enters. Every coordinate is
//! carried from the terminal request by
//! [`ProjectedCustodyRequestV1::founding_prestate_v1`], which varies exactly
//! the four transition fields Custody permits a successor to vary.
//!
//! Child CPI metas are built from this route's own authenticated frame. This is
//! a direct instruction, not an Effect-V3 route adapter, so it never consults a
//! downgraded privilege view.

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};

use dclutch_custody_contract::{
    INITIALIZE_RESULTING_REVISION_V1, OPEN_HOARD_RESULTING_REVISION_V1,
    PROJECTED_CUSTODY_INITIALIZE_ACCOUNT_COUNT_V1, PROJECTED_CUSTODY_OPEN_HOARD_ACCOUNT_COUNT_V1,
    PROJECTED_CUSTODY_REQUEST_BYTES_V1, PROJECTED_CUSTODY_STATE_BYTES_V1,
    ProjectedCustodyCallerSeedsV1, ProjectedCustodyOperationV1, ProjectedCustodyPhaseV1,
    ProjectedCustodyRequestV1, ProjectedCustodyStateV1,
};
use dclutch_market_core_codec::{
    GENERIC_FOUNDING_REQUEST_BYTES_V1, GenericFoundingRequestV1, GenericFoundingStageV1,
};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::TradingSbfError;
use crate::generic_market_founding_v1::authenticate_projected_lock_join_v1;

/// Sole top-level projected-Custody founding-bootstrap instruction.
pub const PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V1: [u8; 8] = *b"DCLTPCB1";
/// Exact outer instruction width. All economic bytes live in readonly accounts.
pub const PROJECTED_CUSTODY_BOOTSTRAP_INSTRUCTION_BYTES_V1: usize = 8;
/// Exact readonly raw-request prefix width.
pub const PROJECTED_CUSTODY_BOOTSTRAP_RAW_ACCOUNT_COUNT_V1: usize = 2;

const FOUND_RAW: usize = 0;
const LOCK_RAW: usize = 1;
const CUSTODY_PROGRAM: usize = 2;
const INITIALIZE_START: usize = 3;

/// Exact total physical frame width for the bootstrap route.
pub const PROJECTED_CUSTODY_BOOTSTRAP_ACCOUNT_COUNT_V1: usize = INITIALIZE_START
    + PROJECTED_CUSTODY_INITIALIZE_ACCOUNT_COUNT_V1
    + PROJECTED_CUSTODY_OPEN_HOARD_ACCOUNT_COUNT_V1;

// Indices shared by every projected-Custody physical frame.
const COMMON_CALLER: usize = 0;
const COMMON_STATE: usize = 1;
const COMMON_CACHE: usize = 2;
const COMMON_REGISTRY: usize = 3;
const COMMON_CALLER_PROGRAM: usize = 4;

// Initialize-specific indices.
const INITIALIZE_CORE_PROGRAM: usize = 7;
const INITIALIZE_PAYER: usize = 8;

// OpenHoard-specific indices.
const OPEN_HOARD_VAULT: usize = 7;
const OPEN_HOARD_PAYER: usize = 11;

/// Return whether bytes select the sole projected-Custody bootstrap route.
#[must_use]
pub fn is_projected_custody_bootstrap_v1(instruction_data: &[u8]) -> bool {
    instruction_data == PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V1
}

/// Create the projected replay and Hoard vault as one rollback domain.
#[inline(never)]
pub fn process_projected_custody_bootstrap_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if !is_projected_custody_bootstrap_v1(instruction_data) {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let frame = BootstrapFrameV1::parse(accounts)?;
    let found_raw = frame.raw_bytes(FOUND_RAW, GENERIC_FOUNDING_REQUEST_BYTES_V1)?;
    let lock_raw = frame.raw_bytes(LOCK_RAW, PROJECTED_CUSTODY_REQUEST_BYTES_V1)?;
    let found = decode_found_request(&found_raw)?;
    let lock = decode_projected_request(&lock_raw)?;

    let custody_program = frame.custody_program;
    let core_program = account(frame.initialize, INITIALIZE_CORE_PROGRAM)?;
    authenticate_programs(program_id, &frame, core_program, &lock)?;
    // Exactly the join the founding outer evaluates for this pair. Sharing the
    // predicate is what makes the prestate admissible at Lock: the two routes
    // cannot drift into disagreeing about what a founding's Lock request is.
    authenticate_projected_lock_join_v1(program_id, core_program.key, &found, &lock)?;

    let prestate = lock
        .founding_prestate_v1()
        .map_err(|_| TradingSbfError::Content)?;

    let initialize_raw = prestate
        .initialize
        .encode()
        .map_err(|_| TradingSbfError::Content)?;
    invoke_projected_child(
        program_id,
        custody_program,
        frame.initialize,
        &prestate.initialize,
        &initialize_raw,
        &[COMMON_STATE, INITIALIZE_PAYER],
        &[INITIALIZE_PAYER],
    )?;
    authenticate_poststate(
        account(frame.initialize, COMMON_STATE)?,
        custody_program,
        &prestate.initialize,
        &initialize_raw,
        ProjectedCustodyPhaseV1::Initialized,
        INITIALIZE_RESULTING_REVISION_V1,
    )?;

    let open_raw = prestate
        .open_hoard
        .encode()
        .map_err(|_| TradingSbfError::Content)?;
    invoke_projected_child(
        program_id,
        custody_program,
        frame.open_hoard,
        &prestate.open_hoard,
        &open_raw,
        &[COMMON_STATE, OPEN_HOARD_VAULT, OPEN_HOARD_PAYER],
        &[OPEN_HOARD_PAYER],
    )?;
    authenticate_poststate(
        account(frame.open_hoard, COMMON_STATE)?,
        custody_program,
        &prestate.open_hoard,
        &open_raw,
        ProjectedCustodyPhaseV1::HoardOpen,
        OPEN_HOARD_RESULTING_REVISION_V1,
    )?;
    Ok(())
}

struct BootstrapFrameV1<'accounts, 'info> {
    raw: &'accounts [AccountInfo<'info>],
    custody_program: &'accounts AccountInfo<'info>,
    initialize: &'accounts [AccountInfo<'info>],
    open_hoard: &'accounts [AccountInfo<'info>],
}

impl<'accounts, 'info> BootstrapFrameV1<'accounts, 'info> {
    #[inline(never)]
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != PROJECTED_CUSTODY_BOOTSTRAP_ACCOUNT_COUNT_V1 {
            return Err(TradingSbfError::Content.into());
        }
        let raw = subslice(accounts, 0, PROJECTED_CUSTODY_BOOTSTRAP_RAW_ACCOUNT_COUNT_V1)?;
        for (index, value) in raw.iter().enumerate() {
            if value.is_signer
                || value.is_writable
                || value.executable
                || raw
                    .get(..index)
                    .is_some_and(|prior| prior.iter().any(|other| other.key == value.key))
            {
                return Err(TradingSbfError::Content.into());
            }
        }
        let open_start = INITIALIZE_START
            .checked_add(PROJECTED_CUSTODY_INITIALIZE_ACCOUNT_COUNT_V1)
            .ok_or(TradingSbfError::Content)?;
        Ok(Self {
            raw,
            custody_program: account(accounts, CUSTODY_PROGRAM)?,
            initialize: subslice(
                accounts,
                INITIALIZE_START,
                PROJECTED_CUSTODY_INITIALIZE_ACCOUNT_COUNT_V1,
            )?,
            open_hoard: subslice(
                accounts,
                open_start,
                PROJECTED_CUSTODY_OPEN_HOARD_ACCOUNT_COUNT_V1,
            )?,
        })
    }

    fn raw_bytes(&self, index: usize, width: usize) -> Result<Vec<u8>, ProgramError> {
        let value = account(self.raw, index)?;
        if value.data_len() != width {
            return Err(TradingSbfError::Content.into());
        }
        value
            .try_borrow_data()
            .map(|data| data.to_vec())
            .map_err(|_| TradingSbfError::Content.into())
    }
}

/// Authenticate every Program identity this route hands a signature to.
///
/// The Custody program is taken from the Market-selected release set the
/// Registry has already activated, never from the caller's word for it, so a
/// substituted program cannot receive a Trading-derived caller signature. Both
/// child frames must name the same activation cache and Registry, so the two
/// transitions cannot be authenticated against different release sets.
#[inline(never)]
fn authenticate_programs(
    program_id: &Pubkey,
    frame: &BootstrapFrameV1<'_, '_>,
    core_program: &AccountInfo<'_>,
    lock: &ProjectedCustodyRequestV1,
) -> Result<(), ProgramError> {
    let cache = account(frame.initialize, COMMON_CACHE)?;
    let registry = account(frame.initialize, COMMON_REGISTRY)?;
    let caller_program = account(frame.initialize, COMMON_CALLER_PROGRAM)?;
    if !frame.custody_program.executable
        || frame.custody_program.is_signer
        || frame.custody_program.is_writable
        || !core_program.executable
        || !registry.executable
        || !caller_program.executable
        || caller_program.key != program_id
        || core_program.key.to_bytes() != lock.core_program
        || account(frame.open_hoard, COMMON_CACHE)?.key != cache.key
        || account(frame.open_hoard, COMMON_REGISTRY)?.key != registry.key
        || account(frame.open_hoard, COMMON_CALLER_PROGRAM)?.key != caller_program.key
        || account(frame.open_hoard, COMMON_STATE)?.key
            != account(frame.initialize, COMMON_STATE)?.key
    {
        return Err(TradingSbfError::Release.into());
    }
    if cache.key
        != &Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, &lock.release_set],
            registry.key,
        )
        .0
        || cache.owner != registry.key
    {
        return Err(TradingSbfError::Release.into());
    }
    let cache_data = cache
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache_data)
        .map_err(|_| TradingSbfError::Release)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| TradingSbfError::Release)?
        .as_bytes()
        != &lock.release_set
        || activated
            .role(ExecutionRoleV1::Custody)
            .map_err(|_| TradingSbfError::Release)?
            .release()
            .program()
            .as_bytes()
            != &frame.custody_program.key.to_bytes()
        || activated
            .role(ExecutionRoleV1::Core)
            .map_err(|_| TradingSbfError::Release)?
            .release()
            .program()
            .as_bytes()
            != &lock.core_program
        || activated
            .role(ExecutionRoleV1::Trading)
            .map_err(|_| TradingSbfError::Release)?
            .release()
            .program()
            .as_bytes()
            != &program_id.to_bytes()
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok(())
}

/// Invoke one projected-Custody transition under its single-use caller PDA.
///
/// Privileges come from this route's own authenticated frame, not from the
/// runtime's view of the incoming accounts: the writable and signer masks are
/// asserted, so a frame that under-privileges an account refuses here instead
/// of failing opaquely inside Custody. Index zero is always the caller PDA and
/// is the only account this program signs for.
#[inline(never)]
fn invoke_projected_child<'info>(
    program_id: &Pubkey,
    custody_program: &AccountInfo<'info>,
    accounts: &[AccountInfo<'info>],
    request: &ProjectedCustodyRequestV1,
    raw: &[u8],
    writable: &[usize],
    signers: &[usize],
) -> Result<(), ProgramError> {
    let digest = hash(raw).to_bytes();
    let seeds = ProjectedCustodyCallerSeedsV1::new(*request, digest);
    let (caller, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if account(accounts, COMMON_CALLER)?.key != &caller {
        return Err(TradingSbfError::Release.into());
    }
    let mut metas = Vec::with_capacity(accounts.len());
    for (index, value) in accounts.iter().enumerate() {
        let is_writable = writable.contains(&index);
        let is_signer = index == COMMON_CALLER || signers.contains(&index);
        if (is_writable && !value.is_writable)
            || (is_signer && index != COMMON_CALLER && !value.is_signer)
        {
            return Err(TradingSbfError::Content.into());
        }
        metas.push(if is_writable {
            AccountMeta::new(*value.key, is_signer)
        } else {
            AccountMeta::new_readonly(*value.key, is_signer)
        });
    }
    let instruction = Instruction {
        program_id: *custody_program.key,
        accounts: metas,
        data: raw.to_vec(),
    };
    let mut infos = accounts.to_vec();
    infos.push(custody_program.clone());
    let bump_seed = [bump];
    let [domain, release, market, root, context, request_digest] = seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[
            domain,
            release,
            market,
            root,
            context,
            request_digest,
            &bump_seed,
        ]],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    Ok(())
}

/// Join the persisted replay against the exact request that produced it.
///
/// Neither transition returns data, so the persisted state is the receipt. It
/// is read back from the Custody-owned account and required to be exactly the
/// poststate of the request this route just signed.
#[inline(never)]
fn authenticate_poststate(
    state_account: &AccountInfo<'_>,
    custody_program: &AccountInfo<'_>,
    request: &ProjectedCustodyRequestV1,
    raw: &[u8],
    phase: ProjectedCustodyPhaseV1,
    next_revision: u64,
) -> Result<(), ProgramError> {
    if state_account.owner != custody_program.key
        || state_account.data_len() != PROJECTED_CUSTODY_STATE_BYTES_V1
    {
        return Err(TradingSbfError::Transition.into());
    }
    let data = state_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Transition)?;
    let state = ProjectedCustodyStateV1::decode(&data).map_err(|_| TradingSbfError::Transition)?;
    if state.phase != phase
        || state.next_revision != next_revision
        || state.locked_amount != 0
        || state.last_request_digest != hash(raw).to_bytes()
        || state.request != *request
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(())
}

fn decode_found_request(bytes: &[u8]) -> Result<Box<GenericFoundingRequestV1>, ProgramError> {
    let request = GenericFoundingRequestV1::decode(bytes).map_err(|_| TradingSbfError::Content)?;
    if request.stage() != GenericFoundingStageV1::FoundAndPermit {
        return Err(TradingSbfError::Content.into());
    }
    Ok(Box::new(request))
}

fn decode_projected_request(bytes: &[u8]) -> Result<Box<ProjectedCustodyRequestV1>, ProgramError> {
    let request =
        ProjectedCustodyRequestV1::decode(bytes).map_err(|_| TradingSbfError::Content)?;
    if request.operation != ProjectedCustodyOperationV1::LockHoardAndCloseSource {
        return Err(TradingSbfError::Content.into());
    }
    Ok(Box::new(request))
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn subslice<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    start: usize,
    count: usize,
) -> Result<&'accounts [AccountInfo<'info>], ProgramError> {
    accounts
        .get(start..start.checked_add(count).ok_or(TradingSbfError::Content)?)
        .ok_or_else(|| TradingSbfError::Content.into())
}

#[cfg(test)]
mod tests {
    use dclutch_custody_contract::{CompartmentV1, PROJECTED_HOARD_CONTEXT_DOMAIN_V1};
    use dclutch_market_core_codec::Identity;
    use solana_program::hash::hashv;

    use super::*;

    fn id(byte: u8) -> Identity {
        Identity::new([byte; 32]).expect("identity")
    }

    fn trading() -> Pubkey {
        Pubkey::new_from_array([21; 32])
    }

    fn core() -> Pubkey {
        Pubkey::new_from_array([22; 32])
    }

    fn found() -> GenericFoundingRequestV1 {
        GenericFoundingRequestV1::new(
            GenericFoundingStageV1::FoundAndPermit,
            3,
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
            11,
            12,
            13,
            14,
            15,
            16,
            4,
            1,
        )
        .expect("found")
    }

    fn lock() -> ProjectedCustodyRequestV1 {
        let found = found();
        ProjectedCustodyRequestV1 {
            operation: ProjectedCustodyOperationV1::LockHoardAndCloseSource,
            caller_role: dclutch_custody_contract::ProjectedCallerRoleV1::TradingCapability,
            market: found.market().to_bytes(),
            generation: found.generation(),
            realm: [0x31; 32],
            product_record: [0x32; 32],
            product: [0x33; 32],
            source: [0x34; 32],
            release_set: found.release_set().to_bytes(),
            projection_receipt_digest: [0x35; 32],
            parent_capability_root: found.capability_root().to_bytes(),
            context_digest: hashv(&[
                PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
                found.context().to_bytes().as_slice(),
            ])
            .to_bytes(),
            caller_program: trading().to_bytes(),
            payer: [0x36; 32],
            core_program: core().to_bytes(),
            rent_program: [0x37; 32],
            refund_owner: found.beneficiary().to_bytes(),
            rent_credit: [0x38; 32],
            hoard_vault: found.hoard().to_bytes(),
            funding_source_vault: found.funding_source().to_bytes(),
            funding_source_context: found.context().to_bytes(),
            funding_source_compartment: CompartmentV1::SeriesEscrow,
            mint: [0x39; 32],
            token_program: [0x3a; 32],
            collateral_release: [0x3b; 32],
            expiry_slot: found.expiry_slot(),
            expected_revision: OPEN_HOARD_RESULTING_REVISION_V1,
            resulting_revision: OPEN_HOARD_RESULTING_REVISION_V1 + 1,
            amount: found.hoard_principal().expect("principal"),
            state_rent_lamports: 41,
            vault_rent_lamports: 42,
            funding_source_replay_revision: 43,
            funding_source_state_rent_lamports: 44,
            funding_source_vault_rent_lamports: 45,
        }
    }

    #[test]
    fn bootstrap_abi_is_data_account_only_and_frame_width_is_fixed() {
        assert!(is_projected_custody_bootstrap_v1(
            &PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V1
        ));
        assert!(!is_projected_custody_bootstrap_v1(&[0; 8]));
        // A prefix of the tag is not the tag: the route carries no payload.
        assert!(!is_projected_custody_bootstrap_v1(&[
            b'D', b'C', b'L', b'T', b'P', b'C', b'B', b'1', 0
        ]));
        assert_eq!(PROJECTED_CUSTODY_BOOTSTRAP_INSTRUCTION_BYTES_V1, 8);
        assert_eq!(PROJECTED_CUSTODY_BOOTSTRAP_ACCOUNT_COUNT_V1, 60);
    }

    #[test]
    fn only_the_terminal_lock_request_is_admitted() {
        let lock = lock();
        assert_eq!(
            decode_projected_request(&lock.encode().expect("bytes"))
                .expect("terminal")
                .operation,
            ProjectedCustodyOperationV1::LockHoardAndCloseSource
        );
        let mut open = lock;
        open.operation = ProjectedCustodyOperationV1::OpenHoard;
        open.expected_revision = INITIALIZE_RESULTING_REVISION_V1;
        open.resulting_revision = OPEN_HOARD_RESULTING_REVISION_V1;
        open.amount = 0;
        assert_eq!(
            decode_projected_request(&open.encode().expect("bytes")).err(),
            Some(TradingSbfError::Content.into())
        );
    }

    #[test]
    fn the_bootstrap_evaluates_the_founding_outers_own_lock_join() {
        let found = found();
        let lock = lock();
        assert_eq!(
            authenticate_projected_lock_join_v1(&trading(), &core(), &found, &lock),
            Ok(())
        );
        // The derived root is the only thing binding this replay's Custody
        // signer namespace to that Market, so a substituted one must refuse
        // before the bootstrap creates any state.
        let mut rerooted = lock;
        rerooted.parent_capability_root = [0x7b; 32];
        assert_eq!(
            authenticate_projected_lock_join_v1(&trading(), &core(), &found, &rerooted),
            Err(TradingSbfError::Content.into())
        );
        // So must a substituted Hoard vault, which is the account OpenHoard
        // creates and the account the Lock stage later credits.
        let mut rehoarded = lock;
        rehoarded.hoard_vault = [0x7c; 32];
        assert_eq!(
            authenticate_projected_lock_join_v1(&trading(), &core(), &found, &rehoarded),
            Err(TradingSbfError::Content.into())
        );
    }

    /// The route's two named hostile inputs, at the level they are decidable
    /// without a validator.
    ///
    /// A substituted caller-seeds account and a substituted Hoard vault are both
    /// refused twice over, and the second refusal is the one that matters: the
    /// caller authority is derived from the exact request bytes, so substituting
    /// any coordinate moves the signer. There is no signature in existence for a
    /// hostile request, so the CPI could not be made even if the frame check
    /// were bypassed. This is a pure derivation argument, not execution
    /// evidence; the on-chain rollback case still waits on a runnable outer.
    #[test]
    fn a_substituted_caller_or_hoard_vault_has_no_signature_in_existence() {
        let honest = lock();
        let caller = |request: ProjectedCustodyRequestV1| {
            let raw = request.encode().expect("bytes");
            Pubkey::find_program_address(
                &ProjectedCustodyCallerSeedsV1::new(request, hash(&raw).to_bytes()).as_slices(),
                &trading(),
            )
            .0
        };
        let honest_prestate = honest.founding_prestate_v1().expect("prestate");

        let mut rehoarded = honest;
        rehoarded.hoard_vault = [0x7c; 32];
        // Refusal one: the shared founding join owns the Hoard coordinate.
        assert_eq!(
            authenticate_projected_lock_join_v1(&trading(), &core(), &found(), &rehoarded),
            Err(TradingSbfError::Content.into())
        );
        // Refusal two: even reached, both of its prestates need signers that do
        // not exist, because the vault is a caller-seed input through the
        // request digest.
        let hostile_prestate = rehoarded.founding_prestate_v1().expect("hostile prestate");
        assert_ne!(
            caller(hostile_prestate.initialize),
            caller(honest_prestate.initialize)
        );
        assert_ne!(
            caller(hostile_prestate.open_hoard),
            caller(honest_prestate.open_hoard)
        );

        // Substituting the caller-seeds account directly is the same argument
        // read the other way: the route requires frame index zero to equal the
        // address derived from the request it is about to send, so any other
        // key refuses before the CPI, and the runtime would refuse the
        // signature after it.
        for hostile in [
            caller(honest_prestate.open_hoard),
            caller(honest),
            Pubkey::new_from_array([0x7d; 32]),
        ] {
            assert_ne!(hostile, caller(honest_prestate.initialize));
        }
    }

    #[test]
    fn each_prestate_has_its_own_single_use_caller_authority() {
        let lock = lock();
        let prestate = lock.founding_prestate_v1().expect("prestate");
        let caller = |request: ProjectedCustodyRequestV1| {
            let raw = request.encode().expect("bytes");
            Pubkey::find_program_address(
                &ProjectedCustodyCallerSeedsV1::new(request, hash(&raw).to_bytes()).as_slices(),
                &trading(),
            )
            .0
        };
        let initialize = caller(prestate.initialize);
        let open_hoard = caller(prestate.open_hoard);
        let terminal = caller(lock);
        assert_ne!(initialize, open_hoard);
        assert_ne!(initialize, terminal);
        assert_ne!(open_hoard, terminal);
    }
}
